# Crowded-frame food sprite bandwidth

This study isolates the native client's GPU preparation/drawing problem after
the received-snapshot, minimap and pellet-raster changes. Access labels:
**restricted owner-local community recording** for the replay; **original
authored synthetic** for the 30,000-food/40-snake workload. Only bounded aggregate
statistics are retained here. Packets, identities, coordinates, replay images,
raw journals and executables remain owner-local.

## Question and initial profile

The earlier preparation study found a GPU bottleneck, but its finite probe
hardcoded a 960×540 world backing. The product computes a 1500×845 projection
and Original world target at these 16:9 physical outputs. This study uses
`LocalGame::window_viewport` and `WorldPresenter` directly, including Original's
blit or Native's physical world target. Do not substitute the old probe's
timings for this study's control.

Before runtime changes, one offscreen observer series tested MSAA4, MSAA1,
no food glows and half the food glow quads. These variants are diagnostics,
not product quality settings. Each uses the same CPU picture and source mips;
the latter two alter only the diagnostic GPU draw list. Three predetermined
populated snapshots per workload/view have five warm pictures and ten measured
repeats. The adapter is DX12 **AMD Radeon(TM) Graphics**, IntegratedGpu,
vendor/device `1002:13c0`, driver `32.0.21001.9024`.

<!-- BEGIN DIAGNOSTICS -->
| Initial diagnostic / Original | MSAA4 median / p95 ms | MSAA1 median / p95 ms | No food glow median / p95 ms | Half food glow median / p95 ms |
| --- | ---: | ---: | ---: | ---: |
| Replay / default / 960×540 Original | 84.734 / 113.007 | 49.312 / 50.374 | 3.225 / 3.862 | 38.781 / 50.512 |
| Replay / whole map / 960×540 Original | 0.502 / 0.518 | 0.187 / 0.203 | 0.525 / 0.547 | 0.524 / 0.567 |
| Authored / default / 960×540 Original | 50.733 / 68.202 | 41.026 / 47.492 | 7.586 / 7.910 | 29.103 / 39.325 |
| Authored / whole map / 960×540 Original | 45.430 / 47.546 | 42.165 / 43.170 | 4.422 / 4.508 | 29.303 / 30.849 |
<!-- END DIAGNOSTICS -->

Coverage is continuous clipped CPU-picture quad area before any diagnostic mask,
divided by world attachment area. The coverage table uses only the unmasked
MSAA4 pictures; masked-run CSV coverage columns retain that nominal CPU picture.
It includes transparent/zero-contribution source pixels and repeated layers.
Each triangle is clipped to the viewport; axis-aligned rectangles have an
equivalent direct intersection calculation. Multiplying by attachment pixels
and four gives **potential sample-equivalent area**, not measured fragment
invocations, memory transactions or an overdraw counter from the driver.

<!-- BEGIN COVERAGE -->
| Initial maximum glow coverage / Original | Snapshot frame | Food body / outline / glow screen areas | Glow / all food MSAA4 sample-equivalent millions |
| --- | ---: | ---: | ---: |
| Replay / default / 960×540 Original | 365 | 0.498 / 9.304 / 392.287 | 1988.898 / 2038.594 |
| Replay / whole map / 960×540 Original | 120 | 0.000 / 0.000 / 0.000 | 0.000 / 0.000 |
| Authored / default / 960×540 Original | 60 | 1.028 / 6.497 / 281.984 | 1429.661 / 1467.816 |
| Authored / whole map / 960×540 Original | 60 | 0.532 / 3.364 / 190.704 | 966.868 / 986.624 |
<!-- END COVERAGE -->

The ranked initial costs are food glow raster/blend work first, then the
full-resolution food/body work, with blit and HUD much smaller. Turning off
MSAA alone leaves a large glow cost. Removing glows is a strong diagnostic
control; changing texture identities alone cannot remove this covered area.

## Implementation

Padded food outlines use a full-sized single-sample intermediate before the
unchanged body/MSAA path. Soft food glows use half/quarter/eighth targets selected
by projected size and opacity; small glows keep their world sample coverage.
Five-tap cubic reconstruction preserves broad gradients on the coarser target.
Only the contiguous additive glow barrier may regroup GPU work. All source
rasters/mips from #427 are retained; resource arrays group exact physical
dimensions and mip counts. Four indexed corners replace six duplicated
vertices, and exact-geometry additive pairs retain both opacity contributions.
The [current guide](../../../client/docs/sprite-bandwidth.md) owns thresholds,
memory formulas, fallback conditions and the queue-ordering rationale.
Reduced passes retain the world-sized source mip footprint with a negative
`log2(factor)` bias. Without it, the reduced attachment's derivatives select
extra mip reduction before reconstruction, softening overlapping glow energy.

