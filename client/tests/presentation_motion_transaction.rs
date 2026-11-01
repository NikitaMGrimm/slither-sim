//! Authored scalar-motion transactions; no recorded data, network, or timing gate.
#[path = "support/protocol13.rs"]
mod wire;
use slither_client::{presentation::AnimatedClient, protocol::ElapsedHeader};

fn ready(protocol: u8) -> AnimatedClient {
    let mut client = AnimatedClient::new(0.).unwrap();
    let mut setup = wire::setup();
    setup[23] = protocol;
    client
        .apply_frame(
            &wire::bundle(&[setup, wire::snake(0), wire::snake(300)]),
            ElapsedHeader::Absent,
        )
        .unwrap();
    client.advance(20., false).unwrap();
    client
}

#[test]
fn singleton_motion_matches_full_transaction_for_both_protocols_and_every_shape() {
    for protocol in [13, 19] {
        let base = ready(protocol);
        let mut accepted = 0;
        for code in *b"eE345d7" {
            for id in [0_u16, 300, 65535] {
                for count in 0..=5 {
                    let mut packet = vec![code];
                    packet.extend(id.to_be_bytes());
                    packet.extend(&[32, 64, 96, 128, 160][..count]);
                    for header in [ElapsedHeader::Absent, ElapsedHeader::TwoBytes] {
                        let packet = if header == ElapsedHeader::TwoBytes {
                            [vec![0, 23], packet.clone()].concat()
                        } else {
                            packet.clone()
                        };
                        let mut full = base.clone();
                        let mut single = base.clone();
                        let expected = full.apply_frame(&packet, header);
                        let actual = single.try_apply_single_motion(&packet, header);
                        match (expected, actual) {
                            (Ok(expected), Ok(Some(actual))) => {
                                assert_eq!(actual, expected);
                                accepted += 1;
                            }
                            (Err(expected), Err(actual)) => assert_eq!(actual, expected),
                            (Ok(expected), Ok(None)) => {
                                assert_eq!(single, base);
                                assert_eq!(single.apply_frame(&packet, header).unwrap(), expected);
                            }
                            pair => panic!("protocol={protocol}, packet={packet:?}: {pair:?}"),
                        }
                        assert_eq!(single, full, "protocol={protocol}, packet={packet:?}");
                    }
                }
            }
        }
        // Explicit own forms in 13, compact own forms and no-op 5 in 19.
        for packet in [
            vec![b'e', 32],
            vec![b'3', 32, 64],
            vec![b'5', 32],
            vec![b'd', 32, 64, 96],
        ] {
            let mut full = base.clone();
            let mut single = base.clone();
            let expected = full.apply_frame(&packet, ElapsedHeader::Absent);
            let actual = single.try_apply_single_motion(&packet, ElapsedHeader::Absent);
            match actual {
                Ok(None) => {
                    single.apply_frame(&packet, ElapsedHeader::Absent).unwrap();
                }
                actual => assert_eq!(actual, expected.map(Some)),
            }
            assert_eq!(single, full);
        }
        assert!(accepted > 50);
    }
}

#[test]
fn detached_own_and_reused_wire_id_keep_original_target_and_gaze() {
    for protocol in [13, 19] {
        let mut base = ready(protocol);
        base.apply_frame(&[b's', 0, 0, 1], ElapsedHeader::Absent)
            .unwrap();
        for packet in [
            vec![b'e', 0, 0, 32],
            vec![b'3', 1, 44, 64, 96],
            vec![b'5', 255, 255, 96],
        ] {
            let mut full = base.clone();
            let mut single = base.clone();
            assert_eq!(
                single.try_apply_single_motion(&packet, ElapsedHeader::Absent),
                full.apply_frame(&packet, ElapsedHeader::Absent).map(Some)
            );
            assert_eq!(single, full);
        }
        base.apply_frame(&wire::snake(300), ElapsedHeader::Absent)
            .unwrap();
        let packet = [b'E', 1, 44, 96, 128];
        let mut full = base.clone();
        assert_eq!(
            base.try_apply_single_motion(&packet, ElapsedHeader::Absent),
            full.apply_frame(&packet, ElapsedHeader::Absent).map(Some)
        );
        assert_eq!(base, full);
    }
}

#[test]
fn batches_other_commands_and_missing_setup_leave_the_fast_lane_untouched() {
    let mut empty = AnimatedClient::new(0.).unwrap();
    let before = empty.clone();
    assert_eq!(
        empty
            .try_apply_single_motion(&[b'e', 32], ElapsedHeader::Absent)
            .unwrap(),
        None
    );
    assert_eq!(empty, before);
    for protocol in [13, 19] {
        let mut client = ready(protocol);
        let before = client.clone();
        for packet in [
            vec![b'p'],
            wire::bundle(&[vec![b'e', 0, 0, 32], vec![b'e', 1, 44, 64]]),
        ] {
            assert_eq!(
                client
                    .try_apply_single_motion(&packet, ElapsedHeader::Absent)
                    .unwrap(),
                None
            );
            assert_eq!(client, before);
        }
        assert!(
            client
                .try_apply_single_motion(&[0], ElapsedHeader::Absent)
                .is_err()
        );
        assert_eq!(client, before);
    }
}
