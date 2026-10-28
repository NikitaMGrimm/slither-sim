# Interrupted operator-only c7a tuning, 2026-10-28

**Spot reclaimed the generator during E1. A1-D1 completed and all missed; no arm qualified.**
The planned two-repetition comparison is incomplete. [PLAN.md](PLAN.md) was frozen before collection;
[selected results](selected-results.json) retain completed summaries and interrupted host evidence.
One c7a.medium core, separate c7a.4xlarge generator, official-500, zero bots. No runtime, hosting-guide
or changelog change. No outside game servers were contacted.

## Results and interruption

The frozen gate requires BOTH valid 120 s repetitions to have pooled excess p99 <=10 ms and no new
deferred ticks. A-D already contain a miss, but their reverse repetitions did not run. E1 has no client
reduction. No replacement hold or recovery session was attempted.

| Arm | Setting | Completed evidence | Excess p99 rep 1 / rep 2 ms |
| --- | --- | --- | --- |
| A | Default | First hold misses; replication incomplete | 59.982 / not run |
| B | Socket nice +5 | First hold misses; replication incomplete | 20.047 / not run |
| C | RX64 | First hold misses; replication incomplete | 32.093 / not run |
| D | Nice +5 + RX64 | First hold misses; replication incomplete | 16.581 / not run |
| E | Adaptive RX on (default alias) | Interrupted; ungraded | unavailable / not run |

EC2 records `Server.SpotInstanceTermination` and `instance-terminated-no-capacity` for the generator,
with termination at 2026-10-27T23:48:27Z during E1. The controller timed out waiting for load completion,
retained the server archive, then cleaned up. Unretrieved client receipts were lost, including A1-D1.
Those four rows are retained AWS reducer outputs, not independently reproducible client-frame evidence.
E1's host capture completes, but its player count falls from 500 to 0. Its low CPU/IRQ values below
reflect interrupted traffic and must not be interpreted as a tuning improvement.

## Every started hold

CPU is a share of one core. Sim/socket use task counters; total includes the diagnostic CLI thread.
IRQ/s counts NIC Tx-Rx queue interrupts, as in #217. User/system splits and total IRQs are in JSON.

| Hold | Result | CPU ms/tick | Core % | Sim/socket % | Tick busy p99/max ms | NIC IRQ/s | Deferred |
| --- | --- | ---: | ---: | --- | --- | ---: | ---: |
| A1 | miss | 4.507 | 64.4 | 27.5 / 36.4 | 9.087 / 34.276 | 24553 | 0 |
| B1 | miss | 4.506 | 64.4 | 27.8 / 36.1 | 5.968 / 21.103 | 23034 | 0 |
| C1 | miss | 4.358 | 62.3 | 26.6 / 35.3 | 8.339 / 20.631 | 20641 | 0 |
| D1 | miss | 4.312 | 61.6 | 26.1 / 35.0 | 5.515 / 13.857 | 20444 | 0 |
| E1 | interrupted | 1.960 | 28.0 | 12.9 / 14.7 | 5.460 / 16.944 | 9014 | 0 |

A1-D1 maintained mean/final playing 500 without generator errors. Tick busy ranks are diagnostics,
not another gate. Process/task user/system accounting is not exactly additive. Retained settings,
excess p50/max, tick gaps/rate, population, RSS, operator load and generator counters are in JSON.

## Settings and method

Boot readback was adaptive RX on, RX20/TX64 us. E is a supported duplicate-default control, not a new
adaptive setting. A/B restore boot values; C/D change only RX to 64. B/D set only socket nice +5,
unprivileged; other threads stay nice 0/SCHED_OTHER, all on CPU [0].
With [adaptive ENA](https://docs.kernel.org/7.0/networking/device_drivers/ethernet/amazon/ena.html), delay changes
dynamically; RX64 readback is not proof of a fixed 64 us packet delay. This matches #217's adaptive-on mode.

[tuning_probe.py](tuning_probe.py) records full ethtool readback before reset, after application, before
warmup ends and after each completed hold. A1-D1 matched all checks. E1 matched pre-hold NIC/priority
checks, but its post-hold NIC readback was not retrieved. Condensed states are in JSON; full logs are local.
Call `configure(ARM, BASELINE)` before the fresh host; reuse #232's `STUDY_SOCKET_NICE` hook, then `verify`.
Add `network_irq()`/`total_irq()` to resource snapshots; [reduce.py](reduce.py) uses those boundaries.

Reuse [#201](../2026-10-27-epyc-host-recheck/REPORT.md)/#232's exact load, gate and frame reducer: seed 7,
eight pinned shards, 100 ms steering, 250 ms death rejoin, 15 s warmup after a shared future epoch,
120 s hold, fresh world, no boost/TLS/latency. Same --stats 5 and study-only 1 ms counter sampler.
Excess subtracts cumulative server-header time and each connection's minimum inside the hold; exact nearest
ranks and half-open tick windows. One eight-client/eight-second smoke with three-second warmup passed.
The attempted order was A1 B1 C1 D1 E1; planned E2 D2 C2 B2 A2 did not run.

Task-start main `4dd66ab00ab8b0acb2b9ea1432a43f9fd31a8196` differs from binary build
`4532a76f22958ae3abcca801e90edd30ed9f466f` only in docs/research/changelog. Runtime/build files
are unchanged; reused binary hashes and Rust toolchain are in JSON. No rebuild was needed.

## Verification, limits and retained evidence

Host CPU/IRQ boundaries, settings, source/binary/worker identity, completed-summary consistency and privacy
were audited. Three reducer/probe tests and seven documentation tests passed. Full Rust checks were not
rerun for this research-only change. Frame reductions ran on AWS; local work used one CPU for small scripts.
No profiling, kernel boot changes or mitigation changes occurred.

Missing reverse repetitions and receipts prevent the planned replication and independent delivery
re-reduction. One instance/seed and short holds give no confidence interval or long-uptime guarantee.
Join order, ecology, IRQ timing and platform variation matter; #232/#217 are context, not fresh repetitions.
This is not an exhaustive operator-setting search. Input-to-tick latency was not measured; these clients
do not model browsers, growing snakes, legacy clients, TLS, Internet paths or slow readers.
No reliable two-repetition 500-player claim follows from this interrupted study.

| Local-only archive (no public download) | SHA-256 |
| --- | --- |
| c7a.medium | `7d753a2a893d0180a5aeba7a634308a58bb6abfb7e0e4baafdb1ffa93209eca9` |
| session-record | `b333cb49c7753ae09ce7d1698bd684c909f3c4c667e2df6381397ebb5bd40f58` |

## Cost and cleanup

Estimated **$0.152**, below $2, not an invoice: Spot history and observed upper lifetimes,
IPv4, 24 GiB roots, conservative storage/egress allowances and a $0.01 reserve. Both instances had verified
90-minute timers, terminate-on-shutdown and delete-on-terminate root storage.
Final scan at 2026-10-27T23:54:38.429712+00:00 attempted 34 commercial regions: all 17 enabled regions
had zero instances, volumes, open/active Spot requests, EIPs and created groups/keys. The other
17 disabled/API-inaccessible regions remain unverified. All created resources were deleted.
