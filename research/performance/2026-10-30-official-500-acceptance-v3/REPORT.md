# Official-500 on-demand acceptance v3

**The two-run acceptance fails.** Run 1 passes with zero deferred ticks; run 2 records **126 deferred ticks** and fails that gate. Both full thirty-minute windows are retained separately. All other gates pass in both runs. No replacement, tuning or retry is used to rescue the completed failure.

World CPU p99 and client frame tails are report-only. Root README and changelog are unchanged; there is no 500-player acceptance claim.

[Frozen plan](PLAN.md), [attempts](ATTEMPTS.md), [selected results](selected-results.json), [preflight](preflight.json), [verification](verification.json), [raw bindings](raw-bindings.json), [cost](cost.json), [cleanup](cleanup.json).

## Method

Runtime `388be59aebf3108f5c510aa7269c08b67851b202`, Rust 1.99, locked portable release `-C target-cpu=x86-64`, thin LTO, one codegen unit. All host/observer threads share the c7a.medium sole vCPU. A separate same-zone c8a.2xlarge runs six full received-frame A4 protocol-19 workers on CPUs 0–5 and control on CPU 7. Both host and generator are on-demand. Identical host/client binaries across fresh pairs; the host binary is identical to v2.

Reuse [the c7a ladder](../2026-10-29-c7a-mature-capacity/REPRODUCE.md) and [#256 class](../2026-10-28-capacity-checkpoint/PLAN.md) through the pinned v2 references in the plan. Natural tick-zero warmup with 100 resident/400 rotating clients, no forced churn after tick 180,000, three bounded entry endpoints at/after 193,000, ten seconds settling and a full thirty-minute window. Final class is living >=490, mean >=45 and p90 >=156, without upper bounds. No checkpoint, thin clients, human player, artificial latency or TLS. Keep all drift.

The plan and tested harness were frozen in `bbc7d18c` before either world. Per-worker CPU is the sum of live decision + connector + publisher CPU clocks, including in-flight jobs and thread overhead, over the same report interval. Each component is in the raw CLIENT reports; the existing strict median <70% / p99 <80% gate consumes the sum. Reconnects/close handshakes and report serialization remain off decision loops.

Both online and offline CPU/frame report tails use nearest rank from startup, using v1 maintainer helpers at `fff66d0a6e26f9552be2ea3ffc4c6c32c106a354`. Full online/offline replay, including all tails/maxima, matches in the explicit post-result checks below. The inherited selector alone did not enforce tail equality; the original standalone comparison receipt was not retained. Preserve the unchanged ladder headroom quantile calculation. World CPU is aligned simulation + socket user/kernel per tick, over complete interior 100-tick blocks; never add component quantiles. Maintenance uses the unchanged absolute 504 ms epoch. Deferrals use the exterior covering census bracket and final class the first census at/after the end. Actual client histogram intervals and boundary offsets remain selected.

## Results

| Run | Pass | Deferred | World CPU p50 / p99 / p99.9 / max ms | Maintenance / other mean ms | End mean / p90 / p99 length |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | True | 0 | 4.827 / 7.458 / 8.802 / 10.596 | 7.359 / 4.841 | 102.698 / 227 / 287 |
| 2 | False | 126 | 4.718 / 7.006 / 8.168 / 10.473 | 7.091 / 4.750 | 97.154 / 213 / 294 |

| Run | Gap p99 ms | Header-gap residual p99 ms | Frame excess p99 ms | Steal % |
| --- | ---: | ---: | ---: | ---: |
| 1 | 27.916 | 2.749 | 3.505 | 0.0006 |
| 2 | 27.952 | 2.770 | 883.503 | 0.0012 |

| Run | Ticks | Count | World mean / p99 ms | Simulation mean ms | Socket mean ms |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | all | 257100 | 4.876 / 7.458 | 2.472 | 2.404 |
| 1 | maintenance | 3571 | 7.359 / 9.587 | 4.548 | 2.811 |
| 1 | ordinary | 253529 | 4.841 / 6.825 | 2.443 | 2.398 |
| 2 | all | 257000 | 4.782 / 7.006 | 2.412 | 2.370 |
| 2 | maintenance | 3569 | 7.091 / 8.845 | 4.486 | 2.605 |
| 2 | ordinary | 253431 | 4.750 / 6.711 | 2.383 | 2.367 |

| Run | Component | CPU p50 / p99 / p99.9 / max ms |
| --- | --- | ---: |
| 1 | Simulation/broadcast | 2.457 / 4.499 / 4.916 / 7.551 |
| 1 | Socket I/O | 2.343 / 4.125 / 4.901 / 6.018 |
| 2 | Simulation/broadcast | 2.389 / 4.428 / 4.814 / 6.771 |
| 2 | Socket I/O | 2.275 / 4.101 / 4.510 / 5.734 |


Component quantiles are separate distributions. The world distribution uses aligned per-tick sums. Client frame-gap, header residual and frame-excess pool the six workers within each run, not across runs. No gate uses those frame tails or world p99.

## Gates and generator headroom

| Run | Gate | Result |
| --- | --- | --- |
| 1 | zeroDeferredTicks | pass |
| 1 | startInMatureClass | pass |
| 1 | endInMatureClass | pass |
| 1 | clientsQueuesGenerator | pass |
| 1 | cpuClockReconciliation | pass |
| 2 | zeroDeferredTicks | fail |
| 2 | startInMatureClass | pass |
| 2 | endInMatureClass | pass |
| 2 | clientsQueuesGenerator | pass |
| 2 | cpuClockReconciliation | pass |

| Run | Worker total CPU median range % | Worker total CPU p99 range % | Worst one-second input-lateness p99 ms |
| --- | ---: | ---: | ---: |
| 1 | 15.36–15.67 | 16.29–16.78 | 1.477652 |
| 2 | 15.53–15.89 | 16.69–17.02 | 1.294105 |

Run 1: 257,100 interior ticks in 1799.702119 canonical seconds, 1800.073915 requested seconds. Covering deferral bracket extends 0.392876 s before / 0.633238 s after the marks. Deferred ticks: 0. Mean/p90/p99 length: 58.582/188/248 to 102.698/227/287; living 500 to 500. World p99 <=7 ms: False.

Run 1 client/queue checks: headroom True, every worker advances frames True, write blocks/full/rejects 0/0/0, forced retirements/admission refusals 0/0, sampled sockets 498–500. Full queue maxima/slopes/first-last thirds and natural joins/deaths remain selected. CPU clock minus task mean -0.000053 ms/tick; observer 0.055777 ms/tick. IRQ/softirq/steal are separate context, never added to world CPU.

Run 2: 257,000 interior ticks in 1799.883918 canonical seconds, 1800.116968 requested seconds. Covering deferral bracket extends 0.549188 s before / 0.617406 s after the marks. Deferred ticks: 126. Mean/p90/p99 length: 54.790/181/244 to 97.154/213/294; living 500 to 500. World p99 <=7 ms: False.

Run 2 client/queue checks: headroom True, every worker advances frames True, write blocks/full/rejects 0/0/0, forced retirements/admission refusals 0/0, sampled sockets 498–500. Full queue maxima/slopes/first-last thirds and natural joins/deaths remain selected. CPU clock minus task mean -0.000009 ms/tick; observer 0.057018 ms/tick. IRQ/softirq/steal are separate context, never added to world CPU.

The post-window [report integrity check](check_reports.py) verifies all 10,806 /
10,812 selected worker reports in [run 1](run-1-headroom.json) /
[run 2](run-2-headroom.json): total CPU matches the recorded decision, connector
and publisher fractions, with at most 5.56e-17 floating-point rounding difference.
Both runs have zero selected reports whose input-lateness p99 reaches 42 ms.
The integrity tolerance only handles separate binary64 division/serialization;
the strict 70%/80% gate is unchanged.

## Run-2 deferral episode: post-result diagnostic

[Deferral reduction](deferrals.py), [run-1 receipt](run-1-deferrals.json),
[run-2 receipt](run-2-deferrals.json). All 126 increments are inside the canonical
measured interval, not a boundary-only effect. They appear in four consecutive
100-tick census intervals, from tick 301,000 to 301,400, at offsets
+745.848 to +749.411 seconds after the requested start:

| After tick | After offset s | Deferred increment |
| --- | ---: | ---: |
| 301100 | 746.820 | 29 |
| 301200 | 747.806 | 40 |
| 301300 | 748.828 | 47 |
| 301400 | 749.411 | 10 |

The largest bracketed busy-wall tick is 301,112: **23.640264 ms wall** versus
**5.188360 ms aligned world CPU** (2.943690 simulation + 2.244670 socket).
CPU tails therefore do not explain every wall delay. The bracket covers the
tick body/handoff, not every wait between ticks. The retained host counters
identify competing user CPU during this episode, as detailed below; they do
not identify the responsible process or each tick's blocking/preemption site.

Run 2 frame-gap p99 remains 27.952 ms, while frame-excess p99 is 883.503 ms
(maximum 1,142.652 ms). **Inference:** the latter is consistent with the
126 × 7 ms = **882 ms** simulation-clock deficit, rather than implying that
every frame gap is almost a second. Steal is reported, not an exclusion.
The failed gate remains failed regardless of this diagnostic; no interval is
trimmed, reclassified as infrastructure or replaced.

### Host CPU: measured and derived

The post-result [host reduction](stall.py), [run-1 receipt](run-1-stall.json)
and [run-2 receipt](run-2-stall.json) use the retained archives only. Sample
end offsets below are relative to each requested window start. Each row is
the delta from the preceding sample, not an instantaneous utilization reading.
All CPU values are seconds. The collector and HTTP observer are **one Python
process**, counted once. Only it and the server have interval task counters.

| Run 2 sample end s | Elapsed s | Server process | Simulation thread | Socket thread | Observer process |
| --- | ---: | ---: | ---: | ---: | ---: |
| 745.275 | 1.016 | 0.67 | 0.34 | 0.32 | 0.02 |
| 746.306 | 1.031 | 0.54 | 0.30 | 0.25 | 0.00 |
| 747.334 | 1.028 | 0.53 | 0.29 | 0.24 | 0.01 |
| 748.370 | 1.036 | 0.51 | 0.28 | 0.23 | 0.01 |
| 749.398 | 1.028 | 0.69 | 0.38 | 0.32 | 0.01 |
| 750.408 | 1.011 | 0.75 | 0.39 | 0.35 | 0.01 |

Server-main CPU is 0.00 s in every displayed interval. The receipts also retain
observer main/helper thread deltas; these generic thread names do not identify
which helper handled collection versus a particular HTTP request. Process and
thread sums can differ by counter rounding and sequential reads. No process
identity is published.

| Run 2 sample end s | Core user | Core system | Core softirq | Core idle | Other user CPU |
| --- | ---: | ---: | ---: | ---: | ---: |
| 745.275 | 0.36 | 0.32 | 0.11 | 0.19 | 0.04 |
| 746.306 | 0.70 | 0.23 | 0.11 | 0.00 | 0.41 |
| 747.334 | 0.61 | 0.31 | 0.10 | 0.00 | 0.30 |
| 748.370 | 0.69 | 0.23 | 0.11 | 0.00 | 0.41 |
| 749.398 | 0.66 | 0.26 | 0.11 | 0.00 | 0.31 |
| 750.408 | 0.36 | 0.28 | 0.12 | 0.22 | -0.01 |

Core nice, iowait, irq and steal deltas are **zero in every displayed interval**.
The one-core `/proc/stat` counters advance in 10 ms quanta. These sequential
snapshots cannot rule out shorter events; the -10 ms residual illustrates their
rounding/edge error. Other user CPU is core user minus server user minus
observer user. Use user only: task system accounting can also charge softirq
execution, so subtracting total task CPU from core user+system would misattribute
it. IRQ/softirq are not added to aligned world CPU.

For the four consecutive sample ends in [746, 750) s:

| Run | Actual sampled span s | Server CPU s | Observer CPU s | Other user CPU s | Core idle s | Core user / system / softirq s |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 745.760–749.799 | 2.85 | 0.04 | 0.03 | 1.04 | 1.78 / 0.81 / 0.44 |
| 2 | 745.275–749.398 | 2.27 | 0.03 | **1.43** | **0.00** | 2.66 / 1.03 / 0.43 |

**Inference:** non-server, non-observer user work competed for the sole vCPU
during run 2's slowdown. About 1.43 CPU-seconds over 4.123 wall seconds is far
beyond counter-edge error; server CPU falls and idle disappears. Softirq is
similar to run 1, and neither observer CPU nor reported steal spikes. This
supports CPU contention as the episode's explanation, rather than attributing
it to simulation CPU, but does not prove every delayed tick was preempted.
Run 1's equivalent relative-time samples have idle headroom and no deferrals.
The +746 s references are 16:23:09.855 UTC for run 1 and 17:19:36.187 UTC for
run 2 on October 30. No retained scheduler trace, continuous all-process CPU,
context-switch counters, runnable-task count, run-queue length or per-task I/O
counters can identify the competitor or its wakeup cause.

### Host image services: measured presence, unknown trigger

Both boundary `ps` inventories show `snapd`, `amazon-ssm-agent`,
`systemd-journald`, `cron`, `rsyslogd` and `unattended-upgrade-shutdown` in both
runs. The last is a shutdown waiter, not evidence that an upgrade ran. `fwupd`
is absent at run 2's start and present at its end (lifetime `ps` CPU 0.1%);
it is absent at both run-1 boundaries. **Inference:** firmware-refresh work
is a plausible image-service lead, not an identified culprit. Lifetime CPU and
presence thirty minutes apart cannot place it at the stall or account for the
1.43 CPU-seconds. A short-lived process could also disappear before the end.

No apt-daily job, active unattended-upgrade job, fstrim, man-db or cloud-init
command is observed in these two boundary inventories. Their timer/service
enablement and execution **remain unknown**: no host journal, systemd unit
status or `systemctl list-timers` output was retained. Neither run can be
matched to a timer firing at the event's wall-clock time. Boundary process
presence is not systemd active/enabled state.

**Source-verified:** the retained provisioning/launch scripts do not disable or
mask these services or timers. Host user-data installs the shutdown deadline
and study directory only. The dependency-install/build script runs on the
generator, not the host; it is not a host apt job during the episode. The
verified 90-minute self-termination deadline is well after the episode, not
an observed shutdown trigger. No claim about default-image timer schedules
substitutes for a missing journal.

### Blocking I/O: source-verified path and limits

The [pinned runtime server](https://github.com/NikitaMGrimm/slither-sim/blob/388be59aebf3108f5c510aa7269c08b67851b202/src/client/server.rs)
is `388be59a`, not the newer runtime in the reporting checkout. Its ordinary
socket I/O is nonblocking. Instrumentation adds
synchronous `eprintln!` calls: simulation `CAPACITY` and `CENSUS` every 100 ticks,
and socket `SOCKET` reports about once per second. Main-thread `--stats 1`
also writes stderr synchronously. These calls can wait on the stderr lock and
on regular-file writes. The observer launches the server with stdout and
stderr redirected to **the regular file `host.log`**, not a pipe; it tails that
file and serves chunks over HTTP. Generator `host-stream.log` is a downstream
copy. A slow HTTP reader cannot fill a server stdout pipe, because none exists.

The [linked observer](../2026-10-29-c7a-mature-capacity/host_agent.py)
separately writes and flushes its telemetry file about once per
second. A slow EBS-backed write, dirty-page throttling or stderr lock contention
is possible in principle; there is no retained syscall-duration/block-I/O trace
to test it directly. Sampled iowait is zero throughout the event, which supplies
no positive evidence for storage blocking but cannot exclude it while competing
work keeps the core runnable. `finish_tick` samples busy wall before its own
CAPACITY emission and before CENSUS; those writes can delay the next scheduler
opportunity outside that tick's busy-wall bracket. No inference that the
23.640 ms tick itself was a log write is justified. The measured extra user CPU
is the stronger lead; a specific logging stall remains **unknown**.

### Deferral rule: repeated delays, not one 18 ms stall

**Source-verified:** in `Simulation::due_ticks` at runtime `388be59a`,
`scheduled = floor(elapsed / 0.007)` and
`backlog = scheduled - (completed + deferred)`, saturating at zero. The limit
is `ceil(0.25 / 0.007) = 36` ticks. Only backlog **greater than 36** defers the
excess; the earliest such integer backlog is 37 scheduled ticks (259 ms).
Each outer pass then executes at most eight catch-up ticks. Eight is a burst
limit, not the deferral threshold. Deferral shifts world/link time behind wall
time instead of skipping physics steps. A solitary extra 18 ms, starting on
schedule, cannot cross this threshold or explain 126 deferred ticks.

The retained 100-tick blocks show a sequence of long ticks and loss of progress:

| Ticks | World CPU sum ms | Bracketed wall sum ms | Ticks with wall >10 ms | Max wall ms |
| --- | ---: | ---: | ---: | ---: |
| 300901–301000 | 479.306 | 865.671 | 29 | 14.977 |
| 301001–301100 | 506.418 | 965.544 | 40 | 13.690 |
| 301101–301200 | 503.068 | 982.035 | 48 | 23.640 |
| 301201–301300 | 501.181 | 1011.673 | 52 | 16.124 |
| 301301–301400 | 478.098 | 581.510 | 11 | 13.406 |
| 301401–301500 | 476.281 | 391.096 | 0 | 8.978 |

The four blocks containing deferral increments include **151 ticks over 10 ms**;
slowdown starts in the preceding block, before backlog reaches the cap. For
comparison, all seven retained run-1 blocks ending at 300900 through 301500
have zero ticks over 10 ms. Run 2's four census intervals take 3.563 wall
seconds to complete 400 ticks (2.800 simulation seconds) and defer another
0.882 seconds, drawing down about 0.119 seconds of previously accumulated
backlog by their end. This is consistent with repeated contention building
backlog, repeated clipping at the cap, and subsequent catch-up. Exact per-call
defer times and off-CPU sites are not recorded; the 29/40/47/10 counts locate
increments to census intervals only. The largest individual tick is not the
whole episode.

### Implications for a separately planned v4 (not run)

The data point to host-image/background contention but do not name its source.
Before another acceptance attempt, v4 would retain full unit/timer inventories
and a timestamped journal, and sample **all** processes' cumulative CPU with
PID/start-time identity, plus `/proc/stat` context switches/runnable tasks,
per-thread scheduling/wait counters and I/O counters. A bounded scheduler trace
triggered by backlog would distinguish preemption from blocking; collector CPU
would still count on the host core.

An explicitly frozen benchmark-image policy would quiesce optional maintenance:
mask present `apt-daily.timer`, `apt-daily-upgrade.timer`, `fwupd-refresh.timer`,
`fstrim.timer` and `man-db.timer`, stop/wait for their jobs, and stop optional
snap refresh/SSM work for the window, recording actual units and states rather
than assuming those names exist. Keep required networking/timekeeping, safety
deadlines and ordinary logging services, and preserve their CPU accounting.
Because service policy changes the tested environment, disclose it before
timing; do not retroactively relabel v3 as an infrastructure replacement.
This is preventive control, not proof any named service caused v3's failure.

To remove the possible harness I/O path, v4 would capture research telemetry in
a preallocated, memory-backed buffer sized for the full window, count its CPU
and memory, and persist it after timing, with overflow failing collection.
Keep timestamps, observations and gates unchanged and explicitly verify the
new collection path before freezing it. A bounded/drop policy must never
silently omit evidence or introduce a producer-blocking pipe. No such image,
instrumentation change, AWS resource or v4 run is created by this diagnosis.

## Verification and reproduction

AWS preflight: portable build, three actual-module runtime tests including live helper clocks before jobs finish, strict summed-headroom boundaries, inherited ladder/gate/token/nearest-rank controls, seven docs tests and repository boundary. Synchronous dispatch, omitted helper CPU and decision-only headroom mutations each failed as intended. The one bounded target was removed; run 2 reuses the verified first binaries. No local Rust work or access to the private bench host; no independent review.

Stage the pinned linked files and commands in [PLAN.md](PLAN.md). After each complete archive, run the v2 selector with corrected v1 helpers, retaining each result separately. Require exact reduce_window replay against the retained online window before selection. Then run the linked v1 `assemble.py selected-results.json RUN1_SELECTED.json RUN2_SELECTED.json` and set only `platform.market` to `On-demand` for this market; do not change run data. The linked v1 `tables.py selected-results.json` recomputes the four CPU/cadence tables. The gate table reads `result.gates`; the headroom table shows the min/max of the six `client.workerCpuFraction` medians/p99 values multiplied by 100, plus `client.worstInputP99Ms`. This study adds no duplicate harness. Full raw logs/binaries/cloud inventories are local-only outside Git; selected archive digests do not make them public.

**Post-result review correction:** [check_replay.py](check_replay.py) compares the
entire freshly reduced result with the retained online result, without dropping
or rounding any field. Run it before selection when reproducing; for this study
the new explicit check was run after publication on both unchanged archives.
[Run 1](run-1-replay.json) and [run 2](run-2-replay.json) match completely and
detect deliberate p50/p99/p99.9/max changes. No measured result changes.

```sh
python3 check_replay.py PINNED_STAGED_HARNESS PRIVATE_RUN_ARCHIVE.tar.gz run-replay.json
```

The [preflight binding](preflight.json) records tested source commit, aggregate
instrumented source identity, host/client binary hashes and retained proof
archive digest/access. [Attempts](ATTEMPTS.md#post-result-review-verification-no-new-timing)
states how the pre-start source matches the committed modules and explicitly
marks the unretained test-executable hashes unavailable.

For each privately retained extracted run, the two small post-result reductions
recompute the headroom integrity receipt and deferral table without changing
selection, gates or measured values:

```sh
python3 check_reports.py PRIVATE_EXTRACTED_RUN run-headroom.json
python3 deferrals.py PRIVATE_EXTRACTED_RUN run-deferrals.json
```

The added post-result host reductions run directly on the unchanged private
archives (run 1 and run 2 separately), in seconds with standard-library Python
3.11+. They publish selected counters and safe service labels only:

```sh
python3 stall.py PRIVATE_RUN_ARCHIVE.tar.gz run-stall.json
```

Both receipts bind to the existing archive SHA-256 values. This follow-up uses
short local retained-data reductions and documentation checks only; no Rust
build/test, guest access, AWS call/spend or new timing. The frozen plan, measured
selected results, failure and cost/cleanup receipts are unchanged.

## Cost and cleanup

The [cost receipt](cost.json) bounds both launch-to-verified-cleanup on-demand
pairs at **$1.141553 compute**, using frozen rate caps and the selected role
lifetimes. Including the full **$0.40** noncompute reserve gives
**$1.541553 < $3**. This is a conservative bound, not a final invoice. The
predeclared full-three-pair fail-safe bound remains $2.976; only two pairs were
used, with no infrastructure replacement or deadline extension.

The [final cleanup audit](cleanup.json) covers **all 17 enabled regions**, none
unavailable. No task instances, active requests, volumes, key or security group
remain; unrelated resources were preserved. Both raw archives were verified
before teardown. Seven final documentation tests passed on AWS after the full
results and diagnostic receipts were staged; final cost/cleanup values and this
prose were filled after teardown without changing link targets or measured data.

## Limits

Two fresh pairs, one seed, one A4 policy, protocol 19 and default official density, with naturally evolving worlds. Results cover this received-frame socket workload; they do not establish sustained 500-player capacity, arbitrary human behavior, Internet/TLS/slow-reader performance, renderer performance, legacy mixes, ecological stationarity or a maximum capacity. Earlier failed v1 and interrupted v2 remain separate. No tuning, discarded complete run or replacement was used.
