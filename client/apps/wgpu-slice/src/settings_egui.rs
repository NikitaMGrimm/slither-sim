//! Settings layout and actions; egui resources and input belong to the native host.
use crate::egui_theme as theme;
use egui::{Id, Rect, Sense, Vec2, pos2};
use slither_client::{
    menu_controls::{Activation, ControlId},
    settings::{self, SettingsState, WorldResolution},
};

pub struct SettingsUi {
    pub controls: Vec<(ControlId, Rect, bool)>,
    pub panel_rect: Rect,
    pub scroll_id: Option<Id>,
    request_focus: Option<ControlId>,
    scroll_offset: f32,
}
impl Default for SettingsUi {
    fn default() -> Self {
        Self {
            controls: Vec::new(),
            panel_rect: Rect::NOTHING,
            scroll_id: None,
            request_focus: None,
            scroll_offset: 0.,
        }
    }
}
pub fn widget_id(id: ControlId) -> Id {
    Id::unique(("settings", id.0))
}
pub fn control_order() -> [ControlId; 10] {
    let q = settings::QUALITY_CHOICES.map(|(id, _)| id);
    let a = settings::AA_CHOICES.map(|(id, _)| id);
    [
        q[0],
        q[1],
        q[2],
        q[3],
        settings::RESOLUTION,
        a[0],
        a[1],
        a[2],
        a[3],
        settings::DONE,
    ]
}
pub fn focus_order() -> [Id; 10] {
    control_order().map(widget_id)
}
impl SettingsUi {
    pub fn request_focus(&mut self, control: ControlId) {
        self.request_focus = Some(control);
    }
    pub fn focused(&self) -> Option<ControlId> {
        self.controls
            .iter()
            .find(|(_, _, focus)| *focus)
            .map(|(id, _, _)| *id)
    }
    pub fn show(&mut self, ctx: &egui::Context, state: &mut SettingsState, reveal: bool) {
        self.controls.clear();
        let area = ctx.input(|i| i.viewport_rect().size());
        let alpha = state.alpha() as f32;
        let interactive = state.is_open();
        let mut actions = Vec::new();
        if interactive && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            actions.push((settings::DONE, None));
        }
        egui::Area::new(Id::unique("settings-backdrop"))
            .fixed_pos(pos2(0., 0.))
            .order(egui::Order::Middle)
            .movable(false)
            .fade_in(false)
            .show(ctx, |ui| {
                let rect = Rect::from_min_size(pos2(0., 0.), area);
                let response = ui.allocate_rect(rect, Sense::click());
                ui.painter().rect_filled(
                    rect,
                    0.,
                    theme::ink(slither_client::menu_theme::MenuTheme::default().backdrop)
                        .gamma_multiply(0.88 * alpha),
                );
                if crate::egui_panel::owned_click(&response) && interactive {
                    actions.push((settings::DONE, None));
                }
            });
        let width = theme::PANEL_WIDTH.min((area.x - 2. * theme::PANEL_GUTTER).max(1.));
        egui::Area::new(Id::unique("settings-panel"))
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .order(egui::Order::Foreground)
            .movable(false)
            .fade_in(false)
            .show(ctx, |ui| {
                ui.set_opacity(alpha);
                if !interactive {
                    ui.disable();
                }
                theme::panel_frame().show(ui, |ui| {
                    ui.set_width((width - 2. * f32::from(theme::PANEL_PADDING)).max(1.));
                    theme::title(ui, "Settings");
                    ui.add_space(theme::ROW_GAP);
                    let mut toggle_held = false;
                    let scroll = egui::ScrollArea::vertical()
                        .animated(false)
                        .max_height((area.y - 238.).max(24.))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Graphics").strong().size(20.));
                            ui.label("Graphics quality");
                            let preferences = state.preferences();
                            let columns = if ui.available_width() < 280. { 1 } else { 2 };
                            let choice_width =
                                (ui.available_width() - 8. * (columns - 1) as f32) / columns as f32;
                            egui::Grid::new("graphics-presets")
                                .num_columns(columns)
                                .spacing(egui::vec2(8., 0.))
                                .show(ui, |ui| {
                                    let width = choice_width;
                                    for (index, (id, preset)) in
                                        settings::QUALITY_CHOICES.into_iter().enumerate()
                                    {
                                        let response = theme::radio_choice(
                                            ui,
                                            widget_id(id),
                                            preset.label(),
                                            preferences.graphics == preset,
                                            width,
                                        );
                                        if self.request_focus == Some(id) {
                                            response.request_focus();
                                        }
                                        if response.has_focus() && reveal {
                                            response.scroll_to_me(Some(egui::Align::Center));
                                        }
                                        self.controls.push((
                                            id,
                                            response.rect.intersect(ui.clip_rect()),
                                            response.has_focus(),
                                        ));
                                        toggle_held |= response.is_pointer_button_down_on();
                                        if crate::egui_panel::owned_click(&response) {
                                            actions.push((id, Some(true)));
                                        }
                                        if (index + 1) % columns == 0 {
                                            ui.end_row();
                                        }
                                    }
                                });
                            theme::help(ui, "Auto adapts to load. Presets apply next game.");
                            ui.add_space(theme::ROW_GAP);
                            let id = settings::RESOLUTION;
                            let checked = preferences.resolution == WorldResolution::Native;
                            let response = theme::checkbox_row(
                                ui,
                                widget_id(id),
                                "World resolution",
                                "Native/sharp",
                                checked,
                                "Sharper world; higher GPU use. Same view area.",
                            );
                            if self.request_focus == Some(id) {
                                response.request_focus();
                            }
                            if response.has_focus() && reveal {
                                response.scroll_to_me(Some(egui::Align::Center));
                            }
                            self.controls.push((
                                id,
                                response.rect.intersect(ui.clip_rect()),
                                response.has_focus(),
                            ));
                            toggle_held |= response.is_pointer_button_down_on();
                            if crate::egui_panel::owned_click(&response) {
                                actions.push((id, Some(!checked)));
                            }
                            ui.label("Anti-aliasing");
                            egui::Grid::new("graphics-aa")
                                .num_columns(columns)
                                .spacing(egui::vec2(8., 0.))
                                .show(ui, |ui| {
                                    let width = choice_width;
                                    for (index, (id, aa)) in
                                        settings::AA_CHOICES.into_iter().enumerate()
                                    {
                                        let response = theme::radio_choice(
                                            ui,
                                            widget_id(id),
                                            aa.label(),
                                            preferences.anti_aliasing == aa,
                                            width,
                                        );
                                        if self.request_focus == Some(id) {
                                            response.request_focus();
                                        }
                                        if response.has_focus() && reveal {
                                            response.scroll_to_me(Some(egui::Align::Center));
                                        }
                                        self.controls.push((
                                            id,
                                            response.rect.intersect(ui.clip_rect()),
                                            response.has_focus(),
                                        ));
                                        toggle_held |= response.is_pointer_button_down_on();
                                        if crate::egui_panel::owned_click(&response) {
                                            actions.push((id, Some(true)));
                                        }
                                        if (index + 1) % columns == 0 {
                                            ui.end_row();
                                        }
                                    }
                                });
                            let desired = match preferences.anti_aliasing {
                                settings::AntiAliasing::X2 => 2,
                                settings::AntiAliasing::X4 => 4,
                                _ => state.world_samples(),
                            };
                            if desired != state.world_samples() {
                                theme::help(
                                    ui,
                                    &format!(
                                        "{}× unavailable here; using {}×.",
                                        desired,
                                        state.world_samples()
                                    ),
                                );
                            } else {
                                theme::help(
                                    ui,
                                    &format!(
                                        "Effective: {}×. Auto keeps the 64 MiB policy.",
                                        state.world_samples()
                                    ),
                                );
                            }
                            if state.native_unavailable() {
                                theme::help(ui, "Native unavailable at this size");
                            }
                            if state.not_saved() {
                                theme::help(ui, "Not saved; applies this session");
                            }
                        });
                    self.scroll_id = Some(scroll.id);
                    // Scrolling moves a held toggle, so its release no longer
                    // matches the press, even after scrolling back.
                    let offset = scroll.state.clamped_offset().y;
                    if self.scroll_offset != offset && toggle_held {
                        crate::egui_panel::release_capture(ui.ctx());
                    }
                    self.scroll_offset = offset;
                    ui.add_space(theme::ROW_GAP);
                    ui.vertical_centered(|ui| {
                        let response = theme::pill_button(
                            ui,
                            widget_id(settings::DONE),
                            "Done",
                            112_f32.min(ui.available_width()),
                            true,
                        );
                        if self.request_focus == Some(settings::DONE) {
                            response.request_focus();
                        }
                        self.controls
                            .push((settings::DONE, response.rect, response.has_focus()));
                        if crate::egui_panel::owned_click(&response) {
                            actions.push((settings::DONE, None));
                        }
                    });
                });
                self.panel_rect = ui.min_rect();
            });
        self.request_focus = None;
        for (control, checked) in actions {
            state.activate(state.intent(control, checked, Activation::Pointer));
        }
    }
}
