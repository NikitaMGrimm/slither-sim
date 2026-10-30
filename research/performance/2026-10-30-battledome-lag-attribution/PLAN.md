# Frozen Battledome wall-clock lag attribution plan

Question: what builds temporary wall-minus-tick debt at 320 constant-boost seats,
how does the loop repay it, and what does the independent socket client receive?
No production change or proposed fix is implemented. This plan precedes timing.

## Runtime and collection

Pin the current main revision recorded by the launch manifest (after #278).
Default portable release, locked dependencies, empty Rust flags, Rust 1.99.
Retain the pristine archive, study-only source overlay/prepare script, one source
manifest digest and binary hashes. Never edit this checkout's `src/`. The overlay
only observes the server loop and registers its simulation/socket thread IDs.

One c7a.medium Spot target, all server tasks on its one core. Same-AZ
c8a.4xlarge Spot generator: six decoding constant-boost received-frame A4 workers
on CPUs 0–5, protocol-19 #286 probe on CPU 6, controller CPU 7, capture CPU 8.
Reuse #286's native client/probe sources and tests by archive digest/reference;
every client decodes every frame, no authoritative bot observations. Shipped
Battledome arena/delivery, seed 7, 641 max players, no internal bots, no modeled
latency. Total seats include the probe. No owner hold or shared SSH guest.

Grow naturally: 79 bots for 300 s, 159 for 60 s, then 319 for 60 s. Warmup is
not counted. At 320, use six 60-s overhead windows, order OFF, LIGHT, TRACE,
TRACE, LIGHT, OFF, with 5-s transition settles. OFF disables observation in the
same binary; it retains the lightweight mode check, so this estimates incremental
observer cost, not a proof of zero code-layout effect against pristine main.
LIGHT records tick and pacing-wait boundaries plus proc scheduling counters.
TRACE additionally records scheduler/IRQ events if the target kernel supports
them. Keep all calibration windows and their client metrics, including failures.

Primary: three consecutive 300-s windows at 320, with 10-s transition settles.
Use LIGHT plus scheduler tracing if trace facilities work and TRACE's median
process CPU/tick overhead versus LIGHT is <=5% and <=0.15 ms absolute in the
calibration. Otherwise use LIGHT, publish the trace calibration, and retain the
resulting attribution ambiguity. No window extension or selection by lag size.
Report zero episodes honestly if they do not recur. If needed, one prospectively
declared additional 300-s LIGHT window is allowed solely to improve episode
coverage when the three primaries contain fewer than three episodes and budget/
timer reserve permit; report it separately as exploratory, never pooled silently.

## Observation and overhead

Per tick: scheduled deadline `(ticks + deferred + 1)*7 ms` from host epoch,
actual start/end on CLOCK_MONOTONIC, simulation thread CPU, socket CPU aligned
to the same boundaries, cumulative runnable-wait/context-switch counters for
both threads from `/proc/<tid>/schedstat`. Instrument pacing sleep entry/return
and requested wake deadline. Buffer fixed-size records, retain any write failure.
Read counters with cached descriptors; no per-tick JSON formatting or flushing.
Record reader overhead with native clock/read/write microcalibration and the
live OFF/LIGHT/TRACE comparison; retain calibration uncertainty and ecology drift.

Enable guest schedstats if required, verifying counters actually grow under a
short non-primary contention smoke. Record 10-Hz `/proc/stat` CPU/IRQ/softirq/
steal counters and host-thread inventory. Steal is guest accounting, not proof
of absent hypervisor interference when zero. Use scheduler switch/wakeup and
IRQ/softirq tracepoints when available, retaining event loss and collector CPU.
If unavailable, do not relabel all off-CPU time as socket contention or wakeup.
Check clock alignment; primary input/frames retain one generator monotonic clock.

Probe capture and timestamps follow #286, including explicit wanted echoes,
ICMP/control RTT and full client-frame metrics. Kernel diagnostics exclude a
connection capture with reported loss, and disclose coverage. Probe stays isolated.
Load qualification: each of six workers covers the entire window, CPU p50<70%,
p99<80%, input p99<42 ms, eligible boost action share>=99%, no decode/write errors.
A failed generator invalidates a primary but remains a reported failed attempt.

## Frozen episode and attribution definitions

Temporary start lag is actual tick start minus its scheduled deadline, including
deferred offset; also publish virtual-clock lag without that offset. An episode
crosses 20 ms after a recovered baseline and ends after five consecutive starts
at <=7 ms. Include the preceding tick/wait and following recovery ticks. Window
boundary episodes are censored and not counted twice. Report peak, duration,
growth and recovery separately; show the count at thresholds 7/20/50/100 ms.

For every episode retain measured world CPU over budget, simulation runnable wait,
socket CPU while simulation waits, pacing requested/actual wakes, other scheduled
tasks, kernel/IRQ and steal evidence. Report overlapping cause tags and fractions
of episodes, not an invented exclusive causal percentage. Also give time-accounting
shares with explicit unknown/overlap bounds. With only proc counters, socket
contention during a tick is bounded by `max(0,socketCPU-blockedOffCPU)` and
`min(socketCPU,simRunnableWait)` (sampling skew is uncertainty); do not treat the
upper bound as measured overlap. Wake deadline overshoot minus runnable wait
is a blocked-wakeup/other-preemption bound, not automatically a timer defect.
Trace events, when usable, narrow these bounds by interval intersection.

Rank (a) world-work CPU bursts, (b) socket contention while sim runnable,
(c) pacing/wakeup delay, (d) other task/kernel/IRQ/steal, plus unattributed time.
Report both event counts and measured/bounded time shares. Separate observation
overhead and controller/collector activity from production work. Do not infer
precise historical #281 causes from a new episode; compare recurrence only.

Debt repayment: count consecutive ticks with no pacing wait, catch-up batch sizes,
effective tick rate and deferred increments. The source caps catch-up batches at
eight and defers backlog beyond approximately 252 ms; verify actual behavior.
Align independent probe frame timestamps to each growth/recovery interval and
report raw/header-adjusted gaps, excess, stalls >20/100/250 ms and bunching
(adjacent frame gaps <1 ms), with non-episode controls. Sparse emission is not
itself a stall. Clock alignment and coarse telemetry limit causal attribution.

## Budget, safety, retention and delivery

Hard combined $3 cap, Frankfurt Spot only, explicit `slither-bench`. Tags at
creation on instances, volumes, key and group. Fresh task group. Default bid caps
target $0.06/h and generator $0.50/h; in-guest shutdown at 130 minutes (100 planned
plus 30 reserve), terminate behavior and DeleteOnTermination. Conservative
132-minute allocation plus $0.65 noncompute reserve is $1.882. No timer extension;
any replacement must fit spent cost plus its full timer and reserve below $3.
Stop on a denied required API, report the call, never work around permissions.

Close/member-verify/hash each archive before atomic readiness, download and verify
before source deletion/termination. Secure native source/binaries before timing
and complete per-window checkpoints between windows, with a target mirror.
Keep failures, interruptions, rejected qualification, trace loss and failed
analysis in ATTEMPTS.md. Raw data outside Git, lean selected episode summaries
and what recomputes tables in Git. No identifiers/IPs/account/hostnames public.

Terminate task resources, remove task key/group and private key, scan all enabled
regions for instances, active/open Spot requests, all volumes and Elastic IPs;
preserve/disclose unrelated resources. Research index only, no changelog. PR
template Change/Validation/checks plus one final Agent-assistance footer. No merge.
Rank unimplemented fixes as exact (same bytes and tick schedule) versus design;
priority, deadline-aware egress and repayment changes are design proposals for
the owner's local play server before a production PR.
