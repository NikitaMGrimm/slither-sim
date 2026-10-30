# Reproduce the direct-bot scaling profile

Linux only, an otherwise idle four-CPU guest, Rust 1.99, locked dependencies,
Python standard library and enough private output space. Never run the study
on the maintainer's Windows PC. No AWS or game-server connection is used.
The runtime is pinned in `selected-results.json` once counting completes.

Read [PLAN.md](PLAN.md), [BOTSTORM_PLAN.md](BOTSTORM_PLAN.md) and the later
[maintainer amendment](AMENDMENT.md) together. The amendment supersedes the
optional-row cost gate and the unexecuted extra-warmup proposal. If a warmup
guard fires, the final snapshot is timed with its **unsettled ecology** label;
it is not retried until a more favorable or steady state appears.

Create a fresh execution directory from its runtime archive. Keep the original
runtime and all of `src/` unchanged. Copy this study's `profile.rs` to
`examples/internal_scaling.rs`, and `run.py`, `PLAN.md` and tests into the execution
directory. Build the root crate, with the repository's portable release profile:

```sh
cargo build --release --locked --no-default-features --features bot,parallel --example internal_scaling
```

Bound build concurrency separately; no native Rust flags. On wrapper installations,
point `RUSTC` and `RUSTDOC` at the direct installed toolchain. The example needs
Rayon, serde, serde_json and the existing sha2 dev dependency; it adds none.

Before primary work, run the four stability-screen tests and three CPU-attribution
tests. A smoke fixture is deliberately newborn and is not a selected timing row:

```sh
python3 test_run.py
python3 test_reduce.py
target/release/examples/internal_scaling smoke smoke 1000 12
target/release/examples/internal_scaling measure smoke serial 24 verify smoke-serial.json
target/release/examples/internal_scaling measure smoke parallel 24 verify smoke-parallel.json
cp smoke-serial.tape smoke/tape.bin
target/release/examples/internal_scaling measure smoke simulation 24 plain smoke-simulation.json
```

Require complete equality of the two action-tape files and action digests, and all
three final-state digests. Source/build pins must record the runtime archive,
one execution-source manifest digest, harness/runner/plan/lockfile/build-log and
binary hashes, exact build command, profile/features and actual toolchain output.
The supplied [pin.py](pin.py) is the public pin producer. Retain the build stdout
and stderr together as `build-final.log`, then run:

```sh
python3 pin.py EXECUTION_DIRECTORY ORIGINAL_RUNTIME_ARCHIVE RUNTIME_REVISION
```

It reruns the smoke, verifies all three states and both action tapes, and writes
`measurement-pins.json` exclusively. Do not duplicate per-file source maps across
modes or windows. `run.py` verifies the binary before and after work. The as-run
pin producer is retained in the private archive; this public recipe is its
path-parameterized equivalent and does not replace or refresh historical pins.

```sh
python3 -u run.py EXECUTION_DIRECTORY
```

The runner retains all warmup census records and uses only the frozen stability
screen to stop warmup. It saves `results/N/fixture/{world.json,meta.json,census.json}`
and the qualification receipt. The world checkpoint includes the full world;
`meta.json` includes admission RNG, tick, refusal retries and held actions.
Policies have reusable scratch, with no learned/per-bot memory. Staggering uses
six sparse buffers so unselected slots do not discard another phase's storage.

The original collection lost its controller during early 10k warmup, before a
checkpoint existed. Its partial log remains in `results/10000-interrupted`.
`restart_primary.py` invokes the unchanged frozen runner on a fresh 10k output
root and moves its completed result back; it does not rerun the completed smaller
populations. This recovery is not needed for a clean reproduction. Run controllers
detached from SSH, with durable stdout/stderr, so a connection loss cannot stop them.

At the frozen checkpoint it verifies a complete 512-tick serial and parallel
continuation and makes the replay tape. Then it runs one discarded triplet and
five alternating serial/parallel pairs with a simulation-only replay around each
pair. Each child reloads the same checkpoint; decision modes prime observation
and policy allocations without advancing or changing the world or held actions.
All counted final-state digests must match the verification continuation.
Profiled diagnostics are separate children on that same checkpoint.

