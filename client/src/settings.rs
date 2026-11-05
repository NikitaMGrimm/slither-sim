//! Login-only settings and bounded persistence, independent of appearance and transport.
use crate::{menu_controls::*, menu_preferences::MenuQuality};

/// Bound host reads before parsing, including unknown future records.
pub const MAX_SETTINGS_BYTES: usize = 64;
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

/// One selectable graphics policy. Auto preserves the historical High path.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GraphicsQuality {
    /// Preserve the historical automatic policy exactly.
    #[default]
    Auto,
    /// Minimize effect and geometry costs.
    Low,
    /// Reduced glow and intermediate body detail.
    Medium,
    /// Full detail without adaptive degradation.
    High,
}
impl GraphicsQuality {
    /// User-facing option name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
    /// Shared runtime policy frozen when Play is requested.
    pub fn mode(self) -> crate::presentation::QualityMode {
        use crate::presentation::QualityMode;
        match self {
            Self::Auto => QualityMode::High,
            Self::Low => QualityMode::Low,
            Self::Medium => QualityMode::Medium,
            Self::High => QualityMode::FixedHigh,
        }
    }
}
/// Requested world AA; Auto retains the historical 64 MiB Native policy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AntiAliasing {
    /// Preserve the historical automatic policy exactly.
    #[default]
    Auto,
    /// Single-sample world attachments.
    Off,
    /// Request two samples, falling back visibly when unavailable.
    X2,
    /// Request four samples, falling back visibly when unavailable.
    X4,
}
/// Preset radios in visual and keyboard order.
pub const QUALITY_CHOICES: [(ControlId, GraphicsQuality); 4] = [
    (ControlId(40), GraphicsQuality::Auto),
    (ControlId(41), GraphicsQuality::Low),
    (ControlId(42), GraphicsQuality::Medium),
    (ControlId(43), GraphicsQuality::High),
];
/// AA radios in visual and keyboard order.
pub const AA_CHOICES: [(ControlId, AntiAliasing); 4] = [
    (ControlId(44), AntiAliasing::Auto),
    (ControlId(45), AntiAliasing::Off),
    (ControlId(46), AntiAliasing::X2),
    (ControlId(47), AntiAliasing::X4),
];
impl AntiAliasing {
    /// User-facing option name.
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Off => "Off",
            Self::X2 => "2×",
            Self::X4 => "4×",
        }
    }
}

