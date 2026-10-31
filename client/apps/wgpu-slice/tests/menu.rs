//! Authored synthetic controls and images; no game server, GPU or private art.
use slither_client::{
    menu_layout::{MenuLayout, MenuMetrics},
    menu_preferences::MenuQuality,
    start_page::{MenuLocale, StartMenuState},
};
use slither_wgpu_slice::{
    cosmetics::{Cosmetics, Images, MenuArt, MenuManifest},
    menu::{Menu, check_menu_draws},
    text::TextCache,
};
use std::io::Cursor;

fn png(size: u32, color: [u8; 4]) -> Vec<u8> {
    png_rect(size, size, color)
}
#[test]
fn quality_roles_use_independent_default_bindings_and_reject_server() {
    let manifest = MenuManifest::default();
    assert!(
        manifest
            .0
            .iter()
            .any(|e| e.art == MenuArt::QualityHigh && e.path == "menu/quality-high.png")
    );
    assert!(
        manifest
            .0
            .iter()
            .any(|e| e.art == MenuArt::QualityLow && e.path == "menu/quality-low.png")
    );
    assert!(MenuManifest::parse(b"menu-art-v1\nserver 1 menu/server.png").is_err());
    let mut images = Images::default();
    images
        .add("menu/quality-high.png", &png(16, [10, 20, 30, 255]))
        .unwrap();
    images
        .add("menu/quality-low.png", &png(16, [30, 20, 10, 255]))
        .unwrap();
    let art = Cosmetics::from_images(images, None).unwrap();
    assert_ne!(
        art.menu_art(MenuArt::QualityHigh).unwrap().asset.handle,
        art.menu_art(MenuArt::QualityLow).unwrap().asset.handle
    );
}
fn png_rect(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(width, height, image::Rgba(color))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}
#[test]
fn physical_menu_rasters_are_pixel_aligned_one_to_one_and_reused() {
    let mut images = Images::default();
    images
        .add("menu/skin.png", &png_rect(1350, 405, [20, 180, 60, 255]))
        .unwrap();
    let art = Cosmetics::from_images(images, None).unwrap();
    assert_eq!(art.menu_art(MenuArt::Skin).unwrap().asset.width, 1350);
    assert_eq!(art.menu_art(MenuArt::Skin).unwrap().asset.height, 405);
    // The larger menu decode bound must not broaden world cosmetic inputs.
    assert!(slither_wgpu_slice::cosmetics::decode(&png_rect(1350, 405, [255; 4])).is_err());
    for dpr in [1., 1.25, 1.5, 1.75, 2.] {
        let mut menu = Menu::default();
        let mut text = TextCache::menu();
        let view = slither_wgpu_slice::menu_specimen::view(4).unwrap();
        let layout =
            slither_wgpu_slice::menu_specimen::layout(&mut text, &view, [3840, 2160], dpr).unwrap();
        let mut previous = None;
        for _ in 0..2 {
            let p = menu
                .prepare(&mut text, &view, &layout, [3840, 2160], dpr, Some(&art))
                .unwrap();
            assert!(!p.frame.draws.is_empty());
            for draw in &p.frame.draws {
                let r = draw.destination;
                for value in [
                    (r.x + draw.translation[0]) * dpr,
                    (r.y + draw.translation[1]) * dpr,
                ] {
                    assert!((value - value.round()).abs() < 1e-9, "{dpr}: {draw:?}");
                }
                let source = draw
                    .source
                    .map(|s| [s.width, s.height])
                    .unwrap_or([f64::from(draw.asset.width), f64::from(draw.asset.height)]);
                assert!((r.width * dpr - source[0]).abs() < 1e-9, "{dpr}: {draw:?}");
                assert!((r.height * dpr - source[1]).abs() < 1e-9, "{dpr}: {draw:?}");
            }
            let assets: Vec<_> = p.frame.draws.iter().map(|d| d.asset).collect();
            if let Some(old) = previous {
                assert_eq!(assets, old);
            }
            previous = Some(assets);
        }
    }
}
fn layout() -> MenuLayout {
    MenuLayout::new(
        1280.,
        720.,
        0.,
        true,
        MenuMetrics {
            last_length_line_height: 18.,
            nickname_baseline_space: 4.,
            padded_play_label_width: 52.,
        },
    )
    .unwrap()
}
fn art() -> Cosmetics {
    let mut images = Images::default();
    images
        .manifest(b"menu-art-v1\nskin 2 menu/first.png\n")
        .unwrap();
    images
        .add("menu/first.png", &png(8, [20, 180, 60, 255]))
        .unwrap();
    Cosmetics::from_images(images, None).unwrap()
}
#[test]
fn manifests_reject_malformed_duplicates_and_escaping_paths() {
    for s in [
        "",
        "menu-art-v2",
        "\u{feff}menu-art-v1\nskin 1 menu/skin.png",
        "menu-art-v1\nunknown 1 menu/a.png",
        "menu-art-v1\nskin 0 menu/a.png",
        "menu-art-v1\nskin 1 ../a.png",
        "menu-art-v1\nskin 1 /a.png",
        "menu-art-v1\nskin 1 C:/a.png",
        "menu-art-v1\nskin 1 https://a/a.png",
        "menu-art-v1\nskin 1 menu\\a.png",
        "menu-art-v1\nskin 1 menu/./a.png",
        "menu-art-v1\nskin 1 menu/a.png\nskin 2 menu/b.png",
        "menu-art-v1\nskin 1 menu/a.png\nbuilder 2 menu/a.png",
    ] {
        assert!(MenuManifest::parse(s.as_bytes()).is_err(), "{s}");
    }
    assert!(MenuManifest::parse(&vec![b'a'; 4097]).is_err());
    assert!(MenuManifest::parse(&[0xff]).is_err());
    let mut images = Images::default();
    assert!(images.add("menu/skin.png", b"corrupt").is_err());
    assert!(images.add("../menu/skin.png", &png(1, [0; 4])).is_err());
}
#[test]
fn colliding_menu_manifest_preserves_existing_cosmetic_bindings() {
    use slither_wgpu_slice::cosmetics::{MANIFEST_PATH, paths};
    for path in paths() {
        let manifest = format!("menu-art-v1\nskin 1 {path}\n");
        assert!(
            MenuManifest::parse(manifest.as_bytes())
                .unwrap_err()
                .contains("collides with cosmetic"),
            "{path}"
        );
    }
    let manifest = b"menu-art-v1\nskin 1 s/a_cap.png\n";
    let bytes = png(8, [20, 80, 50, 255]);
    // The browser's Rust batch seam also keeps the default manifest on failure.
    let mut images = Images::default();
    assert!(images.manifest(manifest).is_err());
    images.add("s/a_cap.png", &bytes).unwrap();
    let batch = Cosmetics::from_images(images, Some(30)).unwrap();
    assert!(batch.accessory(true).is_some());
    assert!(batch.menu_art(MenuArt::Skin).is_none());
    let loaded = Cosmetics::load(
        |path| match path {
            MANIFEST_PATH => Ok(manifest.to_vec()),
            "s/a_cap.png" => Ok(bytes.clone()),
            _ => Err("Authored missing image".into()),
        },
        Some(30),
    )
    .unwrap();
    assert!(loaded.accessory(true).is_some());
    assert!(loaded.menu_art(MenuArt::Skin).is_none());
}
#[test]
fn menu_art_shares_existing_cosmetic_pixel_allowance() {
    let mut images = Images::default();
    let bytes = png(1024, [1, 2, 3, 255]);
    for path in ["s/a_oakley.png", "s/a_cap.png", "s/a_crown.png"] {
        images.add(path, &bytes).unwrap();
    }
    let menu_bytes = png(512, [1, 2, 3, 255]);
    images.add("menu/skin.png", &menu_bytes).unwrap();
    for path in ["menu/builder.png", "menu/cosmetic.png"] {
        images.add(path, &menu_bytes).unwrap();
    }
    assert!(images.add("menu/quality-high.png", &menu_bytes).is_err());
    // The failed insert cannot consume the last frame's pixels.
    let art = Cosmetics::from_images(images, None).unwrap();
    assert!(art.menu_art(MenuArt::Skin).is_some());
    assert!(art.menu_art(MenuArt::Builder).is_some());
    assert_eq!(art.menu_art(MenuArt::Skin).unwrap().asset.width, 512);
}
#[test]
fn maximum_menu_image_cannot_displace_procedural_skin_roles() {
    use slither_client::presentation::SkinSourceRole::*;
    let mut images = Images::default();
    assert_eq!(
        images
            .add("menu/skin.png", &png(2048, [80, 180, 60, 255]))
            .unwrap_err(),
        "Cosmetic pixel budget"
    );
    let art = Cosmetics::from_images(images, None).unwrap();
    let roles = [SingleEyeBase, SingleEyePupil, MouthEyeMarks, AcBulb, CdBulb];
    for role in roles {
        assert!(art.role(role).is_some(), "{role:?}");
    }
    assert_eq!(
        art.resources().map(|r| r.image.pixels.len()).sum::<usize>(),
        79_424
    );
    let mut images = Images::default();
    images
        .manifest(b"menu-art-v1\nskin 1 menu/skin.png\nbuilder 1 menu/builder.png\n")
        .unwrap();
    let frame = png_rect(1350, 405, [80, 180, 60, 255]);
    images.add("menu/skin.png", &frame).unwrap();
    images.add("menu/builder.png", &frame).unwrap();
    let art = Cosmetics::from_images(images, None).unwrap();
    for role in roles {
        assert!(art.role(role).is_some(), "{role:?}");
    }
    assert!(art.menu_art(MenuArt::Skin).is_some());
    assert!(art.menu_art(MenuArt::Builder).is_some());
    assert!(
        art.resources().map(|r| r.image.pixels.len()).sum::<usize>()
            <= slither_wgpu_slice::cosmetics::COSMETIC_PIXEL_BUDGET
    );
}
#[test]
fn full_cosmetics_bank_keeps_essential_menu_and_authored_fallbacks() {
    use slither_wgpu_slice::cosmetics::{ACCESSORIES, COSMETIC_PIXEL_BUDGET};
    let procedural_bytes: usize = Cosmetics::from_images(Images::default(), None)
        .unwrap()
        .resources()
        .map(|r| r.image.pixels.len())
        .sum();
    let mut remaining = COSMETIC_PIXEL_BUDGET - procedural_bytes;
    let mut images = Images::default();
    for accessory in ACCESSORIES {
        if remaining == 0 {
            break;
        }
        let pixels = remaining / 4;
        let width = pixels.min(512) as u32;
        let height = (pixels / width as usize).min(512) as u32;
        images
            .add(
                &format!("s/a_{}.png", accessory.0),
                &png_rect(width, height, [20, 80, 50, 255]),
            )
            .unwrap();
        remaining -= width as usize * height as usize * 4;
    }
    assert_eq!(remaining, 0);
    let mut cosmetics = Cosmetics::from_images(images, None).unwrap();
    assert_eq!(
        cosmetics
            .resources()
            .map(|r| r.image.pixels.len())
            .sum::<usize>(),
        COSMETIC_PIXEL_BUDGET
    );
    assert!(
        cosmetics
            .resources()
            .all(|r| r.asset.width <= 512 && r.asset.height <= 512)
    );
    assert!(
        cosmetics
            .replace_menu_art(MenuArt::Skin, &png(1, [255; 4]))
            .is_err()
    );
    assert!(cosmetics.menu_art(MenuArt::Skin).is_none());
    assert!(cosmetics.menu_art(MenuArt::Skin).is_none());
    let view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    let mut text = TextCache::default();
    let prepared = Menu::default()
        .prepare(
            &mut text,
            &view,
            &layout(),
            [1280, 720],
            1.,
            Some(&cosmetics),
        )
        .unwrap();
    assert!(!prepared.frame.draws.is_empty());
    // This full composition contains both controls and the text/icon fallbacks.
    assert!(
        prepared
            .resources
            .iter()
            .any(|r| r.asset.handle >= 0x7000_0000_0000_0000)
    );
    assert!(text.cached_runs() >= 4);
    prepared
        .frame
        .validate(
            &prepared
                .resources
                .iter()
                .map(|r| (r.asset.into(), r.asset))
                .collect(),
        )
        .unwrap();
}
#[test]
fn menu_draw_boundary_and_whole_picture_rollback() {
    assert!(check_menu_draws(512).is_ok());
    assert!(check_menu_draws(513).is_err());
    let view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    let mut menu = Menu::default();
    let mut text = TextCache::default();
    let p = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    let draw = p.frame.draws[0].clone();
    let make = |count| slither_wgpu_slice::menu::MenuFrame {
        backdrop: None,
        world_cover: None,
        frame: slither_wgpu_slice::frame::Frame {
            draws: vec![draw.clone(); count],
            ..p.frame.clone()
        },
        resources: p.resources.clone(),
    };
    let mut empty = slither_wgpu_slice::render::PreparedFrame {
        overlay: None,
        frame: slither_wgpu_slice::frame::Frame {
            draws: Vec::new(),
            ..p.frame.clone()
        },
        resources: Vec::new(),
    };
    make(512).append_to(&mut empty).unwrap();
    assert_eq!(empty.frame.draws.len(), 512);
    let before_frame = empty.frame.clone();
    assert!(make(513).append_to(&mut empty).is_err());
    assert_eq!(empty.frame, before_frame);
    let mut picture = slither_wgpu_slice::render::PreparedFrame {
        overlay: None,
        frame: p.frame.clone(),
        resources: p.resources.clone(),
    };
    picture.frame.draws = vec![draw; slither_wgpu_slice::frame::MAX_DRAWS];
    let before = picture.frame.draws.len();
    let resources = picture.resources.len();
    assert!(p.append_to(&mut picture).is_err());
    assert_eq!(picture.frame.draws.len(), before);
    assert_eq!(picture.resources.len(), resources);
}
#[test]
fn states_dprs_missing_art_resize_and_replacement_keep_owned_frames() {
    let mut menu = Menu::default();
    let mut text = TextCache::default();
    let mut art = art();
    let mut view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    view.tip = view.wording.tips[2];
    view.tip_alpha = 1.;
    let old = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., Some(&art))
        .unwrap();
    let old_resources: Vec<_> = old
        .resources
        .iter()
        .map(|r| (r.asset, r.image.pixels.clone()))
        .collect();
    for dpr in [1., 1.25, 2., 3.] {
        for state in 0..4 {
            view.hover_alpha = f64::from(state == 1);
            view.press_alpha = f64::from(state == 2);
            view.disabled = state == 3;
            let p = menu
                .prepare(
                    &mut text,
                    &view,
                    &layout(),
                    [(1280. * dpr) as u32, (720. * dpr) as u32],
                    dpr,
                    None,
                )
                .unwrap();
            assert!(p.frame.draws.len() < 512);
            let nick =
                layout().viewport_rect(slither_client::menu_layout::MenuElement::NicknameHolder);
            let material_scale = dpr;
            let pad = (40. * material_scale).ceil() / material_scale;
            // The union of the holder's physical tiles retains its full shadow box.
            let tiles: Vec<_> = p
                .frame
                .draws
                .iter()
                .filter(|d| {
                    d.asset.handle >> 60 == 7 && (d.destination.y - (nick.y - pad)).abs() <= 2.
                })
                .map(|d| d.destination)
                .collect();
            assert!(!tiles.is_empty());
            let left = tiles.iter().map(|r| r.x).fold(f64::INFINITY, f64::min);
            let top = tiles.iter().map(|r| r.y).fold(f64::INFINITY, f64::min);
            let right = tiles
                .iter()
                .map(|r| r.x + r.width)
                .fold(f64::NEG_INFINITY, f64::max);
            let bottom = tiles
                .iter()
                .map(|r| r.y + r.height)
                .fold(f64::NEG_INFINITY, f64::max);
            let shape = slither_client::menu_layout::MenuRect {
                x: left,
                y: top,
                width: right - left,
                height: bottom - top,
            };
            assert!((shape.x + pad - nick.x).abs() <= 2.);
            assert!((shape.y + pad - nick.y).abs() <= 2.);
            assert!((shape.x + shape.width - pad - nick.x - nick.width).abs() <= 2.);
            assert!((shape.y + shape.height - pad - nick.y - nick.height).abs() <= 2.);
            assert!(
                p.resources
                    .iter()
                    .all(|r| r.asset.width <= 2048 && r.asset.height <= 2048)
            );
            assert!(
                p.resources
                    .iter()
                    .flat_map(|r| r.image.pixels.as_chunks::<4>().0)
                    .all(|p| p[..3].iter().all(|v| *v <= p[3]))
            );
        }
    }
    art.replace_menu_art(MenuArt::Skin, &png(8, [50, 80, 150, 255]))
        .unwrap();
    menu.prepare(
        &mut text,
        &view,
        &MenuLayout::new(
            960.,
            540.,
            0.,
            true,
            MenuMetrics {
                last_length_line_height: 18.,
                nickname_baseline_space: 4.,
                padded_play_label_width: 52.,
            },
        )
        .unwrap(),
        [960, 540],
        1.,
        Some(&art),
    )
    .unwrap();
    for (resource, (asset, pixels)) in old.resources.iter().zip(old_resources) {
        assert_eq!(resource.asset, asset);
        assert_eq!(resource.image.pixels, pixels);
    }
    old.frame
        .validate(
            &old.resources
                .iter()
                .map(|r| (r.asset.into(), r.asset))
                .collect(),
        )
        .unwrap();
}