The pair shader separately quantizes the two original UNORM8 contributions.
Simply summing opacity makes individually invisible contributions visible.
Two original half-byte rounding decisions can differ by two output bytes;
the authored sweep covers 18,504 cases at each of sample counts one and four.
Its first one-byte test assumption failed and was replaced by this independently
bounded two-addition allowance. Existing goldens and perceptual budgets were
not regenerated or loosened.

## Matched complete-history method

The primary matrix has five alternating A/B pairs in each of twelve cells:
recorded/authored, default/whole-map, and 960×540 Original, 1920×1080 Original,
1920×1080 Native. Odd pairs run control then candidate; even pairs reverse them.
No quiet interval is reserved and no other process is stopped. All successful
runs are included. CPU load and all process names/paths are sampled around and
during runs; only load ranges and process-category maxima are exported.

The replay reads the complete fixed 6.1-second input at 60 virtual callbacks/s,
without a socket or window. Both peers acknowledge the same prepared pictures.
Every picture is rendered once; summaries select model time at least one second
and at least 10,000 received food keys (302 pictures/callbacks). The authored
workload advances 181 callbacks after constructing its world; the last 141 are
selected. Its toy map, grid and deliberate overlapping snakes stress drawing;
they are not a live-server ecology or a 500-snake capacity test. One late authored
phase has no food glow commands; it remains in both arms.

Per-picture GPU time is the sum of timestamped render/blit/overlay passes.
Resource upload copies, CPU frame preparation, inter-pass CPU supply gaps,
timestamp resolve/readback and GPU completion waits are outside that metric.
The calling-thread sync and renderer preparation/submission intervals are also
retained. They do not include all client advance/planning work. The diagnostic
driver waits for completion to collect timestamps, so neither GPU pass sums nor
launcher elapsed time establish gameplay FPS or input/scanout latency.

Per-run median is the median of selected picture values; p95 is nearest rank.
The report aggregates the five per-run medians/p95s by median, without pooling
quantiles or summing stage quantiles. Run-median ranges retain host drift.
The primary binaries are rebuilt from clean client/app release packages with
the same driver, features, Rust 1.99, thin LTO and two Cargo jobs. The baseline is
main `ffc36bb0`, including #427, #428 and #429; the candidate runtime is
`3556082d`. Full identities are in
[provenance.json](provenance.json).

<!-- BEGIN HISTORY RESULTS -->
| Workload / view / physical output | Before GPU median / p95 ms | After GPU median / p95 ms | After run-median range ms |
| --- | ---: | ---: | ---: |
| Replay / default / 960×540 Original | 65.622 / 109.827 | 4.696 / 6.283 | 4.689–4.918 |
| Replay / default / 1920×1080 Original | 65.914 / 109.712 | 4.847 / 6.410 | 4.840–4.857 |
| Replay / default / 1920×1080 Native | 98.467 / 166.400 | 6.105 / 7.442 | 6.091–6.157 |
| Replay / whole map / 960×540 Original | 0.972 / 1.317 | 1.010 / 1.311 | 0.960–1.064 |
| Replay / whole map / 1920×1080 Original | 1.118 / 1.414 | 1.137 / 1.409 | 1.052–1.220 |
| Replay / whole map / 1920×1080 Native | 1.152 / 1.421 | 1.117 / 1.410 | 1.073–1.163 |
| Authored / default / 960×540 Original | 45.891 / 64.372 | 13.752 / 14.968 | 13.696–14.527 |
| Authored / default / 1920×1080 Original | 45.749 / 63.170 | 13.860 / 14.266 | 13.821–14.194 |
| Authored / default / 1920×1080 Native | 69.557 / 94.688 | 18.940 / 19.507 | 18.784–19.182 |
| Authored / whole map / 960×540 Original | 46.158 / 49.477 | 8.589 / 8.917 | 8.495–8.685 |
| Authored / whole map / 1920×1080 Original | 46.429 / 50.084 | 8.763 / 9.048 | 8.627–8.798 |
| Authored / whole map / 1920×1080 Native | 66.729 / 71.764 | 12.232 / 12.565 | 12.204–12.242 |
<!-- END HISTORY RESULTS -->

## Final pass breakdown