/// Requested settings, never the renderer's adaptive runtime state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SettingsPreferences {
    /// Selected graphics preset.
    pub graphics: GraphicsQuality,
    /// Requested world anti-aliasing.
    pub anti_aliasing: AntiAliasing,
    /// Requested attachment policy. Unsupported sizes keep this preference.
    pub resolution: WorldResolution,
}
impl SettingsPreferences {
    /// Legacy browser/menu projection; the preset remains the single authority.
    pub fn quality(self) -> MenuQuality {
        if self.graphics == GraphicsQuality::Low {
            MenuQuality::Low
        } else {
            MenuQuality::High
        }
    }
    /// Strict v3, or in-memory migration from v2/v1; no startup writes.
    pub fn parse(record: &str) -> Option<Self> {
        if record.len() > MAX_SETTINGS_BYTES {
            return None;
        }
        let mut fields = record.split(':');
        let version = fields.next()?;
        if version == "3" {
            let graphics = match fields.next()? {
                "auto" => GraphicsQuality::Auto,
                "low" => GraphicsQuality::Low,
                "medium" => GraphicsQuality::Medium,
                "high" => GraphicsQuality::High,
                _ => return None,
            };
            let resolution = match fields.next()? {
                "original" => WorldResolution::Original,
                "native" => WorldResolution::Native,
                _ => return None,
            };
            let anti_aliasing = match fields.next()? {
                "auto" => AntiAliasing::Auto,
                "off" => AntiAliasing::Off,
                "2" => AntiAliasing::X2,
                "4" => AntiAliasing::X4,
                _ => return None,
            };
            if fields.next().is_some() {
                return None;
            }
            return Some(Self {
                graphics,
                resolution,
                anti_aliasing,
            });
        }
        let (quality, resolution) = match record {
            "1:0" | "2:0:original" => (MenuQuality::Low, WorldResolution::Original),
            "1:1" | "2:1:original" => (MenuQuality::High, WorldResolution::Original),
            "2:0:native" => (MenuQuality::Low, WorldResolution::Native),
            "2:1:native" => (MenuQuality::High, WorldResolution::Native),
            _ => return None,
        };
        Some(Self {
            graphics: if quality == MenuQuality::Low {
                GraphicsQuality::Low
            } else {
                GraphicsQuality::Auto
            },
            resolution,
            ..Self::default()
        })
    }
    /// Restore valid records or the exact legacy quality mapping without writes.
    pub fn restore(record: Option<&str>, legacy: Option<&str>) -> Self {
        record.and_then(Self::parse).unwrap_or_else(|| {
            let quality = MenuQuality::restore(legacy);
            Self {
                graphics: if quality == MenuQuality::Low {
                    GraphicsQuality::Low
                } else {
                    GraphicsQuality::Auto
                },
                ..Self::default()
            }
        })
    }
    /// Canonical bounded ASCII; no allocation or storage work on startup.
    pub fn serialize(self) -> &'static str {
        const RECORDS: [[[&str; 4]; 2]; 4] = [
            [
                [
                    "3:auto:original:auto",
                    "3:auto:original:off",
                    "3:auto:original:2",
                    "3:auto:original:4",
                ],
                [
                    "3:auto:native:auto",
                    "3:auto:native:off",
                    "3:auto:native:2",
                    "3:auto:native:4",
                ],
            ],
            [
                [
                    "3:low:original:auto",
                    "3:low:original:off",
                    "3:low:original:2",
                    "3:low:original:4",
                ],
                [
                    "3:low:native:auto",
                    "3:low:native:off",
                    "3:low:native:2",
                    "3:low:native:4",
                ],
            ],
            [
                [
                    "3:medium:original:auto",
                    "3:medium:original:off",
                    "3:medium:original:2",
                    "3:medium:original:4",
                ],
                [
                    "3:medium:native:auto",
                    "3:medium:native:off",
                    "3:medium:native:2",
                    "3:medium:native:4",
                ],
            ],
            [
                [
                    "3:high:original:auto",
                    "3:high:original:off",
                    "3:high:original:2",
                    "3:high:original:4",
                ],
                [
                    "3:high:native:auto",
                    "3:high:native:off",
                    "3:high:native:2",
                    "3:high:native:4",
                ],
            ],
        ];
        RECORDS[self.graphics as usize][usize::from(self.resolution == WorldResolution::Native)]
            [self.anti_aliasing as usize]
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
    effective_samples: u32,
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
            effective_samples: 1,
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
    /// Mount only after the login's existing phase gates settle.
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
        if intent.checked == Some(true) {
            if let Some((_, graphics)) =
                QUALITY_CHOICES.iter().find(|(id, _)| *id == intent.control)
            {
                if self.preferences.graphics != *graphics {
                    self.preferences.graphics = *graphics;
                    self.pending = Some(self.preferences.serialize());
                }
                return;
            }
            if let Some((_, aa)) = AA_CHOICES.iter().find(|(id, _)| *id == intent.control) {
                if self.preferences.anti_aliasing != *aa {
                    self.preferences.anti_aliasing = *aa;
                    self.pending = Some(self.preferences.serialize());
                }
                return;
            }
        }
        match intent.control {
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
        self.preferences.graphics = if self.preferences.quality() == MenuQuality::High {
            GraphicsQuality::Low
        } else {
            GraphicsQuality::Auto
        };
        self.preferences.quality().storage_value()
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
    /// The renderer reports the actual prepared attachment, never its ceiling.
    pub fn effective_samples(&mut self, samples: u32) {
        self.effective_samples = samples;
    }
    /// Actual sample count of the prepared world attachment.
    pub fn world_samples(&self) -> u32 {
        self.effective_samples
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
/// Settings needs no connection admission, including while a returned socket closes.
/// Visible return permits local edits; pending Play and champion phases still own the menu.
pub fn available(view: &crate::start_page::StartMenuView) -> bool {
    matches!(
        view.phase,
        crate::start_page::MenuPhase::Login | crate::start_page::MenuPhase::Returning
    ) && view.visible
        && view.ordinary_controls_visible
        && !view.champion_pending
}
