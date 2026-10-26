# Remote official-server timing, 2026-10-26

The original attempt and first continuation misclassified collector-requested
post-death closes as endpoint failures. That interpretation and the endpoint-
unusable conclusion are withdrawn; a block is not indicated. Their four raw
streams remain unchanged and quarantined.

With corrected life-ending handling, the second continuation completed all six
ten-minute batches: **38 connections, 32 deaths and 32 rejoins, peak concurrency
2**, and no unrequested socket error/close. Both held-out batches pass the frozen
clean-exposure gates and classify as **near-zero**: clean frame-excess p99
**2.322 / 2.372 ms**, with one >10 ms episode each. Server-clock motion/mass,
reverse-order and long prey-correction counterexamples persist. No simulator
model, default or bot policy changed; no new numeric replacement model is fitted.
Spawn admission stayed outside scope.

## Original attempt: results and failure

At **2026-10-25 23:30:15 UTC** round 1 opened one socket per endpoint. At
23:31:14.794 the low endpoint emitted `socket-error` with an empty message,
followed by close code **1006** and an empty reason. The study controller aborted
both clients; the coordinator exited 1 at 23:31:15. The second connection was
stopped by that abort, not independently observed to fail. The socket error
followed the collector's requested post-death close; it was not evidence of an
independent server drop. No login/Origin change, workaround, retry or endpoint substitution
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

### Audit correction: requested closes

The [full-stream audit](death-close-audit.json) preserves compressed source
hashes, one-based JSONL lines, death-packet bytes and actual recorded timestamps.
All three stopped clients were in life 1 with one join. Each had exactly one
death-observation start, close request and end. Both `v` and own death-removal
packets occurred in the listed death frame. Times below are milliseconds on
each process's monotonic receipt clock, not invented from event counts.

| Stream | Death packet receipt | Observation start | Close requested | Socket error | Observation end | Close 1006 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Original low | 58529.562 | 58529.835 | 59543.924 | 59771.596 | 59772.450 | 59772.516 |
| First continuation low | 183365.001 | 183365.300 | 184374.067 | 184459.638 | 184459.925 | 184459.950 |
| First continuation second | 125643.057 | 125643.357 | 126650.746 | 126780.560 | 126781.349 | 126781.397 |

The base collector sets `deathObservation.closeRequestedAt` before calling
`ws.close()`. Its close handler records the death-observation end and normally
schedules a rejoin. The original wrapper aborted both clients on any socket
error; the first continuation aborted that endpoint and also rejected 1006
closes even after a requested death close. Thus the wrapper stop rules fired
on requested life endings. The original second stream has no death packet,
death-observation, close-request, socket-error or close event; its terminal
receipt records the sibling abort.

This establishes event order and wrapper classification. It does **not** show
that requesting the close caused the socket error, nor identify transport or
server behavior during close. The claimed independent endpoint failures and
low-endpoint unusability are withdrawn. **A block is not indicated.** Historical
frozen plans and terminal error strings remain as recorded; their erroneous
classification is corrected here and in selected results, not applied as a new
exclusion or silently rewritten into the raw streams.

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

## First continuation

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

Round 1 started at **23:41:07 UTC**. The wrapper stopped the second client first,
at 23:43:14.620; this did **not** cancel the low client. The low client continued
for another **57.7 seconds**, then was stopped at 23:44:12.296. Both sequences
followed their own first-death close requests, as audited above, and emitted
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

The recorded failed outcomes reflect the wrapper's misclassification, not
established server-initiated drops. The previous claim that the low endpoint
was unusable from this VM is withdrawn. A block is not indicated. Neither
endpoint was retried after the wrapper stopped its first batch; both partial
archives remain quarantined under their original interrupted receipts.

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

## Second continuation: corrected life endings

The [second plan](second-continuation-plan.json), corrected collector, analysis
wrapper and regressions were frozen in **`a19f32db04ae44bac22cf923fe0a952d8684445f`**
before any new game connection. A merge, `a20b5ac`, preserved that commit and the
existing/rebased PR histories without rewriting either. All executed script pins
match; the PR changes no simulator sources relative to its main base.

