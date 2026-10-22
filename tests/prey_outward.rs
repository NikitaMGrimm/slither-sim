use slither_sim::{Action, Config, Point, Prey, PreyMode, Simulator};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecordedClip {
    cohort: String,
    prey_id: u64,
    start_x: f64,
    start_y: f64,
    heading: f64,
    transmitted_target: f64,
    end_x: f64,
    end_y: f64,
    nominal_ticks: usize,
}

#[test]
fn recorded_outward_clips_match_production_motion_in_both_cohorts() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        cases: Vec<RecordedClip>,
    }
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/prey-outward-motion.json")).unwrap();
    assert_eq!(fixture.cases.len(), 62);
    for cohort in ["discovery", "validation"] {
        let mut errors = Vec::new();
        let mut baseline_errors = Vec::new();
        for clip in fixture.cases.iter().filter(|clip| clip.cohort == cohort) {
            let mut world = world(0.007);
            world.prey[0].id = clip.prey_id;
            world.prey[0].position = Point {
                x: clip.start_x,
                y: clip.start_y,
            };
            world.prey[0].heading = clip.heading;
            world.prey[0].target_heading = clip.transmitted_target;
            // Generous previous-rule comparator: keep the measured target and
            // omit random retargeting, but retain the former 3.800 baseline.
            let mut baseline = world.prey[0].clone();
            world
                .set_prey_mode(clip.prey_id, PreyMode::Outward)
                .unwrap();
            for _ in 0..clip.nominal_ticks {
                world.step(&[]).unwrap();
                baseline.advance_server_tick(world.config.prey.turn_rate);
            }
            let endpoint = Point {
                x: clip.end_x,
                y: clip.end_y,
            };
            errors.push(world.prey[0].position.distance(endpoint));
            baseline_errors.push(baseline.position.distance(endpoint));
        }
        errors.sort_by(f64::total_cmp);
        baseline_errors.sort_by(f64::total_cmp);
        assert_eq!(errors.len(), 31);
        let within_six = errors.iter().filter(|&&error| error <= 6.0).count();
        eprintln!(
            "{cohort}: median={}, maximum={}, within6={within_six}/31, baseline median={}",
            errors[15], errors[30], baseline_errors[15]
        );
        // All selected clips remain, including the discovery outlier. These
        // aggregate bounds describe this frozen corpus, not universal accuracy.
        assert!(errors[15] <= 3.0, "{cohort}: {errors:?}");
        assert!(errors[30] <= 9.0, "{cohort}: {errors:?}");
        assert!(within_six >= if cohort == "discovery" { 30 } else { 31 });
        assert!(baseline_errors[15] > 30.0);
    }
}

fn world(dt: f64) -> Simulator {
    let mut world = Simulator::new(
        42,
        Config {
            dt,
            radius: 30000.0,
            food_count: 0,
            ambient_replenish_per_second: 0.0,
            ..Config::default()
        },
    )
    .unwrap();
    world.prey.push(Prey {
        id: 100,
        position: Point {
            x: 2000.0,
            y: 3000.0,
        },
        heading: 0.0,
        target_heading: 0.0,
        speed: 3.8 / 0.032,
        size: 6.0,
    });
    world
}

#[test]
fn explicit_outward_state_sets_the_observed_baseline_and_persists() {
    let mut world = world(0.007);
    world.set_prey_mode(100, PreyMode::Outward).unwrap();
    assert_eq!(world.prey[0].speed, 6.06 / 0.032);
    assert_eq!(world.prey[0].target_heading, 3000.0_f64.atan2(2000.0));
    let initial_distance = world.prey[0].position.distance(Point::default());
    for _ in 0..400 {
        world.step(&[]).unwrap();
        assert_eq!(world.prey[0].speed, 6.06 / 0.032);
    }
    assert!(world.prey[0].position.distance(Point::default()) > initial_distance + 200.0);
}

#[test]
fn free_outward_motion_matches_short_batched_simulation_steps() {
    let run = |dt, steps| {
        let mut world = world(dt);
        world.set_prey_mode(100, PreyMode::Outward).unwrap();
        for _ in 0..steps {
            world.step(&[]).unwrap();
        }
        world.prey[0].clone()
    };
    assert_eq!(run(0.007, 32), run(0.028, 8));
    assert_eq!(run(0.007, 32), run(0.032, 7));
    assert_eq!(run(0.007, 32).speed, 6.06 / 0.032);
}

#[test]
fn clone_keeps_outward_state_without_exposing_it_in_observations() {
    let mut world = world(0.007);
    let before = serde_json::to_value(&world.prey[0]).unwrap();
    world.set_prey_mode(100, PreyMode::Outward).unwrap();
    for _ in 0..13 {
        world.step(&[]).unwrap();
    }
    let mut copy = world.clone();
    for _ in 0..50 {
        world.step(&[]).unwrap();
        copy.step(&[]).unwrap();
    }
    assert_eq!(world.prey, copy.prey);
    let after = serde_json::to_value(&world.prey[0]).unwrap();
    assert_eq!(
        before.as_object().unwrap().keys().collect::<Vec<_>>(),
        after.as_object().unwrap().keys().collect::<Vec<_>>()
    );
    assert!(world.set_prey_mode(999, PreyMode::Outward).is_err());
}

