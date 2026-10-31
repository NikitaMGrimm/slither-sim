# World resolution

The native Settings screen defaults to Original; Native/sharp is an opt-in
attachment policy. It changes raster coverage and presentation, not game geometry
or input. `LocalGame::window_viewport` still computes projection
`B = desktop_backing_size(physical / DPR)` using the existing ordered ceilings
and caps, at scale 1. At exact 16:9, B is 1500×845. The presenter borrows the same
Frame; `Frame::quad_iter` still converts against B. Camera, view edges, culling,
curve sampling, draw order, UVs, clip vertices, pointer mapping and packet bytes
are independent of the requested resolution. Quality still applies at Play.

`world_resolution::WorldTargetPlan` separates attachment extent from projection.
Original retains the B-sized MSAA resolve/backing and existing linear blit when
B differs from the surface, including its 1500 backing guard. Native resolves
physical-sized MSAA directly into the acquired presentation image, or draws there
directly at sample count 1. It creates no sampled backing and no blit. Both retain
the current physical-resolution, single-sample HUD/menu overlay loaded over the
world. The public `present`/`present_scaled` entry points default to Original.

Preflight uses actual negotiated texture limits, MAX_EXTENT (8192) and the
unchanged 128 MiB attachment guard, applied to attachment pixels and sample count.
Unsupported Native requests keep Original effective without altering preferences;
resize preflights again. Zero-sized surfaces suspend. Host/device loss still stops
rendering and does not reconnect. Allocation/device failures retain the existing
host-failure policy; capability preflight does not promise allocation under memory
pressure.

`WorldPresenter` is shared by the real surface host and finite offscreen checks.
It validates overlay geometry/extent/attachment before world submission. New
backing and MSAA resources are prepared locally before publication. MSAA publishes
after world submission; backing and effective policy publish after all submissions.
Dropping old handles after submission lets wgpu retain them until queued work
releases its references. Retained pictures own immutable geometry/resources and
can retry under either presentation policy. Rapid changes may retain more than
one previous target in the queue; there is no forced per-frame GPU wait.

## Attachment costs

These calculated byte costs assume RGBA/BGRA8, MSAA4, no depth, B=1500×845,
excluding driver padding, buffers, caches, staging and extra swapchain images.
Original retains 19.34 MiB MSAA plus 4.84 MiB backing (24.18 MiB).

| Physical output | Native pixels | Raster sample area vs Original | Native MSAA4 | One presentation image | Extra persistent world targets |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1920×1080 | 2,073,600 | 1.64× | 31.64 MiB | 7.91 MiB | +7.46 MiB |
| 2560×1440 | 3,686,400 | 2.91× | 56.25 MiB | 14.06 MiB | +32.07 MiB |
| 3840×2160 | 8,294,400 | 6.54× | 126.56 MiB | 31.64 MiB | +102.39 MiB |

Bytes = width × height × 4 × sample count. At 4K, Native MSAA plus one present
image is 158.20 MiB; there is no redundant 31.64 MiB offscreen resolve texture.
Old Original and replacement 4K Native world targets can coexist at 150.74 MiB,
before surfaces/caches and earlier in-flight submissions. Sample count 1 creates
no MSAA target and Native creates no world-owned target. These are allocation and
sample-area calculations, not FPS, power or bandwidth claims. Native can reduce
Original supersampling at small windows, and cannot improve source-art resolution.
Original remains the default; weak integrated-GPU acceptance is separate.

WebGL2 downlevel limits retain adapter resolution via `using_resolution`; the
2048 baseline is not a universal target ceiling. Uploaded/sample assets keep their
independent 2048-per-axis cap. With only the 2048 negotiated baseline, Native at
2560×1440 falls back to Original; it never labels a capped raster Native or changes
the view area. Browser development/reference hosts keep their existing UI and
presentation, while the shared copied parser preserves/migrates v2 preferences.
Future WebGL2 extension work must query drawing-buffer, texture/renderbuffer,
MSAA/resolve support and handle context loss; this PR supplies no GL acceptance.

## Offline evidence

`cargo test --manifest-path apps/wgpu-slice/Cargo.toml --test overlay --test settings`
covers exact projection/frame/clip-bit and steering/boost parity at DPR
1/1.25/1.5/2, retained pictures, strict v2/v1 migration, storage retry, second-toggle
focus and compactness through DPR 5 plus overlays at DPR 6–8. Pure target-policy
tests cover limits, zero sizes, inclusive attachment guard and fallback/recovery.

Explicit GPU checks use
`cargo test --manifest-path apps/wgpu-slice/Cargo.toml --test world_resolution -- --ignored`.
They exercise both sample-count paths, direct resolve without backing/blit,
resize/mode replacement without intervening waits, rejected overlays and retained
output readback. No visible window or game server is used.

`world_resolution_specimens <new-output-directory>` writes authored synthetic
Original/Native PNGs and matched 384×256 physical-pixel edge crops at 1080p, 1440p
and 4K, for samples 1 and 4. `world-resolution.json` separately records projection,
attachment, producer quads, GPU draw runs, passes, resource bytes/slots, persistent
targets, old-plus-replacement transition bytes, one presentation image and CPU
preparation/sync/submission/readback timings. The transition figure is logical
host-owned target accounting for that measured swap, not a driver-residency peak.
CPU submission times exclude PNG encoding and are not GPU timestamps. Physical
input, surface/scanout, weak-GPU performance and perceptual acceptance remain
unverified. Use `CARGO_BUILD_JOBS=2`; full verification remains `npm run check`.
