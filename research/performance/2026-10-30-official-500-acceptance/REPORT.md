# Official-500 thirty-minute AWS acceptance

Run 1 fails the frozen acceptance: endInMatureClass, clientsQueuesGenerator. Run 2 is not started. No retiming, tuning or replacement is used to rescue that failure.

World CPU p99 and client frame tails are report-only under the maintainer's labelled pre-acceptance amendment. This study establishes no seven-ms p99 claim unless the measured value actually meets it. The root README and changelog are unchanged.

[Plan and amendment](PLAN.md), [attempts](ATTEMPTS.md), [selected results](selected-results.json), [reproduction](REPRODUCE.md), [verification](verification.json), [cost](cost.json), [cleanup](cleanup.json).

## Method

Runtime `825c7cba904ae3465c5a1137a3caa0326c425ff1`, Rust 1.99, locked portable release `-C target-cpu=x86-64`, thin LTO and one codegen unit. Every host/observer thread shares one c7a.medium vCPU. Six protocol-19 received-frame A4 workers run on a separate eight-vCPU c8a.2xlarge in the same availability zone, over private WebSockets, with control on CPU 7. No thin clients, TLS, artificial latency, human player or host bots. Full decoding/frame materialization, policy, pings and natural reconnects are retained.

Reuse [the c7a ladder](../2026-10-29-c7a-mature-capacity/REPORT.md) and [#256's exact mature screen](../2026-10-28-capacity-checkpoint/PLAN.md): at least 490 living, mean 45–65, p90 156–196 and p99 at least 200. Start from tick zero, retain natural food and wire history, keep 100 resident/400 rotating clients, stop forced rotation at 180,000 and require three consecutive 500-tick endpoints, ending at or after tick 193,000. The passing screen ends at ticks 192,000/192,500/193,000, with mean lengths 53.870/54.138/54.742, p90 183/181/182 and p99 246/246/248; all have 500 living. No checkpoint or authoritative state edit. Settle ten seconds; count 1,800 seconds without forced churn. Preserve every in-window census, including class drift.

The plan and harness were frozen in `e8b90600` before warmup; the supplied criteria are an unchanged prefix of PLAN.md. Before counting, `52f5f029` records a report-marker newline correction: a bounded helper normalizes only the two metric tokens on generator control CPU 7. Host/client processes, binaries, policy/cadence, warm history, gates and window stay unchanged. Actual worker metric intervals and the raw correction journal are retained. This is not an infrastructure replacement.

Read simulation and socket user+kernel CPU at consecutive tick edges, sum aligned values, then compute nearest-rank tails over complete interior 100-tick blocks. Never add component quantiles. Keep observer CPU separate and reconcile means against task counters. Maintenance classification uses the adopted absolute 504 ms epoch, verified in #284. Deferrals use the exterior census bracket; end maturity uses the first census at/after the end mark. The selected data retain offsets and the last interior census.

## Results

| Run | Pass | Deferred | World CPU p50 / p99 / p99.9 / max ms | Maintenance / other mean ms | End mean / p90 / p99 length |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | False | 0 | 4.750 / 7.331 / 8.517 / 11.174 | 7.287 / 4.805 | 99.034 / 224 / 298 |

| Run | Gap p99 ms | Header-gap residual p99 ms | Frame excess p99 ms | Steal % |
| --- | ---: | ---: | ---: | ---: |
| 1 | 28.016 | 2.875 | 3.629 | 0.0012 |

| Run | Ticks | Count | World mean / p99 ms | Simulation mean ms | Socket mean ms |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | all | 257100 | 4.839 / 7.331 | 2.418 | 2.421 |
| 1 | maintenance | 3571 | 7.287 / 9.146 | 4.564 | 2.722 |
| 1 | ordinary | 253529 | 4.805 / 6.937 | 2.388 | 2.417 |

| Run | Component | CPU p50 / p99 / p99.9 / max ms |
| --- | --- | ---: |
| 1 | Simulation/broadcast | 2.395 / 4.499 / 4.935 / 7.628 |
| 1 | Socket I/O | 2.319 / 4.275 / 4.711 / 5.825 |


Component quantiles are separate distributions. The world distribution comes from aligned per-tick sums. Client frame-gap, header-gap residual and frame-excess pool one-microsecond histograms from all six workers over their recorded intervals; none is an acceptance gate. A failed generator or queue gate makes any host CPU result diagnostic rather than a capacity qualification.

## Gates and world evolution

| Run | Gate | Result |
| --- | --- | --- |
| 1 | zeroDeferredTicks | pass |
| 1 | startInMatureClass | pass |
| 1 | endInMatureClass | fail |
| 1 | clientsQueuesGenerator | fail |
| 1 | cpuClockReconciliation | pass |

Run 1 counts 257,100 interior ticks over 1799.702 canonical seconds (1800.025 requested seconds). The deferral bracket extends 0.452 seconds before and 0.622 seconds after those marks. Deferred ticks: 0; interior-only count: 0.

Length mean/p90/p99 moves from 56.220/182/251 to 99.034/224/298; living 500 to 499. The full ranges remain in selected results. Longer snakes outside the upper class bounds are not relabelled immature or discarded.

Client/queue validity: False. Worst one-second input-lateness p99 505.683 ms; worst worker CPU p99 16.51%. Each worker advances decoded frames; no counted forced rotation or thin mode. Send-queue slopes, first/last thirds, population and error counters are retained for the exact frozen gates.

The failing client predicate is input-lateness p99 **<42 ms**, unchanged from the ladder; frame-gap and frame-excess are distinct report-only measurements. Worker CPU medians are 15.21–15.54% and p99 values 16.07–16.51%, below 70%/80%. Socket population is 498–500; sampled living population is also 498–500. There are zero write-block, buffer-full, reject, admission-refusal or forced-retirement increments, and no `CLIENT_ERROR` or panic in the retained client log. Application send queues stay zero. Kernel send queues peak at 447,562 bytes, slope 0.423 bytes/s, first/last-third means 5,611.8/5,196.2 bytes; both queue gates pass. Thus CPU saturation or persistent queue growth is not established as the cause of lateness.

The generator-failure label does not authorize a replacement here: the end-class failure independently stops the series. The world became longer, rather than failing to mature. Both upper bounds fail at the full-window endpoint. No early subwindow, trimmed client report or replacement is substituted. Run 2 and the optional infrastructure replacement are unused; this is a failed acceptance, not a qualified capacity claim.

Clock/task reconciliation difference 0.000071 ms/tick; observer CPU 0.0564 ms/tick. IRQ/softirq/steal are separate accounting context, never added to world CPU. World p99 <=7 ms: False.

## Cause of the input lateness

**Post-result diagnosis from retained v1 data only; no gate or outcome changes.**
[Reduction](diagnose_lateness.py) and [selected diagnosis](lateness-diagnosis.json).
The frozen overlapping-report selector includes 10,806 worker reports. Exactly
**six reports** reach 42 ms: one per worker, all in the single wall-second bin
starting 1,800 seconds after the start. They end 155–520 ms after the requested
host window ends at +1,800.024945 s. No earlier selected report violates the gate;
the largest earlier p99 is 3.870476 ms and largest pass is 11.583783 ms. These
observations do not remove any report from the frozen gate.

| Worker | Report end offset s | lateP99Nanos | lateMaxNanos | maxPassNanos | Joins / deaths in report interval |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 1800.204273 | 165753686 | 175839550 | 178397543 | 1 / 1 |
| 2 | 1800.460276 | 421551017 | 431591694 | 434448380 | 0 / 0 |
| 3 | 1800.288775 | 252747041 | 262667088 | 263036750 | 0 / 0 |
| 4 | 1800.180336 | 85501122 | 93590828 | 94144078 | 1 / 1 |
| 5 | 1800.375326 | 335401834 | 347264254 | 349343699 | 0 / 1 |
| 6 | 1800.545164 | 505682703 | 516802133 | 519342309 | 0 / 0 |

The cause supported by the timing and retained source is **synchronous metric
publication in the generator decision loops**, serialized through shared stderr.
At +1,800.025693–1,800.025967 s all six workers observe the end token and enter
the `CLIENT_WINDOW` path added by [prepare.py](prepare.py). That path constructs
and prints the accumulated histograms before servicing seats. Each line is
702,494–718,858 bytes; the six appear in worker order **4, 1, 3, 5, 2, 6**. Long
passes rise in that same order: **94, 178, 263, 349, 434, 519 ms**. For workers
1/3/5/2/6, report timestamp minus the pre-publication metric-end timestamp matches
`maxPassNanos` within 18 microseconds. The first worker's subsequent ordinary
report can also wait behind other histogram writers. This accounts for the
endpoint-only staircase while CPU fractions remain low. It is a generator
publication artifact, not evidence of a half-second host admission stall.

Synchronous reconnects are a separate generator hazard, but do not explain this
episode: workers 2/3/6 have neither a join nor a death in their bad interval, and
worker 5 has no completed join. The linked ladder puts `Seat::connect` and its
global mutex inside each worker's seat loop. It blocks on TCP/HTTP upgrade and
login send, adds a 3 ms sleep, then returns a nonblocking socket; challenge/setup
and snake admission are processed later, rather than waited for by this call.
No individual reconnect start/end or mutex-wait durations were recorded. Thus
the requested per-reconnect durations cannot be recovered from aggregate join
deltas; this study does not claim a measured reconnect latency.

The server records only census, socket totals, stats and CPU blocks, with no
per-connection accept, HTTP-handshake completion or snake-admission timestamps.
Those requested event times are unavailable. The common aggregate bracket runs
from tick 451,400 at +1,798.548517 s to tick 451,700 at +1,800.647310 s: living
500→499, occupied 500→500, deaths 2,095→2,101, zero deferrals. Nearby socket
samples retain 500 players, zero application queue, zero write-block/full/reject
counters, and kernel queues 6,010/5,806/6,246 bytes. These aggregates do not prove
individual host handshake latency; the generator publication path and its exact
timing identify this observed stall without that unavailable inference.

V1 remains failed for both recorded reasons. A prospective v2 must move reconnect
work off decision loops **and** move histogram serialization/output off those
loops, while retaining the unchanged overlapping-report gate. No v1 retiming,
trimming, replacement or new cloud collection was performed for this diagnosis.

## Cost and teardown

The [cost receipt](cost.json) bounds launch-to-verified-cleanup compute at $0.405219 using the frozen bid caps and lifetimes of at most 4,052.519/4,050.519 seconds for generator/server. Including the full $1 noncompute reserve gives **$1.405219 < $3**; this is a conservative bound, not a final invoice. The predeclared three-pair fail-safe bound remains $2.836. Only one pair was used, without timer extension or an extra paid service.

The [cleanup receipt](cleanup.json) verifies all 17 enabled regions, zero unavailable regions, no nonterminated instances, open/active Spot requests, volumes or Elastic IPs. Task key and security group are deleted; unrelated resources are preserved. Private raw evidence was retrieved and verified before termination. No cloud identifiers or addresses are published.

## Verification

The [verification receipt](verification.json) records the portable build, linked-source and copied-binary checks, synthetic ladder/amended-gate/token controls, exact online/offline replay and task-clock reconciliation. Final synthetic checks and seven documentation tests passed on the AWS generator after collection; tables recomputed byte-for-byte. No new Rust test matrix or independent review was performed for this research-only change. Final teardown receipts and subsequent prose edits change no measured result or link target.

## Limits

One seed, this A4 policy, stock protocol 19, default official density and an evolving world. These runs do not qualify arbitrary human input mixes, browser rendering, legacy mixes, slow readers, public Internet latency, TLS, long-term ecological stationarity or a maximum player count. A class failure is not repaired by selecting an earlier favorable subwindow. No runtime optimization percentage is inferred from earlier machines or the c7a ladder.
