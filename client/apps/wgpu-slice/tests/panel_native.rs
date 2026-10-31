#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
//! Authored pointer/capture regressions, without a window or clipboard.
use slither_client::{
    champion::MESSAGE,
    menu_input::{EditIntent, EditorId},
    session_policy::{Command, SessionPolicy},
};
use slither_wgpu_slice::{panel_native::NativePanel, text::TextCache};
fn panel() -> NativePanel {
    let mut s = SessionPolicy::new();
    s.prepare_controls(
        &[
            Command {
                opcode: b'a',
                argument: 0,
            },
            Command {
                opcode: b'v',
                argument: 1,
            },
        ],
        0.,
    )
    .unwrap();
    s.commit_controls().unwrap();
    let mut p = NativePanel::new(&s.view());
    p.viewport = [960., 720.];
    p.policy.observe(1, &s.view());
    p.step(0., 1., &s.view()).unwrap();
    p.input.state.apply(EditIntent::KeyText("abcdef".into()));
    p
}
#[test]
fn outside_press_never_starts_a_selection_drag() {
    let mut p = panel();
    let mut text = TextCache::menu();
    let before = p.input.state.editor(EditorId::Champion).selection();
    p.pointer(&mut text, [0., 0.], Some(true)).unwrap();
    assert_eq!(p.controls.capture(), None);
    p.pointer(&mut text, [100., 100.], None).unwrap();
    assert_eq!(p.input.state.editor(EditorId::Champion).selection(), before);
}
#[test]
fn preedit_cannot_swallow_release_or_activate_a_control() {
    let mut p = panel();
    let mut text = TextCache::menu();
    let r = p.panel.resolve(p.viewport).unwrap()[2].rect;
    let point = [r.x + 12., r.y + r.height / 2.];
    p.pointer(&mut text, point, Some(true)).unwrap();
    assert_eq!(p.controls.capture(), Some(MESSAGE));
    p.input.state.apply(EditIntent::Preedit("x".into(), None));
    let before = p.input.state.editor(EditorId::Champion).selection();
    p.pointer(&mut text, [point[0] + 50., point[1]], Some(false))
        .unwrap();
    assert_eq!(p.controls.capture(), None);
    assert_eq!(p.input.state.editor(EditorId::Champion).selection(), before);
    assert!(p.take_submission().is_none());
    p.input.state.apply(EditIntent::ImeCommit("z".into()));
    let committed = p.input.state.editor(EditorId::Champion).selection();
    p.pointer(&mut text, [point[0] + 100., point[1]], None)
        .unwrap();
    assert_eq!(
        p.input.state.editor(EditorId::Champion).selection(),
        committed
    );
}

#[path = "support/announcement_tape.rs"]
mod tape;
fn victory_at(
    end: u32,
) -> (
    slither_wgpu_slice::game::LocalGame,
    slither_wgpu_slice::login_native::NativeLogin,
) {
    use slither_wgpu_slice::login_native::NativeLogin;
    let mut game = tape::playing();
    let mut login = NativeLogin::new("Player").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(8., true, false).unwrap();
    tape::pump(&mut game, 700.);
    login.observe_game(&game).unwrap();
    login.step(700., false, true).unwrap();
    game.receive(game.generation(), &[b'v', 1], 800.).unwrap();
    for now in (808..=end).step_by(40) {
        game.receive(game.generation(), b"p", now as f64).unwrap();
        tape::pump(&mut game, now as f64);
        login.observe_game(&game).unwrap();
        login.step(now as f64, false, false).unwrap();
    }
    (game, login)
}

#[test]
fn returned_champion_empty_full_and_selected_compose_at_high_dpr() {
    let (_, mut login) = victory_at(6008);
    assert_eq!(login.champion.panel.alpha, 1.);
    assert!(!login.state.view(false).last_life.label.is_empty());
    for dpr in [4., 5.] {
        for n in [0, 140, 50] {
            login
                .champion
                .input
                .state
                .reset_editor(EditorId::Champion, &"W".repeat(n));
            if n == 50 {
                login.champion.input.state.apply(EditIntent::SelectAll);
            }
            let frame = login
                .prepare_host([(960. * dpr) as u32, (720. * dpr) as u32], dpr, false, None)
                .unwrap();
            let bytes: usize = frame.resources.iter().map(|r| r.image.pixels.len()).sum();
            assert!(bytes <= slither_wgpu_slice::text::MENU_CACHE_BYTES);
            assert!(frame.frame.draws.len() <= 512);
            println!("returned DPR {dpr}, {n} units: {bytes} bytes");
        }
    }
}

