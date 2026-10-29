//! Independent scalar oracle frozen from d65d89ca. Do not use Safety, Bounds,
//! Point distance helpers or the optimized comparison inside the reference.
use super::*;

struct RawObstacle {
    valid: bool,
    vertices: Vec<Point>,
    bounds: [f64; 4],
    width: f64,
}

fn original_segment(p: Point, a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let denominator = dx * dx + dy * dy;
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy)
        / if denominator == 0. { 1. } else { denominator })
    .clamp(0., 1.);
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

fn original_swept(a: Point, b: Point, c: Point, d: Point) -> f64 {
    fn cross(p: Point, q: Point, r: Point) -> f64 {
        (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)
    }
    if cross(a, b, c) * cross(a, b, d) < 0. && cross(c, d, a) * cross(c, d, b) < 0. {
        return 0.;
    }
    original_segment(a, c, d)
        .min(original_segment(b, c, d))
        .min(original_segment(c, a, b))
        .min(original_segment(d, a, b))
}

fn original_near(p: Point, bounds: [f64; 4], width: f64) -> bool {
    if !bounds.into_iter().all(f64::is_finite) {
        return true;
    }
    let magnitude = [
        p.x, p.y, p.x, p.y, bounds[0], bounds[1], bounds[2], bounds[3], width,
    ]
    .into_iter()
    .map(f64::abs)
    .fold(1., f64::max);
    let r = width + magnitude * 1e-9;
    !(p.x < bounds[0] - r || p.x > bounds[2] + r || p.y < bounds[1] - r || p.y > bounds[3] + r)
}

fn original_head(head: Point, obstacles: &[RawObstacle]) -> bool {
    if !head.x.is_finite() || !head.y.is_finite() {
        return false;
    }
    for obstacle in obstacles {
        if !obstacle.valid {
            return false;
        }
        if original_near(head, obstacle.bounds, obstacle.width) {
            for pair in obstacle.vertices.windows(2) {
                if original_segment(head, pair[0], pair[1]) <= obstacle.width {
                    return false;
                }
            }
        }
    }
    true
}

// The candidate adapter alone constructs Safety. The reference above reads only
// the independent raw geometry and the frozen scalar arithmetic.
fn candidate(obstacles: &[RawObstacle]) -> Safety {
    Safety {
        obstacles: obstacles
            .iter()
            .map(|raw| {
                let edges: Vec<_> = raw
                    .vertices
                    .windows(2)
                    .map(|p| Edge::new(p[0], p[1]))
                    .collect();
                Obstacle {
                    valid: raw.valid,
                    edges,
                    bounds: Bounds {
                        min: Point {
                            x: raw.bounds[0],
                            y: raw.bounds[1],
                        },
                        max: Point {
                            x: raw.bounds[2],
                            y: raw.bounds[3],
                        },
                    },
                    head_width: raw.width,
                    body_width: 0.,
                    path: Edge::new(Point::default(), Point::default()),
                    inward: None,
                    inward_width: 0.,
                    path_width: 0.,
                    departure_width: 0.,
                    departure_path_width: 0.,
                }
            })
            .collect(),
        head_blocks: OnceCell::new(),
        candidate: Vec::new(),
        wall_limit: 1e10,
        segments: 26,
        departure_distance: 0.,
        inward_only: false,
    }
}

fn obstacle(a: Point, b: Point, width: f64) -> RawObstacle {
    RawObstacle {
        valid: true,
        vertices: vec![a, b],
        width,
        bounds: [a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)],
    }
}

fn assert_head(head: Point, raw: &[RawObstacle]) {
    assert_eq!(
        candidate(raw).head_is_clear(head),
        original_head(head, raw),
        "head={head:?}"
    );
}

#[test]
fn exhaustive_head_verdicts_match_independent_linear_scalar_oracle() {
    let points: Vec<_> = [-2., -1., -0., 0., 1., 2.]
        .into_iter()
        .flat_map(|x| [-2., -1., -0., 0., 1., 2.].map(|y| Point { x, y }))
        .collect();
    for &a in &points {
        for &b in &points {
            for width in [-1., -0., 0., 0.5, 1., 2., 4.] {
                let raw = [obstacle(a, b, width)];
                let query = candidate(&raw);
                for &head in &points {
                    assert_eq!(query.head_is_clear(head), original_head(head, &raw));
                }
            }
        }
    }
}

