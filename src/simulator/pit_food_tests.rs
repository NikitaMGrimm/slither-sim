//! Owner-directed pit ecology, separate from official food evidence.
use super::*;

fn world() -> Simulator {
    let mut config = Config::battledome();
    config.dynamic_radius = false;
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    config.death_food.enabled = false;
    config.boost_food.enabled = false;
    config.kiosk.enabled = false;
    let mut world = Simulator::new(WorldOptions {
        seed: 7,
        config,
        ..Default::default()
    })
    .unwrap();
    world.push_ring("floor".into(), 0, 1).unwrap();
    world.record_events(checkpoint::EventRecording {
        pickups: true,
        food_births: true,
    });
    world
}

#[test]
fn pit_food_is_explicit_and_requires_a_pit() {
    for config in [Config::official(1), Config::sandbox()] {
        assert!(!config.food_only_inside_pit);
        assert!(
            serde_json::to_value(config)
                .unwrap()
                .get("foodOnlyInsidePit")
                .is_none()
        );
    }
    let config: Config = serde_json::from_str(r#"{"preset":"battledome"}"#).unwrap();
    assert!(config.food_only_inside_pit);
    let mut invalid = Config::sandbox();
    invalid.food_only_inside_pit = true;
    assert_eq!(
        invalid.validate().unwrap_err().field.as_deref(),
        Some("pit")
    );
}

#[test]
fn ambient_target_and_new_credit_use_the_pit_area_fraction() {
    let preset = Simulator::new(WorldOptions {
        seed: 7,
        config: Config::battledome(),
        ..Default::default()
    })
    .unwrap();
    assert!(preset.food.len() <= 777);
    assert!(!preset.food.is_empty());
    assert!(
        preset
            .food
            .iter()
            .all(|f| f.point().distance(Point::default()) < 1784.)
    );
    let mut config = Config::sandbox();
    config.pit = Some(PitConfig { radius: 500. });
    config.food_only_inside_pit = true;
    config.food_count = 100;
    config.ambient_replenish_per_second = 8.;
    config.ambient_food.enabled = false;
    config.dt = 0.01;
    let mut world = Simulator::new(WorldOptions {
        seed: 7,
        config,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(world.food.len(), 25);
    assert!(
        world
            .food
            .iter()
            .all(|f| f.point().distance(Point::default()) < 500.)
    );
    world.replace_food(vec![]).unwrap();
    for _ in 0..100 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.food.len(), 2, "8 attempts/s times one quarter area");
    let mut restored = Simulator::from_checkpoint(
        &serde_json::to_vec(&world.checkpoint().unwrap()).unwrap(),
        checkpoint::RestoreOptions::default(),
    )
    .unwrap();
    for _ in 0..200 {
        world.step(&[]).unwrap();
        restored.step(&[]).unwrap();
    }
    assert_eq!(
        serde_json::to_value(world.checkpoint().unwrap()).unwrap(),
        serde_json::to_value(restored.checkpoint().unwrap()).unwrap()
    );
}

#[test]
fn a_floor_death_uses_the_ordinary_full_body_budget() {
    let mut world = world();
    let head = Point { x: 0., y: 0. };
    world.snakes[0].head = head;
    world.snakes[0].body.fill(head);
    assert_eq!(world.death_food(0), 53);
    assert!(
        world
            .food
            .iter()
            .all(|f| f.point().distance(Point::default()) < 1784.)
    );
    let mut legacy = world.clone();
    legacy.config.food_only_inside_pit = false;
    assert_eq!(
        legacy.death_food(0),
        0,
        "disabled custom worlds retain their floor mapping"
    );
}

#[test]
fn a_straddling_death_keeps_only_the_exact_inside_subset_without_moving_it() {
    let mut filtered = world();
    let pit = filtered.pit_radius().unwrap();
    filtered.snakes[0].head = Point {
        x: pit - 200.,
        y: 0.,
    };
    for (index, point) in filtered.snakes[0].body.iter_mut().enumerate() {
        *point = Point {
            x: pit + if index < 13 { -200. } else { 200. },
            y: 0.,
        };
    }
    let mut full = filtered.clone();
    full.config.food_only_inside_pit = false;
    full.min_mass = None;
    assert_eq!(full.death_food(0), 53);
    let expected: Vec<_> = full
        .food
        .iter()
        .filter(|f| f.point().distance(Point::default()) < pit)
        .cloned()
        .collect();
    assert!((20..=33).contains(&expected.len()), "{}", expected.len());
    assert_eq!(filtered.death_food(0), expected.len());
    assert_eq!(filtered.food, expected);
    assert_eq!(
        filtered.random.next(),
        full.random.next(),
        "discarded sites still consume ordinary scatter draws"
    );
    assert!(!filtered.food_position_allowed(Point { x: pit, y: 0. }));
    assert!(filtered.food_position_allowed(Point {
        x: pit.next_down(),
        y: 0.
    }));
}

#[test]
fn boost_outside_loses_spent_mass_and_floor_boost_stays_free() {
    for inside in [false, true] {
        let mut world = world();
        if inside {
            let offset = world.snakes[0].head;
            world.snakes[0].head = Point::default();
            for p in &mut world.snakes[0].body {
                p.x -= offset.x;
                p.y -= offset.y;
            }
        }
        world.snakes[0].fam += 5.;
        let mut emitted = 0;
        for _ in 0..100 {
            world
                .step(&[Some(Action {
                    heading: FRAC_PI_2,
                    boost: true,
                })])
                .unwrap();
            emitted += world
                .food_spawn_events()
                .unwrap()
                .iter()
                .filter(|e| e.food.origin == Some(Origin::Boost))
                .count();
        }
        assert!(world.boost_accounting(0).unwrap().boost_mass_spent > 0.);
        assert_eq!(emitted > 0, inside);
        assert!(
            world
                .food
                .iter()
                .all(|f| world.food_position_allowed(f.point()))
        );
    }
    let mut free = world();
    for _ in 0..100 {
        free.step(&[Some(Action {
            heading: FRAC_PI_2,
            boost: true,
        })])
        .unwrap();
    }
    assert_eq!(free.boost_accounting(0).unwrap().boost_mass_spent, 0.);
    assert!(free.food.is_empty());
}

#[test]
fn a_floor_pit_exit_drops_the_inside_body_but_outer_wall_deaths_drop_nothing() {
    for pit in [true, false] {
        let mut world = world();
        let edge = if pit {
            world.pit_radius().unwrap()
        } else {
            world.radius
        };
        world.snakes[0].head = Point {
            x: edge - 0.1,
            y: 0.,
        };
        for (i, p) in world.snakes[0].body.iter_mut().enumerate() {
            *p = Point {
                x: edge - 0.1 - i as f64 * 42.,
                y: 0.,
            };
        }
        world.snakes[0].heading = 0.;
        world.pit_entered[0] = pit;
        world
            .step(&[Some(Action {
                heading: 0.,
                boost: false,
            })])
            .unwrap();
        let death = world.deaths.last().unwrap();
        assert_eq!(
            death.reason,
            if pit {
                DeathReason::Pit
            } else {
                DeathReason::Boundary
            }
        );
        assert_eq!(death.food > 0, pit);
        assert!(
            world
                .food
                .iter()
                .all(|f| world.food_position_allowed(f.point()))
        );
    }
}

#[test]
fn moving_edges_remove_stranded_food_instead_of_relocating_it() {
    let mut world = world();
    world.config.dynamic_radius = true;
    world.config.radius_updates = vec![
        crate::RadiusUpdate {
            time: 0.007,
            radius: 7168.,
        },
        crate::RadiusUpdate {
            time: 0.014,
            radius: 3584.,
        },
    ];
    world.retire(0).unwrap();
    for (x, origin) in [
        (1700., Origin::Ambient),
        (1784., Origin::Boost),
        (3000., Origin::Death),
    ] {
        let mut pellet = Simulator::pellet(Point { x, y: 0. }, 5., origin);
        if x == 1784. {
            pellet.x = x;
        } // Exact edge, independent of food-grid rounding.
        world.insert_food(pellet).unwrap();
    }
    world.step(&[None]).unwrap();
    assert_eq!(world.food.len(), 3);
    world.step(&[None]).unwrap();
    assert_eq!(world.food.len(), 1);
    assert_eq!(world.food[0].x, quantize_food(1700.));
    assert!(world.food[0].point().distance(Point::default()) < world.pit_radius().unwrap());
}
