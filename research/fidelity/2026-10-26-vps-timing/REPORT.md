# VPS official-server timing, 2026-10-26

The original attempt stopped after about one minute with an abnormal low-route
close (1006). An explicitly authorized continuation isolated the endpoints;
both then failed independently with 1006 after about 185/127 seconds. All four
partial recordings have valid gzip/integrity receipts but remain quarantined.
**Q1 and Q2 remain inconclusive: zero eligible batches, no held-out data.**
Across both attempts there were four connections, peak concurrency two, and no
retries. No simulator model, default or bot policy changed. Spawn admission
was outside scope.

## Original attempt: results and failure

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
change or fitted parameter. The independently authorized continuation below
preserves the original attempt and its quarantine.

## Original attempt: load and cleanup

The 60 load records contain 59 adjacent CPU/steal differences, sampled at about
one second. CPU percentages are per-sample descriptive summaries; host busy
excludes idle/iowait and includes steal, and container CPU is percent of one core.

| Covariate | Mean | p50 | Maximum |
| --- | ---: | ---: | ---: |
| Host one-minute loadavg | 0.700 | 0.555 | 1.290 |
| Host busy, four vCPUs | 12.016% | 10.127% | 77.722% |
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
and independently matched before cleanup. One root-owned scratch file needed
elevated removal. The study image was newly pulled
(absent from the saved pre-study image inventory) and removed without force.
Read-only checks at **23:33:53 UTC** verified that the remote scratch directory,
the study container and the pinned image were all absent. Existing service
containers, volumes and directories were not modified.

## Continuation

The owner explicitly authorized one continuation with independent endpoint
failure handling. The [continuation plan](continuation-plan.json), collector,
analysis wrapper and regressions were committed as
`f0593ae946c52f1566933f177a14b1236247ce12` **before any continuation socket opened**.
The two endpoints were rechecked against a fresh saved official list and both
remained active port-444 entries. The original plan, collector and primary
analysis are unchanged. The public text's subsequent privacy cleanup used a
new commit, preserving both frozen commit IDs and all executed source pins.

The continuation restarted the original three ten-minute rounds: rounds 1–2
development, round 3 held out. Questions, metrics, timer/stall mask and gates
compare exactly equal to the original plan. Each endpoint had its own
cancellation state; the first socket error/abnormal close stopped that endpoint
for all remaining rounds, without retry, reconnect or substitution. A normal
death could still rejoin after its socket closed. The same one-CPU, 1 GiB
ephemeral container and host networking were used. Original and continuation
recordings stayed in separate scratch trees and were never combined.

### Endpoint outcomes

Round 1 started at **23:41:07 UTC**. The second endpoint failed first, at
23:43:14.620; its failure did **not** cancel the low endpoint. The low endpoint
continued for another **57.7 seconds**, then failed at 23:44:12.296. Both emitted
`socket-error` with an empty message and close **1006** with an empty reason.

| Endpoint | Round 1 recorded span | Received raw frames | Game pings | Game RTT p50 / p99 ms | Rounds 2 / 3 |
| --- | ---: | ---: | ---: | ---: | --- |
| Low, `57.129.37.42:444` | 184.58 s | 18,258 | 723 | 6.768 / 11.169 | Skipped / skipped |
| Second, `198.244.231.35:444` | 126.90 s | 11,339 | 496 | 23.881 / 28.910 | Skipped / skipped |

Recorded spans run from the first connect metadata to the interrupted receipt
and include approximately 300 ms of draining. Raw counts include handshake/setup.
The coordinator invoked the later rounds only to record skipped outcomes;
**no game socket opened** in either. Continuation totals: **2 connections**,
**peak 2 including connecting sockets**, **0/6 completed batches**, **2 failed**,
**4 skipped**, **0 retries/rejoins**. Each endpoint had one successful join.
Both archives pass gzip and prefix-integrity verification; their sole audit
issue is `interrupted_capture`.

The **low endpoint is unusable from this remote Linux ARM64 VM for the planned
ten-minute captures under the tested setup**. It closed abnormally in both
authorized attempts, after different durations. A block remains possible but
unidentified. Successful gameplay before closure neither identifies a block nor
rules one out. The second endpoint also failed its continuation batch; its
original healthy prefix had been stopped by the original sibling-abort policy.
The empty close/error reasons do not establish a network, server or collector
cause. Neither endpoint was retried after its continuation failure.

### Questions and diagnostic RTT

**Q1:** still unresolved. Frozen analysis admits **zero excess frames / zero
clean eligible seconds** from the two interrupted continuation archives. No
admitted frame-excess p99, >10/20 ms episodes or run rates are available.
No transfer or attribution gate passed; the thresholds were not loosened.

**Q2:** still unresolved. Both scheduler streams are quarantined. Their gap and
ordering counters were not treated as zero observations, and no conclusion
about persistence or receipt burst compression was drawn. Held-out batches are
missing at both endpoints.

The original low-route diagnostic game-ping median **7.006 ms** exceeded the
reported pre-study ICMP RTT **1.64 ms** by about **5.37 ms**. The continuation
median **6.768 ms** gives a similar **5.13 ms** gap. This is a descriptive
application/pong-scheduling observation, consistent with server pong scheduling
and processing as well as protocol/timing differences. It is **not a fitted
claim**, a physical direction split or an identified server-wait distribution.
The RTT p99 column above is game-ping RTT, not frame-receipt excess p99.

**Proposed model change:** none. These failed acquisitions do not justify moving
pauses between link and server, revising a scheduler gate or fitting parameters.

### Continuation load, preservation and cleanup

