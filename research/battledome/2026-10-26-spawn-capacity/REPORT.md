# Crowded Battledome admission, 2026-10-26

Strict arena growth (B) admits the measured 40/64/80-seat populations while
keeping all existing spawn checks. Fixed-arena relaxation (A) is rejected:
it introduces strict head-path intrusions and leaves pending requests.
This is a synthetic **local-simulator** product comparison, not an official
rule or a measured comparison against a community Battledome server.

## Policy and selection

Through 32 seats target the 3,584 wall, 1,784 pit and 2,684–3,400 spawn band.
Above it target `3584 * (1 + 1.5 * (max(seats,32)/32 - 1))`: wall radii
4,928/8,960/11,648 at 40/64/80. Before searching, queued joins contribute too.
At two/four world seconds of selected-request queue age, allow 1.5/2 times
the target. Keep main's immediate growth, 23.4375 units/s shrink and 3% dead zone.
Before further host expansion, hold growth until the latest admitted life has
its three-second departure window. Pause admissions during that hold, so births
cannot postpone expansion indefinitely; spend no search work while held.
Pit and spawn radii follow the live wall; snakes do not stretch or teleport.
A growing pit passing over a snake does not count as physical entry. That
unentered life may escape; a later inward crossing, or a life initialized
inside, still commits it to the one-way exit rule. Already entered lives stay
entered. This reuses the existing pit-entry bit, with no timer or immunity.
Food target/refill retain main's live-area ecology. Floor, boosting, scoring,
food and one-way pit rules are unchanged. The pit raster uses the live wall
like snakes and the stock own-player marker; decorative food follows the pit.

Keep 400-unit body clearance, the 600-unit/two-second head-path exclusion,
three-second normal-speed head runway and inward/tangent departure checks.
Contact + 40 remains a lower bound. At most one 4,096-candidate search per tick,
14 ms after failure, waiting sockets before FIFO bots: queue length does not
multiply search budgets. Request age is host queue state; the existing live
radius retains the extra space. The latest-birth tick and request age are
transient host state. No core saved field/format change; rules v3 rejects old
v1/v2 checkpoints explicitly.

A kept strict-first searching but used body widths 400/300/200, head horizons
1.5/1/0.5 s, path widths 600/400/200 and departure widths 400/300/200 after
2/3/4 seconds. It retained contact exclusion and the head runway, and selected
the best body-clearance relaxed pose only after exhausting the strict search.
The production change contains none of those tiers.

[PLAN.md](PLAN.md) retains the frozen protocol and exploratory amendments.
At 80 seats, initial sqrt-area growth reached 70.8 late living with 45 strict
intrusions; linear-radius growth reached 79.8 but had 10 intrusions. Larger
strict-growth screening reached 79.7 with zero intrusions. A later 40-seat run
had a 10.535 s initial bot wait, motivating timed space allowances before the
final comparison. Retain all 20 s pilots, 60 s preliminary runs and interrupted
attempts. An initial control logger zeroed queue ages; a death logger repeated
retained history. Corrections preserve the originals, emit new deaths only,
deduplicate (victim,time), and require nonnegative ages. The initial apparent
early pit death was a logger artifact, not a waived failure.

## Method

Shared Windows x86-64, Rust 1.99.0, locked release/thin LTO. The first 45-run comparison
is retained in [initial-results.json](initial-results.json). Its growth/64/seed
101 run had three real early pit deaths at ages 1.764/0.357/2.604 s, failing
the safety gate. A controlled no-opponent repro died at 0.651 s because growth
counted as entry. Physical-entry tracking fixes that repro and preserves later
entry/exit and exact checkpoint continuation. These failures remain failures.

An intermediate corrected comparison was also stopped and retained in
[unqualified-corrected-results.json](unqualified-corrected-results.json).
A live process check found affinity 0xFFFF: the Python wrapper's unchecked
Win32 affinity call had failed because its ctypes HANDLE signature was missing.
A minimal repro returns zero/ERROR_INVALID_HANDLE (6). All earlier cohorts are
UNQUALIFIED for the requested resource limit and excluded from the final table.
This was an agent measurement-harness mistake; their causal safety failures
are preserved, not waived.

The first affinity-verified growth/40/seed 7 hold then exposed a remaining pit
death at age 2.534 s. [The rejected round](rejected-moving-edge-results.json)
retains it. A controlled grazing chord reproduces false re-entry during slow
shrink: a head inside the old edge was tested against the smaller new edge.
The corrected test requires an unentered head to start outside both pre-update and
live edges, then applies the ordinary swept test against the live pit. Growth
and shrink passing over a head cannot enter it. No new state or timer is needed.

