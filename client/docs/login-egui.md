# Native login

`login_egui.rs` renders the native start and after-death screen in the shared
`NativeEguiHost`. `StartMenuState`, `MenuLayout` and session policy still own
wording, measured geometry, reveal/return clocks and Play admission. Fonts come
from the same installed font bytes as the retained menu painter; pills and link
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

Settings shares the egui context during its fade. Server picker, skin studio and
champion retain their existing toolkit, input adapters and transition composition
until their own ports. The outgoing login fade still paints while gameplay owns
steering, zoom and Esc. No egui pass, input translation or drain runs when all
egui menus and their fades are inactive. Browser DOM menus are unchanged.

`tests/login_native.rs` exercises the real CPU widget tree: unique focus,
keyboard reachability, pointer actions, containment at small/large sizes and
DPR 1/1.5/2, nickname limits/save restoration, composition/commit/cancellation,
repeats, receipt-derived final length and session replay. Settings, studio,
endpoint and in-game suites retain their integration rules. The inactive-host
allocation regression remains in `tests/egui_inactive.rs`.

The finite `menu_specimens OUTPUT --login` example renders authored offscreen
before/after PNGs without a window or connection. It replaces the old retained-login
export and rejects its former art-directory argument. Its before side is the retained
toolkit specimen fed the same state/layout; it is not a desktop capture. These checks
establish synthetic behavior and offscreen rendering, not physical IME or
perceptual parity acceptance.
