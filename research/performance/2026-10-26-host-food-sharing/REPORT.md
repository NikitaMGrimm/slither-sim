# Sparse food delivery, 2026-10-26

**6.53% fewer user instructions** in the fixed official-500 fixture. Live CPU
is unresolved. Baseline `48977ec`, runtime `1285e69`; full pins, arm medians/
ranges and every paired ratio are in [results](results.json). Raw data stay local.
[Protocol history](PLAN.md).

Initial main `3a23dc9` user shares were encoding 36.19%, visibility/food 29.90%,
food_updates 9.44%, actual sector_changes 0.35%. Share changed listing/pickup
sectors per broadcast instead of full recipient scans; retain the dense walk.
Membership/snapshot identities catch skipped recipients. Decoration-only pit
changes also supply hints. Encoders, order, rules, admission and thread ownership
stay unchanged against the matched baseline.

## Measurements

Shared WSL2 Linux x86-64; Rust 1.99.0, locked release, thin LTO, one codegen
unit, debug 1; independent targets/copied binaries rehashed after runs.
At most two logical CPUs: host threads CPU 0, one generator CPU 2;
build jobs/Rayon/test threads one. Seed-7 loopback only, no background bots/cloud/outside server.

instructions:u: one discarded then five alternating pairs, 100 decisions including
construction/internal warmup, >=99% scheduled. Fixtures include client work,
exclude sockets/kernel work. Battledome uses the current growing preset and one
explicit measured reconnect, with a frame refresh before controls. Stock and
fixed-arena attempts failed the short admission gate; no timed pairs ran from
them. The controlled overlay retains that gate and records its operation.

| Fixture | Median instructions, billions main → candidate | Paired reduction (range) |
| --- | ---: | ---: |
| official-500 | 22.428 → 20.964 | 6.527% (6.525–6.530%) |
| battledome-40 | 16.020 → 15.584 | 2.717% (2.716–2.734%) |
| battledome-64 | 13.559 → 12.862 | 5.142% (5.129–5.165%) |
| battledome-80 | 15.607 → 14.719 | 5.690% (5.673–5.695%) |

**All live holds below are contaminated** by reported concurrent local host/
browser activity from 22:20 UTC. They are unpaired profile context, excluded
from speed claims: 15 s warmup + 45 s hold, cpu-clock:u 199 Hz, DWARF/16384.
Host/probe user+system CPU/executed tick has 10 ms /proc quantization and
polling-boundary error.

| Workload | CPU ms/tick main → candidate | Mean alive/waiting main → candidate | Excess p50/p99 ms main → candidate | Deferred main/candidate |
| --- | ---: | ---: | ---: | ---: |
| official-500 | 6.032 → 5.969 | 500.00/0.00 → 499.96/0.00 | 5.25/60.41 → 179.33/371.67 | 0/25 |
| dome-40 | 1.091 → 1.007 | 39.93/0.00 → 39.92/0.00 | 1.29/8.15 → 1.21/6.31 | 0/0 |
| dome-64 | 2.526 → 2.590 | 64.00/0.00 → 64.00/0.00 | 1.39/30.43 → 1.45/105.84 | 0/0 |
| dome-80 | 4.553 → 4.585 | 79.99/0.00 → 79.98/0.00 | 7.80/449.27 → 9.15/618.32 | 48/56 |

All final populations equal requested seats; final waiters/errors zero.
Waits: login-send to setup-receipt.
JSON retains wait count/p50/p99/max: first setup includes warmup, completed
hold waits and censored ages stay separate. Censored ages are lower bounds.
Excess is receipt minus cumulative header elapsed, normalized by each
connection's hold minimum; it includes polling/decoding and the 1 ms header grid.

Official sampled user shares main→candidate: encoding 33.18→34.54%, visibility/food 32.83→29.53%, physics 16.16→16.12%, framing 7.25→8.86%, sockets 5.19→5.18%, allocation 3.75→4.28%, other 1.64→1.49%.
Food_updates 9.72→5.31%. Shares exclude kernel work and have inline/unwind limits.
At 80 seats, visibility/food is 62.54→64.21%; physics includes admission.
Growing-arena holds see few peers (JSON), not the original dense fixed-3,584
target. No live speed or capacity improvement is established.

## Checks and reproduction

Full Linux Rust group passed. Windows: 5 food, 11 visibility, interruption/
shrinking-pit regressions and 15 socket-host tests passed; macOS unrun locally.
Both overlaid examples pass 16 tests. Nine full decompressed comparisons match:
portable/protocol 32 operations, official-500 one decision after warmup,
Battledome-40/64/80 four, retained step_all/next_frame 384 with reconnect/decoder
replay; 64 repeats in the extended catalog. Two debug pit traces additionally
match every payload vector through growth and stable-membership shrink.
Finite traces exclude live receipt timing and some private queues/RNG.

Negative controls fail without snapshot continuity or decorated hints; final
source passes. Independent source checks of runtime/harness found no remaining
defects; reviewers executed no checks. Earlier series: `e8074c0`; raw pairs stay
local. No completed pair was discarded.

Adapt [prior commands](../2026-10-26-host-prey-visibility/REPRODUCE.md) with these
pins and prior inputs from 3a23dc9. Set jobs/Rayon/tests 1, outer masks 0,2.
Apply [harness.patch](harness.patch) in the extracted harness with GNU patch -p1;
apply [client-reconnect.patch](client-reconnect.patch) to both source roots
before generating perf_battledome.rs. Build/copy/hash the same six examples;
run both trace commands, instructions.py, and both profile arms for all four
workloads. Omit paired.py. Capacity analyze.py/profile.py reduce excess/stacks.

For pit checks, apply [pit-trace.patch](pit-trace.patch) to separate snapshots;
run each pit test named in results with cargo test --lib NAME -- --nocapture.
After success, run [pit_payloads.py](pit_payloads.py) NAME < stdout > vectors;
compare complete vectors with cmp. Results contain their digests/terminal counts.
Keep all raw rerun outputs outside Git.
