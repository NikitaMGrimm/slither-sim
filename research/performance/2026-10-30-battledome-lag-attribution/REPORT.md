# Battledome wall-clock lag attribution

**The historical 81-ms tail did not recur. Its cause remains unresolved.**
Six qualified 300-second windows at 320 constant-boost seats, in two separately
reported series on one c7a.medium core, contain **zero >20-ms start-lag episodes**
and zero deferred ticks. The comparable every-100th-tick completion-progress
metric has p99 **3.491-3.935 ms**, rather than about 81 ms. Start lag p99 is
0.135-0.149 ms; the largest observed start lag is 2.443 ms.

This is fresh negative evidence, not an explanation of the old event, a stutter
fix, or a universal 320-seat capacity claim. There is no valid denominator for
causal shares of unobserved large episodes: those shares are **not estimable**.
Routine runnable-wait pressure is measurable, and the largest small delay can be
accounted for. No production fix or priority/pacing change is implemented.

## Source, workload and observation

Runtime `dec68051c176501ab744a4c8afdf323b39636ec2` (main after #278), Rust 1.99,
default portable release, locked dependencies and no native CPU flags. The
[plan](PLAN.md) was committed before counting. A same-AZ Spot generator uses six
fully decoding received-frame A4 load workers, the unchanged #286 protocol-19
probe on its own CPU, controller/capture on separate CPUs, and spare CPUs for
reduction/transfer work. Total seats are 319 load bots plus one probe. All native
work and reductions ran on AWS, never locally or on the shared SSH guest.

Shipped Battledome arena/delivery, no modeled link latency, no internal bots,
seed 7, and natural 300/60/60-second staged growth at 80/160/320 precede each
series. Load bots override eligible boost actions as in #281/#286; every worker
reports 100% eligible boost actions. This is an action share, not a measured
alive-world boosting-time fraction. All sixteen calibration/count windows pass
whole-window generator coverage and CPU/input-lateness gates; the six count
windows' worst worker CPU p99 is 19.45%, worst input-p99 report
15.886 ms. No thin clients or authoritative bot observations.

The observer is a study-only archived-copy overlay. This checkout's `src/` is
unchanged. [prepare.py](prepare.py) retains its generated diff and one instrumented
source manifest in the native archive. [study_lag.rs](study_lag.rs) records tick
start/end, scheduled deadlines, thread CPU and both threads' scheduling counters,
plus pacing sleep entry/requested wake/return. Binary buffering avoids per-tick
JSON/log flushing. The native source/binary archive was closed, member-verified,
downloaded and hash-checked before counting; every completed row was similarly
secured. The same host binary is used in both series.

Guest schedstats were enabled. Ten-Hz proc telemetry records CPU, runnable wait,
IRQ/softirq and steal; CLK_TCK is 100 here, so jiffies are 10 ms and cannot locate
most individual tick events. Full scheduler+IRQ tracing failed the frozen overhead
gate: LIGHT 3.6563 versus TRACE 3.9256 host CPU ms/tick (+7.36%, +0.2693 ms).
The original three primaries therefore remain LIGHT. A prospective plan addendum,
before follow-up timing, freezes a fresh-session scheduler-only series. Its
LIGHT/SCHED/SCHED/LIGHT calibration passes: 3.6400 versus 3.7325 ms (+2.54%,
+0.0925 ms). The extra three diagnostics do not replace original primaries.

OFF still contains the mode check, enabled schedstats and external telemetry.
These are descriptive incremental overhead comparisons in a changing live world,
not exact fixed-work estimates against completely unobserved main. The target
microcheck's enabled records cost about 1.81 microseconds each with no attached
socket counter; do not use that as the full live observer cost. Source and build
pins occur once in [selected-results.json](selected-results.json); see
[reproduction](REPRODUCE.md) and [attempts](ATTEMPTS.md). Raw captures, binaries,
full counter/trace streams, private environment facts and retrieval receipts stay
local-only outside Git, with no public download URL. Hashes prove consistency,
not attribution correctness.

## Definitions and per-window results

Start lag is actual start minus scheduled `(tick + deferred)*7 ms` from the
host epoch. Completion progress includes that tick's work. The comparable #281
column samples every hundredth completion, subtracts virtual tick time and
normalizes by the window minimum. New observations use CLOCK_MONOTONIC; the old
census used Unix time. These metrics are not interchangeable, nor are they the
client's accumulated header-adjusted frame excess.

A frozen episode starts above 20 ms and recovers after five starts <=7 ms;
boundary fragments are censored. None occurs in 257,144 counted ticks. CPU
periods below align both threads between successive tick starts, while world CPU
covers command application, world/broadcast work and handover only. Off-CPU time
includes deliberate sleep; runnable wait is reported separately. No episode or
window was removed for being slow. Quantiles are nearest rank, milliseconds.

<!-- BEGIN GENERATED TABLES -->
| Window / mode | Ticks | Start lag p50 / p99 / max ms | Completion progress p99 / max ms | Comparable completion p99 / max ms | Episodes >20 ms / deferred |
| --- | --- | --- | --- | --- | --- |
| 07-primary / light | 42857 | 0.067 / 0.141 / 0.747 | 5.239 / 7.741 | 3.935 / 5.580 | 0 / 0 |
| 08-primary / light | 42857 | 0.067 / 0.135 / 0.650 | 5.228 / 7.213 | 3.491 / 4.479 | 0 / 0 |
| 09-primary / light | 42857 | 0.067 / 0.139 / 0.764 | 5.238 / 7.757 | 3.513 / 5.481 | 0 / 0 |
| 15-followup / sched | 42857 | 0.069 / 0.149 / 1.478 | 5.454 / 8.466 | 3.694 / 4.662 | 0 / 0 |
| 16-followup / sched | 42858 | 0.069 / 0.148 / 2.443 | 5.458 / 9.430 | 3.672 / 4.680 | 0 / 0 |
| 17-followup / sched | 42858 | 0.069 / 0.146 / 0.819 | 5.436 / 7.417 | 3.661 / 5.639 | 0 / 0 |

| Window | World CPU p50 / p99 / max ms | Aligned sim CPU p50 / p99 / max ms | Aligned socket CPU p50 / p99 / max ms | Aligned total CPU p50 / p99 / max ms |
| --- | --- | --- | --- | --- |
| 07-primary | 1.816 / 4.126 / 5.735 | 1.889 / 4.170 / 5.743 | 1.527 / 2.312 / 3.053 | 3.479 / 6.019 / 7.176 |
| 08-primary | 1.784 / 4.058 / 5.539 | 1.857 / 4.099 / 5.546 | 1.511 / 2.292 / 2.701 | 3.419 / 5.927 / 7.132 |
| 09-primary | 1.819 / 4.101 / 5.732 | 1.893 / 4.145 / 5.738 | 1.536 / 2.329 / 2.804 | 3.495 / 5.991 / 7.175 |
| 15-followup | 1.796 / 4.224 / 6.654 | 1.873 / 4.269 / 6.661 | 1.530 / 2.313 / 3.252 | 3.467 / 6.094 / 8.337 |
| 16-followup | 1.806 / 4.190 / 6.770 | 1.884 / 4.235 / 6.782 | 1.534 / 2.315 / 3.261 | 3.486 / 6.099 / 8.705 |
| 17-followup | 1.811 / 4.077 / 5.977 | 1.888 / 4.118 / 5.986 | 1.533 / 2.308 / 2.652 | 3.471 / 5.974 / 7.685 |

| Window | World / total periods >7-ms CPU | Sim runnable p99 / max ms | Socket runnable p99 / max ms | No-pacing periods / longest streak | IRQ / softIRQ / steal jiffies |
| --- | --- | --- | --- | --- | --- |
| 07-primary | 0 / 8 | 1.938 / 4.755 | 1.340 / 4.701 | 10 / 1 | 0 / 2676 / 0 |
| 08-primary | 0 / 4 | 1.932 / 3.322 | 1.331 / 3.220 | 3 / 1 | 0 / 2687 / 0 |
| 09-primary | 0 / 10 | 1.929 / 4.496 | 1.357 / 3.669 | 10 / 1 | 0 / 2724 / 0 |
| 15-followup | 0 / 19 | 2.005 / 4.661 | 1.506 / 3.554 | 21 / 1 | 0 / 2785 / 0 |
| 16-followup | 0 / 9 | 1.996 / 5.319 | 1.488 / 4.103 | 15 / 3 | 0 / 2670 / 0 |
| 17-followup | 0 / 5 | 2.001 / 3.672 | 1.483 / 3.101 | 8 / 1 | 0 / 2777 / 1 |

| Window | Probe residual jitter p99 / max ms | Excess p99 / max ms | Residual stalls >20 / >100 / >250 ms | Bunched <1 ms / gaps | Input p99 / max ms (echoes) |
| --- | --- | --- | --- | --- | --- |
| 07-primary | 2.619 / 5.985 | 3.066 / 6.899 | 0 / 0 / 0 | 0 / 37693 | 11.539 / 12.689 (662) |
| 08-primary | 2.566 / 5.309 | 3.000 / 5.620 | 0 / 0 / 0 | 0 / 34856 | 11.939 / 14.224 (647) |
| 09-primary | 2.530 / 5.474 | 3.000 / 6.006 | 0 / 0 / 0 | 0 / 36594 | 12.692 / 14.624 (657) |
| 15-followup | 2.720 / 6.243 | 3.221 / 9.055 | 0 / 0 / 0 | 0 / 35874 | 13.472 / 19.220 (651) |
| 16-followup | 2.685 / 6.752 | 3.157 / 8.907 | 0 / 0 / 0 | 0 / 36194 | 13.226 / 15.252 (650) |
| 17-followup | 2.529 / 4.279 | 2.997 / 5.907 | 0 / 0 / 0 | 0 / 35232 | 11.694 / 12.623 (653) |

| Window | Pacing overshoot p50 / p99 / max ms | Wake-ready delay p50 / p99 / max ms | Wake-dispatch p50 / p99 / max ms |
| --- | --- | --- | --- |
| 07-primary | 0.063 / 0.170 / 1.986 | Not observed | Not observed |
| 08-primary | 0.063 / 0.143 / 1.561 | Not observed | Not observed |
| 09-primary | 0.063 / 0.171 / 1.379 | Not observed | Not observed |
| 15-followup | 0.065 / 0.149 / 1.417 | 0.059 / 0.077 / 1.370 | 0.004 / 0.098 / 1.356 |
| 16-followup | 0.065 / 0.150 / 1.643 | 0.059 / 0.077 / 1.637 | 0.004 / 0.098 / 0.980 |
| 17-followup | 0.065 / 0.147 / 1.523 | 0.059 / 0.077 / 1.512 | 0.004 / 0.095 / 1.446 |

| Calibration series | Window | Host CPU/tick ms |
| --- | --- | --- |
| original | 01-off | 3.474934 |
| original | 02-light | 3.678819 |
| original | 03-trace | 3.946731 |
| original | 04-trace | 3.904507 |
| original | 05-light | 3.633865 |
| original | 06-off | 3.588056 |
| scheduler-only | 11-light | 3.610206 |
| scheduler-only | 12-sched | 3.667780 |
| scheduler-only | 13-sched | 3.797211 |
| scheduler-only | 14-light | 3.669835 |

| Traced window | Socket / observer / other-host / other-guest / idle / IRQ runnable-wait ms |
| --- | --- |
| 15-followup | 47716.463 / 828.626 / 0.000 / 706.800 / 674.326 / Not traced |
| 16-followup | 47562.787 / 822.885 / 0.000 / 714.711 / 675.684 / Not traced |
| 17-followup | 48017.467 / 799.696 / 0.000 / 701.776 / 683.665 / Not traced |

<!-- END GENERATED TABLES -->

The last table is **guest-scheduler overlap during simulation runnable wait**,
not the causal share of 81-ms lag or a claim that every socket-scheduled nanosecond
is socket CPU. Scheduler-only traces do not subtract IRQ time and cannot disprove
unreported hypervisor preemption. Proc CPU bounds remain separate. Other guest
work can include host-main bookkeeping; process-name classification is not a
kernel stack profile. Trace/proc runnable-wait totals differ about 1.3% in the
scheduler-only count windows, within the declared 5%/1-ms audit guard; no fitted
correction is applied. Exact exhaustive nanosecond attribution is not claimed.

## What the measured delays are doing

| Requested cause | Large-episode share | Fresh evidence and limit |
| --- | --- | --- |
| (a) World work alone exceeds 7 ms | Not estimable: no episodes | Zero such world ticks; world CPU max 6.770 ms. Across both series, 55 aligned sim+socket periods exceed 7 ms. This supports occasional shared-core bursts, not sustained world overload. |
| (b) Socket holds the core while simulation is runnable | Not estimable: no episodes | Socket-scheduled intervals account for 95.59% of recorded simulation runnable-wait overlap in the three SCHED windows. This is routine pressure within met deadlines, not proof of an 80-ms delay. |
| (c) Pacing/idle wakeup | Not estimable: no episodes | Wake-ready p99 is tens of microseconds; dispatch p99 about 0.1 ms. Intermediate bounded sleeps may overshoot without missing the final absolute tick deadline. No 80-ms wakeup is observed. |
| (d) Kernel/IRQ/steal/other guest preemption | Not estimable: no episodes | Steal is zero in five windows and one jiffy (10 ms) in `17-followup`; hard-IRQ jiffies are zero in all six and softirq jiffies are nonzero. The coarse sample cannot locate that steal within a tick. SCHED cannot split exact IRQ from its interrupted task, and measured steal does not bound all possible hypervisor interference. |
| Unattributed historical lag | Entire old event remains unresolved | Old recordings lack these scheduler observations. A new quiet host/session does not retroactively identify the old cause. |

The largest fresh start delay, 2.443 ms in `16-followup` at +236.729 seconds,
follows 6.770 ms world CPU plus about 1.924 ms socket CPU in the preceding
aligned period. Guest trace overlap adds about 0.623 ms of observation/controller
work and 0.055 ms other work. The simulation was runnable for 2.592 ms; there was
no pacing sleep in that interval. This is a small shared-core burst with
measurement work contributing, not evidence of a broken timer. The next delayed
start is only 1.497 ms late and the loop catches up.

Several other largest small starts have roughly 0.5-ms observation/controller
preemption. Do not mistake the measurement's Python telemetry/status reader for
production socket work. Conversely, socket overlap dominates the whole-window
runnable-wait aggregate. That distinction is why a single exclusive cause
percentage for all waiting time would be misleading.

The 504-ms phase is enriched among slow world ticks, but only 5.6-11.2% of each
window's slowest 1% lies at tick modulo 72 = 0. It does **not** justify declaring
food maintenance the dominant cause of these world tails. No per-function phase
profile was collected in this low-overhead study.

## Debt repayment and client effects

The [pinned loop](https://github.com/NikitaMGrimm/slither-sim/blob/dec68051c176501ab744a4c8afdf323b39636ec2/src/client/server.rs#L633)
runs due ticks without deliberate pacing between them, capped at eight ticks per
outer pass. Only backlog beyond `ceil(0.25/0.007)` ticks (36, about 252 ms) is
deferred, shifting virtual time behind wall time. Ordinary small debt instead
reduces subsequent idle time; it does not automatically create a slower clock.

Fresh records contain 67 adjacent no-pacing pairs, longest streak
3; no deferral occurs. The next tick usually starts a few microseconds
after the preceding completion, although scheduling can interrupt that path.
The table below reports **exploratory** frame adjacency with a seven-ms margin
around those paired completions, not the frozen large-episode contrast. There
are only 113 adjacent received gaps: do not infer population tail rates
from them. Shortened completion spacing is visible, but no <1-ms client bunching
or >20-ms residual stall appears there. Overall the independent probe also has
zero >20-ms residual stalls in all six windows. Sparse raw gaps are not stalls.

| Window | No-pacing pairs | End-to-next-start p50 / max ms | Completion spacing p50 / max ms | Adjacent received gaps / minimum ms | Adjacent residual max ms / stalls >20 / bunch <1 |
| --- | --- | --- | --- | --- | --- |
| 07-primary | 10 | 0.007 / 0.021 | 3.025 / 4.025 | 21 / 2.950 | 5.985 / 0 / 0 |
| 08-primary | 3 | 0.007 / 0.007 | 2.019 / 3.140 | 4 / 2.915 | 4.151 / 0 / 0 |
| 09-primary | 10 | 0.007 / 0.034 | 3.024 / 4.010 | 19 / 2.305 | 5.474 / 0 / 0 |
| 15-followup | 21 | 0.009 / 1.441 | 3.122 / 6.347 | 35 / 2.448 | 6.243 / 0 / 0 |
| 16-followup | 15 | 0.008 / 0.020 | 3.812 / 6.055 | 20 / 2.003 | 6.752 / 0 / 0 |
| 17-followup | 8 | 0.007 / 0.010 | 3.031 / 3.341 | 14 / 2.162 | 4.235 / 0 / 0 |


The load-worker cohorts still report 380 residual gaps >20 ms across
these windows despite passing their CPU/input gates. They share decoding passes
among many seats; the isolated probe observes no corresponding >20-ms residual
stall. This does not assign every worker gap to the generator, but it prevents
using cohort read gaps as proof of host clock debt. All client timings are
received-frame timings, not browser rendering or Internet-route guarantees.

Packet diagnostics are limited by connection captures with drops/unmatched tails.
Monotonic probe timing includes all successful echoes, while kernel/read-age
checks use only the verified subset below; that subset may be biased. Cross-host
phase alignment relies on recorded anchors and spot NTP checks, not continuous
clock-error bounds. There are no large-episode samples to align.

| Window | Verified playing frames / recorded | Verified echoes / confirmed |
| --- | --- | --- |
| 07-primary | 35850 / 37693 | 625 / 662 |
| 08-primary | 26967 / 34856 | 495 / 647 |
| 09-primary | 25447 / 36594 | 472 / 657 |
| 15-followup | 25620 / 35874 | 467 / 651 |
| 16-followup | 33029 / 36194 | 584 / 650 |
| 17-followup | 23909 / 35232 | 443 / 653 |


## Ranked proposals, not implemented fixes

No production change is justified as a verified fix for the old 81-ms event.
The ranking below follows the fresh measured pressure and the information needed
to address the unresolved problem. Exact candidates still need their own parity
proofs; none has been implemented or timed here.

| Rank | Proposal | Exact or design | Evidence and decision boundary |
| --- | --- | --- | --- |
| 1 | Profile and remove avoidable socket poll/flush/encoding work without changing output, order or scheduled ticks | Exact candidate, unproved | Socket-scheduled overlap is 95.59% of routine runnable wait, and socket CPU helps push the largest fresh period over budget. This is a plausible source of extra headroom, not a measured remedy for 81 ms. |
| 2 | Reduce the largest world-work bursts while preserving all rules/bytes | Exact candidate, unproved | World max 6.770 ms leaves little room for simultaneous egress in the worst small burst. Function-level profiling is needed; modulo-72 enrichment does not establish a food-only target. |
| 3 | Capture a recurring large episode with a lower-cost triggered scheduler recorder and less target-side telemetry | Exact measurement-tool proposal, not a production fix | No large episode recurred, full IRQ tracing fails its overhead gate, and observation work explains some sub-ms outliers. This is the most useful next evidence step before claiming the old cause. |
| 4 | Thread priority, deadline-aware egress, different catch-up/defer policy or altered pacing | Design change | Not supported as necessary by these windows. These can change fairness, ordering or wall-clock feel; the maintaining agent decides which to try on the owner's local play server before a production PR. |

## Validation, retention and limits

Seven focused Python tests pass remotely: interval boundaries, episode recovery
and censoring, socket/IRQ overlap, wake versus dispatch, trace loss, truncated
records, sparse/bunched frames and no-pacing adjacency. Native observer and
microcheck compile; smoke confirms all record kinds and nonzero CPU/wait counters
for both threads. The unchanged client/probe binaries match their inherited
source/proof pins. No full Rust group is rerun for a research-only checkout
change; ordinary PR CI remains separate. No independent review is claimed.

Post-publication CodeQL found that controller RPC path guards used assertions,
which disappear under `python -O`. The future helper now rejects invalid labels
and modes explicitly, before mutation. Two short offline boundary tests pass
normally and under `-O` locally (under one second per invocation); no native or
scientific reduction runs locally. The as-measured helper archives/pins and all
results are preserved, rather than pretending this guard correction was timed.
See the post-collection note in [reproduction](REPRODUCE.md).

The original primaries, failed all-IRQ overhead gate and separately frozen
scheduler follow-up remain distinct. These are two live sessions on one target,
not independent hosts, controlled ecology pairs or a capacity limit. No spike
was induced by changing player counts, priority or tick rules. The historical
81-ms cause remains open: host/environment variability, different ecology and
historical observation overhead are hypotheses, not established explanations.

Archive readiness is written only after closure/member verification; download
size/hash checks precede deletion/termination. See [verification](verification.json),
[cost](cost.json), and [cleanup](cleanup.json). Cleanup completed at 06:36 UTC after verified source retrieval. All 17 enabled
regions were scanned: zero nonterminated instances, all volumes, active/open
Spot requests and Elastic IPs. Both task instances are terminated, their key pair
and security group deleted, and the local private key removed. The launch-to-
cleanup upper bound is **$1.63**, including $0.65 noncompute reserve; this is not
an invoice. Cleanup receipts were appended after closing the analysis archive. The conservative
full-timer allocation bound is $1.882, including the $0.65 noncompute reserve,
below the combined $3 cap. One Spot pair is used for builds, both series and
reductions, with 130-minute self-terminate deadlines; no timer is extended and
there is no owner play hold. Protected/unrelated resources are preserved.
