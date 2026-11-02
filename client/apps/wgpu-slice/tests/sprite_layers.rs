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
    assert_eq!(r.half_glow_extent(), None);
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
            draw(glow, 20.25, 20.75, 88., 0.08, BodyBlend::Lighter),
            draw(glow, 180.25, 20.75, 52., 0.095, BodyBlend::Lighter),
            draw(glow, 70.25, 70.75, 160., 0.02, BodyBlend::Lighter),
        ],
    );
    let resources = bank.resources(&picture).unwrap();
    r.sync_resources(&resources).unwrap();
    let first = offscreen::target(&r, [256, 256]);
    r.render(&picture, &first.create_view(&Default::default()))
        .unwrap();
    assert_eq!(r.food_outline_extent(), Some([256, 256]));
    assert_eq!(r.half_glow_extent(), Some([128, 128]));
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
    assert_eq!(r.half_glow_extent(), Some([192, 128]));
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
    assert_eq!(r.half_glow_extent(), None);
    assert_eq!(r.fine_glow_extent(), None);
    assert_eq!(r.food_glow_extent(), None);
    assert!(pollster::block_on(scope.pop()).is_none());
}
#[test]
#[ignore = "explicit finite AMD offscreen size/opacity sweep"]
fn glow_resolution_boundaries_stay_within_isolated_layer_budget() {
    let mut r = renderer(4);
    let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut bank = Bank::new().unwrap();
    for source in [0, 8, 16] {
        let glow = bank.food(0, [0., 0.]).glows[source].asset();
        let mut cases = Vec::new();
        let mut draws = Vec::new();
        for alpha in [0.02, 0.05, 0.050001, 0.095] {
            for phase in [0.125, 0.5, 0.875] {
                for size in [12., 15.99, 16., 20., 24., 31.99, 32., 40., 64., 160.] {
                    let index = draws.len();
                    let x = (index % 10) as f64 * 192. + 12. + phase;
                    let y = (index / 10) as f64 * 192. + 12. + phase;
                    draws.push(draw(glow, x, y, size, alpha, BodyBlend::Lighter));
                    cases.push((size, alpha, phase, x, y));
                }
            }
        }
        let extent = [1920, 2304];
        let candidate = frame(extent, draws);
        let resources = bank.resources(&candidate).unwrap();
        let mut reference = resources.clone();
        reference[0].food_glow = false;
        reference[0].asset.handle += 1_000_000;
        let mut control = candidate.clone();
        for d in &mut control.draws {
            d.asset = reference[0].asset;
        }
        let mut all = resources;
        all.extend(reference);
        r.sync_resources(&all).unwrap();
        let a = offscreen::target(&r, extent);
        let b = offscreen::target(&r, extent);
        r.render(&candidate, &a.create_view(&Default::default()))
            .unwrap();
        r.render(&control, &b.create_view(&Default::default()))
            .unwrap();
        let actual = offscreen::pixels(&r, &a).unwrap();
        let expected = offscreen::pixels(&r, &b).unwrap();
        let mut failures = Vec::new();
        for (size, alpha, phase, x, y) in cases {
            let mut differences = Vec::new();
            for yy in y.floor() as usize - 2..(y + size).ceil() as usize + 2 {
                for xx in x.floor() as usize - 2..(x + size).ceil() as usize + 2 {
                    let offset = (yy * extent[0] as usize + xx) * 4;
                    if actual[offset..offset + 3]
                        .iter()
                        .chain(&expected[offset..offset + 3])
                        .any(|c| *c != 0)
                    {
                        differences.extend(
                            (0..3).map(|c| actual[offset + c].abs_diff(expected[offset + c])),
                        );
                    }
                }
            }
            differences.sort_unstable();
            let max = *differences.last().unwrap_or(&0);
            let p95 = differences[(differences.len() * 95).div_ceil(100).saturating_sub(1)];
            if max > 5 || p95 > 2 {
                failures.push((size, alpha, phase, max, p95));
            }
        }
        println!("glow boundary source={source}, violations: {failures:?}");
        assert!(failures.is_empty(), "isolated glow budget violations");
    }
    assert!(pollster::block_on(scope.pop()).is_none());
}

#[test]
#[ignore = "explicit finite AMD offscreen source-mip check"]
fn reduced_glows_keep_the_original_world_source_mip_footprint() {
    let mut r = renderer(4);
    let scope = r.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let asset = BodyAsset {
        handle: 900_000,
        revision: 1,
        width: 64,
        height: 64,
        status: slither_client::presentation::BodyAssetStatus::Ready,
    };
    // Distinct authored mip colors make unintended extra minification visible.
    // All three world footprints magnify the red base, whereas their reduced
    // attachments alone would select the green mip without a footprint correction.
    let mut pixels = Vec::new();
    for level in 0..7 {
        let side = 64 >> level;
        let color = if level == 0 {
            [255, 0, 0, 255]
        } else {
            [0, 255, 0, 255]
        };
        for _ in 0..side * side {
            pixels.extend(color);
        }
    }
    let resource = Resource {
        asset,
        food_glow: true,
        food_outline: false,
        image: RgbaImage {
            width: 64,
            height: 64,
            pixels,
        }
        .into(),
    };
    let mut reference = resource.clone();
    reference.food_glow = false;
    reference.asset.handle += 1;
    r.sync_resources(&[resource, reference.clone()]).unwrap();
    let extent = [512, 512];
    let cases = [(16., 16., 64.), (120., 16., 128.), (16., 176., 256.)];
    for paired in [false, true] {
        let mut draws = Vec::new();
        for (x, y, size) in cases {
            draws.push(draw(asset, x, y, size, 0.1, BodyBlend::Lighter));
            if paired {
                draws.push(draw(asset, x, y, size, 0.05, BodyBlend::Lighter));
            }
        }
        let picture = frame(extent, draws);
        let mut control = picture.clone();
        for d in &mut control.draws {
            d.asset = reference.asset;
        }
        let a = offscreen::target(&r, extent);
        let b = offscreen::target(&r, extent);
        r.render(&picture, &a.create_view(&Default::default()))
            .unwrap();
        assert_eq!(r.half_glow_extent(), Some([256, 256]));
        assert_eq!(r.fine_glow_extent(), Some([128, 128]));
        assert_eq!(r.food_glow_extent(), Some([64, 64]));
        r.render(&control, &b.create_view(&Default::default()))
            .unwrap();
        let actual = offscreen::pixels(&r, &a).unwrap();
        let expected = offscreen::pixels(&r, &b).unwrap();
        for (x, y, size) in cases {
            let center = [(x + size / 2.) as usize, (y + size / 2.) as usize];
            for y in center[1] - 8..center[1] + 8 {
                for x in center[0] - 8..center[0] + 8 {
                    let offset = (y * extent[0] as usize + x) * 4;
                    assert_eq!(
                        &actual[offset..offset + 4],
                        &expected[offset..offset + 4],
                        "source footprint changed, paired={paired}, size={size}"
                    );
                }
            }
        }
    }
    assert!(pollster::block_on(scope.pop()).is_none());
}
