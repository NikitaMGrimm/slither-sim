# Reproducible performance checks

These synthetic, offline workloads measure this simulator and its client tools.
They do not measure official servers or bot playing strength. Name and pin the
baseline for each comparison; historical results are linked below.

## Compare a change

One command builds a base ref and the current working tree, checks output
parity, records step profiles and times both sides:

```sh
uv run --locked --only-group tools python -B tools/perf_compare.py --base origin/main --output outputs/perf/my-change
```

Agree on a quiet window with other local work first. The command builds before
it measures; no other builds or tests should run until it finishes. Record
background activity and any interruption. A quiet-window agreement does not
establish an otherwise idle host.

- **Snapshots.** The candidate is the working tree frozen as a Git tree and
  commit: tracked edits, deletions and untracked non-ignored files are included;
  ignored files are not. A temporary index leaves your index, `HEAD`, stash and
  files untouched. The base ref is checked out as a detached worktree, and the
  candidate's measurement inputs (`examples/perf_baseline.rs`,
  `examples/fidelity_metrics.rs`, the selected tests and `tests/fixtures`) are
  copied into it, so both runtimes run the same harness. That harness must
  compile against the base API. Worktrees live under `OUTPUT/work` and are
  removed afterwards.
- **Builds.** Each side runs `cargo build --release --locked --example perf_baseline`
  in its own persistent target, `target/perf-suite/baseline` or `.../candidate`
  (under `CARGO_TARGET_DIR` when set). A lock file next to each target refuses a
  concurrent run; delete a stale lock only when no run is active. Previous
  executables are deleted before every build, so a failed build cannot reuse
  them. Copies go to `OUTPUT/bin-<label>/`; `<label>-identity.json` binds their
  hashes to the commit, tree, overlaid inputs, toolchain, command and build log.
  Hashes are checked again before and after timing.
- **Measurements.** Both harnesses' catalogs must select the same names,
  iterations and trace windows. Then full-state trace parity for every selected
  workload, step profiles for rows that support them (`--no-profiles` skips),
  and adjacent alternating timing pairs after one untallied warmup
  (`--repeats`, default 5). `--suite ci` (default), `--suite full` or
  `--cases NAME ...` selects workloads. `--max-wall-seconds` bounds the
  measurement after the builds; see [comparison deadlines](performance-deadlines.md).
- **Results.** Exit 0: complete and every trace equal. Exit 3: complete, but a
  trace or workload definition changed; timings are kept and labeled
  behavior-changed. Exit 1: incomplete; `failure.json`, logs and partial outputs
  remain. `summary.md` and `summary.json` hold the paired ratios;
  `comparison.json` holds the plan, parity readings and build identities.

The output directory must be new, and outside the checkout or in an ignored
directory such as `outputs/`, so a run never snapshots its own outputs.

## Workloads

```sh
cargo run --release --locked --example perf_baseline -- --list
```

prints the workload catalog as one JSON line: names, suites, default iterations,
trace windows, profile support and resolved definitions. Every runner and the
[PR comparison](pr-reports.md) read it; there is no other case list. The
[workload guide](performance-workloads.md) describes the scenario families,
steady-state lifecycle, validity checks and synthetic labels, and how to add a
workload. Unknown names fail before any measurement.

All native cases use seed 7 and the 7 ms physics tick. Overrides describe workload
complexity; they do not change package defaults.
The compact native client/protocol and Python E2E cases explicitly retain the
sandbox world. The historical `step-default` name also refers to that fixture,
independently of the official gameplay default. The six legacy rows keep their
historical names and meaning:

| Case | Work per iteration |
| --- | --- |
| `step-default` | One tick, four ring-start snakes, `Config::sandbox()` world/350 food attempts. |
| `step-food` | One tick, 32 ring-start snakes, radius 6,000, 12,000 food attempts. |
| `step-body` | One tick, 64 separated 128-anchor bodies, radius 12,000, 350 food attempts. Primarily bounds and rejected pairs. |
| `observe` | `observe_all` on an immutable world: 32 snakes with 32 anchors each, 12,000 food attempts and eight prey. |
| `portable` | Six ticks plus all eight client frames via `step_all`, with wire recording and untimed draining. |
| `protocol` | Fresh decoder replay of a synthetic 48-tick tape for slot zero, with a frame after each of nine record groups. |

Legacy step and portable episodes reset before the next operation after any death
or before exceeding 1,000 episode ticks. Resets and control calculation are outside
native operation timing and inside process totals. Actual admitted food counts
can be below attempts. The short protocol tape exercises initial reconstruction;
it does not cover long-lived minimap deltas. These cases are a starting baseline,
not a distribution of production worlds.

Native operation latency and wall time exclude setup, trace output and report
serialization. Child-process CPU/wall and peak working set include them. CPU is
kernel plus user time; short Windows measurements can be quantized to 15.625 ms.
Peak working set measures resident pages, not private allocation or total system
memory. Per-process CPU and peak memory collection currently use Windows APIs;
other systems explicitly report these metrics unavailable. Phase profiling adds
clocks and counters and is a separate run; use unprofiled measurements for
comparisons. `segmentTests` counts candidate body anchors after broad-phase
acceptance, not the exact number of executed edge tests.

