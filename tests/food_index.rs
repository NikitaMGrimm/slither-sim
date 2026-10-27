mod support;
use slither_sim::{Action, Config, Food, Origin, Simulator};

fn world(food_index: bool) -> Simulator {
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 11,
        config: Config {
            food_count: 3000,
            food_index,
            ..Config::sandbox()
        },
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap();
    for i in 0..40 {
        support::ring_slot(&mut world, format!("s{i}"), i, 40).unwrap();
    }
    world.record_events(slither_sim::EventRecording {
        pickups: true,
        food_births: world.food_spawn_events().is_some(),
    });
    world
}

#[test]
fn indexed_pickup_matches_scanning_every_eater() {
    let (mut indexed, mut scanned) = (world(true), world(false));
    // Compact enough for the dense eater grid, which handles every tick here.
    assert!(40 * indexed.food().len() >= 32000);
    let mut pickups = 0;
    for tick in 0..400 {
        let actions: Vec<Option<Action>> = (0..40)
            .map(|i| {
                indexed.snakes()[i].alive.then(|| Action {
                    heading: i as f64 * 2.4 + tick as f64 * 0.003 * (i % 5) as f64,
                    boost: (tick / 40 + i) % 4 == 0,
                })
            })
            .collect();
        if tick % 50 == 25 {
            // Public edits between steps: a moved, a relabeled and an injected pellet.
            for world in [&mut indexed, &mut scanned] {
                {
                    let mut food = world.food().to_vec();
                    food[7].x += 75.0;
                    world.replace_food(food).unwrap();
                }
                {
                    let mut food = world.food().to_vec();
                    food[9].origin = Some(Origin::Boost);
                    world.replace_food(food).unwrap();
                }
                world
                    .insert_food(Food {
                        x: -0.0,
                        y: 3.0,
                        value: 1.0,
                        size: Some(2.0),
                        origin: Some(Origin::Ambient),
                    })
                    .unwrap();
            }
        }
        indexed.step(&actions).unwrap();
        scanned.step(&actions).unwrap();
        assert_eq!(
            indexed.pickup_events(),
            scanned.pickup_events(),
            "tick {tick}"
        );
        pickups += indexed.pickup_events().unwrap().len();
        if tick % 20 == 0 {
            {
                let mut config = scanned.config().clone();
                config.food_index = true;
                support::reconfigure(&mut scanned, config);
            }
            assert_eq!(
                serde_json::to_string(&indexed.checkpoint().unwrap()).unwrap(),
                serde_json::to_string(&scanned.checkpoint().unwrap()).unwrap(),
                "tick {tick}"
            );
            {
                let mut config = scanned.config().clone();
                config.food_index = false;
                support::reconfigure(&mut scanned, config);
            }
        }
    }
    assert!(pickups > 100, "only {pickups} pickups");
    assert!(!indexed.deaths().is_empty());
}

#[test]
fn pellet_index_matches_scan_through_public_edits_compaction_and_restore() {
    let mut indexed = world(true);
    let actions = vec![Some(Action::default()); 40];
    for _ in 0..12 {
        indexed.step(&actions).unwrap();
    }
    let restore = |world: &Simulator| {
        Simulator::from_checkpoint(
            &serde_json::to_vec(&world.checkpoint().unwrap()).unwrap(),
            slither_sim::RestoreOptions {
                events: slither_sim::EventRecording {
                    pickups: true,
                    food_births: false,
                },
                #[cfg(feature = "parallel")]
                parallel: false,
            },
        )
        .unwrap()
    };
    let mut restored = restore(&indexed);
    let mut scanned = restore(&indexed);
    let mut config = scanned.config().clone();
    config.food_index = false;
    support::reconfigure(&mut scanned, config);
    let mut pickups = 0;
    for tick in 0..96 {
        if tick % 24 == 0 {
            let food: Vec<_> = indexed
                .snakes()
                .iter()
                .take(8)
                .map(|s| {
                    let (p, _) =
                        slither_sim::physics::pickup_circle(s.head, s.heading, s.speed, s.segments);
                    Food {
                        x: p.x,
                        y: p.y,
                        value: 1.0,
                        size: Some(2.0),
                        origin: Some(Origin::Ambient),
                    }
                })
                .collect();
            for world in [&mut indexed, &mut restored, &mut scanned] {
                for pellet in &food {
                    world.insert_food(pellet.clone()).unwrap();
                }
            }
        }
        if tick % 24 == 8 {
            for world in [&mut indexed, &mut restored, &mut scanned] {
                for index in [world.food().len() - 1, world.food().len() / 2, 0] {
                    world.remove_food(index).unwrap();
                }
            }
        }
        if tick == 16 {
            for world in [&mut indexed, &mut restored, &mut scanned] {
                let mut food = world.food().to_vec();
                food[7].x = -128.0f64.next_down();
                food[8].origin = Some(Origin::Boost);
                world.replace_food(food).unwrap();
            }
        }
        if tick == 40 {
            restored = restore(&indexed);
        }
        if tick == 64 {
            for world in [&mut indexed, &mut restored, &mut scanned] {
                world.replace_food(Vec::new()).unwrap();
            }
        }
        for world in [&mut indexed, &mut restored, &mut scanned] {
            let actions: Vec<_> = world
                .snakes()
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    s.alive.then_some(Action {
                        heading: i as f64 * 2.4 + tick as f64 * 0.003,
                        boost: (tick / 12 + i) % 4 == 0,
                    })
                })
                .collect();
            world.step(&actions).unwrap();
        }
        assert_eq!(
            indexed.pickup_events(),
            restored.pickup_events(),
            "restore tick {tick}"
        );
        assert_eq!(
            indexed.pickup_events(),
            scanned.pickup_events(),
            "scan tick {tick}"
        );
        pickups += indexed.pickup_events().unwrap().len();
        let expected = serde_json::to_value(indexed.checkpoint().unwrap()).unwrap();
        assert_eq!(
            expected,
            serde_json::to_value(restored.checkpoint().unwrap()).unwrap(),
            "restore tick {tick}"
        );
        let mut actual = serde_json::to_value(scanned.checkpoint().unwrap()).unwrap();
        actual["config"]["foodIndex"] = serde_json::json!(true);
        assert_eq!(expected, actual, "scan tick {tick}");
    }
    assert!(pickups >= 24, "only {pickups} pickups");
}