The ecology is warmed; derived world-step caches are reconstructed after each
checkpoint load. Observation/policy priming does not step the world. Counted
windows include the first restored tick (1/512 of each window); selected data
also retain its CPU distribution and the distribution excluding that tick.
Interpret maxima with this distinction, rather than treating a checkpoint-load
cache spike as a measured steady live-server stall.

Admission uses public core methods. In particular `spawn_margin` computes a
complete margin, whereas the socket host's private bounded-margin helper may
stop early. The pose/outcome rule is similar, but the extra fallback work is
part of this study's explicit lifecycle; do not label it socket-host admission
cost. No adaptive-radius cap is added. Official wire delivery, minimap and
leaderboard have no code path in this direct core workload.

After all timing ends, copy the current reducer/tests into the execution
directory and run them there:

```sh
python3 test_reduce.py
python3 reduce.py EXECUTION_DIRECTORY
```

The CPU totals use thread clocks for physics/admission and process clocks for
parallel observation/decisions. Policy-loop CPU sums worker-thread brackets;
it includes the sparse-slot loop and excludes pool scheduling. The enclosing
decision CPU includes scheduling/wait overhead. Whole-window process CPU also
retains work between brackets. Simulation-only wall throughput includes tape
decoding and harness overhead; world/admission brackets exclude tape decoding.

`StepProfile` contains wall-time phase brackets. The reducer does not call them
exact CPU: for each phase it bounds CPU by subtracting all measured off-CPU
time from that phase for its lower bound, and capping at total world CPU for
its upper bound. These separate bounds are not additive. Report instrumentation
cost against the same ordinary continuations and preserve unbracketed work.

`segmentTests` is the sum of candidate body lengths after the outer bounds
check, incremented before `strikes_scaled`; it is not a count of executed edge
distance tests, which may return early. Keep that distinction when comparing
work growth with CPU growth.

Memory is whole-process RSS/high-water RSS, not allocated bytes or a component
allocator census. Loaded-fixture memory can retain checkpoint parsing allocations;
simulation-only mode also holds its replay tape. Decision-mode initial memory is
after priming; final memory is sampled before final checkpoint serialization.
Do not interpret differences as exact observation allocation totals.

Keep raw checkpoints, tapes, binaries, samples and logs private/local-only.
Close and member-verify an archive, transfer it, check its digest, and only then
remove task-owned guest data and targets. The public report needs selected values
and a table script, not full recordings or host-identifying command inventories.

## Added center-seeking comparison and observation attribution

Keep [BOTSTORM_PLAN.md](BOTSTORM_PLAN.md) and
[DIAGNOSTIC_PLAN.md](DIAGNOSTIC_PLAN.md) separate from the original frozen plan.
The separate [warmup follow-up](WARMUP_FOLLOWUP_PLAN.md) was proposed but
superseded before execution. At a censored 10k checkpoint, `count_censored.py`
runs the unchanged binary's verification, pairs and profiles without extra
warmup, pinning the amendment and preserving the failed screen. The botstorm
runner likewise times its final checkpoint if censored. Run 6k/9k only when
12k's five-window median whole-loop rate is below `1000/7` ticks/s.
Once the A4 runner has emitted `complete`, copy these study files into a tools
directory on the guest. The botstorm builder checks that completion and the
original binary/source pins before doing any build. It generates a new example
from the frozen `profile.rs` helpers plus `botstorm.rs`, without copying helpers
into a second checked-in file or changing `src/`.

```sh
python3 build_botstorm.py EXECUTION_DIRECTORY STUDY_TOOLS_DIRECTORY
# Optional read-only A4 view census, once for each frozen population:
EXECUTION_DIRECTORY/bin/internal_botstorm views EXECUTION_DIRECTORY/results/1000/fixture VIEW_CENSUS.json
EXECUTION_DIRECTORY/bin/internal_botstorm grid EXECUTION_DIRECTORY/botstorm-results/12000/fixture GRID_CENSUS.json
python3 test_botstorm.py
python3 run_botstorm.py EXECUTION_DIRECTORY
python3 reduce_botstorm.py EXECUTION_DIRECTORY
python3 run_diagnostic.py EXECUTION_DIRECTORY
python3 classify_diagnostic.py EXECUTION_DIRECTORY
```

