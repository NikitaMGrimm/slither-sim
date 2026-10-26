# Safe Battledome spawning, 2026-10-26

Exploratory **local-simulator** design comparison on a shared Windows desktop.
No outside game servers. Baseline: `36eb9cd`, including PR #110. Selected results
and source/build hashes accompany this report.

## Decision

Circular worlds keep `spawnRadius` as their inner circle and add optional
`spawnRadiusMax`. Battledome uses **2,684–3,200 units**, sampled uniformly by
area, with uniform angle and tangent direction. Its 26 anchors still trail
along their own circle, 42 units apart. Pit, wall, floor, kill score and food
rules are unchanged.

The outer edge leaves `sqrt((3584 - 60)^2 - 3200^2) / 437.5 = 3.37 s` of
straight tangent travel at maximum boost before the existing spawn wall margin.
This reserves time to turn without moving the wall or reducing newborn size.
The band starts at the old ring, keeping births outside routes already below it.

Require the full **600-unit centerline clearance** from every living body, and
from every living head's **two-second straight path at its current speed**.
Use the larger existing contact exclusion if needed. Two seconds spans about
48 nominal 42 ms decisions; the same 600-unit width gives lateral room as well
as forward time. These are custom safety choices, not recovered official rules
or predictions of later turns. Circular worlds never take the tight fallback.

If none of 64 poses passes, keep the socket and reserved seat waiting. FIFO
socket waiters precede FIFO bots. Try at most one join per tick; a failed search
delays all further searches by **250 ms of world time**. This replaces per-bot
retries with at most about four failed searches per world second, independent
of seats. Successful admission allows the next tick. Waiting can be indefinite,
and sustained human demand can postpone bots. Retire a rejoining bot's prior
connection before waiting so old frames cannot reach its reset decoder.

Without `spawnRadius`, the official candidate stream, 600-unit preference,
bounded positive-margin fallback and refusal behavior are unchanged.
`Simulator::add` remains deterministic initialization on the inner ring without
admission checks, for synthetic scenes.

## Method

Release/thin LTO, Windows MSVC, Ryzen 7 9800X3D. Two logical CPUs, affinity
`0x30`; `CARGO_BUILD_JOBS=2`, one timing run at a time. Seeds **7 and 29**,
16/32/64 configured seats: four socket seats and 12/28/60 background seekers,
one bot worker, unchanged Battledome presets otherwise, zero added latency.
Each host runs **40 wall seconds**. Baseline and candidate use the same driver
and instrumentation. Report load over wall seconds 5–40, with all startup
births retained for safety. These are configured seats, not promised concurrent
living counts.

One loopback probe starts at second 5 and requests a join each second when
free. It disconnects on admission, waits at most 15 seconds per login and
censors a pending final login one second before shutdown. Busy time includes
instrumentation and shared-host scheduling; it is not a dedicated-host latency
guarantee. The host's deferral counter measures postponed wall-clock ticks,
without skipping physics. Birth snapshots are authoritative diagnostics,
separate from bot inputs.

- **C1:** deaths within 3/10 world seconds, by simulator cause, per background
  birth with the complete followup horizon before shutdown. Socket probes leave
  at admission and are excluded from survival denominators. Late bot births
  are right-censored, never counted as survivors.
- **C2:** a birth hits a head-path zone when any newborn body edge is within
  600 units of any living head's projected two-second segment. Minimum head
  distance is from the whole newborn polyline to any other living head.
- **C3:** at a birth, check living newborns aged at most 10 seconds still
  outside radius 2,684. Count the birth once if a prior newborn's radial route
  from its current head to radius 2,684 passes within 200 units of the new
  body. This measures an obstruction opportunity, not actual intention/dodging.
- **C4:** failed searches, socket outcomes and wall waits; bots' first
  request-to-birth wall waits across refusals/retries, with pending requests
  censored separately; living counts each world second; busy ms/tick and
  deferred ticks. A queued bot need not receive a search to count as waiting.

