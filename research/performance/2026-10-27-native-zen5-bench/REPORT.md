# Native Zen 5 one-core host capacity, 2026-10-27

**Pinned main passes official-500: 5.046 CPU ms/tick, 6.487 ms excess p99 and
zero deferred ticks. Both older commits miss at 500; all commits pass at 375
and Battledome-80.** Synthetic protocol-19 socket
players, zero host bots and no outside game servers. The [plan](PLAN.md) was
frozen before qualification; [selected results](selected-results.json) retain
all attempts. Full evidence is local-only, with no public download.

## Results

One 120-second hold per row, in collection order. Percentages are shares of one core.

| Commit | Case | CPU ms/tick | Core % | Excess p99/max ms | Deferred | Delivery gate |
| --- | --- | ---: | ---: | --- | ---: | --- |
| v0.7.0 | official-500 | 6.214 | 88.8 | 54.999 / 68.713 | 0 | miss |
| #211 | official-500 | 6.166 | 86.7 | 2050.062 / 2181.013 | 275 | miss |
| main | official-500 | 5.046 | 72.1 | 6.487 / 16.958 | 0 | pass |
| main | official-375 | 3.812 | 54.5 | 4.068 / 13.083 | 0 | pass |
| #211 | official-375 | 3.844 | 54.9 | 4.065 / 9.674 | 0 | pass |
| v0.7.0 | official-375 | 4.760 | 68.0 | 6.777 / 22.575 | 0 | pass |
| v0.7.0 | battledome-80 | 1.641 | 23.4 | 8.298 / 12.136 | 0 | pass |
| #211 | battledome-80 | 1.199 | 17.1 | 4.353 / 7.771 | 0 | pass |
| main | battledome-80 | 1.250 | 17.9 | 4.611 / 7.979 | 0 | pass |

| Commit | Case | Ticks/s | Tick busy p99/max ms | Playing mean/final | Admitted mean | Peak RSS MiB |
| --- | --- | ---: | --- | ---: | ---: | ---: |
| v0.7.0 | official-500 | 142.850 | 9.149 / 14.302 | 499.996 / 500 | 499.996 | 50.20 |
| #211 | official-500 | 140.560 | 9.624 / 17.270 | 499.946 / 500 | 499.946 | 51.76 |
| main | official-500 | 142.853 | 7.487 / 10.266 | 500.000 / 500 | 500.000 | 52.33 |
| main | official-375 | 142.858 | 4.940 / 9.758 | 375.000 / 375 | 375.000 | 38.30 |
| #211 | official-375 | 142.855 | 5.596 / 8.984 | 375.000 / 375 | 375.000 | 38.09 |
| v0.7.0 | official-375 | 142.852 | 6.434 / 16.261 | 375.000 / 375 | 375.000 | 38.46 |
| v0.7.0 | battledome-80 | 142.858 | 8.124 / 11.235 | 79.077 / 80 | 79.077 | 11.22 |
| #211 | battledome-80 | 142.858 | 4.016 / 7.213 | 79.233 / 80 | 79.233 | 11.35 |
| main | battledome-80 | 142.850 | 4.206 / 7.274 | 79.125 / 79 | 79.125 | 11.48 |

Pins: v0.7.0 = `4c2471ba9afff3609faae964cdaad49e3209da68`; #211 = `65ff713731cbd436bf5c19a25705fa77b0ebedcc`; main at setup = `9960f4f84d1b831d4468486b1e0524710b7b9c45`.
Main advanced during collection; later commits were not measured.

## Generator and input delivery

| Commit | Case | Generator core % total / busiest core | Worst shard poll / control-lateness p99 ms | Enqueue to ingest p99/max ms | Matched / sent |
| --- | --- | ---: | --- | --- | ---: |
| v0.7.0 | official-500 | 90.3 / 33.7 | 1.194 / 1.143 | 54.463 / 67.018 | 9600 / 9600 |
| #211 | official-500 | 99.5 / 37.8 | 1.323 / 1.210 | 265.358 / 275.421 | 9600 / 9600 |
| main | official-500 | 91.1 / 34.1 | 1.212 / 1.153 | 9.528 / 12.697 | 9601 / 9601 |
| main | official-375 | 72.5 / 27.1 | 1.212 / 1.153 | 7.886 / 11.160 | 7200 / 7200 |
| #211 | official-375 | 67.6 / 25.3 | 1.172 / 1.132 | 7.873 / 9.661 | 7200 / 7200 |
| v0.7.0 | official-375 | 74.7 / 27.9 | 1.239 / 1.159 | 9.226 / 20.365 | 7200 / 7200 |
| v0.7.0 | battledome-80 | 22.2 / 8.2 | 1.203 / 1.093 | 13.518 / 17.016 | 2384 / 2384 |
| #211 | battledome-80 | 22.4 / 8.2 | 1.208 / 1.107 | 9.371 / 11.024 | 2373 / 2373 |
| main | battledome-80 | 23.0 / 8.4 | 1.210 / 1.103 | 9.843 / 11.404 | 2386 / 2386 |

