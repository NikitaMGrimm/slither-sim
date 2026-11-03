# Smaller food frame copies

Status: measured behavior-preserving serial optimization. Native worker adoption is separate.

A frame transaction detached the food array and copied each pellet's frozen source metadata along with its changing state. Dense retained frames also requested full snapshots where five changing paint values suffice. Store immutable source metadata behind a shared owner, keep numeric state together, and expose a 40-byte paint state. Public snapshots, Debug shape, arithmetic, transaction order, and rollback stay unchanged. Painter helper specialization is included; this is the measured combination, not an isolated storage-only claim.

## Matched release results

Baseline: main `dd756cdffd7d3fb8b1ec48e9e15fe5a4df111379`, including #447 and #448. Candidate runtime: `db89a621cc0b284dc66fec46b8ef14523d818d1a`. No booked quiet window. Alternate A/B then B/A; retain every attempt, and exclude only an observed compiler/linker overlap (none of these runs). [runs.csv](runs.csv) contains all 36 runs: five pairs per RTX quality and two pairs per AMD size/quality. Load, cargo, test overlap, binaries and raw-journal hashes are included per run. Machine load varies across pairs and is not subtracted.

The art window is virtual callbacks 120–419, [2,7) seconds, selected before this comparison. Each table entry is the median of per-run medians or per-run p95s, not a pooled percentile. Requested GB is the full 600-callback process-wide requested allocation/reallocation byte count; it is not peak live memory and excludes setup/teardown outside callbacks. Both executables use global atomic allocator counters. CPU stages sum ingress, redraw, resource synchronization and submit; GPU waits are reported in callback wall time.

| Adapter / size / quality | Pairs | Art median ms | Art p95 ms | Food advance ms | CPU stages ms | GPU ms | Requested GB |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| rtx / 3840x2160 / native | 5 | 6.480 → 5.169 | 9.532 → 8.278 | 1.709 → 1.180 | 6.017 → 4.715 | 0.243 → 0.243 | 14.461 → 10.636 |
| rtx / 3840x2160 / original | 5 | 6.780 → 5.211 | 9.778 → 7.871 | 1.757 → 1.172 | 6.342 → 4.813 | 0.225 → 0.225 | 14.548 → 10.723 |
| igpu / 1920x1080 / native | 2 | 13.233 → 11.601 | 16.041 → 14.149 | 1.751 → 1.160 | 6.302 → 4.688 | 5.178 → 5.109 | 14.509 → 10.684 |
| igpu / 1920x1080 / original | 2 | 13.081 → 11.497 | 15.674 → 14.138 | 1.760 → 1.160 | 6.311 → 4.944 | 4.968 → 4.938 | 14.548 → 10.722 |
| igpu / 3840x2160 / native | 2 | 16.541 → 15.297 | 18.579 → 17.867 | 1.750 → 1.175 | 6.002 → 4.828 | 8.356 → 8.294 | 14.461 → 10.635 |
| igpu / 3840x2160 / original | 2 | 13.417 → 12.063 | 15.947 → 14.407 | 1.779 → 1.164 | 6.336 → 4.884 | 5.403 → 5.472 | 14.548 → 10.722 |

| Adapter / size / quality | Baseline art-median range ms | Candidate range ms | Paired median reduction % (range) |
| --- | ---: | ---: | ---: |
| rtx / 3840x2160 / native | 6.329–7.433 | 5.019–5.572 | 19.146 (17.562–25.031) |
| rtx / 3840x2160 / original | 6.715–7.898 | 5.084–6.682 | 22.861 (15.405–24.647) |
| igpu / 1920x1080 / native | 13.225–13.241 | 11.513–11.688 | 12.336 (11.728–12.944) |
| igpu / 1920x1080 / original | 13.016–13.145 | 11.350–11.645 | 12.106 (11.415–12.797) |
| igpu / 3840x2160 / native | 16.414–16.668 | 15.210–15.385 | 7.514 (7.331–7.697) |
| igpu / 3840x2160 / original | 13.355–13.479 | 12.037–12.089 | 10.091 (9.868–10.313) |

Original and Native remain above the owner's 4.167 ms / 240 Hz target. The remaining CPU projection and food-plan bursts motivate the separate bounded-worker experiment. AMD is also GPU-limited, especially at Native 4K; this CPU optimization does not remove that GPU cost.

## Input, parity and checks

Restricted replay local identifier: **retained-join-A**, SHA-256 `a9e9bbdaf3c8ebc940f9c75ecfda7aa6aff58d09f396119e13ab0ead16d8cc43`. The input contains 29,044 envelopes and is not distributed. It is processed in 600 virtual 60 Hz callbacks covering ten seconds. All timings are unpaced, synchronized offscreen release diagnostics, including native menu composition and egui CPU/submit cost. They are not displayed-frame intervals, FPS, owner-play acceptance, or a four-core-machine result. The machine is a Ryzen 7 9800X3D; RTX 5070 Ti uses Vulkan and AMD integrated graphics uses DX12. No adapter default changes.

