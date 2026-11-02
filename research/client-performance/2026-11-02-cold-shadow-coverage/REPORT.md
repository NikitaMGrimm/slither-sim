# Cold sprite shadow coverage

Access: **restricted owner-local community join recording** and **original
authored 25,000-pellet rectangle**. Raw recordings, layouts, player identities,
captured coordinates and full journals remain outside Git. Only bounded results
and source-only diagnostic/rejected-prototype patches are published here.

## Measured cause and decision

The post-#439 join timeline separated sustained CPU drawing from cold resource
preparation. #440 removes the sector-refresh hitch, #441 reuses paired converter
projections, and #442 separately reduces discarded validation projections. This
study starts at `0f45f4acb00431ba461c604340fa6b63428c2db4`, with #440 and #441
merged, and measures the cold raster cause independently of #442.

The slow initial resources are authored food outlines with blurred shadows.
The horizontal Gaussian pass recomputed each 4x4 geometric coverage test for
every neighbouring tap, even though adjacent output pixels revisit the same
sample location. This is CPU raster work, not new protocol work or first-use
shader creation. The separate phase attribution below identifies that resource
burst and the remaining font preparation. Those observer runs are not matched
speed estimates; the alternating release series owns the performance claim.

An earlier iterator-local validation change targets sustained drawing (#442).
This change targets only the independently observed cold shadow work. It does
not move a loading screen, alter smoothing/quality, skip pictures or pre-generate
all possible resources at startup.

## Exact arithmetic and memory

Compute the existing coverage once for each required horizontal sample, including
the blur halo, then reuse it in the original Gaussian loop. Coverage is exactly
an integer count in [0,16] divided by 16. Store those counts as bytes; converting
back reproduces the same f64 value exactly. Integer x plus integer tap offset is
exact at these small extents, so precomputed coordinates are identical to the
old coordinates, including fractional vertical shadow offset. Keep multiply,
divide, addition and tap order unchanged. The vertical pass, colour, alpha,
premultiplication, image bytes, source identity and render submission are unchanged.

The existing recipe extent and blur guards bound the temporary coverage array to
`(512 + 2*48)*512 = 311,296` bytes. It is released during each materialization;
no mask or finished source is retained beyond the existing resource policy.
Unsupported shadow composites still take the same error path. Coverage for
ordinary fills, radial gradients and non-shadow geometry is unchanged.

## Methods and evidence binding

- Both release arms use the pinned merged main above, locked dependencies and
  Rust 1.99.0. Per-command target `D:/CargoTargets/community-lag`, build jobs 2.
  DX12 AMD Radeon(TM) Graphics integrated adapter, driver `32.0.21001.9024`.
  Original quality, 960x540 physical output, production world-target policy and
  MSAA4 via `WorldPresenter`. Offscreen only; no new server connections.
- Same retained first ten seconds: 600 virtual 60 Hz callbacks and 29,044 join
  envelopes. Default and whole-map views. The authored arithmetic rectangle has
  25,000 foods in five listings, an authored handshake/setup and a later clear:
  eight envelopes and the same 600 callbacks. It contains no copied artwork.
- Cold window [0,2) seconds, art window [2,7). Each sample checks complete
  counts, no skipped pictures and the numeric observation digest. Full prepared
  frame/overlay/resource/send and projected-vertex parity is separate from timing.
  Retained control parity uses the exact same control binary/input/driver; final
  candidate parity is newly executed after the final source cleanup.
- Five alternating pairs per case; even pairs reverse arm order. Record CPU
  load and cargo/rustc/link/test/archive presence about every 250 ms around each
  run. Retain every run; exclude only observed compilation overlap. No booked
  quiet window. Sampling cannot rule out short-lived work between samples.
- [runs.csv](runs.csv) has one row per run with status/reason, pair/arm,
  quantiles/maxima, allocations, elapsed processing, load and process counts.
  [identities.json](identities.json) binds the exact measured/final material
  source bytes, driver, binaries, input and parity. There is no source-normalization
  substitution for the measured/final file identity.
- Callback wall time includes diagnostic GPU completion/readback. Processing
  elapsed also includes outside-callback diagnostic work. Requested allocation
  bytes are cumulative requests, not live memory. These costs are not gameplay
  FPS or physical input/scanout latency. Timed runs disable phase observers.
- Tables use medians of per-run values from complete accepted pairs. p95 is
  sorted index `floor((n-1)*0.95)` inside each run. Retain paired reversals and
  ranges; these are workload observations, not confidence intervals.

## Matched release results

<!-- BEGIN RESULTS -->
| Case / median of complete accepted pairs | Before | After | Paired reduction range |
| --- | ---: | ---: | ---: |
| join_default / art callback p50 | 22.834 ms | 22.833 ms | -1.9 to 1.0% |
| join_default / art callback p95 | 26.342 ms | 26.098 ms | -3.3 to 3.3% |
| join_default / art preparation p50 | 9.895 ms | 9.915 ms | -4.2 to 2.0% |
| join_default / art CPU submit p50 | 7.478 ms | 7.458 ms | -1.8 to 1.1% |
| join_default / art GPU p50 | 2.331 ms | 2.330 ms | -1.4 to 1.3% |
| join_default / cold preparation maximum | 82.685 ms | 51.234 ms | 37.4 to 39.9% |
| join_default / cold callback maximum | 95.012 ms | 56.779 ms | 39.0 to 41.7% |
| join_default / 10-second worst callback | 95.012 ms | 56.779 ms | 39.0 to 41.7% |
| join_default / processing elapsed | 9.621 s | 9.539 s | -1.8 to 1.6% |
| join_default / requested allocations | 12.690 GB | 12.690 GB | -0.0 to -0.0% |
| join_whole_map / art callback p50 | 5.407 ms | 5.656 ms | -9.5 to 9.0% |
| join_whole_map / art callback p95 | 7.939 ms | 8.586 ms | -15.2 to 9.4% |
| join_whole_map / art preparation p50 | 2.803 ms | 2.925 ms | -12.4 to 15.9% |
| join_whole_map / art CPU submit p50 | 0.280 ms | 0.282 ms | -12.1 to 3.1% |
| join_whole_map / art GPU p50 | 0.648 ms | 0.651 ms | -3.5 to 4.7% |
| join_whole_map / cold preparation maximum | 49.703 ms | 51.432 ms | -18.6 to 1.6% |
| join_whole_map / cold callback maximum | 54.897 ms | 56.889 ms | -17.9 to 2.0% |
| join_whole_map / 10-second worst callback | 54.897 ms | 56.889 ms | -17.9 to 2.0% |
| join_whole_map / processing elapsed | 2.825 s | 2.970 s | -12.5 to 8.1% |
| join_whole_map / requested allocations | 11.192 GB | 11.192 GB | -0.0 to 0.0% |
| rectangle_default / art callback p50 | 26.002 ms | 25.354 ms | 0.5 to 5.7% |
| rectangle_default / art callback p95 | 34.029 ms | 31.642 ms | 1.4 to 7.1% |
| rectangle_default / art preparation p50 | 11.534 ms | 11.123 ms | 1.5 to 8.4% |
| rectangle_default / art CPU submit p50 | 9.247 ms | 9.129 ms | -2.9 to 4.9% |
| rectangle_default / art GPU p50 | 1.806 ms | 1.801 ms | -0.4 to 1.0% |
| rectangle_default / cold preparation maximum | 28.707 ms | 29.916 ms | -10.8 to 9.5% |
| rectangle_default / cold callback maximum | 56.107 ms | 55.996 ms | -6.9 to 7.8% |
| rectangle_default / 10-second worst callback | 56.107 ms | 55.996 ms | -6.9 to 7.8% |
| rectangle_default / processing elapsed | 10.450 s | 10.305 s | 0.7 to 4.1% |
| rectangle_default / requested allocations | 6.343 GB | 6.343 GB | -0.0 to 0.0% |

join_default control: 5 runs; art p50 range 22.778 to 22.942 ms; art callbacks above 16.7 ms 300 to 300 / 300; observed load 0.0 to 47.8%.

join_default candidate: 5 runs; art p50 range 22.673 to 23.243 ms; art callbacks above 16.7 ms 300 to 300 / 300; observed load 0.0 to 40.8%.

join_whole_map control: 5 runs; art p50 range 5.152 to 6.213 ms; art callbacks above 16.7 ms 0 to 0 / 300; observed load 0.0 to 39.0%.

join_whole_map candidate: 5 runs; art p50 range 5.149 to 5.986 ms; art callbacks above 16.7 ms 0 to 0 / 300; observed load 0.0 to 36.7%.

rectangle_default control: 5 runs; art p50 range 25.492 to 26.309 ms; art callbacks above 16.7 ms 293 to 300 / 300; observed load 0.0 to 67.2%.

rectangle_default candidate: 5 runs; art p50 range 24.627 to 25.874 ms; art callbacks above 16.7 ms 293 to 299 / 300; observed load 0.0 to 41.5%.

All runs: 30; accepted 30; excluded 0.
<!-- END RESULTS -->

Warm art drawing is expected to change little: cached sprite pixels need no new
shadow rasterization. Whole-map and the one-colour rectangle also need not show
the cold mixed-resource saving. Do not add independent PR timing differences to
claim a combined frame time or a 60 Hz budget pass.

## Stage attribution

<!-- BEGIN DIAGNOSTICS -->
| Cold-window maximum per stage / one untimed diagnostic | Join before / after ms | Rectangle before / after ms |
| --- | ---: | ---: |
| redraw_ns | 92.453 / 53.138 | 35.213 / 31.215 |
| raster | 90.983 / 34.401 | 1.448 / 1.403 |
| font_load | 30.750 / 31.113 | 18.322 / 14.904 |
| text_raster | 31.522 / 31.751 | 18.357 / 14.936 |
| hud_text | 15.942 / 15.543 | 0.004 / 0.003 |
| section_minimap | 18.167 / 17.775 | 0.015 / 0.017 |
| final_validate | 4.538 / 3.864 | 7.076 / 5.648 |
| gpu_pack | 10.046 / 7.606 | 13.897 / 11.894 |
| gpu_upload | 0.663 / 0.630 | 1.183 / 0.984 |
| gpu_encode | 6.246 / 5.982 | 8.383 / 7.411 |
| gpu_ns | 1.504 / 1.475 | 4.107 / 4.135 |
<!-- END DIAGNOSTICS -->

These are separate observer runs, not timing pairs. Device GPU time is distinct
from CPU packing/upload/encoding; rasterization is nested in preparation.
[cold-preparation.csv](cold-preparation.csv) retains stage values for each
diagnostic's three worst cold-preparation callbacks. Font decode and minimap
preparation are separate cold costs; gradient/mip generation remains in resource
rasterization. Interactive font/cache lifetimes and display pacing were not measured.
The stage maxima can occur in different callbacks and must not be added.

## Rejected circle-only trial

Before mapping the slow resource IDs to food outlines, a one-entry persistent
circle-mask cache was tested on base `643e4b3e` (before #441). It saved only a
small part of cold preparation and left the Gaussian repeated-sampling cost.
It was rejected rather than adding Bank state for that limited gain. Its
source-only [prototype patch](rejected-circle-only.patch) and
[complete bounded run rows](rejected-circle-runs.csv) remain separate from this
study's current-main results. Authored cached/uncached raster tests passed; full
restricted-input parity was not run for that rejected trial. The proposed fix
uses only the local shadow scratch described above.
The context-free prototype patch applies with `git apply --unidiff-zero`; it
reconstructs the same source tree as the retained original patch, whose identity
is preserved separately in `rejected-circle-identities.json`.

<!-- BEGIN REJECTED -->
Rejected circle-only trial: 30 runs retained, 8 excluded. Five complete default-view pairs: cold preparation maximum median 82.506 to 79.299 ms. This is the earlier 643e4b3e composition, not the current shadow fix.
<!-- END REJECTED -->

## Validation and remaining cost

- Authored regression compares every f64 horizontal accumulation bit with the
  independent old scalar loop across circles/polygons, tiny and boundary widths,
  blur radii, fractional/negative offsets and Gaussian underflow cases. It also
  checks that coverage calls equal the distinct required sample locations rather
  than output-pixel times tap count. No wall-clock unit-test threshold.
- Full replay output and source-pixel parity covers both join views and the
  authored rectangle, including all 600 callbacks and no skipped pictures.
- Mutation proof: omitting the fractional vertical offset from the precomputed
  coverage compiles, then fails the independent scalar-bit comparison. Restored
  production source passes the focused test and strict Clippy.
- Full `npm run check` passes with exit status 0: core/native suites, strict
  Windows latency/frame-profile and Wasm Clippy, desktop/Wasm builds and real
  Canvas ABI. The retained owner-local output is
  `D:/slither-scratch/client-lane-20261101/community-lag/join-20261102/shadow-full-check.log`.
  The study/source test, seven documentation checks, source guards, aggregate
  recomputation and LF/whitespace checks also pass.
- Owner play, current input latency/rubber-banding, browser runtime, other
  physical resolutions/adapters and performance combined with #442 are unverified.
  Sustained main-thread drawing and remaining cold font/gradient work are still
  above-budget risks; this is no owner-play or frame-budget acceptance claim.

## Reproduction and retained attempts

The source check reads [materials-sources.json.gz](materials-sources.json.gz),
a gzip-compressed JSON map from arm to exact UTF-8 `materials.rs` text. Each
payload matches its original measured SHA-256 in `identities.json`; recorded
commit IDs remain origin labels. No old Git objects or current runtime files
are read. Missing or altered snapshots fail validation. This keeps the historical
check usable after squash merges, shallow checkout and later source edits.
Run `python research/client-performance/2026-11-02-cold-shadow-coverage/test_study.py`.
CI does not discover this manual study suite; its documentation checks validate
links/index placement. No measured identities or results changed.

```text
python research/client-performance/2026-11-02-cold-shadow-coverage/recompute.py --check
```

The standard-library reducer checks assignment and reproduces all current tables.
The [shared diagnostic patch](https://github.com/NikitaMGrimm/slither-sim/blob/ead00d6ffdf0b1c4ac7da8ba9b826fc2af2a9294/research/client-performance/2026-11-02-art-validation/diagnostic.patch)
retains the byte-identical passive observers and
`join_profile` example against the base. Build the release example externally
with `--features frame-profile,feel-trace,latency-rig`, jobs 2 and the target above.
Use `replay INPUT NEW_OUTPUT 10 0` (or `-120` for whole-map), or
`rectangle unused NEW_OUTPUT 10 0`. Enable `PROFILE_PHASES=1` for attribution;
use `PROFILE_CPU_ONLY=1 PROFILE_PARITY=1` separately for full output comparison.
All raw data, builds, journals, load receipts, runners and README stay under the
restricted external `community-lag/join-20261102` scratch directory.

The initial external-instrumented Clippy attempt rejected an observer type, then
production Clippy found a redundant closure borrow. The final source removes
that borrow and is rebuilt/rechecked/replayed. Earlier binaries/profiles and
the rejected circle-cache test-build attempt remain retained, not relabelled as
final measurements. No raw recording or captured artwork is published.
