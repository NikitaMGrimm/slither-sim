//! Cached broad phase for one circular spawn search. Exact distances still own
//! admission; distant edge pairs only skip work. See the safe-spawn study.
use crate::{
    Point, Simulator,
    physics::{Bounds, base_speed, collision_distance, segment_distance, swept_distance},
};

struct Edge {
    a: Point,
    b: Point,
    bounds: Bounds,
}

impl Edge {
    fn new(a: Point, b: Point) -> Self {
        Self {
            a,
            b,
            bounds: Bounds::between(a, b),
        }
    }
}

struct Obstacle {
    valid: bool,
    edges: Vec<Edge>,
    bounds: Bounds,
    body_width: f64,
    path: Edge,
    path_width: f64,
    departure_width: f64,
    departure_path_width: f64,
    head_width: f64,
}

/// Immutable neighbors, with a reused candidate buffer. Built after excluding
/// the joining slot; no world tick runs during the search.
pub(crate) struct Safety {
    obstacles: Vec<Obstacle>,
    candidate: Vec<Edge>,
    wall_limit: f64,
    segments: usize,
    departure_distance: f64,
}

fn near(a: Bounds, b: Bounds, width: f64) -> bool {
    if !(a.min.finite() && a.max.finite() && b.min.finite() && b.max.finite()) {
        return true;
    }
    // Broad-phase rounding may only add exact tests, never reject a borderline
    // pose. This is deliberately wider than coordinate/projection roundoff.
    let magnitude = [
        a.min.x, a.min.y, a.max.x, a.max.y, b.min.x, b.min.y, b.max.x, b.max.y, width,
    ]
    .into_iter()
    .map(f64::abs)
    .fold(1.0, f64::max);
    a.near(b, width + magnitude * 1e-9)
}

impl Safety {
    pub(crate) fn new(world: &Simulator, segments: usize, clearance: f64, horizon: f64) -> Self {
        let obstacles = world
            .snakes
            .iter()
            .filter(|s| s.alive)
            .map(|s| {
                let mut edges = Vec::with_capacity(s.body.len() + 1);
                let mut bounds = Bounds::between(s.head, s.head);
                let mut previous = s.head;
                edges.push(Edge::new(s.head, s.head));
                for &point in &s.body {
                    edges.push(Edge::new(previous, point));
                    bounds.include(point);
                    previous = point;
                }
                let contact = collision_distance(s.segments, segments) + 40.;
                let path = Edge::new(
                    s.head,
                    Point {
                        x: s.head.x + s.heading.cos() * s.speed * horizon,
                        y: s.head.y + s.heading.sin() * s.speed * horizon,
                    },
                );
                Obstacle {
                    valid: path.a.finite() && path.b.finite() && s.body.iter().all(|p| p.finite()),
                    edges,
                    bounds,
                    body_width: contact.max(clearance),
                    path,
                    path_width: contact.max(super::CLEARANCE),
                    departure_width: contact.max(super::CIRCULAR_CLEARANCE),
                    departure_path_width: contact.max(super::DEPARTURE_PATH_CLEARANCE),
                    head_width: (super::DEPARTURE_SECONDS * base_speed(segments) + contact)
                        .max(clearance),
                }
            })
            .collect();
        Self {
            obstacles,
            candidate: Vec::with_capacity(segments.saturating_sub(1)),
            wall_limit: world.config.radius - crate::simulator::SPAWN_WALL_MARGIN,
            segments,
            departure_distance: super::DEPARTURE_SECONDS * base_speed(segments),
        }
    }

