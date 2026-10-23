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
sources produced an arbitrary executable.

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

Measurements and the optimization shortlist will be recorded here after the
first agreed baseline window. No speedup is established by the harness alone.