#[test]
fn terminal_transport_before_ack_ends_editor_without_resend_or_early_login() {
    use slither_client::{menu_controls::Activation, start_page::MenuPhase};
    for end in [808, 6008] {
        for failure in 0..3 {
            let (mut game, mut login) = victory_at(end);
            let generation = game.generation();
            let last = login.state.view(false).last_life.clone();
            assert!(login.champion.policy.visible());
            assert!(
                game.save_champion(generation, &[65], Activation::Pointer)
                    .unwrap()
            );
            let pending = game.effect().unwrap();
            match failure {
                0 => game.closed(generation),
                1 => game.fail("Synthetic socket error"),
                _ => game.acknowledge(generation, pending.id, false).unwrap(),
            }
            assert!(tape::effects(&mut game).is_empty());
            login.observe_game(&game).unwrap();
            assert!(!login.champion.panel.enabled);
            if end == 808 {
                assert!(game.login_session().death_pending);
                assert!(!login.state.view(true).visible);
            }
            assert!(login.needs_redraw());
            assert!(
                !game
                    .save_champion(generation, &[66], Activation::Pointer)
                    .unwrap()
            );
            for now in (end + 40..=end + 4800).step_by(40) {
                assert!(tape::pump(&mut game, f64::from(now)).is_empty());
                login.observe_game(&game).unwrap();
                login.step(f64::from(now), game.can_play(), false).unwrap();
                let session = game.login_session();
                if session.death_pending {
                    assert!(login.needs_redraw());
                }
                if session.death_pending || session.return_fraction > 0. {
                    assert_ne!(login.state.view(true).phase, MenuPhase::Login);
                    assert_ne!(login.input.state.focus(), Some(EditorId::Nickname));
                }
            }
            assert!(login.champion.policy.finished());
            assert!(!login.champion.policy.visible());
            assert_eq!(login.champion.policy.ordinary_alpha(), 1.);
            assert!(!game.champion_session().champion_acknowledged);
            assert_eq!(game.generation(), generation);
            assert!(game.can_play());
            let view = login.state.view(game.can_play());
            assert_eq!(view.phase, MenuPhase::Login);
            assert!(!view.champion_pending && view.ordinary_controls_visible && !view.disabled);
            assert_eq!(view.last_life, last);
            login.sync_return_focus(game.can_play());
            assert_eq!(login.input.state.focus(), Some(EditorId::Nickname));
            assert!(login.champion.take_submission().is_none());
            login
                .prepare_host([960, 720], 1., game.can_play(), None)
                .unwrap();
        }
    }
}

#[test]
fn long_os_preedit_is_bounded_before_geometry_and_rasterization() {
    use slither_client::{menu_input::MAX_PREEDIT_UNITS, menu_theme::MenuTheme};
    use slither_wgpu_slice::{editor_geometry::EditorGeometry, menu::Menu};
    use winit::event::{Ime, WindowEvent};
    let mut p = panel();
    p.input.window_event(&WindowEvent::Ime(Ime::Preedit(
        "界".repeat(100_000),
        Some((usize::MAX, usize::MAX)),
    )));
    let editor = p.input.state.editor(EditorId::Champion);
    let preview = editor.preedit().unwrap();
    assert_eq!(preview.text.encode_utf16().count(), MAX_PREEDIT_UNITS);
    assert!(preview.text.capacity() <= 3 * MAX_PREEDIT_UNITS);
    let field = p.panel.resolve(p.viewport).unwrap()[2].rect;
    let mut text = TextCache::menu();
    let geometry = EditorGeometry::new(
        &mut text,
        editor,
        field,
        [0.; 2],
        MenuTheme::default().text_style(slither_client::menu_theme::ControlStyle::Field),
        0.,
    )
    .unwrap();
    assert!(geometry.display.len() <= 3 * MAX_PREEDIT_UNITS + 140);
    assert!(geometry.caret.is_finite() && geometry.scroll.is_finite());
    for dpr in [1., 5.] {
        let frame = Menu::default()
            .prepare_panel(
                &mut text,
                &p.panel,
                &p.controls,
                &p.input.state,
                &mut p.scroll,
                MenuTheme::default(),
                [(960. * dpr) as u32, (720. * dpr) as u32],
                dpr,
            )
            .unwrap();
        assert!(
            frame
                .resources
                .iter()
                .map(|r| r.image.pixels.len())
                .sum::<usize>()
                <= 4 * 1024 * 1024
        );
    }
    assert!(p.take_submission().is_none());
}

