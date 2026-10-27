use super::*;
use crate::client::{
    LatencyConfig, Minimap, encode, minimap::MinimapDecoder, protocol::split_frame,
};

#[test]
fn a_legacy_recipient_preserves_other_slots_bytes_times_and_world() {
    let mut config = SimulationConfig {
        agents: 3,
        world: crate::Config::sandbox(),
        latency: LatencyConfig::measured_route(),
        ..Default::default()
    };
    config.world.radius = 1200.;
    config.world.vision = 3000.;
    config.world.food_count = 12000;
    config.minimap_interval_seconds = 0.1;
    config.leaderboard_interval_seconds = 0.1;
    let mut baseline = RawSimulator::new(7, config.clone(), 3).unwrap();
    let mut mixed = RawSimulator::new(7, config, 3).unwrap();
    for sim in [&mut baseline, &mut mixed] {
        for slot in 0..3 {
            sim.connect(
                slot,
                PlayerAppearance {
                    nickname: format!("Local{slot}").into_bytes(),
                    ..Default::default()
                },
            )
            .unwrap();
        }
        // A legacy socket can also opt into the existing physical-food/rules
        // header; the common local extension must not fail its connection.
        sim.physical_food(1);
    }
    mixed.protocol13(1);
    let mut codes = std::collections::BTreeSet::new();
    for tick in 0..600 {
        for sim in [&mut baseline, &mut mixed] {
            if tick == 350 {
                sim.backend.world.retire(1).unwrap();
            }
            if tick % 7 == 0 {
                for slot in 0..3 {
                    sim.receive(slot, &[(tick % 251) as u8]).unwrap();
                    sim.receive(slot, &[if tick % 21 == 0 { 253 } else { 254 }])
                        .unwrap();
                    sim.receive(slot, &[251]).unwrap();
                }
            }
            sim.advance_ticks(1).unwrap();
        }
        for slot in [0, 2] {
            assert_eq!(
                baseline.take(slot).unwrap(),
                mixed.take(slot).unwrap(),
                "stock slot {slot}, tick {tick}"
            );
        }
        baseline.take(1).unwrap();
        for (_, frame) in mixed.take(1).unwrap() {
            assert!(frame[2] >= 32, "13 cannot decode bundled frames");
            codes.insert(frame[2]);
        }
    }
    for code in b"asUFlpv" {
        assert!(codes.contains(code), "missing {}", char::from(*code));
    }
    assert_eq!(
        serde_json::to_value(baseline.backend.world.checkpoint().unwrap()).unwrap(),
        serde_json::to_value(mixed.backend.world.checkpoint().unwrap()).unwrap()
    );
    // Stock slot reuse clears the legacy encoder and its raster/front bases.
    mixed.connect(1, PlayerAppearance::default()).unwrap();
    assert!(mixed.backend.slots[1].connection.legacy.is_none());
}

#[test]
fn battledome_legacy_keeps_kill_board_pit_raster_and_edge_food() {
    let mut config = SimulationConfig {
        agents: 1,
        world: crate::Config::battledome(),
        ..Default::default()
    };
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    let mut sim = RawSimulator::new(7, config, 1).unwrap();
    sim.connect(
        0,
        PlayerAppearance {
            nickname: b"Local".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    sim.protocol13(0);
    // Synthetic retained kills: check the mode's real board, not a protocol-
    // specific score implementation. The collision owner is unchanged.
    let mut saved = serde_json::to_value(sim.backend.world.checkpoint().unwrap()).unwrap();
    saved["state"]["slots"][0]["killTimes"] = serde_json::json!([0., 0.]);
    sim.backend
        .world
        .restore(&serde_json::to_vec(&saved).unwrap(), Default::default())
        .unwrap();
    sim.backend.next_minimap = 0;
    sim.backend.next_leaderboard = 0;
    sim.advance_ticks(1).unwrap();
    let frames = sim.take(0).unwrap();
    let packets: Vec<_> = frames
        .iter()
        .flat_map(|(_, f)| split_frame(f).unwrap())
        .collect();
    let board = packets.iter().find(|p| p[0] == b'l').unwrap();
    let segments = u16::from_be_bytes([board[6], board[7]]) as usize;
    let fam = u32::from_be_bytes([0, board[8], board[9], board[10]]);
    assert_eq!(encode::display_score(segments, fam), 2.);
    assert!(
        packets.iter().any(|p| p[0] == b'F'
            && p[1..]
                .as_chunks::<6>()
                .0
                .iter()
                .any(|food| food[0] == 8 && food[5] == 31)),
        "edge ring must remain decorative food"
    );
    let map_packet = packets.iter().find(|p| p[0] == b'U').unwrap();
    let mut decoder = MinimapDecoder::default();
    let mut budget = usize::MAX;
    decoder.apply(map_packet, 0., &mut budget).unwrap();
    assert_eq!(
        decoder.shown.unwrap().0,
        Minimap::from_world(&sim.backend.world, 256).unwrap()
    );
}

#[test]
fn a_legacy_conversion_fault_closes_only_that_recipient() {
    let config = SimulationConfig {
        agents: 2,
        world: crate::Config::sandbox(),
        ..Default::default()
    };
    let mut sim = RawSimulator::new(7, config, 2).unwrap();
    for slot in 0..2 {
        sim.connect(slot, PlayerAppearance::default()).unwrap();
    }
    sim.protocol13(1);
    sim.advance_ticks(5).unwrap();
    let now = sim.time();
    // Protocol 13 has no layout for this code; stock frames it unchanged.
    for slot in 0..2 {
        sim.backend.emit(slot, now, &[vec![254, 1, 2]]).unwrap();
    }
    assert!(sim.take(0).is_ok());
    let reason = sim.take(1).unwrap_err();
    assert_eq!(
        reason,
        "Protocol-13 output failed: No protocol-13 encoding for code 254"
    );
    sim.advance_ticks(5).unwrap();
    assert!(sim.take(0).is_ok());
}
