# Observation food index, October 24, 2026

Status: adopted. The food grid uses finer cells, when enough observers repay
building them, and trims its rows to the vision circle. A reused or persistent
index was ruled out. Measured on one shared Windows desktop; no API change.

## Question

Can batch observations get meaningfully faster if they stop rebuilding a food
spatial index for every batch, either by reusing the step's food indexes, by
keeping an observation index across steps, or by some other way of cutting
visibility-filtering cost?

The batch observer in `src/observation/mod.rs` builds a `FoodGrid` from
`world.food` and `world.vision` when `observers >= 8 && observers * food >= 1 << 15`.
Each living slot then marks the visible food candidates in a bitset and walks the
bitset in world order. `transport::Session` frames and `serve` use the same
observer.

## Setup

- Ryzen 7 9800X3D (8 cores, 16 logical), Windows 11 Pro 10.0.26200, rustc and
  Cargo 1.99.0, the locked release profile (thin LTO, one codegen unit), `-j 2`.
- A shared desktop, not an isolated host. Other applications and other agent
  sessions stayed open. Total CPU use usually read 5–18% between runs, with
  occasional spikes to about 80%. No quiet window was arranged.
- Baseline runtime: `main` at `6a812fd`. Later `main` commits up to `fb71ecd`
  change no file under `src/` or `examples/` and no lockfile entry. The harness
  commit `0b09521` adds the `official-128-step` and `official-128-bot` rows:
  `Config::official(128)`, 2,000 warmup ticks and 800 bot iterations, defined as
  in the [borrowed-views](../2026-10-24-borrowed-views/REPORT.md) patch.
- Commit hashes below are the measured local commits. Rebasing onto `main` gave
  the PR new hashes without changing `src/` or `examples/`. In the PR, the
  harness is the `test(perf): add official-128 scenario rows` commit, and the
  adopted runtime is its final `perf(observation)` commit. Intermediate commits
  are local only: v1 is the first adopted form without the narrow-food and
  observer rules, and the other variants are in `variants.diff`.
- `perf_compare --base` overlays the candidate's harness on the baseline. The
  `observe-sparse` baseline is `main`'s runtime built with the new
  `perf_observation` harness.

| Executable | Source | SHA-256 |
| --- | --- | --- |
| `perf_baseline`, baseline | `0b09521` | `d2a334309984135f7d5c87577f378aec0060dafe1d2d67cd2da1e84986f0786d` |
| `perf_baseline`, v1 | `2923018` | `4f85e5aeb3f3f8ec6fc5c1998bd64305e439fcd3e54ebde44294b9f489446c6b` |
| `perf_baseline`, first review round | `13a3c22` | `defc1acd706cf2a110ee896d0efd7243abb7165b352b6292aba5b1d830311c6d` |
| `perf_baseline`, adopted | `c513291` | `e31de69be57568481b51b291ed6b4a3a63d19d6300301147261adb346a61d9e7` |
| `perf_observation`, baseline (six fixtures) | `6a812fd` | `a2ed5310cd994e2f6935be88f51f1bfc4b3d2e6f81de2db4466374c505e4b109` |
| `perf_observation`, baseline (seven fixtures) | `fb71ecd` and new harness | `39f761899c71af460fbbd723d4fd61da0e57a33c4c89a78b11c2a3af22ca48de` |
| `perf_observation`, first review round | `13a3c22` | `733f017530101ec9b2f2d19a67a4c4616ad792f3ec1c7c65b2645bdaaccc81ee` |
| `perf_observation`, adopted | `c513291` | `202cb1cbe7223fc84531b90d886ef5e834fdc817b8cd478ecbe6402c4cb93871` |

After warmup, `official-128` has 128 living snakes and 23,662 pellets in a
7,580-unit arena, and `dome-proxy-96` has 96 living snakes and 11,032 pellets in
a 2,500-unit arena. Both worlds use vision 1,500.

## Profile before the change

