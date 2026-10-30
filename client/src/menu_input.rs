//! Copied menu editing policy. Hosts execute effects outside the core borrow.
//! See docs/start-menu.md for source rules and composition timing limits.

use crate::menu_preferences::MenuQuality;
use crate::start_page::{MenuAction, StartMenuIntent, StartMenuState, StartMenuView};
use std::ops::Range;

/// The two single-line editor policies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextField {
    /// Official UTF-16 ASCII substitution, limited to 24 units.
    Nickname,
    /// Official UTF-16 ASCII substitution, limited to 140 units; no wire encoding.
    Champion,
}

/// Convert a UTF-8 boundary to a UTF-16 offset. Interior bytes are rejected.
pub fn byte_to_utf16(text: &str, byte: usize) -> Option<usize> {
    text.get(..byte).map(|s| s.encode_utf16().count())
}

/// Convert a UTF-16 offset to UTF-8. Interior surrogate offsets are rejected.
pub fn utf16_to_byte(text: &str, unit: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == unit {
            return Some(byte);
        }
        units += ch.len_utf16();
    }
    (units == unit).then_some(text.len())
}

fn boundary(text: &str, byte: usize) -> usize {
    let mut byte = byte.min(text.len());
    while !text.is_char_boundary(byte) {
        byte -= 1;
    }
    byte
}

fn normalize(field: TextField, text: &str) -> String {
    // Nickname input: source/20b-skins-and-cosmetics.js:513-527.
    // Champion input: source/20b-skins-and-cosmetics.js:528-537; Save repeats
    // substitution/truncation in source/06-login-and-hud.js:321-323.
    // Retaining raw preedit until commit approximates Chromium's input-event
    // timing during composition; it does not establish real OS timing parity.
    match field {
        TextField::Nickname => crate::start_page::normalize_nickname(text),
        TextField::Champion => text
            .encode_utf16()
            .take(140)
            .map(|unit| {
                if (32..=127).contains(&unit) {
                    char::from(unit as u8)
                } else {
                    ' '
                }
            })
            .collect(),
    }
}

/// A raw IME preview; selection offsets are UTF-8 bytes, as supplied by winit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Preedit {
    /// Unnormalized Unicode, separate from the committed value and undo history.
    pub text: String,
    /// Optional byte range; invalid host offsets are clamped to scalar boundaries.
    pub selection: Option<(usize, usize)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Snapshot {
    text: String,
    anchor: usize,
    caret: usize,
}

/// Committed text and directional selection. All offsets are UTF-8 boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextEditState {
    field: TextField,
    current: Snapshot,
    preedit: Option<Preedit>,
    composing: bool,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

/// Selection movement in a single-line field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Movement {
    /// Previous committed Unicode scalar.
    Left,
    /// Next Unicode scalar.
    Right,
    /// Previous whitespace-delimited word.
    WordLeft,
    /// Next whitespace-delimited word.
    WordRight,
    /// Beginning of the field.
    Home,
    /// End of the field.
    End,
}

impl TextEditState {
    /// Create an editor from a normalized committed value.
    pub fn new(field: TextField, text: &str) -> Self {
        let text = normalize(field, text);
        let caret = text.len();
        Self {
            field,
            current: Snapshot {
                text,
                anchor: caret,
                caret,
            },
            preedit: None,
            composing: false,
            undo: Vec::new(),
            redo: Vec::new(),
        }
    }

