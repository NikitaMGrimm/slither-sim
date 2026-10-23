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
    assert!(config.dynamic_radius);
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
        "foodCount",
        "ambientReplenishPerSecond",
    ] {
        official[field] = sandbox[field].clone();
    }
    official["prey"]["count"] = sandbox["prey"]["count"].clone();
    assert_eq!(official, sandbox);
}

#[test]
fn official_radius_tracks_living_snakes_but_sandbox_stays_fixed() {
    let mut config = Config::official(4);
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
fn initial_food_allocation_is_bounded() {
    for food_count in [0, 100_000] {
        assert!(
            Config {
                food_count,
                ..Config::default()
            }
            .validate()
            .is_ok()
        );
    }
    for food_count in [100_001, usize::MAX] {
        assert!(
            Config {
                food_count,
                ..Config::default()
            }
            .validate()
            .is_err()
        );
    }
}
