//! Offline appearance transaction. No session, acquisition or transport authority.
use crate::{
    menu_controls::*,
    menu_layout::MenuRect,
    menu_preferences::{MenuPreferences, hex, unhex},
    menu_theme::ControlStyle,
};

/// Upper bound before decoding a v2 ASCII record.
pub const MAX_RECORD_BYTES: usize = 1_200;
/// Authored entrance and return duration, in caller monotonic milliseconds.
pub const TRANSITION_MS: f64 = 230.;
/// Previous built-in control.
pub const PREVIOUS: ControlId = ControlId(10);
/// Next built-in control.
pub const NEXT: ControlId = ControlId(11);
/// Discard the transaction.
pub const CANCEL: ControlId = ControlId(12);
/// Commit the transaction once.
pub const SAVE: ControlId = ControlId(13);
/// Shared modal backdrop cancels on a matching outside release.
pub const BACKDROP: ControlId = ControlId(5);

/// Owned snapshot taken at explicit Play, before asynchronous profile acquisition.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppearanceSelection {
    /// Existing wire skin, including legacy 65.
    pub skin: u8,
    /// Exact admitted trailing login bytes; never renderer data.
    pub custom_bytes: Vec<u8>,
}
impl AppearanceSelection {
    /// Studio admission is separate from the tolerant inbound renderer and old editor.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.skin > 65 {
            return Err("Unsupported skin");
        }
        let bytes = &self.custom_bytes;
        if bytes.is_empty() {
            return Ok(());
        }
        if bytes.len() < 10 || bytes.len() > 255 || !bytes.len().is_multiple_of(2) {
            return Err("Invalid custom length");
        }
        if bytes[..8] != [0; 8] && bytes[..6] != [255, 255, 255, 0, 0, 0] {
            return Err("Invalid custom header");
        }
        if bytes[8..]
            .as_chunks::<2>()
            .0
            .iter()
            .any(|r| r[0] == 0 || !matches!(r[1], 0..=35 | 37 | 39 | 41))
        {
            return Err("Invalid custom run");
        }
        Ok(())
    }
}

/// Grants are not inferred from stored selection, asset readiness or CLI options.
/// PR1 has no grant producer. A later explicit importer can extend this snapshot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UnlockSnapshot {
    _private: (),
}
impl UnlockSnapshot {
    /// This delivery deliberately has no unlocks, including skin 42.
    pub fn skin_available(self, skin: u8) -> bool {
        skin <= 64 && skin != 42
    }
}

