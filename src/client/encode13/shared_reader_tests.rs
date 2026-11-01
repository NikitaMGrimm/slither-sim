//! Round trips through the independent simulator encoders, without sockets or
//! a production API. Root integration tests separately check fixed catalog
//! bytes, and slither-protocol's own tests use authored synthetic inputs.
use crate::{
    client::{
        Minimap, encode,
        encode13::{Encoder, FullMapCache},
        simulation::PlayerAppearance,
    },
    model::{Point, Snake},
    prey::Prey,
};
use slither_protocol::{
    DecodeAdmission::ClientPresentation,
    frame::*,
    v13::{self, *},
};

fn convert(encoder: &mut Encoder, source: Vec<u8>) -> Vec<u8> {
    let mut packets = encoder
        .packets(&[source], &mut FullMapCache::default())
        .unwrap();
    assert_eq!(packets.len(), 1);
    packets.remove(0)
}

fn snake() -> Snake {
    Snake {
        id: "Synthetic".into(),
        head: Point { x: 84., y: 0. },
        body: vec![
            Point { x: 84., y: 0. },
            Point { x: 42., y: 0. },
            Point::default(),
        ],
        heading: 0.,
        speed: 100.,
        radius: 5.,
        length: 3.5,
        segments: 3,
        fam: 0.5,
        score: 123.,
        kills: 0,
        alive: true,
    }
}

fn appearance(encoder: &mut Encoder, id: u16) -> Vec<u8> {
    convert(
        encoder,
        encode::appearance(
            id,
            &snake(),
            0.,
            &PlayerAppearance {
                nickname: b"Reader".to_vec(),
                skin: 3,
                custom_skin: vec![4, 5],
            },
        )
        .unwrap(),
    )
}

#[test]
fn setup_and_appearance_round_trip_with_fixed_raw_fields_and_all_body_pairs() {
    let mut encoder = Encoder::new(9);
    let setup = convert(&mut encoder, encode::setup(1800., 0.028).unwrap());
    let Message::Setup(s) = v13::decode(&setup, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(
        (s.protocol, s.grid_center, s.radius, s.body_spacing),
        (13, 32550, Some(1800), 42)
    );
    for id in [9, 17] {
        let p = appearance(&mut encoder, id);
        // Exercise the actual frame encoder and shared splitter as well.
        let framed = encode::frame(123, &[p]).unwrap();
        let frame = decode_frame(
            &framed,
            Header::TwoBytes,
            Limits::CLIENT,
            FrameAdmission::Bounded,
        )
        .unwrap();
        assert_eq!(frame.prefix(), Some(123));
        let Message::SnakeAppearance(a) =
            v13::decode_command(frame.commands().next().unwrap(), ClientPresentation).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            (a.id, a.heading, a.wanted_heading, a.speed, a.fraction),
            (id, 0, 0, 3200, 8_388_608)
        );
        assert_eq!(
            (a.name, a.custom_skin, a.skin),
            (b"Reader".as_slice(), [4, 5].as_slice(), 3)
        );
        assert_eq!((a.raw_direction, a.cosmetic), (b'0', 0));
        assert_eq!(a.head, [163170, 162750]);
        assert_eq!(a.body.first, Some([162750, 162750]));
        assert_eq!(
            a.body.deltas().collect::<Vec<_>>(),
            [[211, 127], [211, 127]]
        );
    }
}

#[test]
fn absolute_relative_and_growth_steps_keep_explicit_self_and_peer_ids() {
    let mut encoder = Encoder::new(9);
    for (own, id) in [(true, 9), (false, 17)] {
        appearance(&mut encoder, id);
        for (step, fraction) in [None, None, Some(123)].into_iter().enumerate() {
            let p = convert(&mut encoder, encode::anchor(own, id, 0, fraction));
            let Message::Anchor(a) = v13::decode(&p, ClientPresentation).unwrap() else {
                panic!()
            };
            assert_eq!(
                (a.id, a.grew, a.fraction),
                (id, fraction.is_some(), fraction)
            );
            if step == 0 {
                assert_eq!(p[0], b'g');
                assert_eq!(a.coordinates, Coordinates::Absolute([32676, 32550]));
            } else {
                assert_eq!(p[0], if fraction.is_some() { b'N' } else { b'G' });
                assert_eq!(a.coordinates.delta(), Some([42, 0]));
            }
        }
        let p = convert(
            &mut encoder,
            encode::anchor_at(own, id, 0, Point { x: 100., y: 200. }, Some(456)).unwrap(),
        );
        assert_eq!(p[0], b'n');
        assert_eq!(
            v13::decode(&p, ClientPresentation).unwrap(),
            Message::Anchor(AnchorRef {
                id,
                coordinates: Coordinates::Absolute([32650, 32750]),
                grew: true,
                fraction: Some(456),
            })
        );
    }
}

