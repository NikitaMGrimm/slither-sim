//! Authored synthetic raster checks, without windows, transport or external art.
#[allow(dead_code)]
#[path = "../examples/support/offscreen.rs"]
mod offscreen;
use slither_client::presentation::{BodyBlend, BodyRect};
use slither_wgpu_slice::{
    bank::Bank,
    frame::{Draw, Frame},
    gpu::{Renderer, instance},
};

#[test]
fn refinement_keeps_logical_food_size_and_cached_image_ownership() {
    let mut bank = Bank::new().unwrap();
    let source = bank.food(0, [0., 0.]).bodies[0];
    let asset = source.asset();
    assert_eq!([asset.width, asset.height], [4, 4]);
    let frame = frame(asset, 96., 16.);
    let a = bank.resources(&frame).unwrap();
    let b = bank.resources(&frame).unwrap();
    assert_eq!(a[0].asset, asset);
    assert!(std::sync::Arc::ptr_eq(&a[0].image, &b[0].image));
    assert_eq!([a[0].image.width, a[0].image.height], [16, 16]);
    // No constant-ink 2x2/1x1 body mip: it would fill a tiny square.
    assert_eq!(a[0].image.pixels.len(), (256 + 64 + 16) * 4);
}

fn frame(asset: slither_client::presentation::BodyAsset, size: f64, x: f64) -> Frame {
    Frame {
        identity: 1,
        tick: 1,
        extent: [128, 128],
        viewport_scale: 1.,
        snakes: Vec::new(),
        draws: vec![Draw {
            asset,
            source: None,
            repeat_uv: None,
            black_effect: false,
            destination: BodyRect {
                x,
                y: x,
                width: size,
                height: size,
            },
            translation: [0., 0.],
            rotation: 0.,
            effective_alpha: 1.,
            blend: BodyBlend::Lighter,
            corners: None,
        }],
    }
}

#[test]
#[ignore = "explicit finite offscreen GPU check"]
fn single_sample_food_rim_is_round_and_minified_energy_is_phase_stable() {
    pollster::block_on(async {
        let adapter = instance()
            .request_adapter(&Default::default())
            .await
            .unwrap();
        let mut r =
            Renderer::from_adapter_with_max_samples(adapter, wgpu::TextureFormat::Rgba8Unorm, 1)
                .await
                .unwrap();
        let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut bank = Bank::new().unwrap();
        let asset = bank.food(0, [0., 0.]).bodies[0].asset();
        let large = frame(asset, 96., 16.);
        r.sync_resources(&bank.resources(&large).unwrap()).unwrap();
        let target = offscreen::target(&r, [128, 128]);
        r.render(&large, &target.create_view(&Default::default()))
            .unwrap();
        let pixels = offscreen::pixels(&r, &target).unwrap();
        let sample = |x: f64, y: f64| {
            let (x, y) = (x - 0.5, y - 0.5);
            let mut value = 0.;
            for dy in 0..2 {
                for dx in 0..2 {
                    let weight = if dx == 0 { 1. - x.fract() } else { x.fract() }
                        * if dy == 0 { 1. - y.fract() } else { y.fract() };
                    value += f64::from(
                        pixels[((y.floor() as usize + dy) * 128 + x.floor() as usize + dx) * 4],
                    ) * weight;
                }
            }
            value
        };
        let (mut min, mut max) = (255_f64, 0_f64);
        for i in 0..360 {
            let angle = f64::from(i) * std::f64::consts::TAU / 360.;
            let value = sample(64. + 43.2 * angle.cos(), 64. + 43.2 * angle.sin());
            min = min.min(value);
            max = max.max(value);
        }
        assert!(max - min < 16., "magnified rim variation {min}..{max}");
        let mut energy = Vec::new();
        for i in 0..8 {
            r.render(
                &frame(asset, 4., 62. + f64::from(i) / 8.),
                &target.create_view(&Default::default()),
            )
            .unwrap();
            energy.push(
                offscreen::pixels(&r, &target)
                    .unwrap()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| u64::from(p[0]))
                    .sum::<u64>(),
            );
        }
        let min = *energy.iter().min().unwrap() as f64;
        let max = *energy.iter().max().unwrap() as f64;
        assert!((max - min) / max < 0.15, "minified energy {energy:?}");
        assert!(scope.pop().await.is_none());
    });
}