[`phase-instrumentation.diff`](phase-instrumentation.diff) (SHA-256
`2f3198b41e67221370471b0a62d40e2aa4420333b6d491ef032ce749caf8c09e`, applies to
`0b09521` and to the PR's harness commit) adds relaxed atomic counters and `Instant` intervals to the observer.
`perf_baseline --profile` prints them. The diff also holds the exploratory
switches used below: `SLITHER_CELL_DIV` sets the cell divisor (2 by default), and
`SLITHER_NO_TRIM=1` disables row trimming. Set `SLITHER_NO_TRIM=1` to reproduce
the baseline profile.

Wall intervals cover the whole batch, observer construction and the passes of
`FoodGrid::new`. Per-slot intervals, summed over threads, cover marking candidates,
walking the bitset (which includes copying each visible `Food`), and prey plus
opponents. The table gives per-batch medians of three `-bot --profile` runs of
the instrumented baseline. They are not timing comparisons.

| Phase (ms) | `official-128` seq | `official-128` par | `dome-proxy-96` seq | `dome-proxy-96` par |
| --- | ---: | ---: | ---: | ---: |
| Batch (wall) | 1.239 | 0.331 | 1.408 | 0.326 |
| `FoodGrid::new` (wall) | 0.117 (9%) | 0.120 (36%) | 0.050 (4%) | 0.052 (16%) |
| – bounds pass | 0.026 | 0.026 | 0.011 | 0.011 |
| – cell pass and counts | 0.054 | 0.053 | 0.022 | 0.022 |
| – scatter | 0.036 | 0.040 | 0.016 | 0.018 |
| Marking (thread sum) | 0.591 (48%) | 0.776 | 0.843 (60%) | 1.106 |
| Bitset walk and copies (thread sum) | 0.359 (29%) | 0.484 | 0.327 (23%) | 0.488 |
| Prey and opponents (thread sum) | 0.139 (11%) | 0.197 | 0.164 (12%) | 0.244 |
| Candidates / visible pellets | 227,148 / 116,224 | same | 420,929 / 250,194 | same |

Percentages are shares of the batch wall time. In parallel batches the per-slot
sums are thread time on the pool, so they exceed the batch wall time.

- Building the index is a small share of a sequential batch (9% and 4%). It
  runs before the parallel split, so it is a large share of a parallel batch.
- Marking dominates. Half-vision cells (750 units) and the full query box test
  1.95 candidates per visible pellet in `official-128` and 1.68 in the dome, at
  about 2.4 ns per candidate.

## Reusing or persisting the index

Neither step cache can serve observation. `FoodCache` holds hashed per-cell
counts for the admission cell sizes and an ambient count, with no positions or
pellet indices. `EaterGrid` indexes snake pickup circles, not food. The step
keeps no positional food index.

A persistent observation index would have to recognize the current food list.
Pickups compact `Simulator::food` with `retain`, which shifts the index of every
later pellet, and the bitset order depends on those indices. `food` is also
public, and `VisibleState` also serves client frames. Each batch would therefore
need a verification pass like `FoodCache`'s (about the cost of the 0.026 ms bounds
pass) plus an index remap or incremental cell updates after every step. Even a
free index would save at most the construction share: 9% and 4% sequentially.
That is not a clear win in the dense sequential world, and it would tie a cache
to every food mutation path. It was not prototyped.

## Change

Marking was the promising lever. The adopted change is in `FoodGrid::new` and
`Observer::visit`:

