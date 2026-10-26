# Sparse food delivery, 2026-10-26

The fixed official-500 fixture retires **6.44% fewer user instructions**.
Live CPU improvement and native single-core capacity remain unqualified.
Baseline `3a23dc9e130b2ff84100faf71645f655242e6ed4`; runtime
`e8e55d101302a1586b69eaf37113c9503bcb4fd8`. See [plan](PLAN.md) and
[selected results/identities](results.json). Raw material is local-only.

Share changed listing/pickup sectors per broadcast. Stable recipients inspect
sparse changes; dense changes retain a linear scan. Membership/snapshot identities
force skipped recipients through the full diff. Encoders, order, mechanics,
admission and simulation/broadcast thread ownership stay unchanged.

## Method and results

Shared WSL2 Linux x86-64; Rust 1.99.0, locked release, thin LTO, one codegen
unit, debug 1; independent targets and copied binaries rehashed after measurement.
At most two logical CPUs: all host threads on CPU 0, one generator on CPU 2;
build jobs, Rayon and test threads one; shared desktop.
Synthetic seed-7 loopback only, no bots or outside server/cloud access.

Primary: `instructions:u`, one discarded then five alternating pairs,
100 decisions including construction/internal warmup, >=99% scheduled.
These synthetic fixtures include client work, exclude sockets/kernel work and
differ from socket admission/density.

| Fixture | Median instructions, billions main → candidate | Median paired reduction (range) |
| --- | ---: | ---: |
| official-500 | 22.359 → 20.918 | 6.444% (6.436–6.458%) |
| Battledome-40 | 36.499 → 36.103 | 1.086% (1.083–1.089%) |
| Battledome-64 | 80.690 → 80.096 | 0.736% (0.735–0.740%) |
| Battledome-80 | 109.204 → 108.482 | 0.662% (0.658–0.662%) |

Profiled socket holds: 15 s warmup + 45 s hold per arm/workload;
`cpu-clock:u`, 199 Hz, DWARF/16384. These are **unpaired profile context, not
CPU speed comparisons**. CPU is host/probe user+system divided by executed ticks
(6,428–6,429); `/proc` has 10 ms quantization and polling-boundary error.

| Workload | CPU ms/tick main → candidate | Alive mean / waiting mean main → candidate | Excess p50/p99 ms main → candidate |
| --- | ---: | ---: | ---: |
| official-500 | 6.285 → 5.813 | 499.97/0 → 500/0 | 7.91/200.84 → 4.32/51.53 |
| Battledome-40 | 2.845 → 2.522 | 35.69/3.96 → 34.16/5.49 | 3.88/11.37 → 2.23/11.32 |
| Battledome-64 | 3.007 → 2.696 | 34.56/29.07 → 32.30/31.38 | 4.03/13.05 → 3.63/10.12 |
| Battledome-80 | 3.049 → 2.831 | 33.86/45.81 → 33.51/46.19 | 3.63/11.85 → 3.82/12.47 |

All holds: zero deferred ticks/errors. Official final alive: 500 both arms;
Battledome final alive/waiting: 33/7 → 32/8, 31/33 → 34/30, 37/42 → 36/43.
Battledome steers/boosts without background bots. Completed wait p50/p99/max,
censored ages and phase shares are retained per seat count in JSON. At 80,
completed waits reach 45.87 → 51.15 s; unfinished ages reach 29.44 → 34.21 s.
Waits measure login-send to setup-receipt; unfinished intervals include warmup
and are lower bounds. Excess pools receipt minus cumulative header elapsed,
normalized by each connection's hold minimum; it includes polling/decoding and
the 1 ms header grid, not absolute latency or admission delay.

Fresh official main → candidate sampled user shares: encoding 36.19 → 33.76%,
visibility/food 29.90 → 29.15%, physics 15.99 → 17.55%, framing 7.60 → 9.44%,
socket service 5.50 → 5.00%, allocation 3.68 → 3.64%, other 1.14 → 1.47%.
Inclusive `food_updates`: 9.44 → 5.03%; actual `sector_changes` only
0.35 → 0.42%, motivating scan removal over shared encoding. Food sync:
6.02 → 7.24%. Categories exclude kernel work; inlining/unwinding limit attribution.
Battledome physics includes admission and occupies about 89% of user samples.

The target is under 4% of process CPU (2.617 user / 6.285 total ms/tick),
below earlier WSL spread; no extended live pairs were run. Single-hold deltas
cannot resolve a gain: populations, inputs and ecology differ, and the official
generator is 83–88% busy. No real-time/capacity claim follows.

## Validation and reproduction

Full Linux Rust group passed. Windows: two food tests, eleven visibility tests,
interrupted-broadcast regression and all fifteen socket-host tests passed.
Nine complete decompressed traces matched byte-for-byte: portable/protocol 32
operations, official-500 one decision after warmup, Battledome-40/64/80 four,
retained step_all/next_frame 384 including reconnect/decoder replay; 64 repeats
in the extended catalog. JSON retains byte sizes/digests. Finite traces exclude
live receipt timing and some private queues/RNG. macOS was not run locally.

Review found membership-only hints missed interrupted broadcasts. The recovery
test fails at `5f3a467`, passes with snapshot continuity; both streams have oracle
coverage. Independent source recheck found no defects; reviewer ran no checks.
The rejected prototype has no measurements; its witness stays in scratch.

Adapt prior [reproduction commands](../2026-10-26-host-prey-visibility/REPRODUCE.md):
use the pins above, extract prior scripts/patches from `3a23dc9`, then apply
[harness.patch](harness.patch) with `patch -p1` in the harness directory.
Set jobs/Rayon/test threads **1**, outer masks **0,2**. Build/copy/hash the same
six examples in separate targets; run both trace commands and `instructions.py`.
Omit `paired.py`; run both profile arms for all four workloads with baseline
generators. Capacity `analyze.py`/`profile.py` reduce excess/demangled stacks.
Keep rerun raw outputs outside Git.