A separate final-binary diagnostic renders the three predetermined snapshots at
1920×1080 Native, five warm pictures and ten measured repetitions per snapshot.
The table sums **means** of disjoint timestamped passes, not stage quantiles.
World passes include remaining geometry and the full-resolution composites;
they do not isolate snake bodies from food bodies or composition. Absent passes
contribute zero. These profiles are excluded from the matched-history table.

<!-- BEGIN FINAL GPU PROFILE -->
| Final Native 1920×1080 diagnostic | Snapshot | World passes incl. composites ms | Glow targets ms | Outline target ms | Blit / HUD ms |
| --- | ---: | ---: | ---: | ---: | ---: |
| Replay / default / 1920×1080 Native | 120 | 2.515 | 1.048 | 0.379 | 0.000 / 0.048 |
| Replay / default / 1920×1080 Native | 240 | 2.495 | 2.117 | 1.682 | 0.000 / 0.045 |
| Replay / default / 1920×1080 Native | 365 | 2.820 | 2.445 | 2.124 | 0.000 / 0.045 |
| Replay / whole map / 1920×1080 Native | 120 | 0.544 | 0.000 | 0.000 | 0.000 / 0.046 |
| Replay / whole map / 1920×1080 Native | 240 | 0.562 | 0.000 | 0.000 | 0.000 / 0.045 |
| Replay / whole map / 1920×1080 Native | 365 | 0.537 | 0.000 | 0.000 | 0.000 / 0.044 |
| Authored / default / 1920×1080 Native | 60 | 8.893 | 8.390 | 1.523 | 0.000 / 0.000 |
| Authored / default / 1920×1080 Native | 120 | 8.967 | 8.373 | 1.567 | 0.000 / 0.000 |
| Authored / default / 1920×1080 Native | 180 | 5.515 | 0.000 | 1.763 | 0.000 / 0.000 |
| Authored / whole map / 1920×1080 Native | 60 | 4.665 | 6.762 | 0.981 | 0.000 / 0.000 |
| Authored / whole map / 1920×1080 Native | 120 | 4.523 | 6.666 | 0.986 | 0.000 / 0.000 |
| Authored / whole map / 1920×1080 Native | 180 | 3.346 | 0.000 | 0.983 | 0.000 / 0.000 |
<!-- END FINAL GPU PROFILE -->

## Retained frozen-picture comparison

An earlier matched matrix repeats three immutable snapshots per workload/view:
replay frames 120/240/365 and authored frames 60/120/180. Each snapshot has five
warmups and twenty measured repeats. These 120 runs are retained separately.
They use the same rendering design but predate removal of a redundant outer
GPU-handle `Arc` needed for strict Wasm Clippy. They are diagnostic evidence,
not relabelled as final-source or changing-frame measurements.

<!-- BEGIN FROZEN RESULTS -->
| Workload / view / physical output | Before GPU median / p95 ms | After GPU median / p95 ms | After run-median range ms |
| --- | ---: | ---: | ---: |
| Replay / default / 960×540 Original | 82.514 / 110.395 | 3.788 / 4.636 | 3.765–3.794 |
| Replay / default / 1920×1080 Original | 82.912 / 110.123 | 3.908 / 4.770 | 3.881–3.950 |
| Replay / default / 1920×1080 Native | 125.133 / 166.399 | 5.534 / 6.862 | 5.390–5.672 |
| Replay / whole map / 960×540 Original | 0.521 / 0.538 | 0.498 / 0.510 | 0.486–0.505 |
| Replay / whole map / 1920×1080 Original | 0.623 / 0.639 | 0.599 / 0.682 | 0.596–0.647 |
| Replay / whole map / 1920×1080 Native | 0.617 / 0.635 | 0.588 / 0.604 | 0.582–0.609 |
| Authored / default / 960×540 Original | 43.301 / 60.548 | 7.736 / 7.989 | 7.687–7.956 |
| Authored / default / 1920×1080 Original | 44.705 / 61.591 | 7.957 / 8.272 | 7.841–8.013 |
| Authored / default / 1920×1080 Native | 67.787 / 93.556 | 11.676 / 12.105 | 11.372–12.152 |
| Authored / whole map / 960×540 Original | 44.829 / 48.991 | 4.808 / 4.980 | 4.770–4.893 |
| Authored / whole map / 1920×1080 Original | 44.750 / 48.085 | 4.952 / 5.057 | 4.926–4.980 |
| Authored / whole map / 1920×1080 Native | 65.464 / 69.380 | 6.391 / 6.501 | 6.353–6.416 |
<!-- END FROZEN RESULTS -->

