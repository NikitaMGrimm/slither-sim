# One-core c7a mature socket ladder

Freeze before paid collection. Runtime is v0.8.0 commit
`692cd561714b8af3ea95adb3adcbd807b3a48734`, with the read-only CPU-clock
overlay from [#256](../2026-10-28-capacity-checkpoint/REPORT.md). Build on the
generator for portable `x86-64`, never native Zen 5; copy/hash-check that host
binary on the Zen 4 server. No production changes or outside game servers.

## Infrastructure and cost

Frankfurt, one c7a.medium Spot server and one eight-vCPU c8a.2xlarge Spot
generator in the same availability zone. All bot, control and binary-copy
traffic between instances uses private addresses. Public management SSH is
source-restricted. The server also has a public address for the authorized
human player; game ingress is restricted to the supplied single-address rule.
No address or cloud identifier belongs in the repository or PR.

Planned duration is at most 90 minutes, including setup/build/collection.
Each instance has terminate-on-shutdown, a verified boot-installed 120-minute
shutdown timer, one-time Spot with terminate-on-interruption, and root-volume
delete-on-termination. Bid ceilings are $0.06/h server and $0.30/h generator:
even both timers exhausting cost at most $0.72 compute. Reserve $2 for storage,
public IPv4 and bounded artifact egress, keeping the worst-case budget below $3.
No Elastic IP, NAT gateway, extra paid service or persistent instance is needed.
Stop before an extension, third concurrent instance or budget overrun.

Initial inventory attempts all 34 commercial regions: 17 are queryable and
empty; 17 are unavailable to the current session. Do not enable regions or
delete unrelated resources. After teardown repeat every regional attempt and
report coverage honestly, verifying all created resources are gone.

## Warmup and ladder

Start at 350 real received-frame A4 socket bots, zero in-process host bots.
Six generator workers use separate cores, with control work on another core.
They retain #256's decoding, frame materialization, input/ping cadence and
natural reconnect behavior. A serialized connection handshake prevents startup
admission contention; no thin substitution is planned. The server reserves
750 bot slots plus one human slot from startup. All host threads share its one
vCPU. Collect observer CPU separately; it competes on the same core.

Warm the complete host from tick zero, without checkpoints. Preserve #256's
20% resident fraction and approximately 20,000-tick rotating-client lifetime:
70 residents, 280 rotating clients, with retirement scheduled by cumulative
`floor(min(tick,179999)*280/20000)` at observed census boundaries. Stop forced
churn at tick 180,000. Continue to at least tick 193,000 and require three
successive 500-tick endpoints with >=343 living, mean segments 45–65,
p90 156–196 and p99 >=200. Bound the warmup at 45 minutes. Report actual
distribution and any failure; never qualify short snakes as mature.

After the screen, settle ten seconds and count 30 seconds. Add 50 bots per
step while median world CPU is below six ms, then 25 per step. Settle ten
seconds after each count change before its fixed 30-second window. New joiners
start short: retain every census, human occupancy and distribution drift.
Do not require the original length class after climbing changes population.

Stop at median >=6.8 ms, any newly deferred tick, a client/queue/population
failure, or 750 bots. A window passes only with valid clients/queues, zero new
deferrals and median <7 ms. A 6.8–7 ms target stop is a passing stop, not an
invented failing row. Step down exactly once to the preceding tested count
(or 300 if the first 350 row stops) and count one confirmation after settling.
Report the highest tested passing count, first failure or target-stop, and
the descending confirmation; no interpolation between counts or uptime claim.

## Accounting and validity

Use #256's consecutive-tick simulation/socket CPU clocks, including kernel
work, and quantiles of aligned sums over complete 100-tick interior blocks.
Report median/p99 for world, simulation/broadcast and socket components; do
not add independent quantiles. Reconcile mean world clocks with task CPU at
the same census edges. Keep full command inventories at boundaries privately.

Clients must all remain alive, decode/drain traffic, have worker median CPU
<70% and p99 <80%, and worst one-second input-lateness p99 <42 ms. Require no
write backpressure, buffer overflow, unexpected errors or counted admission
refusal; bounded send queues (slope <8 KiB/s and last-third mean at most
512 KiB above first); sampled socket players at least target minus five and
mean living population at least 98% of bot target. Failed load-generator or
queue windows are limits, not publishable host capacity numbers.

Record server CPU model from filtered `lscpu`, process/collector CPU,
IRQ/softirq/steal context, client process/core headroom and actual costs.
No CPU tuning, security-setting changes or unrelated services during counts.

## Human access and teardown

Write the public endpoint only to the requested private scratch endpoint file,
after the host is listening, warmup begins and a source-authorized public TCP
probe succeeds. Never put it in chat, the report, PR or Git. Delete the file
when stopping/terminating the server, including on interruption/failure.

Retain `--stats` player counts and a read-only public-peer playing count.
Label each window with sampled human occupancy, total seats and any transition;
do not quietly treat a human-affected window as bot-only. The human's gameplay
may change the evolving world and remains part of the measured work.

Terminate instances, cancel any open/active task Spot requests, verify attached
volumes are deleted, remove created key pairs/security groups and any task
addresses, then perform the all-region scan. Raw identifiers, logs, keys and
receipts stay outside every repository. Publish selected aggregates, changed
harness only, and links to #256/#248 and the earlier AWS studies.

## Amendment after the initial slot ceiling

**Withdrawn on the maintaining agent's stop instruction.** No second warmup or
capacity measurement was performed. The prepared extension and brief aborted
launch are retained as an attempt in `ATTEMPTS.md`; the study reports only the
first ladder. The text below preserves the proposal, not an active instruction.

The first ladder reached 750 bots at 6.406 ms median world CPU, below the CPU
stop, and its descending 700-bot row passed. All ten windows pass the original
client/queue gates and offline CPU reconciliation. Retain this complete ladder;
it establishes a passing tested count, without locating the requested boundary.

Before new paid collection, extend the design with one fresh mature world:
start at 650 bots, 130 resident and 520 rotating, with 1,500 bot slots plus one
human slot. The construction, tick thresholds, length tolerances and gates are
unchanged; scale the mature living minimum to 637. `extend.py` records these
exact configuration changes in private staging against the first harness. No checkpoint or
world transfer is allowed. Report this world's results separately.

Only one Spot pair may exist at a time. Verify first-pair teardown before
launching a replacement pair with the same permitted types and private-network
layout. The second attempt has a 50-minute planned duration and verified
80-minute shutdown timers, including the required extra 30 minutes. Keep the
whole test inside the original 90-minute wall budget and $3 cap; actual first
duration plus the second plan fits, and combined compute at the bid ceilings
for 90 minutes is $0.54. Retain the $2 noncompute reserve. The private endpoint
file is removed between worlds and rewritten only after the new public probe.
Stop the extension at the same CPU/deferral/validity rule, then step down once.
