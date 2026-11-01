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
radii to avoid clipping the easing boundary. A settled endpoint follows radius,
viewport and stock-scale changes immediately.

The user factor interpolates in log space by 18% of the remaining distance per
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
it never admits a stock pointer event. Returning to factor one restores the last
admitted CSS displacement, including stock's resize/centre quirks. A real pointer
callback is required to change that stock displacement.
The CSS dead zone and command cadence are unchanged. No wheel event
emits a command.

Zoom changes neither protocol nor subscriptions: only already-received objects
can appear. This does not reveal entities the server did not send.

## Bounds and synthetic measurement

The [draw/chunk contracts](draw-chunks.md) remain hard limits. Before allocating
food commands or looking up their unused sprite resources, zoom-out omits bodies
smaller than one logical pixel, including their outline/glow passes; names below
one pixel are omitted before text rasterization. Bodies narrower than two logical
pixels use the existing low-quality passes: omit outline/shadow/inner shimmer,
retaining the body, outer boost and death passes. These rules are opt-in to
zoom-out; default output remains exact.
The background already uses one repeating-texture quad regardless of tile count.
Native food planning combines this zoom policy with the retained catalog index
from [frame performance](frame-performance.md#retained-food-cpu-preparation).
Factor one keeps stock retained-source validation and draw ordering; catalog
changes refresh the index once per plan, without per-food asset-table rescans.
Other geometry retains existing complete-or-reject picture budgeting; there is
no unbounded allocation or geometry trimming at the renderer boundary.

The opt-in `frame_perf --zoom` workload uses only authored synthetic packets and
procedural assets: 65 snakes (64 with 120 points), 65,535 food, one prey and a
23,000-unit arena diameter at 960x540. It opens no socket/window. Full-quality
food alone could request 458,745 quads if all were visible, exceeding the
262,144-picture ceiling before snakes; subpixel LOD prevents that case at the
whole-map scale. Tests check current-radius fit and picture draw/capacity/pixel
bounds with this fixture. This establishes resource behavior for the stated
workload, not every possible server population.

From the repository root, in PowerShell (one Cargo process at a time):

```powershell
$env:CARGO_TARGET_DIR = 'D:/CargoTargets/<worktree-folder>'
$env:CARGO_BUILD_JOBS = '1'
cargo build --release --locked -p slither-wgpu-slice --example frame_perf
& "$env:CARGO_TARGET_DIR/release/examples/frame_perf.exe" --zoom C:/new/external/output
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

Use 16.7 ms p95 as a diagnostic CPU picture-build target for maximum zoom-out on
this workload. It is not a frame-rate promise or an iGPU measurement. Preserve
slow trials and report their scope separately from the final measurements.

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