    /// Committed value, excluding preedit.
    pub fn text(&self) -> &str {
        &self.current.text
    }
    /// Selection anchor and caret, in UTF-8 bytes.
    pub fn selection(&self) -> (usize, usize) {
        (self.current.anchor, self.current.caret)
    }
    /// Current raw IME preview.
    pub fn preedit(&self) -> Option<&Preedit> {
        self.preedit.as_ref()
    }
    /// Composition remains active across winit's empty preedit before Commit.
    pub fn composing(&self) -> bool {
        self.composing
    }
    fn range(&self) -> Range<usize> {
        self.current.anchor.min(self.current.caret)..self.current.anchor.max(self.current.caret)
    }
    fn cancel(&mut self) {
        self.preedit = None;
        self.composing = false;
    }
    fn checkpoint(&mut self) {
        // Bounded history: no unbounded retention of clipboard/editor contents.
        if self.undo.len() == 64 {
            self.undo.remove(0);
        }
        self.undo.push(self.current.clone());
        self.redo.clear();
    }
    fn replace(&mut self, inserted: &str) {
        let range = self.range();
        let mut text = self.current.text.clone();
        text.replace_range(range.clone(), inserted);
        let caret = normalize(self.field, &text[..range.start + inserted.len()]).len();
        let text = normalize(self.field, &text);
        let caret = boundary(&text, caret);
        if text != self.current.text {
            self.checkpoint();
            self.current = Snapshot {
                text,
                anchor: caret,
                caret,
            };
        } else {
            self.current.anchor = caret;
            self.current.caret = caret;
        }
        self.cancel();
    }
    fn select(&mut self, anchor: usize, caret: usize) {
        self.current.anchor = boundary(self.text(), anchor);
        self.current.caret = boundary(self.text(), caret);
    }
    fn target(&self, movement: Movement) -> usize {
        let caret = self.current.caret;
        match movement {
            Movement::Home => 0,
            Movement::End => self.text().len(),
            Movement::Left => self.text()[..caret]
                .char_indices()
                .next_back()
                .map_or(0, |(i, _)| i),
            Movement::Right => self.text()[caret..]
                .chars()
                .next()
                .map_or(caret, |c| caret + c.len_utf8()),
            Movement::WordLeft => {
                let prefix = self.text()[..caret].trim_end_matches(char::is_whitespace);
                prefix
                    .char_indices()
                    .rev()
                    .find(|(_, c)| c.is_whitespace())
                    .map_or(0, |(i, c)| i + c.len_utf8())
            }
            Movement::WordRight => {
                let suffix = &self.text()[caret..];
                let end = suffix.find(char::is_whitespace).unwrap_or(suffix.len());
                caret + end + suffix[end..].len()
                    - suffix[end..].trim_start_matches(char::is_whitespace).len()
            }
        }
    }
    fn move_caret(&mut self, movement: Movement, extend: bool) {
        let range = self.range();
        let target =
            if !extend && !range.is_empty() && matches!(movement, Movement::Left | Movement::Right)
            {
                if movement == Movement::Left {
                    range.start
                } else {
                    range.end
                }
            } else {
                self.target(movement)
            };
        self.current.caret = target;
        if !extend {
            self.current.anchor = target;
        }
    }
    fn delete(&mut self, backwards: bool) {
        let before = self.current.clone();
        if self.range().is_empty() {
            self.current.anchor = self.target(if backwards {
                Movement::Left
            } else {
                Movement::Right
            });
        }
        self.replace("");
        if before.text != self.current.text {
            *self.undo.last_mut().expect("replacement checkpoint") = before;
        }
    }
    fn history(&mut self, redo: bool) {
        let (source, destination) = if redo {
            (&mut self.redo, &mut self.undo)
        } else {
            (&mut self.undo, &mut self.redo)
        };
        if let Some(snapshot) = source.pop() {
            destination.push(std::mem::replace(&mut self.current, snapshot));
        }
        self.cancel();
    }
}

/// Gameplay keys whose releases must survive menu focus and modality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameplayKey {
    /// Left turn.
    Left,
    /// Right turn.
    Right,
    /// Up-arrow boost.
    Up,
    /// Space boost.
    Space,
}
impl GameplayKey {
    /// Existing input policy's key name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "ArrowLeft",
            Self::Right => "ArrowRight",
            Self::Up => "ArrowUp",
            Self::Space => "Space",
        }
    }
}

