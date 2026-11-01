# Native frame workload

## Crowded frame preparation

The minimap compositor retains its opacity-adjusted procedural background.
For each image layer it memoizes the exact scalar byte-to-byte blend for one
present source color in a 1,024-byte table. Transparent pixels need no blend;
other colors keep the scalar operation. Crossfade opacity can change every
picture without repeating floating-point blend/rounding work at every map cell.
Marker position, holder opacity and label placement still apply per picture.
Resource revisions, GPU uploads and picture retirement are unchanged.

Read-only control probes stage the small frame clock before calling control;
the complete world snapshot follows only an accepted callback. Errors retain
the original clock and model, and callbacks still run once before entity motion.

The [crowded preparation study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-01-frame-preparation-reuse/REPORT.md)
records the ranked CPU/offscreen GPU profile, restricted replay and authored
30,000-pellet/40-snake workloads, exact output controls, alternating runs and
rejected texture-array/composition-cache trials. It does not establish gameplay FPS.

## Battledome zoom-out matrix

The finite headless `frame_perf --battledome NEW_DIRECTORY` driver uses authored
packets and procedural placeholder art at default, mid and whole-map wheel zoom,
1080p/4K, and 1x/4x MSAA on each DX12 hardware adapter. It models 120/500-bot arena
sizes, retaining 121/256 live snakes respectively under the current client cap.
The 500 model is not 500 admitted live snakes. It opens no socket or window.

```powershell
$env:CARGO_TARGET_DIR = 'D:/CargoTargets/zoomout-perf'
$env:CARGO_BUILD_JOBS = '2'
cargo build --release --locked -p slither-wgpu-slice --features frame-profile --example frame_perf
& "$env:CARGO_TARGET_DIR/release/examples/frame_perf.exe" --battledome C:/new/external/output
```

Retain each release executable and alternate five runs per side, with no
Cargo/rustc process active during captures. Each cell warms 10 rendered pictures
and measures 30. GPU pass timestamps, CPU prepare/HUD/sync/submit phases, calling
thread allocation bytes, draw calls, vertices and an unclipped quad-area proxy
are recorded. Waits/readbacks are outside CPU timing; this is not gameplay FPS.
`FRAME_PERF_PHASES=1` enables a separate render preparation observer for diagnostic
runs excluded from timing comparisons. It does not extend `latency-trace-v1`.
`FRAME_PERF_NO_PNG=1` omits redundant PNGs; default draw snapshots remain.
`FRAME_PERF_WHOLE_MAP_PNG_ONLY=1` limits presented PNGs to whole-map views while
retaining the full CSV/profile matrix.
`FRAME_PERF_PLAN_ONLY=1` performs workload/budget preflight without timing frames
(adapter/device setup still occurs). All output directories must be new.

The [2026-11-01 shadow study](../../research/client-performance/2026-11-01-zoomout-shadows/REPORT.md)
contains the complete selected matrix, phase results, exactness checks,
loopback-host cross-check and unresolved RTX/AMD limits. The ordinary renderer
does not request timestamp features or allocate query storage.

The non-default `frame-profile` feature enables this example and its render
observer/API-trace instrumentation. Ordinary builds compile every observer
lookup and phase call out. `--battledome` requires `TIMESTAMP_QUERY` and encoder
timestamps (`TIMESTAMP_QUERY_INSIDE_ENCODERS`) for its diagnostic renderer.
An adapter without the required features stops the invocation with exit status
1 and is not measured. A failed invocation retains its newly created output
under a unique `frame-perf-failed-*` directory, freeing the original path for
retry; a pre-existing output directory is never cleaned or overwritten.

### Presented native viewport and GPU-input diagnostics

`--battledome-presented NEW_DIRECTORY` uses the native window-to-backing mapping
and the complete `WorldPresenter`, headlessly, for both Original and Native.
It records physical/backing/attachment extents, DPR, requested/effective mode
and MSAA, plus separate world/blit/overlay GPU timestamps. Native's effective
sample count follows the current shared policy; a requested 4x cell at 4K can
be effective 1x. Compare matching modes/sample counts rather than assuming the
request is the effective result. `FRAME_PERF_DPR` selects DPR (default 1).

`FRAME_PERF_GPU_TRACE_DIR` selects a separately created, empty external trace
root for a **validation-only** run. The pinned wgpu API tracer records actual
buffer/texture write bytes, all resource/pipeline/bind-group descriptors and
ordered pass/draw commands, including the blitter. It runs below the Draw layer.
Payloads are losslessly archived and verified byte-for-byte before retiring
their uncompressed copies. `gpu-trace-json` streams the typed trace for offline
SHA-256 analysis. Trace-enabled timings are excluded from performance tables;
timing runs leave this variable unset. All frame markers and trace hooks are
behind `frame-profile`. Trace validation records one complete rendered picture
per cell, including all preceding resource creation and upload actions; it does
not use the timing run's 10/30 warmup/sample counts.

