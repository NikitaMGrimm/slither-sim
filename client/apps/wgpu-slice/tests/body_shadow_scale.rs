//! Authored CPU-only native viewport/DPR regression, with no artwork or GPU.
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::{presentation::*, settings::WorldResolution};
use slither_wgpu_slice::{
    render,
    world_resolution::{WorldTargetPlan, WorldWindow},
};

fn shadow_count(diameter_in_geometry: f64, logical_scale: f64, factor: f64) -> usize {
    let samples: Vec<_> = (0..12)
        .map(|i| BodyPaintSample {
            ordinal: i,
            x: i as f32 * 7.,
            y: 0.,
            tangent: 0.,
            flag: 2,
        })
        .collect();
    let assets: Vec<_> = (0..4)
        .map(|handle| BodyAsset {
            handle,
            revision: 1,
            width: 64,
            height: 64,
            status: BodyAssetStatus::Ready,
        })
        .collect();
    let use_asset = |asset| BodyAssetUse {
        asset,
        source_rect: None,
    };
    let variants = [use_asset(0)];
    let palettes = [BodyPalette {
        variants: &variants,
        boost_glow: Some(use_asset(0)),
    }];
    let plan = plan_standard_body_with_zoom_and_storage(
        BodyPaintInput {
            geometry: BodyPaintGeometry {
                samples: &samples,
                active_count: samples.len(),
                distance_cursor_seed: slither_client::state::Point { x: 0., y: 0. },
                eligible: true,
            },
            recipe: BodyPaintRecipe {
                base_palette: 0,
                palettes: &palettes,
                runs: &[],
                variant_mode: BodyVariantMode::Cycle,
                variant_period: 1,
                custom: false,
                style: Some(BodyStyle {
                    detailed_segments: false,
                    swell: 0.,
                    rotate_segments: true,
                    head_fade: None,
                    tail_fade: None,
                }),
                outline: Some(use_asset(1)),
                shadow: Some(use_asset(2)),
                death: Some(use_asset(3)),
            },
            assets: &assets,
            view: BodyPaintView {
                camera: slither_client::state::Point { x: 0., y: 0. },
                zoom: diameter_in_geometry / 29.,
                half_width: 750.,
                half_height: 422.5,
            },
            quality: BodyPaintQuality {
                high_quality: true,
                blend: 1.,
            },
            visual: BodyPaintVisual {
                scale: 1.,
                spawn: 1.,
                death: 0.,
                dead: false,
                smoothed_speed: 4.,
                boost_threshold: 5.,
                cruise_speed: 4.,
                maximum_speed: 10.,
                boost_phase: 0.,
            },
            entry_alpha: 1.,
            command_limit: MAX_BODY_PAINT_COMMANDS,
        },
        AntennaBarrierEffect::Disabled,
        Vec::new(),
        factor,
        logical_scale,
    )
    .unwrap();
    plan.commands
        .iter()
        .filter(|c| c.pass == BodyPaintPass::BodyShadow)
        .count()
}

#[test]
fn native_shadow_cut_uses_displayed_logical_pixels_in_both_resolution_modes() {
    for physical in [[1920, 1080], [3840, 2160]] {
        for dpr in [1., 1.5, 2.] {
            let mut previous = None;
            for requested in [WorldResolution::Original, WorldResolution::Native] {
                let window = WorldWindow {
                    physical,
                    dpr,
                    requested,
                };
                let mut game = tape::playing();
                let backing = window.viewport(&mut game).unwrap();
                let conversion =
                    render::body_logical_scale(backing, 1., game.bank.overlay_viewport).unwrap();
                let expected = (0..2)
                    .map(|i| f64::from(physical[i]) / dpr / f64::from(backing[i]))
                    .fold(0., f64::max);
                assert_eq!(conversion, expected);
                let target = WorldTargetPlan::new(requested, backing, physical, 4, 8192).unwrap();
                assert_eq!(
                    target.samples,
                    if requested == WorldResolution::Native && physical == [3840, 2160] {
                        1
                    } else {
                        4
                    }
                );
                // The review's 3.9-backing-unit counterexample must be judged
                // after expansion, independently of attachment/MSAA selection.
                let count = shadow_count(3.9, conversion, 0.2);
                assert_eq!(
                    count == 0,
                    3.9 * conversion < 4.,
                    "{physical:?} DPR {dpr} {requested:?}"
                );
                assert_eq!(shadow_count(3.5 / conversion, conversion, 0.2), 0);
                assert!(shadow_count(4.5 / conversion, conversion, 0.2) > 0);
                assert!(shadow_count(0.1, conversion, 1.) > 0);
                if let Some(previous) = previous {
                    assert_eq!(count, previous);
                }
                previous = Some(count);
            }
        }
    }
    assert_eq!(
        render::body_logical_scale([960, 540], 1.5, None).unwrap(),
        1.
    );
}