The [corrected wrapper](capture-second-continuation.mjs) uses the base collector's
recorded `deathObservation.closeRequestedAt` intent before `ws.close()`. Following
error/close events, including 1006, end that life and may rejoin through the base
collector after close. Intent resets for each new socket. An unrequested error
or close stops only its endpoint for the rest of the run without retry. The
historical wrappers remain frozen for the earlier attempt records.

A regression drives the actual base post-death tick, requested close, socket
error, close handler and rejoin through a fake socket, then checks that an
unrequested error in the next life is fatal. Other tests cover errors/closes
before a death-close request and unchanged independent endpoint cancellation.

### Acquisition and split

The second run used the same official endpoints, Origin, unchanged forage
controls, one-CPU/1-GiB container, image and host networking. It ran on
**2026-10-26, 00:02:47–00:33:48 UTC**, with 30-second round spacing. The original
and first-continuation streams were never combined with these recordings.

| Batch | Split | Outcome | Joins | Deaths | Rejoins |
| --- | --- | --- | ---: | ---: | ---: |
| low-1 | development | Completed | 7 | 6 | 6 |
| second-1 | development | Completed | 8 | 7 | 7 |
| low-2 | development | Completed | 8 | 7 | 7 |
| second-2 | development | Completed | 5 | 4 | 4 |
| low-3 | heldout | Completed | 6 | 5 | 5 |
| second-3 | heldout | Completed | 4 | 3 | 3 |

There were **6/6 completed batches**, **38 connection attempts including 32
normal rejoins**, **peak concurrency 2 including connecting sockets**, and **0
unrequested socket errors/closes or failure retries**. All 32 death observations
had a recorded close request, followed by retained socket-error/1006 events;
the base collector rejoined normally. Every new gzip/prefix receipt verifies
with no quality issues. This validates the wrapper correction, without proving
what causes the error event in the requested close sequence or re-admitting
any previous interrupted archive.

All four development captures were copied and hash-checked after their rounds.
Development analysis was sealed at **00:23:59.893948 UTC**, SHA-256
`83c6d28257af988f0f1034ef49bbb236588b843e5ee3645513e99c160c8db9f9`.
Round 3 had started acquisition at 00:23:48, but no held-out timing was inspected
before that seal. Final copy-back verified the development bytes were identical
to the sealed inputs. Validation checked the seal and unchanged analysis-source
pins before reading held-out data. No fitting, threshold change or new question
was introduced.

### Q1: the dominant historical tail largely vanishes here

All six batches pass the frozen exposure criteria. Each held-out route has
more than 300 clean eligible seconds and more than 80% clean/full exposure;
both meet the predeclared near-zero criterion, p99 ≤3 ms and >10 ms run rate
≤0.05/s. Exposure omits short setup epochs, burn-in, rejoin gaps and masked
intervals; it is not the full 600-second wall budget.

| Batch | Split | Clean seconds (% eligible exposure) | Excess p99 full / clean ms | Clean >10 / >20 ms episodes | Clean >10 runs/s |
| --- | --- | ---: | ---: | ---: | ---: |
| low-1 | development | 544.282 (94.81%) | 2.430 / 2.374 | 1 / 1 | 0.001837 |
| second-1 | development | 527.486 (95.05%) | 2.396 / 2.355 | 1 / 1 | 0.001896 |
| low-2 | development | 472.149 (92.00%) | 2.673 / 2.474 | 0 / 0 | 0.000000 |
| second-2 | development | 537.060 (92.31%) | 2.729 / 2.502 | 0 / 0 | 0.000000 |
| low-3 | heldout | 508.886 (88.04%) | 2.515 / 2.322 | 1 / 1 | 0.001965 |
| second-3 | heldout | 516.900 (88.20%) | 2.549 / 2.372 | 1 / 0 | 0.001935 |

Held-out clean excess p50/p90/p99 is **0.636/1.096/2.322 ms** on low and
**0.661/1.102/2.372 ms** on second. The full held-out >10 ms counts/rates are
**19/0.032873 per second** and **20/0.034126 per second**; full >20 ms counts are
11/10. Masking leaves the clean 1/1 >10 ms episodes shown above and 1/0 >20 ms
episodes. Clean peak excess is **23.514/19.531 ms**: rare events remain.
The clean >10 ms episodes have observed durations **0.078/0.050 ms** and
surrounding-receipt brackets **30.868/24.634 ms**. The low >20 ms episode is a
single receipt, with observed duration zero. These durations are lower bounds,
not measured physical pause lengths. Full maximum excess is 63.932/67.875 ms;
full results and all excluded windows remain retained.