#[test]
fn page_backdrop_keeps_its_colour_and_opacity_through_the_crossfade() {
    for transition in [0., 0.5, 1.] {
        let mut view =
            StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
        view.transition = transition;
        view.login_alpha = 1. - transition;
        let mut layout = layout();
        layout
            .update(
                transition,
                true,
                MenuMetrics {
                    last_length_line_height: 18.,
                    nickname_baseline_space: 4.,
                    padded_play_label_width: 52.,
                },
            )
            .unwrap();
        let p = Menu::default()
            .prepare(
                &mut TextCache::menu(),
                &view,
                &layout,
                [3840, 2160],
                3.,
                None,
            )
            .unwrap();
        let backdrop = p.backdrop.as_ref().unwrap();
        let resource = p
            .resources
            .iter()
            .find(|r| r.asset == backdrop.asset)
            .unwrap();
        assert_eq!(resource.image.pixels, [22, 28, 34, 255]);
        assert_eq!(backdrop.effective_alpha, 1.);
        assert_eq!(
            (backdrop.destination.width, backdrop.destination.height),
            (1280., 720.)
        );
    }
}

#[test]
fn connecting_play_dims_the_whole_button_but_return_uses_its_own_alpha() {
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    view.login_alpha = 0.8;
    view.play_return_alpha = 0.6;
    let enabled = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    view.disabled = true;
    view.phase = slither_client::start_page::MenuPhase::Connecting;
    let disabled = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    let changed: Vec<_> = enabled
        .frame
        .draws
        .iter()
        .filter(|d| d.effective_alpha > 0.)
        .zip(
            disabled
                .frame
                .draws
                .iter()
                .filter(|d| d.effective_alpha > 0.),
        )
        .filter(|(a, b)| a.effective_alpha != b.effective_alpha)
        .collect();
    // Every base-capsule tile and the one white Play label share the multiplier.
    let play = layout().viewport_rect(slither_client::menu_layout::MenuElement::Play);
    let enabled_play_tiles = enabled
        .frame
        .draws
        .iter()
        .filter(|d| {
            let r = d.destination;
            d.effective_alpha > 0.
                && d.asset.handle >> 60 == 7
                && r.x >= play.x - 21.
                && r.x + r.width <= play.x + play.width + 21.
                && r.y >= play.y - 21.
                && r.y + r.height <= play.y + play.height + 21.
        })
        .count();
    assert!(enabled_play_tiles > 0);
    assert_eq!(
        changed
            .iter()
            .filter(|(a, _)| a.asset.handle >> 60 == 7)
            .count(),
        enabled_play_tiles
    );
    assert_eq!(
        changed
            .iter()
            .filter(|(a, _)| a.asset.handle >> 60 == 8)
            .count(),
        1
    );
    for (a, b) in changed {
        let r = a.destination;
        assert!(r.x >= play.x - 21. && r.x + r.width <= play.x + play.width + 21.);
        assert!(r.y >= play.y - 21. && r.y + r.height <= play.y + play.height + 21.);
        assert_eq!(a.asset, b.asset);
        assert_eq!(a.destination, b.destination);
        assert!((b.effective_alpha / a.effective_alpha - 0.38).abs() < 1e-12);
    }
    view.phase = slither_client::start_page::MenuPhase::Returning;
    let returning = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    let visible = |frame: &slither_wgpu_slice::frame::Frame| {
        frame
            .draws
            .iter()
            .filter(|draw| draw.effective_alpha > 0.)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(visible(&returning.frame), visible(&enabled.frame));
}

#[test]
fn explicit_backdrop_is_below_all_world_draws_and_absent_for_hidden_login() {
    use slither_wgpu_slice::render::PreparedFrame;
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    let base = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    let world: Vec<_> = base
        .frame
        .draws
        .iter()
        .take(3)
        .cloned()
        .enumerate()
        .map(|(i, mut d)| {
            d.translation = [11. * i as f64, 13.];
            d.effective_alpha = 1.;
            d
        })
        .collect();
    for (visible, spinner) in [(true, false), (false, false), (false, true)] {
        view.visible = visible;
        view.spinner_visible = spinner;
        view.spinner_alpha = 1.;
        view.spinner_scale = 0.9;
        let prepared = menu
            .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
            .unwrap();
        assert_eq!(prepared.backdrop.is_some(), visible);
        let backdrop = prepared.backdrop.clone();
        let mut controls = prepared.frame.draws.clone();
        if let Some(cover) = &prepared.world_cover {
            controls.insert(0, cover.clone());
        }
        let mut picture = PreparedFrame {
            overlay: None,
            frame: slither_wgpu_slice::frame::Frame {
                draws: world.clone(),
                ..base.frame.clone()
            },
            resources: base.resources.clone(),
        };
        prepared.append_to(&mut picture).unwrap();
        let first_world = usize::from(visible);
        if let Some(backdrop) = backdrop {
            assert_eq!(picture.frame.draws[0], backdrop);
            assert!(picture.resources.iter().any(|r| r.asset == backdrop.asset));
        }
        assert_eq!(
            &picture.frame.draws[first_world..first_world + world.len()],
            world
        );
        assert_eq!(&picture.frame.draws[first_world + world.len()..], controls);
    }
}

#[test]
fn spinner_raster_phase_and_revision_follow_shared_time() {
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    view.visible = false;
    view.spinner_visible = true;
    view.spinner_alpha = 0.6;
    view.spinner_scale = 0.9;
    let mut previous = None;
    for dpr in [1., 2., 3.] {
        for (time, phase) in [
            (0., 0.),
            (44., 1.),
            (88., 2.),
            (125., 2.840909090909091),
            (276.4601535159018, std::f64::consts::TAU),
        ] {
            view.spinner_time = time;
            let extent = [(1280. * dpr) as u32, (720. * dpr) as u32];
            let p = menu
                .prepare(&mut text, &view, &layout(), extent, dpr, None)
                .unwrap();
            assert_eq!((p.frame.draws.len(), p.resources.len()), (1, 1));
            let resource = &p.resources[0];
            let draw = &p.frame.draws[0];
            assert!(draw.corners.is_none());
            assert_eq!(draw.effective_alpha, 0.6);
            assert_eq!(
                (resource.asset.width, resource.asset.height),
                (
                    (128. * dpr * view.spinner_scale).ceil() as u32,
                    (128. * dpr * view.spinner_scale).ceil() as u32
                )
            );
            if let Some((handle, revision)) = previous {
                assert_eq!(resource.asset.handle, handle);
                assert!(resource.asset.revision > revision);
            }
            previous = Some((resource.asset.handle, resource.asset.revision));
            // A ray just beyond the pinched tail crosses green at every phase,
            // independently of the radial wobble. Alpha lives on the one quad.
            let angle = phase + 0.25;
            let raster_scale = f64::from(resource.asset.width) / 128.;
            let green = (25..=41).any(|radius| {
                let x = ((64. + 1.25 * f64::from(radius) * angle.cos()) * raster_scale) as usize;
                let y = ((64. + f64::from(radius) * angle.sin()) * raster_scale) as usize;
                let px = &resource.image.pixels[(y * resource.image.width as usize + x) * 4..][..4];
                px[1] > px[0] && px[1] > px[2] && px[3] > 32
            });
            assert!(green, "time={time}, dpr={dpr}");
            assert!(
                resource
                    .image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[3] > 0 && p[3] < 255)
            );
            assert!(
                resource
                    .image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[..3].iter().all(|v| *v <= p[3]))
            );
            let frozen = menu
                .prepare(&mut text, &view, &layout(), extent, dpr, None)
                .unwrap();
            assert_eq!(frozen.resources[0].asset, resource.asset);
        }
    }
    let revision = previous.unwrap().1;
    view.spinner_visible = false;
    let hidden = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    assert!(hidden.resources.is_empty() && hidden.frame.draws.is_empty());
    view.spinner_visible = true;
    let resumed = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    assert_eq!(resumed.resources[0].asset.handle, previous.unwrap().0);
    assert_eq!(resumed.resources[0].asset.revision, revision + 1);
    view.spinner_alpha = 0.3;
    view.spinner_scale = 0.5;
    let faded = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    assert_ne!(faded.resources[0].asset, resumed.resources[0].asset);
    assert_eq!(faded.frame.draws[0].effective_alpha, 0.3);
    assert_eq!(faded.frame.draws[0].destination.width, 64.);
    view.visible = true;
    let menu_frame = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None)
        .unwrap();
    assert_eq!(
        menu_frame.frame.draws.last().unwrap().asset,
        faded.resources[0].asset
    );
}

