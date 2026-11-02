//! egui painters and input guards shared by screens drawn from a core
//! `PanelView`: each widget sits at the rectangle the core resolves.
use crate::egui_theme as theme;
use egui::{Align, Color32, Id, Rect, Sense, Stroke, pos2, vec2};
use slither_client::{
    menu_controls::{Control, ControlKind, PanelView},
    menu_layout::MenuRect,
    menu_theme::{ControlState, ControlStyle, MenuTheme, SurfaceRecipe},
    presentation::TextStyle,
};

pub fn rect(r: MenuRect) -> Rect {
    Rect::from_min_size(
        pos2(r.x as f32, r.y as f32),
        vec2(r.width as f32, r.height as f32),
    )
}

pub fn available(panel: &PanelView, control: &Control) -> bool {
    panel.visible
        && panel.enabled
        && control.visible
        && control.enabled
        && !matches!(control.kind, ControlKind::Label { .. })
}

/// Whether `id` can hold focus in the current panel.
pub fn focusable(panel: &PanelView, id: slither_client::menu_controls::ControlId) -> bool {
    panel
        .controls
        .iter()
        .any(|c| c.id == id && c.tab_order.is_some() && available(panel, c))
}

/// Whether `id` can take focus by pointer or host request: any available
/// control, including roving grid items outside the tab order.
pub fn focus_target(panel: &PanelView, id: slither_client::menu_controls::ControlId) -> bool {
    panel.controls.iter().any(|c| {
        c.id == id
            && available(panel, c)
            && !matches!(c.kind, ControlKind::Surface | ControlKind::Backdrop)
    })
}

/// Tab wraps within the modal in the core's tab order, skipping disabled controls.
pub fn focus_order(
    panel: &PanelView,
    widget_id: impl Fn(slither_client::menu_controls::ControlId) -> Id,
) -> Vec<Id> {
    let mut order: Vec<_> = panel
        .controls
        .iter()
        .filter(|c| available(panel, c))
        .filter_map(|c| c.tab_order.map(|n| (n, c.id)))
        .collect();
    order.sort();
    order.into_iter().map(|(_, id)| widget_id(id)).collect()
}

/// A primary press held across a panel generation change (open, section,
/// directory or window replacement) must not click whatever control now
/// reuses its ID. Only that press's release is suppressed.
#[derive(Default)]
pub struct PressGuard {
    /// The primary button was down at the end of the last pass.
    primary_down: bool,
    stale: bool,
}
impl PressGuard {
    pub fn invalidate(&mut self) {
        self.stale |= self.primary_down;
    }
    /// Whether this pass's primary release belongs to an invalidated press.
    /// Any primary edge ends the suppression, so a fresh press clicks normally.
    pub fn suppress(&mut self, ctx: &egui::Context) -> bool {
        let mut suppress = false;
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::PointerButton {
                    button: egui::PointerButton::Primary,
                    pressed,
                    ..
                } = event
                {
                    suppress |= !*pressed && self.stale;
                    self.stale = false;
                }
            }
        });
        suppress
    }
    /// Resyncing with egui also retires a press the host cancelled (resize,
    /// focus loss) without delivering its release.
    pub fn settle(&mut self, ctx: &egui::Context) {
        self.primary_down = ctx.input(|i| i.pointer.button_down(egui::PointerButton::Primary));
        if !self.primary_down {
            self.stale = false;
        }
    }
}

/// A retained-toolkit drop shadow: offset down, blur and spread in logical
/// pixels, then peak opacity. egui's linear feather is fitted to the old
/// rasters' measured bottom, side and top edge profiles at DPR 1; their
/// Gaussian tails cannot match exactly.
#[derive(Clone, Copy)]
pub struct Drop {
    offset: f32,
    blur: f32,
    spread: f32,
    alpha: u8,
}
/// Panels and cosmetic cells: a short falloff, mostly below.
pub const PANEL: Drop = Drop {
    offset: 3.,
    blur: 6.,
    spread: 4.,
    alpha: 153,
};
/// Section tabs: as panels, with less above the tab.
pub const TAB: Drop = Drop {
    offset: 4.,
    blur: 6.,
    spread: 3.,
    alpha: 156,
};
/// Compact studio capsules: a plateau below, then a long falloff.
pub const CAPSULE: Drop = Drop {
    offset: 4.,
    blur: 12.,
    spread: 5.,
    alpha: 158,
};
/// Full buttons: the Play surface's lighter shadow with a long tail.
pub const PLAY: Drop = Drop {
    offset: 3.,
    blur: 20.,
    spread: 2.,
    alpha: 148,
};
pub fn shadow(ui: &egui::Ui, r: Rect, radius: f32, scale: f32, drop: Drop) {
    let px = |v: f32| (v * scale).round().min(127.);
    ui.painter().add(
        egui::epaint::Shadow {
            offset: [0, px(drop.offset) as i8],
            blur: px(drop.blur) as u8,
            spread: px(drop.spread) as u8,
            color: Color32::from_black_alpha(drop.alpha),
        }
        .as_shape(r, radius.min(255.) as u8),
    );
}