- Cells are an eighth of the vision radius (187.5 units at vision 1,500), not
  half, when the extra cells pay for themselves. The existing doubling still caps
  either grid at `2 * food + 16` cells. Half-vision cells stay when food is
  narrower than the vision radius on either axis, or when
  `2 * observers * density * vision²` does not exceed the number of extra cells
  the fine grid needs. Here density is placed pellets over the food bounding box;
  see [Sparse batches with few observers](#sparse-batches-with-few-observers).
- Each queried row scans only the columns that a chord of a circle of radius
  `vision + 1` can reach. The chord uses the row's distance from the head reduced
  by one unit, and adds one unit on each side. The margin leaves the squared chord
  more than `2 * vision` above its rounding error for tame coordinates and vision.
  The exact visibility test still decides every candidate.

Output order, values and parallel/sequential agreement do not change. The
existing equivalence tests pass. The new test
`batches_keep_food_on_the_vision_edge_of_trimmed_rows` places food near the
narrow poles of the circle, a few ulps either side of the vision radius, in
batches of 16 observers and 4,096 pellets, for six visions up to 2^40. It fails
when the chord is half a unit too narrow. A chord computed with exact arithmetic,
without the margins, still passes. Cell granularity hides errors that small, so
the margins rest on the rounding argument rather than on the test.

### How the final form was chosen

An exploratory sweep on the instrumented build (two runs each) chose the cell
size. It reports candidates per visible pellet and batch wall time:

| Cells, rows | `official-128` seq | `official-128` par | `dome-proxy-96` seq | `dome-proxy-96` par |
| --- | --- | --- | --- | --- |
| vision/2, full box (baseline) | 1.95, 1.160 ms | 0.340 ms | 1.68, 1.264 ms | 0.326 ms |
| vision/2, trimmed | 1.70, 1.120 ms | 0.325 ms | 1.52, 1.212 ms | 0.314 ms |
| vision/4, trimmed | 1.34, 0.984 ms | 0.287 ms | 1.27, 1.090 ms | 0.303 ms |
| vision/8, full box | 1.43, 0.989 ms | 0.311 ms | 1.33, 1.088 ms | 0.313 ms |
| vision/8, trimmed | 1.17, 0.945 ms | 0.288 ms | 1.13, 1.018 ms | 0.285 ms |
| vision/16, trimmed | 1.08, 1.040 ms | 0.463 ms | 1.07, 1.046 ms | 0.298 ms |

At vision/16, the larger grid and the extra rows cost more than the candidates
they saved.

The first candidate, v1 (`2923018`: vision/8 cells, every row trimmed), improved
every large `perf_baseline` row. The frozen `perf_observation` fixtures showed
`observe-strip` 9.5–17% slower and `observe-wide` about 2% slower:

- In `observe-strip`, all food lies within 150 units of x = 0, so the grid has
  one column and nothing can be trimmed. The instrumented build put the cost in
  the grid build, where scattering pellets into 513 cells instead of 129 cost
  about 6 µs per batch, plus the per-row chord arithmetic.
- In `observe-wide`, the cell cap already coarsens the grid to 750 units. A row
  holds about one candidate, so the chord (a square root and two cell lookups
  per row) costs more than the candidates it removes.

[`variants.diff`](variants.diff) (SHA-256
`45401c7fc4a11a74a0d777472a9569ad8f05c2daa009d69d2c024d214736d1c2`) holds the
three rejected follow-ups. Each section applies alone to `2923018`:

| Variant | Change against v1 | Outcome |
| --- | --- | --- |
| `49128ca` | Trim only when the query box spans at least nine columns | Fixed `observe-wide`; observation 3–7 points less gain in the large worlds |
| `15183de` | Also keep half-vision cells when food is narrower than `2 * vision` | Neutral on all frozen fixtures, but `dome-proxy-32-bot` 10–11% slower: its 3,000-unit arena counts as narrow, while v1 was 12% faster there |
| `2a1857d` | Choose trimming per grid, with one monomorphized row loop for each choice | Still 3–8 points less gain than v1 in the large worlds |

A fourth attempt merged row spans that meet in cell order. It made
`observe-strip` and `observe-dense` slower and was dropped without a commit.

Any branch on the choice to trim, even a loop-invariant one, made the marking loop
slower than v1 in the large worlds. Five rotating rounds of observation time
(`--profile`), as ratios against the baseline:

| Variant | `official-128` seq | `official-128` par | `dome-proxy-96` seq | `dome-proxy-96` par | `dome-proxy-32` seq |
| --- | ---: | ---: | ---: | ---: | ---: |
| v1 | 0.841 | 0.927 | 0.810 | 0.869 | 0.880 |
| `49128ca` | 0.870 | 0.902 | 0.877 | 0.880 | 0.950 |
| `15183de` | 0.864 | 0.905 | 0.871 | 0.866 | 1.104 |
| `2a1857d` | 0.871 | 0.963 | 0.881 | 0.911 | 0.963 |
| adopted | 0.820 | 0.918 | 0.825 | 0.884 | 0.869 |

The variants were compared in separate sessions against their own baselines, so
compare within a column only roughly. The adopted form is v1 plus build-time
rules. The first, that food narrower than the vision radius on either axis keeps
half-vision cells, fixes `observe-strip`'s build cost but not the per-row chord
cost in `observe-strip` and `observe-wide`; see the results. The second came out
of independent review and is described next. The marking loop is identical to
v1's.

### Sparse batches with few observers

Independent review of the first adopted form found a regression the fixtures had
missed: 8 living snakes at the origin, vision 1,500, and 262,144 pellets on a
512×512 lattice over x, y = ±65,625. Refining grows the grid from 30,976 to
491,401 cells, still under the cap. The reviewer measured batches 1.996 → 2.655 ms
(median ratio 1.33, range 1.28–1.37, slower in all 9 pairs). The case is now the
`observe-sparse` fixture.

The instrumented build splits that world (five rotating runs, per batch):

| Cells, rows | Batch | Grid build | Cell pass | Prefix sums | Scatter | Marking | Candidates / visible |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| vision/2, full box (baseline) | 2.090 ms | 1.931 ms | 0.670 ms | 0.007 ms | 0.879 ms | 0.011 ms | 1,568 / 896 |
| vision/2, trimmed | 2.052 ms | 1.912 ms | 0.668 ms | 0.007 ms | 0.885 ms | 0.011 ms | 1,280 / 896 |
| vision/8, trimmed (first form) | 2.671 ms | 2.480 ms | 0.974 ms | 0.117 ms | 1.011 ms | 0.013 ms | 1,024 / 896 |

The grid build grows with cells: 460,425 extra cells cost 0.55 ms, about 1.2 ns
each, in count, prefix and scatter passes over a 2 MB `starts` array. The savings
grow with observers times skipped candidates. Marking cost 2.0 ns (dome) to 2.6 ns
(`official-128`) per candidate. Per observer, refining skipped 716 candidates in
`official-128`, 1,430 in `dome-proxy-96` and 68 here. Scaled by mean bounding-box
density times vision², those are 3.1, 1.4 and 2.0.

The adopted rule refines only when the conservative savings,
1.4 × observers × density × vision² candidates at 2 ns, exceed the extra cells at
1.2 ns. That ratio is 2.3 cells per skipped candidate, rounded down to 2. In this
world, eight observers repay about 550 cells against 460,425, so it keeps
half-vision cells. `official-128` repays about 59,000 against 6,120 extra cells,
`dome-proxy-96` about 190,000 against about 700, and `observe-dense` about 11,300
against 3,936; all three refine. In `observe-wide`, the cap makes both grids
identical. A unit test pins both outcomes for this lattice (8 and 8,192 observers).

## Method

[`pairs.py`](pairs.py) runs two copied `perf_baseline` executables in adjacent
pairs, alternating which runs first, after one untallied warmup pair. Each row
ran sequentially and with `--parallel` (default rayon pool), with 9 measured
pairs each: 800 iterations for `official-128-bot`, 900 for `dome-proxy-96-bot`
and 2,000 for `dome-proxy-32-bot`. Timing mode reports the mean
step-plus-observation time. A second set in `--profile` mode separates the
`observe_all_into` interval from the step. Ratios are medians of per-pair
candidate/baseline ratios, and ranges are min–max across pairs.

[`obs_rounds.py`](obs_rounds.py) does the same in rotating order for
`perf_observation`, with longer samples than the `perf_compare` defaults; Windows
process noise made 0.2 s samples unreliable. [`results.json`](results.json) holds
every measured row of the final comparisons, and the first review round's rows.

`tools/perf_compare.py --base fb71ecd --suite full --repeats 5` then checked the
whole `perf_baseline` catalog: full-state trace parity, step profiles and five
alternating pairs per row.

The results below are for the adopted form (`c513291`), measured after the
review. The first review round's form (`13a3c22`, without the observer rule)
measured within a few points of it in the large worlds.

## Results

Step plus observation (timing mode), in ms per operation:

| World | Batch | Baseline | Adopted | Ratio | Pairs faster |
| --- | --- | --- | --- | ---: | ---: |
| `official-128` | sequential | 1.319 [1.279–1.416] | 1.135 [1.091–1.193] | 0.860 [0.839–0.878] | 9/9 |
| `official-128` | parallel | 0.535 [0.492–0.580] | 0.516 [0.471–0.536] | 0.945 [0.894–0.989] | 9/9 |
| `dome-proxy-96` | sequential | 1.436 [1.352–1.571] | 1.162 [1.132–1.308] | 0.833 [0.793–0.858] | 9/9 |
| `dome-proxy-96` | parallel | 0.459 [0.436–0.512] | 0.414 [0.401–0.458] | 0.902 [0.873–0.928] | 9/9 |
| `dome-proxy-32` | sequential | 0.347 [0.336–0.625] | 0.308 [0.297–0.361] | 0.885 [0.577–0.900] | 9/9 |
| `dome-proxy-32` | parallel | 0.196 [0.187–0.250] | 0.186 [0.173–0.261] | 0.963 [0.918–1.157] | 8/9 |

`observe_all_into` and step intervals (profile mode), in ms per operation:

| World | Batch | Observation: baseline → adopted | Ratio | Pairs faster | Step ratio |
| --- | --- | --- | ---: | ---: | ---: |
| `official-128` | sequential | 1.192 → 1.002 | 0.842 [0.788–0.852] | 9/9 | 1.014 [0.914–1.023] |
| `official-128` | parallel | 0.377 → 0.359 | 0.951 [0.873–2.084] | 7/9 | 0.995 [0.953–1.207] |
| `dome-proxy-96` | sequential | 1.361 → 1.113 | 0.813 [0.686–0.887] | 9/9 | 0.986 [0.691–1.096] |
| `dome-proxy-96` | parallel | 0.350 → 0.320 | 0.883 [0.633–0.985] | 9/9 | 0.995 [0.889–1.030] |
| `dome-proxy-32` | sequential | 0.330 → 0.288 | 0.870 [0.742–0.884] | 9/9 | 0.979 [0.903–1.002] |
| `dome-proxy-32` | parallel | 0.169 → 0.156 | 0.941 [0.735–1.048] | 8/9 | 1.009 [0.913–1.019] |

Some ranges include single disturbed runs (load spikes from other sessions). The
parallel `official-128` observation ratio has varied between sessions: 0.900,
0.916, 0.918, 0.953 and 0.951. Nine rotating rounds that compared both forms
directly measured 0.824 and 0.828 sequentially, and 0.931 and 0.924 in
parallel, for the adopted and first-round forms. The step code is unchanged, and
its ratios stay near 1. Parallel gains are smaller because the sequential grid
build is untouched and is now about 40% of a parallel `official-128` batch.

Frozen `perf_observation` fixtures, 11 rotating rounds, median materialization
per batch:

| Fixture | Iterations | Baseline | Adopted | Ratio | Pairs slower |
| --- | ---: | ---: | ---: | ---: | ---: |
| `observe-dense` | 5,000 | 252.3 µs | 223.8 µs | 0.869 [0.726–0.946] | 0/11 |
| `observe-compact` | 3,000 | 357.1 µs | 356.9 µs | 1.008 [0.738–1.386] | 7/11 |
| `observe-wide` | 50,000 | 78.2 µs | 81.3 µs | 1.025 [0.920–1.083] | 8/11 |
| `observe-strip` | 10,000 | 122.0 µs | 127.6 µs | 1.040 [1.002–1.288] | 11/11 |
| `observe-one` | 50,000 | 7.82 µs | 7.85 µs | 0.985 [0.938–1.108] | 5/11 |
| `observe-empty` | 100,000 | 0.09 µs | 0.08 µs | 1.003 [0.895–1.092] | 6/11 |
| `observe-sparse` | 500 | 2,235.6 µs | 2,381.0 µs | 1.019 [0.949–1.148] | 6/11 |

This session was noisier than the first-round one (baseline ranges up to 25%
wide). In that earlier session, the first-round form measured `observe-wide`
1.034 (11/11 slower) and `observe-strip` 1.033 (10/11), while `observe-one` and
`observe-empty` (which never build a grid) came out about 2% slower. So about
1 point of that cost is a bias floor from code layout or run order. Here, too,
`observe-wide` and `observe-strip` are about 3% (3 µs per batch) slower; the same
cost appeared in every v1-shaped build. In both, the chord removes almost
nothing: the strip grid has a single column, and a row of the wide grid holds
about one candidate. `observe-sparse` keeps half-vision cells and is neutral; the
first-round form was 1.33 there.

The full `perf_baseline` catalog (`perf_compare --base fb71ecd --suite full`, five
pairs per row, adopted sources) produced identical full-state traces for all 23
rows. Median wall ratios for the rows that observe:

| Row | Wall ratio | Pair range |
| --- | ---: | ---: |
| `observe` | 0.889 | 0.838–0.934 |
| `sandbox-16-bot` | 1.028 | 0.995–1.032 |
| `official-30-bot` | 0.850 | 0.794–0.879 |
| `official-100-bot` | 0.848 | 0.821–0.884 |
| `official-200-bot` | 0.910 | 0.881–1.083 |
| `official-500-bot` | 0.880 | 0.722–0.984 |
| `dome-proxy-32-bot` | 0.878 | 0.817–0.987 |
| `dome-proxy-96-bot` | 0.882 | 0.864–0.915 |
| `official-128-bot` | 0.850 | 0.837–0.855 |

The other rows do not call the batch observer. Their ratios ranged from 0.952
to 1.083, and load spikes widened some ranges, for example `official-200-step`
(0.619–1.176). Three rows came out slower in every pair: `step-body` 1.056,
`official-500-step` 1.083 and `sandbox-16-bot` 1.028.
Nine longer alternating pairs on the same executables gave:

- `step-body` (100,000 iterations): 1.000 (0.963–1.408), 4/9 slower.
- `official-500-step` (200): 1.023 (0.921–1.106), 7/9 slower.
- `sandbox-16-bot` (6,000): 1.011 (0.914–1.042), 6/9 slower.

In the first review round, the same rows measured 0.974–1.006, 0.961–1.006 and
0.980–1.044. The step code is unchanged.

Trace parity: the seven `perf_observation` fixtures produce identical trace bytes
against `main` with the new harness (2-iteration traces; digests in
`results.json`). For v1, six bot rows (`official-128`, `dome-proxy-96`,
`official-100`, `official-30`, `sandbox-16`, `dome-proxy-32`, each with 24
iterations, sequential and parallel) were byte-identical. Their sequential and
parallel traces matched each other apart from the report line's `parallel` flag.

## Decision

Adopted. Sequential observation is 16–19% faster in both target worlds, and
parallel observation 5–12% faster. `dome-proxy-32-bot` and `observe-dense` improve
13%, and the step and the other `perf_baseline` rows are unchanged within noise.
The change is a cell-size rule and a per-row chord in the existing grid. It needs
no cache, so nothing can go stale.

The cost is about 3% (3 µs per batch) in the sparse `observe-wide` and narrow
`observe-strip` synthetic fixtures. Every form that avoided it, by branching on
whether to trim, gave up 3–8 points in the large worlds, and one was 10% slower
in `dome-proxy-32-bot`. The owner accepted this cost after the first review
round. Batches with too few observers to repay the fine grid, such as
`observe-sparse`, keep half-vision cells.

No reused or persistent food index is adopted, and none was prototyped (see
above). In parallel batches, the sequential `FoodGrid::new` is now the largest
remaining serial cost (about 0.12 ms in `official-128`). Building it on the pool,
or in fewer passes, could help parallel batches but would not help sequential
ones.

## Limitations

- One machine, without a quiet window. Some parallel ranges are wide and include
  single slower pairs. Rayon start-up and first-fill allocation fall inside the
  measured samples, as in the borrowed-views study.
- The phase profile, the sweep and the variant comparison come from short or
  instrumented runs. They explain the choices but are not timing claims.
- Only vision-1,500 worlds were timed. Sparse worlds whose cells hit the
  `2 * food + 16` cap get coarser cells, as before, and pay the per-row chord.
- The refinement rule uses mean density over the food bounding box. Clustered
  food or clustered observers can make it refine when refinement does not pay
  (or the reverse). The 1.2 ns cell cost comes from a 2 MB `starts` array, and
  smaller grids build cells more cheaply, so the rule leans towards half-vision
  cells.
- Gains depend on codegen. Structurally small edits to the row loop moved
  results by several percent in either direction.
- `transport::Session` frames and `serve` reach the same marking loop through
  `sight_all_into` but were not timed here.
- The full traces (several GB) were compared byte for byte and then deleted.
  Their SHA-256 digests are in `results.json`.

## Reproduction

The profile runs in a separate scratch checkout of the repository, detached at
the harness commit. It reads the patch from the PR branch first, because the
study directory does not exist at that commit:

```sh
STUDY=research/performance/2026-10-24-observation-food-index
git show perf/observation-food-index:$STUDY/phase-instrumentation.diff > /tmp/phase-instrumentation.diff
git switch --detach 0b09521  # or the PR's test(perf) harness commit
git apply /tmp/phase-instrumentation.diff  # profile only
cargo build --release --locked --example perf_baseline -j 2
SLITHER_NO_TRIM=1 target/release/examples/perf_baseline official-128-bot 800 --profile
```

For the `observe-sparse` phases, copy the PR's `examples/perf_observation.rs`
into that checkout as well, print `slither_sim::observation::profile::take()` at
exit, and build `perf_observation`.

Add `--parallel` for the parallel profile. For the paired timings, build
`perf_baseline` at `0b09521` (or the PR's harness commit) and at the candidate.
Build `perf_observation` from `main` with the PR's `examples/perf_observation.rs`
copied in, and from the candidate. Copy the executables out of the target
directories. Then run these from the root of a PR checkout, where the study
directory and `tools/` exist. `BASE`, `CAND`, `OBS_BASE` and `OBS_CAND` are
absolute paths to the copied executables:

```sh
STUDY=research/performance/2026-10-24-observation-food-index
python "$STUDY/pairs.py" BASE CAND rows.jsonl 9 timing official-128-bot:800,dome-proxy-96-bot:900,dome-proxy-32-bot:2000
python "$STUDY/pairs.py" BASE CAND rows.jsonl 9 profile official-128-bot:800,dome-proxy-96-bot:900,dome-proxy-32-bot:2000
python "$STUDY/obs_rounds.py" base,adopted OBS_BASE,OBS_CAND 11 obs.jsonl observe-sparse:500,observe-dense:5000,observe-wide:50000,observe-strip:10000
uv run --locked --only-group tools python -B tools/perf_compare.py --base fb71ecd --suite full --output outputs/perf/food-index
```
