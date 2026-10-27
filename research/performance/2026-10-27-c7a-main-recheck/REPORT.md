# Same-session c7a main recheck, 2026-10-27

**Current main official-500 misses the unchanged delivery target**; the later diagnostic is reported separately.
Main `4532a76f22958ae3abcca801e90edd30ed9f466f` and control `65ff713731cbd436bf5c19a25705fa77b0ebedcc`
ran in one Frankfurt Spot session on the same c7a.medium core. The [plan](PLAN.md) was frozen before collection.
Synthetic protocol-19 players use real sockets; no host bots, TLS or outside game servers.
[Selected results](selected-results.json) include every hold and smoke; raw evidence is local-only.

## Results

Rows are in collection order, one 120 s hold each. Grade: pooled delivery excess p99 <=10 ms and zero newly
deferred ticks, with valid traffic/occupancy. Tick busy p99/max remain diagnostics, not another threshold.

| Arm | Players | Grade | Excess p99 ms | CPU ms/tick | Core % | Deferred | Peak RSS MiB |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| Control | 500 | miss | 40.711 | 4.749 | 67.8 | 0 | 49.88 |
| Main | 500 | miss | 14.837 | 4.286 | 61.2 | 0 | 51.09 |
| Main nice +5 | 500 | miss | 12.453 | 4.269 | 61.0 | 0 | 51.23 |
| Main | 375 | pass | 4.358 | 3.209 | 45.9 | 0 | 36.79 |
| Control | 375 | pass | 5.726 | 3.496 | 50.0 | 0 | 36.89 |

CPU splits are percentages of one core. User/system use the same boundary-aligned Linux counters as total
CPU. Total sim/socket omit the diagnostic CLI thread. Process/task user/system counters are reported
directly, without reconciling their disagreement. Busy spans include preemption.

| Arm / players | Process user/system % | Sim user/system % | Socket user/system % | Busy p99/max ms | Ticks/s |
| --- | --- | --- | --- | --- | ---: |
| Control / 500 | 31.9 / 36.0 | 29.9 / 1.7 | 4.7 / 31.1 | 9.403 / 18.250 | 142.849 |
| Main / 500 | 26.0 / 35.2 | 24.7 / 1.2 | 5.5 / 29.4 | 7.533 / 17.028 | 142.851 |
| Main nice +5 / 500 | 31.5 / 29.5 | 24.7 / 1.1 | 5.2 / 29.5 | 5.206 / 12.442 | 142.859 |
| Main / 375 | 21.7 / 24.1 | 18.5 / 0.7 | 4.3 / 21.9 | 5.425 / 15.434 | 142.858 |
| Control / 375 | 29.0 / 20.9 | 22.8 / 0.6 | 3.2 / 22.9 | 6.236 / 11.377 | 142.863 |

Process versus summed-task user/system counters differ by up to 4.32 percentage points; total
CPU differs by at most 0.025 points. These are observed accounting splits, not isolated kernel costs.

Exact p50/max excess, start-gap ranks, operator load and population/traffic counters are in JSON; unedited
`--stats 5` lines remain in the raw archive. Operator simulation load measures busy wall time, not thread CPU.
Mean/final playing equaled each requested count (500 or 375). No generator errors.
The predeclared nice +5 diagnostic missed (12.453 ms excess p99); socket priority was verified before warmup ended.
No second official-500 pair ran because the first main hold missed. No runtime/priority setting is adopted.

At 500 players, control to main CPU was 4.749 to 4.286 ms/tick
(-9.8%); excess p99 was 40.711 to 14.837 ms.

## Comparison and limits

