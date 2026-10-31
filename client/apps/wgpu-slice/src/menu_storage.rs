//! Bounded native v2 adapter. Startup reads only; Save performs one atomic replacement.
use slither_client::skin_studio::{
    AppearanceRecord, MAX_RECORD_BYTES, PreferenceInput, RestoredAppearance,
};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub struct MenuStorage {
    directory: Option<PathBuf>,
    pub enabled: bool,
}
impl MenuStorage {
    pub fn native() -> Self {
        Self::at(std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("SlitherClient")))
    }
    pub fn at(directory: Option<PathBuf>) -> Self {
        Self {
            directory,
            enabled: true,
        }
    }
    pub fn restore(&mut self) -> RestoredAppearance {
        let read = |name: &str| -> PreferenceInput {
            let Some(directory) = &self.directory else {
                return PreferenceInput::Missing;
            };
            let path = directory.join(name);
            let file = match std::fs::File::open(path) {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return PreferenceInput::Missing;
                }
                Err(_) => return PreferenceInput::Unreadable,
            };
            let mut bytes = Vec::new();
            if file
                .take((MAX_RECORD_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .is_err()
            {
                return PreferenceInput::Unreadable;
            }
            match String::from_utf8(bytes) {
                Ok(text) => PreferenceInput::Text(text),
                Err(_) => PreferenceInput::Corrupt,
            }
        };
        let v2 = read("menu.v2");
        let v1 = if v2 == PreferenceInput::Missing {
            read("menu.v1")
        } else {
            PreferenceInput::Missing
        };
        let missing = v2 == PreferenceInput::Missing && v1 == PreferenceInput::Missing;
        let mut restored = RestoredAppearance::restore(v2, v1);
        if missing {
            restored.record.nickname = "Player".into();
        }
        self.enabled = restored.persistence_enabled;
        restored
    }
    /// A failed write leaves the memory commit intact; caller shows run-only status.
    pub fn save(&self, record: &AppearanceRecord) -> Result<(), String> {
        if !self.enabled {
            return Err("Preference version unavailable".into());
        }
        let bytes = record.serialize().map_err(str::to_owned)?;
        let directory = self
            .directory
            .as_ref()
            .ok_or("Preference storage unavailable")?;
        atomic_write(directory, "menu.v2", &bytes)
    }
}

/// Independent Quality record; unreadable files disable writes for this run.
pub struct SettingsStorage {
    directory: Option<PathBuf>,
    pub enabled: bool,
}
impl SettingsStorage {
    pub fn native() -> Self {
        Self::at(std::env::var_os("APPDATA").map(|p| PathBuf::from(p).join("SlitherClient")))
    }
    pub fn at(directory: Option<PathBuf>) -> Self {
        Self {
            directory,
            enabled: true,
        }
    }
    pub fn restore(&mut self) -> slither_client::settings::SettingsPreferences {
        use slither_client::settings::{MAX_SETTINGS_BYTES, SettingsPreferences};
        let Some(directory) = &self.directory else {
            return SettingsPreferences::default();
        };
        let result = (|| -> std::io::Result<Option<String>> {
            let file = match std::fs::File::open(directory.join("settings.txt")) {
                Ok(file) => file,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e),
            };
            let mut bytes = Vec::new();
            file.take((MAX_SETTINGS_BYTES + 1) as u64)
                .read_to_end(&mut bytes)?;
            Ok(String::from_utf8(bytes).ok())
        })();
        match result {
            Ok(record) => SettingsPreferences::restore(record.as_deref(), None),
            Err(_) => {
                self.enabled = false;
                SettingsPreferences::default()
            }
        }
    }
    pub fn save(&self, record: &str) -> Result<(), String> {
        if !self.enabled {
            return Err("Settings storage unreadable this run".into());
        }
        slither_client::settings::SettingsPreferences::parse(record)
            .ok_or("Invalid settings record")?;
        let directory = self
            .directory
            .as_ref()
            .ok_or("Settings storage unavailable")?;
        atomic_write(directory, "settings.txt", record)
    }
}

fn atomic_write(directory: &std::path::Path, name: &str, bytes: &str) -> Result<(), String> {
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let temp = directory.join(format!(
        ".{name}-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(bytes.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, directory.join(name))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|e| e.to_string())
}
