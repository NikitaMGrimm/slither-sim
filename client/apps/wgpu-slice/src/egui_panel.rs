//! egui painters and input guards shared by screens drawn from a core
//! `PanelView`: each widget sits at the rectangle the core resolves.
use crate::egui_theme as theme;
use egui::{Align, Color32, Id, Rect, Sense, Stroke, pos2, vec2};
use slither_client::{
    menu_controls::{Activation, Control, ControlKind, PanelView},
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

fn capture_key() -> Id {
    Id::unique("menu pointer capture")
}
/// Forget the press a pointer click would match: the host cancelled it or
/// replaced the screen, or the button is no longer down.
pub fn release_capture(ctx: &egui::Context) {
    ctx.data_mut(|data| data.remove::<Id>(capture_key()));
}
/// Whether `response` owns the primary press, recording a new one. egui's own
/// click target does not decide this: another button's press can seed it, and it
/// can outlive the pointer leaving the widget.
fn owns_press(response: &egui::Response) -> bool {
    let ctx = &response.ctx;
    if response.enabled()
        && ctx.input(|i| i.pointer.primary_pressed())
        && response.hovered()
        && response.contains_pointer()
    {
        ctx.data_mut(|data| data.insert_temp(capture_key(), response.id));
    }
    ctx.data(|data| data.get_temp::<Id>(capture_key())) == Some(response.id)
}
fn keyboard_click(response: &egui::Response) -> bool {
    response.clicked() && !response.clicked_by(egui::PointerButton::Primary)
}
/// A primary press and release on the same widget, however long it was held and
/// wherever the pointer went between them. egui's own click gives up after 0.8 s
/// or 6 points of movement, which the menus never did; those limits stay global
/// because text selection needs them to tell a drag from a click.
pub fn pointer_click(response: &egui::Response) -> bool {
    let owned = owns_press(response);
    if !response.enabled() {
        if owned {
            release_capture(&response.ctx);
        }
        return false;
    }
    owned
        && response.ctx.input(|i| i.pointer.primary_released())
        // `hovered` is egui's top-most hit test, so another control or the panel
        // over a backdrop still takes the release.
        && response.hovered()
        && response.contains_pointer()
}
/// How `response` was activated this pass. `suppress` retires a pointer release
/// that belongs to a press from before a panel replacement.
pub fn activation(response: &egui::Response, suppress: bool) -> Option<Activation> {
    if pointer_click(response) {
        (!suppress).then_some(Activation::Pointer)
    } else {
        keyboard_click(response).then_some(Activation::Enter)
    }
}
/// A key activation, or egui's click from the press this widget owns, keeping
/// egui's limits. For the screens with no press guard for a scrolled release.
pub fn owned_click(response: &egui::Response) -> bool {
    let owned = owns_press(response);
    (owned && response.clicked_by(egui::PointerButton::Primary) && response.contains_pointer())
        || keyboard_click(response)
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

/// A drop shadow: offset down, blur and spread in logical pixels, then peak
/// opacity. egui's linear feather is fitted to the bottom, side and top edge
/// profiles measured at DPR 1 from the rasters these replaced; their Gaussian
/// tails cannot match exactly.
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
/// The picker's search and address fields, as the login nickname's.
pub const FIELD: Drop = Drop {
    offset: 4.,
    blur: 16.,
    spread: 0.,
    alpha: 180,
};
/// The champion message: the old field raster's deep shadow, 165 at the bottom
/// edge, 127 at the sides and 90 at the top.
pub const DEEP_FIELD: Drop = Drop {
    offset: 6.,
    blur: 40.,
    spread: 0.,
    alpha: 254,
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

/// The shared keyboard-focus ring, one pixel inside the hit box so neighbours
/// and scroll clipping never cut it.
pub fn outline(ui: &egui::Ui, r: Rect, radius: f32) {
    theme::focus_ring(
        ui.painter(),
        r.shrink(1.),
        (radius - 1.).max(0.),
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
/// buttons take the hover/focus and pressed overlays, at the sampled
/// `fade` when the screen steps them.
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
    fade: Option<[f32; 2]>,
    tokens: MenuTheme,
    palette: Option<bool>,
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
    // The picker shares the pill palette without changing its font, bounds,
    // shadow or hit box. Studio/champion buttons retain their own surfaces.
    let palette = palette.map(|primary| {
        let state = if !ui.is_enabled() {
            theme::PillState::Idle
        } else if pressed {
            theme::PillState::Pressed
        } else if response.hovered() {
            theme::PillState::Hovered
        } else {
            theme::PillState::Idle
        };
        theme::pill_palette(primary, state)
    });
    theme::capsule(ui, r, palette.map_or(colors, |p| p.0), 1.);
    if palette.is_none() && !compact && ui.is_enabled() {
        let [hover, press] = fade.unwrap_or([
            if response.hovered() { 1. } else { 0. },
            if pressed { 1. } else { 0. },
        ]);
        for (state, alpha) in [
            (
                ControlState::Hovered,
                hover.max(if focused { 0.33 } else { 0. }),
            ),
            (ControlState::Pressed, press),
        ] {
            if alpha > 0. {
                let overlay = tokens.surface(control.style, state).colors.map(theme::ink);
                theme::capsule(ui, r, overlay.map(|c| c.gamma_multiply(alpha)), 1.);
            }
        }
    }
    let mut style = style;
    if let Some((_, color)) = palette {
        style.color = [color.r(), color.g(), color.b()];
    }
    label(
        &theme::text_painter(ui),
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
        outline(ui, r, radius);
    }
    response
}

/// A core text field: the shared field surface around a single-line editor
/// that scrolls long text inside its insets.
#[allow(clippy::too_many_arguments)]
pub fn text_field(
    ui: &mut egui::Ui,
    control: &Control,
    text: &mut String,
    placeholder: &str,
    inset: [f64; 4],
    r: Rect,
    scale: f32,
    style: TextStyle,
    id: Id,
    drop: Drop,
    keyboard_focus: bool,
    tokens: MenuTheme,
) -> egui::Response {
    let radius = (f32::from(tokens.surface(control.style, ControlState::Idle).radius) * scale)
        .min(r.height() / 2.);
    shadow(ui, r, radius, scale, drop);
    ui.painter()
        .rect_filled(r, radius, theme::ink(tokens.field));
    ui.painter().rect_stroke(
        r,
        radius,
        Stroke::new(2. * scale, Color32::BLACK),
        egui::StrokeKind::Inside,
    );
    // egui floors a TextEdit at interact height; the core field size wins.
    ui.spacing_mut().interact_size.y = 0.;
    let visuals = ui.visuals_mut();
    visuals.extreme_bg_color = theme::ink(tokens.field);
    visuals.text_cursor.stroke = Stroke::new(1., theme::ink(tokens.text));
    visuals.selection.bg_fill = theme::ink(tokens.selection).gamma_multiply(0.45);
    visuals.selection.stroke.color = theme::ink(tokens.text);
    let font = egui::FontId::new(style.size as f32, theme::family(style));
    let response = ui.put(
        r,
        egui::TextEdit::singleline(text)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                left: (inset[0] as f32 * scale).round() as i8,
                right: (inset[2] as f32 * scale).round() as i8,
                top: 0,
                bottom: 0,
            }))
            .id(id)
            .hint_text(
                // The placeholder shares the field font; egui would otherwise
                // size an empty field by its default body style.
                egui::RichText::new(placeholder)
                    .font(font.clone())
                    .color(theme::ink(tokens.placeholder)),
            )
            .font(font)
            .text_color(theme::ink(style.color))
            .align(egui::Align2::LEFT_CENTER)
            .min_size(r.size())
            .clip_text(true),
    );
    if response.has_focus() && keyboard_focus && ui.is_enabled() {
        outline(ui, r, radius);
    }
    response
}

/// Row columns crop to their own box; other labels crop only to the panel.
pub fn column(ui: &egui::Ui, value: &str, r: Rect, style: TextStyle, align: Align) {
    let painter = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    label(&painter, value, r, style, align, false);
}

/// A centred label on the canvas `middle` baseline: the middle of the em
/// square, not of egui's line box, sits on the rectangle's centre.
pub fn em_middle_label(painter: &egui::Painter, value: &str, r: Rect, style: TextStyle) {
    let color = theme::ink(style.color);
    let size = style.size as f32;
    let galley = painter.layout_no_wrap(
        value.into(),
        egui::FontId::new(size, theme::family(style)),
        color,
    );
    let Some(row) = galley.rows.first() else {
        return;
    };
    let Some(glyph) = row.row.glyphs.first() else {
        return;
    };
    let baseline = r.center().y + size * (glyph.font_ascent / glyph.font_height - 0.5);
    let top = baseline - row.pos.y - glyph.pos.y;
    painter.galley(
        pos2(r.center().x - galley.size().x / 2., top),
        galley,
        color,
    );
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
