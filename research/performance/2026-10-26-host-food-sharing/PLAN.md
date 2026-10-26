# Sparse food delivery plan, 2026-10-26

Baseline main `3a23dc9e130b2ff84100faf71645f655242e6ed4`. Synthetic loopback
only, no bots/cloud/outside server. Do not edit spawn/admission. Fresh WSL2
official-500 profile: encoding 36.19%, visibility/food 29.90% of sampled user
CPU; inclusive food_updates 9.44%, sector_changes only 0.35%, food sync 6.02%.
All 500 clients alive at hold end, no errors; generator busy-wall 87.82%.

Target whole repeated scans: stable food subscriptions currently compare every
key, then inspect every held sector. Share the broadcast's changed-sector set;
give subscription membership an identity changed only on actual key changes.
Stable recipients can inspect sparse changes instead of their entire history.
Keep the original dense scan as fallback and all original encoders/order.
Physical and decorated streams must use their own held listings. No persistent
packet cache or mechanics/default changes.

Before measurement, review identified interrupted broadcasts as a counterexample
to membership-only eligibility. Require the recipient's previous food snapshot
identity as well; a missed transition takes the full diff. Cover interruption
and recovery without another sector change, for physical and decorated streams.

At most two logical CPUs: host threads CPU 0, one generator CPU 2. Build jobs,
Rayon and test threads: one. Rust 1.99.0, locked release, thin LTO, one codegen
unit, debug level 1; independently built targets and immutable copied binaries.

Before primary measurement: focused regressions, full Rust group, Windows socket
checks and all nine full decompressed traces from the previous study. Include
official-500, Battledome 40/64/80 and retained reconnect/decoder cases. Preserve
failed attempts locally; do not regenerate expectations or discard completed pairs.

Primary evidence: whole-process instructions:u for the same fixed fixtures,
100 decisions including construction/internal warmup. One discarded pair,
five adjacent alternating measured pairs per workload; require >=99% counter
scheduling. Report arm medians/ranges and every paired ratio, without a capacity
claim. These fixtures include client-side work and differ from socket admission.

Separate before/after profiles cover all four socket workloads: 15 s warmup,
45 s hold, cpu-clock:u at 199 Hz, DWARF/16384. Report populations, waits and
delivery context, excluding profiler runs from speed claims. The targeted
9.44% of user CPU is under 4% of process CPU in the fresh profile (2.62 user /
6.29 total ms per tick). Its maximum expected live gain is below the preceding
WSL variation, so do not extend paired live timing to chase it. Report that
limit explicitly. Kernel work is outside the profile shares.

Keep the complete study <=20 KB: short report and selected results/reproduction
material only. Raw stacks, holds, traces, logs and rejected code stay in scratch.

Integration amendment before new measurements: main advanced through #176 to
`48977ec`, changing the Battledome preset and moving pit-marker listings.
Runtime `1285e69` integrates it and hints decorated-only changes too. Preserve
the original completed series locally and at the historical record `e8074c0`;
repeat checks, all traces, five instruction pairs and socket profiles against
this new baseline. Keep the same budget/method and label the new growing-arena
Battledome workload; do not attribute its admission changes to this optimization.

The refresh baseline fails its existing four-decision Battledome admission gate
before measurements: a growing arena has no admission calls in that window.
Preserve that failure. Freeze only client-fixture arena sizing in a shared
overlay (size_to_population=false) with matching definition metadata; retain
the admission gate. Socket profiles still use the current growing preset.

That fixed-arena attempt also fails the admission gate; reject the overlay and
keep the witness locally. Before any instruction pairs, define a controlled
client fixture: explicitly reconnect slot zero once at the first measured
Battledome decision. Record it in definition metadata and retain the gate.
Both arms use that overlay and the current growing preset. This adds real
admission work rather than relaxing the gate; it does not change runtime rules.
The first forced-reconnect smoke fails on a stale Playing frame. Refresh that
one frame before controls, with a focused regression, before any timed pairs.
