//! Native in-game overlay; session/input policy remains in LocalGame and the core.
use crate::egui_theme as theme;
use egui::{Id, Rect};

pub fn leave_id() -> Id {
    Id::unique("ingame-leave")
}

#[derive(Clone, Copy)]
pub enum Action {
    Dismiss,
    Leave,
}

pub struct InGameUi {
    pub panel_rect: Rect,
    pub leave_rect: Rect,
    pub focused: bool,
    backdrop_press: bool,
}
impl Default for InGameUi {
    fn default() -> Self {
        Self {
            panel_rect: Rect::NOTHING,
            leave_rect: Rect::NOTHING,
            focused: false,
            backdrop_press: false,
        }
    }
}
impl InGameUi {
    pub fn show(&mut self, ctx: &egui::Context) -> Option<Action> {
        let viewport = ctx.input(|i| i.viewport_rect().size());
        let width = (viewport.x - 32.).clamp(1., 256.);
        let padding = (if width < 192. { 4_f32 } else { 16. })
            .min(((viewport.y - 52.) / 2.).clamp(0., 16.)) as i8;
        let modal = egui::Modal::new(Id::unique("ingame-menu"))
            .area(egui::Modal::default_area(Id::unique("ingame-menu")).fade_in(false))
            .frame(theme::panel_frame().inner_margin(padding))
            .backdrop_color(
                theme::ink(slither_client::menu_theme::MenuTheme::default().backdrop)
                    .gamma_multiply(0.45),
            )
            .show(ctx, |ui| {
                ui.set_width((width - 2. * f32::from(padding)).max(1.));
                let response =
                    theme::pill_button(ui, leave_id(), "Leave game", ui.available_width(), true);
                // Panel/backdrop senses deliberately do not enter the focus order.
                if !response.has_focus() {
                    response.request_focus();
                }
                self.leave_rect = response.rect;
                self.focused = response.has_focus();
                crate::egui_panel::owned_click(&response)
            });
        self.panel_rect = modal.response.rect;
        // egui's modal paints its panel in the backdrop's layer, so its backdrop
        // click doesn't look at the panel.
        let on_backdrop = ctx
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|pos| !self.panel_rect.contains(pos));
        if ctx.input(|i| i.pointer.primary_pressed()) {
            self.backdrop_press = on_backdrop;
        }
        let backdrop_click = self.backdrop_press
            && on_backdrop
            && modal
                .backdrop_response
                .clicked_by(egui::PointerButton::Primary);
        let escape = modal.is_top_modal
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        if backdrop_click || escape {
            Some(Action::Dismiss)
        } else if modal.inner {
            Some(Action::Leave)
        } else {
            None
        }
    }
}
