//! Offline immutable-world observations: perf_observation CASE ITERATIONS [--trace].
//! Timing emits one report compatible with tools/perf_baseline.py::native's workload.
//! Trace is untimed and checks ordered batch outputs against scalar observe(slot).
//!
//! Parity procedure (same toolchain/profile/platform): build this identical file
//! against old/new runtimes, retain binary/source hashes, then byte-compare complete
//! --trace JSONL. Only object keys and the unordered prey_outward set are sorted.
//! To bridge observe-dense to the original perf_baseline observe fixture, compare
//! canonical initial snapshots and each observe record's snapshot/result exactly;
//! report/envelope fields differ. No gameplay or telemetry fields are stripped.
//!
//! Source hypotheses, not measurements: compact stresses accepted output copies;
//! wide stresses sparse visibility; strip stresses x-bin candidates rejected by y;
//! sparse sets a large food grid against only eight observers; clustered puts
//! the food in corner patches that none of 128 observers can see.
//! Before changing an index, independently cover circle/box boundaries (adjacent
//! floats), negative bin boundaries, 64-bin aliasing, signed zero, tiny vision,
//! saturated cell casts, dead slots, mask errors, and insertion-order ties. These
//! eight bounded fixtures do not establish parity for all such numerical cases.
#[path = "support/runtime.rs"]
mod runtime;
#[cfg(not(slither_private_state))]
#[allow(unused_imports)]
use runtime::LegacyState;

#[path = "support/state.rs"]
mod checkpoint_state;
use serde_json::{Value, json};
use slither_sim::{
    Config, Food, Observation, PickupItem, Point, Simulator, physics, profiling::WorldStats,
};
use std::{
    collections::BTreeMap,
    hint::black_box,
    io::{self, BufWriter, Write},
    time::Instant,
};

const SEED: u32 = 7;
const AGENTS: usize = 32;
const ANCHORS: usize = 32;
/// `observe-sparse`: living slots, and the side of its square food lattice.
const SPARSE_LIVING: usize = 8;
const SPARSE_SIDE: usize = 512;
const SPARSE_HALF_SPAN: f64 = 65_625.0;
/// `observe-clustered`: slots on a 12-column head grid, four corner lattices of
/// this side and patch width within +-half span, and a sparse background.
const CLUSTERED_AGENTS: usize = 128;
const CLUSTERED_HEAD_STEP: f64 = 1600.0;
const CLUSTERED_SIDE: usize = 64;
const CLUSTERED_PATCH: f64 = 1200.0;
const CLUSTERED_HALF_SPAN: f64 = 12_000.0;
const CLUSTERED_BACKGROUND: usize = 512;
/// Short bodies keep snake filtering from hiding the food grid's cost.
const CLUSTERED_ANCHORS: usize = 2;
const USAGE: &str = "perf_observation CASE ITERATIONS [--trace]; CASE = observe-dense|observe-compact|observe-wide|observe-strip|observe-one|observe-empty|observe-sparse|observe-clustered; ITERATIONS = 1..100000";

struct Args {
    case: String,
    iterations: usize,
    trace: bool,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if !(2..=3).contains(&args.len())
            || !matches!(
                args[0].as_str(),
                "observe-dense"
                    | "observe-compact"
                    | "observe-wide"
                    | "observe-strip"
                    | "observe-one"
                    | "observe-empty"
                    | "observe-sparse"
                    | "observe-clustered"
            )
            || args.get(2).is_some_and(|option| option != "--trace")
        {
            return Err(USAGE.into());
        }
        let iterations = args[1].parse::<usize>().map_err(|_| USAGE.to_string())?;
        if !(1..=100_000).contains(&iterations) {
            return Err(USAGE.into());
        }
        Ok(Self {
            case: args[0].clone(),
            iterations,
            trace: args.len() == 3,
        })
    }
}

