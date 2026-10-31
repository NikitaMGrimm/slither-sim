# Settings

The native login's secondary row is Change Skin, Choose Server, Settings, after
Nickname and Play in keyboard order. Settings opens only in admitted, settled
Login, with no pending Play, champion editor, skin studio or picker. Its shared
modal backdrop replaces login content and the wordmark. Tab wraps Quality → Done;
Enter/Space activates the focused control. Done and Escape close and restore
Settings focus. Outside clicks do not dismiss it. In-game Escape remains Stop/cancel.

Quality is the existing requested High/Low mode, independent of adaptive runtime
quality and resolution. Changes apply immediately to preferences; Play freezes
that value for the next game through the existing `MenuQuality` → `QualityMode`
mapping. High retains automatic degradation/recovery. No world-resolution setting
or placeholder is included.

`settings::SettingsPreferences` owns the bounded ASCII record `1:<0|1>` (64-byte
read/parse limit). A valid record wins; otherwise exactly legacy `"0"` means Low,
and every other legacy value, including missing/unreadable, means High. Startup
does not write or repair malformed, oversized or future records. Appearance and
server-choice records remain separate.

Native uses `%APPDATA%/SlitherClient/settings.txt`, reads at most 65 bytes, and
shares the appearance adapter's same-directory temporary-write/atomic-replace
helper. An unreadable file disables writes for the run. A failed write keeps the
old file and in-memory choice and shows “Not saved; applies this session”. It
retries only on the next explicit change. Native had no persisted legacy Quality
file. The browser dev/reference hosts retain their UI and legacy key, migrating
through the same copied Wasm parser at `slither-client-settings-v1` and retaining
`slither-client-quality-v1` for older hosts. They perform no startup writes.

The shared Toggle exposes checkbox semantics, checked state and SetChecked action;
matching pointer release and Enter/Space carry an explicit next value. Blur,
disable, resize and generation replacement cancel capture. Native accessibility
bridging remains separate.

Offline tests cover record precedence, write failures, entry/return focus,
modality, capture cancellation and Play snapshots. Settings retains the existing
global menu/picture quotas and adds a compactness regression of 32 resources,
4 MiB and 160 producer quads through DPR 1–5, with ≤64 quads through DPR 2.
Inactive screen resources retire transactionally; retained pictures remain valid.

`menu_specimens <new-output-directory> --settings` produces finite offscreen PNGs
and `settings.json` with geometry, revision, fonts, resource/quad costs and actual
GPU draw runs. It includes 960×540 at 1×/1.5×/2×, narrow and DPR-5 cases, selected,
focused, pressed, disabled and storage-failure states. Run with
`CARGO_BUILD_JOBS=2`. Full verification is `npm run check`; neither proves physical
input, owner appearance/feel or server acceptance. These checks open no windows
and contact no game servers.
