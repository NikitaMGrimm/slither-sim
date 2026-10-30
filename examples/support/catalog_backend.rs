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
    policy: slither_sim::bot::Forager,
    actions: Vec<Option<Action>>,
    control_tick: usize,
    initial_wire: Vec<(usize, f64, Vec<u8>)>,
}

impl Fixture {
    pub(crate) fn world(&self) -> &Simulator {
        self.sim.world()
    }
    pub(crate) fn food_updates(&self) -> u64 {
        self.sim.food_epoch()
    }
    pub(crate) fn food_update_next(&self) -> bool {
        self.sim.food_update_next()
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
        let mut world = runtime::ring(7, kind.config(), count)?;
        let mut snakes = world.snakes().to_vec();

        let mut data = match kind {
            EngineWorld::MatureOfficial => &include_bytes!("catalog_mature.bin")[..],
            EngineWorld::SpawnOverload => &include_bytes!("catalog_spawn.bin")[..],
        };
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
            if matches!(kind, EngineWorld::MatureOfficial) {
                snake.speed = physics::base_speed(count);
            }
        }
        assert!(data.is_empty());
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
        if matches!(kind, EngineWorld::MatureOfficial) {
            let food = world.food()[..28897].to_vec();
            runtime::replace_food(&mut world, food)?;
        }
        if matches!(kind, EngineWorld::MatureOfficial) {
            for slot in 0..count {
                if world.snakes()[slot].alive {
                    continue;
                }
                let mut pose = None;
                for attempt in 0..720 {
                    let angle = attempt as f64 * TAU / 720.;
                    let r = world.radius() * 0.95;
                    let (body, heading) = world
                        .spawn_body(
                            Point {
                                x: r * angle.cos(),
                                y: r * angle.sin(),
                            },
                            angle + FRAC_PI_2,
                        )
                        .map_err(|e| e.to_string())?;
                    if world.body_is_clear(&body, 300.) {
                        pose = Some((body, heading));
                        break;
                    }
                }
                let (body, heading) =
                    pose.ok_or("No clear replacement for the reference's dead slot")?;
                world
                    .respawn_body(slot, body, heading)
                    .map_err(|e| e.to_string())?;
            }
        }
        runtime::record_events(&mut world);
        let sim = RawWorld::new(7, config, world)?;
        let mut fixture = Self {
            sim,
            policy: slither_sim::bot::Forager::default(),
            actions: Vec::new(),
            control_tick: 0,
            initial_wire: Vec::new(),
        };
        fixture.initial_wire = fixture.drain()?;
        for _ in 0..kind.warmup_ticks() {
            fixture.control(kind);
            fixture.sim.tick()?;
            let wire = fixture.drain()?;
            fixture.initial_wire.extend(wire);
        }
        Ok(fixture)
    }
    pub(crate) fn control(&mut self, kind: EngineWorld) {
        if matches!(kind, EngineWorld::MatureOfficial) {
            if self.control_tick.is_multiple_of(6) {
                let observations = self.sim.world().observe_all();
                self.actions = observations
                    .iter()
                    .map(|o| {
                        o.as_ref().map(|o| {
                            self.policy
                                .decide(o, &slither_sim::bot::Timing::default(), &[])
                        })
                    })
                    .collect();
            }
        } else {
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
        }
        self.sim.controls(&self.actions).unwrap();
        self.control_tick += 1;
    }
    pub(crate) fn tick(&mut self, kind: EngineWorld) -> Result<Vec<Search>, String> {
        let mut searches = Vec::new();
        if matches!(kind, EngineWorld::SpawnOverload) {
            for seed in [40, 0] {
                let result = self.sim.search(seed);
                if let Err(error) = &result
                    && error != "Spawn candidate search exhausted: no safe circular pose sampled"
                {
                    return Err(error.clone());
                }
                searches.push(result);
            }
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
#[ignore = "requires the retained reference checkpoint; overwrites selected geometry"]
fn regenerate_mature_fixture() {
    use sha2::{Digest, Sha256};
    let bytes =
        std::fs::read(std::env::var_os("SLITHER_MATURE_CHECKPOINT").expect("checkpoint path"))
            .unwrap();
    assert_eq!(
        Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "f92a38ecc533c8b2cb28519e7189f93bc871632cdf0c3dbb8c2f84718b723e7d"
    );
    let checkpoint: Value = serde_json::from_slice(&bytes).unwrap();
    let slots = checkpoint["state"]["slots"].as_array().unwrap();
    assert_eq!(slots.len(), 500);
    let mut output = Vec::new();
    for slot in slots {
        let snake = &slot["snake"];
        let alive = snake["alive"].as_bool().unwrap();
        let mut emit = |x: f64| output.extend_from_slice(&x.to_le_bytes());
        emit(f64::from(alive));
        if !alive {
            continue;
        }
        for value in [
            &snake["head"]["x"],
            &snake["head"]["y"],
            &snake["heading"],
            &snake["fam"],
            &snake["speed"],
        ] {
            emit(value.as_f64().unwrap());
        }
        let body = snake["body"].as_array().unwrap();
        emit(body.len() as f64);
        for point in body {
            emit(point["x"].as_f64().unwrap());
            emit(point["y"].as_f64().unwrap());
        }
    }
    assert_eq!(
        Sha256::digest(&output)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        "2b0cf55a05c3d0c585deda60497da7298e8f7ff6c2b28e74818efdd91941b702"
    );
    std::fs::write(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/support/catalog_mature.bin"
        ),
        output,
    )
    .unwrap();
}

#[test]
fn catalog_worlds_keep_their_load_and_emit_stock_wire() {
    for (kind, expected) in [
        (EngineWorld::MatureOfficial, 500),
        (EngineWorld::SpawnOverload, 318),
    ] {
        let mut fixture = Fixture::new(kind).unwrap();
        assert!(if matches!(kind, EngineWorld::MatureOfficial) {
            fixture.world().living() >= 490
        } else {
            fixture.world().living() == expected
        });
        assert_eq!(fixture.census()["recipients"], expected);
        if matches!(kind, EngineWorld::MatureOfficial) {
            assert!((27000..29000).contains(&fixture.census()["bodyPoints"].as_u64().unwrap()));
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