`FRAME_PERF_FRESH_CONTROL=1`, with `FRAME_PERF_WIDTH` set to 1920 or 3840 and
`FRAME_PERF_MODE` to Original or Native, isolates the 500-model/default control
in a fresh process/device for each chosen case. `FRAME_PERF_CONTROL_ONLY=1`
selects default 500-model cases without the strict one-case isolation.
The diagnostic build feature `frame-profile-no-shadow-cut` forces the candidate
renderer to retain shadows, for the third-binary comparison; it is not a user
quality setting. Record this feature and each executable hash in provenance.

`FRAME_PERF_SHARED_HISTORY=1` runs the same authored one-pixel geometry upload
before each presented cell, outside preparation/submission timing. It fills nine
full vertex chunks (73,728 quads; 8.4375 MiB of GPU vertex capacity) and waits for
completion. Each real frame must fit that fixture ceiling. This conditions both
capacity and unused mutable-buffer contents, so unaffected controls enter with
matching GPU inputs despite different earlier LOD work. CSVs label it
`shared-9-chunks`. Leave it unset for the additional production-history pair
(`sequence`). This is diagnostic setup, not a production cache-sizing change.
Presented CSVs also record retained vertex-buffer capacities for every measured
picture, sampled outside the reported CPU phases.

## Battledome live cross-check

`--battledome` above is the deterministic, socket-free synthetic matrix.
`--battledome-live HOST:PORT NEW_DIRECTORY` instead connects a finite headless
client to a **separately running local simulator**, settles maximum wheel
zoom-out, and saves a whole-map PNG and received/prepared-picture summary.
It does not start the simulator or produce frozen A/B timing data.

