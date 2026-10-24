//! The Battledome mode rules and the general settings behind them: `pit`,
//! `spawnRadius`, `killScoreSeconds` and `minScore`. Worlds here are synthetic setups.
use slither_sim::physics::{MAX_SEGMENTS, MassFloor, score};
use slither_sim::{Action, Config, DeathReason, PitConfig, Point, Preset, Simulator, spawn};
use std::f64::consts::{FRAC_PI_2, PI};

/// A food-free arena of radius 1000 with a pit of radius 200.
fn pit_world() -> Simulator {
    let mut config = Config::sandbox();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    config.pit = Some(PitConfig { radius: 200.0 });
    Simulator::new(3, config).unwrap()
}

fn place(world: &mut Simulator, slot: usize, head: Point, heading: f64) {
    world.snakes[slot].alive = false;
    world.respawn(slot, head, heading, 2).unwrap();
}

fn straight(world: &Simulator) -> Vec<Option<Action>> {
    world
        .snakes
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
    world.add("diver", 0, 1).unwrap();
    place(&mut world, 0, Point { x: 300.0, y: 0.0 }, PI);
    let mut inside = 0;
    for _ in 0..2000 {
        if !world.snakes[0].alive {
            break;
        }
        world.step(&straight(&world)).unwrap();
        if world.snakes[0].alive && distance(world.snakes[0].head) < 200.0 {
            inside += 1;
        }
    }
    assert!(inside > 100, "{inside} ticks inside");
    let s = &world.snakes[0];
    assert!(!s.alive && s.head.x < 0.0 && distance(s.head) >= 200.0);
    assert_eq!(world.deaths.len(), 1);
    let death = &world.deaths[0];
    assert_eq!(
        (death.reason, death.killer.as_deref()),
        (DeathReason::Pit, None)
    );
    assert!(death.food > 0);
    assert_eq!(world.food.len(), death.food);
}

#[test]
fn a_snake_that_never_entered_cannot_die_at_the_pit() {
    let mut world = pit_world();
    world.add("edge", 0, 2).unwrap();
    world.add("orbit", 1, 2).unwrap();
    // Exactly on the pit radius is not inside; moving outward never enters.
    place(&mut world, 0, Point { x: 200.0, y: 0.0 }, 0.0);
    place(&mut world, 1, Point { x: 0.0, y: -250.0 }, 0.0);
    for _ in 0..300 {
        world.step(&straight(&world)).unwrap();
    }
    assert!(world.snakes.iter().all(|s| s.alive), "{:?}", world.deaths);
}

#[test]
fn crossing_the_pit_within_one_tick_kills() {
    // A synthetic pit of radius 1 that an unboosted head crosses in one 32 ms tick.
    let mut config = pit_world().config;
    config.pit = Some(PitConfig { radius: 1.0 });
    config.dt = 0.032;
    let mut world = Simulator::new(3, config).unwrap();
    world.add("diver", 0, 1).unwrap();
    place(&mut world, 0, Point { x: -2.0, y: 0.0 }, 0.0);
    world.step(&straight(&world)).unwrap();
    let s = &world.snakes[0];
    assert!(!s.alive && s.head.x > 1.0, "{:?}", s.head);
    assert_eq!(world.deaths[0].reason, DeathReason::Pit);
}

#[test]
fn a_tiny_pit_crossing_does_not_underflow() {
    // Synthetic scales: this path's squared length underflows to zero.
    let mut config = pit_world().config;
    config.pit = Some(PitConfig { radius: 1e-175 });
    config.dt = 2e-170 / slither_sim::physics::base_speed(2);
    let mut world = Simulator::new(3, config).unwrap();
    world.add("speck", 0, 1).unwrap();
    place(&mut world, 0, Point { x: -1e-170, y: 0.0 }, 0.0);
    world.step(&straight(&world)).unwrap();
    let s = &world.snakes[0];
    assert!(!s.alive && s.head.x > 1e-175, "{:?}", s.head);
    assert_eq!(world.deaths[0].reason, DeathReason::Pit);
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
    let mut world = Simulator::new(3, config).unwrap();
    world.add("grazer", 0, 1).unwrap();
    let start = Point {
        x: -2.9,
        y: 1783.999,
    };
    place(&mut world, 0, start, 0.0);
    world.step(&straight(&world)).unwrap();
    let end = world.snakes[0].head;
    assert!(
        distance(start) >= 1784.0 && distance(end) >= 1784.0,
        "{end:?}"
    );
    assert!(end.x > 2.0);
    assert!(!world.snakes[0].alive);
    assert_eq!(world.deaths[0].reason, DeathReason::Pit);
}

