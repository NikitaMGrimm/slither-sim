# Validate crowded pictures without projecting discarded vertices

Access: **restricted owner-local community join** and **original authored
synthetic 25,000-pellet rectangle**. No recording, player identities, captured
coordinates, artwork or raw frame/load journals are published here.

## Profile before the fix

Current main at profiling time was `fde706f1600e7d0917bc467a9b578de1b3d3b724`,
after #439. Both #440 and #441 were still separate unmerged PRs. The first
join diagnostic found sustained art callbacks at 25.29 ms median, with 11.10 ms
preparation, 8.24 ms CPU submission (6.85 ms quad packing), and 2.31 ms actual
GPU passes. Final picture validation alone cost 3.65 ms. Its largest cold
preparation was 83.24 ms, of which procedural sprite rasterization cost 81.88 ms.
Font loading accounts for another cold frame, separately from the sustained cost.
The [bounded initial profile](initial-profile.csv) retains those initial medians
and the worst cold-preparation callback attribution for both scenes.

The valid authored rectangle also exposes CPU work: 39.08 ms median callback,
17.14 ms preparation, 11.93 ms packing, 6.74 ms final validation and 1.83 ms GPU
passes in the initial diagnostic. Those observers are for attribution only;
changing load makes them unsuitable as paired speed estimates. The bounded
final diagnostic series below uses identical driver/observer settings per arm.
The rectangle is the arithmetic 200-by-125 patch from the sector regression;
it is not a reconstruction of community artwork. It contains exactly 25,000
food records, rather than the regression's additional unrelated-sector foods.

## Change and correctness argument

`Frame::validate` previously ran full f64-to-f32 projection and discarded every
quad, then GPU submission projected the same picture again. It now shares the
original resource/source/alpha/transform admission and proves ordinary coordinate
ranges safe without constructing GPU vertices. Projection arithmetic, vertex
bits, shader inputs, draw order, resources and smoothing remain unchanged.

Let `L = f32::MAX / 32 / max(viewport_scale, 1)`. Require each rectangle field,
translation component and explicit corner magnitude at most L. A rectangle's
corner is at most 2L; rotation and translation produce magnitude at most 5L;
clip coordinates are at most `10L*viewport_scale + 1`, comfortably below the
finite f32 maximum because nonzero extents are integers at least one. This also
bounds intermediate f64 arithmetic. Valid source crops produce UVs in [0,1].
Repeat-UV components have a separate quarter-maximum bound, so each sum stays
below half the f32 maximum. These are sufficient conditions, not new input
limits: anything outside them uses the unchanged exact projection/error path.
Metadata and individual draw ordering are still validated before advancing to
the next draw, preserving the first error. Zero-extent behavior is unchanged.

## Method, source binding and selection

- Release Rust 1.99.0, locked dependencies, per-command target directory
  `D:/CargoTargets/community-lag`, build jobs 2. DX12 AMD Radeon(TM) Graphics
  integrated adapter, driver `32.0.21001.9024`. Original quality, 960x540 physical
  output, production world-target policy and MSAA4 through `WorldPresenter`.
- The retained community recording includes connection/setup, food art and its
  disappearance. Same first ten seconds: 600 virtual 60 Hz callbacks, 29,044
  envelopes, default and whole-map views. The synthetic input has eight envelopes
  over the same 600 callbacks, including its challenge/setup, five listings and
  clear. No new live sessions or outside-server connections.
- Art window [2,7) seconds; cold window [0,2). Each timing run validates complete
  frame/message counts, zero skipped pictures and all non-timing numeric
  observations. Full prepared-frame/resource/send and projected-vertex parity
  runs are separate from timing. [identities.json](identities.json) preserves
  exact measured-source, binary, input, driver and runner hashes.
- Initial five alternating pairs per case; even pairs reverse arm order. Extra
  default and rectangle pairs were added because other workers' builds overlapped
  the initial samples. All runs remain in [runs.csv](runs.csv), with status,
  reason, arm/pair, quantiles, elapsed time, allocation requests and load/process
  counts. Exclude only observed rustc/link overlap; do not exclude a slow sample
  or a test without compilation. Unpaired accepted arms remain in the CSV but
  are not included in matched tables. No booked quiet window.
- Process/CPU sampling approximately every 250 ms cannot rule out unobserved
  short-lived work. The report includes spread and reversals. Medians are medians
  of per-run values across complete accepted pairs; per-run p95 is sorted index
  `floor((n-1)*0.95)`. Requested allocation bytes are cumulative requests, not RSS.
