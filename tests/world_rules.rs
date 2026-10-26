use serde_json::{Value, json};
use slither_sim::{
    Action, BoundaryMode, Config, ErrorCode, InitialPopulation, Point, RadiusUpdate,
    RestoreOptions, Simulator, WorldOptions, WorldRules, transport,
};

#[test]
fn rules_keep_mode_settings_and_follow_the_live_circular_pit() {
    let config = Config {
        boundary_mode: BoundaryMode::Hard,
        pit: Some(slither_sim::PitConfig { radius: 550. }),
        min_score: Some(400),
        kill_score_seconds: Some(0.014),
        food_count: 0,
        ambient_replenish_per_second: 0.,
        radius_updates: vec![RadiusUpdate {
            time: 0.007,
            radius: 3000.,
        }],
        ..Config::battledome()
    };
    let mut world = Simulator::new(WorldOptions {
        seed: 7,
        config,
        population: InitialPopulation::Dormant {
            ids: vec!["a".into()],
        },
    })
    .unwrap();
    let expected = WorldRules {
        boundary_mode: BoundaryMode::Hard,
        pit_radius: Some(550.),
        min_score: Some(400),
        kill_score_seconds: Some(0.014),
        free_boost_at_floor: true,
    };
    assert_eq!(world.rules(), expected);
    assert_eq!(world.geometric_context(0).unwrap().rules, expected);
    world.set_ambient_rate(7.).unwrap();
    world.step(&[None]).unwrap();
    assert_eq!(world.radius(), 3000.);
    let expected = WorldRules {
        pit_radius: Some(550. * (3000. / 3584.)),
        ..expected
    };
    assert_eq!(world.rules(), expected);
    let restored = Simulator::from_checkpoint(
        &serde_json::to_vec(&world.checkpoint().unwrap()).unwrap(),
        RestoreOptions::default(),
    )
    .unwrap();
    assert_eq!(restored.rules(), expected);
    let error = world.geometric_context(1).unwrap_err();
    assert_eq!(error.code(), ErrorCode::UnknownSlot);
    assert_eq!(error.slot, Some(1));
    assert!(
        serde_json::to_value(&world.snakes()[0])
            .unwrap()
            .get("rules")
            .is_none()
    );
}

#[test]
fn raw_context_keeps_own_rolling_score_across_lives_and_expires_it() {
    let mut world = Simulator::new(WorldOptions {
        seed: 7,
        config: Config {
            food_count: 0,
            ambient_replenish_per_second: 0.,
            kill_score_seconds: Some(0.014),
            ..Config::sandbox()
        },
        population: InitialPopulation::Dormant {
            ids: vec!["a".into()],
        },
    })
    .unwrap();
    // Synthetic accepted history represents a credit from the slot's prior life.
    let mut checkpoint = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
    checkpoint["state"]["slots"][0]["killTimes"] = json!([0.]);
    world
        .restore(
            &serde_json::to_vec(&checkpoint).unwrap(),
            RestoreOptions::default(),
        )
        .unwrap();
    assert_eq!(world.geometric_context(0).unwrap().kill_score, 1);
    world.respawn(0, Point::default(), 0., 2).unwrap();
    assert_eq!(world.snakes()[0].kills, 0);
    assert_eq!(world.geometric_context(0).unwrap().kill_score, 1);
    assert!(
        serde_json::to_value(world.observe(0).unwrap())
            .unwrap()
            .get("rules")
            .is_none()
    );
    for _ in 0..2 {
        world.step(&[Some(Action::default())]).unwrap();
    }
    assert_eq!(world.geometric_context(0).unwrap().kill_score, 0);
}

#[test]
fn every_frame_has_custom_rules_and_absent_values_are_null() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/serve-rules-v5.json")).unwrap();
    let mut session = transport::Session::new();
    let reset: Value =
        serde_json::from_slice(&session.request(&serde_json::to_vec(&fixture["reset"]).unwrap()))
            .unwrap();
    assert_eq!(reset["result"]["worlds"][0]["rules"], fixture["rules"]);
    assert_eq!(
        serde_json::to_value(session.frame(0).unwrap().rules).unwrap(),
        fixture["rules"]
    );
    session.step(&[0., 0.], &[true, true]).unwrap();
    let reply: Value = serde_json::from_slice(
        &session.request(br#"{"op":"step","actions":[{"heading":[0,0],"boost":[true,true]}]}"#),
    )
    .unwrap();
    assert_eq!(reply["result"]["worlds"][0]["rules"], fixture["rules"]);
    let reset: Value = serde_json::from_slice(
        &session
            .request(br#"{"op":"reset","agents":1,"config":{"preset":"sandbox","foodCount":0}}"#),
    )
    .unwrap();
    assert_eq!(
        reset["result"]["worlds"][0]["rules"],
        json!({"boundaryMode":"sliding","pitRadius":null,"minScore":null,"killScoreSeconds":null,"freeBoostAtFloor":false})
    );
    let info: Value = serde_json::from_slice(&session.request(br#"{"op":"info"}"#)).unwrap();
    assert_eq!(info["result"]["rulesRevision"], "slither-sim-rules-v3");
}

#[test]
fn json_causes_match_typed_rust_causes_and_keep_locations() {
    let mut typed = transport::Session::new();
    let error = typed.step(&[], &[]).unwrap_err();
    let json: Value =
        serde_json::from_slice(&typed.request(br#"{"op":"step","actions":[]}"#)).unwrap();
    assert_eq!(
        json["error"],
        serde_json::to_value(error.details()).unwrap()
    );
    typed.request(
        br#"{"op":"reset","agents":2,"worlds":2,"config":{"preset":"sandbox","foodCount":0}}"#,
    );
    let error = typed
        .step(&[0., 0., 0., f64::NAN], &[false; 4])
        .unwrap_err();
    let json: Value = serde_json::from_slice(&typed.request(br#"{"op":"step","actions":[{"heading":[0,0],"boost":[false,false]},{"heading":[0,null],"boost":[false,false]}]}"#)).unwrap();
    assert_eq!(
        json["error"],
        serde_json::to_value(error.details()).unwrap()
    );
    assert_eq!(json["error"]["code"], 4);
    assert_eq!(json["error"]["rule"], 1);
    assert_eq!(json["error"]["world"], 1);
    assert_eq!(json["error"]["slot"], 1);
    let error = typed.frame_checked(8).unwrap_err();
    let json: Value =
        serde_json::from_slice(&typed.request(br#"{"op":"snapshot","world":8}"#)).unwrap();
    assert_eq!(
        json["error"],
        serde_json::to_value(error.details()).unwrap()
    );
}
