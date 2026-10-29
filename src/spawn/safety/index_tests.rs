//! Frozen scalar loops over raw geometry, independent of Safety and its index.
use super::{
    head_tests::{original_segment, original_swept},
    *,
};

#[derive(Clone)]
struct Raw {
    valid: bool,
    vertices: Vec<Point>,
    path: [Point; 2],
    inward: Option<[Point; 2]>,
    widths: [f64; 6],
}

fn bounds(points: &[Point]) -> [f64; 4] {
    let p = points[0];
    points[1..].iter().fold([p.x, p.y, p.x, p.y], |a, p| {
        [a[0].min(p.x), a[1].min(p.y), a[2].max(p.x), a[3].max(p.y)]
    })
}

fn original_near(a: [f64; 4], b: [f64; 4], width: f64) -> bool {
    if !a.into_iter().chain(b).all(f64::is_finite) {
        return true;
    }
    let magnitude = a
        .into_iter()
        .chain(b)
        .chain([width])
        .map(f64::abs)
        .fold(1., f64::max);
    let r = width + magnitude * 1e-9;
    !(a[2] < b[0] - r || a[0] > b[2] + r || a[3] < b[1] - r || a[1] > b[3] + r)
}

fn head(raw: &[Raw], p: Point) -> bool {
    p.x.is_finite()
        && p.y.is_finite()
        && raw.iter().all(|o| {
            o.valid
                && (!original_near(bounds(&[p]), bounds(&o.vertices), o.widths[0])
                    || o.vertices.windows(2).all(|e| {
                        original_segment(p, e[0], e[1])
                            .partial_cmp(&o.widths[0])
                            .is_none_or(|v| v.is_gt())
                    }))
        })
}

fn body(raw: &[Raw], points: &[Point]) -> bool {
    if points.len() != 26
        || points
            .iter()
            .any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.hypot(p.y) >= 1e8)
    {
        return false;
    }
    let query = bounds(points);
    raw.iter().all(|o| {
        if !o.valid {
            return false;
        }
        if original_near(query, bounds(&o.path), o.widths[2])
            && points.windows(2).any(|p| {
                original_near(bounds(p), bounds(&o.path), o.widths[2])
                    && original_swept(p[0], p[1], o.path[0], o.path[1])
                        .partial_cmp(&o.widths[2])
                        .is_none_or(|v| !v.is_gt())
            })
        {
            return false;
        }
        if o.inward.is_some_and(|path| {
            original_near(query, bounds(&path), o.widths[3])
                && points.windows(2).any(|p| {
                    original_near(bounds(p), bounds(&path), o.widths[3])
                        && original_swept(p[0], p[1], path[0], path[1]) <= o.widths[3]
                })
        }) {
            return false;
        }
        !original_near(query, bounds(&o.vertices), o.widths[1])
            || points.windows(2).all(|p| {
                !original_near(bounds(p), bounds(&o.vertices), o.widths[1])
                    || o.vertices.windows(2).all(|e| {
                        !original_near(bounds(p), bounds(e), o.widths[1])
                            || original_swept(p[0], p[1], e[0], e[1]) > o.widths[1]
                    })
            })
    })
}

fn departure(raw: &[Raw], p: Point, heading: f64) -> bool {
    let radius = p.x.hypot(p.y);
    if !radius.is_finite() || radius <= 0. || !heading.is_finite() || !head(raw, p) {
        return false;
    }
    [
        Point {
            x: p.x * (1. - 600. / radius),
            y: p.y * (1. - 600. / radius),
        },
        Point {
            x: p.x + heading.cos() * 600.,
            y: p.y + heading.sin() * 600.,
        },
    ]
    .into_iter()
    .all(|end| {
        let query = bounds(&[p, end]);
        raw.iter().all(|o| {
            (!original_near(query, bounds(&o.path), o.widths[5])
                || original_swept(p, end, o.path[0], o.path[1]) > o.widths[5])
                && o.inward.is_none_or(|path| {
                    !original_near(query, bounds(&path), o.widths[3])
                        || original_swept(p, end, path[0], path[1]) > o.widths[3]
                })
                && (!original_near(query, bounds(&o.vertices), o.widths[4])
                    || o.vertices.windows(2).all(|e| {
                        !original_near(query, bounds(e), o.widths[4])
                            || original_swept(p, end, e[0], e[1]) > o.widths[4]
                    }))
        })
    })
}

