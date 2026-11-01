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
    menu_layout::MenuLayout,
    menu_preferences::MenuQuality,
    start_page::{MenuAction, MenuLocale, StartMenuIntent, StartMenuState},
};

pub struct NativeLogin {
    pub egui: crate::egui_native::NativeEguiHost,
    pub settings_ui: crate::settings_egui::SettingsUi,
    pub login_ui: crate::login_egui::LoginUi,
    admission: bool,
    egui_effects: Vec<HostEditEffect>,
    cancel_requested: bool,
    render_error: Option<String>,
    pub ingame_ui: crate::ingame_egui::InGameUi,
    ingame_visible: bool,
    ingame_viewport: ([u32; 2], f64),
    replay_after_close: bool,
    closing: bool,
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
        self.ingame_visible
            || self.picker.policy.visible()
            || self.champion.policy.visible()
            || matches!(
                self.state.view(true).phase,
                slither_client::start_page::MenuPhase::Login
                    | slither_client::start_page::MenuPhase::Connecting
            )
    }
    /// The host calls this only after menu event routing has declined the wheel.
    /// A visible list/menu keeps exclusive ownership even over a retained game.
    pub fn wheel_zoom(&self, game: &mut crate::game::LocalGame, notches: f64) -> bool {
        self.zoom_input_available() && game.wheel_zoom(notches)
    }
    /// Menus retain ownership; zoom observes the host cursor when play takes it.
    pub fn zoom_input_available(&self) -> bool {
        !self.blocks_gameplay() && !self.state.settings.visible() && !self.studio.state.visible()
    }
    pub fn intercepts_pointer(&self, point: [f64; 2]) -> bool {
        if self.egui.active() {
            // Outgoing controls are disabled by shared policy. Their fading
            // rectangles must not swallow pointer input owned by gameplay.
            return self.egui_owns_input();
        }
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
            egui: Default::default(),
            settings_ui: Default::default(),
            login_ui: crate::login_egui::LoginUi::new(nickname.clone()),
            admission: true,
            egui_effects: Vec::new(),
            cancel_requested: false,
            render_error: None,
            ingame_ui: Default::default(),
            ingame_visible: false,
            ingame_viewport: ([960, 540], 1.),
            replay_after_close: false,
            closing: false,
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
    pub fn set_admission(&mut self, admission: bool) {
        self.admission = admission;
    }
    pub fn step(
        &mut self,
        now: f64,
        admission: bool,
        playing: bool,
    ) -> Result<(Vec<StartMenuIntent>, Vec<HostEditEffect>)> {
        self.admission = admission;
        self.studio.step(now);
        let settings_visible = self.state.settings.visible();
        self.state.settings.step(now).map_err(str::to_owned)?;
        self.egui.step(now);
        self.refresh_egui();
        if let Some(record) = self.state.settings.take_write() {
            self.state
                .settings
                .storage_result(self.settings_storage.save(record).is_ok());
        }
        let intents = self
            .state
            .step_with_admission(now, false, admission)
            .map_err(str::to_owned)?;
        self.refresh_egui();
        self.input.state.step_caret(now);
        if playing {
            self.state.begin_outgoing();
        }
        if !self.state.settings.visible()
            && !self.studio.state.visible()
            && intents.contains(&StartMenuIntent::FocusNickname)
        {
            self.focus(NICKNAME);
        }
        let mut effects = self
            .input
            .state
            .sync_menu(&self.state.view(admission), &intents);
        effects.extend(self.sync_controls(admission));
        if settings_visible && !self.state.settings.visible() {
            effects.extend(self.input.state.apply(EditIntent::Modal(false)));
            effects.extend(self.focus(SETTINGS));
        }
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
        effects.extend(self.take_egui_effects());
        self.refresh_egui();
        Ok((intents, effects))
    }
    pub fn observe_session(&mut self, phase: slither_client::session_policy::Phase) -> bool {
        self.state.attempt_ended(phase)
    }
    pub fn observe_game(&mut self, game: &crate::game::LocalGame) -> Result<()> {
        self.closing = game.stopping();
        let opened = game.ingame_menu_open() && !self.ingame_visible;
        self.ingame_visible = game.ingame_menu_open();
        self.refresh_egui();
        if opened {
            self.egui.begin_screen();
            self.controls.blur();
            self.panel.visible = false;
            self.egui.focus(crate::ingame_egui::leave_id());
        }
        // A closing socket still owns admission. Keep the current screen until
        // its Close effect completes; Enter is retained by ingame_key meanwhile.
        if game.stopping() {
            return Ok(());
        }
        self.champion_session = game.champion_session();
        self.champion.observe(game);
        if self.champion.policy.finished() {
            self.state.champion_finished();
        }
        self.state
            .observe_session(game.login_session(), game.login_final())
            .map_err(str::to_owned)?;
        if self.replay_after_close && game.can_play() && !self.state.view(true).play_disabled {
            self.replay_after_close = false;
            self.state.action(MenuAction::Play, true);
        }
        self.refresh_egui();
        Ok(())
    }
    pub fn cancel_game(&mut self, game: &mut crate::game::LocalGame) {
        self.replay_after_close = false;
        self.state.cancel_pending_play();
        game.request_stop();
        self.closing = game.stopping();
    }
    /// Esc admission is shared by the desktop callback and CPU-only session tests.
    pub fn ingame_key(
        &mut self,
        game: &mut crate::game::LocalGame,
        event: crate::menu_input_native::KeyInput<'_>,
        now: f64,
    ) -> Result<bool> {
        use winit::{
            event::ElementState,
            keyboard::{KeyCode, PhysicalKey},
        };
        if game.stopping()
            && event.physical == PhysicalKey::Code(KeyCode::Enter)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            self.replay_after_close = true;
            return Ok(true);
        }
        if event.physical == PhysicalKey::Code(KeyCode::Escape)
            && event.state == ElementState::Pressed
            && !event.repeat
        {
            return self.open_ingame_menu(game, now);
        }
        Ok(false)
    }
    pub fn open_ingame_menu(
        &mut self,
        game: &mut crate::game::LocalGame,
        now: f64,
    ) -> Result<bool> {
        if !self.zoom_input_available() || !game.open_ingame_menu(now)? {
            return Ok(false);
        }
        self.observe_game(game)?;
        // Establish the modal layer and its centred geometry before the next input.
        self.ingame_frame(game)?;
        self.ingame_frame(game)?;
        Ok(true)
    }
    pub fn prepare_ingame(
        &mut self,
        game: &mut crate::game::LocalGame,
        extent: [u32; 2],
        dpr: f64,
    ) -> Result<()> {
        if !game.ingame_menu_open() {
            return Ok(());
        }
        if self.ingame_viewport != (extent, dpr) {
            self.egui.cancel_pointer();
        }
        self.ingame_viewport = (extent, dpr);
        self.ingame_frame(game)
    }
    fn ingame_frame(&mut self, game: &mut crate::game::LocalGame) -> Result<()> {
        if !game.ingame_menu_open() {
            return Ok(());
        }
        let mut action = None;
        let (extent, dpr) = self.ingame_viewport;
        self.egui
            .frame(extent, dpr, &[crate::ingame_egui::leave_id()], |ctx, _| {
                action = self.ingame_ui.show(ctx)
            });
        self.egui.check_error()?;
        match action {
            Some(crate::ingame_egui::Action::Leave) => game.request_stop(),
            Some(crate::ingame_egui::Action::Dismiss) => {
                game.close_ingame_menu();
                let cursor = self.egui.window.cursor();
                let area = extent.map(|n| f64::from(n) / dpr);
                game.pointer(cursor[0], cursor[1], area[0], area[1], false)?;
            }
            None => {}
        }
        self.observe_game(game)
    }
    pub fn ingame_input(
        &mut self,
        game: &mut crate::game::LocalGame,
        mut input: egui::RawInput,
    ) -> Result<()> {
        // Keep discrete actions ordered, including press/release in one host batch.
        let events = std::mem::take(&mut input.events);
        for event in events {
            if !game.ingame_menu_open() {
                break;
            }
            let discrete = !matches!(
                event,
                egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_)
            );
            input.events.push(event);
            self.egui.input(input.take());
            if discrete {
                self.ingame_frame(game)?;
            }
        }
        Ok(())
    }
    pub fn sync_return_focus(&mut self, admission: bool) -> Vec<HostEditEffect> {
        if self.studio.state.visible() || self.state.settings.visible() {
            return Vec::new();
        }
        if self.studio.state.take_return_focus()
            && self.state.view(true).phase == slither_client::start_page::MenuPhase::Login
        {
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
            self.focus(NICKNAME);
        }
        let mut effects = self
            .input
            .state
            .sync_menu(&self.state.view(admission), &intents);
        effects.extend(self.sync_controls(admission));
        effects
    }
    fn sync_controls(&mut self, admission: bool) -> Vec<HostEditEffect> {
        if self.egui.active() || self.state.settings.visible() {
            if self.state.settings.visible() {
                self.controls.blur();
            }
            self.panel.visible = false;
            return self.input.state.apply(EditIntent::Focus(None));
        }
        self.panel = if self.studio.state.visible() {
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
            self.refresh_egui();
            self.state.play_pointer(false, false, true);
            self.controls.blur();
            self.sync_controls(true)
        } else {
            Vec::new()
        }
    }
    pub fn focused(&self) -> Option<ControlId> {
        if self.login_owns_input() {
            let focused = self.egui.context()?.memory(|m| m.focused());
            [NICKNAME, PLAY, CHANGE_SKIN, SERVER, SETTINGS]
                .into_iter()
                .find(|id| Some(crate::login_egui::widget_id(*id)) == focused)
        } else {
            self.controls.focus()
        }
    }
    pub fn focus(&mut self, focus: ControlId) -> Vec<HostEditEffect> {
        if self.state.settings.visible() {
            self.settings_ui.request_focus(focus);
            self.egui.focus(crate::settings_egui::widget_id(focus));
            self.settings_frame();
            return Vec::new();
        }
        self.refresh_egui();
        if self.login_owns_input() {
            self.login_ui.request_focus(focus);
            self.egui.focus(crate::login_egui::widget_id(focus));
            return Vec::new();
        }
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
            || self.login_ui.composing
        {
            return Vec::new();
        }
        let view = self.state.view(admission);
        if self.state.settings.open(&view) {
            self.refresh_egui();
            self.egui.begin_screen();
            self.settings_ui
                .request_focus(slither_client::settings::QUALITY);
            self.egui.focus(crate::settings_egui::widget_id(
                slither_client::settings::QUALITY,
            ));
            self.settings_frame();
            self.settings_frame();
            let mut effects = self.input.state.apply(EditIntent::Modal(true));
            effects.extend(self.sync_controls(admission));
            effects
        } else {
            Vec::new()
        }
    }
    pub fn close_settings(&mut self, admission: bool) -> Vec<HostEditEffect> {
        self.state.settings.close();
        self.refresh_egui();
        let mut effects = Vec::new();
        if !self.state.settings.visible() {
            effects.extend(self.input.state.apply(EditIntent::Modal(false)));
            effects.extend(self.sync_controls(admission));
            effects.extend(self.focus(SETTINGS));
        }
        effects
    }
    pub fn login_owns_input(&self) -> bool {
        let view = self.state.view(self.admission);
        (view.visible || view.spinner_visible)
            && !self.state.settings.visible()
            && !self.studio.state.visible()
            && !self.picker.policy.visible()
            && !self.champion.policy.visible()
            && !self.ingame_visible
    }
    pub fn egui_owns_input(&self) -> bool {
        if !self.egui.active() {
            return false;
        }
        self.state.settings.visible()
            || self.ingame_visible
            || self.closing
            || matches!(
                self.state.view(self.admission).phase,
                slither_client::start_page::MenuPhase::Login
                    | slither_client::start_page::MenuPhase::Connecting
                    | slither_client::start_page::MenuPhase::Returning
            )
    }
    fn refresh_egui(&mut self) {
        let active =
            self.login_owns_input() || self.state.settings.visible() || self.ingame_visible;
        let opened = active && !self.egui.active();
        self.egui.set_active(active);
        if opened && self.login_owns_input() {
            self.login_ui.request_focus(NICKNAME);
            self.egui.focus(crate::login_egui::widget_id(NICKNAME));
        }
    }
    pub fn take_cancel_request(&mut self) -> bool {
        std::mem::take(&mut self.cancel_requested)
    }
    pub fn take_egui_effects(&mut self) -> Vec<HostEditEffect> {
        std::mem::take(&mut self.egui_effects)
    }
    fn settings_frame(&mut self) {
        let open = self.state.settings.is_open();
        let settings = self.state.settings.visible();
        let mut view = self.state.view(self.admission);
        if self.champion.policy.finished() {
            let alpha = self.champion.policy.ordinary_alpha();
            view.nickname_alpha *= alpha;
            view.play_alpha *= alpha;
            view.skin_alpha *= alpha;
        }
        if !self.login_ui.composing {
            self.login_ui.nickname.clone_from(&view.nickname);
        }
        view.login_alpha *= 1. - self.state.settings.alpha();
        let order = if settings {
            crate::settings_egui::focus_order().to_vec()
        } else {
            crate::login_egui::focus_order(&view)
        };
        let mut action = None;
        self.egui.frame(
            self.area.map(|n| (n * self.pixel_scale).round() as u32),
            self.pixel_scale,
            &order,
            |ctx, reveal| {
                if self.closing
                    && !self.login_ui.composing
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    self.replay_after_close = true;
                }
                if !settings
                    && !self.login_ui.composing
                    && ctx.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    self.cancel_requested = true;
                }
                let status = (self.studio.status == "Saved for this run")
                    .then_some(self.studio.status.as_str());
                match self
                    .login_ui
                    .show(ctx, &view, &self.layout, !settings, status)
                {
                    Ok(value) => action = value,
                    Err(error) => self.render_error = Some(error),
                }
                if settings {
                    self.settings_ui.show(ctx, &mut self.state.settings, reveal);
                }
            },
        );
        if !self.login_ui.composing && !settings {
            self.state
                .edit_nickname(&self.login_ui.nickname.encode_utf16().collect::<Vec<_>>());
        }
        if let Some((id, enter)) = action {
            self.activate_login(id, enter);
        }
        if open && !self.state.settings.is_open() {
            self.close_settings(self.admission);
        }
    }
    fn activate_login(&mut self, id: ControlId, enter: bool) {
        if self.login_ui.composing {
            return;
        }
        let mut opener = PanelInputState::default();
        if self.login_ui.keyboard_focus {
            opener.keyboard_navigation();
        }
        self.controls.inherit_modality(&opener);
        self.picker.controls.inherit_modality(&opener);
        let effects = match id {
            PLAY => {
                self.state.action(
                    if enter {
                        MenuAction::Enter { composing: false }
                    } else {
                        MenuAction::Play
                    },
                    self.admission,
                );
                Vec::new()
            }
            SETTINGS => self.open_settings(self.admission),
            SERVER
                if slither_client::server_picker::available(&self.state.view(self.admission)) =>
            {
                self.picker.open()
            }
            CHANGE_SKIN => self.open_studio(),
            _ => Vec::new(),
        };
        if matches!(id, SERVER | CHANGE_SKIN) {
            self.egui_effects.push(HostEditEffect::ImeAllowed(false));
        }
        self.egui_effects.extend(effects);
        self.refresh_egui();
    }
    /// Authored input and the native egui-winit adapter feed the same widget tree.
    pub fn settings_input(&mut self, mut input: egui::RawInput) {
        if !self.egui.active() {
            return;
        }
        if self.egui.take_ime_cancellation() && self.login_owns_input() {
            if let Some(ctx) = self.egui.context() {
                self.login_ui.cancel_composition(ctx);
            }
            self.settings_frame();
        }
        let events = std::mem::take(&mut input.events);
        for mut event in events {
            if !self.egui.active() {
                break;
            }
            if self.login_owns_input() {
                if matches!(event, egui::Event::Key { pressed: true, .. }) {
                    self.login_ui.keyboard_focus = true;
                }
                if self.login_ui.composing
                    && matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Tab,
                            pressed: true,
                            ..
                        }
                    )
                {
                    continue;
                }
                if self.focused() == Some(NICKNAME)
                    && !self.state.view(self.admission).disabled
                    && matches!(
                        event,
                        egui::Event::Key {
                            key: egui::Key::Escape,
                            pressed: true,
                            ..
                        }
                    )
                {
                    self.egui.cancel_pointer();
                    if let Some(ctx) = self.egui.context() {
                        self.login_ui.cancel_composition(ctx);
                    }
                    self.settings_frame();
                    continue;
                }
            }
            if self.login_owns_input()
                && (matches!(&event, egui::Event::WindowFocused(false))
                    || matches!(&event, egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) if text.is_empty()))
            {
                if matches!(event, egui::Event::WindowFocused(false)) {
                    self.egui.cancel_pointer();
                }
                if let Some(ctx) = self.egui.context() {
                    self.login_ui.cancel_composition(ctx);
                }
            }
            // Normalize inserted UTF-16 units first; the complete value is then
            // truncated by the shared rule. Surrogate pairs consume two spaces.
            if self.login_owns_input() {
                match &mut event {
                    egui::Event::Text(text)
                    | egui::Event::Paste(text)
                    | egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                        *text = slither_client::start_page::normalize_nickname(text);
                    }
                    _ => {}
                }
            }
            let preedit = matches!(event, egui::Event::Ime(egui::ImeEvent::Preedit { .. }));
            let button = matches!(event, egui::Event::PointerButton { .. });
            if button && self.egui.has_pending_events() {
                self.settings_frame();
            }
            if self.login_owns_input()
                && let egui::Event::Ime(egui::ImeEvent::Preedit {
                    text,
                    active_range_chars,
                }) = &mut event
            {
                *text = slither_client::menu_input::bounded_preedit(text);
                if let Some(range) = active_range_chars {
                    let len = text.chars().count();
                    range.start = range.start.min(len);
                    range.end = range.end.min(len);
                }
                if !text.is_empty()
                    && let Some(ctx) = self.egui.context()
                {
                    self.login_ui.begin_composition(ctx);
                }
            }
            // Adjacent preedit replacements are one provisional value. The host
            // keeps only the latest until redraw or an ordered discrete boundary.
            if preedit {
                self.egui.replace_preedit(event);
                continue;
            }
            let discrete = !matches!(
                event,
                egui::Event::PointerMoved(_)
                    | egui::Event::MouseMoved(_)
                    | egui::Event::PointerGone
                    | egui::Event::ModifiersChanged(_)
                    | egui::Event::MouseWheel { .. }
            );
            input.events.push(event);
            self.egui.input(input.take());
            if discrete {
                self.settings_frame();
            }
        }
    }
    fn settings_key(&mut self, key: egui::Key) {
        let modifiers = egui::Modifiers {
            shift: self.input.shift_key(),
            ..Default::default()
        };
        let events = [true, false]
            .into_iter()
            .map(|pressed| egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            })
            .collect();
        self.settings_input(egui::RawInput {
            events,
            ..Default::default()
        });
    }
    pub fn activate(&mut self, admission: bool) {
        if self.picker.policy.visible() {
            return;
        }
        if self.state.settings.visible() {
            self.settings_key(egui::Key::Enter);
            return;
        }
        if self.studio.state.visible() {
            if let Some(id) = self.controls.focus() {
                self.studio.activate(id, &self.state.view(true).nickname);
            }
            return;
        }
        self.admission = admission;
        self.login_ui.keyboard_focus = true;
        self.refresh_egui();
        let focus = self
            .egui
            .context()
            .and_then(|ctx| ctx.memory(|m| m.focused()))
            .and_then(|id| {
                [NICKNAME, PLAY, CHANGE_SKIN, SERVER, SETTINGS]
                    .into_iter()
                    .find(|c| crate::login_egui::widget_id(*c) == id)
            })
            .unwrap_or(NICKNAME);
        self.activate_login(if focus == NICKNAME { PLAY } else { focus }, true);
    }
    pub fn event(
        &mut self,
        event: &winit::event::WindowEvent,
        admission: bool,
    ) -> crate::menu_input_native::NativeInputResult {
        use winit::event::WindowEvent;
        if self.egui.active()
            && matches!(
                event,
                WindowEvent::Focused(false)
                    | WindowEvent::Resized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
            )
        {
            self.egui.cancel_pointer();
        }
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
        if self.egui_owns_input() {
            // Keep legacy focus bookkeeping for its next screen, without letting
            // its IME effects overwrite egui's current owner on window refocus.
            if matches!(event, WindowEvent::Focused(_)) {
                self.input.window_event(event);
            }
            return Default::default();
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
        self.admission = admission;
        use crate::menu_input_native::NativeInputResult;
        use winit::{
            event::ElementState,
            keyboard::{Key, NamedKey},
        };
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
        if self.egui_owns_input() {
            if event.state == ElementState::Pressed && !event.repeat {
                let key = match event.logical {
                    Key::Named(NamedKey::Escape) => Some(egui::Key::Escape),
                    Key::Named(NamedKey::Tab) => Some(egui::Key::Tab),
                    Key::Named(NamedKey::Enter) => Some(egui::Key::Enter),
                    Key::Named(NamedKey::Space) => Some(egui::Key::Space),
                    _ => None,
                };
                if let Some(key) = key {
                    self.settings_key(key);
                }
            }
            return NativeInputResult {
                consumed: true,
                effects: self.take_egui_effects(),
            };
        }
        if self.studio.state.visible() {
            let view = self.state.view(admission);
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
        self.input.key(event)
    }
    pub fn pointer(
        &mut self,
        point: [f64; 2],
        down: Option<bool>,
        admission: bool,
    ) -> Vec<HostEditEffect> {
        self.admission = admission;
        if self.egui_owns_input() {
            let pos = egui::pos2(point[0] as f32, point[1] as f32);
            let mut events = vec![egui::Event::PointerMoved(pos)];
            if let Some(pressed) = down {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                });
            }
            self.settings_input(egui::RawInput {
                events,
                ..Default::default()
            });
            return self.take_egui_effects();
        }
        if self.picker.policy.visible() {
            return self
                .picker
                .pointer_scaled(&mut self.text, point, down, self.pixel_scale)
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
        Vec::new()
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
            self.egui.cancel_pointer();
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
        self.refresh_egui();
        self.admission = admission;
        if self.login_owns_input() || self.state.settings.visible() {
            if let Some(ctx) = self.egui.context() {
                self.login_ui.sync_art(ctx, cosmetics);
            }
            let first = self
                .egui
                .context()
                .is_some_and(|ctx| ctx.cumulative_pass_nr() == 0);
            self.settings_frame();
            // egui's initial Area sizing pass must settle before input can
            // traverse the field and buttons, just as on Settings entry.
            if first {
                self.settings_frame();
            }
            self.egui.check_error()?;
            if let Some(error) = self.render_error.take() {
                return Err(error);
            }
            return MenuFrame::world_crossfade(extent, dpr, view.world_alpha);
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
        self.login_ui.nickname.clone()
    }
    pub fn cursor_area(&mut self) -> Result<Vec<HostEditEffect>> {
        if self.picker.policy.visible() {
            return self.picker.cursor_area(&mut self.text, self.pixel_scale);
        }
        if self.champion.policy.visible() {
            return self.champion.cursor_area(&mut self.text, self.pixel_scale);
        }
        Ok(Vec::new())
    }
}
