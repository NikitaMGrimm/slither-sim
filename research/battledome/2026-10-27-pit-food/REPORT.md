# Pit-only food: owner playtest and short supply check

Battledome's `foodOnlyInsidePit` setting restricts physical ambient, death and
boost pellets to the live pit. It uses the ordinary full-body death budget,
including floor-size snakes, then discards candidates outside the edge. Ambient
target/refill scale by `(pit / wall)²`; all food origins count toward refill.
These are owner-directed mode choices, not fitted official-server rules.

## Owner-side local verification

The owner approved the feel of `720301c` plus main. Their local packet check
reported 0 of approximately 900 pellets outside the pit and edge deaths dropping
only their inside portion. This is owner-reported local verification, not an
independent repetition by this agent; the raw packet archive is not retained
here. Decorative edge markers remain presentation-only and are not physical food.

## Supply-check method

The [frozen plan](PLAN.md) replaces the previously requested twelve ten-minute
runs with one seed (7), 32 and 64 bot seats, 300 world seconds on each side.
Main is `5eb0736ee3650687adff5b5c431c33994bf75bb9`; candidate is
`010b2d48a2b8387745b232b1faee8b4984f78692`, the approved food code merged with
main's live-rule delivery and bot names. The arena controller and configuration
are unchanged. Tick/wall/pit geometry matches at all 301 sample points in each
matched pair.

[The fixture](fixture.rs) drives the real host Simulation/Seat admission, shared
A4, decoded live rules, actions and rejoin paths in one virtual schedule. There
is one empty reserved socket seat, no socket or server connection, responsive
delivery, zero added latency, 42 ms bot decisions and 7 ms physics. Boosting is
allowed. Food counters use authoritative state solely for diagnostics, never
for bot decisions. This synthetic schedule does not test real socket/worker
scheduling or hosting capacity.

Exact global food-count extrema are collected every physics tick; count,
nutrition and score series are sampled once per world second. Nutrition is raw
`Food.value`, not displayed score. Late ranges are sampled values from 240 s to
the endpoint. The target is the live ambient refill threshold, not a death/boost
cap. Source, fixture, instrumented-source and binary hashes accompany results.

Release/thin LTO, networking-only build, locked dependencies, Rust 1.99.0
(`b940084d7`, 2026-09-28), Windows. The runner restricts itself and inherited
build/run processes to two available logical CPUs, with one Cargo job and one
test/Rayon thread. No CPU timing or performance claim is made.
Collection finished before the owner's later one-core limit. Subsequent checks
used one-core affinity; the four selected games were already sequential.

## Observed supply

All selected runs used seed 7, 42,858 ticks and 300.006 world seconds. All ended
with the requested number of living bots and no pending admissions. Peak food
is the exact maximum over physics ticks; late ranges are one-second samples.

| Bots | Version | Peak food | Food at 300 s | Target at 300 s | Last-minute food range |
| --- | --- | ---: | ---: | ---: | ---: |
| 32 | Main | 3,466 | 3,244 | 3,138 | 3,134–3,456 |
| 32 | Pit food | 1,457 | 648 | 777 | 475–1,405 |
| 64 | Main | 64,006 | 29,540 | 29,540 | 29,540–37,579 |
| 64 | Pit food | 12,752 | 4,544 | 7,319 | 4,544–7,506 |

No runaway food-count growth was observed in this window: candidate peaks and
endpoints are below main at both populations. This is a statement about these
runs, not a hard inventory cap. At 32 seats, deaths produce temporary inventory
above the 777-pellet refill threshold. The last-minute mean is 877 versus 697
in the previous minute, while the endpoint is 648; bursts remain visible rather
than being hidden by an average. Main's corresponding means are 3,202 and 3,154.

At 64 seats, both arms temporarily grow to a sampled wall radius of 17,910.48
under main's existing spawn-queue allowance, then shrink to 10,996.40 at the
endpoint. Large food peaks therefore include this transient area growth; the
arena has not settled by 300 s. Candidate last-minute mean food falls from
8,871 in the previous minute to 5,894; main falls from 42,078 to 33,487.
This is not a qualification of the separate arena redesign.

Candidate physical food is outside the pit in zero of 301 sampled snapshots
at each population. Endpoint raw nutrition and growth are retained below;
nutrition units are not displayed score.

| Bots | Version | Food nutrition inside / outside | Living score mean / median / max | Credited kills (count / per min) | Deaths with food / deaths |
| --- | --- | ---: | ---: | ---: | ---: |
| 32 | Main | 59.9 / 397.2 | 622 / 488.5 / 1,309 | 93 / 18.60 | 93 / 93 |
| 32 | Pit food | 285.1 / 0 | 1,510.6 / 903.5 / 6,505 | 120 / 24.00 | 122 / 122 |
| 64 | Main | 115.8 / 3,726.3 | 1,464.6 / 1,483.5 / 2,635 | 7 / 1.40 | 8 / 8 |
| 64 | Pit food | 475.6 / 0 | 1,839.0 / 1,830 / 3,154 | 5 / 1.00 | 5 / 5 |

Full death food increases growth in these runs, especially at 32 seats; lower
global pellet counts do not establish mass or score equilibrium. All recorded
deaths happened to drop food, including main's earned-mass deaths; this cohort
does not isolate floor-only deaths. Synthetic regressions and the owner's edge
check cover that rule directly. No extra balance change is justified by this
short check alone, and none was added.

## Attempts

The two-second main-32 pilot completed with 16 admitted bots and 16 pending,
confirming the fixture reaches real admission/delivery before the selected
runs. All four selected runs then completed on their first attempt, exit 0.
[results.json](results.json) includes all five outcomes and source pins;
[samples.csv](samples.csv) retains all 1,204 selected snapshots. No seed or
failed gameplay attempt was excluded.

## Reproduction and retained evidence

Run from a checkout with a fresh output directory (runner supports Windows and
Linux affinity APIs). Under a one-core budget, launch it from a parent already
restricted to one CPU; the frozen runner honors a narrower inherited mask.

```sh
python research/battledome/2026-10-27-pit-food/run.py --main 5eb0736 --candidate 010b2d4 --output outputs/pit-food-sanity-new
python research/battledome/2026-10-27-pit-food/summarize.py outputs/pit-food-sanity-new outputs/pit-food-sanity-new/selected
```

The runner injects a read-only accessor and ignored test into archived source
copies only; the working checkout's runtime sources are unchanged. Full source
copies, binaries and stdout/stderr are local-only under
`outputs/pit-food-sanity-20261027-v1`, not downloadable from this repository.
Selected results include the pilot and all selected attempt outcomes. No
outside game server is contacted.

## Code verification

The focused pit-food, Battledome, ambient, checkpoint and world-rules suites
passed (84 cases), followed by all 15 steps of `python tools/check.py rust`.
Main's later test-only #186 was then merged: runtime `src/` still matches the
measured candidate. Both configurations of `tests/client_network.rs` passed
(6 each), as did all 104 headless-tool tests, formatting and 7 documentation
tests. These follow-up checks ran on one core. No gameplay source changed after
the owner-approved build plus main, and no additional balance rule was added.

## Limits and decision

This is a short supply sanity check, not the original three-seed ten-minute
study or proof of equilibrium across seeds, player behavior or longer sessions.
The owner approved the gameplay. No additional balance rule is introduced.
Above-floor boost mass spent outside the pit is lost, a choice the owner can
revisit. Moving edges remove stranded food without relocating it; outer-wall
deaths still drop no food. Disabled worlds retain their previous food behavior.
