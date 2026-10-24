# Performance workloads

The native benchmark harness, `examples/perf_baseline.rs`, keeps every workload in
one registration table. That table is the only case list: argument parsing, usage
and the catalog are derived from it. All workloads are synthetic and offline, use
seed 7 and the 7 ms physics tick, and do not describe production worlds or bot
playing strength. Build and comparison procedures are in the
[performance guide](performance.md).

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
  window. Each window keeps an uncompressed trace near 64 MB or less.
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
- **Dome proxy:** a dense, fixed arena with sandbox mechanics, no prey and no
  ambient food. Boost and death pellets are its only food. It stands in for the
  future Battle Dome; it does not implement Battle Dome rules.
- **Client:** `SimulatorBackend` lives on an official world, timed through
  `step_all` with wire recording. These rows need the `networking` feature.

Each sandbox, official and dome-proxy registration expands into `<id>-step`, which
times one `Simulator::step`, and `<id>-bot`, which times `step` followed by a
batch observation into one reused buffer, as a bot that observes every decision
would. The batch observation is `observe_all_into`, or `observe_all` when
`perf_compare --base` builds the harness against a baseline that predates it.
Before the reused buffer, `-bot` rows were defined as `step+observe_all` and
dropped their observations.

## Steady-state lifecycle

Unlike the legacy cases, which reset after any death, scenario rows measure a
running population with deaths, death food, respawns and grown bodies:

- **Placement:** every slot is added, marked dead and enters through
  `Simulator::respawn` at a pose that passes `Simulator::spawn_is_clear`.
  Distances come from `spawn::sample_spawn_distance`. Each life is a straight body
  with a segment count drawn from the registered range.
- **Randomness:** poses and lengths come from a harness-owned generator. It never
  draws the simulator's RNG, so lifecycle work does not shift world randomness.
- **Control:** a stateless wander controller picks a heading per staggered window,
  occasionally chases another head, steers inward near the edge and boosts in a
  staggered window. Controls and lifecycle work are outside the timed interval.
- **Respawn:** between operations, new deaths are read through a cursor;
  `world.deaths` is not modified. Dead slots queue in death order and respawn with
  bounded attempts. A slot without a clear pose is deferred and retried a few
  ticks later.
- **Warmup and epochs:** an untimed warmup runs first. Long samples restore the
  warmed scene every 4,096 operations, bounding death history and drift.
- **Client rows:** a slot reconnects through `reconnect_slot` after its client
  reports the death, as a bot would. Actions go only to slots that are playing and
  not processing an earlier decision.

`--profile` replaces `step` with `step_profiled`. Profiled `-bot` rows also report
`observeSeconds`, the time spent in the batch observation.

Reports keep the `slither-perf-baseline-v1` envelope. Scenario details add the
definition, lifecycle counts, population statistics and warmup results.

## Validity checks

Every mode checks a deterministic prefix: warmup plus the default trace window,
or the whole run if it is shorter. A steady scenario must show at least one body
death, one respawn and, when death food is enabled, one death-food pellet in that
prefix. Otherwise the harness exits non-zero with the counts.

Population depletion is reported, not fatal. `population` gives the minimum, mean
and maximum living snakes and total anchors at operation boundaries; `depleted`
is true when the mean is below 90% of the requested population. Check it before
interpreting throughput.

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

Snapshots and warmup statistics are taken right after the operation, before dead
slots respawn: respawning resets a slot's per-life state, which would otherwise
hide changes to a just-died snake from parity comparison. Scenario respawn records
carry each new pose and length; client slots reconnect before the next decision.

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
No runner, workflow or table edit is needed for a new registration.

## Observation workloads

The immutable `perf_observation` harness separates observation materialization
from output destruction. It performs no physics stepping and has no workload
catalog. It screens geometry/selection costs rather than long-running sessions.

All six cases use seed 7, 32 living slots, 32 body anchors per slot, eight prey,
and vision 1,500. Body anchors extend from each ring-start head at 42-unit spacing.
Food counts below are admitted pellets, not placement attempts.

| Case | Geometry and API | Food | Baseline iterations/sample |
| --- | --- | ---: | ---: |
| `observe-dense` | Radius 6,000, `observe_all`; exactly the original `observe` world. | 11,321 | 1,000 |
| `observe-compact` | Radius 1,000, `observe_all`; synthetic ambient cap disabled, tails extend outside the rim; never stepped. | 2,000 | 500 |
| `observe-wide` | Radius 50,000, `observe_all`. | 11,999 | 2,000 |
| `observe-strip` | Radius 50,000, heads on x=0 at 750-unit y spacing; remapped food within x=±75, y=±48,000; `observe_all`. | 11,999 | 1,000 |
| `observe-one` | Dense world, `observe_selected` for slot zero. | 11,321 | 10,000 |
| `observe-empty` | Dense world, all-false selection mask; still returns 32 empty slots. | 11,321 | 10,000 |

