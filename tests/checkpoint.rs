use serde_json::{Value, json};
use slither_sim::{
    Action, Config, Constraint, CoreStepError, ErrorKind, EventRecording, Food, Point, PreyMode,
    RadiusUpdate, RestoreOptions, Simulator, WorldStatus, physics,
};
use std::f64::consts::{FRAC_PI_2, PI};

fn options() -> RestoreOptions {
    RestoreOptions {
        events: EventRecording {
            pickups: true,
            food_births: true,
        },
        #[cfg(feature = "parallel")]
        parallel: false,
    }
}
fn value(world: &Simulator) -> Value {
    serde_json::to_value(world.checkpoint().unwrap()).unwrap()
}
fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}
fn world() -> Simulator {
    let mut world = Simulator::new(
        7,
        Config {
            food_count: 10,
            ambient_replenish_per_second: 20.,
            prey: slither_sim::PreyConfig {
                count: 3,
                respawn_seconds: 0.021,
                ..Default::default()
            },
            ..Config::sandbox()
        },
    )
    .unwrap();
    for i in 0..3 {
        world.add(i.to_string(), i, 3).unwrap();
    }
    world.pickup_events = Some(Vec::new());
    world.food_spawn_events = Some(Vec::new());
    world
}
fn rejects_atomic(incumbent: &mut Simulator, bad: &Value) -> slither_sim::SimError {
    let before = value(incumbent);
    let events = (
        incumbent.pickup_events.clone(),
        incumbent.food_spawn_events.clone(),
    );
    let cause = incumbent.restore(&bytes(bad), options()).unwrap_err();
    assert!(matches!(
        cause.kind,
        ErrorKind::InvalidCheckpoint { .. } | ErrorKind::UnsupportedCheckpoint { .. }
    ));
    assert!(cause.field.as_ref().is_some_and(|field| !field.is_empty()));
    assert_eq!(value(incumbent), before);
    assert_eq!(
        (
            incumbent.pickup_events.clone(),
            incumbent.food_spawn_events.clone()
        ),
        events
    );
    cause
}

#[test]
fn full_schema_and_every_companion_are_required() {
    let mut incumbent = world();
    let original = value(&incumbent);
    for field in [
        "snake",
        "boost",
        "speed",
        "kiosk",
        "pitEntered",
        "deathTime",
        "join",
        "killTimes",
    ] {
        let mut bad = original.clone();
        bad["state"]["slots"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        let cause = rejects_atomic(&mut incumbent, &bad);
        assert_eq!(
            cause.kind,
            ErrorKind::InvalidCheckpoint {
                rule: Constraint::Missing
            }
        );
        assert_eq!(
            cause.field.as_deref(),
            Some(format!("state.slots[0].{field}").as_str())
        );
    }
    for (group, field) in [
        ("boost", "clock"),
        ("boost", "held"),
        ("speed", "progress"),
        ("kiosk", "anchor"),
    ] {
        let mut bad = original.clone();
        bad["state"]["slots"][0][group]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            rejects_atomic(&mut incumbent, &bad).kind,
            ErrorKind::InvalidCheckpoint {
                rule: Constraint::Missing
            }
        );
    }
    // The reported old boosts=[] input is no longer an importable Simulator.
    let mut old = json!({"boosts": [], "config": original["config"], "snakes": []});
    assert_eq!(
        rejects_atomic(&mut incumbent, &old).field.as_deref(),
        Some("format")
    );
    old = original.clone();
    old["state"]["boosts"] = json!([]);
    rejects_atomic(&mut incumbent, &old);
    old = original.clone();
    old["state"]["slots"][0]["boost"] = json!([]);
    rejects_atomic(&mut incumbent, &old);
    old = original.clone();
    old["state"]["slots"][0]["snake"]["body"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert_eq!(
        rejects_atomic(&mut incumbent, &old).kind,
        ErrorKind::InvalidCheckpoint {
            rule: Constraint::LengthMismatch
        }
    );
    for field in [
        "random",
        "preyRandom",
        "preyMotionClock",
        "minMass",
        "failure",
        "nextJoin",
    ] {
        let mut bad = original.clone();
        bad["state"].as_object_mut().unwrap().remove(field);
        assert_eq!(
            rejects_atomic(&mut incumbent, &bad).kind,
            ErrorKind::InvalidCheckpoint {
                rule: Constraint::Missing
            }
        );
    }
    for field in ["nextDecision", "mode"] {
        let mut bad = original.clone();
        bad["state"]["prey"][0]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            rejects_atomic(&mut incumbent, &bad).kind,
            ErrorKind::InvalidCheckpoint {
                rule: Constraint::Missing
            }
        );
    }
}

