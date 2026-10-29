# Mature 500-socket capacity checkpoint

Research only. Compare #248 runtime d127acd1f0e9db2100c8a1dd5c2fa43e2deff506
with merged #255 main 551cb1030ea2bf84f83bbeb167233b917900929a, in one guest
session, using identical archived-copy instrumentation and one common client.
No production code change, no checkpoint restoration, no outside game servers.
Do not push the accepted optimization branch.

Use exactly 500 remote seats, zero in-process bots, official world/delivery,
seed 7, default 7 ms ticks and zero artificial latency. Real loopback WebSockets
request the normal client-equivalent physical-food/rules extension. Three
received-frame A4 workers handle sockets on vCPUs 1/2/3; simulation and socket
threads share vCPU 0. CLI/collector work uses vCPU 3. Verify every task's affinity
before holds and throughout them. Clients run the standard decision/ping cadence,
decode every delivered frame, and reconnect normally after death.

Warm the full real host and all clients from tick zero. Keep 100 logical clients
resident; retire/rejoin the other 400 in a round-robin schedule, two per 100-tick
census (equivalent to one per 50 ticks, batched at observation boundaries).
Stop forced churn at 180,000 ticks. No authoritative body, food, RNG or policy
edits. After at least 193,000 ticks, require three successive 500-tick census
endpoints with >=490 living, mean length 45–65, p90 156–196, p99 >=200; report
the actual lengths, deviations from mean~55/p90~176 and all drift. The loose
screen is a mature-class screen, not ecological stationarity. Bound each warmup
at three elapsed hours; retain and report failure rather than accept short bodies.
Discard 60 seconds after screening, then collect three consecutive 300-second
holds per commit, base first then current main. These are repeated windows from
one evolving seed per arm, not deterministic paired trajectories.

Prefer full client-equivalent A4. Before any accepted holds, require client
CPU below saturation and bounded input/receive lag. If A4 is the load limit,
retain that attempt and run the same thin mode on both arms: keep complete wire
decoding, frame materialization, 42 ms inputs and ping/reconnect behavior, replace
only policy with a nonboosting gradual-turn input. Discard settling time again.
Do not report a host capacity number when the clients fail their screen.

Keep client worker CPU samples, decision lateness/pass time, frame/byte counts,
receipt ages, connected/playing counts and error/rejoin counters. Retain host
application queues, kernel send queues, actual transport WouldBlock counts,
write-buffer-full counts and rejects. Accepted windows require zero unexpected
transport/decode errors, no send backpressure, no growing send queues, stable
500-client population apart from documented natural reconnects, and client CPU
headroom. Report the actual thresholds/observations and every failed attempt.

Read CPU clocks directly for simulation and socket threads at consecutive tick
completion boundaries. Their sum includes both user and kernel CPU, including
idle-poll CPU between executed ticks; never use wall tick spacing as work time.
Also retain tick busy wall time, mean CPU reconciled with /proc task counters,
per-thread user/system splits, percentiles over complete 100-tick blocks and
throughput/deferrals. Simulation clocks include reporting overhead allocated to
the next tick; bracketed phase clocks remain nested, not additive parents.
Primary windows disable per-read/write syscall clocks. If either runtime misses
7 ms, collect a separate continued-world syscall diagnostic and label its
overhead; rank remaining physics/broadcast and socket/kernel costs in ms.

Report median and p99 world CPU ms/tick and simulation/broadcast versus socket
components for both commits. State the 7 ms margin plainly, distinguishing median
from p99. Cite #248's published 500-bots-plus-one-socket 4.86–5.44 ms/tick as
historical context, not a same-work measurement or a 500-socket qualification.

Use the authorized SSH alias and existing tools. One bounded Cargo target, removed
after copying/pinning binaries. No installs or system/security/limit changes.
Retain full process commands at boundaries with the configured account redacted
before writing. Never publish host/provider/kernel/account/path/IP identifiers
or gateway messages; describe only a 4-vCPU Zen 5 EPYC KVM guest. Link #248's
scripts, retain only changed harness/instrumentation and bounded selected values,
run focused harness/arithmetic checks and docs tests, then open a research PR and
do not merge it. No performance-optimization claim or production changelog entry.

