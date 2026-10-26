use super::*;
use crate::{Config, Food, client::encode};

fn config() -> SimulationConfig {
    let mut world = Config::battledome();
    world.food_count = 0;
    world.ambient_replenish_per_second = 0.;
    SimulationConfig {
        world,
        agents: 1,
        ..Default::default()
    }
}

fn browser(sim: &mut RawSimulator) -> ClientState {
    sim.connect(0, PlayerAppearance::default()).unwrap();
    let mut state = ClientState::new();
    state.new_connection(sim.time()).unwrap();
    sim.advance_ticks(1).unwrap();
    pump(sim, &mut state);
    state
}

fn pump(sim: &mut RawSimulator, state: &mut ClientState) -> Vec<Vec<u8>> {
    pump_for(sim, state, 0)
}

fn pump_for(sim: &mut RawSimulator, state: &mut ClientState, slot: usize) -> Vec<Vec<u8>> {
    let now = sim.time();
    let mut packets = Vec::new();
    for frame in sim.drain(slot).unwrap() {
        state.receive_frame(&frame, now).unwrap();
        packets.extend(
            super::super::protocol::split_frame(&frame)
                .unwrap()
                .into_iter()
                .map(<[u8]>::to_vec),
        );
    }
    packets
}

#[test]
fn pit_markers_simultaneous_readers_keep_distinct_food_through_actual_pickup() {
    let mut config = config();
    config.agents = 2;
    let mut sim = RawSimulator::new(7, config, 2).unwrap();
    let mut readers = [ClientState::new(), ClientState::new()];
    for (slot, reader) in readers.iter_mut().enumerate() {
        sim.connect(slot, PlayerAppearance::default()).unwrap();
        reader.new_connection(0.).unwrap();
        // Synthetic, separated poses give both readers the same pit sectors.
        sim.backend.world.retire(slot).unwrap();
        sim.backend
            .world
            .respawn(
                slot,
                Point {
                    x: 2000.,
                    y: if slot == 0 { 300. } else { -300. },
                },
                0.,
                26,
            )
            .unwrap();
        sim.backend.slots[slot].control.heading = 0.;
    }
    sim.physical_food(1);
    sim.advance_ticks(1).unwrap();
    for (slot, reader) in readers.iter_mut().enumerate() {
        pump_for(&mut sim, reader, slot);
    }
    let marker = food(&readers[0], sim.time())
        .into_iter()
        .max_by(|a, b| a.x.total_cmp(&b.x))
        .unwrap();
    assert!(food(&readers[1], sim.time()).is_empty());
    let real = Food {
        size: Some(2.),
        value: 1.,
        origin: Some(crate::model::Origin::Ambient),
        ..marker.clone()
    };
    let key = encode::food_key(real.point()).unwrap();
    sim.backend.world.insert_food(real.clone()).unwrap();
    sim.backend.broadcast().unwrap();
    let browser_birth = pump_for(&mut sim, &mut readers[0], 0);
    let bot_birth = pump_for(&mut sim, &mut readers[1], 1);
    let at_key =
        |p: &Vec<u8>| p.len() >= 5 && matches!(p[0], b'f' | b'b' | b'c' | b'<') && p[1..5] == key;
    assert!(!browser_birth.iter().any(at_key));
    assert!(bot_birth.iter().any(|p| at_key(p) && p[0] == b'f'));
    assert!(food(&readers[0], sim.time()).contains(&marker));
    let bot_food = food(&readers[1], sim.time());
    assert_eq!(bot_food.len(), 1);
    assert_eq!(
        (bot_food[0].point(), bot_food[0].size),
        (real.point(), real.size)
    );

    // Put the bot's pickup circle over the real pellet, then let physics eat it.
    // The readers stay connected; no pickup event or wire delta is fabricated.
    let heading = std::f64::consts::FRAC_PI_2;
    let snake = &sim.backend.world.snakes()[1];
    let (lead, _) =
        crate::physics::pickup_circle(Point::default(), heading, snake.speed, snake.segments);
    sim.backend.world.retire(1).unwrap();
    sim.backend
        .world
        .respawn(
            1,
            Point {
                x: real.x - lead.x,
                y: real.y - lead.y,
            },
            heading,
            26,
        )
        .unwrap();
    sim.backend.slots[1].control.heading = heading;
    sim.advance_ticks(1).unwrap();
    assert!(sim.backend.world.snakes().iter().all(|s| s.alive));
    assert!(sim.backend.world.food().is_empty());
    let events = sim.backend.world.pickup_events().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].eater, 1);
    assert_eq!(events[0].item, crate::simulator::PickupItem::Food(real));
    let browser_pickup = pump_for(&mut sim, &mut readers[0], 0);
    let bot_pickup = pump_for(&mut sim, &mut readers[1], 1);
    assert!(
        !browser_pickup.iter().any(at_key),
        "no marker removal or pickup animation"
    );
    assert!(
        bot_pickup
            .iter()
            .any(|p| at_key(p) && p[0] == b'<' && p[5..] == [0, 1])
    );
    assert!(food(&readers[0], sim.time()).contains(&marker));
    assert!(food(&readers[1], sim.time()).is_empty());
}

