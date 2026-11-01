//! Authored synthetic sessions through the production received core.
#[path = "support/protocol13.rs"]
mod wire;
use slither_client::{
    protocol::ElapsedHeader::Absent,
    state::{ClientState, Direction, Point, PresentationEvent as Event, SnakeTarget, StateError},
};
use slither_protocol::scalar;
use wire::*;

fn ready() -> ClientState {
    let mut s = ClientState::default();
    s.apply_frame(&bundle(&[setup(), snake(0), snake(300)]), Absent)
        .unwrap();
    s
}

#[test]
fn appearance_uses_all_half_unit_pairs_and_explicit_zero_own_id() {
    let mut s = ClientState::default();
    let applied = s
        .apply_frame_with_events(&bundle(&[setup(), snake(0), snake(300)]), Absent)
        .unwrap();
    assert_eq!(s.setup.as_ref().unwrap().protocol, 13);
    assert_eq!(s.own_snake_id, Some(0));
    assert_eq!(s.snakes.len(), 2);
    let Event::SnakeAppearance(a) = &applied.events[1] else {
        panic!()
    };
    assert_eq!(a.name, "SYN");
    assert_eq!(a.custom_skin, [9, 8]);
    assert_eq!(a.absolute_head, Point { x: 1100., y: 1000. });
    assert_eq!(a.absolute_body.len(), 3);
    assert_eq!(a.absolute_body[0], Point { x: 943.2, y: 999.4 });
    assert_eq!(
        a.absolute_body[2],
        Point {
            x: 900.2,
            y: 1000.4
        }
    );
    assert_eq!(a.anchor_direction, None);
    assert_eq!(a.heading, scalar::angle24(0x400000));
    assert_eq!(a.wire_direction, Direction::Increasing);
    assert_eq!(
        s.snakes[&0].body[0],
        Point {
            x: -56.799999999999955,
            y: -0.6000000000000227
        }
    );
}

#[test]
fn coordinate_anchors_target_self_and_peers_and_survive_own_removal() {
    let mut s = ready();
    for id in [0, 300] {
        let applied = s
            .apply_frame_with_events(
                &bundle(&[
                    anchor(b'g', id, &[3, 232, 4, 76], None), // (1000, 1100)
                    anchor(b'G', id, &[128, 0], None),
                    anchor(b'N', id, &[255, 129], Some(0x800000)),
                ]),
                Absent,
            )
            .unwrap();
        assert_eq!(s.snakes[&id].head, Point { x: 127., y: -27. });
        assert_eq!(s.snakes[&id].body.len(), 4);
        assert_eq!(s.snakes[&id].fam, scalar::fraction24(0x800000));
        let Event::SnakeAnchor {
            target,
            absolute_position,
            grew,
            ..
        } = applied.events[2]
        else {
            panic!()
        };
        assert_eq!(
            target,
            if id == 0 {
                SnakeTarget::Own
            } else {
                SnakeTarget::Id(id)
            }
        );
        assert_eq!(absolute_position, Some(Point { x: 1127., y: 973. }));
        assert!(grew);
    }
    s.apply_frame(&[b's', 0, 0, 1], Absent).unwrap();
    let applied = s
        .apply_frame_with_events(&anchor(b'G', 0, &[129, 128], None), Absent)
        .unwrap();
    assert!(matches!(
        applied.events[0],
        Event::SnakeAnchor {
            target: SnakeTarget::Own,
            absolute_position: Some(Point { x: 1128., y: 973. }),
            ..
        }
    ));
    let applied = s
        .apply_frame_with_events(&[b'e', 0, 0, 32], Absent)
        .unwrap();
    assert!(matches!(
        applied.events[0],
        Event::SnakeMotion {
            target: SnakeTarget::Own,
            ..
        }
    ));
    assert_eq!(s.own_snake_id, Some(0));
}

#[test]
fn relative_base_is_the_last_appearance_delta_not_the_head() {
    let mut s = ready();
    let applied = s
        .apply_frame_with_events(&anchor(b'G', 300, &[129, 127], None), Absent)
        .unwrap();
    let Event::SnakeAnchor {
        absolute_position: Some(p),
        ..
    } = applied.events[0]
    else {
        panic!()
    };
    assert_eq!(p, Point { x: 944.2, y: 998.4 });
}

#[test]
fn every_motion_code_and_field_count_uses_the_explicit_id() {
    let mut s = ready();
    for id in [0u16, 300] {
        for code in *b"eE345" {
            for count in 1..=3 {
                let mut p = vec![code];
                p.extend(id.to_be_bytes());
                p.extend(&[32, 64, 96][..count]);
                if code == b'5' && count == 1 {
                    assert!(s.apply_frame(&p, Absent).is_err());
                    continue;
                }
                let applied = s.apply_frame_with_events(&p, Absent).unwrap();
                let Event::SnakeMotion {
                    target,
                    heading,
                    wanted_heading,
                    speed_per_32ms,
                    direction,
                } = applied.events[0]
                else {
                    panic!()
                };
                assert_eq!(
                    target,
                    if id == 0 {
                        SnakeTarget::Own
                    } else {
                        SnakeTarget::Id(id)
                    }
                );
                let (h, w, speed) = match (code, count) {
                    (_, 3) => (Some(32), Some(64), Some(96)),
                    (b'e', 2) => (Some(32), None, Some(64)),
                    (b'E' | b'4', 2) => (None, Some(32), Some(64)),
                    (b'3' | b'5', 2) => (Some(32), Some(64), None),
                    (b'e', 1) => (Some(32), None, None),
                    (b'E' | b'4', 1) => (None, Some(32), None),
                    (b'3', 1) => (None, None, Some(32)),
                    _ => unreachable!(),
                };
                assert_eq!(heading, h.map(scalar::angle8));
                assert_eq!(wanted_heading, w.map(scalar::angle8));
                assert_eq!(speed_per_32ms, speed.map(scalar::compact_speed_per_32ms));
                assert_eq!(
                    direction,
                    match (code, count) {
                        (b'e', 3) | (b'E' | b'3', 2) | (b'E', 1) => Some(Direction::Decreasing),
                        (_, 3) | (b'4' | b'5', 2) | (b'4', 1) => Some(Direction::Increasing),
                        _ => None,
                    }
                );
            }
        }
    }
}

