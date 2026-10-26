# Native Linux host capacity, 2026-10-26

How many real socket clients one core can serve, and where its CPU goes.
No host or simulator code changed.

## Findings

- On one full core (c7a.medium, AMD EPYC 9R14, no SMT), **100 clients** keep
  official-like delivery: frame excess p99 **2.428 ms** over 300 s at
  142.857 ticks/s with no deferred ticks.
- **250 clients** passed the 120 s sweep at 7.937 ms, but the 300 s
  confirmation reached **11.393 ms** and fails the 10 ms grade without
  deferring any tick. Every client's own p99 was above 10 ms (10.26–12.83 ms),
  so the tail is uniform rather than a few slow clients.
- At **500 clients** the world falls behind: 140.058 ticks/s, 341 deferred
  ticks and excess p99 2.35 s. At 750 it runs at 70.76 ticks/s.
- **500 background bots** with one watching socket client meet real time: no
  deferred ticks, the simulation thread at 50.9% of one core and watcher excess
  p99 1.007 ms. The bot threads ran on other cores.
- At 500 clients the host's CPU goes to per-recipient delta encoding (24.8%),
  framing and flushing (23.2%), visibility and food-sector work (20.1%), socket
  service (15.2%) and physics (11.8%). At 100 clients socket service leads
  (35.1%).

The [remote timing study](../../fidelity/2026-10-26-vps-timing/REPORT.md) saw
official excess p99 of about 2.3–2.4 ms in clean windows. This study masks no
stalls, so that comparison is operational, not a fidelity test.

## Grades

The grades were fixed before collection, and none was relaxed.

| Grade | Definition | Result |
| --- | --- | --- |
| (a) Official-like | Excess p99 ≤3 ms | **100**, confirmed over 300 s |
| (b) Real time | Excess p99 ≤10 ms and no newly deferred ticks | **100** confirmed; 250 passed only the 120 s sweep |
| (c) Falls behind | Deferred ticks and slowed world time | First at **500** |

The 1,000-client hold depended on 750 meeting (b) and was not run. The
grading code does not check trial validity itself, but no trial recorded a
generator error, and every socket trial that passed a grade kept all its
clients playing for the whole hold.

## Setup

