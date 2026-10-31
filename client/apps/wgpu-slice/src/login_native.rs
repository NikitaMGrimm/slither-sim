//! Native login composition and control focus; no transport or independent clock.
use crate::{
    Result,
    cosmetics::Cosmetics,
    menu::{Menu, MenuFrame},
    menu_input_native::NativeMenuInput,
    text::TextCache,
};
use slither_client::{
    login_controls::{self, CHANGE_SKIN, NICKNAME, PLAY, SERVER, SETTINGS},
    menu_controls::{ControlId, PanelInputState, PanelView},
    menu_input::{EditIntent, HostEditEffect, TextField},
    menu_layout::{MenuElement as E, MenuLayout},
    menu_preferences::MenuQuality,
    start_page::{MenuAction, MenuLocale, StartMenuIntent, StartMenuState},
};

pub struct NativeLogin {
    settings_storage: crate::menu_storage::SettingsStorage,
    pub studio: crate::skin_studio_native::SkinStudioNative,
    pub input: NativeMenuInput,
    pub champion: crate::panel_native::NativePanel,
    pub picker: crate::server_picker_native::NativeServerPicker,
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
    pub fn needs_redraw(&self) -> bool {
        // A terminal socket must not idle the host during the hidden death hold.
        self.state.view(true).visible || self.champion_session.death_pending
    }
    pub fn blocks_gameplay(&self) -> bool {
        self.picker.policy.visible()
            || self.champion.policy.visible()
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
        Self::with_preferences(
            slither_client::skin_studio::AppearanceRecord {
                nickname: slither_client::start_page::normalize_nickname(nickname),
                ..Default::default()
            },
            crate::menu_storage::MenuStorage::at(None),
        )
    }
    pub fn with_preferences(
        record: slither_client::skin_studio::AppearanceRecord,
        storage: crate::menu_storage::MenuStorage,
    ) -> Result<Self> {
        let nickname = &record.nickname;
        let mut state = StartMenuState::new(MenuLocale::English, true, MenuQuality::High);
        state.edit_nickname(&nickname.encode_utf16().collect::<Vec<_>>());
        let mut input = NativeMenuInput::default();
        input.state.sync_menu(&state.view(true), &[]);
        let mut text = TextCache::menu();
        let layout = crate::menu_specimen::layout(&mut text, &state.view(true), [960, 540], 1.)?;
        let panel = login_controls::panel(&state.view(true), &layout, [960., 540.]);
        Ok(Self {
            settings_storage: crate::menu_storage::SettingsStorage::at(None),
            studio: crate::skin_studio_native::SkinStudioNative::new(record, storage)?,
            input,
            champion: crate::panel_native::NativePanel::new(
                &slither_client::session_policy::SessionPolicy::new().view(),
            ),
            state,
            picker: crate::server_picker_native::NativeServerPicker::default(),
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
        self.studio.step(now);
        if let Some(record) = self.state.settings.take_write() {
            self.state
                .settings
                .storage_result(self.settings_storage.save(record).is_ok());
        }
        let intents = self
            .state
            .step_with_admission(now, false, admission)
            .map_err(str::to_owned)?;
        self.input.state.step_caret(now);
        if playing {
            self.state.begin_outgoing();
        }
        if !self.state.settings.visible()
            && !self.studio.state.visible()
            && intents.contains(&StartMenuIntent::FocusNickname)
        {
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
        let picker_visible = self.picker.panel.visible || self.picker.policy.visible();
        let picker_effects = self.picker.step(now)?;
        if picker_visible {
            effects = picker_effects;
            if !self.picker.policy.visible() {
                effects.extend(self.focus(NICKNAME));
            }
        }
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
        if self.studio.state.visible() || self.state.settings.visible() {
            return Vec::new();
        }
        if self.studio.state.take_return_focus()
            && self.state.view(true).phase == slither_client::start_page::MenuPhase::Login
        {
            self.menu.skin_hovered = false;
            self.menu.skin_pressed = false;
            self.controls.blur();
            let mut effects = self.sync_controls(admission);
            effects.extend(self.focus(CHANGE_SKIN));
            return effects;
        }
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
        self.panel = if self.state.settings.visible() {
            self.state.settings.panel(self.area)
        } else if self.studio.state.visible() {
            self.studio.state.panel(self.area, &self.studio.status)
        } else {
            let mut panel =
                login_controls::panel(&self.state.view(admission), &self.layout, self.area);
            if self.studio.status == "Saved for this run" {
                panel.controls.push(slither_client::menu_controls::Control {
                    id: ControlId(30),
                    placement: slither_client::menu_controls::Placement::rect(
                        self.area[0] / 2. - 120.,
                        self.area[1] - 32.,
                        240.,
                        28.,
                    ),
                    clip: None,
                    visible: true,
                    enabled: false,
                    alpha: 1.,
                    name: self.studio.status.clone(),
                    tab_order: None,
                    style: slither_client::menu_theme::ControlStyle::HelpText,
                    kind: slither_client::menu_controls::ControlKind::Label {
                        text: self.studio.status.clone(),
                    },
                });
            }
            panel
        };
        self.controls.sync(&self.panel, &mut self.input.state)
    }
    pub fn open_studio(&mut self) -> Vec<HostEditEffect> {
        let admitted = self.state.action(MenuAction::Skin, true)
            == Some(StartMenuIntent::OpenSkinChooser)
            && !self.champion.policy.visible()
            && !self.picker.policy.visible()
            && !self.state.settings.visible();
        if self.studio.open(admitted) {
            self.menu.skin_hovered = false;
            self.menu.skin_pressed = false;
            self.state.play_pointer(false, false, true);
            self.controls.blur();
            self.sync_controls(true)
        } else {
            Vec::new()
        }
    }
    pub fn focus(&mut self, focus: ControlId) -> Vec<HostEditEffect> {
        self.controls.set_focus(focus);
        self.controls.sync_focus(&self.panel, &mut self.input.state)
    }
    /// Startup-only restoration; never writes or changes appearance storage.
    pub fn restore_settings(&mut self, storage: crate::menu_storage::SettingsStorage) {
        self.state.settings = slither_client::settings::SettingsState::new(storage.restore());
        self.settings_storage = storage;
    }
    pub fn open_settings(&mut self, admission: bool) -> Vec<HostEditEffect> {
        if self.studio.state.visible()
            || self.picker.policy.visible()
            || self.champion.policy.visible()
            || self.input.state.editor(TextField::Nickname).composing()
        {
            return Vec::new();
        }
        let view = self.state.view(admission);
        if self.state.settings.open(&view) {
            self.sync_controls(admission)
        } else {
            Vec::new()
        }
    }
    pub fn close_settings(&mut self, admission: bool) -> Vec<HostEditEffect> {
        self.state.settings.close();
        let mut effects = self.sync_controls(admission);
        effects.extend(self.focus(SETTINGS));
        effects
    }
    fn settings_activation(
        &mut self,
        intent: slither_client::menu_controls::ControlIntent,
        admission: bool,
    ) -> Vec<HostEditEffect> {
        self.state.settings.activate(intent);
        if self.state.settings.visible() {
            self.sync_controls(admission)
        } else {
            self.close_settings(admission)
        }
    }
    pub fn activate(&mut self, admission: bool) {
        if self.picker.policy.visible() {
            return;
        }
        if self.state.settings.visible() {
            if let Some(intent) = self.controls.activate(
                &self.panel,
                slither_client::menu_controls::Activation::Enter,
            ) {
                self.settings_activation(intent, admission);
            }
            return;
        }
        if self.studio.state.visible() {
            if let Some(id) = self.controls.focus() {
                self.studio.activate(id, &self.state.view(true).nickname);
            }
            return;
        }
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
            Some(SETTINGS) => {
                self.open_settings(admission);
            }
            Some(SERVER) if slither_client::server_picker::available(&view) => {
                self.picker.policy.open();
                self.picker
                    .input
                    .state
                    .reset_editor(slither_client::server_picker::SEARCH_EDITOR, "");
            }
            Some(CHANGE_SKIN) => {
                let admitted = self.state.action(MenuAction::Skin, true)
                    == Some(StartMenuIntent::OpenSkinChooser)
                    && !self.champion.policy.visible();
                self.studio.open(admitted);
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
        if matches!(
            event,
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. }
        ) {
            self.controls.blur();
        }
        if let WindowEvent::KeyboardInput { event, .. } = event {
            return self.key(event.into(), admission);
        }
        if self.studio.state.visible()
            && let WindowEvent::MouseWheel { delta, .. } = event
        {
            use winit::event::MouseScrollDelta;
            self.studio.wheel += match delta {
                MouseScrollDelta::LineDelta(_, y) => -f64::from(*y),
                MouseScrollDelta::PixelDelta(p) => -p.y / 28.,
            };
            let rows = self.studio.wheel.trunc() as isize;
            self.studio.wheel -= rows as f64;
            self.studio.state.scroll_editor(rows, self.area);
            self.controls.blur();
            return crate::menu_input_native::NativeInputResult {
                consumed: true,
                effects: self.sync_controls(admission),
            };
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
        if event.state == ElementState::Pressed
            && matches!(
                event.logical,
                Key::Named(
                    NamedKey::Tab
                        | NamedKey::Enter
                        | NamedKey::Space
                        | NamedKey::ArrowLeft
                        | NamedKey::ArrowRight
                        | NamedKey::ArrowUp
                        | NamedKey::ArrowDown
                        | NamedKey::Home
                        | NamedKey::End
                )
            )
        {
            self.controls.keyboard_navigation();
        }
        if self.state.settings.visible() {
            let mut effects = Vec::new();
            if event.state == ElementState::Pressed && !event.repeat {
                match event.logical {
                    Key::Named(NamedKey::Escape) => effects.extend(self.close_settings(admission)),
                    Key::Named(NamedKey::Tab) => {
                        self.controls.tab(&self.panel, self.input.shift_key())
                    }
                    Key::Named(NamedKey::Enter | NamedKey::Space) => {
                        let activation = if event.logical == &Key::Named(NamedKey::Space) {
                            slither_client::menu_controls::Activation::Space
                        } else {
                            slither_client::menu_controls::Activation::Enter
                        };
                        if let Some(intent) = self.controls.activate(&self.panel, activation) {
                            effects.extend(self.settings_activation(intent, admission));
                        }
                    }
                    _ => {}
                }
                effects.extend(self.controls.sync_focus(&self.panel, &mut self.input.state));
            }
            return NativeInputResult {
                consumed: true,
                effects,
            };
        }
        if self.studio.state.visible() {
            use slither_client::skin_studio::Navigation;
            let mut effects = Vec::new();
            if event.state == ElementState::Pressed && !event.repeat {
                match event.logical {
                    Key::Named(NamedKey::Escape) => self.studio.state.cancel(self.studio.now),
                    Key::Named(NamedKey::Tab) => {
                        self.controls.tab(&self.panel, self.input.shift_key())
                    }
                    Key::Named(NamedKey::ArrowLeft)
                        if matches!(
                            self.controls.focus(),
                            Some(
                                slither_client::skin_studio::PREVIOUS
                                    | slither_client::skin_studio::NEXT
                            )
                        ) =>
                    {
                        self.studio
                            .activate(slither_client::skin_studio::PREVIOUS, &view.nickname)
                    }
                    Key::Named(NamedKey::ArrowRight)
                        if matches!(
                            self.controls.focus(),
                            Some(
                                slither_client::skin_studio::PREVIOUS
                                    | slither_client::skin_studio::NEXT
                            )
                        ) =>
                    {
                        self.studio
                            .activate(slither_client::skin_studio::NEXT, &view.nickname)
                    }
                    Key::Named(NamedKey::Enter | NamedKey::Space) => self.activate(admission),
                    Key::Named(
                        named @ (NamedKey::ArrowLeft
                        | NamedKey::ArrowRight
                        | NamedKey::ArrowUp
                        | NamedKey::ArrowDown
                        | NamedKey::Home
                        | NamedKey::End),
                    ) => {
                        if let Some(focus) = self.controls.focus() {
                            let direction = match named {
                                NamedKey::ArrowLeft => Navigation::Left,
                                NamedKey::ArrowRight => Navigation::Right,
                                NamedKey::ArrowUp => Navigation::Up,
                                NamedKey::ArrowDown => Navigation::Down,
                                NamedKey::Home => Navigation::Home,
                                _ => Navigation::End,
                            };
                            let next =
                                self.studio
                                    .state
                                    .navigate(focus, direction, self.studio.now);
                            self.studio.state.reveal_control(next, self.area);
                            effects.extend(self.sync_controls(admission));
                            self.controls.set_focus(next);
                        }
                    }
                    _ => {}
                }
                if let Some(focus) = self.controls.focus() {
                    self.studio.state.reveal_control(focus, self.area);
                }
                effects.extend(self.sync_controls(admission));
            }
            return NativeInputResult {
                consumed: true,
                effects,
            };
        }
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
            if (!view.disabled || slither_client::server_picker::available(&view))
                && !self.input.state.editor(TextField::Nickname).composing()
            {
                if event.logical == &Key::Named(NamedKey::Tab) {
                    self.controls.tab(&self.panel, self.input.shift_key());
                    return NativeInputResult {
                        consumed: true,
                        effects: self.controls.sync_focus(&self.panel, &mut self.input.state),
                    };
                }
                if matches!(
                    self.controls.focus(),
                    Some(PLAY | SETTINGS | SERVER | CHANGE_SKIN)
                ) && !event.repeat
                    && matches!(event.logical, Key::Named(NamedKey::Enter | NamedKey::Space))
                {
                    if self.controls.focus() == Some(SETTINGS) {
                        return NativeInputResult {
                            consumed: true,
                            effects: self.open_settings(admission),
                        };
                    }
                    self.activate(admission);
                    return NativeInputResult {
                        consumed: true,
                        effects: self.sync_controls(admission),
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
        if self.state.settings.visible() {
            let mut effects = self.sync_controls(admission);
            let resolved = self.panel.resolve(self.area).unwrap_or_default();
            if let Some(intent) = self.controls.pointer(&self.panel, &resolved, point, down) {
                effects.extend(self.settings_activation(intent, admission));
            }
            effects.extend(self.controls.sync_focus(&self.panel, &mut self.input.state));
            return effects;
        }
        if self.picker.policy.visible() {
            return self
                .picker
                .pointer(&mut self.text, point, down)
                .unwrap_or_else(|e| vec![HostEditEffect::ClipboardFailed(e)]);
        }
        if self.champion.policy.visible() {
            return self
                .champion
                .pointer_scaled(&mut self.text, point, down, self.pixel_scale)
                .unwrap_or_else(|e| vec![HostEditEffect::ClipboardFailed(e)]);
        }
        if self.studio.state.visible() {
            let mut effects = self.sync_controls(admission);
            let resolved = self.panel.resolve(self.area).unwrap_or_default();
            if let Some(intent) = self.controls.pointer(&self.panel, &resolved, point, down) {
                self.studio
                    .activate(intent.control, &self.state.view(true).nickname);
                self.studio.state.reveal_control(intent.control, self.area);
                effects.extend(self.sync_controls(admission));
            }
            effects.extend(self.controls.sync_focus(&self.panel, &mut self.input.state));
            return effects;
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
                        SETTINGS => effects.extend(self.open_settings(admission)),
                        SERVER => {
                            effects.extend(self.input.state.apply(EditIntent::Focus(None)));
                            effects.extend(self.picker.open());
                        }
                        CHANGE_SKIN => effects.extend(self.open_studio()),
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
        self.studio.viewport = area;
        let resized = area != self.area || dpr != self.pixel_scale;
        if resized {
            self.controls.blur();
        }
        self.pixel_scale = dpr;
        let mut fresh = crate::menu_specimen::layout(&mut self.text, &view, extent, dpr)?;
        if self.area == area {
            // Keep resize-time top across fade updates.
            fresh.top = self.layout.top;
        }
        self.layout = fresh;
        self.area = area;
        if self.state.settings.visible() {
            self.sync_controls(admission);
            // Replace login/wordmark resources; retained pictures keep their own handles.
            let mut picture = self.menu.prepare_panel(
                &mut self.text,
                &self.panel,
                &self.controls,
                &self.input.state,
                &mut self.scroll,
                Default::default(),
                extent,
                dpr,
            )?;
            let page = MenuFrame::world_crossfade(extent, dpr, 0.)?;
            picture.backdrop = page.backdrop;
            picture.resources.extend(page.resources);
            return Ok(picture);
        }
        if self.studio.state.visible() {
            if resized && let Some(focus) = self.controls.focus() {
                self.studio.state.reveal_control(focus, area);
            }
            self.sync_controls(admission);
            view.login_alpha *= 1. - self.studio.state.alpha();
            view.visible = view.login_alpha > 0.;
            let mut picture = self.menu.prepare_with_panel(
                &mut self.text,
                &view,
                &self.layout,
                extent,
                dpr,
                cosmetics,
                Some(&self.input.state),
                Some(crate::menu::PanelPaint {
                    thumbnails: Some(&self.studio.thumbnails),
                    panel: &self.panel,
                    input: &self.controls,
                    edit: &self.input.state,
                    scroll: &mut self.scroll,
                    theme: Default::default(),
                }),
            )?;
            if picture.backdrop.is_none() {
                let page = MenuFrame::world_crossfade(extent, dpr, 0.)?;
                picture.backdrop = page.backdrop;
                picture.resources.extend(page.resources);
            }
            return Ok(picture);
        }
        view.nickname = self.displayed_nickname();
        self.sync_controls(admission);
        if self.champion.policy.finished() {
            for control in &mut self.panel.controls {
                if matches!(
                    control.id,
                    NICKNAME | PLAY | CHANGE_SKIN | SERVER | SETTINGS
                ) {
                    control.alpha *= self.champion.policy.ordinary_alpha();
                }
            }
        }
        let panel = if self.champion.policy.visible() {
            self.champion.viewport = extent.map(|n| f64::from(n) / dpr);
            self.champion.panel = self
                .champion
                .policy
                .panel(&self.champion_session, self.champion.viewport);
            // Keep specialized logo/result producers, suppress the old pending label.
            view.champion_pending = false;
            Some(crate::menu::PanelPaint {
                thumbnails: None,
                panel: &self.champion.panel,
                input: &self.champion.controls,
                edit: &self.champion.input.state,
                scroll: &mut self.champion.scroll,
                theme: slither_client::menu_theme::MenuTheme::default(),
            })
        } else if self.picker.policy.visible() {
            self.picker.viewport = area;
            self.picker.panel = self.picker.policy.panel(area);
            Some(crate::menu::PanelPaint {
                thumbnails: None,
                panel: &self.picker.panel,
                input: &self.picker.controls,
                edit: &self.picker.input.state,
                scroll: &mut self.picker.scroll,
                theme: slither_client::menu_theme::MenuTheme::default(),
            })
        } else {
            Some(crate::menu::PanelPaint {
                thumbnails: None,
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
        if self.picker.policy.visible() {
            return self.picker.cursor_area(&mut self.text, self.pixel_scale);
        }
        if self.studio.state.visible() || self.state.settings.visible() {
            return Ok(Vec::new());
        }
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
