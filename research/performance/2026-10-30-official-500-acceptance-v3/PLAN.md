# Official-500 on-demand acceptance v3: frozen plan

New authorized series; keep failed v1 and interrupted v2 intact and separate.
No v3 timing or host world may start before this plan and the tested harness
are committed. Record that commit and the build identities before tick zero.

## Inherited method and changes

Reuse [v2 at 4b6cdf24](https://github.com/NikitaMGrimm/slither-sim/blob/4b6cdf24d57a00b6f7edb2a723c79dbae86570a4/research/performance/2026-10-30-official-500-acceptance-v2/PLAN.md)
and its [harness recipe](https://github.com/NikitaMGrimm/slither-sim/blob/4b6cdf24d57a00b6f7edb2a723c79dbae86570a4/research/performance/2026-10-30-official-500-acceptance-v2/REPRODUCE.md),
the [c7a ladder](../2026-10-29-c7a-mature-capacity/REPRODUCE.md), and
[#256's fully grown class](../2026-10-28-capacity-checkpoint/PLAN.md).
Do not copy those harnesses into this study. The only generator change is summed
headroom accounting. Other prospective changes are on-demand instances and the
completion-based rule for starting run 2 below.

Runtime remains **`388be59aebf3108f5c510aa7269c08b67851b202`**, the v2 pin.
No runtime reason exists to change it: preserve the same host binary/workload.
Rust 1.99, locked portable release `-C target-cpu=x86-64`, thin LTO, one codegen
unit. Both runs use the same verified host/client build. No tuning between runs.

One c7a.medium vCPU for every host/observer thread; separate c8a.2xlarge generator
in the same Frankfurt availability zone within each fresh pair. Six full
received-frame A4 protocol-19 workers on generator CPUs 0–5, control on CPU 7.
Connector/publisher inherit each worker CPU. Default official adaptive density,
seed 7, 500 socket slots, zero host bots. Private traffic, no TLS, artificial
latency, human player or outside game server.

Warm from tick zero, natural food/wire history, 100 resident/400 rotating clients,
the linked 20,000-tick lifetime schedule, no forced churn after tick 180,000.
Entry screen: three consecutive 500-tick endpoints ending at/after 193,000, each
with living >=490, mean 45–65 inclusive, p90 156–196 inclusive, p99 >=200. Warmup
deadline 45 minutes. Settle ten seconds; count **1,800 seconds**. No checkpoint,
forced counted churn, window shortening, favorable subwindow or endpoint choice.

## Gates, reporting and sequence

For each complete run require zero deferred ticks; final living >=490, mean
segments >=45 and p90 >=156, with no upper bounds or extra final p99 bound.
Retain the original bounded entry screen. Final class uses the first census
at/after the requested end; deferrals use the exterior covering census bracket.
Retain all class drift, boundary offsets and last interior census.

Keep every original client/queue/generator gate: all six workers alive/draining;
per-worker median CPU <70%, p99 <80%; input-lateness p99 **<42 ms in every
selected one-second report**, including unchanged boundary-overlapping reports;
zero unexpected decode/transport errors, write blocks, buffer-full/reject events,
counted admission refusals or forced retirement; each worker advances frames;
sampled sockets >=495 and mean living >=490. Each send queue slope <8 KiB/s and
last-third mean <=first-third mean +512 KiB. No exclusions or threshold changes.

**Headroom correction:** `cpuFraction` is now the sum of decision + connector +
publisher CPU deltas, divided by the same report wall interval. Sample live POSIX
thread CPU clocks so in-flight jobs and thread overhead count, not only completed
jobs. Preserve the ladder's headroom quantile calculation and strict 70%/80%
boundaries; only the CPU input changes. Publish all three component fractions
beside the sum and retain process-wide CPU for reconciliation/context. A dead
helper or clock-read failure fails the harness; it cannot become a zero sample.

World CPU is aligned simulation + socket user/kernel per tick over complete
interior 100-tick blocks; never add component quantiles. CPU/task means reconcile
within max(0.03 ms/tick, 2%). Report world p50/p99/p99.9/max, maintenance versus
ordinary ticks on the unchanged 504 ms epoch, client frame-gap/header residual/
excess p99, steal, length census, queues and cost. These CPU/frame report tails
use nearest rank `ceil(n*p)-1`, maximum n-1; histogram cumulative rank
`>=ceil(n*p)`. Stage corrected v1 helpers/selector **before either startup**.
Unlike v2's historical sequence, both v3 online and offline tails are corrected
from the outset and must replay identically. World p99 <=7 ms and client frame
tails remain report-only. No frequency/network/security tuning. No builds,
tests or reductions during counted windows; retain full process commands at
boundaries. Keep the metric-token normalizer and actual histogram intervals.

Two independent fresh runs. **Start run 2 only after run 1 completes a full
window and closes/retrieves its archive, whether its gates pass or fail**, and
after its resources are verified gone. A failed complete run is never replaced
or discarded. At most one infrastructure replacement for interruption/collection
loss before a complete window; preserve it and its cause. It retries that logical
run with unchanged binaries/method. It cannot rescue a completed failed gate or
an entry-class failure. If completion remains impossible, stop incomplete.
No third completed run. Do not pool runs or earlier series. Both complete runs
must pass for acceptance; the maintaining agent writes any README sentence.

## AWS resource ceiling and privacy

Profile `slither-bench`, Frankfurt, **on-demand for both instances**, one pair at
a time, at most three pairs including the single replacement. Tag instances,
volumes, network interfaces, key and security group at creation (Project/Task/
Role/expiry). IMDSv2, terminate-on-shutdown, root DeleteOnTermination. Existing
source-restricted management SSH and peer-only private game/control/artifact
ports; no public game, Elastic IP, NAT or additional paid service.

Boot-installed **90-minute** poweroff timers; verify each is at most **92 minutes
from launch** before work. Planned pair limit **88 minutes**, including first-
generator builds/tests. Never extend a timer or add a fourth pair. Official AWS
on-demand rates are $0.05857/h host and $0.49196/h generator at preparation;
require them <=$0.06/$0.50 before provisioning. Conservative full-three-pair
compute bound **3 × (92/60) × $0.56 = $2.576**; reserve **$0.40** noncompute:
**$2.976 < $3 hard cap**. Stop if quotes/verified timer bounds violate this.

10/24 GiB root volumes; one bounded Cargo target removed after build. Total
artifact Internet transfer <=512 MiB. The $0.40 reserve covers six public IPv4
allocations (<$0.05 over the bound), 34 GiB/pair of gp3 storage for these short
lifetimes, bounded Internet artifact egress and a margin. No extra build instance.
All first-pair tests happen before warmup on its generator, inside this budget.
No local Rust/heavy work; no instabox access. No competing work during timing.

Retrieve/hash each archive; terminate task instances and verify task volumes
gone before another pair. At the end delete task key/security group and scan
**every enabled region**, retaining private inventories. Preserve unrelated
resources. Stop on permission/authentication denial without bypass. Raw logs,
AWS identifiers/addresses/hostnames, account details and personal paths remain
outside Git. Publish lean selected results, cost/cleanup receipts and the report;
change only the research index outside this study, not root README/changelog.

## Reproduction and preflight proof

Stage linked v2 files at `sharedv2/` from `4b6cdf24`; retain its `background.rs`
and other unchanged root collectors. Stage v1 `acceptance.py`, `select.py` and
`check_quantiles.py` from maintainer commit `fff66d0a` before any startup; all
other linked v1 files stay at v2's original pin. Add only this study's files.
`prepare.py` executes v2 and patches CPU accounting, retains the helper handles,
and adds `worker_cpu.rs`. Both live and in-flight helper CPU count.

On the first generator, compile the actual prepared modules with
`rustc --edition=2021 --test check_runtime.rs -o runtime-test`, then run
`./runtime-test --test-threads=1`. Compile/run the `synchronous_mutation` and
`omit_background_cpu_mutation` controls; their targeted tests must fail. Run
`python3 check_headroom.py`; its `--mutate-decision-only` control must fail.
Also run linked ladder, v2 gate/token and corrected quantile controls and docs
checks. Record results before committing the final plan/harness and tick zero.
