# Monotone bounds-index cells, 2026-10-27

Keep truncating cells. Main `6769d71` (merged #216) -> measured runtime `252587a`.
[Selected results and identities](results.json). The delivery head formats
test-only imports, integrates #217's research-only record and adds this study;
production functions are unchanged after measurement. Production changes stay
in `bounds_index.rs`.

## Fresh core profile

Separate current-thread `instructions:u` brackets use the official-500 stock
fixture and the same 853 steady ticks: tick >=120, all 500 alive, excluding forced
reconnect tick720. Physics spans validated advance through final score updates;
host input/broadcast work is outside this scope. Hooks affect compilation and
include overhead, so budgets are diagnostic, not predictions to subtract from
unprofiled process counts.

| Core phase | Base / candidate, M instructions per steady tick |
| --- | ---: |
| Setup/radius/buffers | 0.101 / 0.101 |
| Movement/body/boost | 0.468 / 0.468 |
| Collision records | 0.137 / 0.137 |
| Collision ordering/index construction | 0.245 / 0.188 |
| Collision candidates/exact tests | 0.293 / 0.243 |
| Death resolution/drops | <0.001 / <0.001 |
| Pickup/cache | 0.771 / 0.771 |
| Prey | 0.462 / 0.396 |
| Ambient supply | 0.004 / 0.004 |
| Final scores/buffer commit | 0.027 / 0.027 |
| **Total physics** | **2.508 / 2.335** |

Collision totals **0.676M -> 0.568M**, the largest requested phase before the
change. A separate nested baseline probe identifies **0.333M** in two shared
`BoundsIndex` builds and **0.244M** in about 571 candidate queries per tick, across
collision and prey. Those brackets overlap the table and add per-call overhead;
do not add them to the clean phase budgets. No deaths occur in the selected
window, so it does not characterize death-drop bursts.

## Conservative mapping and gameplay order

The bounds index only needs a monotone cell mapping. For finite `x <= y`,
truncating/saturating `(x / CELL) as i64 <= (y / CELL) as i64`. Overlapping
coordinate ranges therefore share at least one mapped cell. Remove `floor()`
from cell conversion, avoiding its baseline x86-64 library work. Negative cell
numbering changes and the cell around zero is wider; false positives remain
subject to the existing exact tests. Non-finite items remain always-tested and
unusable queries retain the existing exhaustive fallback.

The exact `Bounds::near`, `strikes_scaled`/`strikes` and prey predicates stay
unchanged. Candidate positions still sort/deduplicate in descending order. The
simulator's offline-client-derived oldest-first/newer-target-first rationale
stays beside its resolution loop. Fallen-snake skipping, boundary/death order,
kill attribution, physical arithmetic, RNG and checkpoint format are unchanged.
The canonical evidence limit remains [server rules](../../../docs/server-rules.md#movement-and-geometry).

## Candidate-ordering alternatives

Stock-only pilots use the same baseline executable, complete raw stock-trace
equality, one untallied warmup and five alternating process pairs each.

| Implementation | Base / candidate median, B instructions | Paired change (range) |
| --- | ---: | ---: |
| Truncating cells (selected) | 12.276 / 12.086 | 1.543% reduction (1.536–1.558%) |
| Bitset candidate ordering | 12.275 / 12.302 | 0.207% increase (0.196–0.236%) |
| Truncation plus bitset | 12.276 / 12.114 | 1.324% reduction (1.281–1.332%) |

The bitset prototype replaces sorting/deduplication with a 512-bit stack mask for
small indexes and emits positions newest first; larger indexes keep the existing
sort. It adds work on this fixture. Keep the single mapping change. Alternate
sources, binaries and arrays remain local-only; the full matrix below covers
the selected runtime.

## Final process pairs and full traces

WSL2 Linux x86-64, Rust 1.99.0 locked release/debug=1, thin LTO, one codegen unit.
At most two logical CPUs; counted children pinned to one, Cargo/Rayon/tests one
job. One untallied warmup pair, five alternating whole-process `instructions:u`
pairs per protocol, all **100% scheduled**. Immutable executable copies rehashed
before/after. Counts include construction, all 1,080 ticks, clients and drains;
exclude sockets/kernel. No pair excluded.

| Official 500 | Base / candidate median, B instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 12.276 / 12.086 | 1.543% (1.536–1.558%) |
| Half protocol 13 | 12.626 / 12.430 | 1.551% (1.525–1.582%) |
| All protocol 13 | 12.963 / 12.774 | 1.455% (1.438–1.477%) |

All twelve complete raw traces are byte-identical: three official500 protocol
variants and nine historical portable/protocol, official500, Battledome64,
retained step_all/next_frame and separate Battledome40/64/80 cases. Direct
decompressed comparisons cover frames/order/timestamps, checkpoints and RNG
endpoints. The protocol variants also preserve identical physical checkpoints.

These fixed synthetic fixtures establish no CPU-time, latency, capacity or native
EPYC transfer claim. Shared-machine WSL2 timing is contaminated.

## Validation and reproduction

The broad-phase oracle compares filtered ordered candidates with exhaustive
`Bounds::near` across cell/ULP, signed-zero, huge/saturating, non-finite and
always-tested geometry. The death oracle compares indexed resolution against
the existing exact `strikes` test on randomized geometry/join orders and boundary
cases, independently checking both broad phases. It compares ordered victims,
killers and kill counts. A fixed witness checks that the newest eligible killer
keeps credit even when its boundary death resolves later. Omitting the last query
cell fails the randomized/boundary death oracle; its mutation source/log remain
retained. All four bounds-index tests, 19 physics integration tests, focused
kill-count/kill-score tests and the full Linux Rust group pass. Remaining
platforms await PR CI; documentation checks are recorded with selected evidence.

Run [reproduce.py](reproduce.py) modes `build`, `trace`, `count` with
`REPOSITORY OUTPUT --cpu CPU`; count also needs `--perf PATH`. The wrapper reuses
#204's pinned shared fixture/historical harness. `PICKUP_TARGET_ROOT` optionally
reuses task-owned Cargo caches; executables are still copied and rehashed.

For diagnostics, run [profile.py](profile.py) `REF PROFILE_DIRECTORY` from the
repository root. Build its archived `study_protocols` example in locked
release/debug=1 with one job and CPU affinity; copy it to
`PROFILE_DIRECTORY/binary`, then run pinned with
`PICKUP_PROFILE=PROFILE_DIRECTORY/phases.json`. Run
[select_profile.py](select_profile.py) `PROFILE_DIRECTORY` to select the window.
`--nested` on preparation adds the shared-index brackets. The portable generator
reproduces the baseline overlay byte for byte. All Python entry points, including
the historical child, reject optimization.

Raw arrays, traces, prototypes, binaries, diagnostic overlays and failed checks
stay outside Git and are local-only. Retained validation attempts include an
import-format mismatch, a zero-test filter and an invalid common-HEAD archive;
none contributes selected pairs or final checks. The final validation archive is
bound to explicit `ef8b6c4`, whose production code matches measured `252587a`.
Digests establish consistency, not product capacity or scientific rules.
