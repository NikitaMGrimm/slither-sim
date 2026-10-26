mod support;
use serde_json::Value;
use slither_sim::{
    Action, Config, KioskConfig, Point, Simulator, SpeedModel, SpeedState,
    physics::{ANCHOR_SPACING, BOOST_SPEED, LOOP_SECONDS, base_speed},
};
use std::f64::consts::TAU;

fn projected_sp(window: &Value) -> f64 {
    let anchors = window["anchors"].as_array().unwrap();
    assert_eq!(anchors.len(), 11);
    let heading = window["heading_byte"].as_u64().unwrap() as f64 * TAU / 251.0;
    let first = &anchors[0];
    let projected: Vec<_> = anchors
        .iter()
        .map(|anchor| {
            let elapsed = (anchor[0].as_f64().unwrap() - first[0].as_f64().unwrap()) / 1000.0;
            let distance = (anchor[1].as_f64().unwrap() - first[1].as_f64().unwrap())
                * heading.cos()
                + (anchor[2].as_f64().unwrap() - first[2].as_f64().unwrap()) * heading.sin();
            (elapsed, distance)
        })
        .collect();
    let mean_time = projected.iter().map(|point| point.0).sum::<f64>() / 11.0;
    let mean_distance = projected.iter().map(|point| point.1).sum::<f64>() / 11.0;
    let covariance = projected
        .iter()
        .map(|point| (point.0 - mean_time) * (point.1 - mean_distance))
        .sum::<f64>();
    let variance = projected
        .iter()
        .map(|point| (point.0 - mean_time).powi(2))
        .sum::<f64>();
    assert!(variance > 0.0);
    let speed = covariance / variance * 0.032;
    assert!(speed.is_finite() && speed > 0.0);
    speed
}

