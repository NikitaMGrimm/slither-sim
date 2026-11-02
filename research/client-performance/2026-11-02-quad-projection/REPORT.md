# CPU projection of crowded pellet draws

Access: **restricted owner-local community join recording**; regression rectangles
are **original authored synthetic data**. No recording, layout, player identity,
coordinates or raw callback journal is published here.

## Question and decision

After #439, sustained frames containing the community server's pellet art are
primarily CPU-bound at the measured resolution. Separately, #440 addresses the
large sector-refresh transaction when that art disappears. This main-based change
targets repeated CPU projection of identical geometry within each picture. It
does not contain #440 or claim to resolve the remaining frame-budget problem.

Fine diagnostics attribute the largest CPU submission span to quad packing,
rather than sorting, upload or encoding (see the retained stage table). Paired
pellet commands repeat the same projection with different alpha. The native
picture validation and later GPU conversion both use `Frame::quad_iter`.

Retain the last successful projection only for the iterator's lifetime. Compare
every geometry/resource input by bits; preserve alpha validation and update alpha,
blend and black-effect flags for each draw. The frame and resident table are
immutable for that lifetime. No order, arithmetic, source raster, shader, render
target, quality policy, protocol handling or smoothing changes.

## Method and pins

- Base: `fde706f1600e7d0917bc467a9b578de1b3d3b724`, after #439. Both arms use
  the same passive diagnostic patch and driver. Full source/binary/driver/input
  identities and parity receipts are in [identities.json](identities.json).
- Release Rust 1.99.0, locked dependencies, jobs 2, per-command target directory
  `D:/CargoTargets/community-lag`. DX12 AMD Radeon(TM) Graphics integrated adapter,
  driver `32.0.21001.9024`, Original quality, 960×540 physical output, MSAA4 world
  via the production `WorldPresenter`. The world target follows Original policy.
- Same retained join input, first 10 seconds, 600 synthetic 60 Hz callbacks and
  29,044 message envelopes. Default and whole-map wheel views. The recording
  includes challenge, setup, initial food flood, art and its sector-clear/reload.
  The earlier 6.1-second input ends before that clear and is unsuitable here.
