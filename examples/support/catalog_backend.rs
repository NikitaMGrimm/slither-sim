//! Scene construction using public state adapters and opt-in library raw-host hooks.
use super::*;
use crate::catalog_worlds::EngineWorld;
#[cfg(not(slither_private_state))]
use runtime::LegacyState;
#[cfg(test)]
use slither_sim::client::SimulatorBackend;
use slither_sim::client::{DeliveryProfile, SimulationConfig, simulation::RawWorld};

type Search = Result<(Vec<Point>, f64), String>;

pub(crate) struct Fixture {
    sim: RawWorld,
    actions: Vec<Option<Action>>,
    initial_wire: Vec<(usize, f64, Vec<u8>)>,
}

impl Fixture {
    pub(crate) fn world(&self) -> &Simulator {
        self.sim.world()
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
        let (recipients, known, food) = self.sim.composition();
        serde_json::json!({"living":lengths.len(),"bodyPoints":lengths.iter().sum::<usize>(),
            "recipients":recipients,"meanKnownSnakes":known as f64/recipients as f64,"meanFoodPerView":food as f64/recipients as f64,
            "meanSegments":lengths.iter().sum::<usize>() as f64/lengths.len() as f64,
            "p90Segments":lengths[((lengths.len()-1) as f64*0.9).ceil() as usize],
            "p99Segments":lengths[((lengths.len()-1) as f64*0.99).ceil() as usize],
            "food":self.world().food().len(),"radius":self.world().radius()})
    }
    pub(crate) fn new(kind: EngineWorld) -> Result<Self, String> {
        let config = SimulationConfig {
            agents: 320,
            world: kind.config(),
            delivery: DeliveryProfile::Official,
            ..Default::default()
        };
        let mut world = runtime::ring(7, kind.config(), 320)?;
        let mut snakes = world.snakes().to_vec();
        let mut data = &include_bytes!("catalog_spawn.bin")[..];
        for snake in &mut snakes {
            let mut next = || {
                let (head, tail) = data.split_at(8);
                data = tail;
                f64::from_le_bytes(head.try_into().unwrap())
            };
            if next() == 0. {
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
        runtime::initial_snakes(&mut world, snakes)?;
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
        runtime::record_events(&mut world);
        let sim = RawWorld::new(7, config, world)?;
        let mut fixture = Self {
            sim,
            actions: Vec::new(),
            initial_wire: Vec::new(),
        };
        fixture.initial_wire = fixture.drain()?;
        Ok(fixture)
    }
    pub(crate) fn control(&mut self) {
        self.actions = self
            .sim
            .world()
            .snakes()
            .iter()
            .map(|s| {
                s.alive.then_some(Action {
                    heading: s.heading,
                    boost: true,
                })
            })
            .collect();
        self.sim.controls(&self.actions).unwrap();
    }
    pub(crate) fn tick(&mut self) -> Result<Vec<Search>, String> {
        let mut searches = Vec::new();
        for seed in [40, 0] {
            let result = self.sim.search(seed);
            if let Err(error) = &result
                && error != "Spawn candidate search exhausted: no safe circular pose sampled"
            {
                return Err(error.clone());
            }
            searches.push(result);
        }
        self.sim.tick()?;
        Ok(searches)
    }
    pub(crate) fn drain(&mut self) -> Result<Vec<(usize, f64, Vec<u8>)>, String> {
        self.sim.drain()
    }
}

/// One-time offline fixture generator, used only to select the compact body
/// geometry from the published study's deterministic received-frame warmup.
#[cfg(test)]
pub(crate) fn prepare_spawn(path: &str) -> Result<(), String> {
    use slither_sim::client::{Bot, ClientStatus, ForagerBot};
    let mut backend = SimulatorBackend::new(
        7,
        SimulationConfig {
            agents: 320,
            world: Config::battledome(),
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
            eprintln!("warmup {decision} {}", backend.world().living());
        }
    }
    assert_eq!(backend.world().living(), 318, "study census changed");
    assert_eq!(
        backend
            .world()
            .snakes()
            .iter()
            .filter(|s| s.alive)
            .map(|s| s.body.len())
            .sum::<usize>(),
        18726,
        "study geometry changed"
    );
    let mut bytes = Vec::new();
    for snake in backend.world().snakes() {
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
fn battledome_keeps_its_load_and_emits_stock_wire() {
    let mut fixture = Fixture::new(EngineWorld::SpawnOverload).unwrap();
    assert_eq!(fixture.world().living(), 318);
    assert_eq!(fixture.census()["recipients"], 318);
    assert_eq!(fixture.world().food().len(), 18346);
    fixture.control();
    let searches = fixture.tick().unwrap();
    assert!(searches.iter().any(Result::is_err));
    assert!(searches.iter().any(Result::is_ok));
    assert!(!fixture.drain().unwrap().is_empty());
}