For comparison, #108's CPU-contaminated held-out full p99 was **29.9/110.2 ms**
and >10 ms run rates **2.598/4.831 per second**. Its empty/nearly empty clean
subsets do not provide a clean-host reference. The older network-path held-out
pooled excess p99 was **9.21 ms**, with >10 ms runs **0.351/s**. The same frozen
model on the new held-out server schedules predicts clean p99 medians across
three seeds of **13.424/13.468 ms**. All three seeds fail p50, p90, p99 and burst
transfer gates on both routes. ACF gates at 50/100/500/1000 ms pass on both;
10 ms ACF fails on low and passes on second. Every per-seed gate remains in
selected results, including the passes.

**Answer:** the large effective delivery tail measured at home does not persist
on the low-RTT remote route under these gates. These observations do not support
moving the fitted pauses into a shared server-processing process. They support
treating that fit as route/collector-conditioned effective delivery, rather than
a universal server law. This comparison does not separate home-path effects
from the earlier collector/platform effects, prove the rare events' origin, or
identify synchronized cross-client server delay. No kernel-arrival timestamps
or same-server paired recipients were collected.

The held-out game-ping medians are **6.323/23.519 ms**, separate from frame excess.
The previously recorded 7.006 ms game-ping versus reported 1.64 ms ICMP gap
remains a descriptive application/pong-scheduling observation, not a fitted
wait law or one-way propagation estimate.

### Q2: clean server-clock exceptions persist

The following are fully clean intervals on the **server elapsed-header clock**,
within one connection and uninterrupted entity appearance. They are not merely
short receipt intervals. Counts include own/other snakes and correlated reports;
they are not independent-trial confidence estimates.

| Held-out batch | Motion <35 ms / gaps | Standalone mass <100 ms / gaps | Reverse motion-before-mass pairs | Repeat prey >500 ms / gaps |
| --- | ---: | ---: | ---: | ---: |
| low-3 | 119 / 61665 (min 24 ms) | 29 / 17544 (min 87 ms) | 0 | 1 / 878 (max 3044 ms) |
| second-3 | 142 / 64290 (min 24 ms) | 36 / 19459 (min 84 ms) | 1 | 0 / 985 (max 355 ms) |

Both held-out median motion gaps are **43 ms**, mass gaps **107 ms**, and repeat
prey gaps **338 ms**. The short motion/mass exceptions replicate the development
counterexamples in all four batches. Literal wire minima of 42/105 ms therefore
miss real clean-path observations. An exact hidden six-/15-physics-tick rule
cannot be inferred or rejected solely from these variable send headers.

There are also **2/1** clean receipt-only motion <35 ms crossings and **11/14**
receipt-only mass <100 ms crossings whose corresponding server gap meets the
threshold. Those are compatible with delivery variation/compression and integer
clock effects. They do not explain the **119/142 server-clock motion** or
**29/36 server-clock mass** exceptions. The reverse pair is inside one ordered
frame and cannot be created by downstream burst compression. Its retained
second-route witness is connection 2, frame 19360, S=99234, entity 221: motion
`330700` precedes mass `6800ddaccc18`; death packets share that frame. This is a
counterexample to absolute ordering, not a recovered universal death scheduler.

The long-prey counterexample persists on low: prey **27656**, connection **6**,
frames **57677→58052**, S=**183621→186665**, receipt
**528229.429→531273.821 ms**. The gaps are **3044 ms server / 3044.392 ms receipt**,
with **374 intervening frames** and no intervening same-prey `j` or `y` entry,
removal or reappearance. The whole interval passes the clean mask. This witness
was checked directly against the raw frames; its archive digest and packet
bytes are selected. It exceeds the earlier retained 2694 ms example and cannot
be dismissed as downstream burst compression. The second route's full 675 ms
example crosses a masked interval and is retained only as unfiltered evidence;
its clean maximum is 355 ms.

