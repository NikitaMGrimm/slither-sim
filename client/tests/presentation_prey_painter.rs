//! Authored synthetic pixel-command cases, without browser or external runtime execution.

use slither_client::{
    presentation::*,
    state::{Direction, Point, PreySpawnInput},
};

fn source(handle: u64, width: f64, height: f64, half_width: f64, half_height: f64) -> PreySource {
    PreySource::new(
        BodyAsset {
            handle,
            revision: 9,
            width: 41,
            height: 43,
            status: BodyAssetStatus::Ready,
        },
        width,
        height,
        half_width,
        half_height,
    )
    .unwrap()
}
fn factory() -> PreyFactory {
    PreyFactory {
        random_phase_unit: 0.25,
        random_rate_unit: 0.5,
        palette: 0,
        bodies: vec![source(10, 11.25, 13.5, 2.25, 3.75)],
        glows: vec![source(20, 17.5, 19.25, 4.125, 5.625)],
    }
}
fn add(w: &mut PreyWorld, id: u16, x: f64, y: f64) -> PreyInstance {
    w.spawn(
        PreySpawnInput {
            id,
            absolute_position: Point { x, y },
            size_byte: 0,
            base_size: 0.,
            raw_color: 0,
            direction: Direction::None,
            heading: 0.,
            wanted_heading: 0.,
            speed_per_32ms: 0.,
        },
        factory().input(1.),
    )
    .unwrap()
}
fn assets() -> [BodyAsset; 2] {
    [factory().bodies[0].asset(), factory().glows[0].asset()]
}
fn step(delta: f64) -> FrameStep {
    FrameClock::new(0.)
        .unwrap()
        .advance(delta * 8., false, false)
        .unwrap()
}
fn mature(w: &mut PreyWorld) {
    for _ in 0..31 {
        w.advance(step(5.), 0., |_| panic!("uneaten")).unwrap();
    }
}
fn view() -> BodyPaintView {
    BodyPaintView {
        camera: Point { x: 0., y: 0. },
        zoom: 1.,
        half_width: 100.,
        half_height: 80.,
    }
}
fn input<'a>(w: &'a PreyWorld, a: &'a [BodyAsset]) -> PreyPaintInput<'a> {
    PreyPaintInput {
        preys: w.preys(),
        eaters: &[],
        view: view(),
        assets: a,
        target_handle: Some(999),
        early_entry_alpha: 0.25,
        late_entry_alpha: 0.375,
        command_limit: MAX_PREY_PAINT_COMMANDS,
    }
}

#[test]
fn user_zoom_scales_prey_and_glow_extents_without_changing_alpha_or_order() {
    let mut world = PreyWorld::new();
    add(&mut world, 1, 0., 0.);
    mature(&mut world);
    let assets = assets();
    let stock = plan_prey(input(&world, &assets)).unwrap();
    assert_eq!(
        stock,
        plan_prey_with_zoom(input(&world, &assets), 1.).unwrap()
    );
    for factor in [0.02, 0.5, 1.25] {
        let zoomed = plan_prey_with_zoom(input(&world, &assets), factor).unwrap();
        assert_eq!(stock.counts, zoomed.counts);
        for (a, b) in stock
            .early
            .iter()
            .chain(&stock.late)
            .zip(zoomed.early.iter().chain(&zoomed.late))
        {
            assert_eq!(a.alpha, b.alpha);
            assert_eq!(a.pass, b.pass);
            let extent = |c: &PreyImageCommand| match c.draw {
                FoodImageDraw::Whole { .. } => [
                    f64::from(c.source.asset().width),
                    f64::from(c.source.asset().height),
                ],
                FoodImageDraw::Scaled { destination, .. } => {
                    [destination.width, destination.height]
                }
            };
            for (x, y) in extent(a).into_iter().zip(extent(b)) {
                assert!((x * factor - y).abs() < 1e-12);
            }
        }
    }
}