#[test]
fn hashed_pickup_matches_scanning_every_eater() {
    // Snakes spread over a wide arena, each heading into a line of pellets.
    let world = |food_index| {
        let mut world = Simulator::new(slither_sim::WorldOptions {
            seed: 5,
            config: Config {
                radius: 20_000.0,
                food_count: 3000,
                food_index,
                ..Config::sandbox()
            },
            population: slither_sim::InitialPopulation::Empty,
        })
        .unwrap();
        for i in 0..40 {
            support::ring_slot(&mut world, format!("s{i}"), i, 40).unwrap();
        }
        world
            .replace_food(
                (0..3000)
                    .map(|k| {
                        let s = &world.snakes()[k % 40];
                        let d = 20.0 + 8.0 * (k / 40) as f64;
                        Food {
                            x: s.head.x + d * s.heading.cos(),
                            y: s.head.y + d * s.heading.sin(),
                            value: 1.0,
                            size: Some(2.0),
                            origin: Some(Origin::Ambient),
                        }
                    })
                    .collect(),
            )
            .unwrap();
        world.record_events(slither_sim::EventRecording {
            pickups: true,
            food_births: world.food_spawn_events().is_some(),
        });
        world
    };
    let (mut indexed, mut scanned) = (world(true), world(false));
    // Covers span far more 128-unit cells than four per pellet, so the dense
    // grid declines, while eaters times pellets reaches the hashed threshold.
    let (xs, ys): (Vec<f64>, Vec<f64>) = indexed
        .snakes()
        .iter()
        .map(|s| (s.head.x, s.head.y))
        .unzip();
    let span = |v: &[f64]| {
        let (lo, hi) = v
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        (hi - lo) / 128.0
    };
    let cells = span(&xs) * span(&ys);
    assert!(cells > 4.0 * 3000.0, "{cells} cells");
    assert!(40 * indexed.food().len() >= 32000);
    let headings: Vec<f64> = indexed.snakes().iter().map(|s| s.heading).collect();
    let mut pickups = 0;
    for tick in 0..400 {
        let actions: Vec<Option<Action>> = (0..40)
            .map(|i| {
                indexed.snakes()[i].alive.then(|| Action {
                    heading: headings[i],
                    boost: false,
                })
            })
            .collect();
        indexed.step(&actions).unwrap();
        scanned.step(&actions).unwrap();
        assert_eq!(
            indexed.pickup_events(),
            scanned.pickup_events(),
            "tick {tick}"
        );
        pickups += indexed.pickup_events().unwrap().len();
    }
    {
        let mut config = scanned.config().clone();
        config.food_index = true;
        support::reconfigure(&mut scanned, config);
    }
    assert_eq!(
        serde_json::to_string(&indexed.checkpoint().unwrap()).unwrap(),
        serde_json::to_string(&scanned.checkpoint().unwrap()).unwrap()
    );
    assert!(pickups > 1000, "only {pickups} pickups");
}
