//! Native in-game menu; session/input policy remains in LocalGame and the core.
use crate::egui_theme as theme;
use egui::{Id, Rect, Sense, pos2, vec2};

/// The menu's controls, in focus order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Resume,
    Settings,
    Leave,
}
pub const CONTROLS: [Control; 3] = [Control::Resume, Control::Settings, Control::Leave];

pub fn widget_id(control: Control) -> Id {
    Id::unique(("ingame", control as u8))
}
pub fn focus_order() -> [Id; 3] {
    CONTROLS.map(widget_id)
}

/// Pill slot height from `egui_theme`, including its focus/shadow margin.
const SLOT: f32 = 52.;
const SLOT_GAP: f32 = 4.;
const MAX_WIDTH: f32 = 256.;
const GUTTER: f32 = 16.;
/// The world stays visible under the menu: the game keeps running.
pub const DIM: f32 = 0.45;

pub struct InGameUi {
    pub panel_rect: Rect,
    /// Control, painted rectangle and focus, from the last pass.
    pub controls: Vec<(Control, Rect, bool)>,
    /// The control that keeps focus when a press elsewhere surrenders it.
    focus: Control,
    request_focus: Option<Control>,
}
impl Default for InGameUi {
    fn default() -> Self {
        Self {
            panel_rect: Rect::NOTHING,
            controls: Vec::new(),
            focus: Control::Resume,
            request_focus: None,
        }
    }
}
impl InGameUi {
    pub fn request_focus(&mut self, control: Control) {
        self.focus = control;
        self.request_focus = Some(control);
    }
    pub fn focused(&self) -> Option<Control> {
        self.controls
            .iter()
            .find(|(_, _, focus)| *focus)
            .map(|(control, _, _)| *control)
    }
    pub fn rect(&self, control: Control) -> Rect {
        self.controls
            .iter()
            .find(|(c, _, _)| *c == control)
            .map_or(Rect::NOTHING, |(_, rect, _)| *rect)
    }
    /// `opacity` fades the panel under Settings; only an `interactive` menu
    /// takes input. The dimmed world stays below both.
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        interactive: bool,
        opacity: f32,
        reveal: bool,
    ) -> Option<Control> {
        self.controls.clear();
        let viewport = ctx.input(|i| i.viewport_rect());
        let width = (viewport.width() - 2. * GUTTER).clamp(1., MAX_WIDTH);
        let padding = (if width < 192. { 4_f32 } else { 16. })
            .min(((viewport.height() - SLOT) / 2.).clamp(0., 16.));
        let content = CONTROLS.len() as f32 * SLOT + (CONTROLS.len() - 1) as f32 * SLOT_GAP;
        let margin = GUTTER.min(((viewport.height() - SLOT) / 2. - padding).max(0.));
        let list = content.min((viewport.height() - 2. * (margin + padding)).max(1.));
        self.panel_rect =
            Rect::from_center_size(viewport.center(), vec2(width, list + 2. * padding));
        let mut action = None;
        if interactive && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            action = Some(Control::Resume);
        }
        // One layer below Settings' backdrop and panel. Later widgets win the
        // hit test, so the panel shields its padding from the backdrop.
        egui::Area::new(Id::unique("ingame-menu"))
            .fixed_pos(pos2(0., 0.))
            .order(egui::Order::Background)
            .movable(false)
            .fade_in(false)
            .show(ctx, |ui| {
                let backdrop = ui.allocate_rect(viewport, Sense::CLICK);
                ui.painter().rect_filled(
                    viewport,
                    0.,
                    theme::ink(slither_client::menu_theme::MenuTheme::default().backdrop)
                        .gamma_multiply(DIM),
                );
                // A press and its release must both land outside the panel.
                let outside = ctx
                    .input(|i| i.pointer.interact_pos())
                    .is_some_and(|pos| !self.panel_rect.contains(pos));
                if interactive && outside && crate::egui_panel::owned_click(&backdrop) {
                    action = Some(Control::Resume);
                }
                ui.scope_builder(egui::UiBuilder::new().max_rect(self.panel_rect), |ui| {
                    ui.set_opacity(opacity);
                    if !interactive {
                        ui.disable();
                    }
                    ui.interact(self.panel_rect, Id::unique("ingame-panel"), Sense::CLICK);
                    theme::panel_frame().inner_margin(padding).show(ui, |ui| {
                        let inner = width - 2. * padding;
                        ui.set_width(inner.max(1.));
                        ui.spacing_mut().item_spacing.y = SLOT_GAP;
                        egui::ScrollArea::vertical()
                            .animated(false)
                            .max_height(list)
                            .show(ui, |ui| {
                                for control in CONTROLS {
                                    let label = match control {
                                        Control::Resume => "Resume",
                                        Control::Settings => "Settings",
                                        Control::Leave => "Leave game",
                                    };
                                    let response = theme::pill_button(
                                        ui,
                                        widget_id(control),
                                        label,
                                        inner.max(1.),
                                        control == Control::Resume,
                                    );
                                    if interactive && response.is_pointer_button_down_on() {
                                        self.focus = control;
                                    }
                                    // Panel/backdrop senses never enter the focus
                                    // order, and a press elsewhere, which egui
                                    // treats as surrendering focus, keeps it.
                                    // Tab and arrows move it between controls.
                                    let elsewhere = !ctx
                                        .memory(|m| m.focused())
                                        .is_some_and(|id| focus_order().contains(&id));
                                    if interactive
                                        && (self.request_focus == Some(control)
                                            || (elsewhere && self.focus == control))
                                    {
                                        response.request_focus();
                                    }
                                    if response.has_focus() && reveal {
                                        response.scroll_to_me(None);
                                    }
                                    self.controls.push((
                                        control,
                                        response.rect.intersect(ui.clip_rect()),
                                        response.has_focus(),
                                    ));
                                    if interactive && crate::egui_panel::owned_click(&response) {
                                        action = Some(control);
                                    }
                                }
                            });
                    });
                });
            });
        if let Some(control) = self.focused() {
            self.focus = control;
        }
        self.request_focus = None;
        action
    }
}