The busiest-core column sums shards sharing that CPU. All qualification holds have decoded traffic without generator errors and zero sampled affinity violations. The generator did not saturate its three CPUs; no load adjustment was needed. Enqueue/read/ingest match and missing counts, per-core generator CPU, thread CPU split, individual-shard timing tails, complete client counters and admission waits remain in JSON.

Maximum aggregate steal across the four vCPUs was 0.5086% during these holds; the study does not assume zero steal or isolate operating-system work from CPU 0.

| Battledome-80 commit | Initial wait p50/p95/max ms | Hold respawn wait p50/p95/max ms | Completed respawns | Pending at end |
| --- | --- | --- | ---: | ---: |
| v0.7.0 | 3674.848 / 10351.521 / 10783.588 | 6.468 / 1071.064 / 2021.680 | 277 | 0 |
| #211 | 3536.870 / 10902.104 / 12021.298 | 5.536 / 600.792 / 1768.299 | 268 | 0 |
| main | 3673.829 / 13013.532 / 13044.762 | 6.394 / 908.357 / 1717.761 | 273 | 0 |

Initial waits include warmup. Respawn quantiles include completions inside the hold, start at the first login across retries and exclude the fixed 250 ms death rejoin delay. Pending waits are right-censored and excluded from success quantiles.

## Published one-core comparison

