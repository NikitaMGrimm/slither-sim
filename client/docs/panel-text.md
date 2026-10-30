# Bounded styled panel text

This is a synthetic rendering primitive, with no enabled gameplay producer.
Later announcement and result/editor work can use the shared line breaker,
`prepare_reserved` and one `draw_panel_composition` call containing all active
panels. Its output preserves producer order so the compositor can place the
announcement before HUD/minimap and the result/editor after them. The existing
64 MiB active-resource admission still applies to the completed frame.

Admit `PanelText::new` once when content changes; retain it across compositions
and resizes. Admission is linear and explicitly bounded to 2 MiB / 4096 raw
fragments. It collapses CSS-normal ASCII whitespace while retaining the first
space's style across inline boundaries. Owners retain full original content.
Repeated layout scans only the visible prefix, bounded by 4096 non-whitespace
normalized bytes plus normalized separators/explicit breaks. Even a million
collapsed leading spaces cannot consume the visible-text quota.

`TextMetrics` measures complete same-style runs. Explicit break fragments
preserve line breaks. Words wrap before overlong tokens split at UAX #29 extended
grapheme boundaries, as do cache chunks. `unicode-segmentation` is pinned to
1.13.3: a small platform-independent Unicode boundary implementation avoids an
incomplete hand-written combining-mark/ZWJ rule and builds on wasm32. A grapheme
crossing an inline style boundary takes its first scalar's style. A single
cluster over 1024 bytes projects to one U+FFFD; it is never split or partly
retained, and the original owner content is unaffected. Every cache piece stays
within 1024 UTF-8 bytes. Prefix-end measurements position pieces with boundary
kerning; artificial chunks do not measure each character independently. Ink is
clipped to the line cell and box, with up to one em of overhang on each run's
sides. Visited invalid metrics return an error without publishing a partial plan.

Supported panels have 16–32px line cells, fonts up to 24px, height at most 228px,
and width at most 300px (announcement) or 600px (result/editor). The namespaces
have 48 and 16 replacing slots. A composition preflights every producer before
committing any replacement/retirement; duplicate namespaces and aggregate byte
or tiled-draw excesses fail explicitly. An empty composition retires all panels.
Position, opacity and future pulse values are outside raster keys. Relative
clipping geometry is part of the key. Prepared frames own immutable pixels.
Before allocating replacements, the staged cache retires all changed/hidden
entries together; unchanged entries keep their handles. The final aggregate,
not transient old-plus-new pixels, determines admission. Failure publishes no
retirement or replacement, regardless of producer order.

## Font box and baselines

Ink bounds validate and clip glyphs; they do not set the baseline.
`PanelTextBlock::device_scale` is the actual device pixels per logical pixel,
independent of the raster cap. Like Windows Blink's zoom-for-DSF layout, metrics
are requested at `font_size * device_scale` before rounding; advances return to
logical pixels after complete-run measurement. Invalid scales (nonpositive,
nonfinite or above the shared 8192 viewport bound) fail before backend work.

