use slither_sim::{Action, BoostFoodConfig, Config, Food, Origin, Point, Simulator};

fn scene(snakes: usize) -> Simulator {
    let mut world = Simulator::new(
        42,
        Config {
            food_count: 0,
            ambient_replenish_per_second: 0.0,
            radius: 10_000.0,
            ..Config::sandbox()
        },
    )
    .unwrap();
    for i in 0..snakes {
        world.add(i.to_string(), i, snakes).unwrap();
        let head = if i == 0 {
            Point { x: 135.0, y: 35.0 }
        } else {
            Point { x: 135.0, y: 135.0 }
        };
        world.snakes[i].head = head;
        world.snakes[i].body = vec![head, Point { x: 35.0, y: 35.0 }];
        world.snakes[i].heading = 0.0;
        world.snakes[i].fam = 0.9;
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
        world.snakes.len()
    ];
    world.step(&actions).unwrap();
}

#[test]
fn full_cell_suppresses_pellet_but_boost_still_spends_mass() {
    let mut world = scene(1);
    world.food = crowded_food(9);
    let before = world.snakes[0].fam;
    boost(&mut world);
    assert_eq!(world.food.len(), 9);
    assert!(world.snakes[0].fam < before);
    assert!(world.boost_accounting(0).unwrap().boost_mass_spent > 0.0);
    assert!(world.is_boosting(0));
}

#[test]
fn one_free_slot_admits_ninth_and_earlier_snake_fills_shared_step_capacity() {
    let mut one = scene(1);
    one.food = crowded_food(8);
    boost(&mut one);
    assert_eq!(one.food.len(), 9);
    assert_eq!(one.food[8].origin, Some(Origin::Boost));

    let mut two = scene(2);
    two.food = crowded_food(8);
    boost(&mut two);
    assert_eq!(two.food.len(), 9);
    assert!(two.boost_accounting(0).unwrap().boost_mass_spent > 0.0);
    assert!(two.boost_accounting(1).unwrap().boost_mass_spent > 0.0);
}

#[test]
fn disabled_rule_preserves_randomness_and_mass_after_rejected_candidate() {
    let mut capped = scene(1);
    capped.food = crowded_food(9);
    let mut uncapped = capped.clone();
    uncapped.config.boost_food.enabled = false;
    boost(&mut capped);
    boost(&mut uncapped);
    assert_eq!(capped.food.len(), 9);
    assert_eq!(uncapped.food.len(), 10);
    assert_eq!(capped.snakes, uncapped.snakes);
    assert_eq!(capped.boost_accounting(0), uncapped.boost_accounting(0));

    capped.food.clear();
    uncapped.food.clear();
    capped.config.boost_food.enabled = false;
    for _ in 0..24 {
        boost(&mut capped);
        boost(&mut uncapped);
    }
    assert_eq!(capped.food, uncapped.food);
    assert_eq!(capped.snakes, uncapped.snakes);
}

#[test]
fn configurable_threshold_public_edits_and_settings() {
    let mut world = scene(1);
    world.config.boost_food.max_per_cell = 5;
    world.food = crowded_food(5);
    boost(&mut world);
    assert_eq!(world.food.len(), 5);
    world.food.pop();
    for _ in 0..24 {
        boost(&mut world);
    }
    assert_eq!(world.food.len(), 5);

    let mut config = Config::sandbox();
    for cell_size in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        config.boost_food.cell_size = cell_size;
        assert!(Simulator::new(1, config.clone()).is_err());
    }
    config.boost_food.cell_size = 50.0;
    // Zero admits no boost pellet; see tests/config.rs.
    config.boost_food.max_per_cell = 0;
    assert!(Simulator::new(1, config).is_ok());

    let mut old = serde_json::to_value(Config::sandbox()).unwrap();
    old.as_object_mut().unwrap().remove("boostFood");
    let restored: Config = serde_json::from_value(old).unwrap();
    assert!(restored.boost_food.enabled);
    assert_eq!(restored.boost_food.max_per_cell, 9);
    assert_eq!(BoostFoodConfig::default().cell_size, 75.0);
}