#[test]
fn reciprocal_zoom_preserves_a_growing_preys_declared_crop() {
    let mut world = PreyWorld::new();
    add(&mut world, 1, 0., 0.);
    let assets = assets();
    for _ in 0..1000 {
        world
            .advance(step(0.25), 0., |_| panic!("uneaten"))
            .unwrap();
        let scale = world.preys()[0].snapshot().scale;
        if scale > 0.8 && scale < 1. && scale * scale.recip() == 1. {
            let stock = plan_prey(input(&world, &assets)).unwrap();
            let zoomed = plan_prey_with_zoom(input(&world, &assets), scale.recip()).unwrap();
            for (a, b) in stock
                .early
                .iter()
                .chain(&stock.late)
                .zip(zoomed.early.iter().chain(&zoomed.late))
            {
                let (
                    FoodImageDraw::Scaled {
                        source: sa,
                        destination: da,
                    },
                    FoodImageDraw::Scaled {
                        source: sb,
                        destination: db,
                    },
                ) = (a.draw, b.draw)
                else {
                    panic!("stock crop must survive combined unit scale")
                };
                assert_eq!(sa, sb);
                assert_eq!(db.width, da.width * scale.recip());
                assert_eq!(a.alpha, b.alpha);
            }
            return;
        }
    }
    panic!("no near-mature scale witness");
}
fn pose(x: f64, y: f64) -> PreyEaterPose {
    PreyEaterPose {
        head: Point { x, y },
        head_offset: Point { x: 0., y: 0. },
        heading: 0.,
        angle_offset: 0.,
    }
}
fn scaled(command: PreyImageCommand) -> (BodyRect, BodyRect) {
    match command.draw {
        FoodImageDraw::Scaled {
            source,
            destination,
        } => (source, destination),
        _ => panic!("expected nine arguments"),
    }
}

#[test]
fn fresh_and_zero_scale_prey_have_no_visible_coverage() {
    let mut w = PreyWorld::new();
    add(&mut w, 0, 0., 0.);
    let a = assets();
    let p = plan_prey(input(&w, &a)).unwrap();
    assert!(
        p.early
            .iter()
            .chain(&p.late)
            .all(|c| c.alpha.effective == 0.)
    );
    w.advance(step(0.), 0., |_| panic!("uneaten")).unwrap();
    let p = plan_prey(input(&w, &a)).unwrap();
    assert!(
        p.early
            .iter()
            .chain(&p.late)
            .all(|c| c.alpha.effective == 0.)
    );
}

#[test]
fn full_scale_uses_whole_body_inner_but_explicit_outer_and_pixel_projection() {
    let mut w = PreyWorld::new();
    add(&mut w, 0, 14., -8.);
    mature(&mut w);
    let a = assets();
    let before = w.clone();
    let mut i = input(&w, &a);
    i.view.camera = Point { x: 4., y: 2. };
    i.view.zoom = 2.;
    let p = plan_prey(i).unwrap();
    assert_eq!(w, before);
    assert_eq!(
        p.early[0].draw,
        FoodImageDraw::Whole {
            x: 117.75,
            y: 56.25
        }
    );
    assert_eq!(
        p.late[0].draw,
        FoodImageDraw::Whole {
            x: 100. + 10. * 1.08 * 2. - 4.125,
            y: 80. - 10. * 1.08 * 2. - 5.625
        }
    );
    let (s, d) = scaled(p.late[2]);
    assert_eq!((s.width, s.height), (17.5, 19.25));
    assert_eq!(
        d,
        BodyRect {
            x: 100. + 10. * 1.32 * 2. - 8.25,
            y: 80. - 10. * 1.32 * 2. - 11.25,
            width: 35.,
            height: 38.5
        }
    );
    let pulse = (1. + (w.preys()[0].snapshot().glow_phase / 13.).cos()) / 2.;
    assert_eq!(p.early[0].alpha.effective, 0.75);
    assert_eq!(p.early[1].alpha.effective, 0.75 * pulse);
    assert_eq!(p.late[0].alpha.effective, 0.4 * (1. - 200. / 176200.));
    assert_eq!(p.late[2].alpha.effective, 0.35 * (1. - 200. / 46700.));
}

