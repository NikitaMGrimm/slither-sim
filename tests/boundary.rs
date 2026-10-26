use slither_sim::*;

#[test]
fn shallow_contact_slides_at_normal_and_boost_speeds_without_rotating_the_heading() {
    use std::f64::consts::FRAC_PI_2;
    for dt in [0.008, 0.016, 0.032] {
        for speed in [5.79 / 0.032, 14.0 / 0.032] {
            for scale in [1., 2.] {
                let radius = 8000.;
                let mut head = Point {
                    x: radius - 30.,
                    y: 0.,
                };
                let start = head;
                for _ in 0..(4. / dt) as usize {
                    let heading = head.y.atan2(head.x) + FRAC_PI_2 - 0.6;
                    let proposed = Point {
                        x: head.x + speed * dt * heading.cos(),
                        y: head.y + speed * dt * heading.sin(),
                    };
                    let (next, dead) = boundary::constrain(
                        proposed,
                        heading,
                        scale,
                        radius,
                        BoundaryMode::Sliding,
                    );
                    assert!(!dead);
                    head = next;
                }
                assert!(head.distance(Point::default()) < radius);
                let traveled = head.y.atan2(head.x) * radius;
                let projected = speed * 4. * 0.6_f64.cos();
                assert!(
                    (traveled - projected).abs() < 25.,
                    "{traveled} vs {projected}"
                );
                assert!(head.distance(start) > 500.);
            }
        }
    }
}

#[test]
fn steep_contacts_die_and_contact_is_rotation_and_reflection_invariant() {
    use std::f64::consts::FRAC_PI_2;
    for rotation in [0.0_f64, 1., 3., 5.] {
        for sign in [-1., 1.] {
            let head = Point {
                x: rotation.cos() * 7999.,
                y: rotation.sin() * 7999.,
            };
            for (outward, lethal) in [(0.6, false), (0.75, true), (1.2, true)] {
                let heading = rotation + sign * (FRAC_PI_2 - outward);
                let (next, dead) =
                    boundary::constrain(head, heading, 1., 8000., BoundaryMode::Sliding);
                assert_eq!(dead, lethal);
                if !dead {
                    assert!(next.distance(Point::default()) < 8000.);
                }
            }
        }
    }
}

#[test]
fn sliding_does_not_skip_snake_body_collisions() {
    let mut w = Simulator::new(
        7,
        Config {
            radius: 8000.,
            food_count: 0,
            ..Config::sandbox()
        },
    )
    .unwrap();
    w.add("rider", 0, 2).unwrap();
    w.add("obstacle", 1, 2).unwrap();
    for s in &mut w.snakes {
        s.alive = false;
    }
    w.respawn(
        0,
        Point { x: 7988., y: 0. },
        std::f64::consts::FRAC_PI_2 - 0.6,
        2,
    )
    .unwrap();
    // Lies across the rider's slide, just ahead of its tip.
    w.respawn(1, Point { x: 7985., y: 18. }, 0., 2).unwrap();
    w.step(&[
        Some(Action {
            heading: w.snakes[0].heading,
            boost: false,
        }),
        Some(Action {
            heading: w.snakes[1].heading,
            boost: false,
        }),
    ])
    .unwrap();
    assert!(
        w.deaths
            .iter()
            .any(|d| d.victim == "rider" && d.reason == DeathReason::Body)
    );
    assert!(
        w.deaths
            .iter()
            .filter(|d| d.reason == DeathReason::Body)
            .all(|d| d.food > 0)
    );
}

#[test]
fn boundary_deaths_drop_no_food_even_with_the_body_inside() {
    for mode in [BoundaryMode::Sliding, BoundaryMode::Hard] {
        let mut w = Simulator::new(
            7,
            Config {
                radius: 8000.,
                food_count: 0,
                boundary_mode: mode,
                ..Config::sandbox()
            },
        )
        .unwrap();
        w.add("outward", 0, 8).unwrap();
        w.snakes[0].alive = false;
        w.respawn(0, Point { x: 7998., y: 0. }, 0., 8).unwrap();
        assert!(
            w.snakes[0]
                .body
                .iter()
                .all(|p| p.distance(Point::default()) < 8000.)
        );
        for _ in 0..4 {
            w.step(&[Some(Action {
                heading: 0.,
                boost: false,
            })])
            .unwrap();
        }
        assert_eq!(w.deaths.len(), 1);
        assert_eq!(w.deaths[0].reason, DeathReason::Boundary);
        assert_eq!(w.deaths[0].food, 0);
        assert!(w.food.is_empty());
    }
}

