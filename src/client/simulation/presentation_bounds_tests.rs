//! Fixed input tape for cross-revision host frame and world comparisons.
use super::*;
use crate::Config;
use std::io::Write;

#[test]
fn presentation_bounds_fixed_tape_512_ticks() {
    let config = SimulationConfig {
        agents: 6,
        world: Config {
            radius: 32000.,
            pit: Some(crate::PitConfig { radius: 6000. }),
            food_count: 600,
            ambient_replenish_per_second: 8.,
            ..Config::sandbox()
        },
        delivery: DeliveryProfile::Official,
        minimap_interval_seconds: 0.07,
        leaderboard_interval_seconds: 0.091,
        local_records: true,
        latency: LatencyConfig::measured_route(),
        ..Default::default()
    };
    let mut sim = RawSimulator::new(7, config, 6).unwrap();
    for slot in 0..6 {
        assert!(sim.connect(slot, PlayerAppearance::default()).unwrap());
    }
    sim.protocol13(1);
    sim.full_minimap(1);
    sim.protocol13(3);
    sim.physical_food(2);
    sim.request_session_id(0).unwrap();
    for slot in 0..6 {
        sim.backend.world.retire(slot).unwrap();
        sim.backend
            .world
            .respawn(
                slot,
                Point {
                    x: 4000.,
                    y: slot as f64 * 1000. - 2500.,
                },
                0.,
                if slot == 3 { 256 } else { 16 },
            )
            .unwrap();
    }
    assert_eq!(sim.backend.world.snakes()[3].body.len(), 256);
    let board = broadcast::Board::new(&sim.backend.world, &sim.backend.profiles).unwrap();
    assert_eq!(
        board
            .order
            .iter()
            .map(|&(_, slot)| slot)
            .collect::<Vec<_>>(),
        [3, 0, 1, 2, 4, 5]
    );
    // Optional complete JSONL artifact; the same test is overlaid onto the
    // pinned base for byte comparison, without changing either runtime.
    let mut trace = std::env::var_os("SLITHER_PRESENTATION_TRACE")
        .map(|path| std::io::BufWriter::new(std::fs::File::create(path).unwrap()));
    let mut frames = [0; 6];
    let mut deaths = 0;
    let mut respawns = 0;
    let mut output_faults = 0;
    for tick in 0..512 {
        // Miss several fronts for just one viewer, then resume its old history.
        if tick == 160 || tick == 176 {
            sim.backend.slots[1].connection.setup = tick == 176;
        }
        if tick == 384 {
            sim.backend.slots[2].connection.downlink_bytes = connection::MAX_REMOTE_OUTPUT_BYTES;
            sim.backend.slots[2].connection.pongs += 1;
        }
        if tick == 392 {
            assert!(sim.connect(2, PlayerAppearance::default()).unwrap());
            sim.physical_food(2);
            respawns += 1;
        }
        if tick == 240 {
            // Rebase both streams and queued times as the host does on deferral.
            sim.set_wall_offset(0.125);
        }
        if tick == 120 {
            sim.victory(0, b"fixed tape").unwrap();
        }
        if tick == 96 || tick == 288 {
            let world = &mut sim.backend.world;
            world.retire(0).unwrap();
            world
                .respawn(0, Point { x: 31999., y: 0. }, 0., 16)
                .unwrap();
        }
        if tick == 112 || tick == 304 {
            assert!(!sim.backend.world.snakes()[0].alive);
            assert!(sim.connect(0, PlayerAppearance::default()).unwrap());
            sim.backend.world.retire(0).unwrap();
            sim.backend
                .world
                .respawn(
                    0,
                    Point {
                        x: 4000.,
                        y: -2500.,
                    },
                    0.,
                    16,
                )
                .unwrap();
            respawns += 1;
        }
        if tick == 192 {
            let snake = &sim.backend.world.snakes()[3];
            let mut body = snake.body.clone();
            body[128].y -= 20.;
            sim.backend
                .world
                .correct_pose(3, snake.head, body, snake.heading)
                .unwrap();
            // Exercise invalidated bounds before the next physics step too.
            sim.backend.broadcast().unwrap();
        }
        // Co-located lifetimes exercise hidden pickups and birth ordering.
        if tick == 48 {
            let head = sim.backend.world.snakes()[2].head;
            for size in [2., 3.] {
                sim.backend
                    .world
                    .insert_food(crate::Food {
                        x: head.x,
                        y: head.y,
                        value: 1.,
                        size: Some(size),
                        origin: None,
                    })
                    .unwrap();
            }
            sim.backend.broadcast().unwrap();
        }
        let commands: Vec<_> = (0..6)
            .map(|slot| {
                let heading =
                    if slot == 0 && ((96..112).contains(&tick) || (288..304).contains(&tick)) {
                        0
                    } else {
                        ((tick / 24 + slot * 17) % 251) as u8
                    };
                let mut messages = vec![vec![heading]];
                if tick % 64 == 0 {
                    messages.push(vec![251]);
                }
                if slot == 2 && tick % 32 == 0 {
                    messages.push(vec![if tick % 64 == 0 { 253 } else { 254 }]);
                }
                for bytes in &messages {
                    sim.receive(slot, bytes).unwrap();
                }
                (sim.time().to_bits(), messages)
            })
            .collect();
        let was_alive = sim.backend.world.snakes()[0].alive;
        sim.advance_ticks(1).unwrap();
        if tick == 384 {
            assert!(sim.backend.slots[2].connection.output_fault.is_some());
            assert!(sim.backend.slots[3].connection.setup);
            output_faults += 1;
        }
        if was_alive && !sim.backend.world.snakes()[0].alive {
            deaths += 1;
        }
        let queued: Vec<_> = (0..6)
            .map(|slot| {
                sim.backend.slots[slot]
                    .connection
                    .downlink
                    .iter()
                    .map(|(due, bytes)| (due.to_bits(), bytes.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        let delivered: Vec<_> = (0..6)
            .map(|slot| {
                let output = sim.drain(slot).unwrap();
                frames[slot] += output.len();
                output
            })
            .collect();
        if let Some(trace) = &mut trace {
            let pickups: Vec<_> = sim
                .backend
                .world
                .pickup_events()
                .unwrap()
                .iter()
                .map(|event| {
                    let item = match &event.item {
                        crate::PickupItem::Food(food) => serde_json::json!({"food": food}),
                        crate::PickupItem::Prey(prey) => serde_json::json!({"prey": prey}),
                    };
                    serde_json::json!({"eater": event.eater, "item": item})
                })
                .collect();
            let births: Vec<_> = sim
                .backend
                .world
                .food_spawn_events()
                .unwrap()
                .iter()
                .map(|event| serde_json::json!({"source": event.source, "food": event.food}))
                .collect();
            serde_json::to_writer(
                &mut *trace,
                &serde_json::json!({
                    "tick": tick, "commands": commands,
                    "world": sim.backend.world.checkpoint().unwrap(),
                    "pickups": pickups,
                    "foodBirths": births,
                    "deaths": sim.backend.world.deaths(),
                    "queued": queued, "delivered": delivered,
                    "random": format!("{:?}", (&sim.backend.random, &sim.backend.spawn_random)),
                    "connections": sim.backend.slots.iter().map(|s| format!("{:?}", s.connection)).collect::<Vec<_>>(),
                    "reports": format!("{:?}", sim.backend.reports_for_trace()),
                }),
            )
            .unwrap();
            writeln!(trace).unwrap();
        }
    }
    assert!(deaths >= 2);
    assert_eq!(respawns, 3);
    assert_eq!(output_faults, 1);
    assert!(frames.into_iter().all(|count| count > 30));
}

#[test]
fn partial_presentation_failure_keeps_the_serial_prefix() {
    let mut backend = SimulatorBackend::new(
        7,
        SimulationConfig {
            agents: 2,
            world: Config {
                radius: 6000.,
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::sandbox()
            },
            ..Default::default()
        },
    )
    .unwrap();
    backend.advance_ticks(4).unwrap();
    backend.slots[0].connection.pongs = 1;
    backend.slots[0]
        .connection
        .downlink
        .resize(connection::MAX_IN_FLIGHT, (f64::MAX, vec![0]));
    let later = format!("{:?}", backend.slots[1].connection);
    let error = backend.broadcast().unwrap_err();
    assert_eq!(error, "Downlink queue is full");
    assert_eq!(format!("{:?}", backend.slots[1].connection), later);
    let failed = serde_json::json!({
        "world": backend.world.checkpoint().unwrap(),
        "random": format!("{:?}", backend.random),
        "connections": backend.slots.iter().map(|s| format!("{:?}", s.connection)).collect::<Vec<_>>(),
        "reports": format!("{:?}", backend.reports_for_trace()),
    });
    backend.slots[0].connection.take_downlink();
    backend.broadcast().unwrap();
    if let Some(path) = std::env::var_os("SLITHER_PRESENTATION_FAILURE_TRACE") {
        std::fs::write(path, serde_json::to_vec(&serde_json::json!({
            "error": error, "failed": failed,
            "world": backend.world.checkpoint().unwrap(),
            "random": format!("{:?}", backend.random),
            "connections": backend.slots.iter().map(|s| format!("{:?}", s.connection)).collect::<Vec<_>>(),
            "reports": format!("{:?}", backend.reports_for_trace()),
        })).unwrap()).unwrap();
    }
}
