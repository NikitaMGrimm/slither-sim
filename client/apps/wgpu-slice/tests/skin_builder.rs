//! Authored, offscreen builder controls and actual queued login effects. No sockets/windows.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "support/announcement_tape.rs"]
mod support;
use slither_client::{menu_controls::*, skin_builder::*, skin_studio::*};
use slither_wgpu_slice::{
    game::{LocalGame, LoginProfile},
    login_native::NativeLogin,
    menu_input_native::KeyInput,
};
use winit::{
    event::ElementState,
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
};

fn key(login: &mut NativeLogin, named: NamedKey) {
    let logical = Key::Named(named);
    assert!(
        login
            .key(
                KeyInput {
                    logical: &logical,
                    physical: PhysicalKey::Code(KeyCode::Enter),
                    text: None,
                    state: ElementState::Pressed,
                    repeat: false
                },
                false
            )
            .consumed
    );
}
fn login() -> NativeLogin {
    let mut login = NativeLogin::new("Bytes").unwrap();
    login.step(0., false, false).unwrap();
    login.prepare([960, 540], false, None).unwrap();
    login.open_studio();
    login.step(230., false, false).unwrap();
    key(&mut login, NamedKey::ArrowRight);
    login.step(410., false, false).unwrap();
    login
}

#[test]
fn action_labels_keep_theme_padding_at_nominal_size_in_every_layout() {
    use slither_client::menu_theme::{MenuTheme, STUDIO_ACTION_PADDING_X};
    use slither_wgpu_slice::text::{Align, Baseline, TextCache, TextRun};
    let mut login = login();
    login.studio.state.edit_builder(|b| {
        b.append(1);
    });
    let mut text = TextCache::menu();
    for dpr in [1., 1.25, 1.5, 1.75, 2., 3., 4., 5.] {
        for logical in [
            [960., 540.],
            [360., 640.],
            [960., 360.],
            [280., 320.],
            [240., 320.],
        ] {
            for id in [ADD, REMOVE, RECOLOUR, RESET] {
                login.studio.state.reveal_control(id, logical);
                let panel = login.studio.state.panel(logical, "");
                let resolved = panel.resolve(logical).unwrap();
                let action = resolved.iter().find(|r| r.control.id == id).unwrap();
                let ControlKind::Button { label } = &action.control.kind else {
                    panic!("action button");
                };
                let style = MenuTheme::default().text_style(action.control.style);
                // No max width or fitting: scan the complete native raster's nonzero ink.
                let raster = text
                    .draw(
                        TextRun {
                            text: label,
                            style,
                            position: [0., 0.],
                            align: Align::Left,
                            baseline: Baseline::Alphabetic,
                            alpha: 1.,
                        },
                        dpr,
                    )
                    .unwrap();
                assert_eq!(raster.len(), 1);
                let image = &raster[0].1.image;
                let columns: Vec<_> = image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p[3] > 0)
                    .map(|(index, _)| index % image.width as usize)
                    .collect();
                let ink = (columns.iter().max().unwrap() - columns.iter().min().unwrap() + 1)
                    as f64
                    / dpr;
                assert!(
                    ink <= action.rect.width - 2. * STUDIO_ACTION_PADDING_X,
                    "DPR {dpr}, {logical:?}, {label}: {ink} ink in {} pill, padding {}",
                    action.rect.width,
                    2. * STUDIO_ACTION_PADDING_X
                );
                assert_eq!(
                    intersection(action.rect, action.clip),
                    action.rect,
                    "focused action must be fully visible"
                );
                for other in [ControlId(4), CANCEL, SAVE] {
                    let bounds = resolved
                        .iter()
                        .find(|r| r.control.id == other)
                        .unwrap()
                        .rect;
                    let overlap = intersection(action.rect, bounds);
                    assert!(
                        overlap.width == 0. || overlap.height == 0.,
                        "action overlaps status/footer"
                    );
                }
            }
        }
    }
}
#[test]
fn tabs_grid_and_windowed_strip_are_roving_and_do_not_save_or_connect() {
    let mut login = login();
    assert_eq!(login.focused(), Some(BUILDER_TAB));
    assert_eq!(login.studio.state.section, StudioSection::Builder);
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(ControlId(SWATCH_BASE)));
    key(&mut login, NamedKey::End);
    assert_eq!(login.focused(), Some(ControlId(SWATCH_BASE + 41)));
    key(&mut login, NamedKey::Home);
    key(&mut login, NamedKey::ArrowDown);
    key(&mut login, NamedKey::ArrowRight);
    key(&mut login, NamedKey::Enter);
    assert_eq!(
        login
            .studio
            .state
            .draft()
            .unwrap()
            .builder
            .pattern()
            .colours(),
        0
    );
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(ADD));
    key(&mut login, NamedKey::Enter);
    assert_eq!(
        login.studio.state.draft().unwrap().builder.pattern().runs(),
        &[PatternRun {
            count: 1,
            palette: 7
        }]
    );
    assert_eq!(
        login.studio.state.committed().selection(),
        AppearanceSelection::default()
    );
    for palette in 0..12 {
        login.studio.state.edit_builder(|b| {
            b.append(palette);
        });
    }
    login.prepare([960, 540], false, None).unwrap();
    let selected = login.studio.state.draft().unwrap().builder.selected;
    login.focus(ControlId(RUN_BASE + selected as u16));
    key(&mut login, NamedKey::Home);
    assert_eq!(login.focused(), Some(ControlId(RUN_BASE)));
    key(&mut login, NamedKey::End);
    assert_eq!(login.focused(), Some(ControlId(RUN_BASE + selected as u16)));
    // The core panel remains the reading order egui follows.
    let semantics = login
        .studio_panel
        .semantics([960., 540.], &Default::default(), &Default::default())
        .unwrap();
    let position = |id| semantics.iter().position(|n| n.id == id).unwrap();
    assert!(position(BUILDER_TAB) < position(ControlId(SWATCH_BASE)));
    assert!(position(RESET) < position(CANCEL));
    assert!(position(CANCEL) < position(SAVE));
    assert!(
        semantics
            .iter()
            .any(|n| n.id == BUILDER_TAB && n.selected == Some(true))
    );
    assert_eq!(
        semantics
            .iter()
            .filter(|n| n.id.0 >= SWATCH_BASE && n.id.0 < RUN_BASE)
            .count(),
        39
    );
    assert_eq!(semantics.iter().filter(|n| n.id.0 >= RUN_BASE).count(), 4);
    // Pointer arrows can reach both ends without changing the pattern.
    let before = login
        .studio
        .state
        .draft()
        .unwrap()
        .builder
        .pattern()
        .clone();
    for _ in 0..selected {
        login.studio.activate(RUN_PREVIOUS, "Bytes");
    }
    assert_eq!(login.studio.state.draft().unwrap().builder.selected, 0);
    for _ in 0..selected {
        login.studio.activate(RUN_NEXT, "Bytes");
    }
    assert_eq!(
        login.studio.state.draft().unwrap().builder.selected,
        selected
    );
    assert_eq!(
        login.studio.state.draft().unwrap().builder.pattern(),
        &before
    );
    let (intents, _) = login.step(426., false, false).unwrap();
    assert!(intents.is_empty());
    login.focus(BUILDER_TAB);
    login.input.modifiers(ModifiersState::SHIFT);
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(SAVE));
    login.input.modifiers(ModifiersState::empty());
    key(&mut login, NamedKey::Escape);
    login.step(656., false, false).unwrap();
    assert_eq!(
        login.studio.state.committed().selection(),
        AppearanceSelection::default()
    );
}
#[test]
fn cap_reset_and_scroll_cancel_capture_and_keep_fixed_preview_footer() {
    let mut login = login();
    for n in 0..47 {
        login.studio.state.edit_builder(|b| {
            b.append(PALETTES[n % 39]);
        });
    }
    login.prepare([960, 540], false, None).unwrap();
    assert!(
        !login
            .studio_panel
            .controls
            .iter()
            .find(|c| c.id == ADD)
            .unwrap()
            .enabled
    );
    let area = [360., 640.];
    login.prepare([360, 640], false, None).unwrap();
    let bounds = StudioState::preview_bounds(area);
    login.studio.state.reveal_control(RESET, area);
    login.prepare([360, 640], false, None).unwrap();
    let resolved = login.studio_panel.resolve(area).unwrap();
    let reset = resolved.iter().find(|r| r.control.id == RESET).unwrap();
    let point = [reset.rect.x + reset.rect.width / 2., reset.rect.y + 14.];
    login.pointer(point, Some(true), false);
    // egui-winit's translation of a one-line wheel notch.
    login.settings_input(egui::RawInput {
        events: vec![egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0., 1.),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
            source: egui::MouseWheelSource::Unknown,
        }],
        ..Default::default()
    });
    login.pointer(point, Some(false), false);
    assert_eq!(
        login
            .studio
            .state
            .draft()
            .unwrap()
            .builder
            .pattern()
            .colours(),
        47
    );
    assert_eq!(StudioState::preview_bounds(area), bounds);
    login.studio.activate(RESET, "Bytes");
    let mut calls = 0;
    login.studio.activate_with_random(SAVE, "Bytes", &mut || {
        calls += 1;
        99
    });
    assert_eq!(calls, 0);
    assert_eq!(
        login.studio.state.committed().selection(),
        AppearanceSelection::default()
    );
}
fn sent(selection: &AppearanceSelection) -> Vec<u8> {
    let mut game = LocalGame::new(0.).unwrap();
    game.play_with_appearance(
        "ws://127.0.0.1:19491/slither",
        "Bytes",
        selection,
        &LoginProfile {
            client_version: 291,
            page_echo: [7; 20],
        },
    )
    .unwrap();
    support::pump(&mut game, 1.);
    support::effects(&mut game);
    game.opened(game.generation(), 2.).unwrap();
    support::effects(&mut game);
    game.receive(
        game.generation(),
        &support::bundle(&[support::challenge()]),
        3.,
    )
    .unwrap();
    support::effects(&mut game)[1].clone()
}
fn expected(skin: u8, custom: &[u8]) -> Vec<u8> {
    [
        vec![b's', 30, 1, 35],
        vec![7; 20],
        vec![skin, 5],
        b"Bytes".to_vec(),
        vec![0, 255],
        custom.to_vec(),
    ]
    .concat()
}
#[test]
fn empty_builder_exit_and_empty_builder_save_send_their_own_selection() {
    for next in [false, true] {
        for retained_custom in [false, true] {
            for leave_builder in [false, true] {
                let custom = if retained_custom {
                    vec![1, 2, 3, 4, 5, 6, 7, 8, 2, 41]
                } else {
                    vec![]
                };
                let mut state = StudioState::new(AppearanceRecord {
                    builtin: 7,
                    custom_base: 7,
                    custom_bytes: custom.clone(),
                    ..Default::default()
                })
                .unwrap();
                state.open(0., true);
                state.step(230.);
                state.cycle(next);
                state.set_section(StudioSection::Builder, 230.);
                if retained_custom {
                    state.edit_builder(|b| b.reset());
                }
                if leave_builder {
                    state.set_section(StudioSection::BuiltIn, 410.);
                }
                let saved = state
                    .save("Bytes", 410., &mut || {
                        panic!("empty builder sampled entropy")
                    })
                    .unwrap()
                    .unwrap();
                let skin = if leave_builder {
                    if next { 8 } else { 6 }
                } else {
                    7
                };
                let wire_custom = if leave_builder { &[][..] } else { &custom[..] };
                assert_eq!(saved.custom_bytes, custom);
                assert_eq!(sent(&saved.selection()), expected(skin, wire_custom));
            }
        }
    }
}
#[test]
fn authored_and_retained_patterns_reach_actual_login_with_exact_bytes_and_base() {
    for colours in [
        PALETTES.to_vec(),
        vec![41; 47],
        (0..47).map(|n| PALETTES[n % 39]).collect(),
    ] {
        let mut login = login();
        for palette in &colours {
            login.studio.state.edit_builder(|b| {
                b.append(*palette);
            });
        }
        let mut calls = 0;
        login.studio.activate_with_random(SAVE, "Bytes", &mut || {
            calls += 1;
            if calls == 1 { 19 } else { 237 }
        });
        assert_eq!(calls, 2);
        let selection = login.studio.state.committed().selection();
        let mut bytes = vec![255, 255, 255, 0, 0, 0, 19, 237];
        for palette in colours {
            if bytes.len() > 8 && *bytes.last().unwrap() == palette {
                let index = bytes.len() - 2;
                bytes[index] += 1;
            } else {
                bytes.extend([1, palette]);
            }
        }
        assert_eq!(selection.custom_bytes, bytes);
        assert_eq!(sent(&selection), expected(0, &bytes));
    }
    for skin in [7, 42, 65] {
        for custom in [
            vec![1, 2, 3, 4, 5, 6, 7, 8, 2, 41],
            vec![9; 255],
            [vec![0; 8], [255, 1].repeat(123)].concat(),
        ] {
            let record = AppearanceRecord {
                nickname: "Bytes".into(),
                builtin: skin,
                custom_base: skin,
                custom_bytes: custom.clone(),
                active_custom: true,
                cosmetic: None,
            };
            let mut state = StudioState::new(record).unwrap();
            state.open(0., true);
            state.set_section(StudioSection::Builder, 0.);
            let record = state
                .save("Bytes", 230., &mut || panic!("retained entropy"))
                .unwrap()
                .unwrap();
            assert_eq!(sent(&record.selection()), expected(skin, &custom));
        }
    }
}
#[test]
fn builder_quota_and_held_picture_retry_at_dpr_one_to_five() {
    for dpr in [1., 1.5, 2., 3., 4., 5.] {
        let mut login = login();
        for n in 0..47 {
            login.studio.state.edit_builder(|b| {
                b.append(PALETTES[n % 39]);
            });
        }
        for logical in [[960., 540.], [360., 640.], [960., 360.]] {
            let extent = logical.map(|n| (n * dpr) as u32);
            for focus in [
                BUILDER_TAB,
                ControlId(SWATCH_BASE + 41),
                ControlId(RUN_BASE + 46),
                REMOVE,
                RECOLOUR,
                RESET,
                CANCEL,
                SAVE,
            ] {
                login.focus(focus);
                login.studio.state.reveal_control(focus, logical);
                let menu = login.prepare_host(extent, dpr, false, None).unwrap();
                assert!(
                    menu.frame.draws.len() <= 384,
                    "DPR {dpr}: {} draws",
                    menu.frame.draws.len()
                );
                assert!(menu.resources.len() <= 40, "{} slots", menu.resources.len());
                assert!(
                    menu.resources
                        .iter()
                        .map(|r| r.image.pixels.len())
                        .sum::<usize>()
                        <= 8 * 1024 * 1024
                );
                for (namespace, slots, bytes) in [(7, 8, 2 * 1024 * 1024), (8, 20, 2 * 1024 * 1024)]
                {
                    let group: Vec<_> = menu
                        .resources
                        .iter()
                        .filter(|r| r.asset.handle >> 60 == namespace)
                        .collect();
                    assert!(
                        group.len() <= slots,
                        "DPR {dpr}, {logical:?}, namespace {namespace}: {} slots",
                        group.len()
                    );
                    assert!(
                        group.iter().map(|r| r.image.pixels.len()).sum::<usize>() <= bytes,
                        "DPR {dpr}, {logical:?}, namespace {namespace}: byte allocation"
                    );
                }
                let mut picture = slither_wgpu_slice::render::PreparedFrame {
                    frame: menu.frame,
                    overlay: None,
                    resources: menu.resources,
                };
                login.studio.append_preview(&mut picture, None).unwrap();
                assert!(picture.draw_count() <= 1536);
                let held = picture.resources.clone();
                let (a, _) = login.studio.prepare_preview(extent, dpr, None).unwrap();
                let (b, _) = login.studio.prepare_preview(extent, dpr, None).unwrap();
                assert_eq!(a.draws, b.draws);
                assert!(held.iter().all(|r| !r.image.pixels.is_empty()));
                login.studio.presented();
            }
        }
    }
}

#[test]
fn builder_preserves_the_landed_menu_density_cap_at_extreme_dpr() {
    for dpr in [8., 16.] {
        let mut login = login();
        for palette in PALETTES {
            login.studio.state.edit_builder(|b| {
                b.append(palette);
            });
        }
        let extent = [(360. * dpr) as u32, (240. * dpr) as u32];
        let menu = login.prepare_host(extent, dpr, false, None).unwrap();
        assert!(menu.frame.draws.len() <= 384);
        for (namespace, slots) in [(7, 8), (8, 20)] {
            let group: Vec<_> = menu
                .resources
                .iter()
                .filter(|r| r.asset.handle >> 60 == namespace)
                .collect();
            assert!(group.len() <= slots);
            assert!(group.iter().map(|r| r.image.pixels.len()).sum::<usize>() <= 2 * 1024 * 1024);
        }
        menu.frame
            .validate(
                &menu
                    .resources
                    .iter()
                    .map(|r| (r.asset.into(), r.asset))
                    .collect(),
            )
            .unwrap();
    }
}
