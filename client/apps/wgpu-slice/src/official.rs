//! Native official-site discovery. Downloads are inert, bounded, and never persisted.
use crate::{
    Result,
    game::{LoginProfile, server_endpoint},
    native,
};
use scraper::{Html, Selector};
use slither_client::profile_literals;
use std::{
    collections::BTreeMap,
    io::Read,
    net::{IpAddr, SocketAddr},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use tungstenite::Message;
use url::Url;

const PAGE: &str = "https://slither.io/";
const PAGE_LIMIT: usize = 256 * 1024;
const SCRIPT_LIMIT: usize = 1024 * 1024;
const DIRECTORY_LIMIT: usize = 256 * 1024;

pub struct Prepared {
    pub endpoint: String,
    pub profile: LoginProfile,
    pub directory: Option<Arc<DirectorySnapshot>>,
}

/// Publish completed directory data before handling Play cancellation/failure.
pub fn complete_picker_preparation(
    picker: &mut slither_client::server_picker::ServerPicker,
    cached: &mut Option<Arc<DirectorySnapshot>>,
    result: &Result<Prepared>,
    cancelled: bool,
) -> Result<()> {
    if let Ok(prepared) = result
        && let Some(snapshot) = &prepared.directory
    {
        picker
            .replace(&snapshot.servers, &snapshot.pings)
            .map_err(str::to_owned)?;
        *cached = Some(snapshot.clone());
    }
    if cancelled {
        picker.directory_failed("Directory acquisition cancelled");
    } else if let Err(error) = result {
        picker.directory_failed(error);
    }
    Ok(())
}

/// One preparation per Play, outside the event loop and before the session deadline starts.
pub struct Preparation {
    receiver: mpsc::Receiver<Result<Prepared>>,
    cancel: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Preparation {
    pub fn start(server: Option<String>) -> Result<Self> {
        Self::start_selected(server, None)
    }
    pub fn start_selected(server: Option<String>, id: Option<u16>) -> Result<Self> {
        Self::start_with_directory(server, id, None, None)
    }
    pub fn start_with_directory(
        server: Option<String>,
        id: Option<u16>,
        cached: Option<Arc<DirectorySnapshot>>,
        pending: Option<DirectoryPreparation>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let worker = thread::Builder::new()
            .name("official-discovery".into())
            .spawn(move || {
                let result = (|| {
                    // Reuse the picker's in-flight sweep instead of racing a new one.
                    let cached = if let Some(pending) = pending {
                        Some(Arc::new(pending.finish(&flag)?))
                    } else {
                        cached
                    };
                    prepare_cached_with(server.as_deref(), id, &flag, fetch, probe_clusters, cached)
                })();
                let _ = sender.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            receiver,
            cancel,
            worker: Some(worker),
        })
    }
    pub fn poll(&mut self) -> Option<Result<Prepared>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Discovery worker stopped".into())),
        }
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }
}
impl Drop for Preparation {
    fn drop(&mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err("Discovery cancelled".into())
    } else {
        Ok(())
    }
}

