//! Cached broad phase for one circular spawn search. Exact distances still own
//! admission; distant edge pairs only skip work. See the safe-spawn study.
use crate::{
    Point, Simulator,
    physics::{Bounds, base_speed, collision_distance, segment_distance_le, swept_distance},
};
use std::cell::OnceCell;

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
    head_blocks: OnceCell<Vec<Bounds>>,
    bounds: Bounds,
    body_width: f64,
    path: Edge,
    inward: Option<Edge>,
    inward_width: f64,
    path_width: f64,
    departure_width: f64,
    departure_path_width: f64,
    head_width: f64,
}

const HEAD_BLOCK_EDGES: usize = 8;
const HEAD_BLOCK_LIMIT: f64 = 1e7;

fn head_boundable(point: Point) -> bool {
    point.finite() && point.x.abs() <= HEAD_BLOCK_LIMIT && point.y.abs() <= HEAD_BLOCK_LIMIT
}

fn head_blocks(edges: &[Edge]) -> Vec<Bounds> {
    if edges.len() <= HEAD_BLOCK_EDGES
        || !edges
            .iter()
            .all(|edge| head_boundable(edge.a) && head_boundable(edge.b))
    {
        return Vec::new();
    }
    edges
        .chunks(HEAD_BLOCK_EDGES)
        .map(|chunk| {
            let mut bounds = chunk[0].bounds;
            for edge in &chunk[1..] {
                bounds.include(edge.a);
                bounds.include(edge.b);
            }
            bounds
        })
        .collect()
}

impl Obstacle {
    fn blocks_head(&self, head: Point) -> bool {
        let blocked = |edge: &Edge| segment_distance_le(head, edge.a, edge.b, self.head_width);
        if !head_boundable(head) || !(0. ..=HEAD_BLOCK_LIMIT).contains(&self.head_width) {
            return self.edges.iter().any(blocked);
        }
        // Most successful searches never reach this obstacle. Build only after
        // the outer near check admits a supported head query, then reuse it.
        let blocks = self.head_blocks.get_or_init(|| head_blocks(&self.edges));
        if blocks.is_empty() {
            return self.edges.iter().any(blocked);
        }
        // As in visibility's eight-edge bounds, one unit exceeds projection/
        // hypot roundoff at this finite scale. Borderline boxes keep every
        // original edge test, in order; extreme/non-finite inputs stay linear.
        // research/performance/2026-10-29-spawn-head-blocks/REPORT.md
        let query = Bounds::between(head, head);
        blocks
            .iter()
            .zip(self.edges.chunks(HEAD_BLOCK_EDGES))
            .any(|(&bounds, edges)| {
                bounds.near(query, self.head_width + 1.) && edges.iter().any(blocked)
            })
    }
}

/// Immutable neighbors, with a reused candidate buffer. Built after excluding
/// the joining slot; no world tick runs during the search.
pub(crate) struct Safety {
    obstacles: Vec<Obstacle>,
    candidate: Vec<Edge>,
    wall_limit: f64,
    segments: usize,
    departure_distance: f64,
    inward_only: bool,
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
    #[cfg(test)]
    pub(crate) fn new(world: &Simulator, segments: usize, clearance: f64, horizon: f64) -> Self {
        Self::new_excluding(world, None, segments, clearance, horizon)
    }

