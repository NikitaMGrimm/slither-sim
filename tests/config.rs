use slither_sim::ErrorKind;
use slither_sim::{Config, Simulator};

#[test]
fn omitted_world_settings_select_the_official_baseline() {
    let explicit = serde_json::to_value(Config::official(1)).unwrap();
    assert_eq!(serde_json::to_value(Config::default()).unwrap(), explicit);
    let omitted: Config = serde_json::from_str("{}").unwrap();
    assert_eq!(serde_json::to_value(omitted).unwrap(), explicit);
    let partial: Config = serde_json::from_str(r#"{"prey":{"size":6}}"#).unwrap();
    assert_eq!(partial.prey.count, 1);
    assert!(partial.kiosk.enabled);
    let disabled: Config = serde_json::from_str(r#"{"prey":{"count":0}}"#).unwrap();
    assert_eq!(disabled.prey.count, 0);
}

#[test]
fn official_baseline_enables_current_models_and_nonzero_prey() {
    let config = Config::default();
    assert_eq!(config.radius, 670.);
    assert!(!config.dynamic_radius);
    assert!(config.size_to_population);
    assert_eq!(config.food_count, 109);
    assert!((config.ambient_replenish_per_second - 1.6045635608990811).abs() < 1e-12);
    assert_eq!(config.prey.count, 1);
    assert!(config.kiosk.enabled);
    assert!(config.ambient_food.enabled && config.boost_food.enabled && config.death_food.enabled);
    assert_eq!(config.boundary_mode, slither_sim::BoundaryMode::Sliding);
    assert_eq!(
        config.speed_model,
        slither_sim::SpeedModel::RetainedProgress
    );
    assert_eq!(config.dt, 0.007);
    config.validate().unwrap();
    assert_eq!(Simulator::new(7, config).unwrap().prey.len(), 1);

    let populated = Config::official(100);
    assert_eq!(populated.radius, 6700.);
    assert_eq!(populated.food_count, 10_968);
    assert_eq!(populated.prey.count, 14);
    populated.validate().unwrap();
}

#[test]
fn sandbox_changes_scene_size_and_population_settings_only() {
    let sandbox = Config::sandbox();
    assert_eq!(sandbox.radius, 1000.);
    assert!(!sandbox.dynamic_radius);
    assert_eq!(sandbox.food_count, 350);
    assert_eq!(sandbox.ambient_replenish_per_second, 5.);
    assert_eq!(sandbox.prey.count, 0);
    sandbox.validate().unwrap();

    let mut official = serde_json::to_value(Config::default()).unwrap();
    let sandbox = serde_json::to_value(sandbox).unwrap();
    for field in [
        "radius",
        "dynamicRadius",
        "sizeToPopulation",
        "foodCount",
        "ambientReplenishPerSecond",
    ] {
        official[field] = sandbox[field].clone();
    }
    official["prey"]["count"] = sandbox["prey"]["count"].clone();
    assert_eq!(official, sandbox);
}

#[test]
fn opted_in_radius_tracks_living_snakes_but_sandbox_stays_fixed() {
    let mut config = Config::official(4);
    config.dynamic_radius = true;
    config.food_count = 0;
    config.prey.count = 0;
    let mut official = Simulator::new(7, config).unwrap();
    official.add("one", 0, 1).unwrap();
    let radius = official.config.radius;
    official
        .step(&[Some(slither_sim::Action::default())])
        .unwrap();
    assert!(official.config.radius < radius);

    let mut config = Config::sandbox();
    config.food_count = 0;
    let mut sandbox = Simulator::new(7, config).unwrap();
    sandbox.add("one", 0, 1).unwrap();
    sandbox
        .step(&[Some(slither_sim::Action::default())])
        .unwrap();
    assert_eq!(sandbox.config.radius, 1000.);
}

#[test]
fn official_radius_holds_across_death_and_respawn() {
    let mut config = Config::official(4);
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    config.prey.count = 0;
    let radius = config.radius;
    let mut world = Simulator::new(7, config).unwrap();
    world.add("one", 0, 1).unwrap();
    world.step(&[Some(slither_sim::Action::default())]).unwrap();
    assert_eq!(world.config.radius, radius);
    world.snakes[0].alive = false;
    world
        .respawn(
            0,
            slither_sim::Point {
                x: radius - 1.,
                y: 0.,
            },
            0.,
            2,
        )
        .unwrap();
    world.step(&[Some(slither_sim::Action::default())]).unwrap();
    assert!(!world.snakes[0].alive);
    assert_eq!(world.config.radius, radius);
    world.step(&[Some(slither_sim::Action::default())]).unwrap();
    world
        .respawn(0, slither_sim::Point::default(), 0., 2)
        .unwrap();
    world.step(&[Some(slither_sim::Action::default())]).unwrap();
    assert!(world.snakes[0].alive);
    assert_eq!(world.config.radius, radius);
}

#[test]
fn unmeasured_physics_steps_warn_without_failing() {
    assert!(Config::default().warnings().is_empty());
    for dt in [0.00694, 0.00707] {
        assert!(
            Config {
                dt,
                ..Config::default()
            }
            .warnings()
            .is_empty()
        );
    }
    for dt in [0.001, 0.008, 0.032] {
        let config = Config {
            dt,
            ..Config::default()
        };
        config.validate().unwrap();
        let warnings = config.warnings();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("~7 ms server loop"), "{}", warnings[0]);
    }
}

#[test]
fn food_and_prey_targets_have_no_policy_cap() {
    let config = Config {
        food_count: 100_001,
        ambient_replenish_per_second: 10_001.,
        prey: slither_sim::PreyConfig {
            count: 10_001,
            ..Default::default()
        },
        ..Config::sandbox()
    };
    config.validate().unwrap();
    let world = Simulator::new(7, config).unwrap();
    assert_eq!(world.prey.len(), 10_001);
    assert!(world.food.len() > 1000);
    // An unallocatable target is an error, not an abort.
    let error = Simulator::new(
        7,
        Config {
            food_count: usize::MAX,
            ..Config::sandbox()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::AllocationFailed);
    assert_eq!(error.field.as_deref(), Some("foodCount"));
    let error = Simulator::new(
        7,
        Config {
            prey: slither_sim::PreyConfig {
                count: usize::MAX,
                ..Default::default()
            },
            ..Config::sandbox()
        },
    )
    .unwrap_err();
    assert_eq!(error.kind, ErrorKind::AllocationFailed);
    assert_eq!(error.field.as_deref(), Some("prey.count"));
}

#[test]
fn zero_food_cell_limits_admit_nothing() {
    let mut config = Config::sandbox();
    config.ambient_food.max_per_cell = 0;
    config.boost_food.max_per_cell = 0;
    config.death_food.max_per_cell = 0;
    // No pellet can be admitted, so even an unrepresentable target starts at once.
    config.food_count = usize::MAX;
    let mut world = Simulator::new(7, config).unwrap();
    assert!(world.food.is_empty());
    world.add("a", 0, 1).unwrap();
    // A large snake dies at the boundary; its death food is not admitted.
    let snake = &mut world.snakes[0];
    snake.segments = 100;
    snake.body = vec![slither_sim::Point { x: 2000., y: 0. }; 100];
    snake.head = snake.body[0];
    Simulator::update_mass(snake);
    world.step(&[Some(slither_sim::Action::default())]).unwrap();
    assert!(!world.snakes[0].alive);
    for _ in 0..200 {
        world.step(&[None]).unwrap();
    }
    assert!(world.food.is_empty());
}

#[test]
fn zero_prey_speeds_turning_and_nutrition_are_valid() {
    let prey = slither_sim::PreyConfig {
        cruise_speed: 0.,
        escape_speed: 0.,
        sprint_speed: 0.,
        turn_rate: 0.,
        nutrition_factor: 0.,
        ..Default::default()
    };
    prey.validate().unwrap();
    let mut world = Simulator::new(
        7,
        Config {
            prey,
            ..Config::default()
        },
    )
    .unwrap();
    // The first server tick floors the spawn position onto integers.
    world.step(&[]).unwrap();
    let start = world.prey[0].clone();
    for _ in 0..100 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.prey[0].position, start.position);
    assert_eq!(world.prey[0].heading, start.heading);
}

#[test]
fn zero_kiosk_rates_freeze_their_phase() {
    let kiosk = slither_sim::KioskConfig {
        shrink_per_second: 0.,
        score_decay_per_second: 0.,
        recovery_per_second: 0.,
        grace_seconds: 0.,
        floor_score: 1.,
        ..Default::default()
    };
    kiosk.validate().unwrap();
    let mut config = Config::sandbox();
    config.food_count = 0;
    config.kiosk = kiosk;
    let mut world = Simulator::new(7, config).unwrap();
    world.add("a", 0, 1).unwrap();
    let snake = &mut world.snakes[0];
    snake.segments = 200;
    snake.body = vec![snake.head; 200];
    Simulator::update_mass(snake);
    let segments = world.snakes[0].segments;
    for _ in 0..100 {
        world.step(&[Some(slither_sim::Action::default())]).unwrap();
    }
    assert_eq!(world.snakes[0].segments, segments);
}