/// One logical-pixel keyboard-focus ring, kept inside the hit box.
pub fn outline(ui: &egui::Ui, r: Rect, radius: f32, tokens: MenuTheme) {
    ui.painter().rect_stroke(
        r.shrink(1.),
        (radius - 1.).max(0.),
        Stroke::new(1., theme::ink(tokens.text)),
        egui::StrokeKind::Inside,
    );
}

/// The core panel's passive surface: compact shadow and flat fill. Blank
/// space shields the backdrop without taking focus.
pub fn surface(ui: &egui::Ui, control: &Control, r: Rect, scale: f32, id: Id, tokens: MenuTheme) {
    let surface = tokens.surface(control.style, ControlState::Idle);
    let radius = f32::from(surface.radius) * scale;
    shadow(ui, r, radius, scale, PANEL);
    ui.painter()
        .rect_filled(r, radius, theme::ink(surface.colors[0]));
    ui.interact(r, id, Sense::CLICK);
}

/// Core button styles: compact studio capsules dim while pressed; full
/// buttons take the retained hover/focus and pressed overlays.
#[allow(clippy::too_many_arguments)]
pub fn button(
    ui: &egui::Ui,
    control: &Control,
    text: &str,
    r: Rect,
    scale: f32,
    style: TextStyle,
    id: Id,
    keyboard_focus: bool,
    tokens: MenuTheme,
) -> egui::Response {
    let response = ui.interact(r, id, Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), text));
    let surface = tokens.surface(control.style, ControlState::Idle);
    let compact = surface.recipe == SurfaceRecipe::StudioButton;
    let radius = if compact {
        r.height() / 2.
    } else {
        (f32::from(surface.radius) * scale).min(r.height() / 2.)
    };
    let pressed = response.is_pointer_button_down_on();
    let focused = response.has_focus() && keyboard_focus && ui.is_enabled();
    let drop = if matches!(control.kind, ControlKind::SectionTab { .. }) {
        TAB
    } else if compact {
        CAPSULE
    } else {
        PLAY
    };
    shadow(ui, r, radius, scale, drop);
    let colors = surface.colors.map(|rgb| {
        let color = theme::ink(rgb);
        // Compact actions dim their surface, not their label, while pressed.
        if compact && pressed {
            let [r, g, b, _] = color.gamma_multiply(0.75).to_array();
            Color32::from_rgb(r, g, b)
        } else {
            color
        }
    });
    theme::capsule(ui, r, colors, 1.);
    if !compact && ui.is_enabled() {
        for (state, alpha) in [
            (
                ControlState::Hovered,
                if response.hovered() {
                    1.
                } else if focused {
                    0.33
                } else {
                    0.
                },
            ),
            (ControlState::Pressed, if pressed { 1. } else { 0. }),
        ] {
            if alpha > 0. {
                let overlay = tokens.surface(control.style, state).colors.map(theme::ink);
                theme::capsule(ui, r, overlay.map(|c| c.gamma_multiply(alpha)), 1.);
            }
        }
    }
    label(
        ui.painter(),
        text,
        r.shrink2(vec2(
            slither_client::menu_theme::STUDIO_ACTION_PADDING_X as f32 * scale,
            0.,
        )),
        style,
        Align::Center,
        control.style != ControlStyle::StudioAction,
    );
    if focused {
        outline(ui, r, radius, tokens);
    }
    response
}

/// Row columns crop to their own box; other labels crop only to the panel.
pub fn column(ui: &egui::Ui, value: &str, r: Rect, style: TextStyle, align: Align) {
    let painter = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    label(&painter, value, r, style, align, false);
}

pub fn label(
    painter: &egui::Painter,
    value: &str,
    r: Rect,
    style: TextStyle,
    align: Align,
    fit: bool,
) {
    if value.is_empty() {
        return;
    }
    let family = theme::family(style);
    let color = theme::ink(style.color);
    let mut galley = painter.layout_no_wrap(
        value.into(),
        egui::FontId::new(style.size as f32, family.clone()),
        color,
    );
    if fit && galley.size().x > r.width() && r.width() > 0. {
        let size = style.size as f32 * r.width() / galley.size().x;
        galley = painter.layout_no_wrap(value.into(), egui::FontId::new(size, family), color);
    }
    let x = match align {
        Align::Min => r.left(),
        Align::Center => r.center().x - galley.size().x / 2.,
        Align::Max => r.right() - galley.size().x,
    };
    painter.galley(pos2(x, r.center().y - galley.size().y / 2.), galley, color);
}