#[test]
fn reverse_paint_order_keeps_inner_outer_pairs_per_instance() {
    let mut w = PreyWorld::new();
    let a0 = add(&mut w, 0, 0., 0.);
    let a1 = add(&mut w, 0, 1., 0.);
    let a2 = add(&mut w, 3, 2., 0.);
    let a = assets();
    let p = plan_prey(input(&w, &a)).unwrap();
    assert_eq!(
        p.early.iter().map(|c| c.instance).collect::<Vec<_>>(),
        vec![a2, a2, a1, a1, a0, a0]
    );
    assert_eq!(
        p.late.iter().map(|c| c.instance).collect::<Vec<_>>(),
        vec![a2, a2, a2, a2, a1, a1, a1, a1, a0, a0, a0, a0]
    );
    assert_eq!(
        p.late.iter().map(|c| c.pass).take(4).collect::<Vec<_>>(),
        vec![
            PreyPaintPass::InnerGlow,
            PreyPaintPass::InnerGlow,
            PreyPaintPass::OuterGlow,
            PreyPaintPass::OuterGlow
        ]
    );
    assert_eq!(
        p.early
            .iter()
            .chain(&p.late)
            .map(|c| c.sequence)
            .collect::<Vec<_>>(),
        (0..18).collect::<Vec<_>>()
    );
}

#[test]
fn inclusive_pixel_culls_are_independent_and_do_not_use_source_extents() {
    let a = assets();
    for (x, y, expected) in [
        (-150., 0., true),
        (150., 0., true),
        (0., -130., true),
        (0., 130., true),
        (-150.00000000000003, 0., false),
        (150.00000000000003, 0., false),
    ] {
        let mut w = PreyWorld::new();
        add(&mut w, 0, x, y);
        w.advance(step(0.), 0., |_| panic!()).unwrap();
        let p = plan_prey(input(&w, &a)).unwrap();
        assert_eq!(p.counts.visible_bodies == 1, expected, "{x} {y}");
    }
    let mut w = PreyWorld::new();
    add(&mut w, 0, 200., 0.);
    mature(&mut w);
    let p = plan_prey(input(&w, &a)).unwrap();
    assert_eq!(
        (
            p.counts.visible_bodies,
            p.counts.visible_inner_glows,
            p.counts.visible_outer_glows
        ),
        (0, 1, 0)
    );
    // A zero-scale center exactly at the inclusive glow boundary still emits four calls.
    let mut w = PreyWorld::new();
    add(&mut w, 0, 250., 0.);
    w.advance(step(0.), 0., |_| panic!()).unwrap();
    let p = plan_prey(input(&w, &a)).unwrap();
    assert_eq!((p.counts.early, p.counts.late), (0, 4));
    let mut w = PreyWorld::new();
    add(&mut w, 0, 250.00000000000006, 0.);
    w.advance(step(0.), 0., |_| panic!()).unwrap();
    let p = plan_prey(input(&w, &a)).unwrap();
    assert_eq!(p.counts.total(), 0);
}

#[test]
fn early_cull_precedes_capture_interpolation_and_is_not_repeated() {
    let t = SnakeObjectToken::new(1).unwrap();
    let a = assets();
    let mut outside = PreyWorld::new();
    add(&mut outside, 0, 200., 0.);
    outside.consume(0, Some(t));
    for _ in 0..7 {
        outside
            .advance(step(5.), 0., |_| Ok(pose(-50., 0.)))
            .unwrap();
    }
    let eaters = [(t, pose(-50., 0.))];
    let mut i = input(&outside, &a);
    i.eaters = &eaters;
    let p = plan_prey(i).unwrap();
    assert_eq!(p.counts.visible_bodies, 0);
    assert_eq!(p.counts.late, 4);
    let mut inside = PreyWorld::new();
    add(&mut inside, 0, 0., 0.);
    inside.consume(0, Some(t));
    for _ in 0..7 {
        inside
            .advance(step(5.), 0., |_| Ok(pose(1000., 0.)))
            .unwrap();
    }
    let eaters = [(t, pose(1000., 0.))];
    let mut i = input(&inside, &a);
    i.eaters = &eaters;
    let p = plan_prey(i).unwrap();
    assert_eq!(p.counts.visible_bodies, 1);
    assert_eq!(p.counts.late, 0);
    assert!(scaled(p.early[0]).1.x > 350.);
}