#[test]
fn page_crossfade_and_menu_materials_never_share_a_handle() {
    let mut view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    view.spinner_visible = true;
    let menu = Menu::default()
        .prepare(
            &mut TextCache::menu(),
            &view,
            &layout(),
            [1280, 720],
            1.,
            None,
        )
        .unwrap();
    let page = slither_wgpu_slice::menu::MenuFrame::world_crossfade([1280, 720], 1., 0.5).unwrap();
    for r in &page.resources {
        assert!(
            menu.resources
                .iter()
                .all(|m| m.asset.handle != r.asset.handle)
        );
    }
}

#[test]
fn stroke_wordmark_is_default_with_local_icons_and_bounded_tiled_rasters() {
    use slither_client::menu_layout::MenuElement as E;
    use slither_wgpu_slice::menu_specimen;
    let has_color = |p: &slither_wgpu_slice::menu::MenuFrame, rgb: [u8; 3]| {
        p.resources.iter().any(|r| {
            r.image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .any(|p| p[..3] == rgb && p[3] > 200)
        })
    };
    let mut text = TextCache::menu();
    let mut images = Images::default();
    images
        .add("menu/skin.png", &png_rect(450, 135, [80, 200, 100, 255]))
        .unwrap();
    let art = Cosmetics::from_images(images, None).unwrap();
    let mut view = menu_specimen::view(0).unwrap();
    for transition in [0., 1.] {
        view.transition = transition;
        view.hover_alpha = 1.;
        view.press_alpha = 1.;
        view.last_life.label = view.wording.final_length;
        view.last_life.number = "10000".into();
        view.last_life.suffix = "!";
        view.play_label = if transition == 0. {
            view.wording.play
        } else {
            view.wording.play_again
        };
        for dpr in [1., 1.25, 1.5, 1.75, 2., 3., 5.] {
            let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
            let layout = menu_specimen::layout(&mut text, &view, extent, dpr).unwrap();
            let holder = layout.viewport_rect(E::Logo);
            let mut menu = Menu::default();
            let ordinary = menu.prepare(&mut text, &view, &layout, extent, dpr, Some(&art));
            // Local art is capped at the same 2048 physical texture bound.
            if dpr <= 2. {
                assert!(has_color(&ordinary.unwrap(), [80, 200, 100]));
            }
            for fraction in [0., 0.15, 0.55, 1.] {
                view.logo_reveal = fraction;
                let p = menu
                    .prepare(&mut text, &view, &layout, extent, dpr, Some(&art))
                    .unwrap_or_else(|e| {
                        panic!("dpr={dpr} transition={transition} reveal={fraction}: {e}")
                    });
                assert!(
                    p.resources
                        .iter()
                        .all(|r| r.asset.width <= 2048 && r.asset.height <= 2048)
                );
                assert!(p.frame.draws.len() <= 512);
                // The procedural wordmark uses gradients, never the old flat
                // two-run colours. At zero reveal it allocates no logo pixels.
                let has_stroke_color = p
                    .resources
                    .iter()
                    .filter(|r| {
                        p.frame.draws.iter().any(|d| {
                            d.asset == r.asset
                                && d.destination.y >= holder.y - 1.
                                && d.destination.y + d.destination.height
                                    <= holder.y + holder.height + 1.
                        })
                    })
                    .any(|r| {
                        r.image
                            .pixels
                            .as_chunks::<4>()
                            .0
                            .iter()
                            .any(|p| p[1] > 180 && p[0] < 140 && p[2] < 170 && p[3] == 255)
                    });
                assert_eq!(has_stroke_color, fraction > 0.);
                assert!(has_color(&p, [80, 200, 100]));
            }
            view.logo_reveal = 1.;
        }
    }
}

