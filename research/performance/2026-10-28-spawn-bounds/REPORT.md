# Spawn admission bounds, 2026-10-28

Main baseline `0aba6f97070a06c73b604869761d4c5c14965a78`; runtime
`9fe3099d1983812af34b5575303fca822ad201eb`.
[Frozen plan](PLAN.md), [retained attempts](ATTEMPTS.md).

Keep for dense official admission: at 2,000 snakes, admission plus world step
falls **80.881%**, while the realistic official-500 socket row spends only
about **0.21%** of simulation-thread instructions in spawn search. This does
not materially address that host row's overall cost. Circular admission
already uses the separate `spawn::Safety` cache; its native client row is
essentially unchanged.
[Selected counts, all pairs, coverage and identities](results.json).

Each queried world state lazily caches the bounds of every snake's head and
all body anchors. A candidate visits snakes in the original order and skips
exact geometry only when a box proves it cannot matter. Boolean queries use
the greater of contact width and requested clearance. Margin queries prune
against the accumulated exact minimum, preserving even bounded early-return
bits and fallback ties. One world unit pads roundoff for coordinates within
±10 million; larger/non-finite bounds retain exhaustive geometry.

The first query costs O(total anchors); subsequent candidates cost O(snakes
plus relevant edges), rather than walking every distant body. Overlapping
boxes can still require exhaustive work: this is conservative rejection,
not a worst-case complexity guarantee. Physics invalidates the cache before
motion/growth/deaths; births and replay corrections refresh their slot.
Retirement is filtered by the live flag, and clone/restore retain correct
derived-state semantics. No candidate generation, 64-draw budget, RNG, wall
margin, exclusion, tie, fallback, mechanics or simulation population changes.

## Admission and total instructions

Millions of user instructions per physics tick, fresh current-main control.
Population rows are five-pair medians; native client attribution is one
separate diagnostic run per arm. Total is admission plus world/backend step,
excluding controller/drain/reporting work. These replace no historical #242
measurements and use no historical count as the control.

| Workload | Measured ticks | Admission: control → candidate | Total: control → candidate |
| --- | ---: | ---: | ---: |
| #242, 2,000 | 1,000 | 162.287 → 3.087 | 196.839 → 37.634 |
| #242, capped 5,000 | **20** | 6,221.953 → 72.848 | 6,291.654 → 142.539 |
| Native client official-500 | 150 | 1.1286 → 0.2058 | 21.7280 → 20.7777 |
| Native client Battledome-64 | 240 | 5.7584 → 5.7584 | 7.0284 → 7.0285 |

At 2,000, paired total reduction is 80.88056–80.88065%, admission reduction
98.0980%; all runs admit 869 lives without refusal and end with 1,999 living.
At capped 5,000, total reduction is 97.73448–97.73449%, admission reduction
98.8292%; each 20-tick window admits 65 lives without refusal and ends at
4,995 living. Endpoint mean lengths are 28.07 and 22.76 segments. These
synthetic wander worlds include substantial admission work, unlike the socket
official-500 row below. World step remains about 34.55M and 69.69M/tick after
the change; it limits the 2,000 case, while dense admission still costs 72.85M.

Independent whole-process pairs, including checkpoint validation, the 100
untallied continuation ticks and output serialization, fall 66.7842%
[66.78415, 66.78425%] at 2,000 and 56.7443% [56.74429, 56.74472%] at 5,000.
Those broader counts have a different denominator from the tick brackets.
In the full catalog, native official-500 process counts fall 2.5430%
[2.54249, 2.54319%]; native Battledome is essentially unchanged (0.00169%
more instructions). Its measured population is 42–45 of 64, with
`depleted=true`, retained rather than relabeled as full occupancy. All 23
rows' raw counts and paired ranges are selected; the largest small regression
is `step-default`, +0.0664% [+0.0588, +0.0719%]. Scenario process savings
include placement/respawn outside the step timer and must not be called
world-step savings.

### Catalog loopback host diagnostics

Five alternating diagnostic pairs after warmup; medians of per-run means,
M instructions/tick. Total covers the simulation thread's command application,
advance/broadcast and handover; search is a nested subset. Socket-thread,
load-driver and kernel work are excluded. These wall-clock-driven trajectories
differ, so this table is attribution, **not a matched speed estimate**.

| Socket row | Search: control → candidate | Total: control → candidate | Control search share, median [range] |
| --- | ---: | ---: | ---: |
| Official-500 | 0.013158 → 0.002024 | 6.239553 → 6.226790 | 0.211% [0.191, 0.212%] |
| Battledome-80 | 0.012633 → 0.012631 | 0.197453 → 0.197135 | 6.395% [4.654, 17.628%] |

Host counters report every requested socket connected at both measurement boundaries. Official driver
endpoints have 497–500 playing in control and 499–500 in candidate. Battledome
is still admitting its first lives: only 17–19 / 15–18 playing at driver end,
not 80 living or a mature crowded arena. Refusals, reconnects, actual tick
counts and all per-run ranges remain in selected JSON. No outside server or
bot-strength claim follows from these rows.

## Measurement scope