fn food(state: &ClientState, now: f64) -> Vec<Food> {
    state.frame(now).unwrap().observation.unwrap().food
}

#[test]
fn pit_markers_join_sector_exit_reentry_and_reconnect() {
    let mut sim = RawSimulator::new(7, config(), 1).unwrap();
    let mut state = browser(&mut sim);
    assert!(
        !food(&state, sim.time()).is_empty(),
        "join must list markers"
    );
    assert!(sim.backend.world.food().is_empty());

    // Synthetic pose and clock jumps isolate food subscriptions from movement
    // and pit death. Normal sector retention/cleanup still runs in broadcast.
    {
        let mut snake = sim.backend.world.snakes()[0].clone();
        snake.head = Point { x: 2700., y: 0. };
        sim.backend
            .world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    sim.backend.tick += 500;
    sim.backend.broadcast().unwrap();
    pump(&mut sim, &mut state);
    let near = food(&state, sim.time());
    assert!(near.iter().any(|f| f.x > 1780.));
    {
        let mut snake = sim.backend.world.snakes()[0].clone();
        snake.head = Point { x: -2700., y: 0. };
        sim.backend
            .world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    sim.backend.tick += 500;
    sim.backend.broadcast().unwrap();
    let packets = pump(&mut sim, &mut state);
    assert!(packets.iter().any(|p| p[0] == b'w'));
    assert!(!food(&state, sim.time()).iter().any(|f| f.x > 1780.));
    {
        let mut snake = sim.backend.world.snakes()[0].clone();
        snake.head = Point { x: 2700., y: 0. };
        sim.backend
            .world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    sim.backend.tick += 500;
    sim.backend.broadcast().unwrap();
    let packets = pump(&mut sim, &mut state);
    assert!(packets.iter().any(|p| p[0] == b'F'));
    assert_eq!(food(&state, sim.time()), near);

    sim.disconnect(0).unwrap();
    let state = browser(&mut sim);
    assert!(
        !food(&state, sim.time()).is_empty(),
        "reconnect must relist markers"
    );
}

#[test]
fn pit_markers_reserve_browser_keys_but_bots_keep_colliding_real_food() {
    let mut sim = RawSimulator::new(7, config(), 1).unwrap();
    let mut state = browser(&mut sim);
    let marker = food(&state, sim.time())[0].clone();
    let real = Food {
        size: Some(2.),
        value: 1.,
        ..marker.clone()
    };
    let key = encode::food_key(real.point()).unwrap();
    sim.backend.world.insert_food(real.clone()).unwrap();
    sim.backend.broadcast().unwrap();
    let packets = pump(&mut sim, &mut state);
    assert!(
        !packets
            .iter()
            .any(|p| matches!(p[0], b'f' | b'b' | b'c' | b'<') && p[1..5] == key)
    );
    assert!(food(&state, sim.time()).contains(&marker));

    // A bot connection gets the physical listing, including this reserved key.
    sim.connect(0, PlayerAppearance::default()).unwrap();
    sim.physical_food(0);
    {
        let mut snake = sim.backend.world.snakes()[0].clone();
        snake.head = marker.point();
        sim.backend
            .world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    let mut bot = ClientState::new();
    bot.new_connection(sim.time()).unwrap();
    sim.backend.ingest(0, sim.time()).unwrap();
    sim.backend.broadcast().unwrap();
    pump(&mut sim, &mut bot);
    let physical = food(&bot, sim.time());
    assert_eq!(physical.len(), 1);
    assert_eq!(physical[0].point(), real.point());
    assert_eq!(physical[0].size, Some(2.));

    // Restore browser presentation, then simulate a pickup at the exact marker
    // point. No removal/animation may erase the decoration, even on an equal
    // cached listing or when the physical pellet has been replaced in-place.
    let mut state = browser(&mut sim);
    {
        let mut snake = sim.backend.world.snakes()[0].clone();
        snake.head = marker.point();
        sim.backend
            .world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    sim.backend.broadcast().unwrap();
    pump(&mut sim, &mut state);
    for remove in [false, true] {
        if remove {
            sim.backend.world.replace_food(Vec::new()).unwrap();
        }
        sim.backend
            .food_eaters
            .insert(food::food_identity(real.point()), 0);
        sim.backend.broadcast().unwrap();
        let packets = pump(&mut sim, &mut state);
        assert!(
            !packets
                .iter()
                .any(|p| matches!(p[0], b'c' | b'<') && p[1..5] == key)
        );
        assert!(food(&state, sim.time()).contains(&marker));
    }
}

#[test]
fn pit_markers_preserve_physics_rng_and_native_bot_frames() {
    for seed in [1, 7, 42] {
        let mut config = config();
        config.world.food_count = 120;
        config.world.ambient_replenish_per_second = 30.;
        config.agents = 3;
        config.record_wire = true;
        let mut marked = SimulatorBackend::new(seed, config.clone()).unwrap();
        let mut plain = SimulatorBackend::new(seed, config).unwrap();
        plain.pit_markers = None;
        for tick in 0..180 {
            if tick == 90 {
                marked.reconnect_slot(0).unwrap();
                plain.reconnect_slot(0).unwrap();
            }
            marked.next_frame().unwrap();
            plain.next_frame().unwrap();
            // Checkpoints include physics RNG, prey RNG, admission
            // clocks, pit state and rolling scores. Events are checked separately.
            assert_eq!(
                serde_json::to_vec(&marked.world().checkpoint().unwrap()).unwrap(),
                serde_json::to_vec(&plain.world().checkpoint().unwrap()).unwrap()
            );
            assert_eq!(marked.world.pickup_events(), plain.world.pickup_events());
            assert_eq!(
                marked.world.food_spawn_events(),
                plain.world.food_spawn_events()
            );
            assert_eq!(
                format!("{:?}", marked.spawn_random),
                format!("{:?}", plain.spawn_random)
            );
            assert_eq!(
                format!("{:?}", marked.random),
                format!("{:?}", plain.random)
            );
            for slot in 0..3 {
                assert_eq!(
                    marked.frame_for(slot).unwrap(),
                    plain.frame_for(slot).unwrap()
                );
                assert_eq!(
                    marked.drain_records(slot).unwrap(),
                    plain.drain_records(slot).unwrap()
                );
            }
        }
    }
}

#[test]
fn pit_markers_socket_delivery_preserves_world_and_background_bots() {
    let mut config = config();
    config.agents = 2;
    config.world.food_count = 120;
    config.world.ambient_replenish_per_second = 30.;
    config.latency = crate::client::LatencyConfig::measured_route();
    let mut marked = RawSimulator::new(7, config.clone(), 1).unwrap();
    let mut plain = RawSimulator::new(7, config, 1).unwrap();
    plain.backend.pit_markers = None;
    for sim in [&mut marked, &mut plain] {
        sim.connect(0, PlayerAppearance::default()).unwrap();
    }
    for tick in 0..1500 {
        if tick == 750 {
            for sim in [&mut marked, &mut plain] {
                sim.connect(0, PlayerAppearance::default()).unwrap();
            }
        }
        for sim in [&mut marked, &mut plain] {
            sim.advance_ticks(1).unwrap();
            sim.take(0).unwrap();
        }
        assert_eq!(
            serde_json::to_vec(&marked.backend.world().checkpoint().unwrap()).unwrap(),
            serde_json::to_vec(&plain.backend.world().checkpoint().unwrap()).unwrap(),
            "tick {tick}"
        );
        assert_eq!(
            marked.backend.frame_for(1).unwrap(),
            plain.backend.frame_for(1).unwrap(),
            "tick {tick}"
        );
        assert_eq!(
            format!("{:?}", marked.backend.random),
            format!("{:?}", plain.backend.random)
        );
        assert_eq!(
            format!("{:?}", marked.backend.spawn_random),
            format!("{:?}", plain.backend.spawn_random)
        );
    }
}

#[test]
fn stalled_circular_queues_grow_without_weakening_strict_admission() {
    let mut config = config();
    config.agents = 80;
    let mut sim = RawSimulator::new(7, config, 80).unwrap();
    let before = serde_json::to_value(sim.backend.world.checkpoint().unwrap()).unwrap();
    assert!(!sim.grow_for_waiters(40, 4., false));
    assert_eq!(
        serde_json::to_value(sim.backend.world.checkpoint().unwrap()).unwrap(),
        before
    );
    sim.grow_for_waiters(40, 1.999, true);
    assert_eq!(sim.backend.world.radius(), 4928.);
    sim.grow_for_waiters(40, 2., true);
    assert_eq!(sim.backend.world.radius(), 7392.);
    sim.grow_for_waiters(40, 4., true);
    assert_eq!(sim.backend.world.radius(), 9856.);
    assert_eq!(
        sim.backend.world.pit_radius(),
        Some(1784. * (9856. / 3584.))
    );
    assert_eq!(sim.backend.world.occupied(), 0);
    sim.grow_for_waiters(0, 100., true);
    assert_eq!(sim.backend.world.radius(), 9856.); // only the tick controller shrinks.
    assert!(sim.connect(1, PlayerAppearance::default()).unwrap());
    assert!(sim.connect(0, PlayerAppearance::default()).unwrap());
    let newborn = sim.backend.world.snakes()[0].clone();
    sim.backend.world.retire(0).unwrap();
    assert!(sim.backend.world.body_is_clear(&newborn.body, 400.));
    assert!(sim.backend.world.body_clears_head_paths(&newborn.body));
}

#[test]
fn pit_markers_move_on_the_socket_to_the_live_pit_edge() {
    let mut config = config();
    config.world.radius_updates = vec![crate::RadiusUpdate {
        time: 0.014,
        radius: 7168.,
    }];
    let mut sim = RawSimulator::new(7, config, 1).unwrap();
    let mut state = browser(&mut sim);
    let old = food(&state, sim.time());
    assert!(!old.is_empty());
    assert!(
        old.iter()
            .all(|f| (f.point().distance(Point::default()) - 1787.).abs() < 1.)
    );
    sim.advance_ticks(1).unwrap();
    let packets = pump(&mut sim, &mut state);
    let current = food(&state, sim.time());
    assert!(!current.is_empty());
    assert!(
        current
            .iter()
            .all(|f| (f.point().distance(Point::default()) - 3571.).abs() < 1.)
    );
    assert!(packets.iter().any(|p| matches!(p[0], b'c' | b'<')));
    assert!(sim.backend.world.food().is_empty());
}

#[test]
fn shrinking_pit_keeps_decorated_handles_until_a_replacement_can_fade_in() {
    use std::{f64::consts::FRAC_PI_2, sync::Arc};
    let mut config = config();
    config.agents = 80;
    config.world.radius_updates = vec![crate::RadiusUpdate {
        time: 0.,
        radius: 7168.,
    }];
    let mut sim = RawSimulator::new(7, config, 80).unwrap();
    assert!(sim.connect(0, PlayerAppearance::default()).unwrap());
    let near_pit = |sim: &mut RawSimulator| {
        let head = Point {
            x: sim.backend.world.pit_radius().unwrap() + 400.,
            y: 0.,
        };
        let (body, heading) = sim.backend.world.spawn_body(head, FRAC_PI_2).unwrap();
        sim.backend
            .world
            .correct_pose(0, head, body, heading)
            .unwrap();
        sim.backend.slots[0].control.heading = heading;
    };
    near_pit(&mut sim);
    let mut state = ClientState::new();
    state.new_connection(0.).unwrap();
    sim.advance_ticks(1).unwrap();
    pump(&mut sim, &mut state);
    let visible = food(&state, sim.time());
    assert!(!visible.is_empty());
    let key = encode::food_key(visible[0].point()).unwrap();
    let sector = usize::from(u16::from_be_bytes([key[0], key[1]]));
    let held = sim.backend.decorated_food_sectors[sector]
        .as_ref()
        .unwrap()
        .clone();
    let before = sim.backend.world.pit_radius().unwrap();
    sim.advance_ticks(1).unwrap();
    let packets = pump(&mut sim, &mut state);
    assert!(sim.backend.world.pit_radius().unwrap() < before);
    assert!(Arc::ptr_eq(
        &held,
        sim.backend.decorated_food_sectors[sector].as_ref().unwrap()
    ));
    assert!(
        !packets
            .iter()
            .any(|p| matches!(p[0], b'f' | b'F' | b'c' | b'<'))
    );

    // A replacement waits at least 500 ms; the previous generation stays while
    // the rapid F additions mature.
    let mut changes = Vec::new();
    for _ in 0..90 {
        sim.advance_ticks(1).unwrap();
        changes.extend(pump(&mut sim, &mut state));
        if !Arc::ptr_eq(
            &held,
            sim.backend.decorated_food_sectors[sector].as_ref().unwrap(),
        ) {
            break;
        }
    }
    assert!(!Arc::ptr_eq(
        &held,
        sim.backend.decorated_food_sectors[sector].as_ref().unwrap()
    ));
    assert!(
        changes
            .iter()
            .any(|p| matches!(p[0], b'F' | b'f' | b'c' | b'<'))
    );
    let cell = f64::from(encode::SECTOR_SIZE) / 256.;
    let live = sim.backend.world.pit_radius().unwrap();
    assert!(food(&state, sim.time()).iter().all(|f| {
        let offset = f.point().distance(Point::default()) - live;
        offset > 0. && offset < 3. + cell + 24.
    }));

    // Growth cannot wait for a bucket: a previously safe decoration could be
    // left behind the live edge. Move the viewport so it sees the new ring.
    assert!(sim.grow_for_waiters(79, 0., true));
    near_pit(&mut sim);
    sim.advance_ticks(1).unwrap();
    pump(&mut sim, &mut state);
    assert_eq!(
        sim.backend.pit_markers.as_ref().unwrap().radius(),
        sim.backend.world.pit_radius().unwrap()
    );
    let live = sim.backend.world.pit_radius().unwrap();
    let visible = food(&state, sim.time());
    assert!(!visible.is_empty());
    assert!(
        visible
            .iter()
            .all(|f| (f.point().distance(Point::default()) - live - 3.).abs() < 1.)
    );
    assert!(sim.backend.world.food().is_empty());
}

#[test]
fn queue_space_allowances_do_not_change_generic_circular_worlds() {
    let mut config = config();
    config.agents = 80;
    config.world.min_score = None;
    config.world.radius = 5000.;
    config.world.size_to_population = false;
    let mut sim = RawSimulator::new(7, config, 80).unwrap();
    sim.grow_for_waiters(80, 100., true);
    assert_eq!(sim.backend.world.radius(), 5000.);
    assert_eq!(sim.backend.world.pit_radius(), Some(1784.));
}

#[test]
fn queue_growth_before_the_first_tick_restores_exactly() {
    for sample in [None, Some(0.), Some(0.007)] {
        let mut config = config();
        config.agents = 80;
        if let Some(time) = sample {
            config.world.radius_updates = vec![crate::RadiusUpdate {
                time,
                radius: 4000.,
            }];
        }
        let mut sim = RawSimulator::new(7, config, 80).unwrap();
        if sample == Some(0.007) {
            sim.backend.world.step(&vec![None; 80]).unwrap();
        }
        sim.grow_for_waiters(80, 4., true);
        assert_eq!(
            sim.backend.world.time(),
            if sample == Some(0.007) { 0.007 } else { 0. }
        );
        assert_eq!(sim.backend.world.radius(), 23296.);
        let saved = serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap();
        let mut restored = Simulator::from_checkpoint(&saved, Default::default()).unwrap();
        assert_eq!(
            serde_json::to_vec(&restored.checkpoint().unwrap()).unwrap(),
            saved
        );
        for _ in 0..10 {
            sim.backend.world.step(&vec![None; 80]).unwrap();
            restored.step(&vec![None; 80]).unwrap();
        }
        assert_eq!(
            serde_json::to_value(sim.backend.world.checkpoint().unwrap()).unwrap(),
            serde_json::to_value(restored.checkpoint().unwrap()).unwrap()
        );
    }
}

#[test]
fn admission_checkpoints_reject_space_beyond_the_available_seats() {
    for seats in [1, 80] {
        let mut config = config();
        config.agents = seats;
        let sim = RawSimulator::new(7, config, seats).unwrap();
        let mut saved = serde_json::to_value(sim.backend.world.checkpoint().unwrap()).unwrap();
        let maximum = if seats == 1 { 3584_f64 } else { 23296_f64 };
        saved["state"]["radius"] = serde_json::json!(maximum.next_up());
        assert!(
            Simulator::from_checkpoint(&serde_json::to_vec(&saved).unwrap(), Default::default())
                .is_err()
        );
    }
}

#[test]
fn pit_minimap_defaults_and_explicit_overrides() {
    for (json, side) in [
        (r#"{}"#, 136),
        (r#"{"world":{"preset":"battledome"}}"#, 256),
        (
            r#"{"world":{"preset":"battledome"},"minimapSide":136}"#,
            136,
        ),
        (r#"{"minimapSide":256}"#, 256),
        (
            r#"{"world":{"preset":"battledome"},"minimapSide":null}"#,
            256,
        ),
    ] {
        let config: SimulationConfig = serde_json::from_str(json).unwrap();
        config.validate().unwrap();
        assert_eq!(config.minimap_side(), side);
        let roundtrip: SimulationConfig =
            serde_json::from_value(serde_json::to_value(config).unwrap()).unwrap();
        assert_eq!(roundtrip.minimap_side(), side);
    }
}

#[test]
fn pit_map_masks_outer_snakes_keeps_inner_cells_and_live_outline() {
    let mut sim = RawSimulator::new(7, config(), 1).unwrap();
    let world = &mut sim.backend.world;
    let map = |world: &Simulator| crate::client::Minimap::from_world(world, 256).unwrap();
    // Only an outer snake: its entire centerline is hidden.
    world
        .correct_pose(
            0,
            Point { x: 2700., y: 0. },
            vec![Point { x: 2600., y: 0. }; 26],
            0.,
        )
        .unwrap();
    let outer = map(world);
    world.retire(0).unwrap();
    let outline = map(world);
    assert_eq!(outer, outline);
    world.respawn(0, Point::default(), 0., 26).unwrap();
    let inner = map(world);
    assert!(inner.cells.iter().zip(&outline.cells).any(|(&a, &b)| a > b));
    // A centerline crossing the edge cannot paint cells wholly outside it.
    world
        .correct_pose(0, Point::default(), vec![Point { x: 2800., y: 0. }; 26], 0.)
        .unwrap();
    let crossing = map(world);
    for y in 0..256 {
        for x in 0..256 {
            let near = |v: usize| (v as f64 - 128.).max(0.).max(128. - (v + 1) as f64);
            if near(x).hypot(near(y)) > world.pit_radius().unwrap() / world.radius() * 128. {
                assert_eq!(crossing.cells[y * 256 + x], 0);
            }
        }
    }
}

#[test]
fn rolling_kill_board_retains_dead_disconnected_seats_until_expiry() {
    use super::broadcast::Board;
    let mut config = config();
    config.agents = 4;
    config.world.kill_score_seconds = Some(0.5);
    let mut sim = RawSimulator::new(7, config, 4).unwrap();
    let mut reader = browser(&mut sim);
    sim.connect(
        1,
        PlayerAppearance {
            nickname: b"Scorer".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    sim.connect(2, PlayerAppearance::default()).unwrap();
    // Synthetic retained kill timestamps exercise the same checkpoint-backed
    // rolling-score owner as collision credits, without depending on bot play.
    let mut saved = serde_json::to_value(sim.backend.world.checkpoint().unwrap()).unwrap();
    saved["state"]["slots"][0]["killTimes"] = serde_json::json!([0.]);
    saved["state"]["slots"][1]["killTimes"] = serde_json::json!([0., 0.]);
    sim.backend
        .world
        .restore(&serde_json::to_vec(&saved).unwrap(), Default::default())
        .unwrap();
    // A new life for the same socket connection retains its history.
    sim.connect(
        1,
        PlayerAppearance {
            nickname: b"Scorer".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(sim.backend.world.kill_score(1), Some(2));
    sim.disconnect(1).unwrap();
    sim.backend.world.retire(2).unwrap();
    let board = Board::new(&sim.backend.world, &sim.backend.profiles).unwrap();
    assert_eq!(board.order, [(2., 1), (1., 0)]);
    sim.backend.next_leaderboard = 0;
    sim.advance_ticks(1).unwrap();
    pump(&mut sim, &mut reader);
    let frame = reader.frame(sim.time()).unwrap();
    let board = frame.hud.leaderboard.unwrap();
    assert_eq!(board.rank, 2);
    assert_eq!(
        board
            .entries
            .iter()
            .map(|e| e.score.unwrap())
            .collect::<Vec<_>>(),
        [2., 1.]
    );
    assert_eq!(frame.observation.unwrap().me.score, 400.);
    // A stranger reusing the seat starts at zero; the departed scorer vanishes.
    sim.connect_new_player(
        1,
        PlayerAppearance {
            nickname: b"New".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(sim.backend.world.kill_score(1), Some(0));
    let board = Board::new(&sim.backend.world, &sim.backend.profiles).unwrap();
    assert_eq!(board.order, [(1., 0), (0., 1)]);
    sim.backend.next_leaderboard = 0;
    sim.advance_ticks(1).unwrap();
    pump(&mut sim, &mut reader);
    let board = reader.frame(sim.time()).unwrap().hud.leaderboard.unwrap();
    assert!(board.entries.iter().all(|entry| entry.name != "Scorer"));
    assert_eq!(
        board
            .entries
            .iter()
            .find(|entry| entry.name == "New")
            .unwrap()
            .score,
        Some(0.)
    );
    sim.disconnect(1).unwrap();
    sim.advance_ticks(72).unwrap();
    let board = Board::new(&sim.backend.world, &sim.backend.profiles).unwrap();
    assert_eq!(board.order, [(0., 0)]);
}

#[test]
fn new_player_clears_only_its_slot_and_native_bot_lives_keep_kills() {
    let mut world = Config::sandbox();
    world.radius = 6000.;
    world.food_count = 0;
    world.ambient_replenish_per_second = 0.;
    world.kill_score_seconds = Some(60.);
    let config = SimulationConfig {
        world,
        agents: 3,
        ..Default::default()
    };
    let mut sim = RawSimulator::new(7, config, 2).unwrap();
    sim.connect(0, PlayerAppearance::default()).unwrap();
    sim.connect(1, PlayerAppearance::default()).unwrap();
    let mut saved = serde_json::to_value(sim.backend.world.checkpoint().unwrap()).unwrap();
    for slot in 0..3 {
        saved["state"]["slots"][slot]["killTimes"] = serde_json::json!(vec![0.; slot + 1]);
    }
    sim.backend
        .world
        .restore(&serde_json::to_vec(&saved).unwrap(), Default::default())
        .unwrap();
    sim.disconnect(1).unwrap();
    assert!(
        sim.connect_new_player(1, PlayerAppearance::default())
            .unwrap()
    );
    sim.backend.reconnect_slot(2).unwrap();
    assert_eq!(
        (0..3)
            .map(|slot| sim.backend.world.kill_score(slot).unwrap())
            .collect::<Vec<_>>(),
        [1, 0, 3]
    );
}

/// Stock food lifetime model: pinned client newFood starts fr=0;
/// F uses rsp=3, f uses rsp=1, and fr += rsp * elapsed_ms / (8 * 150).
/// This checks emitted bytes and fade lifetimes, not browser pixels.
#[derive(Default)]
struct MarkerAges(std::collections::BTreeMap<[u8; 4], (f64, f64)>);
impl MarkerAges {
    fn apply(&mut self, packets: &[Vec<u8>], now: f64) {
        for p in packets {
            match p[0] {
                b'F' => {
                    for row in p[3..].as_chunks::<4>().0 {
                        let key = [p[1], p[2], row[1], row[2]];
                        assert!(
                            self.0.insert(key, (now, 3.)).is_none(),
                            "duplicate stock food ID"
                        );
                    }
                }
                b'f' | b'b' => {
                    let key = p[1..5].try_into().unwrap();
                    assert!(
                        self.0
                            .insert(key, (now, if p[0] == b'b' { 3. } else { 1. }))
                            .is_none()
                    );
                }
                b'c' | b'<' => {
                    self.0.remove::<[u8]>(&p[1..5]);
                }
                b'w' => self.0.retain(|key, _| key[..2] != p[1..3]),
                _ => {}
            }
        }
    }
    fn mature(&self, now: f64) -> usize {
        self.0
            .values()
            .filter(|&&(born, rsp)| (now - born) * rsp >= 1.2)
            .count()
    }
}

#[test]
fn stock_pit_markers_survive_growth_shrink_and_settle_with_time_to_fade() {
    let mut results = Vec::new();
    for wall in [8960., 17900.] {
        for joined in [0., 1., 3.5] {
            let mut config = config();
            config.agents = 65;
            config.world.radius_updates = vec![
                crate::RadiusUpdate {
                    time: 0.7,
                    radius: wall,
                },
                // A recorded settle sample isolates delivery after radius change
                // without running minutes of redundant physics in this regression.
                crate::RadiusUpdate {
                    time: 3.,
                    radius: 3584.,
                },
            ];
            let mut sim = RawSimulator::new(7, config, 65).unwrap();
            while sim.time() < joined {
                sim.advance_ticks(1).unwrap();
            }
            assert!(sim.connect(0, PlayerAppearance::default()).unwrap());
            assert!(sim.backend.slots[0].connection.decorations);
            let mut state = ClientState::new();
            state.new_connection(sim.time()).unwrap();
            let mut ages = MarkerAges::default();
            let mut shrink_min = usize::MAX;
            let mut settled_min = usize::MAX;
            for _ in 0..715 {
                let head = Point {
                    x: if sim.time() < 3. && sim.time() + sim.backend.world.config().dt >= 3. {
                        1784.
                    } else {
                        sim.backend.world.pit_radius().unwrap()
                    } + 100.,
                    y: 0.,
                };
                // Synthetic viewport follows the actual ring; no food is injected.
                let (body, heading) = sim
                    .backend
                    .world
                    .spawn_body(head, std::f64::consts::FRAC_PI_2)
                    .unwrap();
                sim.backend
                    .world
                    .correct_pose(0, head, body, heading)
                    .unwrap();
                sim.backend.slots[0].control.heading = heading;
                sim.advance_ticks(1).unwrap();
                let now = sim.time();
                ages.apply(&pump(&mut sim, &mut state), now);
                let received = food(&state, now);
                assert_eq!(received.len(), ages.0.len());
                // Allow stock's initial fade after join and the growth jump.
                if now > (joined + 0.6).max(1.3) && now < 2.95 {
                    shrink_min = shrink_min.min(ages.mature(now));
                }
                if now > (joined + 0.6).max(3.6) {
                    settled_min = settled_min.min(ages.mature(now));
                }
            }
            println!(
                "stock-marker wall={wall} join={joined} shrinking_mature_min={shrink_min} settled_mature_min={settled_min} final_food={}",
                ages.0.len()
            );
            results.push((joined, shrink_min, settled_min));
        }
    }
    for (joined, shrinking, settled) in results {
        if joined < 3. {
            assert!(shrinking >= 20, "no mature ring during shrink: {shrinking}");
        }
        assert!(settled >= 20, "no mature ring after settling: {settled}");
    }
}
