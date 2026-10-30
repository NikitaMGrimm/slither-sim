# Official-500 quiet-image acceptance v4

**Both independent fresh thirty-minute runs pass every frozen gate, with zero deferred ticks.**

World CPU p99, p99.9/max and client frame tails are report-only. No root README or changelog change; the maintaining agent owns any README wording. The failed v3 result stays failed and separate.

[Plan](PLAN.md), [attempts](ATTEMPTS.md), [selected results](selected-results.json), [preflight](preflight.json), [reproduction](REPRODUCE.md), [raw bindings](raw-bindings.json), [cost](cost.json), [cleanup](cleanup.json).

## Method and fixed intervention

Same runtime `388be59aebf3108f5c510aa7269c08b67851b202`, instrumented portable release `-C target-cpu=x86-64`, Rust 1.99, thin LTO, one codegen unit. Reused verified v3 host/client binaries, rehashed on each fresh pair. The same image selection as v3 is privately verified; no cloud image identifier is published. No new Rust build or Cargo target.

All host, observer and diagnostic threads share the c7a.medium sole vCPU. A separate same-zone c8a.2xlarge runs six full received-frame protocol-19 A4 workers on CPUs 0–5, control on CPU 7, with summed decision/connector/publisher CPU headroom. Seed 7, official density, natural tick-zero warmup, inherited resident/rotating schedule, forced churn ends at tick 180,000. The original three-endpoint entry screen, ten-second settle and complete 1,800-second windows are unchanged. End class has lower bounds only. No checkpoint, TLS, artificial latency, human or outside game server.

The initial plan/source commit `f1f3ccf5` predates provisioning. Before any world, actual systemd states exposed verifier/setup errors; the preserved pre-start corrections are in the plan and attempts. Final collection freeze `2abc874e` and its proof bindings precede tick zero. No runtime, generator, gate or timing changes between runs. Neither timed window is retried, replaced, trimmed or pooled.

The sole experimental intervention is the disclosed quiet host image. Mandatory added diagnostics consume CPU on that same core; their measured cost is reported below. Generator services stay unchanged. The ordinary synchronous `host.log` -> HTTP -> `host-stream.log` path remains intact; no memory buffer or logging change.

## Actual quiet policy

Both hosts discover all five requested timers: `apt-daily`, `apt-daily-upgrade`, `fwupd-refresh`, `fstrim`, `man-db`. All are stopped and runtime-masked. Corresponding running maintenance jobs are allowed to finish before the world. fwupd, snapd and snapd.socket are stopped/masked. Snap-packaged amazon-ssm-agent is stopped; its actual unit file in `/etc` outranks a `/run` mask, so it is explicitly stop-only, with snapd and its activation socket disabled for the window. It remains inactive at both recorded window boundaries.

The per-run diagnostic receipts retain actual pre-window/after states. All targeted timers/maskable services remain inactive with `masked-runtime`; SSM remains inactive/enabled (stop-only). Networking, SSH, chrony, journald and rsyslog remain active. Shutdown deadlines stay unchanged. Before/after timer listings and the timestamped journal are retained privately. The first host’s original preparation states/source/failures are also retained separately; no failed setup receipt is erased.

## Results

| Run | Pass | Deferred | World CPU p50 / p99 / p99.9 / max ms | Maintenance / other mean ms | End mean / p90 / p99 length |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | True | 0 | 4.751 / 7.019 / 8.239 / 11.907 | 7.150 / 4.789 | 101.434 / 234 / 289 |
| 2 | True | 0 | 4.632 / 7.078 / 8.246 / 12.019 | 7.087 / 4.707 | 97.036 / 229 / 293 |

| Run | Gap p99 ms | Header-gap residual p99 ms | Frame excess p99 ms | Steal % |
| --- | ---: | ---: | ---: | ---: |
| 1 | 28.023 | 3.080 | 4.073 | 0.0012 |
| 2 | 28.077 | 2.998 | 3.899 | 0.0006 |

