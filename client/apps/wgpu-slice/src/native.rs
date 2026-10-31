//! Native-only platform transport. No socket exists until an explicit Connect effect.
use crate::latency;
use crate::{
    Result,
    game::{Effect, EffectKind, server_endpoint},
};
use mio::{Events, Interest, Poll, Registry, Token, Waker};
use std::{
    io::{self, ErrorKind, Read, Write},
    net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use tungstenite::{
    Error, Message, WebSocket, client::IntoClientRequest, protocol::WebSocketConfig,
};
pub const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) SlitherClient/0.1";
pub const ORIGIN: &str = "http://slither.io";
pub enum Event {
    Opened(u32),
    Binary(u32, Vec<u8>),
    Closed(u32),
    Failed(u32, String),
}
pub(crate) type Socket = WebSocket<Wire>;
#[cfg(test)]
#[derive(Default)]
struct TestIo {
    read_bytes: usize,
    block_writes: bool,
    flushes_before_reset: Option<usize>,
}
/// An absolute handshake deadline bounds slow-drip peers and connector cancellation/join.
pub(crate) struct Wire {
    stream: TcpStream,
    ready: Option<mio::net::TcpStream>,
    write_blocked: bool,
    deadline: Option<Instant>,
    read_first: u64,
    read_last: u64,
    cancel: Arc<AtomicBool>,
    #[cfg(test)]
    test_io: TestIo,
}
impl Wire {
    fn budget(&self) -> io::Result<()> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(io::Error::new(ErrorKind::Interrupted, "Connect cancelled"));
        }
        if let Some(deadline) = self.deadline {
            let left = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| io::Error::new(ErrorKind::TimedOut, "Handshake deadline"))?;
            self.stream.set_read_timeout(Some(left))?;
            self.stream.set_write_timeout(Some(left))?;
        }
        Ok(())
    }
    fn ready(&mut self) -> io::Result<()> {
        self.budget()?;
        self.deadline = None;
        // Only the final handshake read can contain bytes after the HTTP header.
        self.read_first = self.read_last;
        self.stream.set_read_timeout(None)?;
        self.stream.set_write_timeout(None)?;
        self.stream.set_nonblocking(true)?;
        Ok(())
    }
    fn register(&mut self, registry: &Registry) -> io::Result<()> {
        let mut stream = mio::net::TcpStream::from_std(self.stream.try_clone()?);
        registry.register(&mut stream, Token(1), Interest::READABLE)?;
        self.ready = Some(stream);
        Ok(())
    }
}
impl Read for Wire {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.budget()?;
        let result = match &mut self.ready {
            Some(stream) => stream.read(b),
            None => self.stream.read(b),
        };
        if let Ok(count) = &result
            && *count > 0
        {
            let read = latency::raw_read(*count, self.deadline.is_some());
            if self.read_first == 0 {
                self.read_first = read;
            }
            self.read_last = read;
        }
        #[cfg(test)]
        if let Ok(count) = &result {
            self.test_io.read_bytes += *count;
        }
        result
    }
}
impl Write for Wire {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        self.budget()?;
        // Deterministic test backpressure without filling OS send buffers.
        #[cfg(test)]
        if self.test_io.block_writes {
            self.write_blocked = true;
            latency::write_blocked();
            return Err(ErrorKind::WouldBlock.into());
        }
        let result = match &mut self.ready {
            Some(stream) => stream.write(b),
            None => self.stream.write(b),
        };
        if let Ok(count) = &result {
            latency::write_completed(*count);
        }
        if matches!(&result, Err(e) if e.kind() == ErrorKind::WouldBlock) {
            self.write_blocked = true;
            latency::write_blocked();
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        self.budget()?;
        #[cfg(test)]
        if let Some(left) = &mut self.test_io.flushes_before_reset {
            if *left == 0 {
                return Err(ErrorKind::ConnectionReset.into());
            }
            *left -= 1;
        }
        let result = match &mut self.ready {
            Some(stream) => stream.flush(),
            None => self.stream.flush(),
        };
        if result.is_ok() {
            latency::flushed();
        }
        result
    }
}
struct Connector {
    receiver: mpsc::Receiver<Result<Socket>>,
    cancel: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
// No timer or socket I/O on this worker. Mio must own registered reads/writes on
// Windows to rearm IOCP readiness; Wire routes those through the registered clone.
struct Readiness {
    registry: Registry,
    shutdown: Waker,
    worker: Option<thread::JoinHandle<()>>,
    notify: Arc<dyn Fn() + Send + Sync>,
    error: Arc<Mutex<Option<String>>>,
    pending: Arc<AtomicU64>,
}
impl Readiness {
    fn new(notify: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        let mut poll = Poll::new().map_err(|e| e.to_string())?;
        let registry = poll.registry().try_clone().map_err(|e| e.to_string())?;
        let shutdown = Waker::new(poll.registry(), Token(0)).map_err(|e| e.to_string())?;
        let error = Arc::new(Mutex::new(None));
        let failure = error.clone();
        let wake = notify.clone();
        let trace = latency::worker();
        let generation = latency::current_generation();
        let pending = Arc::new(AtomicU64::new(0));
        let first_pending = pending.clone();
        let worker = thread::Builder::new()
            .name("ws-readiness".into())
            .spawn(move || {
                let _trace = trace.as_ref().map(|r| r.attach());
                latency::generation(generation);
                let mut events = Events::with_capacity(8);
                loop {
                    if let Err(e) = poll.poll(&mut events, None) {
                        if e.kind() == ErrorKind::Interrupted {
                            continue;
                        }
                        *failure.lock().expect("readiness error") = Some(e.to_string());
                        wake();
                        break;
                    }
                    if events.iter().any(|e| e.token() == Token(0)) {
                        break;
                    }
                    if !events.is_empty() {
                        // Retain only the first unserviced readiness observation, not an arrival claim.
                        let ns = latency::stamp();
                        if ns != 0
                            && first_pending
                                .compare_exchange(0, ns, Ordering::AcqRel, Ordering::Relaxed)
                                .is_ok()
                        {
                            let interest = events.iter().fold(0, |flags, event| {
                                flags
                                    | u64::from(event.is_readable())
                                    | (u64::from(event.is_writable()) << 1)
                            });
                            latency::event("ready", ns, 0, interest);
                        }
                        wake();
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            registry,
            shutdown,
            worker: Some(worker),
            notify,
            error,
            pending,
        })
    }
}
impl Drop for Readiness {
    fn drop(&mut self) {
        let _ = self.shutdown.wake();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[derive(Default)]
pub struct Transport {
    socket: Option<Socket>,
    connector: Option<Connector>,
    generation: u32,
    pending_bytes: usize,
    readiness: Option<Readiness>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    writable: bool,
    messages: std::collections::VecDeque<u64>,
}
fn would_block(e: &Error) -> bool {
    matches!(e,Error::Io(e)if e.kind()==ErrorKind::WouldBlock)
}

fn addresses(url: &url::Url, deadline: Instant) -> Result<Vec<SocketAddr>> {
    let port = url
        .port_or_known_default()
        .ok_or("Missing WebSocket port")?;
    match url.host().ok_or("Missing WebSocket host")? {
        url::Host::Ipv4(ip) => Ok(vec![SocketAddr::new(ip.into(), port)]),
        url::Host::Ipv6(ip) => Ok(vec![SocketAddr::new(ip.into(), port)]),
        url::Host::Domain(host) => {
            // OS DNS may block; its worker owns no socket and the connector waits at most 1s.
            let host = host.to_owned();
            let (tx, rx) = mpsc::sync_channel(1);
            thread::Builder::new()
                .name("ws-resolve".into())
                .spawn(move || {
                    let result = (host.as_str(), port)
                        .to_socket_addrs()
                        .map(|addresses| addresses.take(8).collect::<Vec<_>>())
                        .map_err(|_| "DNS lookup failed".to_string());
                    let _ = tx.send(result);
                })
                .map_err(|e| e.to_string())?;
            rx.recv_timeout(
                deadline
                    .saturating_duration_since(Instant::now())
                    .min(Duration::from_secs(1)),
            )
            .map_err(|_| "DNS deadline".to_string())?
        }
    }
}

pub(crate) fn connect(url: &str, cancel: &Arc<AtomicBool>, deadline: Instant) -> Result<Socket> {
    let parsed = url::Url::parse(url).map_err(|_| "Invalid WebSocket URL")?;
    if parsed.scheme() != "ws" {
        return Err("Native transport requires ws://".into());
    }
    let addresses = addresses(&parsed, deadline)?;
    let tcp_deadline = deadline.min(Instant::now() + Duration::from_secs(1));
    let mut stream = None;
    let count = addresses.len();
    for (index, address) in addresses.into_iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err("Connect cancelled".into());
        }
        let Some(left) = tcp_deadline.checked_duration_since(Instant::now()) else {
            break;
        };
        // An unreachable IPv6 candidate must leave time for the remaining IPv4 addresses.
        let slice = left / (count - index) as u32;
        if let Ok(connected) = TcpStream::connect_timeout(&address, slice) {
            stream = Some(connected);
            break;
        }
    }
    let stream = stream.ok_or("TCP connect failed/deadline")?;
    stream.set_nodelay(true).map_err(|e| e.to_string())?;
    let mut request = url
        .into_client_request()
        .map_err(|_| "Invalid WebSocket request")?;
    request
        .headers_mut()
        .insert("Origin", ORIGIN.parse().expect("constant header"));
    request
        .headers_mut()
        .insert("User-Agent", USER_AGENT.parse().expect("constant header"));
    let config = WebSocketConfig::default()
        .max_message_size(Some(1 << 20))
        .max_frame_size(Some(1 << 20))
        .write_buffer_size(0)
        .max_write_buffer_size(8192);
    let wire = Wire {
        stream,
        ready: None,
        write_blocked: false,
        deadline: Some(deadline.min(Instant::now() + Duration::from_secs(2))),
        read_first: 0,
        read_last: 0,
        cancel: cancel.clone(),
        #[cfg(test)]
        test_io: TestIo::default(),
    };
    let (mut socket, _) = tungstenite::client::client_with_config(request, wire, Some(config))
        .map_err(|e| e.to_string())?;
    socket.get_mut().ready().map_err(|e| e.to_string())?;
    Ok(socket)
}

impl Transport {
    /// Notify a sleeping host on connector completion and socket readiness.
    /// LocalGame and all WebSocket I/O remain on the caller's thread.
    pub fn with_wake(wake: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            socket: None,
            connector: None,
            generation: 0,
            pending_bytes: 0,
            readiness: None,
            wake: Some(Arc::new(wake)),
            writable: false,
            messages: std::collections::VecDeque::with_capacity(if latency::worker().is_some() {
                64
            } else {
                0
            }),
        }
    }
    fn interest(&mut self) -> Result<()> {
        // Reads can also queue automatic WebSocket Pong/Close writes. Those
        // need writable readiness even without an application Send effect.
        let writable = self.pending_bytes != 0
            || self
                .socket
                .as_ref()
                .is_some_and(|s| s.get_ref().write_blocked);
        if writable != self.writable {
            if let (Some(readiness), Some(socket)) = (&self.readiness, &mut self.socket) {
                readiness
                    .registry
                    .reregister(
                        socket.get_mut().ready.as_mut().expect("registered socket"),
                        Token(1),
                        if writable {
                            Interest::READABLE | Interest::WRITABLE
                        } else {
                            Interest::READABLE
                        },
                    )
                    .map_err(|e| e.to_string())?;
            }
            self.writable = writable;
        }
        Ok(())
    }
    pub fn execute(&mut self, e: &Effect) -> Result<()> {
        latency::generation(e.generation);
        latency::writing(e.id);
        match &e.kind {
            EffectKind::Connect(url) => {
                if self.socket.is_some() || self.connector.is_some() {
                    return Err("Transport already owned".into());
                }
                server_endpoint(url)?;
                if self.readiness.is_none()
                    && let Some(wake) = &self.wake
                {
                    self.readiness = Some(Readiness::new(wake.clone())?);
                }
                let url = url.clone();
                let (tx, receiver) = mpsc::sync_channel(1);
                let cancel = Arc::new(AtomicBool::new(false));
                let flag = cancel.clone();
                let notify = self.readiness.as_ref().map(|r| r.notify.clone());
                let trace = latency::worker();
                let generation = e.generation;
                let worker = thread::Builder::new()
                    .name("ws-connect".into())
                    .spawn(move || {
                        let _trace = trace.as_ref().map(|r| r.attach());
                        latency::generation(generation);
                        let result = connect(&url, &flag, Instant::now() + Duration::from_secs(3));
                        let _ = tx.send(result);
                        if let Some(notify) = notify {
                            notify();
                        }
                    })
                    .map_err(|e| e.to_string())?;
                self.generation = e.generation;
                self.connector = Some(Connector {
                    receiver,
                    cancel,
                    thread: Some(worker),
                });
                Ok(())
            }
            EffectKind::Send(bytes) => {
                if e.generation != self.generation || self.pending_bytes > 4096 {
                    return Err("Stale/full native send queue".into());
                }
                let socket = self.socket.as_mut().ok_or("Socket is not open")?;
                // tungstenite retains a WouldBlock write; never enqueue the same message again.
                match socket.write(Message::Binary(bytes.clone().into())) {
                    Ok(()) => {}
                    Err(error) if would_block(&error) => {
                        latency::event("backpressure", e.id, 0, 0);
                    }
                    Err(error) => return Err(error.to_string()),
                }
                latency::accepted(e.id);
                self.pending_bytes += bytes.len();
                match socket.flush() {
                    Ok(()) => {
                        self.pending_bytes = 0;
                        socket.get_mut().write_blocked = false;
                    }
                    Err(error) if would_block(&error) => {
                        latency::event("backpressure", e.id, 0, 0);
                    }
                    Err(error) => return Err(error.to_string()),
                }
                self.interest()
            }
            EffectKind::Close => {
                self.close();
                Ok(())
            }
        }
    }
    pub fn message_id(&mut self) -> u64 {
        self.messages.pop_front().unwrap_or(0)
    }
    pub fn poll(&mut self) -> Vec<Event> {
        latency::generation(self.generation);
        latency::writing(0);
        let poll = latency::poll();
        if poll != 0
            && let Some(r) = &self.readiness
        {
            let ready = r.pending.swap(0, Ordering::AcqRel);
            if ready != 0 {
                latency::event("poll_ready", poll, ready, 0);
            }
        }
        self.messages.clear();
        let mut events = Vec::new();
        let error = self
            .readiness
            .as_ref()
            .and_then(|r| r.error.lock().expect("readiness error").take());
        if let Some(error) = error {
            self.close();
            events.push(Event::Failed(self.generation, error));
            return events;
        }
        if let Some(c) = &self.connector {
            match c.receiver.try_recv() {
                Ok(result) => {
                    let mut c = self.connector.take().expect("connector");
                    if let Some(t) = c.thread.take() {
                        let _ = t.join();
                    }
                    match result {
                        Ok(mut socket) => {
                            if let Some(readiness) = &self.readiness
                                && let Err(e) = socket.get_mut().register(&readiness.registry)
                            {
                                self.readiness.take();
                                events.push(Event::Failed(self.generation, e.to_string()));
                                return events;
                            }
                            self.socket = Some(socket);
                            events.push(Event::Opened(self.generation));
                        }
                        Err(e) => events.push(Event::Failed(self.generation, e)),
                    }
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.connector.take();
                    events.push(Event::Failed(self.generation, "Connector stopped".into()));
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if let Some(socket) = &mut self.socket {
            match socket.flush() {
                Ok(()) => {
                    self.pending_bytes = 0;
                    socket.get_mut().write_blocked = false;
                }
                Err(e) if would_block(&e) => {
                    latency::event("flush_blocked", 0, 0, 0);
                }
                Err(e) => {
                    let error = e.to_string();
                    self.drop_socket();
                    self.readiness.take();
                    self.pending_bytes = 0;
                    events.push(Event::Failed(self.generation, error));
                    return events;
                }
            }
            for index in 0..64 {
                match socket.read() {
                    Ok(Message::Binary(bytes)) => {
                        let wire = socket.get_mut();
                        let message = latency::message_reads(wire.read_first, wire.read_last);
                        // Buffered messages may share the last raw read. Keep that uncertainty.
                        wire.read_first = wire.read_last;
                        if message != 0 {
                            self.messages.push_back(message);
                        }
                        events.push(Event::Binary(self.generation, bytes.to_vec()))
                    }
                    Ok(Message::Close(_)) => {
                        events.push(Event::Closed(self.generation));
                        break;
                    }
                    Ok(Message::Ping(_) | Message::Pong(_)) => {}
                    Ok(_) => {
                        events.push(Event::Failed(
                            self.generation,
                            "Binary protocol message required".into(),
                        ));
                        break;
                    }
                    Err(e) if would_block(&e) => break,
                    Err(Error::ConnectionClosed | Error::AlreadyClosed) => {
                        events.push(Event::Closed(self.generation));
                        break;
                    }
                    Err(e) => {
                        events.push(Event::Failed(self.generation, e.to_string()));
                        break;
                    }
                }
                // A bounded drain may stop before WouldBlock (including bytes
                // buffered by tungstenite). Continue via a wake, never lose an edge.
                if index == 63
                    && let Some(readiness) = &self.readiness
                {
                    (readiness.notify)();
                }
            }
        }
        if let Err(error) = self.interest() {
            events.push(Event::Failed(self.generation, error));
        }
        if events
            .iter()
            .any(|e| matches!(e, Event::Closed(_) | Event::Failed(_, _)))
        {
            if !events.iter().any(|e| matches!(e, Event::Failed(_, _))) {
                latency::closed_unflushed();
            }
            self.drop_socket();
            self.readiness.take();
            self.pending_bytes = 0;
        }
        events
    }
    /// Start a WebSocket close handshake; continue polling until Closed or a caller deadline.
    /// Unlike close(), this retains the socket to observe the peer's acknowledgement.
    pub fn begin_close(&mut self) -> Result<()> {
        let socket = self.socket.as_mut().ok_or("Socket is not open")?;
        match socket.close(None) {
            Ok(()) => {}
            Err(error) if would_block(&error) => {}
            Err(error) => return Err(error.to_string()),
        }
        self.interest()
    }
    pub fn close(&mut self) {
        latency::generation(self.generation);
        if let Some(mut c) = self.connector.take() {
            c.cancel.store(true, Ordering::SeqCst);
            if let Some(t) = c.thread.take() {
                let _ = t.join();
            }
        }
        if let Some(s) = &mut self.socket {
            let close_failed = s.close(None).is_err_and(|e| !would_block(&e));
            let flush_failed = s.flush().is_err_and(|e| !would_block(&e));
            let _ = s.get_ref().stream.shutdown(Shutdown::Both);
            // Shutdown stays best-effort for play, but fatal I/O must invalidate
            // analysis before pending sends are retired as normal terminal loss.
            if close_failed || flush_failed {
                latency::event(
                    "transport_failed",
                    0,
                    0,
                    u64::from(close_failed) + 2 * u64::from(flush_failed),
                );
            } else {
                latency::closed_unflushed();
            }
        }
        self.drop_socket();
        self.readiness.take();
        self.pending_bytes = 0;
    }
    fn drop_socket(&mut self) {
        latency::abandoned();
        if let Some(mut socket) = self.socket.take()
            && let (Some(readiness), Some(stream)) = (&self.readiness, &mut socket.get_mut().ready)
        {
            let _ = readiness.registry.deregister(stream);
        }
        self.writable = false;
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn wake_and_poll(
        transport: &mut Transport,
        wakes: &mpsc::Receiver<()>,
        deadline: Instant,
    ) -> Vec<Event> {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .expect("readiness deadline");
        wakes
            .recv_timeout(remaining)
            .expect("transport must wake without redraw polling");
        transport.poll()
    }

    fn waking_connection(endpoint: String) -> (Transport, mpsc::Receiver<()>) {
        let (notify, wakes) = mpsc::channel();
        let mut transport = Transport::with_wake(move || {
            let _ = notify.send(());
        });
        transport
            .execute(&Effect {
                generation: 1,
                id: 1,
                kind: EffectKind::Connect(endpoint),
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let events = wake_and_poll(&mut transport, &wakes, deadline);
            if !events.is_empty() {
                assert!(matches!(events.as_slice(), [Event::Opened(1)]));
                return (transport, wakes);
            }
        }
    }

    #[test]
    fn shutdown_reset_is_failed_and_would_block_has_a_normal_close_witness() {
        for resets_after in [Some(0), Some(1), None] {
            let trace = latency::Recorder::new(4096);
            let guard = trace.attach();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
            let (finish, finished) = mpsc::channel();
            let peer = thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let _socket = tungstenite::accept(stream).unwrap();
                finished.recv_timeout(Duration::from_secs(3)).unwrap();
            });
            let (mut transport, _) = waking_connection(endpoint);
            transport
                .socket
                .as_mut()
                .unwrap()
                .get_mut()
                .test_io
                .block_writes = true;
            transport
                .execute(&Effect {
                    generation: 1,
                    id: 2,
                    kind: EffectKind::Send(vec![42]),
                })
                .unwrap();
            let wire = transport.socket.as_mut().unwrap().get_mut();
            wire.test_io.block_writes = resets_after.is_none();
            wire.test_io.flushes_before_reset = resets_after;
            transport.close();
            transport.close(); // Drop/another shutdown must not invent another boundary.
            finish.send(()).unwrap();
            peer.join().unwrap();
            drop(guard);
            let (rows, dropped) = trace.snapshot();
            assert_eq!(dropped, 0);
            if let Some(after) = resets_after {
                let failed = rows.iter().find(|r| r.stage == "transport_failed").unwrap();
                assert_eq!(failed.detail, if after == 0 { 3 } else { 2 });
                assert!(
                    !rows
                        .iter()
                        .any(|r| matches!(r.stage, "normal_close" | "closed_unflushed"))
                );
            } else {
                let closes: Vec<_> = rows.iter().filter(|r| r.stage == "normal_close").collect();
                assert_eq!(closes.len(), 1);
                assert!(rows.iter().any(|r| r.stage == "closed_unflushed"
                    && r.id == 2
                    && r.related == closes[0].id));
                assert!(!rows.iter().any(|r| r.stage == "transport_failed"));
            }
        }
    }
    #[test]
    fn initiated_close_waits_for_the_peer_and_flushes_under_backpressure() {
        // ORIGINAL SYNTHETIC peer. A delayed echo distinguishes a completed
        // handshake from merely sending Close then dropping the TCP connection.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
        let (closed, observed) = mpsc::channel();
        let (reply, replies) = mpsc::channel();
        let peer = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            assert!(matches!(socket.read().unwrap(), Message::Close(None)));
            closed.send(()).unwrap();
            replies.recv_timeout(Duration::from_secs(3)).unwrap();
            match socket.flush() {
                Ok(()) | Err(Error::ConnectionClosed) => {}
                result => panic!("close reply: {result:?}"),
            }
        });
        let (mut transport, wakes) = waking_connection(endpoint);
        transport
            .socket
            .as_mut()
            .unwrap()
            .get_mut()
            .test_io
            .block_writes = true;
        transport.begin_close().unwrap();
        assert!(transport.writable);
        assert!(transport.socket.is_some());
        transport
            .socket
            .as_mut()
            .unwrap()
            .get_mut()
            .test_io
            .block_writes = false;
        assert!(transport.poll().is_empty());
        observed.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(transport.poll().is_empty());
        reply.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let events = wake_and_poll(&mut transport, &wakes, deadline);
            if !events.is_empty() {
                assert!(matches!(events.as_slice(), [Event::Closed(1)]));
                break;
            }
        }
        assert!(transport.socket.is_none());
        peer.join().unwrap();
    }

    #[test]
    fn partial_websocket_header_and_payload_resume_on_the_next_readiness() {
        // ORIGINAL SYNTHETIC server frame. Exercise both a split header and a
        // split payload; the peer cannot send the rest until the first read.
        for split in [1, 4] {
            let trace = latency::Recorder::new(4096);
            let buffer = trace.attach();
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
            let (advance, steps) = mpsc::channel();
            let peer = thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                stream.set_nodelay(true).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut socket = tungstenite::accept(stream).unwrap();
                let frame = [0x82, 4, 42, 43, 44, 45];
                for chunk in [&frame[..split], &[&frame[split..], &[0x82, 1, 99]].concat()] {
                    steps.recv_timeout(Duration::from_secs(2)).unwrap();
                    socket.get_mut().write_all(chunk).unwrap();
                }
                steps.recv_timeout(Duration::from_secs(2)).unwrap();
            });
            let (mut transport, wakes) = waking_connection(endpoint);
            let before = transport
                .socket
                .as_ref()
                .unwrap()
                .get_ref()
                .test_io
                .read_bytes;
            advance.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(2);
            while transport
                .socket
                .as_ref()
                .unwrap()
                .get_ref()
                .test_io
                .read_bytes
                < before + split
            {
                assert!(wake_and_poll(&mut transport, &wakes, deadline).is_empty());
            }
            assert_eq!(
                transport
                    .socket
                    .as_ref()
                    .unwrap()
                    .get_ref()
                    .test_io
                    .read_bytes,
                before + split
            );
            // A partial frame is retained, never emitted or reset on WouldBlock.
            assert!(transport.poll().is_empty());
            advance.send(()).unwrap();
            let mut received = Vec::new();
            let deadline = Instant::now() + Duration::from_secs(2);
            while received.len() < 2 {
                for event in wake_and_poll(&mut transport, &wakes, deadline) {
                    match event {
                        Event::Binary(1, bytes) => received.push(bytes),
                        _ => panic!("unexpected partial-frame event"),
                    }
                }
            }
            assert_eq!(received, [vec![42, 43, 44, 45], vec![99]]);
            assert!(transport.poll().is_empty());
            transport.close();
            advance.send(()).unwrap();
            peer.join().unwrap();
            drop(buffer);
            let (rows, dropped) = trace.snapshot();
            assert_eq!(dropped, 0);
            let messages: Vec<_> = rows.iter().filter(|r| r.stage == "message").collect();
            assert_eq!(messages.len(), 2);
            assert_ne!(messages[0].id, messages[1].id);
            assert_ne!(messages[0].related, messages[0].detail);
            for message in messages {
                for read in [message.related, message.detail] {
                    assert!(
                        rows.iter()
                            .any(|r| matches!(r.stage, "read" | "handshake_read")
                                && r.id == read
                                && r.ns <= message.ns)
                    );
                }
            }
        }
    }

