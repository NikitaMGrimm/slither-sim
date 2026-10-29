# Exact head-to-edge threshold: frozen plan

Baseline `d65d89ca`. Change only the head predicate's existing point-to-edge
`segment_distance <= head_width` comparison to `segment_distance_le`, preserving
its original projection and inclusive scalar result. The original swept-distance
comparisons in body/departure work remain unchanged, including NaN behavior;
`!<=` must not replace `>` there. No grid, candidate, RNG, ordering, admission,
fallback, retry, floor or protocol change.

The preceding AWS profile of the existing synthetic seven-ring overload found
88.6% of sampled work inside the head predicate, 63.5% in its `hypot` calls and
1.5% in `near`. Keep selected profile values and immutable archive pins once,
clearly separate from candidate performance. Full recordings remain local-only.

Build an independent frozen linear head oracle over raw obstacle geometry,
without Safety, Bounds, optimized helpers or Point distance methods in its
reference path. Exhaust all head/edge coordinates on a small signed-zero grid,
then random ordinary/full-bit-pattern inputs, exact distances and adjacent
float thresholds, degeneracy, invalid obstacles, non-finite bounds/points,
underflow/overflow, empty worlds and obstacle reorderings. Independently freeze
the original swept comparison too and check both `<=` and `>` directions.
Focused boundary mutation must fail, restored code must pass. A separate wrong
head-width mutation must change actual queued wire bytes in protocols 13/19.

Run all 23 catalog traces, six historical/extended host traces and both queued
release/admission traces, complete bytes and terminal state/RNG, plus full Rust
checks on AWS only. No local Cargo, matrices or benchmarks. One bounded
Frankfurt Spot builder, explicit `slither-bench`, project tags at creation on
instance/volume/key/group, in-guest planned+30-minute poweroff, terminate behavior
and DeleteOnTermination. The builder cap is $1 including noncompute; its
prelaunch 122-minute bound at $0.20/h plus $0.25 allowance is $0.656667.

Check hardware user instruction access before timing. If qualified on AWS,
label the machine as AWS virtualized EPYC/Spot, keep >=99% PMU scheduling and
run one discarded warmup plus five alternating AB/BA pairs per protocol on the
unchanged exhausted-search fixture and 80-/160-viewer fixed-work controls.
Those controls hold separated synthetic bodies and periodically reconnect one
seat through the actual queue; they test ordinary no-backlog admission and
broadcast overhead, not naturally grown play. Preserve each full parent state,
fork repetition/direct-parent proofs, counts, pose/RNG/complete bytes and all
pair results. Report CPU p50/p99/max, all/search/exhausted/other groups and each
arm's slowest CPU 1% separately. No live capacity claim.

If AWS counters are unavailable, retain that failure, complete proofs on AWS,
retrieve all artifacts and stop spending; wait for the maintaining agent's
explicit SSH release before instruction timing. Do not contact the busy guest.
Never weaken the measurement bar or extend the budget to hide unavailable PMU.

Publish readiness only after archive close, tar/gzip read and hashing. Retrieve,
verify every required member before deleting sources, then terminate/remove
task resources and scan all enabled regions. No identifiers in public material.
Keep evidence lean, link earlier harnesses, use only the assistance footer in
the PR body, open one production PR and do not merge it.
