# Pit-aware shared A4 — 2026-10-27

Local background bots now head into Battledome's pit instead of foraging the
spawn band. From inside, food goals, steering clearance and escape forecasts
treat the live pit edge as lethal. This is mode knowledge supplied to the same
A4 kernel, not a second policy. Pit-less decision bits are unchanged.

## Method and scope

Synthetic local-simulator evidence; no external server or player session.
Three seeds (7/29/47), 64 bot seats plus one empty reserved socket, 180 world
seconds per arm. Adjacent arm order: before/after, after/before, before/after.
One shared A4 instance drives every seat from its delivered protocol frame,
using the real host Simulation/Seat command, admission, delivery and rejoin
paths. Zero added latency, responsive delivery, 42 ms decisions, 7 ms physics.
The fixture drives these paths in one deterministic virtual schedule; it does
not open sockets, exercise worker scheduling or qualify real-time capacity.
Main's arena/queue controller is identical in both arms for this first PR.

Release/thin LTO, locked dependencies, Rust 1.99.0, Windows; every child mask
checked as 0x30 (two logical CPUs). CARGO_BUILD_JOBS=1, RAYON_NUM_THREADS=2,
RUST_TEST_THREADS=2. Sequential runs, no overlap with builds/checks. Full source,
binaries, fixture, plan and outputs are local-only under outputs/pit-redesign;
the selected hashes are in source-pins.json. No personal/machine identifiers
are needed to reproduce the configuration.

Inside share is living-snake-time weighted across every physics tick. Pit
deaths/minute and kills/minute use elapsed world time; kills here are credited
body deaths, not total deaths. Waits are request-to-admission world seconds,
completed requests only. The mode behavior changes, so trajectory parity is
neither expected nor claimed. Conformance tests separately check pit-less parity.

## Results

| Seed | Policy | Inside share | Pit deaths/min | Kills/min | Births | Wait p95 / max, s |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 7 | Baseline | 17.6% | 18.67 | 1.00 | 123 | 1.113 / 1.813 |
| 7 | Pit-aware | 97.3% | 0.00 | 0.67 | 66 | 1.358 / 4.599 |
| 29 | Baseline | 18.1% | 23.33 | 0.00 | 134 | 0.987 / 1.925 |
| 29 | Pit-aware | 87.2% | 0.00 | 6.00 | 82 | 1.155 / 1.785 |
| 47 | Baseline | 15.3% | 21.00 | 0.33 | 128 | 1.127 / 1.897 |
| 47 | Pit-aware | 95.9% | 0.00 | 0.67 | 66 | 4.816 / 4.837 |

All final requests were admitted. Early-death and strict-intrusion counts are
retained per run in results.json; these cohorts are not the arena-redesign
safety qualification. Censored late lives and bots do not establish human safety.

Inside population at approximately the indicated world time, pooled across the
three seeds (the full 10-second count series is in results.json):

| World seconds | Policy | Inside / living | Share |
| --- | --- | ---: | ---: |
| 10 | Baseline | 0 / 192 | 0.0% |
| 10 | Pit-aware | 123 / 192 | 64.1% |
| 30 | Baseline | 12 / 192 | 6.2% |
| 30 | Pit-aware | 187 / 192 | 97.4% |
| 60 | Baseline | 33 / 192 | 17.2% |
| 60 | Pit-aware | 190 / 192 | 99.0% |
| 120 | Baseline | 44 / 192 | 22.9% |
| 120 | Pit-aware | 189 / 192 | 98.4% |
| 170 | Baseline | 34 / 190 | 17.9% |
| 170 | Pit-aware | 191 / 192 | 99.5% |

## Decision and limits

Adopt configured pit knowledge for local host and native background bots.
ForagerBot::with_world_rules retains reference mode geometry, scales the pit
from each frame's received live wall, and keeps fixed pits fixed. ClientFrame
is unchanged; external bots still need separately supplied mode rules.
Forager::decide_with_pit accepts an explicit live radius for offline callers.

No A4 parameters, arena constants, allowances or physics changed here. Outside,
the existing steering fan avoids opponents on the way in. Inside, hard pit
head-center exit tests supplement the ordinary outer-wall check; classification
includes motion during the timing lead. Effective geometry is a scalar carried
with the forecast, without copying an observation or storing per-seat history.

This does not prove collision immunity, human play or the right arena size.
Main's excessive arena growth/queue allowances remain for the separate redesign;
some main-policy seeds can still trigger long moving-wall food reconciliation.
No load or capacity improvement is claimed by this study. Changing where bots
play changes their opponents, kill opportunities and survival, not just pit deaths.

## Attempts and verification

An initial fixture omitted the startup Seat::join call, admitted zero bots and
failed its empty quantile calculation. Its binary/log are retained; no result
is used. The v2 pilot completed six 180-second runs: baseline inside share
15.3–18.1%, pit deaths 56–70; pit-aware share 87.0–97.0%, pit deaths zero.
It was superseded because source review found sliding escape forecasts and
per-decision observation copies. Its six raw result digests, concatenated in
seed/arm order, identify the cohort with SHA-256 c3c038181125ea00f7391349895e0949ff5d26ce7183d2569f94628b609c8125.
Further source review caught lead-time entry classification and lost outer-wall
checks in custom worlds. Final data above follow all corrections.

Regressions cover entry/live scaling, fixed/pit-less modes, shrink/outside-food
steering, tangent hard-edge lethality versus sliding, lead-time entry, a smaller
outer wall, and allocation-free warm decisions. Full Rust check group and docs
checks passed; final release conformance/regression results are recorded in the PR.
Independent review was source-only; the reviewer did not run these measurements.
No checkpoint shape or core tick rule changed, so this PR needs no revision bump.
