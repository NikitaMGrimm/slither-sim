# Reuse core heading and pickup geometry, 2026-10-27

The selected main baseline is `4cd6ce542b45e385d542d9423089d884bc314778`,
after #225, #224 and #226. Candidate
`536f8aa3636f367e2d32b48ca02a72e63b1e146a` changes only `physics.rs` and
`simulator.rs`. Keep the reuse: all twelve full traces match, and the final
protocol matrix uses 2.59–2.79% fewer whole-process user instructions.
[Selected results and revision/archive/binary pins](results.json).

## Fresh world-step breakdown

A separate coarse user-instruction diagnostic covers the validated world step,
including action validation, through final scores and buffer commit. Host input,
broadcast and emission are outside this budget. All 853 selected stock ticks
have 500 living snakes, tick >=120, excluding forced-reconnect tick720. Complete
control/diagnostic raw traces are byte-identical; every counter is 100% scheduled.

| Coarse phase | Base / candidate, M instructions per steady tick |
| --- | ---: |
| Action validation | 0.0071 / 0.0071 |
| Radius, alive/previous/tip setup | 0.1012 / 0.1012 |
| Movement, body, boost and kiosk | 0.4702 / 0.4757 |
| Collision bounds and head records | 0.1374 / 0.0593 |
| Collision order and index construction | 0.1872 / 0.1872 |
| Collision candidates and tests | 0.2439 / 0.2439 |
| Death resolution/drops | <0.0001 / <0.0001 |
| Food pickup/cache | 0.7715 / 0.6928 |
| Prey | 0.3956 / 0.3071 |
| Refill/admission | 0.0040 / 0.0040 |
| Final scores and buffer commit | 0.0267 / 0.0267 |
| **World step** | **2.3450 / 2.1053** |

The window has no deaths and **zero exact collision calls**: its collision
budget is broad-phase work. It cannot characterize death drops, contested dense
body contacts or expensive cold ticks.

A second, more intrusive run splits the leaf work. Rows overlap, include probe
overhead and affect compilation; do not add them or subtract them from process
counts to predict savings.

| Nested operation | M instructions/tick | Mean calls/tick |
| --- | ---: | ---: |
| BoundsIndex construction, collision and prey together | 0.2149 | 2 |
| Collision candidate queries | 0.1901 | 500 |
| Exact collision tests | 0 | 0 |
| Movement math, snake and prey | 0.2741 | 570.97 |
| Body append/contraction | 0.0012 | 14.81 |
| Speed state | 0.0403 | 500 |
| Kiosk | 0.0415 | 500 |
| Cache removal, including positions | 0.1784 | 0.863 |
| Pellet-position suffix refresh | 0.1740 | 0.863 |
| Eater-grid construction | 0.1990 | 1 |
| Pellet lookup, exact tests and sorting | 0.1939 | 1 |
| Admission counts | 0.0025 | 12.15 |
| Cache pushes | 0.0055 | 11.73 |
| Mass-field refresh | 0.1128 | 1007.09 |
| Prey threat decisions | 0.0088 | 1.416 |
| Pickup-circle construction | 0.1372 | 1000 |
| Prey motion, including its movement math | 0.0416 | 70.97 |

The candidate retains 500 circle constructions instead of 1,000. Its movement
pass costs slightly more to retain directions; the end-to-end pairs include that
cost. Collision-record, food-circle and prey-circle work fall. Coarse/nested
traces on both runtimes match their respective uninstrumented controls.

## Selected cut and exactness invariant

Retain the current heading's `(cos, sin)` already computed for snake displacement.
Collision tips and food circles reuse those exact values. Scale is still
computed after boost/kiosk changes. Food pickup changes fam, not the living set,
head, heading, speed or integer segment count, so prey reuses the food circle's
center/radius and computes the original center-minus-head lead. When food is
empty, prey builds current circles from the movement direction.

Every living slot overwrites its direction during the current movement pass.
No geometry is trusted across steps; cloned/restored worlds start with empty
scratch. Debug assertions compare cached direction/circle bits to fresh public
geometry and check complete living-slot coverage. A debug-only should-panic
regression rejects a partial circle batch.

The public `advance`, `head_tip` and `pickup_circle` remain unchanged independent
oracles. New helpers preserve their arithmetic order and libm results. The
65,536-case bit oracle covers randomized turns and numeric boundaries, signed
zeros, huge angles, segment/scale limits, stationary motion and different dt.
Another regression compares cached/fresh prey phases after fam gain, with dead
slots, actual consumption and one/two/four prey substeps, including full
checkpoint bytes and pickup events. Existing randomized/boundary collision and
kill-attribution oracles also pass.

