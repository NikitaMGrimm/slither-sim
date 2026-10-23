//! Run in release mode: perf_baseline CASE ITERATIONS [--profile|--trace].
//! Cases: step-default, step-food, step-body, observe, portable, protocol.
//! --profile is a separate timed step case; --trace emits untimed canonical JSONL.
//! No additional dependencies, sockets, files, or wall-clock bot processing samples.
use serde_json::{Value, json};
use slither_sim::{
    Action, Config, Point, Simulator, physics,
    profiling::{StepProfile, WorldStats},
};
use std::{
    collections::BTreeMap,
    f64::consts::{FRAC_PI_2, PI},
    hint::black_box,
    io::{self, BufWriter, Write},
    time::Instant,
};

const SEED: u32 = 7;
const EPISODE_TICKS: usize = 1000;
const USAGE: &str = "perf_baseline CASE ITERATIONS [--profile|--trace]; CASE = step-default|step-food|step-body|observe|portable|protocol; ITERATIONS = 1..100000; --profile requires a step case";

struct Args {
    case: String,
    iterations: usize,
    profile: bool,
    trace: bool,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let args: Vec<_> = std::env::args().skip(1).collect();
        if !(2..=3).contains(&args.len()) {
            return Err(USAGE.into());
        }
        let case = args[0].as_str();
        if !matches!(
            case,
            "step-default" | "step-food" | "step-body" | "observe" | "portable" | "protocol"
        ) {
            return Err(USAGE.into());
        }
        let iterations = args[1].parse::<usize>().map_err(|_| USAGE.to_string())?;
        if !(1..=100_000).contains(&iterations) {
            return Err(USAGE.into());
        }
        let option = args.get(2).map(String::as_str);
        if !matches!(option, None | Some("--profile" | "--trace")) {
            return Err(USAGE.into());
        }
        let profile = option == Some("--profile");
        if profile && !case.starts_with("step-") {
            return Err(USAGE.into());
        }
        Ok(Self {
            case: case.into(),
            iterations,
            profile,
            trace: option == Some("--trace"),
        })
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

fn core_world(case: &str) -> Result<(Simulator, Vec<Point>, f64), String> {
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

fn world_trace(world: &Simulator) -> Result<Value, String> {
    let mut state = serde_json::to_value(world).map_err(|e| e.to_string())?;
    // This private HashSet is semantically unordered; every ordered array, including
    // food, snakes, deaths and event buffers, retains its original order.
    if let Some(outward) = state.get_mut("prey_outward").and_then(Value::as_array_mut) {
        outward.sort_unstable_by_key(Value::as_u64);
    }
    let pickups = world.pickup_events.as_ref().map(|events| {
        events
            .iter()
            .map(|event| {
                let item = match &event.item {
                    slither_sim::PickupItem::Food(food) => json!({"kind": "food", "value": food}),
                    slither_sim::PickupItem::Prey(prey) => json!({"kind": "prey", "value": prey}),
                };
                json!({"eater": event.eater, "item": item})
            })
            .collect::<Vec<_>>()
    });
    let births = world.food_spawn_events.as_ref().map(|events| {
        events
            .iter()
            .map(|event| json!({"source": event.source, "food": event.food}))
            .collect::<Vec<_>>()
    });
    Ok(
        json!({"world": state, "pickupEvents": pickups, "foodSpawnEvents": births, "observations": world.observe_all()}),
    )
}

fn line(out: &mut impl Write, value: Value) -> Result<(), String> {
    serde_json::to_writer(&mut *out, &canonical(value)).map_err(|e| e.to_string())?;
    writeln!(out).map_err(|e| e.to_string())
}

fn report(args: &Args, meter: Meter, operation: &str, details: Value) -> Value {
    json!({
        "kind": "report", "format": "slither-perf-baseline-v1", "case": args.case,
        "iterations": args.iterations, "seed": SEED, "release": !cfg!(debug_assertions),
        "mode": if args.trace { "trace" } else if args.profile { "profile" } else { "timing" },
        "operation": operation, "timing": meter.report(), "details": details,
    })
}

fn run_core(args: &Args, out: &mut impl Write) -> Result<Value, String> {
    let (mut world, centers, orbit) = core_world(&args.case)?;
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

    fn backend(record_wire: bool) -> Result<SimulatorBackend, String> {
        SimulatorBackend::new(
            SEED,
            SimulationConfig {
                agents: AGENTS,
                record_wire,
                world: Config::sandbox(),
                ..SimulationConfig::default()
            },
        )
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
}

fn run() -> Result<(), String> {
    let args = Args::parse()?;
    let mut out = BufWriter::new(io::stdout().lock());
    let report = match args.case.as_str() {
        "step-default" | "step-food" | "step-body" | "observe" => run_core(&args, &mut out)?,
        #[cfg(feature = "networking")]
        "portable" => portable::run(&args, &mut out)?,
        #[cfg(feature = "networking")]
        "protocol" => portable::run_protocol(&args, &mut out)?,
        _ => return Err("portable and protocol require the networking feature".into()),
    };
    line(&mut out, report)?;
    out.flush().map_err(|e| e.to_string())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
