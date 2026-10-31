//! Bounded synthetic engine work, independent of socket scheduling.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum EngineWorld {
    SpawnOverload,
    CollisionGrid,
}

impl EngineWorld {
    pub(super) fn epoch_ticks(self) -> usize {
        16
    }
    pub(super) fn name(self) -> &'static str {
        if matches!(self, Self::CollisionGrid) {
            return "engine-collision-grid-4096";
        }
        "engine-battledome-spawn-318"
    }
    pub(super) fn mode(self) -> &'static str {
        if matches!(self, Self::CollisionGrid) {
            return "sandbox";
        }
        "battledome"
    }
    pub(super) fn operation(self) -> &'static str {
        if matches!(self, Self::CollisionGrid) {
            return "step";
        }
        "admission-searches+raw-tick"
    }
    pub(super) fn synthetic(self) -> &'static str {
        if matches!(self, Self::CollisionGrid) {
            return "4096 separated straight bodies on a 64 by 64 lattice; 4095 eight-anchor snakes and one 532-anchor snake; fixed heading, no food or prey; 16-tick epochs";
        }
        "study-derived 318-snake geometry at time zero; fresh ambient ecology; constant boost; frozen refused/successful search seeds; no live-capacity claim"
    }
    pub(super) fn config(self) -> Config {
        if matches!(self, Self::CollisionGrid) {
            let mut config = Config::sandbox();
            config.radius = 60000.;
            config.dynamic_radius = false;
            config.size_to_population = false;
            config.food_count = 0;
            config.ambient_replenish_per_second = 0.;
            config.prey.count = 0;
            config.kiosk.enabled = false;
            return pinned_config(self.name(), config);
        }
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
        config.dynamic_radius = false;
        config.size_to_population = false;
        pinned_config(self.name(), config)
    }
    pub(super) fn definition(self) -> Value {
        if matches!(self, Self::CollisionGrid) {
            return json!({"version":1, "config":self.config(),
                "construction":self.synthetic(), "population":4096,
                "latticePitch":512, "anchorSpacing":42, "grownSlot":0,
                "segments":[532,8], "heading":0, "boost":false,
                "warmupTicks":0, "epochTicks":self.epoch_ticks(),
                "trace":"complete initial and per-tick authoritative state and events"});
        }
        use sha2::{Digest, Sha256};
        let hash: String = Sha256::digest(include_bytes!("catalog_spawn.bin"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        json!({"version":3, "config":self.config(), "protocol":19,
            "delivery":"official", "construction":self.synthetic(),
            "epochTicks":self.epoch_ticks(), "warmupTicks":0, "recipients":318,
            "trace":"initial/final complete authoritative state; per-tick ordered wire and search results; extra full state every 72 operations",
            "fixtureSha256":hash, "spawnSearchSeeds":[40,0], "spawnSearchWaitedSeconds":0,
            "foodConstruction":"first 18,346 admitted ambient pellets from a separate seed-7 world with foodCount=100,000 before pit-area scaling, same fixed geometry; no imported food history",
            "fixtureFormat":"320 slots; little-endian f64 alive flag then living head/heading/fam/speed/body count/body xy; exact geometry, new life timers"})
    }
}

#[cfg(feature = "perf-internals")]
pub(super) fn run(args: &Args, world: EngineWorld, out: &mut impl Write) -> Result<Value, String> {
    use crate::catalog_backend::Fixture;
    if args.parallel {
        return Err("simulation-thread rows are serial".into());
    }
    if matches!(world, EngineWorld::CollisionGrid) {
        return catalog_collision::run(args, world, out);
    }
    let setup = Instant::now();
    let mut fixture = Fixture::new(world)?;
    let setup_seconds = setup.elapsed().as_secs_f64();
    let mut meter = Meter::default();
    let initial = fixture.census();
    let initial_wire = fixture.initial_wire();
    if args.trace {
        line(
            out,
            json!({"kind":"initial", "case":args.case, "snapshot":world_events(fixture.world())?, "census":initial, "wire":initial_wire}),
        )?;
        line(out, json!({"kind":"warmed","ticks":0}))?;
    } else {
        drop(initial_wire);
    }
    let epoch = world.epoch_ticks();
    let mut bytes = 0usize;
    let mut refused = 0usize;
    let mut successful = 0usize;
    let mut operation_cpu = Some(0.);
    let mut minimum = fixture.world().living();
    for iteration in 0..args.iterations {
        if iteration > 0 && iteration % epoch == 0 {
            fixture = Fixture::new(world)?;
            let initial_wire = fixture.initial_wire();
            if args.trace {
                line(
                    out,
                    json!({"kind":"epoch","iteration":iteration,"snapshot":world_events(fixture.world())?,"wire":initial_wire}),
                )?;
            }
        }
        fixture.control();
        let cpu_start = (!args.trace).then(host_load::cpu_seconds).flatten();
        let searches = meter.operation(args.trace, || fixture.tick())?;
        if !args.trace {
            operation_cpu = operation_cpu
                .zip(cpu_start)
                .zip(host_load::cpu_seconds())
                .map(|((total, start), end)| total + end - start);
        }
        refused += searches.iter().filter(|r| r.is_err()).count();
        successful += searches.iter().filter(|r| r.is_ok()).count();
        let wire = fixture.drain()?;
        bytes += wire.iter().map(|(_, _, data)| data.len()).sum::<usize>();
        minimum = minimum.min(fixture.world().living());
        if args.trace {
            let mut record =
                json!({"kind":"step","iteration":iteration,"searches":searches,"wire":wire});
            if (iteration + 1) % 72 == 0 || iteration + 1 == args.iterations {
                record["snapshot"] = world_events(fixture.world())?;
            }
            line(out, record)?;
        }
    }
    if args.trace {
        line(
            out,
            json!({"kind":"end","case":args.case,"operations":args.iterations,"census":fixture.census()}),
        )?;
    }
    if bytes == 0 || refused == 0 || successful == 0 || minimum < 300 {
        return Err("engine fixture lost its intended population or emitted no wire bytes".into());
    }
    // Sixteen-tick epochs reset before the first 504 ms food update.
    let mut result = report(
        args,
        meter,
        world.operation(),
        json!({"initial":initial,"final":fixture.census(),
        "minimumLiving":minimum,"wireBytes":bytes,"refusedSearches":refused,"successfulSearches":successful,
        "setupSeconds":if args.trace {Value::Null}else{json!(setup_seconds)},
        "foodUpdateTicks": 0,
        "profile":"unsupported: raw host tick has no phase-profile API"}),
    );
    if !args.trace {
        result["timing"]["operationCpuSeconds"] = json!(operation_cpu);
    }
    Ok(result)
}
