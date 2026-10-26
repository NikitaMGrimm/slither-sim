# Safe Battledome spawning: population follow-up, 2026-10-26

The [initial 40-second attempt](INITIAL-ATTEMPT.md), [results](results.json) and
[source pins](source-pins.json) are retained unchanged. This follow-up selects
the best measured design with **zero observed deaths within three seconds and
zero strict 600-unit/two-second head-path intrusions**. It restores a full
32-seat background population; 64-seat recovery remains partial.

## Chosen policy

- Circular newborns use an area-uniform **2,684–3,400-unit band**, unchanged
  26-anchor bodies and random tangent direction. The outer edge leaves **2.12 s
  at maximum boost / 5.04 s at normal newborn speed** before the existing
  60-unit wall margin along a straight tangent. At 3,200 these were 3.37/8.03 s.
- Keep **400-unit whole-body clearance**, and the strict **600-unit exclusion
  around every living head's straight two-second path** at its current speed.
- The newborn head must also clear current bodies by **three seconds of normal
  travel plus existing contact exclusion** (about **627 units** at the floor).
  Its inward and tangent departure segments cover three seconds at normal
  speed (about 551 units), with **400-unit clearance** from current bodies and
  projected paths. Larger contact exclusions still apply.
- Search at most **4,096 poses**, with cached conservative bounds and unchanged
  exact distance tests. A failed search waits **14 ms of world time**; at most
  one search runs per tick. Waiting socket players precede FIFO bots. There is
  no tight fallback; exhausting a search does not prove that no legal pose exists.

The extra space follows the newborn head rather than requiring its whole
1,050-unit tail to keep 600 units. The frozen grid's cheaper settings admitted
early body deaths; the departure checks protect leaving spawn. These are custom
mode choices, not official mechanics or collision immunity. Pit, wall, size
floor, food, kill score, movement and bot decisions are unchanged.

## Method

Exploratory **local-simulator**, loopback only, shared Windows desktop / Ryzen
7 9800X3D, release/thin LTO. Affinity **0x30**, at most two logical CPUs,
`CARGO_BUILD_JOBS=2`, sequential hosts. Seeds **7/29**, **120 wall seconds** each,
four reserved socket seats and 12/28/60 background seekers, one bot worker,
zero added latency. The original probe driver joins each second when free,
leaves on admission, waits at most 15 s and censors a final pending join before
shutdown. These are configured seats; full background population at 32 seats
is 28 bots, with socket probes appearing briefly.

Primary load/population window: **wall seconds 60–120**. Report 60–90 and 90–120
separately before discussing equilibrium. C1 counts bot births with complete
3/10 world-second followup; probes leave immediately and are excluded. C2 counts
any newborn polyline within the configured head-path zone; report both its own
horizon and strict 600/2. All zones in this study have width 600. C3 retains the
initial study's opportunity metric: a living newborn aged at most 10 s, still
outside radius 2,684, whose radial route to that radius passes within 200 units
of a later newborn. It does not establish actual intent or a dodge.

Waits are first request-to-admission wall time, continuing across control
refusals. Quantiles include admitted requests only; pending requests and actual
rejoins are reported separately. Busy time includes telemetry and shared-host
scheduling; lower population also lowers load. No isolated speedup is claimed.

## Binding constraint

Sampled **4,096 poses per setting per snapshot**, eight evenly indexed late
birth snapshots per seed/seat pair from the initial attempt: 32 snapshots total.
For strict 3,200/600/2, mean admissible fractions were **1.196% at 32 seats and
1.152% at 64**; all sampled birth states had some accepted poses. Averaging
`(1-p)^N` over those per-state fractions, 64 draws miss **55.0% / 51.0%**,
versus **7.8% / 1.3%** for 1,024. This establishes missed search opportunities
in the initial snapshots, not absence of geometric crowding. Birth conditioning
is optimistic, so periodic late states are sampled separately below.

