//! Authored local art, pure transport effects, and finite offscreen menu preparation.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "support/announcement_tape.rs"]
mod support;
use slither_client::{menu_controls::*, skin_studio::*};
use slither_wgpu_slice::{
    cosmetics::{Cosmetics, Images},
    login_native::NativeLogin,
    menu_storage::MenuStorage,
};

fn art() -> Cosmetics {
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(32, 24, image::Rgba([100, 160, 220, 200]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let mut images = Images::default();
    for (name, ..) in slither_wgpu_slice::cosmetics::ACCESSORIES {
        images
            .add(&format!("s/a_{name}.png"), png.get_ref())
            .unwrap();
    }
    Cosmetics::from_images(images, None).unwrap()
}
fn opened(record: AppearanceRecord) -> NativeLogin {
    let mut login = NativeLogin::with_preferences(record, MenuStorage::at(None)).unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, false, false).unwrap();
    }
    login.open_studio();
    login
}
/// The egui studio adds no menu records; its only textures are the visible
/// cosmetic thumbnails, uploaded once and released when the studio closes.
#[test]
fn cosmetics_first_entry_fades_windows_and_return_fit_the_allowances() {
    for logical in [[960., 540.], [360., 640.], [960., 360.]] {
        for dpr in [1., 1.25, 1.5, 1.75, 2., 3., 4., 5., 6., 7., 8.] {
            let extent = logical.map(|v| (v * dpr) as u32);
            let mut art = art();
            let mut login = opened(AppearanceRecord::default());
            login.prepare_host(extent, dpr, false, Some(&art)).unwrap();
            let context = login.egui.context().unwrap().clone();
            let textures = || context.tex_manager().read().num_allocated();
            let baseline = textures();
            login.studio.activate(COSMETICS_TAB, "Player");
            let mut shown = 0;
            for time in [0., 1., 90., 115., 180., 229., 230., 410., 525., 640., 755.] {
                login.step(9600. + time, false, false).unwrap();
                if time == 410. {
                    login.studio.state.scroll_cosmetics(99, logical);
                    login.studio.state.select_cosmetic(Some(31));
                }
                if time == 525. {
                    login.studio.activate(CANCEL, "Player");
                }
                login.studio.sync_art(logical, Some(&mut art)).unwrap();
                let picture = login.prepare_host(extent, dpr, false, Some(&art)).unwrap();
                assert!(picture.frame.draws.is_empty(), "{logical:?} {dpr} {time}");
                assert!(picture.resources.len() <= 64);
                assert!(
                    picture
                        .resources
                        .iter()
                        .map(|r| r.image.pixels.len())
                        .sum::<usize>()
                        <= 13 * 1024 * 1024
                );
                let thumbnails = login.studio.thumbnails.len();
                assert!(thumbnails <= 12);
                shown = shown.max(thumbnails);
                if login.studio.state.visible() {
                    assert_eq!(
                        textures(),
                        baseline + thumbnails,
                        "{logical:?} {dpr} {time}"
                    );
                    let (preview, _) = login
                        .studio
                        .prepare_preview(extent, dpr, Some(&art))
                        .unwrap();
                    assert!(preview.draws.len() <= 1024);
                    login.studio.presented();
                }
            }
            assert!(shown > 0);
            assert!(!login.studio.state.visible());
            assert_eq!(textures(), baseline, "{logical:?} {dpr}");
        }
    }
}

#[test]
fn all_choices_are_selectable_without_art_and_cancel_discards_them() {
    let mut state = StudioState::new(AppearanceRecord::default()).unwrap();
    state.open(0., true);
    state.set_section(StudioSection::Cosmetics, 0.);
    for index in 0..33 {
        let choice = (index > 0).then(|| (index - 1) as u8);
        state.select_cosmetic(choice);
        let panel = state.panel([360., 640.], "");
        assert!(
            panel
                .controls
                .iter()
                .any(|c| c.id == ControlId(COSMETIC_BASE + index) && c.enabled)
        );
        assert_eq!(state.preview_cosmetic(), choice);
        assert!(state.cosmetic_range([360., 640.]).len() <= 12);
        assert!(state.cosmetic_metadata_range([360., 640.]).len() <= 16);
    }
    state.cancel(230.);
    state.step(460.);
    assert_eq!(state.committed().cosmetic, None);
}