fn world(case: &str) -> Result<Simulator, String> {
    let (radius, food_count) = match case {
        "observe-compact" => (1000.0, 2000),
        "observe-wide" | "observe-strip" => (50_000.0, 12_000),
        "observe-sparse" => (100_000.0, 0),
        "observe-clustered" => (20_000.0, 0),
        _ => (6000.0, 12_000),
    };
    let mut config = Config {
        radius,
        food_count,
        vision: 1500.0,
        ..Config::sandbox()
    };
    config.prey.count = 8;
    // SYNTHETIC override: retain 2000 compact pellets to exercise batch indexing.
    if case == "observe-compact" {
        config.ambient_food.enabled = false;
    }
    let agents = agents(case);
    let mut world = runtime::ring(SEED, config, agents)?;
    let mut snakes = world.snakes().to_vec();
    let mut food = world.food().to_vec();
    let anchors = if case == "observe-clustered" {
        CLUSTERED_ANCHORS
    } else {
        ANCHORS
    };
    for (slot, snake) in snakes.iter_mut().enumerate() {
        if case == "observe-strip" {
            snake.head = Point {
                x: 0.0,
                y: (slot as f64 - 15.5) * 750.0,
            };
        }
        if case == "observe-sparse" {
            snake.head = Point::default();
        }
        if case == "observe-clustered" {
            let centre = 5.5 * CLUSTERED_HEAD_STEP;
            snake.head = Point {
                x: (slot % 12) as f64 * CLUSTERED_HEAD_STEP - centre,
                y: (slot / 12) as f64 * CLUSTERED_HEAD_STEP - centre,
            };
        }
        // Keep the dense construction identical to perf_baseline::core_world.
        snake.body = (0..anchors)
            .map(|anchor| Point {
                x: snake.head.x - snake.heading.cos() * anchor as f64 * physics::ANCHOR_SPACING,
                y: snake.head.y - snake.heading.sin() * anchor as f64 * physics::ANCHOR_SPACING,
            })
            .collect();
        snake.segments = snake.body.len();
        snake.fam = 0.75;
        snake.speed = physics::base_speed(snake.segments);
    }
    if case == "observe-strip" {
        // Remap admitted seed-7 pellets without consuming RNG or changing order,
        // values, sizes or origins. This bypasses density admission after remapping.
        let count = food.len();
        if count < 2 {
            return Err("Strip construction requires at least two admitted pellets".into());
        }
        for (index, food) in food.iter_mut().enumerate() {
            food.x = ((index * 37 + SEED as usize) % 151) as f64 - 75.0;
            food.y = -48_000.0 + 96_000.0 * index as f64 / (count - 1) as f64;
        }
    }
    if case == "observe-sparse" {
        // SYNTHETIC: eight observers share one head, and a lattice replaces the
        // (empty) startup food, so the grid has many cells for few observers.
        for snake in &mut snakes[SPARSE_LIVING..] {
            snake.alive = false;
        }
        let step = 2.0 * SPARSE_HALF_SPAN / (SPARSE_SIDE - 1) as f64;
        food = (0..SPARSE_SIDE * SPARSE_SIDE)
            .map(|i| Food {
                x: -SPARSE_HALF_SPAN + step * (i % SPARSE_SIDE) as f64,
                y: -SPARSE_HALF_SPAN + step * (i / SPARSE_SIDE) as f64,
                value: 1.0,
                size: None,
                origin: None,
            })
            .collect();
    }
    if case == "observe-clustered" {
        // SYNTHETIC: dense food only in corner patches beyond every observer's
        // vision, so mean density overstates what observers would skip.
        let step = CLUSTERED_PATCH / (CLUSTERED_SIDE - 1) as f64;
        let far = CLUSTERED_HALF_SPAN - CLUSTERED_PATCH;
        let patch = CLUSTERED_SIDE * CLUSTERED_SIDE;
        food = (0..4 * patch)
            .map(|i| {
                let (corner, at) = (i / patch, i % patch);
                let x0 = if corner % 2 == 0 {
                    -CLUSTERED_HALF_SPAN
                } else {
                    far
                };
                let y0 = if corner / 2 == 0 {
                    -CLUSTERED_HALF_SPAN
                } else {
                    far
                };
                Food {
                    x: x0 + step * (at % CLUSTERED_SIDE) as f64,
                    y: y0 + step * (at / CLUSTERED_SIDE) as f64,
                    value: 1.0,
                    size: None,
                    origin: None,
                }
            })
            .collect();
        let bins = (2.0 * CLUSTERED_HALF_SPAN / 100.0) as usize - 1;
        food.extend((0..CLUSTERED_BACKGROUND).map(|i| Food {
            x: 100.0 - CLUSTERED_HALF_SPAN + ((i * 127 + 13) % bins) as f64 * 100.0,
            y: 100.0 - CLUSTERED_HALF_SPAN + ((i * 73 + 31) % bins) as f64 * 100.0,
            value: 1.0,
            size: None,
            origin: None,
        }));
    }
    runtime::initial_snakes(&mut world, snakes)?;
    runtime::replace_food(&mut world, food)?;
    // Same telemetry path as the original harness, including in timing mode.
    runtime::record_events(&mut world);
    Ok(world)
}

fn agents(case: &str) -> usize {
    if case == "observe-clustered" {
        CLUSTERED_AGENTS
    } else {
        AGENTS
    }
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
        other => other,
    }
}

fn canonical_bytes(value: Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&canonical(value)).map_err(|e| e.to_string())
}

