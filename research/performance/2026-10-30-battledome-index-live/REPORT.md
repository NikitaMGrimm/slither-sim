# Live constant-boost Battledome after the obstacle index

**The index removes spawn-dominated CPU tails, but this study does not establish
consistent zero-deferral play at 320.** The 320 AB/BA rows give base CPU p99
12.84/12.93 ms versus candidate 6.96/7.14 ms. Deferrals vary with run: base
0/73, candidate 27/0. Both arms hold one of their two windows. The first base
already avoids #275's fall-behind, so a historical baseline failure is not
uniformly reproduced here.

At 480, the contemporary comparison improves from 4,246 to 67 deferrals,
CPU p99 from 20.23 to 10.02 ms, jitter p99 from 25.90 to 7.52 ms and completed
respawn p95 from 3.58 to 1.71 s. Candidate mean CPU is 6.81 ms per tick, near
the 7 ms cadence, and it fails the frozen hold rule. **640 was not measured.**
This locates a tested pressure point at 480, not a sustainable seat limit.

Sockets+broadcast now lead candidate slow-tail CPU: 56.7–57.8% at 320 and 59.4%
at 480, against spawn's 28.5–29.2% and 24.1%. The ranked next CPU leads are
socket-thread work and recipient broadcast work; neither is implemented here.

[Frozen plan](PLAN.md), [attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md),
[runtime pins](runtimes.json), [selected results](selected-results.json),
[verification](verification.json). Full raw data, native sources/binaries, manifests
and controllers remain local-only verified archives outside Git.

## Observed rows

| Seats | Arm (order) | CPU p50 / p99 / max ms | Deferred | Gap jitter p99 ms | Excess p99 / max ms | Stalls >20 / >100 / >250 ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 320 | base (1) | 4.081 / 12.842 / 15.824 | 0 | 6.879 | 80.618 / 160.693 | 548 / 0 / 0 |
| 320 | candidate (2) | 4.176 / 6.961 / 8.786 | 27 | 3.399 | 76.751 / 199.884 | 296 / 0 / 0 |
| 320 | candidate (5) | 4.108 / 7.138 / 8.654 | 0 | 3.489 | 4.499 / 19.959 | 0 / 0 / 0 |
| 320 | base (6) | 4.054 / 12.933 / 14.729 | 73 | 6.427 | 669.177 / 772.911 | 1121 / 0 / 0 |
| 480 | candidate (3) | 6.634 / 10.017 / 12.994 | 67 | 7.519 | 589.036 / 724.892 | 19937 / 0 / 0 |
| 480 | base (4) | 6.986 / 20.235 / 22.866 | 4246 | 25.896 | 25116.870 / 29839.837 | 67835 / 0 / 0 |
| 640 | candidate only | Not measured | — | — | — | — |

| Seats | Arm (order) | Respawn p50 / p95 / max ms (n) | Pending last / max age ms | Alive mean / min | Boost % / births | Length first → last | Food first → last |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 320 | base (1) | 11.366 / 58.042 / 630.267 (1057) | 0 / 78.595 | 319.21 / 316 | 99.87 / 72153 | 54.93 → 59.36 | 17266 → 17840 |
| 320 | candidate (2) | 10.619 / 149.850 / 1230.766 (1032) | 0 / 990.973 | 318.79 / 307 | 99.88 / 73833 | 52.23 → 58.51 | 19086 → 17786 |
| 320 | candidate (5) | 10.714 / 133.795 / 524.252 (1069) | 1 / 353.123 | 319.24 / 313 | 99.88 / 72532 | 53.37 → 52.91 | 17465 → 19373 |
| 320 | base (6) | 11.243 / 402.917 / 1054.767 (998) | 0 / 911.374 | 318.95 / 309 | 99.82 / 78018 | 52.06 → 57.11 | 18591 → 18105 |
| 480 | candidate (3) | 709.302 / 1713.702 / 2081.405 (1413) | 11 / 1992.906 | 467.77 / 446 | 99.40 / 95263 | 55.79 → 53.72 | 23913 → 27949 |
| 480 | base (4) | 2259.597 / 3583.720 / 3689.055 (979) | 28 / 3619.162 | 459.55 / 426 | 99.38 / 63237 | 52.57 → 52.00 | 27686 → 30454 |

