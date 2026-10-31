use super::*;
use crate::{
    Config,
    client::{
        Minimap, WireEvent, encode, latency::LatencyConfig, protocol, visibility::SectorInterest,
    },
    model::{Food, Origin, Snake},
    physics,
};
use std::collections::BTreeSet;

use super::{
    broadcast::{Board, LEADERBOARD_ROWS, clock_due},
    connection::{MAX_RECORD_BYTES, RECORD_OVERHEAD},
    food::{FoodChanges, KnownFood, KnownSectors, eaten_sectors, food_identity, food_updates},
    snakes::{FAM_REPORT_TICKS, FRESH_ANCHORS, KnownSnake, MOTION_REPORT_TICKS, changes},
};

#[test]
fn food_rebuild_keeps_birth_colors_and_wire_cell_representatives() {
    let config: SimulationConfig = serde_json::from_value(serde_json::json!({
        "agents": 1,
        "world": {"preset": "sandbox", "foodCount": 0, "ambientReplenishPerSecond": 0}
    }))
    .unwrap();
    let mut backend = SimulatorBackend::new(7, config).unwrap();
    let food: Vec<_> = [(0., 1.), (100., 2.), (0., 9.), (0.1, 7.)]
        .into_iter()
        .map(|(x, size)| Food {
            x,
            y: 0.,
            value: 1.,
            size: Some(size),
            origin: Some(Origin::Ambient),
        })
        .collect();
    backend.world.replace_food(food.clone()).unwrap();
    backend.broadcast().unwrap();
    let sector = food::FoodIndex::sector(&food[0]).unwrap().0;
    let expected = backend.food_index.listing(sector);
    let mut saved = serde_json::to_value(backend.world.checkpoint().unwrap()).unwrap();
    saved["state"]["food"].as_array_mut().unwrap().reverse();
    saved["state"]["foodBirthOrder"]
        .as_array_mut()
        .unwrap()
        .reverse();
    backend
        .world
        .restore(&serde_json::to_vec(&saved).unwrap(), Default::default())
        .unwrap();
    backend.food_presentation = FoodPresentation::new(7, 1);
    backend.food_index.stale = true;
    backend.broadcast().unwrap();
    assert_eq!(backend.food_index.listing(sector), expected);
    assert_eq!(
        backend.world.food()[0],
        food[3],
        "the test must retain unordered storage"
    );
}

#[test]
fn population_sized_backends_hold_across_death_and_respawn() {
    let radius = 670. * 40_f64.sqrt();
    for world in [
        None,
        Some(serde_json::json!({})),
        Some(serde_json::json!({"preset": "official"})),
    ] {
        let mut input = serde_json::json!({"agents": 40});
        if let Some(world) = world {
            input["world"] = world;
        }
        let config: SimulationConfig = serde_json::from_value(input).unwrap();
        let mut backend = SimulatorBackend::new(7, config).unwrap();
        assert_eq!(backend.config().world.radius, radius);
        assert_eq!(backend.world.config().radius, radius);
        assert!(backend.world.config().dynamic_radius);
        assert_eq!(backend.world.config().food_count, 109 * 40);
        assert_eq!(
            backend.world.config().prey.count,
            Config::official(40).prey.count
        );
        assert!(
            (backend.world.config().ambient_replenish_per_second
                - Config::default().ambient_replenish_per_second * 40.)
                .abs()
                < 1e-9
        );
        backend.world.retire(0).unwrap();
        backend
            .world
            .respawn(
                0,
                Point {
                    x: radius - 1.,
                    y: 0.,
                },
                0.,
                2,
            )
            .unwrap();
        backend.slots[0].control = Action::default();
        backend.advance_ticks(1).unwrap();
        assert!(!backend.world.snakes()[0].alive);
        assert_eq!(backend.world.radius(), radius);
        backend.reconnect_slot(0).unwrap();
        backend.advance_ticks(1).unwrap();
        assert!(backend.world.snakes()[0].alive);
        assert_eq!(backend.world.radius(), radius);
    }
}

#[test]
fn backends_validate_spawn_rings_against_the_sized_arena() {
    let mut config = SimulationConfig {
        agents: 40,
        ..Default::default()
    };
    // Outside the 670-unit start, inside 670 * sqrt(40), about 4,237 units.
    config.world.spawn_radius = Some(2000.);
    config.validate().unwrap();
    let backend = SimulatorBackend::new(7, config.clone()).unwrap();
    assert_eq!(backend.world.config().radius, 670. * 40_f64.sqrt());
    config.world.size_to_population = false;
    assert!(config.validate().is_err());
    assert!(SimulatorBackend::new(7, config).is_err());
}

#[test]
fn validation_checks_the_prey_count_of_the_sized_world() {
    let mut config = SimulationConfig {
        agents: 484,
        ..Default::default()
    };
    // 200 prey scale with area to 96,800, beyond the u16 prey IDs.
    config.world.prey.count = 200;
    let error = config.validate().unwrap_err();
    assert!(error.contains("prey count must be at most"), "{error}");
    assert!(SimulatorBackend::new(7, config.clone()).is_err());
    config.world.size_to_population = false;
    // Growth still needs enough prey wire IDs even without startup sizing.
    assert!(config.validate().is_err());
    config.world.dynamic_radius = false;
    config.validate().unwrap();
}

#[test]
fn fixed_backend_worlds_keep_their_radius_and_food_settings() {
    for world in [
        serde_json::json!({"preset": "battledome"}),
        serde_json::json!({"preset": "sandbox"}),
        serde_json::json!({"radius": 900}),
        serde_json::json!({"preset": "official", "radius": 900}),
        serde_json::json!({"sizeToPopulation": false}),
        serde_json::json!({"radius": 9000, "sizeToPopulation": true}),
    ] {
        let config: SimulationConfig = serde_json::from_value(serde_json::json!({
            "agents": 40, "world": world,
        }))
        .unwrap();
        let expected = serde_json::to_value(&config.world).unwrap();
        let backend = SimulatorBackend::new(7, config).unwrap();
        assert_eq!(
            serde_json::to_value(backend.world.config()).unwrap(),
            expected
        );
        assert_eq!(
            serde_json::to_value(&backend.config().world).unwrap(),
            expected
        );
    }
}

#[test]
fn population_sizing_obeys_the_wire_radius_limit() {
    let mut config = SimulationConfig {
        agents: 2500,
        ..Default::default()
    };
    assert!(config.world.dynamic_radius);
    let error = config.validate().unwrap_err();
    assert!(error.contains("arena radii up to 32000"));
    // 670 × sqrt(2281) ≈ 31,999.9 fits; 2,282 seats does not.
    assert!(error.contains("reaches that at 2281 seats"), "{error}");
    config.agents = 2281;
    config.validate().unwrap();
    config.agents = 2500;
    assert!(
        SimulatorBackend::new(7, config.clone())
            .err()
            .unwrap()
            .contains("arena radii up to 32000")
    );
    config.world.size_to_population = false;
    config.world.dynamic_radius = false;
    config.validate().unwrap();
    config.world.dynamic_radius = true;
    assert!(config.validate().is_err());
}

#[test]
fn hosted_disconnects_shrink_the_arena_but_bot_deaths_and_respawns_do_not() {
    // Drive the same raw backend as the host, without sockets or wall time.
    let mut config = SimulationConfig {
        agents: 4,
        ..Default::default()
    };
    config.world = Config::official(4);
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    config.world.prey.count = 0;
    // The threaded host routes bots as remote clients too: two players, two bots.
    let mut host = RawSimulator::new(7, config, 4).unwrap();
    assert_eq!(host.backend.world.occupied(), 0);
    for slot in 0..4 {
        assert!(host.connect(slot, PlayerAppearance::default()).unwrap());
    }
    assert_eq!(host.backend.world.occupied(), 4);
    let radius = host.backend.world.radius();
    // Kill a hosted bot; its seat stays occupied during its respawn wait.
    let rim = Point {
        x: radius - 1.,
        y: 0.,
    };
    host.backend
        .world
        .correct_pose(2, rim, vec![rim; 2], 0.)
        .unwrap();
    host.backend.slots[2].control = Action::default();
    host.advance_ticks(1).unwrap();
    assert!(!host.backend.world.snakes()[2].alive);
    assert_eq!(host.backend.world.occupied(), 4);
    assert_eq!(host.backend.world.radius(), radius);
    for _ in 0..10 {
        host.advance_ticks(1).unwrap();
    }
    assert_eq!(host.backend.world.radius(), radius);
    assert!(host.connect(2, PlayerAppearance::default()).unwrap());
    host.advance_ticks(1).unwrap();
    assert_eq!(host.backend.world.occupied(), 4);
    assert_eq!(host.backend.world.radius(), radius);
    host.disconnect(0).unwrap();
    host.disconnect(1).unwrap();
    host.advance_ticks(1).unwrap();
    assert_eq!(host.backend.world.occupied(), 2);
    assert_eq!(host.backend.world.radius(), radius - 23.4375 * host.dt());
}

#[test]
fn bot_sized_host_admits_a_burst_of_thirty_players_without_waiting_for_growth() {
    for seed in [1, 7, 42] {
        let mut config = SimulationConfig {
            agents: 34,
            ..Default::default()
        };
        config.world.size_for_population(4); // Host startup sizing for four bots.
        // Hosted bots use the same remote admission path as socket players.
        let mut sim = RawSimulator::new(seed, config, 34).unwrap();
        let mut remotes = Vec::new();
        for slot in 0..4 {
            remotes.push(Remote::connect(&mut sim, slot, "Bot"));
        }
        sim.advance_ticks(1).unwrap();
        assert_eq!(sim.backend.world.radius(), 1340.);
        for slot in 4..34 {
            remotes.push(Remote::connect(&mut sim, slot, "Burst player"));
        }
        sim.advance_ticks(1).unwrap();
        assert_eq!(sim.backend.world.occupied(), 34);
        assert_eq!(
            sim.backend.world.radius(),
            crate::simulator::population_radius(34)
        );
        for (slot, remote) in remotes.iter_mut().enumerate() {
            remote.pump(&mut sim, slot);
            assert_eq!(
                remote.frame(&sim).status,
                ClientStatus::Playing,
                "seed {seed}, slot {slot}"
            );
            assert_eq!(
                remote.frame(&sim).observation.unwrap().arena.radius,
                crate::simulator::population_radius(34).round()
            );
        }
    }
}

#[test]
fn shrinking_food_and_surplus_prey_leave_both_decoded_client_views_without_pickups() {
    let mut config = SimulationConfig {
        agents: 2,
        ..Default::default()
    };
    config.world = Config::official(4);
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    config.world.prey.count = 8;
    config.world.prey.cruise_speed = 0.;
    config.world.prey.escape_speed = 0.;
    config.world.prey.sprint_speed = 0.;
    config.world.radius_updates = vec![crate::RadiusUpdate {
        time: 0.021,
        radius: 670.,
    }];
    let mut sim = RawSimulator::new(7, config, 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "Observer A");
    let mut b = Remote::connect(&mut sim, 1, "Observer B");
    for (slot, y) in [(0, -100.), (1, 100.)] {
        sim.backend.world.retire(slot).unwrap();
        sim.backend
            .world
            .respawn(slot, Point { x: 0., y }, 0., 2)
            .unwrap();
        sim.backend.slots[slot].control = Action::default();
    }
    // All prey remain in range and far from either mouth. Only the reduced
    // population target removes them; the sample isolates shrink from navigation.
    let prey = sim
        .backend
        .world
        .prey()
        .iter()
        .cloned()
        .enumerate()
        .map(|(i, mut prey)| {
            prey.position = Point {
                x: 0.,
                y: 300. + i as f64 * 20.,
            };
            prey.speed = 0.;
            crate::ReplayPrey {
                prey,
                mode: crate::PreyMode::Roaming,
            }
        })
        .collect();
    sim.backend.world.replace_prey(prey).unwrap();
    let pellet = |x, y, origin| Food {
        x,
        y,
        value: 1.,
        size: Some(2.),
        origin: Some(origin),
    };
    let outside = [
        pellet(1000., -60., Origin::Ambient),
        pellet(1000., 0., Origin::Boost),
        pellet(1000., 60., Origin::Death),
    ];
    let keys: Vec<_> = outside
        .iter()
        .map(|food| encode::food_key(food.point()).unwrap())
        .collect();
    let mut food = outside.to_vec();
    food.push(pellet(400., 0., Origin::Ambient));
    // Synthetic boost/death pellets need their source colors registered just
    // as a physics birth would; this setup does not fabricate any pickup.
    for food in &outside[1..] {
        sim.backend
            .food_presentation
            .births(&[crate::FoodSpawnEvent {
                source: Some(0),
                food: *food,
            }])
            .unwrap();
    }
    sim.backend.world.replace_food(food).unwrap();
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 2);
    for remote in [&a, &b] {
        let observation = remote.frame(&sim).observation.unwrap();
        assert_eq!(observation.food.len(), 4);
        assert_eq!(observation.prey.len(), 8);
    }
    let removed: Vec<_> = sim.backend.world.prey()[2..]
        .iter()
        .map(|p| sim.backend.prey_wire[&p.id])
        .collect();
    a.packets.clear();
    b.packets.clear();
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 1);
    assert_eq!(sim.backend.world.food().len(), 1);
    assert_eq!(sim.backend.world.prey().len(), 2);
    assert!(sim.backend.world.pickup_events().unwrap().is_empty());
    for remote in [&a, &b] {
        for &key in &keys {
            assert!(remote.packets.contains(&encode::food_remove(key, None)));
            assert!(
                !remote
                    .packets
                    .contains(&encode::sector_clear([key[0], key[1]]))
            );
        }
        for &wire in &removed {
            assert!(remote.packets.contains(&encode::prey_removal(wire, None)));
        }
        let observation = remote.frame(&sim).observation.unwrap();
        assert_eq!(observation.food.len(), 1);
        assert_eq!(observation.prey.len(), 2);
    }
}

#[test]
fn a_shrinking_wall_culls_edge_food_without_resynchronizing_quiet_sectors() {
    shrinking_food_cull(false);
}

#[test]
fn a_shrinking_pit_culls_edge_food_without_resynchronizing_quiet_sectors() {
    shrinking_food_cull(true);
}

fn shrinking_food_cull(pit_only: bool) {
    let mut config = SimulationConfig {
        agents: 2,
        ..Default::default()
    };
    config.world = Config::official(4);
    if pit_only {
        config.world.radius = 2680.;
        config.world.size_to_population = false;
        config.world.spawn_radius = Some(2000.);
        config.world.min_score = Some(10);
        config.world.pit = Some(crate::PitConfig { radius: 1340. });
        config.world.food_only_inside_pit = true;
    }
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    config.world.prey.count = 0;
    config.world.radius_updates = vec![crate::RadiusUpdate {
        time: 0.021,
        radius: if pit_only { 1340. } else { 670. },
    }];
    let mut sim = RawSimulator::new(7, config, 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "Observer A");
    let mut b = Remote::connect(&mut sim, 1, "Observer B");
    if pit_only {
        for slot in 0..2 {
            sim.physical_food(slot);
        }
    }
    for (slot, y) in [(0, -100.), (1, 100.)] {
        sim.backend.world.retire(slot).unwrap();
        sim.backend
            .world
            .respawn(slot, Point { x: 0., y }, 0., 2)
            .unwrap();
        sim.backend.slots[slot].control = Action::default();
    }
    let pellet = |x, y, origin| Food {
        x,
        y,
        value: 1.,
        size: Some(2.),
        origin: Some(origin),
    };
    let quiet = pellet(400., 0., Origin::Ambient);
    let edge = pellet(600., 0., Origin::Ambient);
    let outside = [
        pellet(700., 0., Origin::Ambient),
        pellet(1000., -60., Origin::Ambient),
        pellet(1000., 60., Origin::Death),
    ];
    sim.backend
        .food_presentation
        .births(&[crate::FoodSpawnEvent {
            source: Some(0),
            food: outside[2],
        }])
        .unwrap();
    let mut food = vec![quiet, edge];
    food.extend(outside.iter().cloned());
    sim.backend.world.replace_food(food).unwrap();
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 2);
    assert_eq!(sim.backend.world.food().len(), 5);
    // An ambient birth waiting for maintenance in the quiet sector, recorded
    // as a physics birth would be.
    let hidden = pellet(420., 0., Origin::Ambient);
    sim.backend
        .food_presentation
        .births(&[crate::FoodSpawnEvent {
            source: None,
            food: hidden,
        }])
        .unwrap();
    let color = sim.backend.food_presentation.newest(&hidden);
    sim.backend.food_index.add(&hidden, false, color);
    sim.backend.world.insert_food(hidden).unwrap();
    let key = |food: &Food| encode::food_key(food.point()).unwrap();
    let sector = |food: &Food| usize::from(FoodIndex::sector(food).unwrap().0);
    assert_eq!(sector(&hidden), sector(&quiet));
    assert_eq!(sector(&outside[0]), sector(&edge));
    let listing = sim.backend.food_sectors[sector(&quiet)].clone().unwrap();
    let edge_before = sim.backend.food_sectors[sector(&edge)].clone().unwrap();
    let memberships: Vec<_> = sim
        .backend
        .slots
        .iter()
        .map(|slot| slot.connection.food_interest.revision().clone())
        .collect();
    let colors = [&quiet, &edge].map(|food| sim.backend.food_presentation.color(food, 0));
    a.packets.clear();
    b.packets.clear();
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 1);
    assert_eq!(
        sim.backend.world.radius(),
        if pit_only { 1340. } else { 670. }
    );
    assert_eq!(sim.backend.world.food_radius(), 670.);
    assert_eq!(sim.backend.world.food().len(), 3);
    assert!(sim.backend.world.pickup_events().unwrap().is_empty());
    assert_eq!(sim.backend.food_index.count, 3);
    assert!(!sim.backend.food_index.stale);
    // Radius motion changes food, not the retained view stencil. Exercise
    // #188's unchanged-membership fast path while the culled listing refreshes.
    for (slot, revision) in sim.backend.slots.iter().zip(&memberships) {
        assert!(std::sync::Arc::ptr_eq(
            revision,
            slot.connection.food_interest.revision()
        ));
    }
    assert!(!std::sync::Arc::ptr_eq(
        &edge_before,
        sim.backend.food_sectors[sector(&edge)].as_ref().unwrap()
    ));
    // A full resynchronization would reveal the hidden birth early.
    assert!(std::sync::Arc::ptr_eq(
        &listing,
        sim.backend.food_sectors[sector(&quiet)].as_ref().unwrap()
    ));
    let edge_listing = &sim.backend.food_sectors[sector(&edge)].as_ref().unwrap().0;
    assert_eq!(edge_listing.len(), 1);
    assert_eq!(edge_listing[0].1.point, edge.point());
    assert_eq!(
        [&quiet, &edge].map(|food| sim.backend.food_presentation.color(food, 0)),
        colors
    );
    assert_eq!(sim.backend.food_presentation.lifetimes(&hidden), 1);
    for food in &outside {
        assert_eq!(sim.backend.food_presentation.lifetimes(food), 0);
    }
    for remote in [&a, &b] {
        for food in &outside {
            let key = key(food);
            assert!(remote.packets.contains(&encode::food_remove(key, None)));
            assert!(
                !remote
                    .packets
                    .iter()
                    .any(|p| p[0] == b'<' && p[1..5] == key)
            );
            assert!(
                !remote
                    .packets
                    .contains(&encode::sector_clear([key[0], key[1]]))
            );
        }
        for food in [&quiet, &edge, &hidden] {
            let key = key(food);
            assert!(
                !remote
                    .packets
                    .iter()
                    .any(|p| matches!(p[0], b'f' | b'b' | b'c' | b'<') && p[1..5] == key)
            );
        }
        assert_eq!(remote.frame(&sim).observation.unwrap().food.len(), 2);
    }
}

