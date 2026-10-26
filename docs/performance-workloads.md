# Performance workloads

`examples/perf_baseline.rs` derives argument parsing, usage and its catalog from
one workload registration table. All cases are synthetic and offline, with seed 7
and the 7 ms physics tick; they represent neither production worlds nor bot
playing strength. See the [performance guide](performance.md) for builds/comparisons.

## Catalog

```sh
cargo run --release --locked --example perf_baseline -- --list
```

`--list` prints one line of canonical JSON (sorted keys), format
`slither-perf-catalog-v1`, and exits 0. Each workload in registration order has:

- `name`: the `CASE` argument of
  `perf_baseline CASE ITERATIONS [--profile|--trace|--blocks M] [--parallel]`.
  `--blocks M` (1–16) runs the timed case M times in one process for
  [calibration](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/pr-reports.md#calibration);
  each block rebuilds the case from the seed, including its warmup, and the report
  lists every block's timing and elapsed time under `blocks`.
- `suite`: `ci` rows are timed and traced by PR CI, which allows at most ten;
  `full` rows are local defaults only.
- `iterations` and `traceIterations`: the default timed sample size and parity
  window. Windows target an uncompressed trace near 64 MB or less; the 500-agent
  client row uses the minimum one decision and is about 82 MB including initial
  and warmed snapshots and all client frames.
- `mode`, `operation`: informational labels such as `official` and `step+observe`.
- `profile`: whether `--profile` is supported.
- `available`: false when the row needs the `networking` feature and the harness
  was built without it. The row is still listed; running it fails.
- `legacy`: true only for the six historical cases.
- `synthetic`: null, or what is synthetic about the scene.
- `definition`: the resolved scene, including the serialized `Config`. A changed
  definition means the same name now measures different work.

## Families

- **Legacy:** `step-default`, `step-food`, `step-body`, `observe`, `portable` and
  `protocol` come first. Read their resolved definitions from the catalog.
- **Sandbox and official:** `Config::sandbox()` and `Config::official(n)` at several
  populations, giving a scaling curve.
- **Battledome:** `battledome-32-step` and `battledome-32-bot` use the unmodified
  [`Config::battledome()`](configuration.md#battledome): pit, permanent size floor,
  spawn ring, ambient food and rolling kill score. They replace the obsolete
  sandbox-mechanics `dome-proxy-*` stand-ins.
- **Client admission:** `client-battledome-64` and `client-official-500` run
  `SimulatorBackend` with wire recording. Each timed decision includes
  `reconnect_slot` admission for dead clients plus `step_all`. These rows need
  the `networking` feature. The 500-agent row exercises the target host population;
  the old harness-only 128-agent assertion was not a backend limit. Battledome uses
  responsive delivery, matching preset-based host configuration. It warms for
  600 decisions (25.2 virtual seconds), past the synthetic ring crash and into
  ongoing churn, then times 40 decisions, sized on the bounded-admission runtime.
  Refused clients wait 24 decisions (1.008 virtual seconds); successful lives
  reconnect after their next reported death. This deterministic harness policy
  mirrors the host's one-second refusal backoff in virtual time; it does not
  measure the host's wall-clock scheduling. Both runtimes use the same policy.
  This is 64 seats: inspect `population.depleted` and refusals for living counts.
  Official uses eight warmup decisions, retaining its initial death/respawn burst
  in the measured window.

CI keeps ten rows: the six legacy cases, `official-100-step`, `official-100-bot`,
`battledome-32-step` and `client-battledome-64`. The two proxy CI slots become a real
Battledome physics row and a crowded admission/client row. Battledome batch
observation remains in the full suite; `official-100-bot` retains CI batch
observation coverage. The full-suite `client-official-500` replaces
`client-official-100` with a new name because both population and timed scope change.

Sandbox, official and Battledome registrations expand into `<id>-step` (one
`Simulator::step`) and `<id>-bot` (`step` then batch observation into a reused
buffer). Observations use `observe_all_into`, falling back to `observe_all` when
`perf_compare --base` targets an older API. Earlier `step+observe_all` definitions
dropped observations rather than reusing buffers.

## Steady-state lifecycle

Legacy cases reset after any death. Scenario rows retain deaths, death food,
respawns and grown bodies:

- **Placement:** added slots are marked dead and enter through `Simulator::respawn`
  at poses passing `Simulator::spawn_is_clear`. Distances use
  `spawn::sample_spawn_distance`; lives start with straight bodies and segment
  counts drawn from the registered range. Battledome instead samples the configured
  ring and uses `Simulator::spawn_body`, `body_is_clear` and `respawn_body` for
  floor-size, curved newborns. Its `synthetic` label discloses harness contact
  clearance rather than host admission. Ring capacity limits simultaneous
  newborns, not the eventual living population. Initial placement uses 64 attempts
  per slot and queues blocked lives instead of failing construction. Refused ring
  placements retry after 144 ticks (1.008 virtual seconds), allowing existing
  lives to leave the ring before another bounded search. The 32-slot row warms
  for 8,000 ticks before measurement; reports retain `initialQueued`, initial
  living count and warmup/measurement population. Non-ring placement is unchanged.
  The 64-agent client row separately stresses backend admission.
- **Randomness:** harness-owned pose/length generation never consumes simulator
  RNG, preserving world randomness across lifecycle work.
- **Control:** a stateless wander controller picks a heading per staggered window,
  occasionally chases another head, steers inward near the edge and boosts in a
  staggered window. Controls and lifecycle work are outside the timed interval.
- **Respawn:** between operations, a cursor reads new deaths without modifying
  `world.deaths`. Slots queue in death order with bounded placement attempts;
  blocked non-ring slots retry a few ticks later; ring slots use the interval above.
- **Warmup and epochs:** an untimed warmup runs first. Long samples restore the
  warmed scene every 4,096 operations, bounding death history and drift.
- **Client rows:** a slot reconnects through `reconnect_slot` after its client
  reports the death, as a bot would. Native backend initialization is a synthetic
  ring start, not host admission; only subsequent reconnects exercise admission.
  Actions go only to slots that are playing and not processing an earlier decision.
  Admission and stepping use disjoint timed intervals summed into one operation;
  controller calculation and wire draining remain untimed. `admissionCalls` counts
  all measured reconnect attempts, including refused poses, and `admissionSeconds`
  is their included interval (null in traces). `admissionSecondsPerDecision`
  divides it by the measured decision count; `reconnectTimed` is true. Warmup is
  excluded from operation timing. `windows` reports consecutive 100-decision
  populations and lifecycles, split at warmup and sample boundaries. Lifecycles
  include `refusalsPerDecision` and `respawnsPerDecision`, so the initial crash,
  warmup tail and measured churn remain distinguishable. The default trace window
  must itself include admission work. The four-decision Battledome parity window
  includes refused admissions; the full 40-decision sample also includes deaths
  and successful respawns.

`--profile` replaces `step` with `step_profiled`. Profiled `-bot` rows also report
`observeSeconds`, the time spent in the batch observation.

Reports keep the `slither-perf-baseline-v1` envelope. Scenario details add the
definition, lifecycle counts, population statistics and warmup results.

## Validity checks

Every mode checks warmup plus the default trace window, or the whole run if
shorter. This deterministic prefix must include a body death, respawn and, when
enabled without a size floor, death-food pellet; otherwise the harness exits
non-zero with counts. Size-floor worlds do not require food from deaths, because
a floor-size death correctly produces none. Pit deaths are reported separately
when present.

Population depletion is reported, not fatal. `population` gives minimum, mean
and maximum living snakes, plus minimum and maximum total anchors at operation
boundaries; `depleted` is true when the mean is below 90% of the requested
population. Check it before interpreting throughput.

## Traces

`--trace` emits untimed canonical JSONL. Legacy traces are unchanged. Scenario
traces contain:

1. An `initial` full snapshot after placement.
2. One light `warmup` line per tick: actions, deaths, respawns, events, world
   statistics and the harness RNG state. Then a `warmed` full snapshot.
3. One line per measured operation (`step`, `bot` or client `decision`) with its
   actions, deaths, respawns and a full snapshot. `-bot` lines include the
   observations; client lines include frames and wire records.
4. An `epoch` snapshot whenever the warmed scene is restored.
5. A terminal `end` record with the operation count, physics ticks and lifecycle
   counts, followed by the usual report line. A trace without `end` is incomplete.

`--parallel` calls `Simulator::set_parallel(true)` on the timed world, so `observe`
and `-bot` rows observe on the rayon pool. It needs the default `parallel` feature
and adds `"parallel": true` to the report; every other trace line must match the
run without it. Runners and PR comparisons do not pass it.

Snapshots/warmup statistics precede respawns so per-life resets cannot hide a
just-died snake's state from parity checks. Respawn records retain new poses and
lengths; client slots reconnect before the next decision.

## Adding a workload

1. Add one registration to `REGISTRATIONS`. A new world needs a `World` variant;
   its exhaustive matches require a config, mode, label and definition.
2. Calibrate `iterations` so one release sample takes about one second. Choose a
   `traceIterations` that keeps the uncompressed trace near 64 MB or less.
3. Run the row in timing, `--profile` and `--trace` modes. Confirm the validity
   check passes, the population is not unexpectedly depleted, and two traces are
   byte-identical.
4. Keep at most ten `ci` rows. Run `cargo test --locked --example perf_baseline`.

When changing a workload definition, use a new name so results stay comparable.
No runner or workflow edit is needed for a new registration. The calibration
pilot's explicit groups must also name available workloads when retiring a row.

## Observation workloads

The immutable `perf_observation` harness isolates geometry/selection costs,
separating materialization from output destruction. It has no physics stepping
or catalog and does not measure long-running sessions.

The first six cases use seed 7, 32 living slots, 32 body anchors per slot, eight
prey, and vision 1,500. Body anchors extend from each ring-start head at 42-unit spacing.
Food counts below are admitted pellets, not placement attempts.

| Case | Geometry and API | Food | Baseline iterations/sample |
| --- | --- | ---: | ---: |
| `observe-dense` | Radius 6,000, `observe_all`; exactly the original `observe` world. | 11,321 | 1,000 |
| `observe-compact` | Radius 1,000, `observe_all`; synthetic ambient cap disabled, tails extend outside the rim; never stepped. | 2,000 | 500 |
| `observe-wide` | Radius 50,000, `observe_all`. | 11,999 | 2,000 |
| `observe-strip` | Radius 50,000, heads on x=0 at 750-unit y spacing; remapped food within x=±75, y=±48,000; `observe_all`. | 11,999 | 1,000 |
| `observe-one` | Dense world, `observe_selected` for slot zero. | 11,321 | 10,000 |
| `observe-empty` | Dense world, all-false selection mask; still returns 32 empty slots. | 11,321 | 10,000 |
| `observe-sparse` | Radius 100,000; slots 0–7 alive with heads at the origin, slots 8–31 dead; a 512×512 food lattice over x, y = ±65,625 replaces the startup food; `observe_all`. | 262,144 | 500 |
| `observe-clustered` | Radius 20,000; 128 slots with two-anchor bodies and heads on a 12-column grid at 1,600-unit spacing within ±8,800; four 64×64 lattices in 1,200-unit corner patches within ±12,000, plus 512 background pellets, replace the startup food; `observe_all`. | 16,896 | 2,000 |

Strip remapping preserves RNG, insertion order, values, sizes and origins.
The sparse lattice stresses many grid cells with eight observers; clustered
patches beyond all vision make mean density overstate visible food. Two-anchor
clustered bodies keep snake filtering from hiding grid cost. These explicit
overrides leave package defaults unchanged and expose filtering/copying/index
costs, not a weighted production distribution.

### Reproduction and metric scope

Build `cargo build --release --locked --example perf_observation -j 2` with
identical toolchain, profile and harness source for both runtimes. Follow
[copied-executable identity requirements](performance.md#compare-copied-executables)
and select cases explicitly:

```sh
python -B tools/perf_compare.py --baseline BASELINE --candidate CANDIDATE --output outputs/observation-comparison --cases observe-dense observe-compact observe-wide observe-strip observe-one observe-empty observe-sparse observe-clustered
```

The runner uses 10,000 wide batches and 100,000 empty selections per sample to
reduce Windows CPU quantization; other counts match the table. It uses the
[local comparison](performance.md#compare-a-change) warmup, paired ordering,
five-pair default and quiet-window requirements. Finish traces before timing too.

Materialization includes indexing, filtering, ordering, copying and internal
temporary destruction. `outputDrop` separately times returned-observation
destruction. Both use `Instant` wall intervals without phase attribution.
Process CPU/wall includes setup, both intervals, clock/sample overhead,
serialization and teardown. [CPU/peak scope](performance.md#read-the-results)
applies: CPU is quantized Windows kernel+user time. Throughput and operation tail
latencies describe batches with the stated selection mask.

Historical measurements, rejected candidates and the pinned phase-diagnostic
recipe live in the [observation study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-23-observation-batches/REPORT.md).
That diagnostic recipe targets a historical implementation, not the current
2D food grid. New profiling work must start from the current source.

## Retained client workload

`examples/perf_client_retained.rs` extends the catalog's short portable/protocol
fixtures: a seed-7 synthetic world, up to 384 decisions of six 7 ms ticks
(16.128 virtual seconds). Eight slots retain connections/decoders; slot zero
reconnects after decision 192. Death does not reset the world. In `next_frame`,
native background seekers may reconnect dead background slots.

The explicit workload overrides are radius 6,000, 12,000 food placement attempts,
eight prey, vision 3,000, eight agents, zero added link/processing delays, wire
recording and a 136-square minimap. Decisions use six ticks; gameplay updates
are emitted every tick with [per-family release rules](server-rules.md#packet-timing).
Minimap and leaderboard intervals are 2.5 seconds and three seconds. Other
defaults remain unchanged. Normal spawn bodies are used; available anchor counts
are reported.
These are local engineering fixtures, not official-server presets.

```sh
cargo build --release --locked --example perf_client_retained -j 2
```

Copy into a new owned `outputs/retained-client/bin/` and record platform plus
the [binary/source/build identities](performance.md#compare-copied-executables).
The embedded source label records fixture construction history, not actual
binary/runtime identity.

```sh
./outputs/retained-client/bin/perf_client_retained step_all 384
./outputs/retained-client/bin/perf_client_retained next_frame 384
```

On Windows, use `./outputs/retained-client/bin/perf_client_retained.exe`.

Each invocation starts fresh; `--iterations N` permits one to eight same-seed
worlds. `step_all` controls every playing slot; `next_frame` controls slot zero
and uses built-in background seekers. Modes produce different worlds/work;
compare only matching mode, configuration, seed and decision count. There are
no sockets, listeners, `BotRunner` or compute-time feedback into virtual time.

Every decision/lifecycle checkpoint drains all eight slots' records, replays them
through persistent independent public decoders, materializes frames and checks
backend equality. Wire/coverage counters preserve array order and connection
generations. Reports retain living populations, food, available anchors, packet
codes, full/delta maps, leaderboards and reconnects. False coverage means a case
did not occur, not an error or grounds for policy changes. Sector-clear packets
show exits, not private expiry reasons. Counts use available observations;
dead/waiting frames are not empty worlds.

Wall intervals separately report backend operations, external action submission,
additional backend frames, record draining, replay application, replay frames and
manual reconnect. They exclude control calculation, equality checks, counters,
serialization and returned-output destruction. Process CPU/wall include those
costs. Internal physics/encoding/delivery/decoding remain combined inside the
operation; `next_frame` also includes background-seeker frame construction.
Agree a quiet interval before interpreting timing, retain every raw sample, and
report process CPU and peak memory separately from these inner wall intervals.

`--trace` requires one iteration and disables all measurement intervals. It emits
full available core state (including serialized core RNG/clocks), latest-tick
events, direct observations, ordered actions, frames and wire records. Only JSON
object keys and the internal unordered `prey_outward` set are canonicalized.
Full traces can be large; start with a few decisions, retain outputs and bound
execution when expanding. Use identical harness bytes and complete-byte
comparisons when checking a runtime optimization.

Private backend RNG, transport queues, subscription/presentation state and the
five intervening ticks' event buffers are inaccessible through these public
seams. The trace therefore does not establish full private-backend-state parity.
It does retain sent background commands rather than substituting another seeker.
The fixture does not stage 128-anchor bodies or assert that `V`, death or sector
expiration occurs; inspect the actual coverage report before claiming any of them.
