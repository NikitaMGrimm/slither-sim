# Mature 500-socket capacity checkpoint, 2026-10-28–29

**500 real sockets sustain about 142.8 completed ticks/s in the accepted short
mature windows, but p99 world CPU work still exceeds seven ms.** Post-#255 world
CPU medians range **4.281–4.921 ms/tick**, leaving **2.079–2.719 ms** of median
headroom; p99 ranges **8.848–9.200 ms**, exceeding the budget by **1.848–2.200 ms**.
All six capacity cohorts pass their client, queue and accounting gates.
Separate PMU and phase diagnostics expose lower IPC and a periodic food-maintenance
tail. No sustained-uptime claim is made.

[Capacity results](results.json), [PMU/phase results](diagnostics.json),
[primary tail results](tail-results.json), [verification](verification.json),
[plan](PLAN.md), [attempts](ATTEMPTS.md), [recipe](REPRODUCE.md).

## Question and scope

Do the exact host optimizations since [#248](../2026-10-28-official-density-capacity/REPORT.md)
leave enough time for 500 real socket players within a one-core 7 ms tick?
Compare #248 runtime `d127acd1f0e9db2100c8a1dd5c2fa43e2deff506` against
merged #255 main `551cb1030ea2bf84f83bbeb167233b917900929a`, using one
harness and one session on a 4-vCPU Zen 5 EPYC KVM guest. This is a research
checkpoint; no new production code or optimization is delivered.

The workload has 500 remote seats and zero in-process bots. Every player uses a
real loopback TCP WebSocket, protocol 19 and the normal client-equivalent
physical-food/rules opt-in. Three workers decode every delivered frame and run
the unchanged received-frame A4 policy every 42 ms, with normal ping cadence.
They share stateless policy scratch, as the native host workers do. All use the
same pinned client binary, built from the #248 base; no authoritative observations
or host snapshots drive the clients.

Simulation and socket tasks share vCPU 0. Client workers use vCPUs 1, 2 and 3;
CLI statistics and low-rate collection use vCPU 3. Every task's affinity is read
back before and throughout collection. Defaults retain seed 7, official world
and delivery, 7 ms physics, seat-following radius and zero artificial latency.
Live wire/physics scheduling remains asynchronous; the two worlds are not
trajectory-identical and the results are not an exact paired speedup estimate.

## Mature warmup and accounting

The full host advances from tick zero. Clients connect after a one-second host
startup delay, staggered by three ms per seat/worker. Keep 100 logical clients
resident; retire/rejoin 400 round-robin, two per 100-tick census until tick
180,000, then continue naturally. Food/wire history is never restored or edited.
At or after tick 193,000, three successive 500-tick endpoints must have >=490
living, mean length 45–65, p90 156–196 and p99 >=200. These tolerances define a
mature class, not stationarity. Every sampled census in each primary window must
stay in that class.

Alternate three fresh cohorts per runtime in one guest session. Discard five
seconds after screening, then count 30 seconds and run a separate 15-second
continued-world syscall diagnostic. The original 300-second hold drifted from
mean length 59.064 to 86.478 and is excluded. One second-base attempt failed the
unchanged quiet screen (0.320 other-process CPU seconds versus 0.294049 allowed)
and was repeated once for interference, never its timing outcome. Both attempts
and the 21:30 UTC reporting restart are retained in [ATTEMPTS](ATTEMPTS.md).
The capacity collection, including failed work and allowances, costs 19,797.98
CPU seconds (5.50 hours), below its six-hour ceiling.

Read simulation and socket task CPU clocks at consecutive completed-tick
boundaries. Their sum includes user/kernel work and socket polling between
ticks, excluding client CPU, CLI work and sleep/preemption. It also includes
instrumentation/reporting CPU charged to the next tick. Retain complete
100-tick blocks inside each window and reconcile mean CPU with `/proc` within
max(0.03 ms/tick, 2%). Add aligned tick values before computing world quantiles;
independent component quantiles cannot be added. Phase children nest inside
simulation/broadcast. The separate syscall brackets include clock/wrapper
overhead and use interpolated tick denominators; they are not a primary
CPU decomposition.

Client/queue gates and full process-command inventories are checked separately
for each window. Raw samples, inventories, source copies and binaries stay
private. The [plan](PLAN.md) and [recipe](REPRODUCE.md) retain thresholds,
reconnect handling, failed attempts, source reconstruction and negative controls.

## Separate explanatory diagnostics

After all capacity cohorts, separately built archived-copy hosts repeat the
same tick-zero warmup and common 500-socket client. For each runtime, count a
12-second simulation-thread user-only instruction/cycle group, its IPC and task
user/kernel CPU; require >99% event scheduling. Tick denominators interpolate
between census edges. Broad CPU phase averages use complete interior blocks
with a separate denominator.

Then collect 20 seconds of coarse per-tick phases/events with per-recipient
clocks disabled, followed by ten seconds of fine broadcast subphases. Fine
clocks add overhead and cannot replace capacity or coarse-tail results. Rank
conditional phase means over the slowest 1%/2%; inspect food/HUD opportunities,
anchors, deaths, rejoins, subscription changes and neighboring socket intervals.
Logging cadence is explicit. Five-ms vCPU observations retain the underlying
`USER_HZ` counter quantum: overlapping IRQ/softirq/steal charges provide context,
not exact interrupt attribution to seven-ms ticks. Task system CPU and vCPU
categories are separate accounting views, not additive slices.

These newly requested diagnostics have a separate two-CPU-hour ceiling. Full
client headroom, queues and CPU reconciliation are checked in each of their
three windows. Their synthetic arithmetic control recovers known 72-tick spikes
and the expected top-1%/2% and IPC values.

## Historical context

#248's published reference used **500 in-process client-equivalent bots plus one
real WebSocket player**, hence 501 occupied seats. Its world-plus-one-socket CPU
cost was 5.44, 5.10 and 4.86 ms/tick across three holds: median **5.10 ms/tick**,
range **4.86–5.44**. Those values are whole-hold means normalized by ticks, not
per-tick p50/p99. The corresponding length means moved roughly 94 to 79 during
its first hold, with later drift also reported there. All server tasks, including
bot workers, shared one vCPU in those original holds.

Those historical numbers omit 499 of this checkpoint's real sockets, have a
different length/input/scheduling mix, and are context only. They are not a
same-work control, a remeasurement or evidence of a 500-socket service. The two
runtime pins in this study are the direct 500-socket comparison.

## Results and decision

Each cell gives the median of the three cohort percentiles, followed by its
observed cohort range. Units are CPU ms per completed tick, user plus kernel.

| Scope | #248 base p50 (range) | Post-#255 p50 (range) | #248 base p99 (range) | Post-#255 p99 (range) |
| --- | ---: | ---: | ---: | ---: |
| World: aligned simulation + sockets | 5.064 (4.981–5.161) | 4.587 (4.281–4.921) | 9.139 (9.019–9.808) | 8.908 (8.848–9.200) |
| Simulation/broadcast | 3.141 (3.102–3.229) | 2.517 (2.306–2.723) | 6.434 (6.308–6.792) | 5.787 (5.483–5.994) |
| Socket I/O | 1.933 (1.892–1.936) | 1.985 (1.950–2.166) | 3.214 (3.032–3.246) | 4.081 (4.004–4.226) |

Component quantiles are separate and must not be added. All cohorts retain
4,200 interior ticks: 12,600 ticks per runtime, about 142.81–142.83 ticks/s,
with zero deferred ticks. World mean CPU ranges 5.121–5.343 ms/tick on base and
4.385–5.035 on post-#255. These are descriptive asynchronous-world comparisons,
not a same-trajectory optimization percentage or an interval estimate.

Measured mean-length ranges are 54.148–60.582 on base and 54.352–60.780 on
post-#255. P90 ranges are 175–193 and 179–194; p99 243–251 and 243–253. Living
snakes stay at 499–500. Food ranges 27,865–29,344 and 26,788–30,444 respectively.
The complete per-cohort aggregates and denominators belong in selected results.

Full received-frame A4 clients were used throughout; no thin substitution.
They use 1.111–1.162 cores on base and 1.116–1.242 on post-#255 out of the other
three vCPUs. The largest worker p99 CPU fraction is 43.1% / 47.4%, and worst
one-second input-lateness p99 including boundary reports is 3.470 / 6.390 ms.
Transport counters bracket the full windows and remain zero across the entire
runs. All windows have zero admission
refusals, write WouldBlocks and buffer-full events. Application send queues are
zero at every socket census; kernel queue maxima are 24,221 / 15,356 bytes and
largest fitted growth slopes 36.7 / 25.1 bytes/s, well below 8 KiB/s. Accepted
windows contain 11–21 natural deaths/rejoins and zero forced retirement.

## Instructions versus CPU time

The real-socket diagnostics use one client-valid 12-second PMU window per
runtime. Both events run 100% of their enabled time. The first base window
qualified late and drifted to mean length 64.614–65.838 (p90 190–192), whereas
post-#255 stays at 56.452–57.740 (p90 183). They are mature but **not a matched
primary-body comparison**. A fresh base attempt stayed in class but failed the
unchanged client-cadence gate: worker input-lateness p99 97–109 ms despite CPU
headroom. Its host timings and PMU counts are excluded. This leaves a strict
matched-body PMU repeat unqualified, not a reason to relax the gate.

| Simulation-thread diagnostic | #248 base, drifted | Post-#255 |
| --- | ---: | ---: |
| User instructions/tick, M | 22.779 | 9.535 |
| User cycles/tick, M | 14.863 | 10.215 |
| IPC | 1.533 | 0.933 |
| User CPU ms/tick | 3.396 | 2.316 |
| Kernel CPU ms/tick | 0.047 | 0.099 |
| Cycles per user-CPU second, GHz equivalent | 4.377 | 4.410 |
| Broad broadcast CPU ms/tick, interior blocks | 2.572 | 1.571 |

In these observed windows, instructions fall 58.1% but cycles only 31.3%:
IPC falls 39.1%, while cycles per user-CPU second differ by only 0.76%.
The accounting identity is `user CPU = instructions / IPC / cycles-per-CPU-second`.
It reproduces 3.396 -> 2.316 ms rather than an instruction-proportional time cut.
Lower IPC is measured; a cache or memory bottleneck is not identified by these
counters. Changed work mix is a plausible interpretation, not a measured cause.

The fixture's roughly 53% instruction reduction therefore cannot be read as a
53% CPU-time reduction. Its direct-core controls, fixed 360 advances and excluded
host commands/frame draining also differ from live decoded-view controls and
socket/lifecycle scheduling. In current-main coarse diagnostics, command
processing plus handover is only 0.043 ms/tick, so that omitted host bookkeeping
is not the dominant observed gap. Broadcast still costs 1.579–1.895 ms/tick in
primary windows, other physics remains, and about two ms/tick of socket work
further dilutes whole-world time savings. The initial 12% simulation-median
change is one asynchronous pair; all cohort ranges are reported above, without
claiming a same-trajectory causal speedup.

## Remaining measured CPU budget

Primary mean ranges, ms/tick. Nested rows are not additive to their parents.

| Work | #248 base | Post-#255 |
| --- | ---: | ---: |
| Socket task, total | 1.932–1.996 | 1.992–2.213 |
| Socket kernel CPU, within total | 1.587–1.610 | 1.662–1.808 |
| Broadcast, within simulation | 2.403–2.517 | 1.579–1.895 |
| Pickup/food cache, within simulation | 0.224–0.238 | 0.216–0.267 |
| Collision queries, within simulation | 0.173–0.183 | 0.187–0.197 |
| Collision construction, within simulation | 0.081–0.085 | 0.081–0.093 |
| Movement, within simulation | 0.078–0.082 | 0.078–0.090 |

Separate continued-world syscall diagnostics put writes at 1.404–1.533 ms/tick
on base and 1.360–1.466 on post-#255; readiness polling at 0.127–0.138 and
0.134–0.148; reads at 0.119–0.124 and 0.113–0.121. These brackets include clock
and wrapper overhead and use interpolated tick denominators. They rank socket
work; they are not an exact decomposition of the primary socket CPU samples.

## Primary tail cadence

A separate read-only reduction verifies the source-derived food-maintenance
opportunity against 8,600 measured diagnostic ticks in both arms, then applies it to
all six unchanged primary traces. It reproduces every selected primary p99.
On current main, 88.1–97.6% of the slowest 1% of ticks coincide with this
nominal 504-ms (about 72-tick) cadence; the slowest 2% contain 69.0–70.2%
maintenance ticks. None of the current-main top 1% is a post-report tick.
This corroborates the periodic tail in the primary measurement, independently
of the finer diagnostic logger.

Current-main top-1% conditional means are 9.748–10.166 world ms/tick, comprising
5.922–6.600 simulation and 3.385–3.826 socket ms/tick. These aligned conditional
means add; they are not a sum of independent p99s. Food-maintenance tick means
are 9.303–9.710 ms versus 4.311–4.962 for ticks that are neither maintenance nor
post-report. The current-main coarse window retains 2,800 ticks and stays in class
(mean 57.932–60.488, p90 180–183). Its slowest 1%/2% have these aligned
conditional mean CPU costs, ms/tick:

| Disjoint work | Top 1%: 28 ticks | Top 2%: 56 ticks |
| --- | ---: | ---: |
| Socket task | 3.905 | 3.315 |
| Broadcast | 3.452 | 3.190 |
| Subscription advance | 0.768 | 0.647 |
| All bracketed physics | 0.584 | 0.604 |
| Commands + handover | 0.049 | 0.051 |
| Simulation residual, including diagnostic reporting | 1.009 | 1.332 |

Thirty-nine food-maintenance events are exactly 72 ticks apart. Twenty of the
28 slowest coarse ticks are maintenance ticks; seven occur just after the
100-tick diagnostic log flush, where tick % 100 == 1. `postReportTickCount` in
[diagnostics](diagnostics.json) counts such tail ticks; the retained
`analyze_diagnostics.py` does not emit it, so recount it from the detail rows
with that predicate. That reporting contribution is instrumentation,
not a production target; the unchanged primary traces above corroborate the
food tail without it. There are zero minimap, leaderboard, death or connection
flags in the coarse top 1%. Anchors occur on every tick and average 15.61 on
both all ticks and the top 1%, so there is no observed anchor-count burst.
The top 2% contains one minimap opportunity and no leaderboard/death/rejoin flags.
Maps run every 357/358 ticks and boards every 429/430, not at the dominant
72-tick cadence.

In the separate fine window, the top 1% is 13 ticks. Nested broadcast means are
recipient food 1.634 ms, preparation 0.613, snake visibility/encoding 1.307,
and frame emission/grouping 0.252. Subscription advance is 0.784 ms. Food and
preparation rise from all-tick means 0.260 and 0.142; HUD opportunities do not
explain these slow ticks. Fine clocks add work: simulation mean is 2.898 ms
versus 2.369 in the preceding coarse window, also with an evolving world. No
subtracted overhead correction or exact pristine subphase time is claimed.
These fine children must not be added to the coarse broadcast parent.

The host-vCPU observations overlapping the coarse top 1% show zero IRQ and
steal counter increments, with 70 ms softirq over 90 overlapping sample
intervals (0.478 s total). Top 2% has zero IRQ/steal and 100 ms softirq over
174 intervals (0.921 s). The counters have a 10-ms quantum despite five-ms
sampling; these are overlapping-interval totals, not exact per-tick interrupt
charges. Task system CPU already covers kernel work and is not augmented by
these totals.

## Next measured targets

1. **Periodic food/subscription maintenance and its send burst.** It dominates
   primary top-1% membership; current-main diagnostics expose recipient food,
   shared preparation, subscription cleanup and socket service as the relevant
   costs. Investigate exact reuse/rejection while preserving opportunity times
   and wire semantics; no saving is assumed here.
2. **Steady socket write/kernel work.** Primary socket CPU is 1.992–2.213 ms/tick,
   mostly kernel; separate writes cost 1.360–1.466 ms/tick with bracket overhead.
   This is real player work, not a bot-only path.
3. **Remaining broadcast visibility/encoding.** Broadcast averages
   1.579–1.895 ms/tick overall; collision queries are only 0.187–0.197 ms/tick
   in primary windows and do not spike materially in the observed tail.

No optimization, structural reorganization or new encoding/transport coupling
is delivered by this checkpoint.

## Verification and limits

Both primary hosts and separate diagnostic hosts compile offline with the same
Rust 1.99/native-CPU release configuration, thin LTO and one codegen unit. The
common received-frame client is built once from unchanged base production
sources. Reconstructed overlays and diagnostic sources match; original source
and binary identities remain in selected receipts. Eight-client smoke and
500-client preflight pass. Synthetic doubled-CPU, saturated-client and growing-
queue controls are rejected without changing original preflight files. The
known-periodic tail/IPC arithmetic control passes.

Capacity accounting, failed work and allowances total 5.50 CPU hours. Added
diagnostics, including the drift control and rejected body-class repeat, total
7,006.74 seconds (1.95 hours), below their separate two-hour ceiling. All task
hosts, clients, controllers and samplers have stopped; the single bounded Cargo
target is absent. Full logs, inventories and failed attempts remain private.

This research PR changes no production code. Documentation tests and exact
patch reconstruction are checked; the full Rust test group is not rerun.
No independent review or outside-server validation is claimed. Loopback sockets,
short mature windows, asynchronous trajectories, finite client policy, timing
instrumentation and the drifted base PMU window limit extrapolation to remote
networks, sustained uptime and exact optimization attribution. Main advanced to
a release-preparation commit during collection; measured runtime pins remain
the explicit pre-#248 and post-#255 commits, not an unmeasured moving ref.
