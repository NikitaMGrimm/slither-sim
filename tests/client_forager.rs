#![cfg(feature = "networking")]

use slither_sim::{
    Config,
    client::{
        Bot, CONTRACT, ClientStatus, ForagerBot, LatencyConfig, SimulationConfig, SimulatorBackend,
    },
    forager::{self, Params, Scratch},
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
    let mut bots: Vec<_> = (0..4).map(|_| ForagerBot::default()).collect();
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
            assert_eq!(*action, ForagerBot::default().decide(frame));
            if let Some(o) = &frame.observation {
                playing += 1;
                let timing = bots[0].timing(frame);
                let mut scratch = Scratch::default();
                let expected = forager::decide(o, &timing, &Params::default(), &mut scratch);
                assert_eq!(*action, Some(expected));
            }
        }
        backend.step_all(&actions).unwrap();
    }
    assert!(playing > 300);

    let mut frame = backend.frame_for(0).unwrap();
    let mut bot = ForagerBot::default();
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
    assert!(
        ForagerBot::new(Params {
            cells: 0,
            ..Params::default()
        })
        .is_err()
    );
}

#[test]
fn timing_splits_the_measured_ping_and_ages_by_the_latest_receipt() {
    let mut backend = backend(0.1);
    let mut bot = ForagerBot::default();
    let mut receipts = Vec::new();
    for step in 0..200 {
        let frame = backend.frame_for(0).unwrap();
        let action = bot.decide(&frame);
        if let (Some(ping), Some(_)) = (frame.timing.ping_seconds, &frame.observation) {
            let timing = bot.timing(&frame);
            let receipt = timing.observation_age - ping / 2.0;
            assert!(receipt >= 0.0, "{receipt}");
            assert_eq!(timing.command_delay, ping / 2.0);
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
