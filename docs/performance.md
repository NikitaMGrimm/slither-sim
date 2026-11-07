# Performance checks

Offline synthetic workloads measure simulator/client costs, not official servers
or bot playing strength. Check output parity before claiming a behavior-preserving
speedup.

Automatic PR comparisons use a [covered sequential protocol](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/performance-statistics-v2.md)
with early resolution, a 30-minute job cap and at most three interim comment updates.
The commands below use fixed-repeat descriptive comparisons.

The separate [on-demand release comparison](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/release-performance.md)
measures a pinned published release against an explicit candidate commit.

## Compare a change

```sh
uv run --locked --only-group tools python -B tools/perf_compare.py --base origin/main --output outputs/perf/my-change
```

Builds the base ref and working tree with one harness, checks full-state traces,
records supported phase profiles and times adjacent alternating pairs. Use a new
directory outside the checkout or under ignored `outputs/`. Finish builds/tests
first, arrange a quiet window and record interruptions/background activity.

| Option | Purpose |
| --- | --- |
| `--base REF` | Named runtime baseline; the candidate is the current working tree. |
| `--suite ci` / `--suite full` | Choose the catalog suite; `ci` is the default. |
| `--cases NAME ...` | Select explicit workloads; unknown names fail before measurement. |
| `--features FEATURES` | Enable the same Cargo features on both `--base` builds; the Battledome row needs `perf-internals` (hidden benchmark-harness internals, no stability or semver promise). |
| `--repeats N` | Measured pairs after one untallied warmup; default 5. |
| `--no-profiles` | Skip separate instrumented runs. |
| `--max-wall-seconds N` | Bound measurement time after builds; see below. |

The snapshot includes tracked edits/deletions and untracked non-ignored files.
A temporary index preserves your index, HEAD, stash and files. The candidate's
measurement examples, selected tests and fixtures overlay the detached baseline;
this shared harness must compile against the base API.

`examples/support/runtime.rs` adapts construction and borrowed reads across public
and private state APIs. Its untimed initial-fixture checkpoint import preserves
join order and idle speed as well as ecology RNG and every other timer; new-life
replay imports would intentionally reset those controls. The build identity
records both runtime/state adapter selections and their source hashes.

`examples/support/state.rs` supplies compile-selected state adapters for legacy
Simulator serde and validated checkpoint v1/v2. The overlay copies that adapter and
`build.rs` into both revisions; the build script selects the API from the runtime
source, without changing it. Workloads and measurement code stay shared. The
checkpoint adapter projects every authoritative field back to the legacy shape,
preserving RNG, failures, timers and ordered arrays; only the envelope, slot
grouping, field spelling and optional null representation are normalized.
Producer provenance is outside state comparison. Unprojected fields fail instead
of disappearing. Build identities record adapter selection and source hashes.

For the runtime-radius split, checkpoint v2 and geometric v4 are projected onto
v1/v3's historical live-radius representation for comparison. The explicit live
radius is retained; only the intended Config/radius placement and contract labels
are normalized. Separate ordered radius traces retain the complete original
checkpoints and snapshots, check initial Config invariance and compare every
other field without omission.

Separate persistent targets live under `target/perf-suite/` (or
`CARGO_TARGET_DIR`). Locks refuse concurrent use; verify ownership before removing
stale locks. Freshly built binaries are copied to output, bound to source/tree,
harness, toolchain and binary hashes, and rehashed around timing. Temporary
worktrees are removed afterward.

## Read the results

| Exit | Meaning |
| --- | --- |
| 0 | Complete, with equal deterministic traces; socket rows are explicitly untraced. |
| 3 | Complete, with changed traces or workload definitions; timings are labeled behavior-changed. |
| 1 | Incomplete; inspect `failure.json`, logs and retained partial outputs. |

`summary.md`/`summary.json` hold paired ratios; `comparison.json` holds the plan,
parity and build identities. Catalogs must select identical names, iterations
and trace windows. `--base` overlays the candidate harness on both runtimes,
so added and retired workload names use that shared catalog. Copied executables
with one-sided names fail before timing with a plan-mismatch diagnostic; select
common names explicitly to compare them. Incomplete attempts support no speed claim. Changed behavior
may suit a mechanics correction, but is not a behavior-preserving optimization.

