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

    /// Prepare recipient visibility on `threads` view workers (1 is serial),
    /// in tasks of `chunk` recipients for ranges of at least `min_range`;
    /// `None` keeps the host defaults. Frames must not change.
    #[cfg(feature = "parallel")]
    pub fn set_view_threads(
        &mut self,
        threads: usize,
        chunk: Option<usize>,
        min_range: Option<usize>,
    ) -> Result<(), String> {
        self.0.backend.views = match threads {
            0 => return Err("view threads must be at least 1".into()),
            1 => None,
            threads => Some(super::ViewWorkers::with_split(
                crate::client::server::HostPool::new(threads, "slither-view")?,
                chunk,
                min_range,
            )),
        };
        Ok(())
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

    /// View workers for a replay as `(threads, chunk)`, splitting even the
    /// smallest recipient range; `None` is the serial path.
    type Views = Option<(usize, usize)>;

    fn with_views(sim: &mut RawWorld, views: Views) {
        #[cfg(feature = "parallel")]
        if let Some((threads, chunk)) = views {
            sim.set_view_threads(threads, Some(chunk), Some(0)).unwrap();
        }
        #[cfg(not(feature = "parallel"))]
        {
            let _ = sim;
            assert!(views.is_none());
        }
    }

    /// One tick handing frames of slots `..early` over mid-broadcast, as the
    /// host's socket prefix does; handed frames enter the replay in order.
    fn tick_split(sim: &mut RawWorld, early: usize, replay: &mut RawReplay) {
        let tick = sim.0.backend.tick + 1;
        sim.0
            .advance_tick_with(early, &mut |frames| {
                for (slot, frames) in frames {
                    replay.frames.extend(
                        frames
                            .into_iter()
                            .map(|(at, bytes)| (slot, tick, at.to_bits(), bytes)),
                    );
                }
                Ok(())
            })
            .unwrap();
    }

    /// A split configuration must have prepared at least one wave.
    fn assert_views_ran(sim: &RawWorld, views: Views) {
        #[cfg(feature = "parallel")]
        if views.is_some_and(|(threads, _)| threads > 1) {
            assert!(sim.0.backend.views.as_ref().unwrap().retained() > 0);
        }
        #[cfg(not(feature = "parallel"))]
        let _ = (sim, views);
    }

    fn replay() -> RawReplay {
        replay_with(None, 0)
    }

    // Synthetic grown bodies, then a complete tick-zero warmup. This is a
    // host regression scene, not a fitted model or an official capture.
    fn replay_with(views: Views, early: usize) -> RawReplay {
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
        with_views(&mut sim, views);
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
            if tick % 2 == 1 {
                tick_split(&mut sim, early, &mut replay);
            } else {
                sim.tick().unwrap();
            }
            maintenance |= sim.0.backend.food_epoch > epoch;
            deaths += sim.world().deaths().len();
            pickups += sim.world().pickup_events().unwrap().len();
            sim.record(&mut replay).unwrap();
        }
        assert!(deaths > 0 && pickups > 0 && maintenance);
        assert_eq!(sim.0.backend.slots[2].life, 1);
        assert!(sim.0.backend.slots.iter().all(|s| s.connection.setup));
        assert_views_ran(&sim, views);
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

    /// Long bodies crossing near the origin, recipients posed on coverage-cell
    /// corners, exactly at vision from bodies and prey, one ULP either side,
    /// and on signed zeros. Poses repeat mid-run before standalone broadcasts,
    /// so the edges reach split queries, not only the serial setup broadcast.
    #[cfg(feature = "parallel")]
    fn edges(views: Views) -> RawReplay {
        const N: usize = 16;
        const LONG: usize = 4;
        let mut world_config = Config::official(N);
        world_config.radius = 21_000.;
        world_config.dynamic_radius = false;
        world_config.size_to_population = false;
        let vision = world_config.vision;
        // CoverageGrid cells are max(vision + 1, 256) wide.
        let cell = vision + 1.;
        let config = SimulationConfig {
            agents: N,
            world: world_config,
            delivery: DeliveryProfile::Official,
            ..Default::default()
        };
        let mut world = Simulator::new(WorldOptions {
            seed: 11,
            config: config.world.clone(),
            population: InitialPopulation::Placed {
                ids: (0..N).map(|i| i.to_string()).collect(),
                placement: Placement::TestRing,
            },
        })
        .unwrap();
        // Axis bodies sit 600 units off the axes, so signed-zero heads are
        // within vision but never on a body.
        let long = [
            (Point { x: 6400., y: 600. }, 0.),
            (Point { x: 600., y: 6400. }, TAU / 4.),
            (Point { x: 4525., y: 4525. }, TAU / 8.),
            (
                Point {
                    x: 4525.,
                    y: -4525.,
                },
                -TAU / 8.,
            ),
        ];
        let heads = [
            Point { x: cell, y: cell },
            Point {
                x: cell.next_down(),
                y: -cell,
            },
            Point {
                x: -cell,
                y: 600. + vision,
            },
            Point {
                x: 2. * cell,
                y: (600. + vision).next_up(),
            },
            Point {
                x: -0.,
                y: -2. * cell,
            },
            Point {
                x: -2. * cell,
                y: -0.,
            },
            Point {
                x: 3. * cell,
                y: (-cell).next_up(),
            },
            Point {
                x: (-3. * cell).next_up(),
                y: 2. * cell,
            },
            Point {
                x: 600. - vision,
                y: 6400.,
            },
            Point {
                x: 5. * cell,
                y: 600. + vision,
            },
            Point {
                x: -4. * cell,
                y: -4. * cell,
            },
            Point {
                x: 0.5,
                y: 2. * cell,
            },
        ];
        let straight = |head: Point, heading: f64, points: usize| {
            (0..points)
                .map(|i| Point {
                    x: head.x - i as f64 * 40. * heading.cos(),
                    y: head.y - i as f64 * 40. * heading.sin(),
                })
                .collect::<Vec<_>>()
        };
        let pose = |world: &mut Simulator, slot: usize, fresh: bool| {
            let (head, heading, points) = if slot < LONG {
                (long[slot].0, long[slot].1, 320)
            } else {
                (heads[slot - LONG], 0., 24)
            };
            let body = straight(head, heading, points);
            if fresh {
                world.correct_mass(slot, body.clone(), 0.25).unwrap();
            }
            world.correct_pose(slot, head, body, heading).unwrap();
        };
        for slot in 0..N {
            pose(&mut world, slot, true);
        }
        world.record_events(crate::EventRecording {
            pickups: true,
            food_births: true,
        });
        let mut sim = RawWorld::new(11, config, world).unwrap();
        with_views(&mut sim, views);
        let mut replay = RawReplay::default();
        sim.record(&mut replay).unwrap();
        let controls: Vec<_> = (0..N)
            .map(|slot| {
                Some(Action {
                    heading: if slot < LONG { long[slot].1 } else { 0. },
                    boost: false,
                })
            })
            .collect();
        for tick in 0..120 {
            if tick % 40 == 0 {
                let b = &mut sim.0.backend;
                for slot in LONG..N {
                    let snake = &b.world.snakes()[slot];
                    if snake.alive && snake.body.len() == 24 {
                        pose(&mut b.world, slot, false);
                    }
                }
                // Prey exactly at vision, one ULP beyond, and on the corner.
                let at = heads[0];
                let positions = [
                    Point {
                        x: at.x + vision,
                        y: at.y,
                    },
                    Point {
                        x: at.x,
                        y: (at.y - vision).next_down(),
                    },
                    at,
                ];
                for (k, position) in positions.into_iter().enumerate() {
                    let id = 900_000 + k as u64;
                    let _ = b.world.remove_prey(id);
                    b.world
                        .insert_prey(crate::ReplayPrey {
                            prey: crate::Prey {
                                id,
                                position,
                                heading: 0.,
                                target_heading: 0.,
                                speed: 0.,
                                size: 10.,
                            },
                            mode: crate::prey::PreyMode::Roaming,
                        })
                        .unwrap();
                }
                b.broadcast().unwrap();
                sim.record(&mut replay).unwrap();
            }
            sim.controls(&controls).unwrap();
            if tick % 3 == 1 {
                tick_split(&mut sim, 5, &mut replay);
            } else {
                sim.tick().unwrap();
            }
            sim.record(&mut replay).unwrap();
        }
        assert!(sim.world().snakes()[..LONG].iter().any(|s| s.alive));
        assert_views_ran(&sim, views);
        sim.finish(&mut replay).unwrap();
        replay
    }

    /// Shift a living snake's whole pose a little, so its viewers have news.
    fn nudge(world: &mut Simulator, slot: usize) {
        let snake = &world.snakes()[slot];
        if !snake.alive {
            return;
        }
        let shift = |p: &Point| Point {
            x: p.x + 30.,
            y: p.y - 20.,
        };
        let head = shift(&snake.head);
        let body = snake.body.iter().map(shift).collect();
        let heading = snake.heading;
        world.correct_pose(slot, head, body, heading).unwrap();
    }

    /// Pose `victim`'s head on the middle of `into`'s body, so it dies in
    /// the next step.
    fn crash(world: &mut Simulator, victim: usize, into: usize) {
        let snakes = world.snakes();
        if !snakes[victim].alive || !snakes[into].alive {
            return;
        }
        let target = &snakes[into].body;
        let head = target[target.len() / 2];
        let body = (0..snakes[victim].body.len())
            .map(|i| Point {
                x: head.x + i as f64 * 40.,
                y: head.y,
            })
            .collect();
        world.correct_pose(victim, head, body, TAU / 2.).unwrap();
    }

    /// Synthetic Battledome scene: steered and boosting recipients through
    /// the decorated pit, mixed 13/19 adapters, deaths and slot reuse, and
    /// injected failures each followed by more ticks. With `adaptive`, dt
    /// alternates so report gates run on seconds. Injected errors, coverage
    /// counts and the terminal state all enter the replay. A regression
    /// scene, not a fitted model or an official capture.
    fn faults(views: Views, adaptive: bool) -> RawReplay {
        const N: usize = 12;
        const EARLY: usize = 4;
        let config = SimulationConfig {
            agents: N,
            world: Config::battledome(),
            delivery: DeliveryProfile::Official,
            latency: LatencyConfig::measured_route(),
            minimap_interval_seconds: 0.1,
            leaderboard_interval_seconds: 0.1,
            ..Default::default()
        };
        let mut world = Simulator::new(WorldOptions {
            seed: 5,
            config: config.world.clone(),
            population: InitialPopulation::Placed {
                ids: (0..N).map(|i| i.to_string()).collect(),
                placement: Placement::TestRing,
            },
        })
        .unwrap();
        world.record_events(crate::EventRecording {
            pickups: true,
            food_births: true,
        });
        for k in 0..3u64 {
            let a = k as f64 * TAU / 3.;
            world
                .insert_prey(crate::ReplayPrey {
                    prey: crate::Prey {
                        id: 800_000 + k,
                        position: Point {
                            x: 1500. * a.cos(),
                            y: 1500. * a.sin(),
                        },
                        heading: a,
                        target_heading: a + 1.,
                        speed: 2.,
                        size: 10.,
                    },
                    mode: crate::prey::PreyMode::Roaming,
                })
                .unwrap();
        }
        let mut sim = RawWorld::new(5, config, world).unwrap();
        with_views(&mut sim, views);
        sim.0.protocol13(2);
        sim.0.protocol13(5);
        sim.0.full_minimap(5);
        sim.0.physical_food(9);
        let mut replay = RawReplay::default();
        let mut errors = Vec::new();
        let (mut deaths, mut pickups, mut rejoins, mut decorated) = (0, 0, 0, 0);
        let mut maintenance = false;
        let mut living = Vec::new();
        // A closed session has no queue to drain; close it as a host would.
        let record = |sim: &mut RawWorld, replay: &mut RawReplay, errors: &mut Vec<String>| {
            for slot in 0..N {
                if let Some(fault) = sim.0.backend.slots[slot].connection.output_fault.clone() {
                    errors.push(fault);
                    sim.0.disconnect(slot).unwrap();
                }
            }
            sim.record(replay).unwrap();
        };
        record(&mut sim, &mut replay, &mut errors);
        for tick in 0..200u64 {
            let actions: Vec<_> = sim
                .world()
                .snakes()
                .iter()
                .enumerate()
                .map(|(slot, snake)| {
                    let inward = (-snake.head.y).atan2(-snake.head.x);
                    // The first half circles the spawn ring; the rest dive
                    // through the pit, boosting, and meet there.
                    Some(if slot < N / 2 {
                        Action {
                            heading: inward - TAU / 4. + 0.05,
                            boost: false,
                        }
                    } else {
                        Action {
                            heading: inward + 0.9 * (tick as f64 * 0.07 + slot as f64).sin(),
                            boost: (tick / 25 + slot as u64).is_multiple_of(2),
                        }
                    })
                })
                .collect();
            sim.controls(&actions).unwrap();
            if adaptive && tick % 20 == 10 {
                let dt = if tick % 40 == 10 { 0.006 } else { 0.007 };
                sim.0.set_dt(dt).unwrap();
            }
            if tick % 30 == 15 {
                for slot in 0..N {
                    if !sim.world().snakes()[slot].alive {
                        sim.0.disconnect(slot).unwrap();
                        let appearance = PlayerAppearance {
                            nickname: format!("Again {slot}").into_bytes(),
                            skin: slot as u8,
                            custom_skin: vec![],
                        };
                        match sim.0.connect(slot, appearance) {
                            Ok(_) => rejoins += 1,
                            Err(error) => errors.push(error),
                        }
                        if slot % 4 == 2 {
                            sim.0.protocol13(slot);
                        }
                        if slot % 3 == 0 {
                            sim.0.request_session_id(slot).unwrap();
                        }
                    }
                }
                record(&mut sim, &mut replay, &mut errors);
            }
            if tick.is_multiple_of(9) {
                for slot in 0..N {
                    // A closed or just-reused session may refuse the ping.
                    let _ = sim.0.receive(slot, &[251]);
                }
            }
            if tick.is_multiple_of(20) {
                living.push(sim.world().living());
            }
            if tick == 20 || tick == 120 {
                let victim = N - 1 - (tick / 100) as usize;
                crash(&mut sim.0.backend.world, victim, N / 2);
            }
            if tick == 40 {
                // Queue pressure on a local recipient mid-range, after a pose
                // edit gives every recipient something to send.
                let b = &mut sim.0.backend;
                let full = (EARLY + 2..N)
                    .find(|&slot| b.slots[slot].connection.setup)
                    .unwrap_or(N - 1);
                nudge(&mut b.world, 3);
                let held = b.slots[full].connection.downlink.len();
                b.slots[full].remote = false;
                b.slots[full].connection.pongs = 1;
                b.slots[full].connection.downlink.resize(
                    crate::client::simulation::connection::MAX_IN_FLIGHT,
                    (f64::MAX, vec![0]),
                );
                if let Err(error) = b.broadcast() {
                    errors.push(error);
                }
                b.slots[full].connection.downlink.truncate(held);
                b.slots[full].remote = true;
                record(&mut sim, &mut replay, &mut errors);
            }
            if tick == 30 {
                // Pellets just ahead of each head, registered by a standalone
                // broadcast as a diagnostic edit is, for pickups next step.
                let b = &mut sim.0.backend;
                let ahead: Vec<_> = b
                    .world
                    .snakes()
                    .iter()
                    .filter(|s| s.alive)
                    .map(|s| Point {
                        x: s.head.x + 12. * s.heading.cos(),
                        y: s.head.y + 12. * s.heading.sin(),
                    })
                    .collect();
                for at in ahead {
                    b.world
                        .insert_food(Food {
                            x: at.x,
                            y: at.y,
                            value: 1.,
                            size: None,
                            origin: None,
                        })
                        .unwrap();
                }
                if let Err(error) = b.broadcast() {
                    errors.push(error);
                }
                record(&mut sim, &mut replay, &mut errors);
            }
            if tick == 80 {
                // Every snake's motion is encoded before recipients, but a
                // prey only in recipient assembly: pose one recipient just
                // inside the wire's edge with a prey just beyond it, so it
                // fails after its own snake and a requested session ID. The
                // pose and prey are undone before resuming.
                let near = (EARLY..N)
                    .find(|&slot| {
                        sim.world().snakes()[slot].alive
                            && sim.0.backend.slots[slot].connection.setup
                    })
                    .unwrap_or(N - 1);
                for slot in 0..N {
                    if sim.0.backend.slots[slot].connection.setup {
                        sim.0.request_session_id(slot).unwrap();
                    }
                }
                let b = &mut sim.0.backend;
                let snake = &b.world.snakes()[near];
                let saved = (snake.head, snake.body.clone(), snake.heading);
                let edge = 10. - f64::from(crate::client::encode::GRID_CENTER);
                let body = (0..saved.1.len())
                    .map(|i| Point {
                        x: edge + i as f64 * 40.,
                        y: 0.,
                    })
                    .collect();
                b.world
                    .correct_pose(near, Point { x: edge, y: 0. }, body, TAU / 2.)
                    .unwrap();
                b.world
                    .insert_prey(crate::ReplayPrey {
                        prey: crate::Prey {
                            id: 900_000,
                            position: Point {
                                x: edge - 30.,
                                y: 0.,
                            },
                            heading: 0.,
                            target_heading: 0.,
                            speed: 2.,
                            size: 10.,
                        },
                        mode: crate::prey::PreyMode::Roaming,
                    })
                    .unwrap();
                if let Err(error) = b.broadcast() {
                    errors.push(error);
                }
                b.world.remove_prey(900_000).unwrap();
                b.world
                    .correct_pose(near, saved.0, saved.1, saved.2)
                    .unwrap();
                record(&mut sim, &mut replay, &mut errors);
            }
            if tick == 100 {
                // A fresh legacy adapter has no anchors, so this recipient's
                // next moves fail conversion and close only its session.
                sim.0.protocol13(5);
            }
            let epoch = sim.0.backend.food_epoch;
            let result = if tick == 60 {
                // Queued pongs give the socket prefix frames to hand over.
                for slot in &mut sim.0.backend.slots[..EARLY] {
                    slot.connection.pongs += 1;
                }
                sim.0
                    .advance_tick_with(EARLY, &mut |_| Err("Early handoff failed".into()))
            } else if tick % 2 == 1 {
                let tick = sim.0.backend.tick + 1;
                sim.0.advance_tick_with(EARLY, &mut |frames| {
                    for (slot, frames) in frames {
                        replay.frames.extend(
                            frames
                                .into_iter()
                                .map(|(at, bytes)| (slot, tick, at.to_bits(), bytes)),
                        );
                    }
                    Ok(())
                })
            } else {
                sim.tick()
            };
            if let Err(error) = result {
                errors.push(error);
            }
            maintenance |= sim.0.backend.food_epoch > epoch;
            deaths += sim.world().deaths().len();
            pickups += sim.world().pickup_events().unwrap().len();
            decorated = decorated.max(
                sim.0
                    .backend
                    .slots
                    .iter()
                    .filter(|s| s.connection.decorations)
                    .map(|s| s.connection.decorated_food.perf_len())
                    .sum::<usize>(),
            );
            record(&mut sim, &mut replay, &mut errors);
        }
        assert_views_ran(&sim, views);
        sim.finish(&mut replay).unwrap();
        replay.terminal["scene"] = json!({
            "errors": errors, "deaths": deaths, "pickups": pickups, "rejoins": rejoins,
            "decorated": decorated, "maintenance": maintenance, "living": living,
        });
        replay
    }

    /// The fault scene reaches every injected failure and the coverage it
    /// claims, so a parallel match is not a match of two empty runs.
    fn assert_fault_coverage(replay: &RawReplay, adaptive: bool) {
        let scene = &replay.terminal["scene"];
        let errors: Vec<&str> = scene["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e.as_str().unwrap())
            .collect();
        for expected in [
            "Downlink queue is full",
            "Early handoff failed",
            "Coordinate is outside the protocol-19 wire range",
            "Protocol-13 output failed: Protocol-13 anchor before appearance",
        ] {
            assert!(errors.contains(&expected), "{expected} missing: {errors:?}");
        }
        for count in ["deaths", "pickups", "rejoins", "decorated"] {
            assert!(scene[count].as_u64().unwrap() > 0, "no {count}");
        }
        assert_eq!(scene["maintenance"], json!(true));
        assert_eq!(replay.terminal["variableDt"], json!(adaptive));
    }

    #[test]
    fn the_fault_scene_reaches_its_failures_and_coverage() {
        for adaptive in [false, true] {
            let replay = faults(None, adaptive);
            assert_fault_coverage(&replay, adaptive);
            replay.compare(&faults(None, adaptive)).unwrap();
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_views_match_serial_through_failures_and_resume() {
        for adaptive in [false, true] {
            let serial = faults(None, adaptive);
            for (threads, chunk) in [(1, 64), (2, 1), (3, 2), (4, 3), (7, 64), (8, 5)] {
                serial
                    .compare(&faults(Some((threads, chunk)), adaptive))
                    .unwrap_or_else(|e| {
                        panic!("adaptive {adaptive}, {threads} threads, chunk {chunk}: {e}")
                    });
            }
        }
    }

    /// A pinned motion report gate: slot 1's heading changes and is reported
    /// at once, returns, and changes back to the reported value just before
    /// the gate reopens. Slot 4 first sees slot 1 between those changes, so
    /// only its stale-report observation makes the reopened gate report.
    /// Synthetic poses on a fixed open world, not a capture.
    fn rearm(views: Views) -> RawReplay {
        const N: usize = 6;
        const X: usize = 1;
        const R: usize = 4;
        const E: u64 = 20;
        let mut world_config = Config::official(N);
        world_config.radius = 21_000.;
        world_config.dynamic_radius = false;
        world_config.size_to_population = false;
        let config = SimulationConfig {
            agents: N,
            world: world_config,
            delivery: DeliveryProfile::Official,
            ..Default::default()
        };
        let mut world = Simulator::new(WorldOptions {
            seed: 13,
            config: config.world.clone(),
            population: InitialPopulation::Placed {
                ids: (0..N).map(|i| i.to_string()).collect(),
                placement: Placement::TestRing,
            },
        })
        .unwrap();
        let straight = |head: Point, heading: f64, points: usize| {
            (0..points)
                .map(|i| Point {
                    x: head.x - i as f64 * 40. * heading.cos(),
                    y: head.y - i as f64 * 40. * heading.sin(),
                })
                .collect::<Vec<_>>()
        };
        for slot in 0..N {
            let head = Point {
                x: (slot % 3) as f64 * 6000. - 6000.,
                y: (slot / 3) as f64 * 6000. - 3000.,
            };
            let body = straight(head, 0., 24);
            world.correct_mass(slot, body.clone(), 0.25).unwrap();
            world.correct_pose(slot, head, body, 0.).unwrap();
        }
        let mut sim = RawWorld::new(13, config, world).unwrap();
        with_views(&mut sim, views);
        let mut replay = RawReplay::default();
        sim.record(&mut replay).unwrap();
        let mut controls = vec![
            Some(Action {
                heading: 0.,
                boost: false,
            });
            N
        ];
        for tick in 0..E + 20 {
            // Reported at once (the gate is open), back for four ticks, then
            // the reported value again before the gate reopens at E + 6.
            let pinned = if tick == E || tick >= E + 5 { 0.5 } else { 0. };
            let b = &mut sim.0.backend;
            let snake = &b.world.snakes()[X];
            let (head, body) = (snake.head, snake.body.clone());
            b.world.correct_pose(X, head, body, pinned).unwrap();
            controls[X] = Some(Action {
                heading: pinned,
                boost: false,
            });
            if tick == E + 2 {
                // Bring slot 4 within vision of slot 1, beside its path.
                let head = Point {
                    x: head.x + 200.,
                    y: head.y + 800.,
                };
                let body = straight(head, 0., b.world.snakes()[R].body.len());
                b.world.correct_pose(R, head, body, 0.).unwrap();
            }
            sim.controls(&controls).unwrap();
            sim.tick().unwrap();
            sim.record(&mut replay).unwrap();
        }
        assert!(sim.world().snakes().iter().all(|s| s.alive));
        assert!(sim.0.backend.slots[R].connection.snakes.contains_key(X));
        assert_views_ran(&sim, views);
        sim.finish(&mut replay).unwrap();
        replay
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_views_match_serial_through_report_gate_rearming() {
        let serial = rearm(None);
        for (threads, chunk) in [(1, 64), (2, 1), (3, 2), (4, 64), (7, 1), (8, 3)] {
            serial
                .compare(&rearm(Some((threads, chunk))))
                .unwrap_or_else(|e| panic!("{threads} threads, chunk {chunk}: {e}"));
        }
    }

    /// Each deliberate coordinator fault must change a replay (or abort it):
    /// stale folds against the rearming scene, commit order and failed
    /// partial views against the fault scene.
    #[cfg(feature = "parallel")]
    #[test]
    fn the_view_oracle_detects_perturbed_commits() {
        use super::super::views::{PERTURB, Perturb};
        type Scene = fn(Views) -> RawReplay;
        let scenes: [(Perturb, Scene); 3] = [
            (Perturb::DropStale, rearm),
            (Perturb::SwapCommit, |views| faults(views, false)),
            (Perturb::DropFailedView, |views| faults(views, false)),
        ];
        for (perturb, scene) in scenes {
            let serial = scene(None);
            serial.compare(&scene(Some((4, 2)))).unwrap();
            PERTURB.set(perturb);
            let run = std::panic::catch_unwind(|| scene(Some((4, 2))));
            PERTURB.set(Perturb::None);
            assert!(
                run.map_or(true, |run| serial.compare(&run).is_err()),
                "{perturb:?} went undetected"
            );
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_views_match_the_serial_replay() {
        let serial = replay_with(None, 3);
        serial.compare(&replay_with(Some((1, 64)), 3)).unwrap();
        for (threads, chunk) in [(2, 1), (3, 2), (4, 3), (7, 64), (8, 5)] {
            let parallel = replay_with(Some((threads, chunk)), 3);
            serial
                .compare(&parallel)
                .unwrap_or_else(|e| panic!("{threads} threads, chunk {chunk}: {e}"));
        }
        // One undivided range as well as the socket-prefix/bot split.
        replay().compare(&replay_with(Some((4, 2)), 0)).unwrap();
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_views_match_serial_at_coverage_and_vision_edges() {
        let serial = edges(None);
        for (threads, chunk) in [(1, 64), (2, 1), (3, 4), (4, 64), (7, 2), (8, 3)] {
            serial
                .compare(&edges(Some((threads, chunk))))
                .unwrap_or_else(|e| panic!("{threads} threads, chunk {chunk}: {e}"));
        }
    }
}
