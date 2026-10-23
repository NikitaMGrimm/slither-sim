use slither_sim::{profiling::StepProfile, *};

fn entity(x: f64, speed: f64) -> Prey {
    Prey {
        id: 100,
        position: Point { x, y: 0. },
        heading: 0.,
        target_heading: 0.,
        speed,
        size: 6.,
    }
}
fn world() -> Simulator {
    let mut w = Simulator::new(
        42,
        Config {
            food_count: 0,
            ..Config::sandbox()
        },
    )
    .unwrap();
    w.add("snake", 0, 1).unwrap();
    w.snakes[0].head = Point::default();
    w.snakes[0].heading = 0.;
    w.snakes[0].body = vec![Point::default(), Point { x: -42., y: 0. }];
    w
}
/// Synthetic prey speeds, distinct from the measured defaults, so mechanism tests
/// follow their inputs. One 7 ms tick moves 0.875, 1.54 and 2.66 units.
fn synthetic_prey() -> PreyConfig {
    PreyConfig {
        cruise_speed: 125.,
        escape_speed: 220.,
        sprint_speed: 380.,
        ..PreyConfig::default()
    }
}
#[test]
fn spawned_prey_respect_positive_representable_size_bounds() {
    for (size, max_size) in [
        (6.01, 6.39),
        (0.01, 0.39),
        (6.0, 9.2),
        (6.6000000000000005, 6.8),
        (6.2, 6.599999999999999),
    ] {
        let world = Simulator::new(
            7,
            Config {
                food_count: 0,
                prey: PreyConfig {
                    count: 100,
                    size,
                    max_size,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(world.prey.len(), 100);
        for prey in world.prey {
            assert!(prey.size >= size && prey.size <= max_size && prey.size > 0.0);
            assert!((prey.size * 5.0 - (prey.size * 5.0).round()).abs() < 1e-12);
        }
    }
    for (size, max_size) in [
        (9.21, 9.39),
        (0.01, 0.02),
        (0.21, 0.21),
        (1e308, 1e308),
        (6.6000000000000005, 6.6000000000000005),
    ] {
        assert!(
            Simulator::new(
                7,
                Config {
                    prey: PreyConfig {
                        size,
                        max_size,
                        ..Default::default()
                    },
                    ..Default::default()
                }
            )
            .is_err()
        );
    }
}

#[test]
fn client_prey_units_and_turn_limit() {
    // Public client: mamu2=0.029 radians per 8ms; movement sp*vfr/4.
    let mut p = entity(0., 3.8 / 0.032);
    p.advance(0.008, 0.029 / 0.008);
    assert!((p.position.x - 0.95).abs() < 1e-12);
    p.target_heading = 1.;
    p.advance(0.008, 0.029 / 0.008);
    assert!((p.heading - 0.029).abs() < 1e-12);
    p.target_heading = 0.03;
    p.advance(0.008, 0.029 / 0.008);
    assert!((p.heading - 0.03).abs() < 1e-12);
}

#[test]
fn invalid_runtime_prey_ranges_reject_steps_without_mutation() {
    for profiled in [false, true] {
        let config = Config {
            food_count: 0,
            dt: 0.01,
            prey: PreyConfig {
                count: 1,
                respawn_seconds: 0.01,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut world = Simulator::new(7, config.clone()).unwrap();
        let mut reference = Simulator::new(7, config.clone()).unwrap();
        world.prey.clear();
        reference.prey.clear();
        world.config.prey.size = 9.21;
        world.config.prey.max_size = 9.39;
        let before = serde_json::to_value(&world).unwrap();
        assert!(world.validate_actions(&[]).is_err());
        let mut profile = StepProfile::default();
        let result = if profiled {
            world.step_profiled(&[], &mut profile)
        } else {
            world.step(&[])
        };
        assert_eq!(result.unwrap_err(), "Invalid prey settings");
        assert_eq!(serde_json::to_value(&world).unwrap(), before);
        assert_eq!(profile.ticks, 0);

        world.config.prey = config.prey;
        world.step(&[]).unwrap();
        reference.step(&[]).unwrap();
        assert_eq!(world.prey.len(), 1);
        assert_eq!(
            serde_json::to_value(&world).unwrap(),
            serde_json::to_value(&reference).unwrap()
        );
    }
}

#[test]
fn authoritative_prey_rounds_coordinates_down_on_each_server_tick() {
    let mut w = Simulator::new(
        42,
        Config {
            radius: 10000.,
            food_count: 0,
            ..Config::sandbox()
        },
    )
    .unwrap();
    let mut p = entity(100.4, 3.8 / 0.032);
    p.position.y = 300.2;
    p.heading = 0.25;
    p.target_heading = 0.25;
    w.prey.push(p);
    for _ in 0..14 {
        w.step(&[]).unwrap();
    }
    // Both positive increments are below one unit. Wire prey can remain here
    // while still advertising speed 3.8 to the stock client's predictor.
    assert_eq!(w.prey[0].position, Point { x: 101., y: 300. });
    w.prey[0].heading += std::f64::consts::PI;
    w.prey[0].target_heading = w.prey[0].heading;
    for _ in 0..14 {
        w.step(&[]).unwrap();
    }
    assert_eq!(w.prey[0].position, Point { x: 87., y: 286. });
}

#[test]
fn coarse_simulation_steps_preserve_prey_coordinate_quantization() {
    let run = |dt, steps| {
        let mut w = Simulator::new(
            42,
            Config {
                radius: 10000.,
                food_count: 0,
                dt,
                ..Config::sandbox()
            },
        )
        .unwrap();
        let mut p = entity(100., 3.8 / 0.032);
        p.heading = std::f64::consts::PI + 0.25;
        p.target_heading = p.heading;
        w.prey.push(p);
        for _ in 0..steps {
            w.step(&[]).unwrap();
        }
        w.prey[0].position
    };
    assert_eq!(run(0.007, 32), Point { x: 68., y: -32. });
    assert_eq!(run(0.014, 16), run(0.007, 32));
    assert_eq!(run(0.032, 7), run(0.007, 32));
}

#[test]
fn authoritative_prey_motion_matches_recorded_packet_intervals() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/prey-motion-quantization.json")).unwrap();
    for case in fixture["cases"].as_array().unwrap() {
        let starting = Point {
            x: case["source_x"].as_f64().unwrap(),
            y: case["source_y"].as_f64().unwrap(),
        };
        let mut prey = Prey {
            id: case["prey_id"].as_u64().unwrap(),
            position: starting,
            heading: case["heading"].as_f64().unwrap(),
            target_heading: case["target_heading"].as_f64().unwrap(),
            speed: case["speed_protocol"].as_f64().unwrap() / 0.032,
            size: 6.0,
        };
        let elapsed_ms =
            case["server_end_ms"].as_i64().unwrap() - case["server_start_ms"].as_i64().unwrap();
        let ticks = (elapsed_ms as f64 / 7.0).round() as usize;
        for _ in 0..ticks {
            prey.advance_server_tick(0.029 / 0.008);
        }
        let observed = Point {
            x: starting.x + case["observed_dx"].as_f64().unwrap(),
            y: starting.y + case["observed_dy"].as_f64().unwrap(),
        };
        let tolerance = case["tolerance"].as_f64().unwrap();
        assert!(
            (prey.position.x - observed.x).abs() <= tolerance
                && (prey.position.y - observed.y).abs() <= tolerance,
            "{} conn {} prey {} at {}: predicted {:?}, observed {:?}",
            case["archive"].as_str().unwrap(),
            case["connection"],
            case["prey_id"],
            case["server_end_ms"],
            prey.position,
            observed
        );
    }
}
#[test]
fn later_prey_injection_keeps_the_server_tick_phase() {
    let mut w = Simulator::new(
        42,
        Config {
            radius: 10000.,
            food_count: 0,
            dt: 0.005,
            ..Config::sandbox()
        },
    )
    .unwrap();
    w.step(&[]).unwrap(); // No prey during the first 5 ms.
    let mut p = entity(100., 3.8 / 0.032);
    p.heading = std::f64::consts::PI + 0.25;
    p.target_heading = p.heading;
    w.prey.push(p);
    w.step(&[]).unwrap(); // The global 7 ms tick falls within this step.
    assert_eq!(w.prey[0].position, Point { x: 99., y: -1. });
    w.step(&[]).unwrap(); // The next tick is at 14 ms.
    assert_eq!(w.prey[0].position, Point { x: 98., y: -2. });
    w.step(&[]).unwrap(); // No tick in (15, 20] ms.
    assert_eq!(w.prey[0].position, Point { x: 98., y: -2. });
}

#[test]
fn capture_is_once_only_adds_nutrition_and_waits_for_respawn() {
    let mut w = world();
    w.config.prey.count = 1;
    // Captures happen in the pickup circle ~43 units ahead of a small snake's head.
    w.prey.push(entity(43., 120.));
    w.step(&[Some(Action::default())]).unwrap();
    assert!(w.prey.is_empty());
    assert!(w.food.is_empty());
    assert!(
        (w.snakes[0].fam - physics::nutrition(6., 2.0) * w.config.prey.nutrition_factor).abs()
            < 1e-12
    );
    w.snakes[0].alive = false;
    // Respawn waits one simulated second.
    for _ in 0..150 {
        w.step(&[None]).unwrap();
    }
    assert_eq!(w.prey.len(), 1);
    assert_ne!(w.prey[0].id, 100);
}
#[test]
fn measured_prey_nutrition_matches_clean_catch_and_nonboost_food_controls() {
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/prey-nutrition.json")).unwrap();
    for case in evidence["cases"].as_array().unwrap() {
        let n = case["segments"].as_u64().unwrap() as usize;
        let fam = case["fraction"].as_f64().unwrap();
        let mut w = world();
        w.snakes[0].segments = n;
        w.snakes[0].fam = fam;
        w.snakes[0].body = (0..n)
            .map(|i| Point {
                x: -(i as f64) * 42.,
                y: 0.,
            })
            .collect();
        Simulator::update_mass(&mut w.snakes[0]);
        let mut p = entity(43., 120.);
        p.size = case["size"].as_f64().unwrap();
        w.prey.push(p);
        w.step(&[Some(Action::default())]).unwrap();
        assert!(w.prey.is_empty());
        let prey_gain = w.snakes[0].segments as f64 + w.snakes[0].fam - n as f64 - fam;
        // Concurrent food is only an estimate at initial mass, not an exact replay.
        let food_estimate: f64 = case["food_sizes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|size| physics::nutrition(size.as_f64().unwrap(), n as f64 + fam))
            .sum();
        let observed = case["gain"].as_f64().unwrap();
        let relative_error = ((prey_gain + food_estimate) / observed - 1.).abs();
        assert!(relative_error < 0.01, "{}: {relative_error}", case["time"]);
    }
}
#[test]
fn relative_sweep_catches_fast_crossing_and_dead_snakes_cannot_eat() {
    let mut w = world();
    w.config.prey.flee_half_width = 1.; // Isolate capture from evasive turning.
    w.config.prey.cruise_speed = 20000.;
    w.config.prey.escape_speed = 20000.;
    w.config.prey.sprint_speed = 20000.;
    w.prey.push(entity(-80., 20000.));
    w.step(&[Some(Action::default())]).unwrap();
    assert!(w.prey.is_empty());
    w.snakes[0].alive = false;
    w.prey.push(entity(w.snakes[0].head.x, 120.));
    w.step(&[None]).unwrap();
    assert_eq!(w.prey.len(), 1);
}
#[test]
fn threats_set_prey_flight_speed_and_unthreatened_prey_wanders_deterministically() {
    let mut w = world();
    w.config.prey = synthetic_prey();
    w.prey.push(entity(200., 120.));
    w.step(&[Some(Action::default())]).unwrap();
    assert_eq!(w.prey[0].speed, w.config.prey.escape_speed);
    assert_eq!(w.prey[0].position.x, 201.);
    w.snakes[0].alive = false;
    let mut other = w.clone();
    for _ in 0..1000 {
        w.step(&[None]).unwrap();
        other.step(&[None]).unwrap();
    }
    assert_eq!(w.prey, other.prey);
    assert!(w.prey[0].heading.abs() > 0.01);
    assert!(w.prey[0].position.distance(Point::default()) < w.config.radius);
}
#[test]
fn prey_speed_modes_apply_before_the_next_motion_tick() {
    let p = synthetic_prey();
    for (old_speed, live_threat, boosting, expected_speed, expected_dx) in [
        (p.cruise_speed, true, false, p.escape_speed, 1.),
        (p.cruise_speed, true, true, p.sprint_speed, 2.),
        (p.sprint_speed, true, false, p.escape_speed, 1.),
        (p.escape_speed, false, false, p.cruise_speed, 0.),
        (p.sprint_speed, false, false, p.cruise_speed, 0.),
    ] {
        let mut w = world();
        w.config.prey = p.clone();
        w.snakes[0].alive = live_threat;
        w.snakes[0].segments = if boosting { 50 } else { 2 };
        w.snakes[0].fam = 0.5;
        Simulator::update_mass(&mut w.snakes[0]);
        w.snakes[0].speed = if boosting { 14. / 0.032 } else { 5.79 / 0.032 };
        w.prey.push(entity(200., old_speed));
        w.step(&[Some(Action {
            heading: 0.,
            boost: boosting,
        })])
        .unwrap();
        assert_eq!(w.prey.len(), 1);
        assert_eq!(w.prey[0].speed, expected_speed);
        assert_eq!(w.prey[0].position.x, 200. + expected_dx);
    }
}

#[test]
fn prey_flights_use_ordinary_speed_unless_the_threat_is_boosting() {
    let p = PreyConfig {
        flee_half_width: 2000.,
        ..synthetic_prey()
    };
    let run = |boost, cruise: f64| {
        let mut w = world();
        w.config.prey = p.clone();
        w.snakes[0].segments = 50;
        w.snakes[0].fam = 0.5;
        w.snakes[0].speed = cruise;
        w.initialize_speed_state(0, Some(0.0)).unwrap();
        w.snakes[0].body = (0..50)
            .map(|i| Point {
                x: i as f64 * 42.,
                y: 0.,
            })
            .collect();
        w.snakes[0].heading = std::f64::consts::PI;
        Simulator::update_mass(&mut w.snakes[0]);
        w.prey.push(entity(200., p.cruise_speed));
        for _ in 0..200 {
            w.step(&[Some(Action {
                heading: std::f64::consts::PI,
                boost,
            })])
            .unwrap();
        }
        assert_eq!(w.prey.len(), 1);
        w.prey[0].speed
    };
    assert!(
        (run(false, physics::base_speed(50)) - p.flight_speed(physics::base_speed(50))).abs()
            < 1e-9
    );
    // Idle growth retains the old physical cruise; prey responds to actual speed.
    assert!(
        (run(false, physics::base_speed(2)) - p.flight_speed(physics::base_speed(2))).abs() < 1e-9
    );
    assert!((run(true, physics::base_speed(50)) - p.sprint_speed).abs() < 1e-9);
}
#[test]
fn flight_speed_interpolates_and_clamps_configured_endpoints() {
    // Snake protocol speeds 5.79 and 14 anchor the interpolation.
    let p = synthetic_prey();
    assert!((p.flight_speed(5.79 / 0.032) - p.escape_speed).abs() < 1e-12);
    assert!((p.flight_speed(14. / 0.032) - p.sprint_speed).abs() < 1e-12);
    let midpoint = ((5.79 + 14.) / 2.) / 0.032;
    assert!((p.flight_speed(midpoint) - (p.escape_speed + p.sprint_speed) / 2.).abs() < 1e-12);
    assert_eq!(p.flight_speed(0.), p.escape_speed);
    assert_eq!(p.flight_speed(20. / 0.032), p.sprint_speed);
}
#[test]
fn interpolated_flight_speed_matches_raw_official_packet_fixture() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/prey-flight-speed-fresh.json")).unwrap();
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 13);
    assert_eq!(
        fixture["provenance"][0]["sha256"].as_str().unwrap(),
        "282834351bce3a5e9c5bdf342b179a6ea1634eacca450b98c7924a52c7cf669a"
    );
    let p = PreyConfig::default();
    // The first packets in the slowest and full-boost bands calibrate the endpoints.
    for (name, index, snake_speed) in [("low", 0, 5.79), ("high", 5, 14.)] {
        let case = &fixture["cases"][index];
        assert_eq!(case["snake_speed"].as_f64().unwrap(), snake_speed, "{name}");
        let observed = case["prey_speed"].as_f64().unwrap();
        assert!(
            (p.flight_speed(snake_speed / 0.032) * 0.032 - observed).abs() < 1e-12,
            "{name} endpoint"
        );
    }
    let errors: Vec<_> = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            let snake_speed = case["snake_speed"].as_f64().unwrap();
            let observed = case["prey_speed"].as_f64().unwrap();
            (p.flight_speed(snake_speed / 0.032) * 0.032 - observed).abs()
        })
        .collect();
    assert!(errors.iter().sum::<f64>() / (errors.len() as f64) < 0.15);
    assert!(errors.iter().filter(|error| **error < 0.25).count() >= 12);
}
#[test]
fn raw_visibility_keeps_moving_entities_separate_from_food() {
    let mut w = world();
    w.config.vision = 500.;
    w.prey.extend([entity(100., 120.), entity(600., 120.)]);
    let o = w.observe(0).unwrap();
    assert_eq!(o.prey.len(), 1);
    assert!(o.food.is_empty());
}
#[test]
fn prey_randomness_does_not_perturb_ambient_food_and_profile_is_passive() {
    let plain = Simulator::new(21, Config::sandbox()).unwrap();
    let mut enabled = Simulator::new(
        21,
        Config {
            prey: PreyConfig {
                count: 3,
                ..PreyConfig::default()
            },
            ..Config::sandbox()
        },
    )
    .unwrap();
    assert_eq!(plain.food, enabled.food);
    let mut copy = enabled.clone();
    let mut profile = StepProfile::default();
    for _ in 0..200 {
        enabled.step_profiled(&[], &mut profile).unwrap();
        copy.step(&[]).unwrap();
    }
    assert_eq!(enabled.prey, copy.prey);
    assert_eq!(enabled.food, copy.food);
    assert!(profile.prey_seconds > 0.);
}
#[test]
fn invalid_prey_configuration_is_rejected() {
    for p in [
        PreyConfig {
            size: f64::NAN,
            ..PreyConfig::default()
        },
        PreyConfig {
            count: 10001,
            ..PreyConfig::default()
        },
        PreyConfig {
            sprint_speed: 0.,
            ..PreyConfig::default()
        },
        PreyConfig {
            escape_speed: 400.,
            ..PreyConfig::default()
        },
    ] {
        assert!(
            Simulator::new(
                1,
                Config {
                    prey: p,
                    ..Config::sandbox()
                }
            )
            .is_err()
        );
    }
}
