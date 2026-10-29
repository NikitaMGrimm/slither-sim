# Reproduce the exact spawn prefilter

Baseline `23eddca8`, implementation `bd8915c8`; the selected trace archive's
candidate `edab3287` has the same production Rust source and adds only harness
qualification. Full logs, copied binaries and traces remain local-only.
[Plan](PLAN.md), [attempts](ATTEMPTS.md), [selected correctness](correctness.json).

Run focused tests and the full Rust group serially with `CARGO_BUILD_JOBS=1`
and `RUST_TEST_THREADS=1`. The search oracle is frozen from the baseline and
compares exact pose/heading bits, error strings, RNG and unchanged world state.
For the focused mutation, replace the prefilter's `if !fallback` with `if true`
only; require failed oracle tests, restore the exact source in a `finally`
block, then require the restored oracle to pass. See the retained results.

## Portable byte traces

Reuse the minimap study's `prepare_traces.py` and `trace_matrix.py` by reference
from its pinned branch/PR, staging both in an external directory. The scripts
reuse the existing [protocol fixture](../2026-10-27-host-snake-candidates/protocol_fixture.rs)
and [catalog validator](../../../tools/perf_baseline.py); do not duplicate them
in this study. [prepare.py](prepare.py) wraps that preparation to add only this
study's research accessors, queue fixture and counters. It records the earlier
helper's SHA-256 as well as the changed generated sources and copied binaries.

```sh
python prepare.py PRIOR_PREPARE REPO OUTPUT base 23eddca8
python prepare.py PRIOR_PREPARE REPO OUTPUT candidate edab3287
python prepare.py PRIOR_PREPARE REPO OUTPUT mutation edab3287
python PRIOR_TRACE_MATRIX REPO OUTPUT catalog
python PRIOR_TRACE_MATRIX REPO OUTPUT host
python fixed_parity.py OUTPUT
```

The selected run compares 23 catalog, four historical host, two supplementary
Battledome and two queued-stress traces. All successful traces require complete
bytes and terminal records. The mutation discards fallback-eligible heads: its
wire bytes must differ in both queued protocols. These are debug-build,
untimed correctness checks. The fixture's PMU/clocks remain disabled locally.

## The exhausted-search row

[fixed_host.rs](fixed_host.rs) uses the actual host `Simulation::apply_commands`,
`admit_waiting`, `advance_tick` and `hand_over`, including the existing request
age, fallback, expansion protection and retry logic. The fixture has 80 reserved
seats: seven synthetic replay barriers, one connected observer and 72 queued
joins. Dynamic arena growth is disabled for this labelled synthetic stress
fixture; shipped spawn-band, floor, clearance, candidate and retry rules remain.
Seven held rings cover the band. Their poses and the observer pose are reset
outside the counted interval. The world still advances real physics, food,
marker and wire-history work each tick. No authoritative observation feeds a
bot, and no claim that these barriers are naturally grown bodies is made.

Warm 300 ticks, aging every queued request past the unchanged two-second
threshold and populating full backend/wire state. Freeze that entire host,
not a physical checkpoint. Every primary child counts 360 ticks from this same
state. Qualification requires at least 120 search ticks, each with exactly
4,096 candidates, all refusing with the same 72 requests pending. The local
correctness run instead releases barriers halfway through 360 ticks; its first
half has 90 refusals/368,640 candidates and its release phase checks actual
admitted poses and bytes. A full-budget release search can still accept the
directional fallback; it is not automatically a refusal.

Two separate controls use 80 live viewers held apart inside the pit and zero
waiting joins: `minimap` keeps the arena fixed; `markers` shrinks it with an
explicit radius schedule. They exercise the shared raster and actual marker
transitions respectively, without relying on admission being the bottleneck.
Their synthetic held bodies are fixed-work controls, not capacity or natural
growth evidence. [fixture_access.rs](fixture_access.rs) is an external source
overlay only; no production API is exposed.