#[test]
fn boundary_modes_share_interior_motion() {
    let mut a = Simulator::new(
        9,
        Config {
            radius: 8000.,
            food_count: 100,
            ..Config::sandbox()
        },
    )
    .unwrap();
    let mut b = Simulator::new(
        9,
        Config {
            boundary_mode: BoundaryMode::Hard,
            ..a.config.clone()
        },
    )
    .unwrap();
    a.add("s", 0, 1).unwrap();
    b.add("s", 0, 1).unwrap();
    for _ in 0..100 {
        let actions = [Some(Action {
            heading: 1.,
            boost: true,
        })];
        a.step(&actions).unwrap();
        b.step(&actions).unwrap();
        assert_eq!(
            serde_json::to_value(&a.snakes).unwrap(),
            serde_json::to_value(&b.snakes).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&a.food).unwrap(),
            serde_json::to_value(&b.food).unwrap()
        );
    }
    assert!(boundary::constrain(Point { x: 8001., y: 0. }, 1., 1., 8000., BoundaryMode::Hard).1);
}

#[test]
fn forward_contact_offset_matches_received_anchor_samples() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/boundary-contact.json")).unwrap();
    for capture in fixture["captures"].as_array().unwrap() {
        let mut errors = Vec::new();
        for row in capture["rows"].as_array().unwrap() {
            let n = |key: &str| row[key].as_f64().unwrap();
            let lead = 16. * n("scale");
            let distance =
                (n("x") + lead * n("heading").cos()).hypot(n("y") + lead * n("heading").sin());
            errors.push((distance - n("radius")).abs());
        }
        errors.sort_by(f64::total_cmp);
        assert!(errors.len() >= 80);
        // Anchor quantization, radius receipt ordering and control lag remain.
        assert!(errors[errors.len() / 2] < 3., "{}", capture["name"]);
        assert!(errors[errors.len() * 95 / 100] < 7., "{}", capture["name"]);
    }
}

