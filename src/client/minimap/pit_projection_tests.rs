use super::*;

fn original(map: &mut Minimap, radius: f64) {
    let side = map.side;
    let center = side as f64 / 2.;
    for y in 0..side {
        for x in 0..side {
            let near = |v: usize| (v as f64 - center).max(0.).max(center - (v + 1) as f64);
            let far = |v: usize| {
                (v as f64 - center)
                    .abs()
                    .max(((v + 1) as f64 - center).abs())
            };
            if near(x).hypot(near(y)) > radius {
                map.cells[y * side + x] = 0;
            } else if far(x).hypot(far(y)) >= radius {
                map.cells[y * side + x] = 1;
            }
        }
    }
}

fn check(cache: &mut PitProjectionCache, side: usize, radius: f64) {
    let mut expected = Minimap {
        side,
        cells: (0..side * side)
            .map(|i| ((i ^ (i / side)) & 1) as u8)
            .collect(),
        age_seconds: 1.,
    };
    let mut actual = expected.clone();
    original(&mut expected, radius);
    cache.apply(&mut actual, radius);
    assert_eq!(actual, expected, "side={side}, radius={radius:?}");
}

#[test]
fn projection_matches_original_at_thresholds_and_adjacent_floats() {
    let mut cache = PitProjectionCache::default();
    // Odd/even sizes, every distinct cell threshold, both inequality ties,
    // invalid floating radii and a changed side after every previous cache.
    for side in [1, 2, 3, 8, 17, 32] {
        let center = side as f64 / 2.;
        let mut thresholds = Vec::new();
        for x in 0..=side {
            for y in 0..=side {
                thresholds.push((x as f64 - center).abs().hypot((y as f64 - center).abs()));
            }
        }
        thresholds.sort_by(f64::total_cmp);
        thresholds.dedup();
        for radius in thresholds
            .into_iter()
            .flat_map(|r| [r.next_down(), r, r.next_up()])
        {
            check(&mut cache, side, radius);
        }
        for radius in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1., 0., 1.] {
            check(&mut cache, side, radius);
        }
    }
}

#[test]
fn projection_reuses_exact_interval_and_invalidates_on_shrink() {
    let mut cache = PitProjectionCache::default();
    let radius = 93.7_f64;
    check(&mut cache, 256, radius);
    let first = cache.0.as_ref().unwrap();
    let outside = first.outside.as_ptr();
    let edge = first.edge.as_ptr();
    for r in [radius.next_down(), radius, radius.next_up()] {
        assert!(cache.0.as_ref().unwrap().contains(256, r));
        check(&mut cache, 256, r);
        assert_eq!(cache.0.as_ref().unwrap().outside.as_ptr(), outside);
        assert_eq!(cache.0.as_ref().unwrap().edge.as_ptr(), edge);
    }
    for r in [93., 70., 1., 120., 180., 93.7] {
        check(&mut cache, 256, r);
    }
    check(&mut cache, 512, 180.);
    check(&mut cache, 136, 50.);
}
