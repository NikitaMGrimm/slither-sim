# Stroke wordmark

The native login always selects `wordmark::DEFAULT_TEXT` (SlitherClient). Change that string, or set `Menu::wordmark_text`, to use other
text. All 95 printable ASCII characters have authored round-pen paths in
`apps/wgpu-slice/src/wordmark_glyphs.rs`. This is an original alphabet under the
repository MIT licence: no Hershey distribution, third-party glyph data, bundled
font, official stroke coordinates, code or artwork is included. No additional
third-party font notice is required.

Words split at capitals and spaces and alternate the green/purple gradients in
`wordmark::COLORS`. An unsupported scalar selects the installed bold-font path
for its whole word; supported parts of that word are never drawn separately.
The same fallback policy applies to arbitrary strings, not just the default.
Inputs are bounded to 128 characters and fail before unbounded allocation.

The 450x115 holder remains the vertical anchor. At natural size the ten-unit
cap height is about 72 CSS pixels plus the 12px round pen. Lowercase ascenders
reach that height; lowercase bodies use a six-unit x-height. The wordmark is
centred over the holder and can extend beyond 450px. Only the viewport's 16px
side margins cause additional uniform fitting. Existing menu scaling, including
outgoing growth, still applies.

The core's retained `logo_reveal` drives a chain of circle samples. Each sample
swells from zero radius with delay along its stroke and across stroke order.
Shared analytic ellipse arcs build the round bowls, semicircular shoulders and
tangent-continuous S curves; straight segments retain their authored corners.
A traveling perpendicular sine displacement (65.8% of the initial amplitude)
grows along each stroke and decays
smoothly to exactly zero at completion. Most stroke volume arrives in the first
2.5 seconds; settling uses the full roughly 6.7-second clock. Opacity remains
the independent `login_alpha * logo_alpha`. The soft shadow follows the current
drawn mask. During motion at DPR 1-3 its coverage and blur run at half resolution;
higher densities retain the full shadow to keep packed atlas bounds. The exact
settled frame always uses the original full-resolution pen and shadow.

Reveal rasters are capped at 40 Hz: a new raster requires at least 25 ms of
the retained reveal clock. Intervening frames keep the same shared image.
Geometry/text changes invalidate that cache independently. Motion samples use
the elapsed reveal fraction, so skipped updates do not change pacing; the exact
final frame settles once and then keeps a static key.
Cached atlases retain shared immutable `Arc<RgbaImage>` images; preparing another
frame clones their handles rather than copying pixels. No idle rasterization or
logo texture upload occurs. Size or centring-phase changes produce a new
exact-DPR raster. Premultiplied CPU pixels are cropped to
nonempty bounds including the shadow and tiled horizontally at 2048px. Each
tile is one snapped 1:1 draw, with no texture minification. The transient RGBA
canvas is capped at 10 MiB; retained tiles share the existing 64-slot/13 MiB
Menu allowance with controls and text. At extreme physical sizes, the stroke
layout shrinks uniformly to the transient raster bound while keeping its centre
and 1:1 texels; DPR alone never selects a different font. The installed-font
path remains reachable for unsupported configurable text and the aggregate
budget retry described below. Invalid input and other errors still propagate.
Font colour runs are admitted
only after the controls and spinner are known: adjacent equal-colour words share
a label, and an over-budget suffix becomes an ellipsis. The whole font line fits
the holder. Final slot/byte admission stays in the shared menu composition.
Atlas admission reserves the existing materials and label slots. The final
transaction checks the actual text bytes as well; an aggregate quota failure
retries only the logo with bounded font runs. That retry publishes only if the
same full-menu admission succeeds. Menus that cannot fit even then retain the
existing quota skip. These are ceilings, not eager allocations.

The standalone Canvas/WebGPU DOM development pages retain their separate
plain-text/art logo. This is a known reference-host difference; the future
extension will reuse this native painter through WebGL2.

Every word (including a font fallback) uses one snapped physical canvas origin
plus its rounded relative advance. Absolute word origins never round separately;
fractional-DPR regressions retain the same boundary rule as the prior font logo.
The raster cache includes the first word's local physical offset from the snapped
canvas origin. A centring phase change rebuilds it; integer translations reuse
the same image at the new origin.

Above DPR 3, nonempty horizontal bands are packed in bounded atlases to avoid
charging transparent areas around the letters against essential controls. This
keeps at most four regions per band, merging the smallest gaps to share the
512-draw cap with sliced controls. Larger transparent gaps are not retained, so
the outgoing wordmark, fading spinner and text fit the same 13 MiB ceiling. A
pixel-exact reconstruction test
checks that packing neither drops, overlaps nor changes the raster pixels.

Offline authored tests: `cargo test -p slither-wgpu-slice --locked --test wordmark
--test menu`. `menu_specimens <new-output-dir> --stroke` renders frozen reveal
times, full settled menus at DPR 1/2, arbitrary strings, alphabet sheets, and
seven raster runs per DPR/progress pair (first warmup omitted in the JSON).
Transparent string PNGs unpremultiply a copy; production texture pixels stay
premultiplied. It opens no window, browser or game socket. Specimens are synthetic product
output, not owner perceptual acceptance or original pixel parity.

## CPU reveal workload

`wordmark_perf <new-output-dir> [samples=60] [progress=0.5]` is a finite,
CPU-only release workload. It measures uncached production rasters at DPR 1/2/5
and complete menu preparation across 40 ms reveal advances, excluding file I/O
and output validation from timings. Four warmups precede each 16-128-sample
series. `times.csv` retains every sample; raw RGBA settled/reveal pixels provide
byte comparisons. No GPU, window, browser or network is created. Build with
`cargo build --release --locked -p slither-wgpu-slice --example wordmark_perf`.
Retain both binaries and alternate at least five runs per side for comparisons.

Active rasters stamp antialiased discs. Fully grown strokes use eighth-pixel
radius/centre buckets; swelling samples use half-pixel radii and quarter-pixel
centres to avoid constructing hundreds of one-use sprites. Radii that round to
zero are omitted. Disc data has a 2 MiB per-raster cap; uncommon uncached discs use
the analytic pen. The settled raster bypasses all disc quantization. Row-cached
colours and contiguous integer shadow passes preserve the settled bytes;
regressions compare them with the prior scalar raster at DPR 1/2/5.
