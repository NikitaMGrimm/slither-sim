use crate::{
    Config,
    bot::{self as forager, Forager, PolicyId},
    client::{
        Bot, CONTRACT, ClientStatus, ForagerBot, LatencyConfig, SimulationConfig, SimulatorBackend,
    },
};

fn backend(round_trip: f64) -> SimulatorBackend {
    SimulatorBackend::new(
        11,
        SimulationConfig {
            agents: 4,
            world: Config::official(4),
            latency: LatencyConfig::fixed(round_trip),
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn adapter_checks_lifecycle_and_keeps_no_state_between_decisions() {
    let mut backend = backend(0.1);
    let mut bots: Vec<_> = (0..4).map(|_| ForagerBot::new(PolicyId::A1)).collect();
    let mut playing = 0;
    for _ in 0..120 {
        let frames: Vec<_> = (0..4).map(|s| backend.frame_for(s).unwrap()).collect();
        let actions: Vec<_> = frames
            .iter()
            .zip(&mut bots)
            .map(|(frame, bot)| bot.decide(frame))
            .collect();
        for (frame, action) in frames.iter().zip(&actions) {
            // A fresh bot decides identically: history never changes a decision.
            assert_eq!(*action, ForagerBot::new(PolicyId::A1).decide(frame));
            if let Some(o) = &frame.observation {
                playing += 1;
                let timing = bots[0].timing(frame);
                // A1 ignores opponent intents.
                let expected = Forager::new(PolicyId::A1).decide(o, &timing, &[]);
                assert_eq!(*action, Some(expected));
            }
        }
        backend.step_all(&actions).unwrap();
    }
    assert!(playing > 300);

    let mut frame = backend.frame_for(0).unwrap();
    let mut bot = ForagerBot::new(PolicyId::A1);
    for status in [
        ClientStatus::Waiting,
        ClientStatus::Dead,
        ClientStatus::Disconnected,
    ] {
        frame.status = status;
        assert_eq!(bot.decide(&frame), None);
    }
    frame.status = ClientStatus::Playing;
    frame.contract = "incompatible".into();
    assert_eq!(bot.decide(&frame), None);
    frame.contract = CONTRACT.into();
    frame.observation = None;
    assert_eq!(bot.decide(&frame), None);
}

#[test]
fn timing_splits_the_measured_ping_and_ages_by_the_latest_receipt() {
    let mut backend = backend(0.1);
    let mut bot = ForagerBot::new(PolicyId::A1);
    let mut receipts = Vec::new();
    for step in 0..200 {
        let frame = backend.frame_for(0).unwrap();
        let action = bot.decide(&frame);
        if let (Some(ping), Some(_)) = (frame.timing.ping_seconds, &frame.observation) {
            let timing = bot.timing(&frame);
            let uplink = crate::bot::Forager::new(PolicyId::A1).uplink_share();
            let receipt = timing.observation_age - ping * (1.0 - uplink);
            assert!(receipt >= 0.0, "{receipt}");
            assert_eq!(timing.command_delay, ping * uplink);
            assert_eq!(
                timing.sent_len,
                frame.timing.sent_commands.len().min(forager::MAX_SENT)
            );
            for (sent, command) in timing.sent[..timing.sent_len]
                .iter()
                .zip(&frame.timing.sent_commands)
            {
                assert_eq!(sent.seconds_ago, command.seconds_ago);
                assert_eq!(sent.action, command.action);
            }
            if step > 100 {
                receipts.push(receipt);
            }
        }
        let mut actions = vec![None; 4];
        actions[0] = action;
        backend.step_all(&actions).unwrap();
    }
    // Frames arrive between 42 ms decisions. A frame that updates no received
    // field leaves an older minimum, so only typical receipts are this young.
    assert!(receipts.len() > 50);
    receipts.sort_by(f64::total_cmp);
    assert!(receipts[receipts.len() / 2] < 0.042 + 1e-9, "{receipts:?}");
}

#[test]
fn intents_follow_opponent_ids_and_preserve_the_frozen_first_report_rule() {
    use crate::client::{ReceivedField, ReceivedHistory, ReceivedMetadata, ReceivedSnake};
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/forager-native-parity.json"
    ))
    .unwrap();
    let mut observation: crate::Observation =
        serde_json::from_value(fixture["cases"][0]["observation"].clone()).unwrap();
    let first = observation.me.clone();
    let mut second = first.clone();
    second.id = "second".into();
    let mut missing = first.clone();
    missing.id = "missing".into();
    observation.opponents = vec![first.clone(), second.clone(), missing];
    let received = ReceivedMetadata {
        snakes: vec![
            ReceivedSnake {
                snake_id: second.id,
                wanted_history: vec![],
                heading: ReceivedField {
                    valid: true,
                    age_seconds: 0.25,
                    ..Default::default()
                },
                ..Default::default()
            },
            ReceivedSnake {
                snake_id: first.id,
                // The frozen adapter takes the first entry even when its valid
                // flag is false; it does not search for a different report.
                wanted_history: vec![
                    ReceivedHistory {
                        value: -0.01,
                        valid: false,
                        ..Default::default()
                    },
                    ReceivedHistory {
                        value: 1.0,
                        valid: true,
                        ..Default::default()
                    },
                ],
                heading: ReceivedField {
                    valid: false,
                    age_seconds: 9.0,
                    ..Default::default()
                },
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let mut intents = Vec::new();
    super::fill_intents(&mut intents, &observation, &received);
    assert_eq!(intents.len(), 3);
    assert_eq!(intents[0].wanted.to_bits(), 0.0_f64.to_bits());
    assert_eq!(intents[0].heading_age.to_bits(), 0.0_f64.to_bits());
    assert!(intents[1].wanted.is_nan());
    assert_eq!(intents[1].heading_age, 0.25);
    assert!(intents[2].wanted.is_nan());
    assert_eq!(intents[2].heading_age.to_bits(), 0.0_f64.to_bits());
    super::fill_intents(&mut intents, &observation, &Default::default());
    assert!(
        intents
            .iter()
            .all(|intent| intent.wanted.is_nan() && intent.heading_age == 0.0)
    );
}