The next corrected round still had a real growth/40/seed7 pit death at age
2.891 s. [The pre-hold results](rejected-pre-hold-results.json) retain all its
completed runs. Correct physical-entry tracking did not prevent actual later
crossings near a recent birth after expansion. The final host growth hold
leaves recent births their full three-second departure window, and pauses
admissions while expansion is held. A manually stepped real-host regression
checks that the queue stays intact until the deadline, then admits again;
Raw growth denial leaves the whole core checkpoint unchanged.

The `protected-*` search-only cohort was retained in
[pre-delivery-results.json](pre-delivery-results.json) when timing was extended
to the entire admission call and delivery integrated newer main and the
source-review decoration correction. None of its safety outcomes is waived.

The final confirmation has **30 new sequential 60-wall-second holds**: control
and growth on integrated main with the hold, IDs `delivery-*`, seeds
7/29/47/71/101, seats 40/64/80, alternating pair order.
A was stopped for strict-path failures; all six completed, affinity-verified
relaxed holds (two per seat count, seeds 7/29) are retained as its rejected
comparison. Its fixed-pit/admission path is unchanged by the moving-edge fix.
Its two-run cost is descriptive; no performance ranking is based on that sample.
The typed wrapper asserts parent and every child process mask is exactly 0x30;
separate PowerShell queries check active hosts. Earlier growth results are
kept separate from the final candidate. No safety failure is waived.
Four real loopback socket probes plus 36/60/76 background A4
seekers, one bot worker, no added latency, default ecology. Probes use A4/42 ms,
pings/250 ms, stay five seconds when alive, close for 100 ms and rejoin;
deaths rejoin too. These are configured seats; living is reported separately.

Qualified long runs use verified affinity **0x30**. Builds/checks use `CARGO_BUILD_JOBS=2`,
`RAYON_NUM_THREADS=2`, `RUST_TEST_THREADS=2`; remaining checks were pinned to
separate affinity 0xC0. Earlier search-only holds overlapped required checks
on those separate CPUs; the final strict confirmation starts after the Rust
group finishes. This shared desktop retains frequency/cache/scheduling noise: medians/ranges are descriptive, not
confidence intervals. All seeds were examined by the correction round; this
is adaptive corrective exploration, without an independent held-out claim.

Queue waits are request-to-admission world time, sockets and bots pooled.
Socket waits are wall request-to-first-own-appearance, including handshake.
Quantiles select sorted index `round((N-1)*p)` among completed requests; pending requests remain
right-censored. Bot respawns are later bot lives; socket waits include deliberate
rejoin probes. Early-death denominators include lives with complete 3 s followup
or deaths within 3 s; administrative close/shutdown before 3 s is censored.
Late living averages per-run 30–60-world-second samples. Strict intrusions test
the actual newborn polyline against living heads' 600/2 paths at admission.

## Waits, population and safety

Wait cells are **p50 / p95 / max seconds**, pooled across the reported seeds. Strict policies have five holds/cell;
rejected A has two. Pending
counts sum the reported shutdowns (five strict, two relaxed). Early deaths show deaths/qualified lives;
intrusions show hits/all births. Zero observed failures is not immunity.

| Seats / policy | Runs | Mean late living | Completed queue wait | Pending | Deaths ≤3 s | Strict intrusions |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| 40 / Main strict fixed | 5 | 30.53 | 1.960 / 30.982 / 42.959 | 42 | 0/443 | 0/474 |
| 40 / A: fixed relaxation | 2 | 34.45 | 3.038 / 11.991 / 14.224 | 7 | 0/216 | 101/228 |
| 40 / B: strict growth | 5 | 39.89 | 0.007 / 4.774 / 4.963 | 0 | 0/431 | 0/432 |
| 64 / Main strict fixed | 5 | 31.22 | 2.408 / 54.285 / 60.032 | 166 | 0/466 | 0/501 |
| 64 / A: fixed relaxation | 2 | 46.72 | 8.218 / 33.257 / 40.985 | 33 | 0/229 | 138/243 |
| 64 / B: strict growth | 5 | 63.89 | 0.042 / 1.162 / 4.977 | 0 | 0/577 | 0/579 |
| 80 / Main strict fixed | 5 | 31.45 | 2.520 / 56.035 / 60.025 | 235 | 0/435 | 0/465 |
| 80 / A: fixed relaxation | 2 | 44.52 | 8.456 / 49.266 / 60.018 | 70 | 0/226 | 127/236 |
| 80 / B: strict growth | 5 | 79.92 | 0.105 / 0.511 / 0.567 | 0 | 0/643 | 0/646 |

