//! Authored synthetic images only; no external art, sockets or GPU.
use slither_client::{presentation::*, state::Point};
use slither_wgpu_slice::{
    bank::Bank,
    cosmetics::{self, BackgroundArt, Cosmetics, Images},
    render,
};
use std::io::Cursor;

fn png(width: u32, height: u32) -> Vec<u8> {
    let image = image::RgbaImage::from_fn(width, height, |x, y| {
        image::Rgba([(x % 256) as u8, (y % 256) as u8, 120, 255])
    });
    let mut out = Cursor::new(Vec::new());
    image.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn local_roles_keep_pixels_dimensions_phase_and_layer_order() {
    let tile = png(599, 519);
    let backdrop = png(600, 550);
    let mut requested = Vec::new();
    let art = Cosmetics::load(
        |path| {
            requested.push(path.to_owned());
            match path {
                "s/bg54.jpg" => Ok(tile.clone()),
                "s/gbg.jpg" => Ok(backdrop.clone()),
                _ => Err("absent synthetic art".into()),
            }
        },
        None,
    )
    .unwrap();
    for role in BackgroundArt::ALL {
        assert_eq!(requested.iter().filter(|p| *p == role.path()).count(), 1);
    }
    let mut bank = Bank::new().unwrap();
    bank.cosmetics = Some(art);
    let tile = bank.background_art(BackgroundArt::HexTile).unwrap();
    let backdrop = bank.background_art(BackgroundArt::Backdrop).unwrap();
    assert_eq!((tile.width, tile.height), (599, 519));
    assert_eq!((backdrop.width, backdrop.height), (512, 512));
    let mut client = AnimatedClient::new(0.).unwrap();
    client.set_follows(false);
    client.set_zoom(0.8).unwrap();
    let high = QualityState::new(QualityMode::High, 0.).unwrap().snapshot();
    let first = render::prepare(&mut client, &mut bank, [960, 540], 1., 1, high).unwrap();
    assert_eq!(first.frame.draws[0].asset, backdrop);
    assert_eq!(first.frame.draws[0].effective_alpha, 0.3);
    assert_eq!(first.frame.draws[1].asset, tile);
    assert_eq!(first.frame.draws[1].effective_alpha, 1.);
    let pixels = &first
        .resources
        .iter()
        .find(|r| r.asset == tile)
        .unwrap()
        .image;
    assert_eq!(
        pixels.pixels,
        cosmetics::decode(&png(599, 519)).unwrap().pixels
    );
    let pixels = &first
        .resources
        .iter()
        .find(|r| r.asset == backdrop)
        .unwrap()
        .image;
    assert_eq!(
        pixels.pixels,
        cosmetics::decode(&png(512, 512)).unwrap().pixels
    );
    let previous = client.camera().position();
    client
        .set_camera_position(Point {
            x: previous.x + 17.,
            y: previous.y - 11.,
        })
        .unwrap();
    let second = render::prepare(&mut client, &mut bank, [1920, 1080], 1., 2, high).unwrap();
    let phase = client.background().phase();
    assert_eq!(phase.x, 1. - 17. / 599.);
    assert_eq!(phase.y, 11. / 519.);
    let uv = second.frame.draws[1].repeat_uv.unwrap();
    assert_eq!(uv.x, -960. / client.camera().zoom() / 599. - phase.x);
    assert_eq!(uv.height, 1080. / client.camera().zoom() / 519.);
    assert_eq!(second.frame.draws[1].asset, tile);
    let mut low = QualityState::new(QualityMode::Low, 0.).unwrap();
    low.begin_connection();
    let third = render::prepare(&mut client, &mut bank, [960, 540], 1., 3, low.snapshot()).unwrap();
    assert_eq!(third.frame.draws[0].asset, tile);
    assert!(!third.resources.iter().any(|r| r.asset == backdrop));
}

#[test]
fn missing_invalid_and_over_budget_roles_are_absent_independently() {
    let mut bank = Bank::new().unwrap();
    for valid in BackgroundArt::ALL {
        bank.cosmetics = Some(
            Cosmetics::load(
                |path| {
                    if path == valid.path() {
                        Ok(png(32, 24))
                    } else if path == BackgroundArt::HexTile.path() {
                        Ok(b"invalid".to_vec())
                    } else {
                        Err("absent synthetic art".into())
                    }
                },
                None,
            )
            .unwrap(),
        );
        for role in BackgroundArt::ALL {
            let selected = bank.background_art(role);
            if role == valid {
                let selected = selected.unwrap();
                assert_eq!(
                    (selected.width, selected.height),
                    if role == BackgroundArt::Backdrop {
                        (512, 512)
                    } else {
                        (32, 24)
                    }
                );
            } else {
                assert_eq!(selected, None);
            }
        }
        let mut client = AnimatedClient::new(0.).unwrap();
        for mode in [QualityMode::High, QualityMode::Low] {
            let mut quality = QualityState::new(mode, 0.).unwrap();
            quality.begin_connection();
            let picture = render::prepare(
                &mut client,
                &mut bank,
                [960, 540],
                1.,
                1,
                quality.snapshot(),
            )
            .unwrap();
            let visible = valid == BackgroundArt::HexTile || mode == QualityMode::High;
            assert_eq!(picture.frame.draws.len(), usize::from(visible));
            assert_eq!(picture.resources.len(), usize::from(visible));
            if visible {
                assert_eq!(
                    picture.frame.draws[0].asset,
                    bank.background_art(valid).unwrap()
                );
            }
        }
    }
    let mut images = Images::default();
    let full = png(1024, 1024);
    for path in [
        "s/a_cap.png",
        "s/a_crown.png",
        "s/a_cape.png",
        "s/a_bear.png",
    ] {
        images.add(path, &full).unwrap();
    }
    assert_eq!(images.pixel_bytes(), cosmetics::COSMETIC_PIXEL_BUDGET);
    for role in BackgroundArt::ALL {
        assert!(
            images
                .add(role.path(), &png(599, 519))
                .unwrap_err()
                .contains("pixel budget")
        );
    }
    bank.cosmetics = Some(Cosmetics::from_images(images, None).unwrap());
    assert_eq!(bank.background_art(BackgroundArt::HexTile), None);
    assert_eq!(bank.background_art(BackgroundArt::Backdrop), None);
    let mut images = Images::default();
    for role in BackgroundArt::ALL {
        assert!(images.add(role.path(), &png(1025, 1)).is_err());
        assert!(
            images
                .add(role.path(), &vec![0; cosmetics::MAX_ENCODED_BYTES + 1])
                .is_err()
        );
    }
}

#[test]
fn no_art_has_no_background_draws_or_resources_at_any_quality_or_size() {
    let mut bank = Bank::new().unwrap();
    let mut client = AnimatedClient::new(0.).unwrap();
    for mode in [QualityMode::High, QualityMode::Low] {
        let mut quality = QualityState::new(mode, 0.).unwrap();
        quality.begin_connection();
        for extent in [[960, 540], [1920, 1080]] {
            let picture =
                render::prepare(&mut client, &mut bank, extent, 1., 1, quality.snapshot()).unwrap();
            assert!(picture.frame.draws.is_empty());
            assert!(picture.resources.is_empty());
        }
    }
}
