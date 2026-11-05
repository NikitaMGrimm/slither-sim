//! Authored synthetic controls and images; no game server, GPU or private art.
use slither_wgpu_slice::{
    cosmetics::{Cosmetics, Images, MenuArt, MenuManifest},
    frame::{Frame, MAX_DRAWS},
    menu::MenuFrame,
    render::PreparedFrame,
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
}

fn picture(draws: usize) -> PreparedFrame {
    let page = MenuFrame::world_crossfade([1280, 720], 1., 0.5).unwrap();
    PreparedFrame {
        world_draws: 0,
        #[cfg(feature = "frame-bench")]
        food_submission: None,
        frame: Frame {
            identity: 0,
            extent: [1280, 720],
            viewport_scale: 1.,
            tick: 0,
            draws: vec![page.backdrop.clone().unwrap(); draws],
            snakes: Vec::new(),
            food_batch: None,
        },
        overlay: None,
        resources: page.resources,
    }
}

#[test]
fn failed_page_admission_leaves_the_whole_picture_untouched() {
    let mut full = picture(MAX_DRAWS);
    let before = full.frame.clone();
    let page = MenuFrame::world_crossfade([1280, 720], 1., 0.5).unwrap();
    assert!(page.append_to(&mut full).is_err());
    assert_eq!(full.frame, before);
    let mut other = picture(1);
    let before = other.frame.clone();
    let page = MenuFrame::world_crossfade([640, 720], 1., 0.5).unwrap();
    assert!(page.append_to(&mut other).is_err());
    assert_eq!(other.frame, before);
}

#[test]
fn page_stays_below_the_world_and_keeps_its_colour_through_the_crossfade() {
    for world_alpha in [0., 0.5, 1.] {
        let page = MenuFrame::world_crossfade([3840, 2160], 3., world_alpha).unwrap();
        let backdrop = page.backdrop.clone().unwrap();
        let cover = page.world_cover.clone().unwrap();
        assert_eq!(page.resources[0].image.pixels, [22, 28, 34, 255]);
        assert_eq!(backdrop.effective_alpha, 1.);
        assert_eq!(cover.effective_alpha, 1. - world_alpha);
        assert_eq!(
            (backdrop.destination.width, backdrop.destination.height),
            (1280., 720.)
        );
    }
    let mut composed = picture(3);
    let world = composed.frame.draws.clone();
    let page = MenuFrame::world_crossfade([1280, 720], 1., 0.5).unwrap();
    let (backdrop, cover) = (
        page.backdrop.clone().unwrap(),
        page.world_cover.clone().unwrap(),
    );
    page.append_to(&mut composed).unwrap();
    assert_eq!(composed.frame.draws[0], backdrop);
    assert_eq!(&composed.frame.draws[1..4], world);
    assert_eq!(composed.frame.draws[4], cover);
    // A hidden login adds nothing below or above the world.
    let mut hidden = picture(3);
    MenuFrame::hidden([1280, 720], 1.)
        .unwrap()
        .append_to(&mut hidden)
        .unwrap();
    assert_eq!(hidden.frame.draws, world);
    assert!(MenuFrame::world_crossfade([1280, 720], 1., 1.5).is_err());
}
