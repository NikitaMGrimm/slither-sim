# Settings

The native login's secondary row is Change Skin, Choose Server, Settings, after
Nickname and Play in keyboard order. Settings opens during the visible return
fade and in Login even while connection admission is unavailable, with no pending Play, champion editor, skin
studio or picker. Its shared
modal backdrop replaces login content and the wordmark. Tab wraps graphics Auto → Low → Medium → High → World resolution →
AA Auto → Off → 2× → 4× → Done;
Enter/Space activates the focused control. Done and Escape close and restore
Settings focus after Login has settled. Closing during return leaves nickname
focus to the existing return-completion intent. Outside clicks close on matching release. During play, Escape
opens the single-action Leave game overlay; it still cancels a pending connection.
Settings shares the native egui 0.37 host with that overlay. Other native
menus retain their adapters until ported, and browser hosts retain DOM controls. Settings uses the
shared picker transition timing: 400 ms in, 320 ms out, with a 40 ms step cap.
The outgoing panel keeps input until its fade finishes. Keyboard zoom is disabled;
the host's physical size and DPI remain authoritative.

The Graphics section has one quality preset, one world-resolution choice and one
world anti-aliasing choice. Quality applies to the next game and freezes at the
accepted Play click, including a queued return-to-login Play. Resolution and AA
apply at the next safely prepared picture. There are no independent effect toggles.
The applied effect preset changes on connection initialization; changing the
stored request alone leaves the retained picture intact. Skin previews reset
the applied effects to full quality independently of the saved preset.

| Preset | Requested quality / adaptive degradation | Food glow | Snake detail / shadows / far-zoom extras | Reason |
| --- | --- | --- | --- | --- |
| Auto (default) | Existing High, unchanged draw-count degradation/recovery | Existing quality blend | Existing density 1–1.7, shadows, decorations and continuous zoom fades | Preserve today's default picture and runtime response on every adapter; no adapter-based picture change. |
| Low | Existing Low state, no automatic recovery | Off | Density 1.7; omit body/near-head/tail shadows and outer boost halo after complete geometry visits; omit accessory images and antenna strokes below stock zoom | Reduce fill and geometry cost on weak GPUs without changing motion, body sample ordinals or antenna/death state. |
| Medium | High state, no automatic degradation | Off (existing Low food painter) | Density 1.35; retain shaded bodies, shadows, boost and zoom decorations | Remove broad food fill first; intermediate spacing lowers snake work while retaining depth cues. |
| High | High state, no automatic degradation | Full | Density 1; all existing effects and zoom fades | Explicit full detail even when frame-count adaptation would reduce Auto. |

All presets retain source rasters/filtering, visibility rules, movement and wire
semantics. Auto uses exactly the old High operations; High differs from Auto only
when adaptation would engage. Auto's degraded picture also retains the legacy
effects: its requested mode stays High, so the explicit Low shadow cuts never
apply to automatic fallback. Medium's 1.35 density is the authored midpoint of
the existing 1–1.7 range, not a fitted hardware threshold. Low computes complete
body visits before filtering effects so decoration state does not drift.

Anti-aliasing selects Auto / Off / 2× / 4× for world geometry; HUD and menus stay
single-sample. Auto retains the old sample ceiling (4 if supported, otherwise 1),
including Native's inclusive 64 MiB MSAA allowance. Explicit choices use the
largest supported count no greater than the request, checked for the actual world
format with `Adapter::get_texture_format_features`, including resolve support.
Native enables adapter-specific format features when available for 2×; WebGPU
may offer only 1/4. Explicit choices bypass the Auto sample-memory allowances while retaining
the dimension limits and 128 MiB base-attachment area guard. Their multisample
target can therefore use up to 256 MiB at 2× or 512 MiB at 4×. A supported explicit
count is not lowered merely to fit an authored MSAA budget; unsupported counts
fall back to the next supported count. This avoids moving the old memory cliff
to a slightly larger window. The requested radio remains
checked; Settings shows the effective count and a visible fallback message.
The title and presentation diagnostics report the actual submitted world count,
including Native 4K Auto at 1×. Unsupported Native dimensions retain the request
and use Original; AA is evaluated at that effective attachment extent.

World resolution defaults to Original. The Native/sharp toggle requests physical
world attachments with the same view area: “Sharper world; higher GPU use. Same
view area.” It applies at the next safely prepared picture, including after the
login closes, without changing the game's Quality state. With AA Auto, Native uses MSAA4 only while its world MSAA target fits 64 MiB;
larger sizes use single-sample physical coverage (including 4K). Original Auto
keeps its existing MSAA. Unsupported dimensions
leave Original effective and show “Native unavailable at this size”; the request
is retained and can become effective on a supported resize. See
[world resolution](world-resolution.md) for target ownership and costs.