- Callback wall time includes diagnostic GPU completion/readback. Processing
  elapsed also includes outside-callback diagnostic counting/serialization.
  These are replay costs, not gameplay FPS or input/scanout latency. Phase
  observers are disabled during timing, enabled only for separate attribution.

## Matched release results

<!-- BEGIN RESULTS -->
| Case / median of complete accepted pairs | Before | After | Paired reduction range |
| --- | ---: | ---: | ---: |
| join_default / art callback p50 | 24.892 ms | 21.908 ms | 9.5 to 16.6% |
| join_default / art callback p95 | 28.576 ms | 25.440 ms | 4.9 to 27.9% |
| join_default / art preparation p50 | 10.927 ms | 8.492 ms | 20.1 to 29.2% |
| join_default / art CPU submit p50 | 8.227 ms | 7.853 ms | 0.6 to 6.9% |
| join_default / art GPU p50 | 2.330 ms | 2.328 ms | -0.5 to 1.2% |
| join_default / cold preparation maximum | 81.588 ms | 82.006 ms | -16.2 to 14.9% |
| join_default / cold callback maximum | 133.779 ms | 132.994 ms | -4.4 to 8.6% |
| join_default / 10-second worst callback | 528.494 ms | 523.350 ms | -11.4 to 27.4% |
| join_default / processing elapsed | 11.011 s | 9.871 s | 6.1 to 15.9% |
| join_default / requested allocations | 21.383 GB | 21.383 GB | -0.0 to 0.0% |
| join_whole_map / art callback p50 | 5.111 ms | 5.040 ms | -8.1 to 26.2% |
| join_whole_map / art callback p95 | 7.414 ms | 7.264 ms | -22.2 to 67.0% |
| join_whole_map / art preparation p50 | 2.665 ms | 2.614 ms | -5.3 to 32.7% |
| join_whole_map / art CPU submit p50 | 0.270 ms | 0.262 ms | -4.3 to 38.0% |
| join_whole_map / art GPU p50 | 0.610 ms | 0.607 ms | -4.6 to 5.7% |
| join_whole_map / cold preparation maximum | 49.512 ms | 50.317 ms | -5.1 to 40.3% |
| join_whole_map / cold callback maximum | 117.148 ms | 119.291 ms | -4.4 to 40.3% |
| join_whole_map / 10-second worst callback | 502.745 ms | 502.292 ms | -6.2 to 12.0% |
| join_whole_map / processing elapsed | 3.252 s | 3.205 s | -8.8 to 41.3% |
| join_whole_map / requested allocations | 19.886 GB | 19.886 GB | 0.0 to 0.0% |
| rectangle_default / art callback p50 | 27.906 ms | 23.663 ms | 3.6 to 18.6% |
| rectangle_default / art callback p95 | 34.896 ms | 29.556 ms | -2.1 to 17.3% |
| rectangle_default / art preparation p50 | 12.443 ms | 8.803 ms | 17.2 to 33.9% |
| rectangle_default / art CPU submit p50 | 10.323 ms | 9.907 ms | -7.2 to 9.3% |
| rectangle_default / art GPU p50 | 1.801 ms | 1.799 ms | -0.3 to 0.5% |
| rectangle_default / cold preparation maximum | 27.066 ms | 22.732 ms | -0.4 to 17.5% |
| rectangle_default / cold callback maximum | 53.615 ms | 48.949 ms | -6.5 to 10.9% |
| rectangle_default / 10-second worst callback | 53.615 ms | 48.949 ms | -6.5 to 10.9% |
| rectangle_default / processing elapsed | 11.200 s | 9.469 s | 4.3 to 16.6% |
| rectangle_default / requested allocations | 6.370 GB | 6.370 GB | -0.0 to 0.0% |

join_default control: 10 runs; art p50 range 24.493 to 26.028 ms; art callbacks above 16.7 ms 300 to 300 / 300; observed load 0.0 to 68.8%.

join_default candidate: 10 runs; art p50 range 21.565 to 22.517 ms; art callbacks above 16.7 ms 300 to 300 / 300; observed load 0.0 to 69.1%.

join_whole_map control: 5 runs; art p50 range 5.045 to 7.652 ms; art callbacks above 16.7 ms 0 to 34 / 300; observed load 0.0 to 100.0%.

join_whole_map candidate: 5 runs; art p50 range 4.920 to 5.646 ms; art callbacks above 16.7 ms 0 to 0 / 300; observed load 0.0 to 54.4%.

