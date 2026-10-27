# Ordered physical-food compaction, 2026-10-27

Keep. Main `32d4cbc` (merged #212) -> measured runtime `30040c8`.
[Selected results, source and executable identities](results.json). The delivery
head adds this record and reproducers. Production changes stay in `simulator.rs`.

## Pickup/cache breakdown

Separate current-thread `instructions:u` brackets use the official-500 stock
fixture and the same 853 steady ticks: tick >=120, all 500 alive, excluding
forced reconnect tick720. Means include ticks without pickups/removals. Brackets
affect compilation and include overhead; these diagnostic budgets are not
predictions to subtract from unprofiled process counts.

| Pickup phase | Base / candidate, M instructions per steady tick |
| --- | ---: |
| Eater-circle construction | 0.114 / 0.115 |
| Eater grid and fallback setup | 0.202 / 0.200 |
| Pellet lookup, exact tests, hit collection/sort | 0.192 / 0.197 |
| Event recording and mass application | 0.002 / 0.002 |
| Removal setup | <0.001 / <0.001 |
| Cache removal excluding slot refresh | 0.005 / 0.005 |
| Full `food_slots` position refresh | 0.394 / 0.394 |
| Physical food compaction | 0.585 / 0.085 |
| **Total pickup/cache** | **1.495 / 0.998** |

Nested brackets across the entire tick measure cache pushes at **0.0067M**,
including **0.0025M** in pellet-index insertion; index removal is **0.0011M**.
Admission `has_room` is **0.0026M**, mirror synchronization is zero, and ambient
supply totals **0.0054M**. Hit sorting is **0.0003M**, included in lookup above.
These nested costs overlap the enclosing phases and must not be added again.
The selected window has no deaths and does not characterize death-drop bursts
or cold occupancy/index construction.

A separate intrusive exact-helper diagnostic brackets 1,636 `find_eater` calls
per tick. Its matched empty-probe control measures 0.047M against the helper's
0.119M, giving an approximately **0.072M** body estimate. Instrumentation changes
inlining and adds substantial per-call work; this estimate is not an exact
additive decomposition of the clean 0.192M lookup budget. The primary table
omits that probe. The initial uncalibrated probe and all raw runs are retained.

## Selected change

Physical food retention was the largest operation. Pickup already supplies
unique original pellet indices in descending order. After events and mass gains
have been applied, reverse those indices and compact survivor ranges in ascending
order, keeping the original order and exact field bits. Skip the unchanged
prefix; remove the full-size boolean removal mask.

Safe slice rotations move each survivor range over its preceding removed gap.
When a gap exceeds its survivor range, swap only survivors instead: repeatedly
rotating large gaps for tiny ranges could become quadratic. Thus rotated ranges
are at most twice the survivor span, and total work stays linear. Truncate the
removed suffix after all original indices have been consumed.

The food mirror and pellet index still use their existing removal path. Collision
order, death resolution, pickup selection/ties, event order, arithmetic, RNG and
checkpoint format are unchanged. The slot refresh remains the largest individual
pickup operation after this change; its cost is included in the measurements.

## Paired process instructions and complete trace parity

WSL2 Linux x86-64, Rust 1.99.0 locked release/debug=1, thin LTO, one codegen unit.
At most two logical CPUs; counted children pinned to one, Cargo/Rayon/tests one
job. One untallied warmup pair, then five alternating whole-process
`instructions:u` pairs per protocol, all counters **100% scheduled**. Executables
are immutable copies, rehashed before/after. Counts include construction, all
1,080 ticks, clients and drains, excluding sockets/kernel. No pair excluded.

| Official 500 | Base / candidate median, B instructions | Paired reduction (range) |
| --- | ---: | ---: |
| Stock | 13.024 / 12.452 | 4.394% (4.360–4.397%) |
| Half protocol 13 | 13.554 / 12.976 | 4.263% (4.253–4.282%) |
| All protocol 13 | 14.073 / 13.498 | 4.087% (4.038–4.114%) |

All twelve complete raw traces are byte-identical: three official500 protocol
variants and nine historical portable/protocol, official500, Battledome64,
retained step_all/next_frame and separate Battledome40/64/80 cases. Decompressed
stream comparisons cover frames/order/timestamps, checkpoints and RNG endpoints.
The protocol variants also preserve identical physical checkpoints.

These fixed synthetic fixtures establish no CPU-time, latency, capacity or native
EPYC transfer claim. Shared-machine WSL2 timing is contaminated.

## Validation and reproduction

Full Linux Rust group passes: all lint configurations, default/core/bot/networking
tests, doctests, C ABI and Python crate build. All 67 food-focused unit tests and
both pickup integration tests pass. The new oracle compares every removal subset
through length12 against stable retention, plus long sparse/dense, prefix/suffix,
empty/all-removed cases. Comparisons use exact field bits, including signed zero
and non-finite coordinates. Existing reverse-order/contested-pellet and cache
reset/restore oracles remain in the suite. Changing the helper's rotation from
left to right fails the new oracle; the mutation source and log are retained.
Documentation checks pass; the remaining platform matrix awaits PR CI.

Run [reproduce.py](reproduce.py) modes `build`, `trace`, `count` with
`REPOSITORY OUTPUT --cpu CPU`; count also needs `--perf PATH`. It reuses #204's
pinned shared fixture/historical harness. Optional `PICKUP_TARGET_ROOT` reuses
task-owned Cargo caches; binaries are still copied and rehashed.

For the diagnostic, run [profile.py](profile.py) `REF PROFILE_DIRECTORY` from the
repository root, then build its `source` archive's `study_protocols` release
example with `CARGO_PROFILE_RELEASE_DEBUG=1`, one job and CPU affinity. Copy the
executable to `PROFILE_DIRECTORY/binary` and run it pinned to one CPU with
`PICKUP_PROFILE=PROFILE_DIRECTORY/phases.json`. Run
[select_profile.py](select_profile.py) `PROFILE_DIRECTORY` to select the window.
`--exact` on profile preparation adds the helper/empty-probe diagnostic. All
Python study entry points reject optimization; the historical child does too.

Raw arrays, full traces, binaries, diagnostic overlays and untallied preparation
stay outside Git and are local-only. Digests establish consistency, not product
capacity or scientific rules.
