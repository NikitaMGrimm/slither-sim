use slither_sim::{Action, Config, Point, Simulator};
use slither_sim::{Constraint, ErrorKind};

fn world() -> Simulator {
    let mut world = Simulator::new(
        1,
        Config {
            radius: 3000.,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Default::default()
        },
    )
    .unwrap();
    for i in 0..2 {
        world.add(i.to_string(), i, 2).unwrap();
        world.snakes[i].alive = false;
    }
    world.respawn(0, Point::default(), 0., 2).unwrap();
    world
}

#[test]
fn old_clear_pose_can_kill_both_snakes_before_a_reaction() {
    let mut w = world();
    let head = Point { x: 120., y: 0. };
    assert!(w.spawn_is_clear(head, std::f64::consts::PI, 2, 0.));
    assert!(!w.spawn_is_clear(head, std::f64::consts::PI, 2, 600.));
    w.respawn(1, head, std::f64::consts::PI, 2).unwrap();
    for _ in 0..40 {
        w.step(&[
            Some(Action {
                heading: 0.,
                boost: false,
            }),
            Some(Action {
                heading: std::f64::consts::PI,
                boost: false,
            }),
        ])
        .unwrap();
    }
    assert!(
        w.living() < 2,
        "historical placement must reproduce the sudden collision"
    );
}

#[test]
fn exclusion_checks_new_tail_existing_body_and_boundary() {
    let mut w = world();
    // Head is far away, but the new long tail would appear directly ahead.
    assert!(!w.spawn_is_clear(Point { x: 900., y: 0. }, 0., 22, 600.));
    w.snakes[0].head = Point { x: 1200., y: 1200. };
    w.snakes[0].body = vec![
        Point {
            x: -1200.,
            y: 1200.,
        },
        Point {
            x: -1200.,
            y: -1200.,
        },
    ];
    assert!(!w.spawn_is_clear(Point { x: -1100., y: 0. }, 0., 2, 600.));
    assert!(w.spawn_is_clear(Point::default(), 0., 2, 600.));
    assert!(!w.spawn_is_clear(Point { x: 2950., y: 0. }, 0., 2, 600.));
    assert!(!w.spawn_is_clear(Point::default(), 0., 2, f64::NAN));
    w.snakes[0].alive = false;
    assert!(w.spawn_is_clear(Point { x: -1100., y: 0. }, 0., 2, 600.));
}
#[test]
fn observed_spawn_grid_matches_raw_join_bands() {
    use slither_sim::spawn::sample_spawn_distance;
    // Three prospective joins: .416042, .460013 and .306020 of their
    // respective arena radii, all within packet-coordinate precision of the grid.
    for (radius, observed) in [
        (11_881.0_f64, 0.4160420123_f64),
        (12_274.0, 0.4600131696),
        (12_200.0, 0.3060199308),
    ] {
        let index = ((observed - 0.009) / 0.011).round();
        let sample = ((0.009 + index * 0.011) * radius / (radius - 100.0)).powi(2);
        let sampled = sample_spawn_distance(radius, sample) / radius;
        assert!((sampled - observed).abs() < 0.00005);
    }
    let radius: f64 = 12_274.0;
    for i in 0..1000 {
        let u = i as f64 / 1000.0;
        let sampled = sample_spawn_distance(radius, u);
        let index = ((sampled / radius - 0.009) / 0.011).round();
        assert!((sampled / radius - (0.009 + 0.011 * index)).abs() < 1e-12);
        assert!(sampled <= radius - 100.0);
    }
}

#[test]
fn tiny_custom_arena_without_a_spawn_band_uses_its_center() {
    for radius in [1.0, 100.0, 100.5] {
        for unit in [0.0, 0.5, 0.99] {
            assert_eq!(slither_sim::spawn::sample_spawn_distance(radius, unit), 0.0);
        }
    }
}

#[test]
fn initial_ring_rejects_an_outside_tail_without_changing_any_slot_state() {
    for radius in [50., 84.] {
        let mut world = Simulator::new(
            1,
            Config {
                radius,
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::sandbox()
            },
        )
        .unwrap();
        let before = serde_json::to_value(&world).unwrap();
        let error = world.add("a", 0, 1).unwrap_err();
        assert_eq!(
            error.kind,
            ErrorKind::InvalidState {
                rule: Constraint::OutsideArena
            }
        );
        assert_eq!(error.slot, Some(0));
        assert_eq!(error.field.as_deref(), Some("body"));
        assert_eq!(serde_json::to_value(&world).unwrap(), before);
        // The same ring remains available once the arena contains its tail.
        world.config.radius = 85.;
        assert_eq!(world.add("a", 0, 1).unwrap(), 0);
    }
}
