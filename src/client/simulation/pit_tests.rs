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
