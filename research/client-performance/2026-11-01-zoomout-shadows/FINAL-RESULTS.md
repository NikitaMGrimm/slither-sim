# Corrected presented-history results (2026-11-01, round 08)

The fixed candidate removes distant shadows without reproducing the persistent approximately 1 ms control increase from the old binaries. All 72 unaffected actual-input pictures match under shared capacity history; all 96 forced-off pictures match stock. The separate production-history timing pair also lacks that persistent increase. This bounds the **new candidate** in the measured fixture; it does not retrospectively explain the old direct-offscreen regression. Keep the historical AMD issue available for owner review.

## Exact sources and scope

Every table in this document and every `final08-*.csv` uses **rebased-08-stock-candidate**, pinned below. Each CSV row repeats both commits and executable hashes. The forced-off build validates inputs here; its timing discriminator is the separately pinned [round 07](INPUT-RESULTS.md).

- **baseline**: commit `10e553d299fb1666a2ca2fe59952958f4a455259`, source tree `deeabbe775db869596ebf026a4da3042c02d1329`; frame_perf SHA-256 `8542432cf1730e2803bb95abaa054b073e656c63d156e5793e137b54e0783cbd`; features `frame-profile,feel-trace`.
- **candidate**: commit `095449709fc68cb9bdc28c489008fe12c3a5979e`, source tree `8e953b148d6bd54a550e6e00fc44e98e75e54e41`; frame_perf SHA-256 `b81c925413acedcabf4f3f36623d5a961aab3b84c77912b9dcc2db69bbdcb8c2`; features `frame-profile,feel-trace`.
- **forced_off**: commit `095449709fc68cb9bdc28c489008fe12c3a5979e`, source tree `8e953b148d6bd54a550e6e00fc44e98e75e54e41`; frame_perf SHA-256 `d0eb29cdff9461e641d63f60c24e970229f834af59facf45d4f3fe1856a56560`; features `frame-profile-no-shadow-cut,feel-trace`.

All three were built cleanly on main `3669ac9a3a334a3d6d5b767cbe2080646487dc97`, including #404 and #412. The final published head adds a test-only feature-aware expectation; it does not change the measured production code. [Full build commands, toolchain, features, binary hashes, data checksums and crop manifest](final08-provenance.json) are retained. Old retained-02, preliminary-01, A/A/B/A, and round-07 CSVs retain their original mappings.

The authored model uses 120/500 configured arena populations, but at most **256 live snakes** in client state. The 500-model full-map frame sees all 256; it is not a 500-live-snake admission test. Art is procedural placeholder data only. No game socket, original artwork, simulator advance, or ingress participates in the frozen measurements. The earlier local-host subscription cross-check remains in [REPORT.md](REPORT.md).

## Procedure and validation

- Owner window: 12:00–12:40 Europe/Berlin; compiler-free at 12:00:00.016. All 17 runs succeeded: three separate trace validations, ten controlled timing runs (five baseline/candidate pairs), one extra production-history pair, and two separate observer/PNG profiles. GPU work ended 12:31:16. No cargo/rustc overlap. Timing excludes hashes, resource tracing, observer callbacks, PNG readback and setup.
- Each controlled cell first fills nine full VBO chunks (73,728 authored quads; 9 × 983,040 bytes) and waits for the GPU. This common warm-up changes no camera, RNG or animation state. It runs before the 110 CPU-settle and 10 GPU-warm frames, then 30 timed frames. The fixture asserts its draw budget; it never silently grows beyond this history. The extra production pair omits this warm-up. No production cache-sizing change was made.
- Actual wgpu trace validation hashes every uploaded byte and ordered upload length, resource/bind-group/pipeline descriptors and capacities/usages, passes and draws. Shared history matches **72/72 unaffected candidate frames**, including equal-count mid/out cells, and **96/96 forced-off frames**. All controlled capacity vectors are equal. Validation is separate from timing and behind the non-default frame-profile feature.
- **112/112 default Draw snapshots are byte-identical** across five repeated pairs, the production pair and both profiles (16 per pair). The native feel replay remains raw-byte-identical: 148 samples, 72 acknowledged pictures, seven commands, zero errors and zero feel-perceptual-v1 mismatches. The new prepare_gameplay boundary test covers 3.95/4.05 logical px at 1080p/4K, DPR 1/1.5/2 and both resolution policies; normal and all-features variants pass.
- Timed system CPU run medians: 11.6–17.2%. Non-admin AMD ADL clock/activity/throttler samples are retained in [final08-clocks.csv](final08-clocks.csv); they span warmups and both adapters, and do not establish per-pass clocks. GPU timestamps separately bracket world, blit and overlay. No clocks/power settings were changed.

## Controls and history

