use super::*;
use crate::client::{minimap::MinimapDecoder, protocol::split_frame, simulation::PlayerAppearance};

fn fixtures(version: u8) -> Vec<Vec<u8>> {
    let source = match version {
        13 => include_str!("../../../tests/fixtures/protocol-catalog-13.json"),
        19 => include_str!("../../../tests/fixtures/protocol-catalog-19.json"),
        _ => unreachable!(),
    };
    let fixture: serde_json::Value = serde_json::from_str(source).unwrap();
    assert_eq!(fixture["wireProtocol"], version);
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    fixture["records"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|record| {
            let mut bits = 0_u32;
            let mut count = 0;
            let mut frame = Vec::new();
            for byte in record["frameBase64"]
                .as_str()
                .unwrap()
                .bytes()
                .take_while(|&b| b != b'=')
            {
                bits = (bits << 6) | alphabet.iter().position(|&b| b == byte).unwrap() as u32;
                count += 6;
                if count >= 8 {
                    count -= 8;
                    frame.push((bits >> count) as u8);
                }
            }
            split_frame(&frame)
                .unwrap()
                .into_iter()
                .map(<[u8]>::to_vec)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn convert(encoder: &mut Encoder, p: Vec<u8>) -> Vec<u8> {
    encoder.packets(&[p]).unwrap().remove(0)
}

#[test]
fn captured_setup_and_common_families_are_byte_exact() {
    let mut encoder = Encoder::new(244);
    let mut codes = std::collections::BTreeSet::new();
    for p in fixtures(13) {
        match p[0] {
            b'a' => {
                let mut source = p.clone();
                source[23] = 19;
                assert_eq!(convert(&mut encoder, source), p);
            }
            b'e' | b'E' | b'3' | b'4' | b'5' | b'h' | b'r' | b'w' | b'y' | b'j' | b'l' | b'k'
            | b'p' | b'z' | b'v' | b'S' => {
                assert_eq!(convert(&mut encoder, p.clone()), p);
                codes.insert(p[0]);
            }
            b's' if p.len() <= 7 => assert_eq!(convert(&mut encoder, p.clone()), p),
            _ => {}
        }
    }
    for code in b"eE345hrwyjlkpzvS" {
        assert!(codes.contains(code));
    }
    // The local record and score-credit layouts are shared extensions, not
    // observed protocol-13 packets in #177.
    for p in [
        encode::credit(2),
        encode::record(12, 3, b"Local", b"Text").unwrap(),
    ] {
        assert_eq!(convert(&mut encoder, p.clone()), p);
    }
}

#[test]
fn captured_self_and_peer_motion_use_explicit_ids() {
    let mut encoder = Encoder::new(154);
    let mut seen = std::collections::BTreeSet::new();
    for p in fixtures(19) {
        if !b"eE345d7".contains(&p[0]) {
            continue;
        }
        let self_form = p.len() <= 3 || b"d7".contains(&p[0]);
        let mut expected = p.clone();
        if self_form {
            expected[0] = match p[0] {
                b'd' => b'e',
                b'7' => b'E',
                code => code,
            };
            expected.splice(1..1, 154_u16.to_be_bytes());
        }
        assert_eq!(convert(&mut encoder, p.clone()), expected);
        seen.insert(p[0]);
    }
    for code in b"eE345d7" {
        assert!(seen.contains(code));
    }
}

#[test]
fn captured_relative_absolute_and_growth_anchor_layouts() {
    let mut encoder = Encoder::new(244);
    let mut seen = std::collections::BTreeSet::new();
    for p in fixtures(13) {
        if !b"gGN".contains(&p[0]) {
            continue;
        }
        let id = u16_at(&p, 1);
        let source = if p[0] == b'g' {
            encoder.fronts.insert(
                id,
                Front {
                    source: Point::default(),
                    sent: None,
                },
            );
            encode::anchor_at(
                false,
                id,
                0,
                Point {
                    x: f64::from(u16_at(&p, 3)) - f64::from(encode::GRID_CENTER),
                    y: f64::from(u16_at(&p, 5)) - f64::from(encode::GRID_CENTER),
                },
                None,
            )
            .unwrap()
        } else {
            let delta = [i32::from(p[3]) - 128, i32::from(p[4]) - 128];
            let radians = f64::from(delta[1]).atan2(f64::from(delta[0]));
            let angle = encode::angle_u16(radians);
            let radians = f64::from(angle) * TAU / 65536.;
            // Synthetic source base chosen so this measured integer delta is
            // exactly representable by an angle-only 19 step after rounding.
            encoder.fronts.insert(
                id,
                Front {
                    source: Point {
                        x: 1000. + f64::from(delta[0]) - radians.cos() * 42.,
                        y: 1000. + f64::from(delta[1]) - radians.sin() * 42.,
                    },
                    sent: Some([1000, 1000]),
                },
            );
            let fam = (p[0] == b'N').then(|| u24_at(&p, 5) as u32);
            encode::anchor(false, id, angle, fam)
        };
        assert_eq!(convert(&mut encoder, source), p);
        seen.insert(p[0]);
    }
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), b"GNg");
    // Absolute growth n is documented but unobserved in the 13 catalog.
    let p = encode::anchor_at(true, 244, 0, Point { x: 0., y: 10. }, Some(123)).unwrap();
    assert_eq!(
        convert(&mut encoder, p),
        [b'n', 0, 244, 127, 38, 127, 48, 0, 0, 123]
    );
}

#[test]
fn captured_appearances_keep_headers_and_use_half_unit_pairs() {
    let mut count = 0;
    for p in fixtures(19) {
        if p[0] != b's' || p.len() <= 7 {
            continue;
        }
        let mut encoder = Encoder::new(154);
        let q = convert(&mut encoder, p.clone());
        assert_eq!(&q[..q.len() - 2], &p[..p.len() - 2]);
        let radians = f64::from(u16_at(&p, p.len() - 2)) * TAU / 65536.;
        assert!(((f64::from(q[q.len() - 2]) - 127.) / 2. - 42. * radians.cos()).abs() <= 0.25);
        assert!(((f64::from(q[q.len() - 1]) - 127.) / 2. - 42. * radians.sin()).abs() <= 0.25);
        count += 1;
    }
    assert!(count >= 2);
    for p in fixtures(13) {
        if p[0] != b's' || p.len() <= 7 {
            continue;
        }
        // Inverse of a measured half-unit pair, within its <=0.25-unit
        // quantization. Name/custom/cosmetic/header bytes must remain exact.
        let mut source = p.clone();
        let n = p.len();
        let angle =
            encode::angle_u16((f64::from(p[n - 1]) - 127.).atan2(f64::from(p[n - 2]) - 127.));
        source[n - 2..].copy_from_slice(&angle.to_be_bytes());
        let q = convert(&mut Encoder::new(244), source);
        assert_eq!(&q[..n - 2], &p[..n - 2]);
        for i in n - 2..n {
            assert!((i32::from(q[i]) - i32::from(p[i])).abs() <= 1);
        }
    }
}

#[test]
fn first_anchor_is_absolute_then_relative_steps_do_not_drift() {
    let backend = crate::client::SimulatorBackend::new(
        7,
        crate::client::SimulationConfig {
            agents: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let snake = &backend.world().snakes()[0];
    let appearance = encode::appearance(9, snake, 0., &PlayerAppearance::default()).unwrap();
    let mut encoder = Encoder::new(9);
    convert(&mut encoder, appearance);
    for i in 0..2000 {
        let angle = ((i * 997) % 65536) as u16;
        let p = encode::anchor(true, 9, angle, (i % 7 == 0).then_some(123));
        let q = convert(&mut encoder, p);
        assert_eq!(u16_at(&q, 1), 9);
        assert_eq!(
            q[0],
            if i == 0 {
                b'n'
            } else if i % 7 == 0 {
                b'N'
            } else {
                b'G'
            }
        );
        let front = &encoder.fronts[&9];
        assert_eq!(
            front.sent.unwrap(),
            [
                coordinate(front.source.x).unwrap(),
                coordinate(front.source.y).unwrap()
            ]
        );
    }
    convert(&mut encoder, encode::removal(9, true));
    assert!(!encoder.fronts.contains_key(&9));
    assert!(
        encoder
            .packets(&[encode::anchor(true, 9, 0, None)])
            .is_err()
    );
}

#[test]
fn captured_food_layouts_project_consistently_and_split_dense_listings() {
    let mut encoder = Encoder::new(244);
    let key_at = |x, y| {
        encode::food_key(Point {
            x: x - f64::from(encode::GRID_CENTER),
            y: y - f64::from(encode::GRID_CENTER),
        })
        .unwrap()
    };
    let mut seen = std::collections::BTreeSet::new();
    for p in fixtures(13) {
        let (key, source, coordinates) = match p[0] {
            b'F' | b'f' | b'b' => {
                let key = key_at(f64::from(u16_at(&p, 2)), f64::from(u16_at(&p, 4)));
                let style = encode::FoodStyle {
                    color: p[1],
                    size: p[6],
                    snake_food: p[0] == b'b',
                };
                let source = if p[0] == b'F' {
                    encode::food_sector([key[0], key[1]], &[([key[2], key[3]], style)]).remove(0)
                } else {
                    encode::food_add(key, style)
                };
                (key, source, 2)
            }
            b'c' => {
                let key = key_at(f64::from(u16_at(&p, 1)), f64::from(u16_at(&p, 3)));
                (key, encode::food_remove(key, Some(u16_at(&p, 5))), 1)
            }
            _ => continue,
        };
        let q = convert(&mut encoder, source);
        assert_eq!(q.len(), p.len());
        assert_eq!(&q[..coordinates], &p[..coordinates]);
        assert_eq!(&q[coordinates + 4..], &p[coordinates + 4..]);
        for at in [coordinates, coordinates + 2] {
            assert!((i32::from(u16_at(&q, at)) - i32::from(u16_at(&p, at))).abs() <= 1);
        }
        // The source 19 grid has a 1.171875-unit step; inverse projection of
        // official integer coordinates need not return the same integer.
        let removed = convert(&mut encoder, encode::food_remove(key, None));
        assert_eq!(&removed[1..], &q[coordinates..coordinates + 4]);
        seen.insert(p[0]);
    }
    for c in b"Ffbc" {
        assert!(seen.contains(c));
    }
    let foods = vec![
        (
            [255, 255],
            encode::FoodStyle {
                color: 7,
                size: 50,
                snake_food: false
            }
        );
        5000
    ];
    let source = encode::food_sector([108, 109], &foods);
    let out = encoder.packets(&source).unwrap();
    assert!(
        out.iter()
            .all(|p| encode::frameable(p) && (p.len() - 1) % 6 == 0)
    );
    assert_eq!(out.iter().map(|p| (p.len() - 1) / 6).sum::<usize>(), 5000);
    for sector in [0, 108, 217] {
        let xs: Vec<_> = (0..=255)
            .map(|offset| food_xy([sector, 0, offset, 0])[0])
            .collect();
        assert!(xs.windows(2).all(|pair| pair[0] < pair[1]));
    }
}

#[test]
fn official_minimaps_and_deltas_keep_the_same_raster_in_u() {
    for version in [13, 19] {
        let mut encoder = Encoder::new(0);
        let mut decoder = MinimapDecoder::default();
        let mut source = None;
        let mut count = 0;
        for p in fixtures(version) {
            if !b"MV".contains(&p[0]) {
                continue;
            }
            let expected = Minimap::packet(source.as_ref(), &p).unwrap();
            let q = convert(&mut encoder, p);
            assert_eq!(q[0], b'U');
            let mut budget = usize::MAX;
            decoder.apply(&q, 0., &mut budget).unwrap();
            assert_eq!(decoder.shown.as_ref().unwrap().0, expected);
            source = Some(expected);
            count += 1;
        }
        assert!(count >= 1);
    }
    for side in [1, 136, 512] {
        let mut map = Minimap {
            side,
            cells: vec![0; side * side],
            age_seconds: 0.,
        };
        map.cells[0] = 1;
        let p = minimap(&map);
        let mut decoder = MinimapDecoder::default();
        let mut budget = usize::MAX;
        decoder.apply(&p, 0., &mut budget).unwrap();
        assert_eq!(decoder.shown.unwrap().0, map);
    }
}