fn candidate(raw: &[Raw], force_index: bool) -> Safety {
    let query = Safety {
        obstacles: raw
            .iter()
            .map(|o| {
                let b = bounds(&o.vertices);
                Obstacle {
                    valid: o.valid,
                    edges: o
                        .vertices
                        .windows(2)
                        .map(|e| Edge::new(e[0], e[1]))
                        .collect(),
                    bounds: Bounds {
                        min: Point { x: b[0], y: b[1] },
                        max: Point { x: b[2], y: b[3] },
                    },
                    head_width: o.widths[0],
                    body_width: o.widths[1],
                    path_width: o.widths[2],
                    inward_width: o.widths[3],
                    departure_width: o.widths[4],
                    departure_path_width: o.widths[5],
                    path: Edge::new(o.path[0], o.path[1]),
                    inward: o.inward.map(|p| Edge::new(p[0], p[1])),
                }
            })
            .collect(),
        head_blocks: OnceCell::new(),
        obstacle_index: OnceCell::new(),
        index_queries: Cell::new(0),
        candidate: Vec::new(),
        wall_limit: 1e8,
        segments: 26,
        departure_distance: 600.,
        inward_only: false,
    };
    if force_index {
        assert!(
            query
                .obstacle_index
                .set(ObstacleIndex::build(&query.obstacles))
                .is_ok()
        );
        query.index_queries.set(16);
    }
    query
}

fn obstacle(p: Point) -> Raw {
    Raw {
        valid: true,
        vertices: vec![
            p,
            p,
            Point {
                x: p.x + 300.,
                y: p.y - 80.,
            },
        ],
        path: [
            p,
            Point {
                x: p.x + 700.,
                y: p.y + 200.,
            },
        ],
        inward: Some([
            p,
            Point {
                x: p.x - 500.,
                y: p.y - 600.,
            },
        ]),
        widths: [600., 200., 300., 300., 450., 500.],
    }
}

fn check(raw: &[Raw], p: Point, heading: f64) {
    let points: Vec<_> = (0..26)
        .map(|i| Point {
            x: p.x + i as f64 * 7.,
            y: p.y + i as f64 * 3.,
        })
        .collect();
    let mut indexed = candidate(raw, true);
    let mut linear = candidate(raw, false);
    assert_filter_keeps_linear_candidates(&indexed, raw, &[p]);
    assert_filter_keeps_linear_candidates(&indexed, raw, &points);
    assert_eq!(indexed.head_is_clear(p), head(raw, p));
    assert_eq!(linear.head_is_clear(p), head(raw, p));
    assert_eq!(indexed.clears(&points), body(raw, &points));
    assert_eq!(linear.clears(&points), body(raw, &points));
    assert_eq!(
        indexed.clears_departures(p, heading),
        departure(raw, p, heading)
    );
    assert_eq!(
        linear.clears_departures(p, heading),
        departure(raw, p, heading)
    );
}

fn assert_filter_keeps_linear_candidates(query: &Safety, raw: &[Raw], points: &[Point]) {
    let b = bounds(points);
    if !b.into_iter().all(|v| v.is_finite() && v.abs() <= 1e7) {
        return;
    }
    let Some(index) = query.obstacle_index.get().unwrap() else {
        return;
    };
    let retained: Vec<_> = index
        .candidates(Bounds {
            min: Point { x: b[0], y: b[1] },
            max: Point { x: b[2], y: b[3] },
        })
        .collect();
    assert!(retained.windows(2).all(|v| v[0] < v[1]));
    for (slot, obstacle) in raw.iter().enumerate() {
        let body = bounds(&obstacle.vertices);
        let path = bounds(&obstacle.path);
        let must_visit = !obstacle.valid
            || [0, 1, 4]
                .into_iter()
                .any(|i| original_near(b, body, obstacle.widths[i]))
            || [2, 5]
                .into_iter()
                .any(|i| original_near(b, path, obstacle.widths[i]))
            || obstacle
                .inward
                .is_some_and(|p| original_near(b, bounds(&p), obstacle.widths[3]));
        assert!(
            !must_visit || retained.binary_search(&slot).is_ok(),
            "slot={slot}"
        );
    }
}

#[test]
fn obstacle_index_random_verdicts_match_raw_linear_oracle() {
    let mut state = 0x3462_784f_15ac_9023u64;
    let mut random = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        (state >> 11) as f64 / (1u64 << 53) as f64
    };
    for world in 0..48 {
        let raw: Vec<_> = (0..96)
            .map(|_| {
                let mut o = obstacle(Point {
                    x: random() * 20000. - 10000.,
                    y: random() * 20000. - 10000.,
                });
                o.vertices = (0..(2 + world % 24))
                    .map(|_| Point {
                        x: o.path[0].x + random() * 1000. - 500.,
                        y: o.path[0].y + random() * 1000. - 500.,
                    })
                    .collect();
                for width in &mut o.widths {
                    *width = random() * 900.;
                }
                o
            })
            .collect();
        for _ in 0..80 {
            check(
                &raw,
                Point {
                    x: random() * 24000. - 12000.,
                    y: random() * 24000. - 12000.,
                },
                random() * 6.3,
            );
        }
    }
}

