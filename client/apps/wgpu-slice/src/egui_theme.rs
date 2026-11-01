//! Shared native egui style, installed fonts and authored menu widgets.
use crate::Result;
use egui::{Color32, Id, Rect, RichText, Sense, Stroke, pos2, vec2};
use slither_client::menu_theme::MenuTheme;

pub const PANEL_WIDTH: f32 = 560.;
pub const PANEL_PADDING: i8 = 24;
pub const PANEL_GUTTER: f32 = 24.;
pub const ROW_GAP: f32 = 8.;

pub fn ink(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

pub fn install(context: &egui::Context) -> Result<()> {
    let theme = MenuTheme::default();
    let mut fonts = egui::FontDefinitions::empty();
    for (name, role) in [
        ("link", slither_client::menu_theme::ControlStyle::MenuLink),
        ("body", slither_client::menu_theme::ControlStyle::HelpText),
        ("bold", slither_client::menu_theme::ControlStyle::Heading),
        (
            "login-body",
            slither_client::menu_theme::ControlStyle::Nickname,
        ),
        (
            "login-bold",
            slither_client::menu_theme::ControlStyle::LoginPlay,
        ),
    ] {
        let bytes = crate::text::TextCache::native_font_bytes(theme.text_style(role))?;
        fonts
            .font_data
            .insert(name.into(), egui::FontData::from_owned(bytes).into());
        fonts
            .families
            .insert(egui::FontFamily::Name(name.into()), vec![name.into()]);
    }
    fonts
        .families
        .insert(egui::FontFamily::Proportional, vec!["body".into()]);
    fonts
        .families
        .insert(egui::FontFamily::Monospace, vec!["body".into()]);
    context.set_fonts(fonts);
    context.options_mut(|options| options.zoom_with_keyboard = false);
    context.set_theme(egui::Theme::Dark);
    context.style_mut_of(egui::Theme::Dark, |style| {
        style.visuals = egui::Visuals::dark();
        style.visuals.override_text_color = Some(Color32::from_rgb(
            theme.text[0],
            theme.text[1],
            theme.text[2],
        ));
        style.spacing.item_spacing = vec2(12., 12.);
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(18.));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(16.));
        style.animation_time = 0.;
    });
    Ok(())
}
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(ink(MenuTheme::default().field))
        .corner_radius(16)
        .inner_margin(PANEL_PADDING)
        .shadow(egui::epaint::Shadow {
            offset: [0, 6],
            blur: 18,
            spread: 2,
            color: Color32::from_black_alpha(180),
        })
}
pub fn title(ui: &mut egui::Ui, label: &str) {
    ui.label(
        RichText::new(label)
            .size(24.)
            .family(egui::FontFamily::Name("bold".into())),
    );
}
pub fn help(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).text_style(egui::TextStyle::Small));
}
pub fn pill_button(
    ui: &mut egui::Ui,
    id: Id,
    label: &str,
    width: f32,
    primary: bool,
) -> egui::Response {
    pill(ui, id, label, None, width, primary)
}
pub fn checkbox_row(
    ui: &mut egui::Ui,
    id: Id,
    title: &str,
    label: &str,
    checked: bool,
    help_text: &str,
) -> egui::Response {
    ui.label(title);
    let response = pill(ui, id, label, Some(checked), ui.available_width(), false);
    help(ui, help_text);
    ui.add_space(ROW_GAP);
    response
}
fn pill(
    ui: &mut egui::Ui,
    id: Id,
    label: &str,
    checked: Option<bool>,
    width: f32,
    primary: bool,
) -> egui::Response {
    // Reserve the complete focus/shadow footprint inside scroll clipping.
    let (slot, _) = ui.allocate_exact_size(vec2(width, 52.), Sense::hover());
    let rect = Rect::from_center_size(slot.center(), vec2((slot.width() - 12.).max(1.), 40.));
    painted_pill(ui, id, label, checked, rect, primary, 20., true)
}

pub fn pill_at(
    ui: &mut egui::Ui,
    id: Id,
    label: &str,
    rect: Rect,
    primary: bool,
    font_size: f32,
    focus_visible: bool,
) -> egui::Response {
    painted_pill(ui, id, label, None, rect, primary, font_size, focus_visible)
}

