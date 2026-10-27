use super::*;
use crate::{
    Config, RadiusUpdate,
    client::{encode, protocol, rules_packet::COMMAND},
};

#[test]
fn live_rules_reach_only_opted_in_connections_including_subunit_pit_moves() {
    let mut world = Config::battledome();
    world.food_count = 0;
    world.ambient_replenish_per_second = 0.;
    world.radius_updates = vec![
        RadiusUpdate {
            time: 0.014,
            radius: 7168.,
        },
        RadiusUpdate {
            time: 0.021,
            radius: 7167.9,
        },
    ];
    let mut sim = RawSimulator::new(
        7,
        SimulationConfig {
            world,
            agents: 2,
            ..Default::default()
        },
        2,
    )
    .unwrap();
    for slot in 0..2 {
        assert!(sim.connect(slot, PlayerAppearance::default()).unwrap());
    }
    sim.physical_food(1); // The host calls this only for the explicit bot header.
    let mut readers = [ClientState::new(), ClientState::new()];
    for reader in &mut readers {
        reader.new_connection(0.).unwrap();
    }
    for tick in 1..=3 {
        sim.advance_ticks(1).unwrap();
        for (slot, reader) in readers.iter_mut().enumerate() {
            let mut packets = Vec::new();
            for frame in sim.drain(slot).unwrap() {
                packets.extend(
                    protocol::split_frame(&frame)
                        .unwrap()
                        .into_iter()
                        .map(<[u8]>::to_vec),
                );
                reader.receive_frame(&frame, sim.time()).unwrap();
            }
            let rules = packets.iter().filter(|p| p[0] == COMMAND).count();
            assert_eq!(rules, usize::from(slot == 1), "slot={slot}, tick={tick}");
            assert_eq!(
                reader.frame(sim.time()).unwrap().rules,
                (slot == 1).then(|| sim.backend.world.rules())
            );
            if slot == 1 && tick == 1 {
                assert_eq!(packets[0][0], b'a');
                assert_eq!(packets[1][0], COMMAND, "rules accompany setup");
            }
            if tick == 3 {
                assert!(
                    packets.iter().all(|p| p[0] != b'z'),
                    "rounded wall did not change"
                );
            }
        }
    }
    sim.disconnect(1).unwrap();
    assert!(sim.connect(1, PlayerAppearance::default()).unwrap());
    sim.advance_ticks(1).unwrap();
    for frame in sim.drain(1).unwrap() {
        assert!(
            protocol::split_frame(&frame)
                .unwrap()
                .iter()
                .all(|p| p[0] != COMMAND),
            "reuse must not inherit opt-in"
        );
    }
}

#[test]
fn native_rules_are_received_reports_and_unchanged_rules_are_not_resent() {
    let mut backend = SimulatorBackend::new(7, SimulationConfig::default()).unwrap();
    assert!(backend.frame().unwrap().rules.is_none());
    backend.advance_ticks(1).unwrap();
    assert_eq!(backend.frame().unwrap().rules, Some(backend.world.rules()));
    assert_eq!(
        backend.slots[0].connection.rules_sent,
        Some(backend.world.rules())
    );
    // A repeated broadcast of unchanged geometry sends no rules update.
    backend.broadcast().unwrap();
    let frames = &backend.slots[0].connection.downlink;
    for (_, frame) in frames {
        assert!(
            protocol::split_frame(frame)
                .unwrap()
                .iter()
                .all(|p| p[0] != COMMAND)
        );
    }
    // The extension itself is accepted independently of a snake appearance.
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    state
        .receive_frame(
            &encode::frame(
                0,
                &[
                    encode::setup(670., 4.).unwrap(),
                    crate::client::rules_packet::encode(backend.world.rules()),
                ],
            )
            .unwrap(),
            0.,
        )
        .unwrap();
    assert_eq!(state.frame(0.).unwrap().rules, Some(backend.world.rules()));
}
