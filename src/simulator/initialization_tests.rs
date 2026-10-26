use super::*;
use crate::{EventRecording, RestoreOptions};
use std::num::NonZeroU32;

fn world() -> Simulator {
    Simulator::new(WorldOptions {
        seed: 7,
        config: Config::sandbox(),
        population: InitialPopulation::Placed {
            ids: vec!["a".into()],
            placement: Placement::default(),
        },
    })
    .unwrap()
}
fn bytes(world: &Simulator) -> Vec<u8> {
    serde_json::to_vec(&world.checkpoint().unwrap()).unwrap()
}
fn life(world: &Simulator) -> ReplayLife {
    ReplayLife::from_observed(
        world.snakes()[0].clone(),
        InitialSpeed::Known(world.speeds[0]),
    )
}

#[test]
fn duplicate_or_invalid_append_changes_no_slot_companion_or_rng() {
    let mut world = world();
    let before = bytes(&world);
    assert_eq!(
        world.append_dormant("a".into()).unwrap_err().kind,
        ErrorKind::DuplicateId
    );
    assert_eq!(
        world.spawn("a".into(), vec![], 0.).unwrap_err().kind,
        ErrorKind::DuplicateId
    );
    assert!(
        world
            .spawn("b".into(), vec![Point { x: 1e10, y: 0. }; 2], 0.)
            .is_err()
    );
    assert_eq!(bytes(&world), before);
    world.next_join = u64::MAX - 1;
    let before = format!("{world:?}");
    assert_eq!(
        world.append_dormant("b".into()).unwrap_err().kind,
        ErrorKind::RepresentationLimit
    );
    assert_eq!(format!("{world:?}"), before);
}

#[test]
fn imports_reset_life_controls_and_keep_rolling_kills() {
    let mut world = world();
    world.config.kill_score_seconds = Some(20.);
    world.kill_times[0].push_back(0.);
    world.boosts[0].active_boost_seconds = 3.;
    world.kiosks[0].score_credit = 20.;
    let old_join = world.joins[0];
    let mut imported = life(&world);
    imported.speed = InitialSpeed::Known(SpeedState::with_progress(250., 0.3).unwrap());
    world.import_life(0, imported).unwrap();
    assert_eq!(world.snakes()[0].id, "a");
    assert_eq!(world.speeds[0].progress(), 0.3);
    assert_eq!(world.snakes()[0].speed, 250.);
    assert_eq!(
        world.boost_accounting(0).unwrap(),
        BoostAccounting::default()
    );
    assert_eq!(world.kiosks[0].score_credit, 0.);
    assert_eq!(world.death_time(0), None);
    assert_eq!(world.kill_score(0), Some(1));
    assert!(world.joins[0] > old_join);
    let before = bytes(&world);
    let mut bad = life(&world);
    bad.body.clear();
    assert!(world.import_life(0, bad).is_err());
    assert_eq!(bytes(&world), before);
}

#[test]
fn numerical_import_and_correction_rejections_are_atomic() {
    let mut world = world();
    let before = bytes(&world);
    let mut huge = life(&world);
    huge.head.x = f64::MAX;
    assert!(world.import_life(0, huge).is_err());
    let mut fast = life(&world);
    fast.speed = InitialSpeed::Known(SpeedState::idle(f64::MAX));
    assert!(world.import_life(0, fast).is_err());
    assert!(
        world
            .correct_pose(0, Point { x: 1e151, y: 0. }, vec![Point::default(); 2], 0.)
            .is_err()
    );
    assert!(
        world
            .insert_food(Food {
                x: f64::MAX,
                y: 0.,
                value: 1.,
                size: None,
                origin: None
            })
            .is_err()
    );
    assert_eq!(bytes(&world), before);
}

#[test]
fn targeted_corrections_preserve_hidden_controls_and_kiosk_credit() {
    let mut world = world();
    world.speeds[0] = SpeedState::with_progress(250., 0.4).unwrap();
    world.snakes[0].speed = 250.;
    world.boosts[0].active_boost_seconds = 1.5;
    world.kiosks[0].score_credit = 25.;
    world.kiosks[0].anchor = Some(Point::default());
    world.kiosks[0].held_seconds = 10.;
    let speed = world.speeds[0];
    let join = world.joins[0];
    world
        .correct_mass(0, vec![Point::default(); 20], 2.25)
        .unwrap();
    assert_eq!(
        world.snakes()[0].score,
        (score_exact(20, 2.25) + 25.).floor()
    );
    world
        .correct_pose(0, Point { x: 1e6, y: 0. }, vec![Point::default(); 20], 1.)
        .unwrap();
    assert_eq!(world.speeds[0], speed);
    assert_eq!(world.joins[0], join);
    assert_eq!(world.boosts[0].active_boost_seconds, 1.5);
    assert_eq!(world.kiosks[0].held_seconds, 10.);
    assert_eq!(world.kiosks[0].score_credit, 25.);
    let before = bytes(&world);
    assert!(
        world
            .correct_pose(0, Point::default(), vec![Point::default(); 2], 0.)
            .is_err()
    );
    assert!(
        world
            .correct_mass(0, vec![Point::default(); 20], f64::NAN)
            .is_err()
    );
    assert_eq!(bytes(&world), before);
}

