# Pellet positions from live-bit ranks, 2026-10-27

Final main baseline is `4532a76f22958ae3abcca801e90edd30ed9f466f`, after
#228, #229 and #227. Final runtime is
`47e416a3eacc99163a3af12fa5eb11af317f9e28`.
The delivery adds this record; production changes stay in `src/food_grid.rs`.
[Selected results and artifact identities](results.json).

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

Raw arrays, complete traces, rejected/unbuilt prototypes, executables and
diagnostic overlays remain local-only outside Git. Digests verify artifact
consistency, not capacity. All work is offline with no administrator/security
changes. Every study entry point rejects Python optimization; the delegated
historical helpers receive the same guard. No instruction pair is excluded.
