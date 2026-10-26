# Cadence residual, 2026-10-26

**No host change: the remaining cadence gap comes mostly from our test workload.**
Our bots almost never boost, so they generate far less mass, body and food
traffic than official players, and their motion reports bunch onto fewer tick
phases. Substituting official-like traffic recovers **11.1–12.9 percentage
points** of the ≤9 ms share, and spreading report phases recovers
**3.6–8.2 points**. The effects overlap and measure explanatory capacity, not
causal shares. Our bots already send more motion reports per snake than
official players, and the host shows almost no catch-up or deferral signature.

This follows the [sparse-view study](../2026-10-26-sparse-view-cadence/REPORT.md), using its twelve primary archives. In the 6–10-peer bin the official pool is **83.43%** ≤9 ms, versus local **67.99/68.05/64.00%** for r1/r2/r3: deficits **15.44/15.39/19.44 points**. Frames, windows and snakes are correlated; each acquisition remains separate.

## Ranked hypotheses

| Rank / hypothesis | Evidence for | Against / unknown | Estimated share |
| --- | --- | --- | --- |
| 1. Workload/content mix | Far less high-speed movement, mass, anchors and boost/death food; substitution recovers most of the gap. | Food ecology and mass policy confound behavior. | 11.1–12.9 points capacity: ~72–80% of r1/r2 deficits; ~66% of r3. |
| 2. Motion-report phases | Local six-phase concentration is greater; shifting the same reports improves occupancy. | Synchronized joins unproven; shifts can break physical/gate dependencies. | 3.6–8.2 points capacity: ~23–49% of r1/r2; ~42% of r3. |
| 3. Value suppression / gates | Official repeats unchanged tuples and has short intervals. | Local motion rate is higher; deleting repeats barely changes cadence. | Repeats 0.4–0.6 points (~3–4%); tolerant gates still 0.4–0.6 total; strict gates 1.7–3.1, clock-sensitive. |
| 4. Exact peer mixture | r1/r3 favor the low end of 6–10. | Residual survives exact-count matching; r2 has no ten-peer exposure. | Full-bin standardization adds 1.04/2.31 points to r1/r3; r2 unidentified. |
| 5. Execution / own / ping | Host can defer/catch up; rare receipt anomalies exist. | Almost no burst pairs; pongs/s match; own packets help local more. | Local receipt/header ≤9 difference 0.03–0.10 points; hidden deferral contribution unknown. |

## Clock and histogram

One nominal tick is **7 ms** (`src/physics.rs`, `LOOP_SECONDS`). All local headers are exact multiples of seven; the official one-tick lobe averages **7.019–7.041 ms** per capture, without identifying its internal tick. `Simulation::{run,due_ticks,defer,world_time}` in `src/client/server.rs` schedule from wall time, permit eight catch-up ticks per wake and defer backlog. Headers alone cannot expose deferral; receipts provide a separate check. All nine medians are 7 ms. Bands below are disjoint, in milliseconds; a burst pair has adjacent receipt gaps ≤2 and ≥12, in either order.

| Capture | Gaps / seconds | 0–2% | ~1 tick: 3–9% | ~2: 10–17% | ≥3: ≥18% | Receipt ≤9% | Burst pairs% |
| --- | --- | --- | --- | --- | --- | --- | --- |
| official-low-1 | 40,196 / 334.9 | 0.00 | 84.72 | 12.40 | 2.88 | 83.85 | 0.010 |
| official-second-1 | 31,596 / 265.6 | 0.00 | 84.12 | 12.77 | 3.11 | 82.89 | 0.006 |
| official-low-2 | 29,526 / 255.0 | 0.00 | 82.15 | 13.81 | 4.03 | 81.28 | 0.014 |
| official-second-2 | 25,280 / 218.1 | 0.00 | 81.90 | 14.48 | 3.62 | 80.82 | 0.012 |
| official-low-3 | 38,184 / 328.8 | 0.00 | 82.42 | 13.79 | 3.78 | 81.26 | 0.024 |
| official-second-3 | 30,835 / 257.6 | 0.00 | 84.73 | 12.38 | 2.89 | 83.38 | 0.039 |
| bots-500-r1-retry2 | 17,147 / 169.1 | 0.00 | 67.99 | 25.53 | 6.49 | 67.89 | 0.006 |
| bots-500-r2 | 4,425 / 43.2 | 0.00 | 68.05 | 26.62 | 5.33 | 67.98 | 0.000 |
| bots-500-r3 | 13,071 / 137.4 | 0.00 | 64.00 | 25.21 | 10.79 | 63.97 | 0.000 |

## Per-snake reporting

Rates divide by cached other-snake seconds. Intervals require continuous cache membership and an active observer throughout one uninterrupted 6–10 episode; entry/exit/bin crossings censor long intervals. Values are median/p95 ms. `h` is standalone mass; any mass also includes growth anchors and mass-bearing tail removals. Denominators and bounded quantiles are in [per-capture results](results.json).