#[test]
fn stroke_cache_reuses_pixels_separates_fades_and_rolls_back_failed_text() {
    use slither_wgpu_slice::menu_specimen;
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut view = menu_specimen::view(0).unwrap();
    let layout = menu_specimen::layout(&mut text, &view, [1920, 1080], 1.).unwrap();
    let first = menu
        .prepare(&mut text, &view, &layout, [1920, 1080], 1., None)
        .unwrap();
    let holder = layout.viewport_rect(slither_client::menu_layout::MenuElement::Logo);
    let logo = |p: &slither_wgpu_slice::menu::MenuFrame| {
        p.frame
            .draws
            .iter()
            .filter(|d| is_stroke_draw(d, holder))
            .cloned()
            .collect::<Vec<_>>()
    };
    let original = logo(&first);
    assert_eq!(original.len(), 1);
    view.login_alpha = 0.7;
    view.logo_alpha = 0.6;
    let faded = menu
        .prepare(&mut text, &view, &layout, [1920, 1080], 1., None)
        .unwrap();
    assert_eq!(logo(&faded)[0].asset, original[0].asset);
    assert!((logo(&faded)[0].effective_alpha - 0.42).abs() < 1e-8);
    menu.wordmark_text = Some("x".repeat(129));
    assert!(
        menu.prepare(&mut text, &view, &layout, [1920, 1080], 1., None)
            .is_err()
    );
    menu.wordmark_text = None;
    let after = menu
        .prepare(&mut text, &view, &layout, [1920, 1080], 1., None)
        .unwrap();
    assert_eq!(logo(&after)[0].asset, original[0].asset);
    for p in [&faded, &after] {
        let a = first
            .resources
            .iter()
            .find(|r| r.asset == original[0].asset)
            .unwrap();
        let b = p
            .resources
            .iter()
            .find(|r| r.asset == original[0].asset)
            .unwrap();
        assert_eq!(a.image.pixels, b.image.pixels);
        assert!(std::sync::Arc::ptr_eq(&a.image, &b.image));
    }
    menu.wordmark_text = Some("Wørld".into());
    let unsupported = menu
        .prepare(&mut text, &view, &layout, [1920, 1080], 1., None)
        .unwrap();
    assert!(logo(&unsupported).is_empty());
    assert!(
        unsupported
            .resources
            .iter()
            .any(|r| r.asset.handle >= slither_wgpu_slice::bank::MENU_TEXT_HANDLE_BASE)
    );
}

