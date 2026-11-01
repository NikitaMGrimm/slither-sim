# Settings

The native login's secondary row is Change Skin, Choose Server, Settings, after
Nickname and Play in keyboard order. Settings opens only in admitted, settled
Login, with no pending Play, champion editor, skin studio or picker. Its shared
modal uses the server picker's shared fade and scale ramp. Login content fades
behind it, and the settled panel replaces inactive login/wordmark resources.
Tab wraps Quality → World resolution → Done; Enter/Space activates the focused
control. Done, Escape and a backdrop click start the exit fade. The panel keeps
visual feedback and input ownership until opacity reaches zero, while closed
controls reject activation. Focus then returns to Settings. In-game Escape
remains Stop/cancel.

Quality is the existing requested High/Low mode, independent of adaptive runtime
quality and resolution. Changes apply immediately to preferences; Play freezes
that value for the next game through the existing `MenuQuality` → `QualityMode`
mapping. High retains automatic degradation/recovery.

World resolution defaults to Original. The Native/sharp toggle requests physical
world attachments with the same view area: “Sharper world; higher GPU use. Same
view area.” It applies at the next safely prepared picture, including after the
login closes, without changing the game's Quality state. Native uses MSAA4 only
while its world MSAA target fits 64 MiB; larger sizes use single-sample physical
coverage (including 4K). Original keeps its existing MSAA. Unsupported dimensions
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

The two shared Toggles expose checkbox semantics, checked state and SetChecked action;
matching pointer release and Enter/Space carry an explicit next value. Blur,
disable, resize and generation replacement cancel capture. Native accessibility
bridging remains separate.

Offline tests cover record precedence, write failures, entry/return focus,
modality, capture cancellation and Play snapshots. Settings retains the existing
global menu/picture quotas. The settled-state compactness regression keeps 32 resources,
4 MiB and 160 producer quads through DPR 1–5, with ≤64 quads through DPR 2.
Menus retain the DPR-5 raster cap and physical-resolution placement through DPR 8.
The rounded settings panel stretches only uniform centre/edge strips; its corners
and shadow retain their raster density. Important labels/toggles/headings keep
18/20/24px type, with 16px instructions and status text.
Inactive screen resources retire transactionally; retained pictures remain valid.

On native non-Linux hosts (the app tests and specimen tool are not built on Linux),
`menu_specimens <new-output-directory> --settings` produces finite offscreen PNGs
and `settings.json` with geometry, revision, fonts, resource/quad costs and actual
GPU draw runs. It includes 960×540 at 1×/1.5×/2×, narrow and DPR-5 cases, selected,
focused, pressed, disabled, Native-unavailable and combined storage-failure states. Run with
`CARGO_BUILD_JOBS=2`. Full verification is `npm run check`, which skips native
wgpu checks on Linux; neither proves physical input, owner appearance/feel or
server acceptance. These checks open no windows and contact no game servers.

`menu_specimens <new-output-directory> --settings-fade` captures fixed-clock
entry/exit frames at 1×/1.5×/2× and a DPR 1–8 ledger, including fractional DPR,
narrow/short-wide windows, keyboard/mouse entry, storage/capability messages and
sampled control feedback. It uses procedural geometry only. Partial transitions
share the unchanged global 512-record, 64-resource, 13 MiB menu allowance.
