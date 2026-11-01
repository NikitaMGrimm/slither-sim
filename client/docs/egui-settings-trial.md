# Native egui Settings trial — 2026-11-01

This trial replaces one native screen while retaining the existing settings
model, record format, immediate preference updates, persistence adapter and Play
snapshot. Other native menus retain the old toolkit; browser menus retain their
DOM controls. It establishes a working headless candidate, not acceptance of a
full menu migration or of physical desktop input/text quality.

## Question and result

Can flow layout and one widget focus owner make Settings easier to change while
keeping the existing look and behavior? The two options and Done now use egui
responses, with authored pill/check/shadow painting using the existing theme and
installed font bytes. No official-derived source, image pack or art directory was
read or rendered. All selected images contain this project's geometry/text only.

The trial adds 781 and removes 520 production Rust diff lines
(net +261, including comments/blank lines), counting the
Settings model/view changes and native input, font and GPU integration together.
The Settings-only layout and legacy Toggle rendering/input plumbing are removed.
The old panel slicer remains because the server picker uses it. Dependencies,
lockfile, tests, documentation and these trial notes are excluded from that code count;
the per-file counts are in [measurements.json](egui-settings-trial/measurements.json). Coexistence
charges all egui integration/custom chrome to this screen; it does not remove the
roughly 9k-line toolkit still used by other menus.

The release executable grows by 3,605,504 bytes. The warmed
rebuild measurements do not establish a useful speed improvement: one timed
layout edit per side is insufficient. These results and the two-size captures
support reviewing the trial; broader migration remains inconclusive without
owner appearance/feel acceptance and an editable-text/weak-GPU trial.

## Measurements

| Metric | Before | After |
| --- | ---: | ---: |
| Warm dev rebuild after width 560 → 568 (seconds, one run) | 7.270 | 5.655 |
| Native release executable (bytes) | 12,798,464 | 16,403,968 |
| Native executable, gzip-9 bytes | 5,176,431 | 6,729,418 |
| WebGPU Wasm after bindgen, raw bytes | 3,199,830 | 3,193,574 |
| WebGPU Wasm after bindgen, gzip-9 bytes | 1,051,337 | 1,049,640 |
| GPU-only menu time | Not measured | Not measured |

The WebGPU shipping-Wasm deltas are -6,256 bytes raw and
-1,697 bytes gzip. gzip uses level 9 and mtime 0 on both
artifacts. Compiler-output Wasm sizes/digests are also retained in the data;
the table uses the post-bindgen `_bg.wasm`, excluding JS glue. This does not
claim a separately measured Canvas Wasm size delta.

Five alternating runs per implementation, each with 1,000 warmed preparation
samples and 120 complete menu CPU-frame samples at each size. Values below are
the median of the five run statistics, followed by their minimum/maximum.

| Menu timing | Before: median [range] | After: median [range] |
| --- | ---: | ---: |
| 960x540 prepare median (ms) | 0.0194 [0.0193, 0.0194] | 0.0219 [0.0215, 0.0227] |
| 960x540 frame cpu median (ms) | 0.1169 [0.0728, 0.1184] | 0.1564 [0.1499, 0.1603] |
| 960x540 frame cpu p95 (ms) | 0.2166 [0.1333, 0.2301] | 0.2503 [0.1909, 0.2842] |
| 360x640 prepare median (ms) | 0.0194 [0.0194, 0.0195] | 0.0217 [0.0212, 0.0222] |
| 360x640 frame cpu median (ms) | 0.0713 [0.0680, 0.0769] | 0.1506 [0.1137, 0.1579] |
| 360x640 frame cpu p95 (ms) | 0.1607 [0.1286, 0.2180] | 0.2424 [0.2104, 0.3197] |

The frame timer includes `NativeLogin::prepare_host`, resource synchronization,
encoding and queue submission, including the egui pass. GPU completion is waited
after every submission sample, outside the timed CPU region. There is no world,
socket, window, presentation/VSync wait or PNG readback inside the timer. Input
callbacks are not part of these idle-menu samples. These are descriptive timings
on a shared Windows host; no CPU profile or causal speedup claim is supplied.