/// Directory/ping work has no login profile, game socket or session authority.
pub struct DirectorySnapshot {
    pub servers: Vec<Server>,
    pub pings: BTreeMap<u16, u32>,
}
pub struct DirectoryPreparation {
    receiver: mpsc::Receiver<Result<DirectorySnapshot>>,
    cancel: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl DirectoryPreparation {
    fn finish(self, cancel: &AtomicBool) -> Result<DirectorySnapshot> {
        loop {
            if let Err(error) = check_cancel(cancel) {
                self.cancel();
                return Err(error);
            }
            match self.receiver.recv_timeout(Duration::from_millis(20)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Directory worker stopped".into());
                }
            }
        }
    }
    pub fn start() -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let worker = thread::Builder::new()
            .name("server-directory".into())
            .spawn(move || {
                let result = directory_with(&flag, fetch, probe_clusters);
                let _ = sender.send(result);
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            receiver,
            cancel,
            worker: Some(worker),
        })
    }
    pub fn poll(&mut self) -> Option<Result<DirectorySnapshot>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err("Directory worker stopped".into())),
        }
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}
impl Drop for DirectoryPreparation {
    fn drop(&mut self) {
        self.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn directory_with(
    cancel: &Arc<AtomicBool>,
    mut get: impl FnMut(&Url, usize, &AtomicBool) -> Result<(Url, Inert)>,
    measure: impl FnOnce(&[Server], &Arc<AtomicBool>) -> BTreeMap<u16, u32>,
) -> Result<DirectorySnapshot> {
    check_cancel(cancel)?;
    let (page_url, page) = get(&Url::parse(PAGE).expect("constant URL"), PAGE_LIMIT, cancel)?;
    let script_url = client_script(
        std::str::from_utf8(&page.0).map_err(|_| "Invalid page encoding")?,
        &page_url,
    )?;
    drop(page);
    let (_, script) = get(&script_url, SCRIPT_LIMIT, cancel)?;
    let url =
        directory_url(std::str::from_utf8(&script.0).map_err(|_| "Invalid script encoding")?)?;
    drop(script);
    let (_, bytes) = get(&url, DIRECTORY_LIMIT, cancel)?;
    let servers = decode_directory(&bytes.0)?;
    drop(bytes);
    let pings = measure(&servers, cancel);
    check_cancel(cancel)?;
    Ok(DirectorySnapshot { servers, pings })
}

struct Inert(Vec<u8>);
impl Drop for Inert {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

fn site_url(url: &Url) -> Result<()> {
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str() != Some("slither.io")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.port().is_some()
    {
        return Err("Unexpected official-site URL".into());
    }
    Ok(())
}

fn admit_fetch_url(original: &Url, target: &Url) -> Result<()> {
    site_url(target)?;
    // Only the recorded homepage redirect or an originally HTTP resource can use HTTP.
    if target.scheme() == "http" && original.scheme() == "https" && original.as_str() != PAGE {
        return Err("Official resource redirect downgraded HTTPS".into());
    }
    Ok(())
}

fn read_decoded(
    reader: impl Read,
    limit: usize,
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Inert> {
    // ureq's BodyWithConfig limit is an encoded-body limit. Apply this cap after decoding.
    let mut reader = reader.take(limit as u64 + 1);
    let mut bytes = Inert(Vec::new());
    let mut chunk = Inert(vec![0; 8192]);
    loop {
        check_cancel(cancel)?;
        if Instant::now() >= deadline {
            return Err("Site request deadline".into());
        }
        let count = reader
            .read(&mut chunk.0)
            .map_err(|_| "Official site body read failed")?;
        check_cancel(cancel)?;
        if Instant::now() >= deadline {
            return Err("Site request deadline".into());
        }
        if bytes.0.len() + count > limit {
            return Err("Official site decoded body exceeds size limit".into());
        }
        if count == 0 {
            return Ok(bytes);
        }
        bytes.0.extend_from_slice(&chunk.0[..count]);
    }
}

/// The recorded homepage can redirect to HTTP; HTTPS script/directory URLs cannot.
fn fetch(url: &Url, limit: usize, cancel: &AtomicBool) -> Result<(Url, Inert)> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let original = url.clone();
    let mut url = url.clone();
    for _ in 0..=3 {
        check_cancel(cancel)?;
        admit_fetch_url(&original, &url)?;
        let left = deadline
            .checked_duration_since(Instant::now())
            .ok_or("Site request deadline")?;
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_global(Some(left))
            .build()
            .into();
        let mut response = agent
            .get(url.as_str())
            .header("User-Agent", native::USER_AGENT)
            .call()
            .map_err(|_| "Official site request failed")?;
        check_cancel(cancel)?;
        if response.status().is_redirection() {
            let location = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("Official site redirect lacks Location")?;
            url = url
                .join(location)
                .map_err(|_| "Invalid official site redirect")?;
            continue;
        }
        if response.status() != 200 {
            return Err("Official site returned a non-200 response".into());
        }
        let bytes = read_decoded(
            response.body_mut().with_config().reader(),
            limit,
            deadline,
            cancel,
        )?;
        return Ok((url, bytes));
    }
    Err("Too many official site redirects".into())
}

pub fn client_script(page: &str, page_url: &Url) -> Result<Url> {
    let html = Html::parse_document(page);
    let selector = Selector::parse("script[src]").expect("static selector");
    let mut found = None;
    for element in html.select(&selector) {
        let Some(src) = element.value().attr("src") else {
            continue;
        };
        let Ok(url) = page_url.join(src) else {
            continue;
        };
        if site_url(&url).is_err() {
            continue;
        }
        let Some(version) = url
            .path()
            .strip_prefix("/s/game")
            .and_then(|p| p.strip_suffix(".js"))
        else {
            continue;
        };
        if !version.is_empty()
            && version.bytes().all(|b| b.is_ascii_digit())
            && url.query().is_none()
            && found.replace(url).is_some()
        {
            return Err("Ambiguous official client script".into());
        }
    }
    found.ok_or_else(|| "Official page has no supported client script".into())
}

fn declaration<'a>(source: &'a [u8], name: &[u8], limit: usize) -> Result<&'a [u8]> {
    let mut found = None;
    for (offset, start) in source.windows(3).enumerate().filter(|(_, w)| *w == b"var") {
        let identifier = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'$';
        if offset > 0 && identifier(source[offset - 1]) {
            continue;
        }
        let tail = &source[offset + start.len()..];
        if !tail.first().is_some_and(u8::is_ascii_whitespace) {
            continue;
        }
        let tail = tail.trim_ascii_start();
        if !tail.starts_with(name) || tail.get(name.len()).is_some_and(|b| identifier(*b)) {
            continue;
        }
        let span = &source[offset..source.len().min(offset + limit)];
        let end = span
            .iter()
            .position(|b| *b == b';')
            .ok_or("Unsupported profile declaration")?
            + 1;
        // Requiring a unique literal also rejects marker text in comments/strings rather than guessing.
        if found.replace(&span[..end]).is_some() {
            return Err("Ambiguous profile declaration".into());
        }
    }
    found.ok_or_else(|| "Official script has no supported profile declaration".into())
}

pub fn profile(source: &[u8]) -> Result<LoginProfile> {
    if source.len() > SCRIPT_LIMIT {
        return Err("Official script size limit".into());
    }
    let client_version = profile_literals::client_version(declaration(
        source,
        b"client_version",
        profile_literals::VERSION_LIMIT,
    )?)
    .map_err(|_| "Unsupported client version literal")?;
    let mut result = LoginProfile {
        client_version,
        ..Default::default()
    };
    profile_literals::page_echo(
        declaration(source, b"cpw", profile_literals::PAGE_ECHO_LIMIT)?,
        &mut result.page_echo,
    )
    .map_err(|_| "Unsupported page echo literal")?;
    Ok(result)
}

pub fn directory_url(source: &str) -> Result<Url> {
    let mut found = None;
    for (offset, _) in source.match_indices("https://slither.io/i") {
        let tail = &source[offset + "https://slither.io/i".len()..];
        let digits = tail.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 || digits > 16 || !tail[digits..].starts_with(".txt") {
            continue;
        }
        let url = Url::parse(&source[offset..offset + "https://slither.io/i".len() + digits + 4])
            .map_err(|_| "Invalid directory URL")?;
        if found.as_ref().is_some_and(|previous| previous != &url) {
            return Err("Ambiguous directory URL".into());
        }
        found = Some(url);
    }
    found.ok_or_else(|| "Official script has no supported directory URL".into())
}

pub use slither_client::server_directory::{Server, decode_directory, select_server};
pub fn explicit_endpoint(server: &str) -> Result<String> {
    let endpoint = format!("ws://{server}/slither");
    let url = server_endpoint(&endpoint)?;
    // A port is mandatory even when URL normalization removes the default :80.
    let authority_port = server
        .rsplit_once(':')
        .ok_or("Expected --server host:port")?
        .1;
    if authority_port.is_empty()
        || !authority_port.bytes().all(|b| b.is_ascii_digit())
        || authority_port
            .parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .is_none()
        || server.contains('/')
    {
        return Err("Expected --server host:port".into());
    }
    Ok(url.into())
}

fn probe_clusters(servers: &[Server], cancel: &Arc<AtomicBool>) -> BTreeMap<u16, u32> {
    let mut clusters = BTreeMap::<u16, Vec<IpAddr>>::new();
    for server in servers.iter().rev() {
        let addresses = clusters.entry(server.cluster).or_default();
        if !addresses.contains(&server.address.ip()) {
            addresses.push(server.address.ip());
        }
    }
    let targets: Vec<_> = clusters
        .into_iter()
        .rev()
        .map(|(id, addresses)| {
            (
                id,
                addresses[(rand::random::<f64>() * addresses.len() as f64) as usize],
            )
        })
        .collect();
    let started = Instant::now();
    let first_reply = AtomicU64::new(u64::MAX);
    let next = AtomicUsize::new(0);
    let results = Mutex::new(BTreeMap::new());
    let remaining = || {
        let end = first_reply
            .load(Ordering::SeqCst)
            .saturating_add(2667)
            .min(7000);
        Duration::from_millis(end).checked_sub(started.elapsed())
    };
    thread::scope(|scope| {
        for _ in 0..targets.len().min(32) {
            scope.spawn(|| {
                while !cancel.load(Ordering::SeqCst) {
                    let Some(left) = remaining() else { break };
                    let Some(&(id, address)) = targets.get(next.fetch_add(1, Ordering::SeqCst))
                    else {
                        break;
                    };
                    let url = format!("ws://{}/ptc", SocketAddr::new(address, 80));
                    let Ok(mut socket) = native::connect(
                        &url,
                        cancel,
                        Instant::now() + left.min(Duration::from_secs(3)),
                    ) else {
                        continue;
                    };
                    let mut sent = Instant::now();
                    if !send_ping(&mut socket) {
                        continue;
                    }
                    let mut count = 0;
                    while remaining().is_some() && !cancel.load(Ordering::SeqCst) {
                        match socket.read() {
                            Ok(Message::Binary(bytes)) if bytes.as_ref() == [112] => {
                                let ms = (sent.elapsed().as_secs_f64() * 1000.0)
                                    .round()
                                    .min(f64::from(u32::MAX))
                                    as u32;
                                first_reply.fetch_min(
                                    started.elapsed().as_millis() as u64,
                                    Ordering::SeqCst,
                                );
                                results
                                    .lock()
                                    .expect("probe results")
                                    .entry(id)
                                    .and_modify(|old: &mut u32| *old = (*old).min(ms))
                                    .or_insert(ms);
                                count += 1;
                                if count == 3 {
                                    break;
                                }
                                sent = Instant::now();
                                if !send_ping(&mut socket) {
                                    break;
                                }
                            }
                            Ok(Message::Ping(_) | Message::Pong(_)) => {}
                            Err(tungstenite::Error::Io(e))
                                if e.kind() == std::io::ErrorKind::WouldBlock =>
                            {
                                let _ = socket.flush();
                                thread::sleep(Duration::from_millis(5));
                            }
                            _ => break,
                        }
                    }
                    let _ = socket.close(None);
                }
            });
        }
    });
    results.into_inner().expect("probe results")
}

fn send_ping(socket: &mut native::Socket) -> bool {
    // tungstenite owns a WouldBlock write, just as for normal session sends.
    match socket.send(Message::Binary(vec![112].into())) {
        Ok(()) => true,
        Err(tungstenite::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => true,
        Err(_) => false,
    }
}

pub fn prepare(server: Option<&str>, cancel: &Arc<AtomicBool>) -> Result<Prepared> {
    prepare_with(server, cancel, fetch, probe_clusters)
}

fn prepare_with(
    server: Option<&str>,
    cancel: &Arc<AtomicBool>,
    get: impl FnMut(&Url, usize, &AtomicBool) -> Result<(Url, Inert)>,
    measure: impl FnOnce(&[Server], &Arc<AtomicBool>) -> BTreeMap<u16, u32>,
) -> Result<Prepared> {
    prepare_selected_with(server, None, cancel, get, measure)
}
fn prepare_selected_with(
    server: Option<&str>,
    id: Option<u16>,
    cancel: &Arc<AtomicBool>,
    get: impl FnMut(&Url, usize, &AtomicBool) -> Result<(Url, Inert)>,
    measure: impl FnOnce(&[Server], &Arc<AtomicBool>) -> BTreeMap<u16, u32>,
) -> Result<Prepared> {
    prepare_cached_with(server, id, cancel, get, measure, None)
}
fn prepare_cached_with(
    server: Option<&str>,
    id: Option<u16>,
    cancel: &Arc<AtomicBool>,
    mut get: impl FnMut(&Url, usize, &AtomicBool) -> Result<(Url, Inert)>,
    measure: impl FnOnce(&[Server], &Arc<AtomicBool>) -> BTreeMap<u16, u32>,
    cached: Option<Arc<DirectorySnapshot>>,
) -> Result<Prepared> {
    let forced = server.map(explicit_endpoint).transpose()?;
    check_cancel(cancel)?;
    let (page_url, page) = get(&Url::parse(PAGE).expect("constant URL"), PAGE_LIMIT, cancel)?;
    let script_url = client_script(
        std::str::from_utf8(&page.0).map_err(|_| "Invalid page encoding")?,
        &page_url,
    )?;
    drop(page);
    let (_, script) = get(&script_url, SCRIPT_LIMIT, cancel)?;
    let profile = profile(&script.0)?;
    let directory = if forced.is_none() && cached.is_none() {
        Some(directory_url(
            std::str::from_utf8(&script.0).map_err(|_| "Invalid script encoding")?,
        )?)
    } else {
        None
    };
    drop(script);
    let snapshot = if let Some(cached) = cached {
        Some(cached)
    } else if forced.is_none() {
        let (_, bytes) = get(
            &directory.expect("automatic directory"),
            DIRECTORY_LIMIT,
            cancel,
        )?;
        let servers = decode_directory(&bytes.0)?;
        drop(bytes);
        let pings = measure(&servers, cancel);
        check_cancel(cancel)?;
        Some(Arc::new(DirectorySnapshot { servers, pings }))
    } else {
        None
    };
    let endpoint = if let Some(endpoint) = forced {
        endpoint
    } else {
        let snapshot = snapshot.as_ref().expect("automatic directory");
        if let Some(id) = id {
            let rows = slither_client::server_picker::ranked(&snapshot.servers, &snapshot.pings);
            let selected = rows
                .iter()
                .find(|r| r.server.id == id)
                .ok_or("Selected server is no longer in the directory")?;
            explicit_endpoint(&selected.server.address.to_string())?
        } else {
            snapshot.servers[select_server(&snapshot.servers, &snapshot.pings, rand::random())?]
                .endpoint()
        }
    };
    check_cancel(cancel)?;
    Ok(Prepared {
        endpoint,
        profile,
        directory: snapshot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener};
    // ORIGINAL SYNTHETIC page/script; no retained reference or acquisition authority.
    const SCRIPT: &[u8] =
        b"var client_version=4242; var cpw=[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19];";
    fn synthetic_directory() -> Vec<u8> {
        synthetic_ids([24, 42])
    }
    fn synthetic_ids(ids: impl IntoIterator<Item = u16>) -> Vec<u8> {
        let mut bytes = vec![b'a'];
        for id in ids {
            let mut record = [0u8; 28];
            record[1..5].copy_from_slice(&[192, 0, 2, 1]);
            record[21..23].copy_from_slice(&(400 + id).to_be_bytes());
            record[24] = 40;
            record[25] = 1;
            record[26..28].copy_from_slice(&id.to_be_bytes());
            for nibble in record.iter().flat_map(|b| [b >> 4, b & 15]) {
                let offset = bytes.len() - 1;
                bytes.push(b'a' + ((usize::from(nibble) + offset * 7) % 26) as u8);
            }
        }
        bytes
    }
    #[test]
    fn repeated_opens_and_cached_play_reuse_one_directory_and_ping_sweep() {
        use slither_client::server_picker::ServerPicker;
        use std::cell::Cell;
        let fetches = Cell::new(0);
        let sweeps = Cell::new(0);
        let bytes = synthetic_ids(1..=2000);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut get = |url: &Url, _: usize, _: &AtomicBool| {
            fetches.set(fetches.get() + 1);
            Ok((
                url.clone(),
                Inert(match url.path() {
                    "/" => b"<script src='/s/game999.js'></script>".to_vec(),
                    "/s/game999.js" => {
                        [SCRIPT, b"getData('https://slither.io/i999.txt',handler);"].concat()
                    }
                    "/i999.txt" => bytes.clone(),
                    _ => panic!("unexpected fetch"),
                }),
            ))
        };
        let mut picker = ServerPicker::default();
        picker.open();
        assert!(picker.take_directory_request());
        for _ in 0..3 {
            picker.close();
            picker.open();
            assert!(!picker.take_directory_request());
        }
        let snapshot = Arc::new(
            directory_with(&cancel, &mut get, |servers, _| {
                assert_eq!(servers.len(), 2000);
                sweeps.set(sweeps.get() + 1);
                BTreeMap::from([(1, 12)])
            })
            .unwrap(),
        );
        picker.replace(&snapshot.servers, &snapshot.pings).unwrap();
        for _ in 0..3 {
            picker.close();
            picker.open();
            if picker.take_directory_request() {
                directory_with(&cancel, &mut get, |_, _| {
                    sweeps.set(sweeps.get() + 1);
                    BTreeMap::new()
                })
                .unwrap();
            }
        }
        assert_eq!((fetches.get(), sweeps.get()), (3, 1));
        for choice in [None, Some(42), Some(24)] {
            let prepared = prepare_cached_with(
                None,
                choice,
                &cancel,
                &mut get,
                |_, _| panic!("cached Play must not ping"),
                Some(snapshot.clone()),
            )
            .unwrap();
            assert!(Arc::ptr_eq(prepared.directory.as_ref().unwrap(), &snapshot));
            if let Some(id) = choice {
                assert_eq!(
                    prepared.endpoint,
                    format!("ws://192.0.2.1:{}/slither", 400 + id)
                );
            }
        }
        assert_eq!((fetches.get(), sweeps.get()), (9, 1)); // Only fresh page/profile script per Play.
    }
    #[test]
    fn cancelled_play_does_not_strand_picker() {
        let mut picker = slither_client::server_picker::ServerPicker::default();
        picker.open();
        assert!(picker.take_directory_request());
        picker.status = "Measuring server latency...".into();
        picker.close();

        // Keep the sender alive: the production Play worker can only wait or cancel,
        // never proceed to its network effects in this regression.
        let (_sender, receiver) = mpsc::sync_channel(1);
        let directory_cancel = Arc::new(AtomicBool::new(false));
        let pending = DirectoryPreparation {
            receiver,
            cancel: directory_cancel.clone(),
            worker: None,
        };
        let preparation =
            Preparation::start_with_directory(None, None, None, Some(pending)).unwrap();
        preparation.cancel();
        let result = preparation
            .receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        assert!(preparation.cancelled());
        assert_eq!(result.as_ref().err().unwrap(), "Discovery cancelled");
        let mut cached = None;
        complete_picker_preparation(&mut picker, &mut cached, &result, preparation.cancelled())
            .unwrap();
        drop(preparation);
        assert!(directory_cancel.load(Ordering::SeqCst));
        assert!(cached.is_none());
        assert_ne!(picker.status, "Measuring server latency...");
        let mut requests = 0;
        for _ in 0..3 {
            picker.open();
            requests += usize::from(picker.take_directory_request());
            picker.close();
        }
        assert_eq!(requests, 1);
    }
    #[test]
    fn failed_directory_fetch_retries_once_on_reopen_without_a_loop() {
        let mut picker = slither_client::server_picker::ServerPicker::default();
        let cancel = Arc::new(AtomicBool::new(false));
        let mut fetches = 0;
        picker.open();
        for attempt in 1..=2 {
            assert!(picker.take_directory_request());
            let result = directory_with(
                &cancel,
                |_, _, _| {
                    fetches += 1;
                    Err("Injected fetch failure".into())
                },
                |_, _| panic!("failed fetch must not ping"),
            );
            picker.directory_failed(&result.err().unwrap());
            assert_eq!(fetches, attempt);
            for _ in 0..3 {
                picker.open();
                assert!(!picker.take_directory_request());
            }
            picker.close();
            picker.open();
        }
    }
    #[test]
    fn play_completion_caches_success_and_rearms_dropped_snapshot_errors() {
        for (id, cancelled) in [(Some(77), false), (Some(42), false), (Some(42), true)] {
            let mut picker = slither_client::server_picker::ServerPicker::default();
            picker.open();
            assert!(picker.take_directory_request());
            picker.close();
            let snapshot = Arc::new(DirectorySnapshot {
                servers: decode_directory(&synthetic_directory()).unwrap(),
                pings: BTreeMap::from([(1, 12)]),
            });
            // A missing selection fails after acquisition, dropping the snapshot.
            let result = prepare_cached_with(
                None,
                id,
                &Arc::new(AtomicBool::new(false)),
                |url, _, _| {
                    Ok((
                        url.clone(),
                        Inert(if url.path() == "/" {
                            b"<script src='/s/game999.js'></script>".to_vec()
                        } else {
                            SCRIPT.to_vec()
                        }),
                    ))
                },
                |_, _| panic!("completed acquisition must not ping"),
                Some(snapshot.clone()),
            );
            assert_eq!(result.is_ok(), id == Some(42));
            let mut cached = None;
            complete_picker_preparation(&mut picker, &mut cached, &result, cancelled).unwrap();
            if id == Some(42) {
                assert!(Arc::ptr_eq(cached.as_ref().unwrap(), &snapshot));
                assert_eq!(picker.rows.len(), 2);
                // A later failed Play cannot discard completed directory data.
                complete_picker_preparation(
                    &mut picker,
                    &mut cached,
                    &Err("Profile failed".into()),
                    false,
                )
                .unwrap();
            } else {
                assert!(cached.is_none());
                assert!(picker.rows.is_empty());
            }
            let mut requests = 0;
            for _ in 0..3 {
                picker.open();
                requests += usize::from(picker.take_directory_request());
                picker.close();
            }
            assert_eq!(requests, usize::from(id != Some(42)));
        }
    }
    #[test]
    fn play_can_take_the_existing_directory_worker_and_cancel_its_wait() {
        let (sender, receiver) = mpsc::sync_channel(1);
        sender
            .send(Ok(DirectorySnapshot {
                servers: decode_directory(&synthetic_directory()).unwrap(),
                pings: BTreeMap::from([(1, 12)]),
            }))
            .unwrap();
        let pending = DirectoryPreparation {
            receiver,
            cancel: Arc::new(AtomicBool::new(false)),
            worker: None,
        };
        assert_eq!(
            pending
                .finish(&AtomicBool::new(false))
                .unwrap()
                .servers
                .len(),
            2
        );
        let (_sender, receiver) = mpsc::sync_channel(1);
        let flag = Arc::new(AtomicBool::new(false));
        let pending = DirectoryPreparation {
            receiver,
            cancel: flag.clone(),
            worker: None,
        };
        assert!(pending.finish(&AtomicBool::new(true)).is_err());
        assert!(flag.load(Ordering::SeqCst));
    }
    #[test]
    fn picker_directory_effect_needs_no_profile_or_session_and_selected_play_uses_id() {
        let get = |url: &Url, _limit: usize, _cancel: &AtomicBool| {
            Ok((
                url.clone(),
                Inert(match url.path() {
                    "/" => b"<script src='/s/game999.js'></script>".to_vec(),
                    "/s/game999.js" => b"getData('https://slither.io/i999.txt',handler);".to_vec(),
                    "/i999.txt" => synthetic_directory(),
                    _ => panic!("unexpected effect"),
                }),
            ))
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let snapshot = directory_with(&cancel, get, |s, _| {
            assert_eq!(s.len(), 2);
            BTreeMap::from([(1, 12)])
        })
        .unwrap();
        assert_eq!(snapshot.servers[1].id, 42);
        assert_eq!(snapshot.pings[&1], 12);
        for (id, expected) in [(42, Some("ws://192.0.2.1:442/slither")), (77, None)] {
            let result = prepare_selected_with(
                None,
                Some(id),
                &cancel,
                |url, limit, cancel| {
                    let (url, mut bytes) = get(url, limit, cancel)?;
                    if url.path() == "/s/game999.js" {
                        bytes.0.extend_from_slice(SCRIPT);
                    }
                    Ok((url, bytes))
                },
                |_, _| BTreeMap::from([(1, 12)]),
            );
            match expected {
                Some(endpoint) => assert_eq!(result.unwrap().endpoint, endpoint),
                None => assert_eq!(
                    result.err().unwrap(),
                    "Selected server is no longer in the directory"
                ),
            }
        }
        cancel.store(true, Ordering::SeqCst);
        assert!(
            directory_with(
                &cancel,
                |_, _, _| panic!("cancelled before fetch"),
                |_, _| panic!("cancelled before probe")
            )
            .is_err()
        );
    }
    #[test]
    fn automatic_preparation_composes_page_profile_directory_and_selection() {
        let mut calls = Vec::new();
        let prepared = prepare_with(
            None,
            &Arc::new(AtomicBool::new(false)),
            |url, limit, _| {
                calls.push((url.to_string(), limit));
                let bytes = match calls.len() {
                    1 => b"<script src='/s/game999.js'></script>".to_vec(),
                    2 => [SCRIPT, b"getData('https://slither.io/i999.txt',handler);"].concat(),
                    3 => {
                        let mut record = [0u8; 28];
                        record[1..5].copy_from_slice(&[192, 0, 2, 1]);
                        record[21..23].copy_from_slice(&445u16.to_be_bytes());
                        record[24] = 42;
                        record[25] = 9;
                        let mut bytes = vec![b'a'];
                        for (i, nibble) in record.iter().flat_map(|b| [b >> 4, b & 15]).enumerate()
                        {
                            bytes.push(b'a' + ((usize::from(nibble) + i * 7) % 26) as u8);
                        }
                        bytes
                    }
                    _ => panic!("unexpected fetch"),
                };
                Ok((url.clone(), Inert(bytes)))
            },
            |servers, _| {
                assert_eq!(servers.len(), 1);
                assert_eq!(servers[0].cluster, 9);
                BTreeMap::from([(9, 12)])
            },
        )
        .unwrap();
        assert_eq!(prepared.endpoint, "ws://192.0.2.1:445/slither");
        assert_eq!(prepared.profile.client_version, 4242);
        assert_eq!(
            calls,
            [
                (PAGE.to_string(), PAGE_LIMIT),
                ("https://slither.io/s/game999.js".into(), SCRIPT_LIMIT),
                ("https://slither.io/i999.txt".into(), DIRECTORY_LIMIT)
            ]
        );
    }
    #[test]
    fn forced_server_fetches_page_profile_only_and_cancel_prevents_all_requests() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut calls = Vec::new();
        let prepared = prepare_with(
            Some("127.0.0.1:19491"),
            &cancel,
            |url, limit, _| {
                calls.push((url.to_string(), limit));
                let bytes = if calls.len() == 1 {
                    b"<script src='https://slither.io/s/game999.js'></script>".as_slice()
                } else {
                    SCRIPT
                };
                Ok((url.clone(), Inert(bytes.to_vec())))
            },
            |_, _| panic!("forced server must skip probes"),
        )
        .unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(prepared.endpoint, "ws://127.0.0.1:19491/slither");
        assert_eq!(prepared.profile.client_version, 4242);
        cancel.store(true, Ordering::SeqCst);
        assert!(
            prepare_with(
                None,
                &cancel,
                |_, _, _| panic!("cancelled"),
                |_, _| panic!("cancelled")
            )
            .is_err()
        );
    }
    #[test]
    fn malformed_profile_never_requests_directory_or_probes() {
        let mut calls = 0;
        let result = prepare_with(
            None,
            &Arc::new(AtomicBool::new(false)),
            |url, _, _| {
                calls += 1;
                Ok((
                    url.clone(),
                    Inert(if calls == 1 {
                        b"<script src='/s/game999.js'></script>".to_vec()
                    } else {
                        b"invalid".to_vec()
                    }),
                ))
            },
            |_, _| panic!("invalid profile"),
        );
        assert!(result.is_err());
        assert_eq!(calls, 2);
    }
    #[test]
    fn redirects_remain_canonical_without_credentials_or_alternate_ports() {
        for good in ["http://slither.io/?a", "https://slither.io/s/game999.js"] {
            site_url(&Url::parse(good).unwrap()).unwrap();
        }
        for bad in [
            "https://other.invalid/",
            "https://slither.io.evil.invalid/",
            "http://user@slither.io/",
            "https://slither.io:999/",
            "file:///tmp/script",
        ] {
            assert!(site_url(&Url::parse(bad).unwrap()).is_err());
        }
    }
    #[test]
    fn https_script_and_directory_cannot_downgrade_but_original_http_can() {
        for path in ["/s/game999.js", "/i999.txt"] {
            let https = Url::parse(&format!("https://slither.io{path}")).unwrap();
            let http = Url::parse(&format!("http://slither.io{path}")).unwrap();
            assert!(admit_fetch_url(&https, &http).is_err());
            assert!(admit_fetch_url(&https, &https).is_ok());
            assert!(admit_fetch_url(&http, &http).is_ok());
            assert!(admit_fetch_url(&http, &https).is_ok());
        }
        assert!(
            admit_fetch_url(
                &Url::parse(PAGE).unwrap(),
                &Url::parse("http://slither.io/?a").unwrap()
            )
            .is_ok()
        );
    }
    #[test]
    fn synthetic_gzip_expansion_is_bounded_after_ureq_decoding() {
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&vec![b'x'; 8 * 1024 * 1024]).unwrap();
        let compressed = gzip.finish().unwrap();
        assert!(compressed.len() < PAGE_LIMIT);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let peer = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    other => panic!("Synthetic gzip peer: {other:?}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut byte = [0];
                stream.read_exact(&mut byte).unwrap();
                request.extend(byte);
                assert!(request.len() < 8192);
            }
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Encoding: gzip\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", compressed.len()).unwrap();
            stream.write_all(&compressed).unwrap();
        });
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(5)))
            .build()
            .into();
        let mut response = agent
            .get(format!("http://{address}/synthetic"))
            .call()
            .unwrap();
        // The old .limit(PAGE_LIMIT).read_to_vec() admitted the compressed size then expanded 8 MiB.
        let result = read_decoded(
            response.body_mut().with_config().reader(),
            PAGE_LIMIT,
            Instant::now() + Duration::from_secs(2),
            &AtomicBool::new(false),
        );
        assert!(matches!(result, Err(ref error) if error.contains("decoded body exceeds")));
        peer.join().unwrap();
    }
    #[test]
    fn decoded_reads_accept_the_exact_cap_and_check_cancel_and_deadline() {
        let cancel = AtomicBool::new(false);
        let deadline = Instant::now() + Duration::from_secs(1);
        assert_eq!(
            read_decoded(&b"abc"[..], 3, deadline, &cancel).unwrap().0,
            b"abc"
        );
        assert!(read_decoded(&b"abcd"[..], 3, deadline, &cancel).is_err());
        assert!(read_decoded(&b"abc"[..], 3, Instant::now(), &cancel).is_err());
        struct Cancelling<'a>(&'a AtomicBool);
        impl Read for Cancelling<'_> {
            fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
                output[0] = b'x';
                self.0.store(true, Ordering::SeqCst);
                Ok(1)
            }
        }
        assert!(read_decoded(Cancelling(&cancel), 3, deadline, &cancel).is_err());
    }
}
