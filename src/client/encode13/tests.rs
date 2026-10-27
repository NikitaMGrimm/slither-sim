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
    encoder
        .packets(&[p], &mut FullMapCache::default())
        .unwrap()
        .remove(0)
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
    // Decode the source appearance's front independently, including the 19
    // terminal angle. Neither reference position comes from adapter state.
    let read16 = |p: &[u8], at| f64::from(u16::from_be_bytes([p[at], p[at + 1]]));
    let read24 = |p: &[u8], at| {
        p[at..at + 3]
            .iter()
            .fold(0_u32, |value, &byte| value * 256 + u32::from(byte)) as f64
    };
    let custom = 23 + usize::from(appearance[22]);
    let body = custom + 2 + usize::from(appearance[custom]);
    let mut reference = [
        read24(&appearance, body) / 5.,
        read24(&appearance, body + 3) / 5.,
    ];
    for pair in appearance[body + 6..appearance.len() - 2]
        .as_chunks::<2>()
        .0
    {
        reference[0] += (f64::from(pair[0]) - 127.) / 2.;
        reference[1] += (f64::from(pair[1]) - 127.) / 2.;
    }
    let last_angle = read16(&appearance, appearance.len() - 2) * TAU / 65536.;
    reference[0] += 42. * last_angle.cos();
    reference[1] += 42. * last_angle.sin();
    let mut received = [0.; 2];
    let mut encoder = Encoder::new(9);
    convert(&mut encoder, appearance);
    for i in 0..2000 {
        let angle = ((i * 997) % 65536) as u16;
        let radians = f64::from(angle) * TAU / 65536.;
        reference[0] += 42. * radians.cos();
        reference[1] += 42. * radians.sin();
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
        match q[0] {
            b'g' | b'n' => received = [read16(&q, 3), read16(&q, 5)],
            b'G' | b'N' => {
                received[0] += f64::from(q[3]) - 128.;
                received[1] += f64::from(q[4]) - 128.;
            }
            _ => panic!("unexpected anchor"),
        }
        assert_eq!(received, reference.map(f64::round), "step {i}");
    }
    convert(&mut encoder, encode::removal(9, true));
    assert!(!encoder.fronts.contains_key(&9));
    assert!(
        encoder
            .packets(
                &[encode::anchor(true, 9, 0, None)],
                &mut FullMapCache::default()
            )
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
    let out = encoder
        .packets(&source, &mut FullMapCache::default())
        .unwrap();
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
fn official_minimaps_and_deltas_keep_the_shared_bytes() {
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
            let q = convert(&mut encoder, p.clone());
            assert_eq!(q, p);
            let mut budget = usize::MAX;
            decoder.apply(&q, 0., &mut budget).unwrap();
            assert_eq!(decoder.shown.as_ref().unwrap().0, expected);
            source = Some(expected);
            count += 1;
        }
        assert!(count >= 1);
    }
}

/// Independent reverse-cell reader from the documented client format, without
/// calling the library decoder or adapter state. Synthetic asymmetric cells.
fn client_cells(p: &[u8], side: usize, previous: &mut [u8]) {
    if p[0] != b'V' {
        previous.fill(0);
    }
    let (mut at, mut scanned) = (if p[0] == b'V' { 1 } else { 3 }, 0);
    while at < p.len() && scanned < side * side {
        let byte = p[at];
        at += 1;
        if byte >= 128 {
            scanned += if byte == 255 && p[0] != b'U' {
                let count = 126 * usize::from(p[at]);
                at += 1;
                count
            } else {
                usize::from(byte - 128)
            };
        } else {
            for bit in (0..7).rev() {
                if scanned == side * side {
                    break;
                }
                if byte & (1 << bit) != 0 {
                    let cell = side * side - 1 - scanned;
                    if p[0] == b'V' {
                        previous[cell] ^= 1;
                    } else {
                        previous[cell] = 1;
                    }
                }
                scanned += 1;
            }
        }
    }
}

#[test]
fn asymmetric_live_map_and_delta_align_with_the_dynamic_dot() {
    let side = 256;
    let mut map = Minimap {
        side,
        cells: vec![0; side * side],
        age_seconds: 0.,
    };
    // At radius 2400, world (1200,-1600) projects to (192,42+2/3).
    // The other points form an asymmetric L and a distant reference point.
    for (x, y) in [(192, 42), (192, 43), (193, 43), (17, 209)] {
        map.cells[y * side + x] = 1;
    }
    let mut encoder = Encoder::new(0);
    let full = encode::minimap(&map);
    let p = convert(&mut encoder, full.clone());
    assert_eq!(p, full);
    assert!(p.contains(&255), "exercise the extended-run path");
    let mut received = vec![0; side * side];
    client_cells(&p, side, &mut received);
    assert_eq!(received, map.cells);
    assert_eq!(received[42 * side + 192], 1);
    assert_eq!(
        received[(255 - 42) * side + 255 - 192],
        0,
        "no 180-degree rotation"
    );
    let mut next = map.clone();
    next.cells[42 * side + 192] = 0;
    next.cells[41 * side + 194] = 1;
    let delta = encode::minimap_delta(&map, &next).unwrap();
    let p = convert(&mut encoder, delta.clone());
    assert_eq!(p, delta);
    client_cells(&p, side, &mut received);
    assert_eq!(received, next.cells);
}

