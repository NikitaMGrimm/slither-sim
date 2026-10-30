//! Bounded, synthetic simulation-thread worlds, independent of socket scheduling.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum EngineWorld {
    MatureOfficial,
    SpawnOverload,
}

impl EngineWorld {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::MatureOfficial => "engine-official-mature-500",
            Self::SpawnOverload => "engine-battledome-spawn-318",
        }
    }
    pub(super) fn mode(self) -> &'static str {
        match self {
            Self::MatureOfficial => "official",
            Self::SpawnOverload => "battledome",
        }
    }
    pub(super) fn operation(self) -> &'static str {
        match self {
            Self::MatureOfficial => "raw-tick",
            Self::SpawnOverload => "admission-searches+raw-tick",
        }
    }
    pub(super) fn synthetic(self) -> &'static str {
        match self {
            Self::MatureOfficial => {
                "direct mature bodies; fresh ambient ecology; stock protocol-19 recipients; no live-capacity claim"
            }
            Self::SpawnOverload => {
                "study-derived 318-snake geometry at time zero; fresh ambient ecology; constant boost; frozen refused/successful search seeds; no live-capacity claim"
            }
        }
    }
    pub(super) fn config(self) -> Config {
        let mut config = match self {
            Self::MatureOfficial => Config::official(500),
            Self::SpawnOverload => {
                let mut config = Config::battledome();
                let scale = 11333.603134043473 / config.radius;
                // Freezing the wall also freezes its grown pit and spawn band.
                // Disabling seat-following otherwise restores the small ring.
                config.pit.as_mut().unwrap().radius *= scale;
                config.spawn_radius = config.spawn_radius.map(|r| r * scale);
                config.spawn_radius_max = config.spawn_radius_max.map(|r| r * scale);
                config.radius = 11333.603134043473;
                config.food_count = (config.food_count as f64 * scale * scale) as usize;
                config.ambient_replenish_per_second *= scale * scale;
                config
            }
        };
        config.dynamic_radius = false;
        config.size_to_population = false;
        pinned_config(self.name(), config)
    }
    pub(super) fn definition(self) -> Value {
        let mut definition = json!({"version":1, "config":self.config(), "protocol":19,
            "delivery":"official", "construction":self.synthetic(),
            "epochTicks":match self {Self::MatureOfficial=>144, Self::SpawnOverload=>16},
            "warmupTicks":match self {Self::MatureOfficial=>1,Self::SpawnOverload=>0},
            "recipients":match self {Self::MatureOfficial=>500,Self::SpawnOverload=>318},
            "trace":"initial/final complete authoritative state; per-tick ordered wire and search results; extra full state at each 72-tick food boundary"});
        match self {
            Self::MatureOfficial => {
                definition["segments"] = json!([[250, 16], [150, 60], [75, 176], [25, 247]]);
                definition["lanes"] =
                    json!({"firstRadius":3000,"spacing":180,"gap":600,"bodyArcSpacing":42});
            }
            Self::SpawnOverload => {
                use sha2::{Digest, Sha256};
                let hash: String = Sha256::digest(include_bytes!("catalog_spawn.bin"))
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                definition["fixtureSha256"] = json!(hash);
                definition["spawnSearchSeeds"] = json!([40, 0]);
                definition["spawnSearchWaitedSeconds"] = json!(0);
                definition["foodConstruction"] = json!(
                    "first 18,346 admitted ambient pellets from a separate seed-7 world with 100,000 startup placement attempts, same fixed geometry; no imported food history"
                );
                definition["fixtureFormat"] = json!(
                    "320 slots; little-endian f64 alive flag then living head/heading/fam/speed/body count/body xy; exact geometry, new life timers"
                );
            }
        }
        definition
    }
}

#[cfg(feature = "networking")]
pub(super) fn run(args: &Args, world: EngineWorld, out: &mut impl Write) -> Result<Value, String> {
    use crate::client::simulation::catalog_backend::Fixture;
    if args.parallel {
        return Err("simulation-thread rows are serial".into());
    }
    let setup = Instant::now();
    let mut fixture = Fixture::new(world)?;
    let setup_seconds = setup.elapsed().as_secs_f64();
    let mut meter = Meter::default();
    let initial = fixture.census();
    if args.trace {
        line(
            out,
            json!({"kind":"initial", "case":args.case, "snapshot":world_events(fixture.world())?, "census":initial, "wire":fixture.initial_wire()}),
        )?;
    }
    let epoch = match world {
        EngineWorld::MatureOfficial => 144,
        EngineWorld::SpawnOverload => 16,
    };
    let mut bytes = 0usize;
    let mut refused = 0usize;
    let mut successful = 0usize;
    let mut food_updates = 0;
    let mut operation_cpu = Some(0.);
    let mut minimum = fixture.world().living();
    for iteration in 0..args.iterations {
        if iteration > 0 && iteration % epoch == 0 {
            fixture = Fixture::new(world)?;
        }
        fixture.control(world);
        let previous_food = fixture.food_updates();
        let cpu_start = (!args.trace).then(host_load::cpu_seconds).flatten();
        let searches = meter.operation(args.trace, || fixture.tick(world))?;
        if !args.trace {
            operation_cpu = operation_cpu
                .zip(cpu_start)
                .zip(host_load::cpu_seconds())
                .map(|((total, start), end)| total + end - start);
        }
        food_updates += fixture.food_updates() - previous_food;
        refused += searches.iter().filter(|r| r.is_err()).count();
        successful += searches.iter().filter(|r| r.is_ok()).count();
        let wire = fixture.drain()?;
        bytes += wire.iter().map(|(_, _, data)| data.len()).sum::<usize>();
        minimum = minimum.min(fixture.world().living());
        if args.trace {
            let mut record =
                json!({"kind":"tick","iteration":iteration,"searches":searches,"wire":wire});
            if (iteration + 1) % 72 == 0 {
                record["snapshot"] = world_events(fixture.world())?;
            }
            line(out, record)?;
        }
    }
    if args.trace {
        line(
            out,
            json!({"kind":"final","snapshot":world_events(fixture.world())?,"census":fixture.census()}),
        )?;
    }
    if bytes == 0
        || matches!(world, EngineWorld::SpawnOverload) && (refused == 0 || successful == 0)
        || minimum
            < match world {
                EngineWorld::MatureOfficial => 490,
                EngineWorld::SpawnOverload => 300,
            }
    {
        return Err("engine fixture lost its intended population or emitted no wire bytes".into());
    }
    let mut result = report(
        args,
        meter,
        world.operation(),
        json!({"initial":initial,"final":fixture.census(),
        "minimumLiving":minimum,"wireBytes":bytes,"refusedSearches":refused,"successfulSearches":successful,
        "setupSeconds":if args.trace {Value::Null}else{json!(setup_seconds)},
        "foodUpdateTicks": food_updates,
        "profile":"unsupported: raw host tick has no phase-profile API"}),
    );
    if !args.trace {
        result["timing"]["operationCpuSeconds"] = json!(operation_cpu);
    }
    Ok(result)
}
