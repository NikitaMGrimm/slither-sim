# Retain serialized pellet upload slots

The retained renderer already computes one 96-byte instance per visible pellet.
It then serializes the entire instance mirror into a new byte vector every frame,
including culled slots. Retain the little-endian bytes themselves: serialize only
rewritten slots and upload the existing flat byte slice. No shader, arithmetic,
source grouping, draw order, animation, culling or upload contents are changed.

This is a small CPU reduction, not a 240 Hz or owner-play acceptance result.
The primary case is RTX 5070 Ti / Vulkan at 3840x2160. CPU stages remain well above
the 4.167 ms refresh budget. The AMD integrated GPU / DX12 cases are guardrails.

## Method and boundaries

- Baseline: main `c58fc66b4057bc5b8a3c2338a19113151325e112`. This comparison
  does **not** include the separate [projection specialization PR #447](https://github.com/NikitaMGrimm/slither-sim/pull/447).
  Do not add reductions across studies as if they were a matched combined run.
- Release builds, same retained local input and passive instrumentation, Rust
  1.99.0, jobs 2, target on D:. No live server connections, visible windows,
  adapter-default changes, CPU-specific instructions or thread pinning.
- Replay: 600 virtual 60 Hz callbacks / ten model seconds, 29,044 envelopes;
  art summaries use callbacks 120-419 ([2,7) model seconds). Replay runs as fast
  as possible and does not reproduce the display's Fifo wait or presented cadence.
  Callback time includes ingress, advancement, native menu/egui, preparation,
  resource sync, submit and the diagnostic GPU query wait. It is not presented FPS.
- Adjacent A/B pairs alternate order by pair; quality/configuration order also
  alternates. Collection stops at five complete compiler-free pairs per case.
  All attempts remain in [runs.csv](runs.csv), including the initial four-run
  `mirror` pilot before serializer specialization. That pilot is retained but
  omitted from the adopted comparison; it used a different binary.
- System CPU and process samples are recorded throughout each run, including
  cargo, rustc/link and test processes. Only observed compiler/linker overlap
  excludes a run; either excluded arm excludes the pair from aggregates. Other
  contention stays included. There was no booked quiet window. Individual accepted
  rows from incomplete pairs remain retained; tables use complete accepted pairs.
- The host has eight physical / sixteen logical CPU cores. Results describe this
  shared Windows machine and this recorded workload, not all iGPUs or CPUs.

All GPU figures use per-pass diagnostic timestamps, including every glow and
world pass. CPU submit contains pellet preparation, wgpu/driver calls and encoding;
pellet preparation is a subset, so these columns must not be added together.
CPU stages here sum poll, ingress, redraw, resource sync and submit; menu/egui
composition/painting and query waits are included only in callback wall time.
Moving serialization into slot updates moves some work into the pellet-math
timer; the removed packing timer alone overstates the net improvement.

## Results

<!-- BEGIN RESULTS -->
| Configuration / art metric | Before ms | After ms | Paired reduction range |
| --- | ---: | ---: | ---: |
| rtx 3840x2160 original / callback p50 | 8.390 | 7.893 | -0.4 to +9.2% |
| rtx 3840x2160 original / callback p95 | 11.421 | 10.777 | -1.3 to +10.3% |
| rtx 3840x2160 original / CPU stages | 7.973 | 7.474 | -0.8 to +9.7% |
| rtx 3840x2160 original / pellet preparation | 4.709 | 4.204 | +3.1 to +14.6% |
| rtx 3840x2160 original / CPU submit | 5.319 | 4.808 | +2.5 to +13.3% |
| rtx 3840x2160 original / GPU | 0.224 | 0.224 | -0.5 to +1.0% |
| rtx 3840x2160 original / 10 s callback p50 | 7.901 | 7.407 | +2.5 to +9.7% |
| rtx 3840x2160 original / 10 s callback p95 | 12.554 | 12.048 | -6.4 to +9.8% |
| rtx 3840x2160 native / callback p50 | 7.975 | 7.590 | -1.0 to +9.1% |
| rtx 3840x2160 native / callback p95 | 11.251 | 10.480 | -3.9 to +11.3% |
| rtx 3840x2160 native / CPU stages | 7.498 | 7.106 | -0.3 to +8.8% |
| rtx 3840x2160 native / pellet preparation | 4.513 | 4.126 | +7.5 to +10.6% |
| rtx 3840x2160 native / CPU submit | 4.917 | 4.540 | +6.6 to +9.0% |
| rtx 3840x2160 native / GPU | 0.243 | 0.242 | -0.3 to +0.5% |
| rtx 3840x2160 native / 10 s callback p50 | 7.671 | 7.198 | +1.6 to +8.1% |
| rtx 3840x2160 native / 10 s callback p95 | 12.463 | 11.639 | -12.7 to +17.0% |
| igpu 3840x2160 original / callback p50 | 15.293 | 14.964 | +2.2 to +12.0% |
| igpu 3840x2160 original / callback p95 | 18.308 | 18.035 | +1.5 to +13.4% |
| igpu 3840x2160 original / CPU stages | 8.322 | 7.767 | +3.9 to +16.9% |
| igpu 3840x2160 original / pellet preparation | 4.685 | 4.235 | +8.1 to +17.9% |
| igpu 3840x2160 original / CPU submit | 5.457 | 4.973 | +6.6 to +16.4% |
| igpu 3840x2160 original / GPU | 5.472 | 5.439 | -1.0 to +2.4% |
| igpu 3840x2160 original / 10 s callback p50 | 14.455 | 14.066 | +2.0 to +11.4% |
| igpu 3840x2160 original / 10 s callback p95 | 19.501 | 19.458 | -1.6 to +12.4% |
| igpu 3840x2160 native / callback p50 | 18.545 | 18.231 | +0.5 to +3.3% |
| igpu 3840x2160 native / callback p95 | 20.939 | 21.420 | -6.6 to +3.4% |
| igpu 3840x2160 native / CPU stages | 8.068 | 7.760 | +0.7 to +7.7% |
| igpu 3840x2160 native / pellet preparation | 4.587 | 4.199 | +6.8 to +11.0% |
| igpu 3840x2160 native / CPU submit | 5.154 | 4.763 | +5.8 to +10.1% |
| igpu 3840x2160 native / GPU | 8.298 | 8.294 | -0.7 to +0.2% |
| igpu 3840x2160 native / 10 s callback p50 | 17.007 | 16.560 | -0.4 to +4.3% |
| igpu 3840x2160 native / 10 s callback p95 | 23.196 | 23.538 | -5.3 to +1.5% |
| igpu 1920x1080 original / callback p50 | 14.952 | 14.242 | +1.4 to +5.5% |
| igpu 1920x1080 original / callback p95 | 17.697 | 17.021 | +1.0 to +7.4% |
| igpu 1920x1080 original / CPU stages | 8.252 | 7.654 | +4.3 to +9.5% |
| igpu 1920x1080 original / pellet preparation | 4.635 | 4.229 | +8.2 to +11.8% |
| igpu 1920x1080 original / CPU submit | 5.358 | 4.932 | +7.1 to +9.7% |
| igpu 1920x1080 original / GPU | 4.932 | 4.950 | -1.6 to -0.1% |
| igpu 1920x1080 original / 10 s callback p50 | 14.050 | 13.264 | +1.9 to +6.0% |
| igpu 1920x1080 original / 10 s callback p95 | 18.370 | 18.245 | +0.2 to +4.8% |
| igpu 1920x1080 native / callback p50 | 15.023 | 14.436 | +2.0 to +8.0% |
| igpu 1920x1080 native / callback p95 | 18.124 | 17.223 | -0.0 to +9.1% |
| igpu 1920x1080 native / CPU stages | 8.237 | 7.638 | +3.9 to +12.2% |
| igpu 1920x1080 native / pellet preparation | 4.674 | 4.205 | +7.4 to +17.5% |
| igpu 1920x1080 native / CPU submit | 5.319 | 4.836 | +6.8 to +15.7% |
| igpu 1920x1080 native / GPU | 5.114 | 5.110 | -0.8 to +1.6% |
| igpu 1920x1080 native / 10 s callback p50 | 14.263 | 13.997 | +1.2 to +5.0% |
| igpu 1920x1080 native / 10 s callback p95 | 19.268 | 19.105 | -6.9 to +6.5% |

rtx 3840x2160 original: median requested allocation per art callback 23554053 to 20625477 bytes.

rtx 3840x2160 original baseline: 5 runs; callback p50 range 8.011 to 8.547 ms; load 4.9 to 24.8%; cargo/test samples 0/0.

rtx 3840x2160 original mirror-inline: 5 runs; callback p50 range 7.755 to 8.046 ms; load 3.4 to 38.2%; cargo/test samples 0/0.

rtx 3840x2160 native: median requested allocation per art callback 23326731 to 20400075 bytes.

rtx 3840x2160 native baseline: 5 runs; callback p50 range 7.841 to 8.774 ms; load 6.0 to 48.9%; cargo/test samples 0/0.

rtx 3840x2160 native mirror-inline: 5 runs; callback p50 range 7.498 to 8.546 ms; load 3.8 to 59.9%; cargo/test samples 0/0.

igpu 3840x2160 original: median requested allocation per art callback 23553121 to 20624545 bytes.

igpu 3840x2160 original baseline: 5 runs; callback p50 range 15.033 to 17.245 ms; load 3.7 to 47.4%; cargo/test samples 0/0.

igpu 3840x2160 original mirror-inline: 5 runs; callback p50 range 14.338 to 15.181 ms; load 3.4 to 38.8%; cargo/test samples 0/0.

igpu 3840x2160 native: median requested allocation per art callback 23325803 to 20399147 bytes.

igpu 3840x2160 native baseline: 5 runs; callback p50 range 18.118 to 18.971 ms; load 2.7 to 49.4%; cargo/test samples 0/0.

igpu 3840x2160 native mirror-inline: 5 runs; callback p50 range 17.887 to 18.835 ms; load 3.8 to 64.0%; cargo/test samples 16/0.

igpu 1920x1080 original: median requested allocation per art callback 23553121 to 20624545 bytes.

igpu 1920x1080 original baseline: 5 runs; callback p50 range 14.277 to 15.320 ms; load 4.1 to 42.2%; cargo/test samples 2/0.

igpu 1920x1080 original mirror-inline: 5 runs; callback p50 range 14.071 to 15.013 ms; load 2.3 to 48.9%; cargo/test samples 0/0.

igpu 1920x1080 native: median requested allocation per art callback 23464918 to 20538262 bytes.

igpu 1920x1080 native baseline: 5 runs; callback p50 range 14.545 to 16.499 ms; load 1.6 to 57.7%; cargo/test samples 0/0.

igpu 1920x1080 native mirror-inline: 5 runs; callback p50 range 14.251 to 15.186 ms; load 4.8 to 37.6%; cargo/test samples 0/0.

All retained runs: 100; accepted 72; excluded 28.
<!-- END RESULTS -->

The remaining cost is dominated by unchanged pellet projection/math and food
advancement on the CPU; GPU work dominates the slower iGPU cases. This patch
removes a temporary allocation/pass, not per-visible-pellet animation or the
full GPU upload. A native chunk prototype is a separate investigation.

The 4K Native iGPU callback p95 did not improve: 20.939 to 21.420 ms despite
lower median preparation cost and essentially unchanged GPU medians. This
guardrail establishes neither a tail improvement nor a stable presented cadence;
the remaining tail attribution is unresolved. The workload ran amid recorded
background contention, including cargo without observed compiler work in some
accepted runs. It would be misleading to claim every latency quantile improved.

## Parity and validation

All ten image comparisons have 600 callbacks and zero skips: retained replay
on RTX/Vulkan 4K and AMD/DX12 4K/1080p in both qualities, plus an authored 25,000
pellet rectangle on both adapters at 4K in both qualities. Every complete rendered
world/menu image SHA-256 equals its baseline: 6,000 exact image pairs, including
fading, settled and consumed states. No tolerance was needed for this change.
The baseline image sequences were retained from the immediately preceding study;
their original binary is pinned in [identities.json](identities.json).

These controls compare the pre-change retained renderer with the new serialized
mirror. They share projection arithmetic, textures and fragment math. Their
images omit unchanged egui painting identically; timing runs include native
menu/egui. They establish image preservation by this packing change, not an
independent validation of all retained-renderer behavior. Sent-byte control
digests and hashes of the selected numeric frame columns listed in
`identities.json` match in every timed run; these are not complete world-state
digests. The timed driver's canonical-parity mode was off, so its constant
quad-digest accumulator is not parity evidence. Actual image hashes above
exercise rendering separately from those controls.

The explicit authored GPU regression compares retained submission with the
generic canonical path across spawn fade, settled animation and consumption. It
now also hides and re-exposes slots and replaces the world with the same count.
Its existing two-byte paired-addition allowance is unchanged. It shares textures
and fragment math; it is not an independent Canvas oracle.

Validation commands (from the repository root, using the task's per-command
Cargo target/jobs settings):

```sh
npm --prefix client run check
cargo test --manifest-path client/apps/wgpu-slice/Cargo.toml --locked --lib authored_rectangle_instancing_stays_within_existing_rounding_budget -- --ignored --nocapture
python -B research/client-performance/2026-11-03-pellet-upload-mirror/recompute.py --check
python -B -m unittest discover -s research/client-performance/2026-11-03-pellet-upload-mirror -p test_study.py
python -B -m unittest discover -s tools -p test_documentation.py
```

Native/Wasm compilation, strict Clippy, Canvas ABI and the explicit authored GPU
check pass. Browser/WebGL runtime, a physical four-core machine, presented
intervals and owner play remain unverified. This patch does not change the GPU
layout or shader, but compilation is not browser runtime acceptance.

## Reproduction and source identities

`recompute.py` uses only the bounded CSV and JSON to regenerate every timing
table above. `--check` also verifies callbacks, skips, arm/binary assignments and
matching output/image identities. Study tests work without Git or current source
files. No raw captures, images, player identifiers or layouts are published.

The measured candidate is `4a0443a3` plus the fixed-layout serializer inline
attribute; `e81a5991` records those production expressions and the expanded GPU
regression. The final reviewed source additionally changes a study-link comment
and the regression's state label. [identities.json](identities.json) retains
measured instrumented and reviewed full-file hashes separately; none is silently
substituted for another or used as a current-file test gate. Frozen source
copies, binaries, per-callback journals and process/GPU load remain local-only
beside the restricted recording. There are no new measurements from a game server.
