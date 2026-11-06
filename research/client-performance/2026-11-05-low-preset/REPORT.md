# Exact Low-preset CPU preparation

## Result

<!-- RESULT -->
Low RTX/Vulkan Native 4K at factor 0.065: CPU preparation, resource sync and submission fall from 7.590 to 1.392 ms (81.7% lower) against main `7cb5ec37`, medians of five runs. At far and whole-map zoom Low's median is 0.102–0.248 ms below Auto in all 8 cells; at stock zoom no lead is claimed: Low's median is -0.019 to +0.017 ms from Auto's and never above Auto's slowest run. Low is below Medium in all 12 cells. All 144 full-frame RGBA SHA-256 pairs are identical, and all 8 coincident-pair controls match the canonical renderer.
<!-- /RESULT -->

Low is far below main and Medium everywhere and below Auto at far and whole-map
zoom. At stock zoom it shows no measurable difference from Auto: RTX Native's
median is 0.017 ms above Auto's, and the other three stock cells are
0.009–0.019 ms below. Unrelated compiler work overlapped 15 of the 20 runs
despite a quiet gate; see Timing. These results cover the authored rectangle and measured CPU stages below.
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

The generic packer merges two adjacent additive quads with the same source and
identical positions and texture coordinates into one quantized `PairLighter`
draw. Distinct Low foods meet that rule when they share position, color, size,
factory units and animation state. Each chunk claimant therefore compares every
body with its predecessor by the same rule, re-projecting the predecessor at a
chunk start. Any such adjacency returns the whole frame to the generic packer,
including across chunk and thread boundaries.

The batch freezes its detail mode, resources and exact two-command budget;
retries and failed menu admission retain the issued picture. Intermediate
quality blends, unsupported sources, pressure and nonempty background prefixes
keep the ordinary fallback. Auto, High, Medium and degraded Auto keep their
existing admission and preparation paths. No shader, quality setting,
animation, food selection, MSAA or scaling policy changes.

## Rejected and superseded attempts

All of these are retained outside Git with their binaries, sources and logs.

- **Unpaired coincident bodies (`bed9a259`).** An independent review found that
  the direct stream drew coincident adjacent bodies as two `Lighter` draws
  instead of the generic `PairLighter` pair. On authored coincident pairs this
  changed up to 2,561 pixels by at most 4/255 at RTX/Vulkan Native, and up to
  10,108 pixels by at most 6/255 with 8,192 foods on Original
  (`coincident-controls.json`). The selected change falls back on any such
  adjacency.
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
- **The warmed matrix on `bed9a259`** put Low at or below Auto and Medium in all
  12 cells (11.534 to 1.580 ms on RTX Native at 0.065, under 23–75% machine
  load), with 144 identical images. It measured the unpaired coincident-body
  code above and is superseded by the matrix on the fix.

## Timing

<!-- SUMMARY -->
20 interleaved runs; 48,000 measured frames, each cell visited forward and reversed; 15 of 20 runs observed a compiler/linker from unrelated work despite a quiet gate; only round 3 had none in all four runs, and there Low is at or below Auto in 12 of 12 cells. Per-run median machine CPU load: 9.4–33.3%; all sampled load: 3.1–100.0%.
<!-- /SUMMARY -->

<!-- TIMING -->
| Adapter / world / zoom | Low main (ms) | Low candidate (ms) | Auto candidate (ms) | Medium candidate (ms) |
| --- | ---: | ---: | ---: | ---: |
| rtx / Native / stock | 3.156 [3.030, 3.715] | 0.773 [0.726, 0.854] | 0.756 [0.728, 0.866] | 3.204 [3.099, 3.321] |
| rtx / Native / far | 7.590 [7.124, 11.723] | 1.392 [1.330, 1.506] | 1.582 [1.504, 1.939] | 7.987 [7.544, 8.487] |
| rtx / Native / whole | 7.601 [7.034, 10.285] | 1.336 [1.292, 1.440] | 1.458 [1.428, 1.701] | 8.099 [7.371, 9.206] |
| rtx / Original / stock | 3.192 [3.096, 3.996] | 0.818 [0.794, 0.898] | 0.827 [0.797, 0.988] | 3.282 [3.202, 3.503] |
| rtx / Original / far | 7.541 [7.260, 10.698] | 1.442 [1.403, 1.514] | 1.596 [1.548, 1.853] | 7.959 [7.513, 8.828] |
| rtx / Original / whole | 7.732 [7.151, 10.796] | 1.406 [1.383, 1.453] | 1.593 [1.517, 1.724] | 7.801 [7.427, 9.213] |
| amd / Native / stock | 3.112 [3.003, 3.626] | 0.786 [0.771, 0.813] | 0.805 [0.799, 0.848] | 3.187 [3.156, 3.823] |
| amd / Native / far | 7.853 [7.106, 8.562] | 1.395 [1.275, 1.846] | 1.643 [1.605, 1.823] | 7.849 [7.569, 8.773] |
| amd / Native / whole | 7.781 [7.182, 7.853] | 1.373 [1.238, 1.481] | 1.530 [1.522, 1.554] | 7.846 [7.315, 8.524] |
| amd / Original / stock | 3.166 [3.045, 3.643] | 0.838 [0.825, 0.882] | 0.853 [0.837, 0.886] | 3.245 [3.210, 4.007] |
| amd / Original / far | 7.709 [7.073, 7.775] | 1.498 [1.397, 1.937] | 1.600 [1.593, 1.732] | 7.871 [7.506, 9.840] |
| amd / Original / whole | 7.671 [7.369, 8.017] | 1.438 [1.341, 1.505] | 1.593 [1.562, 1.650] | 8.076 [7.382, 8.757] |
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
retained; no samples or pairs are excluded for load. Before each run the driver
waited up to 80 minutes for 60 seconds without a compiler or linker, but another
agent's builds and tests resumed during most runs: 15 of 20 runs observed a
compiler or linker, and only round 3 was free of them in all four runs. That
clean round alone puts Low at or below Auto in all 12 cells (RTX Native stock
0.726 against 0.728 ms), but one round is not a separate result. `runs.csv`
records each run's sampled load and compiler flag.

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

`coincident-controls.json` adds 8 full-frame 4K comparisons of the candidate
against its own canonical renderer on authored coincident pairs: food 2j and
2j+1 share color, position, size and factory units. Both adapters, Native and
Original, with 4,096 foods (serial preparation) and 8,192 foods (scoped chunks)
all match byte for byte; the review's six `bed9a259` failures are retained beside
them. The offscreen test `coincident_low_bodies_keep_the_generic_pair_packer`
checks that preparation falls back for serial and scoped duplicates, for a
single pair straddling the first chunk boundary and behind a menu prefix, and
not for distinct foods. It fails on `bed9a259` and passes on the candidate.

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

The full client check (`npm run check`), which runs every non-ignored client
test, and the repository guard passed. The explicit offscreen packing and
coincident-pair tests passed on RTX/Vulkan; the other explicit GPU/benchmark
tests were not run. An independent review of `bed9a259` found the coincident-pair
difference above; the fix has not had a second independent review. No
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
