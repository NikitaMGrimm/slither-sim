//! Native menu text contrast, including the actual picker and disabled paint.
// `egui_theme` is compiled only for the native desktop host (see `lib.rs`).
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use egui::Color32;
use slither_wgpu_slice::egui_theme::{PillState, pill_palette};
use slither_wgpu_slice::{egui_theme as theme, login_native::NativeLogin, server_picker_egui};

fn over(fg: Color32, bg: Color32) -> Color32 {
    let a = f32::from(fg.a()) / 255.;
    let rgb = [fg.r(), fg.g(), fg.b()];
    let back = [bg.r(), bg.g(), bg.b()];
    let c = std::array::from_fn::<_, 3, _>(|i| {
        (f32::from(rgb[i]) + f32::from(back[i]) * (1. - a)).round() as u8
    });
    Color32::from_rgb(c[0], c[1], c[2])
}

fn visit(shape: &egui::Shape, f: &mut impl FnMut(&egui::Shape)) {
    if let egui::Shape::Vec(shapes) = shape {
        for shape in shapes {
            visit(shape, f);
        }
    } else {
        f(shape);
    }
}

/// Inspect production egui shapes, without a window, GPU, art or network worker.
#[test]
fn picker_buttons_keep_contrast_in_every_state() {
    use slither_client::menu_controls::ControlKind;
    let mut login = NativeLogin::new("Player").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    // Authored loopback entry exposes Remove without any socket or persistence.
    login
        .picker
        .policy
        .add_custom(format!("{}:1", std::net::Ipv4Addr::LOCALHOST))
        .unwrap();
    login.open_picker();
    login.picker.policy.alpha = 1.;
    login.prepare([960, 720], true, None).unwrap();
    let panel = login.picker.panel.clone();
    let mut automatic_panel = panel.clone();
    automatic_panel
        .controls
        .iter_mut()
        .find(|c| c.id == slither_client::server_picker::AUTOMATIC)
        .unwrap()
        .style = slither_client::menu_theme::ControlStyle::StudioSave;
    let mut failures = Vec::new();
    for panel in [panel, automatic_panel] {
        for control in &panel.controls {
            let ControlKind::Button { label } = &control.kind else {
                continue;
            };
            for state in ["idle", "hovered", "pressed", "focused", "disabled"] {
                let ctx = egui::Context::default();
                theme::install(&ctx).unwrap();
                let mut ui = server_picker_egui::PickerUi::default();
                let mut panel = panel.clone();
                if state == "disabled" {
                    panel
                        .controls
                        .iter_mut()
                        .find(|c| c.id == control.id)
                        .unwrap()
                        .enabled = false;
                }
                let bounds = slither_wgpu_slice::egui_panel::rect(
                    panel
                        .resolve([960., 720.])
                        .unwrap()
                        .iter()
                        .find(|r| r.control.id == control.id)
                        .unwrap()
                        .rect,
                );
                let mut output = None;
                for pass in 0..5 {
                    if state == "focused" {
                        ui.keyboard_focus = true;
                        ui.request_focus(control.id);
                    }
                    let mut events = Vec::new();
                    if matches!(state, "hovered" | "pressed") {
                        events.push(egui::Event::PointerMoved(bounds.center()));
                    }
                    if state == "pressed" && pass == 3 {
                        events.push(egui::Event::PointerButton {
                            pos: bounds.center(),
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            modifiers: Default::default(),
                        });
                    }
                    let mut frame = ctx.run_pass(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(960., 720.),
                            )),
                            events,
                            ..Default::default()
                        },
                        |ctx| {
                            ui.show(ctx, &panel, [960., 720.], true).unwrap();
                        },
                    );
                    frame.textures_delta.clear();
                    output = Some(frame);
                }
                let mut fills = Vec::new();
                let mut text = None;
                for shape in &output.unwrap().shapes {
                    visit(&shape.shape, &mut |shape| match shape {
                        egui::Shape::Mesh(mesh)
                            if bounds.contains(mesh.calc_bounds().center())
                                && (mesh.calc_bounds().width() - bounds.width()).abs() < 2. =>
                        {
                            let alpha = mesh.vertices.iter().map(|v| v.color.a()).max().unwrap();
                            fills.extend(
                                mesh.vertices
                                    .iter()
                                    .filter(|v| v.color.a() == alpha)
                                    .map(|v| v.color),
                            );
                        }
                        egui::Shape::Text(t)
                            if t.galley.job.text == *label && bounds.contains(t.pos) =>
                        {
                            text = Some(text_color(t));
                        }
                        _ => {}
                    });
                }
                assert!(!fills.is_empty(), "missing fill for {label} {state}");
                let expected_state = match state {
                    "hovered" => PillState::Hovered,
                    "pressed" => PillState::Pressed,
                    _ => PillState::Idle,
                };
                let primary = control.style == slither_client::menu_theme::ControlStyle::StudioSave;
                assert_eq!(
                    text.unwrap(),
                    pill_palette(primary, expected_state).1,
                    "{label} {state} must exercise its palette"
                );
                let background = theme::ink(slither_client::menu_theme::MenuTheme::default().field);
                let ratio = fills
                    .into_iter()
                    .map(|fill| {
                        let fill = over(fill, background);
                        contrast(over(text.expect("painted label"), fill), fill)
                    })
                    .fold(f64::INFINITY, f64::min);
                println!("picker {label} {state}: {ratio:.2}:1");
                if ratio < 4.5 {
                    failures.push(format!("{label} {state}: {ratio:.2}:1"));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn disabled_core_panel_buttons_keep_aa_text_and_surface_fade() {
    use slither_client::menu_theme::{ControlStyle, MenuTheme};
    let mut login = NativeLogin::new("Player").unwrap();
    login.open_picker();
    login.picker.policy.alpha = 1.;
    login.prepare([960, 720], true, None).unwrap();
    let template = login
        .picker
        .panel
        .controls
        .iter()
        .find(|c| {
            matches!(
                c.kind,
                slither_client::menu_controls::ControlKind::Button { .. }
            )
        })
        .unwrap();
    for style in [
        ControlStyle::PrimaryButton,
        ControlStyle::SecondaryButton,
        ControlStyle::StudioAction,
        ControlStyle::StudioSave,
        ControlStyle::StudioCancel,
    ] {
        let ctx = egui::Context::default();
        theme::install(&ctx).unwrap();
        let tokens = MenuTheme::default();
        let mut control = template.clone();
        control.style = style;
        control.enabled = false;
        let mut frame = ctx.run_ui(Default::default(), |ui| {
            ui.visuals_mut().disabled_alpha = tokens.disabled_opacity(style) as f32;
            ui.disable();
            slither_wgpu_slice::egui_panel::button(
                ui,
                &control,
                "Test",
                egui::Rect::from_min_size(egui::pos2(20., 20.), egui::vec2(240., 42.)),
                1.,
                tokens.text_style(style),
                egui::Id::unique("test"),
                false,
                None,
                tokens,
                None,
            );
        });
        frame.textures_delta.clear();
        let mut text = None;
        let mut fills = Vec::new();
        for shape in &frame.shapes {
            visit(&shape.shape, &mut |shape| match shape {
                egui::Shape::Text(t) if t.galley.job.text == "Test" => text = Some(text_color(t)),
                egui::Shape::Mesh(mesh) => {
                    let alpha = mesh.vertices.iter().map(|v| v.color.a()).max().unwrap();
                    fills.extend(
                        mesh.vertices
                            .iter()
                            .filter(|v| v.color.a() == alpha)
                            .map(|v| v.color),
                    );
                }
                _ => {}
            });
        }
        assert!(!fills.is_empty());
        for fill in fills {
            assert_eq!(
                fill.a(),
                Color32::WHITE
                    .gamma_multiply(tokens.disabled_opacity(style) as f32)
                    .a(),
                "surface still fades"
            );
            let fill = over(fill, theme::ink(tokens.field));
            let ratio = contrast(over(text.unwrap(), fill), fill);
            assert!(ratio >= 4.5, "{style:?}: {ratio:.2}:1");
        }
    }
}

fn text_color(t: &egui::epaint::TextShape) -> Color32 {
    t.override_text_color
        .unwrap_or(t.galley.rows[0].row.visuals.mesh.vertices[0].color)
        .gamma_multiply(t.opacity_factor)
}

#[test]
fn disabled_text_retains_the_parent_panel_fade() {
    let ctx = egui::Context::default();
    theme::install(&ctx).unwrap();
    let mut frame = ctx.run_ui(Default::default(), |ui| {
        ui.set_opacity(0.6);
        ui.visuals_mut().disabled_alpha = 0.38;
        ui.disable();
        assert!((theme::text_painter(ui).opacity() - 0.6).abs() < 0.0001);
        assert!((ui.painter().opacity() - 0.6 * 0.38).abs() < 0.0001);
    });
    frame.textures_delta.clear();
}

#[test]
fn disabled_shared_buttons_and_checkboxes_keep_aa_text() {
    for primary in [false, true] {
        for checkbox in [false, true] {
            for checked in [false, true] {
                let ctx = egui::Context::default();
                theme::install(&ctx).unwrap();
                let mut output = None;
                for _ in 0..2 {
                    let mut frame = ctx.run_ui(Default::default(), |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            ui.disable();
                            if checkbox {
                                theme::checkbox_row(
                                    ui,
                                    egui::Id::unique("test"),
                                    "",
                                    "Test",
                                    checked,
                                    "",
                                );
                            } else {
                                theme::pill_button(
                                    ui,
                                    egui::Id::unique("test"),
                                    "Test",
                                    240.,
                                    primary,
                                );
                            }
                        });
                    });
                    frame.textures_delta.clear();
                    output = Some(frame);
                }
                let mut text = None;
                let mut fills = Vec::new();
                for shape in &output.unwrap().shapes {
                    visit(&shape.shape, &mut |shape| match shape {
                        egui::Shape::Text(t) if t.galley.job.text == "Test" => {
                            text = Some(text_color(t));
                        }
                        egui::Shape::Mesh(mesh) => {
                            // The single pill's interior excludes its transparent feather.
                            let alpha = mesh.vertices.iter().map(|v| v.color.a()).max().unwrap();
                            fills.extend(
                                mesh.vertices
                                    .iter()
                                    .filter(|v| v.color.a() == alpha)
                                    .map(|v| v.color),
                            );
                        }
                        _ => {}
                    });
                }
                assert!(!fills.is_empty(), "missing disabled pill fill");
                let disabled_alpha = Color32::WHITE
                    .gamma_multiply(ctx.style_of(egui::Theme::Dark).visuals.disabled_alpha)
                    .a();
                assert!(disabled_alpha > 0 && disabled_alpha < 255);
                for fill in &fills {
                    assert_eq!(fill.a(), disabled_alpha, "surface still fades");
                }
                let primary = primary && !checkbox;
                for bg in [
                    slither_client::menu_theme::MenuTheme::default().field,
                    slither_client::menu_theme::MenuTheme::default().backdrop,
                ] {
                    for fill in &fills {
                        let fill = over(*fill, theme::ink(bg));
                        let ratio = contrast(over(text.unwrap(), fill), fill);
                        println!(
                            "disabled primary {primary} checkbox {checkbox} checked {checked}: {ratio:.2}:1"
                        );
                        assert!(ratio >= 4.5, "disabled text {ratio:.2}:1");
                    }
                }
            }
        }
    }
}

