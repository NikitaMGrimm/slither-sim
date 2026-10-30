//! Included as a child of the runtime's private backend in the measurement
//! example only. No library API or runtime source is patched.
use super::super::*;
use crate::{catalog_worlds::EngineWorld, runtime};
#[cfg(not(slither_private_state))]
use runtime::LegacyState;
type Search = Result<(Vec<Point>, f64), String>;

pub(crate) struct Fixture {
    sim: RawSimulator,
    orbits: Vec<f64>,
    initial_wire: Vec<(usize, f64, Vec<u8>)>,
}

impl Fixture {
    pub(crate) fn world(&self) -> &Simulator {
        &self.sim.backend.world
    }
    pub(crate) fn food_updates(&self) -> u64 {
        self.sim.backend.food_epoch
    }
    pub(crate) fn initial_wire(&mut self) -> Vec<(usize, f64, Vec<u8>)> {
        std::mem::take(&mut self.initial_wire)
    }
    pub(crate) fn census(&self) -> serde_json::Value {
        let mut lengths: Vec<_> = self
            .world()
            .snakes()
            .iter()
            .filter(|s| s.alive)
            .map(|s| s.body.len())
            .collect();
        lengths.sort_unstable();
        serde_json::json!({"living":lengths.len(),"bodyPoints":lengths.iter().sum::<usize>(),
            "meanSegments":lengths.iter().sum::<usize>() as f64/lengths.len() as f64,
            "p90Segments":lengths[((lengths.len()-1) as f64*0.9).ceil() as usize],
            "p99Segments":lengths[((lengths.len()-1) as f64*0.99).ceil() as usize],
            "food":self.world().food().len(),"radius":self.world().radius()})
    }
    pub(crate) fn new(kind: EngineWorld) -> Result<Self, String> {
        let count = match kind {
            EngineWorld::MatureOfficial => 500,
            EngineWorld::SpawnOverload => 320,
        };
        let config = SimulationConfig {
            agents: count,
            world: kind.config(),
            delivery: DeliveryProfile::Official,
            ..Default::default()
        };
        let mut sim = RawSimulator::new(7, config, count)?;
        let mut world = runtime::ring(7, kind.config(), count)?;
        let mut snakes = world.snakes().to_vec();
        let mut orbits = vec![0.; count];
        match kind {
            EngineWorld::MatureOfficial => {
                // Bounded approximation of the study's mature length census.
                // Disjoint concentric lanes retain long, spatially extended
                // bodies without a costly growth simulation or initial crash.
                let mut radius = 3000.;
                let mut used = 0.;
                for (slot, snake) in snakes.iter_mut().enumerate() {
                    let segments = match slot {
                        0..250 => 16,
                        250..400 => 60,
                        400..475 => 176,
                        _ => 247,
                    };
                    let extent = (segments as f64 + 1.) * 42. + 600.;
                    if used + extent > TAU * radius {
                        radius += 180.;
                        used = 0.;
                    }
                    let phase = (used + extent - 300.) / radius;
                    used += extent;
                    orbits[slot] = radius;
                    snake.body = (0..segments)
                        .map(|i| {
                            let angle = phase - (i as f64) * 42. / radius;
                            Point {
                                x: radius * angle.cos(),
                                y: radius * angle.sin(),
                            }
                        })
                        .collect();
                    snake.head = snake.body[0];
                    snake.heading = phase + std::f64::consts::FRAC_PI_2;
                    snake.segments = segments;
                    snake.fam = 0.;
                    snake.alive = true;
                    snake.speed = crate::physics::base_speed(segments);
                }
            }
            EngineWorld::SpawnOverload => {
                let mut data = &include_bytes!("catalog_spawn.bin")[..];
                for snake in &mut snakes {
                    let mut next = || {
                        let (head, tail) = data.split_at(8);
                        data = tail;
                        f64::from_le_bytes(head.try_into().unwrap())
                    };
                    let alive = next();
                    if alive == 0. {
                        snake.alive = false;
                        continue;
                    }
                    snake.alive = true;
                    snake.head = Point {
                        x: next(),
                        y: next(),
                    };
                    snake.heading = next();
                    snake.fam = next();
                    snake.speed = next();
                    let count = next() as usize;
                    snake.body = (0..count)
                        .map(|_| Point {
                            x: next(),
                            y: next(),
                        })
                        .collect();
                    snake.segments = count;
                }
                assert!(data.is_empty());
            }
        }
        runtime::initial_snakes(&mut world, snakes)?;
        if matches!(kind, EngineWorld::SpawnOverload) {
            // Match the study's food population without importing snake food
            // whose birth appearance is unavailable in a physics checkpoint.
            let mut food_config = kind.config();
            food_config.food_count = 100_000;
            let source = runtime::ring(7, food_config, 0)?;
            let food = source
                .food()
                .get(..18346)
                .ok_or("bounded ambient construction admitted fewer than 18,346 pellets")?
                .to_vec();
            runtime::replace_food(&mut world, food)?;
        }
        runtime::record_events(&mut world);
        sim.backend.world = world;
        // Only ambient startup food exists: sync can assign its seeded colors.
        // All subsequent boost/death births go through the ordinary raw tick,
        // preserving presentation provenance rather than importing stale food.
        sim.backend
            .food_presentation
            .sync(sim.backend.world.food())?;
        for slot in 0..count {
            let b = &mut sim.backend;
            if !b.world.snakes()[slot].alive {
                continue;
            }
            b.slots[slot].control = Action {
                heading: b.world.snakes()[slot].heading,
                boost: matches!(kind, EngineWorld::SpawnOverload),
            };
            b.connect(slot)?;
            b.slots[slot].connection.decorations = true;
        }
        let mut fixture = Self {
            sim,
            orbits,
            initial_wire: Vec::new(),
        };
        fixture.control(kind);
        match kind {
            EngineWorld::MatureOfficial => fixture.sim.advance_tick_with(0, &mut |_| Ok(()))?,
            EngineWorld::SpawnOverload => {
                // Preserve the selected geometry for the first two probes.
                // Moving even one tick can close the study's seed-0 opening.
                let b = &mut fixture.sim.backend;
                for slot in 0..count {
                    b.ingest(slot, 0.)?;
                }
                b.broadcast_with(0, &mut |_| Ok(()), true)?;
            }
        }
        fixture.initial_wire = fixture.drain()?;
        Ok(fixture)
    }
    pub(crate) fn control(&mut self, kind: EngineWorld) {
        for (slot, snake) in self.sim.backend.world.snakes().iter().enumerate() {
            let heading = match kind {
                EngineWorld::MatureOfficial => {
                    let (x, y) = (snake.head.x, snake.head.y);
                    let radius = self.orbits[slot];
                    y.atan2(x)
                        + std::f64::consts::FRAC_PI_2
                        + ((x.hypot(y) - radius) / radius).clamp(-0.6, 0.6)
                }
                EngineWorld::SpawnOverload => snake.heading,
            };
            self.sim.backend.slots[slot].control = Action {
                heading,
                boost: matches!(kind, EngineWorld::SpawnOverload),
            };
        }
    }
    pub(crate) fn tick(&mut self, kind: EngineWorld) -> Result<Vec<Search>, String> {
        let mut searches = Vec::new();
        if matches!(kind, EngineWorld::SpawnOverload) {
            for seed in [40, 0] {
                self.sim.backend.spawn_random = Random::new(seed);
                let result = self.sim.backend.spawn_pose_waited(usize::MAX, 0.);
                if let Err(error) = &result
                    && error != crate::spawn::WAITING
                {
                    return Err(error.clone());
                }
                searches.push(result);
            }
        }
        self.sim.advance_tick_with(0, &mut |_| Ok(()))?;
        Ok(searches)
    }
    pub(crate) fn drain(&mut self) -> Result<Vec<(usize, f64, Vec<u8>)>, String> {
        let mut records = Vec::new();
        for slot in 0..self.sim.backend.slots.len() {
            records.extend(
                self.sim
                    .take(slot)?
                    .into_iter()
                    .map(|(at, bytes)| (slot, at, bytes)),
            );
        }
        Ok(records)
    }
}

