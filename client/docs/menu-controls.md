# Shared menu controls

Native login and champion editing share `PanelView`, `Control` and the
Label/Button/TextField painter. `login_controls` copies nickname, Play and secondary-action
rectangles from `MenuLayout`, retaining its resize-time `top` and the existing
admission/return projection. Logo, last-life line, page and spinner remain
specialized producers in the same transaction. Browser development pages still compile and retain their existing
login and pending-champion flow. This is not the extension input or GL port.

The login-only [Settings](settings.md) screen moves Quality out of the product
login. Its native controls now use egui and the same settings/persistence owner, with
a separate mesh pass after existing composition. Other menus retain this toolkit.
Done/Escape restore the Settings entry focus after the return fade.

The core resolves logical placements and clips with `MenuRect`, including
half-open hits and inverse mapping. `PanelInputState` owns reverse-order hits,
matching-release capture, explicit Tab order and sampled hover/press progress.
Removal, disable, blur and generation replacement cancel capture. A modal traps
Tab and restores a surviving parent control when that description returns.
Opacity is independent of input admission. Copied semantic nodes expose roles,
names, committed values, bounds and actions; a native OS accessibility bridge
is still separate work.

The small `EditorId` registry separates identity from `TextRule`. Existing
`TextEditState` selection, bounded undo, token-acknowledged clipboard work and
the IME Enter barrier serve both native paths. Equal descriptions and sampled
animation updates retain preedit. Unicode previews retain at most 256 UTF-16
units, without splitting a scalar; native adapters bound their copy before
measurement, and the core rechecks the limit and clamps byte selections. This
preview-only cap does not change committed normalization or the Enter barrier.
Fields declare left/top/right/bottom content padding and whether the value fits
or scrolls. The nickname retains its fitted text; champion text scrolls. Native
pointer hit-testing measures the same fitted font and uses the same content box
as painting. Core mouse policy supports drag selection, Shift+click from the
anchor, double-click word selection and triple-click Select All. A held drag
past either content edge selects to the start/end; release, blur and replacement
cancel capture. Composition suppresses selection and activation while release
still clears capture. On Windows the native adapter copies `GetDoubleClickTime`
and half of the `SM_CXDOUBLECLK`/`SM_CYDOUBLECLK` rectangle into logical units;
other platforms use 500 ms and a 4x4 logical-pixel rectangle.

The champion field scrolls horizontally and
clips text, selection, caret and preedit underline. Painting, pointer selection
and logical IME candidate placement share measured advances. Value, placeholder,
selection, caret, preedit underline and the horizontal content clip use
`menu_layout::single_line_field_placement`, including its font line-box centre.
The native candidate rectangle and painted caret share one physical-pixel snap. Native Lucida bold
still resolves to Arial Bold; synthetic checks do not establish OS IME feel or
font parity.

## Victory authority and send

Only the committed session outcome `Victory` mounts this editor. Rank, displayed
score and timers cannot grant entry. The menu follows the session return fade;
it owns no death timer. `LocalGame::save_champion` is the transport boundary:

- Recheck the current generation, existing socket and committed victory session.
- Encode `[255, 118, text…]` for the admitted protocol 19. For each UTF-16 unit,
  substitute a space outside inclusive 32..127, then retain at most 140 units.
  DEL survives; each surrogate half becomes a space, even at the last unit.
- Ignore empty Enter. Pointer/Space button activation may submit empty text.
- Enqueue on the existing transport, then lock the session against another send.
  A failed transport effect is terminal and never triggers an automatic retry.

This matches the repository's [local record wire contract](../../docs/raw-server.md#reports-and-game-over).
No profile acquisition, new socket or login authority participates in Save.
Life-control argument 2 publishes `champion_acknowledged` only after ingress
commits. The editor then fades out and ordinary controls fade in, at the same
core clock seam. Login focus still waits for the existing return/picture/session
admission. Transport close or error before acknowledgement ends the editor
through the same sampled fade and session return. It does not claim a saved
message, resend, or enable login before the existing return completes.

## Painting and bounds

`Menu::prepare_with_panel` combines specialized login producers and generic
controls in one material/text transaction. Typed theme content enters material/text keys; sampled
position and alpha do not; field text keys include their visible scroll window.
The holder uses tiled three-slice strips;
tall uniform surfaces use nine-slice tiles. Published atlas tiles stay at most
512 pixels per axis, and each tile charges the menu draw cap. Sources and
destinations land on whole physical pixels through the existing snapped overlay
path, with one source pixel per physical destination pixel.

The shared allowance is 64 Menu slots / 13 MiB and 512 Menu draws, with
aggregate caches at 386 slots / 31 MiB. The shared ceiling comes from #358's
full-growth 5x wordmark allowance on main. Isolated champion controls retain
a separate 4 MiB compactness regression. Final picture admission remains
64 MiB / 2,048 textures. Failed preparation publishes neither new caches nor
scroll state; already prepared resources survive replacement/retirement.
Field text is rasterized only across its visible horizontal window (plus one
physical pixel at either edge). Symmetric material atlases retain one half and
mirror their quads. Large selection rectangles reuse 128-pixel uniform tiles.
If menu preparation or whole-picture admission still exceeds a quota, native and
WebGPU game hosts skip that frame's menu overlay and retry on the next frame;
the world picture and editor/session state remain intact. This always-on path
matches exact producer-owned quota errors. Validation failures, including ones
whose wording contains "budget", and handle collisions still surface as errors.
No GPU feature, shader,
texture-storage or compute requirement is added.

## Offline checks

Core regressions: `cargo test --locked --test menu_controls --test menu_input`
from `client/`. App regressions: `champion_controls`, `login_native` and the
`champion` tests in `local`. They use authored synthetic packets and host
capabilities, without sockets, windows or a GPU.

`menu_specimens <new-output-directory> --champion` renders finite offscreen
champion, long-field and login comparisons at 1x, 1.5x and 2x. Set
`CARGO_BUILD_JOBS=2` when building/running it. Full client verification remains
`npm run check`. None of these checks establishes live server, physical IME,
owner appearance/feel or extension readiness acceptance.