#[test]
fn captured_mouth_uses_current_raw_pose_and_corrections_with_left_to_right_products() {
    let mut w = PreyWorld::new();
    add(&mut w, 0, 10., 20.);
    let t = SnakeObjectToken::new(7).unwrap();
    w.consume(0, Some(t));
    w.advance(step(3.), 0., |_| Ok(pose(0., 0.))).unwrap();
    let eater = PreyEaterPose {
        head: Point { x: 300., y: -100. },
        head_offset: Point { x: 2.25, y: -3.5 },
        heading: 0.75,
        angle_offset: -0.25,
    };
    let eaters = [(t, eater)];
    let a = assets();
    let mut i = input(&w, &a);
    i.eaters = &eaters;
    i.view.zoom = 0.75;
    let before = w.clone();
    let p = plan_prey(i).unwrap();
    assert_eq!(w, before);
    let progress = 3_f64 / 47.;
    let k = progress.powf(2.);
    let mut mx = 0.5_f64.cos();
    mx *= 43. - k * 24.;
    mx *= 1. - k;
    mx += 302.25;
    let mut my = 0.5_f64.sin();
    my *= 43. - k * 24.;
    my *= 1. - k;
    my += -103.5;
    let center_x = 10. + (mx - 10.) * k;
    let center_y = 20. + (my - 20.) * k;
    let scale = 1. - progress.powf(3.);
    let (_, d) = scaled(p.early[0]);
    let expected_x = 100. + 0.75 * center_x - 2.25 * scale;
    let expected_y = 80. + 0.75 * center_y - 3.75 * scale;
    // Already projected CSS pixels: for one sample the p95 <=0.5px budget
    // is stricter than the 1px maximum (docs/perceptual-parity.md).
    assert!((d.x - expected_x).hypot(d.y - expected_y) <= 0.5);
    assert_eq!(d.width, 11.25 * scale);
}

#[test]
fn ignored_alpha_retains_previous_preys_pulse_and_explicit_scope_entries() {
    let mut w = PreyWorld::new();
    let t = SnakeObjectToken::new(1).unwrap();
    add(&mut w, 0, 0., 0.);
    mature(&mut w);
    // Repeated captures make a reachable fade of 1.5 without ever reaching flight expiry.
    for _ in 0..16 {
        w.consume(0, Some(t));
        w.advance(step(5.), 0., |_| Ok(pose(0., 0.))).unwrap();
    }
    assert_eq!(w.preys()[0].snapshot().fade, 1.5);
    add(&mut w, 1, 0., 0.);
    let eaters = [(t, pose(0., 0.))];
    let a = assets();
    let mut i = input(&w, &a);
    i.eaters = &eaters;
    let p = plan_prey(i).unwrap();
    assert_eq!(p.early[0].prey_index, 1);
    assert_eq!(p.early[1].alpha.effective, 0.);
    assert_eq!(p.early[2].alpha.effective, 0.);
    let mut single = w.clone();
    single.remove(1);
    let mut i = input(&single, &a);
    i.eaters = &eaters;
    i.early_entry_alpha = 0.625;
    let p = plan_prey(i).unwrap();
    assert_eq!(p.early[0].alpha.effective, 0.625);
    // Empty passes preserve their independent roots without a synthetic assignment.
    let empty = PreyWorld::new();
    let p = plan_prey(input(&empty, &a)).unwrap();
    assert_eq!((p.final_early_alpha, p.final_late_alpha), (0.25, 0.375));
}

