# V3 attempts

Before any provisioning/timing, verified official on-demand quotes for the same
c7a.medium/c8a.2xlarge types and sufficient standard on-demand vCPU quota. Frozen
rate caps, 90-minute shutdown timers with a 92-minute launch bound and a $0.40
noncompute reserve bound three possible pairs at $2.976. No Spot request is used.

The source reuses the pinned v2 implementation and corrects the per-worker CPU
input to sum all three thread clocks. Tests/builds will run on the first fresh
AWS generator before warmup, inside the same resource ceiling. No instabox or
local Rust work. Outcome/replacement records will be appended without erasure.

The first fresh on-demand pair launched with tags, same-zone placement, one/
eight vCPUs and verified shutdown deadlines <=92 minutes from launch. Before
any world: portable locked build passed; all three actual-module runtime tests
passed; synchronous dispatch, omitted helper CPU and decision-only gate mutations
each failed as intended. Strict 70%/80%, inherited ladder/gate/token and nearest-
rank controls passed. Seven documentation tests and repository boundary checks
passed. The target was removed. Host binary equals v2 byte-for-byte. These
results precede the plan/harness freeze and tick-zero startup.
