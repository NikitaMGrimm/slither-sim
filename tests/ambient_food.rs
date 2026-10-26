mod support;
use slither_sim::{Action, AmbientFoodConfig, Config, Food, Origin, Point, Simulator};

fn tiny_world(enabled: bool, food_count: usize, rate: f64) -> Simulator {
    Simulator::new(slither_sim::WorldOptions {
        seed: 7,
        config: Config {
            radius: 20.0,
            dt: 0.01,
            food_count,
            ambient_replenish_per_second: rate,
            ambient_food: AmbientFoodConfig {
                enabled,
                ..AmbientFoodConfig::default()
            },
            ..Config::sandbox()
        },
        population: slither_sim::InitialPopulation::Empty,
    })
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
    assert_eq!(capped.food().len(), 2);
    assert_eq!(uncapped.food().len(), 20);
    assert_eq!(capped.food(), &uncapped.food()[..2]);
}

#[test]
fn sub_margin_arenas_keep_startup_and_refill_food_inside() {
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 7,
        config: Config {
            radius: 1.0,
            food_count: 350,
            ambient_replenish_per_second: 100.0,
            dt: 0.01,
            ambient_food: AmbientFoodConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        },
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap();
    assert_eq!(world.food().len(), 350);
    assert!(
        world
            .food()
            .iter()
            .all(|food| food.point() == Point::default())
    );
    world.replace_food(Vec::new()).unwrap();
    world.step(&[]).unwrap();
    assert!(!world.food().is_empty());
    assert!(
        world
            .food()
            .iter()
            .all(|food| food.point() == Point::default())
    );
}

#[test]
fn refill_counts_all_origins_and_reconciles_public_food_edits() {
    let mut world = tiny_world(true, 0, 100.0);
    {
        let mut config = world.config().clone();
        config.food_count = 5;
        support::reconfigure(&mut world, config);
    }
    world
        .replace_food(vec![food(Origin::Boost), food(Origin::Death)])
        .unwrap();
    world.step(&[]).unwrap();
    assert_eq!(world.food().len(), 2);
    world.remove_food(world.food().len() - 1).unwrap();
    world.step(&[]).unwrap();
    assert_eq!(world.food().len(), 2);
    assert_eq!(world.food()[1].origin, Some(Origin::Ambient));
    for _ in 0..100 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.food().len(), 2);
}

#[test]
fn total_food_target_stops_refill_and_resumes_after_removal() {
    let mut world = tiny_world(false, 0, 1000.0);
    {
        let mut config = world.config().clone();
        config.food_count = 5;

        config.ambient_refill_counts_all_food = true;
        support::reconfigure(&mut world, config);
    }
    world
        .replace_food(vec![food(Origin::Boost), food(Origin::Death)])
        .unwrap();
    // Ten attempts in one tick must stop exactly at five total pellets.
    world.step(&[]).unwrap();
    assert_eq!(world.food().len(), 5);
    assert_eq!(
        world
            .food()
            .iter()
            .filter(|f| f.origin == Some(Origin::Ambient))
            .count(),
        3
    );
    for _ in 0..10 {
        world.step(&[]).unwrap();
        assert_eq!(world.food().len(), 5);
    }
    world.remove_food(world.food().len() - 1).unwrap();
    world.step(&[]).unwrap();
    assert_eq!(world.food().len(), 5);
    // An unspecified origin is still a physical pellet.
    world.replace_food(vec![food(Origin::Death); 6]).unwrap();
    {
        let mut food = world.food().to_vec();
        food[0].origin = None;
        world.replace_food(food).unwrap();
    }
    world.step(&[]).unwrap();
    assert_eq!(world.food().len(), 6);
}