All 36 unaffected AMD controls and all 36 RTX controls are in [final08-comparisons.csv](final08-comparisons.csv), with per-pass deltas and all five paired ranges; no unaffected cell was dropped. [final08-capacities.csv](final08-capacities.csv) records each history.

| Controls (all 36 per adapter) | Controlled paired-median GPU delta range, ms | Single production-pair GPU delta range, ms |
| --- | ---: | ---: |
| AMD integrated | -0.165 to +0.160 | -0.430 to +0.203 |
| RTX 5070 Ti | -0.082 to +0.045 | -0.418 to +0.025 |

For the highlighted 500/default/1080p Original/requested-4x AMD control, the five-pair median is **+0.019945 ms** (individual pairs −0.298650 to +0.521115 ms); the production pair is **+0.081550 ms**. World-pass paired median is +0.015755 ms; blit +0.000200 ms; overlay +0.000220 ms. These are larger excursions than the old A/A noise floor, but none retains the old 1.081 ms median / 0.993–1.641 ms range. Do not label the old increase noise.

The original trace audit proved a grow-only capacity-history difference: the same 392,520-byte vertex upload used a 983,040-byte stock buffer versus a 524,288-byte candidate buffer. Controlled history removes that input difference. The extra round-08 production pair retains differing resource histories but does not reproduce an approximately 1 ms penalty. One pair cannot prove absence of a zoom-out/back-in regression under all play histories. The cache is unchanged because these measurements do not justify a production sizing change; fresh-process or interactive history testing remains a useful follow-up.

## Full matrix (requested 4x)