#[test]
fn obstacle_index_cell_edges_thresholds_invalid_and_extreme_geometry() {
    let mut raw: Vec<_> = (0..96)
        .map(|i| {
            obstacle(Point {
                x: (i % 12) as f64 * 1024. - 6144.,
                y: (i / 12) as f64 * 1024. - 4096.,
            })
        })
        .collect();
    for base in [-8192.0_f64, -1024., -0., 0., 1024., 8192.] {
        for x in [
            base.next_down(),
            base,
            base.next_up(),
            base + 600.,
            (base + 600.).next_up(),
        ] {
            for y in [-4096., -0., 0., 1024., 4096.] {
                check(&raw, Point { x, y }, 0.3);
            }
        }
    }
    for value in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e200,
        -1e200,
        f64::MAX,
    ] {
        check(&raw, Point { x: value, y: 0. }, value);
        for slot in [0, 31, 63, 95] {
            let original = raw[slot].clone();
            raw[slot].valid = false;
            raw[slot].vertices[0].x = value;
            check(&raw, Point { x: 9000., y: 9000. }, 0.7);
            raw[slot] = original.clone();
            raw[slot].widths[0] = value;
            check(&raw, Point { x: 9000., y: 9000. }, 0.7);
            raw[slot] = original.clone();
            raw[slot].vertices = vec![Point { x: value, y: value }; 3];
            check(&raw, Point { x: 9000., y: 9000. }, 0.7);
            raw[slot] = original.clone();
            raw[slot].path[1].x = value;
            check(&raw, Point { x: 9000., y: 9000. }, 0.7);
            raw[slot] = original;
        }
    }
    let mut query = candidate(&raw, true);
    assert!(query.obstacle_index.get().unwrap().is_some());
    assert!(!query.head_is_clear(raw[0].vertices[0]));
    let points = vec![Point { x: 1e200, y: 0. }; 26];
    assert!(!query.clears(&points));
    assert!(!query.clears(&points[..25]));
    for width in [
        -1.0_f64,
        -0.,
        0.,
        600.0_f64.next_down(),
        600.,
        600.0_f64.next_up(),
        f64::NAN,
    ] {
        let original = raw[63].clone();
        raw[63].widths = [width; 6];
        check(
            &raw,
            Point {
                x: raw[63].vertices[0].x + 600.,
                y: raw[63].vertices[0].y,
            },
            0.7,
        );
        raw[63] = original;
    }
    let mut wide = raw.clone();
    wide[0].vertices = vec![Point { x: -1e7, y: -1e7 }; 3];
    wide[95].vertices = vec![Point { x: 1e7, y: 1e7 }; 3];
    assert!(
        candidate(&wide, true)
            .obstacle_index
            .get()
            .unwrap()
            .is_none()
    );
    check(&wide, Point { x: 9000., y: 9000. }, 0.7);
}

#[test]
fn obstacle_index_candidates_keep_original_order_and_lazy_verdicts() {
    let raw: Vec<_> = (0..96)
        .map(|i| {
            obstacle(Point {
                x: (i % 12) as f64 * 1024. - 6144.,
                y: (i / 12) as f64 * 1024. - 4096.,
            })
        })
        .collect();
    let mut indexed = candidate(&raw, true);
    let mut lazy = candidate(&raw, false);
    assert!(lazy.obstacle_index.get().is_none());
    for i in 0..128 {
        let p = Point {
            x: (i % 16) as f64 * 700. - 5600.,
            y: (i / 16) as f64 * 900. - 3600.,
        };
        let expected = head(&raw, p);
        assert_eq!(indexed.head_is_clear(p), expected);
        assert_eq!(lazy.head_is_clear(p), expected);
        let q = Bounds::between(
            p,
            Point {
                x: p.x + 1800.,
                y: p.y + 1300.,
            },
        );
        let candidates: Vec<_> = indexed
            .obstacle_index
            .get()
            .unwrap()
            .as_ref()
            .unwrap()
            .candidates(q)
            .collect();
        assert!(candidates.windows(2).all(|v| v[0] < v[1]));
        let body: Vec<_> = (0..26)
            .map(|j| Point {
                x: p.x + j as f64,
                y: p.y,
            })
            .collect();
        assert_eq!(indexed.clears(&body), lazy.clears(&body));
    }
    assert!(lazy.obstacle_index.get().is_some());
}
