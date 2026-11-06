# Exact Low-preset CPU preparation

## Result

<!-- RESULT -->
Low RTX/Vulkan Native 4K at factor 0.065: CPU preparation, resource sync and submission fall from 11.534 to 1.580 ms (86.3% lower) against main `7cb5ec37`, medians of five runs. Low's candidate median is no greater than Auto or Medium in all 12 hardware/world/zoom cells. All 144 full-frame RGBA SHA-256 pairs are identical.
<!-- /RESULT -->

At far and whole-map zoom Low is 0.36–0.56 ms below Auto in every cell. At stock
zoom its lead is 0.01–0.20 ms; RTX Original's 0.01 ms is within run variation.
These results cover the authored rectangle and measured CPU stages below.
They do not establish a bound for arbitrary worlds, hardware or every individual
frame under competing system load.

## Cause and change

`prepare_composed` admitted retained food only for `high_quality=true`, blend
one, and a non-Medium preset. Low sets `high_quality=false` and blend zero, so
it expanded each visible food into an outline command and a body command, then
copied and projected those generic commands every frame. Auto and High retained
compact food inputs. The regression comes from that admission rule,
independently of MSAA or scale.

Low now retains its two-command food plan. When the outlines precede every draw
on a black clear, they cannot change the picture, and native preparation omits
them; the existing renderer already skipped that black prefix at draw time.
Bodies keep their canonical reverse-world order and the unchanged generic quad
pipeline. The claimant of each ordered vertex chunk projects its bodies with
the existing f64 food projection and writes the f32 vertex bytes and draw state
directly, with no intermediate commands, quads or records. Chunks are claimed
by the existing scoped chunk workers from 8,192 bodies. Shorter passes,
menu backdrops and unusual texture-role metadata stay serial or use the generic
packer. Wasm keeps its serial path. Low records its long single-state body
runs directly; detailed food keeps #474's ordered native bundles.

The batch freezes its detail mode, resources and exact two-command budget;
retries and failed menu admission retain the issued picture. Intermediate
quality blends, unsupported sources, pressure and nonempty background prefixes
keep the ordinary fallback. Auto, High, Medium and degraded Auto keep their
existing admission and preparation paths. No shader, quality setting,
animation, food selection, MSAA or scaling policy changes.

## Rejected and superseded attempts

All of these are retained outside Git with their binaries, sources and logs.

- **Source-grouped Low instancing** was faster, but a supplemental ten-color
  AMD/DX12 control changed **10,606 pixels / 12,062 channels**, by at most
  **2/255** (`rejected-grouping.json`, measured on `1921f44c`). Additive
  8-bit blending is order-sensitive, so body order is kept.
- **Pool projection plus a serial repack** projected bodies into 96-byte records
  on the persistent pellet pool and then copied them into vertex chunks in a
  second, serial pass. That cut Low by about 6×, but its own five-run matrix
  left Low above Auto in 7 of 12 cells, for example 2.008 against 1.696 ms on
  RTX Native at 0.065. A stage probe put about 0.9 ms in projection and merge
  and 0.4 ms in the repack at 25,000 bodies.
- **Scoped chunk claimants for every Low pass** removed that cost at far and
  whole-map zoom, but cost more than a serial pass at stock zoom (about 7,300
  bodies). In exploratory RTX runs under load, Low preparation took 270–380 µs
  serially against 380–500 µs scoped at stock, and 850–1,420 µs serially against
  635–860 µs scoped at 25,000 bodies. The measured crossover is about 8k–12k;
  8,192 selects the serial pass below it.
- **The first rebased matrix** visited presets in one fixed order. Each run
  flipped between a fast and a slow machine state, raising preparation and
  submission about 1.5× together for every preset. Auto, always first, nearly
  always ran fast. That matrix left Low 0.01–0.18 ms above Auto in five cells,
  four of them at stock zoom, while order-reversed probes put Low below Auto.
  The selected matrix therefore visits every cell forward and reversed in each
  run. The first matrix is superseded, not pooled.
- **The balanced matrix on `eaac7df7`** passed: Low fell from 11.225 to
  1.152 ms on RTX Native at 0.065 and stayed at or below Auto and Medium in all
  12 cells, with 144 identical images. #474 then extended ordered native bundles
  to every retained-food frame, so the evidence was rebuilt on main `7cb5ec37`.
