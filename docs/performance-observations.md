# Observation workload follow-up

The immutable observation harness separates materialization from destruction of
the detached output. It is a synthetic cost screen, with no physics stepping,
reset, episode or real-time host. It does not characterize long sessions or
playing strength. The baseline source is
`9b55fee37bdc68bf05db70a74646f965537451e4`, including the protocol/minimap fixes
and collision lookup change that followed the published 0.2.0 baseline.

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

## Reproduction and metric scope

Build `cargo build --release --locked --example perf_observation -j 2`, using the
same toolchain, release profile and source file for each runtime. Copy and hash
each executable into separate owned output storage. The original six-case
comparison defaults remain unchanged; opt in to these cases explicitly:

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

The initial screen started at 15:30:37 UTC on October 23, 2026, with health and
research peers holding heavy work. Three fresh samples followed one warmup.
Machine/build match the [initial report](performance.md): Ryzen 7 9800X3D,
Windows 11 10.0.26200, rustc 1.99.0 (`b940084d7`, LLVM 23.1.1), Cargo 1.99.0,
Python 3.12.9, locked thin-LTO release profile with one codegen unit, jobs=2,
task-owned target and no platform tuning. This was a shared desktop; agreement
with peers does not establish whole-machine isolation.

| Case | Median materialization, ms | Median output drop, ms | Median process CPU, s |
| --- | ---: | ---: | ---: |
| dense | 1,564.40 | 57.96 | 1.578125 |
| compact | 696.30 | 98.63 | 0.765625 |
| wide | 206.86 | 10.38 | 0.218750 |
| strip | 609.21 | 30.95 | 0.656250 |
| one | 437.66 | 8.87 | 0.437500 |
| empty | 226.55 | 0.67 | 0.234375 |

These are totals for the unequal iteration counts in the first table. Do not
compare them as equal amounts of work. Peak child working sets stayed below
10 MiB. Raw rows, exact build/source/binary pins and compressed traces are retained
under `outputs/perf-observation-baseline` in the performance worktree.

## Matched candidate results

The proposed change returns stable empty slots early for an empty/dead-only
batch, after validating the mask. Nonempty batches retain the original observer
construction and observation loop, with the same predicates, index math and order.
The observer remains local to one immutable borrow; no state survives into the
next call. Candidate runtime source is `fdfea1cfeab1d5ae7e1bbbf3a0b38648f575fb5b`.

The [selected measurements](performance-observations-20261023.json) include every
matched pair from **both** experiments, CPU/wall totals, throughput, tail latency,
peak memory, source/build/toolchain/binary hashes and full trace digests. Raw
rows, including warmups, remain in the worktree's ignored `outputs` directories.
Both windows were agreed with research and health peers; the runner was sequential
with at most 100,000 iterations/sample and a 120-second child deadline. No OS hard
CPU/memory cap or whole-machine exclusivity is claimed.

An initial lazy-`Option` form (`cd0bf8b`) was rejected for promotion: its wide-case
CPU increased in all five pairs, with a median ratio of 1.083, despite an empty
selection improvement. Its raw results and parity evidence are retained under
`outputs/perf-observation-candidate`. The following final comparison uses the
early-return form, retained under `outputs/perf-observation-candidate-v2`:

| Case | Iterations/sample | Median paired CPU ratio | Median paired materialization wall ratio |
| --- | ---: | ---: | ---: |
| dense | 1,000 | 0.979 | 1.006 |
| compact | 500 | 1.000 | 0.987 |
| wide | 10,000 | 1.039 | 1.034 |
| strip | 1,000 | 0.973 | 0.986 |
| one | 10,000 | 1.037 | 1.018 |
| empty | 100,000 | 0.0073 | 0.00243 |

Five adjacent alternating pairs follow one warmup per label/case. Ratios are
medians of paired ratios, not ratios of aggregate medians; lower means less cost.
No outliers are removed. In the empty fixture, median materialization totals are
2.225 s → 5.71 ms (about 99.8% less inner wall time). All five pairs improve.
Process CPU medians are 2.219 → 0.015625 s; candidate totals of one or two Windows
CPU quanta are too short for a precise percentage. Median per-batch p99 is
47.2 → 0.1 µs, with clock-resolution limits on the short candidate path. Maximum
working sets are 6.82 → 6.85 MiB: no memory reduction is established.