#[test]
fn dormant_retire_and_respawn_keep_stable_slots_without_fabricated_deaths() {
    let mut world = world();
    let slot = world.append_dormant("waiting".into()).unwrap();
    assert_eq!(slot, 1);
    assert_eq!(world.death_time(slot), None);
    assert!(!world.snakes()[slot].alive);
    let food = world.food().len();
    world.retire(0).unwrap();
    assert_eq!(world.food().len(), food);
    assert!(world.deaths().is_empty());
    world.respawn(0, Point::default(), 0., 2).unwrap();
    assert!(world.snakes()[0].alive);
    assert_eq!(world.snakes()[0].id, "a");
    assert_eq!(
        world.boost_accounting(0).unwrap(),
        BoostAccounting::default()
    );
}

#[test]
fn checked_food_and_prey_edits_match_fresh_caches() {
    let mut world = world();
    world
        .step(&[Some(Action {
            heading: 0.,
            boost: false,
        })])
        .unwrap();
    let pellet = Food {
        x: 100.,
        y: 100.,
        value: 1.,
        size: None,
        origin: Some(Origin::Ambient),
    };
    world
        .replace_food(vec![pellet.clone(), pellet.clone()])
        .unwrap();
    assert_eq!(world.insert_food(pellet.clone()).unwrap(), 2);
    assert_eq!(world.remove_food(1).unwrap(), pellet);
    let prey = Prey {
        id: 90,
        position: Point { x: 500., y: 500. },
        heading: 0.,
        target_heading: 0.,
        speed: 10.,
        size: 6.,
    };
    world
        .insert_prey(ReplayPrey {
            prey: prey.clone(),
            mode: PreyMode::Outward,
        })
        .unwrap();
    assert!(world.prey_outward.contains(&90));
    assert!(world.prey_decision_times.contains_key(&90));
    let before = bytes(&world);
    assert!(
        world
            .insert_prey(ReplayPrey {
                prey: prey.clone(),
                mode: PreyMode::Roaming
            })
            .is_err()
    );
    assert!(
        world
            .replace_prey(vec![
                ReplayPrey {
                    prey: prey.clone(),
                    mode: PreyMode::Roaming
                };
                2
            ])
            .is_err()
    );
    assert_eq!(bytes(&world), before);
    world.remove_prey(90).unwrap();
    assert!(!world.prey_outward.contains(&90));
    assert!(!world.prey_decision_times.contains_key(&90));
    world
        .insert_prey(ReplayPrey {
            prey,
            mode: PreyMode::Roaming,
        })
        .unwrap();
    let mut fresh = Simulator::from_checkpoint(&bytes(&world), RestoreOptions::default()).unwrap();
    for _ in 0..20 {
        let actions = [Some(Action {
            heading: 0.,
            boost: false,
        })];
        world.step(&actions).unwrap();
        fresh.step(&actions).unwrap();
        assert_eq!(bytes(&world), bytes(&fresh));
    }
}

#[test]
fn placement_has_a_separate_deterministic_random_stream() {
    let options = WorldOptions {
        seed: 77,
        config: Config::sandbox(),
        population: InitialPopulation::Placed {
            ids: (0..8).map(|i| i.to_string()).collect(),
            placement: Placement::Admitted {
                attempts: NonZeroU32::new(64).unwrap(),
                preferred_clearance: 600.,
            },
        },
    };
    let first = Simulator::new(options.clone()).unwrap();
    let second = Simulator::new(options.clone()).unwrap();
    assert_eq!(bytes(&first), bytes(&second));
    let empty = Simulator::new(WorldOptions {
        population: InitialPopulation::Empty,
        ..options
    })
    .unwrap();
    assert_eq!(first.food(), empty.food());
    assert_eq!(first.prey(), empty.prey());
    assert_eq!(first.random.0, empty.random.0);
    assert_eq!(first.prey_random.0, empty.prey_random.0);
}

#[test]
fn scenario_bypasses_ecology_and_recording_reuses_capacity() {
    let mut world = Simulator::from_scenario(7, Config::default(), Scenario::default()).unwrap();
    assert!(world.food().is_empty());
    assert!(world.prey().is_empty());
    assert_eq!(world.random.0, 7);
    world.record_events(EventRecording {
        pickups: true,
        food_births: true,
    });
    world.pickup_events.as_mut().unwrap().reserve(10);
    let capacity = world.pickup_events.as_ref().unwrap().capacity();
    world.record_events(EventRecording {
        pickups: true,
        food_births: true,
    });
    assert_eq!(world.pickup_events.as_ref().unwrap().capacity(), capacity);
    world.record_events(EventRecording::default());
    assert_eq!(world.pickup_events(), None);
    assert_eq!(world.food_spawn_events(), None);
}
