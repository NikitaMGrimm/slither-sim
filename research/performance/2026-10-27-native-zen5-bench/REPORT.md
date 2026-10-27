# Native Zen 5 one-core host capacity, 2026-10-27

**Original qualification: pinned main passes official-500 at 5.046 CPU ms/tick,
6.487 ms excess p99 and
zero deferred ticks. Both older commits miss at 500; all commits pass at 375
and Battledome-80.** Synthetic protocol-19 socket
players, zero host bots and no outside game servers. The [plan](PLAN.md) was
frozen before qualification; [selected results](selected-results.json) retain
all attempts. Full evidence is local-only, with no public download.
The original qualification rows remain unchanged. Review diagnostics below add
ordered repeats, current-main holds and CPU/clock accounting under the separate
[diagnostic plan](DIAGNOSTICS.md).

## Qualification results (unchanged)

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
These pins define the original qualification. The later `4cd6ce5` revision is
measured only in the separately labeled review diagnostics below.

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

Reuses #201's 1 ms CLI polling, counter rows about once per second plus hold
boundary samples, `--stats 5`, buffered tick diagnostics
and [reducer](../2026-10-27-epyc-host-recheck/analyze.py). Tick busy wall time
spans command intake through delivery handover, including preemption, with tick
starts clipped to the half-open hold. CPU ms/tick is process user+system CPU
divided by executed ticks; CPU percent is a share of one core. Resource and
public-counter hold boundaries are sampled independently; exact elapsed
windows are retained in JSON. Peak RSS is `VmHWM`, including warmup and tracing.
Process CPU includes system time charged to the host, but is not total CPU-0
utilization; interrupt work and unrelated tasks are not isolated or attributed.
Loopback commonly handles delivery of the host's own sends to receiving
sockets on the sending CPU, in the sender's context or its local receive
softirq. CPU 0 can therefore pay receive-side work that the two-machine NIC
path performs on the generator machine. This can increase host system CPU
here; some reverse-direction work can likewise run on generator CPUs.
The [upstream loopback transmit path](https://github.com/torvalds/linux/blob/v6.18/drivers/net/loopback.c#L66-L88)
queues receive processing through
[`__netif_rx`](https://github.com/torvalds/linux/blob/v6.18/net/core/dev.c),
whose default target is the current CPU (receive steering can alter it).
This is a mechanism relevant to the comparison, not measured per-packet
attribution or an identification of this guest's kernel.

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
holds for 120 s, then writes `summary.json`. Ad-hoc reruns also retain separate
user/system CPU counters and guest-observed MHz. A nonblocking file lock prevents
concurrent builds/holds. The runner checks executable/worker hashes, generator
headroom and sampled affinities. It retains a failed attempt and stops rather
than silently substituting a result. Inspect errors and occupancy before
interpreting the delivery flag. `selected-results.json` exports all attempts;
the qualification label distinguishes the frozen suite from smokes and
single-command ad-hoc holds, which are labeled `rerun`.

To install elsewhere, place [bench.py](bench.py), [run.sh](run.sh),
[verify.py](verify.py) and [diagnostics.py](diagnostics.py) at the
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

## Review diagnostics

These observations follow the owner-requested [diagnostic plan](DIAGNOSTICS.md). Original qualification rows and all thirteen original attempt objects are unchanged. All nine new holds are labeled `rerun`, excluded from qualification, and retained separately in [selected results](selected-results.json). Full CPU/clock and anomaly reductions are in [diagnostic-results.json](diagnostic-results.json). No regression claim is made in either direction.

### Original #211 official-500 anomaly

The 275 deferrals accumulated across **17 consecutive one-second counter intervals**, bracketed by 49.047 and 66.056 seconds after hold start. This was a sustained episode, not one instantaneous burst or scattered events throughout the hold. The 275 deferred ticks account for 1925 ms of world-clock shift (275 x 7 ms), contributing to the 2050 ms frame-excess p99; frame excess also includes backlog and delivery variation.

| Hold-relative interval s | New deferred ticks | Cumulative deferred |
| --- | ---: | ---: |
| 49.047-50.048 | 2 | 2 |
| 50.048-51.048 | 15 | 17 |
| 51.048-52.049 | 15 | 32 |
| 52.049-53.049 | 16 | 48 |
| 53.049-54.050 | 18 | 66 |
| 54.050-55.051 | 19 | 85 |
| 55.051-56.052 | 19 | 104 |
| 56.052-57.052 | 17 | 121 |
| 57.052-58.052 | 21 | 142 |
| 58.052-59.052 | 19 | 161 |
| 59.052-60.053 | 17 | 178 |
| 60.053-61.054 | 20 | 198 |
| 61.054-62.054 | 14 | 212 |
| 62.054-63.055 | 15 | 227 |
| 63.055-64.055 | 14 | 241 |
| 64.055-65.055 | 16 | 257 |
| 65.055-66.056 | 18 | 275 |

| Window, hold-relative s | Busy tick p50/p99/max ms | Host core % | Sim/socket core % | Aggregate steal % | Generator total/busiest core % | Host RSS range MiB |
| --- | --- | ---: | --- | ---: | --- | --- |
| before 34.047-49.047 | 4.441 / 8.993 / 12.460 | 85.3 | 47.4 / 37.3 | 0.453 | 100.7 / 37.7 | 40.68-42.43 |
| deferral episode 49.047-66.056 | 7.625 / 11.703 / 16.262 | 99.8 | 57.7 / 41.2 | 0.251 | 103.1 / 40.2 | 42.43-45.86 |
| after 66.056-81.056 | 5.233 / 9.430 / 16.193 | 92.2 | 52.6 / 38.9 | 1.377 | 105.0 / 39.4 | 45.86-49.48 |

**Proximate mechanism: sustained host CPU saturation; underlying cause: unexplained.** The episode used essentially all of CPU 0 and its median tick exceeded 7 ms, while the whole-hold average hid this interval. Both simulation and socket CPU increased. Aggregate steal decreased during the episode and generator CPU remained well below its three-core budget, so those retained observations do not support a coincident steal spike or saturated generator as the explanation. Aggregate steal cannot rule out CPU-0-specific scheduling effects. RSS grew across all three windows, without an isolated episode-only jump; RSS alone does not diagnose allocation or memory pressure.

Original host user/system counters, CPU-0-specific steal and under-load MHz were not retained and cannot be reconstructed from combined CPU. The published #201/#211 JSON likewise contains total CPU and simulation/socket thread totals, with no user/system split. Consequently neither a historical user-ms/tick comparison nor an original-episode user/system correlation is available. This is an evidence limit, not an inferred split.

Counter increments are localized to about one second. Resource windows use the nearest retained sample at each edge (offsets are in JSON, up to about 0.5 s); tick starts use exact half-open window clipping. Individual-increment tick/resource correlations are retained in JSON as well as the wider windows above.

### Retrospective deferral windows from the new holds

The same reducer is applied to every new rerun with positive deferrals, using the observations already collected. This is an additional diagnostic, without a new hold or result-based exclusion.

Rerun 5, `65ff713` / official-500: 237 deferred ticks.

| Window s | Busy p50/p99 ms | Host core % | User/system core % | CPU-0 steal % | Guest MHz min/max | Generator busiest core % | Host RSS MiB |
| --- | --- | ---: | --- | ---: | --- | ---: | --- |
| before 17.035-32.035 | 3.336 / 9.165 | 79.3 | 47.5 / 31.8 | 0.069 | 2600 / 2600 | 32.8 | 39.39-41.03 |
| deferral episode 32.035-48.046 | 7.499 / 11.801 | 99.9 | 65.3 / 34.6 | 0.062 | 2600 / 2600 | 33.6 | 41.03-44.62 |
| after 48.046-63.046 | 4.759 / 9.481 | 85.4 | 52.1 / 33.3 | 0.000 | 2600 / 2600 | 34.8 | 44.62-47.60 |

Rerun 6, `4c2471b` / official-500: 88 deferred ticks.

| Window s | Busy p50/p99 ms | Host core % | User/system core % | CPU-0 steal % | Guest MHz min/max | Generator busiest core % | Host RSS MiB |
| --- | --- | ---: | --- | ---: | --- | ---: | --- |
| before 100.114-115.114 | 5.943 / 10.622 | 95.6 | 59.2 / 36.4 | 0.072 | 2600 / 2600 | 37.3 | 47.60-48.48 |
| deferral episode 115.114-118.116 | 9.941 / 15.836 | 99.3 | 62.2 / 37.1 | 0.000 | 2600 / 2600 | 34.5 | 48.48-48.60 |
| after 118.116-120.000 | 6.388 / 10.165 | 100.0 | 65.5 / 34.5 | 0.000 | 2600 / 2600 | 45.1 | 48.60-48.85 |

The affected #211 repeat shows host user CPU increasing much more than system CPU during its saturation window, without a corresponding observed clock drop, CPU-0 steal spike or generator saturation. This supports a user-execution cost episode in that rerun, while its source remains unexplained: these counters do not distinguish changing world work from execution-cost variation. They do not reconstruct the original episode or identify a commit regression.

### Ordered official-500 repeats

A=v0.7.0, B=#211, C=original pinned main. One 15 s warmup + 120 s hold per row, in A-B-C then C-B-A order. These reuse the exact original host binaries and shared generator. The only added observation is the Python CPU/clock sampler.

| Order | Commit | CPU ms/tick | Core % | Excess p99/max ms | Busy tick p99/max ms | Deferred | Playing mean/final | Peak RSS MiB | Gate |
| ---: | --- | ---: | ---: | --- | --- | ---: | ---: | ---: | --- |
| 1 | `4c2471b` | 5.629 | 80.4 | 11.578 / 64.921 | 8.458 / 15.572 | 0 | 500.000 / 500 | 49.75 | miss |
| 2 | `65ff713` | 5.154 | 73.5 | 19.958 / 166.399 | 8.083 / 12.239 | 0 | 500.000 / 500 | 52.26 | miss |
| 3 | `9960f4f` | 4.719 | 67.4 | 5.551 / 15.512 | 7.025 / 11.894 | 0 | 500.000 / 500 | 52.53 | pass |
| 4 | `9960f4f` | 4.784 | 68.3 | 5.872 / 14.870 | 7.224 / 14.314 | 0 | 500.000 / 500 | 52.69 | pass |
| 5 | `65ff713` | 5.815 | 81.9 | 1870.562 / 1923.715 | 9.625 / 14.422 | 237 | 499.940 / 500 | 52.85 | miss |
| 6 | `4c2471b` | 5.906 | 83.9 | 841.587 / 878.293 | 10.366 / 19.075 | 88 | 499.989 / 500 | 48.98 | miss |

Original qualification plus the two new repetitions, shown descriptively without pooling frame samples:

| Commit | CPU ms/tick min/median/max | Excess p99 min/median/max ms | Deferred ticks by hold (original, repeat 1, repeat 2) | Gate passes / holds |
| --- | --- | --- | --- | ---: |
| `4c2471b` | 5.629 / 5.906 / 6.214 | 11.578 / 54.999 / 841.587 | 0, 0, 88 | 0 / 3 |
| `65ff713` | 5.154 / 5.815 / 6.166 | 19.958 / 1870.562 / 2050.062 | 275, 0, 237 | 0 / 3 |
| `9960f4f` | 4.719 / 4.784 / 5.046 | 5.551 / 5.872 / 6.487 | 0, 0, 0 | 3 / 3 |

The spread is retained rather than choosing a favorable hold. Three holds per commit remain a small descriptive sample; workload evolution, join order and box noise are not separated causally. The original 275-deferral #211 result is neither replaced nor regraded. No optimization or regression conclusion follows from these repetitions alone.

### Requested current-main diagnostic

`4cd6ce542b45e385d542d9423089d884bc314778` includes #223/#224/#225/#226. Relative to the original `9960f4f` pin, #224 and #226 are additional; #223 and #225 were already included. Built from a fresh local Git archive with the same locked release profile, then its target was removed. The eight-client smoke passed; every row below is labeled `rerun`.

| Commit | Case | CPU ms/tick | Core % | Excess p99/max ms | Busy tick p99/max ms | Deferred | Playing mean/final | Peak RSS MiB | Gate |
| --- | --- | ---: | ---: | --- | --- | ---: | ---: | ---: | --- |
| `4cd6ce5` | official-500 | 4.709 | 67.3 | 5.903 / 13.706 | 6.337 / 12.036 | 0 | 500.000 / 500 | 53.09 | pass |
| `4cd6ce5` | official-375 | 3.191 | 45.6 | 3.649 / 7.936 | 4.447 / 7.643 | 0 | 375.000 / 375 | 38.27 | pass |
| `4cd6ce5` | battledome-80 | 1.034 | 14.8 | 4.261 / 6.047 | 3.762 / 4.979 | 0 | 79.167 / 79 | 11.41 | pass |

### Per-row user/system CPU and observed clock

CPU fields use the same process-boundary samples and executed-tick denominator as total CPU. The new worker retains utime and stime separately in seconds at Linux clock-tick resolution; user + system matches total before rounding. Original qualification values are explicitly unavailable because that sampler discarded the split.

| Label/order | Commit | Case | User ms/tick | System ms/tick | Total ms/tick | Guest CPU-0 MHz min/median/max | CPU-0 steal % |
| --- | --- | --- | ---: | ---: | ---: | --- | ---: |
| qualification | `4c2471b` | official-500 | unavailable | unavailable | 6.214 | unavailable | unavailable |
| qualification | `65ff713` | official-500 | unavailable | unavailable | 6.166 | unavailable | unavailable |
| qualification | `9960f4f` | official-500 | unavailable | unavailable | 5.046 | unavailable | unavailable |
| qualification | `9960f4f` | official-375 | unavailable | unavailable | 3.812 | unavailable | unavailable |
| qualification | `65ff713` | official-375 | unavailable | unavailable | 3.844 | unavailable | unavailable |
| qualification | `4c2471b` | official-375 | unavailable | unavailable | 4.760 | unavailable | unavailable |
| qualification | `4c2471b` | battledome-80 | unavailable | unavailable | 1.641 | unavailable | unavailable |
| qualification | `65ff713` | battledome-80 | unavailable | unavailable | 1.199 | unavailable | unavailable |
| qualification | `9960f4f` | battledome-80 | unavailable | unavailable | 1.250 | unavailable | unavailable |
| rerun 1 | `4c2471b` | official-500 | 3.513 | 2.115 | 5.629 | 2600.000 / 2600.000 / 2600.000 | 0.033 |
| rerun 2 | `65ff713` | official-500 | 3.239 | 1.914 | 5.154 | 2600.000 / 2600.000 / 2600.000 | 0.108 |
| rerun 3 | `9960f4f` | official-500 | 2.653 | 2.066 | 4.719 | 2600.000 / 2600.000 / 2600.000 | 0.071 |
| rerun 4 | `9960f4f` | official-500 | 2.958 | 1.826 | 4.784 | 2600.000 / 2600.000 / 2600.000 | 0.074 |
| rerun 5 | `65ff713` | official-500 | 3.511 | 2.304 | 5.815 | 2600.000 / 2600.000 / 2600.000 | 0.070 |
| rerun 6 | `4c2471b` | official-500 | 3.502 | 2.404 | 5.906 | 2600.000 / 2600.000 / 2600.000 | 0.052 |
| rerun 7 | `4cd6ce5` | official-500 | 2.905 | 1.804 | 4.709 | 2600.000 / 2600.000 / 2600.000 | 0.056 |
| rerun 8 | `4cd6ce5` | official-375 | 1.803 | 1.388 | 3.191 | 2600.000 / 2600.000 / 2600.000 | 0.034 |
| rerun 9 | `4cd6ce5` | battledome-80 | 0.967 | 0.066 | 1.034 | 2600.000 / 2600.000 / 2600.000 | 0.048 |

Clock samples are CPU-0 `cpu MHz` values actually read from `/proc/cpuinfo` inside the hold, about once per second plus boundaries. They are guest-reported values and do not establish physical turbo clock or equal frequency with AWS. The original rows have no under-load clock record. The #201/#211 selected JSON has no user/system CPU fields, so a matching AWS user-ms/tick comparison is unavailable; their total-CPU comparison above remains descriptive and includes the loopback accounting difference.

All nine diagnostic holds have valid decoded traffic, sampled CPU affinity and generator headroom. The busiest generator core reached 33.5% on average. Worst shard poll p99 was 1.215 ms; worst control-lateness p99 was 1.163 ms. Per-row generator counters, input matches, occupancy, RSS, tick start gaps and Battledome waits are retained in JSON.

### Diagnostic reproduction and verification

The installed `./run.sh COMMIT` now adds CPU/clock telemetry for ad-hoc `rerun` holds. `python3 -B bench.py diagnostic-suite` executes the frozen review schedule. [diagnostics.py](diagnostics.py) generates an additive worker from each pinned worker without changing the original; each attempt retains that generated worker and its hash. `python3 -B diagnostics.py` reduces CPU/clock rows and the original anomaly. `python3 -B verify.py` re-reduces all valid attempts and checks source/binary hashes while keeping the original nine-row qualification census.

All 22 valid attempts (nine qualification, nine rerun and four smoke) were
re-reduced exactly from retained raw data, including the new CPU/clock fields;
all four source archives and host binaries matched their recorded hashes. The
rejected initial smoke remains retained. A separate audit compared all thirteen
original attempt objects, the three original build records and every original
table row against the prior PR head: all unchanged. The ordered nine-rerun
census, CPU split sums, clock samples, anomaly increments and privacy checks
passed. Nine native Linux wrapper/diagnostic regressions, six reused timing and
input tests, and all seven documentation checks passed. Full Rust checks were
not rerun for these research-only additions; the new native release host was
built and exercised. The SSH certificate renewal described in [operations notes](OPERATIONS.md) did not interrupt the running bench or trigger a repeated hold. No system settings, packages or services changed. Full receipts, counters, clocks and generated workers remain local-only under the bench directory.