#[test]
fn malformed_state_versions_and_unknown_fields_reject_atomically() {
    let mut incumbent = world();
    let original = value(&incumbent);
    for (path, replacement) in [
        (
            "/state/slots/1/snake/id",
            original["state"]["slots"][0]["snake"]["id"].clone(),
        ),
        (
            "/state/slots/1/join",
            original["state"]["slots"][0]["join"].clone(),
        ),
        ("/state/slots/0/speed/progress", json!(1.1)),
        ("/state/slots/0/boost/active", json!(true)),
        ("/state/slots/0/boost/clock", json!(-0.1)),
        ("/state/slots/0/boost/activeBoostSeconds", json!(-1)),
        ("/state/slots/0/kiosk/heldSeconds", json!(-1)),
        ("/state/slots/0/kiosk/scoreCredit", json!(-1)),
        ("/state/slots/0/pitEntered", json!(true)),
        ("/state/slots/0/deathTime", json!(1.)),
        ("/state/slots/0/killTimes", json!([1., 0.])),
        ("/state/slots/0/snake/segments", json!(usize::MAX)),
        ("/state/slots/0/speed/speed", json!(f64::MAX)),
        ("/state/slots/0/snake/length", json!(999)),
        ("/state/slots/0/snake/score", json!(999)),
        ("/state/slots/0/snake/radius", json!(999)),
        ("/state/slots/0/snake/fam", json!(-1)),
        ("/state/slots/0/snake/head/x", Value::Null),
        ("/state/radiusUpdateIndex", json!(1)),
        ("/state/preyMotionClock", json!(physics::LOOP_SECONDS)),
        ("/state/preyClock", json!(100)),
        ("/state/ambientClock", json!(100)),
        ("/state/time", json!(-1)),
        ("/state/radius", json!(0)),
        ("/state/nextJoin", json!(0)),
        ("/state/nextJoin", json!(u64::MAX)),
        ("/state/minMass", json!({"segments":2,"fam":0})),
        (
            "/state/prey/1/id",
            original["state"]["prey"][0]["id"].clone(),
        ),
        ("/state/prey/0/size", json!(0)),
        ("/state/prey/0/size", json!(1e200)),
        ("/state/prey/0/nextDecision", json!(-1)),
        ("/state/prey/0/mode", json!("unknown")),
        ("/state/food/0/value", json!(-1)),
        ("/state/food/0/size", json!(-1)),
        ("/state/food/0/origin", json!("unknown")),
    ] {
        let mut bad = original.clone();
        *bad.pointer_mut(path).unwrap() = replacement;
        let cause = rejects_atomic(&mut incumbent, &bad);
        assert!(
            matches!(cause.kind, ErrorKind::InvalidCheckpoint { .. }),
            "{path}: {cause}"
        );
    }
    for field in ["format", "rulesRevision"] {
        let mut bad = original.clone();
        bad[field] = json!("future");
        let cause = rejects_atomic(&mut incumbent, &bad);
        assert!(matches!(
            cause.kind,
            ErrorKind::UnsupportedCheckpoint { .. }
        ));
        assert_eq!(cause.field.as_deref(), Some(field));
    }
    for path in [
        "",
        "/state",
        "/state/slots/0/snake/head",
        "/config",
        "/config/prey",
        "/producer",
    ] {
        let mut bad = original.clone();
        bad.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(1));
        rejects_atomic(&mut incumbent, &bad);
    }
    for path in ["/config/dt", "/config/prey/size", "/config/kiosk/enabled"] {
        let mut bad = original.clone();
        let (parent, field) = path.rsplit_once('/').unwrap();
        bad.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            rejects_atomic(&mut incumbent, &bad).kind,
            ErrorKind::InvalidCheckpoint {
                rule: Constraint::Missing
            }
        );
    }
    for input in [b"{}".as_slice(), b"null", b"[]", b"{", b"1e999"] {
        let before = value(&incumbent);
        assert!(matches!(
            incumbent.restore(input, options()).unwrap_err().kind,
            ErrorKind::InvalidCheckpoint { .. }
        ));
        assert_eq!(value(&incumbent), before);
    }
}