The collecting round has 185 load records and 184 adjacent CPU/steal
differences. These include the two-client period and the final low-only period.
The skipped rounds have one startup/status load record each and no CPU-delta
samples; they are excluded from this table.

| Covariate | Mean | p50 | Maximum |
| --- | ---: | ---: | ---: |
| Host one-minute loadavg | 0.762 | 0.740 | 1.090 |
| Host busy, four vCPUs | 10.747% | 8.323% | 58.987% |
| Host steal | 0.338% | 0.252% | 2.284% |
| Capture cgroup CPU, one core | 12.423% | 12.834% | 18.789% |

Observed cgroup throttled-time increase was **0 µs**. Peak RSS was
113,700,864 bytes; max timer extra lateness / loop delay / receive cost were
**57.643 / 20.726 / 1.889 ms**. Host load and steal remain covariates; they
neither loosen gates nor identify the socket-error cause. All stall windows
remain in selected results.

All **36** remote files were copied back and matched the remote SHA-256 manifest
before cleanup. At **23:45:58 UTC**, checks verified that the remote scratch
directory, study container and newly pulled image were absent. No existing
service container, volume, service directory or host configuration was changed.
Full logs, original/continuation receipts, source snapshots and both copy-back
and cleanup verification records remain local-only and hash-pinned.

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

Collection used a remote Linux ARM64 VM with four vCPUs. Reported pre-study
ICMP RTT was 1.64 ms (mdev 0.017 ms) to `57.129.37.42` and 17.9 ms
(mdev 0.02 ms) to `198.244.231.35`. These are pre-study ICMP measurements,
not this study's game-ping observations or guarantees about later TCP traffic.

The official `https://slither.io/i80124.txt` response was saved with HTTP headers
and decoded before collection. Both IPs were active port-444 entries. The plan's
`officialList.fetchedUtc` contains an hour-offset transcription error; the
retained HTTP `Date` is **2026-10-25 23:25:58 GMT**, not 22:25 UTC. The original
plan remains unchanged; its hashes identify the correct saved response.

Each frozen budget is three ten-minute rounds, each with one existing forage
collector per endpoint. Rounds 1–2 are development; round 3 is untouched
held-out validation until development analysis is saved and sealed. All lives
at an endpoint within a round stay together. Two simultaneous endpoints are
different servers, not paired recipients of a single server. There are no
additional probes, controlled input interventions or spawn experiments.

## Collector and resource controls

Acquisition runs in a single ephemeral ARM64 Node container, with
`--rm --cpus=1 --memory=1g`,
one libuv worker, a read-only source mount, and an unprivileged identity.
`--network host` avoids Docker bridge/NAT timing; the preflight gauge used
`--network none`. The pinned image is
`node@sha256:55fb5f5a527ea2c5c4a95f963e48ef90aa38c7b346054fdc8ee9957a2b55d205`.
No host packages, configuration, production services, existing containers or
volumes are changed. Only `~/slither-capture-20261026/` is used remotely.

[capture.mjs](capture.mjs) instruments the existing headless forage collector;
controls are unchanged. In the original attempt an unexpected socket error/close
stopped both clients and the coordinator. The continuation's
[collector](capture-continuation.mjs) isolates endpoint failures and records
skipped outcomes in subsequent rounds. A normal death may rejoin only after
its socket closes. A datacenter IP could be blocked or rate-limited by slither.io;
acquisition failures are retained, not worked around.

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
seeds are specified for comparison on admitted schedules; both failed attempts
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
node --test research/fidelity/2026-10-26-vps-timing/test_capture.mjs research/fidelity/2026-10-26-vps-timing/test_continuation.mjs
python -B -m unittest discover -s research/fidelity/2026-10-26-vps-timing -p 'test_*.py'
python -B -m unittest discover -s tools -p test_documentation.py
```

Acquisition drivers contact the frozen official endpoints and are not offline
tests. For analysis, verify the plan's source pins and raw manifests first,
then run `analyze.py --raw <scratch>` and, only after sealing development,
`analyze.py --raw <scratch> --validation`. Outputs refuse overwrite; preserve
originals and use a separate scratch copy for reproduction.

The original raw argument was the local scratch subdirectory `vps-copy`.
Development analysis completed and was hashed in the local `analysis-seal.json`
before the validation command; validation records two missing batches, not a
successful held-out test. Python **3.13.13**, NumPy **2.5.3**, SciPy **1.18.1**
and acquisition Node **24.22.1** were used. The copied historical numerical
functions' original source hashes are retained; no dependency was installed on
the VPS host.

For the continuation, run `analyze_continuation.py --raw <continuation-copy>`,
then the same command with `--validation`. Its raw argument was the distinct
scratch subdirectory `continuation/vps-copy`. The wrapper checks every source
pin and exact equality of the frozen questions, metrics, telemetry and gates.
It writes development output and a seal before validation verifies that seal.
The continuation's validation again contains missing held-out batches.
[witnesses.py](witnesses.py) materializes already-declared receipt-only short-gap
and maximum-prey witnesses on admitted data; quarantine prevented its use here.
The original numerical/analysis functions were not changed or refitted.

Verification on the final revision: **10** Python study tests, **5** Node
collector policy tests and **7** documentation tests passed. The original
attempt also passed **104** existing headless collector tests and **20**
capture-quality tests. All four archives' gzip, prefix receipts, protocol/setup
and frozen source/plan hashes were checked. Both development runs and missing-data
validation ran without changing their executed source. No Rust code changed or
build was run. Passing offline tests does not make either failed acquisition
eligible.
