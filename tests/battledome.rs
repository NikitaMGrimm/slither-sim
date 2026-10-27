//! The Battledome mode rules and the general settings behind them: `pit`,
//! `spawnRadius`, `killScoreSeconds` and `minScore`. Worlds here are synthetic setups.
mod support;
use slither_sim::physics::{MAX_SEGMENTS, MassFloor, score};
use slither_sim::{Action, Config, DeathReason, PitConfig, Point, Preset, Simulator, spawn};
use slither_sim::{Constraint, ErrorKind};
use std::f64::consts::{FRAC_PI_2, PI};

/// A food-free arena of radius 1000 with a pit of radius 200.
fn pit_world() -> Simulator {
    let mut config = Config::sandbox();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    config.pit = Some(PitConfig { radius: 200.0 });
    Simulator::new(slither_sim::WorldOptions {
        seed: 3,
        config,
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap()
}

fn place(world: &mut Simulator, slot: usize, head: Point, heading: f64) {
    world.retire(slot).unwrap();
    world.respawn(slot, head, heading, 2).unwrap();
}

fn straight(world: &Simulator) -> Vec<Option<Action>> {
    world
        .snakes()
        .iter()
        .map(|s| {
            s.alive.then_some(Action {
                heading: s.heading,
                boost: false,
            })
        })
        .collect()
}

fn distance(p: Point) -> f64 {
    p.distance(Point::default())
}

#[test]
fn entering_the_pit_is_allowed_and_leaving_it_kills_with_death_food() {
    let mut world = pit_world();
    support::ring_slot(&mut world, "diver", 0, 1).unwrap();
    place(&mut world, 0, Point { x: 300.0, y: 0.0 }, PI);
    let mut inside = 0;
    for _ in 0..2000 {
        if !world.snakes()[0].alive {
            break;
        }
        world.step(&straight(&world)).unwrap();
        if world.snakes()[0].alive && distance(world.snakes()[0].head) < 200.0 {
            inside += 1;
        }
    }
    assert!(inside > 100, "{inside} ticks inside");
    let s = &world.snakes()[0];
    assert!(!s.alive && s.head.x < 0.0 && distance(s.head) >= 200.0);
    assert_eq!(world.deaths().len(), 1);
    let death = &world.deaths()[0];
    assert_eq!(
        (death.reason, death.killer.as_deref()),
        (DeathReason::Pit, None)
    );
    assert!(death.food > 0);
    assert_eq!(world.food().len(), death.food);
}

#[test]
fn a_snake_that_never_entered_cannot_die_at_the_pit() {
    let mut world = pit_world();
    support::ring_slot(&mut world, "edge", 0, 2).unwrap();
    support::ring_slot(&mut world, "orbit", 1, 2).unwrap();
    // Exactly on the pit radius is not inside; moving outward never enters.
    place(&mut world, 0, Point { x: 200.0, y: 0.0 }, 0.0);
    place(&mut world, 1, Point { x: 0.0, y: -250.0 }, 0.0);
    for _ in 0..300 {
        world.step(&straight(&world)).unwrap();
    }
    assert!(
        world.snakes().iter().all(|s| s.alive),
        "{:?}",
        world.deaths()
    );
}

#[test]
fn crossing_the_pit_within_one_tick_kills() {
    // A synthetic pit of radius 1 that an unboosted head crosses in one 32 ms tick.
    let mut config = pit_world().config().clone();
    config.pit = Some(PitConfig { radius: 1.0 });
    config.dt = 0.032;
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 3,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["diver".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    place(&mut world, 0, Point { x: -2.0, y: 0.0 }, 0.0);
    world.step(&straight(&world)).unwrap();
    let s = &world.snakes()[0];
    assert!(!s.alive && s.head.x > 1.0, "{:?}", s.head);
    assert_eq!(world.deaths()[0].reason, DeathReason::Pit);
}

#[test]
fn a_tiny_pit_crossing_does_not_underflow() {
    // Synthetic scales: this path's squared length underflows to zero.
    let mut config = pit_world().config().clone();
    config.pit = Some(PitConfig { radius: 1e-175 });
    config.dt = 2e-170 / slither_sim::physics::base_speed(2);
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 3,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["speck".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    place(&mut world, 0, Point { x: -1e-170, y: 0.0 }, 0.0);
    world.step(&straight(&world)).unwrap();
    let s = &world.snakes()[0];
    assert!(!s.alive && s.head.x > 1e-175, "{:?}", s.head);
    assert_eq!(world.deaths()[0].reason, DeathReason::Pit);
}

#[test]
fn an_outward_step_from_the_pit_edge_does_not_enter() {
    // The start lies just outside the pit; rescaling it put it just inside.
    let mut world = pit_world();
    support::ring_slot(&mut world, "edge", 0, 1).unwrap();
    let start = Point {
        x: 198.7222621040017,
        y: 22.57127697469633,
    };
    assert!(distance(start) >= 200.0);
    place(&mut world, 0, start, 0.11309733552923254);
    world.step(&straight(&world)).unwrap();
    assert!(world.snakes()[0].alive, "{:?}", world.deaths());
    assert!(distance(world.snakes()[0].head) > distance(start));
}

#[test]
fn clipping_the_pit_edge_within_one_tick_kills() {
    // A chord near the rim of the Battledome pit: both ends lie outside it.
    let mut config = Config::battledome();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    config.min_score = None;
    config.dt = 0.032;
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 3,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["grazer".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    let start = Point {
        x: -2.9,
        y: 1783.999,
    };
    place(&mut world, 0, start, 0.0);
    world.step(&straight(&world)).unwrap();
    let end = world.snakes()[0].head;
    assert!(
        distance(start) >= 1784.0 && distance(end) >= 1784.0,
        "{end:?}"
    );
    assert!(end.x > 2.0);
    assert!(!world.snakes()[0].alive);
    assert_eq!(world.deaths()[0].reason, DeathReason::Pit);
}

#[test]
fn a_life_starting_inside_the_pit_dies_on_leaving() {
    let mut world = pit_world();
    support::ring_slot(&mut world, "inside", 0, 1).unwrap();
    place(&mut world, 0, Point { x: 199.0, y: 0.0 }, 0.0);
    for _ in 0..10 {
        if world.snakes()[0].alive {
            world.step(&straight(&world)).unwrap();
        }
    }
    assert!(!world.snakes()[0].alive);
    assert_eq!(world.deaths()[0].reason, DeathReason::Pit);
}

#[test]
fn respawning_clears_the_pit_state() {
    let mut world = pit_world();
    support::ring_slot(&mut world, "diver", 0, 1).unwrap();
    place(&mut world, 0, Point { x: 210.0, y: 0.0 }, PI);
    while distance(world.snakes()[0].head) >= 150.0 {
        world.step(&straight(&world)).unwrap();
    }
    // A new life outside the pit does not inherit the old life's entry.
    place(&mut world, 0, Point { x: 300.0, y: 0.0 }, 0.0);
    for _ in 0..100 {
        world.step(&straight(&world)).unwrap();
    }
    assert!(world.snakes()[0].alive && world.deaths().is_empty());
}

#[test]
fn new_lives_start_on_the_configured_ring_outside_the_pit() {
    let config = Config::battledome();
    let ring = config.spawn_radius.unwrap();
    assert_eq!(ring, (1784.0 + 3584.0) / 2.0);
    for unit in [0.0, 0.3, 0.999] {
        let radius = spawn::spawn_distance(&config, config.radius, unit);
        assert!(
            (radius * radius - (ring * ring + unit * (3400.0_f64.powi(2) - ring * ring))).abs()
                < 1e-8
        );
        assert_eq!(
            spawn::spawn_distance(&Config::official(4), Config::official(4).radius, unit),
            spawn::sample_spawn_distance(Config::official(4).radius, unit)
        );
    }
    let world = Simulator::new(slither_sim::WorldOptions {
        seed: 1,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: (0..8).map(|slot| slot.to_string()).collect(),
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    for slot in 0..8 {
        assert_on_ring(&world.snakes()[slot], ring, 26);
        assert_eq!(world.snakes()[slot].score, 400.0);
    }
    // Without a floor, a ring life is two segments along the ring.
    let mut config = Config::sandbox();
    config.spawn_radius = Some(500.0);
    let world = Simulator::new(slither_sim::WorldOptions {
        seed: 1,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["small".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    assert_on_ring(&world.snakes()[0], 500.0, 2);
    assert_eq!(world.snakes()[0].score, 10.0);
}

/// A ring life: `segments` anchors 42 apart on the spawn circle, heading along it.
fn assert_on_ring(s: &slither_sim::Snake, ring: f64, segments: usize) {
    assert_eq!((s.segments, s.body.len()), (segments, segments));
    assert_eq!(s.body[0], s.head);
    for pair in s.body.windows(2) {
        assert!((pair[0].distance(pair[1]) - 42.0).abs() < 1e-9);
    }
    assert!(s.body.iter().all(|&p| (distance(p) - ring).abs() < 1e-9));
    let radial = s.head.y.atan2(s.head.x);
    assert!((s.heading - radial).cos().abs() < 1e-12, "tangent heading");
    // The body trails the head, against the heading.
    let back = Point {
        x: s.body[1].x - s.head.x,
        y: s.body[1].y - s.head.y,
    };
    assert!(back.x * s.heading.cos() + back.y * s.heading.sin() < 0.0);
}

#[cfg(feature = "networking")]
#[test]
fn hosted_lives_start_on_the_ring_at_varied_angles() {
    use slither_sim::client::{SimulationConfig, SimulatorBackend};
    let config = SimulationConfig {
        agents: 3,
        world: Config::battledome(),
        ..Default::default()
    };
    let mut backend = SimulatorBackend::new(5, config).unwrap();
    let mut angles = Vec::new();
    for slot in [0, 1, 2, 0, 1, 2] {
        backend.reconnect_slot(slot).unwrap();
        let snake = &backend.world().snakes()[slot];
        let radius = distance(snake.head);
        assert!((2684.0..=3400.0).contains(&radius));
        assert_on_ring(snake, radius, 26);
        angles.push(snake.head.y.atan2(snake.head.x));
    }
    angles.sort_by(f64::total_cmp);
    angles.dedup();
    assert_eq!(angles.len(), 6);
}

#[cfg(feature = "networking")]
#[test]
fn reconnect_on_the_old_rounding_sensitive_ring_preserves_a_living_slot() {
    use slither_sim::client::{SimulationConfig, SimulatorBackend};
    let mut world = Config::sandbox();
    world.food_count = 0;
    world.ambient_replenish_per_second = 0.0;
    world.kiosk.enabled = false;
    world.min_score = Some(400);
    world.spawn_radius = Some(174.2208260217339);
    world.validate().unwrap();
    let mut backend = SimulatorBackend::new(
        5,
        SimulationConfig {
            world,
            agents: 1,
            ..Default::default()
        },
    )
    .unwrap();
    for _ in 0..30 {
        assert!(backend.world().snakes()[0].alive);
        backend.reconnect_slot(0).unwrap();
        assert_on_ring(&backend.world().snakes()[0], 174.2208260217339, 26);
    }
}

#[test]
fn ring_validation_reserves_the_floor_body_diameter_at_closure() {
    let mut config = Config::sandbox();
    config.radius = 3000.0;
    config.spawn_radius = Some(2869.0);
    config.min_score = Some(18_436_960);
    assert!(config.validate().is_err());
    // The reported floor has a diameter of about 146, rather than 42.
    config.radius = 4000.0;
    config.spawn_radius = Some(3000.0);
    let world = Simulator::new(slither_sim::WorldOptions {
        seed: 1,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["wide".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    let snake = &world.snakes()[0];
    assert_eq!(snake.segments, 429);
    assert!(snake.head.distance(*snake.body.last().unwrap()) > 2.0 * snake.radius);
}

/// Independent center-to-segment projection, including the endpoints.
fn chord_radius(a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let t = (-(a.x * dx + a.y * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
    distance(Point {
        x: a.x + t * dx,
        y: a.y + t * dy,
    })
}

#[test]
fn ring_validation_keeps_complete_body_chords_outside_the_pit() {
    let mut config = Config::sandbox();
    config.min_score = Some(400);
    config.spawn_radius = Some(200.0);
    config.pit = Some(PitConfig { radius: 199.5 });
    assert!(config.validate().is_err());
    // Find the first accepted radius and check actual generated chords, not
    // just anchors or a restatement of the validation inequality.
    let (mut lower, mut upper) = (200.0, 201.0);
    for _ in 0..64 {
        let middle = (lower + upper) / 2.0;
        config.spawn_radius = Some(middle);
        if config.validate().is_ok() {
            upper = middle;
        } else {
            lower = middle;
        }
    }
    config.spawn_radius = Some(upper);
    let world = Simulator::new(slither_sim::WorldOptions {
        seed: 1,
        config,
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap();
    for i in 0..1024 {
        let angle = std::f64::consts::TAU * f64::from(i) / 1024.0;
        let head = Point {
            x: upper * angle.cos(),
            y: upper * angle.sin(),
        };
        for turn in [-FRAC_PI_2, FRAC_PI_2] {
            let (body, _) = world.spawn_body(head, angle + turn).unwrap();
            for edge in body.windows(2) {
                assert!(chord_radius(edge[0], edge[1]) > 199.5, "{angle}: {edge:?}");
            }
        }
    }
}

#[test]
fn the_smallest_validated_floor_rings_spawn_at_every_sampled_angle() {
    for score in [10, 400, *MassFloor::score_range().end()] {
        let mut config = Config::sandbox();
        config.radius = 30_000.0;
        config.min_score = Some(score);
        let (mut lower, mut upper) = (21.0, 15_000.0);
        for _ in 0..64 {
            let middle = (lower + upper) / 2.0;
            config.spawn_radius = Some(middle);
            if config.validate().is_ok() {
                upper = middle;
            } else {
                lower = middle;
            }
        }
        config.spawn_radius = Some(upper);
        let world = Simulator::new(slither_sim::WorldOptions {
            seed: 1,
            config,
            population: slither_sim::InitialPopulation::Empty,
        })
        .unwrap();
        let floor = MassFloor::for_score(score).unwrap();
        let diameter =
            2.0 * slither_sim::physics::BODY_RADIUS * slither_sim::physics::scale(floor.segments);
        for i in 0..2048 {
            let angle = std::f64::consts::TAU * f64::from(i) / 2048.0;
            let head = Point {
                x: upper * angle.cos(),
                y: upper * angle.sin(),
            };
            for turn in [-FRAC_PI_2, FRAC_PI_2] {
                let (body, heading) = world.spawn_body(head, angle + turn).unwrap();
                let tail = *body.last().unwrap();
                assert!(head.distance(tail) > diameter, "{score}: {angle}");
                let tip = slither_sim::physics::head_tip(head, heading, floor.segments);
                assert!(tip.distance(tail) > diameter / 2.0);
                assert!(world.body_is_clear(&body, 600.0));
            }
        }
    }
}

#[test]
fn the_largest_validated_floor_rings_spawn_at_every_sampled_angle() {
    let mut config = Config::sandbox();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    config.radius = 1e15;
    config.spawn_radius = Some(937_684_932_592_310.0);
    config.min_score = Some(10);
    assert!(config.validate().is_err(), "reviewer's upper-limit repro");

    for score in [10, 400, *MassFloor::score_range().end()] {
        config.radius = 1e21;
        config.min_score = Some(score);
        let (mut lower, mut upper) = (15_000.0, 1e20);
        // Find adjacent floats at the upper numerical limit, where the
        // occupied arc becomes too small to resolve head-tail clearance.
        loop {
            let middle = lower + (upper - lower) / 2.0;
            if middle == lower || middle == upper {
                break;
            }
            config.spawn_radius = Some(middle);
            if config.validate().is_ok() {
                lower = middle;
            } else {
                upper = middle;
            }
        }
        config.spawn_radius = Some(upper);
        assert!(config.validate().is_err());
        config.spawn_radius = Some(lower);
        let world = Simulator::new(slither_sim::WorldOptions {
            seed: 1,
            config: config.clone(),
            population: slither_sim::InitialPopulation::Placed {
                ids: vec!["edge".into()],
                placement: slither_sim::Placement::TestRing,
            },
        })
        .unwrap();
        for i in 0..4096 {
            let angle = std::f64::consts::TAU * f64::from(i) / 4096.0;
            let head = Point {
                x: lower * angle.cos(),
                y: lower * angle.sin(),
            };
            for turn in [-FRAC_PI_2, FRAC_PI_2] {
                world.spawn_body(head, angle + turn).unwrap();
            }
        }
    }
}

/// A victim at the origin whose tip meets the killer's body at x = -25.
fn kill_world(window: Option<f64>) -> Simulator {
    let mut config = Config::sandbox();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    // Dyadic steps keep every time exact, so the expiry tick is exact too.
    config.dt = 1.0 / 128.0;
    config.kill_score_seconds = window;
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 7,
        config,
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap();
    support::ring_slot(&mut world, "victim", 0, 2).unwrap();
    support::ring_slot(&mut world, "killer", 1, 2).unwrap();
    let mut detached_s = world.snakes()[0].clone();
    let s = &mut detached_s;
    s.head = Point::default();
    s.body = vec![Point::default(); 30];
    s.segments = 30;
    world.correct_mass(0, s.body.clone(), s.fam).unwrap();
    world
        .correct_pose(0, s.head, s.body.clone(), s.heading)
        .unwrap();
    let mut detached_s = world.snakes()[1].clone();
    let s = &mut detached_s;
    s.head = Point { x: -25.0, y: 342.0 };
    s.heading = FRAC_PI_2;
    s.body = (0..11)
        .map(|i| Point {
            x: -25.0,
            y: 300.0 - 42.0 * i as f64,
        })
        .collect();
    s.segments = 11;
    world.correct_mass(1, s.body.clone(), s.fam).unwrap();
    world
        .correct_pose(1, s.head, s.body.clone(), s.heading)
        .unwrap();
    world
}

#[test]
fn kill_score_counts_each_kill_until_the_window_ends_across_lives() {
    let mut world = kill_world(Some(0.5));
    assert_eq!((world.kill_score(1), world.kill_score(2)), (Some(0), None));
    world.step(&straight(&world)).unwrap();
    assert_eq!(world.deaths()[0].killer.as_deref(), Some("killer"));
    let killed_at = world.time();
    assert_eq!(
        (world.kill_score(0), world.kill_score(1)),
        (Some(0), Some(1))
    );
    // A new life of the killer's slot keeps the score; per-life kills reset.
    place(&mut world, 1, Point { x: 500.0, y: 0.0 }, FRAC_PI_2);
    assert_eq!((world.snakes()[1].kills, world.kill_score(1)), (0, Some(1)));
    while world.time() < killed_at + 0.5 - world.config().dt {
        world.step(&straight(&world)).unwrap();
        assert_eq!(world.kill_score(1), Some(1));
    }
    world.step(&straight(&world)).unwrap();
    assert_eq!(world.time(), killed_at + 0.5);
    assert_eq!(world.kill_score(1), Some(0));
}

#[test]
fn kill_score_is_zero_without_a_window() {
    let mut world = kill_world(None);
    world.step(&straight(&world)).unwrap();
    assert_eq!(world.snakes()[1].kills, 1);
    assert_eq!(world.kill_score(1), Some(0));
}

#[test]
fn battledome_uses_official_ambient_density_and_runs_deterministically() {
    let config = Config::battledome();
    config.validate().unwrap();
    let official = Config::official(1);
    let sectors = std::f64::consts::PI * 3584.0 * 3584.0 / 90_000.0;
    assert_eq!(config.food_count, (7.0 * sectors) as usize);
    assert_eq!(config.ambient_replenish_per_second, 0.1024 * sectors);
    assert_eq!((config.radius, config.dynamic_radius), (3584.0, true));
    assert_eq!(config.pit, Some(PitConfig { radius: 1784.0 }));
    assert_eq!(config.kill_score_seconds, Some(900.0));
    assert_eq!(config.prey.count, 0);
    // Everything else is the official model.
    let mut expected = serde_json::to_value(&official).unwrap();
    let actual = serde_json::to_value(&config).unwrap();
    for field in [
        "radius",
        "dynamicRadius",
        "sizeToPopulation",
        "foodCount",
        "ambientRefillCountsAllFood",
        "ambientReplenishPerSecond",
        "pit",
        "spawnRadius",
        "spawnRadiusMax",
        "killScoreSeconds",
        "minScore",
    ] {
        expected[field] = actual[field].clone();
    }
    expected["prey"]["count"] = 0.into();
    assert_eq!(actual, expected);

    let run = || {
        let mut world = Simulator::new(slither_sim::WorldOptions {
            seed: 9,
            config: Config::battledome(),
            population: slither_sim::InitialPopulation::Placed {
                ids: (0..16).map(|slot| slot.to_string()).collect(),
                placement: slither_sim::Placement::TestRing,
            },
        })
        .unwrap();
        for t in 0..400 {
            let actions: Vec<_> = (0..16)
                .map(|slot| {
                    world.snakes()[slot].alive.then(|| Action {
                        heading: slot as f64 * 0.4 + t as f64 * 0.01,
                        boost: (slot + t) % 5 == 0,
                    })
                })
                .collect();
            world.step(&actions).unwrap();
        }
        assert!(world.prey().is_empty());
        serde_json::to_string(&world.checkpoint().unwrap()).unwrap()
    };
    assert_eq!(run(), run());
}

#[test]
fn presets_select_a_base_that_explicit_fields_override() {
    let parse = |text: &str| serde_json::from_str::<Config>(text);
    let value = |config: Config| serde_json::to_value(config).unwrap();
    for (name, preset) in [
        ("official", Preset::Official),
        ("sandbox", Preset::Sandbox),
        ("battledome", Preset::Battledome),
    ] {
        let parsed = parse(&format!(r#"{{"preset":"{name}"}}"#)).unwrap();
        assert_eq!(value(parsed), value(preset.config()));
    }
    assert_eq!(value(parse("{}").unwrap()), value(Config::official(1)));
    assert_eq!(
        value(Config::battledome()),
        value(Preset::Battledome.config())
    );

    let custom = parse(
        r#"{"preset":"battledome","radius":4000,"pit":{"radius":1000},"kiosk":{"enabled":false}}"#,
    )
    .unwrap();
    assert_eq!(custom.radius, 4000.0);
    assert_eq!(custom.pit, Some(PitConfig { radius: 1000.0 }));
    assert_eq!(custom.spawn_radius, Config::battledome().spawn_radius);
    // Nested objects merge, so other kiosk fields keep the preset's values.
    assert!(!custom.kiosk.enabled);
    assert_eq!(
        custom.kiosk.grace_seconds,
        Config::battledome().kiosk.grace_seconds
    );
    // An explicit radius does not rescale the preset's food.
    assert_eq!(custom.food_count, Config::battledome().food_count);
    let open = parse(r#"{"preset":"battledome","pit":null}"#).unwrap();
    assert_eq!(open.pit, None);
    open.validate().unwrap();

    for (text, error) in [
        (
            r#"{"preset":"battledome","colour":1}"#,
            "unknown field `colour`",
        ),
        (r#"{"colour":1}"#, "unknown field `colour`"),
        (
            r#"{"preset":"battledome","pit":{"radius":900,"depth":2}}"#,
            "unknown field `depth`",
        ),
        (r#"{"preset":"arena"}"#, "unknown variant `arena`"),
        (r#"{"preset":null}"#, "invalid type: null"),
    ] {
        let message = parse(text).unwrap_err().to_string();
        assert!(message.contains(error), "{text}: {message}");
    }
    // Configs pass through `serde_json::Value`, so a repeated key keeps its last value.
    let repeated = parse(
        r#"{"preset":"sandbox","radius":1000,"radius":2000,"kiosk":{"enabled":true,"enabled":false}}"#,
    )
    .unwrap();
    assert_eq!((repeated.radius, repeated.kiosk.enabled), (2000.0, false));
}

#[test]
fn mode_settings_reject_unreachable_rules() {
    let with = |change: fn(&mut Config)| {
        let mut config = Config::battledome();
        config.spawn_radius_max = None;
        change(&mut config);
        config.validate()
    };
    for (change, field, rule) in [
        (
            (|c| c.pit = Some(PitConfig { radius: 0.0 })) as fn(&mut Config),
            "pit.radius",
            Constraint::NonPositive,
        ),
        (
            |c| c.pit = Some(PitConfig { radius: 3584.0 }),
            "pit.radius",
            Constraint::OutOfRange,
        ),
        (
            |c| c.pit = Some(PitConfig { radius: f64::NAN }),
            "pit.radius",
            Constraint::NonFinite,
        ),
        (
            |c| c.spawn_radius = Some(3584.0),
            "spawnRadius",
            Constraint::OutsideArena,
        ),
        (
            |c| c.spawn_radius = Some(3524.0),
            "spawnRadius",
            Constraint::OutsideArena,
        ),
        (
            |c| c.spawn_radius = Some(1784.0),
            "spawnRadius",
            Constraint::OutsideArena,
        ),
        (
            |c| c.spawn_radius = Some(-1.0),
            "spawnRadius",
            Constraint::OutsideArena,
        ),
        (
            |c| c.spawn_radius = Some(f64::NAN),
            "spawnRadius",
            Constraint::NonFinite,
        ),
        (
            |c| c.kill_score_seconds = Some(0.0),
            "killScoreSeconds",
            Constraint::NonPositive,
        ),
        (
            |c| c.kill_score_seconds = Some(f64::INFINITY),
            "killScoreSeconds",
            Constraint::NonFinite,
        ),
        (
            |c| c.kill_score_seconds = Some(f64::NAN),
            "killScoreSeconds",
            Constraint::NonFinite,
        ),
        (
            |c| c.min_score = Some(9),
            "minScore",
            Constraint::OutOfRange,
        ),
        (
            |c| c.min_score = Some(0),
            "minScore",
            Constraint::OutOfRange,
        ),
        (
            |c| c.min_score = Some(u32::MAX),
            "minScore",
            Constraint::OutOfRange,
        ),
        (
            |c| c.spawn_radius = None,
            "spawnRadius",
            Constraint::Missing,
        ),
        (
            |c| {
                c.pit = None;
                c.spawn_radius = Some(150.0);
            },
            "spawnRadius",
            Constraint::OutsideArena,
        ),
    ] {
        let cause = with(change).unwrap_err();
        assert_eq!(cause.kind, ErrorKind::InvalidConfig { rule });
        assert_eq!(cause.field.as_deref(), Some(field));
    }
    // Both ends of the displayed-score range are valid floors: a newborn's 10,
    // and the score at 430 segments, the least any larger snake displays.
    let (low, high) = MassFloor::score_range().into_inner();
    assert_eq!(low, 10);
    assert_eq!(f64::from(high), score(MAX_SEGMENTS, 0.0));
    assert_eq!(score(MAX_SEGMENTS + 1, 0.0), score(MAX_SEGMENTS, 0.0));
    for score in [low, high] {
        let mut config = Config::battledome();
        config.min_score = Some(score);
        // A 429-segment body needs a longer ring than Battledome's.
        config.radius = 30_000.0;
        config.spawn_radius = Some(15_000.0);
        config.spawn_radius_max = None;
        config.validate().unwrap();
    }
    let mut config = Config::battledome();
    config.min_score = Some(high + 1);
    assert_eq!(
        config.validate().unwrap_err().kind,
        ErrorKind::InvalidConfig {
            rule: Constraint::OutOfRange
        }
    );
    // A fractional score is not a displayed score.
    let error = serde_json::from_str::<Config>(r#"{"minScore":400.5}"#).unwrap_err();
    assert!(
        error.to_string().contains("invalid type: floating point"),
        "{error}"
    );
    // Without a pit, any ring that holds the body and keeps the margin is valid.
    with(|c| {
        c.pit = None;
        c.spawn_radius = Some(180.0);
    })
    .unwrap();
    with(|c| {
        c.pit = None;
        c.min_score = None;
        c.spawn_radius = Some(22.0);
    })
    .unwrap();
    with(|c| c.spawn_radius = Some(3523.0)).unwrap();
    assert!(
        Simulator::new(slither_sim::WorldOptions {
            seed: 1,
            config: Config {
                spawn_radius: Some(1000.0),
                ..Config::sandbox()
            },
            population: slither_sim::InitialPopulation::Empty
        })
        .is_err()
    );
}

#[test]
fn spawn_bands_reject_unordered_and_impossible_geometry() {
    for outer in [2600., 3524., f64::INFINITY, f64::NAN] {
        let mut config = Config::battledome();
        config.spawn_radius_max = Some(outer);
        assert!(config.validate().is_err(), "{outer}");
    }
    // An outer edge inside the wall margin names the outer setting.
    let mut config = Config::battledome();
    config.spawn_radius_max = Some(3524.);
    let error = config.validate().unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::InvalidConfig {
            rule: Constraint::OutsideArena
        }
    );
    assert_eq!(error.field.as_deref(), Some("spawnRadiusMax"));
    let mut config = Config::sandbox();
    config.spawn_radius_max = Some(800.);
    let error = config.validate().unwrap_err();
    assert_eq!(
        error.kind,
        ErrorKind::InvalidConfig {
            rule: Constraint::Missing
        }
    );
    assert_eq!(error.field.as_deref(), Some("spawnRadiusMax"));
    let config = Config::battledome();
    let tangent_seconds =
        ((config.radius - 60.).powi(2) - config.spawn_radius_max.unwrap().powi(2)).sqrt()
            / slither_sim::physics::BOOST_SPEED;
    assert!(tangent_seconds > 2.1);
    let normal_seconds =
        tangent_seconds * slither_sim::physics::BOOST_SPEED / slither_sim::physics::base_speed(26);
    assert!(normal_seconds > 5.);
}

#[test]
fn serve_resets_accept_a_preset() {
    let mut session = slither_sim::transport::Session::new();
    let reply: serde_json::Value = serde_json::from_slice(&session.request(
        br#"{"op":"reset","agents":2,"config":{"preset":"battledome","killScoreSeconds":60}}"#,
    ))
    .unwrap();
    let mut expected = Config::battledome();
    expected.kill_score_seconds = Some(60.0);
    assert_eq!(
        reply["result"]["config"],
        serde_json::to_value(expected).unwrap()
    );
    let reply: serde_json::Value = serde_json::from_slice(
        &session.request(br#"{"op":"reset","config":{"preset":"battledome","spawnRadius":100}}"#),
    )
    .unwrap();
    assert!(
        reply["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("spawnRadius must lie outside pit.radius")
    );
}

#[cfg(feature = "networking")]
#[test]
fn host_config_files_accept_a_preset() {
    use slither_sim::client::HostConfig;
    let host: HostConfig =
        serde_json::from_str(r#"{"players":{"max":2},"bots":{"count":6,"threads":2},"simulation":{"world":{"preset":"battledome"}}}"#).unwrap();
    host.validate().unwrap();
    assert_eq!(host.players.max, 2);
    assert_eq!(host.bots.count, 6);
    assert_eq!(
        serde_json::to_value(&host.simulation.world).unwrap(),
        serde_json::to_value(Config::battledome()).unwrap()
    );
    let error = serde_json::from_str::<HostConfig>(
        r#"{"simulation":{"world":{"preset":"battledome","pits":[]}}}"#,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("unknown field `pits`"),
        "{error}"
    );
}

#[test]
fn min_score_converts_to_the_least_mass_that_displays_it() {
    const STEPS: f64 = 16_777_215.0;
    let floor = MassFloor::for_score(400).unwrap();
    assert_eq!(floor.segments, 26);
    assert!((floor.fam - 0.1589).abs() < 1e-4, "{floor:?}");
    // On the protocol grid, so a client decodes the same fam.
    let step = (floor.fam * STEPS).round();
    assert_eq!(floor.fam, step / STEPS);
    assert_eq!(score(floor.segments, floor.fam), 400.0);
    assert_eq!(score(floor.segments, (step - 1.0) / STEPS), 399.0);
    let (low, high) = MassFloor::score_range().into_inner();
    assert_eq!(
        MassFloor::for_score(low),
        Some(MassFloor {
            segments: 2,
            fam: 0.0
        })
    );
    // The top floor is attainable: its mass displays exactly the bound.
    let top = MassFloor::for_score(high).unwrap();
    assert_eq!(score(top.segments, top.fam), f64::from(high));
    assert_eq!(MassFloor::for_score(low - 1), None);
    assert_eq!(MassFloor::for_score(high + 1), None);
    // Targets at and just past each segment boundary, where the score's
    // linear pieces meet and a closed-form inverse cancels to a tiny fam.
    for segments in 2..MAX_SEGMENTS {
        let start = score(segments, 0.0) as u32;
        for target in [start.saturating_sub(1), start, start + 1, start + 2] {
            let Some(floor) = MassFloor::for_score(target) else {
                assert!(target < low || target > high);
                continue;
            };
            let step = (floor.fam * STEPS).round();
            assert_eq!(floor.fam, step / STEPS);
            assert!(score(floor.segments, floor.fam) >= f64::from(target));
            if (floor.segments, step) == (2, 0.0) {
                continue; // A newborn's mass has nothing below it.
            }
            let below = if step > 0.0 {
                score(floor.segments, (step - 1.0) / STEPS)
            } else {
                score(floor.segments - 1, (STEPS - 1.0) / STEPS)
            };
            assert!(below < f64::from(target), "{target}: {floor:?}");
        }
    }
}

/// A food-free, kiosk-free sandbox with a 400-point floor, a ring of radius
/// 500 and one snake.
fn floor_world() -> Simulator {
    let mut config = Config::sandbox();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    config.min_score = Some(400);
    config.spawn_radius = Some(500.0);
    Simulator::new(slither_sim::WorldOptions {
        seed: 11,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["player".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap()
}

fn straight_boost(world: &Simulator) -> Vec<Option<Action>> {
    let mut actions = straight(world);
    for action in actions.iter_mut().flatten() {
        action.boost = true;
    }
    actions
}

/// Turn steadily, so the snake circles inside the arena.
fn circling(world: &Simulator, boost: bool) -> Vec<Option<Action>> {
    world
        .snakes()
        .iter()
        .map(|s| {
            s.alive.then_some(Action {
                heading: s.heading + 1.0,
                boost,
            })
        })
        .collect()
}

#[test]
fn growth_past_the_score_table_keeps_the_top_floor() {
    let high = *MassFloor::score_range().end();
    let mut config = floor_world().config().clone();
    config.radius = 30_000.0;
    config.spawn_radius = Some(15_000.0);
    config.spawn_radius_max = None;
    config.min_score = Some(high);
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 3,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["giant".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    // A full growth unit grows the snake past 430 segments at its next anchor.
    set_mass(&mut world, MAX_SEGMENTS, 1.0);
    for _ in 0..100 {
        world.step(&straight(&world)).unwrap();
        assert!(world.snakes()[0].score >= f64::from(high));
        if world.snakes()[0].segments > MAX_SEGMENTS {
            break;
        }
    }
    let s = &world.snakes()[0];
    assert!(s.alive && s.segments == MAX_SEGMENTS + 1, "{}", s.segments);
    assert_eq!(s.score, f64::from(high));
}

/// Give slot 0 a straight body of this mass along the x axis, facing +x.
fn set_mass(world: &mut Simulator, segments: usize, fam: f64) {
    let mut detached_s = world.snakes()[0].clone();
    let s = &mut detached_s;
    s.segments = segments;
    s.fam = fam;
    s.heading = 0.0;
    s.body = (0..segments)
        .map(|i| Point {
            x: -600.0 - 10.0 * i as f64,
            y: 0.0,
        })
        .collect();
    s.head = s.body[0];
    world.correct_mass(0, s.body.clone(), s.fam).unwrap();
    world
        .correct_pose(0, s.head, s.body.clone(), s.heading)
        .unwrap();
}

/// Boost straight ahead for `ticks`, returning every pellet dropped.
fn boost_drops(world: &mut Simulator, ticks: usize) -> Vec<slither_sim::Food> {
    world.record_events(slither_sim::EventRecording {
        food_births: true,
        pickups: world.pickup_events().is_some(),
    });
    let mut drops = Vec::new();
    for _ in 0..ticks {
        world.step(&straight_boost(world)).unwrap();
        let events = world.food_spawn_events().unwrap();
        drops.extend(events.iter().map(|event| event.food.clone()));
    }
    drops
}

#[test]
fn min_score_spawns_and_respawns_at_the_floor() {
    let floor = MassFloor::for_score(400).unwrap();
    let mut world = floor_world();
    let s = &world.snakes()[0];
    assert_eq!(
        (s.segments, s.fam, s.score),
        (floor.segments, floor.fam, 400.0)
    );
    assert_on_ring(s, 500.0, 26);
    world.retire(0).unwrap();
    let (short, heading) = (vec![Point::default(); 25], 0.0);
    assert_eq!(
        world.respawn_body(0, short, heading).unwrap_err().kind,
        ErrorKind::InvalidState {
            rule: Constraint::OutOfRange
        }
    );
    let (body, heading) = world.spawn_body(Point { x: 0.0, y: -500.0 }, 0.0).unwrap();
    assert!(world.body_is_clear(&body, 600.0));
    world.respawn_body(0, body, heading).unwrap();
    let s = &world.snakes()[0];
    assert_eq!(
        (s.segments, s.fam, s.score),
        (floor.segments, floor.fam, 400.0)
    );
    assert_on_ring(s, 500.0, 26);
    // A head off the ring whose circle cannot hold the body is an error.
    assert!(world.spawn_body(Point { x: 100.0, y: 0.0 }, 0.0).is_err());
}

#[test]
fn boosting_at_the_floor_is_free_speed_without_pellets() {
    let floor = MassFloor::for_score(400).unwrap();
    let mut world = floor_world();
    set_mass(&mut world, floor.segments, floor.fam);
    let cruise = world.snakes()[0].speed;
    let drops = boost_drops(&mut world, 300);
    let s = &world.snakes()[0];
    assert!(s.alive && world.is_boosting(0) && s.speed > cruise);
    assert_eq!(
        (s.segments, s.fam, s.score),
        (floor.segments, floor.fam, 400.0)
    );
    assert!(drops.is_empty() && world.food().is_empty());
    assert_eq!(world.boost_accounting(0).unwrap().boost_mass_spent, 0.0);
}

#[test]
fn boosting_above_the_floor_costs_normally_and_stops_there() {
    let floor = MassFloor::for_score(400).unwrap();
    let mut floored = floor_world();
    let mut config = floored.config().clone();
    config.min_score = None;
    let mut open = Simulator::new(slither_sim::WorldOptions {
        seed: 11,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["player".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    for world in [&mut floored, &mut open] {
        let mut detached_s = world.snakes()[0].clone();
        let s = &mut detached_s;
        s.segments = 40;
        s.fam = 0.5;
        s.body = (0..40)
            .map(|i| Point {
                x: 300.0 - 10.0 * i as f64,
                y: 0.0,
            })
            .collect();
        s.head = s.body[0];
        world.correct_mass(0, s.body.clone(), s.fam).unwrap();
        world
            .correct_pose(0, s.head, s.body.clone(), s.heading)
            .unwrap();
    }
    // Until the floor is near, both pay and drop alike.
    for _ in 0..60 {
        floored.step(&circling(&floored, true)).unwrap();
        open.step(&circling(&open, true)).unwrap();
    }
    assert_eq!(floored.food(), open.food());
    assert!(!floored.food().is_empty());
    assert_eq!(
        (floored.snakes()[0].segments, floored.snakes()[0].fam),
        (open.snakes()[0].segments, open.snakes()[0].fam)
    );
    // Boost cost is small at this size; give it a few simulated minutes.
    for _ in 0..50_000 {
        if (floored.snakes()[0].segments, floored.snakes()[0].fam) == (floor.segments, floor.fam) {
            break;
        }
        floored.step(&circling(&floored, true)).unwrap();
    }
    let s = &floored.snakes()[0];
    assert_eq!(
        (s.segments, s.fam, s.score),
        (floor.segments, floor.fam, 400.0)
    );
    assert_eq!(s.body.len(), floor.segments);
    // Boosting continues there for free.
    floored.step(&circling(&floored, true)).unwrap();
    assert!(floored.is_boosting(0));
    assert_eq!(
        (floored.snakes()[0].segments, floored.snakes()[0].fam),
        (floor.segments, floor.fam)
    );
    // It paid at least the starting mass above the floor; circling also eats
    // some of its own drops.
    let spent = floored.boost_accounting(0).unwrap().boost_mass_spent;
    assert!(spent >= 40.5 - (floor.segments as f64 + floor.fam) - 1e-9);
}

#[test]
fn kiosk_shrinking_stops_at_the_floor() {
    let floor = MassFloor::for_score(400).unwrap();
    let mut world = floor_world();
    {
        let mut config = world.config().clone();
        config.kiosk = slither_sim::KioskConfig {
            floor_score: 100.0,
            grace_seconds: 0.0,
            stationary_radius: 1e5,
            shrink_per_second: 2.0,
            score_decay_per_second: 1.0,
            ..Default::default()
        };
        support::reconfigure(&mut world, config);
    }
    let mut detached_s = world.snakes()[0].clone();
    let s = &mut detached_s;
    s.segments = 60;
    s.body = (0..60)
        .map(|i| Point {
            x: 300.0 - 5.0 * i as f64,
            y: 0.0,
        })
        .collect();
    s.head = s.body[0];
    world.correct_mass(0, s.body.clone(), s.fam).unwrap();
    world
        .correct_pose(0, s.head, s.body.clone(), s.heading)
        .unwrap();
    for _ in 0..1000 {
        world.step(&circling(&world, false)).unwrap();
        let s = &world.snakes()[0];
        assert!(
            (s.segments, s.fam) >= (floor.segments, floor.fam),
            "{} {}",
            s.segments,
            s.fam
        );
        if (s.segments, s.fam) == (floor.segments, floor.fam) {
            break;
        }
    }
    let s = &world.snakes()[0];
    assert!(s.alive);
    assert_eq!((s.segments, s.fam), (floor.segments, floor.fam));
    // Once physical shrinking ends at the floor, the displayed credit decays
    // as usual instead of stalling, here within a few seconds.
    let mut displayed = s.score;
    assert!(displayed > 400.0);
    for _ in 0..1000 {
        world.step(&circling(&world, false)).unwrap();
        let score = world.snakes()[0].score;
        assert!(score <= displayed);
        displayed = score;
    }
    assert_eq!(world.snakes()[0].score, 400.0);
}

#[test]
fn battledome_floor_pit_and_wall_deaths_drop_nothing() {
    let mut config = Config::battledome();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 3,
        config,
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap();
    support::ring_slot(&mut world, "leaver", 0, 2).unwrap();
    support::ring_slot(&mut world, "runner", 1, 2).unwrap();
    // Just inside the pit, heading out; and just inside the wall, heading out.
    for (slot, head, heading) in [
        (0, Point { x: 1780.0, y: 0.0 }, 0.0),
        (1, Point { x: 0.0, y: 3570.0 }, FRAC_PI_2),
    ] {
        world.retire(slot).unwrap();
        world.respawn(slot, head, heading, 26).unwrap();
    }
    for _ in 0..20 {
        world.step(&straight(&world)).unwrap();
    }
    let mut deaths = world.deaths().to_vec();
    deaths.sort_by_key(|death| death.reason as u8);
    let reasons: Vec<_> = deaths.iter().map(|death| death.reason).collect();
    assert_eq!(reasons, [DeathReason::Boundary, DeathReason::Pit]);
    assert_eq!(deaths[0].food, 0);
    assert_eq!(deaths[1].food, 0);
    assert!(world.food().is_empty());
}

#[test]
fn death_food_excludes_the_floor_and_keeps_the_earned_fraction() {
    let mut open = kill_world(None);
    let floor = MassFloor::for_score(400).unwrap();
    let mut killer_body = open.snakes()[1].body.clone();
    killer_body.resize(floor.segments, *killer_body.last().unwrap());
    open.correct_mass(1, killer_body, floor.fam).unwrap();
    let mut config = open.config().clone();
    config.min_score = Some(400);
    config.spawn_radius = Some(500.);
    let mut floored = Simulator::from_scenario(
        7,
        config,
        slither_sim::Scenario {
            snakes: open
                .snakes()
                .iter()
                .map(|snake| {
                    (
                        snake.id.clone(),
                        slither_sim::ReplayLife::from_observed(
                            snake.clone(),
                            slither_sim::InitialSpeed::Infer(snake.speed),
                        ),
                    )
                })
                .collect(),
            food: Vec::new(),
            prey: Vec::new(),
        },
    )
    .unwrap();
    // Disable admission to test the complete nutrition budget, including its
    // fractional last pellet; geometry and full pellet sizes stay official.
    {
        let mut config = open.config().clone();
        config.death_food.enabled = false;
        support::reconfigure(&mut open, config);
    }
    {
        let mut config = floored.config().clone();
        config.death_food.enabled = false;
        support::reconfigure(&mut floored, config);
    }
    {
        let mut snake = open.snakes()[0].clone();
        snake.fam = 0.5;
        open.correct_mass(0, snake.body.clone(), snake.fam).unwrap();
    }
    floored
        .correct_mass(0, open.snakes()[0].body.clone(), open.snakes()[0].fam)
        .unwrap();
    open.step(&straight(&open)).unwrap();
    floored.step(&straight(&floored)).unwrap();
    assert_eq!(floored.deaths()[0].reason, DeathReason::Body);
    assert!(floored.deaths()[0].food > 0);
    let floor = MassFloor::for_score(400).unwrap();
    let fraction = floor.excess(30, 0.5) / 30.5;
    let budget = 61.0 * fraction;
    assert_eq!(floored.deaths()[0].food, budget.ceil() as usize);
    let nominal: f64 = open.food().iter().map(|food| food.value).sum();
    let actual: f64 = floored.food().iter().map(|food| food.value).sum();
    assert!((actual - nominal * fraction).abs() < 1e-12);
    let full = budget.floor() as usize;
    assert!(
        floored.food()[..full]
            .iter()
            .all(|food| food.size == open.food()[0].size)
    );
    let last = floored.food().last().unwrap();
    assert_eq!(
        last.size.unwrap(),
        open.food()[0].size.unwrap() * budget.fract().sqrt()
    );
    assert_eq!(last.origin, Some(slither_sim::Origin::Death));
}

#[test]
fn a_floor_size_body_collision_drops_nothing() {
    let mut source = kill_world(None);
    let floor = MassFloor::for_score(400).unwrap();
    for slot in 0..2 {
        let mut body = source.snakes()[slot].body.clone();
        body.resize(floor.segments, *body.last().unwrap());
        source.correct_mass(slot, body, floor.fam).unwrap();
    }
    let mut config = source.config().clone();
    config.min_score = Some(400);
    config.spawn_radius = Some(500.);
    let mut world = Simulator::from_scenario(
        7,
        config,
        slither_sim::Scenario {
            snakes: source
                .snakes()
                .iter()
                .map(|snake| {
                    (
                        snake.id.clone(),
                        slither_sim::ReplayLife::from_observed(
                            snake.clone(),
                            slither_sim::InitialSpeed::Infer(snake.speed),
                        ),
                    )
                })
                .collect(),
            food: Vec::new(),
            prey: Vec::new(),
        },
    )
    .unwrap();
    world.step(&straight(&world)).unwrap();
    assert_eq!(world.deaths()[0].reason, DeathReason::Body);
    assert_eq!(world.deaths()[0].food, 0);
    assert!(world.food().is_empty());
}

#[cfg(feature = "networking")]
#[test]
fn the_floor_survives_protocol_quantization() {
    use slither_sim::client::{SimulationConfig, SimulatorBackend};
    let mut world = Config::battledome();
    world.food_count = 0;
    world.ambient_replenish_per_second = 0.0;
    let config = SimulationConfig {
        agents: 1,
        world,
        ..Default::default()
    };
    let mut backend = SimulatorBackend::new(5, config).unwrap();
    backend.advance_ticks(6).unwrap();
    let floor = MassFloor::for_score(400).unwrap();
    let core = &backend.world().snakes()[0];
    assert_eq!(
        (core.segments, core.fam, core.score),
        (26, floor.fam, 400.0)
    );
    // The decoded client frame, not only the core, shows the floor exactly.
    let me = backend.frame().unwrap().observation.unwrap().me;
    assert_eq!((me.segments, me.fam, me.score), (26, floor.fam, 400.0));
}

#[test]
fn direct_deserialize_uses_the_preset_aware_trait() {
    use serde::Deserialize;
    let input = r#"{"preset":"battledome","radius":4000}"#;
    let mut deserializer = serde_json::Deserializer::from_str(input);
    let direct = Config::deserialize(&mut deserializer).unwrap();
    let json: Config = serde_json::from_str(input).unwrap();
    assert_eq!(
        serde_json::to_value(&direct).unwrap(),
        serde_json::to_value(json).unwrap()
    );
    assert_eq!(direct.radius, 4000.);
    assert_eq!(direct.pit, Config::battledome().pit);
}

#[test]
fn seat_following_keeps_the_usual_battledome_geometry_and_grows_above_32() {
    use slither_sim::{InitialPopulation, Placement, WorldOptions};
    for seats in [16, 32, 40, 64, 80] {
        let mut config = Config::battledome();
        config.food_count = 0;
        config.ambient_replenish_per_second = 0.;
        let mut world = Simulator::new(WorldOptions {
            config,
            population: InitialPopulation::Placed {
                ids: (0..seats).map(|i| i.to_string()).collect(),
                placement: Placement::TestRing,
            },
            ..Default::default()
        })
        .unwrap();
        let before = world.snakes()[0].head;
        world.step(&straight(&world)).unwrap();
        let factor = 1. + 1.5 * (seats.max(32) as f64 / 32. - 1.);
        assert_eq!(world.radius(), 3584. * factor);
        assert_eq!(world.pit_radius(), Some(1784. * factor));
        assert_eq!(world.rules().pit_radius, world.pit_radius());
        for u in [0., 0.5, 1.] {
            let actual = spawn::spawn_distance(world.config(), world.radius(), u);
            let reference = spawn::spawn_distance(world.config(), 3584., u);
            assert_eq!(actual, reference * factor);
        }
        // Geometry changes don't teleport or stretch snakes.
        assert!(world.snakes()[0].head.distance(before) < 2.);
        assert_eq!(world.config().min_score, Some(400));
        assert_eq!(world.config().kill_score_seconds, Some(900.));
        if seats > 32 {
            for slot in 32..seats {
                world.retire(slot).unwrap();
            }
            let before = world.radius();
            world.step(&straight(&world)).unwrap();
            assert_eq!(world.radius(), before - 23.4375 * world.config().dt);
            let saved = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
            let mut restored = Simulator::from_checkpoint(&saved, Default::default()).unwrap();
            for _ in 0..100 {
                let actions = straight(&world);
                world.step(&actions).unwrap();
                restored.step(&actions).unwrap();
            }
            assert_eq!(
                serde_json::to_value(world.checkpoint().unwrap()).unwrap(),
                serde_json::to_value(restored.checkpoint().unwrap()).unwrap()
            );
        }
    }
}

#[cfg(feature = "networking")]
#[test]
fn the_pit_edge_keeps_its_minimap_cells_as_the_live_wall_grows() {
    use slither_sim::{InitialPopulation, RadiusUpdate, WorldOptions, client::Minimap};
    let mut config = Config::battledome();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    config.radius_updates = vec![RadiusUpdate {
        time: 0.007,
        radius: 8960.,
    }];
    let mut world = Simulator::new(WorldOptions {
        config,
        population: InitialPopulation::Empty,
        ..Default::default()
    })
    .unwrap();
    for side in [80, 136, 512] {
        let initial = Minimap::from_world(&world, side).unwrap();
        assert!(initial.cells.iter().any(|&c| c != 0));
        for radius in [4928., 7168., 8960., 11648., 23296.] {
            let mut config = world.config().clone();
            config.radius_updates[0].radius = radius;
            let mut grown = Simulator::new(WorldOptions {
                config,
                population: InitialPopulation::Empty,
                ..Default::default()
            })
            .unwrap();
            grown.step(&[]).unwrap();
            assert_eq!(grown.radius(), radius);
            assert_eq!(grown.pit_radius(), Some(1784. * (radius / 3584.)));
            assert_eq!(
                initial.cells,
                Minimap::from_world(&grown, side).unwrap().cells
            );
        }
        // A fixed physical edge would move inward in the live-radius raster.
        let projected = |r: f64, wall: f64| ((r / wall + 1.) * side as f64 / 2.).floor();
        assert_ne!(projected(1784., 3584.), projected(1784., 8960.));
    }
    // The stock own-player marker uses the live-radius projection separately;
    // this outer snake no longer paints the pit-only bitmap.
    world.step(&[]).unwrap();
    let head = Point { x: 7000., y: 0. };
    let (body, heading) = world.spawn_body(head, FRAC_PI_2).unwrap();
    world.spawn("marker".into(), body, heading).unwrap();
    let side = 136;
    let index =
        side / 2 * side + ((head.x / world.radius() + 1.) * side as f64 / 2.).floor() as usize;
    assert_eq!(Minimap::from_world(&world, side).unwrap().cells[index], 0);
}

#[test]
fn the_one_way_pit_tests_the_live_edge_after_growth() {
    use slither_sim::{InitialPopulation, RadiusUpdate, WorldOptions};
    let mut config = Config::battledome();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    config.radius_updates = vec![RadiusUpdate {
        time: 0.,
        radius: 7168.,
    }];
    let mut world = Simulator::new(WorldOptions {
        config,
        population: InitialPopulation::Empty,
        ..Default::default()
    })
    .unwrap();
    let head = Point { x: 2500., y: 0. };
    let (body, heading) = world.spawn_body(head, FRAC_PI_2).unwrap();
    world
        .spawn("inside-live-pit".into(), body, heading)
        .unwrap();
    // Outside the reference pit, inside the live one. Outward travel must
    // enter/exit the live boundary, even while it gradually follows the wall.
    for _ in 0..1500 {
        if !world.snakes()[0].alive {
            break;
        }
        world
            .step(&[Some(Action {
                heading: 0.,
                boost: false,
            })])
            .unwrap();
    }
    assert!(!world.snakes()[0].alive);
    assert_eq!(world.deaths().len(), 1);
    assert_eq!(world.deaths()[0].reason, DeathReason::Pit);
    assert!(distance(world.snakes()[0].head) >= world.pit_radius().unwrap());
    assert!(world.food().is_empty()); // floor deaths still drop nothing.
}

#[cfg(feature = "networking")]
#[test]
fn host_validation_reserves_wire_room_for_the_largest_queue_allowance() {
    use slither_sim::client::SimulationConfig;
    for (seats, accepted) in [(80, true), (105, true), (106, false)] {
        let config = SimulationConfig {
            agents: seats,
            world: Config::battledome(),
            ..Default::default()
        };
        assert_eq!(config.validate().is_ok(), accepted);
    }
    let mut fixed = Config::battledome();
    fixed.dynamic_radius = false;
    assert!(
        SimulationConfig {
            agents: 480,
            world: fixed.clone(),
            ..Default::default()
        }
        .validate()
        .is_ok()
    );
    fixed.size_to_population = true;
    assert!(
        SimulationConfig {
            agents: 800,
            world: fixed,
            ..Default::default()
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn a_growing_pit_does_not_count_as_a_newborn_entering_it() {
    use slither_sim::{InitialPopulation, RadiusUpdate, WorldOptions};
    let mut config = Config::battledome();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    config.radius_updates = vec![RadiusUpdate {
        time: 0.007,
        radius: 3000. * 3584. / 1784.,
    }];
    let mut world = Simulator::new(WorldOptions {
        config,
        population: InitialPopulation::Empty,
        ..Default::default()
    })
    .unwrap();
    let (body, heading) = world
        .spawn_body(Point { x: 2900., y: 0. }, FRAC_PI_2)
        .unwrap();
    world.spawn("newborn".into(), body, heading).unwrap();
    let outward = [Some(Action {
        heading: 0.,
        boost: false,
    })];
    world.step(&outward).unwrap();
    assert!(distance(world.snakes()[0].head) < world.pit_radius().unwrap());
    let saved = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
    let mut restored = Simulator::from_checkpoint(&saved, Default::default()).unwrap();
    for _ in 0..500 {
        world.step(&outward).unwrap();
        restored.step(&outward).unwrap();
        assert!(world.snakes()[0].alive, "{:?}", world.deaths());
    }
    assert!(distance(world.snakes()[0].head) > world.pit_radius().unwrap());
    assert_eq!(
        serde_json::to_value(world.checkpoint().unwrap()).unwrap(),
        serde_json::to_value(restored.checkpoint().unwrap()).unwrap()
    );
    // Physical re-entry still commits this life to the ordinary one-way rule.
    while distance(world.snakes()[0].head) >= world.pit_radius().unwrap() - 100. {
        world
            .step(&[Some(Action {
                heading: PI,
                boost: false,
            })])
            .unwrap();
        assert!(world.snakes()[0].alive);
    }
    for _ in 0..1000 {
        if !world.snakes()[0].alive {
            break;
        }
        world.step(&outward).unwrap();
    }
    assert!(!world.snakes()[0].alive);
    assert_eq!(world.deaths()[0].reason, DeathReason::Pit);
}

#[test]
fn a_shrinking_edge_does_not_reenter_a_snake_engulfed_by_growth() {
    use slither_sim::{InitialPopulation, RadiusUpdate, WorldOptions};
    let mut config = Config::battledome();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.;
    config.radius_updates = vec![RadiusUpdate {
        time: 0.007,
        radius: 3000. * 3584. / 1784.,
    }];
    let mut world = Simulator::new(WorldOptions {
        config,
        population: InitialPopulation::Empty,
        ..Default::default()
    })
    .unwrap();
    let (body, heading) = world
        .spawn_body(Point { x: 2900., y: 0. }, FRAC_PI_2)
        .unwrap();
    world.spawn("engulfed".into(), body, heading).unwrap();
    world
        .step(&[Some(Action {
            heading: 0.,
            boost: false,
        })])
        .unwrap();
    let old_pit = world.pit_radius().unwrap();
    let next_pit = 1784. * ((world.radius() - 23.4375 * world.config().dt) / 3584.);
    let half_step = slither_sim::physics::base_speed(26) * world.config().dt / 2.;
    // This horizontal chord grazes the new edge, but begins inside the old one:
    // the shrinking boundary moving past an unentered life is not re-entry.
    let head = Point {
        x: -half_step,
        y: next_pit - 0.00001,
    };
    assert!(distance(head) < old_pit && distance(head) > next_pit);
    let (body, _) = world.spawn_body(head, 0.).unwrap();
    world.correct_pose(0, head, body, 0.).unwrap();
    world
        .step(&[Some(Action {
            heading: 0.,
            boost: false,
        })])
        .unwrap();
    assert!(world.snakes()[0].alive, "{:?}", world.deaths());
    assert!(distance(world.snakes()[0].head) > world.pit_radius().unwrap());
}