    pub(crate) fn new_excluding(
        world: &Simulator,
        excluded: Option<usize>,
        segments: usize,
        clearance: f64,
        horizon: f64,
    ) -> Self {
        let obstacles = world
            .snakes()
            .iter()
            .enumerate()
            .filter(|(slot, s)| Some(*slot) != excluded && s.alive)
            .map(|(_, s)| {
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
                    head_blocks: OnceCell::new(),
                    edges,
                    bounds,
                    body_width: contact.max(clearance),
                    path,
                    inward: world
                        .pit_radius()
                        .filter(|&pit| s.head.distance(Point::default()) > pit)
                        .map(|_| {
                            let radius = s.head.distance(Point::default());
                            let travel = (s.speed * super::DEPARTURE_SECONDS).min(radius);
                            Edge::new(
                                s.head,
                                Point {
                                    x: s.head.x * (1. - travel / radius),
                                    y: s.head.y * (1. - travel / radius),
                                },
                            )
                        }),
                    inward_width: contact.max(300.),
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
            wall_limit: world.radius() - crate::simulator::SPAWN_WALL_MARGIN,
            segments,
            departure_distance: super::DEPARTURE_SECONDS * base_speed(segments),
            inward_only: false,
        }
    }

    /// A hard corridor floor after the preferred policy exhausts its search.
    /// Reserve actual three-second inward travel rather than a full
    /// travel-length circle around a head that will leave the spawn band.
    /// Provisional: research/battledome/2026-10-27-arena-playtest/REPORT.md
    pub(crate) fn departure_floor(world: &Simulator, excluded: usize, segments: usize) -> Self {
        let mut safety = Self::new_excluding(world, Some(excluded), segments, 200., 1.);
        safety.inward_only = true;
        for obstacle in &mut safety.obstacles {
            let contact = obstacle.body_width.min(obstacle.path_width);
            obstacle.path_width = contact.max(300.);
            obstacle.departure_width = contact.max(300.);
            obstacle.departure_path_width = contact.max(300.);
            obstacle.head_width = obstacle.body_width;
        }
        safety
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
            if obstacle.inward.as_ref().is_some_and(|path| {
                near(bounds, path.bounds, obstacle.inward_width)
                    && self.candidate.iter().any(|e| {
                        near(e.bounds, path.bounds, obstacle.inward_width)
                            && swept_distance(e.a, e.b, path.a, path.b) <= obstacle.inward_width
                    })
            }) {
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

    /// Maximin score for the last accepted candidate, with losing poses
    /// stopping at the incumbent. All hard path/runway checks run separately.
    pub(crate) fn body_margin(&self, stop_at: f64) -> f64 {
        let mut margin = f64::INFINITY;
        for edge in &self.candidate {
            for obstacle in &self.obstacles {
                if !near(edge.bounds, obstacle.bounds, margin + obstacle.body_width) {
                    continue;
                }
                for other in &obstacle.edges {
                    if near(edge.bounds, other.bounds, margin + obstacle.body_width) {
                        margin = margin.min(
                            swept_distance(edge.a, edge.b, other.a, other.b) - obstacle.body_width,
                        );
                        if margin <= stop_at {
                            return margin;
                        }
                    }
                }
            }
        }
        margin
    }

    /// Every accepted departure needs this head runway. Check it before the
    /// whole candidate body: a rejected head cannot be rescued by its tail.
    /// Skipping those body tests changes neither accepted poses nor maximin scores.
    pub(crate) fn head_is_clear(&self, head: Point) -> bool {
        if !head.finite() {
            return false;
        }
        let head_bounds = Bounds::between(head, head);
        !self.obstacles.iter().any(|obstacle| {
            !obstacle.valid
                || (near(head_bounds, obstacle.bounds, obstacle.head_width)
                    && obstacle.blocks_head(head))
        })
    }

    /// A valid ring whose head already fails cannot become an accepted body.
    /// Keep invalid ring geometry on the original construction/error path.
    pub(crate) fn rejects_ring_head(&self, head: Point) -> bool {
        !self.head_is_clear(head)
            && super::ring_step(head.distance(Point::default()), self.segments).is_some()
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
        // The head can travel this far in three seconds before steering is
        // known. Keep that runway from old bodies while allowing the harmless
        // trailing body to use the smaller whole-body clearance.
        if !self.head_is_clear(head) {
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
        [end_inward, end_tangent]
            .into_iter()
            .take(if self.inward_only { 1 } else { 2 })
            .all(|end| {
                let query = Bounds::between(head, end);
                self.obstacles.iter().all(|obstacle| {
                    let width = obstacle.departure_width;
                    (!near(query, obstacle.path.bounds, obstacle.departure_path_width)
                        || swept_distance(head, end, obstacle.path.a, obstacle.path.b)
                            > obstacle.departure_path_width)
                        && obstacle.inward.as_ref().is_none_or(|path| {
                            !near(query, path.bounds, obstacle.inward_width)
                                || swept_distance(head, end, path.a, path.b) > obstacle.inward_width
                        })
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
mod head_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, spawn};

    fn reference(world: &Simulator, body: &[Point], clearance: f64, horizon: f64) -> bool {
        world.body_is_clear(body, clearance)
            && world.snakes().iter().filter(|s| s.alive).all(|s| {
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

    fn guarded_reference(world: &Simulator, body: &[Point], clearance: f64, horizon: f64) -> bool {
        reference(world, body, clearance, horizon)
            && world.snakes().iter().filter(|s| s.alive).all(|s| {
                let r = s.head.distance(Point::default());
                if !world.pit_radius().is_some_and(|pit| r > pit) {
                    return true;
                }
                let travel = (s.speed * 3.).min(r);
                let end = Point {
                    x: s.head.x * (1. - travel / r),
                    y: s.head.y * (1. - travel / r),
                };
                let width = (collision_distance(s.segments, body.len()) + 40.).max(300.);
                body.windows(2)
                    .all(|e| swept_distance(e[0], e[1], s.head, end) > width)
            })
    }

    #[test]
    fn preferred_spawns_cannot_cut_an_older_heads_inward_route() {
        let mut config = Config::battledome();
        config.food_count = 0;
        config.ambient_replenish_per_second = 0.;
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config,
            population: crate::InitialPopulation::Placed {
                ids: vec!["old".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let angle = 0.25_f64;
        let head = Point {
            x: 3400. * angle.cos(),
            y: 3400. * angle.sin(),
        };
        let (body, heading) = world
            .spawn_body(head, angle + std::f64::consts::FRAC_PI_2)
            .unwrap();
        let mut old = world.snakes()[0].clone();
        old.head = head;
        old.body = body;
        old.heading = heading;
        old.speed = crate::physics::BOOST_SPEED;
        world
            .import_life(
                0,
                crate::ReplayLife::from_observed(
                    old,
                    crate::InitialSpeed::Infer(crate::physics::BOOST_SPEED),
                ),
            )
            .unwrap();
        let angle = 0.4_f64;
        let (body, _) = world
            .spawn_body(
                Point {
                    x: 2684. * angle.cos(),
                    y: 2684. * angle.sin(),
                },
                angle + std::f64::consts::FRAC_PI_2,
            )
            .unwrap();
        assert!(reference(&world, &body, 400., 2.));
        assert!(
            !Safety::new(&world, 26, 400., 2.).clears(&body),
            "a preferred pose must also leave the older inward route clear"
        );
    }

    #[test]
    fn cached_search_matches_exhaustive_clearance_and_head_paths() {
        let mut config = Config::battledome();
        config.food_count = 0;
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config,
            population: crate::InitialPopulation::Placed {
                ids: (0..16).map(|slot| slot.to_string()).collect(),
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut random = 0x1324_5768_9abc_def0u64;
        let mut unit = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            (random >> 11) as f64 / (1u64 << 53) as f64
        };
        for _ in 0..32 {
            for slot in 0..world.snakes().len() {
                let mut snake = world.snakes()[slot].clone();
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
                world
                    .import_life(
                        slot,
                        crate::ReplayLife::from_observed(
                            snake.clone(),
                            crate::InitialSpeed::Infer(snake.speed),
                        ),
                    )
                    .unwrap();
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
                        guarded_reference(&world, &body, *c, *h),
                        "c={c} h={h}"
                    );
                }
            }
        }
    }

    #[test]
    fn cached_search_keeps_exact_thresholds_and_rejects_invalid_bodies() {
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: (0..1).map(|slot| slot.to_string()).collect(),
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        {
            let mut snake = world.snakes()[0].clone();
            snake.head = Point::default();
            world
                .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
                .unwrap();
        }
        {
            let mut snake = world.snakes()[0].clone();
            snake.body = vec![Point::default(); 2];
            world
                .correct_mass(0, snake.body.clone(), snake.fam)
                .unwrap();
            world
                .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
                .unwrap();
        }
        {
            let mut snake = world.snakes()[0].clone();
            snake.speed = 0.;
            world
                .import_life(
                    0,
                    crate::ReplayLife::from_observed(
                        snake.clone(),
                        crate::InitialSpeed::Infer(snake.speed),
                    ),
                )
                .unwrap();
        }
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
    fn head_prefilter_keeps_the_inclusive_runway_threshold_for_both_tiers() {
        let mut config = Config::sandbox();
        config.food_count = 0;
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config,
            population: crate::InitialPopulation::Placed {
                ids: vec!["old".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut old = world.snakes()[0].clone();
        old.head = Point::default();
        old.body.fill(old.head);
        old.speed = 0.;
        world
            .import_life(
                0,
                crate::ReplayLife::from_observed(old, crate::InitialSpeed::Infer(0.)),
            )
            .unwrap();
        for query in [
            Safety::new(&world, 2, 600., 2.),
            Safety::departure_floor(&world, 1, 2),
        ] {
            let width = query.obstacles[0].head_width;
            for sign in [-1., 1.] {
                for bits in [width.to_bits() - 1, width.to_bits(), width.to_bits() + 1] {
                    assert_eq!(
                        query.head_is_clear(Point {
                            x: sign * f64::from_bits(bits),
                            y: 0.,
                        }),
                        bits > width.to_bits()
                    );
                }
            }
            assert!(!query.head_is_clear(Point { x: f64::NAN, y: 0. }));
        }
    }

    #[test]
    fn ring_head_shortcut_keeps_invalid_geometry_on_the_error_path() {
        let mut world = Simulator::new(crate::WorldOptions {
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["old".into()],
                placement: crate::Placement::TestRing,
            },
            ..Default::default()
        })
        .unwrap();
        let head = Point { x: 2684., y: 0. };
        world.correct_pose(0, head, vec![head; 2], 0.).unwrap();
        let query = Safety::new(&world, 26, 400., 2.);
        assert!(query.rejects_ring_head(head));
        for head in [
            Point::default(),
            Point { x: 1., y: 0. },
            Point { x: f64::NAN, y: 0. },
            Point {
                x: f64::INFINITY,
                y: 0.,
            },
        ] {
            assert!(!query.rejects_ring_head(head));
            assert!(crate::spawn::ring_body(head, 0., 26).is_none());
        }
    }

    #[test]
    fn newborn_departures_reject_a_body_beyond_initial_clearance() {
        let mut config = Config::battledome();
        config.radius = 5000.;
        config.food_count = 0;
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config,
            population: crate::InitialPopulation::Placed {
                ids: (0..1).map(|slot| slot.to_string()).collect(),
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut detached_other = world.snakes()[0].clone();
        let other = &mut detached_other;
        other.head = Point {
            x: 2600.,
            y: -2000.,
        };
        other.body = vec![other.head, Point { x: 2600., y: 300. }];
        other
            .body
            .resize(other.segments, *other.body.last().unwrap());
        other.speed = 0.;
        world
            .import_life(
                0,
                crate::ReplayLife::from_observed(
                    other.clone(),
                    crate::InitialSpeed::Infer(other.speed),
                ),
            )
            .unwrap();
        let (body, heading) = world
            .spawn_body(Point { x: 3200., y: 0. }, std::f64::consts::FRAC_PI_2)
            .unwrap();
        let mut query = Safety::new(&world, 26, 400., 2.);
        assert!(query.clears(&body));
        assert!(!query.clears_departures(body[0], heading));
        world.retire(0).unwrap();
        let mut query = Safety::new(&world, 26, 400., 2.);
        assert!(query.clears(&body));
        assert!(query.clears_departures(body[0], heading));
    }

    #[test]
    fn a_nonfinite_neighbor_path_is_not_admissible() {
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["0".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let body = [Point { x: 800., y: 0. }, Point { x: 800., y: 42. }];
        // A finite neighbor can still overflow its projected path at this horizon.
        assert!(!Safety::new(&world, 2, 400., f64::MAX).clears(&body));
    }

    #[test]
    fn the_newborn_head_keeps_normal_travel_room_even_off_the_departure_rays() {
        let mut config = Config::battledome();
        config.radius = 5000.;
        config.food_count = 0;
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config,
            population: crate::InitialPopulation::Placed {
                ids: (0..1).map(|slot| slot.to_string()).collect(),
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut detached_other = world.snakes()[0].clone();
        let other = &mut detached_other;
        other.head = Point { x: 3184., y: 2000. };
        other.body = vec![other.head, Point { x: 3184., y: 0. }];
        other
            .body
            .resize(other.segments, *other.body.last().unwrap());
        other.speed = 0.;
        world
            .import_life(
                0,
                crate::ReplayLife::from_observed(
                    other.clone(),
                    crate::InitialSpeed::Infer(other.speed),
                ),
            )
            .unwrap();
        let (body, heading) = world
            .spawn_body(Point { x: 2684., y: 0. }, std::f64::consts::FRAC_PI_2)
            .unwrap();
        let mut query = Safety::new(&world, 26, 400., 2.);
        assert!(query.clears(&body));
        assert!(!query.head_is_clear(body[0]));
        assert!(!query.clears_departures(body[0], heading));
    }
}