#[test]
fn exact_budget_is_culled_count_and_failure_leaves_world_unchanged() {
    let mut w = PreyWorld::new();
    add(&mut w, 0, 0., 0.);
    add(&mut w, 1, 500., 0.);
    let a = assets();
    let before = w.clone();
    let mut i = input(&w, &a);
    i.command_limit = 5;
    assert_eq!(plan_prey(i), Err(PreyPaintError::CommandLimit));
    i.command_limit = 6;
    let p = plan_prey(i).unwrap();
    assert_eq!(p.counts.total(), 6);
    assert_eq!((p.early.len(), p.late.len()), (2, 4));
    assert_eq!(w, before);
    let mut empty = PreyWorld::new();
    add(&mut empty, 0, 1000., 1000.);
    let mut i = input(&empty, &a);
    i.command_limit = 0;
    assert_eq!(plan_prey(i).unwrap().counts.total(), 0);
}

#[test]
fn full_population_requires_exactly_24576_calls_and_rejects_excess_instances() {
    let mut w = PreyWorld::new();
    for id in 0..4096 {
        add(&mut w, id, 0., 0.);
    }
    let a = assets();
    let mut i = input(&w, &a);
    i.command_limit = 24575;
    assert_eq!(plan_prey(i), Err(PreyPaintError::CommandLimit));
    i.command_limit = 24576;
    let p = plan_prey(i).unwrap();
    assert_eq!((p.counts.early, p.counts.late), (8192, 16384));
    assert_eq!(p.counts.total(), 24576);
    let too_many = vec![w.preys()[0].clone(); 4097];
    i.preys = &too_many;
    assert_eq!(plan_prey(i), Err(PreyPaintError::PreyLimit));
}

#[test]
fn source_alias_coherence_eater_view_and_arithmetic_validate_before_return() {
    let mut w = PreyWorld::new();
    add(&mut w, 0, 0., 0.);
    let a = assets();
    let mut i = input(&w, &a);
    i.target_handle = Some(10);
    assert_eq!(plan_prey(i), Err(PreyPaintError::TargetAlias));
    i.target_handle = None;
    i.assets = &a[..1];
    assert_eq!(plan_prey(i), Err(PreyPaintError::InvalidSource));
    let mut changed = a[0];
    changed.revision += 1;
    let incoherent = [a[0], a[1], changed];
    i.assets = &incoherent;
    assert_eq!(plan_prey(i), Err(PreyPaintError::InvalidSource));
    i.assets = &a;
    i.early_entry_alpha = f64::NAN;
    assert_eq!(plan_prey(i), Err(PreyPaintError::InvalidVisual));
    i.early_entry_alpha = 0.5;
    i.view.zoom = f64::INFINITY;
    assert_eq!(plan_prey(i), Err(PreyPaintError::InvalidView));
    i.view = view();
    i.view.camera.x = f64::MAX;
    assert_eq!(plan_prey(i), Err(PreyPaintError::Arithmetic));
    let t = SnakeObjectToken::new(1).unwrap();
    w.consume(0, Some(t));
    assert_eq!(
        plan_prey(input(&w, &a)),
        Err(PreyPaintError::MissingEater(t))
    );
    let mut bad = pose(0., 0.);
    bad.angle_offset = f64::NAN;
    let eaters = [(t, bad)];
    let mut i = input(&w, &a);
    i.eaters = &eaters;
    assert_eq!(plan_prey(i), Err(PreyPaintError::InvalidEater));
    let first = pose(0., 0.);
    let mut second = first;
    second.head.x = -0.;
    let eaters = [(t, first), (t, second)];
    let mut i = input(&w, &a);
    i.eaters = &eaters;
    assert_eq!(plan_prey(i), Err(PreyPaintError::InvalidEater));
}