/// Original exhaustive admission, kept independent of the bounded scorer.
fn exhaustive_spawn_pose(
    world: &Simulator,
    random: &mut Random,
) -> Result<(Vec<Point>, f64), String> {
    let mut open: Option<(f64, Vec<Point>, f64)> = None;
    for _ in 0..SPAWN_ATTEMPTS {
        let distance = spawn::spawn_distance(world.config(), world.radius(), random.unit());
        let angle = random.unit() * TAU;
        let heading = random.unit() * TAU;
        let (body, heading) = world
            .spawn_body(
                Point {
                    x: angle.cos() * distance,
                    y: angle.sin() * distance,
                },
                heading,
            )
            .map_err(|error| error.to_string())?;
        if world.body_is_clear(&body, 600.) {
            return Ok((body, heading));
        }
        if let Some(margin) = world.spawn_margin(&body)
            && open.as_ref().is_none_or(|(best, ..)| margin > *best)
        {
            open = Some((margin, body, heading));
        }
    }
    match open {
        Some((margin, body, heading)) if margin > 0. => Ok((body, heading)),
        Some(_) => Err(
            "Spawn candidate search exhausted: no sampled position is clear of other snakes".into(),
        ),
        None => Err(
            "Spawn candidate search exhausted: no sampled position fits inside the arena".into(),
        ),
    }
}

#[test]
fn bounded_admission_matches_exhaustive_poses_errors_and_random_stream() {
    let mut outcomes = [0; 3];
    for seed in [7, 29] {
        for spacing in [None, Some(1000), Some(100)] {
            let mut backend = SimulatorBackend::new(
                seed,
                SimulationConfig {
                    world: Config::sandbox(),
                    agents: 2,
                    ..Default::default()
                },
            )
            .unwrap();
            for slot in 0..backend.world.snakes().len() {
                backend.world.retire(slot).unwrap();
            }
            if let Some(spacing) = spacing {
                let mut detached_neighbor = backend.world.snakes()[1].clone();
                let neighbor = &mut detached_neighbor;
                neighbor.alive = true;
                neighbor.body = (-1000..=1000)
                    .step_by(spacing)
                    .enumerate()
                    .flat_map(|(i, y)| {
                        let x = if i % 2 == 0 { -1000. } else { 1000. };
                        [Point { x, y: y as f64 }, Point { x: -x, y: y as f64 }]
                    })
                    .collect();
                neighbor.head = neighbor.body[0];
                backend
                    .world
                    .import_life(
                        1,
                        crate::ReplayLife::from_observed(
                            neighbor.clone(),
                            crate::InitialSpeed::Infer(neighbor.speed),
                        ),
                    )
                    .unwrap();
            }
            for _ in 0..3 {
                let mut random = backend.spawn_random.clone();
                let expected = exhaustive_spawn_pose(&backend.world, &mut random);
                let actual = backend.spawn_pose(0);
                assert_eq!(actual, expected, "seed {seed}, spacing {spacing:?}");
                assert_eq!(backend.spawn_random.unit(), random.unit());
                let kind = match actual {
                    Ok((body, _)) if backend.world.body_is_clear(&body, 600.) => 0,
                    Ok(_) => 1,
                    Err(_) => 2,
                };
                outcomes[kind] += 1;
            }
        }
    }
    assert!(outcomes.iter().all(|&count| count > 0), "{outcomes:?}");
    let mut backend = SimulatorBackend::new(7, SimulationConfig::default()).unwrap();
    backend.world.retire(0).unwrap();
    crate::client::fixtures::set_live_radius(&mut backend.world, 50.);
    let mut random = backend.spawn_random.clone();
    assert_eq!(
        backend.spawn_pose(0),
        exhaustive_spawn_pose(&backend.world, &mut random)
    );
    assert_eq!(backend.spawn_random.unit(), random.unit());
}

#[test]
fn bounded_spawn_margins_only_prune_losing_candidates() {
    let mut world = Simulator::new(crate::WorldOptions {
        seed: 7,
        config: Config::battledome(),
        population: crate::InitialPopulation::Placed {
            ids: (0..8).map(|slot| slot.to_string()).collect(),
            placement: crate::Placement::TestRing,
        },
    })
    .unwrap();
    for angle in [0., 0.1, 1., 3.] {
        let (body, _) = world
            .spawn_body(
                Point {
                    x: 2684. * f64::cos(angle),
                    y: 2684. * f64::sin(angle),
                },
                angle,
            )
            .unwrap();
        let exact = world.spawn_margin(&body).unwrap();
        for cutoff in [f64::NEG_INFINITY, -100., 0., 100., exact, f64::INFINITY] {
            let bound = world.spawn_margin_bounded(&body, cutoff).unwrap();
            assert!(bound >= exact);
            assert_eq!(bound > cutoff, exact > cutoff);
            if bound > cutoff {
                assert_eq!(bound.to_bits(), exact.to_bits());
            }
        }
    }
    for body in [
        vec![],
        vec![Point::default(); 431],
        vec![Point { x: f64::NAN, y: 0. }; 2],
        vec![Point { x: 3584., y: 0. }; 2],
    ] {
        assert_eq!(world.spawn_margin_bounded(&body, 0.), None);
    }
    for slot in 0..world.snakes().len() {
        world.retire(slot).unwrap();
    }
    assert_eq!(
        world.spawn_margin_bounded(&[Point::default(); 2], 0.),
        Some(f64::INFINITY)
    );
}