No clean server-clock repeat-prey gap is below 210 ms on either held-out route. Speed-11
subsets have median 213/214 ms, but **9/10 clean sub-300 ms gaps** have neither
previous nor current reported speed exactly 11. Thus an exclusive exact-11
fast-correction guard misses clean cases too. The hidden flight/pulse state,
trigger and rearm rule remain unidentified; absence of sub-210 ms cases here
cannot erase historical shorter-gap counterexamples.

**Answer:** sub-35 motion, sub-100 mass, reverse same-frame ordering and long
repeat-prey gaps all persist with clean receipt timing. Only the separately
counted receipt-only threshold crossings can be attributed to receipt-clock
compression/variation at this level; packet-only data do not identify its
physical mechanism. The strict adopted gates and common ordering remain model
choices with counterexamples, not exact official rules.

**Model proposal:** no numerical replacement law is fitted or adopted. Do not
promote the retained route pauses to a shared server process on this evidence.
The scheduler exceptions need continued explicit qualification; these recordings
do not identify a replacement trigger/rearm mechanism or justify a new fitted
stochastic exception model. No model/default changed in this PR.

### Load, access and cleanup

Per-second covariates are retained for all rounds. Host busy excludes idle and
iowait and includes steal; host percentages cover four vCPUs, while capture CPU
is percent of one core. Table entries are descriptive sample means/maxima,
including the final partial sample in each round.

| Round | Loadavg mean / max | Host busy mean / max % | Capture CPU mean / max % | Steal mean / max % | Throttled ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| round-1 | 0.739 / 1.750 | 15.659 / 85.213 | 17.133 / 28.962 | 0.598 / 2.778 | 3.067 |
| round-2 | 0.457 / 0.850 | 14.132 / 72.519 | 16.622 / 34.336 | 0.563 / 3.817 | 18.058 |
| round-3 | 1.053 / 2.190 | 20.978 / 99.501 | 17.611 / 28.844 | 0.988 / 9.367 | 0.000 |

Across the run, mean loadavg was **0.750**, host busy **16.923%**, capture CPU
**17.122% of one core**, and steal **0.717%**. The respective maxima were 2.190,
99.501%, 34.336% and 9.367%. Observed cgroup throttled-time increase totaled
**21.125 ms**; peak process RSS was **135,753,728 bytes**. This was not an entirely
idle or stall-free host. No mask or gate was loosened for the load spikes.

All **49** remote files matched their copy-back SHA-256 manifest. At
**00:34:43 UTC**, checks verified that the remote scratch directory, study
container and newly pulled image were absent. Existing containers, volumes,
service directories and host configuration were not modified.

Full inputs/outputs remain local-only under the existing scratch root's
`second-continuation/` subtree. The live status prefixes are retained separately
and were never timing-analysis inputs. A local operational formatter initially
omitted request/end metadata from its retained event list; that incorrect derived
summary and formatter are preserved under `superseded-derived/`. The corrected
formatter re-read unchanged raw archives. Frozen primary metrics, development
seal and validation were unaffected. Source pins identify both the correction
and superseded outputs; public lifecycle counts were checked against raw order.

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
node --test research/fidelity/2026-10-26-vps-timing/test_capture.mjs research/fidelity/2026-10-26-vps-timing/test_continuation.mjs research/fidelity/2026-10-26-vps-timing/test_second_continuation.mjs
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

For the second continuation use `analyze_second_continuation.py --raw <raw-copy>`,
then the same command with `--validation`, with the pinned source and a fresh
copy of the raw files. Development and its seal were first written under
`second-continuation/development-copy`; identical capture hashes and the unchanged
output/seal were verified in final `second-continuation/vps-copy` before validation.
Legacy collector tests retain the first two frozen policies for their historical
attempts; `test_second_continuation.mjs` covers the corrected active policy.

Verification on the final revision: **11** Python study tests, **8** Node
collector policy tests and **7** documentation tests passed. The original
attempt also passed **104** existing headless collector tests and **20**
capture-quality tests. All ten archives' gzip, prefix receipts, protocol/setup
and frozen source/plan hashes were checked. The earlier missing-data validation and the second continuation's full development/held-out analysis ran without changing their executed source. No Rust code changed or
build was run. Passing offline tests did not re-admit any of the four earlier interrupted archives.
