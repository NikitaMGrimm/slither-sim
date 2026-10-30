//! Bounded, synthetic simulation-thread worlds, independent of socket scheduling.
use super::*;

#[derive(Clone, Copy)]
pub(super) enum EngineWorld {
    MatureOfficial,
    SpawnOverload,
}

impl EngineWorld {
    pub(super) fn epoch_ticks(self) -> usize {
        match self {
            Self::MatureOfficial => 144,
            Self::SpawnOverload => 16,
        }
    }
    pub(super) fn warmup_ticks(self) -> usize {
        match self {
            Self::MatureOfficial => 144,
            Self::SpawnOverload => 0,
        }
    }
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
                "reference-derived mature geometry/mass; cruise-speed new lives and admitted replacement; A4 controls; fresh ambient food/history; no live-capacity claim"
            }
            Self::SpawnOverload => {
                "study-derived 318-snake geometry at time zero; fresh ambient ecology; constant boost; frozen refused/successful search seeds; no live-capacity claim"
            }
        }
    }
    pub(super) fn config(self) -> Config {
        let mut config = match self {
            Self::MatureOfficial => {
                let mut config = Config::official(500);
                config.prey.count = 20;
                config
            }
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
        let mut definition = json!({"version":3, "config":self.config(), "protocol":19,
            "delivery":"official", "construction":self.synthetic(),
            "epochTicks":self.epoch_ticks(),
            "warmupTicks":self.warmup_ticks(),
            "recipients":match self {Self::MatureOfficial=>500,Self::SpawnOverload=>318},
            "trace":"initial/final complete authoritative state; per-tick ordered wire and search results; extra full state at each 72-tick food boundary"});
        match self {
            Self::MatureOfficial => {
                use sha2::{Digest, Sha256};
                definition["fixtureSha256"] = json!(
                    Sha256::digest(include_bytes!("catalog_mature.bin"))
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                );
                definition["sourceCheckpointSha256"] =
                    json!("f92a38ecc533c8b2cb28519e7189f93bc871632cdf0c3dbb8c2f84718b723e7d");
                definition["sourceGeometry"] =
                    json!({"living":499,"bodyPoints":27610,"p90Segments":176,"p99Segments":247});
                definition["replacement"] =
                    json!({"attempts":720,"radiusFraction":0.95,"clearance":300});
                definition["controller"] =
                    json!("direct-core A4-mean every six ticks; outside operation timing");
                definition["initialAmbientFood"] = json!(28897);
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
                    "first 18,346 admitted ambient pellets from a separate seed-7 world with foodCount=100,000 before pit-area scaling, same fixed geometry; no imported food history"
                );
                definition["fixtureFormat"] = json!(
                    "320 slots; little-endian f64 alive flag then living head/heading/fam/speed/body count/body xy; exact geometry, new life timers"
                );
            }
        }
        definition
    }
}

#[cfg(feature = "perf-internals")]
pub(super) fn run(args: &Args, world: EngineWorld, out: &mut impl Write) -> Result<Value, String> {
    use crate::catalog_backend::Fixture;
    if args.parallel {
        return Err("simulation-thread rows are serial".into());
    }
    let setup = Instant::now();
    let mut fixture = Fixture::new(world)?;
    let setup_seconds = setup.elapsed().as_secs_f64();
    let mut meter = Meter::default();
    let mut sampler = PerfControl::new()?;
    let initial = fixture.census();
    let initial_wire = fixture.initial_wire();
    if args.trace {
        line(
            out,
            json!({"kind":"initial", "case":args.case, "snapshot":world_events(fixture.world())?, "census":initial, "wire":initial_wire}),
        )?;
        line(out, json!({"kind":"warmed","ticks":world.warmup_ticks()}))?;
    } else {
        drop(initial_wire);
    }
    let epoch = world.epoch_ticks();
    let mut bytes = 0usize;
    let mut refused = 0usize;
    let mut successful = 0usize;
    let mut food_updates = 0;
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
        fixture.control(world);
        let previous_food = fixture.food_updates();
        let sampled = sampler
            .as_mut()
            .map(|p| p.select(fixture.food_update_next()))
            .unwrap_or(false);
        if sampled {
            sampler.as_mut().unwrap().command("enable")?;
        }
        let cpu_start = (!args.trace).then(host_load::cpu_seconds).flatten();
        let searches = meter.operation(args.trace, || fixture.tick(world))?;
        if !args.trace {
            operation_cpu = operation_cpu
                .zip(cpu_start)
                .zip(host_load::cpu_seconds())
                .map(|((total, start), end)| total + end - start);
        }
        if sampled {
            sampler.as_mut().unwrap().command("disable")?;
        }
        food_updates += fixture.food_updates() - previous_food;
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

// External perf sampling excludes setup/draining and can separate food-update ticks.
#[cfg(feature = "perf-internals")]
struct PerfControl {
    control: std::fs::File,
    ack: io::BufReader<std::fs::File>,
    ticks: String,
}
#[cfg(feature = "perf-internals")]
impl PerfControl {
    fn new() -> Result<Option<Self>, String> {
        let Some(path) = std::env::var_os("SLITHER_PERF_CONTROL") else {
            return Ok(None);
        };
        let ticks = std::env::var("SLITHER_PERF_TICKS").unwrap_or("all".into());
        if !matches!(ticks.as_str(), "all" | "food" | "ordinary") {
            return Err("Invalid perf tick selection".into());
        }
        Ok(Some(Self {
            control: std::fs::OpenOptions::new()
                .write(true)
                .open(path)
                .map_err(|e| e.to_string())?,
            ack: io::BufReader::new(
                std::fs::File::open(
                    std::env::var_os("SLITHER_PERF_ACK")
                        .ok_or("Missing perf acknowledgment FIFO")?,
                )
                .map_err(|e| e.to_string())?,
            ),
            ticks,
        }))
    }
    fn select(&self, food: bool) -> bool {
        self.ticks == "all" || (food && self.ticks == "food") || (!food && self.ticks == "ordinary")
    }
    fn command(&mut self, command: &str) -> Result<(), String> {
        use std::io::BufRead;
        writeln!(self.control, "{command}")
            .and_then(|()| self.control.flush())
            .map_err(|e| e.to_string())?;
        let mut ack = String::new();
        self.ack.read_line(&mut ack).map_err(|e| e.to_string())?;
        if ack.trim_matches(['\0', '\r', '\n']) != "ack" {
            return Err(format!("Unexpected perf acknowledgment {ack:?}"));
        }
        Ok(())
    }
}
