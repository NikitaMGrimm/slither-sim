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
    let mut bytes = Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(size, size, image::Rgba(color))
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
            let pad = (12. * material_scale).ceil() / material_scale;
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
