# 500-player acceptance run: maintainer criteria

Frozen 2026-10-30 ~01:15 UTC, before the food checkpoint (#274 follow-up) reported
and before any acceptance timing. The worker copies this into PLAN.md unchanged;
any later change must be labelled as a post-plan amendment.

## Setup

- Runtime: current main at launch, SHA pinned in PLAN.md before timing. Portable
  release build (`-C target-cpu=x86-64`), locked dependencies, official config for
  500 seats with the default adaptive radius.
- Host: AWS c7a.medium, Frankfurt (one vCPU, EPYC 9R14). Every host thread on that vCPU.
- Load: 500 received-frame A4 socket bots, protocol 19 stock framing, the same client
  path and generator gates as the c7a ladder (2026-10-29-c7a-mature-capacity), from a
  separate generator in the same availability zone. No thin clients, TLS or
  artificial latency.
- Maturity: warm from tick zero with natural food and wire history (no checkpoint).
  The world enters the #256 mature class at 500: three consecutive 500-tick endpoints
  inside #256's tolerances (cite them exactly in PLAN.md).
- Window: after 10 s of settling, count 30 minutes with the population held at 500
  (normal deaths and respawns, no forced churn).
- Metric: world CPU per tick = simulation + socket task CPU (user + kernel), aligned
  per tick, over complete 100-tick interior blocks, as in #258. Never add independent
  component quantiles.

## Pass

Two independent runs on fresh instance pairs must both pass all four:

1. Zero deferred ticks in the window.
2. World CPU p99 at most 7.0 ms.
3. The world is still in the mature class at the window's end.
4. The client, queue and generator gates pass.

If run 1 fails, stop and report; run 2 is only for confirmation. A run that
infrastructure invalidates (Spot reclaim, generator gate failure) may be replaced
once and is logged in ATTEMPTS.md. No reruns until a pass, and no tuning between runs.

## Report, not gating

- p50, p99.9 and max world CPU.
- Food-update ticks vs other ticks.
- Client-side frame-gap jitter p99.
- CPU steal.
- Length census at window start and end.
- Cost.

## Outcome text

If both runs pass, the README states it in one plain sentence, for example: "500 real
players with fully grown snakes run on one core of an AWS c7a.medium; over 30 minutes,
99% of ticks finish within the 7 ms tick and none fall behind." Link the study.
If either run fails, the README claim stays unchanged and the study reports what failed.

Budget: $3 hard cap, usual AWS rules.

## Post-plan amendment (2026-10-30 ~12:45 UTC, before any acceptance timing)

Reason: #284 measured why the p99 bar fails. Food maintenance runs on the shared 504 ms
epoch (one tick in 72, ~1.4% of ticks, recorded in docs/server-rules.md as the official
inference). On the Zen 5 guest those ticks average ~8.5 ms against ~4 ms otherwise, so
world p99 measures that one tick class. Exact attempts at it (#291) failed and the
remaining leads are each a fraction of a millisecond. Moving the epoch would break
fidelity. A player experiences falling behind (deferred ticks and frame gaps), not
whether one tick in 72 overruns its 7 ms slot by 1–3 ms while the server keeps up.

Amended pass (both runs): criteria 1, 3 and 4 unchanged. Criterion 2 becomes
report-only: world CPU p99, p99.9 and max, maintenance vs other ticks, and whether p99
is at most 7.0 ms. Client-side frame-gap and frame-excess p99 (as in #281/#286) are also
reported, not gated: no clean official-500 baseline exists to set a threshold from.

Outcome text if both pass: "500 real players with fully grown snakes run on one core of
an AWS c7a.medium without falling behind: over 30 minutes no tick was deferred. The
food update every 504 ms takes about N ms; other ticks about M ms." Use the measured N
and M. No claim that every tick fits 7 ms unless p99 ≤ 7.0 also held.


## Prospective v2 amendment (maintainer, before any v2 timing)

This is a new study. The complete original criteria and labelled amendment above
remain verbatim; this section prospectively supersedes only the end-class bounds
and generator implementation. The frozen failed predecessor is
[v1](https://github.com/NikitaMGrimm/slither-sim/blob/55f2921594aff00044624e47a8a1f90c4ee0b073/research/performance/2026-10-30-official-500-acceptance/REPORT.md),
with retained-data diagnosis at `55f2921594aff00044624e47a8a1f90c4ee0b073`.
V1 remains failed: zero deferrals, but end mean/p90 **99.034/224** exceeded
**65/196**, and the frozen overlapping client reports reached **505.683 ms**
against **<42 ms**. Its six late reports were caused by synchronous endpoint
histogram publication in generator loops. No v1 result is rescued or pooled here.

The maintaining agent corrects the v1 end-class criteria error before v2 data:
at the full-window endpoint require **living >=490, mean segments >=45, and
p90 >=156**, with **no upper bound and no additional end p99 bound**. A world
that grows longer is heavier load, rather than an easier replacement. Keep the
original bounded entry screen: three consecutive 500-tick endpoints, ending at
or after 193,000 ticks, with living >=490, mean 45–65, p90 156–196, p99 >=200.

Runtime pin: `388be59aebf3108f5c510aa7269c08b67851b202`, current main when this
v2 preparation begins. Runtime src/Cargo files are unchanged from v1's pin.
Build locked portable release `-C target-cpu=x86-64`, Rust 1.99, thin LTO and one
codegen unit. Both pairs use the same verified host/client binaries. No runtime,
policy, input cadence, framing or configuration tuning between runs.

Generator correction: each of the six received-frame A4 decision workers retains
its original generator CPU 0–5 and fixed 42 ms cadence. Separate connector and
publisher threads inherit that worker's CPU. Blocking close/connect handshakes,
the admission mutex and its pacing stay off the decision loop. Completions return
through a channel. Histogram maps move to the publisher; serialization and all
stderr output, including ordinary reports, happen there. Bound pending jobs at
128; overflow or a worker error fails collection. Record connection completion
durations. Control remains on CPU 7. No thin clients or authoritative state.

Before warmup on the first fresh generator, build and run the slow-connect and
slow-publication regression in `background.rs`: deliberately block each job for
500 ms while a second seat makes >=15 decisions in 650 ms, with maximum lateness
<42 ms. The synchronous-dispatch mutation must fail. Run linked ladder controls,
the prospective gate controls, token controls and docs checks. After these pass,
commit this final plan and harness, record the commit in execution receipts, and
verify staged source/binary identities before starting tick zero. Building and
these tests use the first generator within its existing lifetime/budget, not an
extra instance. No instabox access or local Rust work.

## Unchanged collection, gates and stop rule

Use exactly the [c7a ladder](../2026-10-29-c7a-mature-capacity/REPRODUCE.md) and
[#256 mature class](../2026-10-28-capacity-checkpoint/PLAN.md), with the v2
exceptions above. One c7a.medium vCPU for every host/observer thread; separate
c8a.2xlarge generator, same Frankfurt availability zone, private traffic. Protocol
19 stock frames, 500 socket slots, zero host bots, seed 7, default official
adaptive density. No human, TLS or artificial latency.

Warm naturally from tick zero with 100 resident/400 rotating clients; keep the
linked 20,000-tick lifetime schedule, stop forced churn at tick 180,000, pass the
three-endpoint entry screen at >=193,000, settle ten seconds, count **1,800 s**.
No checkpoint, forced counted churn, shortened window or earlier endpoint choice.
Warmup is bounded at 45 minutes. Keep every in-window length census and drift.

For each run gate **zero deferred ticks**, full-window final lower class, and
unchanged client/queue/generator validity: all six workers alive and draining;
worker median CPU <70%, p99 <80%; input-lateness p99 **<42 ms in every selected
one-second report**, including the unchanged boundary-overlapping reports;
zero unexpected decode/transport errors, write blocks, buffer-full/reject events,
counted admission refusals or forced retirement; frame progress from each worker;
sampled sockets >=495 and mean living >=490. Both send queues: slope <8 KiB/s,
last-third mean <=first-third mean +512 KiB. No post-result exclusions. Keep the
metric-token normalizer and actual worker metric intervals.

CPU/task reconciliation must be within max(0.03 ms/tick, 2%). Sum aligned
simulation + socket user/kernel clocks per tick over complete interior 100-tick
blocks, then compute tails; never add independent component quantiles. Deferrals
use the exterior census bracket covering the entire requested interval; end
class uses the first census at/after its end. Retain offsets and last interior
census. Report p50/p99/p99.9/max, maintenance versus ordinary ticks (unchanged
absolute 504 ms epoch), client frame-gap/header residual/excess p99, steal, length
census, backpressure, generator context and cost. CPU p99 <=7 ms and client frame
tails remain **report-only**, not gating. No frequency/network/security tuning.
No build/test/reduction jobs during counted windows. Retain process commands.

Two independent fresh pairs must pass. Launch run 2 only after run 1 passes every
amended gate and its resources are verified gone. A known host/class/queue
failure stops; no rescue or tuning. At most one infrastructure replacement
overall for Spot reclaim or a failed generator gate without an independent host
failure, logged in ATTEMPTS. Never repeat until a pass. Keep failures separate.
If both pass, the maintaining agent may use the original amendment's sentence
from the measured results; this PR changes neither root README nor changelog.

## Resource safeguards

AWS Spot Frankfurt, profile `slither-bench`, tags at creation (Project/Task/Role/
expiry), IMDSv2, one-time requests, terminate on interruption/shutdown, root
DeleteOnTermination. Source-restricted management SSH; peer-only private game,
control and artifact ports. No public game, Elastic IP, NAT or extra paid service.
Boot-installed 100-minute poweroff timers verified <=102 minutes from launch.
One pair at a time, planned maximum 90 minutes per pair, at most three pairs
including replacement. Host/generator bid caps $0.06/$0.30 per hour. Full
three-pair fail-safe compute bound **$1.836**, plus **$1 noncompute reserve =
$2.836 < $3**. All v2 build/test work is inside that cap. No extension/fourth pair.
10/24 GiB roots, one bounded target removed after build, artifact Internet egress
<=512 MiB total. Stop on permission/authentication denial, without bypass.

Retrieve/hash archives, cancel task requests by recorded identity, terminate and
verify task instances/volumes gone before another pair; delete task key/security
group and scan **every enabled region** at completion. Preserve unrelated work.
Keep raw data, AWS identities/addresses, SSH material and personal paths outside
Git. Publish lean selected evidence and scripts linking earlier harnesses.

## Pre-reduction method correction (maintainer, during run-1 warmup)

The stated nearest-rank method means sample index **ceil(n*p)-1**, maximum n-1,
and histogram cumulative count **>=ceil(n*p)**. The v1 helper used a different
index; the maintainer requested correction in both studies before v2 reduction.
Use corrected v1 `acceptance.py` and `select.py` from commit
`fa10b79368e4176f03287de7605d2058c5948665` for v2 reductions and the second pair.
V2 run 1 continues uninterrupted with the original in-memory collector. Retain
that online result and allow only report-tail fields/the derived seven-ms flag
to differ on replay; compare every gate, count, mean and other value exactly.
These helpers calculate report-only CPU/frame tails. No frozen acceptance gate,
the ladder's generator headroom calculation, world, binary, cohort or timing
changes. The historical ladder study remains untouched. This correction is
recorded before any v2 result is reduced; it is not an outcome-based amendment.

## Post-result harness limitation (no v2 gate or result repaired)

Review identified that `cpuFraction` measured only the decision thread, while
its connector and publisher share that worker's CPU. Their CPU was omitted from
the per-worker 70%/80% gate. Whole-process telemetry cannot substitute for the
missing per-worker sum. No v2 result was reduced and no headroom pass is claimed.
Preserve the executed source; v3 will measure decision + connector + publisher
CPU and gate that sum at the same 70% median / 80% p99 thresholds. This note is
a post-result limitation, not a retrospective change to v2 or its outcome.
