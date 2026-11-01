//! Login-only settings and bounded persistence, independent of appearance and transport.
use crate::{menu_controls::*, menu_preferences::MenuQuality, menu_theme::ControlStyle as S};

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
    /// Both independent settings, with readable wrapped help on narrow windows.
    pub fn panel(&self, viewport: [f64; 2]) -> PanelView {
        let width = 560_f64.min((viewport[0] - 32.).max(1.));
        let narrow = width < 480.;
        let height = if narrow { 512. } else { 400. };
        let scale = crate::menu_transition::scale(self.alpha);
        let x = (viewport[0] - width * scale) / 2.;
        let y = ((viewport[1] - height) / 2.).max(8.) + height * (1. - scale) / 2.;
        let mut controls = Vec::new();
        let mut add = |id, rect: [f64; 4], style, text: &str, tab, kind| {
            let mut placement =
                Placement::rect(x + rect[0] * scale, y + rect[1] * scale, rect[2], rect[3]);
            placement.scale = scale;
            controls.push(Control {
                id,
                placement,
                clip: None,
                visible: true,
                enabled: true,
                alpha: 1.,
                name: text.into(),
                tab_order: tab,
                style,
                kind,
            });
        };
        add(
            ControlId(0),
            [0., 0., width, height],
            S::SettingsSurface,
            "Settings",
            None,
            ControlKind::Surface,
        );
        add(
            ControlId(1),
            [24., 16., width - 48., 32.],
            S::Heading,
            "Settings",
            None,
            ControlKind::Label {
                text: "Settings".into(),
            },
        );
        for (row, id, name, caption, checked) in [
            (
                0,
                QUALITY,
                "Quality",
                "High quality",
                self.preferences.quality == MenuQuality::High,
            ),
            (
                1,
                RESOLUTION,
                "World resolution",
                "Native/sharp",
                self.preferences.resolution == WorldResolution::Native,
            ),
        ] {
            let top = if narrow {
                56. + f64::from(row) * 136.
            } else {
                60. + f64::from(row) * 104.
            };
            add(
                ControlId(6 + row),
                [
                    24.,
                    top,
                    if narrow { width - 48. } else { 168. },
                    if narrow { 24. } else { 40. },
                ],
                S::HelpText,
                name,
                None,
                ControlKind::Label { text: name.into() },
            );
            add(
                id,
                if narrow {
                    [24., top + 28., width - 48., 40.]
                } else {
                    [196., top, width - 220., 40.]
                },
                S::SecondaryButton,
                caption,
                Some(row),
                ControlKind::Toggle {
                    label: caption.into(),
                    checked,
                },
            );
            let lines = if row == 0 {
                ["Off: Low. High adapts to load.", "Next game."]
            } else {
                ["Sharper world; higher GPU use.", "Same view area."]
            };
            for (line, text) in lines.iter().enumerate() {
                add(
                    ControlId(10 + row * 2 + line as u16),
                    [
                        24.,
                        top + if narrow { 72. } else { 44. } + line as f64 * 24.,
                        width - 48.,
                        24.,
                    ],
                    S::SettingsHelp,
                    text,
                    None,
                    ControlKind::Label {
                        text: (*text).into(),
                    },
                );
            }
        }
        if self.preferences.resolution == WorldResolution::Native && !self.native_available {
            let lines: &[&str] = if narrow {
                &["Native unavailable", "at this size"]
            } else {
                &["Native unavailable at this size"]
            };
            for (i, text) in lines.iter().enumerate() {
                add(
                    ControlId(20 + i as u16),
                    [
                        24.,
                        if narrow { 320. } else { 272. } + i as f64 * 24.,
                        width - 48.,
                        24.,
                    ],
                    S::SettingsHelp,
                    text,
                    None,
                    ControlKind::Label {
                        text: (*text).into(),
                    },
                );
            }
        }
        if self.not_saved {
            let lines: &[&str] = if narrow {
                &["Not saved;", "applies this session"]
            } else {
                &["Not saved; applies this session"]
            };
            for (i, text) in lines.iter().enumerate() {
                add(
                    ControlId(24 + i as u16),
                    [
                        24.,
                        if narrow { 376. } else { 304. } + i as f64 * 24.,
                        width - 48.,
                        24.,
                    ],
                    S::SettingsHelp,
                    text,
                    None,
                    ControlKind::Label {
                        text: (*text).into(),
                    },
                );
            }
        }
        add(
            DONE,
            [(width - 112.) / 2., height - 56., 112., 40.],
            S::PrimaryButton,
            "Done",
            Some(2),
            ControlKind::Button {
                label: "Done".into(),
            },
        );
        let mut backdrop = controls[0].clone();
        backdrop.id = BACKDROP;
        backdrop.placement = Placement::rect(0., 0., viewport[0], viewport[1]);
        backdrop.kind = ControlKind::Backdrop;
        backdrop.name = "Settings backdrop".into();
        backdrop.alpha = 0.88;
        controls.insert(0, backdrop);
        PanelView {
            id: PANEL,
            generation: self.generation,
            placement: Placement::rect(0., 0., viewport[0], viewport[1]),
            visible: self.visible(),
            // Keep feedback through exit; activation independently rechecks `open`.
            enabled: self.visible(),
            alpha: self.alpha,
            modal: self.visible(),
            initial_focus: Some(QUALITY),
            restore_focus: Some(crate::login_controls::SETTINGS),
            controls,
        }
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
