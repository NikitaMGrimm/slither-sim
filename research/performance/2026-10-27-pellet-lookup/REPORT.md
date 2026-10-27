# Drive food pickup from occupied eater cells, 2026-10-27

Keep the pellet index. Main `422a9e9` (#207) -> measured runtime `967d076`.
The delivery head also fixes the allocation-test probe and adds this record;
simulation sources are unchanged after measurement. [Selected results](results.json).
No socket or recipient-delivery implementation changes.

## Mechanism and maintenance

The old dense path checked every pellet against the eater grid each tick.
Now visit each occupied eater cell once and query its pellets. Reuse the exact
ascending eater list and `find_eater` predicates, including nearest-head and
lexical-ID ties. Each pellet has one cell/handle, so it can produce at most one
hit; sort hits by descending original pellet index before events, mass changes
and stable removal. No gameplay rule, floating-point test or RNG operation changes.

Cell lists hold stable handles. Internal births append a handle; removal uses
its cell-list offset and repairs the swapped handle. Food compaction preserves
handle order and refreshes their current dense-array indices. **That refresh is
still linear in surviving food**; the measurements include it. Empty cell lists
are removed and free handles reused. This adds persistent cell/handle storage.

The index is lazy inside the existing bit-verified `FoodCache`. Public food
insert/remove/replace APIs already reset that cache; checkpoint restore starts
with a default cache. The next query rebuilds. Internal mirrored pushes/removals
maintain membership; radius/pit retention is detected by the next mirror check.
The regular-grid coordinate bound admits packed cells without aliasing. Invalid
or distant pellets cannot win a regular circle test. Existing scan/hashed paths
remain for circles the eater grid cannot index and arrays exceeding handle range.

## Alternatives and narrow diagnostics

Two stock-only pilots used the same #207 Rust sources, one untallied warmup and
five alternating whole-process instruction pairs each, with complete raw stock
trace equality. Counts include setup and all maintenance; no pair excluded.

| Prototype | Base / candidate median, B instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stable-handle pellet index | 17.596 / 14.887 | 15.398% (15.391–15.407%) |
| Cached packed pellet coordinates, reverse scan | 17.595 / 15.528 | 11.744% (11.737–11.759%) |

The smaller alternative avoids repeated float-to-cell conversion and reads a
compact array, but still walks every pellet. The index's roughly 3.66 percentage
point advantage justified its maintenance and larger state. Pilot sources,
binaries and arrays remain local-only; the final implementation was measured
again after extracting the hit helper and fixing lint issues.

Separate current-thread user-instruction brackets use the same 853 steady stock
ticks as #207: tick >=120, all 500 alive, forced reconnect tick720 excluded.
Means divide by all 853 ticks, including ticks without removals. Hooks affect
compilation and include overhead; do not subtract these budgets to predict the
unprofiled whole-process savings.

| Pickup phase | Base / candidate, M instructions per steady tick |
| --- | ---: |
| Eater circles | 0.115 / 0.115 |
| Eater grid | 0.202 / 0.202 |
| Pellet-index access / mirror verification | 0.000 / 0.387 |
| Lookup, exact tests, hit collection/sort | 3.103 / 0.193 |
| Events and mass gain | 0.002 / 0.002 |
| Removal mask, cache counts, handle compaction/remap | 0.003 / 0.398 |
| Physical-food stable retain | 0.585 / 0.568 |
| **Total pickup** | **4.010 / 1.865** |

## Selected instructions and full output parity

WSL2 Linux x86-64, Rust 1.99.0 locked release/debug=1, thin LTO, one codegen unit.
Cargo/Rayon/tests one job; at most two logical CPUs, counted children pinned to
one. One untallied warmup pair, five alternating whole-process `instructions:u`
pairs per protocol case, all counters 100% scheduled. Executable copies rehashed
before/after. Includes construction, all 1,080 ticks, clients and drains; excludes
sockets/kernel. No counted pair excluded.

| Official 500 | Base / candidate median, B instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 17.596 / 14.856 | 15.571% (15.564–15.584%) |
| Half protocol 13 | 18.138 / 15.388 | 15.165% (15.126–15.182%) |
| All protocol 13 | 18.658 / 15.906 | 14.747% (14.739–14.752%) |

One separate instrumented run per arm over the same 853 steady stock ticks
measures simulation-thread `advance_tick_with` means of 15.455M -> 12.848M user
instructions (16.868% lower). This excludes fixture input/drains and does not
replace the five unprofiled whole-process pairs.

All twelve full traces are byte-identical: three official500 protocol variants
and the nine historical portable/protocol, official500, Battledome64, retained
step_all/next_frame and separate Battledome40/64/80 cases. Direct decompressed
stream comparison covers frames/order/timestamps, checkpoints and RNG endpoints;
the three protocols also preserve the same physical checkpoints.

These are fixed synthetic fixtures. WSL2 timing is contaminated: no CPU-time,
latency, capacity or native EPYC transfer claim.

## Validation and reproduction

Full Linux Rust group passes, including formatting/Clippy configurations,
default/core-only/bot/networking tests, doctests, C ABI and Python crate build.
The 54 focused food tests pass. The cache oracle checks 8,000 mirrored operations
with external edits, compaction, handle reuse and cloning; the eater-cell oracle
checks 120,000 probes plus extreme/ULP/non-finite boundaries. Public edit/restore
tests compare complete checkpoints and pickup events against scanning.

Omitting post-compaction handle-position refresh fails the cache oracle; omitting
hit sorting fails the reverse-order/nearest-head/lexical-tie regression. Both
mutation controls are retained. The corrected allocation gate and its positive
control pass 200 repeated executions. Documentation checks pass; Windows/macOS
and the remaining CI matrix await PR checks.

The existing empty-world allocation gate once counted two unrelated libtest-main
thread allocations (48/96-byte channel TLS/waker state), proven with native
allocation stacks and different controller/harness thread IDs. Its process-wide
probe could race test startup. The probe now counts the simulation thread only,
keeps the zero-allocation assertion and has a positive allocation control.
Initial failures, baseline/candidate controls and diagnostic stacks are retained.

Run [reproduce.py](reproduce.py) modes `build`, `trace`, then `count`, with
`REPOSITORY OUTPUT --cpu CPU`; count also needs `--perf PATH`. It reuses #204's
pinned shared fixtures. Optional `PICKUP_TARGET_ROOT` reuses task-owned Cargo
caches; binaries are still copied and rehashed. Python optimization is rejected
by the wrapper and historical child, preserving assertion-based checks.

Raw traces, counter arrays, binaries, diagnostic overlays, rejected coordinate
cache and lint/probe failures stay outside Git. Raw evidence is local-only, not
publicly retrievable through this record. Digests identify retained artifacts;
they do not establish capacity or scientific rules.