Nonempty results are mixed. Wide CPU ratios range 0.864–1.082 and one-slot ratios
0.960–1.083; their positive median changes remain a limitation of this run. These
measurements establish an empty-selection improvement only, without a general
speedup, equivalence bound for nonempty performance, production incidence estimate
or end-to-end throughput claim. The API still allocates and fills the result slots.

Both candidates passed all six complete two-iteration trace comparisons against
the frozen baseline, with the identical harness source hash; batch outputs also
matched public scalar observations. The dense initial state and every snapshot
and ordered result exactly matched the original pinned harness. The regression
covers all-dead batches, malformed masks, subsequent public mutation at unchanged
virtual time/lengths, and retained detached outputs. Final focused core tests and
the full `tools/check.py rust` group passed, including default/core-only all-target
tests, doctests, Clippy with warnings denied, formatting and basic example. All 29
root tools tests passed for the runner extension. Independent source review found
no defects in the final runtime and harness; the reviewer did not execute checks.
Local package/WASM/full tools groups were not run.

Trace mode (`perf_observation CASE ITERATIONS --trace`) has no timing intervals.
It emits full serialized simulator state, separate events and ordered observations,
and checks batch output against scalar `observe(slot)`. Only JSON object keys and
the unordered internal `prey_outward` set are sorted. Compare full decompressed
trace bytes between identically built harnesses. These ordinary finite fixtures
do not cover all numerical/index boundaries; JSON cannot distinguish every
nonfinite floating-point payload.

## Isolated phase diagnostics

`tools/perf_observation_phases.py` prepares source snapshots without building or
running them. It instruments the actual observation implementation in a generated
snapshot; the shipped runtime and original fixture remain unchanged. The recipe
accepts reviewed source shapes and refuses existing output paths. For the current
reviewed shape, use the full public commit:

```sh
uv run --project "REPO_ROOT" --locked --only-group tools --no-sync --offline python -B tools/perf_observation_phases.py --commit 20dd3aff5574af0270d8ad2c1e93f9001e8f1142 --output "NEW_ABS_OUTPUT_DIR"
```

Replace `REPO_ROOT` with the tools-environment checkout and `NEW_ABS_OUTPUT_DIR`
with a new absolute local output directory outside every checkout. The requested
commit must exist locally; preparation does not fetch history. Preparation
retains the exact exported source hashes, recipe, resource and instrumentation
patch. Both snapshots omit only the registration of the unexported `portable_bot`
example. `COMMANDS.txt` contains explicit offline core-only builds, complete trace
comparisons, and separate profiles. Builds use distinct targets and jobs=2; build
and workload deadlines are 600 and 120 seconds. These are direct-child timeouts,
without OS CPU/memory caps. No dependency installation or environment tuning is
performed. Retain failed attempts rather than overwriting them.

The counters report index visits, food candidates/accepted copies, own/opponent
body anchors and stable empty batches. Wall intervals distinguish index setup,
indexed food filtering, accepted-index ordering and copying. Linear food and
prey/opponent predicates and copies remain interleaved. The inclusive batch
interval contains its nested phases, clock/counter overhead and temporary/index
destruction; returned-output destruction is reported separately by the harness.
Do not sum inclusive and nested intervals or use these profiles as comparative
timings. Trace mode disables profiling sessions and compares complete bytes.

Parent validation passed five preparation tests, all 61 root Python tests, both
generated release builds, and all six complete two-iteration baseline/instrumented
trace comparisons. The original scalar-observation assertions also passed. The
same source received an independent read-only review with no findings; that
review did not execute checks. Numeric/mutation coverage remains limited to the
existing finite fixtures. The prepared source tree and raw trace proof are
retained outside Git under the performance workstream's owned outputs.
Preparation tests create their own Git fixture, so shallow CI checkouts require
neither historical public commits nor local commits from before a squash merge.

## Remaining investigation

1. Attribute dense materialization to candidate filtering, accepted-index
   ordering and copying before changing the index. A 2D bucket scheme is a
   hypothesis: using the existing 64 buckets as 8×8 can increase candidate visits
   in large worlds. Keep the wide counter-case and test numeric/order parity.
2. Investigate sparse nonempty selections separately. Skipping unused setup
   cannot accelerate the fully living dense batch; using the scalar path for one
   requested slot needs its own parity and matched measurements.
3. Continue the original measured food-scan/refill and portable frame/encoding
   investigations after this slice. The new immutable harness does not profile
   those paths or justify a general/end-to-end speedup claim.
