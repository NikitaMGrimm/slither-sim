//! Bounded synthetic retained-client workloads; no sockets or BotRunner.
//!
//! perf_client_retained step_all|next_frame DECISIONS [--iterations N] [--trace]
//! DECISIONS: 1..384 (at most 16.128 virtual seconds per world); N: 1..8.
//! Each iteration constructs a fresh seed-7 world; never resets it on death.
//! --trace is untimed JSONL and requires one iteration. Run separately from timing.
//! Parent-owned verification, for example:
//! cargo build --release --locked --example perf_client_retained -j 2
//! target/release/examples/perf_client_retained step_all 384 --trace
//! target/release/examples/perf_client_retained next_frame 384 --trace
//! Use the same example source, copied/pinned binaries and matched settings when
//! comparing runtimes. Neither these workloads nor source hypotheses imply speedups.

#[path = "support/runtime.rs"]
mod runtime;
#[cfg(not(slither_private_state))]
#[allow(unused_imports)]
use runtime::LegacyState;

#[path = "support/state.rs"]
#[cfg(feature = "networking")]
mod checkpoint_state;
#[cfg(feature = "networking")]
mod retained {
    #[cfg(not(slither_private_state))]
    use crate::runtime::LegacyState;
    use serde_json::{Value, json};
    use slither_sim::{
        Action, Config, Simulator,
        client::{
            Backend, ClientFrame, ClientState, ClientStatus, LatencyConfig, SimulationConfig,
            SimulatorBackend, WireEvent, WireRecord, protocol::split_frame, replay_record,
        },
        profiling::WorldStats,
    };
    use std::{
        collections::BTreeMap,
        f64::consts::FRAC_PI_2,
        hint::black_box,
        io::{self, BufWriter, Write},
        time::Instant,
    };

    const SEED: u32 = 7;
    const SLOTS: usize = 8;
    const TICKS: usize = 6;
    const DT: f64 = 0.007;
    const RECONNECT: usize = 192;
    const BASE: &str = "20dd3aff5574af0270d8ad2c1e93f9001e8f1142";
    const TREE: &str = "8e410d0f3246e07a2d73332aa15561ad865c1b99";
    const USAGE: &str = "perf_client_retained step_all|next_frame DECISIONS [--iterations N] [--trace]; DECISIONS=1..384; N=1..8; --trace requires N=1";

    struct Args {
        mode: String,
        decisions: usize,
        iterations: usize,
        trace: bool,
    }

    impl Args {
        fn parse() -> Result<Self, String> {
            let mut input = std::env::args().skip(1);
            let mode = input.next().ok_or(USAGE)?;
            if !matches!(mode.as_str(), "step_all" | "next_frame") {
                return Err(USAGE.into());
            }
            let decisions = input
                .next()
                .ok_or(USAGE)?
                .parse::<usize>()
                .map_err(|_| USAGE)?;
            let mut iterations = 1;
            let mut trace = false;
            let mut have_iterations = false;
            while let Some(option) = input.next() {
                match option.as_str() {
                    "--trace" if !trace => trace = true,
                    "--iterations" if !have_iterations => {
                        iterations = input
                            .next()
                            .ok_or(USAGE)?
                            .parse::<usize>()
                            .map_err(|_| USAGE)?;
                        have_iterations = true;
                    }
                    _ => return Err(USAGE.into()),
                }
            }
            if !(1..=384).contains(&decisions)
                || !(1..=8).contains(&iterations)
                || trace && iterations != 1
            {
                return Err(USAGE.into());
            }
            Ok(Self {
                mode,
                decisions,
                iterations,
                trace,
            })
        }
    }

    #[derive(Default)]
    struct Meter(Vec<f64>);

    impl Meter {
        fn call<T>(
            &mut self,
            timed: bool,
            f: impl FnOnce() -> Result<T, String>,
        ) -> Result<T, String> {
            if !timed {
                return f();
            }
            let start = Instant::now();
            let result = f();
            black_box(&result);
            let seconds = start.elapsed().as_secs_f64();
            let result = result?;
            self.0.push(seconds);
            Ok(result)
        }

        fn report(mut self) -> Value {
            if self.0.is_empty() {
                return Value::Null;
            }
            let total: f64 = self.0.iter().sum();
            self.0.sort_unstable_by(f64::total_cmp);
            let rank = |p: usize| self.0[(self.0.len() * p).div_ceil(100) - 1];
            json!({"samples": self.0.len(), "wallSeconds": total,
                "latencySeconds": {"p50": rank(50), "p95": rank(95), "p99": rank(99), "max": self.0.last()}})
        }
    }

