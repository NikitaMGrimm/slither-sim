# Shared menu controls

Native login, Settings, the server picker, the skin studio and the champion
editor are egui screens. The core still owns their state and layout: `PanelView`
and `Control` describe each panel, `login_controls` copies nickname, Play and
secondary-action rectangles from `MenuLayout`, retaining its resize-time `top`
and the existing admission/return projection, and egui paints and hit-tests at
the resolved rectangles. Browser development pages still compile and retain
their existing login and pending-champion flow. This is not the extension input
or GL port.

The login-only [Settings](settings.md) screen moves Quality out of the product
login. Done/Escape restore the Settings entry focus after the return fade.

The core resolves logical placements and half-open clips with `MenuRect`.
`egui_panel` paints the shared widgets; `login_native` routes each raw event to
the owning screen, in order. Capture, held-key and touch-owner handling stay in
that route and `egui_native`.

Login, picker, studio and champion buttons act on the release matching their
press, however long it was held and even after the pointer left and came back:
`egui_panel::pointer_click` is free of egui's 0.8-second and 6-point click limits.
A blur, touch cancel, disable, screen replacement or moved list window cancels
the press. Those limits stay global because text selection needs them. Settings
and the in-game menu keep them through `owned_click`, and scrolling a held Settings
toggle cancels its press. The in-game backdrop dismisses only when the press and
the release are both outside the panel. On every screen, only a primary press on
the widget counts: another mouse button can't hand one over. Two clicks within 0.5 s, the
Windows default, select a word on every platform; a custom OS double-click
setting is not read.

The champion field applies `TextRule::Champion`: 140 UTF-16 units, normalized on insertion
and again after each pass. Its Unicode preview retains at most 256 UTF-16 units,
without splitting a scalar, and never counts as committed text.

`champion_egui` keeps these editor rules:

- A press on a control moves focus; a press elsewhere keeps it and starts no
  selection. A window blur keeps logical focus and retires the press.
- The keyboard ring appears after Tab, Enter, Space, Left, Right, Home or End,
  never from typing, and a press clears it.
- Tab cycles message and Save, including on auto-repeat, unlike the other egui
  screens. Composition consumes it.
- Enter in the field and Enter or Space on Save act once per physical press.
  Caret keys and Backspace repeat in the field. Up and Down do nothing, and
  neither do Left, Right, Home and End on Save.
- Escape only cancels a composition, restoring the snapshot taken before it.
  Composition owns Enter, so a commit's own press cannot send the message.
- Pointer input during a composition leaves the selection alone and activates
  nothing; a release retires any press from before it.

Save's hover and press overlays step linearly on the core clock with the theme's
rates. Native Lucida bold still resolves to Arial Bold; synthetic checks do not
establish OS IME feel or font parity.

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

Field, Save and labels paint at the core's resolved rectangles and scale, so
opacity fades never move geometry. The message field keeps the old field raster's
deep shadow and labels keep the canvas `middle` baseline. The caret is egui's: it
spans the text row rather than the font size, as in the login and picker fields.
Text clipping and scrolling are `TextEdit`'s, and text lives in egui's font atlas.
A failed page composition skips that frame's page and leaves the world picture and
editor state intact.

## Offline checks

Native presentation sleeps when the visible menu has settled. Input, discovery
results and session changes invalidate the picture; egui repaint deadlines wake
it for caret blink and widget updates. Surface retries retain their backoff.
The initial login's rotating tips, connecting spinner, skin studio snake preview
and retained active game continue presenting. Settings fully hides the login
animation once its entry fade finishes. A returned login has no rotating tips.
Menu fades and the session's hidden death hold keep their existing clocks.

`idle_menu` is a CPU-only regression: over 600 synthetic 60-Hz opportunities in
10 seconds, after an initial picture, the old visibility decision presents 600
frames for both settled Settings and returned login; the animation decision
presents zero. Initial login tips present 600 on both sides. The counter excludes
egui settling/caret repaints, OS events, real vsync and GPU submission; it measures
scheduling decisions, not desktop frame rate, CPU use or power.

Core regressions: `cargo test --locked --test menu_controls --test menu_input`
from `client/`. App regressions: `champion_egui`, `login_native` and the
`champion` tests in `local`, which feed raw egui events with authored synthetic
packets and no sockets, windows or GPU.

`menu_specimens <new-output-directory> --champion` renders finite offscreen
states at 1x, 1.5x and 2x from an authored tape victory: a short message, a long
one, and keyboard focus on Save and the field. Set `CARGO_BUILD_JOBS=2` when
building/running it. Full client verification remains `npm run check`.
None of these checks establishes live server, physical IME,
owner appearance/feel or extension readiness acceptance.