| Run | Ticks | Count | World mean / p99 ms | Simulation mean ms | Socket mean ms |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | all | 257100 | 4.822 / 7.019 | 2.438 | 2.384 |
| 1 | maintenance | 3571 | 7.150 / 8.912 | 4.625 | 2.526 |
| 1 | ordinary | 253529 | 4.789 / 6.744 | 2.408 | 2.382 |
| 2 | all | 257100 | 4.740 / 7.078 | 2.411 | 2.329 |
| 2 | maintenance | 3571 | 7.087 / 8.937 | 4.535 | 2.552 |
| 2 | ordinary | 253529 | 4.707 / 6.726 | 2.381 | 2.326 |

| Run | Component | CPU p50 / p99 / p99.9 / max ms |
| --- | --- | ---: |
| 1 | Simulation/broadcast | 2.416 / 4.567 / 4.938 / 7.393 |
| 1 | Socket I/O | 2.296 / 4.115 / 4.525 / 6.010 |
| 2 | Simulation/broadcast | 2.391 / 4.476 / 4.862 / 7.609 |
| 2 | Socket I/O | 2.243 / 4.090 / 4.546 / 5.540 |


World CPU uses aligned simulation + socket CPU per tick over complete interior blocks. Component quantiles describe separate distributions and are never added. Frame histograms pool workers within each run, not across runs. World/frame tails use nearest rank; the inherited headroom gate keeps its frozen quantile method.

## Gates and endpoint coverage

| Run | Gate | Result |
| --- | --- | --- |
| 1 | zeroDeferredTicks | pass |
| 1 | startInMatureClass | pass |
| 1 | endInMatureClass | pass |
| 1 | clientsQueuesGenerator | pass |
| 1 | cpuClockReconciliation | pass |
| 2 | zeroDeferredTicks | pass |
| 2 | startInMatureClass | pass |
| 2 | endInMatureClass | pass |
| 2 | clientsQueuesGenerator | pass |
| 2 | cpuClockReconciliation | pass |

| Run | Worker CPU median range % | Worker CPU p99 range % | Worst per-report input p99 ms |
| --- | ---: | ---: | ---: |
| 1 | 15.73–16.03 | 16.55–17.00 | 1.579167 |
| 2 | 15.53–15.82 | 16.43–16.70 | 1.928134 |

Run 1: 257,100 interior ticks, 1799.702290 canonical / 1800.177494 requested seconds. Covering deferral bracket extends 0.474585 s before / 0.448194 s after the marks. Mean/p90/p99 length 57.568/184/248 -> 101.434/234/289, living 500 -> 500. World p99 <=7 ms: **False**, report-only.

Run 1: write blocks/full/rejects 0/0/0; forced retirements/admission refusals 0/0; sampled sockets 498–500. Full queue maxima/slopes/thirds and natural joins/deaths remain selected. Clock/task mean discrepancy 0.000030 ms/tick; observer 0.055125 ms/tick. Neither world p99 nor frame tails decide acceptance.

Run 2: 257,100 interior ticks, 1799.702972 canonical / 1800.072147 requested seconds. Covering deferral bracket extends 0.502660 s before / 0.525655 s after the marks. Mean/p90/p99 length 55.248/180/244 -> 97.036/229/293, living 500 -> 500. World p99 <=7 ms: **False**, report-only.

Run 2: write blocks/full/rejects 0/0/0; forced retirements/admission refusals 0/0; sampled sockets 499–500. Full queue maxima/slopes/thirds and natural joins/deaths remain selected. Clock/task mean discrepancy -0.000009 ms/tick; observer 0.055092 ms/tick. Neither world p99 nor frame tails decide acceptance.

## Retained host diagnostics (descriptive)

[Reducer](diagnostics.py), [run 1](run-1-diagnostics.json), [run 2](run-2-diagnostics.json). All-process cumulative CPU with PID/start identity and complete `/proc/stat` (including ctxt/procs_running) is sampled once per second. Raw identities, full commands, unit/timer listings and journal stay outside Git. The following uses complete one-second covering intervals, without interpolation or exclusion.

| Run | Intervals | Max sample gap ms | Observer / diagnostic / other matched CPU ms per wall second | Max matched-other CPU in one interval ms | Journal entries in window |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1 | 1801 | 1001.956 | 7.885 / 5.547 / 3.320 | 20 | 20 |
| 2 | 1801 | 1001.848 | 7.868 / 5.541 / 3.365 | 30 | 20 |