| Seats / policy | Initial bot join | Bot respawn | Socket wall wait, all joins |
| --- | --- | --- | --- |
| 40 / Main strict fixed | 11.907 / 37.849 / 42.959 | 13.202 / 24.542 / 30.758 | 0.571 / 3.032 / 5.383 |
| 40 / A: fixed relaxation | 4.018 / 13.209 / 14.224 | 4.011 / 5.824 / 7.434 | 1.420 / 3.448 / 4.258 |
| 40 / B: strict growth | 0.735 / 4.900 / 4.963 | 0.000 / 0.000 / 0.000 | 0.006 / 0.016 / 0.030 |
| 64 / Main strict fixed | 24.227 / 58.800 / 60.032 | 45.199 / 47.187 / 47.187 | 0.551 / 2.970 / 5.003 |
| 64 / A: fixed relaxation | 11.515 / 34.237 / 40.985 | 15.659 / 25.508 / 27.300 | 2.284 / 4.025 / 4.028 |
| 64 / B: strict growth | 0.238 / 1.575 / 4.977 | 0.000 / 0.000 / 0.000 | 0.006 / 0.026 / 0.046 |
| 80 / Main strict fixed | 23.527 / 58.814 / 60.025 | — | 0.789 / 3.576 / 6.730 |
| 80 / A: fixed relaxation | 19.012 / 53.914 / 60.018 | 27.713 / 37.002 / 37.765 | 2.116 / 4.035 / 4.449 |
| 80 / B: strict growth | 0.294 / 0.532 / 0.567 | 0.000 / 0.000 / 0.000 | 0.005 / 0.018 / 0.029 |

Per-run causes, followup censoring, socket errors, deferrals and search timing
are in [results.json](results.json). An absent respawn quantile means none was
admitted, not zero wait. Every selected row has zero early deaths, so there is no selected early-death
cause. Causes in rejected cohorts remain in their records.
Socket error counts: control/40/seed 7: 4, control/64/seed 29: 4, control/80/seed 29: 4, control/80/seed 71: 1, growth/40/seed 101: 1, growth/40/seed 71: 4, growth/80/seed 29: 1, growth/80/seed 47: 2, fixed/80/seed 7: 2. Readers reported connection resets. Every host stopped successfully. The harness stops the host before joining readers, so shutdown is a plausible cause; missing error timestamps prevent exact attribution. All affected runs and errors are retained.

## Admission cost

Windows `GetThreadTimes` user+kernel deltas measure the entire
`Simulation::admit_waiting` call, including queue scans, growth holds, connects,
candidate searches and nested telemetry, divided by executed host ticks.
Socket command intake and broadcasts are outside that call. A second nested
counter measures searches. Full-call counters accumulate without I/O; per-tick
emission happens afterwards. The rejected A holds have search-only counters;
full admission was not collected for that arm, and its two-run CPU sample does
not rank policies. Search CPU uses the same boundary in all arms.
Coarse scheduler accounting makes zero/near-zero values below useful resolution,
not zero cost. Full admission elapsed per tick is also shown as a conservative
ceiling on true single-thread admission CPU. Search elapsed p50/p95/max and
per-tick totals are retained in the JSON.
Host busy includes telemetry and scheduling; it is not process CPU or generator
cost. The host status counter is sampled before stop and can omit closeout ticks.
Strict-policy phase costs divide by completed per-tick records after shutdown;
the JSON retains both counts and checks consecutive world ticks and one
admission call per tick. Host busy retains its pre-stop status denominator;
rejected A has only its sampled pre-stop denominator.
The fixed controls spend most admission work exhausting failed searches;
growth removes repeated failures by providing geometrically clear poses.

| Seats / policy | Full admission CPU ms/tick median (range) | Search CPU ms/tick median (range) | Full admission elapsed ms/tick median | Host busy ms/tick median | Searches |
| --- | --- | --- | ---: | ---: | ---: |
| 40 / Main strict fixed | 1.7037 (1.6602–1.8211) | 1.6527 (1.6110–1.7628) | 2.0930 | 2.299 | 21679 |
| 40 / A: fixed relaxation | — | 2.5790 (2.3745–2.7835) | — | 3.222 | 8686 |
| 40 / B: strict growth | 0.0582 (0.0455–0.0583) | 0.0565 (0.0437–0.0583) | 0.0672 | 0.835 | 1083 |
| 64 / Main strict fixed | 1.6926 (1.6548–1.7353) | 1.6526 (1.6111–1.7025) | 2.1033 | 2.308 | 21700 |
| 64 / A: fixed relaxation | — | 2.8775 (2.8268–2.9282) | — | 3.507 | 8696 |
| 64 / B: strict growth | 0.0510 (0.0437–0.0621) | 0.0510 (0.0419–0.0602) | 0.0651 | 0.375 | 1122 |
| 80 / Main strict fixed | 1.6600 (1.5793–1.7701) | 1.6199 (1.5283–1.7264) | 2.0870 | 2.297 | 21673 |
| 80 / A: fixed relaxation | — | 3.0736 (2.9595–3.1877) | — | 3.674 | 8692 |
| 80 / B: strict growth | below resolution (0.0000–0.0018) | below resolution (0.0000–0.0018) | 0.0060 | 0.409 | 648 |