| Capture | Motion ms | h mass ms | Any mass ms | Anchor ms | Motion / h per snake·s |
| --- | --- | --- | --- | --- | --- |
| official-low-1 | 43.0/202 | 107.0/572 | 106.0/468 | 112.0/239 | 15.00 / 4.63 |
| official-second-1 | 43.0/193 | 107.0/600 | 106.0/498 | 134.0/240 | 15.13 / 4.53 |
| official-low-2 | 43.0/199 | 107.0/612 | 106.0/523 | 128.0/240 | 14.40 / 4.33 |
| official-second-2 | 43.0/204 | 107.0/584 | 106.0/501 | 128.0/240 | 14.79 / 4.39 |
| official-low-3 | 43.0/204 | 107.0/614 | 106.0/520 | 127.0/240 | 14.31 / 4.21 |
| official-second-3 | 43.0/202 | 107.0/517 | 106.0/432 | 110.0/239 | 15.31 / 4.80 |
| bots-500-r1-retry2 | 42.0/168 | 364.0/1694 | 266.0/1603 | 231.0/238 | 15.97 / 1.53 |
| bots-500-r2 | 42.0/168 | 339.5/1617 | 269.5/1442 | 231.0/238 | 15.86 / 1.69 |
| bots-500-r3 | 42.0/168 | 364.0/1449 | 287.0/1400 | 231.0/238 | 15.47 / 1.74 |

`SnakeReports::update` in `src/client/simulation/snakes.rs` gates changed/stale motion and mass at six/fifteen ticks; `changes` also checks recipient values. Motion intervals near 42 ms (39–45) account for **86.3–86.9% local / 63.0–66.5% official**. Mass near 105 ms (101–109) accounts for **~18% / 54–57%**. Local motion is not starved. Sub-35 ms motion exceptions remain **0.13–0.22% local / 0.17–0.24% official**; their cause is unidentified. Deleting unchanged decoded motion/`h` costs official **0.4–0.6 points**; adding tolerant 35/98 ms minimums changes little. Exact 42/105 ms filtering costs **1.7–3.1 points**, but can falsely reject quantized 41 ms intervals. The tracker omits turn direction: repeated decoded values need not be redundant commands.

## Activity

Changes are decoded counts per snake-second; state fractions are time-weighted. Speed >9 is a high-speed proxy; ≥13.9 is near full boost (14), not an observed boost command. Turning is reported wanted/heading separation above one angle byte, not reconstructed physical turning. `b` includes boost and death food.

| Capture | Heading / wanted / speed changes·s⁻¹ | Turning% | Speed >9% | Near boost% | Mean segments | b packets/s |
| --- | --- | --- | --- | --- | --- | --- |
| official-low-1 | 13.47 / 12.69 / 4.43 | 45.09 | 37.62 | 26.663 | 41.7 | 14.39 |
| official-second-1 | 13.60 / 12.83 / 4.68 | 44.69 | 33.26 | 22.008 | 39.7 | 13.77 |
| official-low-2 | 12.87 / 12.26 / 3.98 | 40.30 | 32.94 | 22.976 | 37.9 | 15.26 |
| official-second-2 | 13.08 / 12.15 / 4.50 | 45.25 | 34.21 | 23.228 | 53.9 | 13.46 |
| official-low-3 | 12.80 / 11.84 / 4.18 | 46.12 | 33.70 | 23.407 | 56.3 | 15.70 |
| official-second-3 | 13.49 / 12.62 / 4.75 | 46.50 | 39.25 | 27.666 | 51.9 | 19.15 |
| bots-500-r1-retry2 | 15.70 / 12.49 / 0.55 | 46.42 | 1.01 | 0.027 | 31.4 | 0.21 |
| bots-500-r2 | 15.55 / 12.68 / 0.76 | 46.21 | 0.71 | 0.015 | 30.9 | 0.97 |
| bots-500-r3 | 15.25 / 11.95 / 0.28 | 46.98 | 0.46 | 0.000 | 46.4 | 0.08 |

Local snakes are not simply straighter: heading changes are more frequent and the turning proxy is similar. The supported mismatch is speed/boost activity. Faster movement yields more anchors at the fixed 42-unit spacing; differing size, food and ecology also confound mass. Pongs match at **~3.9/s**. Removing own packets costs **4.7–6.1 points local / 1.7–2.1 official**, opposing a missing-own-heartbeat explanation.

## Phases and counterfactuals

Use complete one-server-second windows that remain in 6–10 throughout. Top phase is the median motion share on the most populated of six nominal tick residues; official residues can drift and are descriptive proxies. Shift each local other-snake motion train by independent uniform 0–5 ticks/window, holding other packets fixed: twenty fixed-seed trials preserve counts and spacing except at circular boundaries. Join synchronization is untested; six-tick rearming, behavior and respawns can also concentrate phases.

