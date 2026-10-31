//! Authored synthetic bytes; independent of both server encoders.
mod common;
use slither_protocol::{
    DecodeAdmission::*,
    ErrorKind,
    frame::*,
    scalar,
    v13::{self, *},
    v19,
};

#[test]
fn version_selection_and_setup_admission_are_explicit() {
    let mut base = common::setup();
    base[23] = 13;
    for admission in [Sim, ClientReceived, ClientPresentation] {
        let Message::Setup(s) = decode(&base, admission).unwrap() else {
            panic!()
        };
        assert_eq!(
            (s.protocol, s.grid_center, s.sector_size, s.body_spacing),
            (13, 1000, 100, 10)
        );
        assert_eq!(
            v19::decode(&base, admission).unwrap_err().kind,
            ErrorKind::UnsupportedVersion
        );
    }
    assert_eq!(setup_protocol_version(&base, ClientReceived).unwrap(), 13);
    base.extend([0, 1, 0, 0, 2, 2]);
    assert!(decode(&base, Sim).is_ok()); // team mode without its team byte
    assert_eq!(decode(&base, ClientReceived).unwrap_err().offset, 31);
    base.push(7);
    assert!(decode(&base, ClientReceived).is_ok());
    for (at, len) in [(8, 2), (27, 3)] {
        let mut p = base.clone();
        p[at..at + len].fill(0);
        assert!(decode(&p, Sim).is_ok());
        let e = decode(&p, ClientReceived).unwrap_err();
        assert_eq!((e.kind, e.offset), (ErrorKind::InvalidScalar, at));
    }
    for n in [26, 28, 29] {
        assert!(decode(&base[..n], Sim).is_err());
    }
    base[23] = 19;
    assert_eq!(decode(&base, Sim).unwrap_err().offset, 23);
}

