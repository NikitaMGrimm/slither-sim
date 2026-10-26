# One-core EPYC socket capacity, 2026-10-26

**Battledome-32 passes on both cores; Battledome-80 fails on both.** Official 250 and 500 pass on c8a; both c7a
official holds miss the 10 ms grade. These are synthetic clients on the study's own servers. No outside game
server was contacted. [Selected results](selected-results.json) retain all eight holds.

## Results

One hold per row; CPU covers the host process. Load averages the `--stats 5` simulation-load readouts over
complete hold intervals. Playing is time-weighted mean / final. Excess triples are p50 / p99 / max.

| Server | Preset | Clients / seconds | CPU ms/tick | Ticks/s | Deferred | Stats load % | Excess ms | Playing | Peak RSS MiB | Grade |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- |
| c7a.medium | official | 250 / 300 | 3.626 | 142.857 | 0 | 27.1 | 1.693 / 10.245 / 110.034 | 250.000 / 250 | 31.41 | fail |
| c7a.medium | official | 500 / 120 | 6.223 | 142.774 | 0 | 70.7 | 4.357 / 147.584 / 214.170 | 499.991 / 500 | 54.73 | fail |
| c7a.medium | battledome | 32 / 300 | 1.161 | 142.860 | 0 | 11.4 | 1.061 / 7.278 / 18.313 | 31.371 / 31 | 10.18 | pass |
| c7a.medium | battledome | 80 / 300 | 7.842 | 111.517 | 9402 | 89.4 | 50697.519 / 65941.165 / 66067.518 | 79.991 / 80 | 42.68 | fail |
| c8a.medium | official | 250 / 300 | 1.587 | 142.857 | 0 | 14.9 | 1.411 / 2.948 / 9.705 | 250.000 / 250 | 31.46 | pass |
| c8a.medium | official | 500 / 120 | 3.019 | 142.850 | 0 | 30.0 | 1.348 / 7.305 / 27.217 | 500.000 / 500 | 54.81 | pass |
| c8a.medium | battledome | 32 / 300 | 0.561 | 142.857 | 0 | 6.6 | 1.026 / 5.900 / 10.479 | 31.593 / 31 | 10.00 | pass |
| c8a.medium | battledome | 80 / 300 | 3.734 | 142.190 | 201 | 53.2 | 1408.012 / 1518.647 / 1674.723 | 79.977 / 80 | 42.77 | fail |

The unchanged [capacity grade](../2026-10-26-host-capacity/REPORT.md#grades) requires excess p99 ≤10 ms and zero
newly deferred ticks. It does not by itself validate admission or workload occupancy; those are reported
separately.

| Server / Battledome seats | Initial spawn ms, p50 / p95 / max | Respawn ms, p50 / p95 / max | Respawns in hold | Pending at end / oldest age ms |
| --- | --- | --- | ---: | ---: |
| c7a.medium / 32 | 2611.104 / 15157.107 / 15384.106 | 47.699 / 5517.604 / 7680.639 | 170 | 1 / 361.043 |
| c7a.medium / 80 | 3253.665 / 3487.808 / 3514.285 | 19.113 / 32.862 / 32.862 | 9 | 0 / 0.000 |
| c8a.medium / 32 | 1616.373 / 14567.340 / 14794.449 | 20.161 / 2212.991 / 4593.223 | 199 | 0 / 0.000 |
| c8a.medium / 80 | 3254.671 / 3486.730 / 3515.322 | 4.291 / 11.695 / 13.804 | 26 | 0 / 0.000 |

Initial spawn includes warmup. Respawn quantiles include requests completed inside the hold, even if requested
before its start. Wait begins at the first login request, continues across retries and ends on `Playing`; it
excludes the 250 ms death-to-rejoin delay. Unfinished requests are right-censored, excluded from success
quantiles and retained with their ages. Quantiles use nearest rank.

## Method and comparison

- Runtime `48977ec` includes #168, #174, #175 and #176. All game-loop code is unchanged. [The
  patch](harness.patch) adds public-counter sampling and bounded shutdown to the CLI's diagnostic loop, at the
  old probe's 1 ms resolution; the measured command is `slither-sim host --config CONFIG --stats 5`.
- Ubuntu 24.04, Rust 1.99.0, locked release build, thin LTO, one codegen unit and line-table debuginfo. One
  generic x86-64 binary was built on the generator with `CARGO_BUILD_JOBS=1`, copied to both servers and
  hash-checked. c7a.medium: AMD EPYC 9R14, 1 CPU, 1 thread/core, kernel 7.0.0-1014-aws; c8a.medium: AMD EPYC
  9R45, 1 CPU, 1 thread/core, kernel 7.0.0-1014-aws.
- One session in eu-central-1, one availability zone, one server at a time; a separate c7a.4xlarge generator.
  Every hold starts a fresh seed-7 default official or Battledome world, N player seats, zero bots and 15 s
  warmup. The order on each server is official-250/300 s, official-500/120 s, Battledome-32/300 s,
  Battledome-80/300 s. No TLS or added link delay. As in the original controller, clients start before a shared
  epoch eight seconds in the future; the 15 s warmup is counted from that epoch.