Inspect actual living populations and operation counts before interpreting
throughput. Start with short runs; increase duration if CPU quantization or
variance prevents a useful comparison. Compare the same workload, seed,
iterations, machine, toolchain, build flags and harness source.

Trace mode runs separately from timings. The Rust harness emits full simulator
state, including serialized internal RNG/motion state, and the relevant
event/observation or client frame/wire outputs. The runners retain their SHA-256
and compressed bytes; scenario traces must end with a terminal record. Compare
decompressed traces exactly between the same source harness built against
baseline and candidate. Serialization order of unordered internal containers
must be canonicalized; do not exclude gameplay, timing-in-virtual-time or
protocol fields. Real-time hosted tests use the shared decoder's deterministic
wire replay plus existing host contract tests as their semantic checks; they do
not assert equal wall-clock receipt times.

## Manual copied-binary procedure

Use this advanced path to time executables you built yourself, for example the
observation harness or a full baseline report with the E2E cases. Use a release
build with the checked-in lockfile and unmodified release profile (thin LTO, one
codegen unit). No dependencies, runtime threads, services or platform tuning are
needed.

```sh
cargo build --release --locked --bin slither-sim --example perf_baseline -j 2
```

Use task-owned target storage or coordinate use of the existing Cargo target.
For baseline/candidate source snapshots, use a separate target directory per
snapshot, including when source files retain old timestamps. Keep the build log,
absolute source/manifest paths and exact build command with each copied binary.
At build time, bind its binary hash to an immutable source identity: a clean
commit/tree ID or a source-manifest digest covering all compiled inputs and local
edits. A fast cached build alone does not verify that it used the intended snapshot.
Copy both executables into a new owned output directory before timing. Do not
benchmark executables in a shared target that another build can replace. On
Windows, for example (adjust the source target path if configured):

```powershell
New-Item -ItemType Directory outputs/perf-baseline/bin
Copy-Item target/release/slither-sim.exe outputs/perf-baseline/bin/
Copy-Item target/release/examples/perf_baseline.exe outputs/perf-baseline/bin/
uv run --locked --only-group tools python -B tools/perf_baseline.py --harness outputs/perf-baseline/bin/perf_baseline.exe --cli outputs/perf-baseline/bin/slither-sim.exe --output outputs/perf-baseline/run-1
```

The runners use only the Python standard library. Existing locked tools
environments can be reused with `uv run --project PATH --locked --only-group tools
--no-sync --offline python -B ...`; no environment synchronization is needed for
this harness. Unix executables omit `.exe`.

The new output directory contains source/toolchain/binary hashes, the catalog
hash and per-workload definition fingerprints, exact commands, individual
measurements, separate phase profiles and compressed full traces. Outputs are
ignored by Git; preserve relevant results before cleanup. The source status and
diff hash accompany the commit because the benchmark itself can be an addition
to the runtime baseline. Build commands/profile and the exact CPU model must also
be recorded with a report. This runner does not build or infer which sources
produced an arbitrary executable; its measurement-time checkout hashes do not
establish the binary's build-time source identity.

A binary hash identifies the copied bytes, not the source that produced them.
Reject benchmark inputs with missing or mismatched build records before timing;
the copied-binary runners do not enforce that provenance check.
If build provenance is uncertain, retain the attempt as invalid and rebuild in a
fresh snapshot-specific target before running parity checks and then timings.

Each case runs once as an untallied warmup, then five fresh processes by default.
By default every available catalog workload runs with its catalog iterations and
trace window, plus both E2E cases. `--cases`, `--iterations`,
`--trace-iterations`, `--repeats` and `--e2e-steps` bound the run.
Alternate copied baseline/candidate executables within the same agreed window
for a performance claim:

```sh
python -B tools/perf_compare.py --baseline PATH/TO/BASELINE --candidate PATH/TO/CANDIDATE --output outputs/perf-comparison
```

This times the catalog's `ci` suite by default and accepts `--suite` and
`--cases`; it does not trace. Both executables' catalogs must agree.

The end-to-end cases include process startup, Python JSON, stdin/stdout and polling
overhead. `e2e-client` uses eight simulation slots and explicit zero processing
duration. `e2e-host` starts a temporary host on `127.0.0.1:0`, one network client
and four background bots. It measures real-time scheduling and socket processing,
so throughput is cadence-limited. Response latency is observed at the Python
caller and includes its 0.5 ms pipe-poll sleeps. The host is killed and waited for
on success and error; no listener is retained. Hosted receipts vary with wall time
and have no deterministic response hash.

## Follow-up workloads

For a shared time limit and retained incomplete attempts, see
[comparison deadlines](performance-deadlines.md).
The [observation guide](performance-observations.md) adds immutable geometry and
selection cases from a separate harness without a catalog; `perf_compare.py`
keeps their fixed sample sizes. [Retained client processing](performance-clients.md)
covers longer-lived connections and decoder state.

## Initial 0.2.0 cost report, October 23, 2026

The [archived baseline study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-23-baseline/REPORT.md)
retains the original measurements, cost attribution and limitations.

## First optimization: collision bounds lookup

The [archived collision study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-23-collision-bounds/REPORT.md)
retains all paired results and parity evidence. Its gain is specific to the
separated-body fixture; no general or end-to-end speedup was established.
See the [research index](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/README.md) for other completed studies.
