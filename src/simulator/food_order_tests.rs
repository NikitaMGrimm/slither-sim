use super::*;
use serde_json::Value;

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
    for scene in 0..300 {
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