## Retained pre-boundary-sweep comparison

The first complete-history series used runtime `232bfc39`. A subsequent
authored boundary sweep found that its quarter target blurred small bright
glows beyond the isolated max-five/p95-two-byte budget: up to ten/five bytes.
Expanding the sweep to source index zero also rejected the first size/opacity
gradient guard (six/three bytes). Both failures remain in local logs. The final
guard chooses the coarsest factor 8/4/2 only when the shortest edge spans four
reduced pixels and `max(opacity, paired opacity) * factor / edge <= 0.004`.
It preserves the existing error limits. The original full-history series is
retained below as diagnostic evidence, rather than relabelled as corrected-source
measurements or silently discarded. The final matrix reruns both arms.

<!-- BEGIN PRE-BOUNDARY RESULTS -->
| Workload / view / physical output | Before GPU median / p95 ms | After GPU median / p95 ms | After run-median range ms |
| --- | ---: | ---: | ---: |
| Replay / default / 960×540 Original | 66.026 / 110.033 | 3.421 / 4.636 | 3.389–3.475 |
| Replay / default / 1920×1080 Original | 65.973 / 109.986 | 3.560 / 4.764 | 3.547–3.576 |
| Replay / default / 1920×1080 Native | 99.261 / 166.204 | 4.853 / 6.898 | 4.841–4.914 |
| Replay / whole map / 960×540 Original | 1.081 / 1.317 | 1.001 / 1.307 | 0.976–1.073 |
| Replay / whole map / 1920×1080 Original | 1.201 / 1.424 | 1.176 / 1.420 | 1.147–1.187 |
| Replay / whole map / 1920×1080 Native | 1.181 / 1.415 | 1.148 / 1.390 | 1.063–1.171 |
| Authored / default / 960×540 Original | 46.756 / 65.340 | 7.779 / 8.077 | 7.722–8.086 |
| Authored / default / 1920×1080 Original | 46.709 / 64.574 | 7.908 / 8.173 | 7.836–7.969 |
| Authored / default / 1920×1080 Native | 68.331 / 96.600 | 11.367 / 11.820 | 11.299–11.606 |
| Authored / whole map / 960×540 Original | 46.516 / 49.641 | 4.805 / 4.974 | 4.776–5.095 |
| Authored / whole map / 1920×1080 Original | 46.859 / 49.384 | 4.924 / 5.061 | 4.909–4.997 |
| Authored / whole map / 1920×1080 Native | 67.677 / 72.245 | 6.377 / 6.531 | 6.325–6.408 |
<!-- END PRE-BOUNDARY RESULTS -->

## Retained pre-mip picture comparison

The corrected size/opacity guard alone passed the isolated sweep but accumulated
larger whole-map color drift. Its complete 24-run parity/readback matrix is
retained below. This prompted stopping its timing series after five completed
runs and one deliberately interrupted candidate run; all six receipts are bound
in [interrupted-runs.csv](interrupted-runs.csv). None is a final timing sample.

Higher-precision glow accumulation and cubic reconstruction on all bins did not
reduce that drift; their diagnostic binaries, patches, readbacks and logs remain
local. Correcting the implicit source mip footprint reduced the early authored
whole-map p95 RGB error from 14 to 3 bytes in a subsequent diagnostic. The final
matrix reruns both binaries after this correction. An authored distinct-mip-color
regression checks the original source footprint for all three attachment factors,
with both single and paired contributions.

<!-- BEGIN PRE-MIP PICTURE CHECKS -->
| Picture check / output | Exact core/resource/send digest | RGB max / p95 bytes | RGB RMS range bytes |
| --- | --- | ---: | ---: |
| Replay / default / 960×540 Original | `75846df53ae4defc` | 33 / 8 | 1.556–3.358 |
| Replay / default / 1920×1080 Original | `ba63b5047ff4b8b1` | 33 / 8 | 1.619–3.309 |
| Replay / default / 1920×1080 Native | `ba63b5047ff4b8b1` | 29 / 6 | 1.468–2.751 |
| Replay / whole map / 960×540 Original | `696e1b57cb0985e4` | 0 / 0 | 0.000–0.000 |
| Replay / whole map / 1920×1080 Original | `06163a32960d79e9` | 0 / 0 | 0.000–0.000 |
| Replay / whole map / 1920×1080 Native | `06163a32960d79e9` | 0 / 0 | 0.000–0.000 |
| Authored / default / 960×540 Original | `d5127c0955e212ea` | 19 / 5 | 0.013–1.946 |
| Authored / default / 1920×1080 Original | `fd8de742e7647024` | 18 / 4 | 0.013–1.843 |
| Authored / default / 1920×1080 Native | `fd8de742e7647024` | 19 / 4 | 0.010–1.603 |
| Authored / whole map / 960×540 Original | `f81e81d9e9c70a67` | 40 / 14 | 0.023–5.499 |
| Authored / whole map / 1920×1080 Original | `5cd558dbaddd85f2` | 39 / 14 | 0.023–5.435 |
| Authored / whole map / 1920×1080 Native | `5cd558dbaddd85f2` | 33 / 8 | 0.020–3.100 |
<!-- END PRE-MIP PICTURE CHECKS -->

