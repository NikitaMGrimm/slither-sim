# Ordered bundles for steady far-zoom frames

At 0.065 in the authored 100-bot Battledome, RTX/Vulkan Native 4K submission
p95 falls from **19.014 to 16.374 ms** (median of five shorter matched pairs,
13.9%). Every pair improves submission p95, by 0.209–12.698 ms. CPU-dispatch
p95 moves from 25.083 to 22.249 ms. This is a measured component win, not
60/240 Hz or gameplay pacing acceptance. The original long matrix is retained
as inconclusive; other cells have the limits described below.

Native retained-food gameplay now uses the same bounded parallel command recording
as canonical join pictures. The original vertex chunks, shaders, draw ranges and
primitive order are retained. Up to four participants record independent bundles;
the caller executes them in chunk order. Fewer than four intersecting chunks and
Wasm keep direct serial recording. No CPU-specific instructions or affinity apply.

## Profile and selection

One separate instrumented run per adapter preceded the change. These runs are
excluded from the timing comparison. Each cell contains 30 warmed pictures.
Preparation stage p95 values are not additive, nor are CPU and GPU timestamps.
The profile encountered heavy machine load, retained in the external load journal.

<!-- PROFILE -->
| Adapter / bots / view | Prepare / HUD / sync / submit p95 ms | GPU p95 ms | CPU stages ranked by p95 ms |
| --- | ---: | ---: | --- |
| vulkan / 100 / mid | 50.740 / 2.276 / 0.056 / 103.927 | 2.276 | surroundings_end: 42.916, bodies_end: 23.334, names_end: 11.277, curves_end: 4.823 |
| vulkan / 100 / out | 23.795 / 2.771 / 0.051 / 62.010 | 1.791 | surroundings_end: 25.221, names_end: 14.723, bodies_end: 7.326, curves_end: 3.039 |
| vulkan / 500 / mid | 66.075 / 6.234 / 0.050 / 43.414 | 0.081 | names_end: 26.412, food_end: 24.404, curves_end: 18.628, bodies_end: 13.834 |
| vulkan / 500 / out | 77.102 / 4.117 / 0.044 / 31.824 | 0.068 | names_end: 32.145, curves_end: 19.635, bodies_end: 18.433, resources_end: 6.655 |
| dx12 / 100 / mid | 17.343 / 2.474 / 0.058 / 88.874 | 5.768 | surroundings_end: 64.132, bodies_end: 9.725, names_end: 9.641, curves_end: 3.133 |
| dx12 / 100 / out | 35.368 / 9.876 / 0.049 / 93.429 | 5.752 | surroundings_end: 48.864, names_end: 20.219, bodies_end: 6.047, curves_end: 3.984 |
| dx12 / 500 / mid | 86.100 / 5.472 / 0.068 / 62.305 | 1.422 | names_end: 35.564, bodies_end: 28.471, food_end: 26.659, curves_end: 21.022 |
| dx12 / 500 / out | 63.886 / 3.428 / 0.077 / 25.388 | 1.847 | names_end: 31.352, bodies_end: 17.101, curves_end: 13.432, food_end: 4.292 |
<!-- /PROFILE -->

1. **Battledome command recording and surrounding packing** dominate the CPU.
   There are approximately 22,500 draw calls at far zoom, while the RTX world GPU
   work is much smaller. `surroundings_end` includes serial projection/packing;
   the following `submit_end` includes recording and submission. The existing
   bundle recorder was gated to canonical join food, excluding compact gameplay.
   Extending that gate is the selected exact change.
2. **The normal-map preparation path** retains pressure handling. `curves_end`
   covers curve/face/antenna state preparation, `bodies_end` body passes including
   shadows and boost glows, and `names_end` nickname rasters plus antenna emission
   and insertion. These are combined scopes, not isolated function timings.
   Repeated antenna insertion and surrounding vertex packing remain candidates
   for separate exact work; no speedup for those proposals is claimed.
3. **Pellets**: `food_end` is food selection/planning. For the compact Battledome
   path `pellet_prepare_end` measures retained instance projection and upload;
   the normal pressure path uses generic food commands. GPU glow/world/blit/HUD
   timestamps remain in the raw frames. Shared passes do not isolate every
   snake effect. No food/body/glow cutoff is introduced.
4. **Minimap and HUD**: `minimap_end` is the minimap append; `hud_ns` is separate
   HUD composition. The minimap/HUD share the physical overlay GPU pass. Egui
   does no painting during ordinary gameplay; a separate 4K menu-only probe
   records warm preparation and paint-plus-wait costs in `profile.csv`. At p95,
   those separate prepare/paint-plus-wait spans are 0.043/0.233 ms on RTX
   and 0.111/1.556 ms on AMD; these include synchronization and are not pure GPU times. A menu
   and gameplay HUD are never composed together by these controls.

