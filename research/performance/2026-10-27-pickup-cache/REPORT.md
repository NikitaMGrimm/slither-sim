# Compact pickup-cache survivors, 2026-10-27

Use the already ordered consumed-pellet indices to update admission-cell and
ambient counts, then copy contiguous survivor ranges in `FoodCache`. Previously
the cache scanned every pellet twice. Physical food still uses its existing
stable retain; pickup geometry, reverse food visitation, nearest-head selection,
lexical ID ties, event/mass order and RNG operations are unchanged.

Measured runtime: main `af38bf3` -> `7d03f7d`, including #204 and #205 in both
arms. The delivery head also contains the research record. No implementation
edits to recipient broadcasting or the socket gateway.
[Selected results and identities](results.json); raw evidence is local-only.

## Narrow pickup profile

Exploratory current-thread `instructions:u` brackets on the original main
`8acb60b`, before choosing the cache change. Reused #204's official-500 fixture:
853 ticks with tick >=120, all 500 alive, excluding forced reconnect tick720.
Means below divide by all 853 ticks, including ticks without removals. Diagnostic
hooks affect compilation and include bracket overhead; these are phase budgets,
not unprofiled savings or a native-host capacity estimate.

| Pickup phase | Million user instructions / steady tick |
| --- | ---: |
| Eater-circle construction | 0.115 |
| Eater-grid construction | 0.202 |
| Pellet lookup, exact tests and hit collection | 3.101 |
| Event recording and mass gain | 0.002 |
| Removal mask, cache counts and cache compaction | 0.658 |
| Physical-food stable retain | 0.585 |
| **Total** | **4.663** |

All selected ticks use the dense grid. A separate every-64th-pellet diagnostic,
subtracting a measured 44-instruction empty bracket, estimates 2.741M for
candidate-cell lookup and 0.142M for candidate iteration, circle tests and head/ID
selection. These sampled estimates are not an exact additive decomposition of
the normal-compiler scan. A separate Rust global-allocator probe records zero
pickup allocations, reallocations and frees across the selected ticks. Scratch
vectors are reused. Recipient food-sector delivery is outside this pickup phase;
physical admission-cell bookkeeping is inside the cache-removal row.

The 0.658M removal row justified trying a small change that leaves the much
larger lookup path untouched. The cache receives ascending original indices
from the reversed hit list, decrements counts in the same order as before and
compacts surviving entries without per-entry removal tests. No persistent index,
new cache state, arithmetic shortcut or new gameplay rule.

## Paired instructions and complete traces

WSL2 Linux x86-64, Rust 1.99.0 locked release/debug=1, thin LTO, one codegen unit.
Cargo/Rayon/tests one job; task work uses at most two logical CPUs, counted
children one CPU. One untallied warmup pair, then five alternating whole-process
`instructions:u` pairs per case, counters >=99% scheduled, immutable executable
copies rehashed before/after. Includes construction, all 1,080 ticks, clients and
drains; excludes sockets and kernel work. No counted pair excluded.

| Official 500 | Base / candidate median, billion instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 18.375 / 17.596 | 4.241% (4.219–4.246%) |
| Half protocol 13 | 18.919 / 18.136 | 4.137% (4.104–4.144%) |
| All protocol 13 | 19.437 / 18.658 | 4.008% (4.004–4.031%) |

One separate instrumented run per arm over the same 853 steady stock ticks
measures simulation-thread `advance_tick_with` means of 16.127M -> 15.453M
user instructions (4.178% lower). This diagnostic excludes fixture input/drains
and does not replace the five unprofiled whole-process pairs above.

All three official-500 complete raw streams match byte for byte: frames, packet
order, delivery timestamps, initial/final checkpoints and spawn/latency RNG
endpoints. Physical checkpoints also agree across protocols. All nine historical
full traces match: portable, protocol, official500, Battledome64, retained
step_all/next_frame, and the separate Battledome40/64/80 catalog. Full streams
are compared directly after decompression, not only by hashes.

These are fixed synthetic fixtures. Shared-machine WSL2 timing is contaminated;
there is no CPU-time, latency, one-core capacity or EPYC transfer claim.

## Validation and reproduction

Linux Rust check group passes on `25dc14f` before upstream integration. On the
integrated runtime, formatting, default all-target Clippy and 110 focused food,
protocol and loopback-server tests pass. New regressions pin survivor order,
ambient/all-cell counts, reverse hit order, nearest-head selection and lexical
ties. Writing survivor ranges to the wrong destination fails the new regression.
Windows/macOS and the full integrated matrix await PR CI.

Run [reproduce.py](reproduce.py) modes `build`, `trace`, then `count`, each with
`REPOSITORY OUTPUT --cpu CPU`; `count` also needs `--perf PATH`. The wrapper pins
#204's shared fixture and historical harness. Optional `PICKUP_TARGET_ROOT`
reuses task-owned Cargo caches; binaries are still copied and rehashed. Both
the wrapper and historical child reject Python optimization, preserving checks.

Raw traces, counter arrays, diagnostic overlays, binaries and failed attempts
remain outside Git. Retained attempts include the initial Windows/WSL Git-path
failure, a failed diagnostic output path, a failed negative-control launcher,
forced-noinline profiling and builds/traces superseded by upstream integration.
None enters selected instruction pairs. Source/binary/raw digests establish
identity; local-only raw material is not publicly retrievable from this record.
