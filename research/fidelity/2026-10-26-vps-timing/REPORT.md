# VPS official-server timing, 2026-10-26

The first round stopped after about one minute: the low-RTT endpoint closed
abnormally (1006). Both partial recordings have valid gzip/integrity receipts,
but the frozen plan quarantines interrupted captures. **Q1 and Q2 remain
inconclusive: zero eligible batches, no held-out data.** There were two connection
attempts, peak concurrency two, and no retries. No simulator model, default or
bot policy changed. Spawn admission was outside scope.

## Results and failed attempt

At **2026-10-25 23:30:15 UTC** round 1 opened one socket per endpoint. At
23:31:14.794 the low endpoint emitted `socket-error` with an empty message,
followed by close code **1006** and an empty reason. The study controller aborted
both clients; the coordinator exited 1 at 23:31:15. The second connection was
stopped by that abort, not independently observed to fail. The datacenter
blocking/rate-limiting risk remains a possible explanation, **not an identified
cause**. No login/Origin change, workaround, retry or endpoint substitution
followed. Rounds 2–3 were never started.

[Selected results](selected-results.json) retain the terminal receipts,
diagnostic RTT, missing batches, telemetry summary and cleanup verification.
[Source pins](source-pins.json) identify full raw files, copied VPS manifest,
development/validation outputs, source snapshots and the development seal.

| Endpoint batch | Received raw frames | Game-ping count | RTT p50 / p99 ms | Outcome |
| --- | ---: | ---: | ---: | --- |
| low-1, `57.129.37.42:444` | 7,652 | 233 | 7.006 / 12.733 | Interrupted; socket error / 1006 |
| second-1, `198.244.231.35:444` | 6,079 | 234 | 23.977 / 29.949 | Interrupted by sibling failure |

These raw-frame counts include handshake/setup traffic. Both archives have
`completeGzip=true`, `integrityVerified=true`, unchanged collector sources and
exactly one audit issue: `interrupted_capture`. Integrity does not promote them
to complete acquisitions. There were **0/6 completed endpoint batches**,
**2 interrupted**, **4 not attempted**, **2 connections** and **peak concurrency
2 including connecting sockets**. Each client joined once; there was no rejoin
or retry.

**Q1 — server versus path:** unresolved. The frozen analyzer admits **zero excess
frames / zero clean eligible seconds** from interrupted archives. Thus there is
no admitted excess p99, >10/20 ms episode count or run rate to compare with
#108 or the network-path fit. Diagnostic RTT alone cannot answer this question;
it includes two directions and server pong scheduling. Neither moving pauses to
the server nor retaining them as an identified per-link mechanism is validated
by this failed attempt.

**Q2 — scheduler gates and counterexamples:** unresolved. The frozen analyzer
quarantines both scheduler streams, so no admitted motion/mass/prey gap counts
or reverse-order pairs are reported. It would be incorrect to read that as zero
counterexamples. There are no held-out observations to decide which exceptions
persist or which receipt gaps reflect burst compression. The existing scheduler
study and its counterexamples remain unchanged.

**Model proposal:** none. This attempt supports no new gate, delay-placement
change or fitted parameter. Collecting a replacement requires a new explicit
decision and frozen plan; it is not a continuation hidden inside this budget.

## VPS load and cleanup

The 60 load records contain 59 adjacent CPU/steal differences, sampled at about
one second. CPU percentages are per-sample descriptive summaries; host busy
excludes idle/iowait and includes steal, and container CPU is percent of one core.

| Covariate | Mean | p50 | Maximum |
| --- | ---: | ---: | ---: |
| Host one-minute loadavg | 0.700 | 0.555 | 1.290 |
| Host busy, all four cores | 12.016% | 10.127% | 77.722% |
| Host steal | 0.517% | 0.503% | 3.291% |
| Capture cgroup CPU, one core | 18.043% | 17.589% | 25.100% |

Observed cgroup throttled-time increase was **0 µs**. Peak process RSS was
110,579,712 bytes. Max timer extra lateness / loop delay / receive cost were
**43.352 / 27.558 / 1.873 ms**. The same three stalled telemetry intervals were
masked for both clients; they remain in selected results. The brief host busy
spike and nonzero steal are retained covariates. No threshold was relaxed and
no assertion of a completely stall-free VPS is made. These maxima include
collector startup; they do not identify the abnormal socket-close cause.