#[test]
fn official_and_sandbox_keep_the_ambient_only_target() {
    for config in [Config::official(1), Config::sandbox()] {
        assert!(!config.ambient_refill_counts_all_food);
        assert!(
            !serde_json::to_value(&config)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key("ambientRefillCountsAllFood")
        );
        let restored: Config = serde_json::from_str("{}").unwrap();
        assert!(!restored.ambient_refill_counts_all_food);
        let mut world = tiny_world(false, 0, 1000.0);
        {
            let mut config = world.config().clone();
            config.food_count = 5;
            support::reconfigure(&mut world, config);
        }
        world
            .replace_food(vec![food(Origin::Boost), food(Origin::Death)])
            .unwrap();
        world.step(&[]).unwrap();
        assert_eq!(world.food().len(), 7);
    }
    let preset: Config = serde_json::from_str(r#"{"preset":"battledome"}"#).unwrap();
    assert!(preset.ambient_refill_counts_all_food);
    let override_config: Config =
        serde_json::from_str(r#"{"preset":"battledome","ambientRefillCountsAllFood":false}"#)
            .unwrap();
    assert!(!override_config.ambient_refill_counts_all_food);
}

#[test]
fn actual_pickup_frees_capacity_before_the_same_step_refill() {
    // Radius 20 puts every generated ambient candidate at the origin. The
    // inward-facing snake can collect that cell while its head stays inside.
    let mut world = tiny_world(true, 0, 100.0);
    {
        let mut config = world.config().clone();
        config.food_count = 5;
        support::reconfigure(&mut world, config);
    }
    // Add a valid ring life before installing this synthetic tiny-arena pose.
    {
        let mut config = world.config().clone();
        config.radius = 100.;
        support::reconfigure(&mut world, config);
    }
    support::ring_slot(&mut world, "eater", 0, 1).unwrap();
    {
        let mut config = world.config().clone();
        config.radius = 20.;
        support::reconfigure(&mut world, config);
    }
    let mut detached_snake = world.snakes()[0].clone();
    let snake = &mut detached_snake;
    snake.head = Point { x: -19.0, y: 0.0 };
    snake.heading = 0.0;
    snake.body = vec![snake.head; snake.segments];
    world
        .correct_mass(0, snake.body.clone(), snake.fam)
        .unwrap();
    world
        .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
        .unwrap();
    world
        .replace_food(vec![food(Origin::Boost), food(Origin::Death)])
        .unwrap();
    world.record_events(slither_sim::EventRecording {
        pickups: true,
        food_births: world.food_spawn_events().is_some(),
    });
    world
        .step(&[Some(Action {
            heading: 0.0,
            boost: false,
        })])
        .unwrap();
    assert!(world.snakes()[0].alive);
    assert_eq!(world.pickup_events().unwrap().len(), 2);
    assert_eq!(world.food().len(), 1);
    assert_eq!(world.food()[0].origin, Some(Origin::Ambient));
}

#[test]
fn rejected_candidates_consume_rng_and_refill_interval_without_retries() {
    let mut capped = tiny_world(true, 0, 100.0);
    let mut uncapped = tiny_world(false, 0, 100.0);
    {
        let mut config = capped.config().clone();
        config.food_count = 3;
        support::reconfigure(&mut capped, config);
    }
    {
        let mut config = uncapped.config().clone();
        config.food_count = 3;
        support::reconfigure(&mut uncapped, config);
    }
    for _ in 0..3 {
        capped.step(&[]).unwrap();
        uncapped.step(&[]).unwrap();
    }
    assert_eq!(capped.food().len(), 2);
    assert_eq!(uncapped.food().len(), 3);
    capped.replace_food(Vec::new()).unwrap();
    uncapped.replace_food(Vec::new()).unwrap();
    {
        let mut config = capped.config().clone();
        config.ambient_food.enabled = false;
        support::reconfigure(&mut capped, config);
    }
    capped.step(&[]).unwrap();
    uncapped.step(&[]).unwrap();
    assert_eq!(capped.food(), uncapped.food());
}

#[test]
fn single_attempts_and_a_batch_keep_admission_rng_and_event_order() {
    for enabled in [false, true] {
        let mut singles = tiny_world(enabled, 0, 100.0);
        let mut batch = tiny_world(enabled, 0, 100.0);
        {
            let mut config = singles.config().clone();
            config.food_count = 3;
            support::reconfigure(&mut singles, config);
        }
        {
            let mut config = batch.config().clone();
            config.food_count = 3;

            config.dt = 0.03;
            support::reconfigure(&mut batch, config);
        }
        singles.record_events(slither_sim::EventRecording {
            food_births: true,
            pickups: singles.pickup_events().is_some(),
        });
        batch.record_events(slither_sim::EventRecording {
            food_births: true,
            pickups: batch.pickup_events().is_some(),
        });
        let mut events = Vec::new();
        for _ in 0..3 {
            singles.step(&[]).unwrap();
            events.extend_from_slice(singles.food_spawn_events().unwrap());
        }
        batch.step(&[]).unwrap();
        assert_eq!(singles.food(), batch.food());
        assert_eq!(events, *batch.food_spawn_events().unwrap());
        assert_eq!(singles.food().len(), if enabled { 2 } else { 3 });

        // Rejections and reaching the target must spend exactly the same draws.
        for world in [&mut singles, &mut batch] {
            {
                let mut config = world.config().clone();
                config.dt = 0.01;

                config.ambient_food.enabled = false;
                support::reconfigure(world, config);
            }
            world.replace_food(Vec::new()).unwrap();
            world.step(&[]).unwrap();
        }
        assert_eq!(singles.food(), batch.food());
    }
}

#[test]
fn epsilon_due_boundary_preserves_single_and_batch_capacity() {
    for intervals in [1.0, 2.0] {
        for offset in [-2.0e-12, -0.5e-12, 0.0, 0.5e-12, 2.0e-12] {
            let mut world = tiny_world(true, 0, 100.0);
            {
                let mut config = world.config().clone();
                config.food_count = 5;

                config.dt = intervals * 0.01 + offset;
                support::reconfigure(&mut world, config);
            }
            world.record_events(slither_sim::EventRecording {
                food_births: true,
                pickups: world.pickup_events().is_some(),
            });
            world.step(&[]).unwrap();
            let expected = if offset + 1e-12 >= 0.0 {
                intervals as usize
            } else {
                intervals as usize - 1
            };
            assert_eq!(world.food().len(), expected);
            assert_eq!(world.food_spawn_events().unwrap().len(), expected);
            assert!(
                world
                    .food()
                    .iter()
                    .all(|f| f.origin == Some(Origin::Ambient))
            );
        }
    }
}

#[test]
fn tuning_validation_and_config_round_trip() {
    let mut config = Config::sandbox();
    for cell_size in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        config.ambient_food.cell_size = cell_size;
        assert!(
            Simulator::new(slither_sim::WorldOptions {
                seed: 1,
                config: config.clone(),
                population: slither_sim::InitialPopulation::Empty
            })
            .is_err()
        );
    }
    config.ambient_food.cell_size = 50.0;
    // Zero admits no ambient pellet; see tests/config.rs.
    config.ambient_food.max_per_cell = 0;
    assert!(
        Simulator::new(slither_sim::WorldOptions {
            seed: 1,
            config: config.clone(),
            population: slither_sim::InitialPopulation::Empty
        })
        .unwrap()
        .food()
        .is_empty()
    );
    config.ambient_food.max_per_cell = 4;
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 1,
        config: config.clone(),
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap();
    world.replace_food(Vec::new()).unwrap();
    {
        let mut config = world.config().clone();
        config.radius = 20.0;

        config.food_count = 10;

        config.ambient_replenish_per_second = 100.0;

        config.dt = 0.01;
        support::reconfigure(&mut world, config);
    }
    for _ in 0..10 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.food().len(), 4);

    let mut old = serde_json::to_value(Config::sandbox()).unwrap();
    old.as_object_mut().unwrap().remove("ambientFood");
    let restored: Config = serde_json::from_value(old).unwrap();
    assert!(restored.ambient_food.enabled);
    assert_eq!(restored.ambient_food.max_per_cell, 2);
}

#[test]
fn refill_intervals_below_the_tolerance_stop_at_spent_credit() {
    // A 1e-30 s interval is far below the 1e-12 s due tolerance, which used to
    // drive the clock negative until subtraction stopped changing it.
    let mut config = Config::sandbox();
    config.dt = 1e-30;
    config.ambient_replenish_per_second = 1e30;
    config.food_count = 0;
    config.prey.count = 0;
    let (done, finished) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut world = Simulator::new(slither_sim::WorldOptions {
            seed: 7,
            config,
            population: slither_sim::InitialPopulation::Placed {
                ids: vec!["a".into()],
                placement: slither_sim::Placement::TestRing,
            },
        })
        .unwrap();
        world.step(&[Some(Action::default())]).unwrap();
        done.send(world.time()).unwrap();
    });
    let time = finished
        .recv_timeout(std::time::Duration::from_secs(30))
        .expect("one refill step must finish");
    assert_eq!(time, 1e-30);
}
