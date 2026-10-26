mod support;
use slither_sim::{Action, BoostFoodConfig, Config, Food, Origin, Point, Simulator};

fn scene(snakes: usize) -> Simulator {
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 42,
        config: Config {
            food_count: 0,
            ambient_replenish_per_second: 0.0,
            radius: 10_000.0,
            ..Config::sandbox()
        },
        population: slither_sim::InitialPopulation::Placed {
            ids: (0..snakes).map(|i| i.to_string()).collect(),
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    for i in 0..snakes {
        let head = if i == 0 {
            Point { x: 135.0, y: 35.0 }
        } else {
            Point { x: 135.0, y: 135.0 }
        };
        {
            let mut snake = world.snakes()[i].clone();
            snake.head = head;
            world
                .correct_pose(i, snake.head, snake.body.clone(), snake.heading)
                .unwrap();
        }
        {
            let mut snake = world.snakes()[i].clone();
            snake.body = vec![head, Point { x: 35.0, y: 35.0 }];
            world
                .correct_mass(i, snake.body.clone(), snake.fam)
                .unwrap();
            world
                .correct_pose(i, snake.head, snake.body.clone(), snake.heading)
                .unwrap();
        }
        {
            let mut snake = world.snakes()[i].clone();
            snake.heading = 0.0;
            world
                .correct_pose(i, snake.head, snake.body.clone(), snake.heading)
                .unwrap();
        }
        {
            let mut snake = world.snakes()[i].clone();
            snake.fam = 0.9;
            world
                .correct_mass(i, snake.body.clone(), snake.fam)
                .unwrap();
        }
    }
    world
}

fn crowded_food(count: usize) -> Vec<Food> {
    (0..count)
        .map(|i| Food {
            x: 35.0,
            y: 35.0,
            value: 0.0,
            size: Some(1.0),
            origin: match i % 4 {
                0 => Some(Origin::Ambient),
                1 => Some(Origin::Boost),
                2 => Some(Origin::Death),
                _ => None,
            },
        })
        .collect()
}

fn boost(world: &mut Simulator) {
    let actions = vec![
        Some(Action {
            heading: 0.0,
            boost: true,
        });
        world.snakes().len()
    ];
    world.step(&actions).unwrap();
}

#[test]
fn full_cell_suppresses_pellet_but_boost_still_spends_mass() {
    let mut world = scene(1);
    world.replace_food(crowded_food(9)).unwrap();
    let before = world.snakes()[0].fam;
    boost(&mut world);
    assert_eq!(world.food().len(), 9);
    assert!(world.snakes()[0].fam < before);
    assert!(world.boost_accounting(0).unwrap().boost_mass_spent > 0.0);
    assert!(world.is_boosting(0));
}

#[test]
fn one_free_slot_admits_ninth_and_earlier_snake_fills_shared_step_capacity() {
    let mut one = scene(1);
    one.replace_food(crowded_food(8)).unwrap();
    boost(&mut one);
    assert_eq!(one.food().len(), 9);
    assert_eq!(one.food()[8].origin, Some(Origin::Boost));

    let mut two = scene(2);
    two.replace_food(crowded_food(8)).unwrap();
    boost(&mut two);
    assert_eq!(two.food().len(), 9);
    assert!(two.boost_accounting(0).unwrap().boost_mass_spent > 0.0);
    assert!(two.boost_accounting(1).unwrap().boost_mass_spent > 0.0);
}

#[test]
fn disabled_rule_preserves_randomness_and_mass_after_rejected_candidate() {
    let mut capped = scene(1);
    capped.replace_food(crowded_food(9)).unwrap();
    let mut uncapped = capped.clone();
    {
        let mut config = uncapped.config().clone();
        config.boost_food.enabled = false;
        support::reconfigure(&mut uncapped, config);
    }
    boost(&mut capped);
    boost(&mut uncapped);
    assert_eq!(capped.food().len(), 9);
    assert_eq!(uncapped.food().len(), 10);
    assert_eq!(capped.snakes(), uncapped.snakes());
    assert_eq!(capped.boost_accounting(0), uncapped.boost_accounting(0));

    capped.replace_food(Vec::new()).unwrap();
    uncapped.replace_food(Vec::new()).unwrap();
    {
        let mut config = capped.config().clone();
        config.boost_food.enabled = false;
        support::reconfigure(&mut capped, config);
    }
    for _ in 0..24 {
        boost(&mut capped);
        boost(&mut uncapped);
    }
    assert_eq!(capped.food(), uncapped.food());
    assert_eq!(capped.snakes(), uncapped.snakes());
}

#[test]
fn configurable_threshold_public_edits_and_settings() {
    let mut world = scene(1);
    {
        let mut config = world.config().clone();
        config.boost_food.max_per_cell = 5;
        support::reconfigure(&mut world, config);
    }
    world.replace_food(crowded_food(5)).unwrap();
    boost(&mut world);
    assert_eq!(world.food().len(), 5);
    world.remove_food(world.food().len() - 1).unwrap();
    for _ in 0..24 {
        boost(&mut world);
    }
    assert_eq!(world.food().len(), 5);

    let mut config = Config::sandbox();
    for cell_size in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        config.boost_food.cell_size = cell_size;
        assert!(
            Simulator::new(slither_sim::WorldOptions {
                seed: 1,
                config: config.clone(),
                population: slither_sim::InitialPopulation::Empty
            })
            .is_err()
        );
    }
    config.boost_food.cell_size = 50.0;
    // Zero admits no boost pellet; see tests/config.rs.
    config.boost_food.max_per_cell = 0;
    assert!(
        Simulator::new(slither_sim::WorldOptions {
            seed: 1,
            config,
            population: slither_sim::InitialPopulation::Empty
        })
        .is_ok()
    );

    let mut old = serde_json::to_value(Config::sandbox()).unwrap();
    old.as_object_mut().unwrap().remove("boostFood");
    let restored: Config = serde_json::from_value(old).unwrap();
    assert!(restored.boost_food.enabled);
    assert_eq!(restored.boost_food.max_per_cell, 9);
    assert_eq!(BoostFoodConfig::default().cell_size, 75.0);
}
