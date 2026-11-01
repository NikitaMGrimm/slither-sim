//! Login-only settings and bounded persistence, independent of appearance and transport.
use crate::{menu_controls::*, menu_preferences::MenuQuality};

/// Bound host reads before parsing, including unknown future records.
pub const MAX_SETTINGS_BYTES: usize = 64;
/// Quality toggle within the settings panel.
pub const QUALITY: ControlId = ControlId(2);
/// Requested world attachment resolution, independent of Quality.
pub const RESOLUTION: ControlId = ControlId(5);
/// Close without reverting the immediately applied preference.
pub const DONE: ControlId = ControlId(3);
/// Stable panel identity, separate from login, champion and directory.
pub const PANEL: u16 = 4;
const BACKDROP: ControlId = ControlId(30);

/// World raster policy; projection and input are identical in both modes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WorldResolution {
    /// Existing capped world raster and linear presentation blit.
    #[default]
    Original,
    /// Physical-size attachment with bounded MSAA, resolving directly into presentation.
    Native,
}

/// Requested settings, never the renderer's adaptive runtime state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SettingsPreferences {
    /// Existing next-Play quality mode.
    pub quality: MenuQuality,
    /// Requested attachment policy. Unsupported sizes keep this preference.
    pub resolution: WorldResolution,
}
impl SettingsPreferences {
    /// Strict v2 record, or v1 migrated with Original; no startup repair writes.
    pub fn parse(record: &str) -> Option<Self> {
        if record.len() > MAX_SETTINGS_BYTES {
            return None;
        }
        let (quality, resolution) = match record {
            "1:0" | "2:0:original" => (MenuQuality::Low, WorldResolution::Original),
            "1:1" | "2:1:original" => (MenuQuality::High, WorldResolution::Original),
            "2:0:native" => (MenuQuality::Low, WorldResolution::Native),
            "2:1:native" => (MenuQuality::High, WorldResolution::Native),
            _ => return None,
        };
        Some(Self {
            quality,
            resolution,
        })
    }
    /// No startup writes; invalid/missing records use the exact legacy mapping.
    pub fn restore(record: Option<&str>, legacy: Option<&str>) -> Self {
        record.and_then(Self::parse).unwrap_or(Self {
            quality: MenuQuality::restore(legacy),
            resolution: WorldResolution::Original,
        })
    }
    /// Canonical ASCII record, independent of the appearance schema.
    pub fn serialize(self) -> &'static str {
        match (self.quality, self.resolution) {
            (MenuQuality::High, WorldResolution::Original) => "2:1:original",
            (MenuQuality::Low, WorldResolution::Original) => "2:0:original",
            (MenuQuality::High, WorldResolution::Native) => "2:1:native",
            (MenuQuality::Low, WorldResolution::Native) => "2:0:native",
        }
    }
}

/// One settings owner. Hosts acknowledge one write per explicit change.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsState {
    preferences: SettingsPreferences,
    open: bool,
    alpha: f64,
    previous: Option<f64>,
    generation: u64,
    pending: Option<&'static str>,
    not_saved: bool,
    native_available: bool,
}
impl SettingsState {
    /// Restore preferences without scheduling storage work.
    pub fn new(preferences: SettingsPreferences) -> Self {
        Self {
            preferences,
            open: false,
            alpha: 0.,
            previous: None,
            generation: 0,
            pending: None,
            not_saved: false,
            native_available: true,
        }
    }
    /// Copied requested preferences, also used to freeze Play.
    pub fn preferences(&self) -> SettingsPreferences {
        self.preferences
    }
    /// Whether this panel owns menu input and content.
    pub fn visible(&self) -> bool {
        self.open || self.alpha > 0.
    }
    /// Whether preference changes are still admitted, including during entry.
    pub fn is_open(&self) -> bool {
        self.open
    }
    /// Sampled opacity, shared with the directory picker's transition.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
    /// Advance on the host's existing clock; closing never schedules a write.
    pub fn step(&mut self, now: f64) -> Result<(), &'static str> {
        crate::menu_transition::step(&mut self.alpha, &mut self.previous, self.open, now)
            .map_err(|_| "Invalid settings clock")
    }
    /// Mount only after the login's existing admission gates settle.
    pub fn open(&mut self, view: &crate::start_page::StartMenuView) -> bool {
        if !available(view) || self.visible() {
            return false;
        }
        self.open = true;
        self.generation = self.generation.wrapping_add(1);
        true
    }
    /// Done and Escape have identical immediate-apply semantics.
    pub fn close(&mut self) {
        self.open = false;
    }
    /// A generation-bound activation produces an explicit Set, never a blind toggle.
    pub fn activate(&mut self, intent: ControlIntent) {
        if !self.open || intent.panel != PANEL || intent.generation != self.generation {
            return;
        }
        match intent.control {
            QUALITY => {
                if let Some(high) = intent.checked {
                    let quality = if high {
                        MenuQuality::High
                    } else {
                        MenuQuality::Low
                    };
                    if quality != self.preferences.quality {
                        self.preferences.quality = quality;
                        self.pending = Some(self.preferences.serialize());
                    }
                }
            }
            DONE | BACKDROP => self.close(),
            RESOLUTION => {
                if let Some(native) = intent.checked {
                    let resolution = if native {
                        WorldResolution::Native
                    } else {
                        WorldResolution::Original
                    };
                    if resolution != self.preferences.resolution {
                        self.preferences.resolution = resolution;
                        self.pending = Some(self.preferences.serialize());
                    }
                }
            }
            _ => {}
        }
    }
    /// The existing dev/reference control shares the same Quality authority.
    pub fn legacy_toggle(&mut self) -> &'static str {
        self.preferences.quality.toggle()
    }
    /// Drain once, outside input/painter borrows. Failure retries only on a new change.
    pub fn take_write(&mut self) -> Option<&'static str> {
        self.pending.take()
    }
    /// Persistence failure never reverts the selected in-memory preference.
    pub fn storage_result(&mut self, saved: bool) {
        self.not_saved = !saved;
    }
    /// Host preflight reports capability without changing the requested preference.
    pub fn native_available(&mut self, available: bool) {
        self.native_available = available;
    }
    /// Explicit generation-bound action, shared by native widgets and offline tests.
    pub fn intent(
        &self,
        control: ControlId,
        checked: Option<bool>,
        activation: Activation,
    ) -> ControlIntent {
        ControlIntent {
            panel: PANEL,
            generation: self.generation,
            control,
            checked,
            activation,
        }
    }
    /// A failed save is visible until a later explicit change succeeds.
    pub fn not_saved(&self) -> bool {
        self.not_saved
    }
    /// Capability status never changes the requested resolution.
    pub fn native_unavailable(&self) -> bool {
        self.preferences.resolution == WorldResolution::Native && !self.native_available
    }
}
/// Shared entry gate; no pending Play, return, champion, or missing route admission.
pub fn available(view: &crate::start_page::StartMenuView) -> bool {
    view.phase == crate::start_page::MenuPhase::Login
        && view.visible
        && !view.disabled
        && view.ordinary_controls_visible
        && !view.champion_pending
}
