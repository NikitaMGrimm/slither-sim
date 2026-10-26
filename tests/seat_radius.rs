//! Synthetic product-model regressions; no official controller claim.
use serde_json::Value;
use slither_sim::{
    Action, Config, Food, InitialPopulation, Origin, Placement, Point, PreyConfig, RadiusUpdate,
    Simulator, WorldOptions,
};

fn quiet_config(seats: usize) -> Config {
    let mut config = Config::official(seats);
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    config.prey.count = 0;
    config.death_food.max_per_cell = 0;
    config
}

fn placed(config: Config, seats: usize) -> Simulator {
    Simulator::new(WorldOptions {
        seed: 7,
        config,
        population: InitialPopulation::Placed {
            ids: (0..seats).map(|i| i.to_string()).collect(),
            placement: Placement::TestRing,
        },
    })
    .unwrap()
}

fn kill_every_seat(world: &mut Simulator) {
    let rim = Point {
        x: world.radius() - 1.,
        y: 0.,
    };
    for slot in 0..world.snakes().len() {
        world.correct_pose(slot, rim, vec![rim; 2], 0.).unwrap();
    }
    let radius = world.radius();
    world
        .step(&vec![Some(Action::default()); world.snakes().len()])
        .unwrap();
    assert_eq!(world.living(), 0);
    assert_eq!(world.occupied(), world.snakes().len());
    assert_eq!(world.radius(), radius);
}

#[test]
fn deaths_and_respawns_preserve_occupied_seats_and_radius() {
    let mut world = placed(quiet_config(4), 4);
    let radius = world.radius();
    kill_every_seat(&mut world);
    for _ in 0..100 {
        world.step(&[None; 4]).unwrap();
        assert_eq!(world.radius(), radius);
    }
    world.respawn(0, Point::default(), 0., 2).unwrap();
    world
        .step(&[Some(Action::default()), None, None, None])
        .unwrap();
    assert_eq!(world.occupied(), 4);
    assert_eq!(world.radius(), radius);
}

#[test]
fn a_500_to_100_exodus_reaches_the_dead_zone_at_the_rate_limit() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/arena-seat-following-v2.json")).unwrap();
    assert_eq!(
        fixture["rulesRevision"],
        "slither-sim-rules-v2" // Historical scalar fixture; official seat control is unchanged.
    );
    let mut world = placed(quiet_config(500), 500);
    kill_every_seat(&mut world);
    for slot in 100..500 {
        world.retire(slot).unwrap();
    }
    assert_eq!(world.occupied(), 100);
    let actions = vec![None; 500];
    let step = fixture["rate"].as_f64().unwrap() * world.config().dt;
    let ticks = fixture["movingTicks"].as_u64().unwrap();
    let start = world.time();
    for tick in 1..=ticks {
        let before = world.radius();
        world.step(&actions).unwrap();
        assert_eq!(world.radius(), before - step);
        if tick == ticks / 2 {
            let saved = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
            let mut restored = Simulator::from_checkpoint(&saved, Default::default()).unwrap();
            assert_eq!(
                serde_json::to_vec(&restored.checkpoint().unwrap()).unwrap(),
                saved
            );
            let mut continued = world.clone();
            for _ in 0..100 {
                restored.step(&actions).unwrap();
                continued.step(&actions).unwrap();
            }
            assert_eq!(
                serde_json::to_value(restored.checkpoint().unwrap()).unwrap(),
                serde_json::to_value(continued.checkpoint().unwrap()).unwrap()
            );
        }
    }
    assert!((world.time() - start - fixture["shrinkSeconds"].as_f64().unwrap()).abs() < 1e-6);
    assert!((world.radius() - fixture["stoppedRadius"].as_f64().unwrap()).abs() < 1e-8);
    let stopped = world.radius();
    assert!((6700. - stopped).abs() <= 0.03 * stopped);
    for _ in 0..100 {
        world.step(&actions).unwrap();
        assert_eq!(world.radius(), stopped);
    }
    eprintln!(
        "500 -> 100: {ticks} moving ticks, {:.3} seconds, radius {stopped:.9}",
        ticks as f64 * world.config().dt
    );
}