| Capture | Windows | Top phase% | Local grid ≤9% | Shifted phases ≤9% |
| --- | --- | --- | --- | --- |
| official-low-1 | 264 | 28.9 | — | — |
| official-second-1 | 210 | 28.7 | — | — |
| official-low-2 | 195 | 28.6 | — | — |
| official-second-2 | 188 | 29.4 | — | — |
| official-low-3 | 272 | 29.4 | — | — |
| official-second-3 | 204 | 27.8 | — | — |
| bots-500-r1-retry2 | 139 | 44.0 | 68.01 | 75.60 |
| bots-500-r2 | 38 | 40.3 | 68.31 | 71.88 |
| bots-500-r3 | 111 | 58.0 | 64.75 | 72.96 |

Content substitution draws official donor windows with the same rounded mean peer count (nearest available otherwise), replacing selected local packets with donor timestamps on the 7 ms grid at a random circular offset. Mass/body are other-snake `h`/anchors/tails; food includes all food families. Other local packets stay fixed. Report medians over twelve seeds per official donor capture, all six donors retained; combined phase+content uses twenty mixed-donor trials. [Counterfactual selections](counterfactuals.json) preserve each donor-capture median.

| Local capture | Baseline | Mass only | Body only | Food only | All three | All + phase |
| --- | --- | --- | --- | --- | --- | --- |
| bots-500-r1-retry2 | 68.01 | 74.32 | 71.77 | 74.59 | 80.40 | 84.69 |
| bots-500-r2 | 68.31 | 73.84 | 71.42 | 74.33 | 79.42 | 81.02 |
| bots-500-r3 | 64.75 | 70.89 | 68.56 | 71.60 | 77.64 | 82.18 |

All-three substitution adds **12.39/11.11/12.89 points**; phase shifts alone add **7.58/3.57/8.21**. All-three donor-capture median ranges are **79.51–81.45 / 78.40–80.11 / 76.58–78.32%**. These are sensitivities, not confidence intervals. This is synthetic occupancy, not a dynamics replay: donor traffic can be physically incompatible and shifts break dependencies. Effects overlap; boost, ecology and dirty comparisons cannot be causally partitioned. r3's stronger concentration and lower cadence counter a peer-count-only explanation. Full exact-count 6–10 weighting raises r1 **67.99→69.03%**, r3 **64.00→66.31%**; official remains **81.99–84.55%**. r2 has no ten-peer exposure; shared-support 6–9 results are retained separately. All three zero-bot controls have zero eligible 6–10 gaps.

## Limits and recommendation

**No host change, heartbeat or gate/tolerance adjustment.** `src/client/simulation/broadcast.rs` already emits every nonempty tick. Measured: received bytes, cache membership, intervals and decoded activity. Inferred: workload and phase concentration explain most of the residual. Unknown: missing internal ticks, dirty/rearm semantics and causal shares. The original r2 cadence and r3 delay failures remain failures. Older recordings are not pooled; no new held-out validation or frame-level significance is claimed. Raw archives, full CSV/window/event traces and original notes stay external and local-only; no redistribution rights are established. [Source pins](source-pins.json) retain the sparse-view commit, decoder, scripts and [neutral archive hashes](sources.json); hashes establish identity, not scientific validity.

A future official capture should stratify continuous-visibility reports by speed, turn, size and food activity, retaining wire turn direction, unchanged tuples, growth/tail mass and joint phases on both clocks. A future offline in-memory experiment should cross controlled cruise/boost/turn/food schedules with synchronized/staggered starts, hold size/peers/gates fixed and log dirty fields, `SnakeReports::update` opportunities and broadcast reasons. Perturb only initial report phases while preserving six-tick minimum spacing. Only activity-matched causal evidence would justify an opportunity-phase change.

## Reproduction

Copy `sources.json` to an external manifest and set archive paths, preserving aliases/cohorts/hashes; relative paths resolve against that manifest. First run the linked sparse-view study's primary analyzer to obtain external CSVs/windows. `INPUTS.json`, `SPARSE` and `OUT` below denote external locations; OUT must be new. From the repository root, serially:
```sh
STUDY=research/fidelity/2026-10-26-cadence-residual
uv run --locked --only-group tools python -B "$STUDY/analyze.py" --inputs INPUTS.json --sparse-output SPARSE --out OUT
uv run --locked --only-group tools python -B "$STUDY/counterfactual.py" --sparse-output SPARSE --out OUT
uv run --locked --only-group tools python -B "$STUDY/verify.py" --sparse-output SPARSE --out OUT
uv run --locked --only-group tools python -B "$STUDY/select_results.py" --out OUT --check
```
All twelve raw hashes and the decoder pin are checked. [Verification](verification.json) reconciles eligible packet families, counts, exposure and percentages against the earlier CSVs, plus three synthetic occupancy checks; original acquisition audits are not independently repeated. Portable selections reproduce exactly. No game-server connections, services, captures or Rust changes; serial standard-library analysis. Scientific status: **exploratory, self-checked; no independent review**.
