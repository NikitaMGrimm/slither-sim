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
Single-sample Native trades some hard-geometry edge smoothness for lower memory
and raster cost; source-alpha smoothing stays unchanged.

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
| Background | Black clear; optional local tile and 512×512 backdrop | No default texture. Configured local art retains its decoded tile dimensions and cropped backdrop. |
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
A larger file at the same path is not a drop-in HD variant: fixed canvases clip
it, while accessory placement scales decoded dimensions with retained pivots.
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
input, surface/scanout, other weak-GPU workloads and owner acceptance remain
unverified. The finite iGPU measurements below cover one workload. Use
`CARGO_BUILD_JOBS=2`; full verification remains `npm run check`.

## Integrated-GPU measurements (2026-11-01)

These retained measurements predate removal of the built-in background. They
describe the pinned source below, not the current black default.

Keep Original as the default. On this workload, bounded Native at 4K reduces the
Native MSAA4 control's GPU queue span and memory, but still costs more GPU time
than Original. The toggle exposes that sharpness/performance tradeoff explicitly.
There is no evidence for a universally safe automatic upgrade on weak iGPUs.

Windows, AMD Radeon(TM) Graphics (integrated, device `1002:13c0`), DX12 driver
`32.0.21001.9024`; Ryzen 7 9800X3D, 16 logical processors; Rust 1.99.0 release,
thin LTO. The CPU timings describe this desktop CPU, not a weak laptop CPU.
Source commit `4257be8e6d224c6ab6a14c2139980b0ceda123d5` contains the final driver
and production policy. A matched **128 MiB MSAA-budget control** reproduces the
previous Native MSAA4 path at the three tested extents; the candidate uses
64 MiB. Both use exactly the same driver/source inputs. This is a configuration
comparison, not an old-revision executable comparison.

The authored world has nine snakes and 325 retained food (324 lattice additions
plus one base-fixture food), with generated shaded skins, food, prey and
procedural cosmetics from an empty image set. No private art directory is read.
The fixture advances 240 production updates at 16 ms, supplies in-memory pongs,
and asserts every food's fade and scale are exactly one and the session remains
Playing. Simulation/ingress/quality then freeze; stock and maximum 1.25× wheel
zoom-in settle before measurement. Projection is 1500×845, DPR 1, at all outputs.
This is a bounded synthetic world workload, not a population or gameplay-FPS claim.