#[test]
fn a_life_starting_inside_the_pit_dies_on_leaving() {
    let mut world = pit_world();
    world.add("inside", 0, 1).unwrap();
    place(&mut world, 0, Point { x: 199.0, y: 0.0 }, 0.0);
    for _ in 0..10 {
        if world.snakes[0].alive {
            world.step(&straight(&world)).unwrap();
        }
    }
    assert!(!world.snakes[0].alive);
    assert_eq!(world.deaths[0].reason, DeathReason::Pit);
}

#[test]
fn respawning_clears_the_pit_state() {
    let mut world = pit_world();
    world.add("diver", 0, 1).unwrap();
    place(&mut world, 0, Point { x: 210.0, y: 0.0 }, PI);
    while distance(world.snakes[0].head) >= 150.0 {
        world.step(&straight(&world)).unwrap();
    }
    // A new life outside the pit does not inherit the old life's entry.
    place(&mut world, 0, Point { x: 300.0, y: 0.0 }, 0.0);
    for _ in 0..100 {
        world.step(&straight(&world)).unwrap();
    }
    assert!(world.snakes[0].alive && world.deaths.is_empty());
}

#[test]
fn new_lives_start_on_the_configured_ring_outside_the_pit() {
    let config = Config::battledome();
    let ring = config.spawn_radius.unwrap();
    assert_eq!(ring, (1784.0 + 3584.0) / 2.0);
    for unit in [0.0, 0.3, 0.999] {
        assert_eq!(spawn::spawn_distance(&config, unit), ring);
        assert_eq!(
            spawn::spawn_distance(&Config::official(4), unit),
            spawn::sample_spawn_distance(Config::official(4).radius, unit)
        );
    }
    let mut world = Simulator::new(1, config).unwrap();
    for slot in 0..8 {
        world.add(slot.to_string(), slot, 8).unwrap();
        assert_inward(&world.snakes[slot], ring);
    }
}

/// A new life on the ring: the official 2-segment newborn, whatever the
/// `minScore`, facing the center with its tail one anchor outward.
fn assert_inward(s: &slither_sim::Snake, ring: f64) {
    assert_eq!(
        (s.segments, s.fam, s.body.len(), s.score),
        (2, 0.0, 2, 10.0)
    );
    assert_eq!(s.body[0], s.head);
    assert!((distance(s.head) - ring).abs() < 1e-9);
    assert!((distance(s.body[1]) - ring - 42.0).abs() < 1e-9);
    let outward = s.head.y.atan2(s.head.x);
    assert!((s.heading - outward).cos() + 1.0 < 1e-12, "inward heading");
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
        let snake = &backend.world().snakes[slot];
        assert_inward(snake, 2684.0);
        angles.push(snake.head.y.atan2(snake.head.x));
    }
    angles.sort_by(f64::total_cmp);
    angles.dedup();
    assert_eq!(angles.len(), 6);
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
    let mut world = Simulator::new(7, config).unwrap();
    world.add("victim", 0, 2).unwrap();
    world.add("killer", 1, 2).unwrap();
    let s = &mut world.snakes[0];
    s.head = Point::default();
    s.body = vec![Point::default(); 30];
    s.segments = 30;
    Simulator::update_mass(s);
    let s = &mut world.snakes[1];
    s.head = Point { x: -25.0, y: 342.0 };
    s.heading = FRAC_PI_2;
    s.body = (0..11)
        .map(|i| Point {
            x: -25.0,
            y: 300.0 - 42.0 * i as f64,
        })
        .collect();
    s.segments = 11;
    Simulator::update_mass(s);
    world
}

#[test]
fn kill_score_counts_each_kill_until_the_window_ends_across_lives() {
    let mut world = kill_world(Some(0.5));
    assert_eq!((world.kill_score(1), world.kill_score(2)), (Some(0), None));
    world.step(&straight(&world)).unwrap();
    assert_eq!(world.deaths[0].killer.as_deref(), Some("killer"));
    let killed_at = world.time;
    assert_eq!(
        (world.kill_score(0), world.kill_score(1)),
        (Some(0), Some(1))
    );
    // A new life of the killer's slot keeps the score; per-life kills reset.
    place(&mut world, 1, Point { x: 500.0, y: 0.0 }, FRAC_PI_2);
    assert_eq!((world.snakes[1].kills, world.kill_score(1)), (0, Some(1)));
    while world.time < killed_at + 0.5 - world.config.dt {
        world.step(&straight(&world)).unwrap();
        assert_eq!(world.kill_score(1), Some(1));
    }
    world.step(&straight(&world)).unwrap();
    assert_eq!(world.time, killed_at + 0.5);
    assert_eq!(world.kill_score(1), Some(0));
}

