//! Directory picker host binding. No socket/session effects are executed here.
use crate::{
    Result,
    editor_geometry::EditorGeometry,
    menu_input_native::{KeyInput, NativeInputResult, NativeMenuInput},
    text::TextCache,
};
use slither_client::{
    menu_controls::*,
    menu_input::{EditIntent, EditorId, HostEditEffect},
    menu_theme::MenuTheme,
    server_picker::*,
};
use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions, TryLockError},
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use winit::{
    event::{ElementState, MouseScrollDelta, WindowEvent},
    keyboard::{Key, NamedKey},
};

pub struct NativeServerPicker {
    pub policy: ServerPicker,
    pub input: NativeMenuInput,
    pub controls: PanelInputState,
    pub panel: PanelView,
    pub scroll: BTreeMap<EditorId, f64>,
    pub viewport: [f64; 2],
    wheel: f64,
}
impl Default for NativeServerPicker {
    fn default() -> Self {
        let mut policy = ServerPicker::default();
        let mut input = NativeMenuInput::default();
        input.state.register(
            SEARCH_EDITOR,
            slither_client::menu_input::TextRule::NumericServer,
            "",
        );
        input.state.register(
            ADDRESS_EDITOR,
            slither_client::menu_input::TextRule::ServerAddress,
            "",
        );
        Self {
            panel: policy.panel([960., 540.]),
            policy,
            input,
            controls: PanelInputState::default(),
            scroll: BTreeMap::new(),
            viewport: [960., 540.],
            wheel: 0.,
        }
    }
}
impl NativeServerPicker {
    fn actions(&mut self) {
        if self.policy.take_add_request() {
            let value = self.input.state.editor(ADDRESS_EDITOR).text().to_owned();
            let result = normalize_custom_server(value.trim())
                .and_then(|address| self.policy.add_custom(address).map_err(str::to_owned));
            match result {
                Ok(()) => {
                    self.policy.custom_error.clear();
                    self.input.state.reset_editor(ADDRESS_EDITOR, "");
                }
                Err(error) => self.policy.custom_error = error,
            }
        }
    }
    pub fn sync(&mut self) -> Vec<HostEditEffect> {
        self.panel = self.policy.panel(self.viewport);
        self.controls.sync(&self.panel, &mut self.input.state)
    }
    pub fn open(&mut self) -> Vec<HostEditEffect> {
        self.policy.open();
        self.input.state.reset_editor(SEARCH_EDITOR, "");
        self.scroll.clear();
        self.wheel = 0.;
        self.sync()
    }
    pub fn step(&mut self, now: f64) -> Result<Vec<HostEditEffect>> {
        self.policy.step(now).map_err(str::to_owned)?;
        self.input.state.step_caret(now);
        let effects = self.sync();
        self.controls
            .step(&self.panel, now, MenuTheme::default())
            .map_err(str::to_owned)?;
        Ok(effects)
    }
    pub fn commit_effects(&mut self, effects: Vec<HostEditEffect>) -> Vec<HostEditEffect> {
        let mut remaining = Vec::new();
        for effect in effects {
            if matches!(effect, HostEditEffect::Activate(_)) {
                if let Some(intent) = self.controls.activate(&self.panel, Activation::Enter) {
                    self.policy.activate(intent);
                }
            } else {
                remaining.push(effect);
            }
        }
        let value = self.input.state.editor(SEARCH_EDITOR).text().to_owned();
        self.policy.search(&value);
        self.actions();
        remaining.extend(self.sync());
        remaining
    }
    pub fn key(&mut self, event: KeyInput<'_>) -> NativeInputResult {
        if event.state == ElementState::Pressed {
            self.controls.keyboard_navigation();
        }
        let composing = self
            .input
            .state
            .focus()
            .is_some_and(|id| self.input.state.editor(id).composing());
        let mut effects = Vec::new();
        if event.state == ElementState::Pressed && !composing {
            match event.logical {
                Key::Named(NamedKey::Escape) => self.policy.close(),
                Key::Named(NamedKey::Tab) => self.controls.tab(&self.panel, self.input.shift_key()),
                Key::Named(
                    key @ (NamedKey::ArrowDown
                    | NamedKey::ArrowUp
                    | NamedKey::PageDown
                    | NamedKey::PageUp
                    | NamedKey::Home
                    | NamedKey::End),
                ) if self.controls.focus() != Some(ADDRESS)
                    && (self.controls.focus() != Some(SEARCH)
                        || matches!(
                            key,
                            NamedKey::ArrowDown
                                | NamedKey::ArrowUp
                                | NamedKey::PageDown
                                | NamedKey::PageUp
                        )) =>
                {
                    let delta = match key {
                        NamedKey::ArrowDown => 1,
                        NamedKey::ArrowUp => -1,
                        NamedKey::PageDown => 6,
                        NamedKey::PageUp => -6,
                        NamedKey::Home => isize::MIN,
                        _ => isize::MAX,
                    };
                    let target = self.policy.navigate(self.controls.focus(), delta);
                    effects.extend(self.sync());
                    self.controls.focus_control(&self.panel, target);
                }
                Key::Named(NamedKey::Enter | NamedKey::Space)
                    if !matches!(self.controls.focus(), Some(SEARCH | ADDRESS))
                        && !event.repeat =>
                {
                    if let Some(intent) = self.controls.activate(&self.panel, Activation::Enter) {
                        self.policy.activate(intent);
                    }
                }
                _ => return self.input.key(event),
            }
            self.actions();
            effects.extend(self.sync());
            effects.extend(self.controls.sync_focus(&self.panel, &mut self.input.state));
            return NativeInputResult {
                consumed: true,
                effects,
            };
        }
        self.input.key(event)
    }
    pub fn event(&mut self, event: &WindowEvent) -> NativeInputResult {
        match event {
            WindowEvent::KeyboardInput { event, .. } => self.key(event.into()),
            WindowEvent::MouseWheel { delta, .. } => {
                self.wheel += match delta {
                    MouseScrollDelta::LineDelta(_, y) => -f64::from(*y),
                    MouseScrollDelta::PixelDelta(p) => -p.y / 46.,
                };
                let rows = self.wheel.trunc() as isize;
                self.wheel -= rows as f64;
                self.policy.scroll(rows);
                NativeInputResult {
                    consumed: true,
                    effects: self.sync(),
                }
            }
            WindowEvent::Focused(false) => {
                self.controls.blur();
                self.input.window_event(event)
            }
            _ => self.input.window_event(event),
        }
    }
    fn geometry(&mut self, text: &mut TextCache) -> Result<Option<EditorGeometry>> {
        let resolved = self.panel.resolve(self.viewport).map_err(str::to_owned)?;
        let Some(r) = resolved
            .iter()
            .find(|r| Some(r.control.id) == self.controls.focus())
        else {
            return Ok(None);
        };
        Ok(
            crate::editor_geometry::field_geometry(text, r, &self.input.state, &mut self.scroll)?
                .map(|(_, g)| g),
        )
    }
    pub fn pointer(
        &mut self,
        text: &mut TextCache,
        point: [f64; 2],
        down: Option<bool>,
    ) -> Result<Vec<HostEditEffect>> {
        self.pointer_scaled(text, point, down, 1.)
    }
    pub fn pointer_scaled(
        &mut self,
        text: &mut TextCache,
        point: [f64; 2],
        down: Option<bool>,
        dpr: f64,
    ) -> Result<Vec<HostEditEffect>> {
        let (mut effects, intent) = self.input.panel_pointer(
            &mut self.controls,
            &self.panel,
            &mut self.scroll,
            text,
            self.viewport,
            point,
            down,
            dpr,
        )?;
        if down == Some(true) && self.controls.focus() == Some(SEARCH) {
            // The reference deliberately clears numeric search on every click.
            self.input.state.reset_editor(SEARCH_EDITOR, "");
        }
        if let Some(intent) = intent {
            self.policy.activate(intent);
        }
        self.actions();
        effects.extend(self.sync());
        Ok(effects)
    }
    pub fn cursor_area(&mut self, text: &mut TextCache, dpr: f64) -> Result<Vec<HostEditEffect>> {
        if !matches!(self.controls.focus(), Some(SEARCH | ADDRESS)) {
            return Ok(Vec::new());
        }
        if let Some(g) = self.geometry(text)? {
            let r = g.snapped_cursor_rect(dpr);
            return Ok(self.input.state.apply(EditIntent::CursorArea {
                position: [r.x, r.y],
                size: [r.width, r.height],
            }));
        }
        Ok(Vec::new())
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
}
impl CustomServerStorage {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
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
                    let remaining = CUSTOM_LOCK_WAIT.saturating_sub(started.elapsed());
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
            let storage = CustomServerStorage::new(second_path);
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