/// Copied input; no window, clipboard or DOM handles cross this seam.
#[derive(Clone, Debug, PartialEq)]
pub enum EditIntent {
    /// Set editor focus, cancelling the previous composition and pending clipboard.
    Focus(Option<TextField>),
    /// Window activation; blur clears all gameplay holds and composition.
    WindowFocused(bool),
    /// Modal open suspends gameplay and clears holds. Field focus can follow it.
    Modal(bool),
    /// Keyboard-produced text (ignored while IME owns composition).
    KeyText(String),
    /// Raw preedit text and optional UTF-8 byte selection.
    Preedit(String, Option<(usize, usize)>),
    /// IME enabled/disabled; Disabled cancels uncommitted text.
    ImeEnabled(bool),
    /// Exactly one IME commit, normalized like paste and ordinary input.
    ImeCommit(String),
    /// Explicit composition cancellation, without activation.
    CancelComposition,
    /// Select using UTF-8 byte offsets; also supports future pointer placement.
    Select {
        /// Anchor byte.
        anchor: usize,
        /// Caret byte.
        caret: usize,
    },
    /// Move the caret, extending from the anchor with Shift.
    Move {
        /// Direction.
        movement: Movement,
        /// Preserve anchor.
        extend: bool,
    },
    /// Delete the selection or one scalar before the caret.
    Backspace,
    /// Delete the selection or one scalar after the caret.
    Delete,
    /// Select all committed text.
    SelectAll,
    /// Copy selected text.
    Copy,
    /// Delete selected text only after successful clipboard write.
    Cut,
    /// Request clipboard text.
    Paste,
    /// Host completion for a clipboard read. Stale tokens are ignored.
    ClipboardRead {
        /// Request token.
        token: u64,
        /// Host result.
        result: Result<String, String>,
    },
    /// Host completion for a clipboard write.
    ClipboardWritten {
        /// Request token.
        token: u64,
        /// Host result.
        result: Result<(), String>,
    },
    /// Restore the previous committed text and selection.
    Undo,
    /// Restore the next committed text and selection.
    Redo,
    /// Enter down/up; repeats never activate controls.
    Enter {
        /// Press state.
        pressed: bool,
        /// Auto-repeat.
        repeat: bool,
    },
    /// Dispatch boundary expires duplicate text/Enter suppression after a commit.
    EndEventBatch,
    /// Always route releases; suspend presses while editing or modal.
    Gameplay {
        /// Physical gameplay key.
        key: GameplayKey,
        /// Press state.
        pressed: bool,
    },
    /// Logical candidate rectangle supplied by a future menu layout adapter.
    CursorArea {
        /// Logical x/y.
        position: [f64; 2],
        /// Logical width/height.
        size: [f64; 2],
    },
}

/// Host work, executed after releasing the editor borrow.
#[derive(Clone, Debug, PartialEq)]
pub enum HostEditEffect {
    /// Toggle host IME admission.
    ImeAllowed(bool),
    /// Both menu fields use normal single-line IME purpose.
    ImePurposeNormal,
    /// Update the logical IME candidate rectangle.
    ImeCursorArea {
        /// Logical x/y.
        position: [f64; 2],
        /// Logical width/height.
        size: [f64; 2],
    },
    /// Read the host clipboard.
    ReadClipboard(u64),
    /// Write the host clipboard and acknowledge success or failure.
    WriteClipboard {
        /// Request token.
        token: u64,
        /// Copied selection.
        text: String,
    },
    /// Report a host capability failure, never panic or delete failed cuts.
    ClipboardFailed(String),
    /// Route an existing gameplay key, including unconditional releases.
    Gameplay {
        /// Key.
        key: GameplayKey,
        /// Press state.
        pressed: bool,
    },
    /// Clear pointer-requested boost in addition to keyboard holds.
    ReleasePointerBoost,
    /// Request activation after IME ownership is resolved. Grants no admission.
    Activate(TextField),
}

