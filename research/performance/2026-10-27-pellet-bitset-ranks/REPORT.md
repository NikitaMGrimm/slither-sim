# Pellet positions from live-bit ranks, 2026-10-27

Initial main baseline is `4532a76f22958ae3abcca801e90edd30ed9f466f`, after
#228, #229 and #227. Initial measured runtime is
`47e416a3eacc99163a3af12fa5eb11af317f9e28`.
The initial delivery adds this record; its production change is in
`src/food_grid.rs`. The amendment also specializes pickup in `src/simulator.rs`.
[Selected results and artifact identities](results.json).

## Periodic rebuild amendment

Published runtime: `9a3f6f5fe85184e42b32c2251480b94356c9210e`. The initial
rank implementation (`8a756344`) did create a large periodic spike. A first
three-pass in-place prototype still cost 2.55M on the standard official replay
and 5.48M in the food-rich orbit stress. That rejected prototype and all raw
arrays/binaries remain local-only; the original PR snapshot is retained below.
The final change migrates at most
**512 pellets per pickup query**, spreading both construction and old-cell
deallocation across ticks.

The new index always represents the live food prefix. Copied pellets leave the
old cell lists but remain in its rank bits, so the old suffix still maps to
full food positions. New births append to the old index. Removals mirror both
rank maps and compact the new prefix; queries concatenate its cells with the
remaining old cells, emitting each pellet once. Exact tests and the final
descending hit sort are unchanged. When migration finishes, old cell lists are
already empty, avoiding a final per-cell destructor spike.

A temporary unconditional union iterator reduced the short replay's saving to
1.74–1.88%. The final pickup helper dispatches once per phase: ordinary ticks use
the original single-index iterator, and only active migrations use the union.
Both instantiate the same exact tests and final descending sort.

### Frozen long runs

Each arm runs **60,000 ticks (420 virtual seconds)** at the default portable
target, with current-thread user instructions at ≥99% scheduling. The baseline
is `4532a76f`, the initial rank arm is `8a756344`, and the migrated arm is the
published runtime above. Frozen base/initial-rank runs are reused unchanged for
the final query specialization, checking source/fixture/probe/binary hashes;
the final candidate is freshly built and run. Unpublished experiments stay in
the local-only raw archive.

The loop case extends the existing stock protocol fixture. Orbit stress adds a
synthetic tangent controller with radial correction toward 65% of the food
radius, sustaining food consumption/churn; Battledome uses that controller to
enter its pit. Shrink holds 500 seats for 30,000 ticks, then retires four every
12 ticks down to 100. All controls, reconnect admission and drains are outside
the tick bracket. The bracket includes world, host broadcast and emission into
queues; it excludes sockets/kernel work. Both raw frame order/timestamps and
initial/final checkpoints/RNG endpoints match literally across all three arms
for all four runs. Birth/food/radius/alive/map metadata also match at every tick.

Births below are actual admitted food-birth events, excluding the initial food
population. Gaps are observed **between rebuild starts**, not a stationary-rate
prediction. Durations are the number of ticks occupied by a completed migration.

| Case | Births/tick mean [max] | Rebuilds | Start gap median [range], ticks / seconds | Migration duration, ticks | Old atomic max / new batch max, M instructions |
| --- | ---: | ---: | --- | --- | ---: |
| Official 500, loop | 1.274 [66] | 1 | No second start; right-censored | 112–112 | 25.546 / 0.276 |
| Official 500, orbit stress | 9.901 [83] | 3 | 17248 [16226–18270] / 120.736 [113.582–127.89] | 249–273 | 43.200 / 0.279 |
| Battledome 80 seats | 2.621 [547] | 27 | 1962.5 [1571–2734] / 13.7375 [10.997–19.138] | 9–13 | 1.794 / 0.217 |
| Official 500 → 100 shrink | 1.398 [66] | 2 | 45658 [45658–45658] / 319.606 [319.606–319.606] | 75–112 | 25.536 / 0.277 |