#[test]
fn every_emitted_motion_field_combination_round_trips_for_self_and_peers() {
    let mut encoder = Encoder::new(9);
    for (own, id) in [(true, 9), (false, 17)] {
        for direction in [1, 2] {
            for mask in 1..8 {
                let heading = (mask & 1 != 0).then_some(11);
                let wanted = (mask & 2 != 0).then_some(22);
                let speed = (mask & 4 != 0).then_some(33);
                let source = encode::motion(own, id, heading, wanted, speed, direction).unwrap();
                let p = convert(&mut encoder, source);
                let Message::Motion(m) = v13::decode(&p, ClientPresentation).unwrap() else {
                    panic!()
                };
                assert_eq!(
                    (m.id, m.heading, m.wanted_heading, m.speed),
                    (id, heading, wanted, speed)
                );
                assert_eq!(
                    m.direction,
                    wanted.map(|_| if direction == 1 {
                        Direction::Decreasing
                    } else {
                        Direction::Increasing
                    })
                );
            }
        }
    }
}

#[test]
fn dense_food_batches_births_and_pickups_share_exact_absolute_coordinates() {
    let mut encoder = Encoder::new(9);
    let style = encode::FoodStyle {
        color: 7,
        size: 24,
        snake_food: false,
    };
    let foods = (0..5000)
        .map(|i| (if i % 2 == 0 { [0, 0] } else { [128, 255] }, style))
        .collect::<Vec<_>>();
    let source = encode::food_sector([108, 109], &foods);
    let out = encoder
        .packets(&source, &mut FullMapCache::default())
        .unwrap();
    assert!(out.len() > 1);
    let mut count = 0;
    for p in out {
        assert!(encode::frameable(&p));
        let Message::FoodBatch(f) = v13::decode(&p, ClientPresentation).unwrap() else {
            panic!()
        };
        for food in f.foods() {
            assert_eq!((food.color, food.size), (7, 24));
            assert_eq!(
                food.position,
                if count % 2 == 0 {
                    [32400, 32700]
                } else {
                    [32550, 32999]
                }
            );
            count += 1;
        }
    }
    assert_eq!(count, 5000);
    for snake_food in [false, true] {
        let p = convert(
            &mut encoder,
            encode::food_add(
                [108, 109, 128, 255],
                encode::FoodStyle {
                    snake_food,
                    ..style
                },
            ),
        );
        assert_eq!(
            v13::decode(&p, ClientPresentation).unwrap(),
            Message::FoodSpawn(FoodSpawn {
                color: 7,
                position: [32550, 32999],
                size: 24,
                rapid: snake_food,
            })
        );
    }
    for eater in [None, Some(0), Some(17)] {
        let p = convert(
            &mut encoder,
            encode::food_remove([108, 109, 128, 255], eater),
        );
        assert_eq!(
            v13::decode(&p, ClientPresentation).unwrap(),
            Message::FoodEaten(FoodEaten {
                position: [32550, 32999],
                eater
            })
        );
    }
}

