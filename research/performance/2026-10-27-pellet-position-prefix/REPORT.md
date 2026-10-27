# Skip unchanged pellet positions, 2026-10-27

Keep prefix skipping. Main `40ce9fa` (merged #213) -> measured runtime `b3ad144`.
[Selected results and identities](results.json). The delivery head integrates
#214's documentation and adds this record; all Rust/check-runner inputs remain
identical to the measured/tested runtime. Production changes stay in `food_grid.rs`.

## Profile and invariant

Separate current-thread `instructions:u` brackets use the same 853 steady stock
ticks: tick >=120, all 500 alive, forced reconnect tick720 excluded. Means include
ticks without removals. Brackets affect compilation and include overhead; these
budgets are diagnostic, not predictions to subtract from process counts.

| Nested core operation | Base / candidate, M instructions per steady tick |
| --- | ---: |
| Position refresh | 0.394 / 0.174 |
| Cache removal, including refresh | 0.398 / 0.178 |
| Pellet lookup, exact tests and sorting | 0.193 / 0.193 |
| Pellet-index insertion | 0.0025 / 0.0025 |
| **Whole `Simulator::advance`** | **2.736 / 2.516** |

Rows overlap; do not add them. Physics spans action-validated advance through
final score updates, excluding host broadcast/input work. The window has no
deaths and does not characterize death-drop bursts or cold index rebuilds.

Across removal ticks, **64.7% of surviving handles precede the first removed
index**. The old refresh visited an average 49,246 survivors per steady tick;
31,850 belong to that unchanged prefix. Track the least removed original index
and begin refresh there. Stable compaction preserves both handles and food
positions before it. Empty removal lists skip the entire refresh; a removed tail
needs no position writes. The existing cell-list offset repair still runs for
each removed handle, including when a swapped list member belongs to the prefix.

No selection, nearest-head/lexical-ID tie, pellet order, exact predicate, event,
mass arithmetic, death resolution, RNG or checkpoint-format change. The index
keeps its existing allocation/reuse and public reset/restore behavior.

## Alternatives

Stock-only pilots use one untallied warmup and five alternating process pairs
each, with complete raw stock trace equality and the same baseline executable.

| Implementation | Base / candidate median, B instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Prefix refresh (selected) | 12.451 / 12.259 | 1.547% (1.543–1.547%) |
| Fused handle move/position update | 12.451 / 12.370 | 0.647% (0.638–0.677%) |
| Append-ordered handles, implicit positions | 12.453 / 12.287 | 1.328% (1.318–1.354%) |

The fused prototype updates each moved handle while copying it to its new dense
array location, replacing bulk handle copies with a scalar pass. The implicit
prototype removes stored positions and the free list: monotonically appended
IDs stay sorted in the live-handle array, and cell queries recover food positions
by binary search. It discards the lazy index when retained slot storage reaches
twice live food (minimum threshold 4096), bounding tombstone growth.

Both avoid the separate full refresh, but cost more process instructions than
prefix skipping. Keep the smaller change. The alternate sources, binaries and
arrays remain local-only; the full matrix below covers the selected runtime.

## Final instruction pairs and full output parity

WSL2 Linux x86-64, Rust 1.99.0 locked release/debug=1, thin LTO, one codegen unit.
At most two logical CPUs; counted children pinned to one, Cargo/Rayon/tests one
job. One untallied warmup pair, then five alternating whole-process
`instructions:u` pairs per protocol, all **100% scheduled**. Immutable executable
copies rehashed before/after. Counts include construction, all 1,080 ticks,
clients and drains, excluding sockets/kernel. No pair excluded.

| Official 500 | Base / candidate median, B instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 12.451 / 12.259 | 1.547% (1.543–1.547%) |
| Half protocol 13 | 12.975 / 12.783 | 1.487% (1.443–1.497%) |
| All protocol 13 | 13.496 / 13.302 | 1.435% (1.369–1.455%) |

All twelve complete raw traces are byte-identical: three official500 protocol
variants and nine historical portable/protocol, official500, Battledome64,
retained step_all/next_frame and separate Battledome40/64/80 cases. Direct
decompressed comparisons cover frames/order/timestamps, checkpoints and RNG
endpoints. The protocol variants also preserve identical physical checkpoints.

These fixed synthetic fixtures establish no CPU-time, latency, capacity or native
EPYC transfer claim. Shared-machine WSL2 timing is contaminated.

## Validation and reproduction

All 68 food-focused unit tests and both pickup integration tests pass. The new
boundary oracle checks late then early removals, tail/empty/all removal,
consecutive removals, swapped cell-list repair, handle reuse and cloning. Existing
8,000-edit cache and gameplay-order/tie oracles remain unchanged. Advancing the
refresh start by one fails the new oracle at the first moved survivor. The
mutation source and log are retained. Full Linux Rust group passes, including
all lint configurations, default/core/bot/networking tests, doctests, C ABI and
Python crate build. Documentation checks pass; remaining platforms await CI.

Run [reproduce.py](reproduce.py) modes `build`, `trace`, `count` with
`REPOSITORY OUTPUT --cpu CPU`; count also needs `--perf PATH`. The wrapper reuses
#204's pinned shared fixture/historical harness. `PICKUP_TARGET_ROOT` optionally
reuses task-owned Cargo caches; binaries are still copied and rehashed.

For the diagnostic, run [profile.py](profile.py) `REF PROFILE_DIRECTORY` from the
repository root. It reuses the pinned #213 counter source. Build its archived
`study_protocols` example in locked release/debug=1 with one job and CPU affinity;
copy it to `PROFILE_DIRECTORY/binary`, then run it pinned to one CPU with
`PICKUP_PROFILE=PROFILE_DIRECTORY/phases.json`. Run
[select_profile.py](select_profile.py) `PROFILE_DIRECTORY` to select the window.
The portable generator reproduces the selected baseline overlay byte for byte.
All Python entry points, including the historical child, reject optimization.

Raw arrays, full traces, prototype sources, binaries, diagnostic overlays and
compile-only pre-merge preparation stay outside Git and are local-only. Digests
establish artifact consistency, not product capacity or scientific rules.