The cached broad phase produced identical **256 acceptance counts** across
all eight settings on those snapshots. The diagnostic took 46.95 s with exact
unpruned scans and 0.93 s with cached bounds, on the same two CPUs. Exact
distance tests still decide acceptance; randomized differential and threshold
tests protect that equivalence. File hashes preserve the diagnostic identities.

## Frozen sensitivity grid

All eight combinations used **1,024 candidates / 42 ms** backoff, both seeds
at 32/64 seats, plus the original **64 / 250 ms** control: 36 completed runs.
No completed run was repeated after the provider interruption. Counts are
pooled over seeds; living is the mean of late per-run means. C2 own is zero
for every row; shortened horizons still intrude into the strict two-second zone.

| Outer / body / horizon | Living 32 / 64 | C1 ≤3 s, 32 / 64 | Strict C2 hits, 32 / 64 | Bot wait p90, 32 / 64 (s) | ms/tick, 32 / 64 |
| --- | ---: | --- | ---: | ---: | --- |
| 3200 / 600 / 2 s | 26.53 / 27.77 | 1/128 / 1/132 | 0 / 0 | 18.07 / 72.54 | 0.450–0.542 / 0.570–0.613 |
| 3200 / 600 / 1.5 s | 24.75 / 25.29 | 0/111 / 0/133 | 24 / 19 | 17.60 / 78.57 | 0.449–0.453 / 0.513–0.564 |
| 3200 / 400 / 2 s | 27.69 / 27.58 | 2/157 / 2/143 | 0 / 0 | 7.32 / 47.34 | 0.431–0.527 / 0.366–0.605 |
| 3200 / 400 / 1.5 s | 27.27 / 32.74 | 1/158 / 2/182 | 41 / 34 | 7.11 / 58.93 | 0.449–0.607 / 0.655–0.726 |
| 3400 / 600 / 2 s | 27.87 / 30.87 | 3/159 / 2/185 | 0 / 0 | 6.40 / 56.85 | 0.425–0.543 / 0.618–0.721 |
| 3400 / 600 / 1.5 s | 27.92 / 30.27 | 1/147 / 3/184 | 43 / 28 | 6.40 / 53.15 | 0.451–0.472 / 0.600–0.661 |
| 3400 / 400 / 2 s | 28.02 / 40.91 | 5/160 / 22/327 | 0 / 0 | 2.52 / 22.72 | 0.453–0.458 / 0.951–1.065 |
| 3400 / 400 / 1.5 s | 28.03 / 43.02 | 5/183 / 12/326 | 38 / 61 | 2.57 / 20.99 | 0.439–0.733 / 0.970–1.040 |

All grid early deaths were body collisions. No grid point met both population
recovery and the exact early-safety requirement. The 3,200/600/1.5 point had zero
early deaths, but low population and strict-zone intrusions. The 3,400/400 points
filled more seats and shortened waits but admitted early deaths, so were rejected.
One grid run (3,400/400/2, seed 29, 64 seats) had two deferred ticks; others had
none. C1 ten-second causes, C3 and all per-run outcomes remain in
[follow-up-results.json](follow-up-results.json).

## Additional measured refinements

Each prototype used fresh 120-second runs at 32/64 seats and both seeds after
rebasing onto current main. They are adaptive exploratory comparisons, not
held-out validation. Retrospective failed-birth geometry suggested the checks;
only the fresh runs determine their gate result. No failures were waived.

| Prototype | Added departure rule | Living 32 / 64 | C1 ≤3 s, 32 / 64 | Strict C2 total | Verdict |
| --- | --- | ---: | --- | ---: | --- |
| A | 200 units; no derived head runway; 1,024 / 42 ms | 27.98 / 40.49 | 0/156 / 1/271 | 0 | Rejected |
| B | 400 units plus derived head runway; 4,096 / 14 ms | 27.99 / 33.28 | 0/150 / 0/206 | 0 | Chosen |
| C | 200 units plus head runway; 4,096 / 14 ms | 27.99 / 43.12 | 1/141 / 5/298 | 0 | Rejected |
| D | 400 to bodies / 200 to projected paths plus head runway; 4,096 / 14 ms | 28.01 / 37.94 | 0/158 / 1/249 | 0 | Rejected |

