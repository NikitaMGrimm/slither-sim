//! Authored offscreen regressions for additive pairs and soft food layers.
#![cfg(not(target_arch = "wasm32"))]
#[allow(dead_code)]
#[path = "../examples/support/offscreen.rs"]
mod offscreen;
use slither_client::presentation::{BodyAsset, BodyBlend, BodyRect};
use slither_wgpu_slice::{
    bank::Bank,
    frame::{Draw, Frame},
    gpu::{Renderer, instance},
    materials::RgbaImage,
    scene::Resource,
};
fn renderer(samples: u32) -> Renderer {
    let adapter = pollster::block_on(instance().enumerate_adapters(wgpu::Backends::DX12))
        .into_iter()
        .find(|a| a.get_info().vendor == 0x1002)
        .expect("explicit AMD/DX12 offscreen check");
    let r = pollster::block_on(Renderer::from_adapter_with_max_samples(
        adapter,
        wgpu::TextureFormat::Rgba8Unorm,
        samples,
    ))
    .unwrap();
    println!(
        "sprite regression {:?}, samples={}",
        r.adapter_info, r.sample_count
    );
    r
}
fn draw(asset: BodyAsset, x: f64, y: f64, size: f64, alpha: f64, blend: BodyBlend) -> Draw {
    Draw {
        asset,
        corners: None,
        source: None,
        repeat_uv: None,
        black_effect: false,
        destination: BodyRect {
            x,
            y,
            width: size,
            height: size,
        },
        translation: [0., 0.],
        rotation: 0.,
        effective_alpha: alpha,
        blend,
    }
}
fn frame(extent: [u32; 2], draws: Vec<Draw>) -> Frame {
    Frame {
        identity: 1,
        tick: 0,
        extent,
        viewport_scale: 1.,
        snakes: Vec::new(),
        draws,
    }
}
#[test]
#[ignore = "explicit finite AMD offscreen check"]
fn additive_pairs_bound_the_two_original_rounding_steps() {
    for samples in [1, 4] {
        let mut r = renderer(samples);
        let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let colors = [
            [255, 127, 3, 255],
            [1, 17, 213, 255],
            [17, 7, 3, 31],
            [0, 0, 0, 255],
            [17, 17, 17, 255],
            [213, 213, 213, 255],
        ];
        let resources: Vec<_> = colors
            .into_iter()
            .enumerate()
            .map(|(i, c)| Resource {
                food_glow: false,
                food_outline: false,
                asset: BodyAsset {
                    handle: i as u64 + 1,
                    revision: 1,
                    width: 1,
                    height: 1,
                    status: slither_client::presentation::BodyAssetStatus::Ready,
                },
                image: RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: c.to_vec(),
                }
                .into(),
            })
            .collect();
        r.sync_resources(&resources).unwrap();
        let mut paired = Vec::new();
        let mut split = Vec::new();
        let mut count = 0;
        for base in 3..6 {
            for source in 0..3 {
                for a in 0..=256 {
                    for b in [
                        0.,
                        0.49 / 255.,
                        0.5 / 255.,
                        0.51 / 255.,
                        0.01,
                        0.05,
                        0.5,
                        1.,
                    ] {
                        let x = (count % 128) as f64 * 4. + 0.125;
                        let y = (count / 128) as f64 * 4. + 0.375;
                        let background =
                            draw(resources[base].asset, x, y, 2.25, 1., BodyBlend::SourceOver);
                        let first = draw(
                            resources[source].asset,
                            x,
                            y,
                            2.25,
                            f64::from(a) / 256.,
                            BodyBlend::Lighter,
                        );
                        let second =
                            draw(resources[source].asset, x, y, 2.25, b, BodyBlend::Lighter);
                        let noop =
                            draw(resources[base].asset, x, y, 2.25, 0., BodyBlend::SourceOver);
                        paired.extend([background.clone(), first.clone(), second.clone()]);
                        split.extend([background, first, noop, second]);
                        count += 1;
                    }
                }
            }
        }
        let extent = [512, (count as u32).div_ceil(128) * 4];
        let a = offscreen::target(&r, extent);
        let b = offscreen::target(&r, extent);
        r.render(&frame(extent, paired), &a.create_view(&Default::default()))
            .unwrap();
        r.render(&frame(extent, split), &b.create_view(&Default::default()))
            .unwrap();
        let a = offscreen::pixels(&r, &a).unwrap();
        let b = offscreen::pixels(&r, &b).unwrap();
        let max = a.iter().zip(&b).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
        println!("paired cases={count}, max component difference={max}");
        // Each of the two original 8-bit additions can resolve a half-byte
        // differently from prequantizing its source. Bound both decisions;
        // two bytes is below the existing 0.01/0.02 normalized visual budgets.
        assert!(max <= 2, "paired additive rounding drift {max}");
        assert!(pollster::block_on(scope.pop()).is_none());
    }
}
#[test]
#[ignore = "explicit finite AMD offscreen check"]
fn food_layers_keep_small_glows_and_body_rasters_and_reuse_resized_targets() {
    let mut r = renderer(4);
    let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut bank = Bank::new().unwrap();
    let food = bank.food(0, [0., 0.]);
    let body = food.bodies[16].asset();
    let outline = food.outlines[16].asset();
    let glow = food.glows[16].asset();
    let small = frame(
        [256, 256],
        vec![draw(glow, 12.25, 15.75, 2., 0.08, BodyBlend::Lighter)],
    );
    let resources = bank.resources(&small).unwrap();
    r.sync_resources(&resources).unwrap();
    let a = offscreen::target(&r, [256, 256]);
    r.render(&small, &a.create_view(&Default::default()))
        .unwrap();
    assert_eq!(r.food_glow_extent(), None);
    assert_eq!(r.fine_glow_extent(), None);
    let small_pixels = offscreen::pixels(&r, &a).unwrap();
    let mut reference = resources.clone();
    reference[0].food_glow = false;
    reference[0].asset.handle += 1_000_000;
    let mut small_reference = small.clone();
    small_reference.draws[0].asset = reference[0].asset;
    r.sync_resources(&reference).unwrap();
    r.render(&small_reference, &a.create_view(&Default::default()))
        .unwrap();
    assert_eq!(small_pixels, offscreen::pixels(&r, &a).unwrap());
    let picture = frame(
        [256, 256],
        vec![
            draw(outline, 32.25, 32.75, 112., 0.8, BodyBlend::SourceOver),
            draw(body, 45.25, 45.75, 86., 1., BodyBlend::Lighter),
            draw(glow, 20.25, 20.75, 160., 0.08, BodyBlend::Lighter),
            draw(glow, 70.25, 70.75, 160., 0.02, BodyBlend::Lighter),
        ],
    );
    let resources = bank.resources(&picture).unwrap();
    r.sync_resources(&resources).unwrap();
    let first = offscreen::target(&r, [256, 256]);
    r.render(&picture, &first.create_view(&Default::default()))
        .unwrap();
    assert_eq!(r.food_outline_extent(), Some([256, 256]));
    assert_eq!(r.fine_glow_extent(), Some([64, 64]));
    assert_eq!(r.food_glow_extent(), Some([32, 32]));
    let larger = offscreen::target(&r, [384, 256]);
    r.render_world_at(
        &picture,
        &larger.create_view(&Default::default()),
        [384, 256],
        4,
    )
    .unwrap();
    assert_eq!(r.food_outline_extent(), Some([384, 256]));
    assert_eq!(r.fine_glow_extent(), Some([96, 64]));
    assert_eq!(r.food_glow_extent(), Some([48, 32]));
    r.render(&picture, &a.create_view(&Default::default()))
        .unwrap();
    let actual = offscreen::pixels(&r, &a).unwrap();
    assert_eq!(actual, offscreen::pixels(&r, &first).unwrap());
    let reference: Vec<_> = resources
        .iter()
        .cloned()
        .map(|mut r| {
            r.food_glow = false;
            r.food_outline = false;
            r.asset.handle += 1_000_000;
            r
        })
        .collect();
    let mut control = picture.clone();
    for d in &mut control.draws {
        d.asset.handle += 1_000_000;
    }
    r.sync_resources(&reference).unwrap();
    r.render(&control, &a.create_view(&Default::default()))
        .unwrap();
    let expected = offscreen::pixels(&r, &a).unwrap();
    let max = actual
        .iter()
        .zip(&expected)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    println!("isolated layer max component difference={max}");
    assert!(max <= 5, "isolated soft-layer drift {max}");
    assert_eq!(r.food_outline_extent(), None);
    assert_eq!(r.fine_glow_extent(), None);
    assert_eq!(r.food_glow_extent(), None);
    assert!(pollster::block_on(scope.pop()).is_none());
}