/// One-time offline fixture generator, used only to select the compact body
/// geometry from the published study's deterministic received-frame warmup.
pub(crate) fn prepare_spawn(path: &str) -> Result<(), String> {
    use crate::client::{Bot, ClientStatus, ForagerBot};
    let mut backend = SimulatorBackend::new(
        7,
        SimulationConfig {
            agents: 320,
            world: crate::Config::battledome(),
            ..Default::default()
        },
    )?;
    let mut policy = ForagerBot::new();
    let mut cursor = 0;
    for decision in 0..=15301 {
        for offset in 0..320 {
            let slot = (cursor + offset) % 320;
            if matches!(
                backend.frame_for(slot)?.status,
                ClientStatus::Dead | ClientStatus::Disconnected
            ) {
                let _ = backend.reconnect_slot(slot);
                cursor = (slot + 1) % 320;
                break;
            }
        }
        let mut actions = Vec::new();
        for slot in 0..320 {
            let frame = backend.frame_for(slot)?;
            actions.push(policy.decide(&frame).map(|mut a| {
                a.boost = true;
                a
            }));
        }
        backend.step_all(&actions)?;
        if decision % 143 == 0 {
            eprintln!("warmup {decision} {}", backend.world.living());
        }
    }
    assert_eq!(backend.world.living(), 318, "study census changed");
    assert_eq!(
        backend
            .world
            .snakes()
            .iter()
            .filter(|s| s.alive)
            .map(|s| s.body.len())
            .sum::<usize>(),
        18726,
        "study geometry changed"
    );
    let mut bytes = Vec::new();
    for snake in backend.world.snakes() {
        let mut emit = |x: f64| bytes.extend_from_slice(&x.to_le_bytes());
        emit(f64::from(snake.alive));
        if !snake.alive {
            continue;
        }
        for x in [
            snake.head.x,
            snake.head.y,
            snake.heading,
            snake.fam,
            snake.speed,
            snake.body.len() as f64,
        ] {
            emit(x);
        }
        for p in &snake.body {
            emit(p.x);
            emit(p.y);
        }
    }
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

#[test]
#[ignore = "one-time study warmup; overwrites selected geometry"]
fn regenerate_spawn_fixture() {
    prepare_spawn(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/support/catalog_spawn.bin"
    ))
    .unwrap();
}

