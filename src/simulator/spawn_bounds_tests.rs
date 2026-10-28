use super::*;

fn world() -> Simulator {
    Simulator::new(WorldOptions {
        config: Config {
            radius: 1e6,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Config::sandbox()
        },
        ..WorldOptions::default()
    })
    .unwrap()
}

fn probe(world: &Simulator, line: &[Point], excluded: Option<usize>) {
    for clearance in [0., 40., 600., 5000.] {
        assert_eq!(
            world.line_is_clear(line, line.len(), clearance, excluded),
            world.exhaustive_clear(line, line.len(), clearance, excluded),
            "clearance={clearance}, excluded={excluded:?}, line={line:?}"
        );
    }
    for stop in [f64::NEG_INFINITY, -500., 0., 600., 1e6, f64::INFINITY] {
        // Stronger than the documented bounded contract: early returns also
        // retain their exact bits, so callers' comparisons/ties cannot change.
        assert_eq!(
            world
                .spawn_margin_bounded_excluding(line, stop, excluded)
                .map(f64::to_bits),
            world
                .exhaustive_margin(line, stop, excluded)
                .map(f64::to_bits),
            "stop={stop}, excluded={excluded:?}, line={line:?}"
        );
    }
}

#[test]
fn spawn_bounds_match_exhaustive_long_crossing_and_boundary_bodies() {
    let mut world = world();
    let mut random = Random(7);
    for slot in 0..32 {
        let head = Point {
            x: (random.next() - 0.5) * 20000.,
            y: (random.next() - 0.5) * 20000.,
        };
        let body: Vec<_> = (0..if slot == 0 { 20000 } else { 20 })
            .map(|k| Point {
                x: head.x + (k as f64 * 0.1).cos() * 500.,
                y: head.y - k as f64 * 2.,
            })
            .collect();
        let slot = world.spawn(slot.to_string(), vec![head; 2], 0.).unwrap();
        world.correct_mass(slot, body.clone(), 0.).unwrap();
        world.correct_pose(slot, head, body, 0.).unwrap();
    }
    // Long edge crosses the candidate although its endpoints and head are far.
    let slot = world
        .spawn(
            "crossing".into(),
            vec![Point { x: -20000., y: 0. }, Point { x: 20000., y: 0. }],
            0.,
        )
        .unwrap();
    for y in [-600.0f64, -1., 0., 1., 600., 5000.] {
        for y in [y.next_down(), y, y.next_up()] {
            let line = [Point { x: 0., y }, Point { x: 0., y: y + 42. }];
            probe(&world, &line, None);
            probe(&world, &line, Some(slot));
        }
    }
    for _ in 0..100 {
        let line: Vec<_> = (0..2 + (random.next() * 6.) as usize)
            .map(|_| Point {
                x: (random.next() - 0.5) * 40000.,
                y: (random.next() - 0.5) * 40000.,
            })
            .collect();
        probe(&world, &line, None);
        probe(&world, &line, Some(0));
    }
    probe(
        &world,
        &[Point {
            x: world.radius - SPAWN_WALL_MARGIN,
            y: 0.,
        }; 2],
        None,
    );
    probe(&world, &[Point { x: f64::NAN, y: 0. }; 2], None);
}