Historical official-500 holds from [#201](../2026-10-27-epyc-host-recheck/REPORT.md) and [#211](../2026-10-27-epyc-index-recheck/REPORT.md), each 120 s:

| Study / commit | Core | CPU ms/tick | Excess p99 ms | Deferred | Gate |
| --- | --- | ---: | ---: | ---: | --- |
| #201 / `ca7ac2f` | c7a.medium | 6.059 | 150.570 | 0 | miss |
| #201 / `ca7ac2f` | c8a.medium | 2.800 | 3.612 | 0 | pass |
| #211 / `65ff713` | c7a.medium | 4.662 | 58.384 | 0 | miss |
| #211 / `65ff713` | c8a.medium | 2.155 | 3.120 | 0 | pass |

Only the #211 commit is shared with this bench. The #201 current is a different revision from v0.7.0. These are descriptive historical observations, not paired repetitions or a processor-speed ratio. Loopback on one VM shares memory, kernel scheduling and network processing with the generator; the published studies used two instances and a NIC path. The generator executable was rebuilt with sampled input tracing here. CPU generation alone does not predict the same CPU time or delivery tails, and these holds do not establish equivalence to the published c8a result.

## Method and limits

The measurement machine is **a 4-vCPU Zen 5 EPYC KVM guest (Debian 13)**.
The host and all its threads use CPU 0; eight load-generator shards share CPUs
1–3 round-robin. Observers also use CPUs 1–3. Threads inherit affinity before
startup; the observer reads both allowed masks and last executed CPU once per
second and at resource boundaries, from the shared epoch through shutdown
(including warmup). Small existing background services remain
running. No system packages, sysctls, mitigations or security settings changed.

Each arm starts a fresh seed-7 world, zero bots, default official/Battledome
preset, loopback WebSockets, no TLS or added latency. Eight processes perform
the real challenge, decode frames, steer every 100 ms without boosting, and
rejoin 250 ms after death. The load source is
[#178's harness](../2026-10-26-host-capacity/host_capacity_load.rs), with
[#201's admission accounting](../2026-10-26-epyc-host/harness.patch) and
[existing sampled input tracing](../2026-10-27-host-tick-handoff/instrument.py).
One generator executable, built against v0.7.0, drives every host. Its binary
differs from the historical frozen executable because it includes input tracing
and is rebuilt here. This comparison reuses the load algorithm and metrics.

The three hosts were built from local `git archive` snapshots, each in a fresh
target, with Rust 1.99.0, locked release, thin LTO, one codegen unit, line-table
debug info (`CARGO_PROFILE_RELEASE_DEBUG=1`), one Cargo build job, default
features and no CPU-specific compiler flags. Targets were removed after the
binary was copied and hashed. Source archives, instrumented-source digests,
binary digests and harness digests are in JSON. No runtime source changes are
proposed: overlays exist only in isolated measurement snapshots.

Reuses #201's 1 ms CLI counter sampling, `--stats 5`, buffered tick diagnostics
and [reducer](../2026-10-27-epyc-host-recheck/analyze.py). Tick busy wall time
spans command intake through delivery handover, including preemption, with tick
starts clipped to the half-open hold. CPU ms/tick is process user+system CPU
divided by executed ticks; CPU percent is a share of one core. Resource and
public-counter hold boundaries are sampled independently; exact elapsed
windows are retained in JSON. Peak RSS is `VmHWM`, including warmup and tracing.
Process CPU includes system time charged to the host, but is not total CPU-0
utilization; interrupt work and unrelated tasks are not isolated or attributed.
Loopback networking can also charge work to generator CPUs.

Frame excess is receipt time minus cumulative server-header time, minus each
connection's minimum inside the hold. Fixed offsets cancel; world slowdown
remains. Quantiles are exact nearest ranks pooled over received frames. The
delivery gate is excess p99 <=10 ms and zero newly deferred ticks, conditional
on valid decoded traffic and occupancy. Busy tick p99/max are diagnostics.
Sampled enqueue→read→ingest traces cover client IDs divisible by 64; matching
uses FIFO steering bytes and retains unmatched counts. Same-machine wall
clocks remove cross-machine synchronization uncertainty, but these are sampled
client-enqueue timestamps, not kernel arrival timestamps.
The retained matcher has a two-second matching horizon; latency ranks describe
matched samples, with losses reported separately rather than assigned a value.

One 120 s hold per commit/case after 15 s warmup; official-500 and Battledome
use v0.7.0/#211/main order, official-375 reverses it. Eight-client smoke checks
(3 s warmup, 8 s hold) precede qualification and are excluded from the tables.
The initial smoke was rejected for startup races and retained; the
[pre-qualification correction](STARTUP.md) serializes listen/connect startup
and excludes pre-epoch `taskset` samples. All three smokes then restart.
No result-based repeat or exclusion is permitted. One seed and one hold per
row provide no confidence interval or isolated causal attribution to individual
merges. Join order, population and ecology can diverge; this is an as-shipped
preset comparison. These clients do not establish human-browser performance,
larger growing snakes, legacy clients, TLS, Internet delivery, slow readers or
long-uptime capacity. No maximum capacity or target-CPU model qualification is
claimed. No PMU profiling was attempted.

## Reproduction

On the prepared machine, rerun the main case with:

```sh
cd ~/slither-bench
./run.sh 9960f4f84d1b831d4468486b1e0524710b7b9c45
# Also accepts the installed full SHA, unique prefix, v0.7.0, index, or main.
./run.sh index --preset official --count 375
./run.sh v0.7.0 --preset battledome --count 80
# Check the original nine-row schedule against all retained raw inputs:
python3 -B verify.py
python3 -B bench.py select
```

`main` is an alias for this study's pinned main, not a moving remote ref.
Each invocation creates a new timestamped raw directory, warms for 15 s and
holds for 120 s, then writes `summary.json`. A nonblocking file lock prevents
concurrent builds/holds. The runner checks executable/worker hashes, generator
headroom and sampled affinities. It retains a failed attempt and stops rather
than silently substituting a result. Inspect errors and occupancy before
interpreting the delivery flag. `selected-results.json` exports all attempts;
the qualification label distinguishes the frozen suite from smokes and
single-command ad-hoc holds, which are labeled `rerun`.

To install elsewhere, place [bench.py](bench.py), [run.sh](run.sh) and
[verify.py](verify.py) at the
bench root, a snapshot of this research PR under `harness/`, and unmodified
local `git archive --format=tar COMMIT` outputs at `archives/FULL_SHA.tar`.
The measurement machine never needs a repository clone or game-server access.
Build v0.7.0 first to freeze the shared generator, then each desired commit:

```sh
python3 -B bench.py build v0.7.0
python3 -B bench.py build index
python3 -B bench.py build main
python3 -B bench.py suite
```

For a future merge, archive its full SHA from the operator's clone, install it
under `archives/`, then `python3 -B bench.py build FULL_SHA` and
`./run.sh FULL_SHA`. The single-command runner uses an already-built commit;
building is explicit. Builds refuse existing binary/source/target directories,
check free disk space and retain failed targets for inspection. Remove only a
verified failed bench target before retrying. Sources, binaries and full raw
receipts/ticks/logs remain under `~/slither-bench/`; nothing is installed as a
service. Selected raw-file hashes establish identity, not public availability
or scientific acceptance.

## Verification

All nine qualification holds and the three valid smokes were re-reduced from
raw receipts, tick samples and resource/counter logs; every summary field
matched exactly. Source-archive and host-binary hashes also matched. All
qualification holds had zero generator errors, correct sampled affinities,
adequate generator headroom and all sampled inputs matched. One failed initial
smoke remains selected with its original errors and rejection flags; it is not
regraded using the later startup correction. The executed qualification wrapper
and rejected-smoke wrapper hashes are retained separately; later wrapper
changes add export provenance and label ad-hoc invocations `rerun`. The suite's
measurement code and schedule remain the executed version.

Four native Linux wrapper regressions, two frame-excess/nearest-rank tests,
one half-open tick-window test and three input-matching tests passed. Shell
syntax, Python syntax/LF, selected census/order/grade/privacy checks and all
seven documentation tests passed. No full Rust test group was run for this
research-only change; all three release hosts were built and exercised on
native Linux. The report does not change hosting recommendations.