Preserve all counted binaries in `bin/` before the additional builds. The added
row pins reference the original master pin digest rather than duplicating it.
The center policy runs without an observation or worker pool. Death/admission
rates use virtual seconds; separate wall rates are also retained. Report the
normal-public-admission caveat for this family too.

The diagnostic uses separate example names and debug=2, preserving portable
codegen flags and all original primary binaries. `diagnostic_source.py` adds
monotonic world/admission bracket fields only to generated copies. It records
ordinary, sampled and phase-profiled 512-tick continuations at A4 1k/10k and
botstorm 12k. Every final state must equal the corresponding counted reference.
The selected overhead includes ordinary diagnostic/primary ratios and sampled/
ordinary diagnostic ratios; these are distinct effects.

Perf records user CPU at 499 Hz with 4,096-byte DWARF stacks and the explicit
monotonic clock; script output retains nanosecond timestamps. The reducer matches
samples to world/admission brackets and separately selects the six largest
world+admission CPU ticks, including and excluding the first restored tick.
Unresolved stacks, unparsed records, sample counts and unknown lost-sample
counts remain explicit. Source-line brackets distinguish movement, collision,
death, food pickup/cache, pickup-grid construction, prey and ambient work where
debug context resolves them. Inclusive observations are a separate budget.
These samples support ranking, not a replacement throughput measurement or a
proven exact speedup. The original 8,192-byte/no-timestamp diagnostic proposal
was superseded before collection; measured instrumentation costs are retained.

The read-only `grid` mode constructs no observations. It reports checkpoint
geometry separately from the diagnostic's per-tick audit. In the latter, public
event recording reconstructs exact pickup-entry food length as final food plus
Food pickup events minus ambient births. After pickup, only fam/derived score
changes; head, heading, speed, integer segments and alive status still match
the pickup circles. The source's directed circle and public `pickup_circle`
have the same arithmetic order. See the source-bound argument in AMENDMENT.md.

The diagnostic folds those actual circles after the world clock ends and records
all three grid guards and dense/hashed/linear selection on each of 512 ticks.
All ordinary/sampled/profiled guard records must agree, and their final state
must equal the primary reference. No extra timing run or `src/` edit is added.
Event recording inside step is part of the disclosed instrumentation overhead;
the guard fold is outside the world/admission sample intervals. These audit
samples are labelled separately in whole-operation sampled attribution.

`after_primary.py` waits on the main controller with a pidfd, then sequences
post-count work. `finish_collection.py` waits for that controller, runs tests and
reducers, creates a closed archive, verifies every member against its manifest,
and writes `archive-ready.json` last. It never deletes sources. Download only
after that receipt exists; check the downloaded SHA-256 before guest cleanup.

## Requested post-fix supplement

[POSTFIX_PLAN.md](POSTFIX_PLAN.md) pins external PR #331's head and the separate
world-only replay. Main-revision rows are never replaced. `postfix_331.py` waits
for all baseline stages and attribution, then downloads that public source head,
checks that runtime source differs only in the grid cap, verifies the selected
dependency package versions and portable profile, and builds distinct examples
in the existing task-owned target. Main binaries are preserved in `bin/`.

It reuses the same 10k checkpoint/tape: one discarded candidate window, five
counted world-only replays and a separate phase diagnostic. One additional
matched main world-only phase diagnostic uses the original binary; its original
plain windows remain the reference. All final-state bytes must match the main
reference. The botstorm candidate runs only if its baseline box ever exceeds
2^20 cells. The main guard trace decides that trigger, not performance selection.

```sh
python3 postfix_331.py EXECUTION_DIRECTORY STUDY_TOOLS_DIRECTORY
```

The helper writes `postfix-331/selected-results.json`. The archive finisher also
waits for its successful completion. The candidate's expanded source tree is
excluded from the raw archive; the complete pinned source archive, preparation
script, execution manifest and copied binaries are included. No study source
change is applied to `src/` in the contribution worktree.