#[test]
fn circular_spawns_clear_bodies_and_two_seconds_of_forward_travel() {
    let mut backend = SimulatorBackend::new(
        29,
        SimulationConfig {
            agents: 8,
            world: Config::battledome(),
            ..Default::default()
        },
    )
    .unwrap();
    for slot in 0..8 {
        let (body, _) = backend.spawn_pose(slot).unwrap();
        let alive = backend.world.snakes()[slot].alive;
        backend.world.retire(slot).unwrap();
        assert!(
            backend
                .world
                .body_is_clear(&body, spawn::CIRCULAR_CLEARANCE)
        );
        assert!(backend.world.body_clears_head_paths(&body));
        assert!(
            body.iter()
                .all(|p| p.distance(Point::default()) >= 2684. - 1e-9)
        );
        {
            let mut snake = backend.world.snakes()[slot].clone();
            snake.alive = alive;
            backend
                .world
                .import_life(
                    slot,
                    crate::ReplayLife::from_observed(
                        snake.clone(),
                        crate::InitialSpeed::Infer(snake.speed),
                    ),
                )
                .unwrap();
        }
    }
    // A newborn can clear the present body and still block an approaching head.
    let mut world = Simulator::new(crate::WorldOptions {
        seed: 7,
        config: Config {
            radius: 5000.,
            ..Config::sandbox()
        },
        population: crate::InitialPopulation::Placed {
            ids: (0..1).map(|slot| slot.to_string()).collect(),
            placement: crate::Placement::TestRing,
        },
    })
    .unwrap();
    let mut detached_snake = world.snakes()[0].clone();
    let snake = &mut detached_snake;
    snake.head = Point { x: 1000., y: 0. };
    snake.body = vec![snake.head, Point { x: 958., y: 0. }];
    snake.heading = 0.;
    snake.speed = physics::BOOST_SPEED;
    let body: Vec<_> = (0..26)
        .rev()
        .map(|i| Point {
            x: 2100.,
            y: i as f64 * 42.,
        })
        .collect();
    // The head misses the zone; the tail must be checked too.
    world
        .import_life(
            0,
            crate::ReplayLife::from_observed(
                snake.clone(),
                crate::InitialSpeed::Infer(snake.speed),
            ),
        )
        .unwrap();
    assert!(world.body_is_clear(&body, 600.));
    assert!(!world.body_clears_head_paths(&body));
    {
        let mut snake = world.snakes()[0].clone();
        snake.heading = std::f64::consts::PI;
        world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    assert!(world.body_clears_head_paths(&body));
}

#[test]
fn a_circular_join_waits_instead_of_using_a_positive_tight_margin() {
    let mut backend = SimulatorBackend::new(
        7,
        SimulationConfig {
            agents: 2,
            world: Config::battledome(),
            ..Default::default()
        },
    )
    .unwrap();
    backend.world.retire(0).unwrap();
    // Concentric arcs cover the whole band within 600, while leaving positive
    // contact margins. This used to admit the most open of these tight spots.
    let body: Vec<_> = (0..360)
        .map(|i| {
            let a = i as f64 * TAU / 359.;
            Point {
                x: 3000. * a.cos(),
                y: 3000. * a.sin(),
            }
        })
        .collect();
    {
        let mut snake = backend.world.snakes()[1].clone();
        snake.head = body[0];
        backend
            .world
            .correct_pose(1, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    {
        let mut snake = backend.world.snakes()[1].clone();
        snake.body = body;
        backend
            .world
            .correct_mass(1, snake.body.clone(), snake.fam)
            .unwrap();
        backend
            .world
            .correct_pose(1, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    let (candidate, _) = backend
        .world
        .spawn_body(Point { x: 2684., y: 0. }, 0.)
        .unwrap();
    assert!(backend.world.spawn_margin(&candidate).unwrap() > 0.);
    assert!(!backend.world.body_is_clear(&candidate, 600.));
    assert_eq!(backend.spawn_pose(0).unwrap_err(), spawn::WAITING);
    assert!(!backend.world.snakes()[0].alive);
}

#[test]
fn a_clock_past_u64_periods_stays_due() {
    let mut next = 1;
    assert!(clock_due(&mut next, 1., 1e-20));
    assert_eq!(next, u64::MAX);
    assert!(clock_due(&mut next, 1.007, 1e-20));
}

#[test]
fn failed_ring_admission_preserves_the_existing_life() {
    let mut world = Config::sandbox();
    world.food_count = 0;
    world.ambient_replenish_per_second = 0.0;
    world.kiosk.enabled = false;
    world.min_score = Some(400);
    world.spawn_radius = Some(174.2208260217339);
    let mut backend = SimulatorBackend::new(
        5,
        SimulationConfig {
            world,
            agents: 1,
            ..Default::default()
        },
    )
    .unwrap();
    // A measured live radius can shrink inside the initial spawn ring.
    // Failed admission must preserve the existing life.
    crate::client::fixtures::set_live_radius(&mut backend.world, 100.0);
    let before = format!("{:?}", backend.world);
    let life = backend.slots[0].life;
    let connections = backend.connections;
    assert_eq!(backend.reconnect_slot(0).unwrap_err(), spawn::WAITING);
    assert_eq!(format!("{:?}", backend.world), before);
    assert_eq!(backend.slots[0].life, life);
    assert_eq!(backend.connections, connections);
    backend.world.retire(0).unwrap();
    assert!(backend.spawn_pose(0).is_err());
    assert!(!backend.world.snakes()[0].alive);
}

#[test]
fn a_full_arena_refuses_a_join_inside_contact_range() {
    let mut world = Config::sandbox();
    world.food_count = 0;
    world.ambient_replenish_per_second = 0.0;
    world.kiosk.enabled = false;
    let mut backend = SimulatorBackend::new(
        5,
        SimulationConfig {
            world,
            agents: 2,
            ..Default::default()
        },
    )
    .unwrap();
    // Rows 100 apart leave every point within 50 of slot 1's body, inside
    // even a two-segment contact range plus 40.
    let mut rows = Vec::new();
    for (i, y) in (-1000..=1000).step_by(100).enumerate() {
        let (x, y) = (if i % 2 == 0 { -1000. } else { 1000. }, y as f64);
        rows.extend([Point { x, y }, Point { x: -x, y }]);
    }
    let mut detached_neighbor = backend.world.snakes()[1].clone();
    let neighbor = &mut detached_neighbor;
    neighbor.head = rows[0];
    neighbor.body = rows[1..].to_vec();
    backend
        .world
        .correct_mass(1, neighbor.body.clone(), neighbor.fam)
        .unwrap();
    backend
        .world
        .correct_pose(1, neighbor.head, neighbor.body.clone(), neighbor.heading)
        .unwrap();
    let neighbor = backend.world.snakes()[1].clone();
    let life = backend.slots[0].life;

    assert_eq!(
        backend.reconnect_slot(0).unwrap_err(),
        "Spawn candidate search exhausted: no sampled position is clear of other snakes"
    );
    assert_eq!(backend.world.snakes()[1], neighbor);
    assert_eq!(backend.slots[0].life, life);
}

/// Diff against freshly allocated listings, as after a changed broadcast.
fn diff_food(
    packets: &mut Vec<Vec<u8>>,
    connection: &mut Connection,
    sectors: &BTreeMap<[u8; 2], BTreeMap<[u8; 2], KnownFood>>,
    eaters: &BTreeMap<(u64, u64), u16>,
    subscribed: &BTreeSet<[u8; 2]>,
) {
    let mut listings = vec![None; 1 << 16];
    for sector in subscribed {
        let food = sectors.get(sector).into_iter().flatten();
        let listing = SectorFood(food.map(|(offset, food)| (*offset, *food)).collect());
        listings[usize::from(u16::from_be_bytes(*sector))] = Some(Arc::new(listing));
    }
    food_updates(
        packets,
        &mut connection.food,
        &SectorInterest::from_subscribed(subscribed.clone()),
        &listings,
        &FoodChanges {
            changed: subscribed.iter().copied().collect(),
            eaten: eaten_sectors(eaters),
            ..Default::default()
        },
        eaters,
    );
}

#[test]
fn a_retained_listing_still_reports_pickups_at_its_points() {
    let point = Point { x: 650., y: -149. };
    let [sx, sy, rx, ry] = encode::food_key(point).unwrap();
    let food = KnownFood {
        point,
        decorative: false,
        style: encode::FoodStyle {
            color: 3,
            size: 20,
            snake_food: false,
        },
    };
    let mut sectors = vec![None; 1 << 16];
    sectors[usize::from(u16::from_be_bytes([sx, sy]))] =
        Some(Arc::new(SectorFood(vec![([rx, ry], food)])));
    let interest = SectorInterest::from_subscribed(BTreeSet::from([[sx, sy]]));
    let mut known = KnownSectors::default();
    let mut packets = Vec::new();
    let none = BTreeMap::new();
    let initial = FoodChanges::default();
    let unchanged = FoodChanges {
        previous: initial.current.clone(),
        ..Default::default()
    };
    food_updates(
        &mut packets,
        &mut known,
        &interest,
        &sectors,
        &initial,
        &none,
    );
    assert_eq!(packets.len(), 1);
    packets.clear();
    food_updates(
        &mut packets,
        &mut known,
        &interest,
        &sectors,
        &unchanged,
        &none,
    );
    assert!(packets.is_empty());
    // A co-located refill keeps an equal listing; the pickup still animates.
    let eaters = BTreeMap::from([(food_identity(point), 4)]);
    food_updates(
        &mut packets,
        &mut known,
        &interest,
        &sectors,
        &FoodChanges {
            changed: eaten_sectors(&eaters).into_iter().collect(),
            eaten: eaten_sectors(&eaters),
            previous: unchanged.current.clone(),
            ..Default::default()
        },
        &eaters,
    );
    assert_eq!(
        packets,
        [
            encode::food_remove([sx, sy, rx, ry], Some(4)),
            encode::food_add([sx, sy, rx, ry], food.style)
        ]
    );
}

#[test]
fn food_replacement_removes_the_old_item_with_its_actual_eater() {
    let point = Point::default();
    let [sx, sy, rx, ry] = encode::food_key(point).unwrap();
    let first = KnownFood {
        point,
        decorative: false,
        style: encode::FoodStyle {
            color: 3,
            size: 20,
            snake_food: true,
        },
    };
    let second = KnownFood {
        point,
        decorative: false,
        style: encode::FoodStyle {
            color: 7,
            size: 40,
            snake_food: true,
        },
    };
    let mut connection = Connection::default();
    let mut sectors = BTreeMap::from([([sx, sy], BTreeMap::from([([rx, ry], first)]))]);
    let mut packets = Vec::new();
    diff_food(
        &mut packets,
        &mut connection,
        &sectors,
        &BTreeMap::new(),
        &BTreeSet::from([[sx, sy]]),
    );
    assert!(packets.contains(&vec![b'F', sx, sy, 3, rx, ry, 20]));
    sectors.get_mut(&[sx, sy]).unwrap().insert([rx, ry], second);
    let eaters = BTreeMap::from([(food_identity(point), 2)]);
    packets.clear();
    diff_food(
        &mut packets,
        &mut connection,
        &sectors,
        &eaters,
        &BTreeSet::from([[sx, sy]]),
    );
    assert_eq!(
        packets,
        [
            encode::food_remove([sx, sy, rx, ry], Some(2)),
            encode::food_add([sx, sy, rx, ry], second.style)
        ]
    );
    packets.clear();
    diff_food(
        &mut packets,
        &mut connection,
        &sectors,
        &BTreeMap::new(),
        &BTreeSet::from([[sx, sy]]),
    );
    assert!(packets.is_empty());
}

#[test]
fn presentation_telemetry_preserves_physics_and_source_colors() {
    let mut config = raw_config(2);
    config.world.boost_food.enabled = false;
    let mut sim = RawSimulator::new(7, config, 2).unwrap();
    let mut profile = profile("Boost");
    profile.skin = 3;
    sim.connect(0, profile).unwrap();
    let mut reference = sim.backend.world.clone();
    reference.record_events(crate::EventRecording {
        food_births: false,
        pickups: reference.pickup_events().is_some(),
    });
    reference.record_events(crate::EventRecording {
        pickups: false,
        food_births: reference.food_spawn_events().is_some(),
    });
    let mut births = 0;
    for _ in 0..160 {
        sim.backend.slots[0].control.boost = true;
        let actions: Vec<_> = sim
            .backend
            .world
            .snakes()
            .iter()
            .zip(&sim.backend.slots)
            .map(|(snake, slot)| snake.alive.then_some(slot.control))
            .collect();
        reference.step(&actions).unwrap();
        sim.backend.advance().unwrap();
        assert_eq!(sim.backend.world.snakes(), reference.snakes());
        assert_eq!(sim.backend.world.food(), reference.food());
        assert_eq!(sim.backend.world.prey(), reference.prey());
        for event in sim.backend.world.food_spawn_events().unwrap() {
            if event.source == Some(0) {
                births += 1;
                assert_eq!(sim.backend.food_presentation.newest(&event.food), 3);
            }
        }
    }
    assert!(births > 0);
}

#[test]
fn sector_announcements_follow_subscription_entries_and_exits() {
    let mut connection = Connection::default();
    let mut packets = Vec::new();
    let sectors = BTreeMap::new();
    let eaters = BTreeMap::new();
    diff_food(
        &mut packets,
        &mut connection,
        &sectors,
        &eaters,
        &BTreeSet::from([[108, 108]]),
    );
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/client-food-interest.json"
    ))
    .unwrap();
    assert_eq!(evidence["announcements"]["W"], 0);
    assert_eq!(evidence["announcements"]["emptyF"], 0);
    assert!(packets.is_empty());
    assert!(connection.food.contains_key(&[108, 108]));
    packets.clear();
    diff_food(
        &mut packets,
        &mut connection,
        &sectors,
        &eaters,
        &BTreeSet::from([[108, 108]]),
    );
    assert!(packets.is_empty());
    diff_food(
        &mut packets,
        &mut connection,
        &sectors,
        &eaters,
        &BTreeSet::from([[110, 108]]),
    );
    assert_eq!(packets, [vec![b'w', 108, 108]]);
    packets.clear();
    // Food arriving after an empty subscription is a birth, not a new
    // subscription or a replayed stale listing.
    let point = Point { x: 650., y: -149. };
    let food = KnownFood {
        point,
        decorative: false,
        style: encode::FoodStyle {
            color: 3,
            size: 20,
            snake_food: false,
        },
    };
    let sectors = BTreeMap::from([([110, 108], BTreeMap::from([([0, 0], food)]))]);
    diff_food(
        &mut packets,
        &mut connection,
        &sectors,
        &eaters,
        &BTreeSet::from([[110, 108]]),
    );
    assert_eq!(packets, [encode::food_add([110, 108, 0, 0], food.style)]);
}
use std::f64::consts::PI;

fn raw_config(agents: usize) -> SimulationConfig {
    SimulationConfig {
        agents,
        world: Config {
            vision: 3000.,
            ..Config::sandbox()
        },
        ..Default::default()
    }
}

#[test]
fn interrupted_broadcast_resumes_food_diffs_for_later_recipients() {
    for decorations in [false, true] {
        let mut config = raw_config(2);
        if decorations {
            config.world = Config::battledome();
        }
        config.world.food_count = 0;
        config.world.ambient_replenish_per_second = 0.;
        let mut backend = SimulatorBackend::new(7, config).unwrap();
        for slot in &mut backend.slots {
            slot.connection.decorations = decorations;
        }
        backend.advance_ticks(4).unwrap();
        assert!(backend.slots.iter().all(|slot| slot.connection.setup));
        for slot in &mut backend.slots {
            slot.connection.downlink.clear();
        }
        let point = backend.world.snakes()[1].head;
        let food = Food {
            x: point.x,
            y: point.y,
            value: 1.,
            size: Some(2.),
            origin: None,
        };
        let [sx, sy, rx, ry] = encode::food_key(point).unwrap();
        let index = usize::from(u16::from_be_bytes([sx, sy]));
        let old = backend.slots[1]
            .connection
            .food
            .get(&[sx, sy])
            .unwrap()
            .clone();
        backend.world.insert_food(food).unwrap();
        backend.slots[0].connection.pongs = 1;
        // Synthetic queue pressure interrupts emission before recipient one.
        backend.slots[0]
            .connection
            .downlink
            .resize(super::connection::MAX_IN_FLIGHT, (f64::MAX, vec![0]));
        assert_eq!(backend.broadcast().unwrap_err(), "Downlink queue is full");
        let current = backend.food_sectors[index].as_ref().unwrap().clone();
        assert!(!Arc::ptr_eq(&old, &current));
        assert!(Arc::ptr_eq(
            &old,
            backend.slots[1].connection.food.get(&[sx, sy]).unwrap()
        ));
        backend.slots[0].connection.downlink.clear();
        backend.broadcast().unwrap(); // No further food or membership changes.
        assert!(Arc::ptr_eq(
            &current,
            backend.food_sectors[index].as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(
            &current,
            backend.slots[1].connection.food.get(&[sx, sy]).unwrap()
        ));
        if decorations {
            assert!(Arc::ptr_eq(
                backend.decorated_food_sectors[index].as_ref().unwrap(),
                backend.slots[1]
                    .connection
                    .decorated_food
                    .get(&[sx, sy])
                    .unwrap(),
            ));
        } else {
            let mut packets = Vec::new();
            for (_, frame) in &backend.slots[1].connection.downlink {
                packets.extend(
                    protocol::split_frame(frame)
                        .unwrap()
                        .into_iter()
                        .map(<[u8]>::to_vec),
                );
            }
            let style = current[&[rx, ry]].style;
            assert!(packets.contains(&encode::food_add([sx, sy, rx, ry], style)));
        }
    }
}

#[test]
fn coverage_finds_an_edited_pose_after_an_interrupted_broadcast() {
    let mut config = raw_config(2);
    config.world.radius = 6000.;
    config.world.vision = 1500.;
    let mut backend = SimulatorBackend::new(7, config).unwrap();
    backend.advance_ticks(4).unwrap();
    let place = |backend: &mut SimulatorBackend, slot: usize, x: f64| {
        let snake = &backend.world.snakes()[slot];
        let shift = x - snake.head.x;
        let body = snake
            .body
            .iter()
            .map(|p| Point {
                x: p.x + shift,
                y: p.y - snake.head.y,
            })
            .collect();
        backend
            .world
            .correct_pose(slot, Point { x, y: 0. }, body, snake.heading)
            .unwrap();
    };
    place(&mut backend, 0, 0.);
    place(&mut backend, 1, 4000.);
    backend.broadcast().unwrap();
    assert!(!backend.slots[0].connection.snakes.contains_key(1));
    backend.slots[0].connection.pongs = 1;
    backend.slots[0]
        .connection
        .downlink
        .resize(super::connection::MAX_IN_FLIGHT, (f64::MAX, vec![0]));
    assert_eq!(backend.broadcast().unwrap_err(), "Downlink queue is full");
    backend.slots[0].connection.downlink.clear();
    place(&mut backend, 1, 100.);
    backend.broadcast().unwrap();
    assert!(backend.slots[0].connection.snakes.contains_key(1));
    assert!(
        backend.slots[0]
            .connection
            .downlink
            .iter()
            .any(|(_, frame)| {
                protocol::split_frame(frame)
                    .unwrap()
                    .iter()
                    .any(|p| p[0] == b's' && p.len() > 7 && p[1..3] == 1u16.to_be_bytes())
            })
    );
}

#[test]
fn hosted_and_native_snakes_use_finite_whole_body_coverage() {
    let mut config = raw_config(3);
    config.world.radius = 6000.;
    config.world.vision = Config::default().vision;
    assert_eq!(config.world.vision, 1500.);
    let mut sim = RawSimulator::new(7, config, 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "Browser");
    for (slot, x) in [(0, 0.), (1, 1000.), (2, 4000.)] {
        sim.backend.world.retire(slot).unwrap();
        sim.backend
            .world
            .respawn(slot, Point { x, y: 0. }, 0., 2)
            .unwrap();
        sim.backend.slots[slot].control.heading = 0.;
    }
    run(&mut sim, &mut [(0, &mut remote)], 12);
    assert!(sim.backend.slots[0].connection.snakes.contains_key(1));
    assert!(!sim.backend.slots[0].connection.snakes.contains_key(2));
    assert!(sim.backend.slots[1].connection.snakes.contains_key(0));
    assert!(!sim.backend.slots[1].connection.snakes.contains_key(2));

    // A distant head does not hide a snake whose tail reaches the view.
    sim.backend.world.retire(2).unwrap();
    sim.backend
        .world
        .respawn(2, Point { x: 2500., y: 0. }, 0., 40)
        .unwrap();
    sim.backend.broadcast().unwrap();
    remote.pump(&mut sim, 0);
    assert!(sim.backend.slots[0].connection.snakes.contains_key(2));
    {
        let snake = &sim.backend.world.snakes()[2];
        let mut body = snake.body.clone();
        for point in &mut body {
            point.x += 3000.;
        }
        sim.backend
            .world
            .correct_pose(2, snake.head, body, snake.heading)
            .unwrap();
    }
    {
        let mut snake = sim.backend.world.snakes()[2].clone();
        snake.head.x += 3000.;
        sim.backend
            .world
            .correct_pose(2, snake.head, snake.body, snake.heading)
            .unwrap();
    }
    sim.backend.broadcast().unwrap();
    assert!(
        remote
            .pump(&mut sim, 0)
            .contains(&encode::removal(2, false))
    );
    sim.backend.world.retire(1).unwrap();
    sim.backend.broadcast().unwrap();
    assert!(remote.pump(&mut sim, 0).contains(&encode::removal(1, true)));
}

#[test]
fn ground_colors_are_shared_across_connections_and_reconnects() {
    let mut sim = RawSimulator::new(7, raw_config(2), 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "A");
    let mut b = Remote::connect(&mut sim, 1, "B");
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 12);
    let food = *sim
        .backend
        .world
        .food()
        .iter()
        .find(|food| {
            let key = encode::food_key(food.point()).unwrap();
            (0..2).all(|slot| {
                sim.backend.slots[slot]
                    .connection
                    .food
                    .get(&[key[0], key[1]])
                    .is_some_and(|sector| sector.contains_key(&[key[2], key[3]]))
                    && food.point().distance(sim.backend.world.snakes()[slot].head) > 100.
            })
        })
        .expect("shared ground food");
    let key = encode::food_key(food.point()).unwrap();
    let style = |sim: &RawSimulator, slot: usize| {
        sim.backend.slots[slot].connection.food[&[key[0], key[1]]][&[key[2], key[3]]].style
    };
    let original = style(&sim, 0);
    assert_eq!(original, style(&sim, 1));
    let previous_head = sim.backend.world.snakes()[1].head;
    let mut b = Remote::connect(&mut sim, 1, "New life");
    // Hold the observer location for this subscription/color regression;
    // random new spawn locations can legitimately see a different sector.
    let mut snake = sim.backend.world.snakes()[1].clone();
    let delta = Point {
        x: previous_head.x - snake.head.x,
        y: previous_head.y - snake.head.y,
    };
    for point in &mut snake.body {
        point.x += delta.x;
        point.y += delta.y;
    }
    sim.backend
        .world
        .correct_pose(1, previous_head, snake.body, snake.heading)
        .unwrap();
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 6);
    assert_eq!(original, style(&sim, 1));
}

#[test]
fn hosted_background_lives_use_safe_admission_from_startup() {
    for seed in 1..=16 {
        let mut config = raw_config(5);
        config.world.radius = 3000.;
        let mut sim = RawSimulator::new(seed, config, 1).unwrap();
        assert!(!sim.backend.world.snakes()[0].alive);
        for slot in 1..5 {
            let snake = sim.backend.world.snakes()[slot].clone();
            assert!(snake.alive, "seed {seed}, slot {slot}");
            sim.backend.world.retire(slot).unwrap();
            assert!(sim.backend.world.spawn_is_clear(
                snake.head,
                snake.heading,
                snake.segments,
                600.
            ));
            {
                let mut snake = sim.backend.world.snakes()[slot].clone();
                snake.alive = true;
                sim.backend
                    .world
                    .import_life(
                        slot,
                        crate::ReplayLife::from_observed(
                            snake.clone(),
                            crate::InitialSpeed::Infer(snake.speed),
                        ),
                    )
                    .unwrap();
            }
        }
    }
}

#[test]
fn background_respawn_cannot_appear_close_to_a_living_player() {
    for seed in 1..=32 {
        let mut sim = SimulatorBackend::new(seed, raw_config(2)).unwrap();
        sim.world.retire(1).unwrap();
        sim.reconnect_slot(1).unwrap();
        let new_life = sim.world.snakes()[1].clone();
        // Check the complete new body against the existing player's head
        // and body, rather than only preventing overlap at the new head.
        sim.world.retire(1).unwrap();
        assert!(
            sim.world
                .spawn_is_clear(new_life.head, new_life.heading, 2, 600.),
            "seed {seed}"
        );
    }
}

#[test]
fn minimaps_follow_the_server_clock_and_reset_after_death_and_reconnect() {
    let mut config = raw_config(1);
    config.world.radius = 10_000.;
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    let mut sim = RawSimulator::new(7, config, 1).unwrap();
    let interval = sim.backend.config.minimap_interval_seconds;
    // Each map leaves in the first tick of a server-wide period.
    let on_clock = |time: f64, dt: f64| (time / interval).fract() * interval < dt + 1e-9;
    let mut remote = Remote::connect(&mut sim, 0, "Map");
    // Keep this timing fixture away from the rim and random food.
    sim.backend.world.retire(0).unwrap();
    sim.backend
        .world
        .respawn(0, Point::default(), 0., 2)
        .unwrap();
    sim.backend.slots[0].control = Action {
        heading: 0.,
        boost: false,
    };
    let mut times = Vec::new();
    for _ in 0..760 {
        sim.advance_ticks(1).unwrap();
        let packets = remote.pump(&mut sim, 0);
        if let Some(index) = packets.iter().position(|p| matches!(p[0], b'M' | b'V')) {
            // Official mixed frames put the map before gameplay updates.
            assert_eq!(index, 0);
            if packets[0][0] == b'M' {
                assert_eq!(&packets[0][1..3], &136_u16.to_be_bytes());
            }
            times.push(sim.time());
        }
    }
    assert_eq!(times.len(), 2);
    assert!(times.iter().all(|&time| on_clock(time, sim.dt())));

    sim.backend.world.retire(0).unwrap();
    sim.advance_ticks(400).unwrap();
    assert!(
        remote
            .pump(&mut sim, 0)
            .iter()
            .all(|p| !matches!(p[0], b'M' | b'V'))
    );
    // A new connection joins the running phase with a full map, not a
    // delta against the previous life's bitmap.
    let joined = sim.time();
    let mut next_life = Remote::connect(&mut sim, 0, "New map");
    let mut first = None;
    while first.is_none() {
        sim.advance_ticks(1).unwrap();
        first = next_life
            .pump(&mut sim, 0)
            .into_iter()
            .find(|p| matches!(p[0], b'M' | b'V'))
            .map(|packet| (sim.time(), packet));
    }
    let (time, packet) = first.unwrap();
    assert_eq!(packet[0], b'M');
    assert!(on_clock(time, sim.dt()));
    assert!(time - joined < interval - 0.5);
}

/// Send a minimap to each of `slots` that can take one, sharing one
/// raster as a broadcast does.
fn send_minimaps(sim: &mut RawSimulator, slots: &[usize]) {
    let now = sim.time();
    let mut map = None;
    for &slot in slots {
        if let Some(packet) = sim.backend.minimap(slot, &mut map).unwrap() {
            sim.backend.emit(slot, now, &[packet]).unwrap();
        }
    }
}

#[test]
fn minimap_deltas_use_each_recipients_last_sent_bitmap() {
    let mut config = raw_config(2);
    config.world.radius = 10_000.;
    let mut sim = RawSimulator::new(7, config, 2).unwrap();
    let mut first = Remote::connect(&mut sim, 0, "First");
    let mut second = Remote::connect(&mut sim, 1, "Second");
    run(&mut sim, &mut [(0, &mut first), (1, &mut second)], 7);
    send_minimaps(&mut sim, &[0, 1]);
    assert!(first.pump(&mut sim, 0).iter().any(|p| p[0] == b'M'));
    assert!(second.pump(&mut sim, 1).iter().any(|p| p[0] == b'M'));
    let original = second.frame(&sim).minimap.unwrap();

    // Send only to the first recipient after changing global geometry.
    // The second recipient must retain its own older XOR base.
    {
        let mut snake = sim.backend.world.snakes()[1].clone();
        snake.head = Point {
            x: 3000.,
            y: -3000.,
        };
        sim.backend
            .world
            .correct_pose(1, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    {
        let mut snake = sim.backend.world.snakes()[1].clone();
        snake.body = vec![
            Point {
                x: 3100.,
                y: -3000.,
            };
            2
        ];
        sim.backend
            .world
            .correct_mass(1, snake.body.clone(), snake.fam)
            .unwrap();
        sim.backend
            .world
            .correct_pose(1, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    send_minimaps(&mut sim, &[0]);
    assert!(
        first
            .pump(&mut sim, 0)
            .iter()
            .any(|p| matches!(p[0], b'M' | b'V'))
    );
    assert!(second.pump(&mut sim, 1).is_empty());
    let current = first.frame(&sim).minimap.unwrap();
    assert_ne!(original.cells, current.cells);

    // Built for slot one alone, against slot one's older base rather
    // than slot zero's.
    send_minimaps(&mut sim, &[1]);
    second.pump(&mut sim, 1);
    assert_eq!(second.frame(&sim).minimap.unwrap().cells, current.cells);

    send_minimaps(&mut sim, &[1]);
    assert!(second.pump(&mut sim, 1).contains(&vec![b'V']));

    // A new side cannot use an old-size delta; a dead slot before this
    // recipient must not prevent its full replacement snapshot.
    sim.backend.world.retire(0).unwrap();
    sim.backend.config.minimap_side = Some(80);
    send_minimaps(&mut sim, &[0, 1]);
    assert!(first.pump(&mut sim, 0).is_empty());
    assert!(
        second
            .pump(&mut sim, 1)
            .iter()
            .any(|p| p.starts_with(&[b'M', 0, 80]))
    );
    assert_eq!(second.frame(&sim).minimap.unwrap().side, 80);
}

#[test]
fn retained_food_keeps_updating_after_leaving_the_entry_region() {
    let mut config = raw_config(1);
    config.world.radius = 5000.;
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    let mut sim = RawSimulator::new(7, config, 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "Food");
    sim.backend.world.retire(0).unwrap();
    sim.backend
        .world
        .respawn(0, Point::default(), 0., 2)
        .unwrap();
    sim.backend.slots[0].control = Action {
        heading: 0.,
        boost: false,
    };
    run(&mut sim, &mut [(0, &mut remote)], 7);
    let food = crate::Food {
        x: 750.,
        y: 0.,
        value: 1.,
        size: Some(2.),
        origin: None,
    };
    let key = encode::food_key(food.point()).unwrap();
    sim.backend.world.insert_food(food).unwrap();
    sim.backend.broadcast().unwrap();
    remote.pump(&mut sim, 0);
    assert!(
        sim.backend.slots[0].connection.food[&[key[0], key[1]]].contains_key(&[key[2], key[3]])
    );

    // Synthetic pose change isolates retention from navigation and pickup.
    sim.backend.world.retire(0).unwrap();
    sim.backend
        .world
        .respawn(0, Point { x: 3000., y: 0. }, 0., 2)
        .unwrap();
    sim.advance_ticks(1).unwrap();
    {
        let mut food = sim.backend.world.food().to_vec();
        food[0].size = Some(3.);
        sim.backend.world.replace_food(food).unwrap();
    }
    // Physics reports births and pickups; an in-place edit is not one.
    sim.backend.food_index.stale = true;
    sim.backend.broadcast().unwrap();
    let changed = remote.pump(&mut sim, 0);
    assert!(
        changed
            .iter()
            .any(|p| matches!(p[0], b'f' | b'b') && p[1..5] == key)
    );
    assert!(!changed.contains(&encode::sector_clear([key[0], key[1]])));
    sim.advance_ticks(450).unwrap();
    assert!(sim.backend.world.snakes()[0].alive);
    assert!(
        remote
            .pump(&mut sim, 0)
            .contains(&encode::sector_clear([key[0], key[1]]))
    );
}

#[test]
fn unchanged_prey_positions_still_correct_client_prediction() {
    let mut config = raw_config(1);
    config.world.radius = 5000.;
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    let mut sim = RawSimulator::new(7, config, 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "Prey correction");
    sim.backend.world.retire(0).unwrap();
    sim.backend
        .world
        .respawn(0, Point::default(), PI, 2)
        .unwrap();
    sim.backend.slots[0].control.heading = PI;
    run(&mut sim, &mut [(0, &mut remote)], 7);
    let position = Point { x: 700., y: 700. };
    sim.backend
        .world
        .insert_prey(crate::ReplayPrey {
            prey: crate::prey::Prey {
                id: 123,
                position,
                heading: PI / 4.,
                target_heading: PI / 4.,
                speed: 6.060 / physics::SPEED_UNIT_SECONDS,
                size: 6.,
            },
            mode: crate::PreyMode::Roaming,
        })
        .unwrap();
    sim.backend
        .world
        .set_prey_mode(123, crate::prey::PreyMode::Outward)
        .unwrap();
    sim.backend.broadcast().unwrap();
    remote.pump(&mut sim, 0);
    remote.packets.clear();
    let mut report_times = Vec::new();
    let mut last_report = sim.time();
    for _ in 0..150 {
        sim.advance_ticks(1).unwrap();
        for packet in remote.pump(&mut sim, 0) {
            if packet[0] == b'j' {
                report_times.push(sim.time());
                last_report = sim.time();
            }
        }
        let age = sim.time() - last_report;
        assert!(age < 1. / 3. + 6. * sim.dt());
        let received = remote.frame(&sim).observation.unwrap();
        assert!(received.prey[0].position.distance(position) <= 190. * age + 2.);
    }
    // Both positive displacements are below one unit per 7 ms tick and
    // floor to zero, despite the nonzero speed advertised to the client.
    assert_eq!(sim.backend.world.prey()[0].position, position);
    let reports: Vec<_> = remote.packets.iter().filter(|p| p[0] == b'j').collect();
    assert_eq!(reports.len(), 3);
    assert!(reports.iter().all(|p| p.len() == 7));
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/client-reporting-cadence-near-server.json"
    ))
    .unwrap();
    // A nominal model interval inside the clean observed range, not a universal
    // deadline: scheduler-near-server-witnesses.json retains a 3044 ms gap.
    let measured = &evidence["preyUpdateGapsMs"];
    for pair in report_times.windows(2) {
        let gap_ms = (pair[1] - pair[0]) * 1000.;
        assert!(
            (measured["min"].as_f64().unwrap()..=measured["max"].as_f64().unwrap())
                .contains(&gap_ms)
        );
        assert!((gap_ms - measured["median"].as_f64().unwrap()).abs() <= sim.dt() * 1000.);
    }
    let received = remote.frame(&sim).observation.unwrap();
    assert!(received.prey[0].position.distance(position) < 12.);
}

#[test]
fn nominal_hud_periods_match_clean_server_medians() {
    let evidence: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/client-reporting-cadence-near-server.json"
    ))
    .unwrap();
    let config = SimulationConfig::default();
    // The model uses a shared fixed clock. Observed ranges in the fixture and
    // scheduler witnesses remain counterexamples to exact universal cadences.
    assert_eq!(
        config.minimap_interval_seconds * 1000.,
        evidence["minimapGapsMs"]["median"]
    );
    assert_eq!(
        config.leaderboard_interval_seconds * 1000.,
        evidence["leaderboardGapsMs"]["median"]
    );
}

#[test]
fn prey_motion_changes_are_bounded_and_visibility_reentry_resets_the_clock() {
    let mut sim = RawSimulator::new(7, raw_config(1), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "Prey timing");
    run(&mut sim, &mut [(0, &mut remote)], 7);
    let head = sim.backend.world.snakes()[0].head;
    sim.backend
        .world
        .insert_prey(crate::ReplayPrey {
            prey: crate::prey::Prey {
                id: 123,
                position: Point {
                    x: head.x + 700.,
                    y: head.y + 700.,
                },
                heading: PI / 4.,
                target_heading: PI / 4.,
                speed: 6.060 / physics::SPEED_UNIT_SECONDS,
                size: 6.,
            },
            mode: crate::PreyMode::Roaming,
        })
        .unwrap();
    sim.backend
        .world
        .set_prey_mode(123, crate::prey::PreyMode::Outward)
        .unwrap();
    sim.backend.broadcast().unwrap();
    remote.pump(&mut sim, 0);
    let mut reports = Vec::new();
    for tick in 1..=160 {
        sim.advance_ticks(1).unwrap();
        // Continuously changed synthetic motion must not defeat the
        // reporting lower bound or update the last-sent cache too early.
        {
            let mut prey = sim.backend.world.prey()[0].clone();
            prey.heading += 0.2;
            crate::client::fixtures::restore_prey(&mut sim.backend.world, 0, prey);
        }
        // Exact-11 pulse transitions are the retained shorter-gap stratum.
        {
            let mut prey = sim.backend.world.prey()[0].clone();
            prey.speed = 11. / physics::SPEED_UNIT_SECONDS;
            crate::client::fixtures::restore_prey(&mut sim.backend.world, 0, prey);
        }
        sim.backend.broadcast().unwrap();
        for packet in remote.pump(&mut sim, 0) {
            if packet[0] == b'j' {
                assert_eq!(packet.len(), 16);
                reports.push(sim.time());
            }
        }
        if tick == 1 {
            assert!(reports.is_empty());
        }
    }
    assert!(reports.len() >= 4);
    for pair in reports.windows(2) {
        assert!((0.210 - 1e-9..=0.217 + 1e-9).contains(&(pair[1] - pair[0])));
    }
    let id = sim.backend.world.prey().last().unwrap().id;
    let prey = sim.backend.world.remove_prey(id).unwrap();
    sim.backend.broadcast().unwrap();
    assert!(
        remote
            .pump(&mut sim, 0)
            .contains(&encode::prey_removal(123, None))
    );
    sim.backend
        .world
        .insert_prey(crate::ReplayPrey {
            prey,
            mode: crate::PreyMode::Roaming,
        })
        .unwrap();
    sim.backend.broadcast().unwrap();
    let packets = remote.pump(&mut sim, 0);
    assert!(packets.iter().any(|p| p[0] == b'y' && p.len() == 20));
    assert!(packets.iter().all(|p| p[0] != b'j'));
}

#[test]
fn prey_pickup_reports_the_actual_eater_without_inventing_one_for_visibility_exit() {
    let mut sim = RawSimulator::new(7, raw_config(1), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "Prey");
    run(&mut sim, &mut [(0, &mut remote)], 7);
    let snake = &sim.backend.world.snakes()[0];
    let (position, _) =
        physics::pickup_circle(snake.head, snake.heading, snake.speed, snake.segments);
    let prey = crate::prey::Prey {
        id: 123,
        position,
        heading: 0.,
        target_heading: 0.,
        speed: 118.75,
        size: 6.,
    };
    sim.backend
        .world
        .insert_prey(crate::ReplayPrey {
            prey: prey.clone(),
            mode: crate::PreyMode::Roaming,
        })
        .unwrap();
    sim.backend.broadcast().unwrap();
    remote.pump(&mut sim, 0);
    run(&mut sim, &mut [(0, &mut remote)], 6);
    assert!(remote.packets.contains(&encode::prey_removal(123, Some(0))));

    sim.backend
        .world
        .insert_prey(crate::ReplayPrey {
            prey,
            mode: crate::PreyMode::Roaming,
        })
        .unwrap();
    sim.backend.broadcast().unwrap();
    remote.pump(&mut sim, 0);
    sim.backend.world.replace_prey(Vec::new()).unwrap();
    sim.backend.broadcast().unwrap();
    assert!(
        remote
            .pump(&mut sim, 0)
            .contains(&encode::prey_removal(123, None))
    );
}
#[test]
fn prey_ids_that_share_their_low_16_bits_get_distinct_wire_ids() {
    let mut sim = RawSimulator::new(7, raw_config(1), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "Prey ids");
    run(&mut sim, &mut [(0, &mut remote)], 7);
    let head = sim.backend.world.snakes()[0].head;
    let prey = |id: u64, dx: f64| crate::prey::Prey {
        id,
        position: Point {
            x: head.x + dx,
            y: head.y + 600.,
        },
        heading: 0.,
        target_heading: 0.,
        speed: 0.,
        size: 6.,
    };
    // A surviving early prey and a late-life prey whose truncated IDs collide.
    for prey in [prey(0, -100.), prey(65_536, 100.)] {
        sim.backend
            .world
            .insert_prey(crate::ReplayPrey {
                prey,
                mode: crate::PreyMode::Roaming,
            })
            .unwrap();
    }
    sim.backend.broadcast().unwrap();
    let appearances: BTreeSet<u16> = remote
        .pump(&mut sim, 0)
        .iter()
        .filter(|p| p[0] == b'y' && p.len() == 20)
        .map(|p| u16::from_be_bytes([p[1], p[2]]))
        .collect();
    assert_eq!(appearances, BTreeSet::from([0, 1]));
    assert_eq!(remote.frame(&sim).observation.unwrap().prey.len(), 2);

    // A removed prey's ID is reused in the same broadcast, after its removal.
    sim.backend.world.remove_prey(0).unwrap();
    sim.backend
        .world
        .insert_prey(crate::ReplayPrey {
            prey: prey(131_072, 0.),
            mode: crate::PreyMode::Roaming,
        })
        .unwrap();
    sim.backend.broadcast().unwrap();
    let packets = remote.pump(&mut sim, 0);
    let removal = packets
        .iter()
        .position(|p| *p == encode::prey_removal(0, None));
    let added: Vec<(usize, u16)> = packets
        .iter()
        .enumerate()
        .filter(|(_, p)| p[0] == b'y' && p.len() == 20)
        .map(|(index, p)| (index, u16::from_be_bytes([p[1], p[2]])))
        .collect();
    assert_eq!(added.len(), 1);
    assert_eq!(added[0].1, 0);
    assert!(removal.unwrap() < added[0].0);
    assert_eq!(remote.frame(&sim).observation.unwrap().prey.len(), 2);
}

#[test]
fn every_prey_id_can_be_live_when_a_shown_prey_is_replaced() {
    let mut sim = RawSimulator::new(7, raw_config(1), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "Prey ids");
    run(&mut sim, &mut [(0, &mut remote)], 7);
    let head = sim.backend.world.snakes()[0].head;
    // One shown prey; the rest live beyond vision.
    let prey = |id: u64, near: bool| crate::prey::Prey {
        id,
        position: Point {
            x: head.x + if near { 0. } else { 1e6 },
            y: head.y + 600.,
        },
        heading: 0.,
        target_heading: 0.,
        speed: 0.,
        size: 6.,
    };
    sim.backend
        .world
        .replace_prey(
            ((0..65_536).map(|id| prey(id, id == 0)).collect::<Vec<_>>())
                .into_iter()
                .map(|prey| crate::ReplayPrey {
                    prey,
                    mode: crate::PreyMode::Roaming,
                })
                .collect(),
        )
        .unwrap();
    sim.backend.broadcast().unwrap();
    remote.pump(&mut sim, 0);
    // A catch and an immediate respawn: the shown ID retires as it is reused.
    sim.backend.world.remove_prey(0).unwrap();
    sim.backend
        .world
        .insert_prey(crate::ReplayPrey {
            prey: prey(65_536, true),
            mode: crate::PreyMode::Roaming,
        })
        .unwrap();
    sim.backend.broadcast().unwrap();
    let packets = remote.pump(&mut sim, 0);
    let removal = packets
        .iter()
        .position(|p| *p == encode::prey_removal(0, None));
    let added = packets
        .iter()
        .position(|p| p[0] == b'y' && p.len() == 20 && p[1..3] == [0, 0]);
    assert!(removal.unwrap() < added.unwrap());
    assert_eq!(remote.frame(&sim).observation.unwrap().prey.len(), 1);
}

#[test]
fn three_new_anchors_encode_incrementally_and_four_resend() {
    let world = Simulator::new(crate::WorldOptions {
        seed: 1,
        config: Config::sandbox(),
        population: crate::InitialPopulation::Placed {
            ids: vec!["0".into()],
            placement: crate::Placement::TestRing,
        },
    })
    .unwrap();
    let mut snake = world.snakes()[0].clone();
    snake.heading = 0.;
    snake.body = (0..20)
        .map(|i| Point {
            x: -42. * f64::from(i),
            y: 3. * f64::from(i % 2),
        })
        .collect();
    snake.head = snake.body[0];
    snake.segments = 20;
    for insertions in 1..=FRESH_ANCHORS {
        let mut moved = snake.clone();
        let mut known = KnownSnake::new(0, &moved, 0.).unwrap();
        for k in 1..=insertions {
            let anchor = Point {
                x: 42. * k as f64,
                y: 0.,
            };
            physics::append_anchor(&mut moved.body, anchor, false);
            moved.head = anchor;
        }
        let mut packets = Vec::new();
        changes(
            &mut packets,
            false,
            1,
            &mut known,
            &moved,
            0.,
            &NATIVE_PROFILE,
            &SnakeReports::default(),
        )
        .unwrap();
        let anchors = packets
            .iter()
            .filter(|p| matches!(p[0], b'g' | b'='))
            .count();
        if insertions < FRESH_ANCHORS {
            // The old front is still unchanged at index `insertions`.
            assert_eq!(moved.body[insertions], snake.body[0]);
            assert_eq!(anchors, insertions);
            assert!(!packets.iter().any(|p| is_full(p)), "{insertions}");
        } else {
            assert_ne!(moved.body[insertions], snake.body[0]);
            assert!(packets.iter().any(|p| is_full(p)), "{insertions}");
        }
        assert_eq!(known.front, moved.body[0]);
    }
}

#[test]
fn boosting_within_the_window_never_resends_the_body() {
    let mut config = raw_config(2);
    config.world.radius = 6000.;
    config.world.food_count = 0;
    config.world.ambient_replenish_per_second = 0.;
    assert!(config.warnings().is_empty());
    let mut sim = RawSimulator::new(7, config, 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "Observer");
    let mut b = Remote::connect(&mut sim, 1, "Booster");
    for (slot, y, segments) in [(0, 1000., 2), (1, 0., 60)] {
        sim.backend.world.retire(slot).unwrap();
        sim.backend
            .world
            .respawn(slot, Point { x: -1500., y }, 0., segments)
            .unwrap();
        sim.backend.slots[slot].control = Action {
            heading: 0.,
            boost: slot == 1,
        };
    }
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 200);
    a.packets.clear();
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 400);
    assert!(sim.backend.world.is_boosting(1));
    assert!(a.packets.iter().any(|p| p[0] == b'g' && p[1..3] == [0, 1]));
    assert!(!a.packets.iter().any(|p| is_full(p) && p[1..3] == [0, 1]));
}