<!-- BEGIN HOST OBSERVATIONS -->
| Series | Runs retained | Observed compilation | CPU-load snapshot range | Maximum observed browser / Node processes |
| --- | ---: | ---: | ---: | ---: |
| history | 120 | 73 | 3–100% | 2 / 26 |
| pre-boundary | 120 | 105 | 6–100% | 0 / 28 |
| frozen | 120 | 92 | 3–100% | 29 / 29 |
<!-- END HOST OBSERVATIONS -->

## Exactness and picture checks

Each final binary also processes the entire history in a separate parity run
for each cell. Every prepared CPU frame/overlay, source raster byte and sent
command contributes in order to a 64-bit consistency digest. Only renderer
classification hints are omitted; original assets, order, geometry, alpha and
resource pixels remain included. This supports exact-boundary regression tests;
it is not a cryptographic proof or GPU/owner acceptance.

Three predetermined snapshots per cell are read back at physical output size.
The table gives maximum/p95 RGB-component absolute differences and the RMS range
across those three pictures. All components, including unchanged background,
participate. Full per-picture statistics, signed means, differing-component
counts and SHA-256 bindings are in [pixels.csv](pixels.csv). Replay images stay
local. These composite RGB statistics are descriptive; they do not substitute
for the policy's layer-alpha, geometry, exact-controls or lifecycle gates.

<!-- BEGIN PICTURE CHECKS -->
| Picture check / output | Exact core/resource/send digest | RGB max / p95 bytes | RGB RMS range bytes |
| --- | --- | ---: | ---: |
| Replay / default / 960×540 Original | `75846df53ae4defc` | 37 / 4 | 1.337–1.707 |
| Replay / default / 1920×1080 Original | `ba63b5047ff4b8b1` | 37 / 4 | 1.348–1.694 |
| Replay / default / 1920×1080 Native | `ba63b5047ff4b8b1` | 31 / 4 | 1.314–1.723 |
| Replay / whole map / 960×540 Original | `696e1b57cb0985e4` | 0 / 0 | 0.000–0.000 |
| Replay / whole map / 1920×1080 Original | `06163a32960d79e9` | 0 / 0 | 0.000–0.000 |
| Replay / whole map / 1920×1080 Native | `06163a32960d79e9` | 0 / 0 | 0.000–0.000 |
| Authored / default / 960×540 Original | `d5127c0955e212ea` | 17 / 4 | 0.013–1.636 |
| Authored / default / 1920×1080 Original | `fd8de742e7647024` | 17 / 3 | 0.013–1.515 |
| Authored / default / 1920×1080 Native | `fd8de742e7647024` | 17 / 3 | 0.010–1.435 |
| Authored / whole map / 960×540 Original | `f81e81d9e9c70a67` | 22 / 3 | 0.023–1.289 |
| Authored / whole map / 1920×1080 Original | `5cd558dbaddd85f2` | 21 / 3 | 0.023–1.194 |
| Authored / whole map / 1920×1080 Native | `5cd558dbaddd85f2` | 22 / 3 | 0.020–1.170 |
<!-- END PICTURE CHECKS -->

The CPU picture order, geometry, source pixels, protocol decisions and controls
remain exact boundaries. GPU primitive/binding counts may differ under the
existing perceptual policy. Isolated GPU tests preserve minified glows, original
body source/mip ownership, additive contribution rounding, old resource revisions
and resize-back pictures with work in flight. The isolated boundary sweep covers
360 size/opacity/fractional-position cases across three source sizes, without
changing pre-existing goldens. Whole-picture RGB differences are
descriptive raster evidence, separate from the shared-core alpha/geometry budgets
and from owner perceptual acceptance.

