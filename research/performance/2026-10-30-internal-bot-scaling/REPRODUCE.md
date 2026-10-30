# Reproduce the direct-bot scaling profile

Linux only, an otherwise idle four-CPU guest, Rust 1.99, locked dependencies,
Python standard library and enough private output space. Never run the study
on the maintainer's Windows PC. No AWS or game-server connection is used.
The runtime is pinned in `selected-results.json` once counting completes.

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

At the frozen checkpoint it verifies a complete 512-tick serial and parallel
continuation and makes the replay tape. Then it runs one discarded triplet and
five alternating serial/parallel pairs with a simulation-only replay around each
pair. Each child reloads the same checkpoint; decision modes prime observation
and policy allocations without advancing or changing the world or held actions.
All counted final-state digests must match the verification continuation.
Profiled diagnostics are separate children on that same checkpoint.

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
Once the A4 runner has emitted `complete`, copy these study files into a tools
directory on the guest. The botstorm builder checks that completion and the
original binary/source pins before doing any build. It generates a new example
from the frozen `profile.rs` helpers plus `botstorm.rs`, without copying helpers
into a second checked-in file or changing `src/`.

```sh
python3 build_botstorm.py EXECUTION_DIRECTORY STUDY_TOOLS_DIRECTORY
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

The diagnostic uses another example name and debug=2, preserving portable
codegen flags and all original primary binaries. It records an unsampled and
sampled 512-tick serial continuation at 1k/10k, checks final-state identity and
retains overhead ratios. Source-context buckets use the same source revision;
unresolved and outside-operation samples remain explicit. These samples support
ranking, not a replacement throughput measurement or a proven exact speedup.