No index/candidate order or exact predicate changes. The position suffix refresh
keeps its existing optimization; earlier fused/implicit alternatives are retained
in the [position study](../2026-10-27-pellet-position-prefix/REPORT.md).

## Paired process evidence

The complete twelve-case raw set is byte-identical: permanent official-500
stock/half/all protocol-13, portable/protocol, historical official-500 and
Battledome-64, retained step_all/next_frame, and separate Battledome 40/64/80.
Literal decompressed bytes include frames, order, timestamps, checkpoints and
RNG endpoints; no fields are masked.

One untallied warmup pair, then five alternating process `instructions:u` pairs
per protocol. Every counter is 100% scheduled; no pair is excluded or replaced.
Counts include construction, all 1,080 ticks, clients and drains, excluding
socket/kernel work. The following rows come from immutable, rehashed release
copies, separate from diagnostic probes.

| Official-500 clients | Base median | Candidate median | Median paired reduction [full range] |
| --- | ---: | ---: | ---: |
| Stock | 9,228,553,410 | 8,971,501,899 | 2.785% [2.784, 2.789%] |
| Half protocol-13 | 9,585,174,061 | 9,328,049,574 | 2.683% [2.682, 2.684%] |
| All protocol-13 | 9,927,003,035 | 9,669,873,387 | 2.590% [2.589, 2.592%] |

A feasibility-only stock pilot at `b41bb271f2b787a481906c87b96322629eee011f`
passes its complete raw trace and one warmup plus five alternating pairs: median
2.792685% fewer process instructions, range 2.791812–2.793799%, all 100% scheduled.
The final runtime adds only the debug coverage guard/regression. The pilot is
separate from the final protocol matrix.

## Reproduction and limits

The [historical wrapper](https://github.com/NikitaMGrimm/slither-sim/blob/76772d07098c884e9643783a0ba874bce0567e5e/research/performance/2026-10-27-core-heading-geometry/reproduce.py)
reuses the earlier [shared-food driver](../2026-10-27-shared-food-work/reproduce.py)
with this study's default commit pins. Retrieve its exact bytes below; the
remaining study-specific reduction is in [select.py](select.py).

```sh
STUDY=research/performance/2026-10-27-core-heading-geometry
OUT=/tmp/core-geometry-new
REPRODUCE="$OUT-reproduce.py"
git show 76772d07098c884e9643783a0ba874bce0567e5e:research/performance/2026-10-27-core-heading-geometry/reproduce.py > "$REPRODUCE"
python3 "$REPRODUCE" build "$PWD" "$OUT" --cpu 3
python3 "$REPRODUCE" trace "$PWD" "$OUT" --cpu 3
python3 "$REPRODUCE" count "$PWD" "$OUT" --cpu 3 --perf /path/to/perf
python3 "$STUDY/run_profile.py" 4cd6ce542b45e385d542d9423089d884bc314778 /tmp/core-base --cpu 3 --target /tmp/core-base-target
python3 "$STUDY/run_profile.py" 536f8aa3636f367e2d32b48ca02a72e63b1e146a /tmp/core-candidate --cpu 3 --target /tmp/core-candidate-target
python3 "$STUDY/check.py" 536f8aa3636f367e2d32b48ca02a72e63b1e146a /tmp/core-checks --cpu 2
python3 "$STUDY/mutation.py" "$PWD" /tmp/core-negative --cpu 3
```

Linux x86-64, Python 3.12+, Rust 1.99.0, patch and user PMU access. Same default
portable x86-64 release target, thin LTO/one codegen unit and debuginfo=1. At most
two logical CPUs, one job per build/test worker; counts and probes pinned to one.
The WSL2 machine is shared; no CPU-time, latency, native EPYC or capacity claim.
The change removes repeated geometry per living snake; 10k-bot and density
scaling are outside this study and belong to the separate research worker.

All work is offline, without administrator/security changes. Raw arrays,
executables and full traces remain outside Git. Pre-merge overlay compile checks
and a pre-merge runner validation are retained but unselected; they are not the
post-merge baseline. No instruction pair is excluded or replaced. Pilot checks
precede the extra debug guard; final validation runs on the final runtime.

Validation on the final runtime covers five geometry-focused tests, all four
collision-index tests, the full fifteen-command Linux Rust group, seven
documentation tests and nine study-entrypoint `python -O` rejection checks.
A fresh mutation archive uses the prior heading's direction after turning;
the public-geometry bit oracle rejects it. Positive checks and mutation builds
use separate targets. The final debug-guard recheck uses an inactive positive
Cargo cache with freshly touched archived sources, forcing recompilation.
