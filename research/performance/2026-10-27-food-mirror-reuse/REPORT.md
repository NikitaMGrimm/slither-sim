# Reuse verified food mirrors, 2026-10-27

Keep. The physics profile and original pairs compare main `65ff713` (including
#208/#209) -> runtime `3cc3646`. An integration recheck compares main `587b597`
(including #210) -> `e0a71f1`, with identical core changes.
[Selected results and identities](results.json) retain both revision pairs; the
delivery head updates documentation only. Production changes stay in
`food_grid.rs` and `simulator.rs`; no server or recipient-broadcast implementation
changes.

## Fresh core physics profile

Separate current-thread `instructions:u` brackets, official-500 stock fixture,
same 853 steady ticks as #209: tick >=120, all 500 alive, forced reconnect
tick720 excluded. Means below cover `Simulator::advance`, after action validation
through final score updates; input handling and broadcast are outside this scope.
Hooks affect compilation and include overhead, so these are diagnostic budgets,
not predictions to subtract from unprofiled process counts.

| Core phase | Base / candidate, M instructions per steady tick |
| --- | ---: |
| Tick setup, radius, alive/previous/tip buffers | 0.101 / 0.101 |
| Movement, boost, body and kiosk pass | 0.826 / 0.469 |
| Collision bounds and head records | 0.137 / 0.137 |
| Collision ordering and bounds-index construction | 0.245 / 0.245 |
| Collision candidates and exact tests | 0.294 / 0.294 |
| Death resolution and drops | <0.001 / <0.001 |
| Food pickup, including cache work | 1.880 / 1.476 |
| Prey | 0.462 / 0.461 |
| Ambient supply | 0.004 / 0.004 |
| Final score updates and buffer commit | 0.027 / 0.027 |
| **Total** | **3.976 / 3.214** |

A nested bracket identifies **0.743M -> 0 instructions/tick in food-mirror
verification**, shared by movement/boost admission and pickup. Do not add it to
the table: it is already included. This was the largest individually avoidable
operation identified, larger than the entire 0.676M collision phase. The selected
steady window has no deaths; it cannot characterize expensive death-drop ticks.

## Ownership invariant and change

The mirror compared every pellet's coordinate bits and ambient flag every tick,
although the simulator owns its food list. Retain the successful verification
across owned spans with one private cache flag and an O(1) length check:

- Internal births and pickups already mirror every push/removal, including
  occupancy counts, stable compaction and pellet-index maintenance.
- Public insert/remove/replace APIs reset the cache; checkpoint restore starts
  with a default cache. Same-length replacements therefore cannot reuse proof.
- Radius/pit retention is the only other edit and only removes items. A length
  change invalidates the next owned span before cached data is consumed.
- Ordinary `begin()` and reads outside a span still compare untrusted lists bit
  for bit. Initial/unverified or stale caches still verify/rebuild normally.

No collision/death ordering, exact predicates, mass arithmetic, food selection,
event ordering, RNG operation or serialized state changes. Existing collision
rationale remains beside its original code; the ownership rationale is beside
the new span entry and the length guard.

## Paired process instructions and full traces

WSL2 Linux x86-64, Rust 1.99.0 locked release/debug=1, thin LTO, one codegen unit.
Cargo/Rayon/tests one job; at most two logical CPUs, counted child pinned to one.
One untallied warmup pair, then five alternating whole-process `instructions:u`
pairs per protocol case, all counters 100% scheduled. Immutable executable copies
rehashed before/after. Includes construction, all 1,080 ticks, clients and drains;
excludes sockets and kernel work. No counted pair excluded.

The table reports the integration recheck, with #210 on both sides. The original
revision pair used the same protocol and gave 5.505%, 5.323% and 5.136% median
paired reductions; its selected numbers remain in `results.json`.

| Official 500 | Base / candidate median, B instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 13.841 / 13.025 | 5.902% (5.875–5.909%) |
| Half protocol 13 | 14.370 / 13.552 | 5.710% (5.687–5.724%) |
| All protocol 13 | 14.891 / 14.071 | 5.503% (5.482–5.522%) |

All twelve complete raw traces are byte-identical: three permanent-protocol
official500 variants and the nine historical portable/protocol, official500,
Battledome64, retained step_all/next_frame and separate Battledome40/64/80 cases.
Direct decompressed comparison covers frames/order/timestamps, checkpoints and
spawn/latency RNG endpoints. Physical checkpoints also match across protocols.
All twelve comparisons pass on both revision pairs; the integration recheck has
another warmup plus five alternating pairs per protocol, all 100% scheduled.

These fixed synthetic fixtures support no CPU-time, latency, capacity or native
EPYC transfer claim. Shared-machine WSL2 timing is contaminated.

## Validation and reproduction

Full Linux Rust group passes for the original and integrated candidates,
including all lint configurations, default/core/bot/networking tests, doctests,
C ABI and Python crate build. The 58 focused food tests pass. New tests verify
owned-span reuse, untrusted/same-length edits,
retention, public reset and restore behavior. Warm worlds match a fresh-food-cache
oracle on every checkpoint, pickup/birth event and death through recorded radius
changes, ordinary/pit retention and public replacement. The existing 8,000-edit
pellet/cache oracle remains unchanged.

Removing the length guard fails the retained-food/fresh-cache oracle. Removing
the public replacement reset fails the same-length origin-change oracle. Both
mutation controls are retained. Documentation checks pass; Windows/macOS and the
remaining matrix await PR CI.

Run [reproduce.py](reproduce.py) modes `build`, `trace`, then `count` with
`REPOSITORY OUTPUT --cpu CPU`; count also needs `--perf PATH`. The wrapper reuses
#204's pinned shared fixture/historical harness and rejects Python optimization,
as does the historical child. Optional `PICKUP_TARGET_ROOT` reuses task-owned
Cargo caches; executables are still copied and rehashed.

Raw traces, counter arrays, binaries, diagnostic overlays and failed attempts
stay outside Git and are local-only. Retained failures include compile-only
profiler preparation, an initial missing test import, and the superseded
through-ambient profile before the final score pass was bracketed. None enters
the selected instruction pairs. Digests establish artifact identity, not product
capacity or scientific rules.