The standard loop has one start at tick 8,607 (60.249 s), followed by 51,393
right-censored ticks (359.751 s) without a second start. Its births drop to
0.0204/tick over the last 10,000 ticks; extrapolating the short steady fixture’s
early birth rate would overstate rebuild frequency. Every case exceeds 20k
ticks, and Battledome supplies 27 completed rebuilds. The configured 80-seat
Battledome run has 38–80 living players, not a claim of 80 continuously alive.

### Whole host-tick instruction tails

Units are M user instructions. p99/p99.9/max include **all 60,000 ticks**,
including cold startup. The final column separately shows the maximum after
tick 119. Quantiles use linear interpolation at `(n−1)q`; no attempt is removed.

| Case | Arm | p99 | p99.9 | Max, all ticks | Max, tick ≥120 |
| --- | --- | ---: | ---: | ---: | ---: |
| Official 500, loop | Base | 9.104 | 10.131 | 175.499 | 18.949 |
| Official 500, loop | Initial rank | 9.061 | 10.134 | 175.242 | 30.784 |
| Official 500, loop | Migrated | 9.036 | 10.115 | 175.234 | 18.799 |
| Official 500, orbit stress | Base | 25.889 | 28.496 | 175.404 | 34.069 |
| Official 500, orbit stress | Initial rank | 25.239 | 27.737 | 175.100 | 55.376 |
| Official 500, orbit stress | Migrated | 25.215 | 27.703 | 175.094 | 33.605 |
| Battledome 80 seats | Base | 10.500 | 16.323 | 23.017 | 23.017 |
| Battledome 80 seats | Initial rank | 10.484 | 16.298 | 22.979 | 22.979 |
| Battledome 80 seats | Migrated | 10.484 | 16.295 | 22.982 | 22.982 |
| Official 500 → 100 shrink | Base | 9.095 | 9.970 | 175.532 | 18.954 |
| Official 500 → 100 shrink | Initial rank | 9.036 | 9.899 | 175.185 | 30.775 |
| Official 500 → 100 shrink | Migrated | 9.013 | 9.869 | 175.179 | 18.791 |

The largest whole-tick increase versus the base at the same maintenance tick
is **0.511M instructions**, below the requested approximately 2M allowance.
Cold index construction remains separate (18.12M instructions for official
500); it is included in the startup-inclusive tail table. The common first
official host tick is about 175M; cold index construction still uses a full build.
p99/p99.9 are not uniformly lower: the change specifically removes the periodic
single-tick rebuild spike. These are WSL2 instruction results, with no CPU-time
or capacity claim.

### Final whole-process pairs

The final specialized candidate repeats one untallied warmup pair and five
alternating pairs per protocol, with all counters 100% scheduled. Counts include
construction, 1,080 ticks, clients and drains; they exclude socket/kernel work.
All twelve standard full traces match literally. This supersedes the initial
rank-only short-pair table retained below.

| Official-500 clients | Base median | Candidate median | Median paired reduction [full range] |
| --- | ---: | ---: | ---: |
| Stock | 8,778,657,012 | 8,554,349,235 | 2.555% [2.554, 2.556%] |
| Half protocol-13 | 9,134,883,406 | 8,908,245,852 | 2.481% [2.479, 2.483%] |
| All protocol-13 | 9,476,421,969 | 9,251,833,814 | 2.370% [2.368, 2.371%] |

### Amendment validation and reproduction

The prefix/suffix edit oracle compares against a full recount after mixed
removals on both sides of the frontier, births, cloning and emptying both
halves. It also checks that completion restores a single index. A mutation
that leaves a copied pellet in the old cell list fails that oracle. All 75
food-focused tests, both pickup integration suites and the complete Linux Rust
group pass on the final published runtime. Documentation and all twelve Python
optimization guards pass separately.

