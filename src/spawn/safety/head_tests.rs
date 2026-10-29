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
            .map(|raw| Obstacle {
                valid: raw.valid,
                edges: raw
                    .vertices
                    .windows(2)
                    .map(|p| Edge::new(p[0], p[1]))
                    .collect(),
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
            })
            .collect(),
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
