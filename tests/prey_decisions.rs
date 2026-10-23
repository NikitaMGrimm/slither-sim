use slither_sim::{Action, Config, Point, Prey, Simulator};

fn world() -> Simulator {
    let mut world = Simulator::new(
        42,
        Config {
            radius: 10000.0,
            food_count: 0,
            ambient_replenish_per_second: 0.0,
            ..Config::sandbox()
        },
    )
    .unwrap();
    world.add("threat", 0, 1).unwrap();
    world.snakes[0].alive = false;
    world
}

fn prey(id: u64) -> Prey {
    Prey {
        id,
        position: Point { x: 2000.0, y: 0.0 },
        heading: 0.0,
        target_heading: 0.0,
        speed: 3.8 / 0.032,
        size: 6.0,
    }
}

fn step(world: &mut Simulator, threat: bool) {
    world.snakes[0].alive = threat;
    // Stay near the prey while keeping its pickup circle well behind it.
    world.snakes[0].head = Point {
        x: world.prey[0].position.x - 200.0,
        y: world.prey[0].position.y,
    };
    world.snakes[0].heading = 0.0;
    world
        .step(&[Some(Action {
            heading: 0.0,
            boost: false,
        })])
        .unwrap();
}

#[test]
fn entering_and_leaving_threat_wait_for_next_decision() {
    let mut world = world();
    world.prey.push(prey(100));
    step(&mut world, false); // Immediate first decision at t=0.
    let resting_target = world.prey[0].target_heading;
    for _ in 0..47 {
        step(&mut world, true);
        assert_eq!(world.prey[0].speed, world.config.prey.cruise_speed);
        assert_eq!(world.prey[0].target_heading, resting_target);
    }
    step(&mut world, true); // t=.336: first due decision.
    let flight = world.prey[0].speed;
    let fleeing_target = world.prey[0].target_heading;
    assert!(flight > world.config.prey.cruise_speed);
    for _ in 0..47 {
        step(&mut world, false);
        assert_eq!(world.prey[0].speed, flight);
        assert_eq!(world.prey[0].target_heading, fleeing_target);
    }
    step(&mut world, false);
    assert_eq!(world.prey[0].speed, world.config.prey.cruise_speed);
}

#[test]
fn later_introduction_has_its_own_phase_and_clone_preserves_it() {
    let mut world = world();
    world.prey.push(prey(100));
    for _ in 0..10 {
        step(&mut world, false);
    }
    world.prey.push(prey(101));
    step(&mut world, false); // Introduced at t=.070.
    let mut cloned = world.clone();
    for _ in 0..37 {
        step(&mut world, true);
        step(&mut cloned, true);
    }
    step(&mut world, true);
    step(&mut cloned, true); // t=.336.
    assert!(world.prey[0].speed > world.config.prey.cruise_speed);
    assert_eq!(world.prey[1].speed, world.config.prey.cruise_speed);
    for _ in 0..10 {
        step(&mut world, true);
        step(&mut cloned, true);
    }
    assert!(world.prey[1].speed > world.config.prey.cruise_speed);
    assert_eq!(world.prey, cloned.prey);
}

#[test]
fn removal_and_reinsertion_resets_a_present_identity() {
    let mut world = world();
    world.prey.push(prey(100));
    step(&mut world, false);
    world.prey.clear();
    world.step(&[None]).unwrap();
    world.prey.push(prey(100));
    step(&mut world, true);
    assert!(world.prey[0].speed > world.config.prey.cruise_speed);
}

#[test]
fn replenishment_does_not_reuse_an_injected_id_or_its_decision_phase() {
    let mut world = world();
    world.config.prey.count = 2;
    world.config.prey.respawn_seconds = 0.007;
    world.prey.push(prey(0));
    step(&mut world, false);
    assert_eq!(world.prey.len(), 2);
    assert_ne!(world.prey[0].id, world.prey[1].id);
    world.prey[1].position = world.prey[0].position;
    step(&mut world, true);
    assert_eq!(world.prey[0].speed, world.config.prey.cruise_speed);
    assert!(world.prey[1].speed > world.config.prey.cruise_speed);
}

#[test]
fn consumed_identity_is_ready_for_an_immediate_fresh_decision() {
    let mut world = world();
    world.prey.push(prey(100));
    step(&mut world, false);
    world.snakes[0].alive = true;
    world.snakes[0].head = Point {
        x: world.prey[0].position.x - 43.0,
        y: world.prey[0].position.y,
    };
    world.step(&[Some(Action::default())]).unwrap();
    assert!(world.prey.is_empty());
    world.prey.push(prey(100));
    step(&mut world, true);
    assert!(world.prey[0].speed > world.config.prey.cruise_speed);
}