#[test]
fn radius_trace_changes_boundary_and_raw_observations_at_tick_end() {
    let mut w = Simulator::new(
        1,
        Config {
            radius: 1000.,
            food_count: 0,
            radius_updates: vec![
                RadiusUpdate {
                    time: 0.,
                    radius: 800.,
                },
                RadiusUpdate {
                    time: 2. * slither_sim::physics::LOOP_SECONDS,
                    radius: 200.,
                },
            ],
            ..Config::sandbox()
        },
    )
    .unwrap();
    assert_eq!(w.config.radius, 800.);
    w.add("s", 0, 1).unwrap();
    w.food.push(Food {
        x: 300.,
        y: 0.,
        value: 1.,
        size: None,
        origin: None,
    });
    w.step(&[Some(Action {
        heading: std::f64::consts::PI,
        boost: false,
    })])
    .unwrap();
    assert!(w.snakes[0].alive);
    assert_eq!(w.observe(0).unwrap().arena.radius, 800.);
    w.step(&[Some(Action {
        heading: std::f64::consts::PI,
        boost: false,
    })])
    .unwrap();
    assert!(!w.snakes[0].alive);
    assert_eq!(w.deaths[0].reason, DeathReason::Boundary);
    assert_eq!(w.observe(0).unwrap().arena.radius, 200.);
    assert!(w.food.iter().any(|f| f.x == 300. && f.y == 0.)); // No invented expiry.
}
#[test]
fn malformed_radius_updates_fail_before_world_creation() {
    for updates in [
        vec![RadiusUpdate {
            time: 0.,
            radius: f64::NAN,
        }],
        vec![
            RadiusUpdate {
                time: 1.,
                radius: 1000.,
            },
            RadiusUpdate {
                time: 0.,
                radius: 900.,
            },
        ],
    ] {
        assert!(
            Simulator::new(
                1,
                Config {
                    radius_updates: updates,
                    ..Config::sandbox()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn original_received_radius_trace_replays_without_moving_the_center() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/arena-radius-updates.json")).unwrap();
    let updates: Vec<RadiusUpdate> = serde_json::from_value(fixture["updates"].clone()).unwrap();
    assert_eq!(updates.len(), 790);
    let initial_radius = fixture["initial_radius"].as_f64().unwrap();
    let last = *updates.last().unwrap();
    let mut w = Simulator::new(
        1,
        Config {
            food_count: 0,
            radius: initial_radius,
            radius_updates: updates,
            ..Config::sandbox()
        },
    )
    .unwrap();
    while w.time < last.time {
        w.step(&[]).unwrap();
    }
    assert_eq!(w.config.radius, last.radius);
    w.add("center-check", 0, 1).unwrap();
    assert_eq!(w.observe(0).unwrap().arena.center, Point::default());
}
#[test]
fn finite_large_radius_does_not_overflow_contact_checks() {
    use slither_sim::{
        Point,
        boundary::{BoundaryMode, constrain},
    };
    for mode in [BoundaryMode::Hard, BoundaryMode::Sliding] {
        for radius in [1e200, f64::MAX / 2.0] {
            for fraction in [0.5, 0.9, 1.0, 1.1] {
                let point = Point {
                    x: radius * fraction,
                    y: 0.0,
                };
                let (head, lethal) = constrain(point, 0.0, 1.0, radius, mode);
                assert_eq!(head, point);
                assert_eq!(lethal, fraction >= 1.0);
            }
        }
    }
}

/// Slide a boosting minimum-size head with a fixed heading into the rim at a
/// shallow angle, for ticks that keep the contact well below the steep limit.
fn shallow_boost_slide_survives(dt: f64) -> bool {
    use std::f64::consts::FRAC_PI_2;
    let radius = 8000.;
    let mut head = Point {
        x: radius - 30.,
        y: 0.,
    };
    let heading = FRAC_PI_2 - 0.6;
    for _ in 0..30 {
        let step = physics::BOOST_SPEED * dt;
        let proposed = Point {
            x: head.x + step * heading.cos(),
            y: head.y + step * heading.sin(),
        };
        let (next, dead) = boundary::constrain(
            proposed,
            heading,
            physics::MIN_SCALE,
            radius,
            BoundaryMode::Sliding,
        );
        if dead {
            return false;
        }
        head = next;
    }
    true
}

#[test]
fn sliding_steps_are_bounded_by_the_contact_lead_not_a_literal() {
    let limit = boundary::CONTACT_LEAD * physics::MIN_SCALE / physics::BOOST_SPEED;
    assert!((limit - 16. / 437.5).abs() < 1e-15);
    let config = |dt, boundary_mode| Config {
        dt,
        boundary_mode,
        ..Config::sandbox()
    };
    let below = limit * 0.999;
    let beyond = limit * 1.001;
    config(below, BoundaryMode::Sliding).validate().unwrap();
    assert!(shallow_boost_slide_survives(below));
    let error = config(limit, BoundaryMode::Sliding).validate().unwrap_err();
    assert!(error.contains("contact"), "{error}");
    assert!(config(beyond, BoundaryMode::Sliding).validate().is_err());
    assert!(!shallow_boost_slide_survives(beyond));
    // Hard boundaries kill on contact anyway, so longer steps stay valid.
    config(0.1, BoundaryMode::Hard).validate().unwrap();
}

#[test]
fn hard_steps_are_bounded_by_exact_tick_counts() {
    // Prey substeps per tick must be an exact f64 integer that fits usize.
    let exact = 2f64.powi(53).min(usize::MAX as f64);
    let limit = physics::LOOP_SECONDS * exact;
    let error = long_config(1e308).validate().unwrap_err();
    assert!(error.contains("substep"), "{error}");
    assert!(long_config(limit * 1.001).validate().is_err());
    // One tick just inside the bound keeps the head finite.
    let mut world = Simulator::new(1, long_config(limit * 0.999)).unwrap();
    world.add("a", 0, 1).unwrap();
    world.step(&[Some(Action::default())]).unwrap();
    let head = world.snakes[0].head;
    assert!(head.x.is_finite() && head.y.is_finite());
    assert!(head.distance(Point::default()) > 1e15);
}

fn long_config(dt: f64) -> Config {
    Config {
        dt,
        boundary_mode: BoundaryMode::Hard,
        food_count: 0,
        ambient_replenish_per_second: 0.,
        prey: PreyConfig {
            count: 0,
            ..PreyConfig::default()
        },
        ..Config::sandbox()
    }
}

/// A boosting snake of `segments` respawned at the center, after one step.
fn boosted(dt: f64, segments: usize) -> Simulator {
    let mut config = long_config(dt);
    config.radius = 1e6;
    config.boost_food.enabled = false;
    let mut world = Simulator::new(1, config).unwrap();
    world.add("a", 0, 1).unwrap();
    world.snakes[0].alive = false;
    world
        .respawn_body(0, vec![Point::default(); segments], 0.)
        .unwrap();
    world
        .step(&[Some(Action {
            heading: 0.,
            boost: true,
        })])
        .unwrap();
    world
}

#[test]
fn long_boost_steps_count_their_drops() {
    // A 430-segment boost costs no mass, and at dt 1e16 subtracting the 0.155 s
    // drop period no longer changed the clock, so the drop loop never ended.
    let error = long_config(1e16).validate().unwrap_err();
    assert!(error.contains("exact integer"), "{error}");
    let world = boosted(100., 430);
    assert!(world.time == 100.);
    // A drop is due at once and then every 0.155 s: floor((0.155 + 1) / 0.155).
    let world = boosted(1., 100);
    let drops = world
        .food
        .iter()
        .filter(|food| food.origin == Some(Origin::Boost))
        .count();
    assert_eq!(drops, 7);
}

#[test]
fn reviewed_construction_keeps_the_configured_population_radius_until_a_tick() {
    let config = Config {
        food_count: 0,
        ambient_replenish_per_second: 0.,
        ..Config::official(500)
    };
    let radius = config.radius;
    let mut world = Simulator::new(1, config).unwrap();
    assert_eq!(world.time, 0.);
    assert_eq!(world.config.radius, radius);
    world.step(&[]).unwrap();
    assert!(world.config.radius < radius);
}