#[test]
fn spawn_bounds_refresh_after_all_geometry_edits_and_tick() {
    let mut world = world();
    let line = [Point::default(), Point { x: 42., y: 0. }];
    probe(&world, &line, None); // warm an empty cache before appending
    let slot = world.spawn("a".into(), line.to_vec(), 0.).unwrap();
    probe(&world, &line, None);
    assert!(!world.body_is_clear(&line, 600.));
    assert!(world.line_is_clear(&line, 2, 600., Some(slot)));
    world.retire(slot).unwrap();
    probe(&world, &line, None);
    assert!(world.body_is_clear(&line, 600.));
    world
        .respawn_body(slot, vec![Point { x: 10000., y: 0. }; 2], 0.)
        .unwrap();
    probe(&world, &line, None);
    assert!(world.body_is_clear(&line, 600.));
    world
        .correct_pose(slot, Point::default(), line.to_vec(), 0.)
        .unwrap();
    probe(&world, &line, None);
    let mut s = world.snakes[slot].clone();
    s.head.x = 5000.;
    s.body = vec![s.head; 2];
    world
        .import_life(
            slot,
            ReplayLife::from_observed(s, InitialSpeed::Known(world.speeds[slot])),
        )
        .unwrap();
    probe(&world, &line, None);
    world.correct_mass(slot, line.to_vec(), 0.).unwrap();
    probe(&world, &line, None);
    assert!(!world.body_is_clear(&line, 600.));
    let checkpoint = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
    let restored =
        Simulator::from_checkpoint(&checkpoint, crate::RestoreOptions::default()).unwrap();
    probe(&restored, &line, None);
    probe(&world.clone(), &line, None);
    world
        .step(&[Some(Action {
            heading: 0.,
            boost: false,
        })])
        .unwrap();
    assert!(world.spawn_bounds.get().is_none());
    probe(&world, &line, None);
    // A second birth must be visible to the very next query in the same tick.
    let other = world.spawn("b".into(), line.to_vec(), 0.).unwrap();
    probe(&world, &line, Some(slot));
    assert!(!world.line_is_clear(&line, 2, 600., Some(slot)));
    world.retire(other).unwrap();
    probe(&world, &line, None);
}

#[test]
fn spawn_bounds_preserve_unusual_coordinate_and_empty_body_geometry() {
    let mut world = world();
    world.config.radius = 1e150;
    let slot = world
        .spawn("a".into(), vec![Point::default(); 2], 0.)
        .unwrap();
    for x in [1e7f64.next_down(), 1e7, 1e7f64.next_up(), 1e100, -1e100] {
        // Internal oracle also covers historical externally supplied empty bodies.
        world.snakes[slot].head = Point { x, y: 0. };
        world.snakes[slot].body.clear();
        world.spawn_bounds.take();
        probe(&world, &[Point { x, y: 600. }, Point { x, y: 642. }], None);
        probe(&world, &[Point::default(); 2], None);
    }
}

// Exhaustive predicates retained from the pre-optimization runtime.
impl Simulator {
    fn exhaustive_margin(
        &self,
        body: &[Point],
        stop_at: f64,
        excluded: Option<usize>,
    ) -> Option<f64> {
        let limit = self.radius - SPAWN_WALL_MARGIN;
        if !(2..=430).contains(&body.len())
            || body
                .iter()
                .any(|p| !p.finite() || p.distance(Point::default()) >= limit)
        {
            return None;
        }
        let mut margin = f64::INFINITY;
        for (_, s) in self
            .snakes
            .iter()
            .enumerate()
            .filter(|(i, s)| s.alive && Some(*i) != excluded)
        {
            let contact = collision_distance(s.segments, body.len()) + 40.;
            for edge in body.windows(2) {
                let (head, tail) = (edge[0], edge[1]);
                margin = margin.min(segment_distance(s.head, head, tail) - contact);
                if margin <= stop_at {
                    return Some(margin);
                }
                let mut a = s.head;
                for &b in &s.body {
                    margin = margin.min(swept_distance(head, tail, a, b) - contact);
                    if margin <= stop_at {
                        return Some(margin);
                    }
                    a = b;
                }
            }
        }
        Some(margin)
    }
    fn exhaustive_clear(
        &self,
        line: &[Point],
        segments: usize,
        clearance: f64,
        excluded: Option<usize>,
    ) -> bool {
        if !clearance.is_finite()
            || clearance < 0.0
            || line
                .iter()
                .any(|p| p.distance(Point::default()) >= self.radius - SPAWN_WALL_MARGIN)
        {
            return false;
        }
        self.snakes
            .iter()
            .enumerate()
            .filter(|(i, s)| s.alive && Some(*i) != excluded)
            .all(|(_, s)| {
                let width = (collision_distance(s.segments, segments) + 40.).max(clearance);
                line.windows(2).all(|edge| {
                    let (head, tail) = (edge[0], edge[1]);
                    // Include the head even if an externally supplied body is empty.
                    if segment_distance(s.head, head, tail) <= width {
                        return false;
                    }
                    let mut a = s.head;
                    s.body.iter().all(|&b| {
                        let distance = swept_distance(head, tail, a, b);
                        a = b;
                        distance > width
                    })
                })
            })
    }
}