For these installed Windows faces, native metrics read OS/2
`usWinAscent`/`usWinDescent` divided by `unitsPerEm`, matching DirectWrite's
[Skia font box](https://github.com/google/skia/blob/c0c46ee9f938fa32c086bea666199686ad7cf26e/src/ports/SkScalerContext_win_dw.cpp#L1848-L1872).
[Blink rounds A/D in those device pixels](https://github.com/chromium/chromium/blob/a7453b805045ac88d29dfee5262913dcdad7508f/third_party/blink/renderer/platform/fonts/font_metrics.cc#L104-L121).
The web adapter obtains [Canvas font boxes](https://github.com/chromium/chromium/blob/a7453b805045ac88d29dfee5262913dcdad7508f/third_party/blink/renderer/core/html/canvas/text_metrics.cc#L131-L141)
from one lazy measurement canvas/context, separate from rasterization. Canvas
can retain tiny-font fractions; DOM [GetFontHeight uses integer A/D](https://github.com/chromium/chromium/blob/a7453b805045ac88d29dfee5262913dcdad7508f/third_party/blink/renderer/platform/fonts/font_metrics.h#L164-L169),
which the shared layout applies before leading calculation as well.

Line height is represented on Blink's 1/64-device-pixel LayoutUnit grid. Per
[CalculateLeadingSpace](https://github.com/chromium/chromium/blob/a7453b805045ac88d29dfee5262913dcdad7508f/third_party/blink/renderer/core/layout/inline/line_utils.cc#L35-L43),
each style first computes `L = line_height_device - (A + D)`,
`ascent_leading = floor(L / 2)` and `descent_leading = L - ascent_leading`.
The [integer LayoutUnit division](https://github.com/chromium/chromium/blob/a7453b805045ac88d29dfee5262913dcdad7508f/third_party/blink/renderer/platform/geometry/layout_unit.h#L617-L622)
first truncates to its 1/64 grid; the subsequent floor goes toward negative
infinity. For example, -10.015625 / 2 becomes -5 before that floor, rather
than -5.0078125. The line unions these adjusted extents and uses
`top + max(A + ascent_leading) / device_scale` for its
baseline. Thus the floor granularity is 1/DPR logical pixels. The configured
cells stay fixed and clip overflowing extents; `ace`, `Ace` and `...` share the
same baseline for the same styles and scale.

The bounded independent headless Edge check uses the actual
`--force-device-scale-factor` flag at DPR 1, 1.25 and 2, seven real DOM inline
cases, and zero-size baseline markers. It compares exact device coordinates
with shared layout, including mixed fonts, negative leading, fractional
font sizes and the odd LayoutUnit division case. At fractional DPR, the CSS
DOMRect export can lose precision (a
27-device-pixel marker exports 21.600000381469727 CSS pixels at DPR 1.25).
The probe preserves that raw value and reads a post-layout `scale(DPR)` ruler,
which changes coordinate units without changing inline layout. Assertions use
exact device values, with no tolerance or numerical rounding of DOM results.
This does not claim unconstrained multiline DOM flow or pixel/shaping parity.

## Installed faces

Native Windows uses `arial.ttf`, `arialbd.ttf`, `ariali.ttf`, `arialbi.ttf`, and
`l_10646.ttf` (Lucida Sans Unicode regular). Windows has no styled faces for that
Lucida family: all native styled Lucida requests keep the regular Lucida Sans
Unicode face, never Arial. `TextCache::native_face` exposes the exact choice;
missing selected files fail explicitly. No font is bundled or sheared. Canvas
requests the explicit Arial or Lucida Sans Unicode / Lucida Grande CSS stack
and lets the browser resolve installed fonts. **Known non-parity:**
[Chromium's Windows font matching](https://github.com/chromium/chromium/blob/a7453b805045ac88d29dfee5262913dcdad7508f/third_party/blink/renderer/platform/fonts/win/font_cache_skia_win.cc#L484-L498)
keeps the matched family and asks Skia to synthesize missing bold/oblique styles.
The official Lucida uses relevant to later panels are the end-screen length
line's bold number and the champion instruction's regular text; there is no
italic Lucida producer. Synthesis or an explicitly accepted approximation is
deferred to end-screen backlog item 5, and is not implemented here.
Native coverage still uses the fill-colour LUT after the 4×4 average and
before premultiplication, through the same raster loop as existing text.

## Aggregate resource proof

The earlier plan predates the minimap change. The base has 258 text entries /
6 MiB (HUD 40 / 2 MiB, nicknames 216 / 2 MiB, minimap fill and independently keyed
shadow each 1 MiB). These remain unchanged. Panels add 64 lazy slots / 12 MiB;
total ceilings are 322 entries / 18 MiB. Nickname accounting is explicit and
cannot borrow from panels. All pixel allowances are maxima, not eager allocation.

The original 8 MiB / 64 draws per producer does not cover the conservative
overhang/fragment envelope. At the capped raster scale 3:

- There are at most 15 visible cells in 228px, including the partial last cell.
- Rectangular coverage, including 2 raster pixels of rounding per row, is
  `(900 + 1800) × (684 + 2 × 15) = 1,927,800` pixels.
- Each of 64 fragments adds at most two 24px ems plus 2 rounding pixels in width,
  over a maximum 32px cell plus 2 rounding pixels in height:
  `64 × 146 × 98 = 915,712` pixels.
- 510-pixel content tiles require at most
  `48 + floor((900 × 15 + 48 × 146) / 510) = 88` announcement draws and
  `16 + floor((1800 × 15 + 16 × 146) / 510) = 73` result/editor draws.
- Counting two neighbour pixels for every tile conservatively adds
  `2 × (88 + 73) × 98 = 31,556` pixels.

The aggregate envelope is **11,500,272 bytes**, below 12 MiB (12,582,912 bytes).
The test feeds that envelope to the same aggregate admission used for actual
clipped allocations, and checks byte/draw excess rejection. The live preflight
also protects callers supplying hand-built plans instead of the line breaker.
Retiring obsolete entries earlier changes no allocation term. Font-box
baselines translate ink inside the same clipped line cells, so rounding,
overhang and tile padding retain these bounds. The near-full replacement
regression also checks both grow-early/shrink-later producer orders and rollback.

Both named draw reserves are 96: up to 88 text draws plus 8 fills/borders/controls.
Current minimap reserve is **12**, rather than the earlier plan's 8. With HUD 128,
opting in both panels reserves **332**, leaving **7860** non-overlay draws out of
the unchanged 8192. With neither producer enabled the reserve stays **140**.
This does not change existing dense-world overflow behavior or establish dense
gameplay coverage. Future producers must preflight the entire mandatory panel,
then preserve the final whole-frame draw/pixel checks.

Tests use authored fake metrics and installed Windows fonts. Offscreen native
and Canvas specimens are synthetic resource/layout evidence, not gameplay or
official-client perceptual parity.