#[test]
fn high_dpr_atlas_reassembles_the_exact_raster_without_seams_or_overlap() {
    use slither_client::menu_layout::MenuElement;
    use slither_wgpu_slice::{menu_specimen, wordmark};
    let dpr = 5.;
    let extent = [4800, 2700];
    let mut text = TextCache::menu();
    let view = menu_specimen::view(0).unwrap();
    let layout = menu_specimen::layout(&mut text, &view, extent, dpr).unwrap();
    let geometry = wordmark::layout(
        wordmark::DEFAULT_TEXT,
        layout.viewport_rect(MenuElement::Logo),
        960.,
        |_| unreachable!(),
    )
    .unwrap();
    let expected = wordmark::raster(&geometry, dpr, 1.).unwrap();
    let mut actual = vec![0; expected.pixels.len()];
    let p = Menu::default()
        .prepare(&mut text, &view, &layout, extent, dpr, None)
        .unwrap();
    let mut occupied = vec![false; expected.width as usize * expected.height as usize];
    for d in p.frame.draws.iter().filter(|d| {
        is_stroke_draw(
            d,
            layout.viewport_rect(slither_client::menu_layout::MenuElement::Logo),
        )
    }) {
        let source = d.source.unwrap();
        assert_eq!(d.destination.width * dpr, source.width);
        assert_eq!(d.destination.height * dpr, source.height);
        let x = (d.destination.x * dpr - (geometry.rect.x * dpr).round()).round() as u32;
        let y = (d.destination.y * dpr - (geometry.rect.y * dpr).round()).round() as u32;
        let resource = p.resources.iter().find(|r| r.asset == d.asset).unwrap();
        for row in 0..source.height as u32 {
            for col in 0..source.width as u32 {
                let at = ((y + row) * expected.width + x + col) as usize;
                assert!(!occupied[at]);
                occupied[at] = true;
                let src = ((source.y as u32 + row) * resource.image.width + source.x as u32 + col)
                    as usize
                    * 4;
                actual[at * 4..at * 4 + 4].copy_from_slice(&resource.image.pixels[src..src + 4]);
            }
        }
    }
    assert_eq!(actual, expected.pixels);
    assert!(p.frame.draws.len() <= 512);
}