#[test]
fn appearance_preserves_every_half_unit_pair_including_the_last() {
    let p = common::appearance();
    let Message::SnakeAppearance(a) = decode(&p, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(
        (
            a.command,
            a.id,
            a.heading,
            a.raw_direction,
            a.wanted_heading
        ),
        (&p[..], 17, 1, b'2', 0xff_ffff)
    );
    assert_eq!(
        (a.speed, a.fraction, a.skin, a.head, a.cosmetic),
        (321, 8_388_607, 55, [5001, 5002], 7)
    );
    assert_eq!((a.name, a.custom_skin), (&[0, 255][..], &[123, 234][..]));
    assert_eq!(a.body.first, Some([4999, 5000]));
    assert_eq!(a.body.raw_tail(), [0, 255, 0x12, 0x34]);
    for _ in 0..2 {
        let mut deltas = a.body.deltas();
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas.next(), Some([0, 255]));
        assert_eq!(deltas.next(), Some([0x12, 0x34]));
        assert_eq!(deltas.next(), None);
        assert_eq!(deltas.next(), None);
    }
    assert_eq!(
        [scalar::delta_half(0), scalar::delta_half(255)],
        [-63.5, 64.]
    );
    let mut p = p;
    p[6] = 255;
    assert!(decode(&p, Sim).is_ok());
    assert!(decode(&p, ClientReceived).is_ok());
    assert_eq!(decode(&p, ClientPresentation).unwrap_err().offset, 6);
    p[6] = b'0';
    let Message::SnakeAppearance(empty) = decode(&p[..29], ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(empty.body.first, None);
    assert_eq!(empty.body.deltas().len(), 0);
    let Message::SnakeAppearance(first) = decode(&p[..35], ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(first.body.first, Some([4999, 5000]));
    assert_eq!(first.body.deltas().len(), 0);
    for n in [30, 31, 32, 33, 34, 36, 38] {
        assert!(decode(&p[..n], Sim).is_err());
    }
}

#[test]
fn anchors_are_explicit_coordinates_with_no_invented_angle_or_own_id() {
    for admission in [Sim, ClientReceived, ClientPresentation] {
        for (p, expected) in [
            (
                &b"g\0\0\0\0\xff\xff"[..],
                AnchorRef {
                    id: 0,
                    coordinates: Coordinates::Absolute([0, 65535]),
                    grew: false,
                    fraction: None,
                },
            ),
            (
                &b"n\xff\xff\x12\x34\xab\xcd\xff\xff\xff"[..],
                AnchorRef {
                    id: 65535,
                    coordinates: Coordinates::Absolute([0x1234, 0xabcd]),
                    grew: true,
                    fraction: Some(0xff_ffff),
                },
            ),
            (
                &b"G\x12\x34\0\xff"[..],
                AnchorRef {
                    id: 0x1234,
                    coordinates: Coordinates::Relative([0, 255]),
                    grew: false,
                    fraction: None,
                },
            ),
            (
                &b"N\0\0\x80\x7f\0\0\0"[..],
                AnchorRef {
                    id: 0,
                    coordinates: Coordinates::Relative([128, 127]),
                    grew: true,
                    fraction: Some(0),
                },
            ),
        ] {
            assert_eq!(decode(p, admission).unwrap(), Message::Anchor(expected));
        }
        for (code, length) in [(b'g', 7), (b'n', 10), (b'G', 5), (b'N', 8)] {
            for n in 1..=14 {
                let mut p = vec![0; n];
                p[0] = code;
                assert_eq!(decode(&p, admission).is_ok(), n == length, "{code} {n}");
            }
        }
    }
    assert_eq!(Coordinates::Relative([0, 255]).delta(), Some([-128, 127]));
    assert_eq!(Coordinates::Absolute([0, 0]).delta(), None);
}

#[test]
fn every_motion_code_length_has_fixed_fields_and_interpreted_direction() {
    // heading, wanted heading, speed, direction; all IDs are explicit.
    let cases = [
        (b'e', 1, Some(11), None, None, None),
        (b'e', 2, Some(11), None, Some(22), None),
        (
            b'e',
            3,
            Some(11),
            Some(22),
            Some(33),
            Some(Direction::Decreasing),
        ),
        (b'E', 1, None, Some(11), None, Some(Direction::Decreasing)),
        (
            b'E',
            2,
            None,
            Some(11),
            Some(22),
            Some(Direction::Decreasing),
        ),
        (
            b'E',
            3,
            Some(11),
            Some(22),
            Some(33),
            Some(Direction::Increasing),
        ),
        (b'3', 1, None, None, Some(11), None),
        (
            b'3',
            2,
            Some(11),
            Some(22),
            None,
            Some(Direction::Decreasing),
        ),
        (
            b'3',
            3,
            Some(11),
            Some(22),
            Some(33),
            Some(Direction::Increasing),
        ),
        (b'4', 1, None, Some(11), None, Some(Direction::Increasing)),
        (
            b'4',
            2,
            None,
            Some(11),
            Some(22),
            Some(Direction::Increasing),
        ),
        (
            b'4',
            3,
            Some(11),
            Some(22),
            Some(33),
            Some(Direction::Increasing),
        ),
        (
            b'5',
            2,
            Some(11),
            Some(22),
            None,
            Some(Direction::Increasing),
        ),
        (
            b'5',
            3,
            Some(11),
            Some(22),
            Some(33),
            Some(Direction::Increasing),
        ),
    ];
    for admission in [Sim, ClientReceived, ClientPresentation] {
        for (code, count, heading, wanted_heading, speed, direction) in cases {
            let p = [code, 0xff, 0xff, 11, 22, 33];
            let p = &p[..3 + count];
            assert_eq!(
                decode(p, admission).unwrap(),
                Message::Motion(MotionRef {
                    command: p,
                    id: 65535,
                    heading,
                    wanted_heading,
                    speed,
                    direction
                })
            );
        }
        for code in *b"eE345" {
            for n in 1..=8 {
                let mut p = vec![0; n];
                p[0] = code;
                assert_eq!(
                    decode(&p, admission).is_ok(),
                    (4..=6).contains(&n) && !(code == b'5' && n == 4)
                );
            }
        }
    }
}

#[test]
fn foods_keep_absolute_coordinates_color_size_and_optional_explicit_eater() {
    let p = b"F\xff\0\0\xff\xff\x21\x03\x12\x34\xab\xcd\0";
    let Message::FoodBatch(f) = decode(p, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(f.raw_tuples(), &p[1..]);
    let expected = [
        FoodTuple {
            color: 255,
            position: [0, 65535],
            size: 33,
        },
        FoodTuple {
            color: 3,
            position: [0x1234, 0xabcd],
            size: 0,
        },
    ];
    for _ in 0..2 {
        assert_eq!(f.foods().collect::<Vec<_>>(), expected);
    }
    for code in *b"fb" {
        let p = [code, 255, 0, 0, 255, 255, 0];
        assert_eq!(
            decode(&p, Sim).unwrap(),
            Message::FoodSpawn(FoodSpawn {
                color: 255,
                position: [0, 65535],
                size: 0,
                rapid: code == b'b'
            })
        );
    }
    assert_eq!(
        decode(b"c\xff\xff\0\0", Sim).unwrap(),
        Message::FoodEaten(FoodEaten {
            position: [65535, 0],
            eater: None
        })
    );
    assert_eq!(
        decode(b"c\xff\xff\0\0\0\0", Sim).unwrap(),
        Message::FoodEaten(FoodEaten {
            position: [65535, 0],
            eater: Some(0)
        })
    );
    let Message::FoodBatch(empty) = decode(b"F", Sim).unwrap() else {
        panic!()
    };
    assert_eq!(empty.foods().len(), 0);
    for admission in [Sim, ClientReceived, ClientPresentation] {
        for code in *b"Ffbc" {
            for n in 1..=15 {
                let mut p = vec![0; n];
                p[0] = code;
                let valid = match code {
                    b'F' => (n - 1) % 6 == 0,
                    b'f' | b'b' => n == 7,
                    b'c' => matches!(n, 5 | 7),
                    _ => unreachable!(),
                };
                assert_eq!(decode(&p, admission).is_ok(), valid, "{code} {n}");
            }
        }
    }
}

#[test]
fn shared_families_keep_admission_and_never_select_a_consumer_protocol() {
    for n in 3..=7 {
        let mut p = vec![0; n];
        p[0] = b's';
        assert!(decode(&p, Sim).is_ok());
        assert_eq!(decode(&p, ClientReceived).is_ok(), n <= 4);
    }
    for p in [&b"p\0"[..], b"v", b"v\x02tail", b"z\0\0\0"] {
        assert!(decode(p, Sim).is_ok());
        assert!(decode(p, ClientReceived).is_err());
    }
    for code in *b"ULM" {
        let p = if code == b'L' {
            vec![code, 1, 0, 0]
        } else {
            vec![code, 0, 0]
        };
        assert!(decode(&p, Sim).is_err());
        assert!(decode(&p, ClientReceived).is_ok());
    }
    for p in [
        &b"M\0\x02\x40"[..],
        b"U\0\x02\x40",
        b"L\x01\0\x02\x40",
        b"u\x40",
        b"V\x40",
    ] {
        let Message::Minimap(m) = decode(p, ClientReceived).unwrap() else {
            panic!()
        };
        assert_eq!(m.command(), p);
        assert_eq!(m.selected(m.dimension().unwrap_or(2)).unwrap().count(), 1);
    }
    assert!(matches!(
        decode(b"R\x01", Sim).unwrap(),
        Message::ScoreAdjustment(1)
    ));
    assert!(matches!(
        decode(b"m\0\0\x01\0\0\0\0text", Sim).unwrap(),
        Message::Announcement(_)
    ));
    for code in *b"d7=+C<oW?" {
        let p = [code, 0, 0, 1];
        assert_eq!(
            decode(&p, Sim).unwrap(),
            Message::Unknown {
                opcode: code,
                command: &p
            }
        );
    }
}

#[test]
fn errors_retain_opcode_and_original_frame_offsets() {
    let bytes = [0x12, 0x34, 0, 1, b'p', 0, 4, b'G', 0, 1, 128];
    let frame = decode_frame(
        &bytes,
        Header::TwoBytes,
        Limits::CLIENT,
        FrameAdmission::Bounded,
    )
    .unwrap();
    for _ in 0..2 {
        let command = frame.commands().nth(1).unwrap();
        let direct = decode(command.bytes, ClientReceived).unwrap_err();
        let framed = decode_command(command, ClientReceived).unwrap_err();
        assert_eq!(
            (direct.kind, direct.opcode, direct.offset),
            (ErrorKind::InvalidLength, Some(b'G'), 4)
        );
        assert_eq!(framed.offset, 11);
    }
    let mut bad = common::appearance();
    bad[6] = 255;
    let error = decode_command(
        CommandRef {
            index: 7,
            offset: 100,
            bytes: &bad,
        },
        ClientPresentation,
    )
    .unwrap_err();
    assert_eq!(
        (error.kind, error.opcode, error.offset),
        (ErrorKind::InvalidScalar, Some(b's'), 106)
    );
}

#[test]
fn truncated_and_adversarial_inputs_never_panic() {
    let mut packets = vec![common::setup(), common::appearance()];
    packets.extend((0..=255).map(|code| {
        let mut p = vec![255; 40];
        p[0] = code;
        p
    }));
    for p in packets {
        for end in 0..=p.len() {
            for admission in [Sim, ClientReceived, ClientPresentation] {
                let _ = v13::decode(&p[..end], admission);
            }
        }
    }
}