#[test]
fn shared_prey_hud_control_and_map_encoders_round_trip() {
    let mut encoder = Encoder::new(9);
    let prey = Prey {
        id: 123,
        position: Point { x: 3., y: 6. },
        heading: std::f64::consts::TAU / 4.,
        target_heading: std::f64::consts::TAU / 2.,
        speed: 100.,
        size: 3.,
    };
    let p = convert(&mut encoder, encode::prey_appearance(23, &prey).unwrap());
    let Message::PreyAppearance(p) = v13::decode(&p, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(
        (p.id, p.position, p.heading, p.wanted_heading, p.speed),
        (23, [162765, 162780], 4_194_304, 8_388_608, 3200)
    );
    assert_eq!((p.color, p.size, p.raw_direction), (0, 15, b'2'));
    for full in [false, true] {
        let p = convert(&mut encoder, encode::prey_motion(23, &prey, full).unwrap());
        let Message::PreyMotion(p) = v13::decode(&p, ClientPresentation).unwrap() else {
            panic!()
        };
        assert_eq!(
            (p.id, p.position, p.speed),
            (23, [10851, 10852], full.then_some(3200))
        );
        assert_eq!(
            (p.raw_direction, p.heading, p.wanted_heading),
            (
                full.then_some(b'2'),
                full.then_some(4_194_304),
                full.then_some(8_388_608)
            )
        );
    }
    for eater in [None, Some(9)] {
        let p = convert(&mut encoder, encode::prey_removal(23, eater));
        assert_eq!(
            v13::decode(&p, ClientPresentation).unwrap(),
            Message::PreyRemoval { id: 23, eater }
        );
    }
    let mut row = Vec::new();
    encode::leaderboard_row(&mut row, 123, 456, 7, b"Reader").unwrap();
    let p = convert(&mut encoder, encode::leaderboard(1, 2, 3, &row));
    let Message::Leaderboard(l) = v13::decode(&p, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(
        (l.local_row(), l.rank(), l.population(), l.row_count()),
        (1, 2, 3, 1)
    );
    let row = l.rows().next().unwrap();
    assert_eq!(
        (row.count(), row.raw_fraction(), row.raw_color(), row.name()),
        (123, 456, 7, b"Reader".as_slice())
    );
    for (source, expected) in [
        (
            encode::fam(9, 123),
            Message::Mass {
                id: 9,
                fraction: 123,
            },
        ),
        (
            encode::tail(9, 456),
            Message::Tail {
                id: 9,
                fraction: Some(456),
            },
        ),
        (encode::sector_clear([3, 4]), Message::SectorClear([3, 4])),
        (encode::radius(1800.).unwrap(), Message::Radius(1800)),
        (
            encode::session(9, 123),
            Message::Session {
                server_id: 9,
                session_id: 123,
            },
        ),
        (encode::kills(9, 2), Message::Kills { id: 9, count: 2 }),
        (encode::credit(7), Message::ScoreAdjustment(7)),
    ] {
        let p = convert(&mut encoder, source);
        assert_eq!(v13::decode(&p, ClientPresentation).unwrap(), expected);
    }
    let p = convert(&mut encoder, encode::pong());
    assert_eq!(
        v13::decode(&p, ClientPresentation).unwrap(),
        Message::Pong(b"p")
    );
    for (source, status) in [
        (encode::game_over(false), 0),
        (encode::game_over(true), 1),
        (encode::victory_accepted(), 2),
    ] {
        let p = convert(&mut encoder, source);
        assert_eq!(
            v13::decode(&p, ClientPresentation).unwrap(),
            Message::LifeControl {
                status: Some(status),
                command: &[b'v', status],
            }
        );
    }
    for (id, died) in [(9, false), (17, true)] {
        let p = convert(&mut encoder, encode::removal(id, died));
        let Message::SnakeRemoval(r) = v13::decode(&p, ClientPresentation).unwrap() else {
            panic!()
        };
        assert_eq!((r.id, r.raw_status), (id, Some(u8::from(died))));
    }
    let p = convert(
        &mut encoder,
        encode::record(123, 456, b"Reader", b"Text").unwrap(),
    );
    let Message::Announcement(a) = v13::decode(&p, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(
        (a.count(), a.raw_fraction(), a.name(), a.message()),
        (123, 456, b"Reader".as_slice(), b"Text".as_slice())
    );
    let map = Minimap {
        side: 4,
        cells: (0..16).map(|i| u8::from([0, 7, 15].contains(&i))).collect(),
        age_seconds: 0.,
    };
    let p = convert(&mut encoder, encode::minimap(&map));
    let Message::Minimap(m) = v13::decode(&p, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(
        (
            m.format(),
            m.advertised_size(),
            m.dimension(),
            m.layer_count()
        ),
        (minimap::MinimapFormat::SnapshotM, Some(4), Some(4), 1)
    );
    assert_eq!(
        m.selected(4).unwrap().map(|c| c.index).collect::<Vec<_>>(),
        [15, 7, 0]
    );
    let mut next = map.clone();
    next.cells[7] = 0;
    next.cells[14] = 1;
    let p = convert(&mut encoder, encode::minimap_delta(&map, &next).unwrap());
    let Message::Minimap(m) = v13::decode(&p, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(
        (
            m.format(),
            m.advertised_size(),
            m.dimension(),
            m.layer_count()
        ),
        (minimap::MinimapFormat::ToggleV, None, None, 1)
    );
    assert_eq!(
        m.selected(4).unwrap().map(|c| c.index).collect::<Vec<_>>(),
        [14, 7]
    );
}
