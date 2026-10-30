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

## Execution binding (worker, before timing)

The complete maintainer criteria above are copied byte-for-byte, including their
labelled post-plan amendment. That amendment controls the decision: p99 is
reported, never used to pass or fail. Nothing in this binding relaxes its gates.

Runtime pin: `d1bab317d2d1471598971d2c7cd20210e83677af`, current main when this
plan was prepared; re-read main immediately before the first pair is provisioned
and record any pre-data refresh here. Both runs use the same pinned portable
host and client binaries. No runtime/client tuning or binary change between runs.
No production changes or outside game servers; no root README or changelog edit.

Reuse [the c7a ladder](../2026-10-29-c7a-mature-capacity/REPRODUCE.md), its six
received-frame A4 workers on generator CPUs 0-5 and control on CPU 7, private
same-AZ traffic, CPU clocks, census, socket/queue and generator gates. Use exactly
500 socket slots, zero host bots, protocol 19, seed 7, official/default adaptive
radius, no TLS/artificial delay, and no public game ingress or human player.
Every host and host-observer thread shares the c7a.medium's only vCPU; verify
affinity. The separate generator is an eight-vCPU c8a.2xlarge Spot instance.

Use [#256's mature class](../2026-10-28-capacity-checkpoint/PLAN.md) exactly:
**living >=490, mean segments 45-65 inclusive, p90 156-196 inclusive, p99 >=200**.
Keep 100 resident and 400 rotating logical clients with the retained 20,000-tick
lifetime schedule; stop forced churn at tick 180,000. Warm from tick zero to at
least 193,000 and require three consecutive 500-tick endpoints in class. Bound
warmup at 45 elapsed minutes; a class failure is not an infrastructure retry.
After ten seconds settling, count 1,800 seconds, with only natural deaths/rejoins.
Retain all length drift; do not shorten the window to keep the class.

Use complete 100-tick interior blocks and aligned simulation + socket user/kernel
clock sums, as [the ladder/#258](../2026-10-29-c7a-mature-capacity/PLAN.md).
Never add component quantiles. Reconcile mean clocks with task counters within
max(0.03 ms/tick, 2%). Gate zero deferrals over the census bracket covering the
full requested interval, including edges. End maturity uses the first 100-tick
census at/after the end mark; retain the last interior census too. Report the
requested and canonical intervals and their endpoint offsets. Entry/start class
and final class must satisfy the same bounds; no in-window maturity exclusion.

Unchanged client/queue/generator gates: all six workers alive and decoding/draining;
per-worker median CPU <70%, p99 <80%; worst one-second input-lateness p99 <42 ms;
zero unexpected decode/transport errors, write backpressure, buffer-full events
or counted admission refusals; each worker's frame count advances; no forced
retirement in the window. Each send queue has slope <8 KiB/s and last-third mean
at most 512 KiB above first-third mean. Sampled sockets >=495 and mean living
population >=490. Do not replace A4 with thin clients or widen a gate.

Report nearest-rank world CPU p50/p99/p99.9/max, simulation/socket components,
maintenance/other tick distributions and p99 <=7.0 status. Maintenance is classified
by the unchanged absolute 504 ms epoch at consecutive tick times, as verified in
[#284](../2026-10-30-live-capacity-checkpoint/REPORT.md). Client frame-gap,
header-gap residual and frame-excess use the read-only calculation from
[#281/#286](../2026-10-29-battledome-experience/prepare.py). Accumulate 1-microsecond
histogram bins only for the counting phase and publish once per worker at its end,
to bound raw output; retain each worker's actual histogram interval. These three
client distributions are report-only. All one-second headroom/lateness reports
remain unchanged. Retain steal and process commands; no build/reduction jobs during
counted windows. No CPU-frequency, network, security or limit tuning.

Run 2 gets a fresh host/generator pair only if run 1 passes every amended gate.
A measured host failure stops the series. At most one fresh infrastructure
replacement overall (Spot loss or failed generator gate), with identical types,
binaries and method, logged in ATTEMPTS.md. Queue/host/maturity failures are not
silently reclassified as infrastructure. Do not repeat until a pass.

## Resource safeguards (before provisioning)

Use only Frankfurt Spot and profile `slither-bench`. Create Project/Task/Role/expiry
tags with instances/volumes, tag the task key and security group at creation,
require IMDSv2, one-time Spot/terminate-on-interruption, terminate-on-shutdown and
DeleteOnTermination root volumes. Management SSH is source-restricted; private
peer-only game/control/artifact ingress. No Elastic IP, NAT gateway or extra paid
service. Verify the boot-installed 100-minute poweroff timer on each instance
before build/collection, with a 102-minute launch-to-stop safety bound.

One pair at a time, at most three pairs including the single replacement. Planned
pair limit is 90 minutes from first launch. Bid caps $0.06/h host and $0.30/h
generator bound three full fail-safe exposures to **$1.836 compute**. Reserve
**$1.00** for storage, public IPv4, bounded artifact Internet egress and other
charges: **$2.836 < $3 hard cap**. Root volumes are 10/24 GiB; total artifact
Internet transfer is capped at 512 MiB. Never extend timers, add a fourth pair,
change market/type, or exceed that bound. Do not launch a new pair until prior
instances/requests/volumes are verified gone.

Keep credentials, requests, identifiers, addresses, SSH files and raw outputs
outside repositories. Stop on permission/authentication denial; do not bypass it.
Terminate the task instances, cancel remaining task Spot requests by recorded
instance/request identity, verify task volumes gone, delete task key and security
group, then scan every enabled region. Report unrelated resources without touching
them and distinguish unavailable regions from zero inventory. Publish only bounded
selected results and what recomputes the tables, with links to earlier scripts.

## Pre-provisioning source refresh (before data)

Current main is now `825c7cba904ae3465c5a1137a3caa0326c425ff1`. The intervening changes are research/CI only;
simulator src/Cargo files are unchanged. Both runs will use this refreshed pin.

## Pre-data instrumentation correction

Before any counted timing, source inspection found that the linked text-control
writer appends a newline to `RESET`, while the new client metric accumulator
compares a bare `count` token. Add the bounded [marker normalizer](marker_tokens.py)
on generator control CPU 7 before counting, on both pairs. It removes only the
newline from the two report-only `count`/`end` tokens and journals each operation.
The client records the actual metric interval, including normalization delay.
No host/client restart, binary change, policy/cadence change, warm-history reset,
criteria change or outcome-based retry occurs. The 1,800-second host window and
all one-second generator/queue gates remain unchanged. Keep this correction and
all earlier frozen source; do not tune the second run.