Cells use DPR 1. Original projection/backing is 1500×845 at both physical sizes and retains 4x. Native has the same projection but physical-sized attachments: 1920×1080/4x or 3840×2160/**effective 1x** under #412. Overlay is 1x. The logical shadow threshold uses physical/DPR display units independently of MSAA. [final08-selected.csv](final08-selected.csv) includes both requested 1x and 4x, every default/mid/out cell, all backing/attachment/physical dimensions, GPU passes, CPU phases, vertices, draws, allocation bytes and geometric quad-area proxy. No hardware fill-rate counter is claimed.

The following medians pool 150 timed frames per side from five controlled pairs; CPU is prepare + HUD + resource sync + submit. Allocation means requested CPU bytes on the caller thread, not resident heap/driver/GPU memory. GPU is total timestamped world/blit/overlay; it is not display latency.

| GPU | Model | Physical | Mode | Zoom | CPU ms stock → cut | GPU ms stock → cut | Draw calls stock → cut | Alloc MiB stock → cut |
| --- | ---: | --- | --- | --- | ---: | ---: | ---: | ---: |
| AMD | 120 | 1920×1080 | Native (4x) | default | 1.57 → 1.54 | 2.51 → 2.40 | 915 → 915 | 5.44 → 5.44 |
| AMD | 120 | 1920×1080 | Native (4x) | mid | 4.25 → 4.31 | 3.81 → 3.84 | 4,431 → 4,431 | 7.39 → 7.39 |
| AMD | 120 | 1920×1080 | Native (4x) | out | 23.79 → 11.28 | 9.28 → 3.98 | 27,966 → 14,472 | 23.59 → 11.98 |
| AMD | 120 | 1920×1080 | Original (4x) | default | 1.56 → 1.64 | 2.22 → 2.18 | 916 → 916 | 5.44 → 5.44 |
| AMD | 120 | 1920×1080 | Original (4x) | mid | 4.26 → 4.52 | 3.55 → 3.46 | 4,432 → 4,432 | 7.40 → 7.40 |
| AMD | 120 | 1920×1080 | Original (4x) | out | 24.27 → 11.54 | 8.81 → 3.56 | 27,967 → 14,473 | 23.60 → 12.00 |
| AMD | 120 | 3840×2160 | Native (1x) | default | 1.62 → 1.62 | 4.24 → 4.28 | 915 → 915 | 5.44 → 5.44 |
| AMD | 120 | 3840×2160 | Native (1x) | mid | 4.26 → 4.29 | 5.13 → 5.13 | 4,431 → 4,431 | 7.39 → 7.39 |
| AMD | 120 | 3840×2160 | Native (1x) | out | 24.29 → 24.55 | 13.79 → 13.81 | 27,966 → 27,966 | 23.59 → 23.59 |
| AMD | 120 | 3840×2160 | Original (4x) | default | 1.59 → 1.60 | 2.23 → 2.24 | 916 → 916 | 5.45 → 5.45 |
| AMD | 120 | 3840×2160 | Original (4x) | mid | 4.36 → 4.39 | 4.22 → 4.24 | 4,432 → 4,432 | 7.40 → 7.40 |
| AMD | 120 | 3840×2160 | Original (4x) | out | 24.53 → 24.65 | 9.45 → 9.46 | 27,967 → 27,967 | 23.60 → 23.60 |
| AMD | 500 | 1920×1080 | Native (4x) | default | 2.49 → 2.51 | 2.53 → 2.57 | 669 → 669 | 13.22 → 13.22 |
| AMD | 500 | 1920×1080 | Native (4x) | mid | 4.57 → 4.57 | 3.36 → 3.36 | 3,221 → 3,221 | 15.24 → 15.24 |
| AMD | 500 | 1920×1080 | Native (4x) | out | 45.47 → 16.52 | 15.05 → 3.77 | 52,628 → 23,822 | 49.36 → 26.10 |
| AMD | 500 | 1920×1080 | Original (4x) | default | 2.52 → 2.49 | 2.27 → 2.30 | 670 → 670 | 13.23 → 13.23 |
| AMD | 500 | 1920×1080 | Original (4x) | mid | 4.66 → 4.76 | 3.06 → 3.06 | 3,222 → 3,222 | 15.25 → 15.25 |
| AMD | 500 | 1920×1080 | Original (4x) | out | 45.06 → 16.30 | 13.79 → 2.48 | 52,629 → 23,823 | 49.38 → 26.11 |
| AMD | 500 | 3840×2160 | Native (1x) | default | 2.46 → 2.65 | 4.96 → 4.80 | 669 → 669 | 13.22 → 13.22 |
| AMD | 500 | 3840×2160 | Native (1x) | mid | 4.50 → 4.75 | 4.28 → 4.19 | 3,221 → 3,221 | 15.24 → 15.24 |
| AMD | 500 | 3840×2160 | Native (1x) | out | 44.99 → 15.99 | 19.22 → 8.73 | 52,628 → 23,822 | 49.36 → 26.10 |
| AMD | 500 | 3840×2160 | Original (4x) | default | 2.40 → 2.54 | 2.72 → 2.90 | 670 → 670 | 13.23 → 13.23 |
| AMD | 500 | 3840×2160 | Original (4x) | mid | 4.53 → 4.66 | 3.57 → 3.57 | 3,222 → 3,222 | 15.25 → 15.25 |
| AMD | 500 | 3840×2160 | Original (4x) | out | 44.59 → 15.83 | 14.46 → 3.13 | 52,629 → 23,823 | 49.38 → 26.11 |
| RTX | 120 | 1920×1080 | Native (4x) | default | 1.59 → 1.51 | 0.19 → 0.18 | 915 → 915 | 5.44 → 5.44 |
| RTX | 120 | 1920×1080 | Native (4x) | mid | 4.14 → 4.04 | 0.73 → 0.71 | 4,431 → 4,431 | 7.39 → 7.39 |
| RTX | 120 | 1920×1080 | Native (4x) | out | 23.28 → 11.03 | 5.32 → 0.18 | 27,966 → 14,472 | 23.59 → 11.98 |
| RTX | 120 | 1920×1080 | Original (4x) | default | 1.56 → 1.49 | 0.19 → 0.19 | 916 → 916 | 5.44 → 5.44 |
| RTX | 120 | 1920×1080 | Original (4x) | mid | 4.14 → 3.98 | 0.72 → 0.71 | 4,432 → 4,432 | 7.40 → 7.40 |
| RTX | 120 | 1920×1080 | Original (4x) | out | 22.44 → 10.91 | 5.24 → 0.16 | 27,967 → 14,473 | 23.60 → 12.00 |
| RTX | 120 | 3840×2160 | Native (1x) | default | 1.59 → 1.46 | 0.22 → 0.21 | 915 → 915 | 5.44 → 5.44 |
| RTX | 120 | 3840×2160 | Native (1x) | mid | 4.13 → 3.93 | 0.73 → 0.70 | 4,431 → 4,431 | 7.39 → 7.39 |
| RTX | 120 | 3840×2160 | Native (1x) | out | 23.36 → 21.91 | 5.30 → 5.23 | 27,966 → 27,966 | 23.59 → 23.59 |
| RTX | 120 | 3840×2160 | Original (4x) | default | 1.56 → 1.47 | 0.21 → 0.21 | 916 → 916 | 5.45 → 5.45 |
| RTX | 120 | 3840×2160 | Original (4x) | mid | 4.03 → 3.94 | 0.74 → 0.74 | 4,432 → 4,432 | 7.40 → 7.40 |
| RTX | 120 | 3840×2160 | Original (4x) | out | 22.49 → 21.73 | 5.26 → 5.26 | 27,967 → 27,967 | 23.60 → 23.60 |
| RTX | 500 | 1920×1080 | Native (4x) | default | 2.52 → 2.37 | 0.14 → 0.14 | 669 → 669 | 13.22 → 13.22 |
| RTX | 500 | 1920×1080 | Native (4x) | mid | 4.46 → 4.26 | 0.61 → 0.60 | 3,221 → 3,221 | 15.24 → 15.24 |
| RTX | 500 | 1920×1080 | Native (4x) | out | 42.96 → 15.46 | 11.09 → 0.20 | 52,628 → 23,822 | 49.36 → 26.10 |
| RTX | 500 | 1920×1080 | Original (4x) | default | 2.43 → 2.33 | 0.15 → 0.15 | 670 → 670 | 13.23 → 13.23 |
| RTX | 500 | 1920×1080 | Original (4x) | mid | 4.42 → 4.38 | 0.61 → 0.62 | 3,222 → 3,222 | 15.25 → 15.25 |
| RTX | 500 | 1920×1080 | Original (4x) | out | 41.79 → 15.72 | 10.97 → 0.17 | 52,629 → 23,823 | 49.38 → 26.11 |
| RTX | 500 | 3840×2160 | Native (1x) | default | 2.60 → 2.24 | 0.23 → 0.23 | 669 → 669 | 13.22 → 13.22 |
| RTX | 500 | 3840×2160 | Native (1x) | mid | 4.45 → 4.16 | 0.59 → 0.59 | 3,221 → 3,221 | 15.24 → 15.24 |
| RTX | 500 | 3840×2160 | Native (1x) | out | 42.28 → 15.48 | 11.01 → 0.37 | 52,628 → 23,822 | 49.36 → 26.10 |
| RTX | 500 | 3840×2160 | Original (4x) | default | 2.55 → 2.41 | 0.17 → 0.17 | 670 → 670 | 13.23 → 13.23 |
| RTX | 500 | 3840×2160 | Original (4x) | mid | 4.57 → 4.35 | 0.64 → 0.63 | 3,222 → 3,222 | 15.25 → 15.25 |
| RTX | 500 | 3840×2160 | Original (4x) | out | 42.03 → 15.85 | 11.05 → 0.20 | 52,629 → 23,823 | 49.38 → 26.11 |

At 500/full-map/1080p Original 4x, pooled AMD GPU time falls 13.79 → 2.48 ms (82.0%); CPU frame work 45.06 → 16.30 ms; draw calls 52,629 → 23,823 (54.7%); requested allocations 49.38 → 26.11 MiB. Vertices fall 399,012 → 247,710. Summed unclipped quad area falls only 1.953 → 1.902 times the viewport area: the large GPU saving is consistent with reduced interleaved shadow/body state switches and submissions rather than a large fill-area reduction. This is an area proxy, not a hardware overdraw/fill counter. The saving lands in the world pass; the blit/overlay deltas are negligible. The 120/4K bodies remain above four displayed logical pixels, so their shadows stay.

## CPU profile and pixels

Separate instrumented profiles are in [final08-phases.csv](final08-phases.csv). These are diagnostic per-stage medians, excluded from the table above. At AMD 500/full-map/1080p Original 4x:

| Prepare stage | Stock ms | Cut ms |
| --- | ---: | ---: |
| curves | 2.090 | 1.674 |
| bodies | 2.400 | 2.057 |
| food | 0.238 | 0.180 |
| prey | 0.185 | 0.182 |
| names | 0.002 | 0.001 |
| minimap | 0.025 | 0.019 |
| resources | 0.696 | 0.295 |
| build | 2.517 | 1.617 |

Curves and body planning still scale with the visible set. This focused change primarily reduces Draw serialization, shadow/body texture switches, vertex uploads and GPU world work; it does not change culling, glyph policy, food/prey painting or minimap semantics. Profile-only changes in those untouched phases are not attributed to the cut. The uninstrumented CSV includes HUD work and allocations separately.

Matched full-map crop, same source coordinates and native pixel scale, authored geometry and procedural placeholder art only. Both source PNG hashes and exact source commits are in the crop manifest.

![Full-map 500-model crop, stock and corrected distant-shadow cut](https://github.com/user-attachments/assets/e0825b3e-9651-48b5-969e-396c2817029e)

Pixels intentionally change only where the zoom-out eligibility allows the shadow cut. This crop supports a scale-specific visual judgment, not formal GPU perceptual acceptance. RTX sparse-pixel readbacks were nonrepeatable even for unchanged retained baselines; the cause remains unresolved. No owner-play, Canvas/browser GPU parity, pacing, or 500-live-snake admission acceptance is claimed. Raw attempts, raw traces, telemetry, failed captures and archive/restore manifests stay in scratch; verified lossless groups preserve their original bytes and paths.
