//! Settings layout and actions; egui resources and input belong to the native host.
use crate::egui_theme as theme;
use egui::{Id, Rect, Sense, Vec2, pos2};
use slither_client::{
    menu_controls::{Activation, ControlId},
    menu_preferences::MenuQuality,
    settings::{self, SettingsState, WorldResolution},
};

pub struct SettingsUi {
    pub controls: Vec<(ControlId, Rect, bool)>,
    pub panel_rect: Rect,
    request_focus: Option<ControlId>,
}
impl Default for SettingsUi {
    fn default() -> Self {
        Self {
            controls: Vec::new(),
            panel_rect: Rect::NOTHING,
            request_focus: None,
        }
    }
}
pub fn widget_id(id: ControlId) -> Id {
    Id::unique(("settings", id.0))
}
pub fn focus_order() -> [Id; 3] {
    [settings::QUALITY, settings::RESOLUTION, settings::DONE].map(widget_id)
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
                if interactive && response.clicked() {
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
                    egui::ScrollArea::vertical()
                        .animated(false)
                        .max_height((area.y - 238.).max(24.))
                        .show(ui, |ui| {
                            for (id, title, caption, checked, help) in [
                                (
                                    settings::QUALITY,
                                    "Quality",
                                    "High quality",
                                    state.preferences().quality == MenuQuality::High,
                                    "Off: Low. High adapts to load. Next game.",
                                ),
                                (
                                    settings::RESOLUTION,
                                    "World resolution",
                                    "Native/sharp",
                                    state.preferences().resolution == WorldResolution::Native,
                                    "Sharper world; higher GPU use. Same view area.",
                                ),
                            ] {
                                let response = theme::checkbox_row(
                                    ui,
                                    widget_id(id),
                                    title,
                                    caption,
                                    checked,
                                    help,
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
                                if response.clicked() {
                                    actions.push((id, Some(!checked)));
                                }
                            }
                            if state.native_unavailable() {
                                theme::help(ui, "Native unavailable at this size");
                            }
                            if state.not_saved() {
                                theme::help(ui, "Not saved; applies this session");
                            }
                        });
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
                        if response.clicked() {
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
