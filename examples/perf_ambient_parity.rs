//! Untimed, synthetic ambient-refill parity fixtures; not performance worlds.
//!
//! Build this identical file against old/new runtimes with the same toolchain,
//! profile and platform, retain source/binary pins externally, and byte-compare
//! complete stdout JSONL. No runtime pin is embedded in the trace, so provenance
//! alone cannot make otherwise identical traces differ. INITIAL/FINAL serialize
//! Simulator itself, including private RNG/clocks, rather than a debug snapshot.
//! Event buffers are skipped by Simulator serialization and emitted explicitly.
//! Only object keys and the unordered state.prey_outward set are canonicalized.
//!
//! No snakes/prey: public food edits are synthetic capacity probes, not pickups.
//! No eater, collision, boost/death generation, startup-density, observation,
//! protocol, invalid-config or exhaustive numerical-boundary coverage. Retain
//! the normal/portable parity checks separately. This file contains no refill
//! reference implementation, expected RNG state, timing or speed measurement.
use serde::Serialize;
use serde_json::{Value, json};
use slither_sim::{AmbientFoodConfig, Config, Food, Origin, PickupItem, Simulator};
use std::{
    collections::BTreeMap,
    io::{self, BufWriter, Write},
};

const DT: f64 = 0.007;
const SEEDS: [u32; 2] = [7, 19];
const CASES: [&str; 8] = [
    "single-default",
    "epsilon-credit",
    "multiple",
    "cap-disabled",
    "public-food",
    "target-full",
    "rate-changes",
    "dense-batch",
];
const HELP: &str = "perf_ambient_parity [CASE|all] [--seed 7|19|all] [--full-state]
Untimed canonical JSONL to stdout; defaults: all cases, both seeds.
CASE: single-default, epsilon-credit, multiple, cap-disabled, public-food,
      target-full, rate-changes, dense-batch.
No custom geometry/rates/tick counts. Each case has <=1000 ticks and target <=64.
--full-state adds serialized Simulator to every TICK; requires cases <=100 ticks.
--help / -h prints this help. Unknown, repeated or missing arguments are errors.
Compare entire stdout bytes from identical harness sources against old/new runtimes.
REPORT gives fixture/tick/event denominators and coverage limits; no clocks sampled.";

struct Args {
    cases: Vec<&'static str>,
    seeds: Vec<u32>,
    full_state: bool,
}

fn args() -> Result<Option<Args>, String> {
    let raw: Vec<_> = std::env::args().skip(1).collect();
    if raw.len() == 1 && matches!(raw[0].as_str(), "--help" | "-h") {
        return Ok(None);
    }
    let mut case = None;
    let mut seed = None;
    let mut full_state = false;
    let mut iter = raw.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--seed" if seed.is_none() => {
                seed = Some(iter.next().ok_or("--seed requires 7, 19 or all")?.as_str());
            }
            "--full-state" if !full_state => full_state = true,
            "all" if case.is_none() => case = Some("all"),
            name if case.is_none() && CASES.contains(&name) => case = Some(name),
            _ => return Err(format!("Unknown or repeated argument: {arg}\n{HELP}")),
        }
    }
    let cases: Vec<_> = CASES
        .into_iter()
        .filter(|name| case.is_none_or(|chosen| chosen == "all" || chosen == *name))
        .collect();
    let seeds = match seed.unwrap_or("all") {
        "all" => SEEDS.to_vec(),
        "7" => vec![7],
        "19" => vec![19],
        other => return Err(format!("Unsupported seed: {other}\n{HELP}")),
    };
    if full_state && cases.iter().any(|name| fixture(name).ticks > 100) {
        return Err("--full-state requires a case with <=100 ticks; select a case other than single-default".into());
    }
    Ok(Some(Args {
        cases,
        seeds,
        full_state,
    }))
}

#[derive(Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
enum Edit {
    Target { count: usize },
    Rate { per_second: f64 },
    ReplaceFood { food: Vec<Food> },
    RemoveFood { index: usize },
    MoveFood { index: usize, x: f64, y: f64 },
    ReverseFood,
}