## Released Linux bench only

Do not contact the guest until the maintaining agent explicitly releases it.
The following source staging is safe locally and performs **no build**:

```sh
STUDY_STAGE_ONLY=1 python prepare.py PRIOR_PREPARE REPO NATIVE_OUTPUT base 23eddca8
STUDY_STAGE_ONLY=1 python prepare.py PRIOR_PREPARE REPO NATIVE_OUTPUT candidate edab3287
STUDY_STAGE_ONLY=1 python prepare.py PRIOR_PREPARE REPO NATIVE_OUTPUT mutation edab3287
```

Transfer the complete staged directories and verify the archive/delivery digest
before using them. The manifest hashes every staged file with portable path
separators. Once released, [build_staged.py](build_staged.py) verifies all files
and builds locked/offline native release executables serially in one bounded
target, using the existing toolchain/dependencies. No installation, service or
guest setting change is needed. Before each arm it clears only this package's
artifacts in that verified task-owned target: pre-staged source mtimes must not
allow Cargo to reuse another arm's runtime. Cached dependencies and copied proof
binaries remain. Keep the source and binary hashes and the clean/build commands.

Run the same native trace matrix and fixed parity controls before counting,
then [write_gates.py](write_gates.py) binds those results to the selected
archive pins and completed full-Rust receipt. No hand-written success flag:

```sh
python build_staged.py NATIVE_OUTPUT base
python build_staged.py NATIVE_OUTPUT candidate
python build_staged.py NATIVE_OUTPUT mutation
python PRIOR_TRACE_MATRIX REPO NATIVE_OUTPUT catalog
python PRIOR_TRACE_MATRIX REPO NATIVE_OUTPUT host
python fixed_parity.py NATIVE_OUTPUT
python write_gates.py NATIVE_OUTPUT correctness.json verification.json
python run_pairs.py NATIVE_OUTPUT PRIOR_REPRODUCE 19 spawn
python run_pairs.py NATIVE_OUTPUT PRIOR_REPRODUCE 13 spawn
python tables.py NATIVE_OUTPUT spawn
```

The [pair runner](run_pairs.py) reuses the earlier
[process inventory](../2026-10-28-broadcast-distance/reproduce.py), keeps its
controller on CPU 1 and native parents/children on CPU 0, and checks that the
full host parent has one thread. `taskset` launches each parent on its core;
there is no pre-exec Python callback after reader threads start. Repeat fork
proof traces twice per arm and
compare both arms; compare each direct-parent continuation too. One discarded
AB warmup precedes five AB/BA/AB/BA/AB pairs. Every pair compares its complete
wire/state trace and rechecks copied executable hashes. Build/test work finishes
before timing. Keep complete private inventories before/after each pair and no
status commands during counting.

[tick_samples.rs](tick_samples.rs) counts the entire command/admission plus
world/broadcast/hand-over tick, checking >=99% PMU scheduling. Identical
per-search/attempt/exhaustion and actual marker-update hooks remain included on
both arms; the latter includes retirement-only updates. There is no
per-body diagnostic whose elimination could inflate the saving. Held-pose
edits, trace serialization and table reduction stay outside the interval.
The [tables](tables.py) retain every pair, all ticks, search/exhaustion/other,
food epoch, minimap/marker and each arm's slowest CPU 1% separately, with CPU
p50/p99/max and paired instruction median/range. Independent p99 components
are never added. The result is fixed-work user instructions and process CPU,
not real socket time, network latency or a live deferred-tick capacity test.

Use the same harness with the `minimap` and `markers` row names for those
independent candidate/baseline builds. Minimap staging uses
`STUDY_MUTATION=map`; its queued wire-mutation control uses
`FIXED_TRACE_TICKS=720` so new viewers receive a full map after barrier release.
The marker study's wrapper instead drops marker-sector hints. Preserve
each candidate's own source pin and checks; do not treat another candidate's
gates as its evidence. No bench measurements have run yet.