- **Ordered bundles for Low.** In alternating exploratory runs on `7cb5ec37`
  (two per variant and adapter), bundling Low's body runs cost more than direct
  recording at far and whole-map zoom, for example 1.68/1.97 against
  1.26/1.36 ms on AMD Native at 0.065; stock zoom was within noise. Low keeps
  direct recording.
- **The balanced matrix on `7cb5ec37` without warm-up** put Low 0.2–0.4 ms
  below Auto at far and whole-map zoom, but tied at stock: Low's median was
  0.028 ms (3%) above Auto on RTX Native, inside both run ranges. Low's
  submission was below Auto's in every stock cell, but its gameplay preparation
  was 0.36 against 0.29–0.32 ms. Per-visit medians traced that to the harness:
  the first preset visited in a process prepared in 0.24 ms, and every later
  visit, Auto's reverse visit included, in about 0.36 ms. On matched reverse
  visits Low was below Auto (0.956/1.032 against 1.033/1.098 ms). The selected
  matrix adds one untimed traversal of every preset before measuring.

## Timing

<!-- SUMMARY -->
20 interleaved runs; 48,000 measured frames, each cell visited forward and reversed; 20 runs with an observed compiler/linker. Per-run median machine CPU load: 23.0–75.1%; all sampled load: 13.1–100.0%.
<!-- /SUMMARY -->

<!-- TIMING -->
| Adapter / world / zoom | Low main (ms) | Low candidate (ms) | Auto candidate (ms) | Medium candidate (ms) |
| --- | ---: | ---: | ---: | ---: |
| rtx / Native / stock | 4.598 [4.392, 4.969] | 0.950 [0.924, 0.994] | 0.996 [0.974, 1.050] | 4.685 [3.787, 5.082] |
| rtx / Native / far | 11.534 [10.660, 12.837] | 1.580 [1.485, 1.703] | 2.082 [2.012, 2.171] | 11.744 [9.549, 15.350] |
| rtx / Native / whole | 12.069 [10.384, 13.641] | 1.532 [1.378, 1.664] | 1.893 [1.866, 2.366] | 11.915 [9.752, 13.981] |
| rtx / Original / stock | 4.653 [4.474, 5.456] | 1.050 [0.994, 1.072] | 1.060 [1.026, 1.193] | 4.816 [4.091, 5.902] |
| rtx / Original / far | 12.134 [10.582, 13.729] | 1.603 [1.494, 1.794] | 2.052 [2.030, 2.164] | 12.032 [10.545, 15.547] |
| rtx / Original / whole | 11.452 [10.904, 13.758] | 1.592 [1.511, 1.697] | 1.983 [1.954, 2.048] | 11.700 [10.379, 14.114] |
| amd / Native / stock | 4.602 [4.436, 4.762] | 0.915 [0.690, 0.985] | 1.031 [0.856, 1.070] | 4.012 [3.240, 4.886] |
| amd / Native / far | 11.530 [9.474, 12.290] | 1.551 [1.266, 1.654] | 2.115 [1.881, 2.123] | 11.771 [10.266, 12.728] |
| amd / Native / whole | 11.229 [7.705, 13.757] | 1.530 [1.252, 1.589] | 1.918 [1.900, 1.947] | 11.733 [8.524, 12.751] |
| amd / Original / stock | 4.667 [3.973, 5.368] | 0.875 [0.715, 1.034] | 1.079 [0.913, 1.196] | 4.215 [3.604, 4.958] |
| amd / Original / far | 12.364 [8.552, 12.387] | 1.632 [1.428, 1.739] | 2.126 [1.945, 2.146] | 12.283 [9.083, 12.990] |
| amd / Original / whole | 11.870 [7.676, 14.273] | 1.535 [1.415, 1.772] | 1.966 [1.802, 2.001] | 12.075 [8.769, 12.983] |
<!-- /TIMING -->

Entries are median [minimum, maximum] across five per-run medians, in
milliseconds. `runs.csv` also retains each cell's p95 and preparation, sync,
submission and GPU-wait stage medians. The primary metric sums preparation,
resource synchronization and presenter submission **within each frame** before
computing quantiles. It includes native chunk scheduling/projection and CPU
queue submission. It is elapsed CPU-path time, not aggregate thread CPU time.
GPU completion waits, readback, compression, fixture construction, camera warmup,
journaling and frame recycling are outside this metric. Simulation, network,
HUD/menu composition, display pacing and scanout are not measured.

