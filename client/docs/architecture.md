# Architecture

The product has three layers: a shared Rust client core, thin platform adapters,
and renderers. The slither-sim crate at the repository root owns server simulation.

| Path | Responsibility |
| --- | --- |
| ../crates/slither-protocol | Borrowed wire decoding and serialization |
| src/protocol* | Client validation and error/ABI adapters to the shared reader |
| src/challenge.rs | Client-local closed challenge grammar and reply transform |
| src/state.rs | Atomic packet application and received world state |
| src/presentation* | Animation, prediction/correction, camera, skins, ordered draw plans and HUD |
| src/*_policy.rs, src/desktop_input.rs, src/menu_*.rs | Session, input, gameplay and menu policy |
| src/*_browser.rs | Copied Wasm API and Canvas submission |
| web/ | Canvas reference client, browser/DOM/socket glue, authored procedural sources |
| apps/wgpu-slice/ | Shared wgpu renderer, Windows host and browser WebGPU adapter |
| tests/, tools/*.test.mjs | Authored synthetic behavior and adapter regression tests |
| tools/ | Build, local development, ABI verification and source guard |
| .github/workflows/checks.yml | Offline CI on Linux and Windows |

## Shared core

Protocol packets update state atomically. Presentation retains its own timeline,
interpolates/corrects movement and prepares ordered body, food, prey, face and
arena plans. Session and input policies own connection phases, timing, ping,
turning and boost intents. HUD/menu owners retain scores, leaderboard, minimap,
appearance and player-facing state.

[Snake capacity](snake-capacity.md) separates the 512-live-ID received world from
bounded visual geometry, curves, recipes and draw storage. The own object keeps
full detail; previously inadmissible large worlds trim rival presentation.

Setup selects the shared crate's protocol-13 or protocol-19 reader for the
connection. The 13 adapter maps explicit IDs, coordinate anchors, all appearance
deltas and absolute food keys into the same presentation inputs. A later Setup
keeps the selected version; reset starts a new selection. HUD/minimap layouts
are shared between the readers. Login and outbound command encoding are unchanged.

Adapters execute socket, DOM and GPU effects outside Rust borrows and acknowledge
successful work. Preserve packet ordering, arithmetic and transactional rollback
when changing these boundaries; do not add another policy reducer in JavaScript.

## Canvas reference

The local page at / is served by tools/scene-server.mjs. It loads
web/game-bootstrap.mjs, creates the local host and starts web/game-runtime.mjs.
The runtime uses the Wasm core with Canvas2D, authored procedural materials and
owned DOM UI. Module URLs under /game are an explicit server map; generated
bindings live in ignored dist/web. npm run scene remains the standalone server.

Some stock-source adapters are still used by the Canvas composition and its
regression tests. The former document-start host is retained only under
tools/fixtures for those inert tests. There is no extension package or injection
entry on main; the parked extension work is preserved on origin branches
fix/http-com-module-integrity, fix/http-com-live-play and feat/default-appearance.

## Shared wgpu client

apps/wgpu-slice is a separate crate in the root Cargo workspace with a path
dependency on the core. game::LocalGame owns the shared protocol/session/input/presentation
flow. Windows uses winit and tungstenite; the browser uses WebSocket and RAF.
Both submit ordered frames and authored materials through the same wgpu renderer.

Play opens a direct endpoint or, by default natively, an official server chosen
from the directory measured in the background at startup; there is no automatic reconnect. The
renderer covers body, food, prey, paired eyes, arena and platform text for
nicknames, the HUD, leaderboard and official record announcements.
Image faces, antennas and complete chooser menus remain incomplete. Rasterization can
differ from Canvas; passing tests does not establish perceptual parity.

Shared [panel text primitives](panel-text.md) add styled, bounded wrapping and an
independent lazy cache. Production `LocalGame` enables the shared [record
announcement projection](announcements.md) on native and browser WebGPU,
capturing immutable text draws before minimap/HUD at each Setup, ordinary and
resize preparation. [Bounded vertex chunks](draw-chunks.md) preserve crowded
world geometry without overlay reservations; producer/cache quotas remain.
The returning login follows the session lifecycle. Native champion editing uses
the [shared menu controls](menu-controls.md), with committed victory/send/ack
authority; browser development hosts retain the pending champion handoff. Fragment opacity multiplies the shared retained
fade outside raster keys. Existing nickname and HUD cache quotas stay unchanged.
The [menu resource layer](menu-resources.md) supplies native login controls and
semantic local art. The [server picker](server-picker.md) uses shared directory
and windowed-list policy. Both browsers share a semantic DOM login overlay backed by
the same Rust state, clocks and layout; world renderers draw the world only.

Native login, Settings, the server picker and the in-game menu use one lazy
`egui_native::NativeEguiHost`. It owns
the context, egui-winit adapter, ordered input, texture deltas, meshes and
egui-wgpu renderer.
Closed screens bypass translation, draining and context mutation. Window cursor,
DPI, focus, modifier and held-key bookkeeping precedes input ownership; activation
synchronizes the adapter from that state. Repeated key-down events are filtered
before custom Tab traversal and egui processing. Keyboard, pointer, wheel, touch
and IME events reach the owning screen at once, in order
(`egui_native::immediate_menu_input`); leftover adapter input takes the same
ordered route on the next frame, never one raw egui batch. While the
champion editor owns input, the host stays active in paint-only mode (no
translation, platform output or text input) so egui keeps painting the login,
and the desktop presenter draws the editor's old-toolkit draws after egui. The
skin studio is an egui screen; the presenter draws only its preview snake after
egui.

The [native login](login-egui.md) uses `login_egui.rs` for editable nickname,
Play and menu entry points, including the after-death result. Adjacent IME
preedit updates wait for redraw; commits and discrete actions retain order.
`StartMenuState` and session policy retain normalization, persistence boundaries
and transition/admission rules. `ingame_egui.rs` owns
the single Leave game button and outside-click dismissal. `LocalGame` gates
native steering while the overlay is open, releases held boost/turn debt at entry,
and keeps the existing ping/presentation flow running. Leave and an unadmitted
Esc queue the existing Close effect through `request_stop()`. The native effect
owner reuses `begin_close()` and its five-second deadline before acknowledging
that effect and running the existing session teardown/login return. Frames keep
presenting while closing, with no new game sends; Enter retains a Play request
until the old socket has been released. Dismissal samples the current window cursor
without resetting heading cadence. Browser DOM menus are unchanged.
`server_picker_egui.rs` paints the shared `ServerPicker` panel at its resolved
control rectangles; the core keeps choices, list windows and validation, and
`server_picker_native.rs` keeps preference and custom-server storage.
`settings_egui.rs` owns layout and
actions; the shared `settings::SettingsState` owns preferences, fade timing,
generation checks and persistence. `egui_theme.rs` maps `MenuTheme` and installed
fonts to egui style, with shared pill buttons, checkbox rows, titles, shadowed
panel frames and the IME composition snapshot. Other screens will use the same
host/frame callback and widgets
after they are ported, without another context, input adapter or render pass
integration. The host paints
after existing composition and before presentation on the same device and queue.
Other native menus retain their adapters until ported; browser menus retain DOM
controls. See [Settings](settings.md) and [dependency licences](egui-licenses.md).

The wgpu crate still includes its small synthetic diagnostic executable and
examples alongside the renderer. See its README for builds and controls.

[World resolution](world-resolution.md) keeps projection and input in the shared
core while the presenter selects Original backing/blit or Native direct resolve.

The world clears to solid black on native wgpu, browser wgpu and Canvas2D when
no local background images are supplied. There is no built-in background art.
Optional local `s/bg54.jpg` (repeating tile) and `s/gbg.jpg` (backdrop) retain the
shared camera phase/zoom and quality plan; see [Optional cosmetics](../apps/wgpu-slice/README.md#optional-cosmetics).
Canvas retains supplied-image pattern submission for the reference and parity
tooling, without generating any default tile or backdrop.