fn profile(nickname: &str) -> PlayerAppearance {
    PlayerAppearance {
        nickname: nickname.as_bytes().to_vec(),
        skin: 7,
        custom_skin: vec![1, 2, 3],
    }
}
fn angle_close(a: f64, b: f64, tolerance: f64) -> bool {
    ((a - b + PI).rem_euclid(TAU) - PI).abs() < tolerance
}

/// A remote client: the shared decoder fed only with drained payloads.
struct Remote {
    state: ClientState,
    packets: Vec<Vec<u8>>,
}
impl Remote {
    fn connect(sim: &mut RawSimulator, slot: usize, nickname: &str) -> Self {
        sim.connect(slot, profile(nickname)).unwrap();
        let mut state = ClientState::new();
        state.new_connection(sim.time()).unwrap();
        Self {
            state,
            packets: Vec::new(),
        }
    }
    fn pump(&mut self, sim: &mut RawSimulator, slot: usize) -> Vec<Vec<u8>> {
        let now = sim.time();
        let mut packets = Vec::new();
        for frame in sim.drain(slot).unwrap() {
            self.state.receive_frame(&frame, now).unwrap();
            for packet in protocol::split_frame(&frame).unwrap() {
                packets.push(packet.to_vec());
            }
        }
        self.packets.extend(packets.iter().cloned());
        packets
    }
    fn frame(&self, sim: &RawSimulator) -> ClientFrame {
        self.state.frame(sim.time()).unwrap()
    }
}
fn run(sim: &mut RawSimulator, remotes: &mut [(usize, &mut Remote)], ticks: usize) {
    for _ in 0..ticks {
        sim.advance_ticks(1).unwrap();
        for (slot, remote) in remotes.iter_mut() {
            remote.pump(sim, *slot);
        }
    }
}
fn is_full(packet: &[u8]) -> bool {
    packet[0] == b's' && packet.len() > 6
}