#[test]
fn small_churn_is_absorbed_at_500_but_small_worlds_move() {
    let mut world = placed(quiet_config(500), 500);
    kill_every_seat(&mut world);
    let radius = world.radius();
    world.retire(499).unwrap();
    world.step(&vec![None; 500]).unwrap();
    assert_eq!(world.radius(), radius);
    world.respawn(499, Point::default(), 0., 2).unwrap();
    world
        .spawn("extra".into(), vec![Point::default(); 2], 0.)
        .unwrap();
    world.step(&vec![Some(Action::default()); 501]).unwrap();
    assert_eq!(world.occupied(), 501);
    assert_eq!(world.radius(), radius);

    let mut small = placed(quiet_config(4), 4);
    kill_every_seat(&mut small);
    let radius = small.radius();
    small.retire(3).unwrap();
    small.step(&[None; 4]).unwrap();
    assert_eq!(small.radius(), radius - 23.4375 * small.config().dt);
    small.respawn(3, Point::default(), 0., 2).unwrap();
    small
        .spawn("extra".into(), vec![Point::default(); 2], 0.)
        .unwrap();
    small.step(&[Some(Action::default()); 5]).unwrap();
    assert_eq!(small.radius(), 670. * 5_f64.sqrt());
}

#[test]
fn fixed_presets_and_explicit_radii_ignore_seat_changes() {
    for config in [
        Config::sandbox(),
        serde_json::from_str::<Config>(r#"{"radius":1200}"#).unwrap(),
        serde_json::from_str::<Config>(r#"{"preset":"official","radius":1200}"#).unwrap(),
    ] {
        assert!(!config.dynamic_radius);
        let mut world = placed(config, 1);
        let radius = world.radius();
        world.retire(0).unwrap();
        world.step(&[None]).unwrap();
        assert_eq!(world.radius(), radius);
    }
}

#[test]
fn live_area_scales_food_refill_and_prey_and_continues_from_checkpoint() {
    let mut config = quiet_config(4); // radius 1340; one-seat radius has 1/4 the area.
    config.food_count = 100;
    config.ambient_replenish_per_second = 8.;
    config.ambient_food.enabled = false;
    config.prey = PreyConfig {
        count: 20,
        cruise_speed: 0.,
        escape_speed: 0.,
        sprint_speed: 0.,
        ..Default::default()
    };
    config.radius_updates = vec![RadiusUpdate {
        time: 0.007,
        radius: 670.,
    }];
    let mut world = Simulator::new(WorldOptions {
        config,
        ..Default::default()
    })
    .unwrap();
    let pellet = |x, origin| Food {
        x,
        y: 0.,
        size: Some(1.),
        value: 1.,
        origin: Some(origin),
    };
    world
        .replace_food(vec![
            pellet(100., Origin::Ambient),
            pellet(1000., Origin::Ambient),
            pellet(1100., Origin::Death),
        ])
        .unwrap();
    world.step(&[]).unwrap();
    assert_eq!(world.radius(), 670.); // sample overrides the controller without a rate cap.
    assert_eq!(world.food().len(), 1); // stranded food from every origin leaves with the wall.
    assert_eq!(world.prey().len(), 5);
    world.replace_food(vec![]).unwrap();
    for _ in 0..34 {
        world.step(&[]).unwrap();
    }
    let saved = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
    let mut restored = Simulator::from_checkpoint(&saved, Default::default()).unwrap();
    // 8 * 1/4 = 2 attempts/s. At .49 s no pellet; .504 s earns the first.
    for _ in 0..35 {
        world.step(&[]).unwrap();
        restored.step(&[]).unwrap();
    }
    assert!(world.food().is_empty());
    for _ in 0..2 {
        world.step(&[]).unwrap();
        restored.step(&[]).unwrap();
    }
    assert_eq!(world.food().len(), 1);
    assert_eq!(
        serde_json::to_value(world.checkpoint().unwrap()).unwrap(),
        serde_json::to_value(restored.checkpoint().unwrap()).unwrap()
    );
    for _ in 0..2000 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.food().len(), 25);
    assert_eq!(world.prey().len(), 5);
    assert_eq!(world.config().food_count, 100);
    assert_eq!(world.config().prey.count, 20);
}

#[test]
fn ecology_credit_continues_while_the_arena_is_still_shrinking() {
    let mut config = quiet_config(4);
    config.food_count = 100;
    config.ambient_replenish_per_second = 8.;
    config.prey.count = 20;
    let mut world = placed(config, 4);
    kill_every_seat(&mut world);
    for slot in 1..4 {
        world.retire(slot).unwrap();
    }
    world.replace_food(vec![]).unwrap();
    for _ in 0..37 {
        world.step(&[None; 4]).unwrap();
    }
    let saved = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
    let mut restored = Simulator::from_checkpoint(&saved, Default::default()).unwrap();
    let radius = world.radius();
    for _ in 0..200 {
        world.step(&[None; 4]).unwrap();
        restored.step(&[None; 4]).unwrap();
    }
    assert!(world.radius() < radius);
    assert!(!world.food().is_empty());
    assert_eq!(
        serde_json::to_value(world.checkpoint().unwrap()).unwrap(),
        serde_json::to_value(restored.checkpoint().unwrap()).unwrap()
    );
}

#[test]
fn fixed_recorded_replay_keeps_outside_food_and_initial_ecology() {
    let mut config = quiet_config(4);
    config.dynamic_radius = false;
    config.prey.count = 20;
    config.radius_updates = vec![RadiusUpdate {
        time: 0.007,
        radius: 670.,
    }];
    let mut world = Simulator::new(WorldOptions {
        config,
        ..Default::default()
    })
    .unwrap();
    world
        .replace_food(vec![Food {
            x: 1000.,
            y: 0.,
            value: 1.,
            size: Some(1.),
            origin: Some(Origin::Ambient),
        }])
        .unwrap();
    world.step(&[]).unwrap();
    assert_eq!(world.radius(), 670.);
    assert_eq!(world.food().len(), 1);
    assert_eq!(world.prey().len(), 20);
}

#[test]
fn unrepresentable_live_area_refill_stops_without_entering_the_attempt_loop() {
    let mut config = quiet_config(1);
    config.radius = 1e-100;
    config.ambient_replenish_per_second = 1.;
    config.radius_updates = vec![RadiusUpdate {
        time: 0.007,
        radius: 1e150,
    }];
    let mut world = Simulator::new(WorldOptions {
        config,
        ..Default::default()
    })
    .unwrap();
    let error = world.step(&[]).unwrap_err();
    assert!(matches!(error, slither_sim::CoreStepError::Stopped(_)));
    let saved = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
    let restored = Simulator::from_checkpoint(&saved, Default::default()).unwrap();
    assert_eq!(
        serde_json::to_vec(&restored.checkpoint().unwrap()).unwrap(),
        saved
    );
}

#[test]
fn accumulated_live_area_credit_cannot_overflow_at_a_subnormal_rate() {
    let mut config = quiet_config(1);
    config.radius = 1.;
    config.ambient_replenish_per_second = 1e-320; // Reciprocal interval is infinite.
    config.radius_updates = vec![RadiusUpdate {
        time: 0.007,
        radius: 1e150,
    }];
    let mut world = Simulator::new(WorldOptions {
        config,
        ..Default::default()
    })
    .unwrap();
    // Each tick adds about 1e298 reference seconds. Use a checkpoint to retain
    // already earned, finite fractional credit near f64's upper range.
    world.step(&[]).unwrap();
    let mut saved = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
    saved["state"]["ambientClock"] = serde_json::json!(f64::MAX);
    let mut world =
        Simulator::from_checkpoint(&serde_json::to_vec(&saved).unwrap(), Default::default())
            .unwrap();
    assert!(matches!(
        world.step(&[]),
        Err(slither_sim::CoreStepError::Stopped(_))
    ));
    world.checkpoint().unwrap();
}
