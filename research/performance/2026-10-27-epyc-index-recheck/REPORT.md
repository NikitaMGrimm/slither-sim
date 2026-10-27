# One-core host after indexed pickup and delivery, 2026-10-27

**c7a official-500 misses the unchanged delivery target in its 120-second hold.**
Main `65ff713` reuses [#201](../2026-10-27-epyc-host-recheck/REPORT.md)'s method and exact load executable.
Synthetic protocol-19 socket players, zero host bots; no outside game server. The [plan](PLAN.md) was frozen
before collection; [selected results](selected-results.json) retain every qualification hold. Raw evidence is local-only.

## Results

One hold per row. Grade: pooled frame-excess p99 <=10 ms and zero new deferred ticks, with valid traffic and
occupancy. Tick busy p99/max are diagnostics, with no additional gate. CPU percentages are shares of one core;
sim/socket use Linux thread CPU counters, leaving the small CLI diagnostic-thread remainder in total CPU.

| Core / workload | Hold s | Grade | Excess p99 ms | CPU ms/tick | Core % | Sim/socket % | Deferred | Peak RSS MiB |
| --- | ---: | --- | ---: | ---: | ---: | --- | ---: | ---: |
| c7a / official-500 | 120 | miss | 58.384 | 4.662 | 66.6 | 32.0 / 34.2 | 0 | 50.83 |
| c7a / official-375 | 300 | miss | 13.989 | 3.772 | 53.9 | 27.4 / 26.1 | 0 | 43.46 |
| c7a / battledome-80 | 300 | pass | 5.003 | 0.990 | 14.1 | 7.5 / 6.2 | 0 | 12.28 |
| c8a / official-500 | 120 | pass | 3.120 | 2.155 | 30.8 | 19.7 / 10.9 | 0 | 50.99 |

| Core / workload | Ticks/s | Excess p50/max ms | Tick busy p99/max ms | Start-gap p99/max ms | Stats load % | Playing mean/final |
| --- | ---: | --- | --- | --- | ---: | ---: |
| c7a / official-500 | 142.849 | 1.520 / 157.975 | 9.824 / 16.188 | 9.826 / 17.346 | 46.1 | 500.000 / 500 |
| c7a / official-375 | 142.857 | 1.551 / 87.218 | 7.791 / 15.566 | 7.974 / 15.566 | 40.7 | 375.000 / 375 |
| c7a / battledome-80 | 142.857 | 0.721 / 12.555 | 4.921 / 10.621 | 7.051 / 10.621 | 7.5 | 79.284 / 80 |
| c8a / official-500 | 142.859 | 1.006 / 9.877 | 3.938 / 8.735 | 7.038 / 8.735 | 23.5 | 500.000 / 500 |

Stats load is the operator's `--stats 5` simulation busy-wall readout; it differs from simulation-thread CPU.

Battledome-80 initial wait p50/p95/max: 3580.925/12895.452/12928.234 ms (includes warmup).
Respawn wait: 5.322/468.016/1746.054 ms over 666 completed requests; 0 pending at end.
Wait starts at the first login across retries; it excludes the 250 ms death rejoin delay. Successful waits
exclude pending requests. Death/admission counters and own-update gaps are retained in JSON.

The first generator was terminated by AWS Spot during the official-375 hold; unretrieved client receipts
were lost. Its retained official-500 summary also missed (53.503 ms excess p99, 4.674 ms CPU/tick, zero
deferrals). The table uses one [predeclared recovery](RECOVERY.md) with the identical downloaded binary;
the partial miss and server evidence remain retained. Cost includes both attempts. No further recovery ran.

## Comparison with #201 and limits

| Core / workload | #201 → current grade | Excess p99 ms | CPU ms/tick | Deferred |
| --- | --- | --- | --- | --- |
| c7a / official-500 | miss → miss | 150.570 → 58.384 | 6.059 → 4.662 | 0 → 0 |
| c7a / official-375 | miss → miss | 32.111 → 13.989 | 4.951 → 3.772 | 0 → 0 |
| c7a / battledome-80 | pass → pass | 5.405 → 5.003 | 1.430 → 0.990 | 0 → 0 |
| c8a / official-500 | pass → pass | 3.612 → 3.120 | 2.800 → 2.155 | 0 → 0 |

#201's `ca7ac2f` rows are historical, not fresh paired repetitions. One seed and one hold per row provide no
confidence interval or isolated attribution to #204/#205/#207/#208/#209. Offline instruction reductions are
not live capacity estimates. These weaving/food-avoidance clients do not model human browsers, larger growing
snakes, TLS, Internet paths, legacy clients, slow readers or long uptime. Join order and ecology can diverge.
The retail EPYC 9645 was not measured; c7a is the requested cloud proxy, with c8a an architectural control.
No hold was repeated or excluded for speed. Official-500 is a two-minute sample, not five-minute confirmation.

All required holds decoded traffic without generator errors. Generator CPU reached 0.917 cores total;
the busiest pinned shard averaged 12.30% of its core. Host steal was at most 0.0032%.

## Method, verification and evidence

- Same #201 frozen #178 load executable, eight pinned shards, seed 7, default presets, 100 ms controls, no
  boost/TLS/added latency, 15-second warmup after a common epoch eight seconds ahead. Each hold starts a fresh
  world; all host threads were verified on CPU [0]. Load runs separately on c7a.4xlarge. One official eight-client
  eight-second smoke precedes the requested schedule and is excluded from grades.
- Build pinned `65ff713` in an isolated snapshot on the generator: Rust 1.99, locked release, thin LTO, one
  codegen unit, line-table debug info, CARGO_BUILD_JOBS=1 and a fresh target. The same hashed binary runs on
  both cores. Hardware and all source/build/harness pins are in JSON. Local work is limited to two logical CPUs.
- Reuse [instrument.py](../2026-10-27-epyc-host-recheck/instrument.py), the #178 harness patch and
  [analyze.py](../2026-10-27-epyc-host-recheck/analyze.py) unchanged. Tick busy spans command intake through
  handover, includes preemption and is buffered until shutdown. Clip tick starts to the half-open hold.
  RSS is process VmHWM including startup/warmup and diagnostic storage. CPU is user+system / executed ticks.
- Frame excess is receipt time minus cumulative server-header time minus each connection's minimum inside the
  hold: fixed path offsets cancel; world slowdown remains. All quantiles are exact nearest rank. Build the
  instrumented snapshot; run patched run_trial.py host/load roles on separate machines with --preset, --count,
  --warmup 15, --hold, --bin and a common --epoch; reduce via `analyze.py HOST LOAD N OUT`.

All four holds were independently re-reduced locally from downloaded raw data; grades, counters and timing
ranks matched. Tick-window regression and documentation checks passed. Full Rust checks were not run for
this research/docs-only change; the release binary was built and exercised on native Linux. Full logs,
receipts, tick samples, snapshots, binaries and private cloud metadata are outside Git, not publicly downloadable.

| Local-only archive | SHA-256 |
| --- | --- |
| c7a.4xlarge | `bbef44754e7725711233e115d77be3707c500b39285e2007642846b2830e66fd` |
| c7a.medium | `5792e515ffff918a5d1b1723a8e71e72b3ccf81b82e5a2556d9126912b6a37f0` |
| c8a.medium | `9d780923b43e168f4fc60b365bc6e22f8fb32d270ba83db8295ff2352325d4bc` |
| session-record | `327f9dc3aeeafb7cb4a487329cfe9e6d4bcc072c77dcd9bd988b6cb67a14d262` |
| interrupted c7a.medium | `4b17c8f66747a58a97b5cd275895a4244310d2d486fcd14ea10090a67bedb104` |

## Cost and cleanup

Estimated **$0.389**, below $3: $0.282 Spot compute, $0.006 IPv4,
$0.005 storage and $0.086 egress allowances; not an invoice.
Includes a $0.01 minor-transfer reserve. Basis: launch quotes × observed lifetimes; conservative $0.12/GB-month disks and $0.12/GB egress, no credits.
Every instance had delete-on-terminate root storage, terminate-on-shutdown and a verified 120-minute timer.
Final all-region scan at 2026-10-27T13:53:56.764377+00:00 attempted 34 commercial regions.
All 17 enabled regions had zero non-terminated instances, volumes, open/active Spot requests,
EIPs and created groups/keys. The other 17 regions were disabled/API-inaccessible; their inventories cannot
be asserted without enabling them. No credentials or regions were enabled/obtained. Created resources were deleted.
