//! Run in release mode: perf_baseline CASE ITERATIONS [--profile|--trace|--blocks M] [--parallel].
//! `perf_baseline --list` prints the workload catalog as one canonical JSON line;
//! REGISTRATIONS below is the only case list. See docs/performance-workloads.md.
//! --profile is a separate timed step case; --trace emits untimed canonical JSONL.
//! No additional dependencies, sockets, files, or wall-clock bot processing samples.
use serde_json::{Value, json};
use slither_sim::{
    Action, Config, DeathEvent, DeathReason, Observation, Point, PreyConfig, Simulator, physics,
    profiling::{StepProfile, WorldStats},
    spawn,
};
use std::{
    collections::{BTreeMap, VecDeque},
    f64::consts::{FRAC_PI_2, PI, TAU},
    hint::black_box,
    io::{self, BufWriter, Write},
    time::Instant,
};

const SEED: u32 = 7;
const EPISODE_TICKS: usize = 1000;
const MAX_ITERATIONS: usize = 100_000;
/// Calibration repeats a case at most this often in one process.
const MAX_BLOCKS: usize = 16;
const PLACEMENT_ATTEMPTS: usize = 4096;
const RESPAWN_ATTEMPTS: usize = 64;
const RESPAWN_RETRY_TICKS: usize = 4;
/// Long samples restore the warmed scene so death history and drift stay bounded.
const EPOCH_OPERATIONS: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Suite {
    /// Timed and traced by PR CI.
    Ci,
    /// Local default only.
    Full,
}

impl Suite {
    fn label(self) -> &'static str {
        match self {
            Suite::Ci => "ci",
            Suite::Full => "full",
        }
    }
}

use Suite::{Ci, Full};

#[derive(Clone, Copy, Debug)]
struct Operation {
    /// Default timed sample size.
    iterations: usize,
    /// Default parity window; keeps each uncompressed trace bounded.
    trace_iterations: usize,
    suite: Suite,
}

const fn op(iterations: usize, trace_iterations: usize, suite: Suite) -> Operation {
    Operation {
        iterations,
        trace_iterations,
        suite,
    }
}

/// Historical cases. Archived reports cite their names, iterations and traces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Legacy {
    StepDefault,
    StepFood,
    StepBody,
    Observe,
    Portable,
    Protocol,
}

impl Legacy {
    fn name(self) -> &'static str {
        match self {
            Legacy::StepDefault => "step-default",
            Legacy::StepFood => "step-food",
            Legacy::StepBody => "step-body",
            Legacy::Observe => "observe",
            Legacy::Portable => "portable",
            Legacy::Protocol => "protocol",
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Legacy::StepDefault | Legacy::StepFood | Legacy::StepBody => "step",
            Legacy::Observe => "observe",
            Legacy::Portable => "step_all",
            Legacy::Protocol => "replay",
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum World {
    Sandbox,
    Official(usize),
    /// Synthetic stand-in for the future Battle Dome: a dense fixed arena whose
    /// only food comes from boosting and deaths.
    DomeProxy {
        radius: f64,
    },
}

impl World {
    fn config(self) -> Config {
        match self {
            World::Sandbox => Config::sandbox(),
            World::Official(snakes) => Config::official(snakes),
            World::DomeProxy { radius } => Config {
                radius,
                dynamic_radius: false,
                food_count: 0,
                ambient_replenish_per_second: 0.0,
                prey: PreyConfig {
                    count: 0,
                    ..PreyConfig::default()
                },
                ..Config::sandbox()
            },
        }
    }

    fn mode(self) -> &'static str {
        match self {
            World::Sandbox => "sandbox",
            World::Official(_) => "official",
            World::DomeProxy { .. } => "dome-proxy",
        }
    }

    fn synthetic(self) -> &'static str {
        match self {
            World::Sandbox | World::Official(_) => {
                "wander controller; straight drawn-length spawns and respawns"
            }
            World::DomeProxy { .. } => {
                "dome proxy: sandbox mechanics, no prey or ambient food, not Battle Dome rules; wander controller; straight drawn-length spawns and respawns"
            }
        }
    }