A 4-vCPU Zen 5 EPYC KVM guest, Linux x86-64, Rust 1.99.0; portable locked
release, thin LTO, one codegen unit, debug=1. Cargo/Rayon/tests use one worker;
every counted native process is pinned to one logical CPU. Builds/checks
finished first; no active compilation was present at collection start. The
guest has ordinary background services; no CPU-time, latency or capacity claim.
All measurements are offline; socket diagnostics use loopback only.
Other bench work, including compilation on another logical CPU, appeared
during collection; it was left running. Counts, all pairs and PMU scheduling
checks remain, with no uncontended timing claim.

One untallied warmup pair, then five alternating pairs per workload; no pair
is removed. Whole-process `instructions:u` include native setup/warmup/output
and a small identical Python exec trampoline, excluding the parent driver.
PMU enabled/running counts are retained and checked at ≥99%.

The deterministic matrix uses all 23 non-socket rows of the current full
catalog, with its registered sample sizes and trace windows. Separate native
client diagnostic runs bracket admission and backend operations disjointly;
controller calculation and record draining are outside those brackets.
Scenario lifecycle work occurs outside their operation timer, but inside
whole-process counts. These are synthetic fixtures, not a production mix.

The population runs reuse #242's harness at
`cae1d7a5a9c9e44f2f9565164a410020fbb7fd0b` and its retained warmed inputs.
Both arms keep its driver, 100 untallied continuation ticks, checked
memory-bounded importer and checkpoint round-trip verification. Measured
admission calls the production search, with its fast-admission switch unset.
The only harness extension is a shared count-length parameter: 1,000 measured
ticks at 2,000 snakes, **20 at capped 5,000**. The short dense window does not
replace #242's long-run population study. Outer PMU intervals bracket admission
and world step; their sum excludes controller and reporting work. Independent
whole-process counts retain all preparation/serialization costs.

## Verification and reproduction

The exhaustive pre-change predicates are retained as a bit/boolean oracle.
All **23 full-catalog deterministic trace windows** match literally, including
their terminal reports: 1,137,628,349 uncompressed bytes per arm. Every full
measured run also has equal selected lifecycle/population/final statistics.
All six pairs per population world (warmup included) have literal complete
endpoint checkpoint and driver/RNG equality, equal per-tick lifecycle counters,
and identical repeated endpoint hashes. This retains endpoints and per-tick
life counters, not a complete checkpoint at every population tick. Socket
rows have no deterministic cross-arm trace claim.
Coverage includes a 20,000-anchor body, crossing edges with distant endpoints,
clearance larger than contact width, next-float boundaries, wall rejection,
an excluded slot, empty/historical and extreme-coordinate geometry, append,
retirement/respawn, mass/pose/import edits, tick invalidation, clone/restore
and consecutive admissions. Reversing box rejection fails two tests; the
restored implementation passes all three. The complete Linux Rust check group
and seven documentation tests pass. Cross-platform CI remains separate.

The source receipts verify all four changed production files against the
runtime commit; the checkpoint research overlay is append-only. Each arm uses
a separate Cargo target, cleans/recompiles the root package and copies/rechecks
executables before collection. Task-owned Rust targets were removed after
build/check completion. Raw logs, source snapshots, copied binaries, complete
compressed matrix traces and population endpoint bytes remain outside Git;
repeated endpoints are checked before discarding redundant copies.

On a Linux x86-64 bench with user PMU access, replace the path placeholders:

```sh
STUDY=research/performance/2026-10-28-spawn-bounds
OUT="<bench-root>/spawn-bounds"
FIXTURES="<bench-root>/population-scaling/results-baseline"
git fetch origin refs/pull/242/head
python3 "$STUDY/reproduce.py" prepare "$OUT" --repository "$PWD" \
  --base 0aba6f97070a06c73b604869761d4c5c14965a78 \
  --candidate 9fe3099d1983812af34b5575303fca822ad201eb --cpu 0
python3 "$STUDY/reproduce.py" build "$OUT" --cpu 0
python3 "$STUDY/socket_diagnostic.py" build "$OUT" --cpu 0
python3 "$STUDY/reproduce.py" matrix "$OUT" --cpu 0
python3 "$STUDY/reproduce.py" scaling "$OUT" --fixtures "$FIXTURES" --cpu 0
python3 "$STUDY/socket_diagnostic.py" count "$OUT" --cpu 0 --driver-cpu 2
python3 "$STUDY/summarize.py" "$OUT" "$OUT/results.json"
python3 "$STUDY/reproduce.py" cleanup "$OUT" --cpu 0
```

The fixture directory must contain #242's original `2000-common` and
`5000-capped` world/driver pairs with the selected input hashes; do not silently
substitute a freshly generated scene. Its pinned `reproduce.py` documents
preparation when those local-only inputs are unavailable. For checks, use a
separate pristine checkout of the runtime with one worker and
`RUSTC="$(rustup which rustc)" RUSTDOC="$(rustup which rustdoc)"`:
`python3 tools/check.py rust`. `mutation.py SOURCE TARGET NEW_OUTPUT --cpu 0`
temporarily edits only that task-owned check copy and restores it even on
failure. Every study entry point rejects Python optimization.
