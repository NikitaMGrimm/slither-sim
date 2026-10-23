# Reproducible performance checks

These synthetic, offline workloads measure this simulator and its client tools.
They do not measure official servers or bot playing strength. The initial runtime
baseline is 0.2.0, commit `bb56d947df3c9a91e9331824981e4d4adf1cab34`.

All native cases use seed 7 and the 7 ms physics tick. Overrides describe workload
complexity; they do not change package defaults.

| Case | Work per iteration |
| --- | --- |
| `step-default` | One tick, four ring-start snakes, default world/350 food attempts. |
| `step-food` | One tick, 32 ring-start snakes, radius 6,000, 12,000 food attempts. |
| `step-body` | One tick, 64 separated 128-anchor bodies, radius 12,000, 350 food attempts. Primarily bounds and rejected pairs. |
| `observe` | `observe_all` on an immutable world: 32 snakes with 32 anchors each, 12,000 food attempts and eight prey. |
| `portable` | Six ticks plus all eight client frames via `step_all`, with wire recording and untimed draining. |
| `protocol` | Fresh decoder replay of a synthetic 48-tick tape for slot zero, with a frame after each of nine record groups. |

Step and portable episodes reset before the next operation after any death or
before exceeding 1,000 episode ticks. Resets and control calculation are outside
native operation timing and inside process totals. Actual admitted food counts
can be below attempts. The short protocol tape exercises initial reconstruction;
it does not cover long-lived minimap deltas. These cases are a starting baseline,
not a distribution of production worlds.

Use a release build with the checked-in lockfile and unmodified release profile
(thin LTO, one codegen unit). No dependencies, runtime threads, services or
platform tuning are needed. Agree on a quiet window with other local work before
timing; builds and tests must finish first. Record background activity and any
interruption. A quiet-window agreement does not establish an otherwise idle host.

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

The runner uses only the Python standard library. Existing locked tools
environments can be reused with `uv run --project PATH --locked --only-group tools
--no-sync --offline python -B ...`; no environment synchronization is needed for
this harness. Unix executables omit `.exe`. Per-process CPU and peak memory
collection currently use Windows APIs; other systems explicitly report these
metrics unavailable.

The new output directory contains source/toolchain/binary hashes, exact commands,
individual measurements, separate phase profiles and compressed full traces.
Outputs are ignored by Git; preserve relevant results before cleanup. The source
status and diff hash accompany the commit because the benchmark itself can be an
addition to the runtime baseline. Build commands/profile and the exact CPU model
must also be recorded with a report. The runner does not build or infer which
sources produced an arbitrary executable; its measurement-time checkout hashes
do not establish the binary's build-time source identity.

A binary hash identifies the copied bytes, not the source that produced them.
Reject benchmark inputs with missing or mismatched build records before timing;
the runner does not enforce that provenance check.
If build provenance is uncertain, retain the attempt as invalid and rebuild in a
fresh snapshot-specific target before running parity checks and then timings.

Each case runs once as an untallied warmup, then five fresh processes by default.
`--iterations`, `--repeats`, `--cases` and `--e2e-steps` bound the run. Inspect actual
living populations and operation counts before interpreting throughput. Start
with short runs; increase duration if CPU quantization or variance prevents a
useful comparison. Compare the same workload, seed, iterations, machine,
toolchain, build flags and harness source. Alternate copied baseline/candidate
executables within the same agreed window for a performance claim.

Native operation latency and wall time exclude setup, trace output and report
serialization. Child-process CPU/wall and peak working set include them. CPU is
kernel plus user time; short Windows measurements can be quantized to 15.625 ms.
Peak working set measures resident pages, not private allocation or total system
memory. Phase profiling adds clocks and counters and is a separate run; use
unprofiled measurements for comparisons. `segmentTests` counts candidate body
anchors after broad-phase acceptance, not the exact number of executed edge tests.

The end-to-end cases include process startup, Python JSON, stdin/stdout and polling
overhead. `e2e-client` uses eight simulation slots and explicit zero processing
duration. `e2e-host` starts a temporary host on `127.0.0.1:0`, one network client
and four background bots. It measures real-time scheduling and socket processing,
so throughput is cadence-limited. Response latency is observed at the Python
caller and includes its 0.5 ms pipe-poll sleeps. The host is killed and waited for
on success and error; no listener is retained. Hosted receipts vary with wall time
and have no deterministic response hash.

