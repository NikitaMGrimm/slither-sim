# Battledome food balance, 2026-10-25

Synthetic offline design experiment for our own mode. No official or
community-server data, game-server connections, or fitted server claims.
The [plan and amendments](PLAN.md) retain the method changes and failed attempts.

## Method

Compare baseline `421e384c3738e7c4d08e1eb12275733266d7a629` (pristine
`origin/main` at study start) with candidate
`a1fccfda3905e7a89295a9fbfa97d02cf84cde81`. The candidate also includes main's
test-only #101; the measured source delta is exactly these two food rules.
Main's presentation-only #102 was integrated after measurement; it leaves
the core simulator, policy and study harness unchanged. Use seeds 1, 2, 3,
32 and 64 requested slots, 600.012 simulated seconds per world, default 7 ms
physics and six-tick decisions. The unchanged `examples/cautious_bot.rs` policy
controls all slots through direct Rust observations. It avoids snakes/walls,
seeks food and never boosts; boost food is therefore zero throughout.
The Rust harness is [measure.rs](measure.rs); [run.py](run.py) builds an isolated
offline wrapper crate against the selected checkout, copying the repository's
lock before resolving the wrapper. Release build, networking/parallel disabled.
Process affinity restricts every build/run to two logical CPUs, with two Cargo
jobs and a 570-second timeout per child. Runs are sequential within each phase.
Compiler: Rust 1.99.0 (`b940084d7`, 2026-09-28), Windows x86-64.

Initial lives use `Simulator::add`'s deterministic ring. This causes an initial
collision burst (long newborn bodies overlap). Later lives draw up to 64 random
ring bodies, accepting the first contact-clear pose and retrying failure one
second later. A separate seeded LCG drives these draws. This cheaper synthetic
controller replaces the host's preferred 600-unit/best-margin search. Requested
slots and living population are distinct; results retain both.

Samples every ten simulated seconds record pellet counts and raw nutrition
(`Food.value`) by ambient/death/boost origin, living continuous mass
(`segments + fam`), displayed-score mean/median/max, cumulative deaths and
observed lives growing beyond 400. Raw food nutrition is before the eater's
mass-dependent multiplier; it is not displayed score. Late growth counts lives
first observed alive after 300 seconds, with end-of-run censoring; lives that
die within their first decision interval are not observed in that denominator.
No seeds are excluded and no held-out validation is claimed.

## Design choices

With a configured floor, let `M = segments + fam`, `F` be the converted floor,
and `K = 2 * body.len() + 1` the ordinary candidate count. The pellet budget is
`K * max(0, M-F) / M`: select full-size candidates where cumulative credit
crosses an integer in tail-first emission order, then put any remainder at the
head as a pellet of size `normal_size * sqrt(fraction)`. Thus uncapped raw nutrition is
exactly the above-floor fraction of the ordinary drop, including sub-pellet
growth. Scatter and admission remain; rejected shares are discarded. This is
a design mapping of the existing payout, not a claim of exact mass conservation
or an inverse of the nonlinear score table. A floor-size snake drops nothing.
Worlds without a floor retain the original death-food loop and random draws.
The recorded runs below used an earlier tail-prefix selection. Even spacing
keeps the pellet count, uncapped nutrition and refill rate, so the population
study was not rerun; placement can still change pickup and admission, so these
measurements stay pinned to the original candidate.

`ambientRefillCountsAllFood` is independent of the floor: death/boost food can
fill a refill target in any custom ecology, including worlds without free spawn
mass. Only Battledome enables it by default. It pauses refill at the total
pellet target, preserving placement rates, clocks and admission; it does not
delete excess food or cap deaths/boosts. Official/sandbox defaults and serialized
settings remain unchanged (the false field is omitted).

## Results

Values below are means across all three seeds at 600.012 seconds. Food and
mass are world totals; scores include living snakes only. Maximum means the
mean of each world's endpoint maximum, not the largest score ever reached.
Boost pellets and boost nutrition are zero in every sample.

| Slots / phase | Food (ambient / death) | Raw food nutrition | Snake mass | Score mean / median / max | Deaths/min (last 5 min) |
| --- | ---: | ---: | ---: | ---: | ---: |
| 32 before | 4,424 (1,606 / 2,818) | 1,948.0 | 1,898.5 | 1,101 / 623 / 7,237 | 48.4 (47.6) |
| 32 after | 1,748 (1,332 / 416) | 419.3 | 1,519.7 | 887 / 503 / 4,622 | 34.6 (32.0) |
| 64 before | 3,801 (771 / 3,030) | 1,988.1 | 3,469.6 | 1,047 / 614 / 8,004 | 99.9 (102.2) |
| 64 after | 943 (514 / 429) | 322.7 | 2,363.9 | 605 / 454 / 3,243 | 71.5 (67.3) |

