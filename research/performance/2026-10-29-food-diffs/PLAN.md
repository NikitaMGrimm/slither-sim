# Food diff reuse, frozen before measurement

Baseline: main `692cd561714b8af3ea95adb3adcbd807b3a48734` (v0.8.0).
Candidate: reuse a sector's diff within one broadcast when both listing identities
match. Pickups remain fixed for that broadcast; older views use the old diff.
Keep packet timing, ordering, framing and content identical. This does not remove
per-recipient packet-byte copies or change the food-maintenance opportunity.

Hypothesis: repeated recipient listing comparisons account for enough of the
[checkpoint's](https://github.com/NikitaMGrimm/slither-sim/pull/256) 1.634-ms fine
top-1% food cost to reduce maintenance-tick work. That figure has timer overhead
and is separate from the 3.452-ms coarse broadcast parent. Do not add them.

Use a 4-vCPU Zen 5 EPYC KVM guest, one bounded temporary Cargo target, locked
offline dependencies, release/thin-LTO/one-codegen-unit builds with native CPU
targeting. Finish builds/checks before timing and keep other study work stopped.
Retain redacted full process command lines at every measurement boundary.

Reuse the [tick-zero full-host fixture](../2026-10-28-mature-covered-bounds/fixture.rs)
and its [fork/stream workflow](../2026-10-28-mature-covered-bounds/REPRODUCE.md),
with all recipients active through tick 193,000 and no restored checkpoint.
Count 360 complete advances, separating actual food-epoch transitions from
other ticks. Keep the existing mature screen. Retain process CPU ns and user
instructions per tick, with PMU scheduling checks. One discarded AB pair and
five alternating AB/BA/AB/BA/AB pairs, stock and protocol 13. Also retain the
matched 1,080-tick short guard. Report all outcomes, including regressions.

Compare every warm-stream byte live and all measured trace bytes. Run every
available deterministic catalog trace plus stock/mixed/protocol-13 host fixtures.
Require a deliberate wire perturbation to fail comparison in both protocols,
and a previous-identity cache mutation to fail the regression oracle. Run focused
food tests, then the complete offline Rust group and documentation checks.

For a winning fixed-work candidate, collect a bounded real-socket confirmation
using the [checkpoint harness](https://github.com/NikitaMGrimm/slither-sim/blob/research/capacity-checkpoint/research/performance/2026-10-28-capacity-checkpoint/REPRODUCE.md).
To fit approximately one CPU-hour, warm one 500-socket world with the full
received-frame A4 clients, then alternate cache-disabled/enabled windows in that
same evolving mature world. A research-only branch at `append_diff` selects the
original diff or the candidate; both use the same instrumented binary and
per-tick selector. This is descriptive confirmation, not two immutable runtime
binaries or identical live trajectories. Require the checkpoint's client,
queue, body-class and CPU-reconciliation gates; preserve failures. Enforce a
3,600-second host+client+controller CPU ceiling. Do not relax gates to obtain a
timing result. Record maintenance means and aligned simulation+socket world p99.

Keep only cited aggregates and reproduction deltas in Git; full sources, binaries,
inventories, raw samples and failed attempts stay local-only. No production patch
file, outside server, guest configuration/package change, release or merge.
