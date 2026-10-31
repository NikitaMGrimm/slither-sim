//! Login-only settings and bounded persistence, independent of appearance and transport.
use crate::{menu_controls::*, menu_preferences::MenuQuality, menu_theme::ControlStyle as S};

/// Bound host reads before parsing, including unknown future records.
pub const MAX_SETTINGS_BYTES: usize = 64;
/// Quality toggle within the settings panel.
pub const QUALITY: ControlId = ControlId(2);
/// Close without reverting the immediately applied preference.
pub const DONE: ControlId = ControlId(3);
/// Stable panel identity, separate from login, champion and directory.
pub const PANEL: u16 = 3;

/// Requested settings, never the renderer's adaptive runtime state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SettingsPreferences {
    /// Existing next-Play quality mode.
    pub quality: MenuQuality,
}
impl SettingsPreferences {
    /// Strict v1 record. Future versions remain untouched in host storage.
    pub fn parse(record: &str) -> Option<Self> {
        if record.len() > MAX_SETTINGS_BYTES {
            return None;
        }
        Some(Self {
            quality: match record {
                "1:0" => MenuQuality::Low,
                "1:1" => MenuQuality::High,
                _ => return None,
            },
        })
    }
    /// No startup writes; invalid/missing records use the exact legacy mapping.
    pub fn restore(record: Option<&str>, legacy: Option<&str>) -> Self {
        record.and_then(Self::parse).unwrap_or(Self {
            quality: MenuQuality::restore(legacy),
        })
    }
    /// Canonical ASCII record, independent of the appearance schema.
    pub fn serialize(self) -> &'static str {
        match self.quality {
            MenuQuality::High => "1:1",
            MenuQuality::Low => "1:0",
        }
    }
}

/// One settings owner. Hosts acknowledge one write per explicit change.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsState {
    preferences: SettingsPreferences,
    open: bool,
    generation: u64,
    pending: Option<&'static str>,
    not_saved: bool,
}
impl SettingsState {
    /// Restore preferences without scheduling storage work.
    pub fn new(preferences: SettingsPreferences) -> Self {
        Self {
            preferences,
            open: false,
            generation: 0,
            pending: None,
            not_saved: false,
        }
    }
    /// Copied requested preferences, also used to freeze Play.
    pub fn preferences(&self) -> SettingsPreferences {
        self.preferences
    }
    /// Whether this panel owns menu input and content.
    pub fn visible(&self) -> bool {
        self.open
    }
    /// Mount only after the login's existing admission gates settle.
    pub fn open(&mut self, view: &crate::start_page::StartMenuView) -> bool {
        if !available(view) || self.open {
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
            DONE => self.close(),
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
    /// Build a Quality-only panel. Narrow windows wrap help without shrinking type.
    pub fn panel(&self, viewport: [f64; 2]) -> PanelView {
        let width = 560_f64.min((viewport[0] - 32.).max(1.));
        let narrow = width < 480.;
        let height = if narrow { 352. } else { 320. };
        let x = (viewport[0] - width) / 2.;
        let y = ((viewport[1] - height) / 2.).max(8.);
        let mut controls = Vec::new();
        let mut add = |id, rect: [f64; 4], style, text: &str, tab, kind| {
            controls.push(Control {
                id,
                placement: Placement::rect(x + rect[0], y + rect[1], rect[2], rect[3]),
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
            S::StudioSurface,
            "Settings",
            None,
            ControlKind::Surface,
        );
        add(
            ControlId(1),
            [24., 20., width - 48., 36.],
            S::Heading,
            "Settings",
            None,
            ControlKind::Label {
                text: "Settings".into(),
            },
        );
        add(
            ControlId(4),
            if narrow {
                [24., 60., width - 48., 24.]
            } else {
                [24., 80., 120., 48.]
            },
            S::HelpText,
            "Quality",
            None,
            ControlKind::Label {
                text: "Quality".into(),
            },
        );
        add(
            QUALITY,
            if narrow {
                [24., 92., width - 48., 48.]
            } else {
                [168., 80., width - 192., 48.]
            },
            S::SecondaryButton,
            "High quality",
            Some(0),
            ControlKind::Toggle {
                label: "High quality".into(),
                checked: self.preferences.quality == MenuQuality::High,
            },
        );
        let lines: &[&str] = if width < 480. {
            &[
                "Off: Low. High adapts",
                "effects to load.",
                "Quality applies next game.",
            ]
        } else {
            &[
                "Off: Low. High adapts effects to load.",
                "Quality applies next game.",
            ]
        };
        for (index, line) in lines.iter().enumerate() {
            add(
                ControlId(10 + index as u16),
                [
                    24.,
                    (if narrow { 148. } else { 140. }) + index as f64 * 24.,
                    width - 48.,
                    24.,
                ],
                S::HelpText,
                line,
                None,
                ControlKind::Label {
                    text: (*line).into(),
                },
            );
        }
        if self.not_saved {
            let lines: &[&str] = if width < 480. {
                &["Not saved;", "applies this session"]
            } else {
                &["Not saved; applies this session"]
            };
            for (index, line) in lines.iter().enumerate() {
                add(
                    ControlId(20 + index as u16),
                    [
                        24.,
                        (if narrow { 228. } else { 216. }) + index as f64 * 24.,
                        width - 48.,
                        24.,
                    ],
                    S::HelpText,
                    line,
                    None,
                    ControlKind::Label {
                        text: (*line).into(),
                    },
                );
            }
        }
        add(
            DONE,
            [(width - 112.) / 2., height - 56., 112., 40.],
            S::PrimaryButton,
            "Done",
            Some(1),
            ControlKind::Button {
                label: "Done".into(),
            },
        );
        let mut backdrop = controls[0].clone();
        backdrop.id = ControlId(30);
        backdrop.placement = Placement::rect(0., 0., viewport[0], viewport[1]);
        backdrop.kind = ControlKind::Backdrop;
        backdrop.name = "Settings backdrop".into();
        backdrop.alpha = 0.88; // Same modal backdrop as the server picker.
        controls.insert(0, backdrop);
        PanelView {
            id: PANEL,
            generation: self.generation,
            placement: Placement::rect(0., 0., viewport[0], viewport[1]),
            visible: self.open,
            enabled: self.open,
            alpha: 1.,
            modal: self.open,
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