The strip remap consumes no additional RNG and retains insertion order, values,
sizes and origins. These overrides are explicit workload construction, with no
package default changes. The wide/compact/strip cases expose different filtering
and copying costs; they are not weighted samples of production worlds.

### Reproduction and metric scope

Build `cargo build --release --locked --example perf_observation -j 2`, using the
same toolchain, release profile and source file for each runtime. Copy and hash
each executable into separate owned output storage. Opt in to these cases explicitly:

```sh
python -B tools/perf_compare.py --baseline BASELINE --candidate CANDIDATE --output outputs/observation-comparison --cases observe-dense observe-compact observe-wide observe-strip observe-one observe-empty
```

The comparison uses 10,000 wide batches and 100,000 empty selections per sample
to reduce Windows process-CPU quantization; other counts match the table. It
alternates adjacent pairs, with one untallied warmup per executable/case and five
measured pairs by default. Agree a quiet window before timing; finish builds,
tests and traces first. Do not time a mutable Cargo-target executable.

Materialization includes index construction, filtering, ordering, copying and
internal temporary destruction. `outputDrop` separately measures destruction of
the returned observations. Both use `Instant` wall intervals, without internal
phase attribution. Process CPU/wall include setup, both intervals, clock/sample
overhead, report serialization and teardown. CPU is Windows kernel+user time,
quantized; peak working set is resident memory, not total allocation. The runner
reports CPU/peak unavailable on other platforms. Throughput and operation tail
latencies describe batches with the stated selection mask.

Historical measurements, rejected candidates and the pinned phase-diagnostic
recipe live in the [observation study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-23-observation-batches/REPORT.md).
That diagnostic recipe targets a historical implementation, not the current
2D food grid. New profiling work must start from the current source.

## Retained client workload

`examples/perf_client_retained.rs` extends the short portable/protocol fixtures
in the main catalog. It runs a seed-7 synthetic world for
up to 384 decisions, six 7 ms ticks per decision: 16.128 virtual seconds. Eight
slots retain their connections and decoders; slot zero reconnects immediately
after decision 192. Death does not reset the world. Native background seekers
may reconnect dead background slots in `next_frame` mode.

The explicit workload overrides are radius 6,000, 12,000 food placement attempts,
eight prey, vision 3,000, eight agents, zero added link/processing delays, wire
recording and a 136-square minimap. Broadcast/decision, minimap and leaderboard
cadences are six ticks, 2.5 seconds and three seconds. Other defaults remain
unchanged. Normal spawn bodies are used; available anchor counts are reported.
These are local engineering fixtures, not official-server presets.

```sh
cargo build --release --locked --example perf_client_retained -j 2
```

Copy the executable to `outputs/retained-client/bin/` in a new owned output
directory and record its SHA-256, actual runtime/harness source, toolchain,
platform and build command before use.
The embedded source label identifies fixture construction history only. The
caller must pin the actual binary and runtime separately.

```sh
./outputs/retained-client/bin/perf_client_retained step_all 384
./outputs/retained-client/bin/perf_client_retained next_frame 384
```

On Windows, use `./outputs/retained-client/bin/perf_client_retained.exe`.

Each invocation creates a fresh world. `--iterations N` permits one to eight
worlds with the same seed. `step_all` supplies controls for every playing slot;
`next_frame` supplies slot-zero controls and uses the actual built-in background
seekers. The modes produce different worlds and do different work. Compare a
given mode only against the same mode, configuration, seed and decision count.
There are no sockets, listeners, `BotRunner` or measured-compute feedback into
virtual time.

After every decision and lifecycle checkpoint, the harness drains records for
all eight slots, applies them to persistent independent public decoders,
materializes their frames, and checks equality with backend frames. Wire and
coverage counters retain array order and distinguish each connection generation.
Reports include actual living populations, food and available anchors, packet
codes, full/delta maps, leaderboards and reconnects. False coverage flags mean a
case did not occur; they do not establish an error or permission to change policy.
Sector-clear packets witness exits without revealing their private expiry reason.
Counts from available observations do not treat dead/waiting frames as empty worlds.

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