Run trace mode separately from timings for output parity. The Rust harness emits
full simulator state, including serialized internal RNG/motion state, and the
relevant event/observation or client frame/wire outputs. The runner retains their
SHA-256 and compressed bytes. Compare decompressed traces exactly between the
same source harness built against baseline and candidate. Serialization order of
unordered internal containers must be canonicalized; do not exclude gameplay,
timing-in-virtual-time or protocol fields. Real-time hosted tests use the shared
decoder's deterministic wire replay plus existing host contract tests as their
semantic checks; they do not assert equal wall-clock receipt times.

## Initial 0.2.0 cost report, October 23, 2026

The [selected measurements](performance-baseline-20261023.json) retain individual
samples, complete workload configs, phase profiles, source/toolchain/binary hashes
and trace digests. Raw outputs remain in the performance worktree's ignored
`outputs/perf-bb56d94/baseline-1`. Runtime sources match `bb56d94`; harness commit
`3129fcf011df5fa5361280ce700ffcc62baf587e` adds only measurement files.

Machine: Ryzen 7 9800X3D, 8 cores/16 logical CPUs, approximately 31 GiB RAM,
Windows 11 Pro 10.0.26200. Toolchain: rustc 1.99.0 (`b940084d7`, LLVM 23.1.1),
Cargo 1.99.0, Python 3.12.9. Locked release build, task-owned target, two build
jobs, no `RUSTFLAGS`. Binaries were copied and SHA-256 pinned before timing.
Health and research peers agreed to hold CPU-heavy activity for the initial
window around 14:28–14:30 UTC. Pre-run aggregate CPU was 10%; this was a shared
desktop, not an isolated machine. Repeated wall times varied substantially.
Treat this as an exploratory cost baseline, not a stable speed comparison.

Each native sample contains 1,000 iterations; three measured fresh processes
followed one untallied warmup. Inner wall and throughput use the median total
operation time; CPU uses median process total; peak is the largest process working
set; p99 is the median of per-run p99 values. CPU totals include setup, reset and
teardown. Particularly short CPU measurements are too quantized for comparisons.

| Case | Inner wall, ms | Operations/s | Process CPU, s | Peak MiB | p99 operation, µs |
| --- | ---: | ---: | ---: | ---: | ---: |
| step-default | 2.96 | 337,804 | 0.015625 | 4.68 | 18.4 |
| step-food | 103.89 | 9,625 | 0.109375 | 6.60 | 717.9 |
| step-body | 44.35 | 22,546 | 0.062500 | 5.09 | 134.8 |
| observe | 1,715.01 | 583 | 1.593750 | 7.66 | 4,579.9 |
| portable | 600.75 | 1,665 | 0.437500 | 7.09 | 2,759.0 |
| protocol | 222.30 | 4,499 | 0.234375 | 6.58 | 1,031.5 |

These operations do different work: portable iterations advance six ticks and
produce eight frames; protocol iterations reconstruct nine frames from a tape.
Do not compare their iteration rates as equivalent simulation throughput.
All step samples retained 4/32/64 living snakes with zero deaths. Food minima
were 316/11,213/350 respectively; dense food started below its 12,000 attempts.
Body counts and final growth/boost changes are retained in the JSON report.

The 100-decision CLI cases used three samples plus warmup. Local portable CLI
wall time was 1.164 s median (client CPU 0.265625 s, peak 8.51 MiB, response p99
30.97 ms). Hosted wall time was 4.875 s median (client CPU 0.171875 s, peak
7.75 MiB, response p99 68.33 ms). The client was playing for all 100 local frames
and 100/100/99 hosted frames. Real-time hosted conditions differ between samples;
these response latencies include caller polling and scheduling. The host process
used 0.0625 CPU s median and peaked at 6.29 MiB; client and host peaks are separate
process lifetime maxima, not a simultaneous combined-memory measurement.

The separate phase profile found food pickup at 62.07 ms and ambient refill at
19.45 ms of 96.92 ms for 1,000 dense-food ticks. The body case spent 32.39 ms of
50.72 ms in collision processing: 4,032,000 candidate pairs, zero candidate narrow
body anchors. This measures bounds/pair overhead, not expensive contacting bodies.
The profiles include instrumentation overhead and are not the unprofiled totals.