rectangle_default control: 6 runs; art p50 range 27.434 to 28.862 ms; art callbacks above 16.7 ms 300 to 300 / 300; observed load 0.0 to 67.0%.

rectangle_default candidate: 6 runs; art p50 range 23.486 to 26.585 ms; art callbacks above 16.7 ms 276 to 288 / 300; observed load 0.0 to 63.2%.

All runs: 60; accepted 45; excluded 15.
<!-- END RESULTS -->

Cold rasterization is unchanged by this fix. The large sector-clear callback
also remains because these measurements predate #440; do not sum independent
PR results to predict their composed performance. Whole-map savings are small
and noisy. Default-view art still exceeds 16.7 ms throughout the selected
window; this is a sustained CPU reduction, not frame-budget acceptance.

## Final diagnostic attribution

<!-- BEGIN DIAGNOSTICS -->
| Art-period stage / one diagnostic pair | Join before / after p50 ms | Rectangle before / after p50 ms |
| --- | ---: | ---: |
| redraw_ns | 12.814 / 10.041 | 14.631 / 9.900 |
| final_validate | 4.152 / 1.191 | 5.567 / 1.558 |
| gpu_pack | 7.359 / 6.906 | 10.138 / 9.297 |
| section_food | 2.928 / 2.956 | 3.331 / 3.187 |
| advance | 2.734 / 2.793 | 1.557 / 1.484 |
| resource_keys | 1.328 / 1.341 | 1.606 / 1.543 |
| gpu_upload | 0.500 / 0.487 | 0.622 / 0.600 |
| gpu_encode | 0.546 / 0.570 | 0.523 / 0.513 |
| gpu_ns | 2.322 / 2.310 | 1.798 / 1.794 |
<!-- END DIAGNOSTICS -->

GPU numbers are device time; validation and packing are CPU work. Food, resource
collection and validation are nested within preparation, so child medians must
not be added to their parent. [cold-preparation.csv](cold-preparation.csv)
retains bounded stage values for the worst three cold preparation callbacks in
each diagnostic, without world positions or identities.

## Validation and remaining work

- Both views and the authored rectangle preserve complete prepared picture,
  overlay, resource, send and bit-exact projected-quad digests across every one
  of the 600 callbacks, plus numeric state/clock/camera observations. No skips.
- Authored regression compares validation acceptance and first-error strings
  with full projection across ordinary draws, the packed rectangle, source and
  resident errors, UV modes, subnormal/extreme values, signed zero, rotations,
  polygon corners, scales, empty/invalid extents and conservative-bound edges.
- Mutation proof: disabling the projection fallback for unproven extreme draws
  compiles, then fails the validation-versus-projection boundary regression.
  The production file was restored byte-for-byte and focused tests pass.
- Full `npm run check` passes: core/native suites, strict Windows latency and
  frame-profile Clippy, desktop build, strict Wasm Clippy/build and real Canvas
  ABI. Seven documentation tests, source guards, LF/whitespace checks and the
  study recomputation/source-binding test also pass.
- Remaining default-view costs rank preparation and CPU packing ahead of GPU
  work at this resolution. Cold sprite rasterization is an independent follow-up.
  Owner play, current physical input latency/rubber-banding, browser runtime,
  other adapters/resolutions and performance composed with #440/#441 remain
  unverified. No visual trade-off or default setting change is introduced.

## Reproduction and retained failures

Recompute every matched table and check run/source/parity assignment, using only
the Python standard library:

```text
python research/client-performance/2026-11-02-art-validation/earlier-composition/recompute.py --check
```

[diagnostic.patch](diagnostic.patch) retains the common passive observers and
the `join_profile` release example against the pinned base. Build it in an
external source copy with `--features frame-profile,feel-trace,latency-rig`, jobs
2 and the target directory above. Run `replay INPUT NEW_DIR 10 0` or `-120` for
whole-map; `rectangle unused NEW_DIR 10 0` uses only authored packets. Enable
`PROFILE_PHASES=1` for attribution, or `PROFILE_PARITY=1 PROFILE_CPU_ONLY=1` for
full output comparison, separately from timing. Source copies, executables,
all raw journals/load receipts and the alternating runner remain in restricted
external `community-lag/join-20261102` scratch.

Two initial synthetic harness attempts were rejected and retained: one delivered
the challenge before the offline session was ready, and another incorrectly
wrapped a large single listing as a short envelope, producing zero foods. The
accepted rectangle run verifies 25,000 foods and 175,149 draws while populated;
neither rejected attempt enters the timing tables. Shared-target source copies
were explicitly rebuilt and assigned separate binary/source identities.
