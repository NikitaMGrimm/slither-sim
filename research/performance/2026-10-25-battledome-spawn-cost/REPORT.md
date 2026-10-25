# Battledome admission cost, 2026-10-25

Exploratory **local-simulator** study started October 25 and completed across
midnight October 26 on a shared Windows desktop. No outside
game servers. Primary baseline: `30064ec5864cd34273e81981d88f60cf4f66e9b0`
(main, including pit markers, food balance and responsive delivery). An initial
cohort at `5f4b4b48d4546932a0c7a0e22b8e89718369fad1` remains separate.
Selected results and source/build hashes accompany this report.

## Cause and change

Hosted admission prefers the first of 64 sampled newborn bodies clearing 600
units. Otherwise it chooses the greatest positive contact margin. A 26-anchor
Battledome newborn requires 25 candidate edges against every living head/body
edge for each full margin. A refused bot repeats this at its next decision.
The host's retry path is `Seat::decide` → `RawSimulator::connect` →
`SimulatorBackend::spawn_pose`, not the virtual backend's `drive_background`.

The old margin scorer scans every edge even after a candidate has lost to an
earlier candidate. The bounded scorer stops as soon as its running minimum is
at most the greater of zero and the incumbent. A minimum can only decrease:
a stopped candidate cannot pass the positive admission threshold or win the
strict `>` comparison. Winning margins retain the same arithmetic and
traversal order. Geometry validation, first-clear preference, candidate draws,
ties, contact/wall rules and refusal errors are unchanged; public
`Simulator::spawn_margin` still computes the exact margin.

An initial `5f4b4b4` incumbent-only pruning attempt was insufficient: 64 seats still
averaged 35.30 ms/tick, with 4,019 deferrals during seconds 5–40 and no successful
probe (24 explicit refusals and three timeouts). It remains in the results.
The final implementation additionally prunes against zero and applies a **host
policy**: unadmitted background bots wait one wall second after refusal before
their next decision. Successful lives retain normal death/rejoin cadence;
socket players retain their own join timing. The native virtual backend's retry
policy is unchanged. Pacing changes real-time host admission order by design;
the scorer preserves the choice for an identical world and RNG state.

## Method

Rust release/thin LTO, Windows MSVC, Ryzen 7 9800X3D. Each process tree uses
affinity `0x30` (two logical CPUs), `CARGO_BUILD_JOBS=2`, test/Rayon threads at
most two. Hosts run sequentially for 40 wall seconds: seed 7, four socket seats,
12/28/60 Battledome bots or 496 official bots, one bot worker, zero added latency,
unchanged presets. Compare seconds 5–40; retain startup and five-second windows.
Construction and initial native admission precede the timed host run. The
primary Battledome JSON preset defaults to responsive delivery; the initial
cohort used official delivery and predates the food-balance fix. They are not
pooled. After integrating main, one candidate also read the wall clock for every
bot delivery; the final candidate reads it only on closure. Both are retained.
The final 500-seat baseline/candidate pair ran adjacently after the other final
seat counts, without a build between them.

A separate loopback probe attempts one join per wall second from second 5,
disconnecting once its decoded state is `Playing`. Refusal, socket timeout and
decode errors are retained separately. Heavy admission can reduce the number
of attempted probes; denominators are actual attempts. No admitted probe stays
to fill all four reserved seats. These are configured-seat load tests, not
64 simultaneous living snakes.

Temporary instrumentation times entire `spawn_pose` calls and individual margin
calls; five-second reports include attempts, failures and living counts. Timings
include instrumentation and shared-host scheduling. Simulation busy time comes
from `RunningHost::simulation_seconds`; divide its window increase by executed
ticks. Deferrals count postponed wall-clock ticks, with no skipped physics.
Counter reports occur after a batch and may straddle nominal window boundaries.

At stop, retain the authoritative world separately from bot inputs. Offline
analysis samples 720 equally spaced ring points and both tangent newborn
directions (1,440 poses), using exact public clearance/margin methods. Point
occupancy and whole-body admission are different measures. This finite grid
cannot prove the absence of every possible continuous pose.

## Results

Means below use `RunningHost` counters at approximately seconds 5 and 40.
Deferrals exclude the initial five seconds; total-run deferrals are in
parentheses. Socket outcomes cover all attempted probes after second 5.
See [results.json](results.json) for every window and the unsuccessful
incumbent-only and intermediate variants;
[source-pins.json](source-pins.json) identifies artifacts.

| Preset / configured seats | Before → final busy ms/tick | Before → final deferred ticks, seconds 5–40 (total) | Before → final socket outcomes |
| --- | ---: | ---: | --- |
| Battledome / 16 | 0.145 → 0.127 | 0 → 0 (0 → 0) | 0/34 → 0/34 refused |
| Battledome / 32 | 1.145 → 0.437 | 0 → 0 (97 → 0) | 5/34 (14.7%) → 3/34 (8.8%) refused |
| Battledome / 64 | 45.449 → 3.473 | 4,174 → 833 (4,606 → 890) | 34/34 (100%) → 25/34 (73.5%) refused |
| Official / 500, adjacent repeat | 2.886 → 2.888 | 0 → 0 (0 → 0) | 0/34 → 0/34 refused |

At 64 seats, admission accounts for **99.79%** of baseline instrumented busy
time after the first report; margin scans alone account for **85.17%**.
The final shares are 94.95% and 74.67%. There were 1,531 baseline admissions
(1,515 failed, 97,984 margin calls) versus 542 final admissions (464 failed,
34,688 margin calls) in these profile windows. This includes bots and probes;
profile reports can end at second 35 when stop wins the final reporting race.
Five-second living counts rose to 47 in the final 64-seat run, versus 26 before;
the terminal final world had 45. The final nine probes, at seconds 30–38, were
all refused. The host still has stalls and substantial ring contention;
mean busy time below 7 ms does not establish on-time service.