Ranked next work, considering measured cost, scope and parity risk:

1. **Collision bounds lookup:** remove per-pair binary searches of stable living
   slots. The body profile supports investigating this small change first; keep
   join order, broad-phase arithmetic and narrow geometry unchanged. A speedup
   remains unproven until matched runs.
2. **Observation materialization:** the immutable batch costs about 1.6 CPU s per
   1,000 iterations. Profile index/filter/sort/clone costs before changing it;
   visibility, ordered food and detached observations are contract constraints.
3. **Food scan/refill:** the dense-food profile makes this the main core cost for
   that fixture. Investigate scans and allocation; preserve tie order, RNG
   consumption and public food mutation support. No numerical retuning.
4. **Portable broadcast/frame construction:** separate encoding, receipt state,
   recording and frame copies before optimizing. The initial tape is too short
   to characterize long-lived reconstruction or minimap delta costs.

For the longer follow-up workload, see [retained client processing](performance-clients.md).

## First optimization: collision bounds lookup

The runtime change in `src/simulator.rs` indexes body bounds by stable snake slot,
with empty entries for dead slots. Previously, every candidate pair binary-searched
the living list to locate the same bounds. Pair/join order, bounds arithmetic,
segment values and narrow collision calls are unchanged. No settings, physics,
RNG order, visibility or wire behavior changed.

The [matched comparison](performance-collision-20261023.json) retains five adjacent
baseline/candidate pairs, alternating label order, after one untallied warmup per
label/case. Both peers held heavy local work around 14:38–14:40 UTC. Builds,
tests and trace serialization finished first. Source/build conditions match the
initial baseline; candidate runtime commit is `67db506` and both copied executable
hashes are recorded. The benchmark source is identical in both executables.

Reproduce the comparison with fresh output storage and copied executables:

```sh
python -B tools/perf_compare.py --baseline PATH/TO/BASELINE --candidate PATH/TO/CANDIDATE --output outputs/perf-comparison
```

For an optional shared time limit and retained incomplete results, see
[comparison deadlines](performance-deadlines.md).

The runner fixes longer per-case iteration counts below and preserves raw paired
rows. Ratios are medians of each matched candidate/baseline ratio; lower is less
cost. They are descriptive results from five pairs, without confidence intervals.

| Case | Iterations per sample | Process CPU ratio | Inner wall ratio |
| --- | ---: | ---: | ---: |
| step-default | 100,000 | 1.000 | 0.965 |
| step-food | 10,000 | 0.967 | 1.021 |
| step-body | 50,000 | 0.716 | 0.699 |
| observe | 1,000 | 0.989 | 0.989 |
| portable | 5,000 | 0.964 | 0.959 |
| protocol | 5,000 | 0.967 | 1.002 |

The separated 64-snake/8,192-anchor body fixture used **28.4% less process CPU**
and **30.1% less inner wall time** by median paired ratio. All five body pairs
improved (CPU ratios 0.626–0.725). Median process CPU totals were 2.094 → 1.516 s,
median operation p99 93.1 → 68.1 µs, and maximum working sets 5.71 → 5.64 MiB.
Memory differences are small and do not establish an allocation improvement.
The other cases have mixed small changes and outliers; no improvement or
regression conclusion is drawn for them. There is no demonstrated end-to-end
speedup or general production-world speedup.

Output parity passed for all six 32-iteration canonical full-state traces and a
128-decision portable trace (5.376 virtual seconds, including minimap cadence).
The traces retain internal serialized RNG/motion state, events, observations,
frames and wire bytes. A separate bounded regression exercises a dead middle
slot and original/respawn collision join order. Full `tools/check.py rust` passed
(default/core-only all-target tests, doctests, Clippy, formatting and basic
example), along with three standard-library runner failure/cleanup regressions.
An independent source review found no remaining defects in the runtime, runner
or comparison logic; it did not independently execute these checks.

This comparison uses the published 0.2.0 runtime baseline. If correctness changes
are integrated, establish and name that updated baseline before extending these
performance claims. The remaining shortlist above is investigative work, not
additional promised speedups.

The [observation follow-up](performance-observations.md) adds immutable geometry
and selection cases with separate output-destruction timing and an explicitly
updated source baseline.
