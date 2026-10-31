use super::*;
use serde_json::Value;

#[test]
fn many_eaters_keep_dense_pickup_after_food_is_depleted() {
    const SIDE: usize = 48;
    let mut world = Simulator::new(WorldOptions {
        seed: 7,
        config: Config {
            radius: 10000.,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            prey: crate::PreyConfig {
                count: 0,
                ..Default::default()
            },
            ..Config::sandbox()
        },
        population: InitialPopulation::Dormant {
            ids: (0..SIDE * SIDE).map(|i| i.to_string()).collect(),
        },
    })
    .unwrap();
    // Spread non-colliding lives over more than the old 4,096-cell minimum.
    // Only a small fraction have food, as in a depleted, crowded world.
    for slot in 0..SIDE * SIDE {
        let head = Point {
            x: (slot % SIDE) as f64 * 192. - 4608.,
            y: (slot / SIDE) as f64 * 192. - 4608.,
        };
        let (body, heading) = world.spawn_body(head, 0.).unwrap();
        world.respawn_body(slot, body, heading).unwrap();
    }
    let food = world.snakes[..128]
        .iter()
        .flat_map(|snake| {
            [(48., 0.), (96., 96.)].map(|(x, y)| Food {
                x: snake.head.x + x,
                y: snake.head.y + y,
                value: 0.125,
                size: None,
                origin: None,
            })
        })
        .collect();
    world.replace_food(food).unwrap();
    world.record_events(checkpoint::EventRecording {
        pickups: true,
        food_births: true,
    });
    let mut reference = world.clone();
    reference.config.food_index = false;
    let actions = vec![
        Some(Action {
            heading: 0.,
            boost: false,
        });
        SIDE * SIDE
    ];
    for tick in 0..4 {
        world.step(&actions).unwrap();
        reference.step(&actions).unwrap();
        assert!(world.eater_grid.occupied().next().is_some());
        assert_eq!(world.pickup_events(), reference.pickup_events());
        assert_eq!(world.food_spawn_events(), reference.food_spawn_events());
        if tick == 0 {
            assert_eq!(world.pickup_events().unwrap().len(), 128);
            assert_eq!(world.food.len(), 128);
        }
        let actual = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
        let mut expected = serde_json::to_value(reference.checkpoint().unwrap()).unwrap();
        expected["config"]["foodIndex"] = Value::Bool(true);
        assert_eq!(actual, expected, "tick {tick}");
    }
}

fn normalized(world: &Simulator) -> Vec<u8> {
    let mut value = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
    let state = value["state"].as_object_mut().unwrap();
    let food = state.get_mut("food").unwrap().as_array_mut().unwrap();
    food.sort_unstable_by_key(|f| serde_json::to_vec(f).unwrap());
    state.remove("foodBirthOrder");
    // The index chooses candidates only; this is the independent linear path.
    value["config"]["foodIndex"] = Value::Bool(false);
    serde_json::to_vec(&value).unwrap()
}

fn chronological(world: &mut Simulator) {
    let mut rows: Vec<_> = world
        .food
        .iter()
        .copied()
        .zip(world.food_birth_order.iter().copied())
        .collect();
    rows.sort_unstable_by_key(|&(_, birth)| birth);
    for (i, (food, birth)) in rows.into_iter().enumerate() {
        world.food[i] = food;
        world.food_birth_order[i] = birth;
    }
    world.food_cache = FoodCache::default();
}