Operation wall time excludes setup, trace output and report serialization;
process CPU/wall includes them. Peak working set measures resident pages, not
allocation or total system memory. CPU/peak metrics are Windows-only, explicitly
unavailable elsewhere; short CPU samples can be quantized to 15.625 ms.
Separate instrumented profiles have clock/counter overhead and cannot establish
unprofiled speedups.

Inspect living populations, admitted food and operation counts before comparing
throughput. Compare identical workloads, seeds, sample sizes, machine, toolchain,
build flags and harness bytes. These fixtures are not a production distribution;
small or mixed timing shifts remain inconclusive.

Trace mode is untimed and retains canonical full simulator state, serialized
RNG/motion state, events and relevant observations/client frames/wire bytes.
Compare complete decompressed bytes and require a terminal scenario record.
Real-time host checks use deterministic wire replay and host contract tests,
without asserting equal wall-clock receipts.

## Workload selection

```sh
cargo run --release --locked --example perf_baseline -- --list
```

The catalog owns names, suites, sample sizes, resolved definitions and profile
support. The [workload guide](performance-workloads.md) explains scenario
lifecycles, validity checks, adding cases and the specialized observation/retained
client harnesses. [PR comparisons](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/maintainers/pr-reports.md) use the same catalog.

## Observation batches