Before the first mature counted window, extend the read-only vCPU context sampler
with process CPU snapshots. Use #248's largest-other-process screen of 3 CPU
seconds per 300-second hold, excluding this study's tasks and kernel threads.
Retain separate IRQ/softirq/steal context and the limitations of snapshot-based
process accounting. No runtime or client behavior changes with this addition.

Amendment after the retained long-hold drift control, before any accepted mature
comparison: its first 300-second window moved from mean about 58 to 84, so the
original three-long-hold protocol does not stay in the requested length class.
Replace it with three independent fresh-host cohorts per runtime, alternating
base/current/base/current/base/current. Each uses the same tick-zero warmup and
unchanged mature screen, then five seconds of settling, one fixed 30-second
window and a separate 15-second syscall diagnostic. Every sampled 100-tick census
inside the window must satisfy the same mature class. This gives thousands of
ticks per window while limiting length drift; it does not qualify sustained
long-duration capacity or ecological stationarity.

Allow at most two fresh attempts for a cohort when it loses its length class;
retain failures and never retry based on timing results. Other validity failures
stop collection for inspection. Bound each new warmup at 45 elapsed minutes and
all collection at six guest CPU hours, including the drift control plus a
conservative 600 CPU seconds for builds/pilots/checks. If the budget prevents
completion, retain the partial result and report the limit. No restored or
forked multithreaded network state is used. Scale the background-process quiet
threshold from 3 CPU seconds/300 seconds to the actual short window duration.

Diagnostic addition requested after the first accepted pair: after all planned
capacity cohorts end, build separate archived-copy diagnostic hosts. Reuse the
same tick-zero warmup and common 500-socket client, never restoring a checkpoint.
For each runtime, collect a separate 12-second simulation-thread group of
`instructions:u` and `cycles:u`, its IPC, task user/kernel CPU and the existing
phase CPU split. Check the event scheduling fraction and normalize to the
observed ticks at window edges. Keep this separate from capacity results.

In a subsequent 20-second coarse window, enable per-tick phase/event details,
keeping per-recipient subphase clocks disabled. Follow with a separate ten-second
fine window for broadcast subphases; its added CPU-clock overhead is explicit.
On current
main rank the slowest 1% and 2% of aligned simulation-plus-socket CPU ticks, with
conditional phase means, periodic HUD/food flags, anchor births, deaths and
rejoins. Report the actual body class and diagnostic overhead. Match high-rate
vCPU IRQ/softirq/steal observations to tick intervals, retaining the counter
resolution limit rather than claiming exact interrupt attribution. These
diagnostics explain measured work mix and tails; they are not another capacity
test or a same-trajectory causal speedup estimate. The two newly requested
diagnostics require two further fresh warmups and builds. Give that added work
a separate two-CPU-hour ceiling, accounted explicitly; the original capacity
collection retains its six-CPU-hour ceiling.

Interference amendment after the second base attempt: its unchanged quiet
screen failed (0.320 other-process CPU seconds versus 0.294049 permitted).
Retain that excluded attempt and permit one fresh tick-zero repeat of this
cohort for interference. Do not relax the gate or select on timing; another
non-maturity failure stops for inspection. Charge the completed attempt,
termination tails and prior controller allowance before resuming.

Diagnostic drift amendment, before the current-main explanatory windows: the
first base diagnostic qualified late and exceeded the primary length class
during its PMU/coarse windows. Retain it as a drift control. After current-main
diagnostics, permit one fresh base diagnostic for body-class validity if the
remaining two-CPU-hour diagnostic allowance permits. Keep all prior diagnostic
CPU charged and the same warmup/screen; never repeat for a timing outcome.

The completed-block drift-control summary, calculated after the amendment,
refines the approximate observation above: mean 59.064 to 86.478 across 42,800
ticks, with p90 157 to 170. The screen itself was mean 51.1523, p90 175, p99 247
at tick 193,000. These values do not change the amended criteria.