The actual offscreen adapter reported `IntegratedGpu` and the production renderer
rejects software/CPU adapters. The warm menu CPU costs are small against a
16.67 ms / 60 Hz budget at these two 1× sizes. They do not establish GPU frame
completion, weak-iGPU acceptance, cold font/resize cost or high-DPI GPU cost.
The production device requests no timestamp-query feature, so this run has no
GPU-only duration. Geometry/keyboard properties are exercised through DPR 8;
the selected GPU PNGs and timings are DPR 1.

## Selected renders

Both sides use High quality / Original, no cosmetics and initial Quality focus.
The old toolkit suppresses its initial ring under pointer modality; egui shows
one outline. That is the actual initial-open behavior, not an injected pixel
change. These are appearance specimens, not pixel-golden tests or reproduction
of every historical focus/shadow bug.

![960x540 comparison](https://github.com/user-attachments/assets/c30405f1-ae70-4b78-acf1-2e41df9b02c6)

![360x640 comparison](https://github.com/user-attachments/assets/928d495d-8679-427a-ac1d-09adc726c4d8)

The two comparison PNGs are public GitHub user attachments in the authorized
repository; no PR-only media is committed. Original offscreen PNGs, binaries,
full run directories and failed-attempt logs remain local-only in the task
scratch directory. Figure and raw-image digests are recorded with the measurements.
The refreshed baseline and trial PNGs match the source images for those figures
byte for byte after rebasing over PR #398.

## Layout-edit example

Widening the panel is one line, with no control rectangles to update:

```diff
-            let width = 560_f32.min((area.x - 48.).max(1.));
+            let width = 600_f32.min((area.x - 48.).max(1.));
```

Widening was also one constant in the old toolkit. The useful change is that
row order now follows the description array and wrapped text/scrolling takes
space in flow; it no longer needs row-number-based positions and help offsets.
Tests assert properties rather than freezing those positions. A deliberate row
reorder also updates the explicit modal Tab list.

## Problems and limits

- egui 0.37 changes the context/style/modifier APIs. Its `TexturesDelta` must be
  consumed or explicitly cleared. An initial adapter discarded a delta and
  aborted on teardown; moving deltas out of output, applying them before paint,
  and explicitly clearing unsubmitted CPU-only teardown state fixes that error.
- Built-in Tab traversal has an unfocused boundary frame. Settings wraps its
  three stable egui IDs immediately. Animated scroll initially failed a stronger
  short-window reachability test; keyboard reveal now snaps into view. The first
  full check failed that pre-fix property; its log is retained, and the final full
  check was rerun.
- Repeated activations, outside-click focus loss and stale pointer release on a
  new mount need explicit handling. The native adapter processes input per winit
  callback, preserving quick activation order. Blur, resize and DPI changes
  cancel pointer capture. Late egui output cannot take IME from returning controls.
- Installed bold fonts did not cover the chosen checkbox characters; the checks
  are painted geometry instead. Native Lucida/Arial Bold bytes are reused without
  bundling fonts. egui's text rasterizer differs from fontdue; visual similarity
  in these specimens is not a text-quality or physical-DPI acceptance result.
- egui's own area fade and keyboard zoom are disabled. The screen uses the
  shared `menu_transition::step` timing from merged PR #398: nominally
  400 ms in, 320 ms out, with a 40 ms step cap. Its SettingsState remains the
  transition owner. The initial 230 ms choice was corrected before measurements;
  all reported measurements were refreshed after rebasing over #398.
- Fractional DPI could reconstruct a 453-pixel viewport as 452 pixels at 1.75x.
  The conversion now rounds, with a regression that fails on the old code and
  passes on the correction.
- Settings has no editable field. The nickname IME composition gate is tested,
  but this trial does not establish egui nickname/champion editing, clipboard or
  platform IME behavior. Native OS accessibility bridging remains unimplemented,
  as before; checkbox/button widget metadata is present.
- Browser menus still use their DOM adapters; this trial does not validate egui
  browser input or rendering. A future browser migration also needs a licensed
  font-byte source: this native adapter reads installed fonts, while the existing
  browser uses platform text.
- Stock egui widgets do not reproduce this theme's pills. Custom capsule mesh,
  check and shadow painting is still needed. No needed Settings behavior proved
  impossible; maintenance savings for the remaining menus are unmeasured.
- The pinned wasm-bindgen CLI was absent from PATH. An existing verified 0.2.129
  executable was selected per command, without installation or global changes.

## Dependencies and checks

`egui`, `egui-wgpu` and `egui-winit` are exactly `=0.37.0`, all with defaults off,
on the existing native winit targets. The new egui dependencies enable no bundled
fonts, clipboard, link handling, X11/Wayland or second wgpu/winit version.
`cargo tree` confirms one
wgpu 30.0.1 and one winit 0.30.13. Their published license is MIT OR Apache-2.0:
[egui](https://docs.rs/crate/egui/0.37.0),
[egui-wgpu](https://docs.rs/crate/egui-wgpu/0.37.0),
[egui-winit](https://docs.rs/crate/egui-winit/0.37.0).

[licenses.json](egui-settings-trial/licenses.json) records all 34 new lockfile packages. Permissive
MIT/Apache choices are available, including Apache-2.0 for `self_cell` (its GPL
alternative is not selected). `unicode-properties` also requires Unicode-3.0
notices. Apple-only lockfile additions were checked from their crate manifests.
There is no separate client dependency/license-check command; the existing
repository guards and full client check command were run.

Focused shared-model Settings (5), picker (15) and native Settings (13) tests,
strict native all-target Clippy, headless hardware-GPU specimens, and final
`npm run check` pass. That command
covers JS tests, core Rust tests/Clippy, both Wasm builds and real Canvas Wasm ABI,
native wgpu tests/Clippy/desktop build, and strict Wasm Clippy. Repository source
guards are run again against the staged report/data before commit. Hardware-GPU tests
marked ignored by the standard suite remain ignored; the explicit specimen path
is the GPU check here. No outside game server, original client, desktop window,
computer-use session or banned port was used. Automated CodeRabbit review
identified the fractional-DPI correction; no separate agent or maintainer review
was commissioned.

## Reproduce and source pins

Baseline: `3de1ebcfba4b87bc942de4fc893f1440747b4b58` plus the selected
[baseline instrumentation patch](egui-settings-trial/baseline-harness.patch). The final implementation
and build-input file digests, binary digests, per-file line counts, all ten run
summaries and PNG digests are in [measurements.json](egui-settings-trial/measurements.json).
The baseline patch retains equivalent instrumentation for the old code; it is
not applied to the trial. The preserved measured executable is identified by its
digest, rather than asserting deterministic reproduction of linker bytes.

Build on Windows with Rust 1.99.0, per-command
`CARGO_TARGET_DIR=D:/CargoTargets/egui-settings-spike` and `CARGO_BUILD_JOBS=2`.
Release uses the unchanged workspace thin-LTO / one-codegen-unit profile. For
incremental measurements both sides use dev, `CARGO_PROFILE_DEV_DEBUG=0`, a warm
build, the one-line width edit, then restoration of the original source.
wasm-bindgen is 0.2.129 on both sides. The target directory is the same throughout.

```powershell
cargo build --locked --release -p slither-wgpu-slice --bin slither-wgpu-desktop --example menu_specimens
cargo build --locked --release -p slither-wgpu-slice --lib --target wasm32-unknown-unknown
# Run the built specimen executable with a new, external directory:
menu_specimens <new-output-directory> --settings-trial
# From client/, with the same per-command environment and pinned WASM_BINDGEN:
npm run check
node tools/check-repository.mjs
```

No measurement script connects to a server. The PNG/CPU path calls production
Settings preparation and painting with `None` for cosmetics. Twenty-five frames
warm the opened panel before timing. Full captures/logs/binaries are intentionally
unpublished; the selected images/data here are enough to recompute these tables.