    #[test]
    fn would_block_write_flushes_on_readiness_without_requeuing_the_message() {
        // ORIGINAL SYNTHETIC peer; injected WouldBlock avoids host-dependent
        // socket buffer saturation while exercising Transport's real send path.
        let trace = latency::Recorder::new(4096);
        let buffer = trace.attach();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
        let (received, packets) = mpsc::channel();
        let (finish, finished) = mpsc::channel();
        let peer = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            for _ in 0..2 {
                let Message::Binary(bytes) = socket.read().unwrap() else {
                    panic!("binary message required")
                };
                received.send(bytes.to_vec()).unwrap();
            }
            finished.recv_timeout(Duration::from_secs(2)).unwrap();
        });
        let (mut transport, wakes) = waking_connection(endpoint);
        transport
            .socket
            .as_mut()
            .unwrap()
            .get_mut()
            .test_io
            .block_writes = true;
        transport
            .execute(&Effect {
                generation: 1,
                id: 2,
                kind: EffectKind::Send(vec![42, 43]),
            })
            .unwrap();
        assert!(transport.socket.as_ref().unwrap().get_ref().write_blocked);
        assert_eq!(transport.pending_bytes, 2);
        assert!(transport.writable);
        // Accepted means tungstenite owns the bytes, even while Wire cannot write.
        latency::event("test_blocked_boundary", 2, 0, 0);
        transport
            .socket
            .as_mut()
            .unwrap()
            .get_mut()
            .test_io
            .block_writes = false;
        let deadline = Instant::now() + Duration::from_secs(2);
        while transport.pending_bytes != 0 {
            assert!(wake_and_poll(&mut transport, &wakes, deadline).is_empty());
        }
        assert!(!transport.writable);
        assert!(!transport.socket.as_ref().unwrap().get_ref().write_blocked);
        // A following marker proves the retained message was sent exactly once.
        transport
            .execute(&Effect {
                generation: 1,
                id: 3,
                kind: EffectKind::Send(vec![99]),
            })
            .unwrap();
        assert_eq!(
            packets.recv_timeout(Duration::from_secs(2)).unwrap(),
            [42, 43]
        );
        assert_eq!(packets.recv_timeout(Duration::from_secs(2)).unwrap(), [99]);
        assert!(packets.try_recv().is_err());
        transport.close();
        finish.send(()).unwrap();
        peer.join().unwrap();
        drop(buffer);
        let (rows, dropped) = trace.snapshot();
        assert_eq!(dropped, 0);
        let accepted = rows
            .iter()
            .position(|r| r.stage == "accepted" && r.id == 2)
            .unwrap();
        let blocked = rows
            .iter()
            .position(|r| r.stage == "test_blocked_boundary")
            .unwrap();
        let flushed = rows
            .iter()
            .position(|r| r.stage == "flushed" && r.id == 2)
            .unwrap();
        assert!(accepted < blocked && blocked < flushed);
        assert_eq!(
            rows.iter()
                .filter(|r| r.stage == "accepted" && r.id == 2)
                .count(),
            1
        );
        assert_eq!(
            rows.iter()
                .filter(|r| r.stage == "flushed" && r.id == 2)
                .count(),
            1
        );
    }

    #[test]
    fn readiness_wakes_without_frames_drains_past_budget_and_closes_cleanly() {
        // ORIGINAL SYNTHETIC WebSocket peer, not a game server. No winit/GPU.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
        let (commands, batches) = mpsc::channel::<usize>();
        let (sent, send_times) = mpsc::channel();
        let peer = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            while let Ok(count) = batches.recv_timeout(Duration::from_secs(2)) {
                let start = Instant::now();
                for index in 0..count {
                    socket
                        .send(Message::Binary(
                            (index as u16).to_be_bytes().to_vec().into(),
                        ))
                        .unwrap();
                }
                sent.send(start).unwrap();
            }
        });
        let (notify, wakes) = mpsc::channel();
        let mut transport = Transport::with_wake(move || {
            let _ = notify.send(Instant::now());
        });
        transport
            .execute(&Effect {
                generation: 1,
                id: 1,
                kind: EffectKind::Connect(endpoint.clone()),
            })
            .unwrap();
        loop {
            wakes
                .recv_timeout(Duration::from_secs(2))
                .expect("connector must wake a sleeping host");
            if transport
                .poll()
                .iter()
                .any(|e| matches!(e, Event::Opened(1)))
            {
                break;
            }
        }
        let mut max_ingest = Duration::ZERO;
        for count in [130, 1, 1, 1, 1, 1, 1, 1, 1] {
            commands.send(count).unwrap();
            let start = send_times.recv_timeout(Duration::from_secs(2)).unwrap();
            let mut received = Vec::new();
            while received.len() < count {
                wakes
                    .recv_timeout(Duration::from_secs(2))
                    .expect("readiness must wake without redraw polling");
                for event in transport.poll() {
                    match event {
                        Event::Binary(1, bytes) => {
                            received.push(u16::from_be_bytes(bytes.try_into().unwrap()))
                        }
                        _ => panic!("unexpected transport event"),
                    }
                }
                if received.len() == count {
                    max_ingest = max_ingest.max(start.elapsed());
                }
            }
            assert_eq!(received, (0..count as u16).collect::<Vec<_>>());
        }
        eprintln!(
            "synthetic send-to-drain max across 9 bursts: {:.3}ms (includes peer send, readiness worker and channel host; no winit/scanout)",
            max_ingest.as_secs_f64() * 1000.
        );
        drop(commands);
        peer.join().unwrap();
        loop {
            wakes
                .recv_timeout(Duration::from_secs(2))
                .expect("peer close must wake the idle host");
            if transport
                .poll()
                .iter()
                .any(|e| matches!(e, Event::Closed(1) | Event::Failed(1, _)))
            {
                break;
            }
        }
        assert!(
            transport.socket.is_none()
                && transport.connector.is_none()
                && transport.readiness.is_none()
        );
        // A new explicit session creates a fresh watcher; close is also idempotent.
        transport.close();
    }

    #[test]
    fn failed_connection_wakes_and_releases_worker() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
        drop(listener);
        let (notify, wakes) = mpsc::channel();
        let mut transport = Transport::with_wake(move || {
            let _ = notify.send(());
        });
        transport
            .execute(&Effect {
                generation: 4,
                id: 1,
                kind: EffectKind::Connect(endpoint),
            })
            .unwrap();
        wakes.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(transport.poll().as_slice(), [Event::Failed(4, _)]));
        assert!(
            transport.socket.is_none()
                && transport.connector.is_none()
                && transport.readiness.is_none()
        );
    }
    #[test]
    #[allow(
        clippy::result_large_err,
        reason = "tungstenite fixes the server callback error type"
    )]
    fn loopback_double_observes_browser_upgrade_headers_and_hostname_resolution() {
        // ORIGINAL SYNTHETIC peer; no official network or game data.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let peer = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == ErrorKind::WouldBlock && Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    other => panic!("Synthetic peer accept: {other:?}"),
                }
            };
            // Windows sockets accepted from a nonblocking listener stay nonblocking.
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut socket = tungstenite::accept_hdr(
                stream,
                |request: &tungstenite::handshake::server::Request, response| {
                    assert_eq!(request.uri().path(), "/slither");
                    assert_eq!(request.headers()["origin"], ORIGIN);
                    assert_eq!(request.headers()["user-agent"], USER_AGENT);
                    assert_eq!(request.headers()["host"], format!("localhost:{port}"));
                    assert_eq!(request.headers()["sec-websocket-version"], "13");
                    assert!(request.headers().contains_key("sec-websocket-key"));
                    assert_eq!(request.headers()["upgrade"], "websocket");
                    Ok(response)
                },
            )
            .unwrap();
            socket.send(Message::Binary(vec![42, 43].into())).unwrap();
        });
        let mut socket = connect(
            &format!("ws://localhost:{port}/slither"),
            &Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(3),
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match socket.read() {
                Ok(Message::Binary(bytes)) => {
                    assert_eq!(bytes.as_ref(), [42, 43]);
                    break;
                }
                Err(error) if would_block(&error) && Instant::now() < deadline => {
                    thread::sleep(Duration::from_millis(5))
                }
                other => panic!("Synthetic reply: {other:?}"),
            }
        }
        peer.join().unwrap();
    }
}
