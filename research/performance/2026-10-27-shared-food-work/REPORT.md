# Shared food sector work, 2026-10-27

The fresh official-500 profile selects shared food maintenance, specifically
its full subscribed-sector walk. The candidate visits newly subscribed and
dirty sectors, while retaining the complete walk for pit motion and timed
retirement. The listing body and recipient ordering are unchanged. Production
edits are confined to `src/client/simulation/food.rs`; socket, snake updates,
shared coverage and broadcast call sites are untouched.

Base `ef866d73259e35fe8659a1996d7b97c36d511e0b`, candidate
`8ae1336cc1156063f860ee0d8f77f70c130f64aa`. Keep the sparse work selection.
All twelve complete traces remain byte-identical.
[Selected results and revision/archive/binary pins](results.json).

## Uninstrumented process pairs

One untallied warmup pair, then five alternating pairs per protocol case,
without exclusions or replacements. All process and diagnostic counters are
100% scheduled. Whole-process `instructions:u` includes
initialization, all fixture work and terminal validation; these are separate
from the diagnostic phase counts below.

| Official-500 clients | Base median | Candidate median | Median paired reduction [full range] |
| --- | ---: | ---: | ---: |
| Stock | 10,377,571,647 | 9,572,789,613 | 7.755% [7.754, 7.758%] |
| Half protocol-13 | 10,732,721,732 | 9,929,278,848 | 7.486% [7.485, 7.487%] |
| All protocol-13 | 11,067,467,553 | 10,271,078,786 | 7.195% [7.194, 7.196%] |

The twelve raw comparisons include the three permanent protocol cases, portable
and protocol fixtures, historical official-500/Battledome-64, retained
`step_all`/`next_frame`, and separate Battledome 40/64/80. Full bytes are compared
literally, including frames, order, timestamps, checkpoints and RNG endpoints;
no output fields are masked.

## Fresh whole-tick breakdown

The diagnostic counts simulation-thread user instructions around the full
`advance_tick_with`, including adjacent fixture population counting and probes,
separately from uninstrumented process comparisons.
Stock/half/all protocol-13 use the same 853 steady all-500-alive ticks
(tick >=120, excluding reconnect tick720). Every diagnostic's complete output
is compared literally with an uninstrumented control from the same revision.

| Stock phase | Mean instructions/tick, M |
| --- | ---: |
| Whole tick including probes | 9.155 |
| Core world step | 2.346 |
| Recipient snake updates (other worker) | 1.897 |
| Shared food maintenance | 0.857 |
| Recipient prey | 0.677 |
| Recipient food | 0.618 |
| Packet framing | 0.467 |
| First food-interest pass | 0.304 |
| Shared reports | 0.220 |
| Second food-interest pass | 0.153 |
| HUD | 0.041 |

The earlier unattributed work is split: advance glue/probes 0.002M,
broadcast setup/destruction/probes 0.085M, update glue/nested probes 0.195M,
and recipient assembly/destruction/nested probes 0.454M. These are labelled
residuals of inclusive brackets, not independent function costs. They include
probe overhead; do not predict a saving by subtracting overlapping phases.
The complete protocol-specific breakdown, calls and pins are selected in JSON.

## Narrow food profile and invariant

An independent 37-phase diagnostic puts shared food at 0.836M/tick: setup
0.0026M, subscriptions/retirements 0.0217M, sector loop 0.808M (including
0.050M for 18.83 listing builds), and tail/dirty clearing 0.0031M. Compiler and
probe effects make the coarse/narrow totals differ; this is diagnostic
attribution, not a process saving. The other protocol cases agree on the
bottleneck. With the candidate, stock maintenance is 0.0864M, including
0.0556M in the sector loop and 0.0507M building the same 18.83 listings.
Subscription bookkeeping rises from 0.0217M to 0.0248M; the process pairs above
include that cost. Half/all protocol-13 maintenance is 0.0864/0.0871M.
All three candidate diagnostic traces also match their uninstrumented controls.

A retained listing can change only on first subscription, shown-food edits,
or decoration changes. Track zero-to-one subscription transitions and union
those with dirty sectors, then filter by final subscription counts. A same-tick
handoff keeps its handles; a transient entry that later retires supplies no work.
Unsubscribed dirty sectors are still cleared and built afresh when subscribed.
Hidden ambient births supply work when revealed. A changed pit, including
retiring older rings without radius motion, still walks the full union.

The new oracle retains the old exhaustive method and compares every physical
and decorated listing and ordered change hint for 180 ticks in each mode. It
covers duplicate positions, births/removals, hidden reveal, full index rebuilds,
subscription handoffs/empty subscriptions, pit motion and timed retirement.
Existing handle-stability and recipient packet oracles remain in place.

## Reproduction and limits

```sh
STUDY=research/performance/2026-10-27-shared-food-work
OUT=/tmp/shared-food-new
python3 "$STUDY/reproduce.py" build "$PWD" "$OUT" --cpu 3
python3 "$STUDY/reproduce.py" trace "$PWD" "$OUT" --cpu 3
python3 "$STUDY/reproduce.py" count "$PWD" "$OUT" --cpu 3 --perf /path/to/perf
python3 "$STUDY/profile.py" ef866d73259e35fe8659a1996d7b97c36d511e0b /tmp/whole-base --control "$OUT/native-base/bin/study_protocols" --cpu 3 --build-only
python3 "$STUDY/run_profile.py" /tmp/whole-base "$OUT/native-base/bin/study_protocols"
python3 "$STUDY/food_profile.py" ef866d73259e35fe8659a1996d7b97c36d511e0b /tmp/food-base --control "$OUT/native-base/bin/study_protocols" --cpu 3 --build-only
python3 "$STUDY/run_profile.py" /tmp/food-base "$OUT/native-base/bin/study_protocols"
python3 "$STUDY/food_profile.py" 8ae1336cc1156063f860ee0d8f77f70c130f64aa /tmp/food-candidate --control "$OUT/native-candidate/bin/study_protocols" --cpu 3 --build-only
python3 "$STUDY/run_profile.py" /tmp/food-candidate "$OUT/native-candidate/bin/study_protocols"
python3 "$STUDY/mutation.py" "$PWD" /tmp/food-negative --cpu 2
cargo test --locked --lib client::simulation::food
python3 tools/check.py rust
```

Linux x86-64, Python 3.12+, Rust 1.99.0, patch and user PMU access. Default
x86-64 release target, thin LTO/one codegen unit, debuginfo=1/default features.
Build/test/count workers use one pinned CPU/job each, at most two total. The
WSL2 machine is shared. No administrator/security changes or outside game
servers. No CPU-time, latency, native EPYC transfer or capacity claim.

Raw arrays, binaries, full traces and failed preparation stay outside Git.
Retained preparation failures: the 37-phase diagnostic initially exceeded
Serde's fixed-array serialization support (fixed by serializing slices); the
scratch oracle had a missed multiline `self` reference (compile failure), then
used synthetic Boost food without birth provenance (fixture setup failure).
The selected fixture uses ambient food with explicit indexed colors. These
were fixed before the candidate was frozen; selected runs are not replaced.

The post-change diagnostic's first preparation stopped before compilation:
the frozen test oracle repeats comments used as hooks. Restricting those hooks
to production code fixes the preparation; no instruction pair or trace run was
replaced. The failed directory remains local.

Verification: twelve focused food tests, all fifteen commands in the Linux
Rust group, seven documentation tests and nine `python -O` rejection checks
pass. Omitting first-subscription work in a fresh scratch archive makes the
new oracle fail on its first tick. The negative target is isolated from all
positive checks and immutable release binaries.