The [workload guide](performance-workloads.md#steady-state-lifecycle) defines
`-bot` profiles and `observeSeconds`; its [observation fixtures](performance-workloads.md#observation-workloads)
isolate materialization. The [food index study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-observation-food-index/REPORT.md)
measures those fixtures and `official-128-bot`/`dome-proxy-96-bot`, sequentially
and with `--parallel`. Food visibility filtering dominates batch cost.

A batch builds a food grid with at least 8 observers, at least 32,768 observers ×
pellets and vision from 0 to 2^40; smaller batches scan directly. It is rebuilt
per batch, refining cells only when enough observers repay them. Construction is a small
share of sequential cost but precedes the parallel split; the study explains
why it is not reused across steps.

With at least 256 snakes and eight observers, a batch also builds a bounds tree
to skip distant snakes. Very long bodies need only one tree entry. Candidates
are restored to world order before the unchanged visibility predicate runs;
extreme or nonfinite bounds retain the original per-snake check. Those batches
with at least 262,144 food items also use an additional bit per food-mark word
to skip empty words when restoring food order. Both indexes live only for the
batch's immutable world borrow.

## Parallel step

`Simulator::set_parallel(true)` and the host's
[`worldThreads`](hosting.md#world-threads) also split a step over the current
rayon pool once at least 4,096 snakes are alive: one task per 512 living
snakes, at most four per thread. Three phases split:

- **Movement.** A serial pass first applies boost and drops boost food in slot
  order, because drops draw random numbers and add food. Steering, walls, the
  pit, new anchors and kiosk penalties then touch only their own snake and run
  in parallel.
- **Collision search.** Each head finds the newest body it strikes as if no
  snake had died this tick. The usual oldest-first walk then decides deaths in
  order. When a head's first strike is a body that already fell earlier in the
  walk, that head repeats the serial search, so deaths and kill credit match.
- **Food pickup lookup.** Occupied eater cells are split; the hits are sorted
  back into the serial order before anything is eaten.

Index building, deaths and death food, nutrition, prey, ambient refill and
arena radius stay serial. With the flag off, or below 4,096 living snakes,
every phase runs the serial code. Phase profiles still time each phase; with a
split collision search, `bodyPairs` and `segmentTests` also count searches
against bodies that fell earlier in the walk.

`src/simulator/parallel_step_tests.rs` steps official, dense, Battledome and
constructed collision-order worlds on pools of 1, 2, 3, 4 and 7 threads and
requires deaths and recorded events to match the serial world every tick.
Checkpoint digests match every 3 ticks for the official world, every 8 for the
dense world and every 10 for Battledome, plus each final tick. Constructed
edge cases compare checkpoints every tick.

### Measured scaling

On an AWS c8a.4xlarge (16 Zen 5 cores, no SMT, two 32 MiB L3 halves) on
2026-11-05, median of 5 alternating runs (3 for the 4,096-snake world):

| World | Serial | 2 threads | 4 | 8 | 16 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `dense-8192-step` mean | 3.76 ms | 3.60 ms | 2.78 ms | 2.39 ms | 2.23 ms |
| `dense-8192-step` p99 | 4.84 ms | 4.70 ms | 3.88 ms | 3.47 ms | 3.33 ms |
| 4,096 snakes, official density, mean | 2.17 ms | 2.05 ms | 1.75 ms | 1.54 ms | 1.55 ms |
| `official-500-step` mean | 185 µs | 185 µs | 185 µs | 185 µs | — |

A pool of 8 makes the densest world 1.6 times faster, and 16 threads add
little. Each split phase keeps serial work: the collision grid build, the boost
pass and the resolution walk. Splitting also pays a second pass over snake
memory and pool dispatch. Below the threshold nothing splits:
`official-100-step` (32 µs) and `battledome-32-step` (7.5 µs) stayed within
0.2% with 8 and 16 threads. An earlier 1,024-snake threshold made 1,024-snake
official worlds about 10% slower than serial on every pool size, and 2,048
snakes gained nothing on two threads; that set the 4,096 minimum. Traces of
`official-500`, the 1,024–4,096-snake worlds and `dense-8192` were
byte-identical on 1, 2, 4, 8 and 16 threads, apart from the report's
`parallel` flag.

A host gains less. Official sizing stops at 2,281 seats, below the minimum, so
only a fixed-radius arena splits. `host_load --bots 4500 --radius 32000` spent
16.8–18.2 ms of simulation time per tick with one world thread and 15.8 ms with
eight. The world was about twice as dense as official sizing. Per-player
work dominated, and both fell far behind the 8 ms tick. At 500 bots, four
world threads changed nothing (0.84 ms per tick, no deferred ticks).

To repeat the measurement, run the [dense row](performance-workloads.md#families)
on an otherwise idle machine with at least eight cores, alternating pool sizes:

```sh
cargo build --release --locked --example perf_baseline
B=target/release/examples/perf_baseline
for repeat in 1 2 3 4 5; do
  $B dense-8192-step 200 > dense-serial-$repeat.json
  for n in 1 2 4 8 16; do
    RAYON_NUM_THREADS=$n $B dense-8192-step 200 --parallel > dense-$n-$repeat.json
  done
done
```

The last line of each file is the report: the mean step is
`timing.wallSeconds / timing.operations`, and `timing.latencySeconds` holds p50,
p95 and p99. Run `official-500-step` the same way to check that small worlds
are not slowed down. For a host, compare `host_load --world-threads N`.

## Parallel views

The host's [`viewThreads`](hosting.md#view-threads) assembles each player's
update on a separate pool after physics. For every set-up recipient a worker
runs the same code as the serial loop: game over, radius and rules, which
snakes it sees and their packets, food and decorated food, then prey. It works
on a copy of that recipient's view (its snake and prey histories and a few
scalars) and reads its held food listings without changing them, returning
the listing changes as an edit. Delta caches and food-diff memoization are
local to each task; they only reuse identical encodings, so the bytes match a
shared cache. Stale-report observations are recorded instead of written.

The simulation thread still walks recipients in slot order. At the point
where it used to build a recipient's update, it commits that recipient's job:
the view, the food edits and the stale observations. Nothing reads those
flags during a broadcast, so folding them in later gives the same flags.
Minimap, HUD and frame emission then run unchanged. Workers never touch
queues, decoders or the world, and the world never advances while a wave is
out. A failing job carries its partial view: the commit applies it, as the
serial path would leave it, and returns the error. Later jobs of the wave are
dropped, so those recipients stay as the serial path leaves them.

Recipients go to workers in tasks of 64, in waves of four tasks per thread.
The socket-player prefix and the bot range are separate, so early socket
frames never wait for bot views. A range shorter than 1,024 recipients stays
serial. With `viewThreads` 1 there is no pool and no staging; the serial loop
moves the view out of the connection, assembles and moves it back.

`src/client/simulation/broadcast/perf.rs` replays four raw host scenes with
1, 2, 3, 4, 7 and 8 view threads and chunks of 1 to 64 recipients, splitting
even the smallest ranges, and requires every frame, scheduled time and the
terminal host and view state to match the serial replay:

- grown recipients with pickups, maintenance, a death, slot reuse and mixed
  protocols 13 and 19;
- long crossing bodies, recipients on coverage-cell corners, bodies and prey
  exactly at vision distance and one ULP beyond, and signed-zero heads;
- a Battledome scene with decorated food, measured-route jitter, deaths,
  rejoins and session IDs, with fixed and adaptive dt, and four injected
  failures, each followed by more ticks: a full local queue, a failing early
  socket handoff, a legacy conversion fault that closes one session, and an
  encoding error inside one recipient's assembly;
- a report gate that a newly appearing viewer rearms.

Test-only faults (dropping stale observations, committing jobs in a
neighbour's place, or dropping a failed job's partial view) each change a
replay.

### Measured view scaling

On an AWS c8a.4xlarge (16 cores) on 2026-11-07, release builds without
CPU-specific flags, alternating the base revision and each thread count.
`host_load` used `--bot-threads 7`, one world thread and one watcher; the
values are simulation busy time per tick, median of 3 runs of 40 s (4,500
bots) or 5 runs of 30 s (500 bots). The raw row is the median of 5 alternating
runs; its split columns force splitting below the 1,024 threshold.

| Workload | Base | 1 (default) | 2 | 4 | 8 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `host_load --bots 4500 --radius 32000` | 17.12 ms | 16.99 ms | 18.20 ms | 12.96 ms | 11.50 ms |
| `host_load --bots 500` | 0.843 ms | 0.842 ms | — | — | — |
| `engine-raw-official-500`, mean | 0.882 ms | 0.867 ms | 1.005 ms | 0.863 ms | 0.803 ms |
| `engine-raw-official-500`, p99 | 1.61 ms | 1.60 ms | 2.01 ms | 1.89 ms | 1.82 ms |

Eight threads cut simulation time at 4,500 bots by a third (run ranges
11.37–11.69 against 17.10–17.24 ms) and four threads by a quarter. The host
still fell behind its 8 ms tick. Two threads were slower everywhere: the
simulation thread waits while the pool works, so two workers barely cover the
copying, commits and wave dispatch. In the raw row splitting raised p99 even
where the mean fell. Forced splits on the first revision of this code, at
1,000 bots, ran 1.92 ms (8 threads) against
1.91 ms serial and 2.20 ms (4 threads); at 2,000 bots 3.70 and 4.20 ms against
4.77 ms, so the 1,024 threshold stays.

With eight threads, most of the simulation thread's remaining time goes to
memory copies, allocation and frame grouping during emission, applying food
listing edits, and physics. These stay serial.

The default is not slower. The raw row ran 0.867 against 0.882 ms with 1.0%
fewer instructions and 1.5% fewer cycles, and 500-bot host busy time matched
the base. Two inlining regressions were found and fixed on the way. A second
caller stopped the compiler inlining the coverage test into the serial loop,
which cost 4.7% more instructions; `covered` is now always inlined. Later its
body-edge `any` closures, one generic instance shared by both callers, were
left out of line: 9% of the raw row's samples sat in a separate symbol and the
row ran 1% slower. Plain loops fixed that. Traces of
`engine-raw-official-500` (150 operations, 4 lines, 237 MB) hashed identically
for the base, the default, and 2, 3, 4, 7, 8 and 16 view threads with
splitting forced, apart from the report's `viewThreads` field.

## Controller transport

`cargo run --release --locked --example perf_session` writes an untimed JSONL
trace of two-world sandbox, official-128 and Battledome sessions. Typed and JSON
steps must agree on every frame column and debug snapshot. The shared overlay
uses typed prepared reset where available and the existing JSON reset on older
runtimes; every reset/step reply byte is retained for cross-revision comparison.
`tools/perf_transport.py` measures [`serve` controller](bot-api.md) costs, including
reply reading/parsing, beyond the Rust-only workloads above. Its scenario table
defines `small`, `official-128`, `dense-64`, `heavy-food` and 16-world `worlds-16`.
Deterministic scripts use seed 7, six decision ticks and headings independent of
observations, so all three modes simulate identical worlds:

- `server` pipes the whole script into `serve` and drains stdout through a pipe.
  It times from the end of the reset reply to EOF: simulation, serialization and
  pipe writes, with no client in the loop.
- `python` and `node` are closed-loop reference controllers using only `json` or
  `JSON.parse` (`tools/perf_transport_client.py` and `.mjs`). Per step they time
  **wait** (write the request, block for the reply line: server plus pipe),
  **parse** and **decide**, which folds every living slot's nearest visible food
  into a checksum. Decisions are not sent.

```sh
cargo build --release --locked --bin slither-sim
uv run --locked --only-group tools python -B tools/perf_transport.py --output outputs/transport/run
```

Requires Node.js 22+ on `PATH`; starts only the given executable, with no network
service. Reports mean reply size and median ms/step with min–max over `--repeats`
(default 3), rotating mode order between repeats. `--output` creates a directory
with a private binary copy, scripts, raw `report.json` and `summary.md`.
`--scenarios`, `--modes` and `--steps` bound runs; `--threads N` reaches every
`serve`. Different reply bytes across modes/repeats or client checksums fail the
run. Per-step times exclude reset, startup and request encoding (scripts are pre-encoded).
Protocol-specific code stays in request encoders and client `decode`/`decide`.

The [transport v2 study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-transport-v2/REPORT.md)
holds the `slither-sim-json-v1` baseline, the v2 results and the thread comparison.

## Comparison deadlines

`--max-wall-seconds` shares one monotonic deadline across metadata and measurement
children, after builds. Leave startup, file-operation and cleanup margin. It
cannot extend the 30-second metadata or 120-second native-child limits and is
not an OS CPU/memory cap.

Expiry blocks the next child or interrupts the current one. Cleanup kills it,
waits up to ten seconds and closes pipes/error files. Flushed rows remain;
`failure.json` records budget and elapsed time. Failed native attempts retain
exact stdout in `.stdout.bin` and metadata in `.failure.json`, replacing the
stderr file's `.txt` suffix. Diagnostics are not completed measurements;
incomplete runs have no summary. Rehash retained binaries and use fresh output
for another attempt.

## Compare copied executables

For specialized harnesses or copied-binary studies, build each snapshot with the
checked-in lockfile, release profile and separate targets. For the baseline/CLI:

```sh
cargo build --release --locked --bin slither-sim --example perf_baseline -j 2
```

Copy executables out of mutable Cargo targets into separate owned output storage
before timing. Record build command/log, immutable source identity (including edits), toolchain and binary
SHA-256 at build time. Hashes/caches alone do not prove source identity, and these
runners do not enforce it. Rebuild uncertain inputs before parity and timing.

```sh
python -B tools/perf_compare.py --baseline PATH/TO/BASELINE --candidate PATH/TO/CANDIDATE --output outputs/perf-comparison
```

This mode defaults to the catalog's `ci` suite and accepts `--suite`/`--cases`;
it does not trace. Both catalogs must agree. The separate `tools/perf_baseline.py`
runner also accepts `--harness` and `--cli` for end-to-end reports. Its
`e2e-client` includes Python/JSON/process overhead; `e2e-host` additionally runs a
temporary loopback host and is cadence-limited. The host is stopped on success or
error. Hosted receipts have no deterministic response hash.

The runners use standard-library Python. Existing locked environments can be
reused with `uv run --project PATH --locked --only-group tools --no-sync --offline
python -B ...`. Preserve raw results before cleaning ignored outputs. Dated
measurements and decisions live in the
[research index](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md).

## Rules and typed/JSON transport

`perf_session --bench typed 1000` and `--bench json 1000` compare native Session
step/frame access with JSON request decoding/reply encoding for one 128-slot
world. Both use the same fixed sandbox config and six-tick decisions, resetting
every 100 decisions to bound the episode; reset costs are included. An untimed
warmup precedes measurement. Every measured decision retains 128 living slots.
Use matched release binaries, adjacent alternating pairs and retain both runs.
Python owned-array copies and C call overhead are outside this native case.

The shared frame adapter validates geometric rules against effective Config
before projecting the added metadata and v5 coded-error envelope to the older
trace representation. It never changes portable received frames. Checkpoint
and rules revisions remain v2/v1; complete simulation state is still compared.