Five runs per configuration, alternating control/candidate, each with five
alternating Original/Native rounds per output/view, 20 warmup + 40 measured pictures per cell:
**1,000 measured pictures per cell**, 24,000 total. All pictures submit; no failed
picture is counted as fast. p95 is nearest rank. The GPU hardware timestamps
surround the complete world presenter (world plus Original's blit), so the span
includes CPU supply gaps between submissions. It is **not isolated active shader
time**. CPU build and submission are separate; upload completion waits, query
readback, PNG encoding and logging are outside those CPU phases. HUD/menu,
transport, simulation, host retained-picture clones, scanout and desktop pacing
are excluded. Do not add these overlapping CPU/GPU columns to estimate FPS.

Current policy, milliseconds (median / p95); range is the five per-run GPU-span
medians:

| Output / view | Mode | CPU build | CPU submit | GPU queue span | GPU median range |
| --- | --- | ---: | ---: | ---: | ---: |
| 1920×1080 / stock | Original | 0.329 / 0.624 | 0.839 / 1.157 | 3.428 / 3.751 | 3.398–3.504 |
| 1920×1080 / stock | Native | 0.333 / 0.642 | 0.814 / 1.131 | 4.304 / 4.644 | 4.286–4.379 |
| 1920×1080 / zoom-in | Original | 0.296 / 0.576 | 0.634 / 0.875 | 3.294 / 3.539 | 3.279–3.356 |
| 1920×1080 / zoom-in | Native | 0.307 / 0.585 | 0.611 / 0.854 | 4.156 / 4.419 | 4.139–4.196 |
| 2560×1440 / stock | Original | 0.344 / 0.679 | 0.863 / 1.119 | 4.147 / 4.405 | 4.106–4.174 |
| 2560×1440 / stock | Native | 0.352 / 0.685 | 0.845 / 1.135 | 7.546 / 7.897 | 7.484–7.617 |
| 2560×1440 / zoom-in | Original | 0.314 / 0.608 | 0.657 / 0.897 | 3.970 / 4.207 | 3.925–4.013 |
| 2560×1440 / zoom-in | Native | 0.324 / 0.634 | 0.638 / 0.902 | 7.264 / 7.596 | 7.140–7.361 |
| 3840×2160 / stock | Original | 0.354 / 0.715 | 0.898 / 1.683 | 5.552 / 6.981 | 5.406–5.881 |
| 3840×2160 / stock | Native | 0.363 / 0.800 | 0.846 / 1.706 | 9.673 / 11.732 | 9.651–10.552 |
| 3840×2160 / zoom-in | Original | 0.320 / 0.748 | 0.682 / 1.563 | 5.370 / 6.578 | 5.269–6.001 |
| 3840×2160 / zoom-in | Native | 0.328 / 0.708 | 0.637 / 1.322 | 9.643 / 11.353 | 9.614–10.612 |

Warm resource-sync medians were 0.0040–0.0053 ms; p95 values were 0.0071–0.0142 ms.
No CPU or unchanged-size speedup is claimed. Stock pictures contain 2,069 draw
records; zoom-in pictures contain 1,470. Draw counts and active texture bytes
match the control exactly. Active uploaded RGBA rasters are 3,178,876 bytes
(3.032 MiB) stock and 3,049,852 bytes (2.909 MiB) zoom-in, independent of output
and mode; CPU retained pictures share these immutable rasters. This excludes
inactive catalog/session data, buffers, driver allocations and staging.

The 4K Native control comparison isolates the large MSAA target:

| View | MSAA4 control GPU span median / p95 | Bounded Native GPU span median / p95 | Per-run median ranges, control / bounded |
| --- | ---: | ---: | --- |
| Stock | 21.220 / 22.790 ms | 9.673 / 11.732 ms | 19.643–22.483 / 9.651–10.552 ms |
| Zoom-in | 21.001 / 23.780 ms | 9.643 / 11.353 ms | 19.398–23.511 / 9.614–10.612 ms |

The changed work is multisample coverage, color storage and resolve, not game
geometry or texture planning; the measurement does not distinguish GPU raster
and bandwidth sub-stages. The requested world-owned 4K target drops from
126.56 MiB to zero; one 31.64 MiB presentation image remains in both cases.
The attachment table above gives each size/mode's logical allocation. Those
figures are not resident-memory measurements or whole-process limits.

Two additional instrumented whole-workload runs, **excluded from timing
statistics**, sampled OS GPU-process memory counters and CPU process memory:

| Highest observed process counter (MiB) | MSAA4 control | Bounded Native |
| --- | ---: | ---: |
| GPU committed (sum of reported contexts) | 844.49 | 587.49 |
| GPU dedicated | 194.62 | 193.62 |
| GPU shared | 649.80 | 393.80 |
| CPU working set | 823.18 | 590.29 |
| CPU private bytes | 986.27 | 759.99 |

These are sparse observed high-water values from one full run per side, not
true peaks or per-cell residency. They include driver contexts/caches (including
contexts initialized during adapter enumeration), capture/readback staging and
PNG work; GPU committed memory is not physical resident memory. They must not be
presented as the desktop client's steady-state RAM/VRAM use. The controller
retains the raw samples separately from the timing CSVs.

### Source magnification and default regression

At Native 4K, maximum destination pixels per source texel in this fixture:

| Draw source | Stock | Maximum zoom-in |
| --- | ---: | ---: |
| Body, 48×48 | 1.96× | 2.45× |
| Body outline/shadow | 2.94× | 3.67× |
| Food body/outline/glow | 2.56× | 3.20× |
| Prey body | 2.56× | 3.20× |
| Prey glow | 5.12× | 6.40× |
| Paired eye/pupil disks, 64×64 | 0.61× | 0.76× |
| Uniform 512×512 backdrop, horizontal | 7.50× | 7.50× |

Thus body and food texels remain magnified after fixing the world raster;
zoom-in adds another 25%. Glows/shadows are deliberately soft. The measured
backdrop was uniform authored color, so its enlargement lost no detail. Repeating
tile UVs, constant-color quads, body boost/death states and image-dependent cosmetics are
not quantified by this fixture's per-sprite table; their dimensions and scaling
rules are audited above. Optional external art was not loaded, so no claim is
made about its pixel quality. Mipmaps cannot repair any of this magnification.

All **50 expected unchanged PNG comparisons** match byte-for-byte across the
five pairs: Original at all three sizes and Native at 1080p/1440p, both views.
The ten Native 4K comparisons change as expected with different edge coverage.
The separate solid-geometry 4K edge crops show Original's capped-raster blur
without any detailed source artwork; Native single-sample edges are sharper
and can show hard-edge aliasing. This is not an official-art or owner visual
acceptance claim.

The rebased native 1080p CPU-history replay against the untouched baseline also
reports `feel-perceptual-v1`: **0 mismatches, 0 errors**, 148 samples,
72 acknowledged pictures and seven command sends. Projection/vertices/input
and retained-resource GPU checks pass, as does `npm run check` on Windows.
Physical input, scanout, owner play and other weak-GPU workloads remain separate.

### Capture controls and reproduction

Timing captures ran in the reserved 07:30–07:35 CET interval; separate memory
profiles finished at 07:36. Start/end inventories and requested 50 ms process
polls observed no cargo/rustc/bindgen overlap. Actual poll gaps had median
66.0 ms and maximum 571.2 ms, including OS load-query gaps; sampled absence is
not a proof against subinterval processes. CPU-load samples over startup,
warmup, measurement and PNG work had median 8%, range 0–100%. All spikes and
all five runs per configuration remain included. No timing trial was trimmed for load.

Initial 05:56 runs used one capped update and left food near spawn size. Those
runs are retained as labelled diagnostics and excluded from the results above.
The corrected fixture and CPU-only preflight enforce full spawning before every
final scene; frozen clocks, inputs, seeds and failed attempts are preserved.

From the repository root, with a fresh output directory and a compiler-free,
GPU-quiet interval:

```powershell
$env:CARGO_TARGET_DIR = 'D:/CargoTargets/hd-4k'
$env:CARGO_BUILD_JOBS = '2'
cargo build --release --locked -p slither-wgpu-slice --example world_resolution_perf
& "$env:CARGO_TARGET_DIR/release/examples/world_resolution_perf.exe" --validate-fixture
& "$env:CARGO_TARGET_DIR/release/examples/world_resolution_perf.exe" C:/new/external/world-resolution
```

Do not time the build. For the matched control, build the same source with only
`NATIVE_MSAA_BYTES` set to 128 MiB, retain that executable, restore 64 MiB and
build/retain the candidate. Alternate the retained executables five times;
never silently replace a failed capture. Raw CSVs, counters, hashes, binaries,
PNGs and failed diagnostics remain outside Git. Source bindings:

```text
driver SHA-256: eb7ab39ed8a2de7c107504fe0b445386af5c24f82977d944fa9976c9a9e65a2d
control executable SHA-256: 4c34a2bb6239728e69c6e0193772c64a75c7613f03927dfecdf60088b0729b97
candidate executable SHA-256: 3a64663fdd8dd5507c480254ecdd05085904cb951b1a105d612ffc2a15da6a69
```
