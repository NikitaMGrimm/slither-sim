use super::*;
use crate::{
    Config,
    bot::PolicyId,
    client::{LatencyConfig, protocol},
};

fn backend(processing: f64) -> SimulatorBackend {
    SimulatorBackend::new(
        7,
        SimulationConfig {
            agents: 4,
            record_wire: true,
            latency: LatencyConfig {
                processing_seconds: [processing, processing],
                ..Default::default()
            },
            world: Config {
                radius: 6000.,
                ..Config::sandbox()
            },
            ..Default::default()
        },
    )
    .unwrap()
}

#[test]
fn native_shared_policy_matches_fresh_slots_across_pending_decisions_and_reconnect() {
    let mut backend = backend(0.084);
    backend.drive_background().unwrap();
    assert!(
        backend.background_policy.is_none(),
        "waiting clients do not decide"
    );
    backend.advance_ticks(1).unwrap();
    let mut decisions = 0;
    let mut pending = 0;
    let generation = backend.frame_for(2).unwrap().received.connection_generation;
    for step in 0..40 {
        if step == 12 {
            let at = backend.time();
            backend.slots[2]
                .state
                .receive_frame(&[0, 0, b'v', 0], at)
                .unwrap();
        }
        let frames: Vec<_> = (0..4)
            .map(|slot| backend.frame_for(slot).unwrap())
            .collect();
        let outboxes: Vec<_> = backend.slots.iter().map(|s| s.outbox).collect();
        let expected: Vec<_> = frames
            .iter()
            .enumerate()
            .map(|(slot, frame)| {
                if slot == 0 || backend.pending(slot) {
                    None
                } else {
                    ForagerBot::new(PolicyId::default()).decide(frame)
                }
            })
            .collect();
        backend.drive_background().unwrap();
        assert_eq!(
            backend.frame_for(0).unwrap(),
            frames[0],
            "controlled slot stays external"
        );
        for slot in [3, 1, 2] {
            if let Some(action) = expected[slot] {
                let (at, queued, processing) = backend.slots[slot].outbox.unwrap();
                assert_eq!(queued.heading.to_bits(), action.heading.to_bits());
                assert_eq!(queued.boost, action.boost);
                assert_eq!(at, backend.time());
                assert_eq!(processing, 0.084);
                assert_eq!(backend.frame_for(slot).unwrap().timing, frames[slot].timing);
                decisions += 1;
            } else if frames[slot].status == ClientStatus::Playing {
                assert_eq!(backend.slots[slot].outbox, outboxes[slot]);
                pending += 1;
            } else if frames[slot].status == ClientStatus::Dead {
                let frame = backend.frame_for(slot).unwrap();
                assert_eq!(frame.received.connection_generation, generation + 1);
                assert!(frame.timing.sent_commands.is_empty());
                assert!(backend.slots[slot].outbox.is_none());
            }
        }
        backend
            .advance_ticks(backend.config.decision_ticks)
            .unwrap();
    }
    assert!(decisions > 20 && pending > 20);
    assert_eq!(
        backend.frame_for(2).unwrap().received.connection_generation,
        generation + 1
    );
}

#[test]
fn native_background_sends_boost_only_changes_at_transmission() {
    let mut backend = backend(0.);
    backend.advance_ticks(1).unwrap();
    let at = backend.time();
    let action = ForagerBot::new(PolicyId::default())
        .decide(&backend.frame_for(1).unwrap())
        .unwrap();
    let previous = Action {
        boost: !action.boost,
        ..action
    };
    for packet in protocol::action_packets(previous, None).unwrap() {
        backend.slots[1].state.record_sent(&packet, at).unwrap();
    }
    backend.slots[1].previous_boost = Some(previous.boost);
    backend.drain_records(1).unwrap();
    backend.drive_background().unwrap();
    let sent: Vec<_> = backend
        .drain_records(1)
        .unwrap()
        .into_iter()
        .filter(|r| r.event == WireEvent::Send && r.bytes != [251])
        .map(|r| {
            assert_eq!(r.time, at);
            r.bytes
        })
        .collect();
    assert_eq!(
        sent,
        protocol::action_packets(action, Some(previous.boost)).unwrap()
    );
    assert_eq!(sent.len(), 2);
    let history = backend.frame_for(1).unwrap().timing.sent_commands;
    assert_eq!(history[0].seconds_ago, 0.);
    assert_eq!(history[0].action.boost, action.boost);
}