fn verify_scalar(
    world: &Simulator,
    selected: &[bool],
    output: &[Option<Observation>],
) -> Result<(), String> {
    // Public scalar observe bypasses the batch index in src/observation/mod.rs.
    // Do not replicate its visibility predicates in the harness.
    let reference = world
        .snakes()
        .iter()
        .enumerate()
        .map(|(slot, snake)| {
            if snake.alive && selected[slot] {
                world
                    .observe(slot)
                    .map(Some)
                    .map_err(|error| error.to_string())
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let output = serde_json::to_value(output).map_err(|e| e.to_string())?;
    let reference = serde_json::to_value(reference).map_err(|e| e.to_string())?;
    if canonical_bytes(output)? != canonical_bytes(reference)? {
        return Err("Batch/scalar canonical observation bytes differ".into());
    }
    Ok(())
}

fn snapshot(world: &Simulator) -> Result<Value, String> {
    let state = checkpoint_state::projection(world)?;
    let observations = world.observe_all();
    verify_scalar(world, &vec![true; world.snakes().len()], &observations)?;
    let pickups = world.pickup_events().as_ref().map(|events| {
        events
            .iter()
            .map(|event| {
                let item = match &event.item {
                    PickupItem::Food(food) => json!({"kind": "food", "value": food}),
                    PickupItem::Prey(prey) => json!({"kind": "prey", "value": prey}),
                };
                json!({"eater": event.eater, "item": item})
            })
            .collect::<Vec<_>>()
    });
    let births = world.food_spawn_events().as_ref().map(|events| {
        events
            .iter()
            .map(|event| json!({"source": event.source, "food": event.food}))
            .collect::<Vec<_>>()
    });
    Ok(
        json!({"world": state, "pickupEvents": pickups, "foodSpawnEvents": births, "observations": observations}),
    )
}

struct Meter {
    seconds: Vec<f64>,
}

impl Meter {
    fn report(mut self, scope: &str) -> Value {
        if self.seconds.is_empty() {
            return Value::Null;
        }
        let wall_seconds: f64 = self.seconds.iter().sum();
        self.seconds.sort_unstable_by(f64::total_cmp);
        let percentile =
            |percent: usize| self.seconds[(self.seconds.len() * percent).div_ceil(100) - 1];
        json!({
            "scope": scope,
            "wallSeconds": wall_seconds,
            "operationsPerSecond": if wall_seconds > 0.0 { Some(self.seconds.len() as f64 / wall_seconds) } else { None },
            "latencySeconds": {"p50": percentile(50), "p95": percentile(95), "p99": percentile(99), "max": self.seconds.last()},
        })
    }
}

fn line(out: &mut impl Write, value: Value) -> Result<(), String> {
    let bytes = canonical_bytes(value)?;
    out.write_all(&bytes).map_err(|e| e.to_string())?;
    out.write_all(b"\n").map_err(|e| e.to_string())
}

fn run() -> Result<(), String> {
    let args = Args::parse()?;
    // No stepping, resets, episodes or BotRunner: only immutable borrows below.
    let world = world(&args.case)?;
    let selected: Vec<_> = (0..agents(&args.case))
        .map(|slot| match args.case.as_str() {
            "observe-one" => slot == 0,
            "observe-empty" => false,
            _ => true,
        })
        .collect();
    let masked = matches!(args.case.as_str(), "observe-one" | "observe-empty");
    let observe = || {
        if masked {
            world.observe_selected(&selected)
        } else {
            Ok(world.observe_all())
        }
    };
    let capacity = if args.trace { 0 } else { args.iterations };
    let mut materialization = Meter {
        seconds: Vec::with_capacity(capacity),
    };
    let mut output_drop = Meter {
        seconds: Vec::with_capacity(capacity),
    };
    let mut out = BufWriter::new(io::stdout().lock());
    if args.trace {
        line(
            &mut out,
            json!({"kind": "initial", "case": args.case, "seed": SEED, "snapshot": snapshot(&world)?}),
        )?;
    }
    for iteration in 0..args.iterations {
        if args.trace {
            let output = observe().map_err(|error| error.to_string())?;
            black_box(&output);
            verify_scalar(&world, &selected, &output)?;
            line(
                &mut out,
                json!({"kind": "observe", "iteration": iteration, "snapshot": snapshot(&world)?, "result": output}),
            )?;
        } else {
            let start = Instant::now();
            let output = observe().map_err(|error| error.to_string())?;
            black_box(&output);
            let elapsed = start.elapsed().as_secs_f64();
            let start = Instant::now();
            drop(black_box(output));
            let drop_elapsed = start.elapsed().as_secs_f64();
            materialization.seconds.push(elapsed);
            output_drop.seconds.push(drop_elapsed);
        }
    }
    let anchors: Vec<_> = world
        .snakes()
        .iter()
        .map(|snake| snake.body.len())
        .collect();
    line(
        &mut out,
        json!({
            "kind": "report", "format": "slither-perf-baseline-v1", "case": args.case,
            "iterations": args.iterations, "seed": SEED, "release": !cfg!(debug_assertions),
            "mode": if args.trace { "trace" } else { "timing" },
            "operation": if masked { "observe_selected" } else { "observe_all" },
            "timing": materialization.report("sum of Instant call intervals, including index construction/filter/sort/copies and internal temporary destruction; detached output destruction, setup, sample storage, trace and report excluded; includes black_box borrow"),
            "outputDrop": output_drop.report("sum of separate Instant intervals destroying detached output Vec and owned observations via drop(black_box(output)); excludes materialization, setup, sample storage, trace and report"),
            "details": {
                "synthetic": true, "immutableWorld": true, "physicsTicks": 0,
                "config": world.config(), "actualCounts": WorldStats::from_world(&world),
                "foodPlacementAttempts": world.config().food_count,
                "bodyComplexity": {
                    "anchorsPerSlot": anchors, "totalAnchors": anchors.iter().sum::<usize>(),
                    "placement": if args.case == "observe-clustered" { "2 anchors at head - heading_unit * anchor_index * ANCHOR_SPACING; heading = slot / 128 * TAU + PI" } else { "32 anchors at head - heading_unit * anchor_index * ANCHOR_SPACING; heading = slot / 32 * TAU + PI" },
                    "anchorSpacing": physics::ANCHOR_SPACING, "fam": 0.75,
                    "massAndSpeed": if args.case == "observe-clustered" { "update_mass and base_speed(2); initialize_speed_state(slot, Some(0.0))" } else { "update_mass and base_speed(32); initialize_speed_state(slot, Some(0.0))" },
                    "heads": match args.case.as_str() { "observe-strip" => "x=0; y=(slot-15.5)*750; retain ring heading", "observe-sparse" => "slots 0-7 at the origin, retaining ring heading; slots 8-31 dead", "observe-clustered" => "128 slots; head = ((slot%12)*1600-8800, (slot/12)*1600-8800); retain ring heading", _ => "Simulator::add ring at radius/2" },
                    "compactCaveat": if args.case == "observe-compact" { Some("radial tails extend outside radius=1000; synthetic immutable observation geometry, never stepped") } else { None },
                },
                "maskPolicy": {"api": if masked { "observe_selected" } else { "observe_all" }, "selected": selected, "resultOrder": "stable slot order; dead/unselected entries null"},
                "construction": {
                    "denseReference": "examples/perf_baseline.rs::core_world(observe) at 7d33ebd; dense/one/empty have identical worlds",
                    "compactOverride": if args.case == "observe-compact" { Some("SYNTHETIC: ambient_food.enabled=false before Simulator::new, retaining 2000 startup attempts to trigger index; no package default changes") } else { None },
                    "sparseLattice": if args.case == "observe-sparse" { Some("SYNTHETIC: radius 100000 and food_count 0; then 512x512 pellets x,y = -65625 + i*131250/511 (row-major, value 1, no size or origin) replace world.food(), and slots 8-31 are marked dead") } else { None },
                    "clusteredPatches": if args.case == "observe-clustered" { Some("SYNTHETIC: radius 20000 and food_count 0; then four 64x64 lattices with step 1200/63 from corners x,y in {-12000, 10800} (corner c: x from c%2, y from c/2; row-major, value 1, no size or origin) replace world.food(), followed by 512 pellets x=-11900+((i*127+13)%239)*100, y=-11900+((i*73+31)%239)*100") } else { None },
                    "stripRemap": if args.case == "observe-strip" { Some("After seed-7 radius-50000 startup admission, remap each admitted food at insertion index i: x=((i*37+7)%151)-75; y=-48000+96000*i/(admitted_count-1). Retain values/sizes/origins/order and RNG state; no post-remap density cap.") } else { None },
                },
                "scalarParity": if args.trace { "canonical observation bytes checked against public scalar observe(slot), which bypasses the batch index; snapshots also check all slots" } else { "not checked in timing mode; run --trace separately" },
                "clock": "Instant wall intervals, not CPU or internal phase attribution; external process CPU/wall include setup, both intervals, loop/clock/sample overhead, serialization and teardown; trace also includes scalar checks",
                "latencyMethod": "nearest-rank p50/p95/p99/max; null throughput if summed intervals are zero; timing/outputDrop null in trace mode",
                "warmup": "none in process; caller owns untallied warmups and matched timing-window selection",
            },
        }),
    )?;
    out.flush().map_err(|e| e.to_string())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
