# Observation food index, October 24, 2026

Status: adopted. The food grid uses finer cells and trims its rows to the vision
circle. A reused or persistent index was ruled out. Measured on one shared Windows
desktop; no API change.

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
- Baseline runtime: `main` at `6a812fd`. The harness commit `0b09521` adds the
  `official-128-step` and `official-128-bot` rows: `Config::official(128)`, 2,000
  warmup ticks and 800 bot iterations, defined as in the
  [borrowed-views](../2026-10-24-borrowed-views/REPORT.md) patch. Candidates sit
  on top of it. The adopted runtime is `13a3c22`, whose sources equal the
  measured candidate `dc8080e`. `perf_compare --base` overlays the candidate's
  harness on the baseline. The intermediate commits are local only. v1 is the
  adopted source without the narrow-food rule, and the other variants are in
  `variants.diff`.

| Executable | Source | SHA-256 |
| --- | --- | --- |
| `perf_baseline`, baseline | `0b09521` | `d2a334309984135f7d5c87577f378aec0060dafe1d2d67cd2da1e84986f0786d` |
| `perf_baseline`, v1 | `2923018` | `4f85e5aeb3f3f8ec6fc5c1998bd64305e439fcd3e54ebde44294b9f489446c6b` |
| `perf_baseline`, adopted | `13a3c22` | `defc1acd706cf2a110ee896d0efd7243abb7165b352b6292aba5b1d830311c6d` |
| `perf_observation`, baseline | `6a812fd` | `a2ed5310cd994e2f6935be88f51f1bfc4b3d2e6f81de2db4466374c505e4b109` |
| `perf_observation`, adopted | `13a3c22` | `733f017530101ec9b2f2d19a67a4c4616ad792f3ec1c7c65b2645bdaaccc81ee` |

After warmup, `official-128` has 128 living snakes and 23,662 pellets in a
7,580-unit arena, and `dome-proxy-96` has 96 living snakes and 11,032 pellets in
a 2,500-unit arena. Both worlds use vision 1,500.

## Profile before the change

[`phase-instrumentation.diff`](phase-instrumentation.diff) (SHA-256
`2f3198b41e67221370471b0a62d40e2aa4420333b6d491ef032ce749caf8c09e`, applies to
`0b09521`) adds relaxed atomic counters and `Instant` intervals to the observer.
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
  half. The existing doubling still caps the grid at `2 * food + 16` cells. Food
  narrower than the vision radius on either axis keeps half-vision cells.
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
compare within a column only roughly. The adopted form is v1 plus a build-time
rule: food narrower than the vision radius on either axis keeps half-vision cells.
The marking loop is identical to v1's. The rule fixes `observe-strip`'s build
cost but not the per-row chord cost in `observe-strip` and `observe-wide`; see
the results.

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
every measured row of the final comparisons.

`tools/perf_compare.py --base 6a812fd --suite full --repeats 5` then checked the
whole `perf_baseline` catalog: full-state trace parity, step profiles and five
alternating pairs per row.

## Results

Step plus observation (timing mode), in ms per operation:

| World | Batch | Baseline | Adopted | Ratio | Pairs faster |
| --- | --- | --- | --- | ---: | ---: |
| `official-128` | sequential | 1.324 [1.285–1.544] | 1.134 [1.104–1.441] | 0.862 [0.835–0.991] | 9/9 |
| `official-128` | parallel | 0.535 [0.485–0.569] | 0.475 [0.454–0.535] | 0.944 [0.835–0.984] | 9/9 |
| `dome-proxy-96` | sequential | 1.401 [1.352–1.458] | 1.151 [1.110–1.220] | 0.821 [0.798–0.864] | 9/9 |
| `dome-proxy-96` | parallel | 0.444 [0.437–0.492] | 0.404 [0.390–0.478] | 0.902 [0.860–1.027] | 7/9 |
| `dome-proxy-32` | sequential | 0.345 [0.334–0.371] | 0.300 [0.295–0.338] | 0.876 [0.816–0.967] | 9/9 |
| `dome-proxy-32` | parallel | 0.201 [0.183–0.213] | 0.185 [0.170–0.205] | 0.941 [0.883–1.004] | 8/9 |

`observe_all_into` and step intervals (profile mode), in ms per operation:

| World | Batch | Observation: baseline → adopted | Ratio | Pairs faster | Step ratio |
| --- | --- | --- | ---: | ---: | ---: |
| `official-128` | sequential | 1.144 → 0.963 | 0.838 [0.826–0.900] | 9/9 | 1.006 [0.976–1.087] |
| `official-128` | parallel | 0.319 → 0.309 | 0.953 [0.808–1.067] | 7/9 | 0.981 [0.859–1.035] |
| `dome-proxy-96` | sequential | 1.267 → 1.040 | 0.832 [0.778–0.849] | 9/9 | 1.020 [0.943–1.055] |
| `dome-proxy-96` | parallel | 0.347 → 0.299 | 0.885 [0.718–0.986] | 9/9 | 0.996 [0.912–1.049] |
| `dome-proxy-32` | sequential | 0.314 → 0.280 | 0.870 [0.828–1.200] | 8/9 | 0.989 [0.906–1.533] |
| `dome-proxy-32` | parallel | 0.159 → 0.154 | 0.968 [0.877–1.124] | 7/9 | 1.028 [0.939–1.154] |

The parallel `official-128` observation ratio came out at 0.900, 0.916 and 0.918
in three earlier sessions with near-identical code; 0.953 is the weakest
measurement. The step code is unchanged, and its ratios stay near 1. Parallel
gains are smaller because the sequential grid build is untouched and is now about
40% of a parallel `official-128` batch.