```sh
STUDY=research/performance/2026-10-27-pellet-bitset-ranks
python3 "$STUDY/long.py" "$PWD" /tmp/rebuild-long-new --cpu 2
python3 "$STUDY/reproduce.py" build "$PWD" /tmp/rebuild-short-new --cpu 2 --candidate 9a3f6f5fe85184e42b32c2251480b94356c9210e
python3 "$STUDY/reproduce.py" trace "$PWD" /tmp/rebuild-short-new --cpu 2 --candidate 9a3f6f5fe85184e42b32c2251480b94356c9210e
python3 "$STUDY/reproduce.py" count "$PWD" /tmp/rebuild-short-new --cpu 2 --candidate 9a3f6f5fe85184e42b32c2251480b94356c9210e --perf /path/to/perf
python3 "$STUDY/check.py" 9a3f6f5fe85184e42b32c2251480b94356c9210e /tmp/rebuild-check-new --cpu 2 > /tmp/rebuild-check-new.log
python3 "$STUDY/rebuild_mutation.py" "$PWD" /tmp/rebuild-mutation-new --cpu 2
taskset -c 2 python3 "$STUDY/select_long.py" /tmp/rebuild-long-new /tmp/rebuild-short-new /tmp/rebuild-check-new /tmp/rebuild-check-new.log /tmp/rebuild-mutation-new
```

The check helper’s stdout must be retained in `/tmp/rebuild-check-new.log`.
Earlier measurements used at most CPUs 2/3, with one Cargo/Rayon/test job.
After the owner reduced the machine budget, final validation, artifact selection
and cleanup run serially on CPU 2 with one Cargo/Rayon/test job. Full traces were
compared before deletion under that storage policy; selected hashes and
reproducers remain in Git, with losslessly compressed per-tick counters, logs,
source snapshots and failed-attempt metadata retained locally outside Git.
Task-owned Rust target directories are removed after verification. The initial
Battledome preparation incorrectly required its forced reconnect to succeed;
the final fixture records real refusals and
uses the pit-entering controller instead. All entry points reject Python `-O`.
Selected amendment data: [long-results.json](long-results.json).

### Review hardening

Runtime `ec0dd92fa813c0a7be0f07628984c2d9dbf192c1` adds a push-side
guard at twice the shared dense rebuild threshold: four times
`max(entries.len(), 4096)`. Sparse/no-eater stretches can no longer accumulate
retired handles indefinitely; the guard drops both layers, and the next dense
query cold-builds. The regression churns at constant food without queries,
both with and without an active migration, then checks a rebuilt index against
the independent recount. Dense queries retain their original threshold and
512-pellet migration, so the prior instruction results were not remeasured.

All twelve full traces match the owner's merged revision `bf3d103e` literally;
76 food-focused tests, both pickup suites, the complete Linux Rust group,
documentation checks and selector controls pass. Collision-pilot source hashes
must match the recorded overlay hashes before selection, and JSON serialization
rejects NaN and both infinities. Historical results retain their pinned measured
archive hashes; neither historical record stores a standalone selector hash.
[Selected review validation](review-validation.json) records the new checks.
Validation ran serially on CPU 2 with one Cargo/Rayon/test job; task-owned Rust
targets and large trace payloads were removed after comparison.

## Initial rank implementation record
## Collision feasibility gate

Against post-#228 main `194d13047775cca92e2a7f5163e9fdd28e918073`, an
uncommitted persistent `BoundsIndex` prototype recomputes every current
integer span, retaining memberships when the span agrees. Population or hash-mask
changes rebuild; changed spans remove all old occurrences from hashed buckets
before inserting their new cells. This preserves even hash false positives,
fallback and complete descending deduplicated candidate vectors. Six focused
tests pass, including fresh-index comparisons across geometry/ordinal/population
changes and the existing randomized/boundary death/kill oracle.

The complete stock raw trace matches main. One untallied warmup and five
alternating whole-process instruction pairs save **1.054%** (1.052–1.057%),
8.971715485B -> 8.877211160B median instructions, all 100% scheduled. This misses
the owner's 2% gate. Reject the collision prototype and take pickup/cache instead;
the prototype sources, binaries and full pairs stay outside Git. No high-density
measurement or collision-cache adoption claim follows from this pilot.
Membership reuse addresses index construction; body-bound scans and the 500
candidate queries per tick still contribute most of the collision phase.

## Selected pickup invariant

Cell lists hold monotonically appended handles. Stable food compaction preserves
handle order. A live handle's food position is therefore the number of preceding
live bits: a prefix count before its 64-bit word plus a population count below
its bit. Removals clear bits and refresh word prefixes starting at the first
affected word. They no longer write a food position into every surviving handle.
Cell-list offsets still repair swapped entries during removal.

