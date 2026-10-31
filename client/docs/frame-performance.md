# Native frame workload

Run the finite Windows offscreen workload from the repository root:

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo build --release --locked -p slither-wgpu-slice --example frame_perf
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

## Retained-food CPU preparation

The same driver has a GPU-free food-validation workload:

```powershell
$env:CARGO_TARGET_DIR = 'D:/CargoTargets/<worktree-folder>'
$env:CARGO_BUILD_JOBS = '1'
cargo build --offline --release --locked -p slither-wgpu-slice --example frame_perf
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
