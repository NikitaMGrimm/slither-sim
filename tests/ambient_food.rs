use slither_sim::{Action, AmbientFoodConfig, Config, Food, Origin, Point, Simulator};

fn tiny_world(enabled: bool, food_count: usize, rate: f64) -> Simulator {
    Simulator::new(
        7,
        Config {
            radius: 20.0,
            dt: 0.01,
            food_count,
            ambient_replenish_per_second: rate,
            ambient_food: AmbientFoodConfig {
                enabled,
                ..AmbientFoodConfig::default()
            },
            ..Config::default()
        },
    )
    .unwrap()
}

fn food(origin: Origin) -> Food {
    Food {
        x: 0.0,
        y: 0.0,
        value: 1.0,
        size: Some(1.0),
        origin: Some(origin),
    }
}

#[test]
fn startup_attempts_admit_only_two_per_cell_and_disabled_keeps_all() {
    let capped = tiny_world(true, 20, 0.0);
    let uncapped = tiny_world(false, 20, 0.0);
    assert_eq!(capped.food.len(), 2);
    assert_eq!(uncapped.food.len(), 20);
    assert_eq!(capped.food, uncapped.food[..2]);
}

#[test]
fn refill_counts_all_origins_and_reconciles_public_food_edits() {
    let mut world = tiny_world(true, 0, 100.0);
    world.config.food_count = 5;
    world.food = vec![food(Origin::Boost), food(Origin::Death)];
    world.step(&[]).unwrap();
    assert_eq!(world.food.len(), 2);
    world.food.pop();
    world.step(&[]).unwrap();
    assert_eq!(world.food.len(), 2);
    assert_eq!(world.food[1].origin, Some(Origin::Ambient));
    for _ in 0..100 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.food.len(), 2);
}

#[test]
fn actual_pickup_frees_capacity_before_the_same_step_refill() {
    // Radius 20 puts every generated ambient candidate at the origin. The
    // inward-facing snake can collect that cell while its head stays inside.
    let mut world = tiny_world(true, 0, 100.0);
    world.config.food_count = 5;
    world.add("eater", 0, 1).unwrap();
    let snake = &mut world.snakes[0];
    snake.head = Point { x: -19.0, y: 0.0 };
    snake.heading = 0.0;
    snake.body = vec![snake.head; snake.segments];
    world.food = vec![food(Origin::Boost), food(Origin::Death)];
    world.pickup_events = Some(Vec::new());
    world
        .step(&[Some(Action {
            heading: 0.0,
            boost: false,
        })])
        .unwrap();
    assert!(world.snakes[0].alive);
    assert_eq!(world.pickup_events.as_ref().unwrap().len(), 2);
    assert_eq!(world.food.len(), 1);
    assert_eq!(world.food[0].origin, Some(Origin::Ambient));
}

#[test]
fn rejected_candidates_consume_rng_and_refill_interval_without_retries() {
    let mut capped = tiny_world(true, 0, 100.0);
    let mut uncapped = tiny_world(false, 0, 100.0);
    capped.config.food_count = 3;
    uncapped.config.food_count = 3;
    for _ in 0..3 {
        capped.step(&[]).unwrap();
        uncapped.step(&[]).unwrap();
    }
    assert_eq!(capped.food.len(), 2);
    assert_eq!(uncapped.food.len(), 3);
    capped.food.clear();
    uncapped.food.clear();
    capped.config.ambient_food.enabled = false;
    capped.step(&[]).unwrap();
    uncapped.step(&[]).unwrap();
    assert_eq!(capped.food, uncapped.food);
}

#[test]
fn tuning_validation_and_config_round_trip() {
    let mut config = Config::default();
    for cell_size in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        config.ambient_food.cell_size = cell_size;
        assert!(Simulator::new(1, config.clone()).is_err());
    }
    config.ambient_food.cell_size = 50.0;
    config.ambient_food.max_per_cell = 0;
    assert!(Simulator::new(1, config.clone()).is_err());
    config.ambient_food.max_per_cell = 4;
    let mut world = Simulator::new(1, config.clone()).unwrap();
    world.food.clear();
    world.config.radius = 20.0;
    world.config.food_count = 10;
    world.config.ambient_replenish_per_second = 100.0;
    world.config.dt = 0.01;
    for _ in 0..10 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.food.len(), 4);

    let mut old = serde_json::to_value(Config::default()).unwrap();
    old.as_object_mut().unwrap().remove("ambientFood");
    let restored: Config = serde_json::from_value(old).unwrap();
    assert!(restored.ambient_food.enabled);
    assert_eq!(restored.ambient_food.max_per_cell, 2);
}