An exploratory bundle-all-frames pilot also included the generic pressure path.
It matched all 72 image pairs but did not show a consistent generic-path win;
that extension was rejected. Its executable, logs, images and load remain local.
The delivered gate includes retained-food frames only. No pixel-changing trial
was needed or shipped.

## Authored workloads and method

Base: `eaac7df723f5509b262870b6a089ab0763497d15`, Rust 1.99.0 release/thin LTO,
Windows, Ryzen 7 9800X3D. RTX 5070 Ti uses Vulkan; AMD Radeon integrated graphics
uses DX12. Physical output is 3840×2160, DPR 1. Original uses the capped 1500×845
world and requested MSAA4; Native uses the production 4K policy (effective 1x).
All face/bulb/body materials are built-in procedural rasters; no image file,
window, socket or outside server is opened.

- Battledome: 100 bots plus the own snake, the configured pit-food model, authored
  circular 26-anchor bodies, half boosting, 20 procedural rival antennas.
- Normal map: 500 bots plus own, official-model radius for 501 seats and 3× food
  density. The corresponding full target exceeds the production ingress cap.
  This control retains 65,535 foods in a central subscribed disk at that density,
  rather than raising the contract or claiming the complete target is resident.
  The pressure policy retains its normal food/detail selection and rival antenna
  stroke suppression; 100 rival antenna states/bulbs are configured.
- Each view starts from the same frozen authored ingress/animation state: factor
  one, factor 0.065 through real wheel input, and the whole-map endpoint. Camera
  preparation is settled with 110 pictures, followed by 10 rendered warmups and
  30 measured pictures. Camera draw-clock evolution is retained. Across the
  original timing matrix, the far/default camera-scale ratio at matching
  picture indices is 0.065 within the CSV rounding bound (error below 1e-10).
- Five process runs per side interleave A/B and B/A by round. Every successful
  run is retained; no load-based exclusion or fast-frame selection applies.
  `cpu_ns` is prepare + HUD + sync + submit for each picture, excluding timestamp
  readback, GPU completion waits, logging and PNG encoding. It is CPU dispatch
  cost, not a callback wall time or presented cadence. GPU pass times are separate.
  Per-run p95 is sorted index floor((N−1)×0.95). Tables give medians of five
  run quantiles and the full run range; the selected CSV retains each quantile.

## Interleaved timing

The initial matrix is **inconclusive for p95 adoption**: normal machine load
ranges from 54.5% to 100.0% across run medians and the unchanged generic normal-map
path moves substantially too. These are retained observations, not a causal
speedup estimate. A shorter matched-cell experiment below is separate and was
planned after examining this matrix; its samples are not pooled here.

<!-- TIMING -->
| Adapter / bots / view | CPU total p95 before → after ms | Submit p95 before → after ms | Before / after run p95 range ms |
| --- | ---: | ---: | ---: |
| vulkan / 100 / mid | 52.367 → 45.200 | 44.019 → 35.537 | 36.580–163.369 / 28.154–114.285 |
| vulkan / 100 / out | 57.335 → 39.870 | 45.727 → 31.210 | 34.095–177.982 / 26.747–136.285 |
| vulkan / 500 / mid | 38.546 → 31.008 | 14.233 → 10.967 | 27.424–82.337 / 27.729–50.369 |
| vulkan / 500 / out | 69.603 → 78.091 | 21.661 → 22.997 | 57.660–127.419 / 74.181–219.475 |
| dx12 / 100 / mid | 54.163 → 51.934 | 42.742 → 39.270 | 36.194–67.038 / 35.856–208.750 |
| dx12 / 100 / out | 47.612 → 55.494 | 37.683 → 37.138 | 26.836–75.878 / 42.357–163.975 |
| dx12 / 500 / mid | 33.466 → 80.588 | 11.277 → 25.616 | 20.063–43.669 / 27.689–109.528 |
| dx12 / 500 / out | 66.391 → 101.965 | 21.863 → 29.001 | 40.046–110.410 / 56.178–115.838 |
<!-- /TIMING -->

These authored controls do not establish 60 Hz or 240 Hz gameplay acceptance.
The unchanged generic 500-bot path and machine load can dominate the remaining
cost. The 16.7 ms and 4.17 ms budgets remain separate from offscreen dispatch.

## Shorter matched cells

The supplemental experiment selects one Native scene/view/backend per process,
then immediately runs the other arm of the same cell, reversing order each round.
It uses the same 10/30 render counts and five pairs, without load exclusions.
`cells.csv` retains all results and per-process machine load. The generic
500-bot cells remain unchanged-code controls. Raw cell load/process files,
failed attempts, and sealed cell executables remain beside the original matrix.

The 80 short processes span 20.1–83.1% median machine CPU load.