All **21** files in the remote SHA-256 manifest were copied to local scratch
and independently matched before cleanup. The first unprivileged removal hit
`Permission denied` on the no-network gauge's root-owned `gauge/load.jsonl`.
Only that exact scratch file was removed with `sudo -n rm`; the remaining empty
scratch directories were removed with `rmdir`. The study image was newly pulled
(absent from the saved pre-study image inventory) and removed without force.
Read-only checks at **23:33:53 UTC** verified that
`/home/ubuntu/slither-capture-20261026`, the study container and the pinned image
were all absent. Existing service containers, volumes and directories were not
modified.

## Questions and method

The two questions were frozen in [plan.json](plan.json), with the collector,
analysis and synthetic tests, in commit
`86787770df6a82dee43a108e770840d6e9ab2185` before any game socket opened.
The base was `f11c830df1af541805c73e6f1f64ee6e0ac2452d` ([#108](https://github.com/NikitaMGrimm/slither-sim/pull/108)).

1. Compare frame-receipt excess p99 and contiguous >10/20 ms episodes against
   the [home capture](../2026-10-25-official-capture/REPORT.md) and frozen
   [network-path fit](../2026-10-25-network-path/REPORT.md) from a low-RTT VPS.
2. Retest the [scheduler study](../2026-10-25-packet-scheduler/REPORT.md)'s
   wire-level motion/mass minima, reverse mass/motion ordering and repeat prey
   correction gaps, using both server and receipt clocks.

The owner supplied this host description: Oracle Ampere A1 in Frankfurt,
four Neoverse-N1 cores, 24 GB RAM, Ubuntu 24.04; pre-study ICMP RTT
1.64 ms (mdev 0.017 ms) to `57.129.37.42` and 17.9 ms (mdev 0.02 ms) to
`198.244.231.35`. These are owner-reported ICMP measurements, not this study's
game-ping observations or guarantees about later TCP traffic.

The official `https://slither.io/i80124.txt` response was saved with HTTP headers
and decoded before collection. Both IPs were active port-444 entries. The plan's
`officialList.fetchedUtc` contains an hour-offset transcription error; the
retained HTTP `Date` is **2026-10-25 23:25:58 GMT**, not 22:25 UTC. The original
plan remains unchanged; its hashes identify the correct saved response.

The frozen budget is three ten-minute rounds, each with one existing forage
collector per endpoint. Rounds 1–2 are development; round 3 is untouched
held-out validation until development analysis is saved and sealed. All lives
at an endpoint within a round stay together. Two simultaneous endpoints are
different servers, not paired recipients of a single server. There are no
additional probes, controlled input interventions or spawn experiments.

## Collector and resource controls

Windows OpenSSH is the only SSH/SCP transport used. Acquisition runs in a
single ephemeral ARM64 Node container, with `--rm --cpus=1 --memory=1g`,
one libuv worker, a read-only source mount, and the host user's UID/GID.
`--network host` avoids Docker bridge/NAT timing; the preflight gauge used
`--network none`. The pinned image is
`node@sha256:55fb5f5a527ea2c5c4a95f963e48ef90aa38c7b346054fdc8ee9957a2b55d205`.
No host packages, configuration, production services, existing containers or
volumes are changed. Only `~/slither-capture-20261026/` is used remotely.

[capture.mjs](capture.mjs) instruments the existing headless forage collector;
controls are unchanged. An unexpected socket error/close stops both clients
and the round coordinator without retry or replacement. A normal death may
rejoin only after its socket closes. A datacenter IP could be blocked or
rate-limited by slither.io; acquisition failures are retained, not worked around.

The 1 ms high-resolution timer records exact gap/lateness maxima, alongside
`monitorEventLoopDelay({resolution:1})`, GC and receive-handler costs. A
30-second no-network preflight had max gap **6.804 ms**, extra lateness
**5.804 ms**, and event-loop max **6.328 ms**. The frozen clean rule masks each
telemetry interval plus 100 ms margins for >5 ms extra timer lateness,
>6 ms loop delay, >5 ms GC/receive cost, or newly observed write backpressure.
Unlike Windows coarse timer dispatch, Linux ordinarily wakes near 1 ms;
occasional preflight stalls still exceed this conservative threshold and are
excluded. Load never permits loosening it. Uncovered timing and intervals
crossing masks are excluded from clean results.

Per-second telemetry includes host loadavg, aggregate CPU/steal counters,
container cgroup CPU usage and throttling, process CPU/memory and output queues.
Other services' load remains a covariate. Analysis uses at most two local
logical CPUs with numerical libraries limited to one thread.

## Analysis and interpretation

[timing.py](timing.py) reuses the historical network-path baseline, summaries,
ACF and ordered delivery simulation without refitting. Each setup epoch has
one-second burn-in, at least 1,000 frames and 30 seconds. The primary excess
subtracts the Theil–Sen slope of five-second p1 floors, then a centered
±10-second minimum. Slopes, wall/server spans and line-only residuals remain
available. This future-dependent gauge can remove slow queue changes and is
not physical one-way delay. The same six frozen fit parameters and three
seeds are specified for comparison on admitted schedules; this failed attempt
had none, and no new simulator calibration occurred.

Tail episodes retain counts, exposure, run rates, peak, observed duration and
surrounding brackets; a single-frame zero duration is a lower bound. Run rate
is not a latent Poisson pause hazard. Full and clean results are separate.
Held-out attribution requires ≥300 clean eligible seconds and ≥80% clean
exposure. The frozen near-zero/persistent/intermediate criteria are descriptive
decision gates, not confidence intervals across a server population.

Scheduler gaps reset at setup, connection and entity reappearance/removal.
Repeat prey `j` intervals exclude entry-to-first correction and split on
previous/current speed exactly 11. Reverse pairs use same-frame same-snake
standalone mass/tail versus motion. Raw packet witnesses use zero-based
post-setup frame indices, connection and cumulative server time, with the
compressed archive digest. Server elapsed-header gaps cannot be compressed by
the downstream path; short receipt gaps alone can be.

Even with clean callbacks and low ICMP variation, the collector does not record
kernel arrivals, independent one-way propagation or an official physics tick ID.
Remaining delays can originate in server work after timestamping, sender or
receiver TCP/socket handling, or the path. This design cannot identify shared
cross-client delay coupling on one server. Wire gate exceptions do not reveal
the exact hidden dirty/rearm rule or refute every possible internal tick gate.

## Access and reproduction

Full raw bytes, telemetry, logs, failed attempts and source snapshots remain
**local-only**, outside Git, under
`C:\Users\Nikita\.t3\scratch\vps-capture-20261026\`.
Raw official traffic/client material is not redistributed; no redistribution
permission is established. Paths and SHA-256s identify bytes, not scientific
validity or authenticated server source. This is a selected research record,
not a complete evidence-bundle-v1 archive.

From the repository root, offline checks are:

```powershell
node --test research/fidelity/2026-10-26-vps-timing/test_capture.mjs
python -B -m unittest discover -s research/fidelity/2026-10-26-vps-timing -p test_analysis.py
python -B -m unittest discover -s tools -p test_documentation.py
```

Acquisition drivers contact the frozen official endpoints and are not offline
tests. For analysis, verify the plan's source pins and raw manifests first,
then run `analyze.py --raw <scratch>` and, only after sealing development,
`analyze.py --raw <scratch> --validation`. Outputs refuse overwrite; preserve
originals and use a separate scratch copy for reproduction.

The actual raw argument was the local scratch subdirectory `vps-copy`.
Development analysis completed and was hashed in the local `analysis-seal.json`
before the validation command; validation records two missing batches, not a
successful held-out test. Python **3.13.13**, NumPy **2.5.3**, SciPy **1.18.1**
and acquisition Node **24.22.1** were used. The copied historical numerical
functions' original source hashes are retained; no dependency was installed on
the VPS host.

Verification: **7** study analysis tests, **1** study collector policy test,
**104** existing headless collector tests, **20** capture-quality tests and
**7** documentation tests passed. Both archives' gzip, prefix receipts,
protocol/setup and frozen source/plan hashes were checked. The frozen analysis
ran through development and missing-data validation without changing its source.
No Rust code changed or build was run. Passing offline tests does not make this
failed live acquisition eligible.