#[test]
fn champion_field_uses_shared_placement_for_value_placeholder_selection_and_ime() {
    use slither_client::{
        menu_controls::ControlKind, menu_input::HostEditEffect,
        menu_layout::single_line_field_placement, menu_theme::MenuTheme,
    };
    use slither_wgpu_slice::{
        menu::Menu,
        text::{Align, Baseline, TextRun},
    };
    for dpr in [1., 1.25, 1.5, 1.75, 2.] {
        for case in 0..4 {
            let mut p = panel();
            let ControlKind::TextField { placeholder, .. } = &mut p.panel.controls[2].kind else {
                unreachable!()
            };
            *placeholder = "Champion message".into();
            let value = match case {
                0 => String::new(),
                1 => "Hgyp".into(),
                2 => "W".repeat(140),
                _ => "abcdef".into(),
            };
            p.input.state.reset_editor(EditorId::Champion, &value);
            if case == 1 {
                p.input.state.apply(EditIntent::SelectAll);
            }
            if case == 3 {
                p.input
                    .state
                    .apply(EditIntent::Preedit("界g".into(), Some((0, 4))));
            }
            let mut text = TextCache::menu();
            let resolved = p.panel.resolve(p.viewport).unwrap();
            let field = &resolved[2];
            let ControlKind::TextField {
                inset, placeholder, ..
            } = &field.control.kind
            else {
                unreachable!()
            };
            let mut style = MenuTheme::default().text_style(field.control.style);
            style.size *= field.scale;
            let metrics = text.measure_panel("", style).unwrap();
            let rule = single_line_field_placement(
                field.rect,
                inset.map(|v| v * field.scale),
                style.size,
                metrics.font_ascent,
                metrics.font_descent,
            );
            let g = p.geometry(&mut text).unwrap().unwrap();
            let center = rule.caret_top + rule.caret_height / 2.;
            let logical_center =
                g.text_origin()[1] - (metrics.font_ascent - metrics.font_descent) / 2.;
            assert!((logical_center - center).abs() < 1e-9);
            assert!((g.text_origin()[0] + g.scroll - rule.origin[0]).abs() < 1e-9);
            assert_eq!(
                (g.cursor_rect().y, g.cursor_rect().height),
                (rule.caret_top, rule.caret_height)
            );
            assert_eq!(
                (g.selection_rect().y, g.selection_rect().height),
                (rule.caret_top, rule.caret_height)
            );
            assert_eq!(g.rect.x, rule.origin[0]);
            assert!((g.rect.width - 420. * field.scale).abs() < 1e-9);
            if let Some(underline) = g.preedit_rect() {
                assert!(
                    (underline.y + underline.height - rule.caret_top - rule.caret_height).abs()
                        < 1e-9
                );
            }
            let mut display_style = style;
            let display = if g.display.is_empty() {
                display_style.color = MenuTheme::default().placeholder;
                placeholder.clone()
            } else {
                g.display.clone()
            };
            let reference = TextCache::menu()
                .draw(
                    TextRun {
                        text: &display,
                        style: display_style,
                        position: [0.; 2],
                        align: Align::Left,
                        baseline: Baseline::Alphabetic,
                        alpha: 1.,
                    },
                    dpr,
                )
                .unwrap();
            let glyph_top = reference[0].0.destination.y;
            let frame = Menu::default()
                .prepare_panel(
                    &mut text,
                    &p.panel,
                    &p.controls,
                    &p.input.state,
                    &mut p.scroll,
                    MenuTheme::default(),
                    [(960. * dpr) as u32, (720. * dpr) as u32],
                    dpr,
                )
                .unwrap();
            let text_draws: Vec<_> = frame
                .frame
                .draws
                .iter()
                .filter(|d| {
                    d.asset.handle >= slither_wgpu_slice::bank::MENU_TEXT_HANDLE_BASE
                        && d.destination.y >= g.rect.y
                        && d.destination.y < g.rect.y + g.rect.height
                })
                .collect();
            let text_top = text_draws
                .iter()
                .map(|d| d.destination.y)
                .reduce(f64::min)
                .unwrap();
            let text_left = text_draws
                .iter()
                .map(|d| d.destination.x)
                .reduce(f64::min)
                .unwrap();
            assert!((text_top * dpr - (text_top * dpr).round()).abs() < 1e-7);
            let painted_baseline = text_top - glyph_top;
            let center_delta =
                painted_baseline - (metrics.font_ascent - metrics.font_descent) / 2. - center;
            assert!(center_delta.abs() <= 0.5 / dpr + 1e-6);
            let caret = frame
                .frame
                .draws
                .iter()
                .find(|d| {
                    d.asset.width <= (dpr as u32 + 1)
                        && d.asset.height >= (10. * dpr) as u32
                        && frame
                            .resources
                            .iter()
                            .find(|r| r.asset == d.asset)
                            .unwrap()
                            .image
                            .pixels
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .all(|p| *p == [224, 224, 255, 255])
                })
                .unwrap();
            let expected_caret = g.snapped_cursor_rect(dpr);
            assert_eq!(
                (
                    caret.destination.x,
                    caret.destination.y,
                    caret.destination.width,
                    caret.destination.height
                ),
                (
                    expected_caret.x,
                    expected_caret.y,
                    expected_caret.width,
                    expected_caret.height
                )
            );
            assert!(p.cursor_area(&mut text, dpr).unwrap().iter().any(|e| matches!(e,
                HostEditEffect::ImeCursorArea { position, size } if *position == [caret.destination.x, caret.destination.y] && *size == [caret.destination.width, caret.destination.height])));
            if case < 2 {
                assert!((text_left - rule.origin[0]).abs() <= 0.5 / dpr + 1e-6);
                println!(
                    "champion dpr={dpr} case={case} logical_center_delta={} logical_left_delta={} painted_center_delta={center_delta} painted_left_delta={} baseline={painted_baseline} rule_y={} glyph_top={glyph_top} text_top={text_top}",
                    logical_center - center,
                    g.text_origin()[0] + g.scroll - rule.origin[0],
                    text_left - rule.origin[0],
                    rule.origin[1]
                );
            }
        }
    }
}