/// Focus, modality and editing owner for the opt-in menu.
#[derive(Clone, Debug)]
pub struct MenuInputState {
    nickname: TextEditState,
    champion: TextEditState,
    focus: Option<TextField>,
    window_focused: bool,
    modal: bool,
    ime_enabled: bool,
    enter_held: bool,
    commit_guard: Option<String>,
    key_text_guard: Option<(TextField, String, TextEditState)>,
    token: u64,
    pending: Option<(u64, bool)>,
    modal_focus: Option<TextField>,
}
impl Default for MenuInputState {
    fn default() -> Self {
        Self {
            nickname: TextEditState::new(TextField::Nickname, ""),
            champion: TextEditState::new(TextField::Champion, ""),
            focus: None,
            window_focused: true,
            modal: false,
            ime_enabled: false,
            enter_held: false,
            commit_guard: None,
            key_text_guard: None,
            token: 0,
            pending: None,
            modal_focus: None,
        }
    }
}
impl MenuInputState {
    /// Read an editor's copied state.
    pub fn editor(&self, field: TextField) -> &TextEditState {
        match field {
            TextField::Nickname => &self.nickname,
            TextField::Champion => &self.champion,
        }
    }
    fn editor_mut(&mut self, field: TextField) -> &mut TextEditState {
        match field {
            TextField::Nickname => &mut self.nickname,
            TextField::Champion => &mut self.champion,
        }
    }
    /// Active field, if any. Window blur retains logical focus for restoration.
    pub fn focus(&self) -> Option<TextField> {
        self.focus
    }
    /// Gameplay press routing gate. Releases always bypass it.
    pub fn suspends_gameplay(&self) -> bool {
        !self.window_focused || self.modal || self.focus.is_some()
    }
    fn clear_composition(&mut self) {
        self.nickname.cancel();
        self.champion.cancel();
        self.ime_enabled = false;
        self.commit_guard = None;
        self.key_text_guard = None;
        self.enter_held = false;
        self.pending = None;
    }
    fn releases(effects: &mut Vec<HostEditEffect>) {
        for key in [
            GameplayKey::Left,
            GameplayKey::Right,
            GameplayKey::Up,
            GameplayKey::Space,
        ] {
            effects.push(HostEditEffect::Gameplay {
                key,
                pressed: false,
            });
        }
        effects.push(HostEditEffect::ReleasePointerBoost);
    }
    fn ime_effects(&self, effects: &mut Vec<HostEditEffect>) {
        effects.push(HostEditEffect::ImeAllowed(
            self.window_focused && self.focus.is_some(),
        ));
        if self.window_focused && self.focus.is_some() {
            effects.push(HostEditEffect::ImePurposeNormal);
        }
    }
    /// Reduce one copied intent and return host work.
    pub fn apply(&mut self, intent: EditIntent) -> Vec<HostEditEffect> {
        let mut effects = Vec::new();
        if !matches!(
            intent,
            EditIntent::KeyText(_)
                | EditIntent::ImeEnabled(_)
                | EditIntent::Preedit(..)
                | EditIntent::ImeCommit(_)
                | EditIntent::Enter { .. }
                | EditIntent::Gameplay { .. }
        ) {
            self.key_text_guard = None;
        }
        match intent {
            EditIntent::Focus(focus) => {
                if self.focus != focus {
                    self.clear_composition();
                    self.focus = focus;
                    if focus.is_some() {
                        Self::releases(&mut effects);
                    }
                    self.ime_effects(&mut effects);
                }
                return effects;
            }
            EditIntent::WindowFocused(focused) => {
                self.window_focused = focused;
                if !focused {
                    self.clear_composition();
                    Self::releases(&mut effects);
                }
                self.ime_effects(&mut effects);
                return effects;
            }
            EditIntent::Modal(modal) => {
                if modal != self.modal {
                    self.modal = modal;
                    if modal {
                        self.modal_focus = self.focus;
                        self.clear_composition();
                        self.focus = None;
                        Self::releases(&mut effects);
                    } else {
                        self.focus = self.modal_focus.take();
                    }
                    self.ime_effects(&mut effects);
                }
                return effects;
            }
            EditIntent::Gameplay { key, pressed } => {
                if !pressed || !self.suspends_gameplay() {
                    effects.push(HostEditEffect::Gameplay { key, pressed });
                }
                return effects;
            }
            EditIntent::EndEventBatch => {
                self.commit_guard = None;
                // Empty preedit can mean cancellation on hosts where Enabled is
                // long-lived. Defer it through the adjacent Commit, then clear.
                for editor in [&mut self.nickname, &mut self.champion] {
                    if editor.preedit.as_ref().is_some_and(|p| p.text.is_empty()) {
                        editor.cancel();
                    }
                }
                return effects;
            }
            _ => {}
        }
        let Some(field) = self.focus.filter(|_| self.window_focused) else {
            return effects;
        };
        match intent {
            EditIntent::ImeEnabled(enabled) => {
                self.pending = None;
                self.ime_enabled = enabled;
                if !enabled {
                    self.editor_mut(field).cancel();
                }
            }
            EditIntent::Preedit(text, selection) => {
                self.pending = None;
                let selection = selection.map(|(a, c)| (boundary(&text, a), boundary(&text, c)));
                let editor = self.editor_mut(field);
                // Empty preedit is also the mandatory pre-Commit event, not cancellation.
                editor.composing |= !text.is_empty();
                editor.preedit = Some(Preedit { text, selection });
            }
            EditIntent::ImeCommit(text) => {
                if self.ime_enabled || self.editor(field).composing() {
                    self.pending = None;
                    // Direct commits (for example an OS character picker) need
                    // not have preedit. Roll back identical key text from this
                    // batch so either event order produces one undoable edit.
                    if let Some((_, _, before)) =
                        self.key_text_guard
                            .take()
                            .filter(|(previous_field, previous_text, _)| {
                                *previous_field == field && *previous_text == text
                            })
                    {
                        *self.editor_mut(field) = before;
                    }
                    self.editor_mut(field).replace(&text);
                    self.commit_guard = Some(text);
                }
            }
            EditIntent::KeyText(text) => {
                if !self.editor(field).composing() && self.commit_guard.as_deref() != Some(&text) {
                    self.pending = None;
                    self.key_text_guard = Some((field, text.clone(), self.editor(field).clone()));
                    self.editor_mut(field).replace(&text);
                }
            }
            EditIntent::Enter { pressed, repeat } => {
                if !pressed {
                    self.enter_held = false;
                } else {
                    let owned = self.editor(field).composing() || self.commit_guard.is_some();
                    if !owned && !repeat && !self.enter_held {
                        effects.push(HostEditEffect::Activate(field));
                    }
                    // The OS owns composition completion; never insert raw preedit ourselves.
                    self.enter_held = true;
                }
            }
            EditIntent::CancelComposition => {
                self.pending = None;
                self.editor_mut(field).cancel();
                self.ime_enabled = false;
                effects.push(HostEditEffect::ImeAllowed(false));
                self.ime_effects(&mut effects);
            }
            EditIntent::CursorArea { position, size } => {
                if position.iter().chain(&size).all(|v| v.is_finite())
                    && size.iter().all(|v| *v >= 0.)
                {
                    effects.push(HostEditEffect::ImeCursorArea { position, size });
                }
            }
            EditIntent::ClipboardRead { token, result } => {
                if self.pending == Some((token, false)) {
                    self.pending = None;
                    match result {
                        Ok(text) => self.editor_mut(field).replace(&text),
                        Err(error) => effects.push(HostEditEffect::ClipboardFailed(error)),
                    }
                }
            }
            EditIntent::ClipboardWritten { token, result } => {
                if let Some((_, cut)) = self.pending.filter(|(pending, _)| *pending == token) {
                    self.pending = None;
                    match result {
                        Ok(()) if cut => self.editor_mut(field).replace(""),
                        Ok(()) => {}
                        Err(error) => effects.push(HostEditEffect::ClipboardFailed(error)),
                    }
                }
            }
            // The IME owns editing keys until commit/cancel; they must not mutate
            // the committed selection underneath the candidate string.
            _ if self.editor(field).composing() => {}
            EditIntent::Copy | EditIntent::Cut | EditIntent::Paste => {
                if intent != EditIntent::Paste && self.editor(field).range().is_empty() {
                    return effects;
                }
                self.token = self.token.wrapping_add(1);
                let cut = intent == EditIntent::Cut;
                self.pending = Some((self.token, cut));
                if intent == EditIntent::Paste {
                    effects.push(HostEditEffect::ReadClipboard(self.token));
                } else {
                    let editor = self.editor(field);
                    effects.push(HostEditEffect::WriteClipboard {
                        token: self.token,
                        text: editor.text()[editor.range()].to_owned(),
                    });
                }
            }
            intent => {
                self.pending = None;
                let editor = self.editor_mut(field);
                match intent {
                    EditIntent::Select { anchor, caret } => editor.select(anchor, caret),
                    EditIntent::Move { movement, extend } => editor.move_caret(movement, extend),
                    EditIntent::SelectAll => editor.select(0, editor.text().len()),
                    EditIntent::Backspace => editor.delete(true),
                    EditIntent::Delete => editor.delete(false),
                    EditIntent::Undo => editor.history(false),
                    EditIntent::Redo => editor.history(true),
                    _ => {}
                }
            }
        }
        effects
    }

