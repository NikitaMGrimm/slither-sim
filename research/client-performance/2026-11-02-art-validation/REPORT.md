# Validate crowded pictures without projecting discarded vertices

Access: **restricted owner-local community join** and **original authored
synthetic 25,000-pellet rectangle**. No recording, player identities, captured
coordinates, artwork or raw frame/load journals are published here.

## Historical first profile before the fix

Current main at profiling time was `fde706f1600e7d0917bc467a9b578de1b3d3b724`,
after #439. Both #440 and #441 were still separate unmerged PRs then. The first
join diagnostic found sustained art callbacks at 25.29 ms median, with 11.10 ms
preparation, 8.24 ms CPU submission (6.85 ms quad packing), and 2.31 ms actual
GPU passes. Final picture validation alone cost 3.65 ms. Its largest cold
preparation was 83.24 ms, of which procedural sprite rasterization cost 81.88 ms.
Font loading accounts for another cold frame, separately from the sustained cost.
The [bounded initial profile](earlier-composition/initial-profile.csv) retains those initial medians
and the worst cold-preparation callback attribution for both scenes.

The valid authored rectangle also exposes CPU work: 39.08 ms median callback,
17.14 ms preparation, 11.93 ms packing, 6.74 ms final validation and 1.83 ms GPU
passes in the initial diagnostic. Those observers are for attribution only;
changing load makes them unsuitable as paired speed estimates. The bounded
final diagnostic series below uses identical driver/observer settings per arm.
The rectangle is the arithmetic 200-by-125 patch from the sector regression;
it is not a reconstruction of community artwork. It contains exactly 25,000
food records, rather than the regression's additional unrelated-sector foods.

Those first measurements and the initial matched series predate the merged
composition. [Earlier-composition report and artifacts](earlier-composition/REPORT.md)
are preserved separately and are not the current before/after claim.

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

Current comparison base: `0f45f4acb00431ba461c604340fa6b63428c2db4`, including
merged #440 and #441. The candidate preserves #441's paired converter cache and
both authored regression suites; the new fast path applies only to validation.
Both release arms were rebuilt, exact-output parity rerun, and all matched
timings below newly collected against this base. Older composition numbers are
retained only as historical evidence.

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
- Five alternating pairs per case initially; even pairs reverse arm order.
  Add five default-view pairs to characterize its smaller incremental effect
  after #441. All 40 runs are retained and accepted; no compilation overlap
  was observed. [runs-current.csv](runs-current.csv) records status,
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
| join_default / art callback p50 | 25.095 ms | 23.027 ms | 0.5 to 13.3% |
| join_default / art callback p95 | 29.579 ms | 27.378 ms | -5.6 to 11.3% |
| join_default / art preparation p50 | 11.232 ms | 9.209 ms | 6.4 to 23.0% |
| join_default / art CPU submit p50 | 7.837 ms | 7.988 ms | -9.7 to 6.7% |
| join_default / art GPU p50 | 2.323 ms | 2.336 ms | -3.5 to 0.8% |
| join_default / cold preparation maximum | 88.352 ms | 89.965 ms | -15.2 to 10.9% |
| join_default / cold callback maximum | 100.061 ms | 101.858 ms | -13.5 to 10.0% |
| join_default / 10-second worst callback | 100.061 ms | 101.858 ms | -13.5 to 10.0% |
| join_default / processing elapsed | 10.620 s | 9.880 s | -0.8 to 12.2% |
| join_default / requested allocations | 12.690 GB | 12.690 GB | -0.0 to 0.0% |
| join_whole_map / art callback p50 | 6.432 ms | 6.379 ms | 0.3 to 6.2% |
| join_whole_map / art callback p95 | 9.556 ms | 9.710 ms | -10.4 to 9.6% |
| join_whole_map / art preparation p50 | 3.569 ms | 3.511 ms | 0.5 to 9.6% |
| join_whole_map / art CPU submit p50 | 0.308 ms | 0.315 ms | -4.4 to 5.0% |
| join_whole_map / art GPU p50 | 0.650 ms | 0.648 ms | -0.1 to 1.4% |
| join_whole_map / cold preparation maximum | 52.069 ms | 53.166 ms | -11.8 to 5.9% |
| join_whole_map / cold callback maximum | 57.855 ms | 58.989 ms | -10.9 to 5.6% |
| join_whole_map / 10-second worst callback | 57.855 ms | 58.989 ms | -10.9 to 5.6% |
| join_whole_map / processing elapsed | 3.364 s | 3.259 s | -0.1 to 6.4% |
| join_whole_map / requested allocations | 11.192 GB | 11.192 GB | -0.0 to 0.0% |
| rectangle_default / art callback p50 | 26.964 ms | 24.415 ms | 2.2 to 13.4% |
| rectangle_default / art callback p95 | 32.934 ms | 30.953 ms | 4.3 to 11.9% |
| rectangle_default / art preparation p50 | 12.250 ms | 9.371 ms | 13.0 to 28.2% |
| rectangle_default / art CPU submit p50 | 9.701 ms | 9.790 ms | -5.4 to 3.2% |
| rectangle_default / art GPU p50 | 1.805 ms | 1.798 ms | -1.1 to 0.8% |
| rectangle_default / cold preparation maximum | 27.899 ms | 26.733 ms | -11.9 to 21.9% |
| rectangle_default / cold callback maximum | 54.212 ms | 54.992 ms | -9.4 to 12.4% |
| rectangle_default / 10-second worst callback | 54.212 ms | 54.992 ms | -9.4 to 12.4% |
| rectangle_default / processing elapsed | 10.685 s | 9.836 s | 3.3 to 12.1% |
| rectangle_default / requested allocations | 6.343 GB | 6.343 GB | 0.0 to 0.0% |