[parity.json](parity.json) retains complete image-sequence hashes: 6,000/6,000 output images are byte-identical to the retained main-with-#447/#448 controls, with 600 callbacks and zero skips in every configuration. This covers the replay on RTX 4K and AMD 4K/1080p, both qualities, plus an authored 25,000-pellet rectangle on both adapters at 4K. The hash driver renders the same world, menu-backdrop composition, textures, shaders and GPU submission path on both sides; it omits the native egui paint when hashing. It does not use an independent renderer. Timed runs do include native egui. Sent-byte command digests match in every timing pair. The timing launcher's canonical-parity flag was off: its `quad_digest` column is the untouched accumulator `d1fba762150c532c`, which establishes no vertex or world-state parity. The historical timing rows are unchanged and that column is never used as a parity gate. The replay and authored fixture include fading, settled and consumed food. No private pixels or identifying layout are retained here.

A retained-record audit checked the launch-time process records and complete 600-row image hashes for all ten configurations against the sealed executables. All 6,000 pairs used two distinct image binaries: baseline SHA-256 `4605ffc678ec41f4b3ff7cd86c88c6720a600b3240328aa99935fca70c52cc30` and candidate SHA-256 `8d7eae072217b9c325a54fcddf04b115ae1ed8356c6a95592d5b467ace98a542`. These are image-control binaries, separate from the timing binaries. Both identities are recorded in `identities.json`; no measurement was repeated for this audit.

The process records bind each image launch to its executable hash, not a driver-source hash. The retained build logs identify the frozen source checkouts for those executables. Their preserved `join_profile.rs` files are byte-identical, both SHA-256 `5d132698641b3fe8c5c98d6eb3179ffd7eb46e1c4891619a12ecfb606293b4b4`; both driver identities are recorded and compared by the check.

Correction, 2026-11-03: a separate untimed, CPU-only default-view control now explicitly enables `PROFILE_PARITY=1` and processes the same 600 callbacks / 29,044 envelopes with the two sealed image-control binaries. [canonical-parity.json](canonical-parity.json) records the nonempty projected-quad digest, canonical-frame/resource-plus-send digest and selected numeric-column hash for each arm, together with launch-record and log hashes. These three controls match across arms. This control reconstructs the canonical stream and does not replace actual image testing. It is separate evidence, with no change to any timing result. The check rejects an inactive flag or the empty accumulator even when both arms contain that same value.

Authored regressions check compact paint numeric bits, frozen sources, public snapshot/Debug shape, isolation of queued snapshots, failed-advance rollback and existing exact packed-rectangle/alpha oracles. `npm run check` passed on this candidate: core tests/Clippy, native tests/strict Clippy, real-Wasm Canvas smoke, shared wgpu Wasm/WebGL-limit builds and profile checks. Actual browser WebGPU/WebGL presentation and owner play were not run.

## Recompute and identities

From this directory, run:

```text
python recompute.py
python recompute.py --check
python -m unittest test_study.py
```

The first command recomputes every aggregate in the table from the per-run CSV. The check verifies that table, pair completeness/assignment, each arm's timing-binary identity, both distinct image-binary identities, identical image drivers, callback and message counts, zero skips, image-sequence equality, the exact ten-configuration image matrix and the separate active canonical control. Tests reject short or skipped parity, arm reassignment, wrong binary hashes, the baseline binary in both image arms, mismatched image drivers, inactive canonical controls and a renamed configuration even when ten entries remain. No recordings are needed and no test contacts a game server.

[identities.json](identities.json) records uninstrumented runtime hashes separately from the measured source hashes and the image-driver identity. Passive food advance/copy counters and exports explain the instrumented core difference; they are present in both timing arms. The image driver adds SHA-256 readback and omits egui painting. Frozen sources, all raw frames, process/load journals, previous pilots and a corrected local path/arm-name tooling failure remain outside Git. Historical hashes are not a routine current-file gate.

Access: **restricted / local only**. The samples, frozen release executables, source snapshots and private `join_profile` offline driver live together in external performance scratch storage. They are not distributed with the repository; an authorized reviewer needs separately granted access. The public CSV, identities and image-sequence hashes above are the bounded evidence for recomputing this report.

The exact timing invocation was the following, with `$sealedReleaseBinary` selecting the sealed baseline or candidate executable identified in `identities.json`, `$restrictedReplayInput` selecting retained-join-A, and `$freshSampleDirectory` selecting a new external sample directory:

```powershell
# All other PROFILE_* variables are unset; in particular no image hashing/readback probe.
$env:PROFILE_ADAPTER='rtx'
$env:PROFILE_BACKEND='vulkan'
$env:PROFILE_SIZE='3840x2160'
$env:PROFILE_MENUS='1'
$env:PROFILE_PHASES='1'
Remove-Item Env:PROFILE_NATIVE -ErrorAction SilentlyContinue
& $sealedReleaseBinary replay $restrictedReplayInput $freshSampleDirectory 10 0
```

For Native, set `PROFILE_NATIVE=1` before that same invocation. For the AMD guardrails, set `PROFILE_ADAPTER=igpu`, `PROFILE_BACKEND=dx12` and `PROFILE_SIZE` to `1920x1080` or `3840x2160`. Each arm runs in a fresh process; pairs alternate baseline/candidate then candidate/baseline. This command consumes a local recording and makes no game-server connection.

For the separate canonical control, use each sealed image-control executable from `identities.json` with `PROFILE_PARITY=1`, `PROFILE_CPU_ONLY=1`, `PROFILE_SIZE=3840x2160` and `PROFILE_MENUS=1`; unset every other `PROFILE_*` variable, then invoke the same `replay $restrictedReplayInput $freshSampleDirectory 10 0` arguments. This hashes canonical frame/resource records and projected vertices without GPU submission. It is intentionally excluded from timing tables.