#[test]
fn illustrative_fixture_preserves_growth_windows_and_raw_control_chronology() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/cruise-growth-retention-20260930.json"
    ))
    .unwrap();
    assert_eq!(
        fixture["source"],
        "recordings\\headless\\growth-anchor-holdout-20260930\\bot0.jsonl.gz"
    );
    assert_eq!(
        fixture["source_sha256"],
        "0c5f03a3c2134683ffb054f09e4c83b80d967b873ca4bcc35ff96674b490385a"
    );
    assert!(
        fixture["note"]
            .as_str()
            .unwrap()
            .contains("not an independent cohort")
    );
    let windows = fixture["windows"].as_array().unwrap();
    let evidence = &fixture["raw_control_evidence"];
    let phases = evidence["phase_starts"].as_array().unwrap();
    assert_eq!(windows.len(), 3);
    assert_eq!(phases.len(), 4);
    for pair in phases.windows(2) {
        assert!(pair[0]["t"].as_f64().unwrap() < pair[1]["t"].as_f64().unwrap());
        assert!(
            pair[0]["serverClock"].as_f64().unwrap() < pair[1]["serverClock"].as_f64().unwrap()
        );
    }
    for (window, (label, trial, phase, segments, heading_byte, phase_index)) in
        windows.iter().zip([
            ("small_unboosted", 1, "coast", 2, 226, 0),
            ("grown_unboosted", 3, "settle", 22, 29, 1),
            ("grown_after_boost", 3, "coast", 22, 29, 3),
        ])
    {
        assert_eq!(window["label"], label);
        assert_eq!(window["conn"], 1);
        assert_eq!(window["trial"], trial);
        assert_eq!(window["phase"], phase);
        assert_eq!(window["segments_first"], segments);
        assert_eq!(window["segments_last"], segments);
        assert_eq!(window["heading_byte"], heading_byte);
        assert_eq!(phases[phase_index]["life"], 1);
        assert_eq!(phases[phase_index]["trial"], trial);
        assert_eq!(phases[phase_index]["phase"], phase);
        assert_eq!(phases[phase_index]["sct"], segments);
        let anchors = window["anchors"].as_array().unwrap();
        assert_eq!(anchors.len(), 11);
        for pair in anchors.windows(2) {
            let gap = pair[1][0].as_f64().unwrap() - pair[0][0].as_f64().unwrap();
            assert!(gap > 0.0 && gap <= 350.0);
            let delta_x = pair[1][1].as_f64().unwrap() - pair[0][1].as_f64().unwrap();
            let delta_y = pair[1][2].as_f64().unwrap() - pair[0][2].as_f64().unwrap();
            assert!((delta_x.hypot(delta_y) - ANCHOR_SPACING).abs() < 1e-6);
        }
        let controls = &window["control_evidence"];
        assert_eq!(controls["last_raw_heading_before_window"], heading_byte);
        assert!(controls["wrong_headings"].as_array().unwrap().is_empty());
        assert!(controls["boost_on"].as_array().unwrap().is_empty());
        assert!(controls["controls"].as_u64().unwrap() > 0);
        assert!(controls["outgoing"].as_u64().unwrap() > 0);
        assert!(controls["first_server"].as_f64().unwrap() <= anchors[0][0].as_f64().unwrap());
        assert!(controls["last_server"].as_f64().unwrap() >= anchors[10][0].as_f64().unwrap());
        assert!(
            anchors[0][0].as_f64().unwrap()
                >= phases[phase_index]["serverClock"].as_f64().unwrap() + 1000.0
        );
    }
    for pair in windows.windows(2) {
        assert!(
            pair[0]["anchors"][10][0].as_f64().unwrap()
                < pair[1]["anchors"][0][0].as_f64().unwrap()
        );
    }
    assert!(windows[0]["control_evidence"]["last_raw_boost_command_before_window"].is_null());
    assert!(windows[1]["control_evidence"]["last_raw_boost_command_before_window"].is_null());
    assert_eq!(
        windows[2]["control_evidence"]["last_raw_boost_command_before_window"],
        254
    );
    let commands = evidence["raw_boost_commands_through_end"]
        .as_array()
        .unwrap();
    assert_eq!(commands.len(), 2);
    assert_eq!(commands[0]["byte"], 253);
    assert_eq!(commands[1]["byte"], 254);
    assert_eq!(phases[2]["life"], 1);
    assert_eq!(phases[2]["trial"], 3);
    assert_eq!(phases[2]["phase"], "boost");
    assert_eq!(phases[2]["sct"], 22);
    let boost_on = commands[0]["local_ms"].as_f64().unwrap();
    let boost_off = commands[1]["local_ms"].as_f64().unwrap();
    assert!(boost_on > phases[1]["t"].as_f64().unwrap());
    assert!(boost_on >= phases[2]["t"].as_f64().unwrap());
    assert!(boost_on < phases[3]["t"].as_f64().unwrap());
    assert!(boost_off > boost_on);
    assert!(boost_off >= phases[3]["t"].as_f64().unwrap());
    assert!(boost_off < evidence["trial_end"]["t"].as_f64().unwrap());
    assert!(
        windows[1]["anchors"][10][0].as_f64().unwrap() < phases[2]["serverClock"].as_f64().unwrap()
    );
    assert_eq!(evidence["trial_end"]["life"], 1);
    assert_eq!(evidence["trial_end"]["trial"], 3);
    assert_eq!(evidence["trial_end"]["phase"], "coast");
    assert_eq!(evidence["trial_end"]["completed"], true);
    assert!(
        windows[2]["anchors"][10][0].as_f64().unwrap()
            < evidence["trial_end"]["serverClock"].as_f64().unwrap()
    );
}

