//! Native login composition and control focus; no transport or independent clock.
use crate::{
    Result,
    cosmetics::Cosmetics,
    menu::{Menu, MenuFrame},
    menu_input_native::NativeMenuInput,
    text::TextCache,
};
use slither_client::{
    login_controls::{self, NICKNAME, PLAY, QUALITY},
    menu_controls::{ControlId, PanelInputState, PanelView},
    menu_input::{EditIntent, HostEditEffect, TextField},
    menu_layout::{MenuElement as E, MenuLayout},
    menu_preferences::MenuQuality,
    start_page::{MenuAction, MenuLocale, StartMenuIntent, StartMenuState},
};

pub struct NativeLogin {
    pub input: NativeMenuInput,
    pub champion: crate::panel_native::NativePanel,
    pub state: StartMenuState,
    pub layout: MenuLayout,
    pub controls: PanelInputState,
    pub panel: PanelView,
    scroll: std::collections::BTreeMap<TextField, f64>,
    menu: Menu,
    text: TextCache,
    area: [f64; 2],
    pixel_scale: f64,
    champion_session: slither_client::session_policy::View,
}
impl NativeLogin {
    pub fn set_wordmark(&mut self, enabled: bool) {
        self.menu.wordmark = enabled;
    }
    pub fn needs_redraw(&self) -> bool {
        // A terminal socket must not idle the host during the hidden death hold.
        self.state.view(true).visible || self.champion_session.death_pending
    }
    pub fn blocks_gameplay(&self) -> bool {
        self.champion.policy.visible()
            || matches!(
                self.state.view(true).phase,
                slither_client::start_page::MenuPhase::Login
                    | slither_client::start_page::MenuPhase::Connecting
            )
    }
    pub fn intercepts_pointer(&self, point: [f64; 2]) -> bool {
        self.blocks_gameplay()
            || self.controls.capture().is_some()
            || (self.state.view(true).visible
                && self.panel.resolve(self.area).is_ok_and(|controls| {
                    controls.iter().any(|r| r.control.visible && r.hit(point))
                }))
    }
    pub fn new(nickname: &str) -> Result<Self> {
        let mut state = StartMenuState::new(MenuLocale::English, true, MenuQuality::High);
        state.edit_nickname(&nickname.encode_utf16().collect::<Vec<_>>());
        let mut input = NativeMenuInput::default();
        input.state.sync_menu(&state.view(true), &[]);
        let mut text = TextCache::menu();
        let layout = crate::menu_specimen::layout(&mut text, &state.view(true), [960, 540], 1.)?;
        let panel = login_controls::panel(&state.view(true), &layout, [960., 540.]);
        Ok(Self {
            input,
            champion: crate::panel_native::NativePanel::new(
                &slither_client::session_policy::SessionPolicy::new().view(),
            ),
            state,
            layout,
            panel,
            controls: PanelInputState::default(),
            scroll: std::collections::BTreeMap::new(),
            menu: Menu::default(),
            text,
            area: [960., 540.],
            pixel_scale: 1.,
            champion_session: slither_client::session_policy::SessionPolicy::new().view(),
        })
    }
    pub fn step(
        &mut self,
        now: f64,
        admission: bool,
        playing: bool,
    ) -> Result<(Vec<StartMenuIntent>, Vec<HostEditEffect>)> {
        let intents = self
            .state
            .step_with_admission(now, false, admission)
            .map_err(str::to_owned)?;
        self.input.state.step_caret(now);
        if playing {
            self.state.begin_outgoing();
        }
        if intents.contains(&StartMenuIntent::FocusNickname) {
            self.controls.set_focus(NICKNAME);
        }
        let mut effects = self
            .input
            .state
            .sync_menu(&self.state.view(admission), &intents);
        effects.extend(self.sync_controls(admission));
        self.controls
            .step(
                &self.panel,
                now,
                slither_client::menu_theme::MenuTheme::default(),
            )
            .map_err(str::to_owned)?;
        if self.champion.policy.visible() || self.champion.policy.finished() {
            self.champion.step(
                now,
                self.state.view(admission).login_alpha,
                &self.champion_session,
            )?;
            // Champion host executes these against its own clipboard/IME owner.
        }
        if self.champion.policy.finished() {
            self.state.champion_finished();
        }
        if !self.champion.policy.visible() {
            effects.extend(std::mem::take(&mut self.champion.effects));
        }
        Ok((intents, effects))
    }
    pub fn observe_session(&mut self, phase: slither_client::session_policy::Phase) -> bool {
        self.state.attempt_ended(phase)
    }
    pub fn observe_game(&mut self, game: &crate::game::LocalGame) -> Result<()> {
        self.champion_session = game.champion_session();
        self.champion.observe(game);
        if self.champion.policy.finished() {
            self.state.champion_finished();
        }
        self.state
            .observe_session(game.login_session(), game.login_final())
            .map_err(str::to_owned)?;
        Ok(())
    }
    pub fn sync_return_focus(&mut self, admission: bool) -> Vec<HostEditEffect> {
        if self.state.view(admission).disabled {
            return Vec::new();
        }
        let intents: Vec<_> = self.state.take_focus_intent().into_iter().collect();
        if !intents.is_empty() {
            self.controls.set_focus(NICKNAME);
        }
        let mut effects = self
            .input
            .state
            .sync_menu(&self.state.view(admission), &intents);
        effects.extend(self.sync_controls(admission));
        effects
    }
    fn sync_controls(&mut self, admission: bool) -> Vec<HostEditEffect> {
        self.panel = login_controls::panel(&self.state.view(admission), &self.layout, self.area);
        self.controls.sync(&self.panel, &mut self.input.state)
    }
    pub fn focus(&mut self, focus: ControlId) -> Vec<HostEditEffect> {
        self.controls.set_focus(focus);
        self.controls.sync_focus(&self.panel, &mut self.input.state)
    }
    pub fn activate(&mut self, admission: bool) {
        let composing = self.input.state.editor(TextField::Nickname).composing();
        let view = self.state.view(admission);
        if view.phase == slither_client::start_page::MenuPhase::Returning && !view.play_disabled {
            self.state
                .action(MenuAction::Enter { composing }, admission);
            return;
        }
        if composing {
            return;
        }
        match self.controls.focus().or(self.panel.initial_focus) {
            Some(NICKNAME | PLAY) => {
                self.state
                    .action(MenuAction::Enter { composing: false }, admission);
            }
            Some(QUALITY) => {
                self.state.action(MenuAction::Quality, admission);
            }
            _ => {}
        }
    }
    pub fn event(
        &mut self,
        event: &winit::event::WindowEvent,
        admission: bool,
    ) -> crate::menu_input_native::NativeInputResult {
        use winit::event::WindowEvent;
        if matches!(event, WindowEvent::Focused(false)) {
            self.controls.blur();
        }
        if let WindowEvent::KeyboardInput { event, .. } = event {
            return self.key(event.into(), admission);
        }
        self.input.window_event(event)
    }
    /// Real and authored key events share return gating and generic Tab focus.
    pub fn key(
        &mut self,
        event: crate::menu_input_native::KeyInput<'_>,
        admission: bool,
    ) -> crate::menu_input_native::NativeInputResult {
        use crate::menu_input_native::NativeInputResult;
        use winit::{
            event::ElementState,
            keyboard::{Key, NamedKey},
        };
        let view = self.state.view(admission);
        if event.state == ElementState::Pressed {
            // #363: consume Enter through the existing terminal-session guard;
            // it must not become a control packet on the old live connection.
            if !event.repeat
                && event.logical == &Key::Named(NamedKey::Enter)
                && view.phase == slither_client::start_page::MenuPhase::Returning
                && !view.play_disabled
            {
                self.activate(admission);
                return NativeInputResult {
                    consumed: true,
                    effects: Vec::new(),
                };
            }
            if !view.disabled && !self.input.state.editor(TextField::Nickname).composing() {
                if event.logical == &Key::Named(NamedKey::Tab) {
                    self.controls.tab(&self.panel, self.input.shift_key());
                    return NativeInputResult {
                        consumed: true,
                        effects: self.controls.sync_focus(&self.panel, &mut self.input.state),
                    };
                }
                if matches!(self.controls.focus(), Some(PLAY | QUALITY))
                    && !event.repeat
                    && matches!(event.logical, Key::Named(NamedKey::Enter | NamedKey::Space))
                {
                    self.activate(admission);
                    return NativeInputResult {
                        consumed: true,
                        effects: Vec::new(),
                    };
                }
            }
        }
        self.input.key(event)
    }
    pub fn pointer(
        &mut self,
        point: [f64; 2],
        down: Option<bool>,
        admission: bool,
    ) -> Vec<HostEditEffect> {
        if self.champion.policy.visible() {
            return self
                .champion
                .pointer_scaled(&mut self.text, point, down, self.pixel_scale)
                .unwrap_or_else(|e| vec![HostEditEffect::ClipboardFailed(e)]);
        }
        let view = self.state.view(admission);
        self.menu.skin_hovered = view.visible && self.layout.hit_test(E::Skin, point[0], point[1]);
        if let Some(down) = down {
            self.menu.skin_pressed = down && self.menu.skin_hovered;
        }
        let mut effects = self.sync_controls(admission);
        let result = self.input.panel_pointer(
            &mut self.controls,
            &self.panel,
            &mut self.scroll,
            &mut self.text,
            self.area,
            point,
            down,
            self.pixel_scale,
        );
        match result {
            Ok((editing, intent)) => {
                effects.extend(editing);
                if let Some(intent) = intent {
                    match intent.control {
                        PLAY => {
                            self.state.action(MenuAction::Play, admission);
                        }
                        QUALITY => {
                            self.state.action(MenuAction::Quality, admission);
                        }
                        _ => {}
                    }
                }
            }
            Err(error) => effects.push(HostEditEffect::ClipboardFailed(error)),
        }
        let hovered = self.panel.resolve(self.area).is_ok_and(|controls| {
            controls
                .iter()
                .any(|r| r.control.id == PLAY && r.hit(point))
        });
        self.state.play_pointer(
            hovered,
            self.controls.capture() == Some(PLAY) && hovered,
            admission,
        );
        effects
    }
    pub fn prepare(
        &mut self,
        extent: [u32; 2],
        admission: bool,
        cosmetics: Option<&Cosmetics>,
    ) -> Result<MenuFrame> {
        self.prepare_scaled(extent, 1., admission, cosmetics)
    }
    pub fn prepare_host(
        &mut self,
        physical: [u32; 2],
        window_scale: f64,
        admission: bool,
        cosmetics: Option<&Cosmetics>,
    ) -> Result<MenuFrame> {
        crate::render::dimensions(physical, window_scale)?;
        self.prepare_scaled(physical, window_scale, admission, cosmetics)
    }
    fn prepare_scaled(
        &mut self,
        extent: [u32; 2],
        dpr: f64,
        admission: bool,
        cosmetics: Option<&Cosmetics>,
    ) -> Result<MenuFrame> {
        let mut view = self.state.view(admission);
        if self.champion.policy.finished() {
            let alpha = self.champion.policy.ordinary_alpha();
            view.nickname_alpha *= alpha;
            view.play_alpha *= alpha;
            view.skin_alpha *= alpha;
        }
        let area = extent.map(|n| f64::from(n) / dpr);
        self.pixel_scale = dpr;
        let mut fresh = crate::menu_specimen::layout(&mut self.text, &view, extent, dpr)?;
        if self.area == area {
            // Keep resize-time top across fade updates.
            fresh.top = self.layout.top;
        }
        self.layout = fresh;
        self.area = area;
        view.nickname = self.displayed_nickname();
        self.panel = login_controls::panel(&view, &self.layout, self.area);
        let panel = if self.champion.policy.visible() {
            self.champion.viewport = extent.map(|n| f64::from(n) / dpr);
            self.champion.panel = self
                .champion
                .policy
                .panel(&self.champion_session, self.champion.viewport);
            // Keep specialized logo/result producers, suppress the old pending label.
            view.champion_pending = false;
            Some(crate::menu::PanelPaint {
                panel: &self.champion.panel,
                input: &self.champion.controls,
                edit: &self.champion.input.state,
                scroll: &mut self.champion.scroll,
                theme: slither_client::menu_theme::MenuTheme::default(),
            })
        } else {
            Some(crate::menu::PanelPaint {
                panel: &self.panel,
                input: &self.controls,
                edit: &self.input.state,
                scroll: &mut self.scroll,
                theme: slither_client::menu_theme::MenuTheme::default(),
            })
        };
        self.menu.prepare_with_panel(
            &mut self.text,
            &view,
            &self.layout,
            extent,
            dpr,
            cosmetics,
            0,
            Some(&self.input.state),
            panel,
        )
    }
    pub fn displayed_nickname(&self) -> String {
        let editor = self.input.state.editor(TextField::Nickname);
        let mut text = editor.text().to_owned();
        if let Some(preedit) = editor.preedit() {
            let (anchor, caret) = editor.selection();
            text.replace_range(anchor.min(caret)..anchor.max(caret), &preedit.text);
        }
        text
    }
    pub fn cursor_area(&mut self) -> Result<Vec<HostEditEffect>> {
        if self.champion.policy.visible() {
            return self.champion.cursor_area(&mut self.text, self.pixel_scale);
        }
        let resolved = self.panel.resolve(self.area).map_err(str::to_owned)?;
        let Some(r) = resolved.iter().find(|r| r.control.id == NICKNAME) else {
            return Ok(Vec::new());
        };
        let Some((_, geometry)) = crate::editor_geometry::field_geometry(
            &mut self.text,
            r,
            &self.input.state,
            &mut self.scroll,
        )?
        else {
            return Ok(Vec::new());
        };
        let r = geometry.snapped_cursor_rect(self.pixel_scale);
        Ok(self.input.state.apply(EditIntent::CursorArea {
            position: [r.x, r.y],
            size: [r.width, r.height],
        }))
    }
}