#[test]
fn default_live_map_is_unchanged_across_setup_and_radius_updates() {
    let map = Minimap {
        side: 8,
        cells: (0..64).map(|i| u8::from(i == 13)).collect(),
        age_seconds: 0.,
    };
    for radius in [1800., 1498., 32000.] {
        let mut encoder = Encoder::new(0);
        convert(&mut encoder, encode::setup(radius, 0.028).unwrap());
        let full = encode::minimap(&map);
        assert_eq!(convert(&mut encoder, full.clone()), full);
        for radius in [1641., 1596., 1544.] {
            let update = encode::radius(radius).unwrap();
            assert_eq!(convert(&mut encoder, update.clone()), update);
            let delta = encode::minimap_delta(&map, &map).unwrap();
            assert_eq!(convert(&mut encoder, delta.clone()), delta);
        }
    }
}

#[test]
fn non_dynamic_map_reprojects_live_cells_and_tracks_radius_and_delta_base() {
    let side = 8;
    let mut encoder = Encoder::new(0);
    encoder.use_full_minimap();
    convert(&mut encoder, encode::setup(16275., 0.028).unwrap());
    let mut map = Minimap {
        side,
        cells: vec![0; 64],
        age_seconds: 0.,
    };
    map.cells[side + 6] = 1;
    let p = convert(&mut encoder, encode::minimap(&map));
    assert_eq!(p[0], b'U');
    let mut received = vec![0; 64];
    client_cells(&p, side, &mut received);
    assert_eq!(received.iter().sum::<u8>(), 1);
    assert_eq!(
        received[2 * side + 5],
        1,
        "half-radius map centered in grd frame"
    );
    let mut next = map.clone();
    next.cells.fill(0);
    next.cells[6 * side + 1] = 1;
    let p = convert(&mut encoder, encode::minimap_delta(&map, &next).unwrap());
    client_cells(&p, side, &mut received);
    assert_eq!(received.iter().sum::<u8>(), 1);
    assert_eq!(received[5 * side + 2], 1);
    convert(&mut encoder, encode::radius(8138.).unwrap());
    let p = convert(&mut encoder, encode::minimap_delta(&next, &next).unwrap());
    client_cells(&p, side, &mut received);
    assert_eq!(received.iter().sum::<u8>(), 1);
    assert_eq!(
        received[4 * side + 3],
        1,
        "radius update changes the projection"
    );
    // A fresh adapter defaults to dynamic maps, with no prior full-map state.
    assert_eq!(
        convert(&mut Encoder::new(0), encode::minimap(&map))[0],
        b'M'
    );
}

fn full_map_reference(state: &mut FullMap, p: &[u8]) -> Result<Vec<u8>, String> {
    let source = Minimap::packet(state.previous.as_deref(), p)?;
    let output = state.project(&source);
    state.previous = Some(Arc::new(source));
    Ok(output)
}

#[test]
fn shared_full_maps_match_uncached_histories_and_projection_parameters() {
    let map = |bits: u8| Minimap {
        side: 2,
        cells: (0..4).map(|i| (bits >> i) & 1).collect(),
        age_seconds: 0.,
    };
    for old in 0..16 {
        for next in 0..16 {
            let before = map(old);
            let after = map(next);
            let packets = [
                encode::minimap(&before),
                encode::minimap_delta(&before, &after).unwrap(),
                encode::minimap_delta(&after, &after).unwrap(),
                encode::minimap(&after),
            ];
            let mut states: Vec<_> = [(32., 16.), (32., 16.), (32., 8.), (16., 16.), (32., 16.)]
                .into_iter()
                .map(|(grd, radius)| FullMap {
                    grd,
                    radius,
                    previous: None,
                })
                .collect();
            let mut controls = states.clone();
            let mut cache = FullMapCache::default();
            for (step, p) in packets.iter().enumerate() {
                cache.clear();
                if step == 1 {
                    let different = Minimap {
                        cells: before.cells.iter().map(|v| v ^ 1).collect(),
                        ..before.clone()
                    };
                    states[4].previous = Some(Arc::new(different.clone()));
                    controls[4].previous = Some(Arc::new(different));
                }
                if step == 2 {
                    // A skipped update and a separately allocated equal map
                    // must retain their own XOR baselines.
                    states[2].previous = Some(Arc::new(before.clone()));
                    controls[2].previous = Some(Arc::new(before.clone()));
                    states[3].previous = Some(Arc::new(after.clone()));
                    controls[3].previous = Some(Arc::new(after.clone()));
                    states[4].previous = Some(Arc::new(after.clone()));
                    controls[4].previous = Some(Arc::new(after.clone()));
                }
                for i in [0, 2, 4, 1, 3] {
                    assert_eq!(
                        states[i].packet(p, &mut cache),
                        full_map_reference(&mut controls[i], p)
                    );
                    assert_eq!(
                        states[i].previous.as_deref(),
                        controls[i].previous.as_deref()
                    );
                }
                assert_eq!(cache.cached_packets(), [3, 4, 4, 3][step]);
                assert!(Arc::ptr_eq(
                    states[0].previous.as_ref().unwrap(),
                    states[1].previous.as_ref().unwrap()
                ));
            }
        }
    }
}

