//! Hidden benchmark-harness seams, with no stability or semver promise.
use super::super::{RawSimulator, SimulationConfig};
use crate::client::latency::Random;
use crate::{Action, Point, Simulator};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Diagnostic output only, never a restorable host checkpoint. Scheduled times
/// retain their exact f64 bits; vector order retains even identical frames.
#[doc(hidden)]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RawReplay {
    pub frames: Vec<(usize, u64, u64, Vec<u8>)>,
    pub terminal: Value,
}

impl RawReplay {
    pub fn compare(&self, other: &Self) -> Result<(), String> {
        for (index, (a, b)) in self.frames.iter().zip(&other.frames).enumerate() {
            if a != b {
                return Err(format!(
                    "Raw frame {index} differs (slot {}, tick {})",
                    a.0, a.1
                ));
            }
        }
        if self.frames.len() != other.frames.len() {
            return Err("Raw frame count differs".into());
        }
        // Value's numeric equality treats signed zeros alike; canonical JSON
        // bytes preserve their stored representation as well as ordered arrays.
        if serde_json::to_vec(&self.terminal).map_err(|e| e.to_string())?
            != serde_json::to_vec(&other.terminal).map_err(|e| e.to_string())?
        {
            return Err("Raw terminal host/view state differs".into());
        }
        Ok(())
    }
}

/// Name allocation identities by first encounter, never by process address.
/// Keeping these relationships detects a stale revision even when values match.
#[derive(Default)]
pub(in crate::client) struct Identities(BTreeMap<usize, usize>);

impl Identities {
    pub(in crate::client) fn of<T>(&mut self, value: &std::sync::Arc<T>) -> usize {
        let next = self.0.len();
        *self
            .0
            .entry(std::sync::Arc::as_ptr(value) as usize)
            .or_insert(next)
    }
}

#[doc(hidden)]
pub struct RawWorld(RawSimulator);

impl RawWorld {
    /// Drain every queued output, including future deliveries, at each replay
    /// boundary (initial setup, tick, or scripted operation). Call before slot
    /// reuse can discard a session's queue. This never decodes or sorts frames.
    pub fn record(&mut self, replay: &mut RawReplay) -> Result<(), String> {
        let tick = self.0.backend.tick;
        replay.frames.extend(
            self.drain()?
                .into_iter()
                .map(|(slot, at, bytes)| (slot, tick, at.to_bits(), bytes)),
        );
        Ok(())
    }

