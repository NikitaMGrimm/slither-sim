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
    connection_error: Option<String>,
    pub ingame_ui: crate::ingame_egui::InGameUi,
    ingame_visible: bool,
    ingame_viewport: ([u32; 2], f64),
    replay_after_close: bool,
    closing: bool,
    settings_storage: crate::menu_storage::SettingsStorage,
    pub studio: crate::skin_studio_native::SkinStudioNative,
    pub studio_ui: crate::skin_studio_egui::StudioUi,
    /// The studio panel the next egui pass draws, synced from the core.
    pub studio_panel: PanelView,
    /// Any studio key press keeps the focused control inside its scrolled
    /// window, as the old studio input did.
    studio_reveal: bool,
    studio_shown: bool,
    /// Studio navigation keys currently held down.
    studio_keys: Vec<egui::Key>,
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
        if self.egui.active() && !self.egui.paint_only() {
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
        let nickname = record.nickname.clone();
        let mut state = StartMenuState::new(MenuLocale::English, true, MenuQuality::High);
        state.edit_nickname(&nickname.encode_utf16().collect::<Vec<_>>());
        let mut input = NativeMenuInput::default();
        input.state.sync_menu(&state.view(true), &[]);
        let mut text = TextCache::menu();
        let layout = crate::menu_specimen::layout(&mut text, &state.view(true), [960, 540], 1.)?;
        let panel = login_controls::panel(&state.view(true), &layout, [960., 540.]);
        let studio = crate::skin_studio_native::SkinStudioNative::new(record, storage)?;
        let studio_panel = studio.state.panel([960., 540.], &studio.status);
        Ok(Self {
            egui: Default::default(),
            settings_ui: Default::default(),
            login_ui: crate::login_egui::LoginUi::new(nickname.clone()),
            admission: true,
            egui_effects: Vec::new(),
            cancel_requested: false,
            render_error: None,
            connection_error: None,
            ingame_ui: Default::default(),
            ingame_visible: false,
            ingame_viewport: ([960, 540], 1.),
            replay_after_close: false,
            closing: false,
            settings_storage: crate::menu_storage::SettingsStorage::at(None),
            studio,
            studio_ui: Default::default(),
            studio_panel,
            studio_reveal: false,
            studio_shown: false,
            studio_keys: Vec::new(),
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
        let studio_closed = std::mem::replace(&mut self.studio_shown, self.studio.state.visible())
            && !self.studio.state.visible();
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
        self.picker.step(now)?;
        if studio_closed {
            // Drop the closed studio's widget state and thumbnail textures.
            self.studio_ui = Default::default();
            effects.extend(self.modal_effects(false));
        }
        if self.picker.take_closed() {
            effects.extend(self.modal_effects(false));
            effects.extend(self.focus(NICKNAME));
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
    pub fn begin_play_attempt(&mut self, game: &mut crate::game::LocalGame) -> Result<()> {
        game.begin_play_attempt()?;
        self.connection_error = None;
        Ok(())
    }
    pub fn observe_game(&mut self, game: &crate::game::LocalGame) -> Result<()> {
        self.connection_error.clone_from(&game.failure);
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
        if (self.egui.active() && !self.egui.paint_only()) || self.state.settings.visible() {
            if self.state.settings.visible() {
                self.controls.blur();
            }
            self.panel.visible = false;
            return self.input.state.apply(EditIntent::Focus(None));
        }
        self.panel = {
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
        if !self.studio.open(admitted) {
            return Vec::new();
        }
        // The studio takes the opener's input modality and owns the egui
        // host before the next input arrives.
        self.studio_ui.keyboard_focus = self.login_ui.keyboard_focus;
        self.studio_shown = true;
        self.sync_studio();
        self.refresh_egui();
        self.egui.begin_screen();
        self.state.play_pointer(false, false, true);
        self.controls.blur();
        let mut effects = self.modal_effects(true);
        effects.extend(self.sync_controls(true));
        // egui's initial Area sizing pass must settle before input reaches controls.
        self.settings_frame();
        self.settings_frame();
        effects
    }
    fn sync_studio(&mut self) {
        self.studio.viewport = self.area;
        self.studio_panel = self.studio.state.panel(self.area, &self.studio.status);
    }
    fn studio_owns_input(&self) -> bool {
        self.studio.state.visible() && !self.state.settings.visible()
    }
    pub fn focused(&self) -> Option<ControlId> {
        if self.login_owns_input() {
            let focused = self.egui.context()?.memory(|m| m.focused());
            [NICKNAME, PLAY, CHANGE_SKIN, SERVER, SETTINGS]
                .into_iter()
                .find(|id| Some(crate::login_egui::widget_id(*id)) == focused)
        } else if self.studio_owns_input() {
            let focused = self.egui.context()?.memory(|m| m.focused());
            self.studio_panel
                .controls
                .iter()
                .map(|c| c.id)
                .find(|id| Some(crate::skin_studio_egui::widget_id(*id)) == focused)
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
        if self.studio_owns_input() {
            // Control ids belong to the active panel; a login id cannot pierce it.
            if crate::egui_panel::focus_target(&self.studio_panel, focus) {
                self.studio_ui.request_focus(focus);
                self.egui.focus(crate::skin_studio_egui::widget_id(focus));
                self.settings_frame();
            }
            return Vec::new();
        }
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
    /// Shared Server icon entry: the picker takes the opener's input modality
    /// and owns the egui host before the next input arrives.
    pub fn open_picker(&mut self) -> Vec<HostEditEffect> {
        if !slither_client::server_picker::available(&self.state.view(self.admission))
            || self.picker.policy.visible()
        {
            return Vec::new();
        }
        self.picker.ui.keyboard_focus = self.login_ui.keyboard_focus;
        self.picker.viewport = self.area;
        self.cancel_picker_composition();
        self.picker.open();
        self.refresh_egui();
        self.egui.begin_screen();
        let mut effects = self.modal_effects(true);
        effects.extend(self.sync_controls(self.admission));
        // egui's initial Area sizing pass must settle before input reaches fields.
        self.settings_frame();
        self.settings_frame();
        effects
    }
    /// A pointer close can end the native composition without an IME event the
    /// picker sees: `set_ime_allowed(false)` need not report Disabled. Restore
    /// the snapshot so a stale composition cannot outlive the picker.
    fn cancel_picker_composition(&mut self) {
        if let Some(ctx) = self.egui.context() {
            self.picker.ui.cancel_composition(ctx);
        }
    }
    /// Release held gameplay input on entry, like other modals. egui-winit owns
    /// IME admission for egui screens and the nickname they return to; a
    /// legacy IME effect here would desynchronize its cached state.
    fn modal_effects(&mut self, modal: bool) -> Vec<HostEditEffect> {
        let mut effects = self.input.state.apply(EditIntent::Modal(modal));
        effects.retain(|e| {
            !matches!(
                e,
                HostEditEffect::ImeAllowed(_)
                    | HostEditEffect::ImePurposeNormal
                    | HostEditEffect::ImeCursorArea { .. }
            )
        });
        effects
    }
    fn picker_owns_input(&self) -> bool {
        self.picker.policy.visible() && !self.state.settings.visible()
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
        if !self.egui.active() || self.egui.paint_only() {
            return false;
        }
        self.state.settings.visible()
            || self.picker.policy.visible()
            || self.studio.state.visible()
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
        let owns = self.login_owns_input()
            || self.state.settings.visible()
            || self.picker.policy.visible()
            || self.studio.state.visible()
            || self.ingame_visible;
        // The champion editor still owns input on the old toolkit; egui keeps
        // painting the login below it, so no screen transition swaps the
        // login's renderer.
        let underlay = !owns && self.champion.policy.visible();
        let opened = owns && !(self.egui.active() && !self.egui.paint_only());
        self.egui.set_active(owns || underlay);
        self.egui.set_paint_only(underlay);
        if underlay && let Some(ctx) = self.egui.context() {
            // The old toolkit blurred the opener; no focus ring stays below.
            ctx.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }
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
        // Below the champion editor's old toolkit: the login's fade and
        // wording match what that toolkit painted, without input.
        let underlay = self.egui.paint_only();
        let studio = !settings && self.studio.state.visible();
        if studio {
            view.login_alpha *= 1. - self.studio.state.alpha();
            self.sync_studio();
        }
        if self.champion.policy.visible() {
            view.champion_pending = false;
        }
        if !self.login_ui.composing {
            self.login_ui.nickname.clone_from(&view.nickname);
        }
        view.login_alpha *= 1. - self.state.settings.alpha();
        let picker = !settings && self.picker.policy.visible();
        if picker {
            self.picker.viewport = self.area;
            self.picker.sync();
        }
        let order = if underlay {
            Vec::new()
        } else if settings {
            crate::settings_egui::focus_order().to_vec()
        } else if picker {
            crate::server_picker_egui::focus_order(&self.picker.panel)
        } else if studio {
            crate::skin_studio_egui::focus_order(&self.studio_panel)
        } else {
            crate::login_egui::focus_order(&view)
        };
        let mut action = None;
        let mut intents = Vec::new();
        let mut studio_intents = Vec::new();
        self.egui.frame(
            self.area.map(|n| (n * self.pixel_scale).round() as u32),
            self.pixel_scale,
            &order,
            |ctx, reveal| {
                // While open, the picker and studio own Enter and Escape.
                if self.closing
                    && !picker
                    && !studio
                    && !self.login_ui.composing
                    && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                {
                    self.replay_after_close = true;
                }
                if !settings
                    && !picker
                    && !studio
                    && !self.login_ui.composing
                    && ctx.input(|i| i.key_pressed(egui::Key::Escape))
                {
                    self.cancel_requested = true;
                }
                let status = self.connection_error.as_deref().or_else(|| {
                    (self.studio.status == "Saved for this run")
                        .then_some(self.studio.status.as_str())
                });
                match self.login_ui.show(
                    ctx,
                    &view,
                    &self.layout,
                    !settings && !picker && !studio && !underlay,
                    status,
                ) {
                    Ok(value) => action = value,
                    Err(error) => self.render_error = Some(error),
                }
                if settings {
                    self.settings_ui.show(ctx, &mut self.state.settings, reveal);
                }
                if studio {
                    match self.studio_ui.show(
                        ctx,
                        &self.studio_panel,
                        self.area,
                        self.studio.state.interactive(),
                        &self.studio.thumbnails,
                    ) {
                        Ok(value) => studio_intents = value,
                        Err(error) => self.render_error = Some(error),
                    }
                }
                if picker {
                    match self.picker.ui.show(
                        ctx,
                        &self.picker.panel,
                        self.picker.viewport,
                        self.picker.policy.is_open(),
                    ) {
                        Ok(value) => intents = value,
                        Err(error) => self.render_error = Some(error),
                    }
                }
            },
        );
        if studio {
            self.apply_studio(studio_intents);
        }
        if picker {
            self.picker.apply(intents);
            if !self.picker.policy.is_open() {
                self.cancel_picker_composition();
            }
        }
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
            SERVER => self.open_picker(),
            CHANGE_SKIN => self.open_studio(),
            _ => Vec::new(),
        };
        self.egui_effects.extend(effects);
        self.refresh_egui();
    }
    /// Authored input and the native egui-winit adapter feed the same widget tree.
    pub fn settings_input(&mut self, mut input: egui::RawInput) {
        if !self.egui.active() {
            return;
        }
        if self.egui.take_ime_cancellation() {
            if self.login_owns_input() {
                if let Some(ctx) = self.egui.context() {
                    self.login_ui.cancel_composition(ctx);
                }
                self.settings_frame();
            } else if self.picker_owns_input() {
                if let Some(ctx) = self.egui.context() {
                    self.picker.ui.cancel_composition(ctx);
                }
                self.settings_frame();
            }
        }
        let events = std::mem::take(&mut input.events);
        for mut event in events {
            if !self.egui.active() {
                break;
            }
            if !self.studio_owns_input() || matches!(event, egui::Event::WindowFocused(false)) {
                // Releases go elsewhere while the studio does not own input.
                self.studio_keys.clear();
            }
            let routed = if self.picker_owns_input() {
                self.picker_event(&mut event)
            } else if self.studio_owns_input() {
                self.studio_event(&event)
            } else {
                PickerEvent::Forward
            };
            match routed {
                PickerEvent::Forward => {}
                PickerEvent::Handled => continue,
                PickerEvent::Pass => {
                    self.settings_frame();
                    continue;
                }
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
                bound_preedit(text, active_range_chars);
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
            // egui-winit translates a cancelled touch to Touch(Cancel) and
            // PointerGone without a release, and egui keeps the primary
            // button held. Drop the press without a click once egui has
            // seen the cancel.
            let cancelled = matches!(
                event,
                egui::Event::Touch {
                    phase: egui::TouchPhase::Cancel,
                    ..
                }
            );
            input.events.push(event);
            self.egui.input(input.take());
            if discrete {
                self.settings_frame();
            }
            if cancelled {
                self.egui.cancel_pointer();
            }
        }
    }
    /// Picker-owned events that egui must not see: list navigation, keys egui
    /// would turn into directional focus moves, and the bounded list wheel.
    fn picker_event(&mut self, event: &mut egui::Event) -> PickerEvent {
        use crate::server_picker_egui::{focusable, rule, widget_id};
        use egui::Key as K;
        let focus = self
            .picker
            .ui
            .focused()
            .filter(|id| focusable(&self.picker.panel, *id));
        let field = focus.and_then(rule);
        match event {
            egui::Event::Key { key, pressed, .. } => {
                if *pressed {
                    self.picker.ui.keyboard_focus = true;
                }
                if self.picker.ui.composing {
                    // Composition owns traversal and Escape cancels it.
                    return match key {
                        K::Tab if *pressed => PickerEvent::Handled,
                        K::Escape if *pressed => {
                            self.egui.cancel_pointer();
                            if let Some(ctx) = self.egui.context() {
                                self.picker.ui.cancel_composition(ctx);
                            }
                            PickerEvent::Pass
                        }
                        _ => PickerEvent::Forward,
                    };
                }
                match key {
                    K::ArrowDown | K::ArrowUp | K::PageDown | K::PageUp | K::Home | K::End => {
                        if field.is_some() && matches!(key, K::Home | K::End) {
                            return PickerEvent::Forward;
                        }
                        if *pressed && let Some(target) = self.picker.navigate(focus, *key) {
                            self.picker.ui.request_focus(target);
                            self.egui.focus(widget_id(target));
                            return PickerEvent::Pass;
                        }
                        // The address field has no vertical caret movement.
                        PickerEvent::Handled
                    }
                    K::ArrowLeft | K::ArrowRight if field.is_none() => PickerEvent::Handled,
                    _ => PickerEvent::Forward,
                }
            }
            egui::Event::WindowFocused(false) => {
                self.egui.cancel_pointer();
                if let Some(ctx) = self.egui.context() {
                    self.picker.ui.cancel_composition(ctx);
                }
                PickerEvent::Forward
            }
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text,
                active_range_chars,
            }) => {
                bound_preedit(text, active_range_chars);
                if let Some(ctx) = self.egui.context() {
                    if text.is_empty() {
                        self.picker.ui.cancel_composition(ctx);
                    } else if let Some(field) = focus.filter(|_| field.is_some()) {
                        self.picker.ui.begin_composition(ctx, field);
                    }
                }
                PickerEvent::Forward
            }
            // Normalize inserted UTF-16 units first; the complete value is
            // then truncated by the same field rule after the pass.
            egui::Event::Text(text)
            | egui::Event::Paste(text)
            | egui::Event::Ime(egui::ImeEvent::Commit(text)) => {
                if let Some(rule) = field {
                    *text = rule.normalize_units(&text.encode_utf16().collect::<Vec<_>>());
                }
                PickerEvent::Forward
            }
            egui::Event::MouseWheel { unit, delta, .. } => {
                let y = f64::from(delta.y);
                self.picker.wheel(match unit {
                    egui::MouseWheelUnit::Line => -y,
                    // egui-winit divides physical pixels by the window scale.
                    egui::MouseWheelUnit::Point => -y * self.pixel_scale / 46.,
                    egui::MouseWheelUnit::Page => -y * 6.,
                });
                PickerEvent::Handled
            }
            _ => PickerEvent::Forward,
        }
    }
    /// Studio-owned events egui must not see: roving grid/tab navigation,
    /// Previous/Next cycling, keys egui would turn into directional focus
    /// moves, and the bounded editor wheel.
    fn studio_event(&mut self, event: &egui::Event) -> PickerEvent {
        use crate::skin_studio_egui::widget_id;
        use egui::Key as K;
        use slither_client::skin_studio::Navigation as N;
        match event {
            egui::Event::Key {
                key,
                pressed,
                repeat,
                ..
            } => {
                if *pressed {
                    self.studio_ui.keyboard_focus = true;
                    self.studio_reveal = true;
                }
                let direction = match key {
                    K::ArrowLeft => N::Left,
                    K::ArrowRight => N::Right,
                    K::ArrowUp => N::Up,
                    K::ArrowDown => N::Down,
                    K::Home => N::Home,
                    K::End => N::End,
                    K::Tab | K::Enter | K::Space | K::Escape => return PickerEvent::Forward,
                    _ => return PickerEvent::Handled,
                };
                // egui-winit reports native auto-repeats as fresh presses
                // (repeat: false). The studio acts once per physical press,
                // as its old input did; the host's held-key gate covers only
                // Tab/Enter/Space/Escape, since text fields need repeats.
                let held = self.studio_keys.contains(key);
                if *pressed {
                    if held || *repeat {
                        return PickerEvent::Handled;
                    }
                    self.studio_keys.push(*key);
                } else {
                    self.studio_keys.retain(|k| k != key);
                }
                let focus = self
                    .focused()
                    .filter(|id| crate::egui_panel::focus_target(&self.studio_panel, *id));
                let Some(focus) = focus.filter(|_| *pressed && self.studio.state.interactive())
                else {
                    return PickerEvent::Handled;
                };
                let next = self.studio.navigate(focus, direction);
                self.studio.state.reveal_control(next, self.area);
                self.studio_ui.request_focus(next);
                self.egui.focus(widget_id(next));
                self.sync_studio();
                PickerEvent::Pass
            }
            egui::Event::MouseWheel { unit, delta, .. } => {
                let y = f64::from(delta.y);
                self.studio.wheel += match unit {
                    // egui-winit divides physical pixels by the window scale;
                    // the studio scrolls one 28px row per 28 physical pixels.
                    egui::MouseWheelUnit::Point => -y * self.pixel_scale / 28.,
                    egui::MouseWheelUnit::Line | egui::MouseWheelUnit::Page => -y,
                };
                let rows = self.studio.wheel.trunc() as isize;
                self.studio.wheel -= rows as f64;
                self.studio.state.scroll_editor(rows, self.area);
                // Scrolling moves the pressed control: its release never clicks.
                self.studio_ui.invalidate_press();
                self.sync_studio();
                PickerEvent::Handled
            }
            _ => PickerEvent::Forward,
        }
    }
    /// Apply one pass's activations in order. A pointer target, or the focus
    /// after any key press, stays inside the scrolled editor window.
    fn apply_studio(&mut self, intents: Vec<slither_client::menu_controls::ControlIntent>) {
        let nickname = self.state.view(true).nickname;
        for intent in intents {
            self.studio.activate(intent.control, &nickname);
            self.studio.state.reveal_control(intent.control, self.area);
        }
        if std::mem::take(&mut self.studio_reveal)
            && let Some(focus) = self.focused()
        {
            self.studio.state.reveal_control(focus, self.area);
        }
        self.sync_studio();
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
        if self.state.settings.visible() || self.studio.state.visible() {
            self.settings_key(egui::Key::Enter);
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
                    Key::Named(NamedKey::ArrowLeft) => Some(egui::Key::ArrowLeft),
                    Key::Named(NamedKey::ArrowRight) => Some(egui::Key::ArrowRight),
                    Key::Named(NamedKey::ArrowUp) => Some(egui::Key::ArrowUp),
                    Key::Named(NamedKey::ArrowDown) => Some(egui::Key::ArrowDown),
                    Key::Named(NamedKey::Home) => Some(egui::Key::Home),
                    Key::Named(NamedKey::End) => Some(egui::Key::End),
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
        if self.champion.policy.visible() {
            return self
                .champion
                .pointer_scaled(&mut self.text, point, down, self.pixel_scale)
                .unwrap_or_else(|e| vec![HostEditEffect::ClipboardFailed(e)]);
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
        let studio = self.studio.state.visible();
        if studio
            && resized
            && let Some(focus) = self.focused()
        {
            self.studio.state.reveal_control(focus, area);
        }
        if self.login_owns_input()
            || self.state.settings.visible()
            || self.picker.policy.visible()
            || studio
        {
            self.egui_pass(cosmetics)?;
            if !studio {
                return MenuFrame::world_crossfade(extent, dpr, view.world_alpha);
            }
            // The login fades out under the studio; its page and world cover
            // keep the composition the studio has always had.
            view.login_alpha *= 1. - self.studio.state.alpha();
            let login = view.login_alpha > 0.;
            let world = if login { 1. - view.login_alpha } else { 0. };
            let mut page = MenuFrame::world_crossfade(extent, dpr, world)?;
            if !login {
                page.world_cover = None;
            }
            return Ok(page);
        }
        // The champion editor still draws on the old toolkit: egui paints the
        // login below it and the host presents the editor above egui, so the
        // login never changes renderer.
        if self.egui.paint_only() {
            self.egui_pass(cosmetics)?;
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
        // Under the champion editor egui paints the login; keep only the page
        // and world cover its old composition drew.
        let login = view.visible.then_some(view.login_alpha);
        let underlay = self.egui.paint_only();
        if underlay {
            view.visible = false;
        }
        let mut picture = self.menu.prepare_with_panel(
            &mut self.text,
            &view,
            &self.layout,
            extent,
            dpr,
            cosmetics,
            Some(&self.input.state),
            panel,
        )?;
        if underlay && let Some(alpha) = login {
            let page = MenuFrame::world_crossfade(extent, dpr, 1. - alpha)?;
            picture.backdrop = page.backdrop;
            picture.world_cover = page.world_cover;
            picture.resources.extend(page.resources);
        }
        Ok(picture)
    }
    /// One egui pass for the login and the screen that owns it, if any.
    fn egui_pass(&mut self, cosmetics: Option<&Cosmetics>) -> Result<()> {
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
        Ok(())
    }
    pub fn displayed_nickname(&self) -> String {
        self.login_ui.nickname.clone()
    }
    pub fn cursor_area(&mut self) -> Result<Vec<HostEditEffect>> {
        if self.champion.policy.visible() {
            return self.champion.cursor_area(&mut self.text, self.pixel_scale);
        }
        Ok(Vec::new())
    }
}

enum PickerEvent {
    /// Continue through the shared egui event path.
    Forward,
    /// Consumed without a pass.
    Handled,
    /// Consumed; run one ordered pass for its focus or state change.
    Pass,
}

/// Provisional Unicode is bounded separately from each field's committed limit.
fn bound_preedit(text: &mut String, range: &mut Option<std::ops::Range<usize>>) {
    *text = slither_client::menu_input::bounded_preedit(text);
    if let Some(range) = range {
        let len = text.chars().count();
        range.start = range.start.min(len);
        range.end = range.end.min(len);
    }
}
