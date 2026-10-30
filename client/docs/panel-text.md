# Bounded styled panel text

Production `LocalGame` enables the [announcement producer](announcements.md),
using one panel composition before minimap and HUD submission. Result/editor
remains inactive. The existing 64 MiB active-resource admission applies to the
completed frame. Each Setup, ordinary-work and resize preparation captures its
own announcement projection and immutable resource copies.

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
and width at most 300px (announcement) or 600px (result/editor). The announcement source-aware layout also admits explicit line cells
from 14px; its fonts are limited to 13/17px. `preserving_fragments` and
`layout_panel_text_with_sources` keep inline boundaries and source indices
through normalization, wrapping and chunking. Per-run alpha is separate metadata
in `draw_panel_composition_with_run_alphas`, never a raster-key field.
The namespaces have 48 and 16 replacing slots. A composition preflights every producer before
committing any replacement/retirement; duplicate namespaces and aggregate byte
or tiled-draw excesses fail explicitly. An empty composition retires all panels.
Position, opacity and future pulse values are outside raster keys. Relative
clipping geometry is part of the key. Prepared frames own immutable pixels.
Before allocating replacements, the staged cache retires all changed/hidden
entries together; unchanged entries keep their handles. The final aggregate,
not transient old-plus-new pixels, determines admission. Failure publishes no
retirement or replacement, regardless of producer order.

## Font box and baselines

Ink bounds validate and clip glyphs; they do not set the baseline. Native uses
fontdue's horizontal line metrics; WebGPU uses Canvas font metrics directly.
`PanelTextBlock::device_scale` is device pixels per logical pixel, independent
of the raster cap. Metrics are requested at `font_size * device_scale` and
converted to logical pixels without extra rounding or snapping. Invalid scales
(nonpositive, nonfinite or above 8192) fail before backend work.

Each line takes the maximum font ascent A and descent D across its runs and
centers that single strut in its explicit cell:
`baseline = top + (line_height - A - D) / 2 + A`.
Compact cells use the same formula with negative remaining space. The cells
stay fixed and clip overflowing ink; glyph choice never moves the baseline or
the following cell. Mixed-font, fractional-density and compact-cell tests keep
these boundaries explicit, together with invalid-metric and rollback cases.

`TextStyle::line_height()` is the one automatic panel spacing policy:
`round(1.15 * font_size)` logical pixels (13px -> 15px, 17px -> 20px).
Announcements choose the largest requested cell in each block. General panels
continue to supply explicit cell heights. No DOM measurement or font-table line
gap is needed. Visual acceptance allows baseline/cell/box edges to move by up to
2 CSS px and announcement extent by up to 4px, without clipping or overlap.
Historical exact DOM baseline probes are not current acceptance gates.

## Installed faces

Native Windows uses installed Arial regular/italic for Arial text and Lucida
Sans Unicode regular for regular Lucida text. Every bold request uses real
Arial Bold (or Bold Italic), including text styled as Lucida. Linux uses the
corresponding installed DejaVu Sans regular/bold/oblique faces. Browser WebGPU
requests `Arial, DejaVu Sans, sans-serif` for bold and retains the regular family
stack otherwise. No synthetic weight, font matching engine or bundled fonts
are needed. Missing selected native files return explicit errors; failed cache
replacement retains the prior prepared resources.

Measurement and rasterization share a display-only control policy: whitespace
controls become spaces, other C0/C1 controls, DEL and soft hyphen become one
replacement glyph with its ordinary advance. Other whitespace, including NBSP,
retains its normal glyph. Chromium/SimSun fallback widths are not emulated.
Control-heavy display strings can change width substantially; ordinary visible
strings have a 15% width budget and 2px baseline budget, with readable emphasis
and no clipping or overlap. This policy never changes retained nickname bytes,
editor normalization, login encoding or game decisions; byte-exact protocol
regressions still admit DEL.

Native coverage uses one luminance-dependent curve after the 4×4 average and
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

Each panel has a producer limit of 96 draws: up to 88 text draws plus 8
fills/borders/controls. HUD 128 and minimap 12 remain independent producer
limits. They no longer reserve space from world geometry, and announcement
visibility does not change world capacity. The [chunked renderer](draw-chunks.md)
checks complete pictures against its memory-derived bound. Future producers
must preflight their entire panel and preserve the final draw-memory/pixel checks.
Admission and submission use the same captured view; retries reuse its prepared
resources.

Tests use authored fake metrics and installed Windows fonts. Offscreen native
and Canvas specimens are synthetic resource/layout evidence, not gameplay or
official-client perceptual parity.

The enabled announcement has a tighter font envelope than the general panel:
17 partial/full cells at the 14px lower bound, at most 17px fonts and 48 slots.
At raster scale 3 its overhang width is 104px and its tiled bound is
`48 + floor((900 * 17 + 48 * 104) / 510) = 87`, within 88 text draws.
Keeping the result/editor's 73 draws, all 64 fragment heights conservatively at
98 raster pixels, and 17 rounding allowances for both boxes gives an aggregate
10,752,416 bytes, below the existing 12 MiB. Cache quotas are unchanged.
