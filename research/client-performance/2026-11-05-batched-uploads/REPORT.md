# Batched texture-layer uploads

## Result

When a picture brings new sprites, `Renderer::sync_resources` regroups them
into equal-size texture arrays and writes every new or moved layer. Each
(layer, mip) went through `Queue::write_texture`, and wgpu 30 stages every
such call in its own new buffer. A join's second gameplay picture writes
about 170 new rasters with their mip chains, so it creates on the order of a
thousand staging buffers. On the AMD integrated adapter with DX12, that sync
took 25-53 ms in the phase diagnostics; RTX with Vulkan took about 3 ms.

Native builds now copy all of a sync's new layers into one mapped staging
buffer and record one `copy_buffer_to_texture` per (layer, mip) in a single
submitted encoder. Rows are padded to 256 bytes and regions are aligned to
512 bytes inside the staging copy only; texel bytes, layers, mip levels and
array grouping are unchanged. The copy is submitted during the sync, ahead of
the picture that samples it. A failure while creating a later array now
leaves earlier groups unwritten instead of half-written; the renderer's maps
were already left unchanged on that path. Wasm and single-layer syncs keep
`write_texture`.

Padding can make the staging copy far larger than the texels: a 1-pixel-wide
row takes 256 bytes. A sync whose padded copy would pass the device's maximum
buffer size is split into several staging buffers, submitted in order, and a
single layer too large on its own uses `write_texture`. Every measured sync
fit in one buffer (the earlier code had no split and would have failed), so
the timings above are unaffected.

<!-- SUMMARY -->
Runs: 190; pairs: 95; pairs excluded for observed compiler overlap: 47. Accepted-run median machine load: 8.5-77.2%.
<!-- /SUMMARY -->

<!-- TIMING -->
| Adapter / view / quality | Pairs | Frame 60 before → after (median / p95 ms) | Frame 61 before → after (median / p95 ms) | Cold-window max before → after (median / p95 ms) | Art p95 before → after (median / p95 ms) |
| --- | ---: | ---: | ---: | ---: | ---: |
| igpu / far / native | 9 | 85.5 / 86.8 → 82.0 / 83.5 | 75.7 / 80.5 → 53.9 / 55.6 | 85.5 / 86.8 → 82.0 / 83.5 | 15.2 / 15.4 → 15.2 / 15.4 |
| rtx / far / native | 11 | 79.9 / 86.9 → 77.1 / 89.5 | 53.4 / 58.7 → 50.1 / 57.6 | 79.9 / 109.4 → 77.1 / 106.1 | 10.6 / 14.2 → 11.0 / 12.0 |
| rtx / far / original | 10 | 79.3 / 80.5 → 79.0 / 88.7 | 51.4 / 53.8 → 51.6 / 56.1 | 79.3 / 80.5 → 79.0 / 88.7 | 10.6 / 11.0 → 11.0 / 11.8 |
| rtx / stock / native | 9 | 79.7 / 84.6 → 81.8 / 84.9 | 41.8 / 42.9 → 39.9 / 42.8 | 79.7 / 84.6 → 81.8 / 84.9 | 7.0 / 8.9 → 7.3 / 9.1 |
| rtx / whole / native | 9 | 78.9 / 82.0 → 79.1 / 80.8 | 62.7 / 64.2 → 61.9 / 65.3 | 78.9 / 82.0 → 79.1 / 80.8 | 8.9 / 10.1 → 8.9 / 9.3 |
<!-- /TIMING -->

On AMD/DX12, frame 61 falls by about 22 ms (median 75.7 → 53.9 ms). RTX
with Vulkan, already about 3 ms per sync, shows no change beyond run-to-run
spread, and the cold-window maximum stays at frame 60 because this arm keeps
the font and raster costs addressed by the separate cold-resource change.
RTX with DX12 and the two changes combined were not timed.

Frame 60 is the first gameplay callback and frame 61 the next. The cold
window is the first 120 callbacks. Art p95 is the per-run p95 of callbacks
at 2-7 s, a steady-state guardrail. Each cell gives the median and p95 across
accepted runs. Callback wall time includes the driver's GPU completion wait,
so upload work moved to the GPU queue is still counted. These are
synchronized offscreen callbacks, not presented-frame pacing.

## Attribution

One phase-instrumented run per arm (excluded from timing) measures the
frame-61 resource sync: AMD/DX12 far Native 25.69 → 2.50 ms, RTX/Vulkan
3.14 → 2.94 ms. An earlier baseline diagnostic on the same input measured
53.5 ms for the AMD sync.

## Exactness

`gpu::tests::batched_layer_uploads_match_per_call_writes` (explicit offscreen
GPU test) writes authored random texels into equal arrays through both paths
and reads back every layer and mip, at 1x1, odd, row-aligned and short-chain
sizes. Both match each other and the source bytes.

`image-controls.json` records the full-frame RGBA SHA-256 of all 600
callbacks of the authored 25,000-food rectangle at factor one, 0.065 and
whole-map, in Native and Original, on RTX/Vulkan and AMD/DX12 at 4K (menus
composed, egui omitted in both arms). All 12 configurations match frame for
frame, with identical food census and numeric digests. The CPU-only canonical
controls at the three views also match.

## Method and access

The restricted community join recording (retained-join-A, SHA-256
`a9e9bbdaf3c8ebc940f9c75ecfda7aa6aff58d09f396119e13ab0ead16d8cc43`) is replayed
locally for timing only. Its frames, positions, names and addresses are not
published. Image controls use only authored input.

Both arms build from archived sources with the same passive diagnostic overlay
and the retained offline replay driver, release profile, Rust 1.99.0,
`CARGO_TARGET_DIR=D:/CargoTargets/perf-spikes2`, `CARGO_BUILD_JOBS=2`.
`identities.json` binds the revisions, driver and binaries. The measured
candidate is `b95ada1d` on `60bac3ea`; the delivered commit applies the same
source delta to `a9289bfa`, whose intervening changes (#463 pellet projection,
#464 in-game menu) do not touch texture uploads.

The timing series interleaves three arms (this change, the separate cold
resource change and their common baseline), reversing order on alternate
rounds, with phases off and the shared timing lock held. Load and compiler
processes are sampled every 250 ms. A pair is excluded when either run saw
rustc or link; excluded runs stay in `runs.csv`. Configurations: RTX 5070
Ti/Vulkan at factor one, 0.065 (Native and Original) and whole-map; AMD
integrated/DX12 at 0.065 Native; all 3840x2160.

```text
python -B research/client-performance/2026-11-05-batched-uploads/recompute.py --check
```
