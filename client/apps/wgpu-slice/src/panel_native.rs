//! Thin native binding for generic controls; policy/transport remain elsewhere.
use crate::{
    Result,
    editor_geometry::EditorGeometry,
    menu_input_native::{NativeInputResult, NativeMenuInput},
    text::TextCache,
};
use slither_client::{
    champion::{ChampionState, MESSAGE},
    menu_controls::*,
    menu_input::{EditIntent, EditorId, HostEditEffect},
    menu_theme::MenuTheme,
};
use std::collections::BTreeMap;
use winit::{
    event::{ElementState, WindowEvent},
    keyboard::{Key, NamedKey},
};

pub struct NativePanel {
    pub input: NativeMenuInput,
    pub controls: PanelInputState,
    pub policy: ChampionState,
    pub panel: PanelView,
    pub scroll: BTreeMap<EditorId, f64>,
    pub viewport: [f64; 2],
    pub effects: Vec<HostEditEffect>,
    submission: Option<(u32, Vec<u16>, Activation)>,
    drag_anchor: usize,
}
impl NativePanel {
    pub fn new(session: &slither_client::session_policy::View) -> Self {
        let policy = ChampionState::default();
        Self {
            panel: policy.panel(session, [960., 540.]),
            policy,
            input: NativeMenuInput::default(),
            controls: PanelInputState::default(),
            scroll: BTreeMap::new(),
            viewport: [960., 540.],
            effects: Vec::new(),
            submission: None,
            drag_anchor: 0,
        }
    }
    pub fn observe(&mut self, game: &crate::game::LocalGame) {
        let session = game.champion_session();
        let fresh = self.policy.observe(game.generation(), &session);
        if fresh {
            self.input.state.reset_editor(EditorId::Champion, "");
            self.scroll.clear();
            self.submission = None;
        }
        self.panel = self.policy.panel(&session, self.viewport);
        if self.policy.visible() || self.policy.finished() {
            self.effects
                .extend(self.controls.sync(&self.panel, &mut self.input.state));
        }
    }
    pub fn step(
        &mut self,
        now: f64,
        alpha: f64,
        session: &slither_client::session_policy::View,
    ) -> Result<()> {
        self.policy.step(now, alpha).map_err(str::to_owned)?;
        self.panel = self.policy.panel(session, self.viewport);
        self.effects
            .extend(self.controls.sync(&self.panel, &mut self.input.state));
        self.controls
            .step(&self.panel, now, MenuTheme::default())
            .map_err(str::to_owned)
    }
    fn activate(&mut self, intent: Option<ControlIntent>) {
        if let Some(activation) = intent.and_then(|i| self.policy.save_intent(&self.panel, i)) {
            self.submission = Some((
                self.panel.generation as u32,
                self.input
                    .state
                    .editor(EditorId::Champion)
                    .text()
                    .encode_utf16()
                    .collect(),
                activation,
            ));
        }
    }
    pub fn commit_effects(&mut self, effects: Vec<HostEditEffect>) -> Vec<HostEditEffect> {
        let mut remaining = Vec::new();
        for effect in effects {
            if matches!(effect, HostEditEffect::Activate(_)) {
                self.activate(self.controls.activate(&self.panel, Activation::Enter));
            } else {
                remaining.push(effect);
            }
        }
        remaining
    }
    pub fn take_submission(&mut self) -> Option<(u32, Vec<u16>, Activation)> {
        self.submission.take()
    }
    pub fn event(&mut self, event: &WindowEvent) -> NativeInputResult {
        if matches!(event, WindowEvent::Focused(false)) {
            self.controls.blur();
        }
        if let WindowEvent::KeyboardInput { event, .. } = event
            && event.state == ElementState::Pressed
        {
            let composing = self
                .input
                .state
                .focus()
                .is_some_and(|id| self.input.state.editor(id).composing());
            if event.logical_key == Key::Named(NamedKey::Tab) && !composing {
                self.controls.tab(&self.panel, self.input.shift_key());
                return NativeInputResult {
                    consumed: true,
                    effects: self.controls.sync_focus(&self.panel, &mut self.input.state),
                };
            }
            if self.controls.focus() != Some(MESSAGE)
                && !event.repeat
                && matches!(
                    event.logical_key,
                    Key::Named(NamedKey::Enter | NamedKey::Space)
                )
            {
                self.activate(self.controls.activate(
                    &self.panel,
                    if event.logical_key == Key::Named(NamedKey::Space) {
                        Activation::Space
                    } else {
                        Activation::Enter
                    },
                ));
                return NativeInputResult {
                    consumed: true,
                    effects: Vec::new(),
                };
            }
        }
        self.input.window_event(event)
    }
    pub fn geometry(&mut self, text: &mut TextCache) -> Result<Option<EditorGeometry>> {
        let resolved = self.panel.resolve(self.viewport).map_err(str::to_owned)?;
        let Some(r) = resolved.iter().find(|r| r.control.id == MESSAGE) else {
            return Ok(None);
        };
        let ControlKind::TextField { editor, inset, .. } = r.control.kind else {
            return Ok(None);
        };
        let mut style = MenuTheme::default().text_style(r.control.style);
        style.size *= r.scale;
        let rect = slither_client::menu_layout::MenuRect {
            x: r.rect.x + inset[0] * r.scale,
            y: r.rect.y + inset[1] * r.scale,
            width: r.rect.width - 2. * inset[0] * r.scale,
            height: r.rect.height - 2. * inset[1] * r.scale,
        };
        let g = EditorGeometry::new(
            text,
            self.input.state.editor(editor),
            rect,
            style,
            self.scroll.get(&editor).copied().unwrap_or(0.),
        )?;
        self.scroll.insert(editor, g.scroll);
        Ok(Some(g))
    }
    pub fn pointer(
        &mut self,
        text: &mut TextCache,
        point: [f64; 2],
        down: Option<bool>,
    ) -> Result<Vec<HostEditEffect>> {
        if self.input.state.editor(EditorId::Champion).composing() {
            return Ok(Vec::new());
        }
        let resolved = self.panel.resolve(self.viewport).map_err(str::to_owned)?;
        let dragging = self.controls.capture() == Some(MESSAGE);
        let intent = self.controls.pointer(&self.panel, &resolved, point, down);
        let mut effects = self.controls.sync_focus(&self.panel, &mut self.input.state);
        if self.controls.focus() == Some(MESSAGE)
            && (down == Some(true) || dragging)
            && let Some(g) = self.geometry(text)?
        {
            let editor = self.input.state.editor(EditorId::Champion);
            let caret = g.hit(text, editor.text(), point[0])?;
            if down == Some(true) {
                self.drag_anchor = caret;
            }
            effects.extend(self.input.state.apply(EditIntent::Select {
                anchor: self.drag_anchor,
                caret,
            }));
        }
        self.activate(intent);
        Ok(effects)
    }
    pub fn cursor_area(&mut self, text: &mut TextCache) -> Result<Vec<HostEditEffect>> {
        if self.input.state.focus().is_none() {
            return Ok(Vec::new());
        }
        if let Some(g) = self.geometry(text)? {
            let r = g.cursor_rect();
            return Ok(self.input.state.apply(EditIntent::CursorArea {
                position: [r.x, r.y],
                size: [r.width, r.height],
            }));
        }
        Ok(Vec::new())
    }
}
