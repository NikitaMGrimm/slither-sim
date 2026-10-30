# Official-500 on-demand acceptance v3

**The two-run acceptance fails.** Run 1 passes with zero deferred ticks; run 2 records **126 deferred ticks** and fails that gate. Both full thirty-minute windows are retained separately. All other gates pass in both runs. No replacement, tuning or retry is used to rescue the completed failure.

World CPU p99 and client frame tails are report-only. Root README and changelog are unchanged; there is no 500-player acceptance claim.

[Frozen plan](PLAN.md), [attempts](ATTEMPTS.md), [selected results](selected-results.json), [preflight](preflight.json), [verification](verification.json), [raw bindings](raw-bindings.json), [cost](cost.json), [cleanup](cleanup.json).

## Method

Runtime `388be59aebf3108f5c510aa7269c08b67851b202`, Rust 1.99, locked portable release `-C target-cpu=x86-64`, thin LTO, one codegen unit. All host/observer threads share the c7a.medium sole vCPU. A separate same-zone c8a.2xlarge runs six full received-frame A4 protocol-19 workers on CPUs 0–5 and control on CPU 7. Both host and generator are on-demand. Identical host/client binaries across fresh pairs; the host binary is identical to v2.

Reuse [the c7a ladder](../2026-10-29-c7a-mature-capacity/REPRODUCE.md) and [#256 class](../2026-10-28-capacity-checkpoint/PLAN.md) through the pinned v2 references in the plan. Natural tick-zero warmup with 100 resident/400 rotating clients, no forced churn after tick 180,000, three bounded entry endpoints at/after 193,000, ten seconds settling and a full thirty-minute window. Final class is living >=490, mean >=45 and p90 >=156, without upper bounds. No checkpoint, thin clients, human player, artificial latency or TLS. Keep all drift.

The plan and tested harness were frozen in `bbc7d18c` before either world. Per-worker CPU is the sum of live decision + connector + publisher CPU clocks, including in-flight jobs and thread overhead, over the same report interval. Each component is in the raw CLIENT reports; the existing strict median <70% / p99 <80% gate consumes the sum. Reconnects/close handshakes and report serialization remain off decision loops.

Both online and offline CPU/frame report tails use nearest rank from startup, using v1 maintainer helpers at `fff66d0a6e26f9552be2ea3ffc4c6c32c106a354`. Full online/offline replay, including all tails/maxima, matched for each run before selection. Preserve the unchanged ladder headroom quantile calculation. World CPU is aligned simulation + socket user/kernel per tick, over complete interior 100-tick blocks; never add component quantiles. Maintenance uses the unchanged absolute 504 ms epoch. Deferrals use the exterior covering census bracket and final class the first census at/after the end. Actual client histogram intervals and boundary offsets remain selected.

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
tick body/handoff, not every wait between ticks. No retained scheduler/kernel/
storage/hypervisor trace identifies the root cause; none is asserted here.

Run 2 frame-gap p99 remains 27.952 ms, while frame-excess p99 is 883.503 ms
(maximum 1,142.652 ms). **Inference:** the latter is consistent with the
126 × 7 ms = **882 ms** simulation-clock deficit, rather than implying that
every frame gap is almost a second. Steal is reported, not an exclusion.
The failed gate remains failed regardless of this diagnostic; no interval is
trimmed, reclassified as infrastructure or replaced.

## Verification and reproduction

AWS preflight: portable build, three actual-module runtime tests including live helper clocks before jobs finish, strict summed-headroom boundaries, inherited ladder/gate/token/nearest-rank controls, seven docs tests and repository boundary. Synchronous dispatch, omitted helper CPU and decision-only headroom mutations each failed as intended. The one bounded target was removed; run 2 reuses the verified first binaries. No local Rust work or instabox access; no independent review.

Stage the pinned linked files and commands in [PLAN.md](PLAN.md). After each complete archive, run the v2 selector with corrected v1 helpers, retaining each result separately. Require exact reduce_window replay against the retained online window before selection. Then run the linked v1 `assemble.py selected-results.json RUN1_SELECTED.json RUN2_SELECTED.json` and set only `platform.market` to `On-demand` for this market; do not change run data. The linked v1 `tables.py selected-results.json` recomputes the four CPU/cadence tables. The gate table reads `result.gates`; the headroom table shows the min/max of the six `client.workerCpuFraction` medians/p99 values multiplied by 100, plus `client.worstInputP99Ms`. This study adds no duplicate harness. Full raw logs/binaries/cloud inventories are local-only outside Git; selected archive digests do not make them public.

For each privately retained extracted run, the two small post-result reductions
recompute the headroom integrity receipt and deferral table without changing
selection, gates or measured values:

```sh
python3 check_reports.py PRIVATE_EXTRACTED_RUN run-headroom.json
python3 deferrals.py PRIVATE_EXTRACTED_RUN run-deferrals.json
```

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