New handles append after all older live handles, including after emptying the
list. Public food edits/reset/restore retain their existing cache invalidation.
Queries rebuild lazily when retained slots reach twice `max(live food, 4096)`;
handle exhaustion also discards the index before an append. This bounds churn
storage at query boundaries and includes periodic rebuilds in the algorithm.
The earlier implicit-position prototype used per-query binary search; the
[position study](../2026-10-27-pellet-position-prefix/REPORT.md) retains its loss
against prefix skipping. This study uses bit ranks instead.

The four eater-cover coordinates are already validated within +/-8193. Use the
existing exact small-integer floor helper there, preserving the previous cover
bits and boundary padding. A separate stock pilot for this arithmetic alone
saves 0.468% (0.467–0.469%). The combined rank/floor pilot saves 2.384%
(2.382–2.385%), with complete raw stock parity and all counters 100% scheduled.
The final runtime removes unused retired-ID storage and adds regression checks;
it receives a separate complete protocol matrix.

The completed pre-integration matrix at `194d1304` -> `81c3e503` saved
2.214–2.384% across the three protocols, with all twelve traces equal. Its full
arrays remain alongside the repeated final-main matrix; selected medians are
retained separately in `results.json`.

No exact pickup predicate, nearest-head/lexical-ID tie, event/removal order,
collision/death resolution, RNG, mass arithmetic or checkpoint format changes.

## Full output and process evidence

All twelve complete raw traces are byte-identical: permanent official-500
stock/half/all protocol-13, portable/protocol, historical official-500 and
Battledome-64, retained step_all/next_frame, and separate Battledome 40/64/80.
Literal decompressed comparisons cover frames, order, timestamps, checkpoints
and RNG endpoints; no fields are masked. Protocol variants preserve the same
physical checkpoints.

One untallied warmup pair, then five alternating whole-process `instructions:u`
pairs per protocol. All counters are 100% scheduled; no pair is excluded or
replaced. Counts include construction, all 1,080 ticks, clients and drains,
excluding socket/kernel work. Executable copies are immutable and rehashed.

| Official-500 clients | Base median | Candidate median | Median paired reduction [full range] |
| --- | ---: | ---: | ---: |
| Stock | 8,779,200,127 | 8,564,765,894 | 2.443% [2.440, 2.443%] |
| Half protocol-13 | 9,133,454,696 | 8,920,872,454 | 2.328% [2.326, 2.333%] |
| All protocol-13 | 9,476,795,796 | 9,262,490,411 | 2.262% [2.260, 2.262%] |

## Fresh world-step diagnostics

Separate coarse/nested user-instruction probes cover 853 steady stock ticks:
tick >=120, all 500 alive, forced-reconnect tick720 excluded. Complete probe
traces match their uninstrumented controls; all counters are 100% scheduled.

The profile revisions are post-#228 main `194d1304` and runtime `81c3e503`.
The final rebase adds #229's common host broadcast change and #227's docs to
both sides. Every simulator-core source and Cargo/build input agrees with the
respective profiled revision; the reducer verifies the only runtime file delta
is `src/client/simulation/broadcast.rs`. Profiles retain their original build
context; final whole-process pairs and traces use the updated main/runtime above.

| Coarse phase | Base / candidate, M instructions per steady tick |
| --- | ---: |
| Action validation | 0.0071 / 0.0071 |
| Radius, alive/previous/tip setup | 0.1012 / 0.1012 |
| Movement, body, boost and kiosk | 0.4757 / 0.4764 |
| Collision bounds and head records | 0.0593 / 0.0593 |
| Collision order and index construction | 0.1872 / 0.1872 |
| Collision candidates and tests | 0.2439 / 0.2434 |
| Death resolution/drops | <0.0001 / <0.0001 |
| Food pickup/cache | 0.6928 / 0.5418 |
| Prey | 0.3072 / 0.3072 |
| Refill/admission | 0.0040 / 0.0041 |
| Final scores and buffer commit | 0.0267 / 0.0267 |
| **World step** | **2.1054 / 1.9545** |