## Verification

- Full Windows `npm run check` passes at runtime `3556082d`: JavaScript, shared
  core/native regressions, strict native/profiling/Wasm Clippy, desktop build,
  Canvas Wasm ABI checks and browser WebGPU Wasm build/bindings. The commands use
  the dedicated target, two Cargo jobs, offline Cargo and the pinned CLI.
- Seven explicitly enabled offscreen GPU regressions pass: additive rounding,
  small/body-layer preservation and resize/reuse, 360 boundary cases, six
  distinct-mip footprint paths, array binding/revision restoration, and the two
  original pellet-raster quality checks. Existing goldens and budgets stay intact.
- The final full-history digests and image readbacks are reported above. Browser
  runtime, physical display/input, outside-server play and owner perceptual
  acceptance are unrun; no gameplay pacing or end-to-end frame-budget claim is made.
- The stdlib recompute negative checks reject missing rows/cells, duplicate and
  mislabeled IDs, wrong executable bindings, incomplete diagnostics, altered
  report blocks and mismatched parity digests.

## Attempts and limitations

- Texture arrays alone did not materially improve the original fill bottleneck.
  Grouping the additive glow barrier later reduced tens of thousands of binding
  runs, but did not by itself remove the remaining shaded area.
- Discarding sub-byte additive fragments did not materially improve GPU time.
- An analytic radial shader was slower than the retained sampled source path.
- A single coarse-target size gate sent too many growing glows back through
  MSAA4. Separate half/quarter/eighth bins retain detail without that early replay
  spike; the later gradient guard corrects the isolated small-bright-glow failure.
- Single-sampling food bodies changed edges too much and was rejected. Only
  padded outlines use that early intermediate; #427 body sampling stays intact.
- Nine-tap reconstruction was more expensive than the retained five-tap
  approximation. Raw exploratory binaries, CSVs, images and patches remain local;
  they are excluded from primary performance tables.
- The pre-mip full-history parity/readback run exposed accumulated color drift
  missed by isolated tests. Higher-precision accumulation and broader cubic
  reconstruction were rejected; source-footprint correction is retained. The
  timing interruption and all earlier successful/failed runs remain documented.
- The first full check could not find the pinned wasm-bindgen CLI on PATH. A
  per-process absolute 0.2.129 path fixed it. Strict Wasm Clippy then caught the
  redundant outer `Arc`; cloned wgpu handles retain native Send ownership and
  browser-local ownership without a lint suppression.

The remaining costs include crowded snake-body/boost overdraw and CPU
advance/planning/preparation. This is one AMD iGPU and two bounded offline
workloads. It does not establish a full 16.7 ms frame budget, smooth owner play,
official/mod superiority, stock-art acceptance, browser runtime performance or
other-iGPU/discrete-GPU results. No outside server or official website was used.

The selected replay whole-map window has no food quads under the existing LOD;
its low GPU times do not test crowded glow drawing. The authored whole-map
workload supplies that stress case. The authored default scene deliberately
overlaps 40 snakes and does not meet the 8 ms target after this change. Its
remaining cost is retained, rather than treating the replay result as universal.

## Recompute

From this directory, Python 3.11+ and only the standard library:

```sh
python recompute.py --check
```

The script validates complete workload/view/resolution/pair/arm sets, unique
IDs and label consistency, chronological alternating arms, executable hashes,
callback/sample selections and every generated table. `--write` replaces only
marked generated blocks. [reduce_runs.py](reduce_runs.py) reduces the owner-local
journals into the adjacent per-run summaries; raw inputs are not distributed.

The authored workload needs no external recording. With a fresh external output
directory, build and run from the repository root (the `unused` argument is not
read in synthetic mode):

```powershell
$env:CARGO_TARGET_DIR='D:/CargoTargets/frame-prep'
$env:CARGO_BUILD_JOBS='2'
cargo build --offline --locked --release -p slither-wgpu-slice --features frame-profile --example gpu_sprite_probe
$env:GPU_SPRITE_FULL='1'
& "$env:CARGO_TARGET_DIR/release/examples/gpu_sprite_probe.exe" synthetic unused D:/new-output/scene.csv 0 1920 1080 native 4 all 0 1
```

Use zoom `-120` for whole map; use `original` for Original. The driver explicitly
selects the AMD/DX12 adapter and fails if it is absent. That measurement selection
does not introduce an adapter-specific production path. Preserve both executables
before measuring and alternate five pairs without rebuilding between arms.
