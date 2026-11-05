# First native egui paint

## Result

On AMD/DX12 the first login paint falls from 59.46 ms to 1.92 ms
(medians of five interleaved pairs). Settings, the in-game menu and the
server picker fall from 44–45 ms to about 2 ms. RTX/Vulkan's already
inexpensive first paints fall from 2–3 ms to below 1 ms. The remaining
first texture/buffer upload and submission cost is small, but first paints
still cost measurably more than warm paints. No startup latency increase
was observed in this offscreen workload.

The native device now starts one helper while constructing its world
pipelines. The helper builds the **actual** egui-wgpu renderer, with the
same device, target format and options. The shared menu host takes that
renderer on its first paint and retains it across menus. GPU initialization
does not wait for the helper; first paint joins it. Context creation, font
atlases, input, layout, tessellation, texture deltas and draw order remain
serial and unchanged. Failure to start the helper or an additional host on
the same device uses the original serial constructor. Wasm has no native
egui host and acquires no helper or new work.

<!-- SUMMARY -->
80 valid runs; 40 pairs; no load-based exclusions. Run median machine CPU load: 30.7–100.0%. Compiler/linker observed in 80 runs. 138 image pairs are byte-identical.
<!-- /SUMMARY -->

<!-- TIMING -->
| Adapter / menu | Pairs | First paint before (ms) | First paint after (ms) | Warm paint after (ms) |
| --- | ---: | ---: | ---: | ---: |
| rtx-vulkan / login | 5 | 2.14 [1.87, 2.79] | 0.57 [0.52, 1.60] | 0.14 [0.10, 0.21] |
| rtx-vulkan / settings | 5 | 2.51 [1.94, 4.84] | 0.68 [0.68, 3.26] | 0.19 [0.16, 0.21] |
| rtx-vulkan / ingame | 5 | 2.07 [2.03, 6.16] | 0.77 [0.54, 1.07] | 0.13 [0.09, 0.19] |
| rtx-vulkan / picker | 5 | 2.47 [2.00, 3.29] | 0.70 [0.64, 1.06] | 0.17 [0.14, 0.22] |
| amd-dx12 / login | 5 | 59.46 [42.62, 78.52] | 1.92 [1.89, 2.10] | 0.24 [0.24, 0.27] |
| amd-dx12 / settings | 5 | 44.91 [41.86, 54.06] | 2.02 [1.63, 2.33] | 0.44 [0.43, 0.48] |
| amd-dx12 / ingame | 5 | 44.16 [40.63, 63.75] | 1.91 [1.76, 2.26] | 0.32 [0.29, 0.34] |
| amd-dx12 / picker | 5 | 44.26 [40.73, 63.35] | 1.86 [1.68, 2.32] | 0.40 [0.37, 0.45] |
<!-- /TIMING -->

Each cell is the median [minimum, maximum] across five process runs.
First paint is the first `NativeEguiHost::paint` plus GPU completion wait;
it includes tessellation, renderer acquisition, textures and submission.
Warm paint is each run's median of paints 2–5 (zero-based), after the first
successor paint. These are synchronized offscreen costs, not presented
frame pacing or desktop play acceptance. All menus use the native initial
960×540 viewport at DPR 1. No other workload or hardware is inferred.

## Startup guard

<!-- STARTUP -->
| Adapter | Runs per arm | GPU initialization before → after (ms) | First complete picture before → after (ms) |
| --- | ---: | ---: | ---: |
| rtx-vulkan | 20 | 256.95 [215.77, 444.79] → 224.60 [214.76, 379.76] | 312.44 [263.49, 510.26] → 272.73 [252.51, 478.86] |
| amd-dx12 | 20 | 622.37 [541.99, 913.12] → 594.17 [559.13, 862.56] | 714.15 [627.23, 1032.48] → 638.25 [602.07, 951.41] |
<!-- /STARTUP -->

These cells pool all four menus, with 20 processes per arm and adapter,
and report median [minimum, maximum]. GPU initialization includes instance,
adapter, device and the existing world pipelines. First complete picture
also includes `NativeLogin` construction, menu preparation, page rendering,
egui painting, GPU completion and readback; it stops before PNG encoding.
The helper overlaps work already on that startup path. Normal machine load
widens the ranges; these observations do not guarantee zero startup cost
on every device or under every scheduler.