fn wire(selection: &AppearanceSelection, endpoint: bool, cosmetic: Option<u8>) -> Vec<u8> {
    use slither_wgpu_slice::game::{LocalGame, LoginProfile};
    let mut game = LocalGame::new(0.).unwrap();
    // Merely preparing/selecting local art has no connection or Send effect.
    game.configure_cosmetics(Cosmetics::from_images(Default::default(), None).unwrap())
        .unwrap();
    game.set_committed_cosmetic(cosmetic).unwrap();
    assert!(game.effect().is_none());
    if endpoint {
        slither_wgpu_slice::endpoint_login::play_appearance(
            &mut game,
            "ws://127.0.0.1:19491/slither",
            "Bytes",
            selection,
        )
        .unwrap();
    } else {
        game.play_with_appearance(
            "ws://127.0.0.1:19491/slither",
            "Bytes",
            selection,
            &LoginProfile {
                client_version: 291,
                page_echo: [19; 20],
            },
        )
        .unwrap();
    }
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
#[test]
fn cosmetic_edits_preserve_every_login_byte_for_builtins_opaque_legacy_and_65() {
    let patterns = [
        vec![],
        vec![1, 2, 3, 4, 5, 6, 7, 8, 2, 1],
        [vec![0; 8], [255, 1].repeat(123)].concat(),
        vec![9; 255],
    ];
    let mut cases = 0;
    for skin in 0..=65 {
        for custom in &patterns {
            let original = AppearanceRecord {
                nickname: "Bytes".into(),
                builtin: skin,
                custom_base: skin,
                custom_bytes: custom.clone(),
                active_custom: !custom.is_empty(),
                cosmetic: None,
            };
            let mut state = StudioState::new(original.clone()).unwrap();
            state.open(0., true);
            state.set_section(StudioSection::Cosmetics, 0.);
            state.select_cosmetic(Some(skin % 32));
            let saved = state
                .save("Bytes", 230., &mut || {
                    panic!("cosmetic edit sampled pattern entropy")
                })
                .unwrap()
                .unwrap();
            assert_eq!(saved.selection(), original.selection());
            assert_eq!(
                AppearanceRecord::parse(&saved.serialize().unwrap()),
                Some(saved.clone())
            );
            let frozen = saved.selection();
            // A later draft cannot change either delayed-profile or fixed-endpoint snapshots.
            state.step(460.);
            state.open(460., true);
            state.cycle(true);
            state.select_cosmetic(None);
            for endpoint in [false, true] {
                let mut expected = vec![b's', 30, 1, 35];
                expected.extend(if endpoint {
                    vec![
                        54, 206, 204, 169, 97, 178, 74, 136, 124, 117, 14, 210, 106, 236, 8, 208,
                        136, 213, 140, 111,
                    ]
                } else {
                    vec![19; 20]
                });
                expected.extend([skin, 5]);
                expected.extend(b"Bytes");
                expected.extend([0, 255]);
                expected.extend(custom);
                assert_eq!(
                    wire(&frozen, endpoint, saved.cosmetic),
                    expected,
                    "skin {skin}, endpoint {endpoint}"
                );
                cases += 1;
            }
        }
    }
    println!("{cases} exact native cosmetic-only login comparisons");
}
#[test]
fn explicitly_choosing_skin_42_encodes_42() {
    let mut state = StudioState::new(AppearanceRecord {
        builtin: 41,
        ..Default::default()
    })
    .unwrap();
    state.open(0., true);
    state.cycle(true);
    let saved = state.save("Bytes", 230., &mut || 0).unwrap().unwrap();
    assert_eq!(saved.builtin, 42);
    assert_eq!(wire(&saved.selection(), false, None)[24], 42);
    assert_eq!(wire(&saved.selection(), true, None)[24], 42);
}

#[test]
fn committed_setter_after_first_play_preserves_session_and_retained_picture() {
    use slither_wgpu_slice::game::LocalGame;
    let mut game = LocalGame::new(0.).unwrap();
    game.configure_cosmetics(art()).unwrap();
    game.set_committed_cosmetic(Some(0)).unwrap();
    game.play("ws://127.0.0.1:19491/slither", "Bytes", 7)
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
    support::effects(&mut game);
    game.receive(
        game.generation(),
        &support::bundle(&[support::setup(), support::snake(7)]),
        4.,
    )
    .unwrap();
    support::pictures(&mut game);
    game.begin_redraw(20., [960, 540], 1.).unwrap();
    let held = game.take_frame().unwrap();
    let before = held.frame.clone();
    assert!(game.configure_cosmetics(art()).is_err());
    assert!(game.set_committed_cosmetic(Some(31)).is_err());
    game.presented(true).unwrap();
    game.stop();
    support::effects(&mut game);
    support::pump(&mut game, 40.);
    assert!(game.can_play());
    let phase = game.phase();
    let generation = game.generation();
    let effects = game.effect();
    let bank_assets = game.bank.assets.clone();
    game.set_committed_cosmetic(Some(31)).unwrap();
    assert_eq!(game.bank.assets, bank_assets);
    assert_eq!(game.menu_cosmetics().unwrap().own_accessory, Some(31));
    assert_eq!(game.phase(), phase);
    assert_eq!(game.generation(), generation);
    assert_eq!(game.effect(), effects);
    assert_eq!(held.frame, before);
    held.frame
        .validate(
            &held
                .resources
                .iter()
                .map(|r| (r.asset.into(), r.asset))
                .collect(),
        )
        .unwrap();
    game.set_committed_cosmetic(None).unwrap();
    assert!(game.menu_cosmetics().unwrap().accessory(true).is_none());
    assert!(game.set_committed_cosmetic(Some(32)).is_err());
    assert_eq!(held.frame, before);
}

#[test]
fn ready_attachment_fits_preview_and_single_image_faces_suppress_it() {
    use slither_client::presentation::FaceGeometry;
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(1024, 1024, image::Rgba([130, 170, 210, 180]))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let mut images = Images::default();
    images.add("s/a_oakley.png", png.get_ref()).unwrap();
    let mut art = Cosmetics::from_images(images, None).unwrap();
    for skin in [7, 24, 42, 65] {
        for custom in [vec![], vec![1, 2, 3, 4, 5, 6, 7, 8, 2, 1]] {
            let mut login = opened(AppearanceRecord {
                builtin: skin,
                custom_base: skin,
                custom_bytes: custom.clone(),
                active_custom: !custom.is_empty(),
                cosmetic: Some(0),
                ..Default::default()
            });
            login.step(9830., false, false).unwrap();
            login.studio.sync_art([960., 540.], Some(&mut art)).unwrap();
            let source = art.preview_sources_for(Some(0));
            let asset = source.accessory(true).unwrap().asset;
            let resolved = source.skin(skin, &custom).unwrap();
            let expected = !matches!(resolved.face, FaceGeometry::SingleImage(_));
            for dpr in [1., 1.5, 2., 5., 8.] {
                let (frame, resources) = login
                    .studio
                    .prepare_preview([(960. * dpr) as u32, (540. * dpr) as u32], dpr, Some(&art))
                    .unwrap();
                assert_eq!(resources.iter().any(|r| r.asset == asset), expected);
                let bounds = StudioState::preview_bounds([960., 540.]);
                for draw in frame.draws.iter().filter(|d| d.asset == asset) {
                    for x in [
                        draw.destination.x,
                        draw.destination.x + draw.destination.width,
                    ] {
                        for y in [
                            draw.destination.y,
                            draw.destination.y + draw.destination.height,
                        ] {
                            let px = x * draw.rotation.cos() - y * draw.rotation.sin()
                                + draw.translation[0];
                            let py = x * draw.rotation.sin()
                                + y * draw.rotation.cos()
                                + draw.translation[1];
                            assert!(px >= bounds.x - 1e-8 && px <= bounds.x + bounds.width + 1e-8);
                            assert!(py >= bounds.y - 1e-8 && py <= bounds.y + bounds.height + 1e-8);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn three_tabs_keep_full_label_ink_and_padding_in_responsive_layouts() {
    use slither_client::menu_theme::{MenuTheme, STUDIO_ACTION_PADDING_X};
    use slither_wgpu_slice::text::{Align, Baseline, TextCache, TextRun};
    let mut text = TextCache::menu();
    let mut state = StudioState::new(AppearanceRecord::default()).unwrap();
    state.open(0., true);
    for logical in [[960., 540.], [360., 640.], [280., 320.], [240., 320.]] {
        for dpr in [1., 1.25, 1.5, 2., 3., 4., 5., 6., 7., 8.] {
            for c in state.panel(logical, "").resolve(logical).unwrap() {
                if let ControlKind::SectionTab { label, .. } = &c.control.kind {
                    let raster = text
                        .draw(
                            TextRun {
                                text: label,
                                style: MenuTheme::default().text_style(c.control.style),
                                position: [0., 0.],
                                align: Align::Left,
                                baseline: Baseline::Alphabetic,
                                alpha: 1.,
                            },
                            dpr,
                        )
                        .unwrap();
                    let image = &raster[0].1.image;
                    let columns: Vec<_> = image
                        .pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| p[3] > 0)
                        .map(|(i, _)| i % image.width as usize)
                        .collect();
                    let width = (columns.iter().max().unwrap() - columns.iter().min().unwrap() + 1)
                        as f64
                        / dpr;
                    assert!(
                        width <= c.rect.width - 2. * STUDIO_ACTION_PADDING_X,
                        "{logical:?} {dpr} {label}: {width} in {}",
                        c.rect.width
                    );
                }
            }
        }
    }
}

#[test]
fn tiny_window_scrolls_both_directions_without_crossing_preview_status_or_footer() {
    let mut state = StudioState::new(AppearanceRecord::default()).unwrap();
    state.open(0., true);
    state.set_section(StudioSection::Cosmetics, 0.);
    let small = [240., 320.];
    assert_eq!(state.cosmetic_range(small), 0..4);
    state.scroll_cosmetics(1, small);
    assert_eq!(state.cosmetic_range(small), 4..8);
    state.scroll_cosmetics(1, small);
    assert_eq!(state.cosmetic_range(small), 8..12);
    state.scroll_cosmetics(-1, small);
    assert_eq!(state.cosmetic_range(small), 4..8);
    state.select_cosmetic(Some(31));
    assert_eq!(state.cosmetic_range(small), 32..33);
    state.scroll_cosmetics(-1, small);
    assert_eq!(state.cosmetic_range(small), 28..32);
    state.scroll_cosmetics(1, small);
    assert_eq!(state.cosmetic_range(small), 32..33);
    for viewport in [
        small,
        [280., 320.],
        [360., 640.],
        [960., 360.],
        [960., 540.],
    ] {
        let panel = state.panel(viewport, "");
        let resolved = panel.resolve(viewport).unwrap();
        let preview = StudioState::preview_bounds(viewport);
        for cell in resolved
            .iter()
            .filter(|r| matches!(r.control.kind, ControlKind::CosmeticCell { .. }))
        {
            assert!(cell.rect.width >= 28. && cell.rect.height >= 28.);
            assert_eq!(
                intersection(cell.rect, preview).width * intersection(cell.rect, preview).height,
                0.
            );
            for id in [
                ControlId(4),
                CANCEL,
                SAVE,
                BUILT_IN_TAB,
                BUILDER_TAB,
                COSMETICS_TAB,
            ] {
                let other = resolved.iter().find(|r| r.control.id == id).unwrap().rect;
                let overlap = intersection(cell.rect, other);
                assert!(
                    overlap.width == 0. || overlap.height == 0.,
                    "{viewport:?} cell overlaps {id:?}"
                );
            }
        }
    }
}

#[test]
fn generation_capture_blur_resize_escape_and_joint_save_keep_transaction_rules() {
    use slither_wgpu_slice::menu_input_native::KeyInput;
    use winit::{
        event::{ElementState, WindowEvent},
        keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    };
    let mut login = opened(AppearanceRecord {
        builtin: 7,
        ..Default::default()
    });
    login.step(9830., false, false).unwrap();
    login.studio.activate(COSMETICS_TAB, "Player");
    login.prepare_host([360, 640], 1., false, None).unwrap();
    let resolved = login.studio_panel.resolve([360., 640.]).unwrap();
    let cell = resolved
        .iter()
        .find(|r| r.control.id == ControlId(COSMETIC_BASE + 1))
        .unwrap()
        .rect;
    let point = [cell.x + cell.width / 2., cell.y + cell.height / 2.];
    login.pointer(point, Some(true), false);
    login.studio.state.scroll_cosmetics(3, [360., 640.]);
    login.prepare_host([360, 640], 1., false, None).unwrap();
    login.pointer(point, Some(false), false);
    assert_eq!(login.studio.state.preview_cosmetic(), None);
    login.studio.state.select_cosmetic(Some(31));
    login.event(&WindowEvent::Focused(false), false);
    login.prepare_host([240, 320], 1., false, None).unwrap();
    assert_eq!(login.studio.state.preview_cosmetic(), Some(31));
    let logical = Key::Named(NamedKey::Escape);
    login.key(
        KeyInput {
            logical: &logical,
            physical: PhysicalKey::Code(KeyCode::Escape),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
        },
        false,
    );
    login.step(10060., false, false).unwrap();
    assert_eq!(login.studio.state.committed().cosmetic, None);
    login.open_studio();
    login.step(10290., false, false).unwrap();
    login.studio.activate(BUILDER_TAB, "Player");
    login.studio.state.edit_builder(|b| {
        b.append(1);
    });
    login.studio.activate(COSMETICS_TAB, "Player");
    login.studio.state.select_cosmetic(Some(31));
    let mut entropy = 0;
    login.studio.activate_with_random(SAVE, "Player", &mut || {
        entropy += 1;
        17
    });
    let saved = login.studio.state.committed().clone();
    assert_eq!(saved.cosmetic, Some(31));
    assert!(saved.active_custom);
    assert_eq!(saved.custom_bytes, [255, 255, 255, 0, 0, 0, 17, 17, 1, 1]);
    login
        .studio
        .activate_with_random(SAVE, "Ignored", &mut || panic!("repeated Save"));
    assert_eq!(entropy, 2);
    assert_eq!(login.studio.state.committed(), &saved);
}