#[test]
fn illustrative_growth_anchors_match_retained_simulator_and_reject_direct_speed() {
    let fixture: Value = serde_json::from_str(include_str!(
        "fixtures/cruise-growth-retention-20260930.json"
    ))
    .unwrap();
    let measured: [f64; 3] = std::array::from_fn(|index| projected_sp(&fixture["windows"][index]));
    let mut retained = Simulator::new(slither_sim::WorldOptions {
        seed: 42,
        config: Config {
            radius: 100_000.0,
            food_count: 0,
            ambient_replenish_per_second: 0.0,
            emit_boost_food: false,
            speed_model: SpeedModel::RetainedProgress,
            kiosk: KioskConfig {
                enabled: false,
                ..KioskConfig::default()
            },
            ..Config::sandbox()
        },
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["illustrative_growth".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    {
        let mut snake = retained.snakes()[0].clone();
        snake.head = Point::default();
        retained
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    {
        let mut snake = retained.snakes()[0].clone();
        snake.heading = 0.0;
        retained
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    {
        let mut snake = retained.snakes()[0].clone();
        snake.body = vec![Point::default(), Point { x: -42.0, y: 0.0 }];
        retained
            .correct_mass(0, snake.body.clone(), snake.fam)
            .unwrap();
        retained
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    let spawn_speed = retained.snakes()[0].speed;
    assert_eq!(retained.snakes()[0].segments, 2);
    assert_eq!(spawn_speed, base_speed(2));
    let idle = Some(Action {
        heading: 0.0,
        boost: false,
    });
    for _ in 0..350 {
        retained.step(&[idle]).unwrap();
        assert!(retained.snakes()[0].alive);
        assert_eq!(retained.snakes()[0].segments, 2);
        assert_eq!(retained.snakes()[0].speed, spawn_speed);
    }
    assert!((retained.snakes()[0].speed * 0.032 - measured[0]).abs() < 0.01);
    {
        let mut snake = retained.snakes()[0].clone();
        snake.fam += 20.99;
        retained
            .correct_mass(0, snake.body.clone(), snake.fam)
            .unwrap();
    }
    let mut direct = retained.clone();
    {
        let mut config = direct.config().clone();
        config.speed_model = SpeedModel::DirectSpeed;
        support::reconfigure(&mut direct, config);
    }
    let mut state = SpeedState::idle(spawn_speed);
    for _ in 0..1100 {
        retained.step(&[idle]).unwrap();
        direct.step(&[idle]).unwrap();
        assert!(retained.snakes()[0].alive && direct.snakes()[0].alive);
        assert_eq!(retained.snakes()[0].speed, spawn_speed);
        assert_eq!(
            state.step(
                false,
                retained.snakes()[0].segments,
                LOOP_SECONDS,
                SpeedModel::RetainedProgress
            ),
            retained.snakes()[0].speed
        );
        assert_eq!(state.progress(), 0.0);
    }
    assert_eq!(retained.snakes()[0].segments, 22);
    assert_eq!(direct.snakes()[0].segments, 22);
    assert_eq!(retained.snakes()[0].body.len(), 22);
    assert!((retained.snakes()[0].speed * 0.032 - measured[1]).abs() < 0.01);
    assert!((direct.snakes()[0].speed - base_speed(22)).abs() < 1e-10);
    assert!(
        (direct.snakes()[0].speed * 0.032 - measured[1]).abs() > 0.03,
        "DirectSpeed must fail the grown-idle fixture's 0.01-sp agreement"
    );
    assert_eq!(
        retained.boost_accounting(0).unwrap().active_boost_seconds,
        0.0
    );
    let commands = fixture["raw_control_evidence"]["raw_boost_commands_through_end"]
        .as_array()
        .unwrap();
    let boost_seconds = (commands[1]["local_ms"].as_f64().unwrap()
        - commands[0]["local_ms"].as_f64().unwrap())
        / 1000.0;
    let boost_ticks = (boost_seconds / retained.config().dt).ceil() as usize;
    let boost = Some(Action {
        heading: 0.0,
        boost: true,
    });
    for _ in 0..boost_ticks {
        retained.step(&[boost]).unwrap();
        assert!(retained.snakes()[0].alive && retained.is_boosting(0));
        assert_eq!(retained.snakes()[0].segments, 22);
        assert_eq!(
            state.step(true, 22, LOOP_SECONDS, SpeedModel::RetainedProgress),
            retained.snakes()[0].speed
        );
    }
    assert_eq!(retained.snakes()[0].speed, BOOST_SPEED);
    assert_eq!(state.progress(), 1.0);
    for _ in 0..400 {
        retained.step(&[idle]).unwrap();
        assert!(retained.snakes()[0].alive && !retained.is_boosting(0));
        assert_eq!(retained.snakes()[0].segments, 22);
        assert_eq!(
            state.step(false, 22, LOOP_SECONDS, SpeedModel::RetainedProgress),
            retained.snakes()[0].speed
        );
    }
    assert_eq!(state.progress(), 0.0);
    assert_eq!(retained.snakes()[0].speed, base_speed(22));
    assert!((retained.snakes()[0].speed * 0.032 - measured[2]).abs() < 0.01);
    assert!(measured[2] - measured[1] > 0.03);
    assert!(retained.boost_accounting(0).unwrap().boost_mass_spent > 0.0);
    assert!(retained.food().is_empty() && direct.food().is_empty());
    assert!(retained.prey().is_empty() && direct.prey().is_empty());
    assert!(retained.deaths().is_empty() && direct.deaths().is_empty());
}