#[test]
fn reveal_frames_inside_25ms_reuse_the_shared_raster() {
    use slither_wgpu_slice::menu_specimen;
    let mut text = TextCache::menu();
    let mut menu = Menu::default();
    let mut view = menu_specimen::view(0).unwrap();
    let layout = menu_specimen::layout(&mut text, &view, [1920, 1080], 1.).unwrap();
    let mut previous = None;
    let mut changes = Vec::new();
    for ms in 0..=100 {
        view.logo_reveal = f64::from(ms) * 0.00375 / 25.;
        let p = menu
            .prepare(&mut text, &view, &layout, [1920, 1080], 1., None)
            .unwrap();
        let logo = p.frame.draws.iter().find(|d| {
            is_stroke_draw(
                d,
                layout.viewport_rect(slither_client::menu_layout::MenuElement::Logo),
            )
        });
        if let Some(draw) = logo {
            let image = p
                .resources
                .iter()
                .find(|r| r.asset == draw.asset)
                .unwrap()
                .image
                .clone();
            if let Some((asset, old)) = &previous {
                if *asset == draw.asset {
                    assert!(std::sync::Arc::ptr_eq(old, &image));
                } else {
                    changes.push(ms);
                }
            } else {
                changes.push(ms);
            }
            previous = Some((draw.asset, image));
        } else {
            assert!(ms < 25);
        }
    }
    assert_eq!(changes, [25, 50, 75, 100]);
}

