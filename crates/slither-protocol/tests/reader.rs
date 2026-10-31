//! Authored synthetic compatibility and independent byte expectations.
mod common;
use slither_protocol::{
    DecodeAdmission::*,
    ErrorKind,
    frame::*,
    scalar,
    v19::{self, *},
};

#[test]
fn framing_is_repeatable_and_attributed_to_original_offsets() {
    let bytes = [0x12, 0x34, 0, 1, b'p', 34, b'v', 2, 0, 3, b'?', 1, 2];
    let frame = decode_frame(
        &bytes,
        Header::TwoBytes,
        Limits::CLIENT,
        FrameAdmission::Bounded,
    )
    .unwrap();
    assert_eq!(frame.prefix(), Some(0x1234));
    assert_eq!(frame.command_count(), 3);
    let expected = [
        CommandRef {
            index: 0,
            offset: 4,
            bytes: b"p",
        },
        CommandRef {
            index: 1,
            offset: 6,
            bytes: &[b'v', 2],
        },
        CommandRef {
            index: 2,
            offset: 10,
            bytes: b"?\x01\x02",
        },
    ];
    for _ in 0..3 {
        assert_eq!(frame.commands().collect::<Vec<_>>(), expected);
    }
    assert_eq!(frame.commands().len(), 3);
    let bad = [0, 1, b'p', 0];
    let err = decode_frame(
        &bad,
        Header::Absent,
        Limits::CLIENT,
        FrameAdmission::Bounded,
    )
    .unwrap_err();
    assert_eq!((err.kind, err.offset), (ErrorKind::Truncated, 4));
    let bytes = [0, 0, 0, 2, b'h', 0];
    let command = decode_frame(
        &bytes,
        Header::TwoBytes,
        Limits::CLIENT,
        FrameAdmission::Bounded,
    )
    .unwrap()
    .commands()
    .next()
    .unwrap();
    let error = decode_command(command, ClientReceived).unwrap_err();
    assert_eq!(error.opcode, Some(b'h'));
    assert_eq!(error.offset, 6);
    // First high byte always starts a bare command, even if it resembles a short prefix.
    assert_eq!(
        decode_frame(
            &[33, b'p'],
            Header::Absent,
            Limits::CLIENT,
            FrameAdmission::Bounded
        )
        .unwrap()
        .commands()
        .next()
        .unwrap()
        .bytes,
        &[33, b'p']
    );
}
#[test]
fn historical_and_bounded_envelopes_keep_distinct_limits() {
    let low = [0, 1, 0];
    assert!(
        decode_frame(
            &low,
            Header::Absent,
            Limits::UNBOUNDED,
            FrameAdmission::Historical
        )
        .is_ok()
    );
    assert_eq!(
        decode_frame(
            &low,
            Header::Absent,
            Limits::CLIENT,
            FrameAdmission::Bounded
        )
        .unwrap_err()
        .kind,
        ErrorKind::InvalidCommand
    );
    let many = [0, 1, b'p'].repeat(65_537);
    assert_eq!(
        decode_frame(
            &many,
            Header::Absent,
            Limits::UNBOUNDED,
            FrameAdmission::Historical
        )
        .unwrap()
        .command_count(),
        65_537
    );
    assert_eq!(
        decode_frame(
            &many,
            Header::Absent,
            Limits::CLIENT,
            FrameAdmission::Bounded
        )
        .unwrap_err()
        .kind,
        ErrorKind::TooManyCommands
    );
    for count in [1, 4096] {
        let bytes = [0, 1, b'p'].repeat(count);
        assert_eq!(
            decode_frame(
                &bytes,
                Header::Absent,
                Limits::CLIENT,
                FrameAdmission::Bounded
            )
            .unwrap()
            .command_count(),
            count
        );
    }
    assert!(
        decode_frame(
            b"p",
            Header::Absent,
            Limits {
                max_bytes: 0,
                max_commands: 1
            },
            FrameAdmission::Historical
        )
        .is_err()
    );
    assert!(
        decode_frame(
            b"p",
            Header::Absent,
            Limits {
                max_bytes: 1,
                max_commands: 0
            },
            FrameAdmission::Historical
        )
        .is_err()
    );
    for bytes in [&[][..], &[0], &[0, 0], &[0, 2, b'p'], &[0, 1, b'p', 32]] {
        assert!(
            decode_frame(
                bytes,
                Header::Absent,
                Limits::CLIENT,
                FrameAdmission::Bounded
            )
            .is_err()
        );
    }
}
#[test]
fn setup_boundaries_and_consumer_scalar_admission_are_explicit() {
    let base = common::setup();
    for n in 1..25 {
        assert!(decode(&base[..n], Sim).is_err());
    }
    let mut full = base.clone();
    full.extend([1, 2, 0, 0, 1, 2, 3, 99]);
    for n in [25, 27, 30, 32, 33] {
        assert!(decode(&full[..n], ClientReceived).is_ok(), "{n}");
    }
    for n in [26, 28, 29] {
        assert!(decode(&full[..n], Sim).is_err());
    }
    assert!(decode(&full[..31], Sim).is_ok());
    assert!(decode(&full[..31], ClientReceived).is_err());
    let Message::Setup(s) = decode(&full, ClientReceived).unwrap() else {
        panic!()
    };
    assert_eq!(
        (s.game_mode, s.team, s.extensions),
        (Some(2), Some(3), &[99][..])
    );
    for (at, len) in [(8, 2), (27, 3)] {
        let mut p = full.clone();
        p[at..at + len].fill(0);
        assert!(decode(&p, Sim).is_ok());
        assert!(decode(&p, ClientReceived).is_err());
    }
    full[23] = 13;
    assert_eq!(setup_protocol_version(&full, Sim).unwrap(), 13);
    assert_eq!(
        decode(&full, Sim).unwrap_err().kind,
        ErrorKind::UnsupportedVersion
    );
}
#[test]
fn appearance_retains_metadata_and_lazy_body_wire_values() {
    let p = common::appearance();
    let Message::SnakeAppearance(a) = decode(&p, ClientPresentation).unwrap() else {
        panic!()
    };
    assert_eq!(
        (a.id, a.heading, a.wanted_heading, a.speed, a.fraction),
        (17, 1, 16_777_215, 321, 8_388_607)
    );
    assert_eq!((a.raw_direction, a.skin, a.cosmetic), (b'2', 55, 7));
    assert_eq!((a.name, a.custom_skin), (&[0, 255][..], &[123, 234][..]));
    assert_eq!(a.head, [5001, 5002]);
    assert_eq!(a.body.first, Some([4999, 5000]));
    let expected = [
        BodyDelta::HalfUnits([0, 255]),
        BodyDelta::FinalDirection(0x1234),
    ];
    for _ in 0..2 {
        assert_eq!(a.body.deltas().collect::<Vec<_>>(), expected);
    }
    let mut p = p;
    p[6] = 255;
    assert!(decode(&p, Sim).is_ok());
    assert!(decode(&p, ClientReceived).is_ok());
    assert!(decode(&p, ClientPresentation).is_err());
    p.pop();
    assert!(decode(&p, Sim).is_err());
}
#[test]
fn removal_motion_and_controls_keep_admission_differences() {
    for len in 3..=7 {
        let mut p = vec![b's', 0, 4];
        p.resize(len, 1);
        assert!(decode(&p, Sim).is_ok());
        assert_eq!(decode(&p, ClientReceived).is_ok(), len <= 4);
    }
    for code in *b"d7" {
        assert!(decode(&[code, 1, 2, 3], ClientReceived).is_ok());
        assert!(decode(&[code, 1, 2, 3, 4, 5], Sim).is_ok());
        assert!(decode(&[code, 1, 2, 3, 4, 5], ClientReceived).is_err());
    }
    for p in [&[b'5', 4][..], &[b'5', 0, 4, 1]] {
        assert!(decode(p, Sim).is_err());
        let Message::Motion(m) = decode(p, ClientReceived).unwrap() else {
            panic!()
        };
        assert_eq!((m.heading, m.wanted_heading, m.speed), (None, None, None));
    }
    for p in [&b"p\0"[..], &b"v"[..], &b"v\x02extra"[..]] {
        assert!(decode(p, Sim).is_ok());
        assert!(decode(p, ClientReceived).is_err());
    }
    for status in [0, 1, 2, 255] {
        assert!(
            matches!(decode(&[b'v', status], ClientReceived).unwrap(), Message::LifeControl { status: Some(s), .. } if s == status)
        );
    }
    assert_eq!(
        decode(b"?raw", Sim).unwrap(),
        Message::Unknown {
            opcode: b'?',
            command: b"?raw"
        }
    );
    // Generic framing never starts validating gameplay bodies for disabled lanes.
    assert!(
        decode_frame(
            b"h",
            Header::Absent,
            Limits::CLIENT,
            FrameAdmission::Bounded
        )
        .is_ok()
    );
}
#[test]
fn canonical_motion_anchor_and_prey_shapes() {
    for admission in [Sim, ClientReceived, ClientPresentation] {
        for code in *b"eE345" {
            for n in 1..=3 {
                let mut p = vec![code, 0, 9];
                p.resize(3 + n, 1);
                assert_eq!(
                    decode(&p, admission).is_ok(),
                    !(admission == Sim && code == b'5' && n == 1)
                );
            }
        }
        for (code, lengths) in [
            (b'g', &[3, 5][..]),
            (b'n', &[6, 8]),
            (b'G', &[1, 3]),
            (b'N', &[4, 6]),
            (b'=', &[7, 9]),
            (b'+', &[10, 12]),
        ] {
            for len in lengths {
                let mut p = vec![0; *len];
                p[0] = code;
                assert!(
                    matches!(decode(&p, admission), Ok(Message::Anchor(_))),
                    "{code} {len}"
                );
            }
        }
        for len in [7, 9, 10, 11, 12, 13, 14, 16] {
            let mut p = vec![0; len];
            p[0] = b'j';
            if [11, 13, 14, 16].contains(&len) {
                p[7] = b'1';
            }
            assert!(matches!(decode(&p, admission), Ok(Message::PreyMotion(_))));
        }
    }
    let mut p = [0; 20];
    p[0] = b'y';
    p[3] = 247;
    p[11] = b'2';
    let Message::PreyAppearance(v) = decode(&p, ClientReceived).unwrap() else {
        panic!()
    };
    assert_eq!((v.color, v.raw_direction), (247, b'2'));
    p[11] = 0;
    assert!(decode(&p, Sim).is_ok());
    assert!(decode(&p, ClientReceived).is_err());
    assert_eq!(
        decode(b"y\0\x01\0\x02", ClientReceived).unwrap(),
        Message::PreyRemoval {
            id: 1,
            eater: Some(2)
        }
    );
}
#[test]
fn food_preserves_cache_markers_color_and_eater() {
    let Message::FoodSector(f) =
        decode(b"F\x01\x02\xff\x03\x04\x05\x08\x09\x0a\x0b", ClientReceived).unwrap()
    else {
        panic!()
    };
    assert_eq!(f.sector, [1, 2]);
    assert_eq!(
        f.foods().collect::<Vec<_>>(),
        [
            FoodTuple {
                color: 255,
                offset: [3, 4],
                size: 5
            },
            FoodTuple {
                color: 8,
                offset: [9, 10],
                size: 11
            }
        ]
    );
    for n in 4..=7 {
        let mut p = vec![b'b', 1, 2, 3, 4, 5, 6];
        p.truncate(n);
        let Message::FoodSpawn(f) = decode(&p, ClientReceived).unwrap() else {
            panic!()
        };
        assert_eq!(f.sector.is_some(), n >= 6);
        assert_eq!(f.color.is_some(), n == 5 || n == 7);
        assert!(f.rapid);
    }
    for (p, eater) in [
        (&b"c\x01\x02"[..], Eater::Absent),
        (&b"C\x01\x02"[..], Eater::Cached),
        (&b"<\x01\x02\x12\x34"[..], Eater::Id(0x1234)),
    ] {
        let Message::FoodEaten(f) = decode(p, ClientPresentation).unwrap() else {
            panic!()
        };
        assert_eq!(f.eater, eater);
        assert_eq!(f.sector, None);
    }
}
#[test]
fn hud_and_sim_only_families_keep_every_row_and_signed_value() {
    let p = b"l\xff\0\x02\0\x03\0\x04\xff\xff\xff\xfe\x02\0\xff";
    let l = hud::parse_leaderboard(p).unwrap();
    assert_eq!(
        (l.local_row(), l.rank(), l.population(), l.row_count()),
        (255, 2, 3, 1)
    );
    let row = l.rows().next().unwrap();
    assert_eq!(
        (row.count(), row.raw_fraction(), row.raw_color(), row.name()),
        (4, 16_777_215, 254, &[0, 255][..])
    );
    let a = hud::parse_announcement(b"m\0\0\x01\xff\xff\xff\x01\xffmessage").unwrap();
    assert_eq!((a.name(), a.message()), (&[255][..], &b"message"[..]));
    let Message::TeamScores(scores) =
        decode(&[b'o', 128, 0, 0, 0, 127, 255, 255, 255], Sim).unwrap()
    else {
        panic!()
    };
    assert_eq!(scores.scores().collect::<Vec<_>>(), [[i32::MIN, i32::MAX]]);
    assert!(!scores.is_history());
    assert_eq!(scalar::team_score([255; 4]), -1);
    assert_eq!(
        decode(b"S\x12\x34\x89\xab\xcd\xef", Sim).unwrap(),
        Message::Session {
            server_id: 0x1234,
            session_id: 0x89abcdef
        }
    );
    assert_eq!(
        decode(b"k\0\x02\xff\xff\xff", Sim).unwrap(),
        Message::Kills {
            id: 2,
            count: 16_777_215
        }
    );
    assert_eq!(
        decode(b"W\x01\x02", Sim).unwrap(),
        Message::SectorVisible([1, 2])
    );
}
#[test]
fn all_minimap_formats_reachability_orientation_and_admission() {
    use minimap::*;
    for p in [
        &b"U\0\x02\x40"[..],
        &b"M\0\x02\x40"[..],
        &b"L\x01\0\x02\x40"[..],
    ] {
        let m = parse_minimap(p, ClientReceived).unwrap();
        assert_eq!(
            m.selected(2).unwrap().map(|c| c.index).collect::<Vec<_>>(),
            [3]
        );
    }
    let m = parse_minimap(b"u\x40", Sim).unwrap();
    assert_eq!(m.selected(80).unwrap().next().unwrap().index, 0);
    let m = parse_minimap(b"L\x02\0\x01\x40\x40", Sim).unwrap();
    let cells = m.selected(1).unwrap();
    assert_eq!(cells.visited_layers(), 2);
    assert_eq!(cells.map(|c| c.layer).collect::<Vec<_>>(), [0, 1]);
    for code in *b"UML" {
        let p = if code == b'L' {
            vec![code, 1, 0, 0, 255]
        } else {
            vec![code, 0, 0, 255]
        };
        assert!(parse_minimap(&p, ClientReceived).is_ok());
        assert!(parse_minimap(&p, Sim).is_err());
    }
    assert!(parse_minimap(b"M\x02\x01", Sim).is_err());
    assert_eq!(
        parse_minimap(b"M\xff\xff", ClientReceived)
            .unwrap()
            .dimension(),
        Some(512)
    );
    assert_eq!(
        parse_minimap(b"U\xff\xff", Sim).unwrap().dimension(),
        Some(512)
    );
    assert!(parse_minimap(b"M\0\x02\xff", ClientReceived).is_err());
    assert!(parse_minimap(b"M\0\x01\x80\x81\xff", ClientReceived).is_ok()); // escape unreachable
    assert!(parse_minimap(b"M\0\x01\xff\0\x40", ClientReceived).is_ok()); // zero skip
    let v = parse_minimap(b"V\xff", ClientReceived).unwrap();
    assert!(v.selected(1).is_err());
    assert!(v.selected(0).is_ok());
    assert!(parse_minimap(b"L\0\0\x01\xff", Sim).is_ok());
}
#[test]
fn supported_families_reject_truncation_without_panicking() {
    let mut packets = vec![
        common::setup(),
        common::appearance(),
        vec![b'j'; 16],
        vec![b'y'; 20],
        vec![b'F'; 15],
        vec![b'l'; 15],
        vec![b'm'; 20],
        vec![b'~'; 24],
    ];
    packets.extend((0..=255).map(|op| vec![op; 34]));
    for p in packets {
        for end in 0..=p.len() {
            for a in [Sim, ClientReceived, ClientPresentation] {
                let _ = v19::decode(&p[..end], a);
            }
        }
    }
}