#[test]
fn head_thresholds_and_nonfinite_states_keep_original_comparisons() {
    let values = [
        0.,
        -0.,
        f64::MIN_POSITIVE,
        1e-200,
        1e-50,
        1.,
        400.,
        627.,
        1e50,
        1e200,
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ];
    for &x in &values {
        for &y in &values {
            let p = Point { x, y };
            for (a, b) in [
                (Point::default(), Point::default()),
                (Point { x: -1., y: 0. }, Point { x: 1., y: 0. }),
                (p, Point { x: y, y: x }),
            ] {
                let distance = original_segment(p, a, b);
                for width in
                    values
                        .into_iter()
                        .chain([distance.next_down(), distance, distance.next_up()])
                {
                    assert_eq!(segment_distance_le(p, a, b, width), distance <= width);
                    // Check both original swept comparison directions without
                    // treating `!<=` as `>` when NaN is possible.
                    let swept = original_swept(p, p, a, b);
                    let actual = swept_distance(p, p, a, b);
                    assert_eq!(actual <= width, swept <= width);
                    assert_eq!(actual > width, swept > width);
                    let mut raw = [obstacle(a, b, width)];
                    assert_head(p, &raw);
                    raw[0].valid = false;
                    assert_head(p, &raw);
                    raw[0].valid = true;
                    raw[0].bounds = [f64::NAN, f64::NEG_INFINITY, f64::INFINITY, f64::NAN];
                    assert_head(p, &raw);
                }
            }
        }
    }
    for width in [200_f64, 400., 627., 1e-49, 1e49] {
        for sign in [-1., 1.] {
            for distance in [width.next_down(), width, width.next_up()] {
                assert_head(
                    Point {
                        x: sign * distance,
                        y: 0.,
                    },
                    &[obstacle(Point::default(), Point::default(), width)],
                );
            }
        }
    }
}

