# Exact Low-preset CPU preparation

## Result

<!-- RESULT -->
Low RTX/Vulkan Native 4K at factor 0.065: CPU preparation, resource sync and submission fall from 7.349 to 1.211 ms (83.5% lower) against main `7cb5ec37`, medians of five runs. Low's median is below Auto's in all 12 hardware/world/zoom cells: by 0.207–0.332 ms at far and whole-map zoom and by 0.027–0.059 ms at stock zoom. Low is below Medium in all 12 cells. All 144 full-frame RGBA SHA-256 pairs are identical, and all 16 coincident-pair controls match the canonical renderer.
<!-- /RESULT -->

At stock zoom Low's lead over Auto is small, 0.027–0.059 ms (3–7%), but it
holds in all four stock cells. No compiler or linker ran during any
measured run. These results cover the authored rectangle and measured CPU
stages below.
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

The generic packer greedily merges two adjacent additive quads with the same
source and identical positions and texture coordinates into one quantized
`PairLighter` draw. Distinct Low foods meet that rule when they share position,
color, size, factory units and animation state. Each chunk claimant stores
every body at once. When a body matches an unpaired predecessor by the same
rule, the claimant turns that stored quad into the same `PairLighter`, with the
second quad's alpha, and keeps the consumed quad's slot as a noop draw, which
the encoder skips, so chunk boundaries never move. A chunk's last unpaired body
projects the next chunk's first body to complete a pair across the boundary. A chunk's
first body counts the equal bodies before it, re-projecting them, to know
whether it is the consumed half of a pair begun in the previous chunk. Only
pairs of outline- or glow-tagged body sources, whose layer spans a noop slot
would split, still return the frame to the generic packer.

Low inserts no late food, so its late barrier is empty. The draws on both sides
are packed as one stream, and a caller-authored pair across that barrier keeps
the generic lookahead.

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
  (`coincident-controls.json`).
- **Whole-frame fallback for coincident bodies (`f8695b43`).** The first fix
  returned the whole frame to serial generic packing whenever two adjacent
  bodies coincided. A second independent review found that one such pair among
  25,000 foods costs roughly two to three times the fast path's projection and
  storage, without parallelism. The same review found that Low packed caller
  draws separately on each side of its empty late barrier: two coincident
  `Lighter` caller draws on a non-food resource across it lost their
  `PairLighter`, changing 5,300 RTX/Vulkan Native pixels by at most 2/255 on
  `4a49757f`. The selected change emits the pairs in place and packs across the
  empty barrier. The matrix measured on `f8695b43` is superseded; another
  agent's builds overlapped 15 of its 20 runs despite the quiet gate.
- **Delayed body emission (`bd08a746`).** The first in-place pairing held each
  body until its successor was known. In a matrix with no compiler overlap, Low
  stayed 0.05–0.28 ms below Auto at far zoom but tied it at RTX whole-map zoom
  (1.420 against 1.425 ms Native, 1.484 against 1.484 ms Original) and was
  0.017–0.047 ms above Auto in all four stock cells; Low's submission rose by
  up to 0.09 ms over `f8695b43` while Auto's fell. The selected change stores each body at once and rewrites the stored
  quad only when a follower matches.
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
20 interleaved runs; 48,000 measured frames, each cell visited forward and reversed; 0 of 20 runs observed a compiler or linker process. Per-run median machine CPU load: 8.9–18.8%; all sampled load: 3.7–94.3%.
<!-- /SUMMARY -->

<!-- TIMING -->
| Adapter / world / zoom | Low main (ms) | Low candidate (ms) | Auto candidate (ms) | Medium candidate (ms) |
| --- | ---: | ---: | ---: | ---: |
| rtx / Native / stock | 3.070 [3.059, 3.089] | 0.704 [0.694, 0.885] | 0.758 [0.732, 0.778] | 3.200 [3.128, 3.220] |
| rtx / Native / far | 7.349 [7.131, 7.585] | 1.211 [1.196, 1.326] | 1.520 [1.492, 1.707] | 8.026 [7.521, 8.137] |
| rtx / Native / whole | 7.392 [7.211, 7.797] | 1.183 [1.176, 1.307] | 1.419 [1.406, 1.533] | 7.854 [7.358, 8.062] |
| rtx / Original / stock | 3.108 [3.084, 3.164] | 0.786 [0.740, 0.799] | 0.813 [0.804, 0.851] | 3.227 [3.183, 3.323] |
| rtx / Original / far | 7.395 [7.201, 8.068] | 1.278 [1.259, 1.392] | 1.542 [1.527, 1.704] | 7.918 [7.614, 8.466] |
| rtx / Original / whole | 7.641 [7.264, 7.704] | 1.282 [1.232, 1.363] | 1.492 [1.482, 2.063] | 7.802 [7.395, 8.476] |
| amd / Native / stock | 3.178 [3.063, 3.224] | 0.745 [0.734, 0.752] | 0.804 [0.791, 0.822] | 3.218 [3.156, 3.237] |
| amd / Native / far | 7.555 [7.289, 7.636] | 1.298 [1.232, 1.348] | 1.630 [1.617, 1.646] | 7.883 [7.638, 8.021] |
| amd / Native / whole | 7.428 [7.184, 9.006] | 1.283 [1.213, 1.314] | 1.489 [1.485, 1.540] | 7.701 [7.390, 7.903] |
| amd / Original / stock | 3.188 [3.181, 3.249] | 0.797 [0.783, 0.806] | 0.851 [0.845, 0.867] | 3.320 [3.194, 3.743] |
| amd / Original / far | 7.481 [7.213, 7.616] | 1.406 [1.345, 1.487] | 1.615 [1.595, 1.643] | 7.882 [7.675, 8.034] |
| amd / Original / whole | 7.476 [7.420, 8.091] | 1.356 [1.281, 1.489] | 1.568 [1.543, 1.603] | 8.067 [7.466, 8.390] |
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
retained; no samples or pairs are excluded for load. Before the matrix the
driver waited for 60 seconds without a compiler or linker, then sampled for
them every 5 seconds: none appeared in any of the 20 runs. `runs.csv` records
each run's sampled load and compiler flag.

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

`coincident-controls.json` adds 16 full-frame 4K comparisons of the candidate
against its own canonical renderer, on both adapters, Native and Original:
authored coincident pairs (food 2j and 2j+1 share color, position, size and
factory units) with 4,096 foods (serial preparation) and 8,192 foods (scoped
chunks); one coincident pair among 25,000 hashed foods; and the review's
caller-authored pair across the empty late barrier. All match byte for byte.
The reviews' failures on `bed9a259` and `4a49757f` are retained beside them,
with 16 further passing authored controls of menu prefixes, background art and
counts around the threading threshold.

Two explicit offscreen tests compare every prepared byte, draw state and
coverage field against the generic packer, apart from omitted black outlines
and noop slots, and compare rendered pixels.
`coincident_low_bodies_pair_like_the_generic_packer` covers serial and scoped
pairs, a pair, two triples and a run of four around the first chunk boundary,
one pair among 25,000 foods and distinct foods, each with and without a menu
prefix. `caller_pairs_cross_the_empty_low_late_barrier` covers the review's
barrier pair with and without omitted outlines. Both fail on `4a49757f` and
pass on the candidate.

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
test, and the repository guard passed. The three explicit offscreen packing
tests in `canonical_pellets` passed on RTX/Vulkan; the other explicit
GPU/benchmark tests were not run. Independent reviews of `bed9a259` and
`4a49757f` found the pairing differences above; the in-place pairing has not
had a further independent review. No interactive play acceptance is claimed.

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