    pub(crate) fn clears(&mut self, body: &[Point]) -> bool {
        if body.len() != self.segments
            || !(2..=430).contains(&body.len())
            || body
                .iter()
                .any(|p| !p.finite() || p.distance(Point::default()) >= self.wall_limit)
        {
            return false;
        }
        self.candidate.clear();
        self.candidate
            .extend(body.windows(2).map(|e| Edge::new(e[0], e[1])));
        let mut bounds = Bounds::between(body[0], body[0]);
        for &point in &body[1..] {
            bounds.include(point);
        }
        self.obstacles.iter().all(|obstacle| {
            if !obstacle.valid {
                return false;
            }
            if near(bounds, obstacle.path.bounds, obstacle.path_width)
                && self.candidate.iter().any(|e| {
                    near(e.bounds, obstacle.path.bounds, obstacle.path_width)
                        && swept_distance(e.a, e.b, obstacle.path.a, obstacle.path.b)
                            .partial_cmp(&obstacle.path_width)
                            .is_none_or(|order| !order.is_gt())
                })
            {
                return false;
            }
            !near(bounds, obstacle.bounds, obstacle.body_width)
                || self.candidate.iter().all(|e| {
                    !near(e.bounds, obstacle.bounds, obstacle.body_width)
                        || obstacle.edges.iter().all(|other| {
                            !near(e.bounds, other.bounds, obstacle.body_width)
                                || swept_distance(e.a, e.b, other.a, other.b) > obstacle.body_width
                        })
                })
        })
    }