#[test]
fn random_edges_and_multi_obstacle_heads_match_original_scalar_bits() {
    let mut state = 0x8c71_1a96_257d_3efbu64;
    let mut bits = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for i in 0..100_000 {
        let mut number = || {
            if i % 3 == 0 {
                f64::from_bits(bits())
            } else {
                ((bits() >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 65536.
            }
        };
        let p = Point {
            x: number(),
            y: number(),
        };
        let a = Point {
            x: number(),
            y: number(),
        };
        let b = if i % 5 == 0 {
            a
        } else {
            Point {
                x: number(),
                y: number(),
            }
        };
        let d = original_segment(p, a, b);
        for width in [d.next_down(), d, d.next_up(), number().abs()] {
            assert_eq!(segment_distance_le(p, a, b, width), d <= width, "case={i}");
            let mut raw = [obstacle(a, b, width), obstacle(b, a, 400.)];
            assert_head(p, &raw);
            raw.swap(0, 1);
            assert_head(p, &raw);
        }
    }
    assert_head(Point::default(), &[]);
    assert_head(Point { x: f64::NAN, y: 0. }, &[]);
}

#[test]
fn head_blocks_keep_thresholds_partial_blocks_and_nonfinite_edges() {
    let origin = Point::default();
    for edges in [0, 1, 7, 8, 9, 15, 16, 17, 24, 25, 65] {
        for at in 0..=edges {
            for width in [0_f64, 200., 400., HEAD_BLOCK_LIMIT] {
                for distance in [width.next_down(), width, width.next_up()] {
                    let mut raw = RawObstacle {
                        valid: true,
                        vertices: vec![Point { x: 1e6, y: 0. }; edges + 1],
                        bounds: [-f64::INFINITY, -f64::INFINITY, f64::INFINITY, f64::INFINITY],
                        width,
                    };
                    raw.vertices[at] = Point { x: distance, y: 0. };
                    assert_head(origin, std::slice::from_ref(&raw));
                    for exceptional in [
                        f64::NAN,
                        f64::INFINITY,
                        f64::NEG_INFINITY,
                        HEAD_BLOCK_LIMIT.next_up(),
                        f64::MAX,
                    ] {
                        let old = raw.vertices[0];
                        raw.vertices[0].y = exceptional;
                        assert_head(origin, std::slice::from_ref(&raw));
                        raw.vertices[0] = old;
                    }
                    raw.valid = false;
                    assert_head(origin, &[raw]);
                }
            }
        }
    }
}

#[test]
fn randomized_multiblock_worlds_match_independent_linear_oracle() {
    let mut state = 0xdfe2_94a1_2c75_b863u64;
    let mut number = |bits_only: bool| {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        if bits_only {
            f64::from_bits(state)
        } else {
            ((state >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 65536.
        }
    };
    for case in 0..25_000 {
        let bits_only = case % 5 == 0;
        let head = Point {
            x: number(bits_only),
            y: number(bits_only),
        };
        let mut world: Vec<_> = (0..1 + case % 3)
            .map(|index| {
                let mut vertices: Vec<_> = (0..10 + (case + index) % 25)
                    .map(|_| Point {
                        x: number(bits_only),
                        y: number(bits_only),
                    })
                    .collect();
                if case % 7 == 0 {
                    vertices[8] = vertices[7];
                }
                let distance = original_segment(head, vertices[7], vertices[8]);
                RawObstacle {
                    valid: true,
                    vertices,
                    bounds: [-f64::INFINITY, -f64::INFINITY, f64::INFINITY, f64::INFINITY],
                    width: match case % 4 {
                        0 => distance.next_down(),
                        1 => distance,
                        2 => distance.next_up(),
                        _ => number(bits_only).abs(),
                    },
                }
            })
            .collect();
        assert_head(head, &world);
        world.reverse();
        assert_head(head, &world);
        for raw in &world {
            let query = candidate(std::slice::from_ref(raw));
            assert_eq!(
                query.blocks_head(0, head),
                raw.vertices
                    .windows(2)
                    .any(|p| original_segment(head, p[0], p[1]) <= raw.width)
            );
        }
    }
}

#[test]
fn block_guard_domain_edges_and_cancellation_keep_scalar_verdicts() {
    for scale in [
        1e-200,
        1e-50,
        1.,
        HEAD_BLOCK_LIMIT.next_down(),
        HEAD_BLOCK_LIMIT,
        HEAD_BLOCK_LIMIT.next_up(),
        1e50,
        f64::MAX,
    ] {
        for sign in [-1., 1.] {
            let head = Point {
                x: sign * scale,
                y: -sign * scale,
            };
            let mut vertices: Vec<_> = (0..34)
                .map(|i| {
                    if i % 2 == 0 {
                        Point {
                            x: -scale,
                            y: scale,
                        }
                    } else {
                        Point {
                            x: scale,
                            y: -scale,
                        }
                    }
                })
                .collect();
            vertices[16] = Point {
                x: head.x + 400.,
                y: head.y,
            };
            for width in [
                -0.,
                0.,
                400_f64.next_down(),
                400.,
                400_f64.next_up(),
                HEAD_BLOCK_LIMIT.next_down(),
                HEAD_BLOCK_LIMIT,
                HEAD_BLOCK_LIMIT.next_up(),
                f64::NAN,
                f64::INFINITY,
            ] {
                let raw = RawObstacle {
                    valid: true,
                    vertices: vertices.clone(),
                    width,
                    bounds: [-f64::INFINITY, -f64::INFINITY, f64::INFINITY, f64::INFINITY],
                };
                assert_head(head, std::slice::from_ref(&raw));
                let query = candidate(std::slice::from_ref(&raw));
                assert_eq!(
                    query.blocks_head(0, head),
                    raw.vertices
                        .windows(2)
                        .any(|p| original_segment(head, p[0], p[1]) <= width)
                );
            }
        }
    }
}

#[test]
fn lazy_and_eager_blocks_have_identical_verdicts_and_leave_far_obstacles_unbuilt() {
    let raw = [RawObstacle {
        valid: true,
        vertices: (0..34)
            .map(|i| Point {
                x: i as f64 * 40.,
                y: if i % 3 == 0 { 200. } else { 0. },
            })
            .collect(),
        bounds: [0., 0., 1320., 200.],
        width: 200.,
    }];
    let lazy = candidate(&raw);
    let eager = candidate(&raw);
    let cells = eager
        .obstacles
        .iter()
        .map(|obstacle| {
            let cell = OnceCell::new();
            assert!(cell.set(head_blocks(&obstacle.edges)).is_ok());
            cell
        })
        .collect();
    assert!(eager.head_blocks.set(cells).is_ok());
    assert!(lazy.head_blocks.get().is_none());
    let distant = Point { x: -1e6, y: -1e6 };
    assert_eq!(lazy.head_is_clear(distant), eager.head_is_clear(distant));
    assert!(lazy.head_blocks.get().is_none());
    // Unsupported head/width inputs retain the linear path without allocating.
    assert_eq!(
        lazy.blocks_head(0, Point { x: f64::MAX, y: 0. }),
        eager.blocks_head(0, Point { x: f64::MAX, y: 0. })
    );
    assert!(lazy.head_blocks.get().is_none());
    for x in [
        -200_f64,
        -200_f64.next_down(),
        -200_f64.next_up(),
        0.,
        280.,
        640.,
        1320.,
        1520.,
    ] {
        for y in [-200., 0., 200., 400.] {
            let head = Point { x, y };
            let expected = original_head(head, &raw);
            assert_eq!(lazy.head_is_clear(head), expected);
            assert_eq!(eager.head_is_clear(head), expected);
        }
    }
    let cached = lazy.head_blocks.get().unwrap()[0].get().unwrap();
    assert!(!cached.is_empty());
    let address = cached.as_ptr();
    for _ in 0..3 {
        assert!(!lazy.head_is_clear(Point::default()));
    }
    assert_eq!(
        address,
        lazy.head_blocks.get().unwrap()[0].get().unwrap().as_ptr()
    );
}