#[test]
fn full_map_cache_distinguishes_input_bytes_and_preserves_failed_states() {
    let before = Minimap {
        side: 3,
        cells: vec![0; 9],
        age_seconds: 0.,
    };
    let mut a = before.clone();
    a.cells[1] = 1;
    let mut b = before.clone();
    b.cells[7] = 1;
    let source = Arc::new(before.clone());
    let state = FullMap {
        grd: 16.,
        radius: 16.,
        previous: Some(source.clone()),
    };
    let mut states = [state.clone(), state.clone(), state.clone()];
    let mut cache = FullMapCache::default();
    for (i, after) in [&a, &b, &a].into_iter().enumerate() {
        let p = encode::minimap_delta(&before, after).unwrap();
        let mut reference = state.clone();
        assert_eq!(
            states[i].packet(&p, &mut cache),
            full_map_reference(&mut reference, &p)
        );
    }
    assert_eq!(cache.cached_packets(), 2);
    assert!(Arc::ptr_eq(
        states[0].previous.as_ref().unwrap(),
        states[2].previous.as_ref().unwrap()
    ));
    assert!(!Arc::ptr_eq(
        states[0].previous.as_ref().unwrap(),
        states[1].previous.as_ref().unwrap()
    ));
    let mut fresh = FullMap {
        previous: None,
        ..state.clone()
    };
    for p in [vec![b'V'], vec![b'M', 0, 0], vec![b'M']] {
        let mut reference = fresh.clone();
        assert_eq!(
            fresh.packet(&p, &mut cache),
            full_map_reference(&mut reference, &p)
        );
        assert!(fresh.previous.is_none());
        assert_eq!(cache.cached_packets(), 2);
    }
    let full = encode::minimap(&a);
    assert_eq!(fresh.packet(&full, &mut cache).unwrap(), state.project(&a));
}

#[test]
fn full_map_cache_retains_baseline_identity_until_clear() {
    let map = Minimap {
        side: 1,
        cells: vec![1],
        age_seconds: 0.,
    };
    let previous = Arc::new(map.clone());
    let weak = Arc::downgrade(&previous);
    let mut state = FullMap {
        grd: 16.,
        radius: 8.,
        previous: Some(previous),
    };
    let mut cache = FullMapCache::default();
    state.packet(&encode::minimap(&map), &mut cache).unwrap();
    assert!(weak.upgrade().is_some());
    cache.clear();
    assert!(weak.upgrade().is_none());
    let delta = encode::minimap_delta(&map, &map).unwrap();
    let mut reference = state.clone();
    assert_eq!(
        state.packet(&delta, &mut cache),
        full_map_reference(&mut reference, &delta)
    );
}

#[test]
fn different_full_map_histories_converge_on_one_current_snapshot() {
    let a = Minimap {
        side: 3,
        cells: vec![0; 9],
        age_seconds: 0.,
    };
    let b = Minimap {
        side: 3,
        cells: vec![1; 9],
        age_seconds: 0.,
    };
    let current = Minimap {
        side: 3,
        cells: (0..9).map(|i| u8::from(i == 1 || i == 7)).collect(),
        age_seconds: 0.,
    };
    let mut states = [
        FullMap {
            grd: 16.,
            radius: 16.,
            previous: Some(Arc::new(a.clone())),
        },
        FullMap {
            grd: 16.,
            radius: 16.,
            previous: Some(Arc::new(b.clone())),
        },
        FullMap {
            grd: 16.,
            radius: 8.,
            previous: None,
        },
    ];
    let mut cache = FullMapCache::default();
    for (i, p) in [
        encode::minimap_delta(&a, &current).unwrap(),
        encode::minimap_delta(&b, &current).unwrap(),
        encode::minimap(&current),
    ]
    .iter()
    .enumerate()
    {
        let mut reference = states[i].clone();
        assert_eq!(
            states[i].packet(p, &mut cache),
            full_map_reference(&mut reference, p)
        );
    }
    assert_eq!(cache.cached_packets(), 3);
    assert_eq!(cache.snapshots.len(), 1);
    assert_eq!(cache.snapshots[0].projected.len(), 2);
    assert!(states.windows(2).all(|pair| Arc::ptr_eq(
        pair[0].previous.as_ref().unwrap(),
        pair[1].previous.as_ref().unwrap()
    )));
    cache.clear();
    let delta = encode::minimap_delta(&current, &a).unwrap();
    for state in &mut states[..2] {
        state.packet(&delta, &mut cache).unwrap();
    }
    assert_eq!(
        cache.cached_packets(),
        1,
        "equal histories share the next decode too"
    );
    assert_eq!(cache.snapshots.len(), 1);
    assert_eq!(cache.snapshots[0].projected.len(), 1);
}