B is the only refinement passing the strict gates. The frozen selection ranking
maximizes the smaller of recovered 32/64-seat population ratios, then prefers
lower waits and cost; no tie-break changes the result because the other recovery
candidates fail safety. All prototype outcomes are retained.

## Before / chosen policy

The control is #113 at `dfd64f2`, extended to 120 s. The initial ring baseline
and 40-second attempt remain in the initial report. Wait cells are p50/p90/max
seconds; pending bot counts include right-censored rejoins.

| Seats / policy | Mean late living | Bot wait, all admitted requests | Rejoin wait | Pending bots | Socket wait | ms/tick |
| --- | ---: | --- | --- | ---: | --- | --- |
| 16 / Before | 12.04 | 0.003 / 1.827 / 3.854 | 0.001 / 0.002 / 0.003 | 0 | 0.019 / 0.024 / 1.536 | 0.180–0.196 |
| 16 / Chosen | 12.00 | 0.001 / 0.061 / 0.329 | 0.001 / 0.001 / 0.001 | 0 | 0.019 / 0.022 / 0.041 | 0.169–0.205 |
| 32 / Before | 21.02 | 18.212 / 43.970 / 55.337 | 35.941 / 52.596 / 55.337 | 9 | 0.542 / 3.072 / 8.366 | 0.425–0.468 |
| 32 / Chosen | 27.99 | 0.007 / 5.556 / 11.765 | 0.002 / 0.527 / 5.406 | 0 | 0.033 / 0.382 / 2.355 | 0.476–0.645 |
| 64 / Before | 22.87 | 29.308 / 97.783 / 118.539 | none admitted | 71 | 0.762 / 3.789 / 8.849 | 0.555–0.706 |
| 64 / Chosen | 33.28 | 31.648 / 41.393 / 56.332 | 36.383 / 42.858 / 56.332 | 52 | 0.396 / 1.980 / 7.678 | 2.107–2.420 |

The 32-seat problem is recovered: approximately 28 living bots, no pending bots,
and a subsecond admitted rejoin p90. At 64 seats the chosen mean is about 33.3,
up from 22.9, but below the initial ring's roughly 41 and with long rejoin waits.
The control admitted no 64-seat rejoins before shutdown; comparing its admitted
initial-request median to completed chosen rejoins would hide the censored tail.
This is the best passing measured candidate, not a claim that the old 64-seat
population or short waits have been fully recovered.

| Seats / policy | C1 ≤3 s | C1 ≤10 s | Strict C2 hits / births | C3 opportunities / births | Deferred ticks, 60–120 s |
| --- | --- | --- | --- | --- | ---: |
| 16 / Before | 0/49 | 2/47 | 0/275 | 0/275 | 0 |
| 16 / Chosen | 0/53 | 2/51 | 0/279 | 1/279 | 0 |
| 32 / Before | 0/89 | 4/85 | 0/225 | 0/225 | 0 |
| 32 / Chosen | 0/150 | 9/144 | 0/376 | 1/376 | 0 |
| 64 / Before | 0/97 | 4/93 | 0/229 | 0/229 | 0 |
| 64 / Chosen | 0/206 | 22/198 | 0/375 | 1/375 | 0 |

C3 is not identically zero: its full inward-route opportunity horizon extends
beyond the three-second departure guard. Zero observed C1/C2 is not a guarantee
about later turns, boosting, different controllers or future runs.

### Admissions versus deaths

Values pool the two seeds; rates count bots only, per wall second. Population
and rates still change within 120 s, so no equilibrium capacity is asserted.

| Seats / policy | Living 60–90 → 90–120 | Admissions/s 60–90 → 90–120 | Deaths/s 60–90 → 90–120 |
| --- | --- | --- | --- |
| 32 / Before | 20.15 → 21.88 | 0.167 → 0.200 | 0.167 → 0.100 |
| 32 / Chosen | 28.02 → 27.97 | 0.417 → 0.350 | 0.417 → 0.350 |
| 64 / Before | 22.37 → 23.37 | 0.317 → 0.267 | 0.233 → 0.233 |
| 64 / Chosen | 34.90 → 31.65 | 0.617 → 0.483 | 0.717 → 0.433 |