The window has no deaths and zero exact collision calls. It measures broad-phase
collision work, not dense body contacts/death bursts. The pre-#228 quoted 0.569M
collision budget is now about 0.490M. Host input/broadcast/emission are outside
this world-step budget.

Nested rows overlap, include bracket overhead and affect compilation; do not add
them or subtract them from process counts to predict savings.

| Nested pickup operation | Base / candidate, M instructions per steady tick |
| --- | ---: |
| Cache removal, including position/rank refresh | 0.1784 / 0.0112 |
| Survivor-position / word-prefix refresh | 0.1740 / 0.00685 |
| Eater-grid construction | 0.1990 / 0.1641 |
| Pellet lookup, exact tests and sorting | 0.1939 / 0.2438 |

Rank lookup costs more per queried pellet, while the word-prefix maintenance and
exact cover arithmetic reduce total work. The final process pairs include those
query costs, maintenance, cold construction and the rest of the host fixture.
Long-run churn and 10k density costs receive no separate empirical claim here;
the regression explicitly crosses the lazy-rebuild threshold.

## Validation

The 74 food-focused tests and both pickup integration suites cover ordering,
nearest-head/lexical-ID ties, one winner per pellet, bit-word edges, entirely
retired words, first/middle/tail/empty/all removals, cell-list swap repair,
appends, cloning, cache resets/restores and radius/pit retention. The 8,000-edit
recount oracle checks full cell membership and ranks after each edit. A new
9,000-pellet regression checks lazy rebuild after substantial shrink. The bounded
floor oracle compares library floor at all +/-8193 cell edges and adjacent values,
including signed zero and subnormals.

The mutation includes the current live bit in its rank; the new oracle rejects
it. The first wrapper expected a trailing colon in Rust's assertion diagnostic
and rejected the correctly failing test log. A fresh archive with the corrected
format check passes the control; both attempts remain outside Git. Positive and
mutation targets stay separate. Reused inactive caches rebuild freshly touched
archived sources.

The complete fifteen-command Linux Rust group passes on the rebased runtime,
including all lint configurations, default/core/bot/networking tests, doctests,
C ABI and the Python crate build. Seven documentation checks and all nine
study-entrypoint optimization-rejection checks pass. Other platforms await CI.

## Reproduction and limits

```sh
STUDY=research/performance/2026-10-27-pellet-bitset-ranks
OUT=/tmp/pellet-ranks-new
python3 "$STUDY/reproduce.py" build "$PWD" "$OUT" --cpu 3
python3 "$STUDY/reproduce.py" trace "$PWD" "$OUT" --cpu 3
python3 "$STUDY/reproduce.py" count "$PWD" "$OUT" --cpu 3 --perf /path/to/perf
python3 "$STUDY/run_profile.py" 194d13047775cca92e2a7f5163e9fdd28e918073 /tmp/rank-base --cpu 3 --target /tmp/rank-profile-target
python3 "$STUDY/run_profile.py" 81c3e503ea657f5e9416910bb4d9c8841efb9774 /tmp/rank-candidate --cpu 3 --target /tmp/rank-profile-target
python3 "$STUDY/check.py" 47e416a3eacc99163a3af12fa5eb11af317f9e28 /tmp/rank-checks --cpu 2
python3 "$STUDY/mutation.py" "$PWD" /tmp/rank-negative --cpu 3
```

Linux x86-64, Python 3.12+, Rust 1.99.0, patch and user PMU access. Identical
default portable x86-64 release target, thin LTO/one codegen unit, debuginfo=1.
At most two logical CPUs, one job per Cargo/Rayon/test worker; counted children
pinned to one. The machine is shared WSL2: no CPU-time, latency, native EPYC or
capacity claim. 10k-snake/density scaling belongs to the separate research worker.

At initial delivery, raw arrays, complete traces, rejected/unbuilt prototypes,
executables and diagnostic overlays were retained locally outside Git. After
the amendment's full comparisons, the owner's storage policy removed large
trace payloads and Rust targets; compressed counters, hashes, logs and source
snapshots remain. Digests verify artifact consistency, not capacity.
All work is offline with no administrator/security
changes. Every study entry point rejects Python optimization; the delegated
historical helpers receive the same guard. No instruction pair is excluded.