Start the simulator with `slither-sim host --config YOUR_LOCAL_CONFIG`, using a
Battledome configuration as described in the [server guide](../../docs/raw-server.md#presets).
Replace `HOST:PORT` below with `127.0.0.1:PORT`, using the simulator's listening
port. The validator accepts only `127.0.0.1`; other loopback IPs, remote hosts
and hostnames are rejected.
Choose a free port other than the reserved 8766, 18884 and 18885. Do not use an
outside game server. The output directory must be new and remain outside Git.

```powershell
& "$env:CARGO_TARGET_DIR/release/examples/frame_perf.exe" --battledome-live HOST:PORT C:/new/external/output
```

## General frame workload

Run the finite Windows offscreen workload from the repository root:

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo build --release --locked -p slither-wgpu-slice --features frame-profile --example frame_perf
target/release/examples/frame_perf.exe C:/new/external/output
```

It enumerates one DX12 adapter per vendor/device pair, excluding CPU adapters.
Set `FRAME_PERF_VULKAN=1` to run the same offscreen cases on Vulkan for pixel checks.
Each adapter runs world, world plus physical HUD, and world/HUD plus
native login at 960x540, using the renderer's normal sample count. The authored
synthetic fixture adds 24 straight 120-point snakes and 2,400 food to the offline
packet fixture. It exercises about 29,800 draw records; this is a density stress
case, not a measurement of official-server populations.

Each scene warms for 40 frames, then records 60 frames in `frames.csv` and three
premultiplied RGBA8 readbacks. Preparation calls production world, HUD and login
composition, including the native retained-picture clone. Simulation and ingress
are frozen to isolate frame preparation. Resource synchronization and render
submission (world plus overlay) are timed separately. GPU completion waits, readbacks and
logging are outside those CPU phase timings. This does not measure simulation,
networking, desktop pacing, scanout or gameplay FPS.

Completed pictures return their owned draw vectors to the bank, matching native
retirement; queued/retried pictures are never lent to the next preparation.

Heap counts are requested Rust allocation/reallocation bytes on the calling
thread, using a wrapper around `System`. They exclude driver heaps, GPU memory,
worker threads and live/resident heap size. The renderer retains application
MSAA targets and vertex buffers; wgpu still owns transient command/upload staging.

Retain both release binaries and alternate five runs per side on the same PC (300 measured frames per scene/side).
Compare readback bytes within each adapter and report per-phase median/p95,
per-frame allocation bytes, and variation across runs. Draw planning, geometry
conversion and GPU submission remain work after cached texture copies disappear.

## High-DPI world workload

The finite `world_resolution_perf` example covers a separate high-DPI world
workload at 1080p, 1440p and 4K, including stock and maximum wheel zoom-in.
It requires a DX12 integrated GPU and uses authored procedural artwork only.
Run `world_resolution_perf --validate-fixture` first for a GPU-free check that
food has completed spawning; see [world resolution](world-resolution.md) for the
policy, sampling method, memory accounting and measurements. Do not collect
timings while other compilers or GPU workloads are running.

## Retained-food CPU preparation

The `frame_perf` driver also has a GPU-free food-validation workload:

```powershell
$env:CARGO_TARGET_DIR = 'D:/CargoTargets/<worktree-folder>'
$env:CARGO_BUILD_JOBS = '1'
cargo build --offline --release --locked -p slither-wgpu-slice --features frame-profile --example frame_perf
& "$env:CARGO_TARGET_DIR/release/examples/frame_perf.exe" --cpu-food C:/new/external/output
```

Both authored synthetic scenes use production `prepare_gameplay`, default automatic
zoom, a frozen simulation/ingress history, 960x540 backing pixels, 40 warmup frames
and 60 measured frames. Each complete picture returns its draw storage to the
bank. No window, GPU, socket, server or external artwork is used. Failures stop
the run; missing/over-budget pictures are never counted as fast frames.

- `stress`: 65 snakes and 65,535 retained food on an 8-unit lattice over roughly
  2048x2048 world units. It exercises validation of retained, including culled,
  food rather than putting every pellet inside the viewport.
- `official-density`: eight snakes and 324 retained food on a 114-unit lattice,
  roughly seven pellets per 300x300 sector. This uses the repository's
  [official-density model](../../docs/configuration.md#official-and-sandbox),
  not a live-server population measurement.

Each scene writes `prepare_ns` and calling-thread allocation bytes to CSV, plus
the complete ordered debug draw records at frames 40, 70 and 99 for exact byte
comparison/SHA-256 hashing. Debug serialization is outside timing. This tests
CPU draw-list equality, not GPU pixels, pacing or perceptual acceptance. Retain
both binaries and alternate five runs per side; report median/p95 across 300
frames per side/scene and the range of each run's median. The existing rendering
workload above remains unchanged.

### Retained-food validation comparison (2026-10-31)

Baseline `5ed5b3e67c96d18f46434171caf782abc1968761`, same driver on both
sides; Windows, Ryzen 7 9800X3D, Rust 1.99.0 release/thin LTO, `feel-trace`
enabled without an attached recorder. Five alternating runs per side, each
40 warmup + 60 measured frames; median/p95 below pool 300 frames per cell.
Other builds were active, so the per-run median ranges retain that variation.
These are CPU picture-build measurements, not low-end hardware or GPU/FPS claims.

| Scene | Before median / p95 (ms) | After median / p95 (ms) | Per-run median range before / after (ms) |
| --- | ---: | ---: | --- |
| Stress, 65 snakes / 65,535 food | 121.8084 / 137.0340 | 11.8132 / 17.2949 | 97.0492–128.4168 / 10.9570–14.5833 |
| Official-density model, 8 snakes / 324 food | 0.8793 / 1.1866 | 0.3013 / 0.4878 | 0.6901–1.0977 / 0.2969–0.3609 |

A separate, excluded counter run incremented the original validator's filter
predicate: each warmed stress frame performed 262,140 source checks and
330,558,540 asset comparisons (1,261 assets). The normal scene performed 1,296
checks and 1,634,256 comparisons. The index replaces each full scan with a hash
lookup, retaining one table-content comparison per plan and rebuilding only
when metadata changes. It does not change food generation, culling or commands.

All 600 measured frame pairs matched retained-food/draw counts and requested
allocation bytes exactly: 35,490,500 bytes per stress picture and 536,240 per
normal picture. Existing picture/planner allocations remain; the warmed index
adds none. All 30 sampled complete ordered draw lists matched byte-for-byte.
Representative shared SHA-256 values at frame 40:

- Stress: `00c12b4756c03fb157addb85e9156e8d45ea2cb36765efc01b6b9177cfcac420`.
- Official density: `da3404bfc2418b8e96c4db75047c5784ec7d3b5b721053cfb26ecf513e1dfac8`.

The before/after native synthetic two-session feel replay, with two authored food
insertions before its first ordinary draw, also matched its complete JSON bytes:
149 samples, 72 acknowledged pictures and seven gameplay command sends.
`feel-perceptual-v1` reported zero mismatches/errors. This is native CPU history
and draw-list exactness, not fresh Canvas/GPU perceptual acceptance. Full CSVs,
counter receipts and binaries remain outside Git.

## Dense singleton motion ingress

The `food_ingress` test `singleton_motion_allocation_is_independent_of_retained_food`
is an authored, socket/GPU-free allocation workload. For both protocols it sends
100 scalar-motion packets with 64 and 6,000 retained foods, then checks allocation
growth and the received/presentation world. It exercises production LocalGame,
including session admission and the canonical decoder:

```sh
cargo test --locked -p slither-wgpu-slice --test food_ingress singleton_motion_allocation_is_independent_of_retained_food -- --exact --nocapture
```

This bounds copying regressions without a wall-time test gate. The
[2026-11-01 motion study](../../research/client-performance/2026-11-01-motion-transactions/REPORT.md)
records the separate restricted-input replay comparison and its remaining limits.
