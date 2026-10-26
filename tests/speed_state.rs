mod support;
use slither_sim::{physics::*, *};

fn world() -> Simulator {
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 42,
        config: Config {
            radius: 100_000.,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Config::sandbox()
        },
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["test".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    let mut detached_snake = world.snakes()[0].clone();
    let snake = &mut detached_snake;
    snake.head = Point::default();
    snake.heading = 0.;
    snake.segments = 37;
    snake.fam = 0.99;
    snake.body = (0..37)
        .map(|i| Point {
            x: -(i as f64) * 42.,
            y: 0.,
        })
        .collect();
    world
        .correct_mass(0, snake.body.clone(), snake.fam)
        .unwrap();
    world
        .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
        .unwrap();
    world
}

fn steps(world: &mut Simulator, count: usize, boost: bool) {
    for _ in 0..count {
        world.step(&[Some(Action { heading: 0., boost })]).unwrap();
        assert!(world.snakes()[0].alive);
    }
}

#[test]
fn recorded_same_size_pair_retains_idle_speed_then_refreshes_after_boost() {
    let pair: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/cruise-retention-pair.json")).unwrap();
    let measured: Vec<f64> = pair["windows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|window| {
            let anchors = window["anchors"].as_array().unwrap();
            // This fixture stores the outgoing control byte, not a received heading.
            let angle = window["heading_byte"].as_f64().unwrap() * std::f64::consts::TAU / 251.;
            let t0 = anchors[0][0].as_f64().unwrap();
            let x0 = anchors[0][1].as_f64().unwrap();
            let y0 = anchors[0][2].as_f64().unwrap();
            let points: Vec<_> = anchors
                .iter()
                .map(|p| {
                    (
                        (p[0].as_f64().unwrap() - t0) / 1000.,
                        (p[1].as_f64().unwrap() - x0) * angle.cos()
                            + (p[2].as_f64().unwrap() - y0) * angle.sin(),
                    )
                })
                .collect();
            let mt = points.iter().map(|p| p.0).sum::<f64>() / points.len() as f64;
            let md = points.iter().map(|p| p.1).sum::<f64>() / points.len() as f64;
            points.iter().map(|p| (p.0 - mt) * (p.1 - md)).sum::<f64>()
                / points.iter().map(|p| (p.0 - mt).powi(2)).sum::<f64>()
                * 0.032
        })
        .collect();
    let mut world = world();
    steps(&mut world, 350, false);
    assert!((world.snakes()[0].speed * 0.032 - measured[0]).abs() < 0.01);
    steps(&mut world, 122, true);
    steps(&mut world, 400, false);
    assert_eq!(world.snakes()[0].segments, 37);
    assert!((world.snakes()[0].speed * 0.032 - measured[1]).abs() < 0.01);
}

#[test]
fn fixed_size_ramps_preserve_validated_slope_and_partial_reactivation() {
    for segments in [2, 37, 106, 293, 430] {
        let mut state = SpeedState::idle(base_speed(segments));
        let mut direct = state.speed;
        for (boosting, ticks) in [(true, 21), (false, 7), (true, 1200), (false, 1200)] {
            for _ in 0..ticks {
                direct = approach_speed(
                    direct,
                    if boosting {
                        BOOST_SPEED
                    } else {
                        base_speed(segments)
                    },
                    segments,
                    0.007,
                );
                let actual = state.step(boosting, segments, 0.007, SpeedModel::RetainedProgress);
                assert!((actual - direct).abs() < 1e-9);
                assert!((0.0..=1.0).contains(&state.progress()));
            }
        }
    }
}