// Shared controls now have cropped sources too; identify the logo by its holder.
fn is_stroke_draw(
    d: &slither_wgpu_slice::frame::Draw,
    holder: slither_client::menu_layout::MenuRect,
) -> bool {
    d.source.is_some()
        && d.asset.handle < slither_wgpu_slice::bank::MENU_TEXT_HANDLE_BASE
        && d.destination.y >= holder.y - 1.
        && d.destination.y + d.destination.height <= holder.y + holder.height + 1.
}

#[test]
fn wordmark_raster_limits_fit_strokes_and_preserve_menu_and_errors() {
    use slither_client::menu_layout::MenuElement;
    use slither_wgpu_slice::menu_specimen;
    let mut text = TextCache::menu();
    let mut menu = Menu::default();
    let mut view = menu_specimen::view(0).unwrap();
    for dpr in [6., 8.] {
        let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
        let layout = menu_specimen::layout(&mut text, &view, extent, dpr).unwrap();
        let holder = layout.viewport_rect(MenuElement::Logo);
        let p = menu
            .prepare(&mut text, &view, &layout, extent, dpr, None)
            .unwrap_or_else(|e| panic!("DPR {dpr}: {e}"));
        assert!(p.frame.draws.iter().any(|d| is_stroke_draw(d, holder)));
        for draw in p.frame.draws.iter().filter(|d| is_stroke_draw(d, holder)) {
            let source = draw.source.unwrap();
            assert!(
                (draw.destination.width
                    * dpr.min(slither_wgpu_slice::menu::MAX_MENU_RASTER_DENSITY)
                    - source.width)
                    .abs()
                    < 1e-8
            );
            assert!(
                (draw.destination.height
                    * dpr.min(slither_wgpu_slice::menu::MAX_MENU_RASTER_DENSITY)
                    - source.height)
                    .abs()
                    < 1e-8
            );
        }
        assert!(
            p.frame
                .draws
                .iter()
                .any(|d| d.destination.y > holder.y + holder.height)
        );
        assert!(
            p.resources
                .iter()
                .all(|r| r.asset.width <= 2048 && r.asset.height <= 2048)
        );
        assert!(
            p.resources
                .iter()
                .map(|r| r.image.pixels.len())
                .sum::<usize>()
                <= slither_wgpu_slice::text::MENU_CACHE_BYTES
        );
        menu.wordmark_text = Some("x".repeat(129));
        assert_eq!(
            menu.prepare(&mut text, &view, &layout, extent, dpr, None)
                .err()
                .as_deref(),
            Some("Wordmark layout bounds")
        );
        menu.wordmark_text = None;
        view.logo_reveal = f64::NAN;
        assert_eq!(
            menu.prepare(&mut text, &view, &layout, extent, dpr, None)
                .err()
                .as_deref(),
            Some("Wordmark reveal bounds")
        );
        view.logo_reveal = 1.;
    }
}