#[test]
fn raw_join_sends_setup_then_own_appearance_and_shares_the_decoder() {
    let mut sim = RawSimulator::new(3, raw_config(3), 2).unwrap();
    sim.advance_ticks(60).unwrap();
    // Unconnected remote slots stay absent; the background slot plays.
    let world = &sim.backend.world;
    assert!(!world.snakes()[0].alive && !world.snakes()[1].alive && world.snakes()[2].alive);
    assert!(sim.drain(0).is_err() && sim.receive(0, &[1]).is_err());
    assert!(sim.drain(2).is_err() && sim.connect(2, profile("")).is_err());
    let long = PlayerAppearance {
        nickname: vec![b'x'; 25],
        ..profile("")
    };
    assert!(sim.connect(0, long).is_err());
    assert!(!sim.backend.world.snakes()[0].alive);

    // Slot 0's id differs from the setup server number; slot 1's equals it.
    assert_eq!(encode::SERVER_NUMBER, 1);
    let mut a = Remote::connect(&mut sim, 0, "Zero");
    assert!(a.pump(&mut sim, 0).is_empty());
    sim.advance_ticks(1).unwrap();
    // Setup leads the tick's frame; that tick's updates follow it.
    let setup = a.pump(&mut sim, 0);
    assert_eq!(
        setup[0],
        encode::setup(
            sim.backend.world.radius(),
            sim.backend.world.config().prey.turn_rate
        )
        .unwrap()
    );
    assert!(sim.drain(0).unwrap().is_empty());
    let mut b = Remote::connect(&mut sim, 1, "One");
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 12);

    for (slot, remote, nickname) in [(0, &a, "Zero"), (1, &b, "One")] {
        assert_eq!(remote.packets[0][0], b'a');
        assert_eq!(remote.packets.iter().filter(|p| p[0] == b'a').count(), 1);
        // The first full appearance is the client's own, with its profile.
        let own = &remote.packets[1];
        assert!(is_full(own));
        assert_eq!(u16::from_be_bytes([own[1], own[2]]), slot as u16);
        assert_eq!(own[15], 7);
        let n = nickname.len();
        assert_eq!(own[22], n as u8);
        assert_eq!(&own[23..23 + n], nickname.as_bytes());
        assert_eq!(own[23 + n..27 + n], [3, 1, 2, 3]);
        let frame = remote.frame(&sim);
        assert_eq!(frame.status, ClientStatus::Playing);
        let observation = frame.observation.unwrap();
        assert_eq!(observation.me.id, slot.to_string());
        // Within the movement since the last broadcast, six ticks ago.
        let actual = &sim.backend.world.snakes()[slot];
        assert!(observation.me.head.distance(actual.head) < 6. * sim.dt() * actual.speed + 1.);
        // One shared world: each client sees the other remote and the bot.
        let mut seen: Vec<_> = observation.opponents.iter().map(|o| o.id.clone()).collect();
        seen.sort();
        let others: Vec<_> = (0..3)
            .filter(|&i| i != slot)
            .map(|i| i.to_string())
            .collect();
        assert_eq!(seen, others);
    }
}

#[test]
fn raw_slots_never_ping_decode_or_record_on_their_own() {
    let config = SimulationConfig {
        record_wire: true,
        ..raw_config(2)
    };
    assert!(RawSimulator::new(1, config, 1).is_err());
    assert!(RawSimulator::new(1, raw_config(2), 0).is_err());
    assert!(RawSimulator::new(1, raw_config(2), 3).is_err());
    let mut sim = RawSimulator::new(1, raw_config(2), 2).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "");
    // Longer than the client's ping interval.
    for _ in 0..1500 {
        sim.advance_ticks(1).unwrap();
        remote.pump(&mut sim, 0);
        assert!(sim.backend.slots[0].connection.uplink.is_empty());
    }
    assert!(!remote.packets.contains(&encode::pong()));
    let s = &sim.backend.slots[0];
    assert!(s.records.is_none() && s.outbox.is_none());
    let internal = s.state.frame(sim.time()).unwrap();
    assert_eq!(internal.status, ClientStatus::Disconnected);
    assert!(remote.frame(&sim).status != ClientStatus::Waiting);
}

#[test]
fn raw_controls_travel_the_uplink_and_reject_without_mutation() {
    let mut sim = RawSimulator::new(5, raw_config(2), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "");
    run(&mut sim, &mut [(0, &mut remote)], 7);
    for (message, heading, boost) in [
        (10, 10.5, false),
        (253, 10.5, true),
        (250, 250.5, true),
        (254, 250.5, false),
    ] {
        sim.receive(0, &[message]).unwrap();
        sim.advance_ticks(1).unwrap();
        let control = sim.backend.slots[0].control;
        assert_eq!(control.heading, heading * TAU / 251.);
        assert_eq!(control.boost, boost);
    }
    sim.receive(0, &[251]).unwrap();
    sim.advance_ticks(1).unwrap();
    assert!(remote.pump(&mut sim, 0).contains(&encode::pong()));
    let control = sim.backend.slots[0].control;
    for message in [&[252][..], &[255], &[], &[1, 2], &[252, 1, 2]] {
        let error = sim.receive(0, message).unwrap_err();
        assert_eq!(error, "Unsupported client message");
        assert!(sim.backend.slots[0].connection.uplink.is_empty());
    }
    assert!(sim.receive(1, &[1]).is_err());
    sim.advance_ticks(1).unwrap();
    assert_eq!(sim.backend.slots[0].control, control);

    // The steered snake reports the requested heading to its own decoder.
    sim.receive(0, &[100]).unwrap();
    run(&mut sim, &mut [(0, &mut remote)], 120);
    let me = remote.frame(&sim).observation.unwrap().me;
    let world = sim.backend.world.snakes()[0].heading;
    assert!(angle_close(me.heading, world, 0.03));
    assert!(angle_close(world, 100.5 * TAU / 251., 0.03));
}

#[test]
fn keyboard_turns_move_the_wanted_heading_both_ways() {
    let mut sim = RawSimulator::new(5, raw_config(1), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "");
    run(&mut sim, &mut [(0, &mut remote)], 7);
    sim.receive(0, &[0]).unwrap();
    sim.advance_ticks(1).unwrap();
    let start = sim.backend.slots[0].control.heading;
    let step = 0.033 * physics::scang(sim.backend.world.snakes()[0].segments);
    for (amount, turns) in [(0, 0.), (128, 0.), (10, -10.), (138, 10.), (255, 127.)] {
        let before = sim.backend.slots[0].control;
        sim.receive(0, &[252, amount]).unwrap();
        sim.advance_ticks(1).unwrap();
        let after = sim.backend.slots[0].control;
        assert!(angle_close(
            after.heading,
            before.heading + turns * step,
            1e-12
        ));
        assert!((0. ..TAU).contains(&after.heading) && !after.boost);
    }
    assert!(angle_close(
        sim.backend.slots[0].control.heading,
        start + 127. * step,
        1e-12
    ));
    // Turning down past zero wraps into [0, TAU).
    sim.receive(0, &[252, 127]).unwrap();
    sim.receive(0, &[252, 127]).unwrap();
    sim.advance_ticks(1).unwrap();
    let heading = sim.backend.slots[0].control.heading;
    assert!(angle_close(heading, start - 127. * step, 1e-12));
    assert!((0. ..TAU).contains(&heading));
}

#[test]
fn a_listing_shows_its_own_pellets_color_beside_a_hidden_older_one() {
    let ambient = Food {
        x: 1000.,
        y: 1000.,
        value: 1.,
        size: Some(2.),
        origin: None,
    };
    let death = Food {
        origin: Some(Origin::Death),
        ..ambient
    };
    // Find a seed whose ambient color differs from the death palette's.
    let (presentation, ambient_color, death_color) = (1..100)
        .find_map(|seed| {
            let mut presentation = FoodPresentation::new(seed, 1);
            presentation.register(0, 3, &[]);
            let births = [(&ambient, None), (&death, Some(0))].map(|(food, source)| {
                presentation
                    .births(&[crate::FoodSpawnEvent {
                        source,
                        food: *food,
                    }])
                    .unwrap();
                presentation.newest(food)
            });
            (births[0] != births[1]).then_some((presentation, births[0], births[1]))
        })
        .unwrap();
    // The ambient pellet waits for maintenance; the death pellet is prompt.
    let mut index = FoodIndex::default();
    index.add(&ambient, false, ambient_color);
    index.add(&death, true, death_color);
    let (sector, _) = FoodIndex::sector(&death).unwrap();
    let run = index.listing(sector);
    assert_eq!(run.len(), 1);
    assert_eq!(run[0].1.style.color, death_color);
    assert!(run[0].1.style.snake_food);
    // A rebuild assigns each lifetime its own color too.
    index.rebuild(&[ambient, death], &presentation);
    assert_eq!(index.listing(sector)[0].1.style.color, ambient_color);
}

#[test]
fn a_viewer_holding_a_newer_value_is_corrected_when_the_snake_returns() {
    let known = |motion: [u8; 3], fam| KnownSnake {
        life: 1,
        front: Point::default(),
        len: 2,
        heading: motion[0],
        wanted: motion[1],
        speed: motion[2],
        fam,
        kills: 0,
    };
    let (a, b) = ([10, 20, 30], [10, 21, 30]);
    let mut reports = SnakeReports::default();
    reports.update(1, [0; 3], 100, 0, DeliveryProfile::Official);
    reports.update(1, a, 150, 10, DeliveryProfile::Official);
    assert!(reports.motion_due && reports.fam_due);
    // A viewer appearing at tick 11 holds B; a growth anchor told every
    // viewer mass 200 while the mass gate was closed.
    reports.update(1, b, 200, 11, DeliveryProfile::Official);
    reports.observe(&known(b, 200));
    // The snake returns to the reported A and mass 150 before the gates
    // open: six ticks for motion, 15 for mass.
    for tick in 12..25 {
        reports.update(1, a, 150, tick, DeliveryProfile::Official);
        assert_eq!(reports.motion_due, tick == 16, "{tick}");
        assert!(!reports.fam_due, "{tick}");
    }
    assert_eq!(reports.motion, a);
    reports.update(1, a, 150, 25, DeliveryProfile::Official);
    assert!(reports.fam_due && reports.fam == 150);
    reports.update(1, a, 150, 40, DeliveryProfile::Official);
    assert!(!reports.motion_due && !reports.fam_due);
}

#[test]
fn input_arriving_during_a_tick_shapes_that_ticks_frame() {
    let mut sim = RawSimulator::new(2, raw_config(1), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "");
    run(&mut sim, &mut [(0, &mut remote)], 12);
    let sent = sim.time() + sim.dt() / 2.;
    sim.receive_at(0, &[251], sent).unwrap();
    sim.receive_at(0, &[253], sent).unwrap();
    sim.advance_ticks(1).unwrap();
    assert!(sim.backend.slots[0].control.boost);
    let packets = remote.pump(&mut sim, 0);
    assert_eq!(packets[0], encode::pong());
    assert_eq!(packets.iter().filter(|p| **p == encode::pong()).count(), 1);
}

#[test]
fn raw_queues_are_bounded_and_rejections_are_atomic() {
    let mut sim = RawSimulator::new(2, raw_config(1), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "");
    let mut accepted = 0;
    while sim.receive(0, &[251]).is_ok() {
        accepted += 1;
    }
    // The connect itself occupies one place.
    assert_eq!(accepted, REMOTE_IN_FLIGHT - 1);
    let uplink = sim.backend.slots[0].connection.uplink.len();
    assert!(sim.receive(0, &[3]).is_err());
    assert_eq!(sim.backend.slots[0].connection.uplink.len(), uplink);
    // Pongs lead the tick's frame after setup rather than taking one
    // frame each.
    sim.advance_ticks(1).unwrap();
    assert!(sim.backend.slots[0].connection.downlink.len() < 4);
    let packets = remote.pump(&mut sim, 0);
    assert_eq!(packets[0][0], b'a');
    assert!(packets[1..=accepted].iter().all(|p| *p == encode::pong()));
    assert!(packets[accepted + 1..].iter().all(|p| *p != encode::pong()));
    sim.receive(0, &[251]).unwrap();
}

#[test]
fn raw_latency_keeps_order_without_processing_delay() {
    let mut config = raw_config(1);
    config.latency.round_trip_seconds = vec![0.1];
    config.latency.processing_seconds = [0.5, 0.5];
    let mut sim = RawSimulator::new(4, config, 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "");
    let start = sim.time();
    let (at, setup) = loop {
        sim.advance_ticks(1).unwrap();
        if let Some(packet) = remote.pump(&mut sim, 0).first() {
            break (sim.time(), packet.clone());
        }
    };
    assert_eq!(setup[0], b'a');
    // One round trip, rounded up to the ticks of uplink and downlink.
    assert!(at >= start + 0.1 - 1e-9 && at < start + 0.1 + 2. * sim.dt());
    run(&mut sim, &mut [(0, &mut remote)], 30);
    sim.receive(0, &[251]).unwrap();
    let sent = sim.time();
    let pong = loop {
        sim.advance_ticks(1).unwrap();
        if remote.pump(&mut sim, 0).contains(&encode::pong()) {
            break sim.time();
        }
    };
    assert!(pong >= sent + 0.1 - 1e-9 && pong < sent + 0.1 + 2. * sim.dt());
}

#[test]
fn equal_player_and_bot_models_preserve_raw_frames_and_rng() {
    for latency in [LatencyConfig::default(), LatencyConfig::measured_route()] {
        let mut config = raw_config(2);
        config.latency = latency.clone();
        let mut baseline = RawSimulator::new(7, config.clone(), 2).unwrap();
        let mut grouped = RawSimulator::new(7, config, 2).unwrap();
        grouped.set_player_latency(1, latency);
        for sim in [&mut baseline, &mut grouped] {
            for slot in 0..2 {
                assert!(sim.connect(slot, PlayerAppearance::default()).unwrap());
            }
        }
        for tick in 0..80 {
            for sim in [&mut baseline, &mut grouped] {
                if tick == 40 {
                    sim.disconnect(0).unwrap();
                    assert!(sim.connect(0, PlayerAppearance::default()).unwrap());
                }
                if tick % 6 == 0 {
                    for slot in 0..2 {
                        sim.receive(slot, &[251]).unwrap();
                        sim.receive(slot, &[tick as u8]).unwrap();
                    }
                }
                sim.advance_ticks(1).unwrap();
            }
            for slot in 0..2 {
                assert_eq!(grouped.take(slot).unwrap(), baseline.take(slot).unwrap());
                assert_eq!(
                    format!("{:?}", grouped.backend.slots[slot].connection.link),
                    format!("{:?}", baseline.backend.slots[slot].connection.link),
                );
            }
            assert_eq!(
                format!("{:?}", grouped.backend.random),
                format!("{:?}", baseline.backend.random)
            );
        }
    }
}

#[test]
fn every_full_appearance_is_followed_by_its_directed_motion() {
    let mut sim = RawSimulator::new(9, raw_config(4), 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "");
    let mut b = Remote::connect(&mut sim, 1, "");
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 13);
    let mut logs = Vec::new();
    // Slot 1 rejoins while slot 0 turns up, then down: its new session
    // first sees slot 0 in the middle of each turn.
    for offset in [60, 251 - 60] {
        let heading = sim.backend.world.snakes()[0].heading;
        let byte = (heading.rem_euclid(TAU) * 251. / TAU).floor() as usize;
        sim.receive(0, &[((byte + offset) % 251) as u8]).unwrap();
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 2);
        logs.push((1, std::mem::take(&mut b.packets)));
        b = Remote::connect(&mut sim, 1, "");
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 8);
    }
    // Break continuity so the fallback path resends a known snake.
    let resent = a.packets.len();
    {
        let snake = &sim.backend.world.snakes()[2];
        let mut body = snake.body.clone();
        for point in &mut body {
            point.x += 5.;
        }
        sim.backend
            .world
            .correct_pose(2, snake.head, body, snake.heading)
            .unwrap();
    }
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 6);
    assert!(
        a.packets[resent..]
            .iter()
            .any(|p| is_full(p) && p[1..3] == [0, 2])
    );
    logs.push((0, a.packets));
    logs.push((1, b.packets));
    let mut seen = [0; 3];
    for (slot, packets) in &logs {
        for k in (0..packets.len()).filter(|&k| is_full(&packets[k])) {
            let (s, motion) = (&packets[k], &packets[k + 1]);
            let id = u16::from_be_bytes([s[1], s[2]]);
            let own = usize::from(id) == *slot;
            let direction = s[6] - b'0';
            seen[usize::from(direction)] += 1;
            let code = match (own, direction == 2) {
                (true, false) => b'd',
                (true, true) => b'7',
                (false, false) => b'e',
                (false, true) => b'E',
            };
            assert_eq!(motion[0], code);
            assert_eq!(motion.len(), if own { 4 } else { 6 });
            let at = if own { 1 } else { 3 };
            if !own {
                assert_eq!(u16::from_be_bytes([motion[1], motion[2]]), id);
            }
            let angle = |m: usize| {
                f64::from(u32::from_be_bytes([0, s[m], s[m + 1], s[m + 2]])) * TAU / 16777215.
            };
            assert_eq!(motion[at], encode::angle_byte(angle(3)));
            assert_eq!(motion[at + 1], encode::angle_byte(angle(7)));
            if direction > 0 {
                assert_eq!(encode::direction(angle(3), angle(7)), direction);
            }
        }
    }
    assert!(seen.iter().all(|&count| count > 0), "{seen:?}");
}