The same-session control is the requested #211 revision, rather than a historical timing substituted into
the comparison. [#211](../2026-10-27-epyc-index-recheck/REPORT.md) measured control official-500 at 58.384 ms
excess p99 and 4.662 ms CPU/tick. Its official-375 hold lasted 300 s; these 375 holds last 120 s.
Both revisions passed 375 in this session; that pass alone cannot be attributed to the new optimizations.
[#214](../2026-10-27-host-kernel-tails/REPORT.md) identified scheduling and kernel-network tails on earlier
main. This study records CPU splits but has no new kernel profile or isolated attribution to any optimization.
Average CPU below one core does not establish timely delivery.

One seed and short conditional repetitions provide bounded observations, not a confidence interval or
500-human-player guarantee. Join order/ecology can diverge between revisions. These weaving/food-avoidance
clients do not model growing snakes, browsers, legacy clients, TLS, Internet paths, slow readers or long uptime.
The retail target CPU was not measured. No hold was discarded/repeated for speed beyond the frozen rule.
Generator total CPU peaked at 0.914 cores; busiest pinned shard
averaged 12.25% of its core. Host steal was at most 0.0081%.

## Reproduction and verification

- Reuse [#201 instrumentation/reducer](../2026-10-27-epyc-host-recheck/REPORT.md) and
  [#178 harness.patch](../2026-10-26-epyc-host/harness.patch) unchanged. Apply `instrument.py` to each pinned
  isolated snapshot, then call `worker.py`'s `patch` on its generated `scripts/run_trial.py` for CPU counters
  and optional `STUDY_SOCKET_NICE=5`. [cpu_split.py](cpu_split.py) reduces those additional counters.
- Rust 1.99 locked release, thin LTO, one codegen unit, line-table debug info, CARGO_BUILD_JOBS=1; fresh
  per-arm targets on c7a.4xlarge. The exact frozen #178 load executable was reused; hashes are in JSON.
- Eight pinned generator shards, seed 7, 100 ms controls, 250 ms death rejoin, 15 s warmup after a common
  epoch eight seconds ahead, default official preset, fresh world each hold. Host threads verified on CPU [0].
  Two eight-client/eight-second smokes with three-second warmup precede qualification and do not affect grades.
- Run the generated worker's host/load roles on separate machines with `--preset official --count N
  --warmup 15 --hold 120 --epoch E --bin DIR`; reduce with unchanged #201 `analyze.py HOST LOAD N OUT`.
  Excess subtracts cumulative server-header time and each connection's minimum inside the hold; exact nearest
  ranks. Tick starts use a half-open hold. RSS is VmHWM including startup/warmup and buffered diagnostics.

All holds were re-reduced locally from downloaded receipts; timing ranks, grades and counters matched exactly.
Only the unordered per-shard service-list order was canonicalized across Linux/Windows filesystems.
Source/binary/worker identity, population, errors, CPU splits, affinity and priority audits passed.
CPU-boundary, existing tick-window and seven documentation tests passed. Full Rust checks were not rerun
for this research-only change; both release binaries were built/exercised on native Linux. Local work used
at most two logical CPUs. A private draft helper named `select.py` briefly shadowed Python's stdlib import;
renaming it fixed local progress inspection before holds, without changing the running controller or workers.

| Local-only archive (no public download) | SHA-256 |
| --- | --- |
| c7a.medium | `4899cd63b79fe85d3a75c3d519de10c82f6d1f37734fe8c4ec426a735a712282` |
| c7a.4xlarge | `e3933d53c6d7a4ef57b815e0aa576ce4b01aedfe0c3385e2eaaebdf92356f196` |
| session-record | `4a7131b8ce63f66c91da6930be6868567b296cbb13ee1c3d56ecb003a7f3b08e` |

## Cost and cleanup

Estimated **$0.235**, below $3: $0.172 compute, $0.004 IPv4,
$0.003 storage, $0.046 egress and $0.01 minor-transfer reserve; not an invoice.
Compute integrates EC2 Spot price history over observed lifetimes; storage/egress allowances are $0.12/GB-month
and $0.12/GB. Every instance had a verified 120-minute timer, terminate-on-shutdown and delete-on-terminate root.
Final all-region scan at 2026-10-27T22:39:09.567985+00:00 attempted 34 commercial regions.
All 17 enabled regions had zero nonterminated instances, volumes, open/active Spot requests,
EIPs and created groups/keys. Other 17 regions were disabled/API-inaccessible; their inventories are unverified.
All created resources were deleted; no credentials or regions were enabled/obtained.
The first final inventory stopped on a regional API error; its partial scan is retained. STS still verified
the existing login; a read-only repeat completed. No new resource, measurement or credential change followed.