    #[derive(Default)]
    struct Timing {
        operation: Meter,
        controlled_send: Meter,
        extra_frames: Meter,
        reconnect_frames: Meter,
        drain: Meter,
        replay: Meter,
        replay_frames: Meter,
        reconnect: Meter,
    }

    impl Timing {
        fn report(self) -> Value {
            json!({
                "operation": self.operation.report(),
                "nextFrameControlledActionSubmission": self.controlled_send.report(),
                "additionalBackendFrames": self.extra_frames.report(),
                "manualReconnectFrames": self.reconnect_frames.report(),
                "drainAllSlotRecords": self.drain.report(),
                "applyAllSlotRecords": self.replay.report(),
                "materializeAllReplayFrames": self.replay_frames.report(),
                "manualReconnect": self.reconnect.report(),
                "scope": "disjoint wall intervals; initial setup/checkpoint, controls, counters, equality checks, trace/report serialization and returned-output destruction excluded",
                "inseparable": "operation includes backend recording allocations, physics, encoding/delivery/decoding and returned frames; next_frame also includes actual background seekers and their internal frame materialization",
                "additionalFrames": "next_frame decisions materialize only slots 1..7 here; step_all decisions materialize no extra frames; reconnect checkpoints materialize all eight separately",
                "actionSubmission": "step_all submits actions inside operation; next_frame controlled send_action is timed separately when an action exists; control calculation and zero processing metadata setters are untimed",
                "replayScope": "persistent decoders, one frame per slot after each decision and lifecycle checkpoint; application and frame construction timed separately; zero processing metadata",
                "noPhaseSumClaim": "these intervals are not total process cost or exclusive internal subsystem costs"
            })
        }
    }

    fn config() -> SimulationConfig {
        let mut world = Config {
            radius: 6000.0,
            food_count: 12_000,
            vision: 3000.0,
            dt: DT,
            ..Config::sandbox()
        };
        world.prey.count = 8;
        SimulationConfig {
            world,
            agents: SLOTS,
            controlled_slot: 0,
            decision_ticks: TICKS,
            minimap_interval_seconds: 2.5,
            minimap_side: 136,
            leaderboard_interval_seconds: 3.0,
            latency: LatencyConfig::one_way(0.0, 0.0),
            record_wire: true,
            ..SimulationConfig::default()
        }
    }

    // Deterministic diagnostic controls use only each client's received frame.
    // The built-in background policy in next_frame is deliberately not duplicated.
    fn control(frame: &ClientFrame, decision: usize) -> Option<Action> {
        if frame.status != ClientStatus::Playing {
            return None;
        }
        let me = &frame.observation.as_ref()?.me;
        let (x, y) = (me.head.x, me.head.y);
        Some(Action {
            heading: y.atan2(x) + FRAC_PI_2 + ((x.hypot(y) - 3000.0) / 3000.0).clamp(-0.6, 0.6),
            boost: decision % 128 < 16,
        })
    }

    fn frames(backend: &SimulatorBackend) -> Result<Vec<ClientFrame>, String> {
        (0..SLOTS).map(|slot| backend.frame_for(slot)).collect()
    }

    fn canonical(value: Value) -> Value {
        match value {
            Value::Object(fields) => {
                let ordered: BTreeMap<_, _> = fields
                    .into_iter()
                    .map(|(key, value)| (key, canonical(value)))
                    .collect();
                Value::Object(ordered.into_iter().collect())
            }
            Value::Array(values) => Value::Array(values.into_iter().map(canonical).collect()),
            other => other,
        }
    }

    fn line(out: &mut impl Write, value: Value) -> Result<(), String> {
        serde_json::to_writer(&mut *out, &canonical(value)).map_err(|e| e.to_string())?;
        writeln!(out).map_err(|e| e.to_string())
    }