#[test]
fn kill_score_is_zero_without_a_window() {
    let mut world = kill_world(None);
    world.step(&straight(&world)).unwrap();
    assert_eq!(world.snakes[1].kills, 1);
    assert_eq!(world.kill_score(1), Some(0));
}

#[test]
fn battledome_uses_official_density_and_runs_deterministically() {
    let config = Config::battledome();
    config.validate().unwrap();
    let official = Config::official(1);
    let sectors = std::f64::consts::PI * 3584.0 * 3584.0 / 90_000.0;
    assert_eq!(config.food_count, (7.0 * sectors) as usize);
    assert_eq!(config.ambient_replenish_per_second, 0.1024 * sectors);
    assert_eq!((config.radius, config.dynamic_radius), (3584.0, false));
    assert_eq!(config.pit, Some(PitConfig { radius: 1784.0 }));
    assert_eq!(config.kill_score_seconds, Some(900.0));
    assert_eq!(config.prey.count, 0);
    // Everything else is the official model.
    let mut expected = serde_json::to_value(&official).unwrap();
    let actual = serde_json::to_value(&config).unwrap();
    for field in [
        "radius",
        "dynamicRadius",
        "foodCount",
        "ambientReplenishPerSecond",
        "pit",
        "spawnRadius",
        "killScoreSeconds",
        "minScore",
    ] {
        expected[field] = actual[field].clone();
    }
    expected["prey"]["count"] = 0.into();
    assert_eq!(actual, expected);

    let run = || {
        let mut world = Simulator::new(9, Config::battledome()).unwrap();
        for slot in 0..16 {
            world.add(slot.to_string(), slot, 16).unwrap();
        }
        for t in 0..400 {
            let actions: Vec<_> = (0..16)
                .map(|slot| {
                    world.snakes[slot].alive.then(|| Action {
                        heading: slot as f64 * 0.4 + t as f64 * 0.01,
                        boost: (slot + t) % 5 == 0,
                    })
                })
                .collect();
            world.step(&actions).unwrap();
        }
        assert!(world.prey.is_empty());
        serde_json::to_string(&world).unwrap()
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
        change(&mut config);
        config.validate()
    };
    let pit = "pit.radius must be positive and less than radius";
    let ring = "spawnRadius must lie outside pit.radius and more than 102 inside radius";
    let window = "killScoreSeconds must be positive and finite";
    let floor = "minScore must be from a newborn's displayed 10 to";
    for (change, error) in [
        (
            (|c| c.pit = Some(PitConfig { radius: 0.0 })) as fn(&mut Config),
            pit,
        ),
        (|c| c.pit = Some(PitConfig { radius: 3584.0 }), pit),
        (|c| c.pit = Some(PitConfig { radius: f64::NAN }), pit),
        (|c| c.spawn_radius = Some(3584.0), ring),
        // A new life's tail, 42 outward, must keep the 60-unit spawn wall margin.
        (|c| c.spawn_radius = Some(3482.0), ring),
        (|c| c.spawn_radius = Some(1784.0), ring),
        (|c| c.spawn_radius = Some(-1.0), ring),
        (|c| c.spawn_radius = Some(f64::NAN), ring),
        (|c| c.kill_score_seconds = Some(0.0), window),
        (|c| c.kill_score_seconds = Some(f64::INFINITY), window),
        (|c| c.kill_score_seconds = Some(f64::NAN), window),
        (|c| c.min_score = Some(9), floor),
        (|c| c.min_score = Some(0), floor),
        (|c| c.min_score = Some(u32::MAX), floor),
    ] {
        let message = with(change).unwrap_err();
        assert!(message.starts_with(error), "{message}");
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
        config.validate().unwrap();
    }
    let mut config = Config::battledome();
    config.min_score = Some(high + 1);
    assert!(config.validate().unwrap_err().starts_with(floor));
    // A fractional score is not a displayed score.
    let error = serde_json::from_str::<Config>(r#"{"minScore":400.5}"#).unwrap_err();
    assert!(
        error.to_string().contains("invalid type: floating point"),
        "{error}"
    );
    // Without a pit, any distance inside the wall margin is a valid ring.
    with(|c| {
        c.pit = None;
        c.spawn_radius = Some(0.0);
    })
    .unwrap();
    with(|c| c.spawn_radius = Some(3481.0)).unwrap();
    assert!(
        Simulator::new(
            1,
            Config {
                spawn_radius: Some(1000.0),
                ..Config::sandbox()
            }
        )
        .is_err()
    );
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
        reply["error"]
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
        serde_json::from_str(r#"{"simulation":{"world":{"preset":"battledome"}}}"#).unwrap();
    host.validate().unwrap();
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

/// A food-free, kiosk-free sandbox with a 400-point floor and one snake.
fn floor_world() -> Simulator {
    let mut config = Config::sandbox();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    config.min_score = Some(400);
    let mut world = Simulator::new(11, config).unwrap();
    world.add("player", 0, 1).unwrap();
    world
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
        .snakes
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
    let mut config = floor_world().config;
    config.radius = 30_000.0;
    config.spawn_radius = Some(15_000.0);
    config.min_score = Some(high);
    let mut world = Simulator::new(3, config).unwrap();
    world.add("giant", 0, 1).unwrap();
    // A full growth unit grows the snake past 430 segments at its next anchor.
    set_mass(&mut world, MAX_SEGMENTS, 1.0);
    for _ in 0..100 {
        world.step(&straight(&world)).unwrap();
        assert!(world.snakes[0].score >= f64::from(high));
        if world.snakes[0].segments > MAX_SEGMENTS {
            break;
        }
    }
    let s = &world.snakes[0];
    assert!(s.alive && s.segments == MAX_SEGMENTS + 1, "{}", s.segments);
    assert_eq!(s.score, f64::from(high));
}

/// Give slot 0 a straight body of this mass along the x axis, facing +x.
fn set_mass(world: &mut Simulator, segments: usize, fam: f64) {
    let s = &mut world.snakes[0];
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
    Simulator::update_mass(s);
}

/// Boost straight ahead for `ticks`, returning every pellet dropped.
fn boost_drops(world: &mut Simulator, ticks: usize) -> Vec<slither_sim::Food> {
    world.food_spawn_events = Some(Vec::new());
    let mut drops = Vec::new();
    for _ in 0..ticks {
        world.step(&straight_boost(world)).unwrap();
        let events = world.food_spawn_events.as_ref().unwrap();
        drops.extend(events.iter().map(|event| event.food.clone()));
    }
    drops
}

#[test]
fn a_newborn_below_the_floor_boosts_free() {
    let mut world = floor_world();
    set_mass(&mut world, 2, 0.0);
    let cruise = world.snakes[0].speed;
    let drops = boost_drops(&mut world, 300);
    let s = &world.snakes[0];
    assert!(s.alive && world.is_boosting(0) && s.speed > cruise);
    assert_eq!((s.segments, s.fam, s.score), (2, 0.0, 10.0));
    assert_eq!(world.boost_accounting(0).unwrap().boost_mass_spent, 0.0);
    assert!(drops.len() > 5 && drops.iter().all(|food| food.value > 0.0));
}

#[test]
fn boosting_at_the_floor_drops_normal_pellets_for_free() {
    let floor = MassFloor::for_score(400).unwrap();
    let mut floored = floor_world();
    let mut config = floored.config.clone();
    config.min_score = None;
    let mut open = Simulator::new(11, config).unwrap();
    open.add("player", 0, 1).unwrap();
    for world in [&mut floored, &mut open] {
        set_mass(world, floor.segments, floor.fam);
    }
    let cruise = floored.snakes[0].speed;
    let (free, paid) = (boost_drops(&mut floored, 300), boost_drops(&mut open, 300));
    let s = &floored.snakes[0];
    assert!(s.alive && floored.is_boosting(0) && s.speed > cruise);
    assert_eq!(
        (s.segments, s.fam, s.score),
        (floor.segments, floor.fam, 400.0)
    );
    assert_eq!(floored.boost_accounting(0).unwrap().boost_mass_spent, 0.0);
    // The same rate, and the first pellet, at equal mass, is the one an
    // unfloored snake pays for.
    assert!(free.len() > 5, "{} drops", free.len());
    assert_eq!(free.len(), paid.len());
    assert_eq!(free[0], paid[0]);
    assert!(open.snakes[0].segments < floor.segments);
}

#[test]
fn boosting_above_the_floor_costs_normally_and_stops_there() {
    let floor = MassFloor::for_score(400).unwrap();
    let mut floored = floor_world();
    let mut config = floored.config.clone();
    config.min_score = None;
    let mut open = Simulator::new(11, config).unwrap();
    open.add("player", 0, 1).unwrap();
    for world in [&mut floored, &mut open] {
        let s = &mut world.snakes[0];
        s.segments = 40;
        s.fam = 0.5;
        s.body = (0..40)
            .map(|i| Point {
                x: 300.0 - 10.0 * i as f64,
                y: 0.0,
            })
            .collect();
        s.head = s.body[0];
        Simulator::update_mass(s);
    }
    // Until the floor is near, both pay and drop alike.
    for _ in 0..60 {
        floored.step(&circling(&floored, true)).unwrap();
        open.step(&circling(&open, true)).unwrap();
    }
    assert_eq!(floored.food, open.food);
    assert!(!floored.food.is_empty());
    assert_eq!(
        (floored.snakes[0].segments, floored.snakes[0].fam),
        (open.snakes[0].segments, open.snakes[0].fam)
    );
    // Boost cost is small at this size; give it a few simulated minutes.
    for _ in 0..50_000 {
        if (floored.snakes[0].segments, floored.snakes[0].fam) == (floor.segments, floor.fam) {
            break;
        }
        floored.step(&circling(&floored, true)).unwrap();
    }
    let s = &floored.snakes[0];
    assert_eq!(
        (s.segments, s.fam, s.score),
        (floor.segments, floor.fam, 400.0)
    );
    assert_eq!(s.body.len(), floor.segments);
    // It paid at least the starting mass above the floor; circling also eats
    // some of its own drops.
    let spent = floored.boost_accounting(0).unwrap().boost_mass_spent;
    assert!(spent >= 40.5 - (floor.segments as f64 + floor.fam) - 1e-9);
}

#[test]
fn kiosk_shrinking_stops_at_the_floor() {
    let floor = MassFloor::for_score(400).unwrap();
    let mut world = floor_world();
    world.config.kiosk = slither_sim::KioskConfig {
        floor_score: 100.0,
        grace_seconds: 0.0,
        stationary_radius: 1e5,
        shrink_per_second: 2.0,
        score_decay_per_second: 1.0,
        ..Default::default()
    };
    let s = &mut world.snakes[0];
    s.segments = 60;
    s.body = (0..60)
        .map(|i| Point {
            x: 300.0 - 5.0 * i as f64,
            y: 0.0,
        })
        .collect();
    s.head = s.body[0];
    Simulator::update_mass(s);
    for _ in 0..1000 {
        world.step(&circling(&world, false)).unwrap();
        let s = &world.snakes[0];
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
    let s = &world.snakes[0];
    assert!(s.alive);
    assert_eq!((s.segments, s.fam), (floor.segments, floor.fam));
    // Once physical shrinking ends at the floor, the displayed credit decays
    // as usual instead of stalling, here within a few seconds.
    let mut displayed = s.score;
    assert!(displayed > 400.0);
    for _ in 0..1000 {
        world.step(&circling(&world, false)).unwrap();
        let score = world.snakes[0].score;
        assert!(score <= displayed);
        displayed = score;
    }
    assert_eq!(world.snakes[0].score, 400.0);
}

#[test]
fn battledome_pit_deaths_drop_food_and_wall_deaths_none() {
    let mut config = Config::battledome();
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.kiosk.enabled = false;
    let mut world = Simulator::new(3, config).unwrap();
    world.add("leaver", 0, 2).unwrap();
    world.add("runner", 1, 2).unwrap();
    // Just inside the pit, heading out; and just inside the wall, heading out.
    place(&mut world, 0, Point { x: 1780.0, y: 0.0 }, 0.0);
    place(&mut world, 1, Point { x: 0.0, y: 3570.0 }, FRAC_PI_2);
    for _ in 0..20 {
        world.step(&straight(&world)).unwrap();
    }
    let mut deaths = world.deaths.clone();
    deaths.sort_by_key(|death| death.reason as u8);
    let reasons: Vec<_> = deaths.iter().map(|death| death.reason).collect();
    assert_eq!(reasons, [DeathReason::Boundary, DeathReason::Pit]);
    assert_eq!(deaths[0].food, 0);
    assert!(deaths[1].food > 0);
    assert_eq!(world.food.len(), deaths[1].food);
}

#[test]
fn death_food_follows_actual_mass_under_a_floor() {
    let mut open = kill_world(None);
    let mut floored = kill_world(None);
    floored.config.min_score = Some(400);
    let mut floored = Simulator::new(7, floored.config.clone()).unwrap();
    floored.add("victim", 0, 2).unwrap();
    floored.add("killer", 1, 2).unwrap();
    for (from, to) in open.snakes.clone().into_iter().zip(&mut floored.snakes) {
        *to = from;
    }
    open.step(&straight(&open)).unwrap();
    floored.step(&straight(&floored)).unwrap();
    assert_eq!(floored.deaths[0].reason, DeathReason::Body);
    assert!(floored.deaths[0].food > 0);
    assert_eq!(floored.deaths, open.deaths);
    assert_eq!(floored.food, open.food);
}