/// Packets about another snake `id`, grouped by the tick they arrived in.
fn about(packets: &[Vec<Vec<u8>>], id: u16, codes: &[u8]) -> Vec<(usize, Vec<u8>)> {
    packets
        .iter()
        .enumerate()
        .flat_map(|(tick, packets)| packets.iter().map(move |p| (tick, p)))
        .filter(|(_, p)| codes.contains(&p[0]) && p.len() >= 3 && p[1..3] == id.to_be_bytes())
        .map(|(tick, p)| (tick, p.clone()))
        .collect()
}
const MOTION: &[u8] = b"eE345d7";

#[test]
fn responsive_inputs_reach_self_and_opponents_in_the_next_tick() {
    for delivery in [DeliveryProfile::Official, DeliveryProfile::Responsive] {
        let mut config = raw_config(2);
        config.delivery = delivery;
        config.world.radius = 6000.;
        config.world.food_count = 0;
        config.world.ambient_replenish_per_second = 0.;
        config.world.prey.count = 0;
        let mut sim = RawSimulator::new(5, config, 2).unwrap();
        let mut viewer = Remote::connect(&mut sim, 0, "Viewer");
        let mut mover = Remote::connect(&mut sim, 1, "Mover");
        for (slot, x) in [(0, -500.), (1, 500.)] {
            sim.backend.world.retire(slot).unwrap();
            sim.backend
                .world
                .respawn(slot, Point { x: 0., y: x }, 0., 30)
                .unwrap();
            sim.backend.slots[slot].control.heading = 0.;
        }
        run(&mut sim, &mut [(0, &mut viewer), (1, &mut mover)], 60);
        assert!(sim.backend.world.snakes().iter().all(|snake| snake.alive));
        // Two fractional gains inside the standalone mass gate; no motion
        // changes or segment growth are needed for the second responsive h.
        for (first, fam) in [(true, 0.1), (false, 0.2)] {
            {
                let mut snake = sim.backend.world.snakes()[1].clone();
                snake.fam = fam;
                sim.backend
                    .world
                    .correct_mass(1, snake.body.clone(), snake.fam)
                    .unwrap();
            }
            sim.advance_ticks(1).unwrap();
            for packets in [viewer.pump(&mut sim, 0), mover.pump(&mut sim, 1)] {
                assert_eq!(
                    packets.iter().any(|p| p[0] == b'h'),
                    first || delivery == DeliveryProfile::Responsive
                );
                assert!(!packets.iter().any(|p| MOTION.contains(&p[0])));
            }
        }
        // Seed a recent report, then change the target inside its six-tick gate.
        sim.receive(1, &[60]).unwrap();
        let mut reported = false;
        for _ in 0..MOTION_REPORT_TICKS {
            run(&mut sim, &mut [(0, &mut viewer), (1, &mut mover)], 1);
            if sim.backend.reports[1].motion_due {
                reported = true;
                break;
            }
        }
        assert!(reported);
        for input in [vec![61], vec![253], vec![254]] {
            sim.receive(1, &input).unwrap();
            sim.advance_ticks(1).unwrap();
            let other = viewer.pump(&mut sim, 0);
            let own = mover.pump(&mut sim, 1);
            assert!(sim.backend.world.snakes().iter().all(|snake| snake.alive));
            let prompt = delivery == DeliveryProfile::Responsive;
            assert_eq!(
                other.iter().any(|p| MOTION.contains(&p[0])),
                prompt,
                "{delivery:?} {input:?}"
            );
            assert_eq!(
                own.iter().any(|p| MOTION.contains(&p[0])),
                prompt,
                "{delivery:?} {input:?}"
            );
        }
    }
}

#[test]
fn responsive_held_target_keeps_six_tick_motion_spacing_and_prompt_mass() {
    let mut reports = SnakeReports::default();
    let delivery = DeliveryProfile::Responsive;
    reports.update(1, [0, 120, 80], 0, 0, delivery);
    let mut sent = Vec::new();
    for tick in 1..=60 {
        reports.update(1, [tick as u8, 120, 80], tick as u32, tick, delivery);
        if reports.motion_due {
            sent.push(tick);
        }
        assert!(reports.fam_due, "changed mass at {tick}");
    }
    assert_eq!(sent, (1..=60).step_by(6).collect::<Vec<_>>());
    reports.update(1, [60, 120, 80], 60, 100, delivery);
    reports.update(1, [60, 120, 80], 60, 106, delivery);
    assert!(!reports.motion_due && !reports.fam_due);
}
const ANCHORS: &[u8] = b"gn=+";

#[test]
fn motion_reports_share_one_gate_per_snake_and_lead_its_anchors() {
    let config = SimulationConfig {
        delivery: DeliveryProfile::Official,
        ..raw_config(3)
    };
    let mut sim = RawSimulator::new(5, config, 3).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "A");
    let mut b = Remote::connect(&mut sim, 1, "B");
    let mut mover = Remote::connect(&mut sim, 2, "Mover");
    run(
        &mut sim,
        &mut [(0, &mut a), (1, &mut b), (2, &mut mover)],
        60,
    );
    let mut seen = [Vec::new(), Vec::new()];
    let mut steer = |sim: &mut RawSimulator, turn: f64, ticks: usize| {
        let mut seen = [Vec::new(), Vec::new()];
        for _ in 0..ticks {
            sim.backend.slots[2].control.heading += turn;
            sim.advance_ticks(1).unwrap();
            seen[0].push(a.pump(sim, 0));
            seen[1].push(b.pump(sim, 1));
            mover.pump(sim, 2);
        }
        seen
    };
    // A settled, straight snake keeps sending anchors but no motion.
    let straight = steer(&mut sim, 0., 120);
    assert!(about(&straight[0], 2, MOTION).is_empty());
    assert!(!about(&straight[0], 2, ANCHORS).is_empty());
    // Continuous steering: reports at most every MOTION_REPORT_TICKS, in
    // the same ticks for both recipients.
    for (all, new) in seen.iter_mut().zip(steer(&mut sim, 0.03, 120)) {
        *all = new;
    }
    let ticks = |seen: &[Vec<Vec<u8>>]| -> Vec<usize> {
        about(seen, 2, MOTION)
            .iter()
            .map(|(tick, _)| *tick)
            .collect()
    };
    let (first, second) = (ticks(&seen[0]), ticks(&seen[1]));
    assert!(first.len() >= 10, "{first:?}");
    assert_eq!(first, second);
    assert!(
        first
            .windows(2)
            .all(|w| w[1] - w[0] >= MOTION_REPORT_TICKS as usize)
    );
    // Same-snake motion precedes its anchor within a tick's packets.
    for packets in &seen[0] {
        let position = |codes: &[u8]| {
            packets
                .iter()
                .position(|p| codes.contains(&p[0]) && p.len() >= 3 && p[1..3] == [0, 2])
        };
        if let (Some(motion), Some(anchor)) = (position(MOTION), position(ANCHORS)) {
            assert!(motion < anchor);
        }
    }
}

#[test]
fn standalone_mass_reports_wait_for_their_opportunity() {
    let config = SimulationConfig {
        delivery: DeliveryProfile::Official,
        ..raw_config(2)
    };
    let mut sim = RawSimulator::new(5, config, 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "A");
    let mut mover = Remote::connect(&mut sim, 1, "Mover");
    run(&mut sim, &mut [(0, &mut a), (1, &mut mover)], 60);
    let mut seen = Vec::new();
    for _ in 0..120 {
        // Mass changes every tick without new segments.
        let snake = sim.backend.world.snakes()[1].clone();
        let fam = (snake.fam + 0.003) % 1.;
        sim.backend.world.correct_mass(1, snake.body, fam).unwrap();
        sim.advance_ticks(1).unwrap();
        seen.push(a.pump(&mut sim, 0));
        mover.pump(&mut sim, 1);
    }
    let reports: Vec<usize> = about(&seen, 1, b"h")
        .iter()
        .map(|(tick, _)| *tick)
        .collect();
    assert!(reports.len() >= 6, "{reports:?}");
    assert!(
        reports
            .windows(2)
            .all(|w| w[1] - w[0] >= FAM_REPORT_TICKS as usize)
    );
}

#[test]
fn ordinary_food_waits_for_maintenance_while_snake_food_is_prompt() {
    let mut config = raw_config(2);
    config.world.food_count = 2000;
    config.world.ambient_replenish_per_second = 2000.;
    config.world.radius = 6000.;
    let mut sim = RawSimulator::new(9, config, 2).unwrap();
    // Start empty so replenishment keeps adding ordinary food.
    sim.backend.world.replace_food(Vec::new()).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "A");
    let mut booster = Remote::connect(&mut sim, 1, "Booster");
    run(&mut sim, &mut [(0, &mut a), (1, &mut booster)], 20);
    // The viewer drives beside the booster's tail, inside its food interest.
    for (slot, head, segments) in [
        (0, Point { x: -500., y: 300. }, 10),
        (1, Point { x: 1500., y: 0. }, 60),
    ] {
        sim.backend.world.retire(slot).unwrap();
        sim.backend.world.respawn(slot, head, 0., segments).unwrap();
        sim.backend.slots[slot].control.heading = 0.;
    }
    sim.backend.slots[1].control = Action {
        heading: 0.,
        boost: true,
    };
    let (mut ordinary, mut snake_food) = (BTreeSet::new(), BTreeSet::new());
    for _ in 0..300 {
        sim.advance_ticks(1).unwrap();
        let now = sim.time();
        let epoch = |t: f64| (t / super::super::visibility::CLEANUP_SECONDS).floor();
        let maintenance = epoch(now) > epoch(now - sim.dt());
        for packet in a.pump(&mut sim, 0) {
            match packet[0] {
                b'f' => ordinary.insert(maintenance),
                b'b' => snake_food.insert(maintenance),
                _ => false,
            };
        }
        booster.pump(&mut sim, 1);
    }
    assert_eq!(ordinary, BTreeSet::from([true]));
    assert!(snake_food.contains(&false), "{snake_food:?}");
}

#[test]
fn prey_corrections_share_one_clock_across_viewers() {
    let mut config = raw_config(2);
    config.world.prey.count = 3;
    let mut sim = RawSimulator::new(4, config, 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "A");
    run(&mut sim, &mut [(0, &mut a)], 100);
    // A later viewer joins the existing clocks rather than starting its own.
    let mut b = Remote::connect(&mut sim, 1, "B");
    let mut seen = [Vec::new(), Vec::new()];
    for _ in 0..400 {
        sim.advance_ticks(1).unwrap();
        for (seen, (slot, remote)) in seen.iter_mut().zip([(0, &mut a), (1, &mut b)]) {
            let packets = remote.pump(&mut sim, slot);
            seen.extend(
                packets
                    .into_iter()
                    .filter(|p| p[0] == b'j')
                    .map(|p| (sim.backend.tick, p[1..3].to_vec())),
            );
        }
    }
    let late = seen[1].first().expect("second viewer saw a correction").0;
    let first: Vec<_> = seen[0].iter().filter(|(tick, _)| *tick >= late).collect();
    assert!(first.len() >= 6, "{first:?}");
    assert_eq!(first, seen[1].iter().collect::<Vec<_>>());
}

#[test]
fn post_death_opponents_keep_the_live_broadcast_cadence() {
    for rtt in [0., 0.1] {
        let mut config = raw_config(2);
        config.delivery = DeliveryProfile::Official;
        config.latency.round_trip_seconds = vec![rtt];
        config.minimap_interval_seconds = 0.01;
        let interval = MOTION_REPORT_TICKS as f64 * config.world.dt;
        let mut sim = RawSimulator::new(6, config, 2).unwrap();
        let mut a = Remote::connect(&mut sim, 0, "Dead");
        let mut b = Remote::connect(&mut sim, 1, "Moving");
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 60);
        sim.backend.world.retire(0).unwrap();
        let mut over = false;
        let mut deaths = 0;
        let mut motion_at = Vec::new();
        for _ in 0..120 {
            // Synthetic steering keeps the opponent's wanted angle changing
            // on every broadcast, so packet gaps expose a stopped stream.
            sim.backend.slots[1].control.heading += 0.02;
            sim.advance_ticks(1).unwrap();
            b.pump(&mut sim, 1);
            for packet in a.pump(&mut sim, 0) {
                if packet[0] == b'v' {
                    over = true;
                    deaths += 1;
                }
                if !over {
                    continue;
                }
                assert!(
                    !matches!(packet[0], b'M' | b'V' | b'u'),
                    "post-death minimap"
                );
                assert!(!matches!(packet[0], b'G' | b'N' | b'd' | b'7'));
                if matches!(packet[0], b'e' | b'E' | b'3' | b'4' | b'5')
                    && packet.get(1..3) == Some(&[0, 1])
                {
                    motion_at.push(sim.time());
                }
            }
        }
        assert_eq!(deaths, 1);
        assert!(
            motion_at.len() >= 10,
            "missing post-death motion: {motion_at:?}"
        );
        for gap in motion_at.windows(2) {
            assert!((gap[1] - gap[0] - interval).abs() < 1e-9, "{gap:?}");
        }
        assert_eq!(a.frame(&sim).status, ClientStatus::Dead);
        assert!(a.frame(&sim).observation.is_none());
    }
}

#[test]
fn sole_dead_session_still_receives_food_and_prey() {
    let mut sim = RawSimulator::new(6, raw_config(1), 1).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "");
    run(&mut sim, &mut [(0, &mut a)], 12);
    sim.backend.world.retire(0).unwrap();
    assert!(
        next_broadcast(&mut sim, 0, &mut a)
            .iter()
            .any(|p| p[0] == b'v')
    );
    let head = sim.backend.world.snakes()[0].head;
    let food = crate::model::Food {
        x: head.x + 100.,
        y: head.y,
        value: 1.,
        size: Some(2.),
        origin: None,
    };
    let key = encode::food_key(food.point()).unwrap();
    let prey = crate::prey::Prey {
        id: 123,
        position: Point {
            x: head.x,
            y: head.y + 100.,
        },
        heading: 0.,
        target_heading: 0.,
        speed: 100.,
        size: 6.,
    };
    let appear = encode::prey_appearance(123, &prey).unwrap();
    // Synthetic world edits isolate visibility even with no living sessions.
    sim.backend.world.insert_food(food).unwrap();
    sim.backend
        .world
        .insert_prey(crate::ReplayPrey {
            prey,
            mode: crate::PreyMode::Roaming,
        })
        .unwrap();
    sim.backend.next_minimap = 0;
    sim.backend.broadcast().unwrap();
    let packets = a.pump(&mut sim, 0);
    assert!(
        packets.contains(&encode::food_add(
            key,
            encode::FoodStyle {
                color: sim
                    .backend
                    .food_presentation
                    .color(sim.backend.world.food().last().unwrap(), 0),
                size: encode::food_size(2.),
                snake_food: false,
            }
        ))
    );
    assert!(packets.contains(&appear));
    assert!(
        packets
            .iter()
            .all(|p| !matches!(p[0], b'v' | b'M' | b'V' | b'u'))
    );
    assert_eq!(a.frame(&sim).status, ClientStatus::Dead);
}

#[test]
fn remote_death_sends_mass_game_over_then_removal_and_stays_dead() {
    let mut sim = RawSimulator::new(6, raw_config(3), 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "");
    let mut b = Remote::connect(&mut sim, 1, "");
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 13);
    while !sim.backend.tick.is_multiple_of(6) {
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 1);
    }
    let snake = sim.backend.world.snakes()[0].clone();
    sim.backend.world.correct_mass(0, snake.body, 0.75).unwrap();
    sim.backend.world.retire(0).unwrap();
    sim.advance_ticks(6).unwrap();
    let fam = encode::fam_u24(0.75);
    let death = [
        encode::fam(0, fam),
        encode::game_over(false),
        encode::removal(0, true),
    ];
    let packets = a.pump(&mut sim, 0);
    assert!(packets.starts_with(&death));
    assert!(packets.iter().all(|p| p[0] != b'm'));
    assert!(sim.backend.record.is_none());
    assert_eq!(a.frame(&sim).status, ClientStatus::Dead);
    let seen = b.pump(&mut sim, 1);
    assert!(seen.contains(&encode::removal(0, true)));
    assert!(seen.iter().all(|p| p[0] != b'm'));
    // A dead session must still answer pings during the stock client's
    // death fade, without accepting controls or reviving the snake.
    let control = sim.backend.slots[0].control;
    for bytes in [&[0][..], &[252, 100], &[253], &[251]] {
        sim.receive(0, bytes).unwrap();
    }
    let rank = sim.backend.slots[0].connection.final_rank.unwrap();
    let mut boards = 0;
    let mut pongs = 0;
    for _ in 0..600 {
        sim.advance_ticks(1).unwrap();
        b.pump(&mut sim, 1);
        let living = sim
            .backend
            .world
            .snakes()
            .iter()
            .filter(|s| s.alive)
            .count();
        for packet in a.pump(&mut sim, 0) {
            if packet == encode::pong() {
                pongs += 1;
                continue;
            }
            assert!(!matches!(packet[0], b'v' | b'M' | b'V' | b'u'));
            if packet[0] != b'l' {
                continue;
            }
            let (position, own_rank, count, _) = board_packet(&packet);
            assert_eq!((position, own_rank, count), (0, rank, living));
            boards += 1;
        }
    }
    assert!(boards > 0);
    assert_eq!(pongs, 1);
    assert_eq!(sim.backend.slots[0].control, control);
    assert!(!sim.backend.world.snakes()[0].alive);
}