### Periodic-state geometry check

Eight periodic states after wall second 60 per seed/seat pair, 4,096 poses
per setting per state: 96 snapshots, 16 per table cell. Original-zone columns
use 3200/600/2; chosen columns include all chosen departure/head-runway rules.
These are state-based estimates, not existence proofs.

| World / seats | Original-zone mean admissible | Chosen-rule mean admissible | States with zero chosen poses /16 |
| --- | ---: | ---: | ---: |
| Before / 32 | 0.4608% | 1.4282% | 0/16 |
| Before / 64 | 0.2686% | 1.5305% | 0/16 |
| Larger budget, original geometry / 32 | 1.8250% | 2.4811% | 1/16 |
| Larger budget, original geometry / 64 | 0.0092% | 0.3128% | 1/16 |
| Chosen / 32 | 0.2899% | 0.9109% | 2/16 |
| Chosen / 64 | 0.0046% | 0.0031% | 14/16 |

The initial policy was search-limited in many available-pose states. The larger
budget substantially recovers 32-seat population, but at 64 seats the selected
rules have zero sampled poses in 14/16 mature states despite 4,096 draws. Repeated
searches cannot fill those sampled holes until bodies move. Geometry now binds
much of the remaining 64-seat cost; this finite diagnostic does not prove there
are no rare legal poses. The selected policy spends about 2.1–2.4 ms/tick
at 64 seats, including its higher search budget.


## Integration, verification and limits

The frozen grid used `dfd64f2` plus cache/instrumentation. Refinements and chosen
sanity runs used main `9480608` with this change, including #116/#117. #116's
time-zero construction and containment checks do not change this fixed arena's
contained initial bodies; its error wording changes no pose decision. #117's
prey paths are not exercised (`prey.count: 0`). Kiosk grace is 1,200 s, beyond
these runs; configured latency has no variation or pauses. Intervening host and
latency fixes remain integrated. Scheduling-dependent control means frozen grid
counts are not claimed bit-identical on new main, and are kept separate from
post-integration results.

The final branch also includes main `aeb3cfe` (#118), a documentation/research
addition. Runtime and build files match the checked performance snapshot
`f7b5628`; no grid or chosen-policy result depends on that last integration.

`uv run --locked python tools/check.py rust` passed after integration,
including formatting, Clippy feature combinations, default/core-only tests,
doctests, C ABI, Python crate checks and performance-harness tests. The five
focused cache/departure tests and players-first/Battledome checks passed.

`python -B -m unittest discover -s tools -p test_documentation.py`: 7 passed.
`uv run --locked python tools/perf_compare.py --base origin/main --repeats 1
--no-profiles` completed with expected **exit 3**: all **eight official/legacy
definitions and full-state traces equal**, including wire bytes; only
`battledome-32-step` and `client-battledome-64` definitions/traces changed.
Trace hashes and verdicts are retained with the follow-up results.

Source-level regressions cover the complete newborn body, head-path zone,
departure/head runway, exact thresholds, cached/exhaustive admission equality,
invalid geometry, waiter cancellation, players-first ordering and previous bot
connection retirement. The existing official fallback/positive-margin/RNG tests
remain in place. All selected 120-second runs have zero deferred ticks.


Full plans, raw events/periodic states, timing logs, scripts and binaries are
**local-only** under `outputs/safe-spawn-r2/` in this task worktree. Selected
numbers and file identities are in `follow-up-results.json` and
`follow-up-source-pins.json`; no raw recordings or binaries are committed.
No outside servers, stock-browser verification or independent review. These
short, scheduling-dependent custom-mode experiments do not identify official
mechanics or establish long-run capacity. The unchanged official fallback keeps
its evidence-based scope, candidate draws and positive-margin requirement.
