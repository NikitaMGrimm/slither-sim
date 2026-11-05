# Mouse-wheel zoom

During native desktop play, wheel up zooms in and wheel down zooms out on every
server. One line/notch multiplies the requested factor by **1.12**; precision
scrolling uses fractional notches (40 logical pixels per notch). Menus and lists
receive the event first and retain exclusive wheel ownership. No settings or
reset key is added. The preference survives deaths and world resets in the same
process, and starts at the stock view on a new launch.

The camera keeps its existing snake-size automatic scale `s` and follows the
snake. The user factor multiplies `s`; the **absolute** result is clamped between
the whole-arena scale and **1.25 * s**. These are authored presentation choices,
not measured server mechanics. Without nonzero wheel input, stock draw-clock
arithmetic, input angles/bytes and `feel-perceptual-v1` remain unchanged.

For logical viewport `w,h`, radius `r`, arena centre `c` and camera `p`, the
whole-map endpoint is the smaller of `w / (2*(r+abs(p.x-c.x)))` and
`h / (2*(r+abs(p.y-c.y)))`, capped at the stock scale when the arena already fits.
At the centre this fits the diameter into the shorter viewport side. Away from
the centre it also accounts for the camera offset, so the entire circle fits
while the snake stays centred. The radius follows existing Setup/`z` state;
while the displayed boundary contracts, use the larger of received and displayed
radii to avoid clipping the easing boundary. The explicit whole-map endpoint
follows radius, camera motion, viewport and stock-scale changes immediately.
It is a fit constraint: smoothing a moving endpoint can otherwise leave the
limiting edge permanently clipped.

Away from that endpoint, the user factor interpolates in log space by 18% of the remaining distance per
eligible draw, snapping within `1e-7` log units. It shares the stock camera's draw
clock; retrying a frozen picture consumes no extra sample.
Requests within `1e-12` of factor one normalize to exact stock zoom, so reciprocal
wheel steps leave no floating-point residue in paint policy.
Culling, body/face geometry, arena, world labels and repeating background use the
resulting scale.
Food/prey keep stock creation-time source selection, with user zoom applied to
their sprite extents. At nondefault zoom, the arena keeps the stock nearby arc
while it covers the visible boundary band. Circle/viewport intersections select
a whole circle only when visible coverage extends beyond that arc or its
4,000-unit distance gate. Full-circle tessellation uses the projected outer band
radius to keep chord sagitta at most **0.5 logical pixels**, with 40 to 4,096
segments; a request beyond that hard bound rejects the picture rather than
allocating without a limit. Default zoom retains the exact stock path. HUD, minimap and
leaderboard stay in screen space. At nondefault zoom, steering starts at the
corrected displayed head, including camera smoothing offsets and the backing to
CSS transform. Factor one preserves the stock centre/raw-head policy exactly.
The client retains admitted cursor coordinates and refreshes their nondefault
projection before each existing heading phase, so a stationary cursor follows
wheel interpolation, camera corrections and viewport changes. It emits no
synthetic input event or direct send. The native host retains physical cursor
positions even when a menu consumes the move, and converts them with current DPR
at the next gameplay handoff/frame. This observation only seeds zoom's source;
it never admits a stock pointer event. From opening the in-game menu until the
next gameplay pointer move it is ignored, so the menu's cursor never steers. Returning to factor one restores the last
admitted CSS displacement, including stock's resize/centre quirks. A real pointer
callback is required to change that stock displacement.
The CSS dead zone and command cadence are unchanged. No wheel event
emits a command.

Zoom changes neither protocol nor subscriptions: only already-received objects
can appear. This does not reveal entities the server did not send.

## Bounds and synthetic measurement

The [draw/chunk contracts](draw-chunks.md) remain hard limits. Zoom-out keeps
pellet bodies and glows instead of discarding the entire food when its body is
subpixel. Each image uses its own extent: floor its drawn width/height at one
displayed physical pixel and multiply opacity by true/drawn area. A broad glow
therefore survives independently of its tiny body. The floor blends smoothly
from exact stock at factor one to full strength at 0.9. No protocol, movement
clock, source selection, animation phase or camera smoothing changes.