    fn definition(self) -> Value {
        match self {
            World::Sandbox => json!({"kind": "sandbox"}),
            World::Official(snakes) => json!({"kind": "official", "snakes": snakes}),
            World::DomeProxy { radius } => json!({"kind": "dome-proxy", "radius": radius}),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Controller {
    Wander,
}

/// A steady-state scene: deaths, death food and respawns happen during measurement.
struct Scenario {
    id: &'static str,
    world: World,
    population: usize,
    /// Inclusive segment-count range drawn for every placement and respawn.
    segments: (usize, usize),
    /// `Simulator::spawn_is_clear` centerline clearance; 0 is the capsule margin.
    clearance: f64,
    controller: Controller,
    warmup_ticks: usize,
    epoch: usize,
    /// `<id>-step`: one `Simulator::step`.
    step: Operation,
    /// `<id>-bot`: `step` then a batch observation into one reused buffer.
    bot: Option<Operation>,
}

/// In-process client lives on `SimulatorBackend`, timed through `step_all`.
struct ClientScenario {
    id: &'static str,
    world: World,
    agents: usize,
    warmup_decisions: usize,
    operation: Operation,
}

enum Registration {
    Legacy(Legacy, Operation),
    Scenario(Scenario),
    Client(ClientScenario),
}

/// New sample sizes target roughly one second per release sample on the development machine.
static REGISTRATIONS: &[Registration] = &[
    Registration::Legacy(Legacy::StepDefault, op(100_000, 32, Ci)),
    Registration::Legacy(Legacy::StepFood, op(10_000, 32, Ci)),
    Registration::Legacy(Legacy::StepBody, op(50_000, 32, Ci)),
    Registration::Legacy(Legacy::Observe, op(1000, 32, Ci)),
    Registration::Legacy(Legacy::Portable, op(5000, 32, Ci)),
    Registration::Legacy(Legacy::Protocol, op(5000, 32, Ci)),
    Registration::Scenario(Scenario {
        id: "sandbox-16",
        world: World::Sandbox,
        population: 16,
        segments: (4, 24),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 2000,
        epoch: EPOCH_OPERATIONS,
        step: op(30_000, 32, Full),
        bot: Some(op(2000, 24, Full)),
    }),
    Registration::Scenario(Scenario {
        id: "official-30",
        world: World::Official(30),
        population: 30,
        segments: (10, 60),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 2000,
        epoch: EPOCH_OPERATIONS,
        step: op(10_000, 32, Full),
        bot: Some(op(800, 16, Full)),
    }),
    Registration::Scenario(Scenario {
        id: "official-100",
        world: World::Official(100),
        population: 100,
        segments: (10, 80),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 2000,
        epoch: EPOCH_OPERATIONS,
        step: op(1000, 16, Ci),
        bot: Some(op(150, 4, Ci)),
    }),
    Registration::Scenario(Scenario {
        id: "official-200",
        world: World::Official(200),
        population: 200,
        segments: (10, 80),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 1000,
        epoch: EPOCH_OPERATIONS,
        step: op(300, 8, Full),
        bot: Some(op(80, 2, Full)),
    }),
    Registration::Scenario(Scenario {
        id: "official-500",
        world: World::Official(500),
        population: 500,
        segments: (10, 80),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 600,
        epoch: EPOCH_OPERATIONS,
        step: op(100, 4, Full),
        bot: Some(op(20, 1, Full)),
    }),
    Registration::Scenario(Scenario {
        id: "dome-proxy-32",
        world: World::DomeProxy { radius: 1500.0 },
        population: 32,
        segments: (8, 24),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 2000,
        epoch: EPOCH_OPERATIONS,
        step: op(20_000, 32, Ci),
        bot: Some(op(400, 8, Ci)),
    }),
    Registration::Scenario(Scenario {
        id: "dome-proxy-96",
        world: World::DomeProxy { radius: 2500.0 },
        population: 96,
        segments: (8, 24),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 2000,
        epoch: EPOCH_OPERATIONS,
        step: op(4000, 32, Full),
        bot: Some(op(130, 2, Full)),
    }),
    Registration::Scenario(Scenario {
        id: "official-128",
        world: World::Official(128),
        population: 128,
        segments: (10, 80),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 2000,
        epoch: EPOCH_OPERATIONS,
        step: op(7000, 16, Full),
        bot: Some(op(800, 3, Full)),
    }),
    Registration::Client(ClientScenario {
        id: "client-official-100",
        world: World::Official(100),
        agents: 100,
        warmup_decisions: 200,
        operation: op(80, 4, Full),
    }),
];

#[derive(Clone, Copy)]
enum Driver {
    Legacy(Legacy),
    Scenario {
        scenario: &'static Scenario,
        bot: bool,
    },
    Client(&'static ClientScenario),
}

impl Driver {
    fn mode(self) -> &'static str {
        match self {
            Driver::Legacy(Legacy::Portable | Legacy::Protocol) | Driver::Client(_) => "client",
            Driver::Legacy(_) => "sandbox",
            Driver::Scenario { scenario, .. } => scenario.world.mode(),
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Driver::Legacy(case) => case.operation(),
            Driver::Scenario { bot: false, .. } => "step",
            Driver::Scenario { bot: true, .. } => "step+observe",
            Driver::Client(_) => "step_all",
        }
    }

    fn profile(self) -> bool {
        matches!(
            self,
            Driver::Legacy(Legacy::StepDefault | Legacy::StepFood | Legacy::StepBody)
                | Driver::Scenario { .. }
        )
    }

    fn networking(self) -> bool {
        matches!(
            self,
            Driver::Legacy(Legacy::Portable | Legacy::Protocol) | Driver::Client(_)
        )
    }

    fn synthetic(self) -> Option<&'static str> {
        match self {
            Driver::Legacy(_) => None,
            Driver::Scenario { scenario, .. } => Some(scenario.world.synthetic()),
            Driver::Client(_) => Some("wander controller; backend ring start and reconnects"),
        }
    }

    fn definition(self) -> Result<Value, String> {
        Ok(match self {
            Driver::Legacy(Legacy::Portable | Legacy::Protocol) => json!({
                "config": Config::sandbox(), "simulation": simulation_definition(self)?,
            }),
            Driver::Legacy(case) => {
                let (agents, config) = core_config(case.name())?;
                json!({"agents": agents, "config": config})
            }
            Driver::Scenario { scenario, bot } => json!({
                "world": scenario.world.definition(), "config": scenario.world.config(),
                "population": scenario.population,
                "segments": {"min": scenario.segments.0, "max": scenario.segments.1},
                "clearance": scenario.clearance, "controller": scenario.controller.definition(),
                "lifecycle": {
                    "placement": "add, mark dead, then Simulator::respawn at a clear sampled pose",
                    "respawn": "dead slots queue in death order and respawn at the next operation boundary",
                    "placementAttempts": PLACEMENT_ATTEMPTS, "respawnAttempts": RESPAWN_ATTEMPTS,
                    "respawnRetryTicks": RESPAWN_RETRY_TICKS,
                    "epochOperations": scenario.epoch,
                },
                "warmupTicks": scenario.warmup_ticks,
                "operation": if bot { "step+batch_observe" } else { "step" },
            }),
            Driver::Client(client) => json!({
                "world": client.world.definition(), "config": client.world.config(),
                "agents": client.agents, "simulation": simulation_definition(self)?,
                "controller": Controller::Wander.definition(),
                "lifecycle": {"reconnect": "reconnect_slot after the client reports death; failed spawns retry next decision"},
                "warmupDecisions": client.warmup_decisions,
            }),
        })
    }
}

#[cfg(feature = "networking")]
fn simulation_definition(driver: Driver) -> Result<Value, String> {
    let simulation = match driver {
        Driver::Client(client) => portable::client_simulation(client),
        _ => portable::simulation(true),
    };
    serde_json::to_value(simulation).map_err(|e| e.to_string())
}

#[cfg(not(feature = "networking"))]
fn simulation_definition(_: Driver) -> Result<Value, String> {
    Ok(Value::Null)
}

struct Workload {
    name: String,
    driver: Driver,
    operation: Operation,
}

fn catalog() -> Vec<Workload> {
    let mut workloads = Vec::new();
    for registration in REGISTRATIONS {
        match registration {
            Registration::Legacy(case, operation) => workloads.push(Workload {
                name: case.name().into(),
                driver: Driver::Legacy(*case),
                operation: *operation,
            }),
            Registration::Scenario(scenario) => {
                workloads.push(Workload {
                    name: format!("{}-step", scenario.id),
                    driver: Driver::Scenario {
                        scenario,
                        bot: false,
                    },
                    operation: scenario.step,
                });
                if let Some(operation) = scenario.bot {
                    workloads.push(Workload {
                        name: format!("{}-bot", scenario.id),
                        driver: Driver::Scenario {
                            scenario,
                            bot: true,
                        },
                        operation,
                    });
                }
            }
            Registration::Client(client) => workloads.push(Workload {
                name: client.id.into(),
                driver: Driver::Client(client),
                operation: client.operation,
            }),
        }
    }
    workloads
}

fn catalog_json() -> Result<Value, String> {
    let workloads = catalog()
        .into_iter()
        .map(|workload| {
            let driver = workload.driver;
            Ok(json!({
                "available": cfg!(feature = "networking") || !driver.networking(),
                "definition": driver.definition()?, "iterations": workload.operation.iterations,
                "legacy": matches!(driver, Driver::Legacy(_)), "mode": driver.mode(),
                "name": workload.name, "operation": driver.operation(), "profile": driver.profile(),
                "suite": workload.operation.suite.label(), "synthetic": driver.synthetic(),
                "traceIterations": workload.operation.trace_iterations,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(json!({
        "format": "slither-perf-catalog-v1", "kind": "catalog",
        "networking": cfg!(feature = "networking"), "seed": SEED, "workloads": workloads,
    }))
}

struct Args {
    case: String,
    iterations: usize,
    profile: bool,
    trace: bool,
    /// `Simulator::set_parallel(true)` on the timed world.
    parallel: bool,
    /// Reset-identical timed blocks in this process; None is the ordinary single run.
    blocks: Option<usize>,
    driver: Driver,
    operation: Operation,
}

enum Command {
    List,
    Run(Args),
}

impl Args {
    fn parse() -> Result<Command, String> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if args == ["--list"] {
            return Ok(Command::List);
        }
        let workloads = catalog();
        let usage = || {
            let names: Vec<_> = workloads.iter().map(|w| w.name.as_str()).collect();
            format!(
                "perf_baseline CASE ITERATIONS [--profile|--trace|--blocks M] [--parallel] | perf_baseline --list; CASE = {}; ITERATIONS = 1..{MAX_ITERATIONS}; M = 1..{MAX_BLOCKS}; --profile requires a profiled case (see --list); --parallel requires the parallel feature",
                names.join("|")
            )
        };
        if !(2..=5).contains(&args.len()) {
            return Err(usage());
        }
        let workload = workloads
            .iter()
            .find(|workload| workload.name == args[0])
            .ok_or_else(usage)?;
        let iterations = args[1].parse::<usize>().map_err(|_| usage())?;
        if !(1..=MAX_ITERATIONS).contains(&iterations) {
            return Err(usage());
        }
        let (mut option, mut parallel, mut blocks) = (None, false, None);
        let mut rest = args[2..].iter();
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--parallel" if !parallel => parallel = true,
                "--profile" | "--trace" if option.is_none() => option = Some(arg.as_str()),
                "--blocks" if option.is_none() => {
                    option = Some("--blocks");
                    let count = rest.next().and_then(|count| count.parse::<usize>().ok());
                    blocks = count.filter(|count| (1..=MAX_BLOCKS).contains(count));
                    if blocks.is_none() {
                        return Err(usage());
                    }
                }
                _ => return Err(usage()),
            }
        }
        if parallel && !cfg!(feature = "parallel") {
            return Err(usage());
        }
        let profile = option == Some("--profile");
        if profile && !workload.driver.profile() {
            return Err(usage());
        }
        Ok(Command::Run(Self {
            case: workload.name.clone(),
            iterations,
            profile,
            trace: option == Some("--trace"),
            parallel,
            blocks,
            driver: workload.driver,
            operation: workload.operation,
        }))
    }
}

#[derive(Default)]
struct Meter {
    seconds: Vec<f64>,
}

impl Meter {
    fn operation<T>(
        &mut self,
        trace: bool,
        f: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        if trace {
            return f();
        }
        let start = Instant::now();
        let result = f();
        black_box(&result);
        let seconds = start.elapsed().as_secs_f64();
        let output = result?;
        self.seconds.push(seconds);
        Ok(output)
    }

    fn report(mut self) -> Value {
        if self.seconds.is_empty() {
            return Value::Null;
        }
        let wall_seconds: f64 = self.seconds.iter().sum();
        self.seconds.sort_unstable_by(f64::total_cmp);
        // Nearest-rank percentiles, including the single-operation case.
        let percentile =
            |percent: usize| self.seconds[(self.seconds.len() * percent).div_ceil(100) - 1];
        json!({
            "scope": "sum of operation intervals; setup, controls, reset, output drop and serialization excluded",
            "wallSeconds": wall_seconds,
            "operations": self.seconds.len(),
            "operationsPerSecond": if wall_seconds > 0.0 { Some(self.seconds.len() as f64 / wall_seconds) } else { None },
            "latencySeconds": {"p50": percentile(50), "p95": percentile(95), "p99": percentile(99), "max": self.seconds.last()},
        })
    }
}

struct Progress {
    ticks: usize,
    episode_ticks: usize,
    resets: usize,
    min_living: usize,
    min_food: usize,
    deaths: usize,
    previous_deaths: usize,
}

impl Progress {
    fn new(world: &Simulator) -> Self {
        Self {
            ticks: 0,
            episode_ticks: 0,
            resets: 0,
            min_living: world.living(),
            min_food: world.food.len(),
            deaths: 0,
            previous_deaths: world.deaths.len(),
        }
    }

    fn needs_reset(&self, world: &Simulator, next_ticks: usize) -> bool {
        self.episode_ticks + next_ticks > EPISODE_TICKS || world.living() < world.snakes.len()
    }

    fn reset(&mut self) {
        self.resets += 1;
        self.episode_ticks = 0;
        self.previous_deaths = 0;
    }

    fn record(&mut self, world: &Simulator, ticks: usize) {
        self.ticks += ticks;
        self.episode_ticks += ticks;
        self.min_living = self.min_living.min(world.living());
        self.min_food = self.min_food.min(world.food.len());
        self.deaths += world.deaths.len() - self.previous_deaths;
        self.previous_deaths = world.deaths.len();
    }

    fn report(&self) -> Value {
        json!({
            "physicsTicks": self.ticks, "finalEpisodeTicks": self.episode_ticks,
            "episodeLimitPhysicsTicks": EPISODE_TICKS, "resets": self.resets,
            "resetPolicy": "before next operation if any snake died or episode would exceed tick limit",
            "minimumLivingAtOperationBoundaries": self.min_living,
            "minimumFoodAtOperationBoundaries": self.min_food, "deathsAcrossEpisodes": self.deaths,
        })
    }
}

fn controls(world: &Simulator, centers: &[Point], orbit: f64, tick: usize) -> Vec<Option<Action>> {
    world
        .snakes
        .iter()
        .zip(centers)
        .map(|(snake, center)| {
            snake.alive.then(|| {
                let (x, y) = (snake.head.x - center.x, snake.head.y - center.y);
                Action {
                    heading: y.atan2(x)
                        + FRAC_PI_2
                        + ((x.hypot(y) - orbit) / orbit).clamp(-0.6, 0.6),
                    boost: tick % 128 < 16,
                }
            })
        })
        .collect()
}

fn core_config(case: &str) -> Result<(usize, Config), String> {
    let (agents, mut config) = match case {
        "step-default" => (4, Config::sandbox()),
        "step-food" | "observe" => (
            32,
            Config {
                radius: 6000.0,
                food_count: 12_000,
                ..Config::sandbox()
            },
        ),
        "step-body" => (
            64,
            Config {
                radius: 12_000.0,
                food_count: 350,
                ..Config::sandbox()
            },
        ),
        _ => return Err("Unknown core case".into()),
    };
    if case == "observe" {
        config.prey.count = 8;
    }
    Ok((agents, config))
}

fn core_world(case: &str) -> Result<(Simulator, Vec<Point>, f64), String> {
    let (agents, config) = core_config(case)?;
    let mut world = Simulator::new(SEED, config)?;
    let mut centers = vec![Point::default(); agents];
    let orbit = if case == "step-body" {
        400.0
    } else {
        world.config.radius * 0.5
    };
    for (slot, center) in centers.iter_mut().enumerate() {
        world.add(slot.to_string(), slot, agents)?;
        if case == "step-body" || case == "observe" {
            let snake = &mut world.snakes[slot];
            if case == "step-body" {
                let origin = Point {
                    x: (slot % 8) as f64 * 2100.0 - 7350.0,
                    y: (slot / 8) as f64 * 2100.0 - 7350.0,
                };
                snake.head = Point {
                    x: origin.x - 315.0,
                    y: origin.y - 147.0,
                };
                snake.heading = PI;
                *center = Point {
                    x: snake.head.x,
                    y: snake.head.y - orbit,
                };
                // A nonintersecting 16x8 serpentine has exactly 42-unit edges.
                // Separate 2100-unit cells leave room for the subsequent local orbits.
                snake.body = (0..128)
                    .map(|anchor| {
                        let row = anchor / 16;
                        let column = if row % 2 == 0 {
                            anchor % 16
                        } else {
                            15 - anchor % 16
                        };
                        Point {
                            x: snake.head.x + column as f64 * physics::ANCHOR_SPACING,
                            y: snake.head.y + row as f64 * physics::ANCHOR_SPACING,
                        }
                    })
                    .collect();
            } else {
                snake.body = (0..32)
                    .map(|anchor| Point {
                        x: snake.head.x
                            - snake.heading.cos() * anchor as f64 * physics::ANCHOR_SPACING,
                        y: snake.head.y
                            - snake.heading.sin() * anchor as f64 * physics::ANCHOR_SPACING,
                    })
                    .collect();
            }
            snake.segments = snake.body.len();
            snake.fam = 0.75;
            snake.speed = physics::base_speed(snake.segments);
            Simulator::update_mass(snake);
            world.initialize_speed_state(slot, Some(0.0))?;
        }
    }
    // Keep timing and trace workloads on the same telemetry path. Events are
    // explicitly included in traces because Simulator's serde representation skips them.
    world.pickup_events = Some(Vec::new());
    world.food_spawn_events = Some(Vec::new());
    Ok((world, centers, orbit))
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

fn pickups(world: &Simulator) -> Option<Vec<Value>> {
    world.pickup_events.as_ref().map(|events| {
        events
            .iter()
            .map(|event| {
                let item = match &event.item {
                    slither_sim::PickupItem::Food(food) => json!({"kind": "food", "value": food}),
                    slither_sim::PickupItem::Prey(prey) => json!({"kind": "prey", "value": prey}),
                };
                json!({"eater": event.eater, "item": item})
            })
            .collect()
    })
}

fn births(world: &Simulator) -> Option<Vec<Value>> {
    world.food_spawn_events.as_ref().map(|events| {
        events
            .iter()
            .map(|event| json!({"source": event.source, "food": event.food}))
            .collect()
    })
}

fn world_events(world: &Simulator) -> Result<Value, String> {
    let mut state = serde_json::to_value(world).map_err(|e| e.to_string())?;
    // This private HashSet is semantically unordered; every ordered array, including
    // food, snakes, deaths and event buffers, retains its original order.
    if let Some(outward) = state.get_mut("prey_outward").and_then(Value::as_array_mut) {
        outward.sort_unstable_by_key(Value::as_u64);
    }
    Ok(json!({"world": state, "pickupEvents": pickups(world), "foodSpawnEvents": births(world)}))
}

fn world_trace(world: &Simulator) -> Result<Value, String> {
    let mut trace = world_events(world)?;
    trace["observations"] = json!(world.observe_all());
    Ok(trace)
}

/// `perf_compare --base` builds this harness against older baselines too. Inherent
/// methods win method resolution, so this runs only where `Simulator::observe_all_into`
/// is missing, and bot rows there time the allocating loop bots had to use.
#[allow(dead_code)]
trait ObserveAllInto {
    fn observe_all_into(&self, observations: &mut Vec<Option<Observation>>);
}

impl ObserveAllInto for Simulator {
    fn observe_all_into(&self, observations: &mut Vec<Option<Observation>>) {
        *observations = self.observe_all();
    }
}

/// Clones of the world keep the setting, including epoch and reset templates.
fn set_parallel(world: &mut Simulator, args: &Args) {
    #[cfg(feature = "parallel")]
    world.set_parallel(args.parallel);
    #[cfg(not(feature = "parallel"))]
    let _ = (world, args);
}

fn line(out: &mut impl Write, value: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *out, &canonical(value)).map_err(|e| e.to_string())?;
    writeln!(out).map_err(|e| e.to_string())
}

fn report(args: &Args, meter: Meter, operation: &str, details: Value) -> Value {
    let mut report = json!({
        "kind": "report", "format": "slither-perf-baseline-v1", "case": args.case,
        "iterations": args.iterations, "seed": SEED, "release": !cfg!(debug_assertions),
        "mode": if args.trace { "trace" } else if args.profile { "profile" } else { "timing" },
        "operation": operation, "timing": meter.report(), "details": details,
    });
    // Present only when set, so default trace bytes stay comparable with older harnesses.
    if args.parallel {
        report["parallel"] = json!(true);
    }
    report
}

fn run_core(args: &Args, out: &mut impl Write) -> Result<Value, String> {
    let (mut world, centers, orbit) = core_world(&args.case)?;
    set_parallel(&mut world, args);
    let initial = WorldStats::from_world(&world);
    let template = world.clone();
    let mut progress = Progress::new(&world);
    let mut meter = Meter::default();
    let mut profile = StepProfile::default();
    let observe = args.case == "observe";
    if args.trace {
        line(
            out,
            json!({"kind": "initial", "case": args.case, "seed": SEED, "snapshot": world_trace(&world)?}),
        )?;
    }
    for iteration in 0..args.iterations {
        if !observe && progress.needs_reset(&world, 1) {
            world = template.clone();
            progress.reset();
            if args.trace {
                line(
                    out,
                    json!({"kind": "reset", "iteration": iteration, "episode": progress.resets, "snapshot": world_trace(&world)?}),
                )?;
            }
        }
        let actions = if observe {
            Vec::new()
        } else {
            controls(&world, &centers, orbit, progress.episode_ticks)
        };
        if observe {
            let observations = meter.operation(args.trace, || Ok(world.observe_all()))?;
            black_box(&observations);
            if args.trace {
                line(
                    out,
                    json!({"kind": "observe", "iteration": iteration, "snapshot": world_trace(&world)?, "result": observations}),
                )?;
            }
        } else {
            meter.operation(args.trace, || {
                if args.profile {
                    world.step_profiled(&actions, &mut profile)
                } else {
                    world.step(&actions)
                }
            })?;
            black_box(&world);
            if args.trace {
                line(
                    out,
                    json!({"kind": "step", "iteration": iteration, "episode": progress.resets, "episodeTick": progress.episode_ticks + 1, "actions": actions, "snapshot": world_trace(&world)?}),
                )?;
            }
        }
        progress.record(&world, usize::from(!observe));
    }
    Ok(report(
        args,
        meter,
        if observe {
            "observe_all"
        } else if args.profile {
            "step_profiled"
        } else {
            "step"
        },
        json!({
            "initial": initial, "final": WorldStats::from_world(&world), "progress": progress.report(),
            "config": world.config, "eventBuffersEnabled": true,
            "control": "own-head tangent with radial correction; boost held for 16 of 128 episode ticks",
            "bodyShape": if args.case == "step-body" { "64 separated 128-anchor serpentine bodies; local 400-unit orbits" } else if observe { "32 radial 32-anchor bodies and eight prey; immutable world" } else { "Simulator::add ring starts" },
            "profile": if args.profile { Some(profile) } else { None },
        }),
    ))
}

#[cfg(feature = "networking")]
mod portable {
    use super::*;
    use slither_sim::client::{
        ClientFrame, ClientState, ClientStatus, SimulationConfig, SimulatorBackend, WireRecord,
        replay_record,
    };

    const AGENTS: usize = 8;
    const DECISION_TICKS: usize = 6;
    const TAPE_GROUPS: usize = 8;

    pub(super) fn simulation(record_wire: bool) -> SimulationConfig {
        SimulationConfig {
            agents: AGENTS,
            record_wire,
            world: Config::sandbox(),
            ..SimulationConfig::default()
        }
    }

    fn backend(record_wire: bool) -> Result<SimulatorBackend, String> {
        SimulatorBackend::new(SEED, simulation(record_wire))
    }

    #[test]
    fn client_workload_retains_the_compact_fixture() {
        let backend = backend(false).unwrap();
        let world = &backend.world().config;
        assert_eq!(world.radius, 1000.);
        assert!(!world.dynamic_radius);
        assert_eq!(world.food_count, 350);
        assert_eq!(world.prey.count, 0);
        assert_eq!(backend.slots(), 8);
    }

    fn frames(backend: &SimulatorBackend) -> Result<Vec<ClientFrame>, String> {
        (0..backend.slots())
            .map(|slot| backend.frame_for(slot))
            .collect()
    }

    fn actions(
        backend: &SimulatorBackend,
        frames: &[ClientFrame],
        tick: usize,
    ) -> Vec<Option<Action>> {
        let centers = [Point::default(); AGENTS];
        let mut actions = controls(
            backend.world(),
            &centers,
            backend.world().config.radius * 0.5,
            tick,
        );
        for (action, frame) in actions.iter_mut().zip(frames) {
            if frame.status != ClientStatus::Playing {
                *action = None;
            }
        }
        actions
    }

    fn records(backend: &mut SimulatorBackend) -> Result<Vec<Vec<WireRecord>>, String> {
        (0..backend.slots())
            .map(|slot| backend.drain_records(slot))
            .collect()
    }

    pub(super) fn run(args: &Args, out: &mut impl Write) -> Result<Value, String> {
        // Recording is enabled in both modes: parity tracing does not switch the
        // measured backend onto a different record-allocation path.
        let mut backend = backend(true)?;
        let mut current_frames = frames(&backend)?;
        let initial = WorldStats::from_world(backend.world());
        let config = backend.config().clone();
        let mut progress = Progress::new(backend.world());
        let mut meter = Meter::default();
        let initial_records = records(&mut backend)?;
        if args.trace {
            line(
                out,
                json!({"kind": "initial", "case": args.case, "seed": SEED, "frames": current_frames, "wireRecords": initial_records, "snapshot": world_trace(backend.world())?}),
            )?;
        }
        for iteration in 0..args.iterations {
            if progress.needs_reset(backend.world(), DECISION_TICKS) {
                backend = self::backend(true)?;
                current_frames = frames(&backend)?;
                progress.reset();
                let wire = records(&mut backend)?;
                if args.trace {
                    line(
                        out,
                        json!({"kind": "reset", "iteration": iteration, "episode": progress.resets, "frames": current_frames, "wireRecords": wire, "snapshot": world_trace(backend.world())?}),
                    )?;
                }
            }
            let actions = actions(&backend, &current_frames, progress.episode_ticks);
            current_frames = meter.operation(args.trace, || backend.step_all(&actions))?;
            black_box(&current_frames);
            let wire = records(&mut backend)?;
            black_box(&wire);
            progress.record(backend.world(), DECISION_TICKS);
            if args.trace {
                line(
                    out,
                    json!({"kind": "decision", "iteration": iteration, "episode": progress.resets, "episodeTick": progress.episode_ticks, "actions": actions, "frames": current_frames, "wireRecords": wire, "snapshot": world_trace(backend.world())?}),
                )?;
            }
        }
        Ok(report(
            args,
            meter,
            "step_all (six physics ticks and all client frames)",
            json!({
                "initial": initial, "final": WorldStats::from_world(backend.world()), "progress": progress.report(),
                "config": config, "wireRecording": true, "recordDrainTimed": false,
                "frameScope": "all eight slots; no BotRunner or automatic background controller",
            }),
        ))
    }

    struct Group {
        at: f64,
        records: Vec<WireRecord>,
    }

    struct Tape {
        groups: Vec<Group>,
        initial: WorldStats,
        final_world: WorldStats,
        config: SimulationConfig,
    }

    fn tape() -> Result<Tape, String> {
        let mut backend = backend(true)?;
        let mut current_frames = frames(&backend)?;
        let initial = WorldStats::from_world(backend.world());
        let config = backend.config().clone();
        let mut groups = vec![Group {
            at: backend.time(),
            records: records(&mut backend)?.remove(0),
        }];
        for group in 0..TAPE_GROUPS {
            let actions = actions(&backend, &current_frames, group * DECISION_TICKS);
            current_frames = backend.step_all(&actions)?;
            groups.push(Group {
                at: backend.time(),
                records: records(&mut backend)?.remove(0),
            });
        }
        if backend.world().living() != AGENTS {
            return Err("Synthetic protocol tape lost a snake during setup".into());
        }
        Ok(Tape {
            groups,
            initial,
            final_world: WorldStats::from_world(backend.world()),
            config,
        })
    }

    fn replay(state: &mut ClientState, tape: &Tape) -> Result<Vec<ClientFrame>, String> {
        tape.groups
            .iter()
            .map(|group| {
                for record in &group.records {
                    replay_record(state, record)?;
                }
                state.frame(group.at)
            })
            .collect()
    }

    fn received_counts(frame: &ClientFrame) -> Value {
        let observation = frame.observation.as_ref();
        json!({
            "status": frame.status, "localTime": frame.time, "life": frame.life,
            "knownSnakeRecords": frame.received.snakes.len(),
            "livingVisible": observation.map_or(0, |o| usize::from(o.me.alive) + o.opponents.iter().filter(|s| s.alive).count()),
            "bodyAnchors": observation.map_or(0, |o| o.me.body.len() + o.opponents.iter().map(|s| s.body.len()).sum::<usize>()),
            "food": observation.map_or(0, |o| o.food.len()), "prey": observation.map_or(0, |o| o.prey.len()),
        })
    }

    pub(super) fn run_protocol(args: &Args, out: &mut impl Write) -> Result<Value, String> {
        // Tape generation and each fresh decoder's construction are outside timing.
        let tape = tape()?;
        let record_count: usize = tape.groups.iter().map(|group| group.records.len()).sum();
        let wire_bytes: usize = tape
            .groups
            .iter()
            .flat_map(|group| &group.records)
            .map(|record| record.bytes.len())
            .sum();
        let initial = received_counts(&ClientState::new().frame(0.0)?);
        let mut final_counts = Value::Null;
        let mut meter = Meter::default();
        for iteration in 0..args.iterations {
            let mut state = ClientState::new();
            let output = meter.operation(args.trace, || replay(&mut state, &tape))?;
            black_box(&output);
            final_counts = received_counts(output.last().ok_or("Empty synthetic tape")?);
            if args.trace {
                for (group, (input, frame)) in tape.groups.iter().zip(&output).enumerate() {
                    line(
                        out,
                        json!({"kind": "replay", "iteration": iteration, "group": group, "at": input.at, "wireRecords": input.records, "frame": frame}),
                    )?;
                }
            }
        }
        Ok(report(
            args,
            meter,
            "fresh-decoder replay of eight decision groups plus initial connect; frame after each group",
            json!({
                "initialReceived": initial, "finalReceived": final_counts,
                "physicsTicksPerformedInMeasuredRegion": 0, "decoderResets": args.iterations,
                "tape": {"synthetic": true, "sourceSlot": 0, "decisionGroups": TAPE_GROUPS,
                    "sourcePhysicsTicks": TAPE_GROUPS * DECISION_TICKS, "records": record_count, "wireBytes": wire_bytes,
                    "framesPerIteration": tape.groups.len(), "initialWorld": tape.initial, "finalWorld": tape.final_world, "config": tape.config},
                "processingSeconds": 0.0, "processingSource": "fixed zero; no BotRunner measurements",
            }),
        ))
    }

    pub(super) fn client_simulation(client: &ClientScenario) -> SimulationConfig {
        SimulationConfig {
            agents: client.agents,
            record_wire: true,
            world: client.world.config(),
            ..SimulationConfig::default()
        }
    }

    fn wire_summary(wire: &[Vec<WireRecord>]) -> Value {
        let records: usize = wire.iter().map(Vec::len).sum();
        let bytes: usize = wire.iter().flatten().map(|record| record.bytes.len()).sum();
        json!({"records": records, "bytes": bytes})
    }

    pub(super) fn run_client(
        args: &Args,
        client: &ClientScenario,
        out: &mut impl Write,
    ) -> Result<Value, String> {
        let simulation = client_simulation(client);
        let decision_ticks = simulation.decision_ticks;
        let mut backend = SimulatorBackend::new(SEED, simulation)?;
        let death_food = backend.world().config.death_food.enabled;
        let mut cursor = backend.world().deaths.len();
        let mut current_frames = frames(&backend)?;
        let initial = WorldStats::from_world(backend.world());
        let wire = records(&mut backend)?;
        if args.trace {
            line(
                out,
                json!({"kind": "initial", "case": args.case, "seed": SEED, "frames": current_frames, "wireRecords": wire, "snapshot": world_events(backend.world())?}),
            )?;
        }
        let window = args.iterations.min(args.operation.trace_iterations);
        let mut tick = 0;
        let mut warm = Lifecycle::default();
        let mut lifecycle = Lifecycle::default();
        let mut warm_population = PopulationStats::new(client.agents);
        let mut population = PopulationStats::new(client.agents);
        let mut warmed = Value::Null;
        let mut validity = Value::Null;
        let mut meter = Meter::default();
        for decision in 0..client.warmup_decisions + args.iterations {
            let measured = decision >= client.warmup_decisions;
            if decision == client.warmup_decisions {
                warmed = json!(WorldStats::from_world(backend.world()));
                if args.trace {
                    line(
                        out,
                        json!({"kind": "warmed", "decisions": decision, "lifecycle": warm.json(), "frames": current_frames, "snapshot": world_events(backend.world())?}),
                    )?;
                }
            }
            // Lives restart only after their client has seen the death, as a bot would.
            let (mut reconnects, mut deferred) = (Vec::new(), Vec::new());
            for (slot, frame) in current_frames.iter().enumerate() {
                if matches!(
                    frame.status,
                    ClientStatus::Dead | ClientStatus::Disconnected
                ) {
                    match backend.reconnect_slot(slot) {
                        Ok(()) => reconnects.push(slot),
                        Err(error) if error == "No clear spawn position" => deferred.push(slot),
                        Err(error) => return Err(error),
                    }
                }
            }
            let mut actions = Controller::Wander.actions(backend.world(), tick);
            for (slot, action) in actions.iter_mut().enumerate() {
                // step_all rejects the whole batch for a slot that cannot take a decision.
                if current_frames[slot].status != ClientStatus::Playing
                    || backend.decision_pending(slot)?
                {
                    *action = None;
                }
            }
            current_frames = if measured {
                meter.operation(args.trace, || backend.step_all(&actions))?
            } else {
                backend.step_all(&actions)?
            };
            black_box(&current_frames);
            tick += decision_ticks;
            let wire = records(&mut backend)?;
            black_box(&wire);
            let world = backend.world();
            let deaths = world.deaths[cursor..].to_vec();
            cursor = world.deaths.len();
            let counts = if measured { &mut lifecycle } else { &mut warm };
            counts.deaths(&deaths);
            counts.respawns += reconnects.len();
            counts.deferred += deferred.len();
            let stats = if measured {
                &mut population
            } else {
                &mut warm_population
            };
            stats.record(world);
            if args.trace {
                let mut record = json!({"tick": tick, "actions": actions, "deaths": deaths, "reconnects": reconnects, "deferred": deferred});
                if measured {
                    record["kind"] = json!("decision");
                    record["iteration"] = json!(decision - client.warmup_decisions);
                    record["frames"] = json!(current_frames);
                    record["wireRecords"] = json!(wire);
                    record["snapshot"] = world_events(world)?;
                } else {
                    record["kind"] = json!("warmup");
                    record["decision"] = json!(decision);
                    record["wire"] = wire_summary(&wire);
                    record["stats"] = json!(WorldStats::from_world(world));
                }
                line(out, record)?;
            }
            if measured && decision + 1 - client.warmup_decisions == window {
                validity = check_validity(&args.case, window, warm.plus(lifecycle), death_food)?;
            }
        }
        if args.trace {
            line(
                out,
                json!({"kind": "end", "case": args.case, "operations": args.iterations, "physicsTicks": tick, "lifecycle": lifecycle.json()}),
            )?;
        }
        Ok(report(
            args,
            meter,
            "step_all (six physics ticks and all client frames)",
            json!({
                "definition": args.driver.definition()?, "initial": initial,
                "warmup": {"decisions": client.warmup_decisions, "stats": warmed, "lifecycle": warm.json(), "population": warm_population.json()},
                "final": WorldStats::from_world(backend.world()), "lifecycle": lifecycle.json(),
                "population": population.json(), "validity": validity, "physicsTicks": tick,
                "wireRecording": true, "recordDrainTimed": false,
                "reconnectTimed": false, "epochs": "none; samples are short",
            }),
        ))
    }
}

/// Draws only harness decisions (poses and lengths), never the simulator's RNG.
#[derive(Clone)]
struct HarnessRandom(u64);

impl HarnessRandom {
    fn new() -> Self {
        Self(u64::from(SEED) ^ 0x7065_7266_7370_6177)
    }

    fn unit(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

const RETARGET_TICKS: usize = 64;
const BOOST_PERIOD: usize = 512;
const BOOST_TICKS: usize = 48;
const CHASE_LEAD: f64 = 120.0;

impl Controller {
    /// Stateless in the harness: depends only on the world and the tick, so step,
    /// profile and trace modes issue identical actions.
    fn actions(self, world: &Simulator, tick: usize) -> Vec<Option<Action>> {
        match self {
            Controller::Wander => wander(world, tick),
        }
    }

    fn definition(self) -> Value {
        match self {
            Controller::Wander => json!({
                "kind": "wander", "retargetTicks": RETARGET_TICKS, "chaseShare": 0.25,
                "chaseLead": CHASE_LEAD, "edgeMargin": "max(300, 0.12 radius), steer inward",
                "boostTicks": BOOST_TICKS, "boostPeriod": BOOST_PERIOD,
            }),
        }
    }
}

fn wander(world: &Simulator, tick: usize) -> Vec<Option<Action>> {
    let radius = world.config.radius;
    let margin = (radius * 0.12).max(300.0);
    let count = world.snakes.len();
    world
        .snakes
        .iter()
        .enumerate()
        .map(|(slot, snake)| {
            snake.alive.then(|| {
                // Staggered windows keep slots from retargeting and boosting in unison.
                let window = (tick + slot * 17) / RETARGET_TICKS;
                let hash = mix(((slot as u64) << 32) ^ window as u64);
                let jitter = (hash >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
                let (x, y) = (snake.head.x, snake.head.y);
                let target = (hash >> 8) as usize % count;
                let heading = if x.hypot(y) > radius - margin {
                    (-y).atan2(-x) + jitter
                } else if hash.is_multiple_of(4) && target != slot && world.snakes[target].alive {
                    let other = &world.snakes[target];
                    (other.head.y + other.heading.sin() * CHASE_LEAD - y)
                        .atan2(other.head.x + other.heading.cos() * CHASE_LEAD - x)
                } else {
                    jitter * TAU
                };
                Action {
                    heading,
                    boost: (tick + slot * 37) % BOOST_PERIOD < BOOST_TICKS,
                }
            })
        })
        .collect()
}

#[derive(Clone, Copy, Default)]
struct Lifecycle {
    body_deaths: usize,
    boundary_deaths: usize,
    death_food: usize,
    respawns: usize,
    deferred: usize,
    epochs: usize,
}

impl Lifecycle {
    fn deaths(&mut self, deaths: &[DeathEvent]) {
        for death in deaths {
            if death.reason == DeathReason::Body {
                self.body_deaths += 1;
            } else {
                // Workloads configure no pit. Keep that invariant without
                // naming variants absent from older overlaid baselines.
                assert_eq!(death.reason, DeathReason::Boundary);
                self.boundary_deaths += 1;
            }
            self.death_food += death.food;
        }
    }

    fn plus(self, other: Self) -> Self {
        Self {
            body_deaths: self.body_deaths + other.body_deaths,
            boundary_deaths: self.boundary_deaths + other.boundary_deaths,
            death_food: self.death_food + other.death_food,
            respawns: self.respawns + other.respawns,
            deferred: self.deferred + other.deferred,
            epochs: self.epochs + other.epochs,
        }
    }

    fn json(self) -> Value {
        json!({
            "bodyDeaths": self.body_deaths, "boundaryDeaths": self.boundary_deaths,
            "deathFoodPellets": self.death_food, "respawns": self.respawns,
            "deferredRespawns": self.deferred, "epochs": self.epochs,
        })
    }
}

/// Fails the run unless the steady-state lifecycle was exercised within warmup
/// plus the default trace window (or the whole run when shorter), so every mode
/// checks the same deterministic prefix.
fn check_validity(
    case: &str,
    operations: usize,
    total: Lifecycle,
    death_food: bool,
) -> Result<Value, String> {
    if total.body_deaths == 0 || total.respawns == 0 || (death_food && total.death_food == 0) {
        return Err(format!(
            "{case} is not steady: warmup plus {operations} operations had {} body deaths, {} respawns and {} death-food pellets; each must be at least one{}",
            total.body_deaths,
            total.respawns,
            total.death_food,
            if death_food {
                ""
            } else {
                " (death food disabled)"
            }
        ));
    }
    Ok(json!({
        "operations": operations, "lifecycle": total.json(), "deathFoodRequired": death_food,
        "requirement": "at least one body death, respawn and, when enabled, death-food pellet within warmup plus the default trace window",
    }))
}

/// Living population at operation boundaries, after respawns.
struct PopulationStats {
    requested: usize,
    samples: usize,
    living_sum: usize,
    living: (usize, usize),
    anchors: (usize, usize),
}

impl PopulationStats {
    fn new(requested: usize) -> Self {
        Self {
            requested,
            samples: 0,
            living_sum: 0,
            living: (usize::MAX, 0),
            anchors: (usize::MAX, 0),
        }
    }

    fn record(&mut self, world: &Simulator) {
        let living = world.living();
        let anchors = world
            .snakes
            .iter()
            .filter(|snake| snake.alive)
            .map(|snake| snake.body.len())
            .sum();
        self.samples += 1;
        self.living_sum += living;
        self.living = (self.living.0.min(living), self.living.1.max(living));
        self.anchors = (self.anchors.0.min(anchors), self.anchors.1.max(anchors));
    }

    fn json(&self) -> Value {
        if self.samples == 0 {
            return Value::Null;
        }
        let mean = self.living_sum as f64 / self.samples as f64;
        json!({
            "requested": self.requested, "samples": self.samples,
            "minLiving": self.living.0, "meanLiving": mean, "maxLiving": self.living.1,
            "minAnchors": self.anchors.0, "maxAnchors": self.anchors.1,
            "depleted": mean < 0.9 * self.requested as f64,
        })
    }
}

/// The world plus the only runner-side writer of slot liveness.
#[derive(Clone)]
struct Scene {
    world: Simulator,
    random: HarnessRandom,
    /// Deaths are read through this cursor; `world.deaths` stays untouched.
    cursor: usize,
    /// Dead slots with their drawn segment counts and due ticks, in death order.
    queue: VecDeque<(usize, usize, usize)>,
    segments: (usize, usize),
    clearance: f64,
}

struct Boundary {
    deaths: Vec<DeathEvent>,
    respawns: Vec<(usize, Point, f64, usize)>,
    deferred: Vec<usize>,
}

impl Boundary {
    fn json(&self) -> Value {
        let respawns: Vec<_> = self
            .respawns
            .iter()
            .map(|(slot, head, heading, segments)| {
                json!({"slot": slot, "head": head, "heading": heading, "segments": segments})
            })
            .collect();
        json!({"deaths": self.deaths, "respawns": respawns, "deferred": self.deferred})
    }
}

impl Scene {
    fn place(scenario: &Scenario) -> Result<Self, String> {
        let mut world = Simulator::new(SEED, scenario.world.config())?;
        world.pickup_events = Some(Vec::new());
        world.food_spawn_events = Some(Vec::new());
        for slot in 0..scenario.population {
            world.add(slot.to_string(), slot, scenario.population)?;
        }
        // Initial lives enter through the respawn path, so they share its spawn rules.
        for snake in &mut world.snakes {
            snake.alive = false;
        }
        let mut scene = Self {
            world,
            random: HarnessRandom::new(),
            cursor: 0,
            queue: VecDeque::new(),
            segments: scenario.segments,
            clearance: scenario.clearance,
        };
        for slot in 0..scenario.population {
            let segments = scene.draw_segments();
            let (head, heading) = scene
                .pose(segments, PLACEMENT_ATTEMPTS)
                .ok_or_else(|| format!("{}: no clear initial pose for slot {slot}", scenario.id))?;
            scene.world.respawn(slot, head, heading, segments)?;
        }
        scene.cursor = scene.world.deaths.len();
        Ok(scene)
    }

    fn draw_segments(&mut self) -> usize {
        let (min, max) = self.segments;
        min + ((self.random.unit() * (max - min + 1) as f64) as usize).min(max - min)
    }

    fn pose(&mut self, segments: usize, attempts: usize) -> Option<(Point, f64)> {
        let radius = self.world.config.radius;
        for _ in 0..attempts {
            let distance = spawn::sample_spawn_distance(radius, self.random.unit());
            let angle = self.random.unit() * TAU;
            let heading = self.random.unit() * TAU;
            let head = Point {
                x: angle.cos() * distance,
                y: angle.sin() * distance,
            };
            if self
                .world
                .spawn_is_clear(head, heading, segments, self.clearance)
            {
                return Some((head, heading));
            }
        }
        None
    }

    fn step(
        &mut self,
        actions: &[Option<Action>],
        profile: Option<&mut StepProfile>,
    ) -> Result<(), String> {
        match profile {
            Some(profile) => self.world.step_profiled(actions, profile),
            None => self.world.step(actions),
        }
    }

    /// Untimed work between operations: queue new deaths and respawn due slots.
    fn boundary(&mut self, tick: usize, lifecycle: &mut Lifecycle) -> Result<Boundary, String> {
        let deaths = self.world.deaths[self.cursor..].to_vec();
        self.cursor = self.world.deaths.len();
        lifecycle.deaths(&deaths);
        for death in &deaths {
            let slot = death
                .victim
                .parse::<usize>()
                .map_err(|_| format!("Unknown death victim {}", death.victim))?;
            let segments = self.draw_segments();
            self.queue.push_back((slot, segments, tick));
        }
        let (mut respawns, mut deferred) = (Vec::new(), Vec::new());
        for _ in 0..self.queue.len() {
            let (slot, segments, due) = self.queue.pop_front().expect("queued respawn");
            if due > tick {
                self.queue.push_back((slot, segments, due));
                continue;
            }
            match self.pose(segments, RESPAWN_ATTEMPTS) {
                Some((head, heading)) => {
                    self.world.respawn(slot, head, heading, segments)?;
                    respawns.push((slot, head, heading, segments));
                }
                None => {
                    deferred.push(slot);
                    self.queue
                        .push_back((slot, segments, tick + RESPAWN_RETRY_TICKS));
                }
            }
        }
        lifecycle.respawns += respawns.len();
        lifecycle.deferred += deferred.len();
        Ok(Boundary {
            deaths,
            respawns,
            deferred,
        })
    }
}

/// Captures post-operation state, then runs the untimed lifecycle. Respawning resets
/// a just-died slot's per-life state, so a later capture would hide its changes
/// from parity traces.
fn settle<T>(
    scene: &mut Scene,
    tick: usize,
    lifecycle: &mut Lifecycle,
    capture: impl FnOnce(&Simulator) -> Result<T, String>,
) -> Result<(T, Boundary), String> {
    let captured = capture(&scene.world)?;
    Ok((captured, scene.boundary(tick, lifecycle)?))
}

fn run_scenario(
    args: &Args,
    scenario: &Scenario,
    bot: bool,
    out: &mut impl Write,
) -> Result<Value, String> {
    let controller = scenario.controller;
    let mut scene = Scene::place(scenario)?;
    set_parallel(&mut scene.world, args);
    let death_food = scene.world.config.death_food.enabled;
    let initial = WorldStats::from_world(&scene.world);
    if args.trace {
        line(
            out,
            json!({"kind": "initial", "case": args.case, "seed": SEED, "random": scene.random.0, "snapshot": world_events(&scene.world)?}),
        )?;
    }
    let mut tick = 0;
    let mut warm = Lifecycle::default();
    let mut warm_population = PopulationStats::new(scenario.population);
    for _ in 0..scenario.warmup_ticks {
        let actions = controller.actions(&scene.world, tick);
        scene.step(&actions, None)?;
        tick += 1;
        let (stats, boundary) = settle(&mut scene, tick, &mut warm, |world| {
            Ok(args.trace.then(|| json!(WorldStats::from_world(world))))
        })?;
        warm_population.record(&scene.world);
        if args.trace {
            let mut record = boundary.json();
            record["kind"] = json!("warmup");
            record["tick"] = json!(tick);
            record["actions"] = json!(actions);
            record["pickupEvents"] = json!(pickups(&scene.world));
            record["foodSpawnEvents"] = json!(births(&scene.world));
            record["stats"] = json!(stats);
            record["random"] = json!(scene.random.0);
            line(out, record)?;
        }
    }
    let warmed = WorldStats::from_world(&scene.world);
    if args.trace {
        line(
            out,
            json!({"kind": "warmed", "ticks": tick, "lifecycle": warm.json(), "random": scene.random.0, "snapshot": world_events(&scene.world)?}),
        )?;
    }
    let template = scene.clone();
    let window = args.iterations.min(args.operation.trace_iterations);
    let mut lifecycle = Lifecycle::default();
    let mut population = PopulationStats::new(scenario.population);
    let mut validity = Value::Null;
    let mut meter = Meter::default();
    let mut profile = StepProfile::default();
    let mut observe_seconds = 0.0;
    let mut buffer = Vec::new();
    for iteration in 0..args.iterations {
        if iteration > 0 && iteration % scenario.epoch == 0 {
            scene = template.clone();
            lifecycle.epochs += 1;
            if args.trace {
                line(
                    out,
                    json!({"kind": "epoch", "iteration": iteration, "epoch": lifecycle.epochs, "random": scene.random.0, "snapshot": world_events(&scene.world)?}),
                )?;
            }
        }
        let actions = controller.actions(&scene.world, tick);
        let observations = meter.operation(args.trace, || {
            scene.step(&actions, args.profile.then_some(&mut profile))?;
            if !bot {
                return Ok(None);
            }
            let started = args.profile.then(Instant::now);
            // Bot rows refill one buffer, as a bot loop that observes every decision would.
            scene.world.observe_all_into(&mut buffer);
            if let Some(started) = started {
                observe_seconds += started.elapsed().as_secs_f64();
            }
            if args.trace {
                return Ok(Some(buffer.clone()));
            }
            black_box(&buffer);
            Ok(None)
        })?;
        black_box(&scene.world);
        tick += 1;
        let (snapshot, boundary) = settle(&mut scene, tick, &mut lifecycle, |world| {
            args.trace.then(|| world_events(world)).transpose()
        })?;
        population.record(&scene.world);
        if args.trace {
            let mut record = boundary.json();
            record["kind"] = json!(if bot { "bot" } else { "step" });
            record["iteration"] = json!(iteration);
            record["tick"] = json!(tick);
            record["actions"] = json!(actions);
            record["random"] = json!(scene.random.0);
            record["snapshot"] = json!(snapshot);
            if bot {
                record["observations"] = json!(observations);
            }
            line(out, record)?;
        }
        if iteration + 1 == window {
            validity = check_validity(&args.case, window, warm.plus(lifecycle), death_food)?;
        }
    }
    if args.trace {
        line(
            out,
            json!({"kind": "end", "case": args.case, "operations": args.iterations, "physicsTicks": tick, "lifecycle": lifecycle.json()}),
        )?;
    }
    let operation = match (args.profile, bot) {
        (false, false) => "step",
        (true, false) => "step_profiled",
        (false, true) => "step then batch observe",
        (true, true) => "step_profiled then batch observe",
    };
    Ok(report(
        args,
        meter,
        operation,
        json!({
            "definition": args.driver.definition()?, "initial": initial,
            "warmup": {"ticks": scenario.warmup_ticks, "stats": warmed, "lifecycle": warm.json(), "population": warm_population.json()},
            "final": WorldStats::from_world(&scene.world), "lifecycle": lifecycle.json(),
            "population": population.json(), "validity": validity, "physicsTicks": tick,
            "eventBuffersEnabled": true, "boundaryTimed": false,
            "profile": if args.profile { Some(profile) } else { None },
            "observeSeconds": (args.profile && bot).then_some(observe_seconds),
        }),
    ))
}

fn run() -> Result<(), String> {
    let args = match Args::parse()? {
        Command::List => {
            let mut out = BufWriter::new(io::stdout().lock());
            line(&mut out, catalog_json()?)?;
            return out.flush().map_err(|e| e.to_string());
        }
        Command::Run(args) => args,
    };
    let mut out = BufWriter::new(io::stdout().lock());
    let Some(blocks) = args.blocks else {
        let report = run_case(&args, &mut out)?;
        line(&mut out, report)?;
        return out.flush().map_err(|e| e.to_string());
    };
    // Each block rebuilds the case from the seed, including its internal warmup, so
    // every block times the same trajectory from an equivalent state. Setup stays
    // outside the operation timer but inside each block's elapsed time.
    let mut timings = Vec::with_capacity(blocks);
    let mut last = Value::Null;
    for _ in 0..blocks {
        let start = Instant::now();
        let mut report = run_case(&args, &mut out)?;
        let mut timing = report["timing"].take();
        timing["elapsedSeconds"] = json!(start.elapsed().as_secs_f64());
        timings.push(timing);
        last = report;
    }
    last["blocks"] = json!(timings);
    line(&mut out, last)?;
    out.flush().map_err(|e| e.to_string())
}

fn run_case(args: &Args, out: &mut impl Write) -> Result<Value, String> {
    Ok(match args.driver {
        Driver::Legacy(
            Legacy::StepDefault | Legacy::StepFood | Legacy::StepBody | Legacy::Observe,
        ) => run_core(args, out)?,
        #[cfg(feature = "networking")]
        Driver::Legacy(Legacy::Portable) => portable::run(args, out)?,
        #[cfg(feature = "networking")]
        Driver::Legacy(Legacy::Protocol) => portable::run_protocol(args, out)?,
        Driver::Scenario { scenario, bot } => run_scenario(args, scenario, bot, out)?,
        #[cfg(feature = "networking")]
        Driver::Client(client) => portable::run_client(args, client, out)?,
        #[cfg(not(feature = "networking"))]
        Driver::Legacy(Legacy::Portable | Legacy::Protocol) | Driver::Client(_) => {
            return Err(format!("{} requires the networking feature", args.case));
        }
    })
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_TRACE_ITERATIONS: usize = 1000;
    /// PR CI times and traces every `ci` row within its job budget.
    const MAX_CI_ROWS: usize = 10;

    /// Dense and short so debug tests reach deaths, respawns and an epoch quickly.
    static SMALL: Scenario = Scenario {
        id: "test-small",
        world: World::DomeProxy { radius: 700.0 },
        population: 10,
        segments: (4, 12),
        clearance: 0.0,
        controller: Controller::Wander,
        warmup_ticks: 300,
        epoch: 48,
        step: op(100, 16, Full),
        bot: Some(op(100, 16, Full)),
    };

    fn args(case: &str, driver: Driver, operation: Operation, iterations: usize) -> Args {
        Args {
            case: case.into(),
            iterations,
            profile: false,
            trace: false,
            parallel: false,
            blocks: None,
            driver,
            operation,
        }
    }

    #[test]
    fn catalog_keeps_legacy_rows_and_bounds() {
        let workloads = catalog();
        let legacy = [
            ("step-default", 100_000),
            ("step-food", 10_000),
            ("step-body", 50_000),
            ("observe", 1000),
            ("portable", 5000),
            ("protocol", 5000),
        ];
        for (workload, (name, iterations)) in workloads.iter().zip(legacy) {
            assert!(matches!(workload.driver, Driver::Legacy(case) if case.name() == name));
            assert_eq!(workload.name, name);
            assert_eq!(workload.operation.iterations, iterations);
            assert_eq!(workload.operation.trace_iterations, 32);
            assert_eq!(workload.operation.suite, Ci);
        }
        assert!(
            workloads[legacy.len()..]
                .iter()
                .all(|workload| !matches!(workload.driver, Driver::Legacy(_)))
        );
        let mut names: Vec<_> = workloads.iter().map(|w| w.name.as_str()).collect();
        assert!(names.iter().all(|name| {
            name.split('-').all(|part| {
                !part.is_empty()
                    && part
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
            })
        }));
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), workloads.len(), "workload names are unique");
        for workload in &workloads {
            let operation = workload.operation;
            assert!((1..=MAX_ITERATIONS).contains(&operation.iterations));
            assert!((1..=MAX_TRACE_ITERATIONS).contains(&operation.trace_iterations));
            assert!(workload.driver.definition().is_ok(), "{}", workload.name);
        }
        let ci = workloads.iter().filter(|w| w.operation.suite == Ci);
        assert!(ci.count() <= MAX_CI_ROWS);
        for registration in REGISTRATIONS {
            match registration {
                Registration::Legacy(case, _) => {
                    if !matches!(case, Legacy::Portable | Legacy::Protocol) {
                        core_config(case.name()).unwrap().1.validate().unwrap();
                    }
                }
                Registration::Scenario(scenario) => {
                    scenario.world.config().validate().unwrap();
                    let (min, max) = scenario.segments;
                    assert!(2 <= min && min <= max && max <= 430, "{}", scenario.id);
                    assert!(scenario.population > 1 && scenario.epoch > 0);
                    assert!(scenario.clearance >= 0.0);
                }
                Registration::Client(client) => {
                    client.world.config().validate().unwrap();
                    assert!((1..=128).contains(&client.agents));
                }
            }
        }
    }

    #[test]
    fn catalog_line_matches_the_contract() {
        let catalog = catalog_json().unwrap();
        assert_eq!(catalog["format"], "slither-perf-catalog-v1");
        assert_eq!(catalog["networking"], cfg!(feature = "networking"));
        let workloads = catalog["workloads"].as_array().unwrap();
        assert_eq!(workloads.len(), self::catalog().len());
        for workload in workloads {
            let keys: Vec<_> = workload.as_object().unwrap().keys().collect();
            assert_eq!(
                keys,
                [
                    "available",
                    "definition",
                    "iterations",
                    "legacy",
                    "mode",
                    "name",
                    "operation",
                    "profile",
                    "suite",
                    "synthetic",
                    "traceIterations"
                ]
            );
            let networking = matches!(
                workload["name"].as_str().unwrap(),
                "portable" | "protocol" | "client-official-100"
            );
            assert_eq!(
                workload["available"],
                cfg!(feature = "networking") || !networking
            );
        }
    }

    #[test]
    fn registered_scenes_place_every_slot() {
        for registration in REGISTRATIONS {
            if let Registration::Scenario(scenario) = registration {
                let scene = Scene::place(scenario).unwrap();
                assert_eq!(scene.world.living(), scenario.population, "{}", scenario.id);
                assert!(scene.world.deaths.is_empty());
            }
        }
    }

    #[test]
    fn calibration_blocks_repeat_the_same_trajectory() {
        for bot in [false, true] {
            let operation = if bot { SMALL.bot.unwrap() } else { SMALL.step };
            let driver = Driver::Scenario {
                scenario: &SMALL,
                bot,
            };
            let args = args("test-small", driver, operation, 100);
            let [mut first, mut second] =
                [(), ()].map(|_| run_case(&args, &mut io::sink()).unwrap());
            assert_eq!(first["timing"]["operations"], 100);
            first["timing"].take();
            second["timing"].take();
            assert_eq!(first, second);
        }
    }

    #[test]
    fn small_scenes_warm_and_pass_validity() {
        for bot in [false, true] {
            let operation = if bot { SMALL.bot.unwrap() } else { SMALL.step };
            let driver = Driver::Scenario {
                scenario: &SMALL,
                bot,
            };
            let mut profiled = args("test-small", driver, operation, 100);
            profiled.profile = true;
            for args in [args("test-small", driver, operation, 100), profiled] {
                let report = run_scenario(&args, &SMALL, bot, &mut io::sink()).unwrap();
                let details = &report["details"];
                assert!(details["validity"].is_object());
                assert_eq!(details["lifecycle"]["epochs"], 2);
                assert_eq!(details["physicsTicks"], 400);
            }
        }
        let sandbox = REGISTRATIONS
            .iter()
            .find_map(|registration| match registration {
                Registration::Scenario(scenario) if scenario.id == "sandbox-16" => Some(scenario),
                _ => None,
            })
            .unwrap();
        let driver = Driver::Scenario {
            scenario: sandbox,
            bot: false,
        };
        let report = run_scenario(
            &args("sandbox-16-step", driver, sandbox.step, 4),
            sandbox,
            false,
            &mut io::sink(),
        )
        .unwrap();
        assert!(report["details"]["validity"].is_object());
    }

    #[test]
    fn profile_steps_match_plain_steps() {
        let mut plain = Scene::place(&SMALL).unwrap();
        let mut profiled = plain.clone();
        let (mut first, mut second) = (Lifecycle::default(), Lifecycle::default());
        let mut profile = StepProfile::default();
        for tick in 0..400 {
            let actions = SMALL.controller.actions(&plain.world, tick);
            plain.step(&actions, None).unwrap();
            profiled.step(&actions, Some(&mut profile)).unwrap();
            plain.boundary(tick + 1, &mut first).unwrap();
            profiled.boundary(tick + 1, &mut second).unwrap();
            assert_eq!(
                world_events(&plain.world).unwrap(),
                world_events(&profiled.world).unwrap(),
                "tick {tick}"
            );
            assert_eq!(plain.random.0, profiled.random.0);
        }
        assert!(first.body_deaths > 0 && first.respawns > 0);
        assert_eq!(profile.ticks, 400);
    }

    #[test]
    fn operation_snapshots_precede_respawns() {
        let mut scene = Scene::place(&SMALL).unwrap();
        let mut lifecycle = Lifecycle::default();
        for tick in 0..2000 {
            let actions = SMALL.controller.actions(&scene.world, tick);
            scene.step(&actions, None).unwrap();
            if let Some(death) = scene.world.deaths.get(scene.cursor) {
                let victim: usize = death.victim.parse().unwrap();
                let mut changed = scene.clone();
                changed.world.snakes[victim].heading += 1.0;
                let record = |scene: &mut Scene| {
                    let (snapshot, boundary) =
                        settle(scene, tick + 1, &mut Lifecycle::default(), world_events).unwrap();
                    assert!(boundary.respawns.iter().any(|r| r.0 == victim));
                    (snapshot, scene.world.snakes[victim].heading)
                };
                let (original, respawned) = record(&mut scene);
                let (mutated, respawned_after_change) = record(&mut changed);
                assert_eq!(respawned, respawned_after_change, "respawn resets the slot");
                assert_ne!(original, mutated, "snapshot keeps the dead slot's state");
                return;
            }
            scene.boundary(tick + 1, &mut lifecycle).unwrap();
        }
        panic!("no death within 2000 ticks");
    }

    #[test]
    fn scenario_traces_repeat_and_end_with_a_terminal_record() {
        let driver = Driver::Scenario {
            scenario: &SMALL,
            bot: true,
        };
        let mut traced = args("test-small-bot", driver, SMALL.bot.unwrap(), 60);
        traced.trace = true;
        let trace = || {
            let mut out = Vec::new();
            let report = run_scenario(&traced, &SMALL, true, &mut out).unwrap();
            line(&mut out, report).unwrap();
            out
        };
        let output = trace();
        assert_eq!(output, trace());
        let records: Vec<Value> = output
            .split(|&b| b == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).unwrap())
            .collect();
        let kinds: Vec<_> = records
            .iter()
            .map(|r| r["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds[0], "initial");
        assert_eq!(kinds[SMALL.warmup_ticks + 1], "warmed");
        assert_eq!(kinds.iter().filter(|&&kind| kind == "warmup").count(), 300);
        assert_eq!(kinds.iter().filter(|&&kind| kind == "bot").count(), 60);
        assert_eq!(kinds.iter().filter(|&&kind| kind == "epoch").count(), 1);
        assert_eq!(kinds[kinds.len() - 2..], ["end", "report"]);
        assert_eq!(records[kinds.len() - 2]["operations"], 60);
    }
}