    /// Complete persistent raw-host/view diagnostics. No producer metadata or
    /// restore API: compare only with the same harness on a pinned runtime.
    pub fn finish(&mut self, replay: &mut RawReplay) -> Result<(), String> {
        self.record(replay)?;
        let b = &self.0.backend;
        let mut ids = Identities::default();
        let revision = ids.of(&b.food_revision);
        let listings = |sectors: &[Option<std::sync::Arc<super::super::food::SectorFood>>],
                        ids: &mut Identities| {
            sectors
                .iter()
                .map(|s| s.as_ref().map(|s| (ids.of(s), format!("{s:?}"))))
                .collect::<Vec<_>>()
        };
        let food = listings(&b.food_sectors, &mut ids);
        let decorated = listings(&b.decorated_food_sectors, &mut ids);
        let slots: Vec<_> = b
            .slots
            .iter()
            .map(|s| {
                // Exhaustive destructuring makes new persistent slot fields require
                // an explicit oracle decision. Raw slots never run client decoders.
                let super::super::connection::Slot {
                    state,
                    connection,
                    life,
                    outbox,
                    processing_override,
                    previous_boost,
                    control,
                    pinged_at,
                    records,
                    record_bytes,
                    remote,
                    living,
                } = s;
                json!({"client":format!("{state:?}"), "connection":format!("{connection:?}"),
                "life":life, "outbox":outbox, "processingOverride":processing_override,
                "previousBoost":previous_boost,"control":control,"pingedAt":pinged_at,
                "records":records,"recordBytes":record_bytes,"remote":remote,"living":living,
                "interestIdentity": ids.of(connection.food_interest.revision()),
                "minimapIdentity":connection.minimap_sent.as_ref().map(|m| ids.of(m)),
                "foodIdentities":connection.food.oracle_identities(&mut ids),
                "decoratedIdentities":connection.decorated_food.oracle_identities(&mut ids)})
            })
            .collect();
        replay.terminal = json!({
            "world": b.world.checkpoint().map_err(|e| e.to_string())?,
            "deaths":b.world.deaths(),"pickups":format!("{:?}",b.world.pickup_events()),"births":format!("{:?}",b.world.food_spawn_events()),
            "config":b.config,"playerLatencySlots":b.player_latency.slots,
            "playerLatencyConfig":b.player_latency.config,
            "slots":slots,"random":format!("{:?}",b.random),"spawnRandom":format!("{:?}",b.spawn_random),
            "presentation":b.food_presentation.oracle_state(),
            "foodEaters":b.food_eaters.iter().collect::<Vec<_>>(),"preyEaters":b.prey_eaters,"preyWire":b.prey_wire,
            "food":food,"foodRevision":revision,"decoratedFood":decorated,
            "pitMarkers":format!("{:?}",b.pit_markers),"foodIndex":b.food_index.oracle_state(),
            "foodEpoch":b.food_epoch,"nextMinimap":b.next_minimap,"nextLeaderboard":b.next_leaderboard,
            "preyClocks":format!("{:?}",b.prey_clocks),"reports":format!("{:?}",b.reports),
            "subscriptions":b.food_subscriptions.oracle_state(&mut ids),
            "coverageGrid":b.coverage_grid.as_ref().map(|g| g.oracle_state()),
            "coverageBounds":format!("{:?}", b.coverage_bounds),
            "pitProjection":format!("{:?}", b.pit_projection),
            "legacyMaps":b.legacy_maps.oracle_state(&mut ids),
            "backgroundPolicy":format!("{:?}", b.background_policy),
            "tick":b.tick,"variableDt":b.variable_dt,"wallOffsetBits":b.wall_offset.to_bits(),
            "remote":b.remote,"profiles":format!("{:?}",b.profiles),"record":format!("{:?}",b.record),
            "connections":b.connections,"sessionKey":b.session_key
        });
        Ok(())
    }

    pub fn new(seed: u32, config: SimulationConfig, world: Simulator) -> Result<Self, String> {
        if config.agents != world.snakes().len() || world.time() != 0. {
            return Err("Raw fixture needs matching slots and a time-zero world".into());
        }
        let count = config.agents;
        let mut raw = RawSimulator::new(seed, config, count)?;
        raw.backend.world = world;
        raw.backend
            .food_presentation
            .sync(&raw.backend.world.food_in_birth_order())?;
        for slot in 0..count {
            if raw.backend.world.snakes()[slot].alive {
                raw.backend.slots[slot].control = Action {
                    heading: raw.backend.world.snakes()[slot].heading,
                    boost: false,
                };
                raw.backend.connect(slot)?;
                raw.backend.slots[slot].connection.decorations = true;
                raw.backend.ingest(slot, 0.)?;
            }
        }
        raw.backend.broadcast_with(0, &mut |_| Ok(()), true)?;
        Ok(Self(raw))
    }

    pub fn world(&self) -> &Simulator {
        &self.0.backend.world
    }

    pub fn controls(&mut self, actions: &[Option<Action>]) -> Result<(), String> {
        if actions.len() != self.0.backend.slots.len()
            || actions.iter().flatten().any(|a| !a.heading.is_finite())
        {
            return Err("Invalid raw fixture controls".into());
        }
        for (slot, action) in self.0.backend.slots.iter_mut().zip(actions) {
            if let Some(action) = action {
                slot.control = *action;
            }
        }
        Ok(())
    }