#[test]
fn debug_snapshot_is_full_precision_but_not_a_checkpoint() {
    let mut session = slither_sim::transport::Session::new();
    session.request(br#"{"op":"reset","agents":1}"#);
    let snapshot: Value =
        serde_json::from_slice(&session.request(br#"{"op":"snapshot"}"#)).unwrap();
    let cause = Simulator::from_checkpoint(&bytes(&snapshot["result"]), options()).unwrap_err();
    assert_eq!(
        cause.kind,
        ErrorKind::InvalidCheckpoint {
            rule: Constraint::Missing
        }
    );
    assert_eq!(cause.field.as_deref(), Some("format"));
}

#[test]
fn restore_options_choose_fresh_event_buffers_and_parallelism() {
    let mut source = world();
    #[cfg(feature = "parallel")]
    source.set_parallel(true);
    source.step(&[Some(Action::default()); 3]).unwrap();
    let saved = value(&source);
    let plain = Simulator::from_checkpoint(&bytes(&saved), RestoreOptions::default()).unwrap();
    assert_eq!(value(&plain), saved);
    assert!(plain.pickup_events.is_none() && plain.food_spawn_events.is_none());
    #[cfg(feature = "parallel")]
    assert!(!plain.parallel());
    let chosen = Simulator::from_checkpoint(
        &bytes(&saved),
        RestoreOptions {
            events: EventRecording {
                pickups: true,
                food_births: true,
            },
            #[cfg(feature = "parallel")]
            parallel: true,
        },
    )
    .unwrap();
    assert_eq!(value(&chosen), saved);
    assert_eq!(chosen.pickup_events, Some(Vec::new()));
    assert_eq!(chosen.food_spawn_events, Some(Vec::new()));
    #[cfg(feature = "parallel")]
    assert!(chosen.parallel());
    assert_eq!(source.observe_all(), chosen.observe_all());
}

#[test]
fn valid_replay_states_are_not_restricted_to_spawn_invariants() {
    let mut source = world();
    for _ in 0..5 {
        source.step(&[Some(Action::default()); 3]).unwrap();
    }
    let mut checkpoint = value(&source);
    let snake = &mut source.snakes[0];
    snake.segments = 512;
    snake.fam = 3.25;
    snake.body = vec![Point { x: 2000., y: 0. }; 512];
    snake.head = Point { x: 2001., y: 0. };
    Simulator::update_mass(snake);
    checkpoint["state"]["slots"][0]["snake"] = serde_json::to_value(snake).unwrap();
    checkpoint["state"]["slots"][0]["speed"] = json!({"speed":200.,"progress":0.});
    checkpoint["state"]["prey"][0]["size"] = json!(18.8);
    checkpoint["state"]["prey"][0]["nextDecision"] = json!(0.);
    checkpoint["state"]["prey"][0]["mode"] = json!("outward");
    checkpoint["state"]["nextPreyId"] = json!(0);
    checkpoint["producer"] = json!({"version":"not this version","commit":"provenance-only"});
    let restored = Simulator::from_checkpoint(&bytes(&checkpoint), options()).unwrap();
    let state = value(&restored);
    assert_eq!(state["state"], checkpoint["state"]);
    assert_eq!(restored.snakes[0].segments, 512);
    assert_ne!(restored.snakes[0].head, restored.snakes[0].body[0]);
}

fn intervention(world: &mut Simulator, tick: usize) -> bool {
    let mut respawned = false;
    for i in 0..world.snakes.len() {
        if !world.snakes[i].alive {
            world
                .respawn(
                    i,
                    Point {
                        x: -300. + i as f64 * 300.,
                        y: -300. + i as f64 * 300.,
                    },
                    0.,
                    12,
                )
                .unwrap();
            respawned = true;
        }
    }
    if tick.is_multiple_of(41) {
        world.food.clear();
    }
    let snake = &world.snakes[0];
    let (point, _) = physics::pickup_circle(snake.head, snake.heading, snake.speed, snake.segments);
    if tick.is_multiple_of(17) {
        world.food.push(Food {
            x: point.x,
            y: point.y,
            value: 0.5,
            size: None,
            origin: None,
        });
    }
    if tick.is_multiple_of(37) && !world.prey.is_empty() {
        world.prey[0].position = point;
        world.prey[0].speed = 0.;
    }
    if tick % 79 == 20 {
        world.snakes[0].head.x = world.config.radius * 2.;
    }
    respawned
}

#[test]
fn continuation_matches_for_320_ticks_across_every_lifecycle_phase() {
    let config = Config {
        radius: 1200.,
        radius_updates: vec![
            RadiusUpdate {
                time: 0.,
                radius: 1200.,
            },
            RadiusUpdate {
                time: 0.7,
                radius: 1250.,
            },
            RadiusUpdate {
                time: 1.8,
                radius: 1300.,
            },
        ],
        food_count: 20,
        ambient_replenish_per_second: 50.,
        prey: slither_sim::PreyConfig {
            count: 3,
            respawn_seconds: 0.021,
            ..Default::default()
        },
        ..Config::sandbox()
    };
    let mut uninterrupted = Simulator::new(91, config).unwrap();
    for i in 0..3 {
        uninterrupted.add(i.to_string(), i, 3).unwrap();
        uninterrupted.snakes[i].alive = false;
        uninterrupted
            .respawn(
                i,
                Point {
                    x: -300. + i as f64 * 300.,
                    y: -300. + i as f64 * 300.,
                },
                0.,
                12,
            )
            .unwrap();
    }
    uninterrupted.pickup_events = Some(Vec::new());
    uninterrupted.food_spawn_events = Some(Vec::new());
    let actions = |w: &Simulator, tick: usize| {
        w.snakes
            .iter()
            .map(|s| {
                s.alive.then_some(Action {
                    heading: s.heading + 0.01,
                    boost: tick % 30 < 15,
                })
            })
            .collect::<Vec<_>>()
    };
    for tick in 0..40 {
        intervention(&mut uninterrupted, tick);
        uninterrupted.step(&actions(&uninterrupted, tick)).unwrap();
    }
    let encoded = bytes(&value(&uninterrupted));
    let mut resumed = Simulator::from_checkpoint(&encoded, options()).unwrap();
    assert_eq!(value(&uninterrupted), value(&resumed));
    assert_eq!(resumed.pickup_events, Some(Vec::new()));
    assert_eq!(resumed.food_spawn_events, Some(Vec::new()));
    let mut respawns = 0;
    let mut pickups = 0;
    let mut prey_pickups = 0;
    let mut births = 0;
    for tick in 40..360 {
        respawns += usize::from(intervention(&mut uninterrupted, tick));
        intervention(&mut resumed, tick);
        let a = actions(&uninterrupted, tick);
        uninterrupted.step(&a).unwrap();
        resumed.step(&a).unwrap();
        assert_eq!(
            bytes(&value(&uninterrupted)),
            bytes(&value(&resumed)),
            "tick {tick}"
        );
        assert_eq!(
            serde_json::to_vec(&uninterrupted.observe_all()).unwrap(),
            serde_json::to_vec(&resumed.observe_all()).unwrap(),
            "observations {tick}"
        );
        assert_eq!(
            uninterrupted.pickup_events, resumed.pickup_events,
            "pickups {tick}"
        );
        assert_eq!(
            uninterrupted.food_spawn_events, resumed.food_spawn_events,
            "births {tick}"
        );
        pickups += uninterrupted.pickup_events.as_ref().unwrap().len();
        prey_pickups += uninterrupted
            .pickup_events
            .as_ref()
            .unwrap()
            .iter()
            .filter(|e| matches!(e.item, slither_sim::PickupItem::Prey(_)))
            .count();
        births += uninterrupted.food_spawn_events.as_ref().unwrap().len();
    }
    assert!(respawns > 0 && pickups > 0 && prey_pickups > 0 && births > 0);
    assert!(!uninterrupted.deaths.is_empty());
    assert!(uninterrupted.snakes.iter().enumerate().any(|(i, _)| {
        uninterrupted
            .boost_accounting(i)
            .unwrap()
            .active_boost_seconds
            > 0.
    }));
    assert_eq!(value(&uninterrupted)["state"]["radiusUpdateIndex"], 3);
    assert_eq!(uninterrupted.config.radius, 1300.);
}

#[test]
fn stopped_checkpoint_stays_stopped_and_retry_changes_nothing() {
    let mut source = world();
    let s = &source.snakes[0];
    let (p, _) = physics::pickup_circle(s.head, s.heading, s.speed, s.segments);
    source.food = vec![
        Food {
            x: p.x,
            y: p.y,
            value: f64::MAX,
            size: None,
            origin: None
        };
        2
    ];
    let a = source
        .snakes
        .iter()
        .map(|s| {
            Some(Action {
                heading: s.heading,
                boost: false,
            })
        })
        .collect::<Vec<_>>();
    assert!(matches!(source.step(&a), Err(CoreStepError::Stopped(_))));
    let original = value(&source);
    let mut restored = Simulator::from_checkpoint(&bytes(&original), options()).unwrap();
    assert_eq!(value(&restored), original);
    assert!(
        matches!(restored.status(),WorldStatus::Stopped(e) if e.kind == ErrorKind::NumericOverflow)
    );
    assert!(
        matches!(restored.step(&a),Err(CoreStepError::Rejected(ref e)) if e.kind == ErrorKind::WorldStopped)
    );
    assert_eq!(value(&restored), original);
}

#[test]
fn nonfinite_prey_motion_cause_survives_checkpoint_restore() {
    let mut source = Simulator::new(
        7,
        Config {
            dt: 2.,
            boundary_mode: slither_sim::BoundaryMode::Hard,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            prey: slither_sim::PreyConfig {
                count: 1,
                cruise_speed: f64::MAX,
                escape_speed: f64::MAX,
                sprint_speed: f64::MAX,
                turn_rate: 0.,
                ..Default::default()
            },
            ..Config::sandbox()
        },
    )
    .unwrap();
    assert!(matches!(source.step(&[]), Err(CoreStepError::Stopped(_))));
    let original = value(&source);
    let mut restored = Simulator::from_checkpoint(&bytes(&original), options()).unwrap();
    assert_eq!(value(&restored), original);
    assert!(
        matches!(restored.status(),WorldStatus::Stopped(e) if e.kind == ErrorKind::NumericOverflow && e.field.as_deref() == Some("prey.position"))
    );
    assert!(
        matches!(restored.step(&[]),Err(CoreStepError::Rejected(ref e)) if e.kind == ErrorKind::WorldStopped)
    );
    assert_eq!(value(&restored), original);
}

#[test]
fn mass_floor_kiosk_credit_and_cross_life_kill_history_round_trip() {
    let source = Simulator::new(
        7,
        Config {
            radius: 1000.,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            spawn_radius: Some(500.),
            spawn_radius_max: None,
            pit: Some(slither_sim::PitConfig { radius: 100. }),
            kill_score_seconds: Some(2.),
            ..Config::battledome()
        },
    )
    .unwrap();
    let mut source = source;
    source.add("a", 0, 1).unwrap();
    let mut saved = value(&source);
    saved["state"]["slots"][0]["kiosk"]["scoreCredit"] = json!(37.);
    saved["state"]["slots"][0]["snake"]["score"] = json!(437.);
    saved["state"]["slots"][0]["killTimes"] = json!([0.]);
    let restored = Simulator::from_checkpoint(&bytes(&saved), options()).unwrap();
    assert_eq!(value(&restored)["state"], saved["state"]);
    assert_eq!(restored.kill_score(0), Some(1));
    saved["state"]["minMass"]["fam"] = json!(0.);
    assert!(matches!(
        Simulator::from_checkpoint(&bytes(&saved), options())
            .unwrap_err()
            .kind,
        ErrorKind::InvalidCheckpoint {
            rule: Constraint::InconsistentState
        }
    ));
}

#[test]
fn export_rejects_invalid_public_edits_and_orphan_prey_controls() {
    for change in [
        (|w: &mut Simulator| w.snakes[0].fam = f64::NAN) as fn(&mut Simulator),
        |w| w.snakes[0].body.clear(),
        |w| w.snakes[1].id = w.snakes[0].id.clone(),
        |w| w.food[0].x = f64::INFINITY,
        // Finite but large enough to overflow continuation arithmetic.
        |w| w.food[0].x = 1e200,
        |w| w.snakes[0].speed = 1e200,
        |w| {
            let s = &mut w.snakes[0];
            s.body[1].x = 1e200;
            Simulator::update_mass(s);
        },
        |w| w.prey[0].size = 0.,
        |w| w.config.dt = -1.,
        |w| {
            let id = w.prey[0].id;
            w.set_prey_mode(id, PreyMode::Outward).unwrap();
            w.prey.clear();
        },
    ] {
        let mut source = world();
        change(&mut source);
        assert!(matches!(
            source.checkpoint().unwrap_err().kind,
            ErrorKind::InvalidCheckpoint { .. }
        ));
    }
}

#[test]
fn restored_kill_counts_saturate_when_one_tick_credits_several() {
    let mut world = Simulator::new(
        7,
        Config {
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Config::sandbox()
        },
    )
    .unwrap();
    for i in 0..3 {
        world.add(i.to_string(), i, 3).unwrap();
    }
    // Two heads beside the third snake's body, both lethal next tick.
    for (i, y) in [(0, 0.), (1, 84.)] {
        let s = &mut world.snakes[i];
        s.head = Point { x: 0., y };
        s.heading = PI;
        s.body = vec![s.head; 30];
        s.segments = 30;
        Simulator::update_mass(s);
    }
    let s = &mut world.snakes[2];
    s.head = Point { x: -25., y: 342. };
    s.heading = FRAC_PI_2;
    s.body = (0..11)
        .map(|i| Point {
            x: -25.,
            y: 300. - 42. * i as f64,
        })
        .collect();
    s.segments = 11;
    Simulator::update_mass(s);
    let mut source = value(&world);
    source["state"]["slots"][2]["snake"]["kills"] = json!(usize::MAX - 1);
    let mut world = Simulator::from_checkpoint(&bytes(&source), options()).unwrap();
    let actions: Vec<_> = world
        .snakes
        .iter()
        .map(|s| {
            Some(Action {
                heading: s.heading,
                boost: false,
            })
        })
        .collect();
    world.step(&actions).unwrap();
    let credited = |d: &&slither_sim::DeathEvent| d.killer.as_deref() == Some("2");
    assert_eq!(world.deaths.iter().filter(credited).count(), 2);
    assert_eq!(world.snakes[2].kills, usize::MAX);
}

#[test]
fn live_radius_inside_the_initial_spawn_ring_still_checkpoints() {
    let mut config = Config {
        food_count: 0,
        ambient_replenish_per_second: 0.,
        spawn_radius: Some(500.),
        radius_updates: vec![RadiusUpdate {
            time: 0.007,
            radius: 550.,
        }],
        ..Config::sandbox()
    };
    config.kiosk.enabled = false;
    let mut world = Simulator::new(7, config).unwrap();
    world.add("a", 0, 1).unwrap();
    let heading = world.snakes[0].heading;
    world
        .step(&[Some(Action {
            heading,
            boost: false,
        })])
        .unwrap();
    assert_eq!(world.config.radius, 550.);
    let saved = value(&world);
    let restored = Simulator::from_checkpoint(&bytes(&saved), options()).unwrap();
    assert_eq!(value(&restored), saved);
}

#[test]
fn load_does_not_allocate_from_population_targets_or_require_initial_live_radius_equality() {
    let mut source = value(&world());
    source["config"]["foodCount"] = json!(usize::MAX);
    source["config"]["prey"]["count"] = json!(usize::MAX);
    source["config"]["radius"] = json!(2000.);
    source["state"]["radius"] = json!(1000.);
    let loaded = Simulator::from_checkpoint(&bytes(&source), options()).unwrap();
    assert_eq!(
        loaded.food.len(),
        source["state"]["food"].as_array().unwrap().len()
    );
    assert_eq!(
        loaded.prey.len(),
        source["state"]["prey"].as_array().unwrap().len()
    );
    assert_eq!(loaded.config.radius, 1000.);
}

#[test]
fn valid_refill_credit_and_large_batched_motion_roundoff_are_preserved() {
    let mut source = Simulator::new(
        7,
        Config {
            dt: f64::MIN_POSITIVE,
            food_count: 0,
            ambient_replenish_per_second: 1e13,
            ..Config::sandbox()
        },
    )
    .unwrap();
    source.step(&[]).unwrap();
    assert!(value(&source)["state"]["ambientClock"].as_f64().unwrap() < 0.);
    source.set_ambient_rate(1e5).unwrap();
    let saved = value(&source);
    let restored = Simulator::from_checkpoint(&bytes(&saved), options()).unwrap();
    assert_eq!(value(&restored), saved);
    for dt in [
        1e6,
        1e10,
        1e13,
        physics::LOOP_SECONDS * 2f64.powi(53) * 0.999,
    ] {
        let mut source = Simulator::new(
            7,
            Config {
                dt,
                boundary_mode: slither_sim::BoundaryMode::Hard,
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::sandbox()
            },
        )
        .unwrap();
        source.step(&[]).unwrap();
        let saved = value(&source);
        let mut restored = Simulator::from_checkpoint(&bytes(&saved), options()).unwrap();
        source.step(&[]).unwrap();
        restored.step(&[]).unwrap();
        assert_eq!(value(&restored), value(&source));
    }
}

#[test]
fn checkpoint_slots_have_no_500_agent_policy_cap() {
    let mut source = Simulator::new(
        7,
        Config {
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Config::sandbox()
        },
    )
    .unwrap();
    for slot in 0..512 {
        source.add(slot.to_string(), slot, 512).unwrap();
    }
    let saved = value(&source);
    let restored = Simulator::from_checkpoint(&bytes(&saved), RestoreOptions::default()).unwrap();
    assert_eq!(restored.snakes.len(), 512);
    assert_eq!(value(&restored), saved);
}
