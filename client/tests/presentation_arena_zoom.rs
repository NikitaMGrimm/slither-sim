//! Authored geometry properties; no raster, sockets, or game-server evidence.
use std::f64::consts::{PI, TAU};

use slither_client::{
    presentation::{
        ArenaEdgeInput, ArenaEdgeView, MAX_ARENA_EDGE_SEGMENTS, ZoomArenaEdgePlan, plan_arena_edge,
        plan_arena_edge_with_zoom,
    },
    state::Point,
};

fn input(radius: f64, distance: f64, angle: f64, zoom: f64) -> ArenaEdgeInput {
    ArenaEdgeInput {
        view: ArenaEdgeView::new(12000., angle, distance).unwrap(),
        camera: Point {
            x: 12000. + distance * angle.cos(),
            y: 12000. + distance * angle.sin(),
        },
        zoom,
        width: 960,
        height: 540,
        radius,
    }
}

#[test]
fn stock_mode_preserves_main_geometry_and_gates_exactly() {
    for radius in [0., 100., 637., 4000., 11500., 32768.] {
        for distance in [0., radius, radius + 3999., radius + 4000.] {
            for angle in [-3., 0., 0.7, PI] {
                let frozen = input(radius, distance, angle, 1.);
                let stock = plan_arena_edge(frozen).unwrap();
                let actual = plan_arena_edge_with_zoom(frozen, false).unwrap();
                assert_eq!(actual, stock.map(ZoomArenaEdgePlan::Local));
            }
        }
    }
}

#[test]
fn nearby_edge_keeps_stock_arc_on_both_sides_of_default_zoom() {
    for zoom in [0.9, 0.999999, 1., 1.000001, 1.25] {
        for angle in [-3., -1., 0., 0.7, PI] {
            let frozen = input(11500., 11400., angle, zoom);
            assert_eq!(
                plan_arena_edge_with_zoom(frozen, true).unwrap(),
                plan_arena_edge(frozen)
                    .unwrap()
                    .map(ZoomArenaEdgePlan::Local),
            );
        }
        // Its half diagonal exceeds the local half arc, but its visible circle
        // does not: radial screen width must not select the whole-circle path.
        let frozen = ArenaEdgeInput {
            width: 4000,
            ..input(11500., 11500., 0., zoom)
        };
        assert!(matches!(
            plan_arena_edge_with_zoom(frozen, true).unwrap(),
            Some(ZoomArenaEdgePlan::Local(_))
        ));
    }
}

#[test]
fn visible_edge_beyond_arc_or_distance_gate_uses_whole_circle() {
    for frozen in [
        input(11500., 0., 0., 0.02),
        input(11500., 7000., 0., 0.08),
        input(11500., 11500., 0., 0.1),
        ArenaEdgeInput {
            height: 8000,
            ..input(11500., 11500., 0., 1.)
        },
    ] {
        assert!(matches!(
            plan_arena_edge_with_zoom(frozen, true).unwrap(),
            Some(ZoomArenaEdgePlan::Whole { .. })
        ));
    }
    // A retained local angle can be stale; use the current camera projection.
    let frozen = ArenaEdgeInput {
        view: ArenaEdgeView::new(12000., PI, 11500.).unwrap(),
        ..input(11500., 11500., 0., 0.9)
    };
    assert!(matches!(
        plan_arena_edge_with_zoom(frozen, true).unwrap(),
        Some(ZoomArenaEdgePlan::Whole { .. })
    ));
}

#[test]
fn visible_band_samples_are_never_omitted_by_the_stock_arc_or_gate() {
    for radius in [100., 1000., 11500., 32768.] {
        for distance in [0., radius * 0.6, radius, radius + 4500.] {
            for angle in [-3., -1., 0., 0.7, 2.] {
                for zoom in [0.01, 0.05, 0.15, 0.5, 1.25] {
                    let frozen = input(radius, distance, angle, zoom);
                    let plan = plan_arena_edge_with_zoom(frozen, true).unwrap();
                    for step in 0..360 {
                        let theta = f64::from(step) * TAU / 360.;
                        for r in [radius, radius + 2000., radius + 4000.] {
                            let x = 480. + (12000. + r * theta.cos() - frozen.camera.x) * zoom;
                            let y = 270. + (12000. + r * theta.sin() - frozen.camera.y) * zoom;
                            if !(0. ..=960.).contains(&x) || !(0. ..=540.).contains(&y) {
                                continue;
                            }
                            let half_arc = (2000. / radius).min(PI);
                            let offset = (theta - angle + PI).rem_euclid(TAU) - PI;
                            if (radius - distance).abs() >= 4000. || offset.abs() > half_arc {
                                assert!(matches!(plan, Some(ZoomArenaEdgePlan::Whole { .. })));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn whole_circle_chords_keep_both_band_radii_within_half_a_pixel() {
    for radius in [0., 100., 1000., 11500., 32768.] {
        for zoom in [0.02, 0.1, 0.5, 1., 1.25] {
            let frozen = ArenaEdgeInput {
                width: 8000,
                height: 8000,
                // A stale opposite angle also exercises large projected circles.
                view: ArenaEdgeView::new(12000., PI, radius).unwrap(),
                ..input(radius, radius, 0., zoom)
            };
            let Some(plan @ ZoomArenaEdgePlan::Whole { .. }) =
                plan_arena_edge_with_zoom(frozen, true).unwrap()
            else {
                continue;
            };
            let segments = plan.stroke().len() - 2;
            assert!((40..=MAX_ARENA_EDGE_SEGMENTS).contains(&segments));
            assert_eq!(plan.fill().len(), 2 * segments + 3);
            let center = Point {
                x: 4000. + (12000. - frozen.camera.x) * zoom,
                y: 4000. + (12000. - frozen.camera.y) * zoom,
            };
            for (points, r) in [
                (&plan.stroke()[1..], radius),
                (&plan.fill()[segments + 2..], radius + 4000.),
            ] {
                for pair in points.windows(2) {
                    let midpoint = Point {
                        x: (pair[0].x + pair[1].x) / 2.,
                        y: (pair[0].y + pair[1].y) / 2.,
                    };
                    let sagitta = r * zoom - (midpoint.x - center.x).hypot(midpoint.y - center.y);
                    assert!((-1e-8..=0.5 + 1e-8).contains(&sagitta));
                }
            }
        }
    }
    let excessive = ArenaEdgeInput {
        view: ArenaEdgeView::new(12000., PI, 1e9).unwrap(),
        ..input(1e9, 1e9, 0., 1.)
    };
    assert!(plan_arena_edge_with_zoom(excessive, true).is_err());
}