/// `(segments, fam, color, name)` of one leaderboard row.
type Row = (usize, u32, u8, Vec<u8>);
/// Fields of the stock `l` handler: own row, rank, population and rows.
fn board_packet(p: &[u8]) -> (u8, usize, usize, Vec<Row>) {
    assert_eq!(p[0], b'l');
    let u16_at = |m: usize| usize::from(u16::from_be_bytes([p[m], p[m + 1]]));
    let mut rows = Vec::new();
    let mut m = 6;
    while m < p.len() {
        let fam = u32::from_be_bytes([0, p[m + 2], p[m + 3], p[m + 4]]);
        let n = usize::from(p[m + 6]);
        rows.push((u16_at(m), fam, p[m + 5], p[m + 7..m + 7 + n].to_vec()));
        m += 7 + n;
    }
    (p[1], u16_at(2), u16_at(4), rows)
}
fn set_mass(snake: &mut Snake, segments: usize, fam: f64) {
    snake.segments = segments;
    snake.fam = fam;
    snake.score = physics::score(segments, fam);
    snake.body.resize(segments, *snake.body.last().unwrap());
}
fn correct_mass(world: &mut Simulator, slot: usize, segments: usize, fam: f64) {
    let mut body = world.snakes()[slot].body.clone();
    body.resize(segments, *body.last().unwrap());
    world.correct_mass(slot, body, fam).unwrap();
}
/// Run to the next motion-report boundary; returns the packets delivered.
fn next_broadcast(sim: &mut RawSimulator, slot: usize, remote: &mut Remote) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    loop {
        sim.advance_ticks(1).unwrap();
        packets.extend(remote.pump(sim, slot));
        if sim.backend.tick.is_multiple_of(MOTION_REPORT_TICKS) {
            return packets;
        }
    }
}

#[test]
fn leaderboard_ranks_living_snakes_with_slot_ties() {
    let mut config = raw_config(13);
    config.world.radius = 3000.;
    let mut sim = RawSimulator::new(1, config, 2).unwrap();
    let mut zero = Remote::connect(&mut sim, 0, "Zero");
    run(&mut sim, &mut [(0, &mut zero)], 7);
    // Slot 1 never connected and slot 4 is dead: neither is counted.
    let world = &mut sim.backend.world;
    correct_mass(world, 0, 30, 0.5);
    for slot in 2..13 {
        correct_mass(world, slot, 10 + slot, 0.25);
    }
    correct_mass(world, 3, 40, 0.);
    correct_mass(world, 5, 40, 0.);
    world.retire(4).unwrap();
    let board = Board::new(&sim.backend.world, &sim.backend.profiles).unwrap();
    let order: Vec<_> = board.order.iter().map(|&(_, slot)| slot).collect();
    assert_eq!(order, [3, 5, 0, 12, 11, 10, 9, 8, 7, 6, 2]);
    let p = encode::leaderboard(3, 3, 11, &board.rows);
    let (_, _, count, rows) = board_packet(&p);
    assert_eq!(count, 11);
    assert_eq!(rows.len(), LEADERBOARD_ROWS);
    assert_eq!(rows[0], (40, 0, 0, b"3".to_vec()));
    assert_eq!(rows[1].3, b"5");
    assert_eq!(rows[2], (30, encode::fam_u24(0.5), 7, b"Zero".to_vec()));
    assert_eq!(rows[9].3, b"6");
    // A dead snake ranks after living ones it does not outscore; slot breaks ties.
    let score = |segments| encode::display_score(segments, 0);
    assert_eq!(board.rank(score(40), 4), 2);
    assert_eq!(board.rank(score(40), 1), 1);
    assert_eq!(board.rank(score(2), 4), 12);

    // Each connection's packet shares the rows, with its own position.
    zero.pump(&mut sim, 0);
    sim.backend.next_leaderboard = 0;
    sim.advance_ticks(1).unwrap();
    let board = Board::new(&sim.backend.world, &sim.backend.profiles).unwrap();
    let index = board.order.iter().position(|&(_, i)| i == 0).unwrap();
    let count = board.order.len();
    let expected = encode::leaderboard(index as u8 + 1, index + 1, count, &board.rows);
    assert!(zero.pump(&mut sim, 0).contains(&expected));
}

#[test]
fn leaderboards_follow_the_server_clock_after_own_appearance() {
    let mut config = raw_config(2);
    config.world.radius = 10_000.;
    let mut sim = RawSimulator::new(4, config, 1).unwrap();
    let interval = sim.backend.config.leaderboard_interval_seconds;
    // Join between periods: the first board waits for the running phase.
    sim.advance_ticks(143).unwrap();
    let joined = sim.time();
    let mut remote = Remote::connect(&mut sim, 0, "Me");
    let mut times = Vec::new();
    while sim.time() < joined + 9.5 {
        sim.advance_ticks(1).unwrap();
        let packets = remote.pump(&mut sim, 0);
        let living = sim
            .backend
            .world
            .snakes()
            .iter()
            .filter(|s| s.alive)
            .count();
        let own = &sim.backend.slots[0].connection;
        for packet in packets.iter().filter(|p| p[0] == b'l') {
            let (position, rank, count, rows) = board_packet(packet);
            assert_eq!((count, rows.len()), (living, living));
            times.push(sim.time());
            // A dead remote keeps its frozen rank without a row.
            if own.over {
                assert_eq!((position, Some(rank)), (0, own.final_rank));
                continue;
            }
            assert!(position == rank as u8 && (1..=living).contains(&rank));
            assert_eq!(rows[rank - 1].3, b"Me");
        }
    }
    assert_eq!(times.len(), 3, "{times:?}");
    assert!(times[0] - joined < interval - 0.5, "{times:?}");
    for time in &times {
        let phase = (time / interval).fract() * interval;
        assert!(phase < sim.dt() + 1e-9, "{times:?}");
    }
}

#[test]
fn credit_and_hud_carry_the_represented_score() {
    let mut sim = RawSimulator::new(2, raw_config(2), 1).unwrap();
    let mut remote = Remote::connect(&mut sim, 0, "Me");
    run(&mut sim, &mut [(0, &mut remote)], 7);
    // A score above the shown mass, as kiosk credit produces; overridden
    // here between ticks, then broadcast without a physics step.
    let world = &mut sim.backend.world;
    let own = &world.snakes()[0];
    let fam = encode::fam_u24(own.fam);
    let own_segments = own.segments;
    let own_score = encode::display_score(own.segments + 5, fam);
    let bot = &world.snakes()[1];
    let bot_fam = encode::fam_u24(bot.fam);
    let bot_segments = bot.segments;
    let bot_score = encode::display_score(bot.segments + 3, bot_fam);
    let mut saved = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
    for (slot, score) in [(0, own_score), (1, bot_score)] {
        let snake = &world.snakes()[slot];
        saved["state"]["slots"][slot]["kiosk"]["scoreCredit"] =
            serde_json::json!(score - physics::score_exact(snake.segments, snake.fam));
        saved["state"]["slots"][slot]["snake"]["score"] = serde_json::json!(score);
    }
    world
        .restore(
            &serde_json::to_vec(&saved).unwrap(),
            crate::RestoreOptions {
                events: crate::EventRecording {
                    pickups: true,
                    food_births: true,
                },
                #[cfg(feature = "parallel")]
                parallel: true,
            },
        )
        .unwrap();
    sim.backend.next_leaderboard = 0;
    sim.backend.broadcast().unwrap();
    let packets = remote.pump(&mut sim, 0);
    assert!(packets.contains(&encode::credit(5)));
    let board = packets.iter().find(|p| p[0] == b'l').unwrap();
    let (_, _, _, rows) = board_packet(board);
    let mut rows: Vec<_> = rows.into_iter().map(|row| (row.0, row.1)).collect();
    rows.sort();
    let mut expected = vec![(own_segments + 5, fam), (bot_segments + 3, bot_fam)];
    expected.sort();
    assert_eq!(rows, expected);
    // The decoder applies the transmitted own credit to its own score.
    let me = remote.frame(&sim).observation.unwrap().me;
    assert_eq!(me.score, encode::display_score(me.segments + 5, fam));
    // A checked replay life resets the kiosk credit before the next report.
    for slot in 0..2 {
        let snake = sim.backend.world.snakes()[slot].clone();
        let speed = snake.speed;
        sim.backend
            .world
            .import_life(
                slot,
                crate::ReplayLife::from_observed(snake, crate::InitialSpeed::Infer(speed)),
            )
            .unwrap();
    }
    let packets = next_broadcast(&mut sim, 0, &mut remote);
    assert!(packets.contains(&encode::credit(0)));
    let packets = next_broadcast(&mut sim, 0, &mut remote);
    assert!(packets.iter().all(|p| p[0] != b'R'));
}

#[test]
fn kill_counts_follow_appearances_and_changes() {
    let mut sim = RawSimulator::new(3, raw_config(3), 1).unwrap();
    {
        let mut snake = sim.backend.world.snakes()[1].clone();
        snake.kills = 3;
        sim.backend
            .world
            .import_life(
                1,
                crate::ReplayLife::from_observed(
                    snake.clone(),
                    crate::InitialSpeed::Infer(snake.speed),
                ),
            )
            .unwrap();
    }
    let mut remote = Remote::connect(&mut sim, 0, "");
    run(&mut sim, &mut [(0, &mut remote)], 7);
    let packets = &remote.packets;
    let appear = packets.iter().position(|p| is_full(p) && p[1..3] == [0, 1]);
    let kill = packets.iter().position(|p| *p == encode::kills(1, 3));
    // After the appearance and its motion; zero counts are not sent.
    assert_eq!(kill.unwrap(), appear.unwrap() + 2);
    assert_eq!(packets.iter().filter(|p| p[0] == b'k').count(), 1);
    {
        let mut snake = sim.backend.world.snakes()[2].clone();
        snake.kills = 1;
        sim.backend
            .world
            .import_life(
                2,
                crate::ReplayLife::from_observed(
                    snake.clone(),
                    crate::InitialSpeed::Infer(snake.speed),
                ),
            )
            .unwrap();
    }
    {
        let mut snake = sim.backend.world.snakes()[0].clone();
        snake.kills = 2;
        sim.backend
            .world
            .import_life(
                0,
                crate::ReplayLife::from_observed(
                    snake.clone(),
                    crate::InitialSpeed::Infer(snake.speed),
                ),
            )
            .unwrap();
    }
    let packets = next_broadcast(&mut sim, 0, &mut remote);
    let kills: Vec<_> = packets.iter().filter(|p| p[0] == b'k').cloned().collect();
    assert_eq!(kills, [encode::kills(0, 2), encode::kills(2, 1)]);
    let packets = next_broadcast(&mut sim, 0, &mut remote);
    assert!(packets.iter().all(|p| p[0] != b'k'));
}

#[test]
fn sole_dead_record_holder_sets_text_once_and_slot_reuse_cannot() {
    let mut config = raw_config(1);
    config.local_records = true;
    let mut sim = RawSimulator::new(7, config, 1).unwrap();
    let mut ann = Remote::connect(&mut sim, 0, "Ann");
    run(&mut sim, &mut [(0, &mut ann)], 7);
    // Text before a record exists, and invalid text, change nothing.
    sim.victory(0, b"early").unwrap();
    assert!(sim.victory(0, &[b'x'; MAX_VICTORY_TEXT + 1]).is_err());
    assert!(sim.victory(0, "\u{e9}".as_bytes()).is_err());
    sim.victory(0, &[b'x'; MAX_VICTORY_TEXT]).unwrap();
    sim.advance_ticks(1).unwrap();
    assert!(sim.backend.record.is_none());
    let die = |sim: &mut RawSimulator, segments| {
        let mut detached_snake = sim.backend.world.snakes()[0].clone();
        let snake = &mut detached_snake;
        set_mass(snake, segments, 0.5);
        snake.alive = false;
        sim.backend
            .world
            .import_life(
                0,
                crate::ReplayLife::from_observed(
                    snake.clone(),
                    crate::InitialSpeed::Infer(snake.speed),
                ),
            )
            .unwrap();
    };
    die(&mut sim, 30);
    let fam = encode::fam_u24(0.5);
    let packets = next_broadcast(&mut sim, 0, &mut ann);
    assert!(packets.contains(&encode::game_over(true)));
    assert!(packets.contains(&encode::record(30, fam, b"Ann", b"").unwrap()));

    // The only connection is over; its accepted text still reaches it.
    sim.victory(0, b"gg").unwrap();
    sim.receive(0, &[251]).unwrap();
    let packets = next_broadcast(&mut sim, 0, &mut ann);
    assert_eq!(
        packets,
        [
            encode::victory_accepted(),
            encode::pong(),
            encode::record(30, fam, b"Ann", b"gg").unwrap(),
        ]
    );
    sim.victory(0, b"again").unwrap();
    ann.packets.clear();
    run(&mut sim, &mut [(0, &mut ann)], 700);
    assert!(
        ann.packets
            .iter()
            .all(|p| p[0] != b'm' || p.ends_with(b"gg"))
    );
    assert!(!ann.packets.contains(&encode::pong()));

    // A new session in the slot sees the record but cannot edit it, and a
    // lower or equal life does not replace it.
    sim.disconnect(0).unwrap();
    let mut bob = Remote::connect(&mut sim, 0, "Bob");
    run(&mut sim, &mut [(0, &mut bob)], 7);
    assert!(
        bob.packets
            .contains(&encode::record(30, fam, b"Ann", b"gg").unwrap())
    );
    die(&mut sim, 30);
    let packets = next_broadcast(&mut sim, 0, &mut bob);
    assert!(packets.contains(&encode::game_over(false)));
    sim.victory(0, b"mine").unwrap();
    run(&mut sim, &mut [(0, &mut bob)], 30);
    assert_eq!(bob.packets.iter().filter(|p| p[0] == b'm').count(), 1);
    let record = sim.backend.record.as_ref().unwrap();
    assert_eq!(
        (&record.name[..], &record.message[..]),
        (&b"Ann"[..], &b"gg"[..])
    );

    let mut bob = Remote::connect(&mut sim, 0, "Bob");
    run(&mut sim, &mut [(0, &mut bob)], 7);
    die(&mut sim, 31);
    let packets = next_broadcast(&mut sim, 0, &mut bob);
    assert!(packets.contains(&encode::game_over(true)));
    assert!(packets.contains(&encode::record(31, fam, b"Bob", b"").unwrap()));
    // Submitting an empty message still dismisses the stock input once.
    sim.victory(0, b"").unwrap();
    let packets = next_broadcast(&mut sim, 0, &mut bob);
    assert!(packets.contains(&encode::victory_accepted()));
    sim.victory(0, b"late replacement").unwrap();
    let packets = next_broadcast(&mut sim, 0, &mut bob);
    assert!(!packets.contains(&encode::victory_accepted()));
    assert!(sim.backend.record.as_ref().unwrap().message.is_empty());
    assert!(sim.backend.record.as_ref().unwrap().message_set);
    // A new world run starts without a record.
    assert!(
        RawSimulator::new(7, raw_config(1), 1)
            .unwrap()
            .backend
            .record
            .is_none()
    );
}

#[test]
fn session_id_follows_own_appearance_once_per_connection() {
    let mut sim = RawSimulator::new(5, raw_config(2), 2).unwrap();
    assert!(sim.request_session_id(0).is_err());
    let mut a = Remote::connect(&mut sim, 0, "");
    sim.request_session_id(0).unwrap();
    sim.request_session_id(0).unwrap();
    let mut b = Remote::connect(&mut sim, 1, "");
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 20);
    let session = sim.backend.slots[0].connection.session;
    let sessions: Vec<_> = a.packets.iter().filter(|p| p[0] == b'S').collect();
    assert_eq!(sessions, [&encode::session(0, session)]);
    let at = a.packets.iter().position(|p| p[0] == b'S').unwrap();
    assert!(a.packets[0][0] == b'a' && is_full(&a.packets[1]) && at > 1);
    assert_eq!(a.frame(&sim).status, ClientStatus::Playing);
    assert!(b.packets.iter().all(|p| p[0] != b'S'));

    // Slot reuse gets a fresh ID; IDs stay distinct per connection.
    let mut seen = BTreeSet::from([session, sim.backend.slots[1].connection.session]);
    for _ in 0..200 {
        sim.disconnect(0).unwrap();
        sim.connect(0, profile("")).unwrap();
        sim.request_session_id(0).unwrap();
        assert!(seen.insert(sim.backend.slots[0].connection.session));
    }
    // Deterministic for a seed.
    let mut again = RawSimulator::new(5, raw_config(2), 2).unwrap();
    again.connect(0, profile("")).unwrap();
    assert_eq!(again.backend.slots[0].connection.session, session);
}

#[test]
fn disconnect_removes_the_snake_and_a_reused_slot_is_a_new_life() {
    let mut sim = RawSimulator::new(8, raw_config(3), 2).unwrap();
    let mut a = Remote::connect(&mut sim, 0, "Old");
    let mut b = Remote::connect(&mut sim, 1, "");
    run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 13);
    sim.receive(0, &[251]).unwrap();
    sim.disconnect(0).unwrap();
    assert!(!sim.backend.world.snakes()[0].alive);
    assert!(sim.drain(0).is_err() && sim.receive(0, &[1]).is_err());
    sim.disconnect(0).unwrap();
    run(&mut sim, &mut [(1, &mut b)], 6);
    assert!(b.packets.contains(&encode::removal(0, true)));
    let observation = b.frame(&sim).observation.unwrap();
    assert!(observation.opponents.iter().all(|o| o.id != "0"));

    // Replace a life before any broadcast: observers drop the stale one.
    Remote::connect(&mut sim, 0, "Old");
    run(&mut sim, &mut [(1, &mut b)], 6);
    let life = sim.backend.slots[0].life;
    sim.disconnect(0).unwrap();
    b.packets.clear();
    let mut c = Remote::connect(&mut sim, 0, "New");
    assert_eq!(sim.backend.slots[0].life, life + 1);
    run(&mut sim, &mut [(0, &mut c), (1, &mut b)], 7);
    let removed = b
        .packets
        .iter()
        .position(|p| *p == encode::removal(0, true));
    let full = b
        .packets
        .iter()
        .position(|p| is_full(p) && p[1..3] == [0, 0]);
    assert!(removed.unwrap() < full.unwrap());
    assert_eq!(b.packets[full.unwrap()][22..26], [3, b'N', b'e', b'w']);
    // The new session starts from its own setup, without the old pong.
    assert_eq!(c.packets[0][0], b'a');
    assert!(is_full(&c.packets[1]) && c.packets[1][22..26] == [3, b'N', b'e', b'w']);
    assert!(!c.packets.contains(&encode::pong()));
    assert_eq!(c.frame(&sim).status, ClientStatus::Playing);
}