## Attribution

Separate instrumented baseline diagnostics, excluded from the timing table,
measured first renderer creation at 31.28 ms on AMD/Vulkan and 88.53 ms on
AMD/DX12. First tessellation was 0.05/0.08 ms, textures 0.30/0.31 ms and
submission 0.64/0.57 ms. Later renderer constructions on the same device
took about 1–1.5 ms. This locates the large cold paint cost in renderer
construction, rather than font atlas generation or layout. The formal
Vulkan series selects RTX and benefits from the normal driver cache;
it is not the AMD/Vulkan diagnostic. The helper removes construction from
the paint thread without changing or pre-running a menu pass.

## Exactness

The existing `menu_specimens` harness rendered login, Settings, server picker
and polish controls, including 1080p/4K, portrait and higher-DPR states.
`ingame_menu_specimens` rendered the authored live-packet world with menus,
in-game Settings and its gameplay control. All 58 PNG files match byte for
byte before/after, establishing identical decoded pixels as well. Selected
PNG and decoded RGBA SHA-256 identities are in `image-controls.json`.
Every cold and warm saved timing image also matches: 80 additional pairs
on RTX/Vulkan and AMD/DX12. No official artwork or art directory was used.

The explicit offscreen regression
`prefetched_and_serial_renderers_have_identical_pixels_and_separate_contexts`
compares the prepared renderer with the serial fallback on the same device,
including texture namespaces, resize, DPR change and reopen. Existing
inactive-host, lifecycle, login and menu/HUD transition tests pass. The
optimization does not prepare hidden menus, draw a HUD, or change the
menu/HUD exclusion policy.

## Method and access

Baseline is main `1921f44c`; both arms use identical authored harness code,
release profile, Rust 1.99.0, `CARGO_TARGET_DIR=D:/CargoTargets/first-egui`
and `CARGO_BUILD_JOBS=2`. `identities.json` binds the base revision, runtime
source snapshots, harness and executables. Complete sources, binaries,
raw timing/load samples, PNGs, diagnostic and failed attempts remain in
`D:/slither-scratch/client-lane-20261105/perf/first-egui/`, outside Git.
The measured candidate and delivered runtime differ only in expanding the
study-reference comment to its canonical URL; both file hashes are retained.
All work is headless/offscreen; no window, server or network connection
was opened by the measurement/render drivers.

The menu harness mode is `menu_specimens OUTPUT --first-egui BACKEND MENU`,
where OUTPUT must not exist, BACKEND is `vulkan` or `dx12`, and MENU is
`login`, `settings`, `ingame` or `picker`. Vulkan requests the high-performance
adapter and DX12 the low-power adapter; `timing.json` records the actual
selection. Paints 0 and 5 are saved as authored PNG controls.

Five pairs per menu/backend alternate A/B and B/A order. Each process
starts a fresh device and context and paints one menu six times. System
CPU and compiler/linker processes are sampled every 250 ms. All valid
runs, including observed compiler overlap, are retained in the primary
normal-load comparison; no quiet window was booked or process stopped.
That policy was declared before the first valid series, replacing the
initial compiler-exclusion plan. `runs.csv` retains the exposure flags.
The shared timing lock records purpose, UTC start and owner PID and is
released in `finally`; each acquisition stayed below 60 minutes.

An initial archive build reused a candidate library from the shared target
because archived source timestamps predated the cached artifacts. Its
80-process series is **invalid**, preserved in scratch and excluded here.
The baseline was rebuilt after refreshing source timestamps, with verbose
Cargo receipts. Its serial implementation and cold DX12 paint were checked
before collecting the replacement series. Early harness compile/path
failures, a failed output-directory attempt and an unsuitable optimized-PE
string assertion are also retained. None supports a performance claim.

## Validation

Focused inactive-host, offscreen GPU exactness/lifecycle, login and menu/HUD
checks passed: 47 tests, including both explicit GPU tests. The complete
`npm run check` passed, including native and Wasm strict Clippy, tests,
desktop/Wasm builds and generated bindings. Checks used the same requested
Cargo target and two build jobs, with command-local dev debug info disabled.
Documentation tests (seven), the staged repository guard and the selected
evidence recomputation passed. No independent review or interactive play
acceptance is claimed.

```text
python -B research/client-performance/2026-11-05-first-egui/recompute.py --check
```