Each run first traverses all four presets, three zooms and two world modes on
one adapter without measuring. It then measures them in the order Auto, Low,
Medium, High and again reversed. Each visit has 10 warmup and 50 measured frames
of a frozen animation state, so each cell pools 100 frames from both positions. Five rounds alternate main/candidate
order; both adapters use the same release driver. All scheduled runs are
retained; no samples or pairs are excluded for load. Other work, including
compilers, ran on the machine throughout; `runs.csv` records its sampled load.

## Exactness

`image-controls.json` contains every paired **full-frame RGBA SHA-256**, not
crops or a tolerance: 4 presets × 3 zooms × 2 world modes × 2 adapters × 3
animation states = **144 comparisons** of main `7cb5ec37` against the candidate,
with zero changed bytes. Each image is 33,177,600 decoded RGBA bytes at
3840×2160, captured after three actual renders. The animation controls use 1,
12 and 240 update steps. Camera/zoom preparations settle for 110 pictures
without advancing that animation state.

`palette-controls.json` adds 36 full-frame comparisons of the candidate against
its own canonical renderer on ten interleaved food colors: both adapters, all
three zooms, both world modes and all three animation states. Every decoded
RGBA byte matches. This exercises the source-order counterexample above; it is
separate from the main-versus-candidate 144-image matrix.

The authored fixture repeats #470's 25,000-food protocol-13 rectangle: 200
columns, 125 rows, color 9, size 1, plus one authored snake. Its authored arena
radius is enlarged to admit factor 0.065 separately from the whole-map endpoint.
The measured far factor is 0.06499999999999999; whole-map is approximately
0.005716. All 25,000 received foods remain retained; the ordinary stock viewport
culls outside food. No local art pack or official art is loaded. The projection
is 1500×845, DPR 1; Native draws at 4K, while Original uses the existing backing
and blit. Auto AA keeps Native single-sample and Original four-sample on both
adapters.

Hardware controls use NVIDIA GeForce RTX 5070 Ti / Vulkan and AMD Radeon
Graphics / DX12. Device creation requests no timestamp features. Each arm is
built from a `git archive` of its revision with the identical driver, cleaning
only the two client release packages between arms. `identities.json` binds main
`7cb5ec37`, the candidate revision, the driver, release executables, source
archives and relevant runtime files.

`low_stream_matches_food_plan_bits_and_two_command_budget` compares the retained
plan against ordinary `plan_food_with_zoom_and_density`, every projected float
by bits, and exact admission boundaries over animation, zoom and density. The
explicit offscreen `prepared_bytes_states_and_coverage_match_the_canonical_stream`
test covers both detail modes at all three zooms. It compares complete vertex
bytes, states, coverage and canonical rendered pixels over an authored menu
backdrop, and every byte, state and coverage field of the direct body store
against the generic packer on the ten-color rectangle.

## Verification and access

Focused pellet tests: 26 passed, with four explicit GPU/benchmark tests left to
their own runs. The explicit offscreen packing test passed. The full client
check (`npm run check`) and the repository guard passed on the rebased branch.
No independent review or
interactive play acceptance is claimed.

Raw compressed RGBA, timing frames, system load/process journals, source
archives, executables, build/check logs, stage probes and failed attempts stay
outside Git under `D:/slither-scratch/client-lane-20261105/perf/low-preset/`.
Process/host details are not published. Timing and GPU controls acquire the
shared lane timing lock atomically with purpose, UTC start and PID, wait while
another live holder is younger than 90 minutes, and remove only their own
token. Every lease is released within 60 minutes.

Reproduce with the pinned toolchain and per-command
`CARGO_TARGET_DIR=D:/CargoTargets/low-preset CARGO_BUILD_JOBS=2`.
Build `low_preset_perf` in release in each pinned source arm, supplying the
identical example to main. Hold the shared timing lock and run:

```text
low_preset_perf NEW_DIRECTORY rtx control
low_preset_perf NEW_DIRECTORY amd control
low_preset_perf NEW_DIRECTORY rtx timing
low_preset_perf NEW_DIRECTORY amd timing
```

Use fresh directories. Repeat timing five times per arm/adapter in alternating
arm order; retain machine load. Decompress `.rgba.zlib` files before SHA-256
comparison. The driver opens no window, transport or server. Recompute the
published selected reduction without the raw artifacts:

```text
uv run --no-project python -B research/client-performance/2026-11-05-low-preset/recompute.py --check
```