<!-- CELLS -->
| Adapter / bots / view | CPU p95 before → after ms | Submit p95 before → after ms | Submit paired p95 delta range ms |
| --- | ---: | ---: | ---: |
| vulkan / 100 / mid | 25.083 → 22.249 | 19.014 → 16.374 | -12.698 to -0.209 |
| vulkan / 100 / out | 22.294 → 22.439 | 17.145 → 16.236 | -2.353 to 0.791 |
| vulkan / 500 / mid | 17.199 → 16.015 | 6.222 → 6.070 | -3.713 to 0.145 |
| vulkan / 500 / out | 48.129 → 46.492 | 13.678 → 13.983 | -0.554 to 0.740 |
| dx12 / 100 / mid | 26.051 → 25.113 | 21.381 → 19.243 | -3.447 to 1.241 |
| dx12 / 100 / out | 27.582 → 26.832 | 21.829 → 20.743 | -12.549 to -0.803 |
| dx12 / 500 / mid | 20.829 → 22.224 | 8.010 → 8.647 | 0.071 to 2.870 |
| dx12 / 500 / out | 47.085 → 47.513 | 14.634 → 14.568 | -3.611 to 3.621 |
<!-- /CELLS -->

RTX/far and AMD/whole submission p95 improve in all five matched pairs. RTX/whole
and AMD/far paired ranges cross zero; they remain inconclusive. Generic 500-bot
rendering uses the same algorithm before/after. Its observed AMD/far submission
p95 increases by 0.637 ms at the median, with positive deltas in all five pairs;
no gain or refresh-budget acceptance is claimed for that path. Ambient load and
unmodified controls still move, so the tables do not establish a universal frame
p95 reduction. Remaining normal-map name/antenna insertion and generic packing
are the next ranked exact opportunities, without measured speedup claims here.

## Exactness, provenance and reproduction

`image-controls.json` records full physical RGBA8 SHA-256 identities at rendered
pictures 10, 25 and 39 for two scenes × three views × Original/Native × two GPUs.
All 72 before/after pairs match, with zero changed bytes and zero maximum
byte difference. Readbacks include the world, minimap and gameplay HUD. Egui is
inactive in these gameplay images; its separate diagnostic never includes a HUD.
This is cross-revision identity on the same renderer, not Canvas parity,
interactive appearance acceptance or a universal pixel guarantee.

`identities.json` binds the sealed executables and source snapshots. The measured
runtime and final runtime differ only in comments and test-only code;
legacy diagnostic output naming was also restored outside the far-zoom mode.
The first Windows source archive used cp1252 for a non-ASCII comment; final
source restores that original spelling. Corrected UTF-8 archives were written
separately after reconstructing and verifying every original source byte hash;
the original archives remain unchanged. No compiled expression changed.
Complete binaries, source snapshots, patches, frame/phase CSVs, images, machine
load, process snapshots and failed build attempts remain under the task's raw
`D:/slither-scratch/client-lane-20261105/perf/far-zoom-p95/` directory. No addresses
or external artwork are committed. Each GPU run acquires the shared timing lock
atomically with purpose, UTC start and PID, waits for a live recent lease, and
releases its own token in `finally`; each run has a 55-minute timeout.

Build each frozen side with command-local `CARGO_TARGET_DIR=D:/CargoTargets/far-zoom-p95`
and `CARGO_BUILD_JOBS=2`, using `cargo build --offline --release --locked -p
slither-wgpu-slice --features frame-profile --example frame_perf`. Retain its
executable. Run `frame_perf --battledome-presented NEW_DIRECTORY` with
`FRAME_PERF_FAR_ZOOM=1`; set `FRAME_PERF_VULKAN=1` for RTX, unset it for AMD/DX12.
Use `FRAME_PERF_PHASES=1` only for separate diagnostics; `FRAME_PERF_NO_PNG=1`
omits readbacks for timing. Keep the other workload/trace overrides unset.

```text
uv run --no-project python research/client-performance/2026-11-05-far-zoom-bundles/recompute.py --check
```

## Validation

Four focused draw-run tests and both diagnostic/output regressions passed.
The explicit offscreen `partial_chunk_bundles_preserve_pixels_and_call_counts`
regression passed on RTX/Vulkan and AMD/DX12: mixed source-over/additive draws,
more than five chunks, full and partial chunk ranges, identical rendered bytes
and draw counts. The image is checked to contain varied color output.
Full `npm run check` passed, including native/Wasm strict Clippy, desktop/Wasm
builds, tests and generated bindings. It used command-local
`CARGO_PROFILE_TEST_OPT_LEVEL=1`, `CARGO_PROFILE_DEV_DEBUG=0`, the required target
and two jobs; these settings do not change the sealed release timing binaries.
Retained failed attempts include two corrected test/fixture Clippy lints and one
unchanged startup/cancellation-test failure, which passed in isolation and the
final full run. Complete logs remain in scratch. No independent review or
interactive play acceptance is claimed.