#[test]
fn extension_preserves_raw_bits_and_leaves_physics_range_to_adapter() {
    let mut p = [0; 24];
    p[0] = b'~';
    p[1] = 1;
    p[2] = 1;
    p[3] = 15;
    p[4..12].copy_from_slice(&1.25_f64.to_be_bytes());
    p[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
    p[16..24].copy_from_slice(&0.5_f64.to_be_bytes());
    let Message::Rules(r) = decode(&p, Sim).unwrap() else {
        panic!()
    };
    assert_eq!(
        (
            r.boundary,
            r.flags,
            r.pit_radius_bits,
            r.raw_min_score,
            r.kill_score_seconds_bits
        ),
        (1, 15, 1.25_f64.to_bits(), u32::MAX, 0.5_f64.to_bits())
    );
    for at in [1, 2, 3] {
        let mut invalid = p;
        invalid[at] = 255;
        assert!(decode(&invalid, Sim).is_err());
    }
    for value in [0., -1., f64::NAN, f64::INFINITY] {
        let mut invalid = p;
        invalid[4..12].copy_from_slice(&value.to_be_bytes());
        assert!(decode(&invalid, Sim).is_err());
    }
    p[3] = 0;
    assert!(decode(&p, Sim).is_err()); // absent fields cannot hide values
}

#[test]
fn exact_client_byte_limit_and_unbounded_hud_are_distinct() {
    let mut p = vec![b'?'; MAX_FRAME_BYTES];
    assert!(decode_frame(&p, Header::Absent, Limits::CLIENT, FrameAdmission::Bounded).is_ok());
    p.push(0);
    assert_eq!(
        decode_frame(&p, Header::Absent, Limits::CLIENT, FrameAdmission::Bounded)
            .unwrap_err()
            .kind,
        ErrorKind::TooLarge
    );
    let mut announcement = vec![0; MAX_FRAME_BYTES + 1];
    announcement[0] = b'm';
    assert!(hud::parse_announcement(&announcement).is_err());
    assert!(decode(&announcement, Sim).is_ok());
    assert!(decode(&announcement, ClientPresentation).is_err());
    let Message::TeamScores(history) = decode(
        &[
            b'o', 255, 255, 255, 255, 0, 0, 0, 1, 128, 0, 0, 0, 127, 255, 255, 255,
        ],
        Sim,
    )
    .unwrap() else {
        panic!()
    };
    assert!(history.is_history());
    assert_eq!(
        history.scores().collect::<Vec<_>>(),
        [[-1, 1], [i32::MIN, i32::MAX]]
    );
}