    /// Reserve short normal-speed departures toward the middle and along the
    /// tangent. Initial body clearance alone does not keep a newborn from
    /// turning into an older body as it leaves spawn. These are custom design
    /// corridors, not forecasts of later steering or boosting.
    /// research/battledome/2026-10-26-safe-spawn/REPORT.md
    pub(crate) fn clears_departures(&self, head: Point, heading: f64) -> bool {
        let radius = head.distance(Point::default());
        if !radius.is_finite() || radius <= 0. || !heading.is_finite() {
            return false;
        }
        let head_bounds = Bounds::between(head, head);
        // The head can travel this far in three seconds before steering is
        // known. Keep that runway from old bodies while allowing the harmless
        // trailing body to use the smaller whole-body clearance.
        if self.obstacles.iter().any(|obstacle| {
            !obstacle.valid
                || (near(head_bounds, obstacle.bounds, obstacle.head_width)
                    && obstacle
                        .edges
                        .iter()
                        .any(|edge| segment_distance(head, edge.a, edge.b) <= obstacle.head_width))
        }) {
            return false;
        }
        let end_inward = Point {
            x: head.x * (1. - self.departure_distance / radius),
            y: head.y * (1. - self.departure_distance / radius),
        };
        let end_tangent = Point {
            x: head.x + heading.cos() * self.departure_distance,
            y: head.y + heading.sin() * self.departure_distance,
        };
        [end_inward, end_tangent].into_iter().all(|end| {
            let query = Bounds::between(head, end);
            self.obstacles.iter().all(|obstacle| {
                let width = obstacle.departure_width;
                (!near(query, obstacle.path.bounds, obstacle.departure_path_width)
                    || swept_distance(head, end, obstacle.path.a, obstacle.path.b)
                        > obstacle.departure_path_width)
                    && (!near(query, obstacle.bounds, width)
                        || obstacle.edges.iter().all(|other| {
                            !near(query, other.bounds, width)
                                || swept_distance(head, end, other.a, other.b) > width
                        }))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, spawn};

    fn reference(world: &Simulator, body: &[Point], clearance: f64, horizon: f64) -> bool {
        world.body_is_clear(body, clearance)
            && world.snakes.iter().filter(|s| s.alive).all(|s| {
                let end = Point {
                    x: s.head.x + s.heading.cos() * s.speed * horizon,
                    y: s.head.y + s.heading.sin() * s.speed * horizon,
                };
                let width =
                    (collision_distance(s.segments, body.len()) + 40.).max(spawn::CLEARANCE);
                body.windows(2)
                    .all(|e| swept_distance(e[0], e[1], s.head, end) > width)
            })
    }

    #[test]
    fn cached_search_matches_exhaustive_clearance_and_head_paths() {
        let mut config = Config::battledome();
        config.food_count = 0;
        let mut world = Simulator::new(7, config).unwrap();
        world.add_slots(16).unwrap();
        let mut random = 0x1324_5768_9abc_def0u64;
        let mut unit = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            (random >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..32 {
            for snake in &mut world.snakes {
                let angle = unit() * std::f64::consts::TAU;
                let radius = 1800. + unit() * 1600.;
                let (body, heading) = spawn::ring_body(
                    Point {
                        x: radius * angle.cos(),
                        y: radius * angle.sin(),
                    },
                    unit() * 6.,
                    26,
                )
                .unwrap();
                snake.head = body[0];
                snake.body = body;
                snake.heading = heading;
                snake.speed = 100. + unit() * 337.5;
                snake.alive = unit() > 0.2;
            }
            let mut queries = [(600., 2.), (600., 1.5), (400., 2.), (400., 1.5)]
                .map(|(c, h)| (c, h, Safety::new(&world, 26, c, h)));
            for _ in 0..16 {
                let radius = 2684. + unit() * 716.;
                let angle = unit() * std::f64::consts::TAU;
                let (body, _) = world
                    .spawn_body(
                        Point {
                            x: radius * angle.cos(),
                            y: radius * angle.sin(),
                        },
                        unit() * 6.,
                    )
                    .unwrap();
                for (c, h, query) in &mut queries {
                    assert_eq!(
                        query.clears(&body),
                        reference(&world, &body, *c, *h),
                        "c={c} h={h}"
                    );
                }
            }
        }
    }

    #[test]
    fn cached_search_keeps_exact_thresholds_and_rejects_invalid_bodies() {
        let mut world = Simulator::new(7, Config::sandbox()).unwrap();
        world.add_slots(1).unwrap();
        world.snakes[0].head = Point::default();
        world.snakes[0].body = vec![Point::default(); 2];
        world.snakes[0].speed = 0.;
        let mut query = Safety::new(&world, 2, 600., 2.);
        for bits in [600f64.to_bits() - 1, 600f64.to_bits(), 600f64.to_bits() + 1] {
            let body = [
                Point {
                    x: f64::from_bits(bits),
                    y: 0.,
                },
                Point {
                    x: f64::from_bits(bits),
                    y: 42.,
                },
            ];
            assert_eq!(query.clears(&body), reference(&world, &body, 600., 2.));
        }
        for body in [
            vec![],
            vec![Point::default(); 3],
            vec![Point { x: f64::NAN, y: 0. }; 2],
            vec![Point { x: 1000., y: 0. }; 2],
        ] {
            assert!(!query.clears(&body));
        }
    }

    #[test]
    fn newborn_departures_reject_a_body_beyond_initial_clearance() {
        let mut config = Config::battledome();
        config.radius = 5000.;
        config.food_count = 0;
        let mut world = Simulator::new(7, config).unwrap();
        world.add_slots(1).unwrap();
        let other = &mut world.snakes[0];
        other.head = Point {
            x: 2600.,
            y: -2000.,
        };
        other.body = vec![other.head, Point { x: 2600., y: 300. }];
        other.speed = 0.;
        let (body, heading) = world
            .spawn_body(Point { x: 3200., y: 0. }, std::f64::consts::FRAC_PI_2)
            .unwrap();
        let mut query = Safety::new(&world, 26, 400., 2.);
        assert!(query.clears(&body));
        assert!(!query.clears_departures(body[0], heading));
        world.snakes[0].alive = false;
        let mut query = Safety::new(&world, 26, 400., 2.);
        assert!(query.clears(&body));
        assert!(query.clears_departures(body[0], heading));
    }

    #[test]
    fn a_nonfinite_neighbor_path_is_not_admissible() {
        let mut world = Simulator::new(7, Config::sandbox()).unwrap();
        world.add_slots(1).unwrap();
        world.snakes[0].head = Point { x: 0., y: 0. };
        world.snakes[0].body = vec![];
        world.snakes[0].heading = f64::NAN;
        let body = [Point { x: 800., y: 0. }, Point { x: 800., y: 42. }];
        assert!(!Safety::new(&world, 2, 400., 2.).clears(&body));
    }

    #[test]
    fn the_newborn_head_keeps_normal_travel_room_even_off_the_departure_rays() {
        let mut config = Config::battledome();
        config.radius = 5000.;
        config.food_count = 0;
        let mut world = Simulator::new(7, config).unwrap();
        world.add_slots(1).unwrap();
        let other = &mut world.snakes[0];
        other.head = Point { x: 3184., y: 2000. };
        other.body = vec![other.head, Point { x: 3184., y: 0. }];
        other.speed = 0.;
        let (body, heading) = world
            .spawn_body(Point { x: 2684., y: 0. }, std::f64::consts::FRAC_PI_2)
            .unwrap();
        let mut query = Safety::new(&world, 26, 400., 2.);
        assert!(query.clears(&body));
        assert!(!query.clears_departures(body[0], heading));
    }
}
