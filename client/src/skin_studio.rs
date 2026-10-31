//! Offline appearance transaction. No session, acquisition or transport authority.
use crate::{
    menu_controls::*,
    menu_layout::MenuRect,
    menu_preferences::{MenuPreferences, hex, unhex},
    menu_theme::ControlStyle,
    skin_builder::BuilderDraft,
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
/// Roving section tabs.
pub const BUILT_IN_TAB: ControlId = ControlId(6);
/// Builder section tab.
pub const BUILDER_TAB: ControlId = ControlId(7);
/// Builder actions after the windowed run strip.
pub const REMOVE: ControlId = ControlId(14);
/// Recolour the selected run with the focused palette.
pub const RECOLOUR: ControlId = ControlId(15);
/// Clear builder colours only.
pub const RESET: ControlId = ControlId(16);
/// Append the selected palette when below the expanded-colour cap.
pub const ADD: ControlId = ControlId(17);
/// Pointer navigation for the windowed run strip.
pub const RUN_PREVIOUS: ControlId = ControlId(18);
/// Next run in the bounded strip.
pub const RUN_NEXT: ControlId = ControlId(19);
/// Stable palette IDs are added to this base.
pub const SWATCH_BASE: u16 = 100;
/// Stable run indices are added to this base.
pub const RUN_BASE: u16 = 200;

mod builder_view;
pub use builder_view::Navigation;

/// Owned snapshot taken at explicit Play, before asynchronous profile acquisition.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppearanceSelection {
    /// Existing wire skin, including legacy 65.
    pub skin: u8,
    /// Exact admitted trailing login bytes; never renderer data.
    pub custom_bytes: Vec<u8>,
}
impl AppearanceSelection {
    /// Retained wire data follows the encoder's byte bound, without interpreting its prefix/runs.
    pub fn validate_wire(&self) -> Result<(), &'static str> {
        if self.skin > 65 {
            return Err("Unsupported skin");
        }
        if self.custom_bytes.len() > 255 {
            return Err("Invalid custom length");
        }
        Ok(())
    }
    /// Only newly authored studio patterns require the builder header and complete valid runs.
    pub fn validate_authored(&self) -> Result<(), &'static str> {
        self.validate_wire()?;
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
/// No grant producer is mounted. A later explicit importer can extend this snapshot.
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
        .validate_wire()?;
        AppearanceSelection {
            skin: self.custom_base,
            custom_bytes: self.custom_bytes.clone(),
        }
        .validate_wire()?;
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
/// Storage read outcome, separate from the versioned record grammar.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PreferenceInput {
    /// No file/key exists.
    Missing,
    /// An existing record could not be read; its contents must remain protected.
    Unreadable,
    /// Bytes were read but cannot represent a text record.
    Corrupt,
    /// Bounded text, including malformed or unknown-version records.
    Text(String),
}
/// Startup restoration result. Unknown or unreadable records disable writes for this run.
pub struct RestoredAppearance {
    /// Restored/default player choices.
    pub record: AppearanceRecord,
    /// Whether explicit Save may replace v2.
    pub persistence_enabled: bool,
}
impl RestoredAppearance {
    /// Read v2 first; only an absent v2 allows v1 migration. Never writes at startup.
    pub fn restore(v2: PreferenceInput, v1: PreferenceInput) -> Self {
        let legacy = match v2 {
            PreferenceInput::Text(saved) => {
                return Self {
                    record: AppearanceRecord::parse(&saved).unwrap_or_default(),
                    persistence_enabled: saved.split(':').next() == Some("2"),
                };
            }
            PreferenceInput::Unreadable => {
                return Self {
                    record: Default::default(),
                    persistence_enabled: false,
                };
            }
            PreferenceInput::Corrupt => {
                return Self {
                    record: Default::default(),
                    persistence_enabled: true,
                };
            }
            PreferenceInput::Missing => match v1 {
                PreferenceInput::Text(saved) => saved,
                PreferenceInput::Unreadable => {
                    return Self {
                        record: Default::default(),
                        persistence_enabled: false,
                    };
                }
                PreferenceInput::Missing | PreferenceInput::Corrupt => String::new(),
            },
        };
        let old = MenuPreferences::restore(false, &legacy).view();
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
/// Mounted studio sections; cosmetics remain a later delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StudioSection {
    /// Built-in selection, including retained legacy choices.
    BuiltIn,
    /// Bounded custom-pattern editor.
    Builder,
}
/// Bounded draft; large imported custom runs remain opaque and unchanged.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkinDraft {
    /// Complete transaction, retaining inactive custom bytes.
    pub record: AppearanceRecord,
    /// Editor contents are separate from retained wire bytes.
    pub builder: BuilderDraft,
    builder_active: bool,
    builder_base: u8,
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
    /// Active section; switching retains all transaction contents.
    pub section: StudioSection,
    section_started: Option<f64>,
    section_alpha: f64,
    editor_scroll: ScrollList,
    run_window: ScrollList,
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
            section_started: None,
            section_alpha: 1.,
            editor_scroll: ScrollList::new(1),
            run_window: ScrollList::new(4),
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
            builder: BuilderDraft::new(&self.committed.custom_bytes),
            builder_active: false,
            builder_base: if self.committed.custom_bytes.is_empty() {
                self.committed.builtin
            } else {
                self.committed.custom_base
            },
        });
        self.section = StudioSection::BuiltIn;
        self.section_started = None;
        self.section_alpha = 1.;
        self.editor_scroll = ScrollList::new(1);
        self.run_window = ScrollList::new(4);
        self.generation += 1;
        self.phase = Phase::Entering(now);
        self.alpha = 0.;
        self.restore_focus = false;
        true
    }
    /// Sample both transitions from the caller clock, without intervals.
    pub fn step(&mut self, now: f64) {
        if let Some(at) = self.section_started {
            self.section_alpha = ((now - at) / 180.).clamp(0., 1.);
        }
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
        self.draft.as_mut().expect("mounted draft").builder_active = false;
    }
    /// Switch the mounted controls, cancelling capture through the panel generation.
    pub fn set_section(&mut self, section: StudioSection, now: f64) {
        if !self.interactive() || section == self.section || !now.is_finite() {
            return;
        }
        self.section = section;
        self.section_started = Some(now);
        self.section_alpha = 0.;
        self.generation += 1;
        if section == StudioSection::Builder {
            let draft = self.draft.as_mut().expect("mounted draft");
            if draft.record.custom_bytes.is_empty() && draft.builder.pattern().colours() == 0 {
                draft.builder_base = draft.record.builtin;
            }
            draft.builder_active = true;
        }
    }
    /// Mutate only the bounded builder; record bytes remain untouched until Save.
    pub fn edit_builder(&mut self, edit: impl FnOnce(&mut BuilderDraft)) {
        if self.interactive() && self.section == StudioSection::Builder {
            let draft = self.draft.as_mut().expect("mounted draft");
            edit(&mut draft.builder);
            draft.builder_active = true;
            self.run_window
                .reveal(draft.builder.selected, draft.builder.pattern().runs().len());
        }
    }
    /// Preview uses no entropy and contains no serialized spacer or special palette.
    pub fn preview_selection(&self) -> Result<AppearanceSelection, &'static str> {
        Ok(self.draft_record()?.selection())
    }
    fn draft_record(&self) -> Result<AppearanceRecord, &'static str> {
        let draft = self.draft.as_ref().ok_or("Studio draft unavailable")?;
        let mut record = draft.record.clone();
        // Save the whole transaction, even when a later built-in choice makes the new custom inactive.
        if draft.builder.changed() && draft.builder.pattern().colours() > 0 {
            record.custom_base = draft.builder_base;
            record.custom_bytes = draft
                .builder
                .pattern()
                .encode([255, 255, 255, 0, 0, 0, 0, 0])?;
        }
        if draft.builder_active {
            if draft.builder.pattern().colours() == 0 {
                record.active_custom = !self.committed.custom_bytes.is_empty();
                record.builtin = self.committed.builtin;
                record.custom_base = self.committed.custom_base;
                record.custom_bytes = self.committed.custom_bytes.clone();
            } else {
                record.active_custom = true;
            }
        }
        Ok(record)
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
        random_byte: &mut impl FnMut() -> u8,
    ) -> Result<Option<AppearanceRecord>, &'static str> {
        if !self.interactive() {
            return Ok(None);
        }
        let mut record = self.draft_record()?;
        record.nickname = nickname.into();
        record.validate()?;
        if record.custom_bytes != self.committed.custom_bytes {
            AppearanceSelection {
                skin: record.custom_base,
                custom_bytes: record.custom_bytes.clone(),
            }
            .validate_authored()?;
        }
        // A retained legacy choice remains byte-preserving. New selections
        // must still be available at commit, independently of ready artwork.
        if record.selection() != self.committed.selection()
            && !record.active_custom
            && !grants.skin_available(record.selection().skin)
        {
            return Err("Skin unavailable");
        }
        let draft = self.draft.as_ref().expect("mounted draft");
        if draft.builder.changed() && draft.builder.pattern().colours() > 0 {
            // Sample only after every fallible validation; repaints and retained Saves use none.
            record.custom_bytes[6] = random_byte();
            record.custom_bytes[7] = random_byte();
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
                height: b.height - 232.,
            }
        } else {
            MenuRect {
                x: b.x + 24.,
                y: b.y + 112.,
                width: b.width - 48.,
                height: (b.height - 352.).max(12.),
            }
        }
    }
    /// Concrete active-section controls share the preview, status band and footer.
    pub fn panel(&self, viewport: [f64; 2], status: &str) -> PanelView {
        let b = Self::bounds(viewport);
        let wide = b.width >= 640.;
        let x = if wide { b.width - 280. } else { 24. };
        // Navigation ends at least 12px above the reserved status band, even
        // in short panels. The footer keeps its existing bottom inset.
        let y = if wide {
            160f64.min(b.height - 208.)
        } else {
            b.height - 208.
        };
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
                    ControlStyle::StudioCancel,
                    true,
                    Some(2),
                ),
                control(
                    13,
                    [b.width - 132., b.height - 64., 108., 40.],
                    "Save".into(),
                    ControlStyle::StudioSave,
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
        self.mount_sections(&mut panel, viewport);
        // Paint and copied reading order follow the same section-first flow as Tab.
        panel.controls.sort_by_key(|c| match c.id {
            BACKDROP => 0,
            ControlId(0) | ControlId(1) => 1,
            BUILT_IN_TAB | BUILDER_TAB => 2,
            ControlId(4) => 4,
            CANCEL => 5,
            SAVE => 6,
            _ => 3,
        });
        panel
    }
}
