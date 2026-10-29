# One-core c7a mature socket capacity, 2026-10-29

**750 bots passed the first ladder at 6.406 ms median world CPU and
9.933 ms p99, with zero deferred ticks. The run stopped at its 750-slot ceiling;
the requested 6.8 ms boundary was not measured.** Median headroom is 0.594 ms,
while p99 exceeds seven ms by 2.933 ms. This is the highest tested passing
count, not a measured maximum for fully grown players.

[Selected ladder results](ladder-350-results.json), [cost](cost.json),
[cleanup](cleanup.json), [verification](verification.json),
[plan](PLAN.md), [attempts](ATTEMPTS.md),
[reproduction](REPRODUCE.md).

## Question and scope

How many real received-frame A4 socket bots can one Frankfurt c7a.medium run
with median world CPU just below seven ms per tick, starting from a mature
world? This is a short capacity ladder on v0.8.0 runtime
`692cd561714b8af3ea95adb3adcbd807b3a48734`, not another optimization. Production
sources are unchanged. The server reports **AMD EPYC 9R14**, x86-64, one vCPU.
A separate eight-vCPU c8a.2xlarge in the same availability zone drives all bots
over private TCP WebSockets. Both instances are Spot.

The generator builds the host and client with Rust 1.99.0, locked release
dependencies and `-C target-cpu=x86-64`; it never enables native Zen 5 code.
The server verifies the copied host binary's SHA-256. The read-only CPU-clock
overlay and received-frame A4 driver derive from
[#256](../2026-10-28-capacity-checkpoint/REPORT.md); earlier instrumentation and
selection helpers are linked in the recipe instead of duplicated. This study
retains only new remote/dynamic-count collection code and selected aggregates.

Every host thread shares the server's only vCPU. Six client workers pin to
generator cores 0–5, with control on core 7. Clients decode/drain every delivered
frame, opt into physical-food observations and decide every 42 ms with normal
ping cadence. They receive no authoritative world observations. No thin-client
substitution or outside game server is used. There is no TLS or artificial
latency. A source-restricted public game connection allows the authorized human
to play; its seat and work are included and labeled when present.

## Mature warmup and measurement

Warm the full host from tick zero with 350 sockets: keep 70 residents, stagger
retirement/rejoin of 280 logical clients with a 20,000-tick rotating lifetime,
and stop forced churn at tick 180,000. Food and wire history arise naturally;
no checkpoint, restored food state or edited growth is used. At or after tick
193,000 require three consecutive 500-tick endpoints with at least 343 living,
mean length 45–65, p90 156–196 and p99 at least 200. These are the #256 mature
class tolerances scaled for the starting population, not an equilibrium claim.

After ten seconds settling, count 30 seconds. Add 50 bots while median world
CPU is below six ms, then 25. Stop at median at least 6.8 ms, newly deferred
ticks, failed client/queue/population gates or the predeclared 750-bot ceiling.
Take one descending confirmation at the preceding count. Joiners start short;
retain each window's length census rather than calling the whole ladder a
constant mature distribution.

Read simulation and socket task CPU clocks at consecutive completed-tick edges,
including user and kernel work. Select complete 100-tick interior blocks; sum
aligned components before taking world quantiles. Independent component p99s
must not be added. Reconcile means against interpolated task counters within
max(0.03 ms/tick, 2%). The world number includes host-clock/logging overhead
charged to those tasks; CLI and the separate observer are accounted separately.
Retain private full process command inventories at each measurement boundary.

Client/queue gates require six healthy workers, worker median CPU below 70%
and p99 below 80%, worst one-second input-lateness p99 below 42 ms, no write
backpressure/buffer-full/admission errors, bounded application/kernel send
queues and at least 98% mean living population. Each worker must advance its
received-frame count. A failed load generator is a limit, not a host capacity
measurement. Exact thresholds and the descending rule were frozen in the plan.

## Results

The mature screen passed at ticks 192,000/192,500/193,000, with 351 living
including the human: means 55.111/55.194/55.909 segments, p90 185/186/186 and
p99 237/236/236. The first counted row retained p90 181–187. **Upper windows
are younger than #256's mature class:** p90 falls as newcomers join, reaching
116–132 in the 550–750 rows and 136–141 in the descending confirmation.
Similar means do not establish similar body distributions.

Every row below passes the frozen client/queue/population gates and offline
task-counter reconciliation. Each 30-second window retains 4,200 complete
interior ticks, except 450 bots with 4,100; total 41,900 ticks. All have zero
new deferred ticks and about 142.8 completed ticks/s. No failing step was
measured. The descending 700-bot confirmation passes at 6.050 ms median and
9.771 ms p99, in the same continuing, older world.

| Bots | World median / p99 ms | Simulation median ms | Socket median ms | Deferred | Human |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 350 | 2.942 / 5.206 | 1.361 | 1.552 | 0 | 1 |
| 400 | 3.329 / 5.816 | 1.560 | 1.756 | 0 | 0–1 |
| 450 | 3.823 / 5.961 | 1.744 | 2.055 | 0 | 1 |
| 500 | 4.077 / 6.685 | 1.906 | 2.145 | 0 | 1 |
| 550 | 4.644 / 7.946 | 2.150 | 2.489 | 0 | 0–1 |
| 600 | 5.019 / 7.959 | 2.347 | 2.672 | 0 | 0–1 |
| 650 | 5.535 / 8.629 | 2.540 | 2.806 | 0 | 1 |
| 700 | 5.928 / 9.051 | 2.905 | 2.932 | 0 | 0 |
| 750 | 6.406 / 9.933 | 3.324 | 3.048 | 0 | 0 |
| 700 down | 6.050 / 9.771 | 3.270 | 2.888 | 0 | 0 |

Simulation and socket medians are separate quantiles. At 750 bots the complete
split is:

| CPU ms per tick | Median | P99 | Mean |
| --- | ---: | ---: | ---: |
| World | 6.406 | 9.933 | 6.438 |
| Simulation/broadcast | 3.324 | 6.945 | 3.404 |
| Socket I/O | 3.048 | 5.281 | 3.033 |

These are aligned task CPU clocks, not elapsed wall time or sums of component
quantiles. The largest absolute clock-versus-task mean difference across rows
is 0.004780 ms/tick. At 750, nested simulation phase means include broadcast
2.119 ms, collision queries 0.393 ms and food pickup/cache work 0.287 ms. These
phase means are not a p99 decomposition. Host observer CPU is separate:
0.051–0.062 ms/tick across the ladder. The selected results retain host-vCPU
IRQ/softirq/steal context as a separate accounting view, without adding it to
task CPU or claiming exact interrupt attribution.

## Census and human occupancy

Ranges below include every selected census in each window. Living and stats
counts include the human when connected; short death/rejoin gaps remain visible.

| Bots | Living range | Mean segments | P90 segments | P99 segments | `--stats` seats |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 350 | 350–351 | 57.51–60.87 | 181–187 | 238–240 | 351–351 |
| 400 | 399–401 | 54.56–58.65 | 158–171 | 240–243 | 400–401 |
| 450 | 449–451 | 53.88–57.15 | 128–145 | 239–244 | 450–451 |
| 500 | 500–501 | 53.20–56.15 | 125–138 | 239–245 | 501–501 |
| 550 | 550–551 | 52.25–56.51 | 116–125 | 241–245 | 550–551 |
| 600 | 600–601 | 53.23–56.41 | 121–126 | 243–246 | 599–601 |
| 650 | 651–651 | 54.37–57.16 | 122–124 | 245–248 | 651–651 |
| 700 | 699–700 | 54.40–58.46 | 123–129 | 249–250 | 699–700 |
| 750 | 749–750 | 56.50–59.97 | 129–132 | 239–245 | 749–750 |
| 700 down | 699–700 | 65.02–68.23 | 136–141 | 249–254 | 699–700 |

The results table labels sampled public-playing occupancy: `1` throughout the
350/450/500/650 rows, `0–1` in 400/550/600, and `0` in ascending 700/750 and
descending 700. Human-occupied rows are retained with labels, not treated as
bot-only. Selected results also retain sampled human-playing periods during
warmup and the ladder. A human-free window still inherits the prior world.

All six full A4 client workers stay well below saturation: the largest worker
p99 CPU fraction is 19.09%; aggregate client CPU ranges 0.567–1.095 cores out
of eight generator vCPUs. Worst one-second input-lateness p99 is 9.248 ms,
below 42 ms. Each worker advances its decoded-frame count. No thin substitution,
unexpected client error, counted admission refusal, write backpressure or
buffer-full event occurs. Application send queues are always zero; kernel send
queues peak at 74,754 bytes, with largest fitted growth 453.72 bytes/s, below
the 8 KiB/s gate. No failed timing row is omitted.

## Cost, human access and cleanup

The current combined Spot rate at launch was $0.2128/hour. The predeclared
90-minute plan had verified 120-minute shutdown timers on both instances,
terminate-on-shutdown and delete-on-termination root volumes. Compute bid
ceilings bound the two-hour timer exposure to $0.72, with a further $2 reserve
for storage, public IPv4 and bounded egress, below the $3 hard cap. No Elastic
IP, NAT gateway or persistent service is created.

The endpoint is written only to the requested private scratch file after an
authorized public TCP probe succeeds. No endpoint, access-rule address, instance,
volume, key, account or image identifier belongs in this report. The endpoint
file must be absent and task-resource deletion verified before publication.

The first server/generator lived 1,874/1,876 seconds. A proposed capacity
extension briefly launched a replacement pair, then the maintaining agent
explicitly stopped it; its instances lived 68/71 seconds. No second warmup,
endpoint or capacity window is selected. Those costs and cleanup are included.

Integrated Spot rate-times-duration compute is **$0.11524**, including the aborted
replacement. Public IPv4 adds about $0.00540 at the
[published $0.005/hour rate](https://aws.amazon.com/vpc/pricing/). A conservative
storage allowance of $0.00401 uses $0.15/GiB-month and an extra minute per root
volume; that is an allowance, not a quoted Frankfurt tariff. The known-rate
subtotal is **about $0.125**, before separately unmetered Internet egress/tax,
not a final invoice. Compute plus the original $2 noncompute reserve is $2.115,
below the $3 budget. [Selected cost inputs](cost.json) retain durations, weighted
rates and arithmetic, without cloud identifiers.

The initial cleanup found two still-active Spot request records despite removed
instances/volumes; a key-name matcher missed them. A launch guard failed but
PowerShell continued to the replacement launch. The requests were canceled by
matching task instance/request identity. On the stop instruction the replacement
controller was killed and both replacement instances were terminated immediately.
Expired CLI credentials briefly blocked queries; refreshing the existing login
session allowed cleanup to finish. No new resources were launched afterward.
These failures are retained in [ATTEMPTS.md](ATTEMPTS.md).

**Final cleanup is verified:** all four instances are terminated, all four Spot
requests closed/canceled, all task volumes gone, both runs' security groups and
key pairs removed, and the endpoint file absent. The agent scan at 02:11 UTC
attempted all 34 commercial regions: all 17 queryable/enabled regions had zero
nonterminated instances, open/active Spot requests, volumes and Elastic IPs;
17 unavailable regions are not falsely reported as scanned. The maintaining
agent separately reported the same zero inventory for all 17 enabled regions
at approximately 02:15 UTC and checked both runs' groups/keys and all four Spot
requests. Unrelated pre-existing resources were left untouched. See
[cleanup.json](cleanup.json) for the selected verification.

## Limits

One seed, one evolving ladder and one descending confirmation support a short
passing tested count, not a measured capacity boundary or an uptime guarantee. The
world, food history, packet scheduling, deaths and population distribution
continue to change. Newly added bots are short. Human gameplay can change the
world even after the human disconnects. The labels identify sampled connection
occupancy, not a causal subtraction of that influence. Browser rendering, other
policies, legacy-client mixes, slow readers, public bot traffic, TLS and sustained
Internet service remain unqualified.

Earlier [c7a short-snake results](../2026-10-27-epyc-host-recheck/REPORT.md)
reported roughly 6.06 CPU ms/tick at 500 on older code and a different scripted
input workload. #256 used a different Zen 5 machine, loopback sockets and build
target. Neither is a same-session control for this test, and no optimization
percentage is inferred from those comparisons.
