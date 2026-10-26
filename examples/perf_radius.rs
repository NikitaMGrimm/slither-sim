//! Untimed ordered full-state radius comparison; overlay this same source on both runtimes.
#[path = "support/runtime.rs"]
mod runtime;
#[cfg(not(slither_private_state))]
use runtime::LegacyState;
use serde_json::{Value, json};
use slither_sim::{Action, Config, RadiusUpdate, transport::Session};
use std::io::{self, Write};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for (name, dynamic_radius, samples) in [
        ("dynamic", true, false),
        ("replay", false, true),
        ("combined", true, true),
    ] {
        let config = Config {
            radius: 3000.,
            dynamic_radius,
            food_count: 30,
            ambient_replenish_per_second: 7.,
            prey: slither_sim::PreyConfig {
                count: 2,
                ..Default::default()
            },
            radius_updates: if samples {
                vec![
                    RadiusUpdate {
                        time: 0.,
                        radius: 2800.,
                    },
                    RadiusUpdate {
                        time: 0.007,
                        radius: 2750.,
                    },
                    RadiusUpdate {
                        time: 0.7,
                        radius: 4000.,
                    },
                    RadiusUpdate {
                        time: 1.4,
                        radius: 2500.,
                    },
                    RadiusUpdate {
                        time: 2.8,
                        radius: 3400.,
                    },
                    RadiusUpdate {
                        time: 3.5,
                        radius: 2700.,
                    },
                ]
            } else {
                vec![]
            },
            ..Config::sandbox()
        };
        let mut world = runtime::ring(7, config.clone(), 4)?;
        runtime::record_events(&mut world);
        let initial_config = serde_json::to_value(&config)?;
        let mut session = Session::new();
        let reset = session.request(&serde_json::to_vec(
            &json!({"op":"reset", "seed":7, "agents":4, "decisionTicks":1, "config":config}),
        )?);
        let result: Value = serde_json::from_slice(&reset)?;
        assert!(result.get("error").is_none(), "{result}");
        for tick in 0..=512 {
            #[cfg(slither_runtime_radius)]
            assert_eq!(serde_json::to_value(world.config())?, initial_config);
            let checkpoint = serde_json::to_value(world.checkpoint()?)?;
            let snapshot: Value =
                serde_json::from_slice(&session.request(br#"{"op":"snapshot"}"#))?;
            let row = json!({"kind":"state", "case":name, "tick":tick, "initialConfig":initial_config,
                "checkpoint":checkpoint, "observations":world.observe_all(),
                "pickupEvents":format!("{:?}", world.pickup_events()), "foodSpawnEvents":format!("{:?}", world.food_spawn_events()),
                "snapshot":snapshot, "frame":runtime::frame_debug(session.frame(0).unwrap(), &runtime::rules(&config)),
                "spawnDistance":runtime::spawn_distance(&world, 0.375)});
            serde_json::to_writer(&mut out, &row)?;
            writeln!(out)?;
            if tick == 512 {
                break;
            }
            let headings: Vec<_> = world.snakes().iter().map(|s| s.heading).collect();
            let boosts: Vec<_> = (0..4).map(|slot| (slot + tick) % 20 < 5).collect();
            let actions: Vec<_> = headings
                .iter()
                .zip(&boosts)
                .map(|(&heading, &boost)| Some(Action { heading, boost }))
                .collect();
            world.step(&actions)?;
            session.step(&headings, &boosts)?;
        }
    }
    serde_json::to_writer(&mut out, &json!({"kind":"end", "cases":3, "ticks":512}))?;
    writeln!(out)?;
    Ok(())
}
