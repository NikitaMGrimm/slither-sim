//! Original synthetic runs; Windows tests rasterize the installed system Arial.
use slither_wgpu_slice::text::*;

fn run(text: &str) -> TextRun<'_> {
    TextRun {
        text,
        style: TextStyle {
            family: slither_client::presentation::TextFamily::Arial,
            slant: slither_client::presentation::TextSlant::Normal,
            size: 15.,
            weight: Weight::Normal,
            color: [132, 182, 255],
        },
        position: [200., 100.],
        align: Align::Center,
        baseline: Baseline::Middle,
        alpha: 0.5,
    }
}
#[test]
fn rejects_invalid_runs_before_platform_work() {
    let mut cache = TextCache::default();
    assert!(cache.draw(run(""), 1.).unwrap().is_empty());
    for scale in [0., -1., f64::NAN, f64::INFINITY, 100.] {
        assert!(cache.draw(run("Name"), scale).unwrap().is_empty());
    }
    let mut bad = run("Name");
    bad.alpha = f64::NAN;
    assert!(cache.draw(bad, 1.).unwrap().is_empty());
    assert!(cache.draw(run(&"x".repeat(1025)), 1.).unwrap().is_empty());
    assert_eq!(cache.cached_runs(), 0);
}

#[cfg(windows)]
#[test]
fn native_arial_preserves_style_alpha_scale_and_evicted_frame_pixels() {
    use slither_wgpu_slice::{bank::Bank, frame::Frame};
    let mut cache = TextCache::default();
    let (first, resource) = cache.draw(run("AV café ÿ"), 1.).unwrap().remove(0);
    assert!(
        resource
            .image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0)
    );
    for p in resource.image.pixels.as_chunks::<4>().0 {
        for c in 0..3 {
            assert!(p[c] <= p[3]);
        }
    }
    let mut moved = run("AV café ÿ");
    moved.alpha = 0.12;
    moved.position = [201.25, 99.5];
    let (second, _) = cache.draw(moved, 1.).unwrap().remove(0);
    assert_eq!(first.asset, second.asset);
    assert_eq!(first.destination, second.destination);
    assert_eq!(second.effective_alpha, 0.12);
    assert_eq!(second.translation, [201.25, 99.5]);
    assert_eq!(cache.cached_runs(), 1);
    let (scaled, _) = cache.draw(run("AV café ÿ"), 2.).unwrap().remove(0);
    assert_ne!(scaled.asset, first.asset);
    assert!((scaled.destination.width - first.destination.width).abs() < 2.);
    let mut bold = run("AV café ÿ");
    bold.style.weight = Weight::Bold;
    assert_ne!(cache.draw(bold, 1.).unwrap().remove(0).0.asset, first.asset);
    // A queued frame owns the old pixels even when the bounded LRU drops them.
    let mut bank = Bank::new().unwrap();
    bank.stage_text(1, vec![resource]).unwrap();
    for i in 0..300 {
        cache
            .draw(run(&format!("{i:03} WWWWWWWWWWWWWWWWWWWWWWWW")), 1.)
            .unwrap();
    }
    assert!(cache.cached_runs() <= CACHE_RUNS);
    assert!(cache.cached_bytes() <= CACHE_BYTES);
    let frame = Frame {
        snakes: Vec::new(),
        identity: 1,
        extent: [960, 540],
        viewport_scale: 1.,
        tick: 0,
        draws: vec![first],
    };
    let resources = bank.resources(&frame).unwrap();
    frame
        .quads(
            &resources
                .iter()
                .map(|r| (r.asset.into(), r.asset))
                .collect(),
        )
        .unwrap();
    assert_ne!(
        cache.draw(run("AV café ÿ"), 1.).unwrap().remove(0).0.asset,
        frame.draws[0].asset
    );
    assert!(
        cache
            .draw(
                TextRun {
                    style: TextStyle {
                        family: slither_client::presentation::TextFamily::Arial,
                        slant: slither_client::presentation::TextSlant::Normal,
                        size: 128.,
                        weight: Weight::Normal,
                        color: [255; 3]
                    },
                    ..run(&"W".repeat(100))
                },
                1.
            )
            .unwrap()
            .is_empty()
    );
    let long_name = "ÿ".repeat(255);
    let tiles = cache.draw(run(&long_name), 2.).unwrap();
    assert!(tiles.len() > 1);
    for (_, resource) in &tiles {
        assert!(resource.asset.width <= 512);
    }
    for pair in tiles.windows(2) {
        let a = &pair[0].1.image;
        let b = &pair[1].1.image;
        for (left, right) in a
            .pixels
            .chunks_exact(a.width as usize * 4)
            .zip(b.pixels.chunks_exact(b.width as usize * 4))
        {
            assert_eq!(&left[left.len() - 8..], &right[..8]);
        }
        assert_eq!(
            pair[0].0.destination.x + pair[0].0.destination.width,
            pair[1].0.destination.x
        );
    }
}

#[test]
fn hud_raster_errors_are_explicit_instead_of_omitting_fields() {
    let mut cache = TextCache::default();
    assert!(cache.draw_hud(0, run("HUD"), 100., None).is_err());
    assert_eq!(cache.cached_runs(), 0);
}

#[cfg(windows)]
#[test]
fn native_colour_separates_coverage_and_invalidates_hud_slot_pixels() {
    let mut cache = TextCache::default();
    let coloured = |color| TextRun {
        style: TextStyle {
            color,
            ..run("Orbit Lane").style
        },
        ..run("Orbit Lane")
    };
    let (dark_draw, dark) = cache.draw(coloured([0; 3]), 1.).unwrap().remove(0);
    let (light_draw, light) = cache.draw(coloured([255; 3]), 1.).unwrap().remove(0);
    assert_ne!(dark_draw.asset, light_draw.asset);
    assert_eq!(dark_draw.destination, light_draw.destination);
    assert_eq!(dark_draw.translation, light_draw.translation);
    let dark_alpha: Vec<_> = dark
        .image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3])
        .collect();
    let light_alpha: Vec<_> = light
        .image
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3])
        .collect();
    assert_ne!(dark_alpha, light_alpha);
    assert!(
        dark_alpha
            .iter()
            .zip(&light_alpha)
            .all(|(dark, light)| dark <= light)
    );
    assert!(
        dark_alpha.iter().map(|a| u64::from(*a)).sum::<u64>()
            < light_alpha.iter().map(|a| u64::from(*a)).sum::<u64>()
    );
    assert_eq!(cache.cached_runs(), 2);
    for (color, asset) in [([0; 3], dark_draw.asset), ([255; 3], light_draw.asset)] {
        assert_eq!(cache.draw(coloured(color), 1.).unwrap()[0].0.asset, asset);
    }

    let (_, old) = cache.draw_hud(0, coloured([0; 3]), 1., None).unwrap();
    let (_, new) = cache.draw_hud(0, coloured([255; 3]), 1., None).unwrap();
    assert_ne!(old[0].0.asset, new[0].0.asset);
    assert_eq!(old[0].1.image.pixels, dark.image.pixels);
    assert_eq!(new[0].1.image.pixels, light.image.pixels);
    assert_eq!(cache.cached_runs(), 3);
}