    /// Consume #300's focus intents and disabled/hidden projection. Called after
    /// menu updates; it grants neither champion editing nor connection authority.
    pub fn sync_menu(
        &mut self,
        view: &StartMenuView,
        intents: &[StartMenuIntent],
    ) -> Vec<HostEditEffect> {
        if !view.visible || view.disabled || !view.ordinary_controls_visible {
            return self.apply(EditIntent::Focus(None));
        }
        if self.modal {
            return Vec::new();
        }
        if self.nickname.text() != view.nickname {
            self.nickname = TextEditState::new(TextField::Nickname, &view.nickname);
            self.pending = None;
            self.key_text_guard = None;
        }
        if intents.contains(&StartMenuIntent::FocusNickname) {
            self.apply(EditIntent::Focus(Some(TextField::Nickname)))
        } else {
            Vec::new()
        }
    }

    /// Route committed nickname and Enter through the existing menu admission
    /// gate. Play remains deferred until StartMenuState::step and grants no route.
    pub fn apply_menu(
        &mut self,
        intent: EditIntent,
        menu: &mut StartMenuState,
        admission: bool,
    ) -> Vec<HostEditEffect> {
        let view = menu.view(admission);
        let mut effects = self.sync_menu(&view, &[]);
        let edited = self.apply(intent);
        effects.extend(self.commit_menu(edited, menu, admission));
        effects
    }

