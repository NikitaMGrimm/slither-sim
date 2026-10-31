# Shared menu controls

Native champion editing is the first adopter of `PanelView`, `Control` and the
Label/Button/TextField painter. Login keeps its existing control/input path;
logo, last-life line, page and spinner remain specialized producers in the same
transaction. Browser development pages still compile and retain their existing
login and pending-champion flow. This is not the extension input or GL port.

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
animation updates retain preedit. The champion field scrolls horizontally and
clips text, selection, caret and preedit underline. Painting, pointer selection
and logical IME candidate placement share measured advances. Native Lucida bold
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
admission. Transport close without that acknowledgement keeps a disabled
champion handoff; it cannot claim that a message was saved.

## Painting and bounds

`Menu::prepare_with_panel` combines specialized login producers and generic
controls in one material/text transaction. `prepare_panel` also supports isolated
headless specimens. Typed theme content enters material/text keys; sampled
position, alpha and scrolling do not. The holder uses tiled three-slice strips;
tall uniform surfaces use nine-slice tiles. Published atlas tiles stay at most
512 pixels per axis, and each tile charges the menu draw cap. Sources and
destinations land on whole physical pixels through the existing snapped overlay
path, with one source pixel per physical destination pixel.

The shared allowance remains 64 Menu slots / 4 MiB and 512 Menu draws, with
aggregate caches at 386 slots / 22 MiB. Final picture admission remains
64 MiB / 2,048 textures. Failed preparation publishes neither new caches nor
scroll state; already prepared resources survive replacement/retirement.
No GPU feature, shader, texture-storage or compute requirement is added.

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
