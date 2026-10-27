# Small-server costs and latency, 2026-10-27

**Active small fixtures use less total work, but the optimizations are not free
at every boundary.** v0.7.0 `4c2471b` → `aee5421` reduces whole-fixture user
instructions at all seven tested sizes. The empty gateway and cold official
tick increase. Shared-machine socket timing cannot establish that latency
always stays equal or improves. This is a research-only record; no runtime
change or capacity claim. [Selected results](selected-results.json),
[plan and pilot corrections](PLAN.md), [reproducer](reproduce.py).

## Fixed work

Synthetic official/Battledome worlds, seed 7, 1,080 ticks, controls every six
ticks, dead-slot retries every 72, disconnect/rejoin at 720. Reuses #204's
fixture and its default responsive delivery policy, with host seat sizing.
These are world presets, not outside-server measurements. The exact revision
range also includes #199's spawn-head rejection; expensive admission is part
of whole-process work, especially in Battledome-32.

One warmup pair then five alternating whole-process `instructions:u` pairs
per case, including construction, admission and drains; all counters 100%
scheduled, no excluded pairs. Millions of instructions:

| Configured seats | Base → main median | Paired change (range), % | Alive min / mean / final |
| --- | ---: | ---: | --- |
| Official 1 | 30.848 → 28.919 | -6.255 (-6.276 to -6.226) | 1 / 1.00 / 1 |
| Official 10 | 280.887 → 207.450 | -26.138 (-26.160 to -26.082) | 9 / 9.99 / 10 |
| Official 50 | 1550.848 → 1065.122 | -31.319 (-31.325 to -31.303) | 49 / 49.94 / 50 |
| Official 100 | 3278.298 → 2145.965 | -34.526 (-34.576 to -34.516) | 99 / 99.99 / 100 |
| Battledome 8 | 258.896 → 209.654 | -19.021 (-19.166 to -18.908) | 8 / 8.00 / 8 |
| Battledome 16 | 1107.842 → 711.027 | -35.824 (-35.850 to -35.779) | 14 / 15.81 / 16 |
| Battledome 32 | 10000.438 → 5020.749 | -49.795 (-49.795 to -49.794) | 18 / 23.74 / 29 |

The last row is a **32-seat admission-limited fixture**, not 32 continuously
living snakes. Preserve the 15 initial refusals and 123 later refused attempts;
do not widen its arena to improve the result. Initial refusals at 16 seats: two.
The adapted summary's `forcedReconnect` flag records the scripted disconnect;
admission can defer the rejoin to a later retry.
All seven full decompressed traces match byte for byte, including official-10
and Battledome-16: payloads/order, modeled timestamps, initial/final
checkpoints, population/refusal summaries and spawn/latency RNG endpoints.

Separate simulation-thread brackets around `advance_tick_with`, one run per
arm, all 1,080 ticks, exact nearest ranks. Millions of user instructions/tick;
bracket overhead is included. Admission retries and client drains are outside
these brackets. No alive-only selection:

| Seats | p50 base → main | p99 base → main | Max base → main |
| --- | ---: | ---: | ---: |
| Official 1 | 0.0241 → 0.0224 | 0.0991 → 0.0983 | 0.4784 → 0.4757 |
| Official 10 | 0.2230 → 0.1580 | 0.4400 → 0.3592 | 2.7645 → 3.0672 |
| Official 50 | 1.2938 → 0.8511 | 2.1456 → 1.5316 | 14.7100 → 16.0868 |
| Official 100 | 2.8099 → 1.6930 | 4.4493 → 3.3512 | 30.2170 → 32.9212 |
| Battledome 8 | 0.1985 → 0.1528 | 0.3432 → 0.2940 | 8.6162 → 8.5688 |
| Battledome 16 | 0.3646 → 0.3144 | 0.5338 → 0.4812 | 8.9155 → 8.8642 |
| Battledome 32 | 0.5936 → 0.5372 | 0.8845 → 0.8215 | 9.3837 → 9.3177 |

Post-warmup ticks >=120 also retain reconnect/map bursts; their maxima improve
at every size. Full distributions and first-tick counts are in JSON.

## Fixed overhead and memory

The official cold tick is tick zero: main adds 43,191 / 302,689 / 1,376,784 /
2,704,154 instructions at 1/10/50/100 seats, respectively (+17.76%, +10.95%,
+9.36%, +8.95%). This can matter to first-tick latency despite lower steady
cost. A research-only control that builds #209's lazy pellet index during setup
reduces first-tick counts to 242,409 / 2,702,797 / 14,370,886 / 29,513,762,
below v0.7.0 at every official size. Its full official-10 trace is identical.
This isolates the cold index build as the added first-tick work; it relocates
work and is not a proposed fix. Construction, restore or invalidating edits can
require rebuilding; restore/edit latency was not separately measured here.