    pub fn tick(&mut self) -> Result<(), String> {
        self.0.advance_tick_with(0, &mut |_| Ok(()))
    }

    pub fn search(&mut self, seed: u64) -> Result<(Vec<Point>, f64), String> {
        self.0.backend.spawn_random = Random::new(seed);
        self.0.backend.spawn_pose_waited(usize::MAX, 0.)
    }

    pub fn composition(&self) -> (usize, usize, usize) {
        let mut scratch = Vec::new();
        let (mut recipients, mut known, mut food) = (0, 0, 0);
        for slot in &self.0.backend.slots {
            if slot.connection.setup {
                recipients += 1;
                slot.connection.snakes.candidates(&[], &mut scratch);
                known += scratch.len();
                food += slot.connection.food.perf_len();
            }
        }
        (recipients, known, food)
    }

    pub fn drain(&mut self) -> Result<Vec<(usize, f64, Vec<u8>)>, String> {
        let mut records = Vec::new();
        for slot in 0..self.0.backend.slots.len() {
            records.extend(
                self.0
                    .take(slot)?
                    .into_iter()
                    .map(|(at, bytes)| (slot, at, bytes)),
            );
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{DeliveryProfile, LatencyConfig, simulation::PlayerAppearance};
    use crate::{Config, Food, InitialPopulation, Placement, WorldOptions};
    use std::f64::consts::TAU;

    // Synthetic grown bodies, then a complete tick-zero warmup. This is a
    // host regression scene, not a fitted model or an official capture.
    fn replay() -> RawReplay {
        let config = SimulationConfig {
            agents: 8,
            world: Config::official(8),
            delivery: DeliveryProfile::Official,
            latency: LatencyConfig::measured_route(),
            minimap_interval_seconds: 0.1,
            leaderboard_interval_seconds: 0.1,
            ..Default::default()
        };
        let mut world = Simulator::new(WorldOptions {
            seed: 7,
            config: config.world.clone(),
            population: InitialPopulation::Placed {
                ids: (0..8).map(|i| i.to_string()).collect(),
                placement: Placement::TestRing,
            },
        })
        .unwrap();
        for slot in 0..8 {
            let head = Point {
                x: (slot % 4) as f64 * 900. - 1350.,
                y: (slot / 4) as f64 * 1200. - 600.,
            };
            let body = (0..96)
                .map(|i| {
                    let a = i as f64 * TAU / 48.;
                    Point {
                        x: head.x - 300. + 300. * a.cos(),
                        y: head.y + 300. * a.sin(),
                    }
                })
                .collect::<Vec<_>>();
            world.correct_mass(slot, body.clone(), 0.25).unwrap();
            world.correct_pose(slot, head, body, 0.).unwrap();
        }
        world.record_events(crate::EventRecording {
            pickups: true,
            food_births: true,
        });
        let mut sim = RawWorld::new(7, config, world).unwrap();
        // Start mixed adapters before the first latency-delayed connect emits
        // setup; use both full-map and ordinary map legacy recipients.
        sim.0.protocol13(1);
        sim.0.protocol13(3);
        sim.0.full_minimap(1);
        let mut replay = RawReplay::default();
        sim.record(&mut replay).unwrap();
        let mut deaths = 0;
        let mut pickups = 0;
        let mut maintenance = false;
        for tick in 0..240 {
            if tick == 144 {
                assert_eq!(sim.world().living(), 8, "warmup lost grown recipients");
                assert!(sim.world().snakes().iter().all(|s| s.body.len() >= 90));
                let b = &mut sim.0.backend;
                let head = b.world.snakes()[0].head;
                b.world
                    .insert_food(Food {
                        x: head.x,
                        y: head.y,
                        value: 1.,
                        size: None,
                        origin: None,
                    })
                    .unwrap();
                // Register the edited pellet before its pickup, exactly as a
                // diagnostic standalone broadcast does.
                b.broadcast().unwrap();
                sim.record(&mut replay).unwrap();
            }
            if tick == 160 {
                let b = &mut sim.0.backend;
                let head = Point {
                    x: b.world.radius() - 1.,
                    y: 0.,
                };
                let body = (0..96)
                    .map(|i| Point {
                        x: head.x - i as f64 * 42.,
                        y: 0.,
                    })
                    .collect();
                b.world.correct_pose(2, head, body, 0.).unwrap();
                b.slots[2].control.heading = 0.;
            }
            if tick == 190 {
                assert!(!sim.world().snakes()[2].alive);
                sim.0.disconnect(2).unwrap();
                assert!(
                    sim.0
                        .connect(
                            2,
                            PlayerAppearance {
                                nickname: b"Reused".to_vec(),
                                skin: 9,
                                custom_skin: vec![]
                            }
                        )
                        .unwrap()
                );
                sim.0.protocol13(2);
                sim.0.request_session_id(2).unwrap();
                sim.record(&mut replay).unwrap();
            }
            if tick % 7 == 0 {
                for slot in 0..8 {
                    sim.0.receive(slot, &[251]).unwrap();
                }
            }
            let epoch = sim.0.backend.food_epoch;
            sim.tick().unwrap();
            maintenance |= sim.0.backend.food_epoch > epoch;
            deaths += sim.world().deaths().len();
            pickups += sim.world().pickup_events().unwrap().len();
            sim.record(&mut replay).unwrap();
        }
        assert!(deaths > 0 && pickups > 0 && maintenance);
        assert_eq!(sim.0.backend.slots[2].life, 1);
        assert!(sim.0.backend.slots.iter().all(|s| s.connection.setup));
        sim.finish(&mut replay).unwrap();
        assert!((0..8).all(|slot| replay.frames.iter().any(|f| f.0 == slot)));
        for (slot, versions) in [
            (0, vec![19]),
            (1, vec![13]),
            (2, vec![19, 13]),
            (3, vec![13]),
        ] {
            let setups: Vec<_> = replay
                .frames
                .iter()
                .filter(|f| f.0 == slot && f.3.get(2) == Some(&b'a'))
                .map(|f| f.3[25])
                .collect();
            assert_eq!(setups, versions, "protocol setup/reuse for slot {slot}");
        }
        replay
    }

    #[test]
    fn raw_host_replay_detects_bytes_order_and_terminal_state() {
        let expected = replay();
        expected.compare(&replay()).unwrap();
        let encoded = serde_json::to_vec(&expected).unwrap();
        let decoded: RawReplay = serde_json::from_slice(&encoded).unwrap();
        expected.compare(&decoded).unwrap();
        // Optional export allows the identical oracle harness to run on a
        // pinned old runtime, without checking in a large generated recording.
        if let Ok(path) = std::env::var("SLITHER_RAW_REPLAY_PATH") {
            std::fs::write(path, encoded).unwrap();
        }
        let mut changed = expected.clone();
        changed.frames[0].3[0] ^= 1;
        assert!(
            expected
                .compare(&changed)
                .unwrap_err()
                .starts_with("Raw frame 0 differs")
        );
        let mut changed = expected.clone();
        let second = changed
            .frames
            .iter()
            .position(|f| f.0 == changed.frames[0].0 && f != &changed.frames[0])
            .unwrap();
        changed.frames.swap(0, second);
        assert!(expected.compare(&changed).is_err());
        let mut changed = expected.clone();
        changed.frames.pop();
        assert_eq!(
            expected.compare(&changed),
            Err("Raw frame count differs".into())
        );
        let mut changed = expected.clone();
        changed.terminal["random"] = json!("perturbed");
        assert_eq!(
            expected.compare(&changed),
            Err("Raw terminal host/view state differs".into())
        );
        let mut negative_zero = RawReplay::default();
        let mut positive_zero = RawReplay::default();
        negative_zero.terminal = json!(-0.0);
        positive_zero.terminal = json!(0.0);
        assert!(negative_zero.compare(&positive_zero).is_err());
    }
}