The first integrated 500-seat pair was 3.472 → 3.934 ms/tick, both with zero
deferrals. Removing the avoidable per-delivery clock read preceded the adjacent
repeat above; these data do not isolate its cost or establish a speedup. The
final pair differs by about 0.09%, with unchanged admission and deferral outcomes.
The older `5f4b4b4` cohort had 39.254 → 5.829 ms/tick at 64 seats, but also a
slower 16-seat final mean (0.512 → 1.280), mostly outside admission, and a
9.446-ms official baseline. Those mixed shared-desktop results remain selected,
not discarded. No run establishes a dedicated-host latency guarantee.

## Capacity and owner decisions

The 2,684-unit ring has circumference 16,864.1 units. Each minimum-size newborn
has 25 × 42 = 1,050 units of body chords (about 1,050.01 units of arc), before
clearance. Sixteen newborn bodies alone occupy about one circumference; 64
would require 67,200 units, nearly four circumferences. Existing snakes can
leave the ring, so this is a simultaneous-on-ring constraint, not a global
population cap. The annulus between the 1,784-unit pit and 3,584-unit wall is
about 30.355 million square units; the full arena is about 40.353 million.

The 26-segment contact exclusion is 75.566 units (collision distance plus 40).
An idealized unmoved ring of minimum-size newborns holds at most **14** with
that separation, or **10** with the preferred 600-unit gap. This geometric
calculation assumes disjoint ring arcs and uniform size; moving snakes can
leave the ring, so neither number is a steady-state population limit.

Terminal authoritative worlds, sampled offline:

| Variant / seats | Living | Actual head/body polyline length, units | Ring points inside contact exclusion / 600 units | Admissible full newborn poses / 1,440 |
| --- | ---: | ---: | ---: | ---: |
| Before / 64 | 26 | 18,901 | 419/720 (58.2%) / 720/720 | 0 |
| Final / 16 | 12 | 6,233 | 73/720 (10.1%) / 547/720 | 918 (63.8%) |
| Final / 32 | 28 | 16,632 | 60/720 (8.3%) / 706/720 | 713 (49.5%) |
| Final / 64 | 45 | 28,651 | 324/720 (45.0%) / 720/720 | 30 (2.1%) |

Actual lengths sum distances between stored successive points, including the
head, rather than `Snake::length` (continuous segment mass). The final 64-seat
body length is 1.70 ring circumferences across a 30.355-million-unit²
annulus. The best sampled **whole-body** pose has positive margin 93.333,
but only 2.1% of sampled poses fit.
No sampled pose clears 600 units in either final 32/64-seat world. Many free
points do not make a free 1,050-unit newborn corridor. This establishes frequent
ring-admission congestion at 64 configured seats, not a packed whole arena or
a universal population cap. Population was still rising during the 40-second
run; a longer mature-world check would be needed to identify steady capacity.

Owner options, **not implemented**:

- Keep the ring and 400-point floor, with fewer background bots: 12 bots / 16
  seats had no probe refusal; 28 bots / 32 seats had 3/34. Those are samples,
  not guaranteed admission limits.
- Retain 64 configured seats and queue or stagger demand until a full newborn
  corridor opens. This preserves spawn geometry but trades waiting for refusal;
  the observed maximum was 47 living, so this run cannot promise 64 living.
  A longer failed-bot retry delay can reduce CPU further, with slower refill.
- If 64 concurrent lives are required, decide whether to broaden spawn geometry,
  enlarge the arena, or change newborn size. Sixty-four newborns alone need
  67,200 chord units against a 16,864-unit ring. Changing ring radius alone
  inside the existing wall cannot provide four times its circumference.

No spawn radius, size floor, pit, arena, clearance or attempt budget is changed
in this PR.

## Verification and limits

Synthetic regressions compare exact selected bodies/headings, refusal errors
and subsequent spawn RNG draws with the exhaustive admission algorithm across
empty, crowded-admissible and blocked ring scenes. Bounds are checked at the
exact margin (ties), either side of zero, infinities and invalid geometry.

`uv run --locked python tools/perf_compare.py --base origin/main --repeats 1
--no-profiles --output outputs/spawn-cost/perf-compare` exited **0** against
`30064ec`: all ten workload definitions and full-state traces equal, including
wire bytes. Its snapshot's four changed Rust source files match the final
runtime/test files byte for byte. These deterministic workloads do not assert
identical real-time host admission order after the deliberate retry-policy change.
The final branch also includes `f11c830`, a documentation/research-only main
update; runtime source is unchanged from the measured `30064ec` comparison.

Final `uv run --locked python tools/check.py rust` passed, including format,
Clippy feature combinations, default/core-only tests, doctests, C ABI and Python
crate checks. Final `python -B -m unittest discover -s tools -p
test_documentation.py` passed all seven tests. Both groups also passed before
main integration. No independent review or external-server validation is claimed.

Full measurement sources, executables, stdout/profile JSONL, terminal worlds,
plan and failed attempts live under `outputs/spawn-cost/` in the task worktree:
**local-only**, not downloadable through this path. Selected data and hashes
are checked in; no raw recordings or binaries are committed. This is one seed,
one short run per configuration/variant, with scheduling-dependent controllers
and probe timing. It does not establish long-run capacity, bot quality,
human-rendered smoothness, official rules or statistical equivalence.