Run 1: sampled ctxt delta 6,484,631, runnable-task sample range 1–6, max process scan 18.664 ms. Covering-interval iowait/steal are 0.030/0.020 CPU-seconds. Signed global/task user-counter residual is -33.540 CPU-seconds; it does not reconcile exactly and is **not** an estimate of other-process CPU.

Run 2: sampled ctxt delta 6,629,163, runnable-task sample range 1–5, max process scan 17.898 ms. Covering-interval iowait/steal are 0.030/0.010 CPU-seconds. Signed global/task user-counter residual is -50.180 CPU-seconds; it does not reconcile exactly and is **not** an estimate of other-process CPU.

Process CPU is direct matched PID/start counter differences. Tasks absent at either endpoint, including short-lived jobs, can be missed; the sampler itself contributes to runnable-task counts. Counter quantization, sequential scans and non-closing global/task user accounting limit attribution. No per-tick off-CPU trace is collected. The separately selected steal percentage uses the unchanged canonical window.

These runs do not identify the specific process responsible for v3’s 1.43 CPU-second contention burst. These two quiet-image passes support this tested configuration; they do not prove that fwupd or any particular timer caused v3. All v3 failures remain intact. No following acceptance series is authorized by this report; if v4 fails any gate, the frozen stop rule requires clearly improved maintenance-tick headroom below the current 7.1–7.4 ms before another attempt.

## Verification and reproduction

Per-run AWS preflight: exact v3 portable binary hashes and committed collection delta, focused syntax/service-target/protected-unit/runtime-mask-state controls, live all-process/full-proc-stat samples, inherited summed-headroom/class/gate/nearest-rank controls and seven documentation checks. The focused public check is byte-identical to the retained pre-world script. No independent review, new local Rust work or other bench host.

Both full online/offline results are compared before selection, including every tail and maximum; four deliberate p50/p99/p99.9/max perturbations are detected per run. [Run-1 replay](run-1-replay.json), [run-2 replay](run-2-replay.json), [proof bindings](preflight.json). No omitted fields, tolerance or changed gate.

The pinned v3 report-integrity check verifies all 10,812 selected reports in each run: decision + connector + publisher fractions match the total to at most 5.56e-17 rounding error, and zero reports reach 42 ms input-lateness p99. [Run 1](run-1-headroom.json), [run 2](run-2-headroom.json), [verification](verification.json). Its rounding tolerance never changes the strict headroom gate.

[REPRODUCE.md](REPRODUCE.md) links the unchanged harness and exact commands. The inherited v1 tables.py recomputes the four CPU/cadence tables; gate cells read result.gates. Worker ranges are min/max of the six stored p50/p99 fractions times 100. Host rows read the named diagnostic fields (seconds multiplied by 1,000 where marked ms). Full gated fields, boundary coverage and nearest-rank tails remain in selected-results.json.

Final documentation checks run on AWS after timing; the cost/cleanup values below are filled after teardown without changing link targets or measured results.

## Cost and cleanup

The [cost receipt](cost.json) conservatively bounds launch-to-verified-cleanup compute at **$1.198168** using the frozen rate caps. With the full **$0.40** noncompute reserve, the bound is **$1.598168 < $3**, not a final invoice. Frozen worst-two-pair full-timer bound remains $2.117334.

The [cleanup audit](cleanup.json) covers **all 17 enabled regions**, none unavailable. Both pairs are terminated, and task volumes, key and security group are deleted; unrelated resources are preserved. No replacement or third pair. Both raw archives and final check proof were retrieved/verified before teardown. Seven final documentation checks passed on AWS after timing; only verified cleanup/cost values and matching prose were filled afterwards, without changing link targets or measured data.

## Limits

Two fresh pairs, one seed/policy, protocol 19, naturally evolving mature worlds and a quiet host image with added diagnostic overhead. This is the stated received-frame socket workload, not arbitrary human behavior, Internet/TLS/slow-reader or renderer performance, another CPU/OS configuration, ecological stationarity or a maximum capacity. No code optimization or root README claim.
