# Subscription cleanup skip, frozen before measurement

Base: `23e987f37b061ac80307404c24f6b7fecc71a770` (main after #260).
Candidate: `7a384183d0816ed903402e4b4062f410d584b0f2`.

Track the exact minimum stored timestamp outside each recipient's current
stencil. At a cleanup epoch, skip the retain sweep only if no non-current
sector can expire under the original subtraction and strict comparison.
Preserve previous-stencil rewrites, membership revisions, reconnect resets,
packet bytes, ordering, boundaries and timing.

Hypothesis: this removes enough of the previous out-of-class explanatory
[subscription diagnostic](../2026-10-29-food-diffs/REPORT.md#remaining-measured-work)
(1.000 CPU ms per maintenance tick, including processing flags) to reduce
maintenance work. That diagnostic ranks the lead; it is not a qualified baseline.

Reuse the [food-diff fixed-work rows and fork workflow](../2026-10-29-food-diffs/REPRODUCE.md)
unchanged: short stock (0 warm, 1,080 advances), mature stock and protocol 13
(193,000 warm, 360 advances). Warm each full backend once per arm/protocol
without sockets or wall pacing, then fork for every comparison. The validated
physics checkpoint omits maintenance-path recipient and food-index state.
Compare every warm byte and complete continuation traces. Use one discarded
AB warmup and five AB/BA/AB/BA/AB pairs, separating actual food-epoch transitions
from other ticks. Record per-advance user instructions and process CPU, including
the identical clock/PMU wrappers. Report mean and p99 timing with variation.

Use a 4-vCPU Zen 5 EPYC KVM guest, release/native CPU/thin LTO/one codegen unit,
locked offline dependencies and one bounded target. Finish builds/checks before
timing; keep the guest otherwise idle and record redacted full process commands
at measurement boundaries. No new live capacity confirmation is planned.

Require all 27 deterministic catalog/host traces and both stock/protocol-13
wire mutation controls. Run the frozen full-sweep oracle over 48,000 synthetic
random-walk advances plus directed re-entry/reset/large-jump/rounding cases.
Wrong outgoing timestamps, missing re-entry refresh and rounded future deadlines
must each fail the oracle. Run the complete offline Rust group and docs checks.

Retain failed attempts and raw artifacts outside Git; only cited aggregates and
what recomputes the tables belong here. Link earlier harnesses instead of copying
them. No production patch file, socket optimization, outside server or merge.