#[allow(clippy::too_many_arguments)]
fn painted_pill(
    ui: &mut egui::Ui,
    id: Id,
    label: &str,
    checked: Option<bool>,
    rect: Rect,
    primary: bool,
    font_size: f32,
    focus_visible: bool,
) -> egui::Response {
    let response = ui.interact(rect, id, Sense::click());
    response.widget_info(|| {
        if let Some(checked) = checked {
            egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), checked, label)
        } else {
            egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label)
        }
    });
    let theme = MenuTheme::default();
    let tokens = theme.surface(
        if primary {
            slither_client::menu_theme::ControlStyle::PrimaryButton
        } else {
            slither_client::menu_theme::ControlStyle::SecondaryButton
        },
        slither_client::menu_theme::ControlState::Idle,
    );
    let shadow = egui::epaint::Shadow {
        offset: [0, 3],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(100),
    };
    ui.painter().add(shadow.as_shape(rect, 20));
    let colors = tokens
        .colors
        .map(|rgb| Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    capsule(
        ui,
        rect,
        colors,
        if response.is_pointer_button_down_on() {
            0.8
        } else if response.hovered() {
            1.1
        } else {
            1.
        },
    );
    if response.has_focus() && focus_visible {
        ui.painter().rect_stroke(
            rect.expand(3.),
            23.,
            Stroke::new(2., ink(theme.text)),
            egui::StrokeKind::Outside,
        );
    }
    let galley = ui.painter().layout_no_wrap(
        label.into(),
        egui::FontId::new(font_size, egui::FontFamily::Name("bold".into())),
        Color32::WHITE,
    );
    let extra = if checked.is_some() { 34. } else { 0. };
    let left = rect.center().x - (galley.size().x + extra) / 2.;
    if let Some(checked) = checked {
        let box_rect = Rect::from_center_size(pos2(left + 11., rect.center().y), vec2(22., 22.));
        let fill = if checked {
            theme.selection
        } else {
            theme.backdrop
        };
        ui.painter()
            .rect_filled(box_rect, 4., Color32::from_rgb(fill[0], fill[1], fill[2]));
        if checked {
            ui.painter().add(egui::Shape::line(
                vec![
                    box_rect.min + vec2(5., 11.),
                    box_rect.min + vec2(9., 15.),
                    box_rect.min + vec2(17., 6.),
                ],
                Stroke::new(3., Color32::WHITE),
            ));
        }
    }
    ui.painter().galley(
        pos2(left + extra, rect.center().y - galley.size().y / 2.),
        galley,
        Color32::WHITE,
    );
    response
}

pub fn link_icon(ui: &egui::Ui, rect: Rect, id: slither_client::menu_controls::ControlId) {
    use slither_client::login_controls::{CHANGE_SKIN, SERVER};
    let center = pos2(rect.center().x, rect.top() + 46.);
    let tokens = MenuTheme::default();
    if id == CHANGE_SKIN {
        capsule(
            ui,
            Rect::from_center_size(center, vec2(48., 25.)),
            tokens
                .surface(
                    slither_client::menu_theme::ControlStyle::PrimaryButton,
                    slither_client::menu_theme::ControlState::Idle,
                )
                .colors
                .map(ink),
            1.,
        );
    } else {
        let icon = Rect::from_min_size(
            pos2(rect.center().x - 24., rect.top() + 37.),
            vec2(48., 22.),
        );
        for (row, knob) in [(0, 12.), (1, 32.), (2, 22.)] {
            let y = icon.top() + row as f32 * 8.;
            if id == SERVER {
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(icon.left(), y), vec2(48., 6.)),
                    0.,
                    ink(tokens.field),
                );
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(icon.left() + 4., y + 2.), vec2(2., 2.)),
                    0.,
                    ink(tokens.selection),
                );
            } else {
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(icon.left(), y), vec2(48., 2.)),
                    0.,
                    ink(tokens.field),
                );
                ui.painter().rect_filled(
                    Rect::from_min_size(pos2(icon.left() + knob, y - 2.), vec2(5., 6.)),
                    0.,
                    ink(tokens.selection),
                );
            }
        }
    }
}

// A capsule mesh carries the existing theme's two colour stops; its outer ring
// feathers one physical pixel, avoiding a raster atlas or clipped image shadow.
fn capsule(ui: &egui::Ui, rect: Rect, colors: [Color32; 2], brightness: f32) {
    let mut mesh = egui::epaint::Mesh::default();
    let color = |y: f32| {
        let t = ((y - rect.top()) / rect.height()).clamp(0., 1.);
        let a = colors[0].to_array();
        let b = colors[1].to_array();
        Color32::from_rgb(
            ((a[0] as f32 * (1. - t) + b[0] as f32 * t) * brightness).min(255.) as u8,
            ((a[1] as f32 * (1. - t) + b[1] as f32 * t) * brightness).min(255.) as u8,
            ((a[2] as f32 * (1. - t) + b[2] as f32 * t) * brightness).min(255.) as u8,
        )
    };
    let vertex = |pos, color| egui::epaint::Vertex {
        pos,
        uv: egui::epaint::WHITE_UV,
        color,
    };
    mesh.vertices
        .push(vertex(rect.center(), color(rect.center().y)));
    let radius = (rect.height() / 2.).min(rect.width() / 2.);
    let feather = 0.5 / ui.pixels_per_point();
    for (center, angle) in [
        (
            pos2(rect.right() - radius, rect.top() + radius),
            -std::f32::consts::FRAC_PI_2,
        ),
        (pos2(rect.right() - radius, rect.bottom() - radius), 0.),
        (
            pos2(rect.left() + radius, rect.bottom() - radius),
            std::f32::consts::FRAC_PI_2,
        ),
        (
            pos2(rect.left() + radius, rect.top() + radius),
            std::f32::consts::PI,
        ),
    ] {
        for i in 0..=12 {
            let angle = angle + i as f32 / 12. * std::f32::consts::FRAC_PI_2;
            let direction = vec2(angle.cos(), angle.sin());
            let inner = center + direction * (radius - feather);
            mesh.vertices.push(vertex(inner, color(inner.y)));
            mesh.vertices.push(vertex(
                center + direction * (radius + feather),
                Color32::TRANSPARENT,
            ));
        }
    }
    let count = (mesh.vertices.len() as u32 - 1) / 2;
    for i in 0..count {
        let a = 1 + i * 2;
        let b = 1 + ((i + 1) % count) * 2;
        mesh.indices.extend([0, a, b, a, b, b + 1, a, b + 1, a + 1]);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}