- Source `6bcb6f0` (main at launch, including #111 and #112). #116 and #117
  landed during the study and were not measured.
- Ubuntu 24.04 x86-64 on EC2, one availability zone. Server: c7a.medium, one
  core and 2 GiB. Load generator: c7a.4xlarge, 16 cores. Rust 1.99.0 release
  build with thin LTO, one codegen unit and line-table debuginfo. One binary
  served both machines. [source-pins.json](source-pins.json) pins the kernel,
  toolchain and binaries.
- [`host_probe.rs`](host_probe.rs) runs the public `Host` runtime behind
  `slither-sim host`, adding only counter sampling and shutdown. The nofile
  soft limit was 4,096; there was no kernel or socket tuning.

## Workload

Each hold starts a fresh world with seed 7, `maxClients=N`, no bots and
otherwise default `HostConfig`. That is the default official approximation,
with seat-sized radius and food (`670·√N`, food scaled by area, one prey),
dynamic radius and a 7 ms tick (142.857 ticks/s). No link delay is added.

The generator, [`host_capacity_load.rs`](host_capacity_load.rs), adapts the
2026-10-24 host I/O study's client. Eight processes log in through the real
challenge, decode every frame with `ClientState`, ping, and steer every 100 ms
toward food and away from snakes and the rim. Clients never boost and rejoin
250 ms after death. Failed admissions back off and are counted. Workers share a
start epoch, warm up for 15 s, then hold for 120 s (sweep) or 300 s
(confirmation).

In the bot scenario the host runs on the 16-core machine with `bots=500`,
seven bot threads, a fixed 40 ms RTT and one watcher client. Simulation is
pinned to CPU 0, the gateway and watcher to CPU 1 and the bot threads to CPUs
2–15; the affinities were read back. The bots' living count was not
instrumented.

Metrics:

- **Frame excess:** receipt time minus the server's cumulative elapsed header,
  minus that connection's minimum within the hold. Fixed link offsets cancel,
  but accumulated world slowdown does not, so a slowed world fails even when
  its frames arrive regularly. Headers have 1 ms resolution, and receipt time
  includes generator polling and decoding.
- **Own-update gap:** the interval between own-motion frames on one connection.
- **Quantiles:** exact nearest rank, pooled over all receipts. `perClientSpread`
  gives the minimum, median and maximum of each client's own quantiles. Samples
  are correlated, so p99 and max are not long-run bounds.
- **CPU:** Linux user plus system time, as a percentage of one core.
  `simulation_seconds()` busy-wall load includes preemption and is reported
  separately.
- **Traffic:** payload counts received protocol bytes. NIC counters add TCP
  and management traffic.

## Results

| Run | Requested / admitted / playing | Ticks/s | Deferred in hold | CPU, one core | Peak RSS MiB | Own gap p50 / p99 / max ms | Excess p50 / p99 / max ms |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| sweep-100 | 100 / 100.000 / 100.000 | 142.859 | 0 | 21.19% | 14.25 | 41.331 / 43.415 / 51.997 | 0.932 / 2.323 / 10.527 |
| sweep-250 | 250 / 250.000 / 250.000 | 142.862 | 0 | 49.96% | 30.64 | 41.354 / 45.572 / 70.975 | 1.439 / 7.937 / 30.103 |
| sweep-500 | 500 / 499.856 / 499.856 | 140.058 | 341 | 96.59% | 62.68 | 40.148 / 83.602 / 289.023 | 1,171.786 / 2,349.462 / 2,409.430 |
| sweep-750 | 750 / 749.978 / 749.978 | 70.760 | 8,650 | 96.97% | 98.79 | 79.469 / 154.419 / 387.472 | 28,670.241 / 59,930.691 / 60,558.785 |
| confirm-250 | 250 / 250.000 / 250.000 | 142.858 | 0 | 53.21% | 31.62 | 41.350 / 47.615 / 77.561 | 1.685 / 11.393 / 43.151 |
| bots-500 | 1 / 0.998 / 0.998 | 142.858 | 0 | 94.94% | 121.38 | 42.310 / 63.524 / 63.681 | 0.502 / 1.007 / 2.160 |
| confirm-100 | 100 / 100.000 / 100.000 | 142.857 | 0 | 21.96% | 15.60 | 41.328 / 43.424 / 52.961 | 0.991 / 2.428 / 11.997 |

Payload was about 2.1 KB per playing client per second; host NIC traffic was
about 9.9 KB out and 3.6 KB in. The generator used under 1.2 of its 16 cores in
every run. In the bot row, CPU covers the simulation, gateway and bot threads,
and the playing count is the single watcher. The full resource, traffic, lag
and per-process figures are in [selected-results.json](selected-results.json);
[`write_report.py`](write_report.py) renders them as tables.

## Profiles

The profiles come from separate holds, excluded from the grades. They sampled
60 s with `perf record -e cycles -F 99 --call-graph fp` on a frame-pointer
build of the same source. Profiling slowed the 500-client world to 129.7
ticks/s (140.1 unprofiled), so the shares are descriptive.
[`profile.py`](profile.py) assigns each sample to one category by explicit
rules; inlining and unwinding limit their precision.

| Exclusive category | 500 clients (5,841 samples) | 100 clients (1,217 samples) |
| --- | ---: | ---: |
| Per-recipient delta encoding | 24.77% | 15.37% |
| Framing and flush | 23.15% | 19.97% |
| Visibility and food sectors | 20.06% | 11.64% |
| Socket service | 15.16% | 35.11% |
| Physics | 11.83% | 10.08% |
| Allocation (leaf) | 2.59% | 2.25% |
| Other or unknown | 2.44% | 5.58% |

At 500 clients the top self symbols are kernel interrupt entry (12.76%),
`SimulatorBackend::updates` (10.49%), `food_updates` (9.94%),
`Simulator::advance` (6.92%), unresolved (5.80%) and `broadcast_with` (4.27%).
Inclusively, `Simulation::run` has 58.94%, `broadcast_with` 45.43%,
`Gateway::run` 38.79% and `updates` 31.33%. The selected results keep the full
lists.

Recipient diffs and food-sector updates (44.8% at 500) are the first
optimization target. Framing, flushing and socket service (38.3%) come second
and dominate at 100 clients (55.1%). These shares do not predict any gain.

## Failed attempts

1. The first attempt's sampler failed when a host thread exited mid-read. That
   attempt is excluded, and a regression test covers the race.
2. An immediate relaunch hit `MaxSpotInstanceCountExceeded` with nothing
   running; a bounded retry succeeded.
3. The admitted sweep-100 run read process-wide CPU for each thread. Its thread
   figures are discarded (process CPU is valid); the sampler was fixed before
   250.
4. The first DWARF profiles lost user call chains for 2,561 of 5,840 samples at
   500 and were replaced by the frame-pointer profiles. A late 250-client
   profile ended during teardown.

All attempts are kept locally, with hashes in source pins.

## Limits

This covers one source, seed and instance pair, with short ascending holds and
one longer confirmation. It makes no claim about large bodies, clustering,
boosting, slow readers, the public Internet, TLS, multi-hour runs or several
instances. It brackets capacity and current CPU costs rather than finding an
exact maximum.

## Reproduce

The scripts need a checkout, Rust 1.99, Python 3.12+, OpenSSH and an
authenticated AWS CLI. Credentials, keys, addresses and raw output stay in a
separate scratch directory, and a new run refuses an existing one.

For the pinned revision below, use the [study-era harness](https://github.com/NikitaMGrimm/slither-sim/tree/aeb3cfe/research/performance/2026-10-26-host-capacity);
current config generators target the grouped host schema.

```sh
python3 prepare.py /path/to/new/build --ref 6bcb6f0b1057810bc7b31889c080ce9e3e57d6ae --build
python3 smoke.py /path/to/new/build/target/release/examples /path/to/new/smoke
python3 -m unittest discover -s . -p 'test_*.py'
python3 aws_run.py /path/to/new/aws --repo /path/to/checkout --aws aws
python3 select_results.py /path/to/completed/aws .
python3 write_report.py selected-results.json /path/to/tables.md
```

[`run_trial.py`](run_trial.py) is the per-machine worker and
[`analyze.py`](analyze.py) reduces receipts and resources. Raw timings, perf
data, binaries, flamegraphs and failed attempts stay local;
[source-pins.json](source-pins.json) lists their hashes. All game traffic
stayed between the two study instances or on loopback, with no outside game
server.

## Cost and cleanup

The estimated cost is **$0.50**: $0.40 of Spot compute from launch quotes and
lifetimes, plus $0.10 for storage, public IPv4 and 0.5 GiB of downloads
([aws-summary.json](aws-summary.json)). It is an estimate, not an invoice.
Instances were one-time Spot with root volumes deleted on termination and a
150-minute self-shutdown. After teardown at 02:11:23 UTC, a regional inventory
showed no running instances, volumes, addresses, snapshots or open Spot
requests, and the key pair and security groups were gone.
