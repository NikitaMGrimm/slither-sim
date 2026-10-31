//! Native login composition and control focus; no transport or independent clock.
use crate::{
    Result,
    cosmetics::Cosmetics,
    menu::{Menu, MenuFrame},
    menu_input_native::NativeMenuInput,
    text::{Family, Slant, TextCache, TextStyle, Weight},
};
use slither_client::{
    menu_input::{EditIntent, HostEditEffect, TextField},
    menu_layout::{MenuElement as E, MenuLayout, single_line_field_placement},
    menu_preferences::MenuQuality,
    start_page::{MenuAction, MenuLocale, StartMenuIntent, StartMenuState},
};

pub struct NativeLogin {
    pub input: NativeMenuInput,
    pub champion: crate::panel_native::NativePanel,
    pub state: StartMenuState,
    pub layout: MenuLayout,
    pub focus: u8,
    menu: Menu,
    text: TextCache,
    area: [f64; 2],
    pixel_scale: f64,
    pressed: bool,
    pressed_quality: bool,
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
            || (self.state.view(true).visible
                && [E::Nickname, E::Play, E::Quality]
                    .into_iter()
                    .any(|e| self.layout.hit_test(e, point[0], point[1])))
    }
    pub fn new(nickname: &str) -> Result<Self> {
        let mut state = StartMenuState::new(MenuLocale::English, true, MenuQuality::High);
        state.edit_nickname(&nickname.encode_utf16().collect::<Vec<_>>());
        let mut input = NativeMenuInput::default();
        input.state.sync_menu(&state.view(true), &[]);
        let mut text = TextCache::menu();
        let layout = crate::menu_specimen::layout(&mut text, &state.view(true), [960, 540], 1.)?;
        Ok(Self {
            input,
            champion: crate::panel_native::NativePanel::new(
                &slither_client::session_policy::SessionPolicy::new().view(),
            ),
            state,
            layout,
            focus: 0,
            menu: Menu::default(),
            text,
            area: [960., 540.],
            pixel_scale: 1.,
            pressed: false,
            pressed_quality: false,
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
            self.focus = 0;
        }
        let mut effects = self
            .input
            .state
            .sync_menu(&self.state.view(admission), &intents);
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
            self.focus = 0;
        }
        self.input
            .state
            .sync_menu(&self.state.view(admission), &intents)
    }
    pub fn focus(&mut self, focus: u8) -> Vec<HostEditEffect> {
        self.focus = focus;
        let mut effects = self.input.state.apply(EditIntent::Focus(if focus == 0 {
            Some(TextField::Nickname)
        } else {
            None
        }));
        for key in [
            slither_client::menu_input::GameplayKey::Left,
            slither_client::menu_input::GameplayKey::Right,
            slither_client::menu_input::GameplayKey::Up,
            slither_client::menu_input::GameplayKey::Space,
        ] {
            effects.push(HostEditEffect::Gameplay {
                key,
                pressed: false,
            });
        }
        effects.push(HostEditEffect::ReleasePointerBoost);
        effects
    }
    pub fn activate(&mut self, admission: bool) {
        match self.focus {
            0 | 1 => {
                if !self.input.state.editor(TextField::Nickname).composing() {
                    self.state.action(MenuAction::Play, admission);
                }
            }
            2 => {
                self.state.action(MenuAction::Quality, admission);
            }
            _ => {}
        }
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
                .pointer(&mut self.text, point, down)
                .unwrap_or_else(|e| vec![HostEditEffect::ClipboardFailed(e)]);
        }
        let view = self.state.view(admission);
        self.menu.skin_hovered = view.visible && self.layout.hit_test(E::Skin, point[0], point[1]);
        if let Some(down) = down {
            self.menu.skin_pressed = down && self.menu.skin_hovered;
        }
        if !view.visible || view.disabled {
            self.pressed = false;
            return Vec::new();
        }
        let hit = [E::Nickname, E::NicknameHolder, E::Play, E::Quality]
            .into_iter()
            .find(|e| self.layout.hit_test(*e, point[0], point[1]));
        let hover = hit == Some(E::Play);
        // Windows owns commit ordering. A pointer cannot cancel raw preedit and
        // then submit the older committed name in the same gesture.
        if self.input.state.editor(TextField::Nickname).composing() && hit != Some(E::Nickname) {
            self.pressed = false;
            return Vec::new();
        }
        if down == Some(true) {
            self.pressed = hover;
            self.pressed_quality = hit == Some(E::Quality);
        }
        self.state
            .play_pointer(hover, self.pressed && hover, admission);
        if down == Some(false) {
            if self.pressed_quality && hit == Some(E::Quality) {
                self.state.action(MenuAction::Quality, admission);
            }
            self.pressed_quality = false;
            let clicked = self.pressed && hover;
            self.pressed = false;
            self.state.play_pointer(hover, false, admission);
            if clicked {
                self.activate_play(admission);
            }
        }
        if down == Some(true) {
            match hit {
                Some(E::Nickname | E::NicknameHolder) => {
                    let mut effects = self.focus(0);
                    let r = self.layout.viewport_rect(E::Nickname);
                    let editor = self.input.state.editor(TextField::Nickname);
                    let style = TextStyle {
                        size: 15. * self.layout.scale,
                        weight: Weight::Normal,
                        family: Family::Lucida,
                        slant: Slant::Normal,
                        color: [224, 224, 255],
                    };
                    let mut selected = 0;
                    let inset = crate::menu::FIELD_CONTENT_INSET.map(|v| v * self.layout.scale);
                    let content_width = r.width - 2. * inset[0];
                    let Ok(m) = self.text.measure_panel(editor.text(), style) else {
                        return effects;
                    };
                    let origin_x = single_line_field_placement(
                        r,
                        inset,
                        style.size,
                        m.font_ascent,
                        m.font_descent,
                    )
                    .origin[0];
                    let width = m.advance;
                    let fit = if width > content_width {
                        content_width / width
                    } else {
                        1.
                    };
                    for end in editor.text().char_indices().map(|(i, c)| i + c.len_utf8()) {
                        if let Ok(m) = self.text.measure_panel(&editor.text()[..end], style)
                            && point[0] - origin_x >= m.advance * fit
                        {
                            selected = end;
                        }
                    }
                    effects.extend(self.input.state.apply(EditIntent::Select {
                        anchor: selected,
                        caret: selected,
                    }));
                    return effects;
                }
                Some(E::Play) => return self.focus(1),
                Some(E::Quality) => {
                    return self.focus(2);
                }
                _ => {}
            }
        }
        Vec::new()
    }
    fn activate_play(&mut self, admission: bool) {
        if !self.input.state.editor(TextField::Nickname).composing() {
            self.state.action(MenuAction::Play, admission);
        }
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
        self.menu.focus = if view.disabled {
            None
        } else {
            match self.focus {
                1 => Some(E::Play),
                2 => Some(E::Quality),
                _ => None,
            }
        };
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
            None
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
        let r = self.layout.viewport_rect(E::Nickname);
        let editor = self.input.state.editor(TextField::Nickname);
        let (anchor, caret) = editor.selection();
        let prefix = if let Some(preedit) = editor.preedit() {
            let end = preedit.selection.map_or(preedit.text.len(), |(_, end)| end);
            format!(
                "{}{}",
                &editor.text()[..anchor.min(caret)],
                &preedit.text[..end]
            )
        } else {
            editor.text()[..caret].to_owned()
        };
        let display = self.displayed_nickname();
        let style = TextStyle {
            size: 15. * self.layout.scale,
            weight: Weight::Normal,
            family: Family::Lucida,
            slant: Slant::Normal,
            color: [224, 224, 255],
        };
        let inset = crate::menu::FIELD_CONTENT_INSET.map(|v| v * self.layout.scale);
        let content_width = r.width - 2. * inset[0];
        let width = self.text.measure_panel(&display, style)?.advance;
        let fit = if width > content_width {
            content_width / width
        } else {
            1.
        };
        let x = (self.text.measure_panel(&prefix, style)?.advance * fit).min(content_width - 1.);
        let fitted = TextStyle {
            size: style.size * fit,
            ..style
        };
        let m = self.text.measure_panel("", fitted)?;
        let placement =
            single_line_field_placement(r, inset, fitted.size, m.font_ascent, m.font_descent);
        Ok(self.input.state.apply(EditIntent::CursorArea {
            position: [placement.origin[0] + x, placement.caret_top]
                .map(|v| (v * self.pixel_scale).round() / self.pixel_scale),
            size: [1., placement.caret_height]
                .map(|v| (v * self.pixel_scale).round().max(1.) / self.pixel_scale),
        }))
    }
}
