# Remaining overlapped-sprite GPU cost

The selected portable change reduces authored crowded GPU pass time while
preserving the CPU picture, source rasters, mip policy, MSAA setting and paint
barriers. The 8 ms default-view target is **not yet met**. This is offscreen GPU
pass evidence, not gameplay FPS, presentation latency or owner acceptance.

The authored scene starts with 30,000 food and 40 deliberately overlapping snakes,
using the previous [sprite-bandwidth study's synthetic driver](../2026-11-02-sprite-bandwidth/probe.patch).
It is not a server-population model. No restricted recording, outside connection,
official art or visible desktop window is used. Baseline: main
`3f2b0bd93e5f0fe824859a3dc0f3fd92477481e1`, including the three ingress fixes.
Physical output: 1920×1080; Native and Original use production `WorldTargetPlan`.
Native uses MSAA4. The sparse 4K control follows the allocation cap and uses
single sampling. No quality setting changes.

Adapter: **AMD Radeon(TM) Graphics**, integrated, DX12, vendor/device
`1002:13c0`, driver `32.0.21001.9024`. Windows, Rust 1.99.0, release profile,
two Cargo jobs, no CPU-specific target flags. Executable/measured-module SHA-256
bindings are in [sources.json](sources.json). Binaries, raw sample CSVs, logs,
compressed authored RGBA histories and process journals remain local-only under
the task's `frame-prep/body-20261102` scratch.

## Profile recorded before production edits

The initial series freezes pictures 60/120/180 and alternates five A/B pairs per
diagnostic, reversing arm order on even pairs: five warm and ten measured repeats
per arm, default and whole-map. Production pass intervals are disjoint. Default
picture 120, ranked median-of-five per-run pass means, resolve-control before arm:

<!-- profile:start -->
| Pass | GPU ms |
| --- | ---: |
| food-glows-coarse | 8.6589 |
| world-before-glows | 8.4452 |
| world-after-glows | 1.6911 |
| food-outlines | 1.4912 |
| overlay | 0.0006 |
<!-- profile:end -->

A separate diagnostic moves the resolve to an otherwise empty load/resolve pass;
its extra pass/store/load is diagnostic overhead. Mask deltas below are causal
diagnostics, **not additive exclusive stage times**: removing geometry can change
blend/cache behavior. The snake-family mask leaves eyes/text/other geometry.
Masks and MSAA1 are diagnostics, not shipping options.

<!-- diagnostics:start -->
| Diagnostic | Before / after GPU median ms |
| --- | ---: |
| no-body | 20.2387 / 20.8574 |
| no-boost | 20.2058 / 20.2891 |
| no-shadow | 20.2682 / 14.2478 |
| no-snakes | 20.1561 / 13.1923 |
| no-food | 20.1856 / 8.0709 |
| resolve-only | 20.1173 / 20.1362 |
| msaa1 | 20.1434 / 14.8733 |
| Isolated resolve pass (after arm) | 1.0268 |
<!-- diagnostics:end -->

Food coarse glows are the largest individual pass. Removing colored body rasters
does not help; removing snake shadow/outline rasters does. The initial suspected
"body shading" explanation is incomplete. [coverage.csv](coverage.csv) retains
six pictures. Default picture 120 has 18.542 screen areas of clipped snake
shadow/outline quads, 2.605 colored-body areas and 281.937 food-glow areas. These
are geometric CPU-quad areas, including transparent pixels and repeated quads
before pairing, not fragment invocations. Selected pictures have **zero boost
glow draws**. Boosted-crowd performance is unverified. The history includes a late
quality transition with no food glows; headlines retain it rather than repeating
only a chosen picture.

## Selected change and discarded attempts

- Skip paired UNORM8 draws only when both opacities are below `0.49 / 255`.
  Each original shader contribution, including alpha, rounds separately to zero.
  Every draw still validates before upload/submission.
- Skip an initial black source-over prefix over the opaque black world clear.
  Later shadows and loaded overlays remain ordered. Omit a wholly black outline
  target/pass, retaining logical admission history for unchanged hysteresis.
- Within existing paired additive food-glow groups, put zero pairs last and group
  by source array layer. Invisible records no longer fragment active runs and
  source locality improves. Source-over and non-paired groups retain their order.
  Actual rounding differences are measured below.
- Paint independent reduced glow targets first, then paint the world's original
  geometry/composites in one MSAA pass, removing the intermediate store/load.
  Existing conservative admission thresholds remain unchanged.

The [current guide](../../../client/docs/sprite-bandwidth.md) explains invariants
and accounting. There is no new target, format, optional device feature, shader,
source raster, blending equation or quality knob.

Exploratory series: five alternating pairs at Native 1080p, complete 181-callback
histories, first 40 callbacks excluded from timing summaries. Each remaining
picture renders once. Values are the median of five per-run medians/p95s; p95
uses nearest rank within each 141-sample run. All successful samples are retained.
The MSAA1 arm changes quality and is diagnostic only. Experiments are not pooled
with selected runs.

<!-- experiments:start -->
| Series / view | Before median / p95 ms | After median / p95 ms | Per-run p95 ranges before / after ms |
| --- | ---: | ---: | --- |
| main-msaa / default | 18.8698 / 19.4075 | 14.7857 / 15.1163 | 19.2061–19.7580 / 15.0829–15.3851 |
| main-msaa / whole-map | 12.1918 / 12.5232 | 10.0410 / 10.3773 | 12.4657–12.6376 / 10.2310–10.4873 |
| r8-trial / default | 18.7632 / 19.1378 | 18.7693 / 19.3438 | 19.0366–19.2528 / 19.1966–19.7180 |
| r8-trial / whole-map | 12.2101 / 12.5975 | 12.2086 / 12.5595 | 12.5057–12.8012 / 12.4969–12.9218 |
| rgb-trial / default | 18.7282 / 19.1844 | 17.0825 / 17.5115 | 19.0590–19.6868 / 17.4520–18.0770 |
| rgb-trial / whole-map | 12.1940 / 12.5604 | 11.1574 / 11.6146 | 12.4969–12.6056 / 11.4612–12.0337 |
| noop-trial / default | 18.7451 / 19.1157 | 11.7305 / 16.2258 | 19.1005–19.2925 / 16.0888–16.5970 |
| noop-trial / whole-map | 12.1955 / 12.4923 | 4.7208 / 6.5634 | 12.4319–12.9521 / 6.5590–6.7115 |
| combined-trial / default | 18.8238 / 19.3124 | 9.2842 / 9.9479 | 19.0112–19.6965 / 9.8782–9.9902 |
| combined-trial / whole-map | 12.1950 / 12.6045 | 4.4892 / 5.2358 | 12.4712–12.6394 / 5.2120–5.2935 |
<!-- experiments:end -->

`r8-trial` rejected one-channel black rasters: no meaningful speedup, so no image
parity qualification. `rgb-trial` combines black-prefix skipping with discarded
RGB-zero/unused-alpha shader work. `noop-trial` restores the original shader and
skips zero pairs but leaves fragmented runs. `combined-trial` adds source-layer
grouping and pass hoisting. Retained patches: [R8](r8.patch), [RGB](rgb.patch),
[uncompacted no-ops](noop.patch). The selected shader is identical to main.
The R8 patch includes the measured `texture_arrays.rs` changes: `R8Unorm`
allocation for alpha-only arrays and alpha-byte packing at every mip upload.
The original retained patch omitted that module; it was present in the measured
source export and binary. Its before/after source hashes are now retained in
`sources.json`, and the study regression reconstructs all three changed R8
modules and checks them against the measured hashes. No timings were rerun or
changed. Reconstruct the initial diagnostic baseline with the previous study's
probe patch and this study's `profile.patch`, then apply `r8.patch` with
`git apply --unidiff-zero`.
A [stable-order variant](stable-order.patch) removed a one-byte difference in two
red channels in one exploratory capture by retaining visible pair order. Its
attempted A/B series is invalid for comparison for the build-cache reason below;
no timing conclusion is claimed from that series.

## Fresh matched comparison

Five fresh pairs per view/policy, odd AB/even BA, separate fresh processes,
identical authored clocks/configuration. Every process prepares/acknowledges 181
callbacks and renders every picture once; the last 141 samples enter quantiles.
Resource sync and calling-thread encoding/submission are retained separately in
per-run CSVs, outside GPU timestamp intervals. The driver waits for each offscreen
result: this is serial GPU-pass time, not asynchronous display throughput. No
hashing, readbacks or per-pass console logging run in headline measurements.

<!-- headline:start -->
| Series / view | Before median / p95 ms | After median / p95 ms | Per-run p95 ranges before / after ms |
| --- | ---: | ---: | --- |
| selected-native / default | 19.0174 / 19.6317 | 9.4696 / 10.1548 | 19.3641–21.1065 / 10.0173–10.2705 |
| selected-native / whole-map | 12.3533 / 12.9161 | 4.5349 / 5.3272 | 12.7800–15.1740 / 5.3086–5.3873 |
| selected-original / default | 13.9675 / 14.4772 | 6.7340 / 7.3720 | 14.2111–15.2706 / 7.1930–7.5011 |
| selected-original / whole-map | 8.7936 / 9.1323 | 3.6867 / 4.2584 | 8.9237–9.5187 / 4.2013–4.3083 |
<!-- headline:end -->

Calling-thread encoding/submission remains a separate bottleneck, particularly
at whole-map zoom. These measurements include buffer staging, pass/draw encoding
and submission; they exclude shared game-frame preparation and GPU completion.
The older exploratory probe queried draw calls after overlay rendering, which
could report zero; only the selected driver's total world+overlay counts are
used here. CPU draw commands and resource contents remain unchanged.

<!-- encoding:start -->
| Policy / view | Before / after CPU encode+submit median ms | Before / after GPU draw-call median |
| --- | ---: | ---: |
| selected-native / default | 8.4673 / 8.6271 | 5617 / 5614 |
| selected-native / whole-map | 19.6349 / 20.3246 | 5632 / 5623 |
| selected-original / default | 8.3206 / 8.6255 | 5617 / 5614 |
| selected-original / whole-map | 19.9791 / 20.2573 | 5634 / 5625 |
<!-- encoding:end -->

CPU load and all concurrent processes are sampled roughly every 500 ms around
each process. No quiet interval was booked, no process stopped, and no successful
sample discarded. Identifying paths/details stay local. Per-run rows retain load
ranges, process/category maxima, compiler overlap and journal hashes. Our required
check build overlaps part of the selected matrix; alternating arms share the load.

<!-- load:start -->
| Series | Host CPU load range % | Concurrent processes range | Compiler overlap runs |
| --- | ---: | ---: | ---: |
| main-msaa | 2–87 | 376–391 | 0 |
| r8-trial | 2–80 | 377–388 | 0 |
| rgb-trial | 3–80 | 375–392 | 0 |
| noop-trial | 4–82 | 375–389 | 0 |
| combined-trial | 4–85 | 375–389 | 0 |
| selected-native | 10–94 | 383–399 | 16 |
| selected-original | 7–90 | 385–395 | 20 |
<!-- load:end -->

The first exported-baseline final-native attempt reused a newer renderer artifact
in the shared Cargo target because copying preserved old source timestamps. Its
apparent baseline matched an optimized experiment, not main. **That entire series
is invalid and excluded.** Its receipts, reduced CSV and build logs remain local
with the failure note. Both selected arms were rebuilt after package-scoped
release cleaning and explicit source timestamp invalidation, then frozen to
distinct executable paths. The rebuilt baseline recovers the independently
measured main range. No selected renderer skips or process failures occurred.
An earlier launcher ran before its executable existed; no measurement occurred.
An unused diagnostic index warning in the combined experiment was fixed in
production. The source timestamps/cache issue does not alter the separately
frozen earlier diagnostic/experiment executables.

## Sparse and picture controls

The previous study's sparse driver uses a single body, one Low-quality
outline/body, or one eligible glow/body. Five alternating process pairs; ten warm
and thirty measured repeats per cell. Every selected sparse render has **one
world pass**, with no outline/glow reconstruction pass added.
Original also retains its ordinary physical-output scaling pass (two total
presenter passes); both Native sizes use one total presenter pass.

<!-- sparse:start -->
| Sparse size / case | Before median / p95 ms | After median / p95 ms |
| --- | ---: | ---: |
| 1920-original / body | 0.2671 / 0.2761 | 0.2673 / 0.2776 |
| 1920-original / low-quality | 0.2682 / 0.2746 | 0.2670 / 0.2737 |
| 1920-original / one-glow | 0.2867 / 0.2914 | 0.2843 / 0.2900 |
| 1920-native / body | 0.3653 / 0.3706 | 0.3643 / 0.3751 |
| 1920-native / low-quality | 0.3671 / 0.3726 | 0.3642 / 0.3720 |
| 1920-native / one-glow | 0.4019 / 0.4113 | 0.4031 / 0.4108 |
| 3840-native / body | 0.0122 / 0.0174 | 0.0125 / 0.0233 |
| 3840-native / low-quality | 0.0131 / 0.0180 | 0.0123 / 0.0174 |
| 3840-native / one-glow | 0.0393 / 0.0420 | 0.0393 / 0.0442 |
<!-- sparse:end -->

Separate correctness runs compare every RGBA byte across all 181 callbacks per
policy/view, without alignment or dropped pictures. Compression/readbacks/hashing
exclude these runs from timing. Whole-history SHA-256, CPU frame/resource/send
digests and differences are in [parity.csv](parity.csv); [pixels.csv](pixels.csv)
binds snapshots 60/120/180. Exact output is claimed only for cells whose complete
hashes match, and isolated no-op controls. Source-layer grouping follows the
existing perceptual budget, not a universal byte-identical raster claim.

<!-- parity:start -->
| Policy / view | Frames | Changed frames / RGBA bytes | Max byte difference | Alpha changed bytes |
| --- | ---: | ---: | ---: | ---: |
| native / default | 181 | 10 / 634 | 1 | 0 |
| native / whole-map | 181 | 10 / 7143 | 1 | 0 |
| original / default | 181 | 9 / 1018 | 1 | 0 |
| original / whole-map | 181 | 23 / 10401 | 1 | 0 |
<!-- parity:end -->

All four full histories contain some one-byte RGB differences; none is claimed
byte-identical. Eleven of twelve selected snapshots match exactly, including both
Native frame-120 PR crops. Native default frame 120 has shared RGBA SHA-256
`8d30304b682149319adaad0b428573111702ed3130ff1c855d6bbf2781cb0875`.
The differing snapshot is Original whole-map frame 60 (two red-channel bytes).
These differences do not alter CPU pictures, resources, protocol/control output,
alpha or quality choices. The owner still decides perceptual acceptance.

The new ignored GPU regression independently checks black-only prefix omission,
later shadows, loaded/transparent overlays, zero/above-boundary pairs and invalid
draw rejection after invisible pairs. Existing layer, crossover, source-mip,
resize-back and in-flight budgets remain unchanged. The artificial load fixture
now gives both compared arms a colored full-screen background: otherwise the
new prefix elimination correctly removes its black-only outline pass and it no
longer exercises reconstruction. Only hoisted pass-count expectations change.
No golden or pixel budget is relaxed. Normal CI ignores GPU tests; they run
explicitly here. Matched authored crops are PR-only attachments, with raw sources
and media manifests retained outside Git.

## Recompute and reproduce

No GPU, recording or third-party Python module is needed to regenerate all marked
numeric tables from the retained per-run summaries:

```powershell
python -B research/client-performance/2026-11-02-overlapping-sprites/test_recompute.py
python -B research/client-performance/2026-11-02-overlapping-sprites/recompute.py --check
```

The patch reconstruction regression also needs Git and the pinned baseline
commit locally. Its per-command line-ending overrides retain the measured LF
bytes without modifying shared Git configuration.

`--write` updates marked tables. Validation requires complete expected cells,
unique IDs matching labels, sample counts, alternating order, finite metrics,
matched CPU-quad counts and executable bindings before aggregation. With local
scratch as `$raw`, use [reduce.py](reduce.py) `$raw` for profile/coverage; use
`--series <name>` for each named timing series, `sparse`, `parity`, or `frozen`.

Export baseline or PR revision to fresh external source directories. Apply the
previous study's `probe.patch` with `git apply --unidiff-zero`, then this study's
[probe.patch](probe.patch). For the initial diagnostic baseline, instead apply
this study's [profile.patch](profile.patch) after the previous patch, then use
`BODY_DIAGNOSTICS=1 GPU_SPRITE_PASSES=1`, warm 5/count 10, no full-history flag.
That driver owns seven alternating masks and isolated resolve. Resolve isolation
cannot be reproduced against the selected production module.

Every Cargo command must use target `D:/CargoTargets/frame-prep`, jobs 2 and
offline mode. Clean `slither-wgpu-slice` for the release profile in that target
before switching exports; invalidate exported source timestamps. Build release
with feature `frame-profile`, examples `gpu_sprite_probe` and `sparse_probe`.
Copy each completed executable to a distinct scratch path before building another
arm. Never time a still-building or overwritten binary. No diagnostic example or
Cargo addition ships. Primary baseline instrumentation includes the disabled
resolve diagnostic; selected production contains none. The module hashes bind
the exact measured sources.

[run-history.ps1](run-history.ps1) runs a labeled Native/Original five-pair matrix
from frozen executables; [run-controls.ps1](run-controls.ps1) runs sparse and
all-frame RGBA controls, plus the final `frozen` pass comparison. Process journals
remain private. Rebuilds cannot reproduce
clock/load noise or executable hashes; retained summaries regenerate the tables.

## Limits and decision

Five additional frozen-picture pairs use the selected binaries, five warm and
ten measured repeats per picture, at Native default. Per-pass console diagnostics
exclude these runs from the headline matrix. The selected after-arm picture 120
ranked pass means show what remains:

<!-- remaining:start -->
| Selected default picture 120 pass | GPU ms |
| --- | ---: |
| world | 8.7606 |
| food-glows-coarse | 0.2944 |
| overlay | 0.0003 |
<!-- remaining:end -->

Ship the portable reduction without changing quality. The remaining default
Native tail exceeds 8 ms. This is **not a hardware lower bound**: further ordered
shadow/overdraw work may help. Front-to-back source-over reordering or dropping
MSAA is not justified here. Colored-body and absent-boost diagnostics do not
support adding body-shader or boost-specific production paths.

Unverified: other adapters/backends, physical display pacing, real gameplay FPS,
new restricted-replay performance, boosted crowds, arbitrary 40–500-snake layouts,
official-art appearance, owner play and owner perceptual acceptance. Automated
budgets prove only their authored workload; normal CI does not run ignored GPU
regressions.

## Executed checks

- `npm run check` passed on Windows, including strict native/Wasm Clippy, core
  and app tests, Canvas/Wasm ABI checks and desktop/Wasm builds, under the task's
  Cargo target/jobs/offline environment. No external game server was contacted.
- Explicit ignored GPU checks passed: retained in-flight world/overlay reuse,
  black-shadow blending, all seven sprite-layer cases, and ordered texture arrays
  with retained revisions. Sprite checks use AMD/DX12; the array check uses the
  AMD Vulkan adapter. This narrow Vulkan check is not whole-scene validation or
  a cross-backend performance comparison.
- Study matrix/negative tests, recompute `--check`, rustfmt and source guards
  passed. GPU tests remain ignored in normal CI. Independent review and owner
  play are not part of these checks.