| Seats | Arm (order) | Tail n / mean CPU ms | Spawn ms / % | Sockets+broadcast ms / % | Minimap ms / % | Other ms / % | Search / exhausted ticks |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 320 | base (1) | 127 / 13.363 | 8.010 / 59.94 | 4.298 / 32.16 | 0.326 / 2.44 | 0.729 / 5.46 | 127 / 116 |
| 320 | candidate (2) | 128 / 7.357 | 2.145 / 29.16 | 4.171 / 56.69 | 0.313 / 4.26 | 0.727 / 9.89 | 122 / 111 |
| 320 | candidate (5) | 128 / 7.576 | 2.158 / 28.49 | 4.381 / 57.83 | 0.320 / 4.22 | 0.717 / 9.46 | 127 / 114 |
| 320 | base (6) | 127 / 13.301 | 8.029 / 60.36 | 4.255 / 31.99 | 0.317 / 2.38 | 0.700 / 5.26 | 127 / 123 |
| 480 | candidate (3) | 127 / 10.677 | 2.578 / 24.14 | 6.339 / 59.37 | 0.483 / 4.52 | 1.278 / 11.97 | 126 / 111 |
| 480 | base (4) | 85 / 20.956 | 12.366 / 59.01 | 6.984 / 33.33 | 0.484 / 2.31 | 1.122 / 5.35 | 85 / 65 |

| Order | Arm / seats | Canonical ticks / seconds | Full-run deferred | Calibration deferred | Outside counted interiors |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | base / 320 | 12700 / 88.900 | 0 | 0 | 0 |
| 2 | candidate / 320 | 12800 / 89.526 | 76 | 0 | 49 |
| 3 | candidate / 480 | 12700 / 89.619 | 75 | 0 | 8 |
| 4 | base / 480 | 8500 / 89.234 | 6076 | 0 | 1830 |
| 5 | candidate / 320 | 12800 / 89.604 | 0 | 0 | 0 |
| 6 | base / 320 | 12700 / 89.386 | 73 | 0 | 0 |

| Order | Arm / seats | Original online deferred | Canonical deferred | Enclosing census deferred | Holds |
| ---: | --- | ---: | ---: | ---: | --- |
| 1 | base / 320 | 0 | 0 | 0 | True |
| 2 | candidate / 320 | 27 | 27 | 66 | False |
| 3 | candidate / 480 | 67 | 67 | 72 | False |
| 4 | base / 480 | 4246 | 4246 | 4361 | False |
| 5 | candidate / 320 | 0 | 0 | 0 | True |
| 6 | base / 320 | 73 | 73 | 73 | False |

| Order | Arm / seats | Slow-tail spawn % | Sockets+broadcast % | Other including minimap % | Socket CPU ms | Broadcast excluding minimap ms |
| ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | base / 320 | 59.94 | 32.16 | 7.90 | 2.782 | 1.515 |
| 2 | candidate / 320 | 29.16 | 56.69 | 14.15 | 2.666 | 1.505 |
| 3 | candidate / 480 | 24.14 | 59.37 | 16.49 | 3.622 | 2.717 |
| 4 | base / 480 | 59.01 | 33.33 | 7.66 | 4.481 | 2.503 |
| 5 | candidate / 320 | 28.49 | 57.83 | 13.68 | 2.856 | 1.525 |
| 6 | base / 320 | 60.36 | 31.99 | 7.64 | 2.766 | 1.489 |

| Order | Arm / seats | Busy wall p99 / max ms | Simulation CPU at largest wall tick ms | Census spans with deferrals | Largest span deferred |
| ---: | --- | ---: | ---: | ---: | ---: |
| 1 | base / 320 | 12.856 / 15.999 | 11.974 | 0 | 0 |
| 2 | candidate / 320 | 5.964 / 17.483 | 5.819 | 1 | 27 |
| 3 | candidate / 480 | 10.220 / 13.504 | 8.782 | 22 | 6 |
| 4 | base / 480 | 20.594 / 23.563 | 19.212 | 72 | 93 |
| 5 | candidate / 320 | 6.047 / 8.059 | 5.056 | 0 | 0 |
| 6 | base / 320 | 12.995 / 14.883 | 10.965 | 7 | 17 |