#[test]
fn actual_pickup_retains_idle_cruise_and_direct_toggle_restores_old_law() {
    let mut retained = world();
    let speed = retained.snakes()[0].speed;
    let (center, _) = pickup_circle(retained.snakes()[0].head, 0.0, speed, 37);
    retained
        .insert_food(Food {
            x: center.x,
            y: center.y,
            value: 1.0,
            size: Some(10.0),
            origin: Some(Origin::Death),
        })
        .unwrap();
    let mut direct = retained.clone();
    {
        let mut config = direct.config().clone();
        config.speed_model = SpeedModel::DirectSpeed;
        support::reconfigure(&mut direct, config);
    }
    steps(&mut retained, 100, false);
    steps(&mut direct, 100, false);
    assert!(retained.food().is_empty());
    assert!(retained.snakes()[0].segments > 37);
    assert_eq!(retained.snakes()[0].speed, speed);
    assert!((direct.snakes()[0].speed - base_speed(direct.snakes()[0].segments)).abs() < 1e-10);
}

#[test]
fn candidate_progress_survives_mass_only_change_and_clone() {
    // This is candidate-model continuity, not evidence for online within-ramp growth.
    let mut world = world();
    steps(&mut world, 20, true);
    let old = world.snakes()[0].speed;
    let q = (old - base_speed(37)) / (BOOST_SPEED - base_speed(37));
    {
        let mut snake = world.snakes()[0].clone();
        snake.segments = 100;
        snake
            .body
            .resize(snake.segments, *snake.body.last().unwrap());
        world
            .correct_mass(0, snake.body.clone(), snake.fam)
            .unwrap();
    }
    let expected_q = q + (0.015 / 0.008) * world.config().dt / (1.0 + 0.305 * (scale(100) - 1.0));
    let expected = base_speed(100) + (BOOST_SPEED - base_speed(100)) * expected_q;
    let mut cloned = world.clone();
    steps(&mut world, 1, true);
    steps(&mut cloned, 1, true);
    assert!((world.snakes()[0].speed - expected).abs() < 1e-10);
    assert_eq!(world.snakes(), cloned.snakes());
    steps(&mut world, 30, false);
    steps(&mut cloned, 30, false);
    assert_eq!(world.snakes(), cloned.snakes());
}

#[test]
fn checked_life_imports_infer_ramps_or_preserve_explicit_idle_speed() {
    let mut world = world();
    let life =
        |world: &Simulator, speed| ReplayLife::from_observed(world.snakes()[0].clone(), speed);
    world
        .import_life(0, life(&world, InitialSpeed::Infer(300.)))
        .unwrap();
    steps(&mut world, 1, false);
    assert!(world.snakes()[0].speed < 300.);
    let speed = world.snakes()[0].speed;
    world
        .import_life(
            0,
            life(&world, InitialSpeed::Known(SpeedState::idle(speed))),
        )
        .unwrap();
    steps(&mut world, 3, false);
    assert_eq!(world.snakes()[0].speed, speed);
    world
        .import_life(0, life(&world, InitialSpeed::Infer(320.)))
        .unwrap();
    steps(&mut world, 1, false);
    assert!(world.snakes()[0].speed < 320.);
    let before = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
    assert!(
        world
            .import_life(0, life(&world, InitialSpeed::Infer(f64::NAN)))
            .is_err()
    );
    assert!(SpeedState::with_progress(speed, f64::NAN).is_err());
    assert!(SpeedState::with_progress(speed, 1.1).is_err());
    assert!(
        world
            .import_life(5, life(&world, InitialSpeed::Infer(speed)))
            .is_err()
    );
    assert_eq!(
        serde_json::to_value(world.checkpoint().unwrap()).unwrap(),
        before
    );
}

#[test]
fn respawn_discards_progress_and_uses_spawn_cruise() {
    let mut world = world();
    steps(&mut world, 25, true);
    world.retire(0).unwrap();
    world.respawn(0, Point::default(), 0.0, 15).unwrap();
    steps(&mut world, 20, false);
    assert_eq!(world.snakes()[0].speed, base_speed(15));
    assert!(!world.is_boosting(0));
}

#[test]
fn zero_time_does_not_refresh_retained_speed() {
    let mut state = SpeedState::idle(base_speed(2));
    let before = state;
    state.step(true, 200, 0.0, SpeedModel::RetainedProgress);
    assert_eq!(state, before);
}