impl Edit {
    fn apply(&self, world: &mut Simulator) -> Result<(), String> {
        match self {
            Self::Target { count } => world.config.food_count = *count,
            Self::Rate { per_second } => world.set_ambient_rate(*per_second)?,
            Self::ReplaceFood { food } => world.food.clone_from(food),
            Self::RemoveFood { index } => {
                if *index >= world.food.len() {
                    return Err(format!("Scheduled removal index {index} absent"));
                }
                world.food.remove(*index);
            }
            Self::MoveFood { index, x, y } => {
                let food = world.food.get_mut(*index).ok_or("Scheduled move absent")?;
                food.x = *x;
                food.y = *y;
            }
            Self::ReverseFood => world.food.reverse(),
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct Scheduled {
    /// One-based physics tick; edits occur before step(&[]), in listed order.
    tick: usize,
    edit: Edit,
}

#[derive(Serialize)]
struct Fixture {
    name: &'static str,
    ticks: usize,
    radius: f64,
    initial_rate: f64,
    cap_enabled: bool,
    setup: Vec<Edit>,
    schedule: Vec<Scheduled>,
    coverage: &'static str,
}

fn pellet(origin: Option<Origin>, x: f64) -> Food {
    Food {
        x,
        y: 0.0,
        value: 1.0,
        size: Some(1.0),
        origin,
    }
}

fn fixture(name: &str) -> Fixture {
    use Edit::*;
    use Origin::*;
    let mut f = Fixture {
        name: CASES[0],
        ticks: 1000,
        radius: 1000.0,
        initial_rate: 5.0,
        cap_enabled: true,
        setup: vec![Target { count: 64 }],
        schedule: vec![],
        coverage: "Default 5/s rate, accumulated fractional credit, at most one due interval per tick",
    };
    let entries = match name {
        "single-default" => vec![],
        "epsilon-credit" => {
            f.name = CASES[1];
            f.ticks = 32;
            // Fixed synthetic intervals straddle the existing 1e-12 tolerance.
            // Do not compute due counts here: only the actual runtime does that.
            f.initial_rate = 1.0 / (DT + 0.5e-12);
            f.coverage = "First interval DT+0.5e-12; zero resets credit before DT+2e-12, then fractional 7.5/s";
            vec![
                (5, Rate { per_second: 0.0 }),
                (
                    6,
                    Rate {
                        per_second: 1.0 / (DT + 2.0e-12),
                    },
                ),
                (12, Rate { per_second: 7.5 }),
            ]
        }
        "multiple" | "cap-disabled" | "dense-batch" => {
            f.name = match name {
                "multiple" => CASES[2],
                "cap-disabled" => CASES[3],
                _ => CASES[7],
            };
            f.ticks = 100;
            f.initial_rate = 1000.0;
            f.cap_enabled = name != "cap-disabled";
            f.radius = if name == "multiple" { 1000.0 } else { 20.0 };
            f.coverage = if name == "dense-batch" {
                "Multiple due intervals with deterministic admission then rejection in the same origin cell; public clear at tick 51; rejected candidates consume actual runtime RNG"
            } else {
                "Multiple due intervals per tick, target reached inside batch; clear at tick 51; radius 20 disabled-cap variant puts every candidate at origin"
            };
            vec![(51, ReplaceFood { food: vec![] })]
        }
        "public-food" => {
            f.name = CASES[4];
            f.ticks = 32;
            f.radius = 20.0;
            f.initial_rate = 100.0;
            f.coverage = "Single-attempt accept/reject at origin; boost/death/ambient/missing origins count toward occupancy; remove/replace/move/reverse edits; negative and 75-unit cell edges";
            f.setup.push(ReplaceFood {
                food: vec![pellet(Some(Boost), 0.0), pellet(Some(Death), 0.0)],
            });
            vec![
                (3, RemoveFood { index: 0 }),
                (
                    5,
                    ReplaceFood {
                        food: vec![pellet(Some(Boost), 0.0), pellet(None, 0.0)],
                    },
                ),
                (7, RemoveFood { index: 0 }),
                (
                    9,
                    ReplaceFood {
                        food: vec![pellet(Some(Ambient), 0.0), pellet(Some(Boost), 0.0)],
                    },
                ),
                (11, RemoveFood { index: 1 }),
                (
                    13,
                    ReplaceFood {
                        food: vec![
                            pellet(Some(Boost), -75.25),
                            pellet(Some(Death), -75.0),
                            pellet(None, -0.25),
                            pellet(Some(Ambient), 75.0),
                        ],
                    },
                ),
                (
                    18,
                    MoveFood {
                        index: 4,
                        x: -0.25,
                        y: 0.0,
                    },
                ),
                (19, ReverseFood),
            ]
        }
        "target-full" => {
            f.name = CASES[5];
            f.ticks = 20;
            f.radius = 20.0;
            f.initial_rate = 1000.0;
            f.cap_enabled = false;
            f.coverage = "Already full, reached within batch, zero target, target below current count; non-ambient food does not satisfy ambient target";
            f.setup = vec![
                Target { count: 2 },
                ReplaceFood {
                    food: vec![
                        pellet(Some(Ambient), 0.0),
                        pellet(Some(Ambient), 0.0),
                        pellet(Some(Boost), 0.0),
                    ],
                },
            ];
            vec![
                (3, Target { count: 3 }),
                (5, Target { count: 0 }),
                (7, Target { count: 2 }),
                (
                    9,
                    ReplaceFood {
                        food: vec![pellet(Some(Death), 0.0)],
                    },
                ),
            ]
        }
        "rate-changes" => {
            f.name = CASES[6];
            f.ticks = 100;
            f.coverage = "Public rate setter rescales fractional credit, unchanged-rate no-op, zero discard, restart, switch to/from multi-attempt batches";
            vec![
                (3, Rate { per_second: 7.5 }),
                (20, Rate { per_second: 7.5 }),
                (25, Rate { per_second: 0.0 }),
                (40, Rate { per_second: 1000.0 }),
                (50, Rate { per_second: 5.0 }),
                (64, Rate { per_second: 0.0 }),
                (65, Rate { per_second: 5.0 }),
            ]
        }
        _ => unreachable!("CLI validates case names"),
    };
    f.schedule = entries
        .into_iter()
        .map(|(tick, edit)| Scheduled { tick, edit })
        .collect();
    f
}

fn canonical(value: Value) -> Value {
    match value {
        Value::Object(fields) => {
            let sorted: BTreeMap<_, _> = fields
                .into_iter()
                .map(|(key, value)| (key, canonical(value)))
                .collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
        scalar => scalar,
    }
}

fn state(world: &Simulator) -> Result<Value, String> {
    let mut state = serde_json::to_value(world).map_err(|e| e.to_string())?;
    if let Some(outward) = state.get_mut("prey_outward").and_then(Value::as_array_mut) {
        outward.sort_unstable_by_key(Value::as_u64);
    }
    Ok(state)
}

fn emit(out: &mut impl Write, value: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *out, &canonical(value)).map_err(|e| e.to_string())?;
    out.write_all(b"\n").map_err(|e| e.to_string())
}

fn run(args: Args, out: &mut impl Write) -> Result<(), String> {
    let mut runs = Vec::new();
    for name in args.cases {
        let f = fixture(name);
        for &seed in &args.seeds {
            let mut config = Config {
                radius: f.radius,
                dt: DT,
                food_count: 0,
                ambient_replenish_per_second: f.initial_rate,
                ambient_food: AmbientFoodConfig {
                    enabled: f.cap_enabled,
                    cell_size: 75.0,
                    max_per_cell: 2,
                },
                dynamic_radius: false,
                radius_updates: vec![],
                ..Config::default()
            };
            config.prey.count = 0;
            let construction_config = config.clone();
            let mut world = Simulator::new(seed, config)?;
            world.food_spawn_events = Some(Vec::new());
            world.pickup_events = Some(Vec::new());
            for edit in &f.setup {
                edit.apply(&mut world)?;
            }
            emit(
                out,
                json!({
                    "record": "INITIAL", "case": name, "seed": seed,
                    "fixture": &f, "construction_config": construction_config,
                    "event_sinks_enabled": true, "full_state_every_tick": args.full_state,
                    "state": state(&world)?,
                }),
            )?;
            let mut spawn_count = 0;
            let mut pickup_count = 0;
            for tick in 1..=f.ticks {
                let edits: Vec<_> = f
                    .schedule
                    .iter()
                    .filter(|entry| entry.tick == tick)
                    .map(|entry| &entry.edit)
                    .collect();
                for edit in &edits {
                    edit.apply(&mut world)?;
                }
                world.step(&[])?;
                let spawns: Vec<_> = world
                    .food_spawn_events
                    .as_ref()
                    .ok_or("Spawn sink disabled")?
                    .iter()
                    .map(|event| json!({"source": event.source, "food": &event.food}))
                    .collect();
                let pickups: Vec<_> = world
                    .pickup_events
                    .as_ref()
                    .ok_or("Pickup sink disabled")?
                    .iter()
                    .map(|event| {
                        let item = match &event.item {
                            PickupItem::Food(food) => json!({"food": food}),
                            PickupItem::Prey(prey) => json!({"prey": prey}),
                        };
                        json!({"eater": event.eater, "item": item})
                    })
                    .collect();
                spawn_count += spawns.len();
                pickup_count += pickups.len();
                let mut record = json!({
                    "record": "TICK", "case": name, "seed": seed, "tick": tick,
                    "actions": [], "edits_before_step": edits, "time": world.time,
                    "food_count": world.food.len(), "ambient_target": world.config.food_count,
                    "rate": world.config.ambient_replenish_per_second,
                    "food_spawn": spawns, "pickup": pickups,
                });
                if args.full_state {
                    record["state"] = state(&world)?;
                }
                emit(out, record)?;
            }
            emit(
                out,
                json!({
                    "record": "FINAL", "case": name, "seed": seed, "ticks": f.ticks,
                    "state": state(&world)?,
                }),
            )?;
            runs.push(json!({
                "case": name, "seed": seed, "ticks": f.ticks, "time": world.time,
                "food_spawn_events": spawn_count, "pickup_events": pickup_count,
                "final_food_count": world.food.len(), "coverage": f.coverage,
            }));
        }
    }
    emit(
        out,
        json!({
            "record": "REPORT", "schema": "ambient-parity-v1", "synthetic": true,
            "mode": "untimed", "dt": DT, "fixture_runs": runs.len(), "runs": runs,
            "bounds": {"cases": 8, "seeds": 2, "ticks_per_case": 1000, "ambient_target": 64,
                       "public_injected_food": 4, "full_state_ticks_per_case": 100},
            "event_denominator": "Reported births/pickups are actual sink events; due intervals and rejected candidates are not counted or reimplemented",
            "coverage": "Ambient refill only; ordered events, edits and endpoint full states retain actual RNG and virtual clocks; optional per-tick full state",
            "uncovered": ["eaters and same-tick pickup freeing capacity", "collisions and snake motion",
                          "prey", "boost/death pellet generation", "startup density admission",
                          "observations and protocol", "invalid configuration", "exhaustive numeric boundaries"],
            "comparison": "Byte-compare complete JSONL from identical harness sources, toolchain, platform, profile and arguments; retain runtime/harness/binary pins separately",
        }),
    )
}

fn main() {
    let result = (|| {
        let Some(args) = args()? else {
            println!("{HELP}");
            return Ok(());
        };
        let mut out = BufWriter::new(io::stdout().lock());
        run(args, &mut out)?;
        out.flush().map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