#[test]
fn resized_stroke_cache_matches_fresh_pixels_and_snapped_positions() {
    use slither_client::menu_layout::MenuElement;
    use slither_wgpu_slice::menu_specimen;
    let snapshot = |p: &slither_wgpu_slice::menu::MenuFrame, holder| {
        p.frame
            .draws
            .iter()
            .filter(|d| is_stroke_draw(d, holder))
            .map(|d| {
                let r = p.resources.iter().find(|r| r.asset == d.asset).unwrap();
                (
                    d.destination,
                    d.source,
                    r.image.width,
                    r.image.height,
                    r.image.pixels.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    for dpr in [1., 1.25, 1.5, 2.] {
        for progress in [0.5, 1.] {
            let mut text = TextCache::menu();
            let mut view = menu_specimen::view(0).unwrap();
            view.logo_reveal = progress;
            let mut cached = Menu::default();
            for width in [1920, 1921, 1922, 1923] {
                let extent = [width, 1080];
                let layout = menu_specimen::layout(&mut text, &view, extent, dpr).unwrap();
                let holder = layout.viewport_rect(MenuElement::Logo);
                let actual = cached
                    .prepare(&mut text, &view, &layout, extent, dpr, None)
                    .unwrap();
                let fresh = Menu::default()
                    .prepare(&mut text, &view, &layout, extent, dpr, None)
                    .unwrap();
                let actual = snapshot(&actual, holder);
                assert!(!actual.is_empty());
                assert!(
                    actual == snapshot(&fresh, holder),
                    "width={width}, DPR={dpr}, progress={progress}"
                );
            }
        }
    }
}

#[test]
fn integer_translation_reuses_stroke_pixels() {
    use slither_client::menu_layout::MenuElement;
    use slither_wgpu_slice::menu_specimen;
    let mut text = TextCache::menu();
    let mut menu = Menu::default();
    let view = menu_specimen::view(0).unwrap();
    let mut pictures = Vec::new();
    for width in [1920, 1922] {
        let extent = [width, 1080];
        let layout = menu_specimen::layout(&mut text, &view, extent, 1.).unwrap();
        let p = menu
            .prepare(&mut text, &view, &layout, extent, 1., None)
            .unwrap();
        let draw = p
            .frame
            .draws
            .iter()
            .find(|d| is_stroke_draw(d, layout.viewport_rect(MenuElement::Logo)))
            .unwrap();
        let image = p
            .resources
            .iter()
            .find(|r| r.asset == draw.asset)
            .unwrap()
            .image
            .clone();
        pictures.push((draw.asset, draw.destination.x, image));
    }
    assert_eq!(pictures[0].0, pictures[1].0);
    assert_eq!(pictures[0].1 + 1., pictures[1].1);
    assert!(std::sync::Arc::ptr_eq(&pictures[0].2, &pictures[1].2));
}

#[test]
fn production_outgoing_spinner_and_wordmark_fit_at_dpr_five() {
    use slither_client::start_page::{MenuAction, MenuPhase};
    use slither_wgpu_slice::{menu_specimen, text::MENU_CACHE_BYTES};
    let mut state = StartMenuState::new(MenuLocale::English, true, MenuQuality::High);
    state.edit_nickname(&"Player".encode_utf16().collect::<Vec<_>>());
    for now in (0..=7200).step_by(16) {
        state.step(f64::from(now), false).unwrap();
    }
    state.action(MenuAction::Play, true);
    for now in (7216..=8192).step_by(16) {
        state.step(f64::from(now), false).unwrap();
    }
    assert_eq!(state.view(true).spinner_alpha, 1.);
    assert!(state.begin_outgoing());
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let extent = [8000, 5400];
    let mut peak = (0, 0);
    for frame in 0..=32 {
        state.step(f64::from(8192 + 16 * frame), false).unwrap();
        let view = state.view(true);
        if view.transition < 1. {
            assert_eq!(view.phase, MenuPhase::Outgoing);
        }
        assert!(view.spinner_visible && view.spinner_alpha > 0.);
        if frame > 0 {
            assert!(view.spinner_alpha < 1.);
        }
        let layout = menu_specimen::layout(&mut text, &view, extent, 5.).unwrap();
        let p = menu
            .prepare(&mut text, &view, &layout, extent, 5., None)
            .unwrap_or_else(|e| panic!("transition {}: {e}", view.transition));
        let bytes = p
            .resources
            .iter()
            .map(|r| r.image.pixels.len())
            .sum::<usize>();
        peak.0 = peak.0.max(bytes);
        peak.1 = peak.1.max(p.frame.draws.len());
        assert!(bytes <= MENU_CACHE_BYTES);
        assert!(p.frame.draws.len() <= 512);
        assert!(
            p.resources
                .iter()
                .all(|r| r.image.width <= 2048 && r.image.height <= 2048)
        );
    }
    eprintln!("outgoing peak: {} bytes, {} draws", peak.0, peak.1);
}

#[test]
fn many_case_boundaries_and_long_runs_fit_the_font_fallback() {
    use slither_wgpu_slice::{
        menu_specimen,
        text::{MENU_CACHE_BYTES, MENU_CACHE_RUNS},
    };
    for dpr in [6., 8.] {
        for value in ["Àb".repeat(60), format!("ø{}", "w".repeat(127))] {
            let mut menu = Menu::default();
            menu.wordmark_text = Some(value);
            let mut text = TextCache::menu();
            let view = menu_specimen::view(0).unwrap();
            let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
            let layout = menu_specimen::layout(&mut text, &view, extent, dpr).unwrap();
            let p = menu
                .prepare(&mut text, &view, &layout, extent, dpr, None)
                .unwrap();
            let material_slots = p
                .resources
                .iter()
                .filter(|r| r.asset.handle >> 60 == 7)
                .count();
            assert!(material_slots + text.cached_runs() <= MENU_CACHE_RUNS);
            assert!(
                p.resources
                    .iter()
                    .map(|r| r.image.pixels.len())
                    .sum::<usize>()
                    <= MENU_CACHE_BYTES
            );
            assert!(
                p.resources
                    .iter()
                    .all(|r| r.image.width <= 2048 && r.image.height <= 2048)
            );
            assert!(p.frame.draws.len() < 512);
            assert!(p.frame.draws.iter().any(|d| d.asset.handle >> 60 == 8));
        }
    }
}