At 0/1/2/5/10 idle loopback sockets, 4,000 real gateway passes, the same
warmup/five-pair protocol gives main/base user-instruction changes of
+5.323% / -25.276% / -36.111% / -47.385% / -52.940%. At zero sockets this is
about **21 extra user instructions/pass**, amortized across this run including
setup/teardown. Omitting #205's readiness snapshot in a main-source control
removes about 13 instructions/pass (3.111% of main's zero-socket total).
Also omitting #208's indexed-retain bookkeeping removes about seven more;
roughly one instruction/pass remains unassigned, including compilation/setup
effects. These zero-socket-only controls are not functional host fixes.
No `poll` syscall occurs for an
empty readiness array; with sockets one nonblocking readiness query replaces
their empty reads. User counts exclude kernel instructions, so they cannot
price `poll` versus `recv` in the kernel. Retained `wait4` kernel CPU seconds
are descriptive WSL2 samples, not a kernel-speed conclusion.

The #209 pellet index is built even for one eligible eater, then maintained
through births/removals. On this x86-64 build `PelletSlot` is 16 bytes, with a
4-byte food-to-slot entry and 4-byte cell-list handle per live pellet: **24
additional logical bytes/pellet**, plus a 32-byte map entry per occupied cell,
capacity slack and the free list. Physical `Food` remains 48 bytes in both arms.
Measured final index vector capacities plus occupied map-entry payloads give
lower bounds of 44–59 additional bytes/live pellet in these fixtures; this
excludes spare hash buckets, metadata, inline state and allocator overhead.
It is not an RSS estimate. For example official-10 retains at least 70,288
extra payload bytes for 1,190 pellets. Final food counts/capacities match.

#219 adds an empty per-broadcast cache on stock/default [99,4] paths; only
opt-in [99,3] recipients populate it. Empty clear work is still executed.
#215 retains a per-frame dispatch on stock output; its
[own study](../2026-10-27-host-owned-legacy-frames/REPORT.md) measured +0.105%
stock work at 500. Those costs are included in this net small-host comparison,
not independently isolated here. The [full-map study](../2026-10-27-host-full-map-sharing/REPORT.md)
also retains small default-path increases. Neither optimization is assumed
free just because the full stack wins.

## Loopback timing

Three alternating pairs per case, fresh seed-7 worlds, zero bots/added link
delay/TLS, 10 s warmup +30 s hold. Same existing drain-only stock/legacy load,
100 ms controls and staggered boosts in both arms. Medians of the three
hold-level quantiles, in milliseconds (not pooled independent trials):

| Clients / arm | Tick busy p50 / p99 | Frame excess p50 / p99 | Enqueue → ingest p50 / p99 |
| --- | ---: | ---: | ---: |
| Stock 10 / base | 0.040 / 0.222 | 0.669 / 6.373 | 4.257 / 12.755 |
| Stock 10 / main | 0.033 / 0.135 | 0.684 / 5.935 | 4.145 / 7.983 |
| Stock 50 / base | 0.151 / 0.704 | 0.848 / 8.530 | 4.302 / 7.992 |
| Stock 50 / main | 0.126 / 0.605 | 0.798 / 6.628 | 4.126 / 8.357 |
| Protocol 13, 10 / base | 0.049 / 0.243 | 0.746 / 5.777 | 4.163 / 7.730 |
| Protocol 13, 10 / main | 0.041 / 0.156 | 0.723 / 5.761 | 4.045 / 7.663 |

Tick busy includes command intake, simulation and handover, including
preemption. Frame excess uses the EPYC recheck definition: receipt minus
cumulative elapsed-header time, minus each connection's minimum within the
hold. Inputs reuse #217's buffered FIFO matching: enqueue is sampled after the
client write accepts the unchanged steering byte, ingest is before physics.
Only client 0 is sampled here. Same-machine UTC samples do not cross hosts.

Tick-busy p50/p99 improve in all nine pairs, but **latency is not monotonically
better**. Retain these descriptive counterexamples: stock-50 pair 0 input p99
7.746 → 8.866 ms; protocol-13 pair 0 frame-excess p99 5.777 → 6.352 ms;
stock-10 pair 2 frame-excess p50 0.666 → 0.727 ms. Stock-50 pair 0 maximum tick
busy also rises 3.007 → 21.987 ms. Shared WSL2 scheduling, input phase and
evolving worlds prevent attributing these differences to a particular commit.
The unchanged 7 ms tick/input cadence remains even as simulation work falls.

All 18 holds have zero generator errors and zero new deferred ticks. Occupancy,
frame counts, generator busy percentages, every pair and all input sample
match/loss counts remain in JSON. No hold was replaced for a timing result;
these results establish neither latency equivalence nor capacity.

## Reproduce and limits

Linux x86-64/WSL2, Rust 1.99.0, locked release/debug=1, thin LTO, one codegen
unit; Cargo/Rayon/tests one job. At most two taskset-pinned logical CPUs;
counted fixtures/builds use one, socket host and generator use separate ones.
Matched research overlays are compiled into both arms; whole-fixture counts
include fixture work and disabled-probe branches.
The PMU wrapper enables own-child inherited user instructions on exec;
`perf_event_paranoid=2` remains unchanged. No sysctl/security changes or outside
servers. Shared-machine CPU/wall timing is descriptive throughout.

Run `python3 reproduce.py MODE REPOSITORY OUTPUT --cpu CPU --load-cpu CPU`
in order: `build`, `trace`, `count`, `ticks`, `idle`, `memory`, `latency`,
`controls`, `empty-control`, `select`. Choose output outside the checkout. Build accepts
`--archives DIR` containing commit-verified `base.tar`/`candidate.tar` from
`git archive`; this avoids Windows-worktree Git path incompatibility in WSL.
Optional `--target-root DIR` reuses only task-owned Cargo caches; executable
copies remain separately pinned. Controls are diagnostic source snapshots,
never runtime edits in this PR. Build requires a C compiler and `patch`.
Python checks use explicit exceptions; the historical overlay's assertions
are converted before execution. Optimized-Python reducer checks are supported.

Raw sources/binaries, complete traces, counter/tick arrays, loopback receipts,
input samples and failed pilots remain local-only outside Git. Selected hashes
identify artifacts; they do not establish scientific laws or native capacity.
This short seed-7 workload does not cover arbitrary long snakes, long uptime,
TLS, Internet delivery, browser rendering or continuously full Battledome-32.

Validation: release harnesses build and all declared measurements/trace gates
pass; three reducer regressions pass with normal Python and `-O`.
Documentation/navigation checks pass. Full Rust/check-group CI was not run
locally for this research-only change; production files are unchanged.
