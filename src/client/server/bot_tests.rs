//! Manually clocked policy-call regressions; no sockets or wall-clock samples.
use super::*;
use crate::{
    Config,
    bot::DEFAULT_POLICY,
    client::{SimulatorBackend, replay_record},
};

fn received_seats() -> (Vec<Seat>, f64) {
    let mut backend = SimulatorBackend::new(
        7,
        SimulationConfig {
            agents: 4,
            record_wire: true,
            world: Config {
                radius: 6000.,
                ..Config::sandbox()
            },
            ..Default::default()
        },
    )
    .unwrap();
    backend.advance_ticks(1).unwrap();
    let at = backend.time();
    let seats = (0..4)
        .map(|slot| {
            let mut seat = Seat::new(slot, 0.042);
            for record in backend.drain_records(slot).unwrap() {
                replay_record(&mut seat.state, &record).unwrap();
            }
            assert_eq!(
                seat.state.frame(at).unwrap(),
                backend.frame_for(slot).unwrap()
            );
            seat.clock = at;
            seat.decide_at = at;
            seat
        })
        .collect();
    (seats, at)
}

#[test]
fn shared_worker_scratch_matches_fresh_adapters_and_preserves_pending_history() {
    let (mut seats, at) = received_seats();
    let mut policy = ForagerBot::new(DEFAULT_POLICY);
    let latency = LatencyConfig {
        processing_seconds: [0.02, 0.02],
        ..Default::default()
    };
    let mut random = Random::new(1);
    let (commands, received) = mpsc::channel();
    for slot in [3, 0, 2, 1] {
        let seat = &mut seats[slot];
        seat.link = Link::connect(&latency, &mut random);
        let frame = seat.state.frame(at).unwrap();
        let expected = ForagerBot::new(DEFAULT_POLICY).decide(&frame).unwrap();
        seat.decide(at, &commands, &latency, &mut random, &mut policy)
            .unwrap();
        let pending = seat.outbox.unwrap();
        assert_eq!(pending.1.heading.to_bits(), expected.heading.to_bits());
        assert_eq!(pending.1.boost, expected.boost);
        assert_eq!(pending.2, 0.02);
        seat.decide(at, &commands, &latency, &mut random, &mut policy)
            .unwrap();
        assert_eq!(seat.outbox, Some(pending));
        assert_eq!(seat.state.frame(at).unwrap(), frame);
        assert!(received.try_recv().is_err(), "deciding does not transmit");
        seat.transmit(at + pending.2, &commands).unwrap();
        let sent = seat.state.frame(at + pending.2).unwrap().timing;
        assert_eq!(sent.processing_seconds, pending.2);
        assert_eq!(sent.sent_commands[0].seconds_ago, 0.);
        assert_eq!(
            sent.sent_commands[0].action.heading,
            crate::physics::quantize_heading(expected.heading)
        );
        assert_eq!(sent.sent_commands[0].action.boost, expected.boost);
        let packets: Vec<_> = received
            .try_iter()
            .map(|command| match command {
                Command::Message { bytes, sent, .. } => {
                    assert_eq!(sent, at + pending.2);
                    bytes
                }
                _ => panic!("unexpected lifecycle command"),
            })
            .collect();
        assert_eq!(packets, protocol::action_packets(expected, None).unwrap());
    }
}

#[test]
fn same_heading_sends_boost_change_and_rejoin_clears_seat_history() {
    let (mut seats, at) = received_seats();
    let seat = &mut seats[0];
    let mut policy = ForagerBot::new(DEFAULT_POLICY);
    let expected = policy.decide(&seat.state.frame(at).unwrap()).unwrap();
    let previous = Action {
        boost: !expected.boost,
        ..expected
    };
    for packet in protocol::action_packets(previous, None).unwrap() {
        seat.state.record_sent(&packet, at).unwrap();
    }
    seat.previous_boost = Some(previous.boost);
    let (commands, received) = mpsc::channel();
    let latency = LatencyConfig::default();
    let mut random = Random::new(1);
    seat.decide(at, &commands, &latency, &mut random, &mut policy)
        .unwrap();
    let action = seat.outbox.unwrap().1;
    assert_eq!(
        protocol::action_packets(action, None).unwrap()[0],
        protocol::action_packets(previous, None).unwrap()[0]
    );
    assert_ne!(action.boost, previous.boost);
    seat.transmit(at, &commands).unwrap();
    let packets: Vec<_> = received
        .try_iter()
        .filter_map(|command| match command {
            Command::Message { bytes, .. } => Some(bytes),
            _ => None,
        })
        .collect();
    assert_eq!(
        packets,
        protocol::action_packets(action, Some(previous.boost)).unwrap()
    );
    assert_eq!(packets.len(), 2);

    // A received death starts a new connection through the ordinary decision path.
    seat.state.receive_frame(&[0, 0, b'v', 0], at).unwrap();
    let generation = seat.state.frame(at).unwrap().received.connection_generation;
    seat.decide(at, &commands, &latency, &mut random, &mut policy)
        .unwrap();
    let frame = seat.state.frame(at).unwrap();
    assert_eq!(frame.status, ClientStatus::Waiting);
    assert_eq!(frame.received.connection_generation, generation + 1);
    assert!(frame.timing.sent_commands.is_empty());
    assert!(seat.outbox.is_none() && seat.previous_boost.is_none());
    assert!(matches!(
        received.try_recv().unwrap(),
        Command::Connect {
            physical_food: true,
            ..
        }
    ));
    seat.decide(at, &commands, &latency, &mut random, &mut policy)
        .unwrap();
    assert!(seat.outbox.is_none());
    assert!(received.try_recv().is_err());
}