join_default control: 10 runs; art p50 range 23.387 to 26.940 ms; art callbacks above 16.7 ms 300 to 300 / 300; observed load 0.0 to 57.8%.

join_default candidate: 10 runs; art p50 range 22.101 to 25.044 ms; art callbacks above 16.7 ms 300 to 300 / 300; observed load 0.0 to 53.7%.

join_whole_map control: 5 runs; art p50 range 5.774 to 6.488 ms; art callbacks above 16.7 ms 0 to 1 / 300; observed load 0.0 to 33.8%.

join_whole_map candidate: 5 runs; art p50 range 5.417 to 6.438 ms; art callbacks above 16.7 ms 0 to 0 / 300; observed load 0.0 to 43.8%.

rectangle_default control: 5 runs; art p50 range 24.967 to 28.179 ms; art callbacks above 16.7 ms 296 to 300 / 300; observed load 0.0 to 86.7%.

rectangle_default candidate: 5 runs; art p50 range 23.359 to 24.801 ms; art callbacks above 16.7 ms 274 to 300 / 300; observed load 0.0 to 40.1%.

All runs: 40; accepted 40; excluded 0.
<!-- END RESULTS -->

Cold rasterization is unchanged by this fix. The large sector-clear callback
is evaluated with the merged #440 path already present. The #441 cache is
also part of both current arms, so these are incremental validation results.
Whole-map savings are small and noisy. Default-view art still exceeds 16.7 ms throughout the selected
window; this is a sustained CPU reduction, not frame-budget acceptance.

## Final diagnostic attribution

<!-- BEGIN DIAGNOSTICS -->
| Art-period stage / one diagnostic pair | Join before / after p50 ms | Rectangle before / after p50 ms |
| --- | ---: | ---: |
| redraw_ns | 10.904 / 8.873 | 11.602 / 9.262 |
| final_validate | 2.557 / 1.128 | 3.453 / 1.390 |
| gpu_pack | 6.092 / 6.226 | 7.820 / 8.133 |
| section_food | 2.874 / 2.796 | 2.995 / 3.001 |
| advance | 2.558 / 2.105 | 1.464 / 1.403 |
| resource_keys | 1.206 / 1.132 | 1.565 / 1.486 |
| gpu_upload | 0.463 / 0.425 | 0.579 / 0.551 |
| gpu_encode | 0.563 / 0.555 | 0.487 / 0.483 |
| gpu_ns | 2.337 / 2.329 | 1.816 / 1.810 |
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
- The first current-composition full check observed a late progress row in the
  unchanged official-startup cancellation test (`official.rs:893`). The same
  injected-fetch test passed alone. The complete retry command was `npm run check`
  from `client/`, using the same target/jobs/optimized-dev environment as the
  original check (`CARGO_PROFILE_DEV_OPT_LEVEL=1`, `CARGO_PROFILE_DEV_DEBUG=0`)
  and the pinned wasm-bindgen 0.2.129 executable. It completed
  with **exit status 0** and `All checks passed.` Its complete output is retained
  owner-locally at `D:/slither-scratch/client-lane-20261101/community-lag/join-20261102/val-v2-full-check-retry.log`.
  No assertion or unrelated startup code was changed. This test opens no
  outside-server connection.
- Remaining default-view costs rank preparation and CPU packing ahead of GPU
  work at this resolution. Cold sprite rasterization is an independent follow-up.
  Owner play, current physical input latency/rubber-banding, browser runtime,
  other adapters/resolutions and physical gameplay acceptance remain
  unverified. The current replay does include both merged fixes. No visual trade-off or default setting change is introduced.

## Reproduction and retained failures

The source check reads [frame-sources.json.gz](frame-sources.json.gz), a
gzip-compressed JSON map of composition, arm and exact UTF-8 `frame.rs` text.
Both current and earlier-composition payloads match their original measured
and reviewed SHA-256 values. Recorded commit IDs remain origin labels; the
check needs neither those Git objects nor today's runtime file. Missing or
altered snapshots fail validation, while squash merges, shallow checkout and
later source edits leave the historical check usable. Run
`python research/client-performance/2026-11-02-art-validation/test_study.py`.
CI does not discover this manual study suite; its documentation checks validate
links/index placement. No measured identities or results changed.

Recompute every matched table and check run/source/parity assignment, using only
the Python standard library:

```text
python research/client-performance/2026-11-02-art-validation/recompute.py --check
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