| Slots / phase | Endpoint food range | Endpoint mean-score range | Mean score at 5 min -> 10 min | Late living mean | Late lives growing above 400 |
| --- | ---: | ---: | ---: | ---: | ---: |
| 32 before | 4,327–4,492 | 1,054–1,169 | 1,155 -> 1,101 | 31.98 | 713/714 (99.9%) |
| 32 after | 1,702–1,776 | 816–984 | 800 -> 887 | 31.97 | 476/479 (99.4%) |
| 64 before | 3,350–4,107 | 1,029–1,072 | 1,054 -> 1,047 | 61.20 | 1,518/1,533 (99.0%) |
| 64 after | 730–1,143 | 578–621 | 654 -> 605 | 63.83 | 996/1,010 (98.6%) |

Growth is reasonable for this ten-minute, non-boosting policy: lower food
inventory, mass, scores and death rates, with continued newborn growth at both
populations. It is not proof of equilibrium: 32-slot after mean score rises
800 -> 887 in the second half. The baseline itself does not show sustained
mean-score acceleration in the second half, so it does not reproduce every
reported symptom of rapid player growth. The joint change reduces endpoint
pellets by 60.5%/75.2% and mean scores by 19.4%/42.2% at 32/64 slots.

These data do not justify a third rule or show food starvation. Next, run a
longer matched trial with boosting and pit-aware policies before considering
death-food decay. No extra balance rule is added here.

[samples.csv](samples.csv) retains all 732 selected samples (61 per world),
including seed-level scores, origin nutrition and death counts.
[results.json](results.json) records seed ranges, exact aggregates, build/harness
identities and source/series hashes. All twelve runs exited 0; the longest
baseline/candidate runs took 469/264 wall seconds. Shared-machine wall times
are operational bounds, not a performance claim.

## Verification

`uv run --locked python tools/perf_compare.py --base origin/main --repeats 1
--no-profiles` (with an external `--output` and 570-second sampling deadline)
exited 0 against study-start main: all ten CI-suite full-state traces and
workload definitions are equal.
These are non-floor worlds, including the dome proxy, not the changed preset.
Timing ratios from this shared, CPU-limited machine are not used as performance
evidence. Comparison artifacts remain local-only under `parity/`.
After integrating #102, the same command against main
`d790cdc9fd86829b862d8f0675b34926f9bfbf37` also exited 0 with all ten traces
and definitions equal (`parity-final/`).

`uv run --locked python tools/check.py rust` passed all 12 commands (format,
four Clippy configurations, default/core tests and doctests, C ABI tests,
Python-crate build and basic example). The focused Battledome, ambient-food
and death-food regressions passed (51 tests). Documentation tests also passed.

## Reproduction

Run from the repository root, with new output directories outside the checkout:

```sh
python research/battledome/2026-10-25-food-balance/run.py --repo BASELINE_CHECKOUT --output BEFORE
python research/battledome/2026-10-25-food-balance/run.py --repo . --output AFTER
python research/battledome/2026-10-25-food-balance/summarize.py BEFORE AFTER NEW_SELECTED_RESULTS
```

The full build logs, wrapper locks, executables, failed attempts and individual
CSVs are local-only at
`C:/Users/Nikita/.t3/scratch/battledome-food-balance-20261025/`.
The successful phases are `before-cautious02` and `after`.
Selected time series and identity/result summaries accompany this report.

## Limits and retained failures

- The first seeker wrapper build failed on a harness field-name typo
  (`DeathEvent.snake`, corrected to `victim`). The rebuilt seeker-backend attempt
  was stopped after 85 wall seconds without its first ten-second sample.
- The first direct cautious run with full best-margin spawn search was also
  stopped before its first ten-second sample. A separate 1.008-second probe
  completed, showing only 12 living snakes after the initial burst. Their logs,
  copied harnesses and partial outputs remain in `before`, `before02`,
  `before-cautious` and `probe-1s.csv`; they are not pooled with usable results.
- Three seeds, one non-boosting policy, ten minutes and synthetic respawns do not
  establish long-run balance, player behavior, boost-heavy balance or live host
  performance. The paired trajectories diverge after the changed rules. This
  joint intervention has no ablation separating the two rules' contributions.
- No food decay is introduced. These two rules do not guarantee a hard pellet
  cap or a conserved ecosystem; conclusions apply to these sampled worlds.