/// WCAG 2.2 relative luminance of an opaque sRGB colour.
fn luminance(color: Color32) -> f64 {
    let [r, g, b, a] = color.to_array();
    assert_eq!(a, 255, "pill fills and labels are opaque");
    let channel = |c: u8| {
        let c = f64::from(c) / 255.;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}
fn contrast(a: Color32, b: Color32) -> f64 {
    let [light, dark] = {
        let (a, b) = (luminance(a), luminance(b));
        [a.max(b), a.min(b)]
    };
    (light + 0.05) / (dark + 0.05)
}

#[test]
fn pill_labels_keep_aa_contrast_against_every_fill_stop() {
    // Focus adds only an outside ring and checked state only the box, so
    // normal, focused and checked labels share the idle fill. Disabled paint
    // is checked separately after compositing against the menu backgrounds.
    for primary in [true, false] {
        for state in [PillState::Idle, PillState::Hovered, PillState::Pressed] {
            let (fill, label) = pill_palette(primary, state);
            // Every channel moves the same way between the stops, so the
            // gradient's luminance is extreme at a stop.
            for stop in fill {
                let ratio = contrast(label, stop);
                assert!(
                    ratio >= 4.5,
                    "primary {primary} {state:?}: {ratio:.2}:1 against {stop:?}"
                );
            }
        }
    }
    // The checked box's white tick sits on the idle primary green.
    let ratio = contrast(Color32::WHITE, pill_palette(true, PillState::Idle).0[0]);
    assert!(ratio >= 4.5, "tick {ratio:.2}:1");
}