- Reuses the [original runner and weaving client](../2026-10-26-host-capacity/REPORT.md#reproduce): eight pinned
  generator shards, full frame decoding, food/obstacle steering every 100 ms, no boost, and rejoin after death.
  Battledome permits authenticated spawn requests to wait without occupying the three-handshake limit or timing
  out; official admission behavior is unchanged. At 32 seats geometry is the original size; above 32 it grows
  under #176, so 80 is a different geometry.
- CPU ms/tick is Linux process user+system CPU divided by executed physics ticks, sampled at aligned hold
  boundaries. Peak RSS is process `VmHWM`, including warmup. Simulation load excludes sockets; CPU includes
  them. Frame excess is receipt time minus cumulative server-header time, minus that connection's within-hold
  minimum. Pooling and per-connection normalization match #122.

At 250 official clients, #122's 3.534 ms/tick becomes 3.626 (+2.6%); deferred ticks 0 → 0, and excess p99 9.938
→ 10.245 ms. At 500 official clients, #122's 6.690 ms/tick becomes 6.223 (-7.0%); deferred ticks 76 → 0, and
excess p99 740.637 → 147.584 ms. The sources were not run as a matched pair; these differences are not a causal
speedup estimate. The #122 250-client pass was already just below 10 ms.

At Battledome-80, game-loop / socket CPU was 78.6% / 8.5% (c7a.medium), 50.4% / 2.6% (c8a.medium). Median stats
load was 99.7% / 61.0%. These are thread-level costs; no hot function was profiled.

All holds decoded real socket traffic without generator errors. Generator CPU peaked at 0.920 of its 16 cores;
host steal was at most 0.0034%. The diagnostic/main thread used at most 0.45% of one core, included in CPU.
Battledome deaths, admissions and unfinished requests are retained in JSON. This is one seed and one hold per
case, without an interleaved historical build or confidence interval. It supports current capacity for this
workload and a descriptive historical comparison. It does not isolate individual optimizations or cover long
snakes, boosting, slow readers, TLS, Internet clients or long uptime.

## Reproduction and evidence

Use the original `prepare.py` at the pinned ref; copy its `run_trial.py` into the snapshot's `scripts/`, then
apply `harness.patch` at the snapshot root. Build the CLI and `host_capacity_load` with the flags above. Run the
patched worker's `host` and `load` roles on separate machines with a common future `--epoch`, `--preset`,
`--count`, `--warmup 15` and `--hold`; the load role also needs `--server` set to the study-owned private
endpoint. Reduce each pair with:

```sh
python analyze.py HOST_RAW LOAD_RAW CLIENTS summary.json
python -B -m unittest discover -s . -p 'test_*.py'
```

All raw logs, receipts, telemetry, scripts, failed attempts and binaries are outside Git, local-only and not
publicly downloadable. Their archive SHA-256s:

| Retained artifact | SHA-256 |
| --- | --- |
| c7a.medium raw archive | `172bd2a4729331bfecc8b6d498a93402f7b19ed6796ab0d6e14e9d8450faed9f` |
| c8a.medium raw archive | `6fca75f0039aaa99de1165e9f79b9a0405eaf3eda9b6af1aca91325f6a8b5d37` |
| c7a.4xlarge raw archive | `4eab3ab2d8f8edfcbedacb61ae8e6b5b07319f5130cada9ece7c4829260b12e4` |
| session-record raw archive | `01a4a50e7975f226531cefe5706c5b33a49a49c209c530e6081a9ff23df2186b` |

Source, toolchain, executable and harness hashes are in selected JSON. The precollection plan, reporting
amendment and raw scripts are retained locally; the amendment separately summarizes initial waits including
warmup. Both 8-client/8 s preset smoke checks passed before the holds. Preflight first timed out querying a
disabled region before any launch; bounded requests then completed the enabled-region scan in the same session.
No qualification hold was repeated, excluded or replaced. Raw evidence retains that preflight failure.

## Cost and cleanup

Estimated total **$0.427**, below the $5 cap: launch Spot quotes × observed lifetimes give $0.360 compute, plus
$0.008 IPv4, $0.006 storage and $0.052 egress allowances for 417.1 MiB returned. Storage/egress are
conservatively priced at $0.12/GB-month and $0.12/GB, without credits; this is not an invoice. Public IPv4 uses
[AWS pricing](https://aws.amazon.com/vpc/pricing/). All instances were one-time Spot, with delete-on-termination
root disks, terminate-on-shutdown and a verified in-instance 90-minute timer (planned duration at most 60
minutes plus 30). No instance pairs overlapped. Final inventory at 2026-10-26T22:42:50.019150+00:00 enumerated
34 commercial regions. All 17 enabled regions verified zero non-terminated instances, volumes, open/active Spot
requests, elastic IPs and study-created security groups or key pairs. The other 17 regions were disabled and
returned authentication/opt-in failures: their resource inventories cannot be asserted without enabling them. No
region was enabled or credentials obtained; this limits the all-region claim.
