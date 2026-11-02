//! Authored consume transactions; the ordinary batch path is the exact oracle.
use super::*;
#[path = "../../tests/support/protocol13.rs"]
mod wire;

fn apply(client: &mut AnimatedClient, packet: &[u8]) {
    client
        .apply_frame_with_factories(
            packet,
            ElapsedHeader::Absent,
            |_| Ok(PairedFaceGeometry::default()),
            super::resolved_draw_tests::factory,
        )
        .unwrap();
}

fn ready(protocol: u8) -> AnimatedClient {
    let mut client = AnimatedClient::new(0.).unwrap();
    let mut setup = wire::setup();
    setup[23] = protocol;
    apply(
        &mut client,
        &wire::bundle(&[setup, wire::snake(7), wire::snake(300)]),
    );
    for _ in 0..2 {
        let food = if protocol == 13 {
            wire::food(b'f', 7, 1000, 1000, 20)
        } else {
            vec![b'f', 10, 10, 0, 0, 7, 20]
        };
        apply(&mut client, &food);
    }
    client.advance(100., false).unwrap();
    client
}

#[test]
fn singleton_consume_matches_batches_including_caches_duplicates_and_id_reuse() {
    for protocol in [13, 19] {
        let mut fast = ready(protocol);
        let mut full = fast.clone();
        // Retain the old eater independently of a replacement with the same ID.
        for packet in [vec![b's', 1, 44, 1], wire::snake(300)] {
            apply(&mut fast, &packet);
            apply(&mut full, &packet);
        }
        let packets = if protocol == 13 {
            vec![
                wire::eaten(1000, 1000, Some(300)),
                wire::eaten(1000, 1000, None),
                wire::eaten(1000, 1000, Some(65535)),
                wire::eaten(1001, 1000, Some(7)),
            ]
        } else {
            vec![
                vec![b'<', 10, 10, 0, 0, 1, 44],
                vec![b'C', 0, 0],
                vec![b'<', 0, 0, 255, 255],
                vec![b'C', 0, 0],
                vec![b'c', 10, 10, 0, 0],
                vec![b'c', 1, 0],
            ]
        };
        for packet in packets {
            for timed in [false, true] {
                let before = fast.clone();
                let header = if timed {
                    ElapsedHeader::TwoBytes
                } else {
                    ElapsedHeader::Absent
                };
                let prefix = if timed { vec![0x12, 0x34] } else { vec![] };
                let mut single = prefix.clone();
                single.extend(&packet);
                let mut batch = prefix;
                batch.extend(wire::bundle(&[packet.clone(), vec![b'?']]));
                fast.try_apply_single_consume(&single, header)
                    .unwrap()
                    .unwrap();
                full.apply_frame(&batch, header).unwrap();
                let received = Arc::make_mut(&mut full.received);
                received.commands_applied -= 1;
                received.ignored_commands -= 1;
                assert_eq!(fast, full);
                assert_eq!(format!("{fast:?}"), format!("{full:?}"));
                assert_eq!(
                    before.received().frames_applied + 1,
                    fast.received().frames_applied
                );
            }
        }
        fast.advance(120., false).unwrap();
        full.advance(120., false).unwrap();
        assert_eq!(fast, full);
    }
}

#[test]
fn malformed_consumes_preserve_snapshots_and_batches_keep_rollback() {
    for protocol in [13, 19] {
        let base = ready(protocol);
        let packet = if protocol == 13 {
            wire::eaten(1000, 1000, Some(7))
        } else {
            vec![b'<', 10, 10, 0, 0, 0, 7]
        };
        for length in 1..packet.len() {
            let mut fast = base.clone();
            let mut full = base.clone();
            let actual = fast.try_apply_single_consume(&packet[..length], ElapsedHeader::Absent);
            let expected = full.apply_frame(&packet[..length], ElapsedHeader::Absent);
            match (actual, expected) {
                (Err(a), Err(b)) => {
                    assert_eq!(a, b);
                    assert_eq!(fast, base);
                }
                (Ok(Some(_)), Ok(_)) => assert_eq!(fast, full),
                pair => panic!("protocol={protocol}, length={length}: {pair:?}"),
            }
        }
        let mut client = base.clone();
        let invalid = wire::bundle(&[packet.clone(), vec![b'h', 0, 7, 0]]);
        assert!(client.apply_frame(&invalid, ElapsedHeader::Absent).is_err());
        assert_eq!(client, base);
        let batch = wire::bundle(&[packet]);
        let report = client
            .try_apply_single_consume(&batch, ElapsedHeader::Absent)
            .unwrap();
        assert!(report.is_some());
        assert_eq!(base, ready(protocol));
    }
}
