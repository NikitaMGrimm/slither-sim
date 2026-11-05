# Native login

`login_egui.rs` renders the native start and after-death screen in the shared
`NativeEguiHost`. `StartMenuState`, `MenuLayout` and session policy still own
wording, measured geometry, reveal/return clocks and Play admission. Fonts come
from the installed font bytes `TextCache` reads; pills and link
glyphs use `egui_theme`. The authored stroke wordmark and spinner retain their
procedural producers, with bounded egui texture uploads.

Tab and Shift+Tab traverse nickname, Play, Change Skin, Servers and Settings,
skipping unavailable controls. Pointer and keyboard Play retain their distinct
return admission rules. Committed nickname input uses the existing per-UTF-16
substitution and 24-unit whole-value limit. Editing alone does not write storage;
the existing skin Save boundary persists the committed nickname.

The egui-winit adapter owns native composition, candidate placement and platform
output while login owns input. Adjacent preedit replacements coalesce until
redraw; commit, keys and pointer boundaries retain order. Provisional Unicode is
bounded separately from the committed nickname. Cancellation, including native
IME Disabled, restores the pre-composition text, selection and copied undo history.
Committed edits keep their undo history without provisional candidates. Windows
clipboard access uses the existing clipboard dependency, since egui-winit's
optional system clipboard is disabled.

Settings, the [server picker](server-picker.md), the
[skin studio](skin-studio.md) and the [champion editor](menu-controls.md) share
the egui context during their fades; login paints below them without input. The
desktop host presents only the studio's preview snake in a layer above egui.
On native non-Linux hosts (the app tests and specimen tool are not built
on Linux), `menu_specimens OUTPUT --studio` renders that order offscreen for
the studio entering and open over the settled login; elsewhere it fails rather
than writing other specimens. The outgoing login fade still paints while gameplay owns
steering, zoom and Esc. No egui pass, input translation or drain runs when all
egui menus and their fades are inactive. Browser DOM menus are unchanged.
The disabled outgoing controls allow pointer input through to gameplay.

Game HUD visibility comes from `session_policy::View::game_hud_visible`: only
active Playing sessions may display it. Dead/victory returns, connecting and
terminal login cannot display retained leaderboard, minimap, readout or record
announcements. The death world alone keeps its existing hold/crossfade; terminal
sessions blank it, matching the reference's disconnect reset. Native in-game
menus also suppress HUD while retaining their live world. `LocalGame::take_frame`
rechecks the policy for queued and retried pictures; the shared HUD projection
does not discard retained scores or last-life history. Authored transitions and
the offscreen reproduction live in `tests/menu_hud.rs` and
`examples/menu_hud_specimens.rs`.
The desktop host also supplies visible-menu admission at issue, so an outgoing
login still being drawn over accepted gameplay suppresses HUD until it is gone.

During the visible death-return fade, Servers and Settings already accept input;
Change Skin waits for Login. The panel keeps its own opacity and modal input
while the session continues the login/world crossfade underneath. Closing before
return completion defers nickname focus to the existing completion intent; a
panel retained into Login keeps focus until its ordinary close handoff. The
hold, return clock, queued Play and receipt-derived final length are unchanged.
An Enter deliberately buffered on the login during explicit Close is retained
while Servers or Settings is open, including its exit fade, and dispatches once
after the panel releases input and Play becomes available. Enter inside either
panel belongs to its controls and never buffers another Play.
An accepted native Play attempt consumes any older buffered request, so a failed
replacement connection cannot trigger another Play without input. Rejected
admission retains the buffer. Waiting through the panel exit is an intentional
deviation from the reference picker, because native panels retain modal input.
Authored regressions are in `tests/return_panels.rs`.

After the death return completes, the menu keeps presenting while the old
socket's Close handshake finishes (up to five seconds). Servers, Change Skin and
Settings remain usable; Play still waits for Close acknowledgement and committed
presentation. The synthetic
`returned_menu_keeps_presenting_buttons_while_death_close_is_pending` regression
withholds Close while opening each panel and preserving the connection generation.

After a committed own-death removal or game-over, socket resets and abnormal
closes end the native session quietly. Death observations belong to one connection;
rival deaths and removals without the death flag do not hide failures. Received
terminal packets are drained before EOF classification, including packets fenced
by pending sends or presentation. Unacknowledged frame candidates are discarded.
Earlier transport failures remain visible as bounded, wrapped text between the
logo and nickname field, replacing the last-length and hint rows while present.
The complete message is visible without hover; narrow layouts reduce its text
size only when needed to fit. The next admitted Play clears
the diagnostic before asynchronous profile acquisition, so a later acquisition
failure reports its own error.
The stock reference keeps WebSocket errors silent and has no visible error label;
this native diagnostic uses the existing login typography and fade. Browser
connection-message presentation is unchanged.

`tests/login_native.rs` exercises the real CPU widget tree: unique focus,
keyboard reachability, pointer actions, containment at small/large sizes and
DPR 1/1.5/2, nickname limits/save restoration, composition/commit/cancellation,
repeats, receipt-derived final length and session replay. Settings, studio,
endpoint and in-game suites retain their integration rules. The inactive-host
allocation regression remains in `tests/egui_inactive.rs`.

The finite `menu_specimens OUTPUT --login` example renders authored offscreen
PNGs without a window or connection; it is not a desktop capture. These checks
establish synthetic behavior and offscreen rendering, not physical IME or
perceptual parity acceptance.
