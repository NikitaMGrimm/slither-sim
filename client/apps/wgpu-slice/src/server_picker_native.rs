//! Directory picker host binding and storage. No socket/session effects are executed here.
use crate::{Result, server_picker_egui::PickerUi};
use slither_client::{
    menu_controls::{ControlId, ControlIntent, PanelView},
    server_picker::*,
};
use std::{
    fs::{File, OpenOptions, TryLockError},
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

/// Shared picker policy plus its egui widget state. The native login owns the
/// egui host, so input and passes arrive through `NativeLogin`.
pub struct NativeServerPicker {
    pub policy: ServerPicker,
    pub ui: PickerUi,
    pub panel: PanelView,
    pub viewport: [f64; 2],
    wheel: f64,
    shown: bool,
}
impl Default for NativeServerPicker {
    fn default() -> Self {
        let mut policy = ServerPicker::default();
        Self {
            panel: policy.panel([960., 540.]),
            policy,
            ui: PickerUi::default(),
            viewport: [960., 540.],
            wheel: 0.,
            shown: false,
        }
    }
}
impl NativeServerPicker {
    pub fn sync(&mut self) {
        self.panel = self.policy.panel(self.viewport);
    }
    pub fn open(&mut self) {
        self.policy.open();
        self.ui.search.clear();
        self.wheel = 0.;
        self.shown = true;
        self.sync();
    }
    /// True once after the picker, including its exit fade, stops being visible,
    /// however it closed: by a pass, before any fade step, or directly.
    pub fn take_closed(&mut self) -> bool {
        let closed = self.shown && !self.policy.visible();
        if closed {
            self.shown = false;
        }
        closed
    }
    /// The startup choice seeds the address field; it is not a new Add.
    pub fn set_address(&mut self, address: &str) {
        self.ui.address = address.to_owned();
    }
    pub fn step(&mut self, now: f64) -> Result<()> {
        self.policy.step(now).map_err(str::to_owned)?;
        self.sync();
        Ok(())
    }
    /// Apply one pass's activations, then numeric search and a requested Add,
    /// in the order the shared panel input committed them.
    pub fn apply(&mut self, intents: Vec<ControlIntent>) {
        for intent in intents {
            self.policy.activate(intent);
        }
        if !self.ui.composing {
            self.policy.search(&self.ui.search);
        }
        // Add never saves provisional IME text.
        if self.policy.take_add_request() && !self.ui.composing {
            let result = normalize_custom_server(self.ui.address.trim())
                .and_then(|address| self.policy.add_custom(address).map_err(str::to_owned));
            match result {
                Ok(()) => {
                    self.policy.custom_error.clear();
                    self.ui.address.clear();
                }
                Err(error) => self.policy.custom_error = error,
            }
        }
        self.sync();
    }
    /// Physical wheel units: one line, or 46 physical pixels, per row.
    pub fn wheel(&mut self, rows: f64) {
        self.wheel += rows;
        let whole = self.wheel.trunc() as isize;
        self.wheel -= whole as f64;
        self.policy.scroll(whole);
        self.sync();
    }
    /// Arrow, Page and Home/End list navigation. Text fields keep caret keys:
    /// none in the address, Home/End in numeric search.
    pub fn navigate(&mut self, focus: Option<ControlId>, key: egui::Key) -> Option<ControlId> {
        use egui::Key as K;
        let delta = match key {
            K::ArrowDown => 1,
            K::ArrowUp => -1,
            K::PageDown => 6,
            K::PageUp => -6,
            K::Home => isize::MIN,
            K::End => isize::MAX,
            _ => return None,
        };
        if focus == Some(ADDRESS) || (focus == Some(SEARCH) && matches!(key, K::Home | K::End)) {
            return None;
        }
        let target = self.policy.navigate(focus, delta);
        self.sync();
        Some(target)
    }
}

pub fn preference_path() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|p| {
        PathBuf::from(p)
            .join("SlitherClient")
            .join("server-choice.txt")
    })
}
pub fn read_preference(path: &Path) -> ServerPreference {
    let mut bytes = String::new();
    let value = std::fs::File::open(path)
        .ok()
        .and_then(|file| {
            file.take((MAX_CUSTOM_ADDRESS + 33) as u64)
                .read_to_string(&mut bytes)
                .ok()
        })
        .filter(|_| bytes.len() <= MAX_CUSTOM_ADDRESS + 32)
        .map(|_| bytes.as_str());
    match ServerPreference::restore(value) {
        ServerPreference::Custom(address) => normalize_custom_server(&address)
            .map_or(ServerPreference::Automatic, ServerPreference::Custom),
        choice => choice,
    }
}
pub fn write_preference(path: &Path, value: &str) -> Result<()> {
    write_record(path, value)
}
fn write_record(path: &Path, value: &str) -> Result<()> {
    crate::menu_storage::atomic_write(
        path.parent().ok_or("Missing preference directory")?,
        path.file_name()
            .and_then(|n| n.to_str())
            .ok_or("Invalid preference name")?,
        value,
    )
}
/// Same URL/authority admission as --server; retain :80 in the user-owned record.
pub fn normalize_custom_server(value: &str) -> Result<String> {
    let result = (|| {
        let endpoint = crate::official::explicit_endpoint(value)?;
        let url = crate::game::server_endpoint(&endpoint)?;
        let authority = format!(
            "{}:{}",
            &url[url::Position::BeforeHost..url::Position::AfterHost],
            url.port_or_known_default().ok_or("Missing port")?
        );
        if authority.len() > MAX_CUSTOM_ADDRESS {
            return Err("Address too long".into());
        }
        Ok(authority)
    })();
    result.map_err(|_: String| "Enter a valid host:port".into())
}
const CUSTOM_FILE_LIMIT: usize = MAX_CUSTOM_SERVERS * (MAX_CUSTOM_ADDRESS + 1) + 2;
const CUSTOM_LOCK_WAIT: Duration = Duration::from_millis(200);
const CUSTOM_MEMORY_ONLY: &str = "Storage busy or unavailable; saved for this run only";
enum SaveStep {
    Read,
    Contended,
}
pub struct CustomServerStorage {
    path: PathBuf,
    lock_wait: Duration,
}
impl CustomServerStorage {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock_wait: CUSTOM_LOCK_WAIT,
        }
    }
    /// Merge user operations with the latest bounded record, not a startup snapshot.
    /// Unreadable/malformed storage retains the caller's in-memory fallback.
    /// Lock failure edits memory only and returns a status for the picker.
    pub fn apply(
        &self,
        operation: &CustomServerOperation,
        current: &mut Vec<String>,
    ) -> Result<()> {
        self.apply_with(operation, current, |_| {})
    }
    // Observe actual read/contention boundaries for the deterministic overlap regression.
    fn apply_with(
        &self,
        operation: &CustomServerOperation,
        current: &mut Vec<String>,
        mut observe: impl FnMut(SaveStep),
    ) -> Result<()> {
        let _lock = match self.lock(|| observe(SaveStep::Contended)) {
            Ok(lock) => lock,
            Err(_) => {
                apply_custom_operation(operation, current)?;
                return Err(CUSTOM_MEMORY_ONLY.into());
            }
        };
        let mut servers = current.clone();
        read_custom(&self.path, &mut servers);
        observe(SaveStep::Read);
        let result = apply_custom_operation(operation, &mut servers);
        *current = servers;
        result?;
        write_custom(&self.path, current)
    }
    fn lock(&self, mut contended: impl FnMut()) -> std::io::Result<File> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("Missing store directory"))?;
        std::fs::create_dir_all(parent)?;
        // Keep the sidecar: deleting/replacing it could let instances lock different files.
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.path.with_extension("lock"))?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(file),
                Err(TryLockError::Error(error)) => return Err(error),
                Err(TryLockError::WouldBlock) => {
                    contended();
                    let remaining = self.lock_wait.saturating_sub(started.elapsed());
                    if remaining.is_zero() {
                        return Err(std::io::ErrorKind::TimedOut.into());
                    }
                    std::thread::sleep(remaining.min(Duration::from_millis(10)));
                }
            }
        }
    }
}
fn apply_custom_operation(
    operation: &CustomServerOperation,
    servers: &mut Vec<String>,
) -> Result<()> {
    match operation {
        CustomServerOperation::Add(address) => {
            let address = normalize_custom_server(address)?;
            if !servers.contains(&address) {
                if servers.len() >= MAX_CUSTOM_SERVERS {
                    return Err("Custom server limit: 32".into());
                }
                servers.push(address);
            }
        }
        CustomServerOperation::Remove(address) => servers.retain(|s| s != address),
    }
    Ok(())
}
/// Invalid/missing/oversized data never replaces the caller's in-memory list.
pub fn read_custom(path: &Path, current: &mut Vec<String>) -> bool {
    let read = (|| -> Option<Vec<String>> {
        let mut value = String::new();
        std::fs::File::open(path)
            .ok()?
            .take((CUSTOM_FILE_LIMIT + 1) as u64)
            .read_to_string(&mut value)
            .ok()?;
        if value.len() > CUSTOM_FILE_LIMIT {
            return None;
        }
        let mut lines = value.lines();
        if lines.next()? != "1" {
            return None;
        }
        let mut servers = Vec::new();
        for line in lines {
            let address = normalize_custom_server(line).ok()?;
            if servers.len() == MAX_CUSTOM_SERVERS || servers.contains(&address) {
                return None;
            }
            servers.push(address);
        }
        Some(servers)
    })();
    if let Some(servers) = read {
        *current = servers;
        true
    } else {
        false
    }
}
pub fn write_custom(path: &Path, servers: &[String]) -> Result<()> {
    if servers.len() > MAX_CUSTOM_SERVERS {
        return Err("Custom server limit".into());
    }
    let mut value = String::from("1\n");
    let mut seen = std::collections::BTreeSet::new();
    for server in servers {
        let address = normalize_custom_server(server)?;
        if !seen.insert(address.clone()) {
            return Err("Duplicate custom server".into());
        }
        value.push_str(&address);
        value.push('\n');
    }
    write_record(path, &value)
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    use std::sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
        mpsc,
    };

    fn store_path() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "custom-lock-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        root.join("custom-servers.txt")
    }
    fn remove_store(path: &Path) {
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(path.with_extension("lock")).unwrap();
        std::fs::remove_dir(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn overlapping_saves_lock_the_entire_read_apply_replace() {
        let path = store_path();
        write_custom(&path, &[]).unwrap();
        let read = Arc::new(Barrier::new(2));
        let (contended, observed) = mpsc::channel();
        let first_path = path.clone();
        let first_read = read.clone();
        let first = std::thread::spawn(move || {
            let storage = CustomServerStorage::new(first_path);
            let mut current = Vec::new();
            storage
                .apply_with(
                    &CustomServerOperation::Add("192.0.2.1:80".into()),
                    &mut current,
                    |step| {
                        if matches!(step, SaveStep::Read) {
                            first_read.wait();
                            // Do not finish the first save until the second save really tries its lock.
                            observed.recv_timeout(Duration::from_secs(2)).unwrap();
                        }
                    },
                )
                .unwrap();
        });
        let second_path = path.clone();
        let second = std::thread::spawn(move || {
            // The first save finishes after contention is signalled; slow CI file replacement
            // can outlast the production wait, which the timeout test covers separately.
            let storage = CustomServerStorage {
                lock_wait: Duration::from_secs(10),
                ..CustomServerStorage::new(second_path)
            };
            let mut current = Vec::new();
            let mut contended = Some(contended);
            read.wait();
            storage
                .apply_with(
                    &CustomServerOperation::Add("192.0.2.2:80".into()),
                    &mut current,
                    |step| {
                        if matches!(step, SaveStep::Contended)
                            && let Some(sender) = contended.take()
                        {
                            sender.send(()).unwrap();
                        }
                    },
                )
                .unwrap();
        });
        first.join().unwrap();
        second.join().unwrap();
        let mut saved = Vec::new();
        assert!(read_custom(&path, &mut saved));
        assert_eq!(saved.len(), 2);
        assert!(saved.contains(&"192.0.2.1:80".into()));
        assert!(saved.contains(&"192.0.2.2:80".into()));
        // Both operations release the sidecar without deleting it.
        drop(CustomServerStorage::new(path.clone()).lock(|| {}).unwrap());
        remove_store(&path);
    }

    #[test]
    fn contended_lock_times_out_and_edits_only_memory() {
        let path = store_path();
        let original = vec!["192.0.2.1:80".to_owned()];
        write_custom(&path, &original).unwrap();
        let held = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.with_extension("lock"))
            .unwrap();
        held.lock().unwrap();
        for (operation, expected) in [
            (
                CustomServerOperation::Add("192.0.2.2:80".into()),
                vec!["192.0.2.1:80", "192.0.2.2:80"],
            ),
            (CustomServerOperation::Remove("192.0.2.1:80".into()), vec![]),
        ] {
            let storage = CustomServerStorage::new(path.clone());
            let mut current = original.clone();
            let (sender, receiver) = mpsc::channel();
            let worker = std::thread::spawn(move || {
                let started = Instant::now();
                let mut contended = false;
                let result = storage.apply_with(&operation, &mut current, |step| {
                    contended |= matches!(step, SaveStep::Contended);
                });
                sender
                    .send((result, current, started.elapsed(), contended))
                    .unwrap();
            });
            let (result, current, elapsed, contended) =
                receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            worker.join().unwrap();
            assert!(!result.unwrap_err().is_empty());
            assert_eq!(current, expected);
            assert!(contended);
            assert!(elapsed >= CUSTOM_LOCK_WAIT);
            assert!(elapsed < Duration::from_secs(2));
            let mut saved = Vec::new();
            assert!(read_custom(&path, &mut saved));
            assert_eq!(saved, original);
        }
        drop(held);
        remove_store(&path);
    }
}