#[test]
fn aggregate_broadcasts_fit_transport_limits_and_preserve_packet_order() {
    let mut backend = SimulatorBackend::new(1, SimulationConfig::default()).unwrap();
    let packets: Vec<_> = (0..130)
        .map(|index| {
            let mut packet = vec![b'F'; 8191];
            packet[1] = index;
            packet
        })
        .collect();
    backend.emit(0, 0., &packets).unwrap();
    let frames = &backend.slots[0].connection.downlink;
    assert!(frames.len() > 1);
    let mut decoded = Vec::new();
    for (_, bytes) in frames {
        // Reserve the maximum unmasked WebSocket header inside the host's
        // 1 MiB send buffer, also staying below the native receive limit.
        assert!(bytes.len() + 10 <= 1 << 20);
        decoded.extend(protocol::split_frame(bytes).unwrap());
    }
    assert_eq!(
        decoded,
        packets.iter().map(Vec::as_slice).collect::<Vec<_>>()
    );
}

#[test]
fn oversized_packets_travel_alone_in_stream_order() {
    let config = SimulationConfig {
        minimap_side: Some(512),
        record_wire: true,
        ..Default::default()
    };
    let mut backend = SimulatorBackend::new(1, config).unwrap();
    while backend.frame().unwrap().status != ClientStatus::Playing {
        backend.advance_ticks(1).unwrap();
    }
    backend.drain_records(0).unwrap();
    let cells = (0..512 * 512).map(|i| u8::from(i % 2 == 0)).collect();
    let dense = Minimap {
        side: 512,
        cells,
        age_seconds: 0.,
    };
    let packet = encode::minimap(&dense);
    assert!(!encode::frameable(&packet));
    let now = backend.time();
    backend
        .emit(0, now, &[encode::pong(), packet, encode::pong()])
        .unwrap();
    assert_eq!(backend.slots[0].connection.downlink.len(), 3);
    backend.advance_ticks(1).unwrap();
    let records = backend.drain_records(0).unwrap();
    let sizes: Vec<_> = records
        .iter()
        .filter(|r| r.event == WireEvent::Receive)
        .map(|r| r.bytes.len())
        .take(3)
        .collect();
    assert_eq!(sizes[0], 3);
    assert!(sizes[1] > 8192);
    assert_eq!(sizes[2], 3);
    let received = backend.frame().unwrap().minimap.unwrap();
    assert_eq!(received.cells, dense.cells);
    // Split frames share one emission time: later ones carry zero elapsed.
    let headers: Vec<_> = records
        .iter()
        .filter(|r| r.event == WireEvent::Receive)
        .take(3)
        .map(|r| u16::from_be_bytes([r.bytes[0], r.bytes[1]]))
        .collect();
    assert_eq!(headers[1..], [0, 0]);
    let last = records
        .iter()
        .filter(|r| r.event == WireEvent::Receive)
        .map(|r| r.time)
        .fold(now, f64::max);
    let connection = &backend.slots[0].connection;
    let since_setup = (last - connection.setup_at.unwrap()) * 1000.;
    assert!((connection.header_ms as f64 - since_setup).abs() <= 1.);
}

#[test]
fn recording_exhaustion_preserves_reconnect_state_and_allows_retry() {
    let snapshot = |backend: &SimulatorBackend| {
        let s = &backend.slots[0];
        (
            backend.frame().unwrap(),
            serde_json::to_value(backend.world.checkpoint().unwrap()).unwrap(),
            format!(
                "{:?}",
                (
                    (&s.state, &s.connection),
                    (
                        s.life,
                        s.outbox,
                        s.processing_override,
                        s.previous_boost,
                        s.control,
                        s.pinged_at,
                        s.living,
                    ),
                    (&s.records, s.record_bytes),
                    (
                        backend.connections,
                        &backend.random,
                        &backend.spawn_random,
                        &backend.record,
                    ),
                )
            ),
        )
    };
    for used in [MAX_RECORD_BYTES, MAX_RECORD_BYTES - RECORD_OVERHEAD] {
        let mut backend = SimulatorBackend::new(
            1,
            SimulationConfig {
                record_wire: true,
                latency: LatencyConfig {
                    processing_seconds: [0.1; 2],
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        while backend.frame().unwrap().status != ClientStatus::Playing {
            backend.advance_ticks(1).unwrap();
        }
        backend
            .send_to(
                0,
                Action {
                    heading: 1.,
                    boost: true,
                },
            )
            .unwrap();
        backend.slots[0].record_bytes = used;
        let before = snapshot(&backend);
        if used == MAX_RECORD_BYTES {
            assert_eq!(
                backend.connect(0).unwrap_err(),
                "Wire record buffer is full; drain records first"
            );
            assert_eq!(snapshot(&backend), before);
        }
        assert_eq!(
            backend.reconnect_slot(0).unwrap_err(),
            "Wire record buffer is full; drain records first"
        );
        assert_eq!(snapshot(&backend), before);

        let life = backend.slots[0].life;
        let connections = backend.connections;
        let mut records = backend.drain_records(0).unwrap();
        backend.reconnect_slot(0).unwrap();
        assert_eq!(backend.slots[0].life, life + 1);
        assert_eq!(backend.connections, connections + 1);
        while backend.frame().unwrap().status != ClientStatus::Playing {
            backend.advance_ticks(1).unwrap();
        }
        records.extend(backend.drain_records(0).unwrap());
        let mut replay = ClientState::new();
        for record in &records {
            super::super::replay_record(&mut replay, record).unwrap();
        }
        assert_eq!(
            replay.frame(backend.time()).unwrap(),
            backend.frame().unwrap()
        );
    }
}

#[test]
fn reconnect_rechecks_record_room_after_queued_delivery() {
    let mut backend = SimulatorBackend::new(
        1,
        SimulationConfig {
            record_wire: true,
            ..Default::default()
        },
    )
    .unwrap();
    while backend.frame().unwrap().status != ClientStatus::Playing {
        backend.advance_ticks(1).unwrap();
    }
    let now = backend.time();
    let s = &mut backend.slots[0];
    s.connection
        .downlink
        .push_front((now, vec![0, 0, b'e', 64]));
    s.record_bytes = MAX_RECORD_BYTES - 2 * RECORD_OVERHEAD;
    let mut connection = s.connection.clone();
    connection.downlink.pop_front();
    let life = s.life;
    let connections = backend.connections;
    let world = serde_json::to_value(backend.world.checkpoint().unwrap()).unwrap();
    let random = format!("{:?}", (&backend.random, &backend.spawn_random));
    let records = backend.slots[0].records.as_ref().unwrap().len();

    assert_eq!(
        backend.reconnect_slot(0).unwrap_err(),
        "Wire record buffer is full; drain records first"
    );
    assert_eq!(backend.frame().unwrap().status, ClientStatus::Playing);
    assert_eq!(backend.slots[0].life, life);
    assert_eq!(backend.connections, connections);
    assert_eq!(
        serde_json::to_value(backend.world.checkpoint().unwrap()).unwrap(),
        world
    );
    assert_eq!(
        format!("{:?}", (&backend.random, &backend.spawn_random)),
        random
    );
    assert_eq!(
        format!("{:?}", backend.slots[0].connection),
        format!("{connection:?}")
    );
    assert_eq!(
        backend.slots[0].records.as_ref().unwrap().len(),
        records + 1
    );
    assert_eq!(
        backend.slots[0]
            .records
            .as_ref()
            .unwrap()
            .last()
            .unwrap()
            .event,
        WireEvent::Receive
    );
}

#[test]
fn rejected_departure_leaves_every_side_unchanged() {
    let config = SimulationConfig {
        record_wire: true,
        ..Default::default()
    };
    let mut backend = SimulatorBackend::new(1, config).unwrap();
    while backend.frame().unwrap().status != ClientStatus::Playing {
        backend.advance_ticks(1).unwrap();
    }
    backend.drain_records(0).unwrap();
    // Room for the heading record alone, not heading plus boost.
    backend.slots[0].record_bytes = MAX_RECORD_BYTES - 2 * RECORD_OVERHEAD - 1;
    let uplink = backend.slots[0].connection.uplink.len();
    let control = backend.slots[0].control;
    let action = Action {
        heading: 1.,
        boost: true,
    };
    assert!(backend.send_to(0, action).is_err());
    let s = &backend.slots[0];
    assert!(s.outbox.is_some_and(|(_, queued, _)| queued == action));
    assert_eq!(s.previous_boost, None);
    assert_eq!(s.connection.uplink.len(), uplink);
    assert_eq!(s.control, control);
    assert!(s.records.as_ref().unwrap().is_empty());
    assert!(backend.frame().unwrap().timing.sent_commands.is_empty());
    assert!(backend.decision_pending(0).unwrap());

    // The queued decision departs once records are drained.
    backend.drain_records(0).unwrap();
    backend.advance_ticks(1).unwrap();
    let sent: Vec<_> = backend
        .drain_records(0)
        .unwrap()
        .into_iter()
        .filter(|r| r.event == WireEvent::Send && r.bytes != [251])
        .map(|r| r.bytes)
        .collect();
    assert_eq!(sent.len(), 2);
    assert_eq!(sent[1], [253]);
    assert_eq!(backend.slots[0].previous_boost, Some(true));
    let commands = backend.frame().unwrap().timing.sent_commands;
    assert!(commands.len() == 1 && commands[0].action.boost);
}

#[test]
fn rejected_departure_clock_check_leaves_every_side_unchanged() {
    let mut backend = SimulatorBackend::new(
        1,
        SimulationConfig {
            record_wire: true,
            ..Default::default()
        },
    )
    .unwrap();
    while backend.frame().unwrap().status != ClientStatus::Playing {
        backend.advance_ticks(1).unwrap();
    }
    let at = backend.time();
    let s = &mut backend.slots[0];
    // Reproduce the future client clock left by a partial pre-tick failure.
    s.state.record_sent(&[253], at + 0.001).unwrap();
    s.previous_boost = Some(true);
    s.control.boost = true;
    s.outbox = Some((
        at,
        Action {
            heading: 1.,
            boost: false,
        },
        0.,
    ));
    let snapshot = |backend: &SimulatorBackend| {
        let s = &backend.slots[0];
        format!(
            "{:?}",
            (
                &s.state,
                &s.connection,
                s.outbox,
                s.previous_boost,
                s.control,
                &s.records,
                s.record_bytes,
                &backend.random,
            )
        )
    };
    let before = snapshot(&backend);
    assert_eq!(
        backend.client_io_until(0, at).unwrap_err(),
        "Invalid local clock"
    );
    assert_eq!(snapshot(&backend), before);
}

#[test]
fn native_capacity_failure_keeps_all_slots_retryable() {
    for failure in [
        "departure records",
        "receipt and departure records",
        "uplink",
    ] {
        let config = SimulationConfig {
            world: Config {
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::battledome()
            },
            agents: 2,
            record_wire: true,
            ..Default::default()
        };
        let mut backend = SimulatorBackend::new(1, config.clone()).unwrap();
        let mut uninterrupted = SimulatorBackend::new(1, config).unwrap();
        for sim in [&mut backend, &mut uninterrupted] {
            while (0..2).any(|slot| sim.frame_for(slot).unwrap().status != ClientStatus::Playing) {
                sim.advance_ticks(1).unwrap();
            }
            let at = sim.time();
            for slot in 0..2 {
                sim.drain_records(slot).unwrap();
                sim.set_slot_processing_seconds(slot, 0.001).unwrap();
                sim.send_to(
                    slot,
                    Action {
                        heading: sim.slots[slot].control.heading,
                        boost: true,
                    },
                )
                .unwrap();
                if failure == "receipt and departure records" {
                    sim.slots[slot]
                        .connection
                        .downlink
                        .push_back((at + 0.0005, encode::frame(0, &[encode::pong()]).unwrap()));
                }
            }
        }
        let uplink_len = backend.slots[1].connection.uplink.len();
        let expected_error = match failure {
            "uplink" => {
                // Synthetic queue pressure; these entries must not be ingested.
                backend.slots[1].connection.uplink.resize(
                    super::connection::MAX_IN_FLIGHT,
                    (f64::MAX, super::connection::Uplink::Message(vec![0])),
                );
                "Uplink queue is full"
            }
            _ => {
                // The combined case fits either the receipts or the two command
                // records alone, so the preflight must charge them together.
                backend.slots[1].record_bytes = MAX_RECORD_BYTES - 2 * (RECORD_OVERHEAD + 1)
                    + usize::from(failure == "departure records");
                "Wire record buffer is full; drain records first"
            }
        };
        let snapshot = |sim: &SimulatorBackend| {
            (
                sim.time(),
                serde_json::to_value(sim.world.checkpoint().unwrap()).unwrap(),
                format!("{:?}", sim.random),
                (0..2)
                    .map(|slot| {
                        let s = &sim.slots[slot];
                        (
                            sim.frame_for(slot).unwrap(),
                            format!(
                                "{:?}",
                                (
                                    &s.state,
                                    &s.connection,
                                    s.outbox,
                                    s.previous_boost,
                                    s.control,
                                    &s.records,
                                    s.record_bytes,
                                )
                            ),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        };
        let before = snapshot(&backend);
        assert_eq!(backend.advance_ticks(1).unwrap_err(), expected_error);
        assert_eq!(snapshot(&backend), before, "{failure}");
        for slot in 0..2 {
            assert!(backend.decision_pending(slot).unwrap());
        }
        assert_eq!(
            backend
                .send_to(
                    0,
                    Action {
                        heading: 1.,
                        boost: false,
                    },
                )
                .unwrap_err(),
            "Slot is still processing its previous decision"
        );
        assert_eq!(snapshot(&backend), before);

        backend.drain_records(1).unwrap();
        if failure == "uplink" {
            backend.slots[1].connection.uplink.truncate(uplink_len);
        }
        for sim in [&mut backend, &mut uninterrupted] {
            sim.advance_ticks(1).unwrap();
            for slot in 0..2 {
                assert!(!sim.decision_pending(slot).unwrap());
                assert!(sim.slots[slot].control.boost);
            }
            sim.set_slot_processing_seconds(0, 0.).unwrap();
            sim.send_to(
                0,
                Action {
                    heading: sim.slots[0].control.heading,
                    boost: false,
                },
            )
            .unwrap();
            sim.advance_ticks(1).unwrap();
            assert!(!sim.slots[0].control.boost);
            assert_eq!(sim.slots[0].previous_boost, Some(false));
        }
        // Recovery preserves bytes, receipt/departure times and future RNG draws.
        assert_eq!(snapshot(&backend), snapshot(&uninterrupted), "{failure}");
        let records = backend.drain_records(0).unwrap();
        assert!(
            records
                .iter()
                .any(|r| r.event == WireEvent::Send && r.bytes == [254])
        );
        assert!(records.windows(2).all(|w| w[0].time <= w[1].time));
    }
}

#[test]
fn pending_native_commands_apply_in_their_arrival_tick() {
    for (processing, uplink) in [(0., 0.), (0.001, 0.), (0.007, 0.), (0.001, 0.007)] {
        let mut backend = SimulatorBackend::new(
            4,
            SimulationConfig {
                world: Config {
                    food_count: 0,
                    ambient_replenish_per_second: 0.,
                    ..Config::sandbox()
                },
                record_wire: true,
                latency: LatencyConfig {
                    processing_seconds: [processing; 2],
                    ..LatencyConfig::one_way(uplink, 0.)
                },
                ..Default::default()
            },
        )
        .unwrap();
        while backend.frame().unwrap().status != ClientStatus::Playing {
            backend.advance_ticks(1).unwrap();
        }
        let before = backend.slots[0].control.heading;
        let action = Action {
            heading: (before + 1.).rem_euclid(TAU),
            boost: true,
        };
        let started = backend.time();
        backend.send_to(0, action).unwrap();
        backend.advance_ticks(1).unwrap();
        let arrived = processing + uplink <= backend.world.config().dt;
        assert_eq!(
            backend.slots[0].control.boost, arrived,
            "processing {processing}, uplink {uplink}"
        );
        assert_eq!(backend.slots[0].control.heading != before, arrived);
        if !arrived {
            backend.advance_ticks(1).unwrap();
            assert!(backend.slots[0].control.boost);
        }
        let records = backend.drain_records(0).unwrap();
        assert!(records.windows(2).all(|w| w[0].time <= w[1].time));
        let sent = records
            .iter()
            .find(|r| r.event == WireEvent::Send && r.bytes == [253])
            .unwrap();
        assert!((sent.time - started - processing).abs() < 1e-12);
    }
}

#[test]
fn pending_departure_keeps_earlier_receipts_before_measured_processing() {
    let mut backend = SimulatorBackend::new(
        4,
        SimulationConfig {
            world: Config {
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::sandbox()
            },
            record_wire: true,
            ..Default::default()
        },
    )
    .unwrap();
    while backend.frame().unwrap().status != ClientStatus::Playing {
        backend.advance_ticks(1).unwrap();
    }
    backend.drain_records(0).unwrap();
    let at = backend.time();
    backend.slots[0]
        .connection
        .downlink
        .push_back((at + 0.0005, encode::frame(0, &[encode::pong()]).unwrap()));
    backend.set_slot_processing_seconds(0, 0.001).unwrap();
    backend
        .send_to(
            0,
            Action {
                heading: 1.,
                boost: true,
            },
        )
        .unwrap();
    let life = backend.slots[0].life;
    backend.advance_ticks(1).unwrap();
    assert!(backend.slots[0].control.boost);
    assert_eq!(backend.slots[0].life, life);
    assert!(backend.slots[0].outbox.is_none());
    let records = backend.drain_records(0).unwrap();
    assert!(records.windows(2).all(|w| w[0].time <= w[1].time));
    assert_eq!(records[0].event, WireEvent::Receive);
    assert_eq!(records[0].time, at + 0.0005);
    assert!(
        records
            .iter()
            .filter(|r| r.event == WireEvent::Send && r.bytes != [251])
            .all(|r| r.time == at + 0.001)
    );
}

#[test]
fn spawn_search_exhaustion_does_not_mean_the_arena_is_full() {
    let world = Config {
        radius: 100.5,
        food_count: 0,
        ambient_replenish_per_second: 0.,
        ..Config::sandbox()
    };
    let mut backend = SimulatorBackend::new(
        1,
        SimulationConfig {
            world,
            agents: 1,
            ..Default::default()
        },
    )
    .unwrap();
    backend.world.retire(0).unwrap();
    let (body, _) = backend
        .world
        .spawn_body(Point { x: 21., y: 0. }, 0.)
        .unwrap();
    assert!(backend.world.body_is_clear(&body, 600.));
    assert_eq!(
        backend.spawn_pose(0).unwrap_err(),
        "Spawn candidate search exhausted: no sampled position fits inside the arena"
    );
}