Frozen `perf_observation` fixtures, 11 rotating rounds, median materialization
per batch:

| Fixture | Iterations | Baseline | Adopted | Ratio | Pairs slower |
| --- | ---: | ---: | ---: | ---: | ---: |
| `observe-dense` | 5,000 | 241.6 µs | 208.8 µs | 0.863 [0.824–0.915] | 0/11 |
| `observe-compact` | 3,000 | 340.0 µs | 327.4 µs | 0.992 [0.794–1.329] | 3/11 |
| `observe-wide` | 50,000 | 76.6 µs | 79.3 µs | 1.034 [1.011–1.659] | 11/11 |
| `observe-strip` | 10,000 | 115.5 µs | 119.1 µs | 1.033 [0.908–1.193] | 10/11 |
| `observe-one` | 50,000 | 6.97 µs | 7.03 µs | 1.018 [0.954–1.056] | 7/11 |
| `observe-empty` | 100,000 | 0.08 µs | 0.08 µs | 1.020 [0.926–1.083] | 8/11 |

`observe-one` and `observe-empty` never build a grid, yet they came out about 2%
slower, in most pairs. That bias, probably from code layout or run order, is the
floor for this comparison. `observe-wide` and `observe-strip` are about 3%
(3 µs per batch) slower, in 11 and 10 of 11 pairs; the same cost appeared in every v1-shaped build. In both
the chord removes almost nothing: the strip grid has a single column, and a row
of the wide grid holds about one candidate.

The full `perf_baseline` catalog (`perf_compare --base 6a812fd --suite full`, five
pairs per row, adopted sources) produced identical full-state traces for all 23
rows. Median wall ratios for the rows that observe:

| Row | Wall ratio | Pair range |
| --- | ---: | ---: |
| `observe` | 0.882 | 0.660–1.315 |
| `sandbox-16-bot` | 0.980 | 0.843–1.009 |
| `official-30-bot` | 0.868 | 0.856–0.892 |
| `official-100-bot` | 0.864 | 0.823–0.905 |
| `official-200-bot` | 0.885 | 0.860–0.903 |
| `official-500-bot` | 0.921 | 0.904–0.929 |
| `dome-proxy-32-bot` | 0.826 | 0.617–0.927 |
| `dome-proxy-96-bot` | 0.863 | 0.811–0.877 |
| `official-128-bot` | 0.864 | 0.827–0.953 |

The `-step` rows and `step-default`, `step-food`, `step-body` and `protocol`
ranged from 0.952 to 1.020. Load spikes during the run widened some step ranges,
for example `dome-proxy-96-step` (0.603–1.892). `portable` came out at 1.023
(1.023–1.108) and `client-official-100` at 1.038 (0.971–1.255). Neither calls
the batch observer. Nine longer alternating pairs on the same executables gave 0.997
(0.971–1.013) for `portable` at 20,000 iterations and 1.000 (0.888–1.041) for
`client-official-100` at 300 iterations.

Trace parity: the six `perf_observation` fixtures produce identical trace bytes
(2-iteration traces; digests in `results.json`). Six bot rows (`official-128`,
`dome-proxy-96`, `official-100`, `official-30`, `sandbox-16`, `dome-proxy-32`,
each with 24 iterations, sequential and parallel) were byte-identical for v1.
Their sequential and parallel traces matched each other apart from the report
line's `parallel` flag.

## Decision

Adopted. In both target worlds, sequential observation is about 16–17% faster
and parallel observation 5–12% faster. `dome-proxy-32-bot` and `observe-dense`
improve 13–14%, and the step and the other `perf_baseline` rows are unchanged
within noise. The change is a cell-size rule and a per-row chord in the existing
grid. It needs no cache, so nothing can go stale.

The cost is about 3% (3 µs per batch) in the sparse `observe-wide` and narrow
`observe-strip` synthetic fixtures. Every form that avoided it, by branching on
whether to trim, gave up 3–8 points in the large worlds, and one was 10% slower
in `dome-proxy-32-bot`. The decision rule asked for no slowdown in the
`perf_baseline` workload set, and none was found there. The fixture cost is
recorded here rather than hidden.

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
- Gains depend on codegen. Structurally small edits to the row loop moved
  results by several percent in either direction.
- `transport::Session` frames and `serve` reach the same marking loop through
  `sight_all_into` but were not timed here.
- The full traces (several GB) were compared byte for byte and then deleted.
  Their SHA-256 digests are in `results.json`.

## Reproduction

```sh
git switch --detach 0b09521
git apply research/performance/2026-10-24-observation-food-index/phase-instrumentation.diff  # profile only
cargo build --release --locked --example perf_baseline -j 2
SLITHER_NO_TRIM=1 target/release/examples/perf_baseline official-128-bot 800 --profile
```

Add `--parallel` for the parallel profile. For the paired timings, build
`perf_baseline` and `perf_observation` at `0b09521` (or `6a812fd`) and at the
candidate, copy the executables out of the target directory, then run:

```sh
python pairs.py BASE CAND rows.jsonl 9 timing official-128-bot:800,dome-proxy-96-bot:900,dome-proxy-32-bot:2000
python pairs.py BASE CAND rows.jsonl 9 profile official-128-bot:800,dome-proxy-96-bot:900,dome-proxy-32-bot:2000
python obs_rounds.py base,adopted OBS_BASE,OBS_CAND 11 obs.jsonl observe-dense:5000,observe-wide:50000,observe-strip:10000
uv run --locked --only-group tools python -B tools/perf_compare.py --base 6a812fd --suite full --output outputs/perf/food-index
```