#[test]
fn unordered_storage_matches_chronological_linear_steps() {
    let mut rng = Random(0x5a17);
    // 60 scenes cover every combination of the size, arena, refill and radius
    // cycles below (lcm of 4, 3, 2 and 5); 300 scenes took over two minutes per
    // CI test configuration in debug builds.
    for scene in 0..60 {
        let n = [1, 8, 24, 80][scene % 4];
        let mut config = if scene % 3 == 0 {
            Config::official(n)
        } else {
            Config::sandbox()
        };
        config.food_count = 0;
        config.ambient_replenish_per_second = if scene % 2 == 0 { 500. } else { 0. };
        config.prey.count = 3;
        if scene % 5 == 0 {
            config.dynamic_radius = true;
            config.radius_updates = vec![
                crate::RadiusUpdate {
                    time: config.dt,
                    radius: 900.,
                },
                crate::RadiusUpdate {
                    time: 2. * config.dt,
                    radius: 850.,
                },
            ];
        }
        let mut candidate = Simulator::new(WorldOptions {
            seed: scene as u32 + 1,
            config,
            population: InitialPopulation::Placed {
                ids: (0..n).map(|i| format!("{i:03}")).collect(),
                placement: Placement::TestRing,
            },
        })
        .unwrap();
        let food: Vec<_> = (0..512)
            .map(|i| {
                let s = &candidate.snakes[i % n];
                let spread = if i % 5 == 0 { 3000. } else { 30. };
                Food {
                    x: s.head.x + (rng.next() - 0.5) * spread,
                    y: s.head.y + (rng.next() - 0.5) * spread,
                    value: rng.next(),
                    size: (i % 3 != 0).then(|| 1. + rng.next() * 12.),
                    origin: Some(Origin::Ambient),
                }
            })
            .collect();
        candidate.replace_food(food).unwrap();
        candidate.record_events(checkpoint::EventRecording {
            pickups: true,
            food_births: true,
        });
        let mut reference = candidate.clone();
        reference.config.food_index = false;
        // Distinct logical states include food moved to the front, exact aliases
        // and different sizes. Dense row order must never select nutrition order.
        for i in (1..candidate.food.len()).rev() {
            let j = (rng.next() * (i + 1) as f64) as usize;
            candidate.food.swap(i, j);
            candidate.food_birth_order.swap(i, j);
        }
        for tick in 0..6 {
            chronological(&mut reference);
            let actions: Vec<_> = candidate
                .snakes
                .iter()
                .map(|s| {
                    s.alive.then_some(Action {
                        heading: s.heading + (rng.next() - 0.5) * 0.5,
                        boost: tick % 2 == 0,
                    })
                })
                .collect();
            let a = candidate.step(&actions);
            let b = reference.step(&actions);
            assert_eq!(a, b, "scene {scene} tick {tick}");
            assert_eq!(candidate.pickup_events(), reference.pickup_events());
            assert_eq!(candidate.food_spawn_events(), reference.food_spawn_events());
            assert_eq!(
                normalized(&candidate),
                normalized(&reference),
                "scene {scene} tick {tick}"
            );
        }
    }
}

#[test]
fn birth_identity_survives_checkpoint_edits_and_sequence_exhaustion() {
    let mut world = Simulator::new(WorldOptions {
        config: Config {
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Config::sandbox()
        },
        ..Default::default()
    })
    .unwrap();
    for value in [1., 2., 3.] {
        world
            .insert_food(Food {
                x: value,
                y: 0.,
                value,
                size: None,
                origin: None,
            })
            .unwrap();
    }
    world.food.swap(0, 2);
    world.food_birth_order.swap(0, 2);
    world.next_food_birth = u64::MAX;
    world
        .insert_food(Food {
            x: 4.,
            y: 0.,
            value: 4.,
            size: None,
            origin: None,
        })
        .unwrap();
    assert_eq!(world.food_birth_order, [2, 1, 0, 3]);
    let bytes = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
    let restored = Simulator::from_checkpoint(&bytes, Default::default()).unwrap();
    assert_eq!(world.food, restored.food);
    assert_eq!(world.food_birth_order, restored.food_birth_order);
    assert_eq!(world.next_food_birth, restored.next_food_birth);
    for malformed in ["length", "duplicate", "future"] {
        let mut value: Value = serde_json::from_slice(&bytes).unwrap();
        match malformed {
            "length" => {
                value["state"]["foodBirthOrder"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            "duplicate" => {
                value["state"]["foodBirthOrder"][1] = value["state"]["foodBirthOrder"][0].clone()
            }
            _ => value["state"]["foodBirthOrder"][1] = value["state"]["nextFoodBirth"].clone(),
        }
        assert!(
            Simulator::from_checkpoint(&serde_json::to_vec(&value).unwrap(), Default::default())
                .is_err()
        );
    }
}

#[test]
fn terminal_cause_keeps_global_reverse_birth_order() {
    let mut candidate = Simulator::new(WorldOptions {
        config: Config {
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Config::sandbox()
        },
        population: InitialPopulation::Placed {
            ids: vec!["first".into(), "second".into()],
            placement: Placement::TestRing,
        },
        ..Default::default()
    })
    .unwrap();
    let food: Vec<_> = [0, 0, 1, 1]
        .into_iter()
        .map(|slot| {
            let s = &candidate.snakes[slot];
            let (p, _) = pickup_circle(s.head, s.heading, s.speed, s.segments);
            Food {
                x: p.x,
                y: p.y,
                value: f64::MAX,
                size: None,
                origin: None,
            }
        })
        .collect();
    candidate.replace_food(food).unwrap();
    candidate.record_events(checkpoint::EventRecording {
        pickups: true,
        food_births: true,
    });
    let mut reference = candidate.clone();
    reference.config.food_index = false;
    candidate.food.reverse();
    candidate.food_birth_order.reverse();
    let actions: Vec<_> = candidate
        .snakes
        .iter()
        .map(|s| {
            Some(Action {
                heading: s.heading,
                boost: false,
            })
        })
        .collect();
    let a = candidate.step(&actions);
    assert!(matches!(a, Err(CoreStepError::Stopped(_))));
    assert_eq!(a, reference.step(&actions));
    assert_eq!(candidate.pickup_events(), reference.pickup_events());
    assert_eq!(normalized(&candidate), normalized(&reference));
    let saved = normalized(&candidate);
    assert!(candidate.step(&actions).is_err());
    assert_eq!(normalized(&candidate), saved);
}
