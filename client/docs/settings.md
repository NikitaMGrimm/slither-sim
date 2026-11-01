# Settings

The native login's secondary row is Change Skin, Choose Server, Settings, after
Nickname and Play in keyboard order. Settings opens only in admitted, settled
Login, with no pending Play, champion editor, skin studio or picker. Its shared
modal backdrop replaces login content and the wordmark. Tab wraps Quality →
World resolution → Done;
Enter/Space activates the focused control. Done and Escape close and restore
Settings focus. Outside clicks close on matching release. In-game Escape remains Stop/cancel.
Settings is the first screen on the shared native egui 0.37 host. Other native
menus retain their adapters until ported, and browser hosts retain DOM controls. Settings uses the
shared picker transition timing: 400 ms in, 320 ms out, with a 40 ms step cap.
The outgoing panel keeps input until its fade finishes. Keyboard zoom is disabled;
the host's physical size and DPI remain authoritative.

Quality is the existing requested High/Low mode, independent of adaptive runtime
quality and resolution. Changes apply immediately to preferences; Play freezes
that value for the next game through the existing `MenuQuality` → `QualityMode`
mapping. High retains automatic degradation/recovery.

World resolution defaults to Original. The Native/sharp toggle requests physical
world attachments with the same view area: “Sharper world; higher GPU use. Same
view area.” It applies at the next safely prepared picture, including after the
login closes, without changing the game's Quality state. Unsupported dimensions
leave Original effective and show “Native unavailable at this size”; the request
is retained and can become effective on a supported resize. See
[world resolution](world-resolution.md) for target ownership and costs.

`settings::SettingsPreferences` owns the bounded ASCII record
`2:<0|1>:<original|native>` (64-byte read/parse limit). Valid v2 wins; valid
quality-only v1 (`1:<0|1>`) migrates in memory with Original. Otherwise exactly
legacy `"0"` means Low, and every other legacy value, including missing/unreadable,
means High; resolution defaults to Original. Startup
does not write or repair malformed, oversized or future records. Appearance and
server-choice records remain separate.

Native uses `%LOCALAPPDATA%/SlitherClient/settings.txt`, reads at most 65 bytes, and
shares the appearance adapter's same-directory temporary-write/atomic-replace
helper. A failed startup read falls back in memory without writing. A failed write
keeps the old file and in-memory choice and shows “Not saved; applies this session”.
Each explicit change writes canonical v2 and attempts persistence again, including after a startup read
failure. Native had no persisted legacy Quality
file. The browser dev/reference hosts retain their UI and legacy key, migrating
through the same copied Wasm parser at `slither-client-settings-v1` and retaining
`slither-client-quality-v1` for older hosts. They perform no startup writes.

The two egui controls expose checkbox semantics and checked state; matching
pointer release and Enter/Space carry an explicit next value into the same
generation-checked settings owner. The host tracks ordered held-key state before
egui processing, including egui-winit auto-repeat events marked `repeat: false`.
A held opener cannot activate the new screen; key releases and blur reset the
gate. Tab/Shift+Tab wrap
without an unfocused frame. Blur, resize, DPI changes and a new mount cancel
pointer capture. Nickname composition blocks entry; the modal releases gameplay
holds and disables nickname IME. Native accessibility bridging remains separate.

Offline tests cover record precedence, write failures, entry/return focus,
modality, capture cancellation and Play snapshots. Layout tests check containment,
non-overlap and keyboard reachability across narrow/wide windows and DPR 1–8;
they do not compare pixels or exact coordinates. The panel uses flow layout and
scrolling on short windows. Labels/toggles/headings retain 18/20/24px type, with
16px help and status text, using the same installed font bytes as native text.
The shared `egui_theme.rs` module owns style/fonts, pills, checkbox rows, titles
and the panel drop shadow. Width, spacing and visual changes belong there; the
existing `MenuTheme` remains the colour-token authority. Focus/shadow space is
reserved outside the widget ink. The lazy native host owns the context, input
adapter, glyph atlas and meshes; the former
Settings-only raster-resource quotas no longer describe this screen. Other
menus retain their existing quotas and retained pictures remain valid.

On native non-Linux hosts (the app tests and specimen tool are not built on Linux),
`menu_specimens <new-output-directory> --settings`
produces authored offscreen Settings PNGs at 960×540 and 360×640, plus
`timings.json`: 1,000 warmed preparation samples and 120 CPU submission samples
per size, with GPU completion waited outside each timed CPU frame. No art
directory is loaded. Run with `CARGO_BUILD_JOBS=2`. Full verification is `npm run check`, which skips native
wgpu checks on Linux; neither proves physical input, owner appearance/feel or
server acceptance. These checks open no windows and contact no game servers.

The host stays uninitialized until an egui menu activates. While closed it does
not translate events, drain raw input or mutate the context; a cold/warm
allocation regression exercises 3,000 closed callbacks/drains without allocations.
Physical cursor and DPI bookkeeping runs before routing, so closing Settings
and clicking without moving uses the current point in the returning menu.

The historical [trial measurements and renders](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client/2026-11-01-egui-settings-trial/REPORT.md)
remain pinned to their measured revision. They do not measure the new host.
See [dependency licences and Unicode notice](egui-licenses.md).
