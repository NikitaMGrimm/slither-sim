//! Authored exact single-anchor comparisons against ordinary packet transactions.
#[path = "support/protocol13.rs"]
mod wire;
use slither_client::{
    presentation::{AnimatedClient, ClientPresentationError, SnakePresentation},
    protocol::ElapsedHeader,
};

fn ready(protocol: u8) -> AnimatedClient {
    let mut setup = wire::setup();
    setup[23] = protocol;
    let mut client = AnimatedClient::new(0.).unwrap();
    client
        .apply_frame(
            &wire::bundle(&[setup, wire::snake(0), wire::snake(300)]),
            ElapsedHeader::Absent,
        )
        .unwrap();
    client.advance(20., false).unwrap();
    client
}

fn compare(base: &AnimatedClient, packet: &[u8], header: ElapsedHeader) -> bool {
    let mut fast = base.clone();
    let mut full = base.clone();
    let expected = full.apply_frame(packet, header);
    let actual = fast.try_apply_single_anchor(packet, header);
    let applied = match (actual, expected) {
        (Ok(Some(a)), Ok(b)) => {
            assert_eq!(a, b);
            true
        }
        (Err(a), Err(b)) => {
            assert_eq!(a, b);
            assert_eq!(&fast, base);
            false
        }
        (Ok(None), expected) => {
            assert_eq!(&fast, base);
            assert_eq!(fast.apply_frame(packet, header), expected);
            false
        }
        pair => panic!("packet={packet:?}, result={pair:?}"),
    };
    assert_eq!(fast, full, "packet={packet:?}");
    assert_eq!(format!("{fast:?}"), format!("{full:?}"));
    applied
}

#[test]
fn anchor_shapes_targets_headers_errors_and_detached_own_are_exact() {
    for protocol in [13, 19] {
        let initial = ready(protocol);
        let mut detached = initial.clone();
        detached
            .apply_frame(&[b's', 0, 0, 1], ElapsedHeader::Absent)
            .unwrap();
        detached
            .apply_frame(&wire::snake(0), ElapsedHeader::Absent)
            .unwrap();
        let mut accepted = 0;
        for base in [&initial, &detached] {
            for code in *b"gG=" {
                for id in [0_u16, 300, u16::MAX] {
                    for count in 0..=8 {
                        let mut command = vec![code];
                        command.extend(id.to_be_bytes());
                        command.extend(&[0, 127, 128, 255, 0, 1, 0, 0][..count]);
                        for header in [ElapsedHeader::Absent, ElapsedHeader::TwoBytes] {
                            let mut packet = if header == ElapsedHeader::TwoBytes {
                                vec![0x12, 0x34]
                            } else {
                                vec![]
                            };
                            packet.extend(&command);
                            accepted += usize::from(compare(base, &packet, header));
                        }
                    }
                }
            }
            for packet in [
                vec![b'g'],
                vec![b'g', 64],
                vec![b'G', 64],
                vec![b'=', 4, 0, 4, 1],
                vec![b'=', 0, 0, 4, 0, 4, 1],
            ] {
                accepted += usize::from(compare(base, &packet, ElapsedHeader::Absent));
            }
        }
        assert!(accepted > 20, "protocol={protocol}, accepted={accepted}");
    }
}