## Results

The two seeds are pooled below; per-run values remain in
[results.json](results.json). Zero observed events are not a safety guarantee.

**C1 — newborn deaths / births with complete followup.** B = body, P = pit;
there were no boundary deaths within these horizons.

| Seats | Current ring, ≤3 s | New band, ≤3 s | Current ring, ≤10 s | New band, ≤10 s |
| --- | ---: | ---: | ---: | ---: |
| 16 | 1/29 (1 B) | 0/32 | 2/29 (2 B) | 1/31 (1 P) |
| 32 | 15/104 (15 B) | 0/57 | 27/100 (24 B, 3 P) | 2/51 (1 B, 1 P) |
| 64 | 34/167 (34 B) | 0/55 | 49/130 (45 B, 4 P) | 1/49 (1 P) |

**C2 — births hitting another head's path zone / all births; minimum
newborn-polyline-to-other-head distance.** Includes admitted socket probes.

| Seats | Current ring: hits / births | New band: hits / births | Minimum distance, current → new |
| --- | ---: | ---: | ---: |
| 16 | 33/96 | 0/98 | 111.5 → 618.5 units |
| 32 | 145/162 | 0/108 | 94.0 → 608.4 units |
| 64 | 187/205 | 0/102 | 80.7 → 621.5 units |

**C3 — inward-route obstruction opportunities / all births.**

| Seats | Current ring | New band |
| --- | ---: | ---: |
| 16 | 1/96 | 0/98 |
| 32 | 15/162 | 0/108 |
| 64 | 80/205 | 0/102 |

**C4 — joins and wall waits.** A/R/C = admitted/refused/censored sockets.
Wait quantiles include admitted lives only. Bot pending counts expose the
right-censored tail; a completed-life median alone understates crowding.
The one censored socket waited 2.624 s before study shutdown, not the 15 s cap.

| Seats / design | Failed searches | Socket A/R/C | Socket wait p50 / p90 / max, s | Bot wait p50 / p90 / max, s | Bot admitted / pending |
| --- | ---: | ---: | ---: | ---: | ---: |
| 16 / current | 3 | 66/0/0 | 0.020 / 0.025 / 0.032 | 0.001 / 0.010 / 1.037 | 30/0 |
| 16 / new | 41 | 66/0/0 | 0.018 / 0.029 / 1.022 | 0.027 / 2.843 / 3.854 | 32/0 |
| 32 / current | 175 | 55/11/0 | 0.049 / 0.061 / 0.101 | 0.021 / 6.832 / 10.437 | 107/0 |
| 32 / new | 316 | 47/0/1 | 0.757 / 2.292 / 3.333 | 8.701 / 28.325 / 39.900 | 61/11 |
| 64 / current | 1,523 | 22/44/0 | 0.058 / 0.077 / 0.458 | 2.657 / 25.561 / 39.085 | 183/33 |
| 64 / new | 316 | 43/0/0 | 0.394 / 2.822 / 6.792 | 6.417 / 33.588 / 38.926 | 59/79 |

At 64 seats, refusing 44/66 probes becomes admitting 43/43 attempted probes,
with fewer attempts because the same single probe waits. At 32 seats, one
request remains pending at shutdown. Searches that fail now create waits,
not refusals; do not compare failed searches as though they were lost players.

Busy ms/tick ranges span the two seeds; deferred ticks sum them, both over
wall seconds 5–40. Living cells give seed 7 / seed 29 at the nearest one-second
diagnostic to the named **world** second. Last samples are at world 39.039 s
except the overloaded current 64-seat runs (31.031 / 30.030 s).

