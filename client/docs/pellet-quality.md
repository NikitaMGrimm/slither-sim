# Pellet quality

The native/shared wgpu path formerly sampled food bodies from 4–25 px square
rasters. The radial alpha falls from 20% to zero in the last 1% of the radius;
one sample per texel leaves angular lobes when that tiny raster is reconstructed
with bilinear filtering. High quality adds glow passes but uses the same body.
Native/sharp exposes source texels more clearly; Original also enlarges the
1500×845 world backing at 16:9. Neither attachment policy can restore source detail.

Filtering was already linear for both minification and magnification. Food
positions are fractional, without overlay pixel snapping. Uploads and blending
already use premultiplied encoded-sRGB components. MSAA covers the textured quad's
boundary, not the alpha contour inside it. The former single-level textures also
undersampled large glow sources at strong minification. These are source-sampling
issues, independent of world resolution and the quality pass policy.

`bank.rs` now materializes body gradients at four times their logical dimensions
(16–100 px), with filtered mip levels for food body and glow gradients. Body
mips stop at a minimum of four texels per axis: 2×2 and 1×1 collapse a radial
body into equal ink in every texel, visibly filling a square across its unpadded
quad. The soft, padded glows retain full chains down to 1×1. Logical
asset dimensions, creation-time source selection, crop UVs, centers, destination
extents, layer order, alpha and quality policy remain unchanged. Source rectangles
are normalized against logical dimensions; the denser GPU raster covers the same
UV interval. Other source families retain their existing rasters and single level.
The Canvas reference path is unchanged.

Mip levels average premultiplied encoded components with area weights, including
the last row and column of odd-sized sources. Linear mip interpolation avoids
abrupt level transitions. Materialization and uploads occur on cache misses;
retained pictures share immutable images. The fragment shader, vertices, draw
counts, attachment sizes and per-frame passes are unchanged. The existing active
and resident 64 MiB texture limits charge the actual raster and every mip level.
This trades a small bounded cache increase for sampling quality, without a
global resolution increase or a new per-frame shader calculation.

For the full set of 170 body and 170 glow rasters, texture payload is 12.23 MiB
versus 6.77 MiB on the pre-change main. Cold uploads increase from 340 to 1,900
`queue.write_texture` calls (one per allocated mip level). These totals exclude
other textures and driver overhead; the bank still loads only selected sources.

Remaining raster limits apply in both quality modes. Original still upscales its
1500×845 backing at 16:9, and Low removes the glow passes. Fractional bodies under
four backing pixels retain some coverage instability. Smoothness at every zoom
is not claimed.

## Offline verification

`pellet_quality` is an authored 80-food scene with no external artwork, socket,
window or game server. It renders the production presenter to offscreen images
on a DX12 integrated GPU. The matrix covers High/Low, Original/Native, stock
zoom/whole-map wheel endpoint, and DPR 1/1.5 at a fixed 1920×1080 logical window
(1920×1080 / 2880×1620 physical). Native therefore exercises both MSAA4 and the
bounded single-sample attachment path. Applied
quality snapshots are frozen explicitly so automatic FPS fallback cannot change
the requested comparison. The small synthetic arena keeps pellets visible even
at the whole-map endpoint; normal large arenas can cull subpixel pellets by the
existing zoom policy. Full zoom-out is exercised through the actual wheel policy.

The matched baseline and candidate have identical complete draw records in all
16 cells, including fractional centers, UVs, alpha and blending; GPU draw counts
are also identical. Active image bytes in this scene rise from 2.49 to 3.51 MiB
at High stock zoom, and 3.50 to 4.95 MiB at High full zoom-out. Low uses 0.54 and
0.71 MiB respectively after the change. These are texture payloads, not driver
residency, FPS or whole-process memory measurements. No gameplay performance or
owner visual acceptance is inferred from offscreen rendering.

Run from the repository root in PowerShell (use a new external output directory):

```powershell
$env:CARGO_TARGET_DIR = 'D:/CargoTargets/pellet-quality'
$env:CARGO_BUILD_JOBS = '2'
cargo run --offline --locked -p slither-wgpu-slice --example pellet_quality -- C:/new/pellet-crops
cargo test --offline --locked -p slither-wgpu-slice --lib food_texture
cargo test --offline --locked -p slither-wgpu-slice --test pellet_quality -- --include-ignored
```

The CPU regression bounds bilinear alpha variation along a circular rim of the
smallest sprite, validates complete mip payloads and premultiplied channels, and
checks odd-size averaging. The explicit GPU regression uses sample count one to
check a magnified circular rim and phase-stable minification at a four-pixel
diameter without MSAA masking the issue. At fractional diameters below that,
single-sample quad coverage itself can change as pixel centers enter or leave;
invariant subpixel brightness or roundness is not claimed. A separate test checks
retained raster sharing, logical dimensions and the minimum body mip size.
Visual acceptance is perceptual; the authored tests do not establish byte parity
with Canvas or final owner play acceptance.

The isolated glow GPU regression strongly minifies a 157×157 source to a 2×2
backing-pixel quad across subpixel phases. It checks energy against the source's
area average, and runs a single-level control that must fail that bound. This
tests actual mip sampling separately from the four-pixel body-rim check.