The owner's separate [all-socket main baseline](https://github.com/NikitaMGrimm/slither-sim/blob/4915803/research/performance/2026-10-26-host-known-slots/REPORT.md)
uses WSL2 with all host threads on one logical CPU. At 40/64/80, total process
CPU is 3.203/2.859/3.132 ms/tick; inclusive admission shares are
90.39/89.36/90.53% of sampled user CPU. At 64/80, completed wait p50/p99 is
21.651/35.862 and 33.738/48.643 s, with about 29/44 waiting. Those shares cannot
be multiplied into measured admission CPU/tick: denominators/platforms differ.
Our matched Windows table measures full admission and search cost in a
bot-plus-probe workload; it does not replace the one-core all-socket grade.
Final strict-policy measurements include that worker's known-slot optimization
through main integration; the same implementation is present in both arms.

## Feel, verification and limits

Tests check identical initial wall, pit, band and floor/score settings at 16 and
32 seats. After a larger wave, gradual shrink and the 3% dead zone still apply.
Normal radial spawn-to-pit distance stays 900–1,616 units, about 4.90–8.79 s at
floor cruise; area/density at the same population are unchanged. At 80 the normal
wall is 3.25 times base and density about 23.7% of 32-seat density; radial travel
is about 15.92–28.58 s. Extra queue space can lengthen it further. This is an
explicit tradeoff above typical sizes, not unchanged crowded-mode feel.

Synthetic regressions cover growth/shrink, queued-seat room, 2/4 s allowances,
strict body/head paths after admission, checkpoint continuation before/after tick one, bounded admission-radius
validation and old-revision rejection, live pit collisions, wire-coordinate capacity, identical pit raster
cells at 40/64/80 and allowance radii on 80/136/512-cell maps, and actual socket
removal/replacement of pit food markers. Floor/food/scoring and cache/exhaustive
regressions remain. Changed startup geometry exposed a debug food-index count
wrap: addition now matches existing wrapping subtraction before rebuild.

The CONTRIBUTING Rust group passed on the final production sources: formatting,
Clippy feature groups, default/core/bot/networking tests, doctests, C ABI and
Python crate check. The minimap extension and documentation checks passed. An independent
GPT 6.1 Sol source review covered admission, moving-pit semantics, checkpoint
contracts, stock geometry and regressions. Its redundant decorative-update
finding was corrected with a failing-before/passing-after shrink regression;
a same-cell physical replacement test preserves pickup lifetime identity.
The review was read-only and did not independently run checks or measurements.
No outside server, browser, macOS/Linux execution, full
tools/WASM/package group or human gameplay test was run.

Study base: `901b02e2e71144c6ffd35291ddb6603b2c6a1ca7`. The final strict
measurement binary uses main `150d4b1b70f7ebf0447cde70e9daa97aa69d3bc1`
plus this policy and instrumentation. Delivery rebases onto
`3a23dc9` (prey-visibility bounds and fidelity-report updates); source merged
automatically and both Unreleased changelog entries are preserved. This mode's
prey count is zero. Admission, spawn geometry and safety owners are unchanged;
the table stays pinned to the measured build rather than being relabeled as
a fresh measurement. The full Rust group was rerun after that rebase.
[Source pins](source-pins.json) retain measured production, instrumented and
current production hashes separately. The six rejected A holds use the earlier
`249cdb2` build; its fixed-pit admission is unaffected by later moving-pit fixes.
Its cost remains descriptive and search-only.
Full sources, runner, events,
copied binaries and failed checks remain **local-only** under this worktree's
`outputs/spawn-capacity/`. Hashes identify files without making them downloadable.
Selected data and retained failed-cohort tables preserve the declared
comparison cohorts; exploratory and interrupted attempts remain local-only. The owner relayed two-second spot reuse, a 500-unit clearance
with crowded fallbacks, between-spot placement and modest population growth
from a community Battledome server. This is inspiration only; its human death
rate and performance were not measured here. Measured A weakens strict
head-path protection; B instead makes its extra-space tradeoff explicit.

This does not prove 80 simultaneous humans, one-core dense network throughput,
long-run equilibrium or superiority over a community Battledome server. Search
work and space allowances are bounded, not worst-case wait in every crowd: if
all sampled poses violate strict safety, it continues waiting. Later turns,
boosts and shrinking-pit encounters remain possible. Observed short waits and
zero early failures apply to these runs.