#[test]
fn native_viewport_metadata_reaches_shadow_eligibility_through_gameplay_preparation() {
    for physical in [[1920, 1080], [3840, 2160]] {
        for dpr in [1., 1.5, 2.] {
            for requested in [WorldResolution::Original, WorldResolution::Native] {
                let mut game = tape::playing();
                // Allow the fractional wheel factors around four pixels without
                // the small authored arena clamping them to its whole-map limit.
                let mut radius = vec![b'z'];
                radius.extend(tape::u24(20_000));
                game.receive(game.generation(), &tape::bundle(&[radius]), 21.)
                    .unwrap();
                tape::pictures(&mut game);
                tape::pump(&mut game, 1000.);
                let window = WorldWindow {
                    physical,
                    dpr,
                    requested,
                };
                let backing = window.viewport(&mut game).unwrap();
                tape::pictures(&mut game);
                assert_eq!(game.bank.overlay_viewport, Some((physical, dpr)));
                // Disable the separate near-head/tail quality passes, making
                // this one snake's shadow asset identify only BodyShadow draws.
                let quality = QualitySnapshot {
                    high_quality: false,
                    quality_blend: 0.,
                    ..game.quality()
                };
                let template = game.client().clone();
                let shadow = game.bank.assets[game.bank.shadow.asset];
                let displayed_scale = (0..2)
                    .map(|axis| f64::from(physical[axis]) / dpr / f64::from(backing[axis]))
                    .fold(0., f64::max);
                for target_diameter in [3.95, 4.05] {
                    let mut world = template.clone();
                    let snake_scale = world.own_snake().unwrap().metrics().scale;
                    let factor = target_diameter
                        / (29. * snake_scale * world.camera().automatic_zoom() * displayed_scale);
                    assert!(factor > 0. && factor < 1.);
                    world.wheel_zoom(factor.ln() / 1.12_f64.ln());
                    for frame in 0..110 {
                        let prepared = render::prepare_gameplay(
                            &mut world,
                            &mut game.bank,
                            backing,
                            1.,
                            30_000 + frame,
                            quality,
                        )
                        .unwrap();
                        if frame == 109 {
                            let diameter = 29.
                                * world.own_snake().unwrap().metrics().scale
                                * world.camera().zoom()
                                * displayed_scale;
                            assert!((diameter - target_diameter).abs() < 0.02);
                            assert!(world.camera().user_zoom_factor() < 1.);
                            assert_eq!(prepared.frame.extent, backing);
                            assert_eq!(prepared.frame.snakes.len(), 1);
                            assert!(prepared.frame.snakes[0].body_commands > 0);
                            let body_shadows = prepared
                                .frame
                                .draws
                                .iter()
                                .filter(|draw| draw.asset == shadow)
                                .count();
                            assert_eq!(
                                body_shadows > 0,
                                cfg!(feature = "frame-profile-no-shadow-cut")
                                    || target_diameter > 4.,
                                "{physical:?} DPR {dpr} {requested:?}: {diameter} logical px"
                            );
                        }
                        game.bank.recycle_frame(prepared);
                    }
                }
            }
        }
    }
}