#[test]
fn absolute_food_keys_preserve_adjacent_pellets_and_sector_clears() {
    let mut s = ready();
    let mut batch = food(b'F', 7, 301, 599, 20);
    batch.extend(&food(b'F', 8, 302, 599, 25)[1..]);
    let applied = s
        .apply_frame_with_events(
            &bundle(&[
                batch,
                food(b'f', 9, 600, 900, 15),
                food(b'b', 10, 65535, 65535, 30),
            ]),
            Absent,
        )
        .unwrap();
    assert_eq!(s.foods.len(), 4);
    let key = |x: u32, y: u32| (x << 16) | y;
    assert_eq!(
        s.foods[&key(301, 599)].position,
        Point { x: -699., y: -401. }
    );
    assert_eq!(s.foods[&key(301, 599)].sector, (1, 1));
    for (i, rapid) in [true, true, false, true].into_iter().enumerate() {
        let Event::FoodSpawn(input) = applied.events[i] else {
            panic!()
        };
        assert_eq!(input.rapid, rapid);
    }
    let applied = s
        .apply_frame_with_events(
            &bundle(&[
                eaten(301, 599, Some(0)),
                eaten(600, 900, None),
                vec![b'w', 1, 1],
            ]),
            Absent,
        )
        .unwrap();
    assert_eq!(s.foods.len(), 1);
    assert!(s.foods.contains_key(&u32::MAX));
    assert!(matches!(
        applied.events[0],
        Event::FoodConsume {
            eater_id: Some(0),
            ..
        }
    ));
    assert!(matches!(
        applied.events[1],
        Event::FoodConsume { eater_id: None, .. }
    ));
}

#[test]
fn mass_tail_short_removal_and_absolute_growth_share_existing_events() {
    let mut s = ready();
    let mut mass = vec![b'h', 1, 44];
    mass.extend(u24(0x800000));
    let mut tail = vec![b'r', 1, 44];
    tail.extend(u24(0x400000));
    let applied = s
        .apply_frame_with_events(
            &bundle(&[
                mass,
                tail,
                vec![b'r', 1, 44],
                anchor(b'n', 300, &[4, 0, 4, 0], Some(0)),
                vec![b's', 1, 44],
            ]),
            Absent,
        )
        .unwrap();
    assert!(matches!(
        applied.events[0],
        Event::SnakeMass { id: 300, .. }
    ));
    assert!(matches!(
        applied.events[1],
        Event::SnakeTailRemoved { fam: Some(_), .. }
    ));
    assert!(matches!(
        applied.events[2],
        Event::SnakeTailRemoved { fam: None, .. }
    ));
    assert!(matches!(
        applied.events[4],
        Event::SnakeRemoved {
            id: 300,
            died: false
        }
    ));
    assert!(!s.snakes.contains_key(&300));
}

#[test]
fn malformed_frames_rollback_bases_food_and_setup_and_protocol_is_connection_scoped() {
    let mut s = ready();
    let original = s.clone();
    for bad in [
        vec![b'G', 0, 0, 128],
        vec![b'F', 1],
        vec![b'f', 1, 0],
        vec![b'c', 0, 1, 0, 2, 0],
        vec![b'5', 0, 0, 1],
    ] {
        assert!(
            s.apply_frame(
                &bundle(&[
                    anchor(b'g', 0, &[3, 232, 3, 232], None),
                    food(b'b', 1, 1, 1, 1),
                    bad
                ]),
                Absent
            )
            .is_err()
        );
        assert_eq!(s, original);
    }
    let mut other = setup();
    other[23] = 19;
    assert_eq!(
        s.apply_frame(&other, Absent),
        Err(StateError::UnsupportedProtocol(19))
    );
    s.apply_frame(&setup(), Absent).unwrap();
    // Unsupported 19-only codes stay opaque even with malformed bodies.
    for code in *b"d7=+C<oW?" {
        assert_eq!(s.apply_frame(&[code], Absent).unwrap().ignored, 1);
    }
}

#[test]
fn shared_prey_and_radius_layouts_emit_identical_presentation_inputs() {
    let mut legacy = ready();
    let mut modern = ClientState::default();
    let mut config = setup();
    config[23] = 19;
    modern
        .apply_frame(&bundle(&[config, snake(0)]), Absent)
        .unwrap();
    let mut prey = vec![b'y', 0, 31, 9];
    prey.extend(u24(5500));
    prey.extend(u24(5000));
    prey.extend([20, b'0']);
    prey.extend(u24(0x123456));
    prey.extend(u24(0x234567));
    prey.extend(1000u16.to_be_bytes());
    let mut motion = vec![b'j', 0, 31, 1, 44, 1, 45, b'2'];
    motion.extend(u24(0x345678));
    motion.extend(u24(0x456789));
    motion.extend(900u16.to_be_bytes());
    for p in [
        prey,
        motion,
        vec![b'y', 0, 31, 0, 0],
        vec![b'y', 0, 31],
        vec![b'z', 0, 4, 0],
    ] {
        assert_eq!(
            legacy.apply_frame_with_events(&p, Absent).unwrap().events,
            modern.apply_frame_with_events(&p, Absent).unwrap().events
        );
    }
}
