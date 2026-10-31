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
            food_count: 600,
            ambient_replenish_per_second: 8.,
            ..Config::sandbox()
        },
        ..Default::default()
    };
    let mut sim = RawSimulator::new(7, config, 4).unwrap();
    for slot in 0..4 {
        assert!(sim.connect(slot, PlayerAppearance::default()).unwrap());
    }
    sim.protocol13(1);
    sim.full_minimap(1);
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
    // Optional complete JSONL artifact; the same test is overlaid onto the
    // pinned base for byte comparison, without changing either runtime.
    let mut trace = std::env::var_os("SLITHER_PRESENTATION_TRACE")
        .map(|path| std::io::BufWriter::new(std::fs::File::create(path).unwrap()));
    let mut frames = [0; 4];
    let mut deaths = 0;
    let mut respawns = 0;
    for tick in 0..512 {
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
                .respawn(0, Point { x: 4000., y: -2500. }, 0., 16)
                .unwrap();
            respawns += 1;
        }
        if tick == 192 {
            let snake = &sim.backend.world.snakes()[3];
            let mut body = snake.body.clone();
            body[128].y -= 2000.;
            sim.backend
                .world
                .correct_pose(3, snake.head, body, snake.heading)
                .unwrap();
            // Exercise invalidated bounds before the next physics step too.
            sim.backend.broadcast().unwrap();
        }
        let commands: Vec<_> = (0..4)
            .map(|slot| {
                let heading = if slot == 0 && (96..112).contains(&tick)
                    || slot == 0 && (288..304).contains(&tick)
                {
                    0
                } else {
                    ((tick / 24 + slot * 17) % 251) as u8
                };
                sim.receive(slot, &[heading]).unwrap();
                if tick % 64 == 0 {
                    sim.receive(slot, &[251]).unwrap();
                }
                heading
            })
            .collect();
        let was_alive = sim.backend.world.snakes()[0].alive;
        sim.advance_ticks(1).unwrap();
        if was_alive && !sim.backend.world.snakes()[0].alive {
            deaths += 1;
        }
        let queued: Vec<_> = (0..4)
            .map(|slot| {
                sim.backend.slots[slot]
                    .connection
                    .downlink
                    .iter()
                    .map(|(due, bytes)| (due.to_bits(), bytes.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();
        let delivered: Vec<_> = (0..4)
            .map(|slot| {
                let output = sim.drain(slot).unwrap();
                frames[slot] += output.len();
                output
            })
            .collect();
        if let Some(trace) = &mut trace {
            serde_json::to_writer(
                &mut *trace,
                &serde_json::json!({
                    "tick": tick, "commands": commands,
                    "world": sim.backend.world.checkpoint().unwrap(),
                    "events": sim.backend.world.events(),
                    "queued": queued, "delivered": delivered,
                }),
            )
            .unwrap();
            writeln!(trace).unwrap();
        }
    }
    assert!(deaths >= 2);
    assert_eq!(respawns, 2);
    assert!(frames.into_iter().all(|count| count > 30));
}
