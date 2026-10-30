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
    menu_layout::{MenuElement as E, MenuLayout},
    menu_preferences::MenuQuality,
    start_page::{MenuAction, MenuLocale, StartMenuIntent, StartMenuState},
};

pub struct NativeLogin {
    pub input: NativeMenuInput,
    pub state: StartMenuState,
    pub layout: MenuLayout,
    pub focus: u8,
    menu: Menu,
    text: TextCache,
    area: [f64; 2],
    pressed: bool,
    pressed_quality: bool,
}
impl NativeLogin {
    pub fn blocks_gameplay(&self) -> bool {
        matches!(
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
            state,
            layout,
            focus: 0,
            menu: Menu::default(),
            text,
            area: [960., 540.],
            pressed: false,
            pressed_quality: false,
        })
    }
    pub fn step(
        &mut self,
        now: f64,
        admission: bool,
        playing: bool,
    ) -> Result<(Vec<StartMenuIntent>, Vec<HostEditEffect>)> {
        let intents = self.state.step(now, false).map_err(str::to_owned)?;
        if playing {
            self.state.begin_outgoing();
        }
        if intents.contains(&StartMenuIntent::FocusNickname) {
            self.focus = 0;
        }
        let effects = self
            .input
            .state
            .sync_menu(&self.state.view(admission), &intents);
        Ok((intents, effects))
    }
    pub fn observe_session(&mut self, phase: slither_client::session_policy::Phase) -> bool {
        self.state.attempt_ended(phase)
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
        let view = self.state.view(admission);
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
                        family: Family::Arial,
                        slant: Slant::Normal,
                        color: [224, 224, 255],
                    };
                    let mut selected = 0;
                    let width = self
                        .text
                        .measure_panel(editor.text(), style)
                        .map(|m| m.advance)
                        .unwrap_or(0.);
                    let fit = if width > r.width { r.width / width } else { 1. };
                    for end in editor.text().char_indices().map(|(i, c)| i + c.len_utf8()) {
                        if let Ok(m) = self.text.measure_panel(&editor.text()[..end], style)
                            && point[0] - r.x >= m.advance * fit
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
        let area = physical.map(|n| f64::from(n) / window_scale);
        let backing = crate::render::backing_size(area)?;
        self.prepare_scaled(physical, window_scale, admission, cosmetics)?
            .into_backing(backing)
    }
    fn prepare_scaled(
        &mut self,
        extent: [u32; 2],
        dpr: f64,
        admission: bool,
        cosmetics: Option<&Cosmetics>,
    ) -> Result<MenuFrame> {
        let mut view = self.state.view(admission);
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
        let fresh = crate::menu_specimen::layout(&mut self.text, &view, extent, dpr)?;
        if self.area != area {
            self.layout = fresh;
            self.area = area;
        } else {
            // Keep resize-time top across fade updates.
            let style = TextStyle {
                size: 20.,
                weight: Weight::Bold,
                family: Family::Arial,
                slant: Slant::Normal,
                color: [255; 3],
            };
            self.layout
                .update(
                    view.transition,
                    view.tips_visible,
                    slither_client::menu_layout::MenuMetrics {
                        last_length_line_height: 18.,
                        nickname_baseline_space: 4.,
                        padded_play_label_width: self
                            .text
                            .measure_panel(view.play_label, style)?
                            .advance
                            + 20.,
                    },
                )
                .map_err(str::to_owned)?;
        }
        view.nickname = self.displayed_nickname();
        self.menu.prepare_with_input(
            &mut self.text,
            &view,
            &self.layout,
            extent,
            dpr,
            cosmetics,
            0,
            Some(&self.input.state),
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
            family: Family::Arial,
            slant: Slant::Normal,
            color: [224, 224, 255],
        };
        let width = self.text.measure_panel(&display, style)?.advance;
        let fit = if width > r.width { r.width / width } else { 1. };
        let x = (self.text.measure_panel(&prefix, style)?.advance * fit).min(r.width - 1.);
        Ok(self.input.state.apply(EditIntent::CursorArea {
            position: [r.x + x, r.y],
            size: [1., r.height],
        }))
    }
}
