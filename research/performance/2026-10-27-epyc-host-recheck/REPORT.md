# One-core host after the tail fixes, 2026-10-27

**Current main does not confirm 500 official clients; the highest tested passing c7a count is 250.** Synthetic
protocol-19 socket players, zero server bots; no outside game server. The [plan](PLAN.md) was frozen before
collection. [Selected results](selected-results.json) retain every hold; full raw artifacts are local-only
outside Git.

## Same-session results

Grade is unchanged from [#178](../2026-10-26-epyc-host/REPORT.md): pooled frame-excess p99 <=10 ms and zero new
deferred ticks, with valid traffic and occupancy. Tick busy p99/max is a new diagnostic, with no additional
grade threshold. One hold per row; B=baseline `48977ec`, M=main `ca7ac2f`.

| Core | Arm / preset | Clients / hold s | CPU ms/tick | Ticks/s | Deferred | Excess p50/p99/max ms | Tick busy p99/max ms | Playing mean/final | Grade |
| --- | --- | ---: | ---: | ---: | ---: | --- | --- | ---: | --- |
| c7a | B / official | 500 / 120 | 6.209 | 142.616 | 36 | 8.092 / 425.093 / 511.142 | 14.091 / 84.279 | 499.981 / 500 | fail |
| c7a | M / official | 500 / 120 | 6.059 | 142.867 | 0 | 2.246 / 150.570 / 212.187 | 11.817 / 23.139 | 499.992 / 500 | fail |
| c7a | M / official | 375 / 300 | 4.951 | 142.837 | 0 | 1.980 / 32.111 / 107.065 | 9.584 / 19.293 | 375.000 / 375 | fail |
| c7a | M / official | 250 / 300 | 3.444 | 142.858 | 0 | 1.367 / 4.817 / 42.522 | 5.684 / 15.851 | 250.000 / 250 | pass |
| c7a | B / battledome | 80 / 300 | 9.043 | 103.030 | 11948 | 68372.458 / 83710.070 / 83907.774 | 33.432 / 108.820 | 79.999 / 80 | fail |
| c7a | M / battledome | 80 / 300 | 1.430 | 142.857 | 0 | 0.733 / 5.405 / 17.329 | 5.289 / 12.190 | 79.146 / 79 | pass |
| c8a | M / official | 500 / 120 | 2.800 | 142.851 | 0 | 1.076 / 3.612 / 15.770 | 5.019 / 10.897 | 500.000 / 500 | pass |
| c8a | B / official | 500 / 120 | 3.016 | 142.859 | 0 | 1.417 / 6.543 / 31.437 | 4.530 / 26.675 | 500.000 / 500 | pass |

The descending ladder stops at **250**, passing for 300 s; no claim is made between tested counts.

| Core / arm | Stats load mean % | Peak RSS MiB | Tick start-gap p99/max ms | Own-update gap p99/max ms |
| --- | ---: | ---: | --- | --- |
| c7a / B / official-500 (120 s) | 72.6 | 55.43 | 14.091 / 84.307 | 67.802 / 225.487 |
| c7a / M / official-500 (120 s) | 59.7 | 48.77 | 11.834 / 23.139 | 56.187 / 147.186 |
| c7a / M / official-375 (300 s) | 46.0 | 41.04 | 9.591 / 19.293 | 49.718 / 81.615 |
| c7a / M / official-250 (300 s) | 30.2 | 28.30 | 7.205 / 15.851 | 43.603 / 74.162 |
| c7a / B / battledome-80 (300 s) | 96.3 | 42.59 | 33.433 / 108.820 | 132.293 / 316.595 |
| c7a / M / battledome-80 (300 s) | 9.6 | 12.12 | 7.050 / 12.191 | 43.403 / 107.993 |
| c8a / M / official-500 (120 s) | 31.7 | 48.48 | 7.107 / 10.897 | 43.511 / 55.981 |
| c8a / B / official-500 (120 s) | 29.7 | 54.67 | 7.089 / 26.675 | 44.414 / 68.015 |

| c7a Battledome-80 arm | Initial wait p50/p95/max ms | Respawn p50/p95/max ms | Respawns | Pending / oldest age ms |
| --- | --- | --- | ---: | ---: |
| B | 3251.451 / 3483.564 / 3510.022 | 24.383 / 24.383 / 24.383 | 1 | 0 / 0.000 |
| M | 3677.334 / 10377.966 / 11427.620 | 5.325 / 822.724 / 2020.275 | 684 | 0 / 0.000 |

Initial waits include warmup; respawns complete inside the hold. The first login starts a wait across retries;
it excludes the 250 ms death rejoin delay. Pending requests are right-censored and excluded from success
quantiles. All ranks are exact nearest rank.

## Before/after and limits

| Core / workload | #178 p99 ms / deferred | Fresh baseline p99 ms / deferred | Main p99 ms / deferred | #178 → main CPU ms/tick |
| --- | --- | --- | --- | --- |
| c7a / official-250 | 10.245 / 0 | unrun | 4.817 / 0 | 3.626 → 3.444 |
| c7a / official-500 | 147.584 / 0 | 425.093 / 36 | 150.570 / 0 | 6.223 → 6.059 |
| c7a / battledome-80 | 65941.165 / 9402 | 83710.070 / 11948 | 5.405 / 0 | 7.842 → 1.430 |
| c8a / official-500 | 7.305 / 0 | 6.543 / 0 | 3.612 / 0 | 3.019 → 2.800 |

Historical comparisons are descriptive. Fresh baseline tails differ from #178; same-session rows compare the
binaries, without treating historical holds as matched repetitions. The 250/375 ladder holds last 300 s;
official-500 comparison holds last 120 s. Battledome baseline/main also recorded 1/684 deaths and respawns, with
different evolving populations; this is not a fixed-state CPU comparison. Battledome is an **as-shipped preset
comparison**: #187/#191 also changed pit-only food, constant-density arena scaling and guarded spawn fallback.
At 80 seats the nominal radius multiplier changed from 3.25 to `sqrt(80/32)`; old queue-age expansion was
removed. Changes in its costs cannot be assigned to #199's spawn-search optimization, nor treated as the same
geometry/ecology. Optional vision `a979b10` was skipped: the shared Cargo target reused the baseline
same-version executable (identical hash) for the intended source, and its smoke occupancy check failed. No
vision qualification result is selected. Use separate targets to avoid the retained cache failure. The branch
lacks #196/#197/#199, so it would not isolate vision alone.

All required holds decoded real traffic without generator errors. Generator CPU reached 0.917 cores total; the
busiest shard averaged 12.28% of its pinned core. Host steal was at most 0.0032%. No hold was repeated or
excluded for its speed. These are weaving/food-avoidance clients, not human browsers or realistic growing
snakes. One seed and one hold per arm give no confidence interval or causal attribution to individual changes.
Join order, deaths and physiology can diverge. The retail EPYC 9645 was not measured; c7a is the requested cloud
proxy. TLS, Internet paths, legacy clients, slow readers and long uptime remain unqualified.

## Method and reproduction

- Reuses #178's exact load executable and hash, eight pinned shards, seed 7, default presets and 100 ms
  controls, no boost, no added latency and 15 s warmup after a common epoch eight seconds in the future. Each
  hold starts a fresh world. All host threads share the server's only core; load runs on a separate c7a.4xlarge.
- Baseline/current are built independently from pinned Git sources. Same Rust 1.99 locked release profile, thin
  LTO, one codegen unit and line-table debug info, `CARGO_BUILD_JOBS=1`; required versions compiled freshly,
  each binary copied and hashed before the next build. Use separate targets when reproducing; see the retained
  cache failure. The same binaries are copied to each server. c7a.medium: AMD EPYC 9R14, 1 CPU, 1 thread/core;
  c8a.medium: AMD EPYC 9R45, 1 CPU, 1 thread/core.
- [instrument.py](instrument.py) applies #178's harness and per-tick diagnostics to isolated source snapshots;
  no runtime edits are proposed by this PR. The CLI uses `--stats 5` and the original 1 ms counter sampler. Tick
  start, busy duration and start gap are buffered, then written after shutdown; busy spans command intake
  through handover and includes preemption. Starts are clipped to the half-open hold. The same capture code runs
  in every arm; RSS includes this diagnostic storage and is not directly comparable with #178.
- Process user+system CPU / executed ticks gives CPU ms/tick. Peak RSS is `VmHWM` including warmup; operator
  load averages complete five-second intervals. Frame excess is receipt time minus cumulative server-header time
  minus each connection's minimum inside the hold; fixed path offsets cancel and world slowdown remains.
- Build each snapshot's CLI; use the patched `run_trial.py` host/load roles on separate machines with `--bin`,
  `--preset`, `--count`, `--warmup 15`, `--hold` and a common `--epoch`. Reduce retained data with `python
  analyze.py HOST LOAD N OUT`. Focused checks: `python -B -m unittest discover -s . -p 'test_*.py'`.

Baseline/current AWS smoke checks passed; local re-analysis reproduced all selected counters, timing ranks and
grades. Tick-window regression and documentation checks passed. Full Rust group was not run for this
research-only PR; the release binaries were built and exercised on native Linux. Raw logs, full receipts, tick
samples, sources, binaries and private cloud metadata are not publicly downloadable. Source, executable and
harness digests are in JSON.

| Local-only archive | SHA-256 |
| --- | --- |
| c7a.medium | `3458c722c141a79215b16939860e2797003dc7717f9857386c60efbd9aa76c68` |
| c8a.medium | `eea7cccba1f95b202975c7de0f09ee55fd44dd6d7fe791665c88a6fa5d1b2172` |
| c7a.4xlarge | `315add72f7aa8de25fa2351d52844baba4e08410735aaa6e0f448413a871cb53` |
| session-record | `03d4c7b1abf2bffedb79a7590deb5005f296ee63e2b7aeec4457d978917da005` |

## Cost and teardown

Estimated total **$0.419**, below $8: $0.327 Spot compute from launch quotes × observed lifetimes, $0.007 IPv4,
$0.006 storage and $0.079 egress allowances (624.2 MiB returned). Storage/egress use conservative $0.12/GB-month
and $0.12/GB, no credits; this is an estimate, not an invoice. One-time Spot; delete-on-termination roots,
terminate-on-shutdown and verified 120-minute hard timers. Servers ran sequentially with one separate generator.
Final scan at 2026-10-27T09:23:44.843170+00:00 attempted 34 commercial regions. All 17 enabled regions verified
zero non-terminated instances, volumes, open/active Spot requests, EIPs and created groups/keys. The other 17
regions were disabled/API-inaccessible: their inventories cannot be asserted without enabling them. No regions
or credentials were enabled/obtained.
