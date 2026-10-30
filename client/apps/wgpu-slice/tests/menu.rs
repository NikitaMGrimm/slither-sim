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
    images.manifest(b"menu-art-v1\nlogo:0 2 menu/first.png\nlogo:1 3 menu/second.png\nlogo:2 4 menu/third.png\nskin 1 menu/skin.png\n").unwrap();
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
        "\u{feff}menu-art-v1\nlogo:0 1 menu/logo.png",
        "menu-art-v1\nlogo:32 1 menu/a.png",
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
    assert!(images.add("menu/logo.png", b"corrupt").is_err());
    assert!(images.add("../menu/logo.png", &png(1, [0; 4])).is_err());
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
fn two_logo_frames_and_failed_replacement_preserve_prepared_resources() {
    let mut art = art();
    let first = art.menu_art(MenuArt::Logo(0)).unwrap().clone();
    art.replace_menu_art(MenuArt::Logo(1), &png(8, [80, 20, 150, 255]))
        .unwrap();
    assert!(art.replace_menu_art(MenuArt::Logo(2), b"corrupt").is_err());
    assert!(art.menu_art(MenuArt::Logo(0)).is_some());
    art.replace_menu_art(MenuArt::Logo(2), &png(8, [150, 20, 80, 255]))
        .unwrap();
    assert!(art.menu_art(MenuArt::Logo(0)).is_none());
    assert!(art.menu_art(MenuArt::Logo(1)).is_some());
    assert!(art.menu_art(MenuArt::Logo(2)).is_some());
    art.replace_menu_art(MenuArt::Logo(0), &png(8, [180, 30, 80, 255]))
        .unwrap();
    assert!(art.menu_art(MenuArt::Logo(1)).is_none());
    let replaced = art.menu_art(MenuArt::Logo(0)).unwrap();
    assert_eq!(first.asset.handle, replaced.asset.handle);
    assert!(replaced.asset.revision > first.asset.revision);
    assert_eq!(&first.image.pixels[..4], &[20, 180, 60, 255]);
    assert!(
        art.replace_menu_art(MenuArt::Logo(3), &png(8, [0; 4]))
            .is_err()
    );
}
#[test]
fn menu_art_shares_existing_cosmetic_pixel_allowance() {
    let mut images = Images::default();
    let bytes = png(1024, [1, 2, 3, 255]);
    for path in ["s/a_oakley.png", "s/a_cap.png", "s/a_crown.png"] {
        images.add(path, &bytes).unwrap();
    }
    images.add("menu/logo.png", &bytes).unwrap();
    for path in ["menu/skin.png", "menu/builder.png", "menu/cosmetic.png"] {
        images.add(path, &bytes).unwrap();
    }
    assert!(images.add("s/a_visor.png", &png(1, [0; 4])).is_err());
    // The failed insert cannot consume the last frame's pixels.
    let art = Cosmetics::from_images(images, None).unwrap();
    assert!(art.menu_art(MenuArt::Logo(0)).is_some());
    assert!(art.menu_art(MenuArt::Skin).is_some());
    assert_eq!(art.menu_art(MenuArt::Logo(0)).unwrap().asset.width, 512);
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
            .replace_menu_art(MenuArt::Logo(0), &png(1, [255; 4]))
            .is_err()
    );
    assert!(cosmetics.menu_art(MenuArt::Logo(0)).is_none());
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
            0,
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
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None, 0)
        .unwrap();
    let draw = p.frame.draws[0].clone();
    let make = |count| slither_wgpu_slice::menu::MenuFrame {
        backdrop: None,
        frame: slither_wgpu_slice::frame::Frame {
            draws: vec![draw.clone(); count],
            ..p.frame.clone()
        },
        resources: p.resources.clone(),
    };
    let mut empty = slither_wgpu_slice::render::PreparedFrame {
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
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., Some(&art), 0)
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
                    0,
                )
                .unwrap();
            assert!(p.frame.draws.len() < 512);
            let nick =
                layout().viewport_rect(slither_client::menu_layout::MenuElement::NicknameHolder);
            let material_scale = dpr.min(1.5);
            let pad = (40. * material_scale).ceil() / material_scale;
            let shape = p.frame.draws[0].destination;
            assert!((shape.x + pad - nick.x).abs() <= 2.);
            assert!((shape.y + pad - nick.y).abs() <= 2.);
            assert!((shape.x + shape.width - pad - nick.x - nick.width).abs() <= 2.);
            assert!((shape.y + shape.height - pad - nick.y - nick.height).abs() <= 2.);
            assert!(
                p.resources
                    .iter()
                    .all(|r| r.asset.width <= 512 && r.asset.height <= 512)
            );
            assert!(
                p.resources
                    .iter()
                    .flat_map(|r| r.image.pixels.as_chunks::<4>().0)
                    .all(|p| p[..3].iter().all(|v| *v <= p[3]))
            );
        }
    }
    art.replace_menu_art(MenuArt::Logo(0), &png(8, [50, 80, 150, 255]))
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
        0,
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
fn logo_paints_its_aspect_ratio_without_changing_the_flow_holder() {
    use slither_client::menu_layout::MenuElement as E;
    let mut images = Images::default();
    images
        .add("menu/logo.png", &png_rect(450, 135, [80, 200, 100, 255]))
        .unwrap();
    let art = Cosmetics::from_images(images, None).unwrap();
    let asset = art.menu_art(MenuArt::Logo(0)).unwrap().asset;
    let view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    let layout = layout();
    let holder = layout.viewport_rect(E::Logo);
    let p = Menu::default()
        .prepare(
            &mut TextCache::menu(),
            &view,
            &layout,
            [1280, 720],
            1.,
            Some(&art),
            0,
        )
        .unwrap();
    let logo = p
        .frame
        .draws
        .iter()
        .find(|d| d.asset == asset)
        .unwrap()
        .destination;
    assert_eq!(
        (logo.x, logo.y, logo.width),
        (holder.x, holder.y, holder.width)
    );
    assert_eq!(logo.height, logo.width * 135. / 450.);
    assert_eq!(holder.height, 115.);
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
                0,
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
fn disabled_play_dims_the_normal_surface_and_label_together() {
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    view.login_alpha = 0.8;
    view.play_return_alpha = 0.6;
    let enabled = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None, 0)
        .unwrap();
    view.disabled = true;
    let disabled = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None, 0)
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
    // The base capsule and white Play label share the same 0.38 multiplier.
    assert_eq!(changed.len(), 2);
    for (a, b) in changed {
        assert_eq!(a.asset, b.asset);
        assert_eq!(a.destination, b.destination);
        assert!((b.effective_alpha / a.effective_alpha - 0.38).abs() < 1e-12);
    }
}

#[test]
fn explicit_backdrop_is_below_all_world_draws_and_absent_for_hidden_login() {
    use slither_wgpu_slice::render::PreparedFrame;
    let mut menu = Menu::default();
    let mut text = TextCache::menu();
    let mut view = StartMenuState::new(MenuLocale::English, false, MenuQuality::High).view(true);
    let base = menu
        .prepare(&mut text, &view, &layout(), [1280, 720], 1., None, 0)
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
            .prepare(&mut text, &view, &layout(), [1280, 720], 1., None, 0)
            .unwrap();
        assert_eq!(prepared.backdrop.is_some(), visible);
        let backdrop = prepared.backdrop.clone();
        let controls = prepared.frame.draws.clone();
        let mut picture = PreparedFrame {
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
