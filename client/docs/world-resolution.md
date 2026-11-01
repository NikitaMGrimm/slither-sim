# World resolution

The native Settings screen defaults to Original; Native/sharp is an opt-in
attachment policy. Keep this toggle rather than making Auto the default: probing
or upgrading raster coverage can add work on weak iGPUs, and one synthetic
workload cannot guarantee spare frame budget in gameplay. Original keeps the
exact 1080p default. Native now bounds its persistent MSAA allocation: use the
renderer-supported sample count while the MSAA4 target fits **64 MiB**, otherwise
use single-sample physical coverage. This keeps 1080p/1440p Native at MSAA4 and
4K Native at sample count one, without a large world-owned attachment. The
64 MiB allowance is an authored memory policy, not a fitted FPS threshold.
No timing probe, automatic mode switching or texture upload occurs on resize;
the attachment decision is stable at a fixed size and cannot oscillate with load.
Settings storage and the existing Native/sharp control remain unchanged.

The policy changes raster coverage and presentation, not game geometry or input.
`LocalGame::window_viewport` still computes projection
`B = desktop_backing_size(physical / DPR)` using the existing ordered ceilings
and caps, at scale 1. At exact 16:9, B is 1500×845. The presenter borrows the same
Frame; `Frame::quad_iter` still converts against B. Camera, view edges, culling,
curve sampling, draw order, UVs, clip vertices, pointer mapping and packet bytes
are independent of the requested resolution. Quality still applies at Play.

`slither_client::world_resolution::WorldTargetPlan` owns the pure shared policy
and separates attachment extent from projection.
Original retains the B-sized MSAA resolve/backing and existing linear blit when
B differs from the surface, including its 1500 backing guard. Native resolves
physical-sized MSAA directly into the acquired presentation image, or draws there
directly at sample count 1. It creates no sampled backing and no blit. Both retain
the current physical-resolution, single-sample HUD/menu overlay loaded over the
world. The public `present`/`present_scaled` entry points default to Original.

Preflight uses actual negotiated texture limits, MAX_EXTENT (8192) and the
unchanged 128 MiB hard attachment guard, applied to attachment pixels and the
selected sample count. Native availability includes its sample-count-one path;
4096×2160 no longer falls back merely because MSAA4 exceeds the guard.
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

These calculated costs assume RGBA/BGRA8, no depth, B=1500×845, excluding
driver padding, buffers, caches, staging and extra swapchain images. Original
retains 19.34 MiB MSAA4 plus 4.84 MiB backing (24.18 MiB) at all three outputs.

| Physical output | Original upscale per axis (x / y) | Native pixels vs Original | Previous Native MSAA4 | Current Native samples / world targets | One presentation image |
| --- | ---: | ---: | ---: | ---: | ---: |
| 1920×1080 | 1.280 / 1.278 | 1.64× | 31.64 MiB | 4 / 31.64 MiB | 7.91 MiB |
| 2560×1440 | 1.707 / 1.704 | 2.91× | 56.25 MiB | 4 / 56.25 MiB | 14.06 MiB |
| 3840×2160 | 2.560 / 2.556 | 6.54× | 126.56 MiB | 1 / 0 MiB | 31.64 MiB |

Bytes = width × height × 4 × sample count. Native never needs an offscreen resolve
or sampled backing: at sample count one it writes directly into the presentation
image. At 4K this removes the previous 126.56 MiB MSAA target (158.20 → 31.64 MiB
including one presentation image). A transition from Original to 4K Native can
retain Original's 24.18 MiB until queued work retires; no new world target is
allocated. This is logical ownership accounting, not a driver-residency peak.
MSAA4's inclusive 64 MiB boundary is 4,194,304 pixels, independent of aspect
ratio and DPR. Physical output already incorporates DPR; do not multiply twice.
On sample-count-one devices both modes retain their existing single-sample path.

MSAA covers geometry edges, not missing texels inside alpha sprites. Native at
4K still fills 6.54 times as many world pixels as Original, so it remains opt-in
even without MSAA. Native can reduce Original supersampling at small windows,
and does not itself improve source-art resolution. Desktop pacing, power and
other weak-iGPU workloads need separate acceptance.

## Source textures versus raster resolution

`bank.rs` materializes only the current picture's used procedural sources and
shares immutable rasters with retained pictures; this is already demand loading,
not an eager HD pack. `gpu.rs` uploads premultiplied encoded-sRGB RGBA8 with **one
mip level**, linear minification/magnification and clamp-to-edge (repeat for the
background pattern). The 2048-axis upload cap, 64 MiB active/resident pixel budget
and 2048-resource bound are separate from attachment limits. Mipmaps would help
minification at zoom-out, not magnification at 4K or zoom-in, and would add up to
about one third to texture residency. Enabling them globally would also change
the 1080p default filtering.

