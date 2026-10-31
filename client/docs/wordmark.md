# Stroke wordmark

Native `--wordmark`, or missing local logo art, selects `wordmark::DEFAULT_TEXT`
(SlitherClient). Change that string, or set `Menu::wordmark_text`, to use other
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
A traveling perpendicular sine displacement grows along each stroke and decays
smoothly to exactly zero at completion. Most stroke volume arrives in the first
2.5 seconds; settling uses the full roughly 6.7-second clock. Opacity remains
the independent `login_alpha * logo_alpha`. The soft shadow follows the current
drawn mask. Bitmap logo art now uses a plain fade instead of the earlier wipe.

Reveal rasters are capped at 40 Hz: a new raster requires at least 25 ms of
the retained reveal clock. Intervening frames keep the same shared image.
Geometry/text changes invalidate that cache independently. Motion samples use
the elapsed reveal fraction, so skipped updates do not change pacing; the exact
final frame settles once and then keeps a static key.
Cached atlases retain shared immutable `Arc<RgbaImage>` images; preparing another
frame clones their handles rather than copying pixels. No idle rasterization or
logo texture upload occurs. A resize or outgoing size
change produces a new exact-DPR raster. Premultiplied CPU pixels are cropped to
nonempty bounds including the shadow and tiled horizontally at 2048px. Each
tile is one snapped 1:1 draw, with no texture minification. The transient RGBA
canvas is capped at 10 MiB; retained tiles share the existing 64-slot/13 MiB
Menu allowance with controls and text. DPR is bounded to 5.5; admission failure
preserves the previous cache. These are ceilings, not eager allocations.

Every word (including a font fallback) uses one snapped physical canvas origin
plus its rounded relative advance. Absolute word origins never round separately;
fractional-DPR regressions retain the same boundary rule as the prior font logo.

Above DPR 3, nonempty horizontal bands are packed in bounded atlases to avoid
charging transparent areas around the letters against essential controls. This
uses more quads within the same 512-draw cap; a pixel-exact reconstruction test
checks that packing neither drops, overlaps nor changes the raster pixels.

Offline authored tests: `cargo test -p slither-wgpu-slice --locked --test wordmark
--test menu`. `menu_specimens <new-output-dir> --stroke` renders frozen reveal
times, full settled menus at DPR 1/2, arbitrary strings, alphabet sheets, and
seven raster runs per DPR/progress pair (first warmup omitted in the JSON).
It opens no window, browser or game socket. Specimens are synthetic product
output, not owner perceptual acceptance or original pixel parity.