/// Complete v2 preference, retaining inactive custom data independently.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppearanceRecord {
    /// Current nickname sampled from its owner on Save.
    pub nickname: String,
    /// Last explicitly selected built-in; 65 is retained when imported.
    pub builtin: u8,
    /// Base wire skin for retained custom bytes.
    pub custom_base: u8,
    /// Inactive custom bytes are retained when selecting a built-in.
    pub custom_bytes: Vec<u8>,
    /// Whether the custom choice is active.
    pub active_custom: bool,
    /// Stored future preference only; this never grants or selects an accessory.
    pub cosmetic: Option<u8>,
}
impl AppearanceRecord {
    /// Freeze the effective wire appearance without rewriting its bytes.
    pub fn selection(&self) -> AppearanceSelection {
        AppearanceSelection {
            skin: if self.active_custom {
                self.custom_base
            } else {
                self.builtin
            },
            custom_bytes: if self.active_custom {
                self.custom_bytes.clone()
            } else {
                vec![]
            },
        }
    }
    /// Validate the entire draft, including retained inactive fields.
    pub fn validate(&self) -> Result<(), &'static str> {
        let mut name = MenuPreferences::new(false);
        name.set_nickname(&self.nickname)?;
        AppearanceSelection {
            skin: self.builtin,
            custom_bytes: vec![],
        }
        .validate()?;
        AppearanceSelection {
            skin: self.custom_base,
            custom_bytes: self.custom_bytes.clone(),
        }
        .validate()?;
        if (self.active_custom && self.custom_bytes.is_empty())
            || self.cosmetic.is_some_and(|v| v > 31)
        {
            return Err("Invalid appearance record");
        }
        Ok(())
    }
    /// Bounded canonical ASCII. Callers validate before committing or writing.
    pub fn serialize(&self) -> Result<String, &'static str> {
        self.validate()?;
        Ok(format!(
            "2:{}:{}:{}:{}:{}:{}",
            hex(self.nickname.as_bytes()),
            if self.active_custom { "c" } else { "b" },
            hex(&[self.builtin]),
            hex(&[self.custom_base]),
            hex(&self.custom_bytes),
            self.cosmetic.map_or_else(|| "--".into(), |v| hex(&[v]))
        ))
    }
    /// Strict all-or-nothing v2 decoding; malformed present files are not migrated.
    pub fn parse(saved: &str) -> Option<Self> {
        if saved.len() > MAX_RECORD_BYTES || !saved.is_ascii() {
            return None;
        }
        let fields: Vec<_> = saved.split(':').collect();
        let ["2", name, active, builtin, base, custom, cosmetic] = fields.as_slice() else {
            return None;
        };
        let byte = |s: &str| {
            let b = unhex(s)?;
            (b.len() == 1).then(|| b[0])
        };
        let record = Self {
            nickname: String::from_utf8(unhex(name)?).ok()?,
            active_custom: match *active {
                "b" => false,
                "c" => true,
                _ => return None,
            },
            builtin: byte(builtin)?,
            custom_base: byte(base)?,
            custom_bytes: unhex(custom)?,
            cosmetic: if *cosmetic == "--" {
                None
            } else {
                Some(byte(cosmetic)?)
            },
        };
        record.validate().ok()?;
        Some(record)
    }
}
/// Startup restoration result. Unknown versions disable writes for this run.
pub struct RestoredAppearance {
    /// Restored/default player choices.
    pub record: AppearanceRecord,
    /// Whether explicit Save may replace v2.
    pub persistence_enabled: bool,
}
impl RestoredAppearance {
    /// Read v2 first; only an absent v2 allows v1 migration. Never writes at startup.
    pub fn restore(v2: Option<&str>, v1: Option<&str>) -> Self {
        if let Some(saved) = v2 {
            return Self {
                record: AppearanceRecord::parse(saved).unwrap_or_default(),
                persistence_enabled: saved.split(':').next() == Some("2"),
            };
        }
        let old = MenuPreferences::restore(false, v1.unwrap_or("")).view();
        Self {
            record: AppearanceRecord {
                nickname: old.nickname,
                builtin: old.skin,
                custom_base: old.skin,
                active_custom: !old.custom_skin.is_empty(),
                custom_bytes: old.custom_skin,
                cosmetic: None,
            },
            persistence_enabled: true,
        }
    }
    /// Only explicit CLI values override restored choices. Explicit skin selects built-in.
    pub fn override_cli(
        &mut self,
        nickname: Option<&str>,
        skin: Option<u8>,
    ) -> Result<(), &'static str> {
        let mut record = self.record.clone();
        if let Some(name) = nickname {
            record.nickname = name.into();
        }
        if let Some(skin) = skin {
            record.builtin = skin;
            record.active_custom = false;
        }
        record.validate()?;
        self.record = record;
        Ok(())
    }
}
/// PR1 mounts only built-ins; later sections add their own controls.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StudioSection {
    /// Built-in selection is the only mounted section in PR1.
    BuiltIn,
}
/// Bounded draft; large imported custom runs remain opaque and unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkinDraft {
    /// Complete transaction, retaining inactive custom bytes.
    pub record: AppearanceRecord,
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    Closed,
    Entering(f64),
    Open,
    Returning { at: f64, from: f64 },
}
/// Single transaction and sampled transition, with no platform side effects.
pub struct StudioState {
    committed: AppearanceRecord,
    draft: Option<SkinDraft>,
    phase: Phase,
    alpha: f64,
    generation: u64,
    restore_focus: bool,
    /// Active section, fixed in PR1.
    pub section: StudioSection,
}
impl StudioState {
    /// Start closed with a validated committed appearance.
    pub fn new(committed: AppearanceRecord) -> Result<Self, &'static str> {
        committed.validate()?;
        Ok(Self {
            committed,
            draft: None,
            phase: Phase::Closed,
            alpha: 0.,
            generation: 0,
            restore_focus: false,
            section: StudioSection::BuiltIn,
        })
    }
    /// The only source of the next Play snapshot.
    pub fn committed(&self) -> &AppearanceRecord {
        &self.committed
    }
    /// Pending selection, never a connection input.
    pub fn draft(&self) -> Option<&SkinDraft> {
        self.draft.as_ref()
    }
    /// Includes the input-blocking return transition.
    pub fn visible(&self) -> bool {
        self.phase != Phase::Closed
    }
    /// Save and navigation are unavailable during return.
    pub fn interactive(&self) -> bool {
        matches!(self.phase, Phase::Entering(_) | Phase::Open)
    }
    /// Last sampled authored fade.
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
    /// Caller checks ordinary idle-login admission, independently of endpoint availability.
    pub fn open(&mut self, now: f64, admitted: bool) -> bool {
        if !admitted || self.visible() || !now.is_finite() {
            return false;
        }
        self.draft = Some(SkinDraft {
            record: self.committed.clone(),
        });
        self.generation += 1;
        self.phase = Phase::Entering(now);
        self.alpha = 0.;
        self.restore_focus = false;
        true
    }
    /// Sample both transitions from the caller clock, without intervals.
    pub fn step(&mut self, now: f64) {
        match self.phase {
            Phase::Entering(at) => {
                self.alpha = ((now - at) / TRANSITION_MS).clamp(0., 1.);
                if self.alpha == 1. {
                    self.phase = Phase::Open;
                }
            }
            Phase::Returning { at, from } => {
                let t = ((now - at) / TRANSITION_MS).clamp(0., 1.);
                self.alpha = from * (1. - t);
                if t == 1. {
                    self.phase = Phase::Closed;
                    self.draft = None;
                    self.restore_focus = true;
                }
            }
            _ => {}
        }
    }
    /// One focus restoration request after the return finishes.
    pub fn take_return_focus(&mut self) -> bool {
        std::mem::take(&mut self.restore_focus)
    }
    /// Select a new built-in, preserving inactive custom bytes and skipping locked IDs.
    pub fn cycle(&mut self, next: bool, grants: UnlockSnapshot) {
        if !self.interactive() {
            return;
        }
        let record = &mut self.draft.as_mut().expect("mounted draft").record;
        let mut id = record.selection().skin;
        loop {
            id = if next {
                if id >= 64 { 0 } else { id + 1 }
            } else if id == 0 || id > 64 {
                64
            } else {
                id - 1
            };
            if grants.skin_available(id) {
                break;
            }
        }
        record.builtin = id;
        record.active_custom = false;
    }
    /// Begin return without committing or requesting persistence.
    pub fn cancel(&mut self, now: f64) {
        if self.interactive() {
            self.phase = Phase::Returning {
                at: now,
                from: self.alpha,
            };
        }
    }
    /// One memory commit and one returned write request. Repeated Save returns None.
    pub fn save(
        &mut self,
        nickname: &str,
        now: f64,
        grants: UnlockSnapshot,
    ) -> Result<Option<AppearanceRecord>, &'static str> {
        if !self.interactive() {
            return Ok(None);
        }
        let mut record = self.draft.as_ref().expect("mounted draft").record.clone();
        record.nickname = nickname.into();
        record.validate()?;
        // A retained legacy choice remains byte-preserving. New selections
        // must still be available at commit, independently of ready artwork.
        if record.selection() != self.committed.selection()
            && !grants.skin_available(record.selection().skin)
        {
            return Err("Skin unavailable");
        }
        self.committed = record.clone();
        self.cancel(now);
        Ok(Some(record))
    }
    /// Responsive bounds shared by painting, preview fitting and outside dismissal.
    pub fn bounds(viewport: [f64; 2]) -> MenuRect {
        let width = (viewport[0] - 32.).clamp(240., 896.);
        let height = (viewport[1] - 32.).clamp(320., 476.);
        MenuRect {
            x: (viewport[0] - width) / 2.,
            y: (viewport[1] - height) / 2.,
            width,
            height,
        }
    }
    /// Clear live-pose region, shorter above the controls on narrow windows.
    pub fn preview_bounds(viewport: [f64; 2]) -> MenuRect {
        let b = Self::bounds(viewport);
        if b.width >= 640. {
            MenuRect {
                x: b.x + 24.,
                y: b.y + 112.,
                width: b.width - 328.,
                height: b.height - 208.,
            }
        } else {
            MenuRect {
                x: b.x + 24.,
                y: b.y + 88.,
                width: b.width - 48.,
                height: b.height - 268.,
            }
        }
    }
    /// Existing buttons provide all focus stops; unavailable sections are omitted.
    pub fn panel(&self, viewport: [f64; 2], status: &str) -> PanelView {
        let b = Self::bounds(viewport);
        let wide = b.width >= 640.;
        let x = if wide { b.width - 280. } else { 24. };
        let y = if wide { 160. } else { b.height - 176. };
        let selection = self
            .draft
            .as_ref()
            .map_or_else(|| self.committed.selection(), |d| d.record.selection());
        let title = if !selection.custom_bytes.is_empty() {
            "Saved custom skin".into()
        } else if selection.skin == 65 {
            "Saved skin 65".into()
        } else if selection.skin == 42 {
            "Saved skin 42".into()
        } else {
            format!("Skin {} / 64", selection.skin)
        };
        let control = |id, rect: [f64; 4], label: String, style, button, order| Control {
            id: ControlId(id),
            placement: Placement::rect(b.x + rect[0], b.y + rect[1], rect[2], rect[3]),
            clip: None,
            visible: true,
            enabled: self.interactive() && id != 0,
            alpha: 1.,
            name: label.clone(),
            tab_order: order,
            style,
            kind: if button {
                ControlKind::Button { label }
            } else {
                ControlKind::Label { text: label }
            },
        };
        let mut panel = PanelView {
            id: 3,
            generation: self.generation,
            placement: Placement::rect(0., 0., viewport[0], viewport[1]),
            visible: self.visible(),
            enabled: self.interactive(),
            alpha: self.alpha,
            modal: true,
            initial_focus: Some(NEXT),
            restore_focus: Some(crate::login_controls::CHANGE_SKIN),
            controls: vec![
                control(
                    0,
                    [0., 0., b.width, b.height],
                    String::new(),
                    ControlStyle::StudioSurface,
                    true,
                    None,
                ),
                control(
                    1,
                    [24., 24., b.width - 48., 40.],
                    "Skin studio".into(),
                    ControlStyle::Heading,
                    false,
                    None,
                ),
                control(
                    2,
                    [24., 68., b.width - 48., 28.],
                    "Built-in skins".into(),
                    ControlStyle::HelpText,
                    false,
                    None,
                ),
                control(
                    3,
                    [x, y, 232., 32.],
                    title,
                    ControlStyle::Heading,
                    false,
                    None,
                ),
                control(
                    10,
                    [x, y + 48., 108., 40.],
                    "Previous".into(),
                    ControlStyle::SecondaryButton,
                    true,
                    Some(0),
                ),
                control(
                    11,
                    [x + 124., y + 48., 108., 40.],
                    "Next".into(),
                    ControlStyle::SecondaryButton,
                    true,
                    Some(1),
                ),
                control(
                    4,
                    [24., b.height - 108., b.width - 48., 28.],
                    status.into(),
                    ControlStyle::HelpText,
                    false,
                    None,
                ),
                control(
                    12,
                    [b.width - 256., b.height - 64., 108., 40.],
                    "Cancel".into(),
                    ControlStyle::SecondaryButton,
                    true,
                    Some(2),
                ),
                control(
                    13,
                    [b.width - 132., b.height - 64., 108., 40.],
                    "Save".into(),
                    ControlStyle::PrimaryButton,
                    true,
                    Some(3),
                ),
            ],
        };
        // The passive surface shields the backdrop from inside clicks, using
        // the same hit/paint primitives as the server picker.
        panel.controls[0].kind = ControlKind::Surface;
        panel.controls[0].enabled = self.interactive();
        panel.controls.insert(
            0,
            Control {
                id: BACKDROP,
                placement: Placement::rect(0., 0., viewport[0], viewport[1]),
                clip: None,
                visible: true,
                enabled: self.interactive(),
                alpha: 0.88,
                name: "Cancel skin studio".into(),
                tab_order: None,
                style: ControlStyle::StudioSurface,
                kind: ControlKind::Backdrop,
            },
        );
        panel
    }
}