`settings::SettingsPreferences` owns the bounded ASCII v3 record
`3:<auto|low|medium|high>:<original|native>:<auto|off|2|4>` (64-byte read/parse limit).
Valid v3 wins. Valid v2 (`2:<0|1>:<original|native>`) and quality-only v1
(`1:<0|1>`) migrate in memory: 1 becomes Auto, 0 becomes Low, missing resolution
becomes Original, and AA becomes Auto. Otherwise exactly legacy `"0"` means Low;
every other legacy value, including missing/unreadable, means Auto. Startup does
not write or repair malformed, oversized or future records. Appearance and
server-choice records remain separate. The High/Low value exposed to legacy
menu/browser callers is derived from the preset; it is not separately stored.

Native uses `%LOCALAPPDATA%/SlitherClient/settings.txt`, reads at most 65 bytes, and
shares the appearance adapter's same-directory temporary-write/atomic-replace
helper. A failed startup read falls back in memory without writing. A failed write
keeps the old file and in-memory choice and shows “Not saved; applies this session”.
Each explicit change writes canonical v3 and attempts persistence again, including after a startup read
failure. Native had no persisted legacy Quality
file. The new preset/AA selectors are desktop-only for now. Browser dev/reference
hosts retain their existing Quality UI and legacy key, migrating
through the same copied Wasm parser at `slither-client-settings-v1` and retaining
`slither-client-quality-v1` for older hosts. They perform no startup writes. The copied Wasm parser exposes the same
`graphics_quality`, `anti_aliasing`, `world_resolution` and canonical record fields.
Existing browser rendering still consumes its legacy High/Low projection; clicking
its legacy Quality control selects Auto/Low while retaining resolution and AA.

Preset and AA choices expose radio semantics and checked state; world resolution
retains checkbox semantics. Reselecting a checked radio schedules no write. The
visual and Tab orders match; matching
pointer release and Enter/Space carry an explicit next value into the same
generation-checked settings owner. The host tracks ordered held-key state before
egui processing, including egui-winit auto-repeat events marked `repeat: false`.
A held opener cannot activate the new screen; key releases and blur reset the
gate. Native synthetic key presses on refocus restore the held-key gate before
egui-winit suppresses those presses, so a still-held key cannot activate again.
Tab/Shift+Tab wrap
without an unfocused frame. Blur, resize, DPI changes and a new mount cancel
pointer capture. Nickname composition blocks entry; the modal releases gameplay
holds and disables nickname IME. Native accessibility bridging remains separate.

Pointer motion, modifier changes and wheel/touchpad bursts queue for the next
frame. Discrete actions run layout/action passes in callback order: batching Space, Tab, Space
into one pass would retarget the first activation to the final focus. Only the
final shapes are tessellated at paint, once per redraw. Pointer press/release
boundaries flush queued events first, including within one raw input batch, so
a scrollbar drag can start and apply motion before release ends it. The same
boundaries preserve wheel-before-click and click-before-wheel ordering. IME
Preedit/Commit still run immediate passes; Settings has no editable field, and
composition ordering/cost remains a validation task for editable-menu ports.

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
produces authored offscreen Settings PNGs at 960×540, 360×640, 1920×1080 and 3840×2160, plus
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

## Authored graphics controls and costs

`graphics_settings_perf NEW_EXTERNAL_DIRECTORY --control` renders the authored
fixture with 24 rivals, the own snake and 2,400 food with Auto quality and Auto AA on each DX12 hardware
adapter at 1080p and 4K, Original/Native, normal zoom and factor 0.065. No external
art, socket or window is used. The main baseline uses the same authored driver
against e066bc33; compare decoded RGBA bytes for every matching cell. This proves
those default controls, not arbitrary gameplay or physical-display acceptance.

Without `--control`, the driver measures each preset × AA choice in both world
modes and zoom levels: RTX at 4K and integrated GPU at 1080p. Five alternating
rounds per cell use 10 warmup and 40 measured pictures. CPU preparation and submit
are separate from GPU queue-span timestamps, waits and readbacks. The queue span
includes CPU supply gaps between presenter submissions; it is not gameplay FPS.
Record host CPU/GPU load, retain failed attempts, and acquire the lane timing lock
before running. Full CSVs/PNGs stay outside Git. Results and command details are in the
[authored validation report](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-05-graphics-settings/REPORT.md);
timings do not establish owner appearance or feel.
