# Performance workloads

`examples/perf_baseline.rs` builds its argument parsing, usage text and catalog
from one registration table. Every case is synthetic and offline, with seed 7
and the 7 ms physics tick. The cases measure engine cost, not production worlds
or bot playing strength. See the [performance guide](performance.md) for builds
and comparisons.

## Catalog

```sh
cargo run --release --locked --example perf_baseline -- --list
```

`--list` prints one line of canonical JSON (sorted keys, format
`slither-perf-catalog-v1`) and exits 0. Each workload, in registration order, has:

- `name`: the `CASE` in
  `perf_baseline CASE ITERATIONS [--profile|--trace|--blocks M] [--parallel]`.
  `--blocks M` (1–16) runs the timed case M times in one process for
  [calibration](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/pr-reports.md#calibration).
  Each block rebuilds the case from the seed, warmup included, and the report
  lists every block's timing and elapsed time under `blocks`.
- `suite`: `ci` preserves the ten deterministic rows used by local/direct release
  comparisons; `full` adds optional rows. PR timing selects six named rows
  separately (see below).
- `iterations` and `traceIterations`: the default timed sample size and parity
  window. Trace windows aim for about 64 MB uncompressed or less. The 500-agent
  client row uses the minimum of one decision and still produces about 82 MB,
  including the initial and warmed snapshots and all client frames.
- `mode` and `operation`: informational labels such as `official` and
  `step+observe`.
- `profile`: whether `--profile` is supported.
- `available`: false when a required feature (`networking`, or `perf-internals`
  for the engine row) is absent. The row is still listed; running it fails.
- `legacy`: true only for the six historical cases.
- `synthetic`: null, or what is synthetic about the scene.
- `definition`: the resolved scene, including the serialized `Config`. A changed
  definition means the same name now measures different work.

## Families

The Windows product's separate native frame workload (`client/docs/frame-performance.md`)
measures synthetic offscreen preparation, resource synchronization, render
submission and calling-thread heap bytes on each exposed hardware adapter.
It is outside the simulator timing/trace suites below.

- **Legacy:** `step-default`, `step-food`, `step-body`, `observe`, `portable` and
  `protocol` come first; the catalog holds their resolved definitions.
- **Sandbox and official:** `Config::sandbox()` and `Config::official(n)` at
  several populations, giving a scaling curve.
- **Battledome:** `battledome-32-step` and `battledome-32-bot` use the unmodified
  [`Config::battledome()`](configuration.md#battledome): pit, permanent size
  floor, circular spawn band, ambient food and rolling kill score.
- **Client admission:** `client-battledome-64` and `client-official-500` run
  `SimulatorBackend` with wire recording and need the `networking` feature. Each
  timed decision includes `reconnect_slot` admission for dead clients plus
  `step_all`. The 500-agent row exercises the target host population.
  - Battledome sets responsive delivery and the official row official delivery,
    so default changes do not move either row.
    It warms for 600 decisions (25.2 virtual seconds), past the synthetic ring
    crash and into ongoing churn, then times 40 decisions. Check
    `population.depleted` and refusals for living counts.
  - Refused clients retry after 24 decisions (1.008 virtual seconds); successful
    lives reconnect after their next reported death. This deterministic harness
    policy keeps per-slot retries, unlike circular hosts, which share an
    admission queue with socket priority and a 14 ms failed-search backoff. It
    does not measure host waiting or wall-clock scheduling. Both compared
    runtimes use the same harness policy.
  - Official warms for eight decisions, so its initial death and respawn burst
    stays in the measured window.

PR timing runs six rows: `step-food`, `observe`, `official-100-step`,
`official-100-bot`, `host-official-500` and `host-battledome-80`. Ten existing
trace windows remain: the six legacy cases, the two official-100 rows,
`battledome-32-step` and `client-battledome-64`. Retiring a timing row does not
remove its trace, local catalog entry or calibration workload.

The socket rows reuse `examples/host_load.rs`: 500 real loopback socket clients
in a fixed official-size world, or 80 socket clients with the Battledome preset.
They run without bot workers. Each client sends a scripted circle command every
100 ms and logs in again after death; a separate child process drains messages.
Linux sampling pins all host threads to one logical CPU and the driver to a
second. Samples open every socket, warm for one second, allow up to five additional
untimed seconds for every slot to connect, then observe at least 286 physics
ticks with a 20-second progress timeout and every socket connected at both
measurement boundaries. Death reconnects and temporary capacity refusals wait
100 ms before retrying while other clients continue draining; refusal counts remain. They report process CPU over
that window, simulation busy wall, elapsed wall, tick/deferred counts, connected
and living-client diagnostics. CPU excludes the child driver. Counts normalize
to the requested ticks; raw actual counts remain. These rows support timing,
not `--trace`, `--profile` or `--parallel`. The PR collector retains the ten
older deterministic traces instead. Short runs include admission and churn;
connected sockets do not prove the same count of living snakes.

The [audit](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-pr-report-audit/REPORT.md)
explains the selection. The head's resolved Config values are supplied to both
PR runtimes; defaults changing no longer inherently make a row unavailable.

Sandbox, official and Battledome registrations expand into `<id>-step` (one
`Simulator::step`) and `<id>-bot` (`step`, then batch observation into a reused
buffer through `observe_all_into`). When `perf_compare --base` targets an older
API without it, the harness falls back to `observe_all`.

## Battledome spawn row

`engine-battledome-spawn-318` is an optional `full` row for spawn admission and
the simulation tick in a crowded Battledome. It needs the `perf-internals` Cargo
feature, which also enables `networking`. That feature exists only for this
harness: it is off by default, and its hidden internals carry no stability or
semver promise. Ordinary builds and the packaged build script are unchanged.

**Scene.** The geometry comes from the 320-slot synthetic constant-boost fixture
of the [spawn index study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-spawn-obstacle-index/REPORT.md),
with 318 snakes alive. `examples/support/catalog_spawn.bin` (317,440 bytes)
stores little-endian f64 values: an alive flag for each slot, then head x/y,
heading, fractional mass, speed, body count and ordered body x/y for each living
slot. It holds no session data or recording. Lives and timers start at zero in a
fixed-radius 11,333.603134043473 Battledome; pit, spawn band and ecology scale
with that wall before automatic sizing is turned off. Food is the first 18,346
pellets admitted by a separate seed-7 construction with `foodCount=100000` before
pit-area scaling (at most 100,000 attempts, ordinary ambient cell limit).
Recipients attach before the first tick, and one untimed initial broadcast (about
0.067 s) gives them presentation history, so later food births flow through the
backend. The scene is derived from the study's geometry rather than continuing
its checkpoint, and setup never repeats the study's 642.684-second warmup.

**Operation.** Each operation runs two complete waited=0 spawn searches (RNG seeds
40 and 0), timing construction, refusal and pose selection without installing
poses. It then runs one ordinary constant-boost tick on the raw host's serial
simulation thread, including per-recipient stock protocol-19 encoding and food
presentation. There are no sockets, client decoders or bot workers. Controls,
construction, wire draining and serialization stay outside the timed interval.
The default sample has 192 operations. The scene is rebuilt every 16 ticks, so
each 112 ms epoch ends before the first food-update tick. On Linux,
`timing.operationCpuSeconds` sums process CPU over the same operations; elsewhere
it is null. `--profile` is unavailable because the raw host tick has no phase
profile, and `--parallel` is rejected. Reports count both search outcomes and the
minimum living population. The census adds mean known snakes, held food entries
per recipient and body points.

**Parity.** Traces cover four operations. They retain the initial and final
authoritative state, the census and the ordered per-tick wire and search results.
Presentation and connection internals are not serialized, so parity covers only
these outputs and state boundaries. Public checkpoint and state adapters build the
world, and hidden library hooks attach recipients, advance raw ticks and run
searches, so all measured engine code is the compiled library. The comparison
overlay copies only helpers and geometry. A base older than the hooks needs the
same recorded hook-only patch on both sides.

```sh
python -B tools/perf_compare.py --base BASE --features perf-internals --output outputs/perf/engine-check --cases engine-battledome-spawn-318
```

**Fixture.** The catalog definition pins the fixture's SHA-256; it was last
regenerated at `556e73f0`. The ignored `regenerate_spawn_fixture` test rebuilds it
from seed 7 with 15,302 decisions and checks the living and anchor counts before
writing. It is a slow, explicit maintenance step, separate from catalog setup:

```sh
cargo test --release --locked --features perf-internals --example perf_baseline regenerate_spawn_fixture -- --ignored --nocapture
```

The row measures engine cost in a prescribed synthetic world, not live capacity.

## Shared-policy workload

`shared-forager-a4-mean-8` is a separate local-only row. It runs eight synthetic sandbox
clients with one shared A4-mean `ForagerBot`, then `step_all`. Its timed operation
includes all decisions plus the six physics ticks and all returned frames;
wire-record draining remains outside that interval. It uses modeled virtual
processing only, resets the world after a death, and records the policy and
original frozen parameter-manifest hash in its catalog definition. `--trace` disables
timing and retains actions, frames and wire records in order. This replaces
`shared-forager-a1-8`; historical measurements retain that identity and must not
be compared as the same workload.

Existing `-bot` rows still mean scripted wander plus step and observation; they
do not time Forager. Legacy and client-admission rows retain their controllers
and workload identities. The new row is a bounded integration workload, not a
host capacity or playing-strength measurement; host capacity comes from the
[500-bot host qualification](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-500-bot-qualification/REPORT.md).

## Steady-state lifecycle

Legacy cases reset after any death. Scenario rows keep deaths, death food,
respawns and grown bodies:

- **Placement:** added slots are marked dead and enter through
  `Simulator::respawn` at poses that pass `Simulator::spawn_is_clear`, with
  distances from `spawn::sample_spawn_distance`, straight bodies and segment
  counts from the registered range.
  - Battledome instead samples the configured band and uses
    `Simulator::spawn_body`, `body_is_clear` and `respawn_body` for floor-size,
    curved newborns. Its `synthetic` label discloses that the harness uses
    contact clearance, not host admission.
  - Band capacity limits simultaneous newborns, not the eventual population.
    Initial placement tries 64 poses per slot and queues blocked lives instead
    of failing construction. A refused band placement retries after 144 ticks
    (1.008 virtual seconds), giving existing lives time to move away.
  - The 32-slot row warms for 8,000 ticks; reports keep `initialQueued`, the
    initial living count and the warmup and measurement populations.
- **Randomness:** harness pose and length generation never consumes simulator
  RNG, so lifecycle work leaves world randomness unchanged.
- **Control:** a stateless wander controller picks a heading per staggered
  window, occasionally chases another head, steers inward near the edge and
  boosts in a staggered window. Control and lifecycle work stay outside the
  timed interval.
- **Respawn:** between operations, a cursor reads new deaths without modifying
  `world.deaths`. Slots queue in death order with bounded placement attempts.
  Blocked non-band slots retry a few ticks later; band slots use the interval
  above.
- **Warmup and epochs:** an untimed warmup runs first. Long samples restore the
  warmed scene every 4,096 operations, bounding death history and drift.
- **Client rows:**
  - A slot reconnects through `reconnect_slot` after its client reports the
    death, as a bot would. Backend initialization is a synthetic ring start, so
    only later reconnects exercise admission. Actions go only to slots that are
    playing and not still processing an earlier decision.
  - Admission and stepping are timed in disjoint intervals summed into one
    operation; controller calculation and wire draining are untimed.
    `admissionCalls` counts every measured reconnect attempt, including refused
    poses. `admissionSeconds` is their time (null in traces) and
    `admissionSecondsPerDecision` divides it by the measured decisions.
    `reconnectTimed` is true; warmup is never timed.
  - `windows` reports populations and lifecycles per 100 decisions, split at
    warmup and sample boundaries. `refusalsPerDecision` and
    `respawnsPerDecision` separate the initial crash, the warmup tail and
    measured churn.
  - The default trace window must include admission work. The four-decision
    Battledome parity window includes refused admissions; the full 40-decision
    sample also includes deaths and successful respawns.

`--profile` replaces `step` with `step_profiled`; profiled `-bot` rows also
report `observeSeconds`, the batch observation time. Reports keep the
`slither-perf-baseline-v1` envelope, and scenario rows add the definition,
lifecycle counts, population statistics and warmup results.

## Validity checks

Every mode checks warmup plus the default trace window, or the whole run if it is
shorter. That deterministic prefix must contain a body death, a respawn and, when
enabled in a world without a size floor, a death-food pellet. Otherwise the
harness exits non-zero with the counts. Size-floor worlds skip the death-food
requirement: legacy floor budgets or the pit-position filter can reject a drop.
Pit deaths are reported
separately.

Population depletion is reported, not fatal. `population` gives the minimum,
mean and maximum living snakes and the minimum and maximum total anchors at
operation boundaries. `depleted` is true when the mean falls below 90% of the
requested population; check it before reading throughput.

## Traces

`--trace` emits untimed canonical JSONL. Legacy traces are unchanged. Scenario
traces contain:

1. An `initial` full snapshot after placement.
2. One light `warmup` line per tick (actions, deaths, respawns, events, world
   statistics and the harness RNG state), then a `warmed` full snapshot.
3. One line per measured operation (`step`, `bot` or client `decision`) with its
   actions, deaths, respawns and a full snapshot. `-bot` lines add the
   observations; client lines add frames and wire records.
4. An `epoch` snapshot whenever the warmed scene is restored.
5. A final `end` record with the operation count, physics ticks and lifecycle
   counts, followed by the usual report line. A trace without `end` is
   incomplete.

Snapshots and warmup statistics come before respawns, so a per-life reset cannot
hide a just-died snake's state from parity checks. Respawn records keep the new
poses and lengths; client slots reconnect before the next decision.

`--parallel` calls `Simulator::set_parallel(true)` on the timed world, so
`observe` and `-bot` rows observe on the rayon pool. It needs the default
`parallel` feature and adds `"parallel": true` to the report; every other trace
line must match the run without it. Runners and PR comparisons don't pass it.

## Adding a workload

1. Add one registration to `REGISTRATIONS`. A new world needs a `World`
   variant, whose exhaustive matches require a config, mode, label and
   definition.
2. Calibrate `iterations` so one release sample takes about one second, and pick
   `traceIterations` so the uncompressed trace stays near 64 MB or less.
3. Run the row with timing, `--profile` and `--trace`. Confirm the validity check
   passes, the population isn't unexpectedly depleted, and two traces are
   byte-identical.
4. Keep at most ten `ci` rows and run `cargo test --locked --example perf_baseline`.

Give an incompatible workload a new name so archived results stay interpretable.
PR runs pin head Config defaults on both runtimes; other definition differences
show a head-only Redefined value with a reason when possible. A new
registration needs no runner or workflow edit, but when retiring a row, the
calibration pilot's explicit groups must still name available workloads.

## Observation workloads

The immutable `perf_observation` harness isolates geometry and selection costs,
timing materialization separately from output destruction. It has no physics
stepping or catalog and doesn't measure long-running sessions.

The first six cases use seed 7 and 32 living slots with 32 body anchors each
(42 units apart from a ring-start head), eight prey and vision 1,500. Food
counts below are admitted pellets, not placement attempts.

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

Strip remapping preserves RNG, insertion order, values, sizes and origins. The
sparse lattice spreads eight observers over many grid cells. In the clustered
case, patches beyond all vision make mean density overstate visible food, and
two-anchor bodies keep snake filtering from hiding grid cost. These overrides
leave package defaults unchanged; they expose filtering, copying and index costs,
not a weighted production mix.

### Reproduction and metric scope

Build both runtimes with `cargo build --release --locked --example perf_observation -j 2`,
using identical toolchain, profile and harness source. Follow the
[copied-executable identity requirements](performance.md#compare-copied-executables)
and select cases explicitly:

```sh
python -B tools/perf_compare.py --baseline BASELINE --candidate CANDIDATE --output outputs/observation-comparison --cases observe-dense observe-compact observe-wide observe-strip observe-one observe-empty observe-sparse observe-clustered
```

The runner uses 10,000 wide batches and 100,000 empty selections per sample to
reduce Windows CPU quantization; other counts match the table. It follows the
[local comparison](performance.md#compare-a-change) warmup, paired ordering,
five-pair default and quiet-window rules. Finish traces before timing.

Materialization covers indexing, filtering, ordering, copying and internal
temporary destruction; `outputDrop` separately times destroying the returned
observations. Both are `Instant` wall intervals without phase attribution.
Process CPU and wall time also include setup, clock and sample overhead,
serialization and teardown, and the [CPU and peak scope](performance.md#read-the-results)
applies: CPU is quantized Windows kernel plus user time. Throughput and tail
latencies describe batches with the stated selection mask.

The [observation study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-23-observation-batches/REPORT.md)
holds historical measurements, rejected candidates and a pinned phase-diagnostic
recipe. That recipe targets an implementation before the current 2D food grid,
so new profiling must start from current source.

## Retained client workload

`examples/perf_client_retained.rs` extends the catalog's short portable and
protocol fixtures: a seed-7 synthetic world run for up to 384 decisions of six
7 ms ticks (16.128 virtual seconds). Eight slots keep their connections and
decoders; slot zero reconnects after decision 192. Death does not reset the
world, and in `next_frame` native background Foragers may reconnect dead
background slots.

The workload overrides radius (6,000), food placement attempts (12,000), prey
(8), vision (3,000), agents (8) and minimap size (136 square). It adds no link or
processing delay and records wire output. Gameplay updates go out every tick
under the [per-family release rules](server-rules.md#packet-timing); minimap and
leaderboard intervals are 2.5 and 3 seconds. Other defaults are unchanged and
spawn bodies are normal, with available anchor counts reported. These are local
engineering fixtures, not official-server presets.

```sh
cargo build --release --locked --example perf_client_retained -j 2
```

Copy the binary into a new `outputs/retained-client/bin/` and record the platform
and the [binary, source and build identities](performance.md#compare-copied-executables).
The embedded source label records how the fixture was built, not the identity of
the binary you run.

```sh
./outputs/retained-client/bin/perf_client_retained step_all 384
./outputs/retained-client/bin/perf_client_retained next_frame 384
```

On Windows, use `./outputs/retained-client/bin/perf_client_retained.exe`.

Each invocation starts fresh; `--iterations N` runs one to eight same-seed worlds.
`step_all` controls every playing slot, while `next_frame` controls slot zero and
lets built-in Foragers drive the rest. The modes do different work, so compare
only runs with matching mode, configuration, seed and decision count. `next_frame`
uses A4-mean and its live traces can differ from historical seeker runs; those
runs are not a behavior-preserving comparison. There are no sockets, listeners,
`BotRunner` or compute-time feedback into virtual time.

At every decision and lifecycle checkpoint, the harness drains all eight slots'
records, replays them through independent public decoders, builds frames and
checks them against the backend. Wire and coverage counters keep array order and
connection generations. Reports keep living populations, food, available
anchors, packet codes, full and delta maps, leaderboards and reconnects. False
coverage means a case didn't occur, not an error. Sector-clear packets show
exits, not private expiry reasons, and dead or waiting frames are not empty
worlds.

Wall intervals separately report backend operations, external action
submission, extra backend frames, record draining, replay application, replay
frames and manual reconnects. They exclude control calculation, equality checks,
counters, serialization and returned-output destruction, which process CPU and
wall time include. Physics, encoding, delivery and decoding stay combined inside
the operation, and `next_frame` also includes background-policy frame
construction. Agree a quiet interval before reading timing, keep every raw
sample, and report process CPU and peak memory separately from the inner wall
intervals.

`--trace` requires one iteration and disables all timing. It emits the full
available core state (including serialized core RNG and clocks), latest-tick
events, direct observations, ordered actions, frames and wire records. Only JSON
object keys and the internal unordered `prey_outward` set are canonicalized.
Traces can be large: start with a few decisions and bound longer runs. Use
identical harness bytes and whole-file comparisons when checking a runtime
optimization.

Private backend RNG, transport queues, subscription and presentation state, and
the event buffers of the five intermediate ticks are not reachable through these
public seams, so a matching trace does not prove full private-state parity. It
does keep the sent background commands rather than substituting another policy.
The fixture doesn't stage 128-anchor bodies or assert that `V`, death or sector
expiration occurs; check the coverage report before claiming any of them.