#[test]
fn pressure_growth_and_batches_decline_without_changing_state() {
    for protocol in [13, 19] {
        let mut client = ready(protocol);
        let growth = if protocol == 13 {
            wire::anchor(b'n', 0, &[4, 0, 4, 1], Some(0x123456))
        } else {
            vec![b'+', 4, 0, 4, 1, 0x12, 0x34, 0x56]
        };
        let anchor = if protocol == 13 {
            wire::anchor(b'g', 0, &[4, 0, 4, 1], None)
        } else {
            vec![b'=', 4, 0, 4, 1]
        };
        for packet in [growth, wire::bundle(&[anchor.clone(), vec![b'?']])] {
            let before = client.clone();
            assert!(
                client
                    .try_apply_single_anchor(&packet, ElapsedHeader::Absent)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(client, before);
        }
        let appearances: Vec<_> = (1..257).map(wire::snake).collect();
        client
            .apply_frame(&wire::bundle(&appearances), ElapsedHeader::Absent)
            .unwrap();
        let before = client.clone();
        assert!(
            client
                .try_apply_single_anchor(&anchor, ElapsedHeader::Absent)
                .unwrap()
                .is_none()
        );
        assert_eq!(client, before);
        compare(&client, &anchor, ElapsedHeader::Absent);
    }
}

#[test]
fn consecutive_and_zero_distance_anchors_keep_the_ordinary_error_boundary() {
    for protocol in [13, 19] {
        for moving in [false, true] {
            let mut fast = ready(protocol);
            let mut full = fast.clone();
            for i in 0..100_u16 {
                let x = (1200 + if moving { i } else { 0 }).to_be_bytes();
                let packet = if protocol == 13 {
                    wire::anchor(b'g', 0, &[x[0], x[1], 4, 76], None)
                } else {
                    vec![b'=', 0, 0, x[0], x[1], 4, 76]
                };
                let before = fast.clone();
                let expected = full.apply_frame(&packet, ElapsedHeader::Absent);
                let actual = fast.try_apply_single_anchor(&packet, ElapsedHeader::Absent);
                assert_eq!(actual, expected.clone().map(Some));
                assert_eq!(fast, full);
                assert_eq!(format!("{fast:?}"), format!("{full:?}"));
                if expected.is_err() {
                    assert_eq!(fast, before);
                    break;
                }
            }
        }
    }
}

#[test]
fn staged_anchor_rejection_preserves_live_and_frozen_state() {
    for protocol in [13, 19] {
        let mut client = ready(protocol);
        let frozen = client.clone();
        let frozen_before = format!("{frozen:?}");
        let anchor = if protocol == 13 {
            wire::anchor(b'g', 0, &[4, 176, 4, 76], None)
        } else {
            vec![b'=', 0, 0, 4, 176, 4, 76]
        };
        assert!(
            client
                .try_apply_single_anchor(&anchor, ElapsedHeader::Absent)
                .unwrap()
                .is_some()
        );
        assert_ne!(format!("{client:?}"), frozen_before);
        assert_eq!(format!("{frozen:?}"), frozen_before);
        client.prepare_draw(960, 540).unwrap();
        assert!(client.own_snake().unwrap().metrics().visible);

        let snapshot = client.clone();
        // These owned observations cannot change through shared snapshot Arcs.
        let before = format!("{client:?}");
        let own_before: SnakePresentation = client.own_snake().unwrap().clone();
        let camera_before = client.camera().clone();
        let mut repeated = vec![0x12, 0x34];
        repeated.extend(&anchor);
        // The valid repeated anchor succeeds in received state, then fails in
        // visible presentation at zero distance. It must not publish that staged
        // received snake, elapsed header or counters before presentation accepts.
        let mut received = client.received().clone();
        received
            .apply_frame(&repeated, ElapsedHeader::TwoBytes)
            .unwrap();
        assert_ne!(&received, client.received());
        let rejected = client.try_apply_single_anchor(&repeated, ElapsedHeader::TwoBytes);
        assert!(
            matches!(rejected, Err(ClientPresentationError::Presentation(_))),
            "protocol={protocol}, result={rejected:?}"
        );
        assert_eq!(format!("{client:?}"), before);
        assert_eq!(format!("{snapshot:?}"), before);
        assert_eq!(client.own_snake().unwrap(), &own_before);
        assert_eq!(snapshot.own_snake().unwrap(), &own_before);
        assert_eq!(client.camera(), &camera_before);
        assert_eq!(format!("{frozen:?}"), frozen_before);
    }
}