## Method and interpretation

Base `aeca19a9` is main immediately before #278; candidate `287d1dc4` differs
only by #278. Both include #272, #273 and #274. In contrast, the historical
[#275 candidate](../2026-10-29-battledome-post-spawn/REPORT.md) predates #274.
The concurrent base isolates the obstacle index; changes from the historical
study cannot all be assigned to it.

Reuse #275's real protocol-19 sockets, fully decoded received-frame shared A4
steering and received-rule-gated constant boost. Use shipped Battledome arena
and delivery defaults, seed 7, 641 socket slots and zero in-process bots. No
authoritative bot observations or thin clients. Both hosts use Rust 1.99 and
the default portable release profile, empty Rust flags and debug=0. Both native
boost-heading/rule-gate regressions passed; shared policy/client sources match,
and every arm uses the one candidate client binary.

The target is one c7a.medium vCPU; all simulation/socket work is on CPU 0.
A same-AZ c8a.4xlarge Spot generator has 16 physical/vCPUs and one thread per
core, with six decoding workers on CPUs 0–5 and control on CPU 7. Traffic uses
private IPs over a real network. This measures same-AZ delivery, not player WAN
latency. No human play hold, local native work or shared SSH guest use.

Each row starts fresh, warms 300 wall seconds at 80, then settles 60 and
measures 90 seconds at each natural 80/160/320/480 stage through the requested
target. Earlier stages are calibration only. CPU sums aligned simulation and
socket clocks per tick and independently reconciles process task clocks.
Canonical results use complete 100-tick interior blocks; client reports must
be wholly within the interval. Keep all transitions and full-run deferrals.
The ledger's legacy JSON field `deferredOutsideAllMarkedWindows` counts outside
the counted interior intervals, including trimmed edges; the table labels it
"Outside counted interiors" to avoid implying exact mark-boundary coverage.

The frozen generator gates require worker CPU p50 below 70%, p99 below 80%,
worst input-lateness p99 below 42 ms, stable queues, no socket errors and at
least 99% living-tick boost duty, including calibration. Server overload does
not discard a qualified row. Completed respawn waits are censored by the
window; pending count/age and alive seats provide the accompanying context.

"Holds" requires zero deferrals with valid client/queue/boost gates. The old
online reducer also trims partial 100-tick edges, so whole-marked-window zero
is proved with the last census before the start and first census after the end,
as well as online/canonical zeros. A nonzero enclosing counter difference is
not proof that its deferrals all occurred inside the exact marked interval;
it simply cannot establish a hold. Missing boundary coverage also fails that
proof. The audit is bound to each verified archive, original summary and census.
The host permits about 250 ms of backlog and at most eight catch-up ticks per
wake before deferring excess backlog (`Simulation::due_ticks` in
`src/client/server.rs`). Zero deferrals therefore does not mean every tick fits
7 ms; CPU tails and actual delivery/stall measurements remain separate axes.

Worlds have matched scheduled wall prehistory, not identical simulation age,
geometry or commands. Socket scheduling and admission speed can alter later
client inputs and ecology. Report every window and its order separately;
AB/BA is two windows per arm, not a repeatable speed ratio or a capacity study.
The slowest-1% partition is exclusive; nested broadcast phases overlap their
parent. The requested three-way "other" includes minimap.

## Limits and next leads

At candidate-480, the slowest 1% contains 127 ticks averaging 10.677 ms CPU.
Sockets+broadcast take 59.37%, spawn 24.14%, and other including minimap 16.49%.
The next two measured CPU leads at this tested limit are:

1. **Socket-thread work:** 3.622 ms per slow-tail tick, about 33.9% of total.
   Profile its send/queue paths under this constant-boost traffic before choosing
   an exact change. This phase clock does not separate polling from individual
   sends, kernel work or queue bookkeeping.
2. **Broadcast work excluding minimap:** 2.717 ms, about 25.4% of total.
   Recipient instrumentation is 2.740 ms within the 3.200-ms parent broadcast
   span, including nested work; do not add these to the parent. Of the 127 tail
   ticks, 45 coincide with the food-epoch event (72-tick cadence). Profile the
   recipient packet/visibility work at those epochs before choosing an exact
   sharing or reuse change. Coincidence does not identify a removable cost.

Phase clocks locate cost; they do not prove the work is removable. These are
ranked investigation leads only; no proposed optimization is implemented here.
The post-hoc diagnostic keeps the same canonical intervals. Candidate's first
320 row puts all 27 deferrals in one 100-tick census span; its largest busy-wall
tick is 17.483 ms with 5.819 ms simulation CPU and 2.736 ms socket CPU. The
second candidate row has no deferrals and an 8.059-ms busy-wall maximum. Base's
second 320 row accumulates 73 deferrals in seven census spans. These observations
show burst/run variation; they do not identify a scheduler, logging, socket or
hypervisor cause. Host core accounting reports zero steal in both first-pair
windows, which does not locate the unexplained wall/CPU difference.

At 480, candidate deferrals occur in 22 census spans (largest six ticks), versus
72 spans for base (largest 93). The CPU and wall tails both improve, but mean
CPU leaves little room at this load. The counter adds about 0.47 s versus
29.72 s of world-time lag over the respective canonical windows at 7 ms/tick.
No row is dropped or reselected using this post-hoc diagnostic.

## Provenance, cost and cleanup

Each primary requires a closed archive, matching size/SHA, complete source-
verified retrieval receipt, exact online replay, qualified canonical interval,
and archived runtime/source/binary pins matching the master build. The public
assembler keeps one manifest pin per arm and binds hold proofs to the same
archive. Failed attempts and rejected generators are retained separately.

The combined cap is $3, Spot only, through the limited profile. Both instances
have creation tags, terminate-on-shutdown, delete-on-termination volumes and
verified 160-minute in-guest poweroff timers. No timer extension is allowed.
The initial full-timer conservative bound is $2.162, including $0.65 reserved
for noncompute. The final conservative [cost](cost.json) bound is **$1.640**
(rounded up): $0.989334 compute through confirmed termination, plus the $0.65
noncompute allowance. This is not an invoice. Sources were retained until the
closed final archive downloaded with matching size/SHA; the
[cleanup receipt](cleanup.json) pins that retrieval before termination.
No Spot interruption, replacement or rejected generator occurred. The initial
setup SSH dispatch timeout and receipt-audit correction are retained in the
attempt log. There were no human seats.

All 17 enabled regions were audited: no study instances, volumes or open/active
Spot requests remain, and the study key was deleted. Global inventory includes
one unrelated running catalog-rows worker builder, its attached volume and its
active Spot request. That builder is also attached to this study's security
group, so deletion returned a dependency error. The maintaining agent directed
that the group remain in place and will delete it after that builder terminates;
the task audit passes with this explicit handoff. No unrelated or protected
resources were changed. The queued latency round must use a fresh group.

Both portable builds/treatment regressions, inherited reducer/delivery synthetic
controls, focused diagnostic unit/boundary controls and all 16 archived receipt,
master-pin and hold mismatch controls passed on AWS after counting. Complete
online ranges reproduce exactly; task clocks reconcile within 0.002925 ms per
tick at worst. All 20 windows qualify: maximum primary worker CPU p99 is 28.0%,
worst input-lateness p99 34.853 ms and minimum living-tick boosting 99.38%.
The collector consumes 0.052–0.080 ms CPU per tick separately from reported world
CPU and shares the target core. This is an instrumented-host study.

No fitted model, rule change or held-out physics inference is made. Two primary
windows per arm at 320 and one per arm at 480 support descriptive comparisons,
not a repeatable speed ratio. No independent agent review is claimed. The full
Rust group is inapplicable to this research-only change. Research policy #279
requires only the index row; no changelog or shipped file changes.
