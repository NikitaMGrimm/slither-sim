# High-duty boost Battledome check, 2026-10-29

At 99.7–99.9% boosting, 80 bots retain sub-millisecond p99 gap jitter and
zero deferrals; 160 have occasional admission spikes; 320 overload in this run
with 705 counted deferred ticks and 1.23 s p95 completed respawns. Admission
searches take 61.8% of the slowest 1% at 320. The lower-boost #262 row at 320
had zero deferrals. This closes its boost-treatment gap without establishing
a maximum population or human play acceptance.

This follow-up measures the same host runtime as
[#262](../2026-10-29-battledome-experience/REPORT.md) with A4 steering and a
research-client-only boost override. No production code or new shared bot policy.

[Frozen plan](PLAN.md), [attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md).

## Compared rows

[Selected results](selected-results.json), [baseline](../2026-10-29-battledome-experience/selected-results.json)
and [table generator](tables.py) recompute all three tables below. CPU sums
aligned simulation and socket CPU before quantiles. Excess is receipt time
minus cumulative header time relative to the window's running minimum; gap
jitter subtracts each elapsed-header increment from the receipt gap. These
measure server slowdown plus delivery/scheduling, not absolute network latency.
Residual stalls use the latter gap, so seconds of accumulated excess at 320
can coexist with no single residual gap above 100 ms.

| Bots | Treatment | CPU p50 / p99 / max ms | Deferred | Excess p99 / max ms | Gap jitter p99 ms | Residual stalls >20 / >100 / >250 ms | Alive mean / min | Boost % | Boost births |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 80 | #262 A4 | 1.118 / 1.963 / 3.270 | 0 | 0.981 / 5.504 | 0.673 | 0 / 0 / 0 | 79.91 / 79 | 4.15 | 3803 |
| 80 | High boost | 1.297 / 2.058 / 3.508 | 0 | 1.195 / 5.826 | 0.867 | 0 / 0 / 0 | 79.73 / 78 | 99.87 | 21692 |
| 160 | #262 A4 | 2.141 / 3.402 / 4.442 | 0 | 1.483 / 8.026 | 1.020 | 0 / 0 / 0 | 159.84 / 159 | 4.38 | 8613 |
| 160 | High boost | 2.381 / 3.876 / 9.785 | 0 | 2.831 / 17.583 | 2.200 | 0 / 0 / 0 | 159.52 / 156 | 99.88 | 41024 |
| 320 | #262 A4 | 4.058 / 5.801 / 7.450 | 0 | 3.144 / 12.913 | 2.345 | 0 / 0 / 0 | 319.84 / 317 | 3.46 | 13790 |
| 320 | High boost | 4.394 / 15.120 / 18.557 | 705 | 3981.238 / 5187.163 | 10.500 | 3460 / 0 / 0 | 316.89 / 293 | 99.67 | 69215 |

| Bots | Treatment | Added-cohort initial p50 / p95 / max ms (n) | Respawn p50 / p95 / max ms (n) | Pending last / maximum observed age ms | Mean length first → last | Food first → last |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 80 | #262 A4 | 3118.001 / 8144.701 / 8881.362 (80) | 6.222 / 9.158 / 12.066 (148) | 0 / 3.174 | 76.95 → 76.66 | 1658 → 2268 |
| 80 | High boost | 3118.309 / 5458.054 / 6041.128 (80) | 6.046 / 9.200 / 15.250 (364) | 0 / 5.022 | 60.66 → 59.74 | 5500 → 5884 |
| 160 | #262 A4 | 3063.591 / 3116.688 / 3122.581 (40) | 6.415 / 10.220 / 12.355 (180) | 0 / 0.000 | 60.82 → 69.94 | 3612 → 3981 |
| 160 | High boost | 3131.269 / 4062.412 / 4240.707 (80) | 7.946 / 11.195 / 199.662 (642) | 0 / 80.933 | 58.49 → 59.41 | 9642 → 10383 |
| 320 | #262 A4 | 3114.528 / 3314.255 / 3356.196 (80) | 10.124 / 13.153 / 16.455 (358) | 0 / 6.867 | 63.32 → 66.10 | 5315 → 8028 |
| 320 | High boost | 3320.854 / 5687.320 / 6019.983 (160) | 12.737 / 1227.329 / 2013.984 (998) | 26 / 1939.221 | 51.85 → 55.75 | 17910 → 21257 |

| High-boost bots | Slow-tail n | Mean world ms | Admission ms / % | Socket ms | Broadcast ms | Minimap ms (nested) | Search / refused ticks | Food-epoch ticks |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 80 | 127 | 2.396 | 0.039 / 1.62 | 0.957 | 1.068 | 0.349 | 16 / 0 | 16 |
| 160 | 128 | 5.760 | 2.429 / 42.17 | 1.541 | 1.328 | 0.324 | 88 / 37 | 17 |
| 320 | 120 | 15.894 | 9.816 / 61.76 | 3.153 | 2.171 | 0.332 | 120 / 117 | 9 |

Completed waits exclude pending requests: 26 remain in the last 320 worker
reports, oldest observed pending age 1.94 s. Added cohorts differ from #262,
as the sample counts show. Mean length is body segments, not wire score.
The permanent 400-point floor is unchanged. Counts of boost-food births are
physical pellets admitted above the floor; floor boosting is free/pellet-free.
There are **1,030 deferred ticks over the entire run**, including **325 outside
counted windows**. No failed row or transition deferrals were discarded.

## Causes and decisions

The slow-tail table selects the largest aligned world-CPU 1% independently at
each count. Broadcast contains minimap, so nested values must not be added again.

1. **Admission search — exact acceleration candidate; dominant overload cause.**
   At 320 all 120 tail ticks search and 117 exhaust; admission is 9.816 of
   15.894 ms (61.76%). The full window has 2,436 searches, 1,008 strict accepts,
   no fallback accepts and 1,428 exhausted searches. At 160 it already takes
   42.17% of slow-tail time with 37 exhausted searches; at 80 only 1.62% and
   none exhaust. Preserve candidates, draw order/RNG, clearance, selected poses
   and refusal outcomes. Changing retry cadence, admission clearance, floor,
   population sizing or spawn rules is a feel/rule proposal only and needs the
   maintaining agent's approval on a local play server first.
2. **Sockets and broadcast — plausible exact work-reuse/encoding candidates.**
   At 80 these cost 0.957 + 1.068 ms (84.5%) of slow-tail time. At 320 they
   remain 3.153 + 2.171 ms (33.5%). No write-block/buffer/reject errors or
   application queue accumulation are observed. Batching or skipping delivery
   changes needs separate feel approval; no such change is validated here.
3. **Pit minimap — plausible exact projection cache.** Nested minimap costs
   0.349/0.324/0.332 ms in the 80/160/320 slow tails. Its nominal 2.504 s
   opportunities account for 35/19/2 of those tails. The instrumented minimap
   scope also contains per-recipient work; it is not all removable raster cost.
   Keep masks, outlines, shrink behavior and wire bytes exact.
4. **Food maintenance and shrinking markers — secondary here.** The 504 ms
   food epochs occupy only 16/127, 17/128 and 9/120 tail ticks, unlike the
   official-density bottleneck. No marker change lands in these tails and its
   nested cost is about 0.001 ms. Three marker-change ticks occur in the full
   320 window. This ascending run does not replace #262's descending/shrinking
   marker diagnosis. Marker/listing reuse remains an exact candidate requiring
   separate shrink fixtures; changing quantization, fade or cadence is a feel
   proposal.

No production optimization is validated by these phase shares. Follow-up code
work will be separate, one exact change per PR, with byte traces and matched
fixed-work measurements. There is no measured maximum between 160 and 320,
no Broadwell speed ratio, no long-uptime claim and no human play acceptance.
Physical-food bot delivery omits decorative pellets, although shared marker
work remains. This run also does not stress a full 15-minute same-connection
kill history. Runtime predates #260; these are not post-#260 performance rows.

## Method and interpretation

Runtime `165a5bb764aba6cbf39f42503e090f9b5e3c4d22`, shipped responsive Battledome,
seed 7, 641 reserved socket slots, zero in-process bots. Portable x86-64 Rust
1.99 locked release build on the AWS generator. One c7a.medium core owns every
host task; six real received-frame protocol-19 client workers use separate cores
on the same-AZ eight-vCPU generator, with private network traffic and no TLS or
added latency. Every frame is decoded. No authoritative policy input is used.

The unchanged A4 policy chooses each action. The harness sets only its boost bit
when the received rules report free boosting at the floor. Otherwise its original
action is retained. A native regression checks exact heading bits and the rule
gate; shared policy bytes are checked against the pinned archive. Actual
living-tick boosting must reach 70% in each accepted window. The permanent floor
allows free boost without pellets; boost-food births are recorded separately.

Warm naturally for five minutes at 80, then settle 60 seconds and count 90 at
80, 160 and 320. No restored state, edited ecology, forced churn, descending arm
or human hold. Unlike #262, the ladder omits intermediate 120/240 populations;
cohort sizes and world history therefore differ. The side-by-side rows are
descriptive, not a paired estimate of the boost bit's isolated CPU cost. Length,
food, living population, joins and actual boost share remain visible.

Reuse #262's aligned simulation-plus-socket CPU clocks, canonical complete
100-tick interior selection, whole client-report boundaries, input-lateness/CPU
and queue gates, phase/cadence diagnostics and task-counter reconciliation.
Clock/phase overhead stays included. Frame excess and residual gap jitter are
application delivery diagnostics, not absolute one-way or input-to-effect
latency. Raw sparse emission gaps are not automatically stalls. Waits begin at
login enqueue, excluding fixed reconnect delay and that connection's preceding
TCP/upgrade handshake; pending requests are reported separately.

No builds or measurements use the occupied SSH bench guest. All cloud resources
and public access remain temporary and privately identified. Terminal archive
readiness requires complete close/gzip/hash verification; local size, SHA-256
and required raw sources must verify before source cleanup. Full raw recordings,
commands, binaries and cloud receipts remain local-only, not publicly downloadable.

## Verification, cost and cleanup

All three generator/queue gates pass, including the overloaded host row. Maximum
worker CPU p99 is 6.78/10.93/20.23%; worst one-second input-lateness p99 is
3.678/4.029/18.647 ms. Decode and receipt progress remain valid. Aligned CPU
means reconcile with independent task counters within 0.00140 ms/tick; the
separate collector process uses 0.0477–0.0531 ms/tick. The actual host executable
is byte-identical to #262 and shared policy source hashes match the runtime
archive. The native boost override regression and inherited synthetic
clock/cadence/histogram/archive/source-preservation controls passed. See
[verification](verification.json) and [attempts](ATTEMPTS.md).

One paid pair was used. [Cost](cost.json) estimates **$0.113**, or **$0.154 at
bid caps**, including conservative storage, IPv4 and transfer/misc allowances;
not an invoice and no credits assumed. The frozen full-timer prelaunch bound
was $1.85 under the $2 cap. Both guests had verified 75-minute boot terminate
timers, terminate-on-shutdown and DeleteOnTermination root volumes. There was
no human seat and no play hold.

The 50,541,466-byte archive was locally size/hash/source-verified before any
source teardown. [Cleanup](cleanup.json), verified at 04:56:03 UTC, terminated
both instances, deleted the task key/security group and verified task volumes
absent. All 17 enabled regions were fully scanned: zero nonterminated
instances, open/active Spot requests, volumes or Elastic IPs. Seventeen other
regions were unavailable and were not claimed scanned. The endpoint file is
absent. The AWS session stayed valid throughout; it was never refreshed.