- Art window: model seconds [2,7); recovered window: [8,10). The initial study
  found approximately 25–26 thousand ordinary protocol-13 `f` pellets (size 0.2,
  palette 9), then 225 `w` sector clears around 7.5–7.6 seconds, rather than a
  mass eat. The [sector-refresh study](https://github.com/NikitaMGrimm/slither-sim/pull/440)
  owns those join observations. Raw packets and the identifying arrangement remain restricted.
- Five alternating pairs per view initially; five extra default-view pairs after
  the first series showed background drift. Even pairs reverse arm order. No
  quiet window; all runs retained. Process/load sampling approximately every
  250 ms records cargo/rustc/link/test/archive presence and total CPU load. Exclude
  only observed compilation overlap, never a slow result. None was observed.
- [runs.csv](runs.csv) has one row per run: arm/pair/status/reason, binary and
  source identity, callback quantiles, elapsed processing, requested allocations,
  load range and process counts. Each timing run checks complete message/frame
  counts, no skipped pictures and its full non-timing numeric observation digest.
- Frame wall time covers main-thread ingress, advance/preparation, submission and
  the diagnostic GPU wait/readback. Elapsed process time also includes diagnostic
  counting/serialization outside callbacks. Requested allocation bytes are
  cumulative requests, not live memory. Neither measure establishes gameplay FPS,
  input latency or network rubber-banding. No new live acquisition was needed.
- Medians below are medians of per-run values over complete accepted pairs. p95
  uses sorted index `floor((n-1)*0.95)` within a run. Paired reduction ranges retain
  reversals and are not confidence intervals. Diagnostic observers are disabled
  during timing; separate phase runs are attribution, not extra A/B samples.

## Matched results

<!-- BEGIN RESULTS -->
| View / median across complete pairs | Before | After | Paired reduction range |
| --- | ---: | ---: | ---: |
| default / art callback p50 | 26.497 ms | 24.940 ms | -6.2 to 19.5% |
| default / art callback p95 | 31.925 ms | 31.062 ms | -5.5 to 15.8% |
| default / all callbacks p95 | 31.443 ms | 29.964 ms | -6.1 to 15.3% |
| default / worst callback | 570.320 ms | 598.446 ms | -17.2 to 33.8% |
| default / CPU submission p50 | 7.970 ms | 7.161 ms | -0.1 to 18.4% |
| default / advance/preparation p50 | 10.495 ms | 9.691 ms | -3.3 to 21.8% |
| default / GPU passes p50 | 1.451 ms | 1.464 ms | -3.8 to 1.6% |
| default / process elapsed | 12.683 s | 12.358 s | -2.3 to 15.8% |
| default / requested allocations | 21.383 GB | 21.383 GB | -0.0 to 0.0% |
| whole_map / art callback p50 | 5.211 ms | 5.338 ms | -19.4 to 3.5% |
| whole_map / art callback p95 | 7.612 ms | 7.549 ms | -23.2 to 2.0% |
| whole_map / all callbacks p95 | 7.394 ms | 7.525 ms | -19.3 to 1.7% |
| whole_map / worst callback | 530.752 ms | 538.801 ms | -10.8 to 4.3% |
| whole_map / CPU submission p50 | 0.272 ms | 0.282 ms | -8.5 to 0.9% |
| whole_map / advance/preparation p50 | 2.549 ms | 2.551 ms | -26.7 to 0.1% |
| whole_map / GPU passes p50 | 0.553 ms | 0.569 ms | -8.7 to 1.6% |
| whole_map / process elapsed | 4.217 s | 4.214 s | -11.3 to 0.1% |
| whole_map / requested allocations | 19.886 GB | 19.886 GB | -0.0 to 0.0% |

default control: 10 runs, art p50 range 25.776–29.755 ms; load 0.0–71.3%; art callbacks >16.7 ms 300–300 / 300; longest over-budget virtual interval 6183.333 ms.


default candidate: 10 runs, art p50 range 23.408–28.150 ms; load 0.0–75.7%; art callbacks >16.7 ms 300–300 / 300; longest over-budget virtual interval 6183.333 ms.


whole_map control: 5 runs, art p50 range 5.135–5.535 ms; load 0.0–61.4%; art callbacks >16.7 ms 0–0 / 300; longest over-budget virtual interval 33.333 ms.


whole_map candidate: 5 runs, art p50 range 5.222–6.606 ms; load 0.0–52.6%; art callbacks >16.7 ms 0–0 / 300; longest over-budget virtual interval 33.333 ms.


All runs: 30; accepted: 30; excluded: 0.

<!-- END RESULTS -->

The default-view median saving is modest and noisy, with a reversal retained;
the CPU submission reduction is the clearest direct signal. Whole-map results
show no measurable end-to-end benefit. Allocation requests are unchanged.
The sector-refresh maximum remains large in both arms because this comparison
does not include #440. Do not add results from the two independent comparisons
to predict a combined frame time.

## Remaining cost and limits

The following single offscreen diagnostic pair gives stage attribution, not a
matched speed claim. Food, advance and resource spans are nested within redraw;
GPU packing/sorting/upload/encoding are CPU work. GPU passes are device time.
Redraw also includes snapshots, late overlay validation and unseparated host work;
the selected child spans do not exhaust it.

<!-- BEGIN DIAGNOSTICS -->
| Art-period stage / one untimed diagnostic | Before p50 / p95 ms | After p50 / p95 ms |
| --- | ---: | ---: |
| redraw | 12.134 / 15.257 | 10.060 / 11.825 |
| gpu_pack | 7.620 / 10.139 | 6.214 / 7.209 |
| section_food | 2.956 / 3.838 | 2.812 / 3.311 |
| advance | 2.249 / 2.793 | 2.080 / 2.514 |
| section_resources | 1.253 / 1.802 | 1.149 / 1.586 |
| gpu_sort | 0.486 / 0.637 | 0.433 / 0.537 |
| gpu_upload | 0.451 / 0.615 | 0.414 / 0.568 |
| gpu_encode | 0.599 / 0.781 | 0.523 / 0.721 |
| gpu_actual | 2.358 / 2.856 | 2.333 / 2.835 |
<!-- END DIAGNOSTICS -->

Remaining main-thread work ranks as full advance/preparation, then CPU GPU packing;
food planning, animation and resource collection/validation account for named
parts of the former. GPU execution is smaller at this resolution. The art period
still exceeds 16.7 ms throughout the selected default-view window. This change
does not shorten that over-budget interval or provide frame-budget acceptance.
Further gains need a separately profiled preparation/packing change; retain the
current error, transaction and exact-output guarantees rather than dropping work.

## Correctness and validation

The measured candidate `frame.rs` is
`6f5632ac06df6ba7f4a656b6cbc6241d8946a919b00ba053c3eed584ec75da85`;
the reviewed source at `9f9eab4684b578acdb668ab17b479238d817349f` is
`86a3ba7e6bd4ea71244b04cd28b4ff477ac29f5cb44ee10d59d22795dade5dee`.
Both identities are preserved. The measured bytes remain owner-local, and the
[exact measured-to-reviewed delta](frame-measured-to-reviewed.patch) is published
so reversing it against the reviewed source reconstructs those measured bytes.
It adds only one study-link comment and two regression blocks (signed-zero key
discrimination and the packed rectangle). No production expression differs;
the measured source contains the current-alpha assignment, not the stale-alpha
mutation. Independent review verified this linkage. No timing or parity rerun
was needed for this study-only correction.

The check now uses [frame-reviewed.rs.gz](frame-reviewed.rs.gz), the exact
reviewed UTF-8 source compressed with gzip, rather than today's `frame.rs`.
It still reverses the published delta, checks both original measured/reviewed
hashes and proves unchanged production expressions. Missing or altered source
evidence fails, with no dependency on old Git objects or current runtime files.
Run `python research/client-performance/2026-11-02-quad-projection/test_study.py`.
This manual study suite is not discovered by CI; CI checks its documentation
links/index placement. No historical identity, delta or result changed.

- Both views: every prepared frame/overlay/resource/send digest matches, every
  projected vertex component matches by f32 bits in order, all quad flags/resource
  identities match, and all non-timing numeric observations match. See the parity
  receipts for both complete 600-callback histories; no skipped pictures.
- The authored regression compares cached iterators with fresh singleton
  conversion for a packed 1,024-draw rectangle, alpha and paint changes, crops,
  repeat UVs, transformed/polygon geometry, resource changes, signed zero,
  invalid/overflow inputs, errors followed by valid draws, new viewports and
  missing resident tables. It tests exact bits and errors, not tolerances.
- Mutation proof: deliberately keep the previous alpha on a cache hit. It
  compiles and fails the independent fresh-conversion vertex-bit assertion.
  Production code was restored byte-for-byte and focused tests passed afterward.
- `npm run check` passed on the final runtime and regression source: strict
  Windows/native/frame-profile and Wasm Clippy, core/native suites, desktop and
  Wasm builds, and real Canvas ABI checks. Seven documentation checks, LF,
  repository boundary checks and aggregate recomputation also passed.
  Owner gameplay, other physical resolutions/adapters, browser runtime and new
  physical input-latency measurements are unverified. Existing historical latency
  measurements are not promoted to current acceptance.

## Reproduction and retained attempts

Independent review approved runtime correctness, edge cases, composition and
measurement validity, while narrowing the performance claim. Seven of ten paired
art medians improved; median paired reduction was 6.575%. Reversals are retained.
The independent release converter probe found an approximately 9–10% all-miss
tax, about 0.074 ms per 32,768-draw traversal, while paired conversion improved
38.5% (21.660 to 13.320 ns/quad). This is converter-only work, not sparse-gameplay
or end-to-end FPS. Ten alternating pairs per authored scene consume each quad
through `black_box`; the retained probe and its limits are recorded in
`identities.json`. [Bounded reviewer rows](review-converter.csv) recompute the
following figures without rerunning measurements:

<!-- BEGIN REVIEW -->
7 of 10 paired default-art medians improve; median paired reduction 6.575%.

| Independent converter-only scene | Main ns/quad | Candidate ns/quad | Change | Extra per traversal |
| --- | ---: | ---: | ---: | ---: |
| sparse-256 | 21.424 | 23.442 | +9.421% | +0.000517 ms |
| miss-32768 | 21.470 | 23.719 | +10.476% | +0.073703 ms |
| paired-32768 | 21.660 | 13.320 | -38.505% | -0.273297 ms |
<!-- END REVIEW -->

Recompute every table, verify run/source/parity bindings and check the published
aggregates using only the Python standard library:

```text
python research/client-performance/2026-11-02-quad-projection/recompute.py --check
```

[diagnostic.patch](diagnostic.patch) preserves the passive instrumentation and
driver against the base, including the projected-vertex digest extension. Apply
it only in an external source copy. Build the `join_profile` release example with
`--features frame-profile,feel-trace,latency-rig`; replay with arguments
`replay INPUT NEW_OUTPUT 10 0` (or `-120` for whole map). Set `PROFILE_PHASES=1`
for attribution; set `PROFILE_CPU_ONLY=1` and `PROFILE_PARITY=1` for parity,
separately from timing. Raw inputs, all runs/load journals, build logs, the
alternating runner and a short README remain under the restricted external
`community-lag/join-20261102` scratch directory; hashes bind those artifacts.

Before timing, Cargo reused a shared-target artifact when switching to a source
copy with older mtimes. That build was rejected. Both source copies were touched,
fully rebuilt and assigned distinct binary hashes before any measurement. The
failed build receipt is retained outside Git. No timed sample was replaced.
