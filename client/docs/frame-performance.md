# Native frame workload

Run the finite Windows offscreen workload from the repository root:

```powershell
$env:CARGO_BUILD_JOBS = '2'
cargo build --release --locked -p slither-wgpu-slice --example frame_perf
target/release/examples/frame_perf.exe C:/new/external/output
```

It enumerates one DX12 adapter per vendor/device pair, excluding CPU adapters.
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

Heap counts are requested Rust allocation/reallocation bytes on the calling
thread, using a wrapper around `System`. They exclude driver heaps, GPU memory,
worker threads and live/resident heap size. The renderer retains application
MSAA targets and vertex buffers; wgpu still owns transient command/upload staging.

Retain both release binaries and alternate five runs per side on the same PC (300 measured frames per scene/side).
Compare readback bytes within each adapter and report per-phase median/p95,
per-frame allocation bytes, and variation across runs. Draw planning, geometry
conversion and GPU submission remain work after cached texture copies disappear.
