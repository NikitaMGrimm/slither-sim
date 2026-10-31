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
    io::Read,
    path::{Path, PathBuf},
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
        remaining
    }
    pub fn key(&mut self, event: KeyInput<'_>) -> NativeInputResult {
        let composing = self.input.state.editor(SEARCH_EDITOR).composing();
        let mut effects = Vec::new();
        if event.state == ElementState::Pressed && !composing {
            if matches!(
                event.logical,
                Key::Named(
                    NamedKey::Enter
                        | NamedKey::Space
                        | NamedKey::ArrowLeft
                        | NamedKey::ArrowRight
                        | NamedKey::Home
                        | NamedKey::End
                )
            ) {
                self.controls.keyboard_navigation();
            }
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
                ) if self.controls.focus() != Some(SEARCH)
                    || matches!(
                        key,
                        NamedKey::ArrowDown
                            | NamedKey::ArrowUp
                            | NamedKey::PageDown
                            | NamedKey::PageUp
                    ) =>
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
                    if self.controls.focus() != Some(SEARCH) && !event.repeat =>
                {
                    if let Some(intent) = self.controls.activate(&self.panel, Activation::Enter) {
                        self.policy.activate(intent);
                    }
                }
                _ => return self.input.key(event),
            }
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
        let Some(r) = resolved.iter().find(|r| r.control.id == SEARCH) else {
            return Ok(None);
        };
        let ControlKind::TextField { inset, .. } = r.control.kind else {
            return Ok(None);
        };
        let mut style = MenuTheme::default().text_style(r.control.style);
        style.size *= r.scale;
        Ok(Some(EditorGeometry::new(
            text,
            self.input.state.editor(SEARCH_EDITOR),
            r.rect,
            inset.map(|n| n * r.scale),
            style,
            0.,
            false,
        )?))
    }
    pub fn pointer(
        &mut self,
        text: &mut TextCache,
        point: [f64; 2],
        down: Option<bool>,
    ) -> Result<Vec<HostEditEffect>> {
        if self.input.state.editor(SEARCH_EDITOR).composing() {
            if down == Some(false) {
                self.controls.blur();
            }
            return Ok(Vec::new());
        }
        let resolved = self.panel.resolve(self.viewport).map_err(str::to_owned)?;
        let intent = self.controls.pointer(&self.panel, &resolved, point, down);
        let effects = self.controls.sync_focus(&self.panel, &mut self.input.state);
        if down == Some(true) && self.controls.focus() == Some(SEARCH) {
            // The reference deliberately clears numeric search on every click.
            self.input.state.reset_editor(SEARCH_EDITOR, "");
        }
        if let Some(intent) = intent {
            self.policy.activate(intent);
        }
        // Keep geometry on the same measured field path as the shared painter.
        self.geometry(text)?;
        Ok(effects)
    }
    pub fn cursor_area(&mut self, text: &mut TextCache, dpr: f64) -> Result<Vec<HostEditEffect>> {
        if self.controls.focus() != Some(SEARCH) {
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
        .and_then(|file| file.take(33).read_to_string(&mut bytes).ok())
        .filter(|_| bytes.len() <= 32)
        .map(|_| bytes.as_str());
    ServerPreference::restore(value)
}
pub fn write_preference(path: &Path, value: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(path, value).map_err(|e| e.to_string())
}