    fn snapshot(world: &Simulator) -> Result<Value, String> {
        let state = crate::checkpoint_state::projection(world)?;
        let pickups = world.pickup_events().as_ref().map(|events| {
            events
                .iter()
                .map(|event| {
                    let item = match &event.item {
                        slither_sim::PickupItem::Food(food) => {
                            json!({"kind": "food", "value": food})
                        }
                        slither_sim::PickupItem::Prey(prey) => {
                            json!({"kind": "prey", "value": prey})
                        }
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
        Ok(json!({"world": state, "latestTickPickupEvents": pickups,
            "latestTickFoodSpawnEvents": births, "directObservations": world.observe_all()}))
    }

    #[derive(Default)]
    struct Range {
        samples: usize,
        min: usize,
        max: usize,
        last: usize,
    }

    impl Range {
        fn add(&mut self, n: usize) {
            self.min = if self.samples == 0 {
                n
            } else {
                self.min.min(n)
            };
            self.max = self.max.max(n);
            self.last = n;
            self.samples += 1;
        }

        fn report(&self) -> Value {
            if self.samples == 0 {
                Value::Null
            } else {
                json!({"samples": self.samples, "min": self.min, "max": self.max, "lastAvailable": self.last})
            }
        }
    }

    #[derive(Default)]
    struct SlotCoverage {
        records: usize,
        wire_bytes: usize,
        connects: usize,
        disconnects: usize,
        sent_heading_commands: usize,
        packets: BTreeMap<String, usize>,
        appearances: usize,
        removals: usize,
        killed_removals: usize,
        game_overs: usize,
        subscription_packets: usize,
        sector_listings: usize,
        sector_clears: usize,
        // Private subscriptions (including empty sectors) cannot be counted
        // from F/w: record packet occurrences, without inventing that denominator.
        full_maps: usize,
        delta_maps: usize,
        boards: usize,
        cadence: Vec<Value>,
        connected_at: f64,
        last_map_at: Option<f64>,
        last_board_at: Option<f64>,
        first_map_after_manual: Option<Value>,
        living_visible: Range,
        frame_anchors: Range,
        frame_food: Range,
        frame_prey: Range,
        snake_records: Range,
        playing_frames: usize,
        dead_frames: usize,
        waiting_frames: usize,
        final_counts: Value,
    }

    impl SlotCoverage {
        fn wire(&mut self, records: &[WireRecord], after_manual: bool) -> Result<(), String> {
            for record in records {
                self.records += 1;
                self.wire_bytes += record.bytes.len();
                match record.event {
                    WireEvent::Connect => {
                        self.connects += 1;
                        self.connected_at = record.time;
                        self.last_map_at = None;
                        self.last_board_at = None;
                    }
                    WireEvent::Disconnect => self.disconnects += 1,
                    WireEvent::Send => {
                        if record.bytes.len() == 1 && record.bytes[0] <= 250 {
                            self.sent_heading_commands += 1;
                        }
                    }
                    WireEvent::Receive => {
                        // Native backend emits elapsed-ms frames; split_frame
                        // retains packet order and does not reinterpret payloads.
                        for packet in split_frame(&record.bytes)? {
                            let code = packet[0];
                            *self
                                .packets
                                .entry(char::from(code).to_string())
                                .or_default() += 1;
                            match code {
                                b's' if packet.len() > 7 => self.appearances += 1,
                                b's' => {
                                    self.removals += 1;
                                    self.killed_removals += usize::from(packet.get(3) == Some(&1));
                                }
                                b'v' if packet.get(1) != Some(&2) => self.game_overs += 1,
                                b'W' => self.subscription_packets += 1,
                                b'F' => self.sector_listings += 1,
                                b'w' => self.sector_clears += 1,
                                b'M' => self.full_maps += 1,
                                b'V' => self.delta_maps += 1,
                                b'l' => self.boards += 1,
                                _ => {}
                            }
                            if matches!(code, b'M' | b'V' | b'l') {
                                let previous = if code == b'l' {
                                    &mut self.last_board_at
                                } else {
                                    &mut self.last_map_at
                                };
                                let interval = previous.map(|at| record.time - at);
                                *previous = Some(record.time);
                                let event = json!({"connection": self.connects,
                                    "receiptTime": record.time, "packet": char::from(code).to_string(),
                                    "sinceConnectSeconds": record.time - self.connected_at,
                                    "sincePreviousSameStreamSeconds": interval});
                                if after_manual
                                    && matches!(code, b'M' | b'V')
                                    && self.first_map_after_manual.is_none()
                                {
                                    self.first_map_after_manual = Some(event.clone());
                                }
                                self.cadence.push(event);
                            }
                        }
                    }
                }
            }
            Ok(())
        }

        fn frame(&mut self, frame: &ClientFrame) {
            self.snake_records.add(frame.received.snakes.len());
            match frame.status {
                ClientStatus::Playing => self.playing_frames += 1,
                ClientStatus::Dead => self.dead_frames += 1,
                ClientStatus::Waiting => self.waiting_frames += 1,
                ClientStatus::Disconnected => {}
            }
            let available = frame.observation.as_ref().map(|o| {
                let living =
                    usize::from(o.me.alive) + o.opponents.iter().filter(|s| s.alive).count();
                let anchors =
                    o.me.body.len() + o.opponents.iter().map(|s| s.body.len()).sum::<usize>();
                self.living_visible.add(living);
                self.frame_anchors.add(anchors);
                self.frame_food.add(o.food.len());
                self.frame_prey.add(o.prey.len());
                json!({"livingVisibleSnakes": living, "retainedFrameAnchors": anchors,
                    "retainedFrameFood": o.food.len(), "retainedFramePrey": o.prey.len()})
            });
            self.final_counts = json!({"status": frame.status, "life": frame.life,
                "connectionGeneration": frame.received.connection_generation,
                "knownSnakeRecords": frame.received.snakes.len(), "observationCounts": available});
        }

        fn report(&self) -> Value {
            json!({
                "records": self.records, "wireBytes": self.wire_bytes,
                "connects": self.connects, "disconnects": self.disconnects,
                "sentHeadingCommands": self.sent_heading_commands, "packetsByCode": self.packets,
                "fullAppearances": self.appearances, "removals": self.removals,
                "killedRemovals": self.killed_removals, "gameOverPackets": self.game_overs,
                "subscriptionPackets_W": self.subscription_packets,
                "nonemptySectorListingPackets_F": self.sector_listings, "sectorClearPackets_w": self.sector_clears,
                "fullM": self.full_maps, "deltaV": self.delta_maps, "leaderboardL": self.boards,
                "cadenceOrderedReceipts": self.cadence, "firstMapAfterManualReconnect": self.first_map_after_manual,
                "firstPostReconnectMapIsFullM": self.first_map_after_manual.as_ref().map(|event| event["packet"] == "M"),
                "livingVisibleSnakes": self.living_visible.report(),
                "retainedAnchorsInAvailableFrames": self.frame_anchors.report(),
                "retainedFoodInAvailableFrames": self.frame_food.report(),
                "retainedPreyInAvailableFrames": self.frame_prey.report(),
                "knownSnakeRecords": self.snake_records.report(),
                "playingFrames": self.playing_frames, "deadFrames": self.dead_frames,
                "waitingFrames": self.waiting_frames, "final": self.final_counts
            })
        }
    }

    struct Run {
        backend: SimulatorBackend,
        decoders: Vec<ClientState>,
        coverage: Vec<SlotCoverage>,
        current: Vec<ClientFrame>,
        living: Range,
        food: Range,
        world_anchors: Range,
        checked_frames: usize,
    }

    impl Run {
        fn new() -> Result<Self, String> {
            let backend = SimulatorBackend::new(SEED, config())?;
            let current = frames(&backend)?;
            Ok(Self {
                backend,
                decoders: (0..SLOTS).map(|_| ClientState::new()).collect(),
                coverage: (0..SLOTS).map(|_| SlotCoverage::default()).collect(),
                current,
                living: Range::default(),
                food: Range::default(),
                world_anchors: Range::default(),
                checked_frames: 0,
            })
        }

        // The drain/replay group boundary is one decision, never one packet.
        // A separate same-time reconnect checkpoint preserves lifecycle ordering.
        fn checkpoint(
            &mut self,
            timing: &mut Timing,
            args: &Args,
            out: &mut impl Write,
            context: Value,
            timed: bool,
            after_manual: bool,
        ) -> Result<(), String> {
            let wire: Vec<Vec<WireRecord>> = timing.drain.call(timed, || {
                (0..SLOTS)
                    .map(|slot| self.backend.drain_records(slot))
                    .collect()
            })?;
            timing.replay.call(timed, || {
                for (decoder, records) in self.decoders.iter_mut().zip(&wire) {
                    for record in records {
                        replay_record(decoder, record)?;
                    }
                }
                Ok(())
            })?;
            let at = self.backend.time();
            let replayed: Vec<ClientFrame> = timing.replay_frames.call(timed, || {
                self.decoders
                    .iter()
                    .map(|decoder| decoder.frame(at))
                    .collect()
            })?;
            // Public-frame replay equality is a code invariant, not a claim of
            // full private backend-state parity or cross-runtime output parity.
            for (slot, ((expected, actual), records)) in
                self.current.iter().zip(&replayed).zip(&wire).enumerate()
            {
                if expected != actual {
                    return Err(format!(
                        "Public replay frame differs for slot {slot} at {at}"
                    ));
                }
                self.coverage[slot].wire(records, after_manual && slot == 0)?;
                self.coverage[slot].frame(expected);
            }
            self.checked_frames += SLOTS;
            self.living.add(self.backend.world().living());
            self.food.add(self.backend.world().food().len());
            self.world_anchors.add(
                self.backend
                    .world()
                    .snakes()
                    .iter()
                    .filter(|s| s.alive)
                    .map(|s| s.body.len())
                    .sum(),
            );
            black_box((&wire, &replayed));
            if args.trace {
                line(
                    out,
                    json!({"kind": "checkpoint", "context": context, "virtualTime": at,
                    "coreTime": self.backend.world().time(), "framesBySlot": self.current,
                    "replayFramesBySlot": replayed, "wireRecordsBySlot": wire,
                    "snapshot": snapshot(self.backend.world())?}),
                )?;
            }
            Ok(())
        }
    }

    fn metadata(args: &Args) -> Value {
        json!({
            "format": "slither-perf-client-retained-v1", "mode": args.mode,
            "sourceBaseline": BASE, "sourceBaselineTree": TREE,
            "sourcePinScope": "construction baseline only; caller must pin actual runtime/harness source and copied binary externally",
            "seed": SEED, "slots": SLOTS, "decisions": args.decisions, "iterations": args.iterations,
            "physicsTicksPerDecision": TICKS, "maxVirtualSecondsPerWorld": 16.128,
            "execution": if args.trace { "untimedTrace" } else { "timing" },
            "release": !cfg!(debug_assertions), "config": config(),
            "overrides": "synthetic radius=6000, food placement attempts=12000, prey count=8, vision=3000, agents=8, controlledSlot=0, recordWire=true; explicitly fixed dt=.007, decisionTicks=6 (gameplay emitted every tick), minimapSide=136, minimap interval=2.5s, leaderboard interval=3s, uplink/downlink/processing/variation/pause delays=zero; all other package defaults retained",
            "bodyStaging": "normal SimulatorBackend spawn bodies; no public mutable world/staging seam; actual available anchor counts reported",
            "control": "step_all: all playing slots receive deterministic radial/tangent controls; next_frame: slot0 receives those controls, slots1..7 use ACTUAL built-in seekers; requested boost true for decisions 0..15 of each 128 (zero-based)",
            "lifecycle": "manual slot0 reconnect immediately AFTER decision192, even at the run endpoint; no whole-world resets; others keep their connections unless next_frame's built-in seekers reconnect a dead background slot",
            "processingSeconds": 0.0, "processingSource": "fixed zero, never measured wall time",
            "traceLimits": [
                "full lossless public serde core state (including core RNG/motion state) and all available events, direct observations, frames, ordered wire and external actions at checkpoints",
                "private backend RNG/spawn RNG, transport queues, server subscription sets, presentation state and seeker controls are not public/serializable; this is NOT full backend-state parity",
                "pickup/birth buffers expose only the latest physics tick at decision boundaries; preceding five ticks' buffers and per-tick internal controls are inaccessible through step_all/next_frame",
                "actual sent background commands are preserved in wire, rather than substituting controls for seekers",
                "frame anchor/food/prey counts exist only when observation is available; null after death/waiting is not zero; removed/private decoder anchor allocations are inaccessible",
                "F/w count listing/exit packet occurrences, not private subscription cardinality; empty subscriptions emit no entry packet",
                "shared state adapters normalize checkpoint envelopes, slot grouping and field spelling; object keys and prey_outward set are sorted; all ordered arrays retained"
            ],
            "sourceHypotheses": [
                "broadcast rebuilds sector-food maps; food_updates calls known.clone_from(current) even for unchanged maps (src/client/simulation.rs)",
                "drive_background materializes a full frame for each eligible playing seeker (src/client/simulation.rs)",
                "minimaps repeats full-M encoding per eligible recipient before choosing a strictly shorter V (src/client/simulation.rs)"
            ],
            "interpretation": "synthetic cost/coverage screen and source hypotheses only; no speed claim, forced compression policy or official-server inference"
        })
    }

    pub(super) fn run() -> Result<(), String> {
        let args = Args::parse()?;
        let mut out = BufWriter::new(io::stdout().lock());
        if args.trace {
            line(
                &mut out,
                json!({"kind": "metadata", "details": metadata(&args)}),
            )?;
        }
        let mut timing = Timing::default();
        let mut reports = Vec::new();
        for iteration in 0..args.iterations {
            let mut run = Run::new()?;
            let initial = WorldStats::from_world(run.backend.world());
            run.checkpoint(
                &mut timing,
                &args,
                &mut out,
                json!({"phase": "initial", "iteration": iteration}),
                false,
                false,
            )?;
            for decision in 1..=args.decisions {
                let mut actions = vec![None; SLOTS];
                let externally_driven = if args.mode == "step_all" { SLOTS } else { 1 };
                for (slot, action) in actions.iter_mut().enumerate().take(externally_driven) {
                    run.backend.set_slot_processing_seconds(slot, 0.0)?;
                    *action = control(&run.current[slot], decision - 1);
                }
                let produced = if args.mode == "step_all" {
                    timing
                        .operation
                        .call(!args.trace, || run.backend.step_all(&actions))?
                } else {
                    // This public seam separates external action submission from
                    // next_frame without changing its actual background policy.
                    if let Some(action) = actions[0] {
                        timing
                            .controlled_send
                            .call(!args.trace, || run.backend.send_action(action))?;
                    }
                    let controlled = timing
                        .operation
                        .call(!args.trace, || run.backend.next_frame())?;
                    timing.extra_frames.call(!args.trace, || {
                        let mut all = Vec::with_capacity(SLOTS);
                        all.push(controlled);
                        for slot in 1..SLOTS {
                            all.push(run.backend.frame_for(slot)?);
                        }
                        Ok(all)
                    })?
                };
                run.current = produced;
                let expected_time = decision as f64 * TICKS as f64 * DT;
                if (run.backend.time() - expected_time).abs() > 1e-9 {
                    return Err("Zero-delay decision advanced an unexpected number of ticks".into());
                }
                run.checkpoint(&mut timing, &args, &mut out,
                    json!({"phase": "decision", "iteration": iteration, "decision": decision,
                        "externallyRequestedActionsBySlot": actions, "externalProcessingSeconds": 0.0}),
                    !args.trace, decision > RECONNECT)?;
                if decision == RECONNECT {
                    timing
                        .reconnect
                        .call(!args.trace, || run.backend.reconnect_slot(0))?;
                    run.current = timing
                        .reconnect_frames
                        .call(!args.trace, || frames(&run.backend))?;
                    run.checkpoint(&mut timing, &args, &mut out,
                        json!({"phase": "manualReconnect", "iteration": iteration, "afterDecision": decision, "slot": 0}),
                        !args.trace, true)?;
                }
            }
            let any = |f: fn(&SlotCoverage) -> bool| run.coverage.iter().any(f);
            let post_map = run.coverage[0].first_map_after_manual.as_ref();
            reports.push(json!({
                "iteration": iteration, "initial": initial, "final": WorldStats::from_world(run.backend.world()),
                "virtualTime": run.backend.time(), "physicsTicks": args.decisions * TICKS,
                "wholeWorldResets": 0, "manualReconnects": usize::from(args.decisions >= RECONNECT),
                "checkedPublicReplayFrames": run.checked_frames,
                "livingAtCheckpoints": run.living.report(), "foodAtCheckpoints": run.food.report(),
                "livingWorldBodyAnchorsAtCheckpoints": run.world_anchors.report(),
                "slots": run.coverage.iter().map(SlotCoverage::report).collect::<Vec<_>>(),
                "achievedCoverage": {
                    "V": any(|s| s.delta_maps > 0), "sectorExit_w": any(|s| s.sector_clears > 0),
                    "gameOver": any(|s| s.game_overs > 0), "postReconnectMap": post_map,
                    "coreDeath": !run.backend.world().deaths().is_empty(),
                    "privateSectorExpiryReason": Value::Null,
                    "backgroundReconnect": run.coverage.iter().skip(1).any(|s| s.connects > 1)
                },
                "coverageLimits": "false flags mean unachieved, not failures; w witnesses sector exit but does not identify the private expiry reason; no V/death/expiry/128-anchor coverage assertion; post-reconnect full M/cadence need decisions AFTER192 and a live slot; cadence receipts are listed per connection, never bridged across reconnects; checkpoint minima can miss intra-decision minima"
            }));
        }
        line(
            &mut out,
            json!({"kind": "report", "details": metadata(&args),
            "timing": timing.report(), "runs": reports}),
        )?;
        out.flush().map_err(|e| e.to_string())
    }
}

fn main() {
    #[cfg(feature = "networking")]
    if let Err(error) = retained::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    #[cfg(not(feature = "networking"))]
    {
        eprintln!("perf_client_retained requires the networking feature (no sockets are opened)");
        std::process::exit(1);
    }
}
