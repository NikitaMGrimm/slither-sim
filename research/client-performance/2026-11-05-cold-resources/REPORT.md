# Cold join resources across threads

## Result

The first gameplay pictures of a join paid for several one-time CPU rasters
on the frame thread. On the retained ten-second join at 4K, the first two
gameplay callbacks took 79-86 ms and 42-76 ms (medians by configuration).
This change keeps every byte of output and takes most of that work off the
frame thread: they now take 18-24 ms and 18-42 ms.

- **Fonts.** Decoding each installed face with fontdue takes about 15 ms, and the
  first HUD picture needs Arial regular and bold. Decoded faces are now shared
  process-wide, and those two are decoded on a helper thread when the first text
  rasterizer is created, long before the join. Each file is still decoded once,
  with the same settings. A failed decode leaves the slot empty and is retried,
  as before.
- **Sprite rasters.** A join's first two pictures need 90 and 171 cold recipes
  (food bodies, glows, outlines and snake materials), about 24 and 40 ms
  serially. They are independent, so up to four threads (including the caller,
  as for the pellet pool) now claim them. Results are admitted in index order,
  stopping at the first error as the serial loop did.
- **Minimap background.** Its first build took about 20 ms. Each of its three
  passes depends only on the previous pass, so rows now run on the same
  bounded threads.

Wasm and single-item inputs stay serial. No per-CPU path, intrinsic or
pinning is used; `available_parallelism` can only lower the four-thread cap.

<!-- SUMMARY -->
Runs: 190; pairs: 95; pairs excluded for observed compiler overlap: 48. Accepted-run median machine load: 8.5-77.2%.
<!-- /SUMMARY -->

<!-- TIMING -->
| Adapter / view / quality | Pairs | Frame 60 before → after (median / p95 ms) | Frame 61 before → after (median / p95 ms) | Cold-window max before → after (median / p95 ms) | Art p95 before → after (median / p95 ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| igpu / far / native | 9 | 85.5 / 86.8 → 24.2 / 29.0 | 75.7 / 80.5 → 42.3 / 49.9 | 85.5 / 86.8 → 77.3 / 80.7 | 15.2 / 15.4 → 15.1 / 17.0 |
| rtx / far / native | 10 | 79.8 / 81.1 → 18.1 / 20.8 | 52.9 / 55.0 → 22.7 / 26.3 | 79.9 / 87.6 → 68.3 / 75.5 | 10.6 / 12.2 → 10.4 / 12.1 |
| rtx / far / original | 10 | 79.3 / 80.5 → 18.8 / 19.8 | 51.4 / 53.8 → 22.5 / 24.3 | 79.3 / 80.5 → 69.5 / 75.1 | 10.6 / 11.0 → 10.8 / 12.0 |
| rtx / stock / native | 9 | 79.7 / 84.6 → 18.9 / 20.4 | 41.8 / 42.9 → 18.4 / 21.4 | 79.7 / 84.6 → 51.4 / 58.0 | 7.0 / 8.9 → 7.4 / 8.8 |
| rtx / whole / native | 9 | 78.9 / 82.0 → 18.0 / 21.7 | 62.7 / 64.2 → 31.4 / 37.9 | 78.9 / 82.0 → 62.6 / 66.3 | 8.9 / 10.1 → 9.0 / 9.7 |
<!-- /TIMING -->

Frame 60 is the first gameplay callback (model time 1,000 ms) and frame 61 the
next. The cold window is the first 120 callbacks (two seconds); its maximum
now falls in the menu tail described below. Art p95 is the per-run p95 of
callbacks at 2-7 s, a steady-state guardrail. Each cell gives the median and
p95 across accepted runs, not a confidence interval. Callback wall time
includes the harness's GPU completion wait. These are synchronized offscreen
callbacks, not presented-frame pacing. Frame 61 on AMD still includes its
resource upload (see the batched-upload study).

## Attribution

One phase-instrumented run per arm and configuration (excluded from timing,
several overlapped other workers' compiles) gives exclusive time per span.
Spans that move to helper threads leave the journal, so the candidate column
shows only the caller's share.

| Span, RTX/Vulkan far Native | Baseline f60 / f61 ms | Candidate f60 / f61 ms |
| --- | ---: | ---: |
| Font decode (two faces) | 38.4 / 0 | 0 / 0 |
| Minimap background (exclusive) | 26.6 / 0 | 9.5 / 0 |
| Sprite materialization on the caller | 39.0 / 46.0 | 9.5 / 14.3 |

## What remains

The same diagnostics show the join's remaining long callbacks, which this
change does not address:

- **Menu crossfade and spinner tail** (callbacks 60-101, about 700 ms on this
  schedule). Menu composition expands the retained pellet batch to canonical
  draws whenever a page backdrop is present, so these callbacks cost 50-90 ms
  at far zoom (composition 32-47 ms plus generic submission 21-30 ms). The
  compact path differs from canonical expansion by up to two RGB bytes in
  places, so switching it would not be byte-identical. That needs a separate
  look-approval decision or an exact rewrite of the expansion.
- **First egui paint** (callback 0, the first menu picture): 38-102 ms of the
  harness's offscreen egui paint and wait, before any join.
- **First texture upload on AMD/DX12**: the second gameplay callback's
  resource sync. The separate batched-upload study addresses it.

## Exactness

`image-controls.json` records, for each arm, the full-frame RGBA SHA-256 of
all 600 callbacks of the authored 25,000-food rectangle at factor one, 0.065
and whole-map, in Native and Original, on RTX/Vulkan and AMD/DX12 at 4K
(menus composed, egui omitted in both arms). All 12 configurations match
frame for frame, with identical food census and numeric observation digests.
The CPU-only canonical controls at the same three views match numeric, quad
and census digests with canonical parity active. The unit test
`cold_materialization_matches_the_serial_rasters` compares every authored
catalog raster from `Bank::resources` with the serial materializers.

## Method and access

The restricted community join recording (retained-join-A, SHA-256
`a9e9bbdaf3c8ebc940f9c75ecfda7aa6aff58d09f396119e13ab0ead16d8cc43`) is replayed
locally for timing only. Its frames, positions, names and addresses are not
published. Image controls use only the authored rectangle.

Both arms build from archived sources with the same passive diagnostic overlay
and the retained offline replay driver, release profile, Rust 1.99.0,
`CARGO_TARGET_DIR=D:/CargoTargets/perf-spikes2`, `CARGO_BUILD_JOBS=2`.
`identities.json` binds the revisions, driver and binaries. The measured
candidate is `475e6416` on `60bac3ea`; the delivered commit applies the same
source delta to `5bc7a4c5` (#463), which changes only pellet projection, plus
a `FaceSlot` type alias required by strict Clippy (no behavior change).

Timing runs hold the shared lock and alternate arm order by pair, with
phases off. Load and compiler processes are sampled every 250 ms. A pair is
excluded when either run saw rustc or link; excluded runs stay in `runs.csv`.
Configurations: RTX 5070 Ti/Vulkan at factor one, 0.065 (Native and
Original) and whole-map; AMD integrated/DX12 at 0.065 Native; all 3840x2160.

```text
python -B research/client-performance/2026-11-05-cold-resources/recompute.py --check
```