| Source family | Current raster dimensions | Source and scaling behavior |
| --- | --- | --- |
| Shaded skin/body variants | 48×48 | Authored analytic color raster, circular coverage and optional stars; 7 variants per ordinary palette, 60 for the cycle palette. Same sources for built-in/custom skin runs. |
| Body boost / outline / shadow / death | 62×62 / 52×52 / 62×62 / 32×32 | Analytic falloffs or authored circle; glow softness is intentional. |
| Food body / outline / glow | 4–25 / 26–49 / 29–157 square | 17 creation-selected source sizes per palette; radial gradients/circle shadows. Wheel zoom changes destination extents, not source selection. |
| Prey body / glow | 44–86 / 29–157 square | 22 circle/shadow bodies and the 17 food glow sources per palette. |
| Paired eyes/pupils | 64×64 disk per used color | Authored circle; normally strongly minified. |
| Procedural cosmetic roles | Eye base 64×64, pupil/eye marks 48×48, bulbs 64×64 or 84×84 | Authored analytic pixels, gradient or coverage raster. Generated even without an art pack. |
| Background | Tile 324×192, uniform backdrop 512×512 | Authored paths/flat color. A flat color has no missing detail when enlarged. Arena/other solid quads likewise have no source-resolution limit. |
| Runtime cosmetic images | Fixed face/bulb canvases: 79×130 through 190×188; accessories retain decoded dimensions | Optional owner-supplied PNG/JPEG only, max 1024 per axis, 4 MiB encoded, 16 MiB combined cosmetic pixels. Existing insets/shadows and source crops stay authoritative. Backdrop retains its 512×512 crop. |
| Menu art / studio thumbnails | Menu up to 2048 per axis; thumbnails 128×128 | Separate semantic runtime loading/windowed thumbnail rules; physical HUD/menu raster remains independent of the world mode. |

The exact magnification of an axis is `abs(destination extent) / source-crop
extent × attachment extent / projection extent` (also apply `viewport_scale`
when not one). For repeating backgrounds use the UV span and texture extent;
for a rotated quad use its transformed edge lengths. Surface size alone cannot
determine which sprite is magnified: snake size, stock camera zoom, creation-time
food choice, source crops and the user's extra zoom all matter. A 4K Original
picture first samples sprites at B and then stretches that entire raster by
about 2.56×; Native removes that second reconstruction but preserves the texel
limit of each source.

All procedural families could be regenerated from their own analytic/path
sources at a larger sampling grid; scaling their existing pixel buffers adds no
detail. Doing this safely also needs matching immutable resource identities,
logical source/crop metadata, retained-picture ownership and bounded cold-path
CPU work. A blanket 2×/4× source multiplier costs 4×/16× pixels, including large
glow padding, and cannot recover detail in optional external bitmaps. Keep
existing source sizes/loading in this change, rather than adding synchronous HD
rasterization or new private-pack file conventions to an iGPU resolution fix.
No official artwork is bundled, read for these measurements or published.

WebGL2 downlevel limits retain adapter resolution via `using_resolution`; the
2048 baseline is not a universal target ceiling. Uploaded/sample assets keep their
independent 2048-per-axis cap. With only the 2048 negotiated baseline, Native at
2560×1440 falls back to Original; it never labels a capped raster Native or changes
the view area.
The existing physical overlay keeps its original MAX_EXTENT/128 MiB guard;
it does not allocate a sampled or multisample texture at the surface size.
Browser development/reference hosts keep their existing UI and
presentation, while the shared copied parser preserves/migrates v2 preferences.
Future WebGL2 extension work must query drawing-buffer, texture/renderbuffer,
MSAA/resolve support and handle context loss; this PR supplies no GL acceptance.

## Offline evidence

`cargo test --manifest-path apps/wgpu-slice/Cargo.toml --test overlay --test settings`
covers exact projection/frame/clip-bit and steering/boost parity at DPR
1/1.25/1.5/2, retained pictures, strict v2/v1 migration, storage retry, second-toggle
focus and compactness through DPR 5 plus overlays at DPR 6–8. Pure target-policy
tests cover limits, zero sizes, inclusive MSAA/hard attachment guards,
overflow-safe malformed sizes and capability fallback/recovery.

Explicit GPU checks use
`cargo test --manifest-path apps/wgpu-slice/Cargo.toml --test world_resolution -- --ignored`.
They exercise both sample-count paths, direct resolve without backing/blit,
resize/mode replacement without intervening waits, rejected overlays and retained
output readback. No visible window or game server is used.

`world_resolution_specimens <new-output-directory>` writes authored synthetic
Original/Native PNGs and matched 384×256 physical-pixel edge crops at 1080p, 1440p
and 4K, for sample ceilings 1 and 4. Use `--integrated` to require the DX12 iGPU.
Filenames/JSON distinguish the ceiling from the selected world sample count.
`world-resolution.json` separately records projection,
attachment, producer quads, GPU draw runs, passes, resource bytes/slots, persistent
targets, old-plus-replacement transition bytes, one presentation image and CPU
preparation/sync/submission/readback timings. The transition figure is logical
host-owned target accounting for that measured swap, not a driver-residency peak.
CPU submission times exclude PNG encoding and are not GPU timestamps. Physical
input, surface/scanout, weak-GPU performance and perceptual acceptance remain
unverified. Use `CARGO_BUILD_JOBS=2`; full verification remains `npm run check`.