#[test]
fn removed_identity_does_not_lend_outward_state_to_a_new_entity() {
    let mut world = world(0.007);
    let original = world.prey[0].clone();
    world.set_prey_mode(100, PreyMode::Outward).unwrap();
    world.prey.clear();
    world.step(&[]).unwrap();
    world.prey.push(original);
    world.step(&[]).unwrap();
    assert_eq!(world.prey[0].speed, world.config.prey.cruise_speed);
}

#[test]
fn ordinary_threat_overrides_then_returns_to_outward_baseline() {
    let mut world = world(0.007);
    world.add("threat", 0, 1).unwrap();
    world.set_prey_mode(100, PreyMode::Outward).unwrap();
    for _ in 0..60 {
        // Keep the mouth pointing away, while the head remains in the threat box.
        let head = Point {
            x: world.prey[0].position.x + 200.0,
            y: world.prey[0].position.y,
        };
        world.snakes[0].head = head;
        world.snakes[0].heading = 0.0;
        world.snakes[0].body = vec![
            head,
            Point {
                x: head.x - 42.0,
                y: head.y,
            },
        ];
        world.step(&[Some(Action::default())]).unwrap();
        assert_eq!(world.prey.len(), 1);
    }
    assert!(world.prey[0].speed > 6.06 / 0.032);
    world.snakes[0].alive = false;
    for _ in 0..60 {
        world.step(&[None]).unwrap();
    }
    assert_eq!(world.prey[0].speed, 6.06 / 0.032);
    assert!(
        (world.prey[0].target_heading - world.prey[0].position.y.atan2(world.prey[0].position.x))
            .abs()
            < 0.03
    );
}

#[test]
fn outward_target_is_not_replaced_by_the_provisional_roaming_rim_turn() {
    let mut world = world(0.007);
    world.prey[0].position = Point { x: 28000.0, y: 0.0 };
    world.set_prey_mode(100, PreyMode::Outward).unwrap();
    for _ in 0..60 {
        world.step(&[]).unwrap();
    }
    assert_eq!(world.prey[0].target_heading, 0.0);
    assert!(world.prey[0].position.x > 28000.0);
    // Boundary removal itself is not inferred by this interior trajectory.
    assert!(world.prey[0].position.x < world.config.radius);
}

#[test]
fn resetting_mode_is_explicit_and_ambiguous_ids_are_rejected() {
    let mut world = world(0.007);
    world.set_prey_mode(100, PreyMode::Outward).unwrap();
    world.set_prey_mode(100, PreyMode::Roaming).unwrap();
    assert_eq!(world.prey[0].speed, world.config.prey.cruise_speed);
    world.prey.push(world.prey[0].clone());
    assert!(world.set_prey_mode(100, PreyMode::Outward).is_err());
    assert!(
        world
            .prey
            .iter()
            .all(|p| p.speed == world.config.prey.cruise_speed)
    );
}

#[test]
fn initializing_roaming_preserves_its_immediate_first_threat_decision() {
    let mut world = world(0.007);
    world.add("threat", 0, 1).unwrap();
    let head = Point {
        x: 2200.0,
        y: 3000.0,
    };
    world.snakes[0].head = head;
    world.snakes[0].heading = 0.0;
    world.snakes[0].body = vec![
        head,
        Point {
            x: 2158.0,
            y: 3000.0,
        },
    ];
    let mut explicit = world.clone();
    explicit.set_prey_mode(100, PreyMode::Roaming).unwrap();
    for _ in 0..5 {
        world.step(&[Some(Action::default())]).unwrap();
        explicit.step(&[Some(Action::default())]).unwrap();
        assert_eq!(world.prey, explicit.prey);
        assert!(explicit.prey[0].speed > explicit.config.prey.cruise_speed);
    }
}

#[test]
fn consumed_identity_does_not_transfer_outward_mode() {
    let mut world = world(0.007);
    let original = world.prey[0].clone();
    world.add("eater", 0, 1).unwrap();
    let head = Point {
        x: 1957.0,
        y: 3000.0,
    };
    world.snakes[0].head = head;
    world.snakes[0].heading = 0.0;
    world.snakes[0].body = vec![
        head,
        Point {
            x: 1915.0,
            y: 3000.0,
        },
    ];
    world.set_prey_mode(100, PreyMode::Outward).unwrap();
    world.step(&[Some(Action::default())]).unwrap();
    assert!(world.prey.is_empty());
    world.snakes[0].alive = false;
    world.prey.push(original);
    world.step(&[None]).unwrap();
    assert_eq!(world.prey[0].speed, world.config.prey.cruise_speed);
}