| Seats / design | Busy ms/tick | Deferred ticks | Living at 5 s | 10 s | 20 s | 30 s | Last living |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 16 / current | 0.158–0.210 | 0 | 12/12 | 13/13 | 12/13 | 12/13 | 12/12 |
| 16 / new | 0.153–0.166 | 0 | 12/12 | 13/12 | 13/12 | 12/12 | 12/12 |
| 32 / current | 0.693–0.734 | 0 | 19/19 | 27/25 | 29/28 | 29/28 | 28/28 |
| 32 / new | 0.455–0.530 | 0 | 12/13 | 15/17 | 18/20 | 20/21 | 19/23 |
| 64 / current | 4.107–4.546 | 2,306 | 18/19 | 28/28 | 35/36 | 40/41 | 41/41 |
| 64 / new | 0.411–0.675 | 0 | 12/13 | 17/18 | 16/21 | 17/19 | 17/24 |

Lower load includes the effect of fewer living snakes and admissions. These
numbers do not isolate a behavior-preserving CPU optimization.

Choose the new band and strict waiting: it meets the owner's early-safety and
unobstructed-route criteria in these runs, at an explicit population/wait cost.
It does **not** promise 64 simultaneous lives in this unchanged arena. A wider
band extending inward would place newborns on routes toward the middle;
extending outward would consume the tangent wall runway. Neither is needed to
obtain the observed safety improvement. Future capacity work needs longer runs
and owner agreement on the population versus clearance tradeoff.

## Verification and limits

`uv run --locked python tools/check.py rust` passed on integrated main
`bb9d457`, including format, Clippy feature combinations, default/core-only
tests, doctests, C ABI, Python crate checks and all 11 performance-harness tests.
`python -B -m unittest discover -s tools -p test_documentation.py` passed all
seven tests.

`uv run --locked python tools/perf_compare.py --base origin/main --repeats 1
--no-profiles --output outputs/safe-spawn/perf-compare-delivery` completed with
**exit 3**, the expected behavior-change result: all **eight official/legacy
definitions and full-state traces are equal**, including wire bytes;
`battledome-32-step` and `client-battledome-64` definitions/traces changed.
These changed workloads do not support an output-preserving speedup claim.
Selected trace hashes and verdicts are in `results.json`.

Regressions cover full-body/head-path exclusion (including the newborn tail),
waiting despite positive tight margins, socket-first ordering, waiter
cancellation, prior bot-connection retirement and band geometry/area sampling.
Existing official crowded admission plus exhaustive-versus-bounded admission
tests retain positive fallback, refusal, selected poses and subsequent RNG draws.
The old pit-marker presentation fixture explicitly pins the inner circle so
its food-interest expectation remains independent of the new outer band.

The measured runtime owners match the delivered runtime. `server.rs` differs
only in that later `cfg(test)` presentation-fixture pin; the source-pins file
records the exact comparison. No independent review, outside-server or stock
browser validation is claimed.

The initial baseline cohort lacked request timestamps. A first candidate pilot
stopped near 24 seconds, followed by a probe timeout; a diagnostic repeat
completed. These preliminary runs remain local and are excluded from the final
cohort. The pilot's original host error was masked by the probe panic, so its
precise cause is unknown. The subsequent old-connection correction is protected
by a deterministic regression; it is not presented as a proven pilot diagnosis.

Full local raw snapshots, logs, measurement sources, binaries and unsuccessful
attempts are under `outputs/safe-spawn/` in this task worktree (**local-only**).
The final cohort is in `final/`. Selected results are checked in; raw files and
binaries are not. Two short scheduling-dependent seeker runs per configuration
do not establish long-run capacity, human behavior or guaranteed safety.
Future steering/boost changes, other heads' turns and pit exits remain lethal.
There is no fitted official mechanics claim; held-out fitting is inapplicable.

PR #109 merged as `bb9d457` during this task and was integrated from main.
Its `client-battledome-64` definition changes with this PR. The new workload's
tests now accept the circular wait result and each body's own radius; its
per-slot harness retry policy remains distinct from the host queue. The direct
physics workload still uses its declared contact clearance, not host admission.
PR #109's branch was not edited.