#[test]
fn catalog_worlds_keep_their_load_and_emit_stock_wire() {
    for (kind, expected) in [
        (EngineWorld::MatureOfficial, 500),
        (EngineWorld::SpawnOverload, 318),
    ] {
        let mut fixture = Fixture::new(kind).unwrap();
        assert_eq!(fixture.world().living(), expected);
        if matches!(kind, EngineWorld::MatureOfficial) {
            assert_eq!(fixture.census()["bodyPoints"], 32375);
            let food_updates = fixture.food_updates();
            let head = fixture.world().snakes()[0].head;
            // Both food-maintenance boundaries fall inside the default window.
            for _ in 0..144 {
                fixture.control(kind);
                fixture.tick(kind).unwrap();
                fixture.drain().unwrap();
            }
            assert!(fixture.world().living() >= 490);
            assert_eq!(fixture.food_updates() - food_updates, 2);
            assert!(fixture.world().snakes()[0].head.distance(head) > 1.);
        } else {
            assert_eq!(fixture.world().food().len(), 18346);
            let searches = fixture.tick(kind).unwrap();
            assert!(searches.iter().any(Result::is_err));
            assert!(searches.iter().any(Result::is_ok));
        }
        assert!(
            !fixture.drain().unwrap().is_empty() || matches!(kind, EngineWorld::MatureOfficial)
        );
    }
}
