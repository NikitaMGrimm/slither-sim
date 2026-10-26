# Selected protocol, 2026-10-26

The complete pre-measurement [frozen plan and amendments](https://github.com/NikitaMGrimm/slither-sim/blob/78d9ef824d7e6f6b66d1375aa5170b1d11fc9f88/research/performance/2026-10-26-host-food-sharing/PLAN.md)
remain at that immutable commit. This publication summary avoids duplicating it.

Initial baseline 3a23dc9; final baseline 48977ec, runtime 1285e69 after #176.
Keep each series separate. Preserve completed pairs, failures and full outputs
locally. Final primary evidence: instructions:u, one discarded plus five
alternating pairs, 100 decisions including construction/warmup, >=99% scheduled.
Four fixtures: official-500 and Battledome-40/64/80. Byte parity precedes timing.

Two CPUs maximum: host CPU 0, one generator CPU 2; jobs/Rayon/test threads one.
Independent release targets/copied binaries. No spawn/rule changes or outside
game traffic. Stock and fixed-arena short windows lacked admission; the final
shared overlay reconnects slot zero once, refreshes its frame, retains the gate
and labels the operation. Its stale-frame regression must pass first.

Profile holds: 15 s warmup + 45 s hold, user cpu-clock 199 Hz, DWARF/16384.
Reported concurrent local host/browser activity from 22:20 UTC contaminates
all final live holds; exclude them from speed claims. After the primary series,
two untimed pit payload checks cover growth and stable-membership shrink.