    /// Complete an adapter event (including synchronous clipboard feedback),
    /// passing Enter to #300 and synchronizing its immediate disable/focus gate.
    pub fn commit_menu(
        &mut self,
        effects: Vec<HostEditEffect>,
        menu: &mut StartMenuState,
        admission: bool,
    ) -> Vec<HostEditEffect> {
        menu.edit_nickname(&self.nickname.text().encode_utf16().collect::<Vec<_>>());
        let mut remaining = Vec::new();
        for effect in effects {
            if effect == HostEditEffect::Activate(TextField::Nickname) {
                if !self.modal {
                    menu.action(MenuAction::Enter { composing: false }, admission);
                }
            } else {
                remaining.push(effect);
            }
        }
        remaining.extend(self.sync_menu(&menu.view(admission), &[]));
        remaining
    }
}

/// Minimal semantic roles for a later native accessibility bridge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibleRole {
    /// Editable field.
    TextInput,
    /// Activatable control.
    Button,
    /// Read-only status or last-life result.
    Status,
}
/// Supported semantic action; bridges dispatch it back to the policy owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessibleAction {
    /// Focus the nickname input.
    FocusNickname,
    /// Change the committed nickname via editor intents.
    EditNickname,
    /// Existing menu action, subject to admission.
    Menu(MenuAction),
}
/// A copied flat login tree in reading/tab order, with stable IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccessibleNode {
    /// Stable semantic identifier.
    pub id: &'static str,
    /// Control role.
    pub role: AccessibleRole,
    /// Accessible plain name.
    pub name: String,
    /// Committed value; never preedit or transport details.
    pub value: String,
    /// Effective keyboard focus.
    pub focused: bool,
    /// Whether actions can currently execute.
    pub enabled: bool,
    /// Actions supported when enabled.
    pub actions: Vec<AccessibleAction>,
}

/// Project only mounted ordinary login controls. Champion handoff stays explicitly
/// incomplete, with no fake working message/Save control or AccessKit dependency.
pub fn login_accessibility(view: &StartMenuView, input: &MenuInputState) -> Vec<AccessibleNode> {
    if !view.visible {
        return Vec::new();
    }
    let mut nodes = Vec::new();
    let mut add = |id, role, name: String, value: String, focused, enabled, actions| {
        nodes.push(AccessibleNode {
            id,
            role,
            name,
            value,
            focused,
            enabled,
            actions,
        });
    };
    if view.ordinary_controls_visible {
        let enabled = !view.disabled && !input.modal;
        add(
            "nickname",
            AccessibleRole::TextInput,
            view.wording.nickname.into(),
            view.nickname.clone(),
            enabled && input.window_focused && input.focus == Some(TextField::Nickname),
            enabled,
            vec![
                AccessibleAction::FocusNickname,
                AccessibleAction::EditNickname,
            ],
        );
        for (id, name, action) in [
            ("play", view.play_label, MenuAction::Play),
            ("skin", view.wording.change_skin, MenuAction::Skin),
            ("server", view.wording.choose_server, MenuAction::Server),
            (
                "quality",
                view.wording.graphics_quality,
                MenuAction::Quality,
            ),
        ] {
            add(
                id,
                AccessibleRole::Button,
                name.into(),
                if id == "quality" {
                    match view.quality {
                        MenuQuality::High => view.wording.quality_high,
                        MenuQuality::Low => view.wording.quality_low,
                    }
                    .into()
                } else {
                    String::new()
                },
                false,
                enabled,
                vec![AccessibleAction::Menu(action)],
            );
        }
    }
    if !view.last_life.label.is_empty() {
        add(
            "last-life",
            AccessibleRole::Status,
            view.last_life.label.into(),
            format!("{}{}", view.last_life.number, view.last_life.suffix),
            false,
            true,
            vec![],
        );
    }
    if view.spinner_visible || view.champion_pending {
        add(
            "status",
            AccessibleRole::Status,
            if view.champion_pending {
                "Champion message handoff pending"
            } else {
                "Connecting"
            }
            .into(),
            String::new(),
            false,
            true,
            vec![],
        );
    }
    nodes
}
