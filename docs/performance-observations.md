# Observation workload follow-up

The immutable observation harness separates materialization from destruction of
the detached output. It is a synthetic cost screen, with no physics stepping,
reset, episode or real-time host. It does not characterize long sessions or
playing strength. Pin the source and build identity for every comparison; the
dated baseline and both candidate experiments are retained in the study below.

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

## Matched candidate results

The [archived observation study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-23-observation-batches/REPORT.md)
retains the original baseline screen, all 60 measured pairs across both attempts,
source/build identities, exact-trace checks and limitations. Only the empty-mask
fixture improved decisively; nonempty results were mixed, with a positive wide
median. The rejected lazy-`Option` attempt is preserved alongside the adopted
early return. These are historical results, not a current baseline selection.

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

Validation of the pinned snapshot passed five preparation tests, all 61 root Python
tests, both generated release builds, and all six complete two-iteration baseline/instrumented
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