Snake decorations fade across 1-3 displayed logical pixels and body shadows
across 2-6, replacing their hard two/four-pixel switches. All geometry visits
are computed first so sample ordinals, distance cursors and antenna/death
barriers survive. Names shrink a retained stock glyph raster and fade below
two physical pixels instead of disappearing at a font-size cutoff. Native
presentation derives the displayed size from physical size/DPR and the unchanged
projection. Original still rasterizes into its capped target, so its final
filtering can differ from Native even with the same intended physical floor.
These are owner-approved presentation rules, not measured server mechanics.
See the [continuous zoom study](../../research/client-performance/2026-11-04-continuous-zoom/REPORT.md)
for authored threshold probes, controls and cost. The earlier
[shadow study](../../research/client-performance/2026-11-01-zoomout-shadows/REPORT.md)
records the superseded omission policy; its measurements remain historical.
A configured local background uses one repeating-texture quad regardless of tile
count; without local art, the world clears to black.
Native food planning combines this zoom policy with the retained catalog index
from [frame performance](frame-performance.md#retained-food-cpu-preparation).
Factor one keeps stock retained-source validation and draw ordering; catalog
changes refresh the index once per plan, without per-food asset-table rescans.
The existing pressure food allowance stays active at every zoom, selecting the
same newest visible foods on either side of factor one. Disabling it on the
first zoom-out microstep would abruptly admit previously excluded pellets.
Outside that pressure selection, geometry retains complete-or-reject picture
budgeting; there is no trimming at the renderer boundary.

The opt-in `frame_perf --zoom` workload uses only authored synthetic packets and
procedural assets: 65 snakes (64 with 120 points), 65,535 food, one prey and a
23,000-unit arena diameter at 960x540. It opens no socket/window. Compact native
zoom-out charges three existing paired quads per food: 196,605 for that food
population. The authored 4K whole-map replay reached **238,567 of 262,144** quads
including its other content. This is a measured fixture bound, not a universal
maximum. Other geometry can still exhaust the remaining budget.

A generic/canonical fallback needs up to seven commands per food: 458,745 for
65,535, already above the picture limit before snakes. World-preparation
admission rejects the complete picture; native release skips it and leaves the
previous presentation displayed. A menu backdrop also requires the canonical
outlines and primitive order. Menu composition retains the food inputs and
streams those primitives at submission, preserving the generic renderer's
chunking, layers and blending without materializing command/draw vectors.
Native preparation uses at most four participants, including the caller, for
independent projection and vertex stores; coverage merges in primitive order.
Large native command ranges record those same vertex chunks in render bundles
on up to four threads, then execute the bundles in their original order.
Wasm and pictures below 2,048 visible foods keep the serial stream.
It still charges the full seven-command budget. If composition exceeds it, the menu
overlay is skipped and the original world picture is submitted. No partial
pellet selection conceals the limit. Canonical food command-limit failures use
the same recognised draw-quota error, so optional composition takes this fallback
in strict builds as well as release.
Tests check compact retention, complete fallback rejection, current-radius fit
and draw/capacity/pixel bounds. These checks do not promise every population fits.

From the repository root, in PowerShell (one Cargo process at a time):

```powershell
$env:CARGO_TARGET_DIR = 'D:/CargoTargets/<worktree-folder>'
$env:CARGO_BUILD_JOBS = '2'
cargo build --release --locked -p slither-wgpu-slice --features frame-profile --example frame_perf
& "$env:CARGO_TARGET_DIR/release/examples/frame_perf.exe" --zoom D:/new/external/output
```

The driver settles interpolation, then alternates five rounds per level with
20 warm and 40 measured pictures each. `zoom.csv` records CPU world/HUD picture
build time (including validation/resources), logical quads, actual GPU draw calls,
draw capacity and active pixels. Synchronization, GPU waits, readbacks and PNG
encoding are outside build timing. Default/in/mid/out PNGs are offscreen
specimens from the same frozen world. Mid uses 17 notches out. These are feature
specimens, not a before/after fidelity comparison. Report CPU median/p95 and
round variation; GPU frame rate, physical wheel feel and owner acceptance remain
separate checks.

At 240 Hz the frame budget is 4.17 ms (170 Hz: 5.88 ms). The retained
continuous view remains above that in dense art workloads; timing tables state
CPU/GPU stages and spread without claiming presented cadence or a budget pass.
Preserve slow trials separately from the final matched measurements.

## Default-view regression

`tests/fixtures/zoom-stock-main.json` pins command bytes and complete frame/overlay
draw-list hashes captured from main at the fixture's source commit, using the
authored `announcement_tape` session and the three listed pointer/frame times.
The exact hash test runs on its pinned Windows/Rust target because native trig
and Debug float text can differ by platform. It does not compare two instances
of the zoom implementation. Cross-revision native CPU replay separately uses
the unchanged `feel-perceptual-v1` comparator and authored
`tools/feel-replay/browser-synthetic.json` schedule; neither check is a pixel or
physical-input acceptance claim.

The Windows factor-one golden keeps the historical `zoom-stock-main.json`
unchanged. `zoom-stock-black.json` removes only the three built-in background
draws: pristine `3209d873` reproduces every historical frame hash, then its
remaining Frame/overlay records and ordered material dimensions/pixels match
the black-background candidate byte for byte after replacing allocation handles
with content hashes. The fixture records those projection rules and byte hashes;
wire packets and stationary/repeated-pointer cases remain exact. This is an
authored CPU-frame control, not browser pixels or live gameplay acceptance.
