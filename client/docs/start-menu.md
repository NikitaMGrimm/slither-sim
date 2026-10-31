# Start menu

All three product entries open on the shared login. Native uses `NativeLogin`,
`MenuFrame` and the PR2 editor; Canvas and WebGPU use the same
`apps/wgpu-slice/web/login-overlay.mjs` semantic DOM adapter. `LoginMenu` exports
the copied core projection in both Wasm builds. Real input/button elements own
browser editing, IME, clipboard and accessibility; Rust owns normalization,
once-only Play, styles' state, transformed geometry and animation clocks.

`StartMenuState` emits a frozen Play request on the next admitted menu update. Each host
then uses its existing explicit route/profile/session admission. Initial native
CLI values seed the login; neither startup nor quality/skin interaction opens
a game socket. The first accepted gameplay update starts the outgoing fade;
the spinner remains while connecting. Focus changes release held turn/boost
inputs. Window blur and stale generations cannot admit a new attempt.

The Choose Server control is hidden. Change Skin is a labelled passive icon
until the chooser arrives. Server picker and chooser remain known gaps. The initial
final-length slot is blank.

Adapters consume the localized wording, separate last-length prefix/bold
number/regular punctuation, geometry, reveal/transition clocks, requested quality
and deferred Play/focus/link/storage intents. Measure the padded label again when
Play becomes Play Again. Nickname holder dimensions include the CSS border; its
input rectangle specifies the declared input content area. A DOM adapter must
normalize input UA padding/border so its content matches this rectangle.
The 50px Play bottom margin is trailing flow space, not part of its hit rectangle.

Supply normal last-length line-box height and nickname row baseline space from
the chosen platform font/flow implementation. The numeric tests use authored
18px/4px/52.2px metrics, not measured font parity. Native measures the selected
Lucida font box for the last-length line, the two NBSPs around the bold Arial
Play label, and the 20px gap directly after the nickname border box. Creation
and retained updates use the same measurements. The logo flow holder stays
450x115. Native nickname, Play, tips and font fallbacks use ascent/descent baselines;
tips use a normal line at the top of their reserved row. Nickname painting,
pointer selection and IME anchoring share the single-line input content box.
`menu_layout::single_line_field_placement(field, inset, font_size, ascent,
descent)` is shared by all single-line painters: it returns the content left
edge/alphabetic baseline and the centred font-size caret top/height. Insets
are symmetric horizontal/vertical content padding in logical units; callers
include borders only if their field rect still includes them. Values and
placeholders share this placement; caret and IME hosts snap its outputs once.
`tests/field_text.rs` independently checks line/caret centring at multiple font
sizes, input/champion-sized boxes and DPRs. The champion TextField painter can
call this same API without depending on the login renderer.
The caret uses a 530ms on/off clock from the menu
callback, restarted by focus/edit/movement; raw IME preedit stays separate.

Native local logo art keeps its aspect ratio and independent opacity, with a
plain fade. Without local logo art, or with desktop `--wordmark`, the authored
[stroke wordmark](wordmark.md) swells and ripples into place on the retained
reveal clock, then stops. Its natural letter height follows the same holder;
longer text extends horizontally and fits uniformly only at the viewport margin.
Authored tests and offscreen specimens establish these effects, not original
curve-motion or physical IME parity. Login scales about the center
of its viewport-sized border box, with its 64px margin outside that box. Fixed
corner links and the centered 128px spinner are siblings: they never inherit the
login transform. The spinner uses its own center-origin scale from the view.
`hit_test` only tests geometry; adapters must also check visibility and enabled
state before dispatching actions.

Initialize `MenuLayout::new` only at creation or an actual viewport resize.
Between resizes, call `update(view.transition, view.tips_visible, metrics)`;
this retains the last resize's top offset even when combined scale crosses 1.
The first completed outgoing fade permanently removes the tips row. A false
`tips_visible` removes its 30px flow space and makes its hit rectangle empty:
returning Play starts at y396 instead of y426 in the 1280x720 authored fixture.
Pass current admission availability to `play_pointer`; disabled Play rejects
new hover/press activation while still accepting leave/release cleanup.

Pass clock milliseconds relative to state creation. Logo/outgoing use elapsed/25;
the frame clock clamps elapsed/8 to 0..5 before damping. Button layers advance
once per callback. A Play action consumes the menu's click lock immediately;
the next admitted `step` returns one frozen nickname/quality request. It does not grant
route/profile authority. The existing admission owner must authorize execution.
Call `begin_outgoing` only after that request has been emitted and gameplay
accepted, at the same clock seam. Effects execute outside core borrows.

`accepted_game_over` accepts only committed Dead/Victory outcomes during a life,
with the command-time `HudFinal` capture. It never reads the current displayed
score. Duplicate outcomes and close-only phases cannot rewrite it. Missing
captures stay blank; no disconnect supplies a score. Last-life content survives
the next Play and disconnect; `EndScreenState` remains independent. Champion
outcomes hand off to the native [shared champion controls](menu-controls.md), with
ordinary controls disabled until acknowledgement or terminal transport ends the
editor, followed by its exit fade and the session's existing return.
Browser development pages retain the labelled pending handoff.
The menu follows the committed SessionPolicy hold/return fraction and retained
accepted outcome, including after a transport close. It owns no death timer.
The session closes/resets ordinary lives at return completion; each replay uses
the existing explicit admission and fresh native profile preparation. Final
length is an unbordered line with a 0.45-alpha prefix, real bold ungrouped
number and regular punctuation. Native restores nickname focus after the
completed picture commits.
During ordinary return, `play_disabled` is independent of `disabled`: the Play
button can accept one click while nickname editing/focus remain disabled. The
click freezes the stored nickname/quality and shows the connecting spinner,
but the old session continues to drive the return fraction, scale and opacity.
The request waits for both return completion and the existing host admission
(including terminal session, Close acknowledgement and committed picture).
Enter during return is accepted only after the old session is no longer connected;
a hidden death hold never accepts Play. Cancel clears a queued request without
restarting the return. Champion controls and result ownership are unchanged.
Authored regressions: `early_return_play_locks_but_waits_for_return_and_admission`
and `early_return_click_uses_animated_hit_rect_and_waits_for_close_ack`. Canvas and WebGPU remain working reference tools;
owner play and physical IME acceptance are separate.

`MenuQuality` retains high/low in memory and emits best-effort storage writes;
storage failure cannot undo a toggle. It has an independent key and does not
modify adaptive `QualityState` or migrate the existing appearance record.

Logo assets/curve settling, renderer styles, chooser/builder/server modal policy,
and browser champion editing remain later work.
Incoming/API skin 65 and the existing chooser are untouched; the later official
chooser owns the 0..64 inventory and 42 gate. Code redemption remains hidden.

## Opt-in text editing and native input

`menu_input::MenuInputState` owns a bounded `EditorId`/`TextRule` registry of
`TextEditState` values (including nickname and champion), directional UTF-8
byte selections, focus, modality, composition and bounded undo/redo (64 edits).
`EditIntent` and `HostEditEffect` contain copied data only. No DOM, window,
clipboard, transport or accessibility-library handle enters the shared core.
Movement and deletion use committed scalar boundaries; raw Unicode preedit,
including combining marks, remains IME-owned. Word movement uses whitespace boundaries; this is a
small native editing policy, not a locale-specific word/grapheme segmenter.
`byte_to_utf16` and `utf16_to_byte` reject offsets inside a UTF-8 scalar or UTF-16
surrogate pair rather than silently interpreting different coordinate systems.
Selection intents clamp invalid byte positions backwards to a scalar boundary.

Typing, paste and IME commit all replace the selection, normalize the complete
result and apply the field limit. Nicknames reuse #300's inclusive 32..127 UTF-16
unit rule and 24-unit cap: DEL survives and an emoji becomes two spaces, or one
space at unit 24 when the limit splits the pair. Champion editing uses the same
ASCII substitution with a 140-unit cap: each surrogate becomes a space, including unit 140 when the
limit splits a pair. This follows the private
`source/20b-skins-and-cosmetics.js:528-537` input handler; Save repeats substitution
and truncation at `source/06-login-and-hud.js:321-323`. The plan's earlier Unicode
inference was incorrect. Native Save uses the shared
[champion protocol boundary](menu-controls.md#victory-authority-and-send).

Raw Unicode preedit and its optional UTF-8 byte selection stay separate from
the committed value, field limit and undo history. The preview keeps at most
256 UTF-16 units on scalar boundaries, before copying/measuring/rasterizing;
its byte selection clamps to that prefix. Retaining this raw Unicode prefix until
commit approximates Chromium's input-event timing during composition, rather
than establishing timing parity. Empty preedit immediately clears the raw
preview, including winit's mandatory pre-Commit clear.
Disabled, explicit cancellation,
focus changes, modal open and blur discard uncommitted text. The OS owns Enter's
composition commit: the core waits for `Ime::Commit`, never inserts the preview
or activates Play/Save on that key. IME availability alone does not suppress
ordinary key text (important for hosts with long-lived Enabled notifications).
Windows text input follows **winit 0.30.13**: `src/window.rs:1260-1268` documents
that the window receives no `KeyboardInput` during preedit. Its
`src/platform_impl/windows/event_loop.rs:1526-1576` emits the composed result from
`WM_IME_COMPOSITION` and returns without `DefWindowProc`, so it generates no
second `WM_CHAR` text for that result. `keyboard_layout.rs:946` in the same
Windows directory maps `VK_PROCESSKEY` to `Key::Named(Process)`. This does not
imply an empty `KeyEvent.text`: Windows can provide a payload for Process, which
the adapter rejects regardless of content. These citations were checked against
the local registry source, not inferred from synthetic events.

Every `Ime::Commit` inserts and normalizes its text. `KeyEvent.text` inserts
while not composing, including ordinary typing and repeats with IME allowed.
Unexpected keyboard text during a non-empty preedit is ignored defensively:
the contract excludes it, and it must not overwrite the raw preview. There is
no string comparison for duplicate suppression or retrospective rollback.
`Process` events never insert text or trigger shortcuts; key releases still
clear held gameplay keys and the composition Enter state.

A composition Enter flag is armed only by an observed physical Enter press
consumed by IME: logical `Process`, or a key event delivered while composing.
Preedit, commit, candidate selection and cancellation never arm it themselves.
It clears on that physical key's release (including a release reported as
`Process`) or blur/focus teardown. Unrelated key presses and releases do not
release a held IME Enter. Normal activation requires a non-repeat logical Enter
press while not composing and without an owned IME Enter. No event-batch hook
controls this barrier. X11 and macOS input are unsupported
by this Windows rule; they need a platform-specific rule before support.
Real OS ordering still needs physical-host validation; synthetic traces are
not that acceptance.

The private `source/21-logo.js:303,339-347` keydown handler does not filter
repeat events. Its Play and Save callbacks reject disabled controls and disable
after acceptance (`source/06-login-and-hud.js:294-307,320-344`). This native seam
uses the explicit non-repeat activation rule; its event-level filtering differs
from the source dispatch, while menu admission still owns once-only execution.

Clipboard effects carry tokens. Focus, selection, editing and composition
changes invalidate pending work; failed cuts retain their selection and text,
and successful cuts enter undo history. The native `EditHost` capability is
injected in tests. Windows uses exactly pinned `clipboard-win` 5.4.3 with no
optional monitoring features: a small Unicode clipboard API with one mandatory
dependency (`error-code`), without an image clipboard, toolkit or Linux display
backend. Other native targets report clipboard unavailability. Resolution of an
initial 5.4.1 pin failed with `version 5.4.1 is yanked`; the pin was corrected
after querying Cargo, without bypassing the registry safeguard.

Focus gain, modal open and window blur emit releases for both turn keys, both
boost keys and pointer boost. Editing/modal state suspends gameplay presses;
key-up always passes. This opt-in seam does not change the legacy
`DesktopInput::blur` or its accumulated turn-debt semantics. Modal close restores
the previous text focus. `sync_menu` consumes #300's focus intents and current
hidden/disabled state. `apply_menu` (or `commit_menu` after adapter/clipboard
work) updates the committed nickname and dispatches Enter through the existing
admission gate; the once-only Play request still comes from the next menu step.

`login_accessibility` returns stable copied nodes in reading order, with role,
plain name, committed value, effective focus, enabled state and semantic actions.
It covers the ordinary login, last-life status and busy/incomplete champion
status. Actions must return through the menu/editor admission policy. It adds
no AccessKit bridge or fake functioning champion editor.
Skin, server and graphics-quality names and high/low values come from the shared
`MenuWording` table in English, German, French and Portuguese. These accessible
names/values are authored: the source image links at `site/index.html:288-301`
and `347-350` provide neither alt nor title text. They are not claimed as official
translations. Each locale has a projection test covering both quality states.
Connecting and incomplete champion-handoff status names are also authored in
the same table and covered in each locale's projection test.

The native adapter maps winit 0.30 logical editing keys, physical gameplay keys,
modifiers (including Windows AltGr), IME and focus events. `NativeLogin` supplies
measured caret/candidate geometry and draws committed selection/raw preedit.
Tab/Shift+Tab routes nickname, Play and quality; pointer input uses core hit
rectangles. A Play pointer gesture during preedit cannot cancel it and submit
the older name. Commit first, then activate Play. The host executes IME and
clipboard effects outside core borrows. No UI toolkit or accessibility bridge
is added; physical-host IME and screen-reader acceptance remains open.

The authored traces in `tests/menu_input.rs` and
`apps/wgpu-slice/tests/menu_input_native.rs` exercise offsets, normalization,
limits, selection/history, Windows IME/Enter and key-release traces, injected
clipboard success/failure, gameplay releases and login semantic snapshots.
winit's `KeyEvent` has a private platform payload, so synthetic key events inject
the exact public fields production extracts through `KeyInput`; no fabricated
platform payload or unsafe constructor is used. IME and focus tests use actual
`WindowEvent` values. No test opens a window, real clipboard, IME or server.
Real OS candidate placement, IME/keyboard interaction and screen readers remain
unverified. Native accessibility bridging remains later work; browser inputs are real DOM controls.

## Label width

The width metric is the padded label's untransformed, zoom-adjusted CSS layout
width. Add 35 CSS px of button padding directly, retaining fractional widths.
The former integer `offsetWidth` emulation changes width by at most 0.5 CSS px;
transformed control edges stay within the [perceptual budget](perceptual-parity.md).
Font shaping remains the metric provider's responsibility.
The source-rationale table below retains the historical integer checkpoints;
the label-width test now compares them with tolerances instead of emulating Blink.
Their historical Blink pin is `4dfbf1c0f8e59669a8d578f02c258a759d5c0b70`.

## Authored numeric fixtures and source rationale

The private readable source snapshot is a read-only behavioral specification.
Paths below are relative to that snapshot; these citations paraphrase rules,
and do not import source code, assets or curve data. Tests are in
[`tests/start_page.rs`](../tests/start_page.rs). Rows map every source numeric
constant used by this slice to independently authored checkpoints.

| Source constants / rule | Private source citation | Test / authored checkpoint |
| --- | --- | --- |
| Login margin 64; logo 450x115, top 50; last slot top gap 44; tips 360x30 | `site/index.html:232-249` | `layout_border_boxes_and_measured_flow`: logo (415,114), result y273, tips y291 at 1280x720 |
| Nickname content holder 244x43, border 2; input 220x24 at left12/top10; nickname top gap34 | `site/index.html:21-65,248-249` | `layout_border_boxes_and_measured_flow`: outer 248x47 at (516,355), input (530,367) |
| Play height47, top20/bottom50; integer offsetWidth then +35 | `source/06-login-and-hud.js:264-273`; `source/03-buttons.js:441-442,491-492`; pinned Blink above | `layout_border_boxes_and_measured_flow`: width87/y426, remeasured width125/y431; `round1_label_width_rounds_offset_width_before_adding_padding` |
| Skin left20/bottom16, 95x89; server right20/bottom16, 107x89; quality right20/top16, 90x87 | `site/index.html:288-301,347-350` | `layout_border_boxes_and_measured_flow`: corners (20,615), (1153,615), (1170,16) |
| Height threshold560, floor50; combined scale and top rounded to 1e-5; scale multiplier 1+0.1p² | `source/16-eyes-and-layout.js:194-206`; `source/06-login-and-hud.js:70-72` | `resize_small_height_rounding_and_transform_origin`: heights559/280/40, scale0.99821/0.5/0.08929; `inverse_hit_test_round_trips` |
| Top changes only on resize, including the scale1 branch; fades retain it | `source/16-eyes-and-layout.js:199-206`; `source/06-login-and-hud.js:70-73`; `source/13-update-frame.js:574-581` | `round1_animation_retains_resize_top_when_scale_crosses_one`: height540/p0.60858 gives scale1, retained top-19.28571 |
| Tips row removed after first outgoing transition, never restored on return | `source/06-login-and-hud.js:52-53`; `source/13-update-frame.js:522-605` | `round1_return_removes_tips_from_flow_and_hits`: returning Play y396, removed row cannot hit, second life stays hidden |
| Logo elapsed/25, alpha +0.05, reveal +0.00375, nickname progress +0.006; nickname min(1,6p), skin clamp(5(p-0.05)), Play clamp(5(p-0.01)) | `source/21-logo.js:137-184,292-302` | `logo_clock_checkpoints_are_not_clamped_or_lagged`: 25/250/10000ms, animated versus initially complete |
| Tips initial phase1.9/index-1; phase +0.017delta, wrap2π; alpha sqrt((1-cos phase)/2), round1e-5 | `source/06-login-and-hud.js:273-293`; `source/13-update-frame.js:495-508` | `tip_cycle_clamp_lag_and_rounding`: first tip at step52, alpha0.01841, full three-tip cycle; initial blank/alpha0 |
| Frame elapsed/8, clamp0..5; lag *=0.85 floor0.2, recovery +0.05 cap1 | `source/13-update-frame.js:15-52` | `tip_cycle_clamp_lag_and_rounding`: 8000ms clamps to5 then4.25, alpha0.83389, negative elapsed0; `strict_return_boundary_lag_floor_recovery_and_focus` |
| Hover/press +0.33 or -0.2 per callback; disabled layers0 and activation rejected; releases accepted | `source/03-buttons.js:271-318,320-348,355-408` | `overlays_are_per_callback_and_disabled_layers_are_zero`: 0.33/0.66/0.99/1/0.8; `round1_disabled_pointer_rejects_activation_but_accepts_releases` |
| Click-once; nickname/Play disabled; request on subsequent update | `source/06-login-and-hud.js:294-307`; `source/13-update-frame.js:70-73` | `click_once_deferred_admission_focus_and_links`: one request, composed Enter ignored, explicit focus/link intents |
| Spinner128, viewport center offset64; unlagged opacity/86, scale-progress/93; scale round(0.1+1e5*0.9p(1+2(1-alpha)²))/1e5 | `source/06-login-and-hud.js:118-126`; `source/13-update-frame.js:105-169`; `source/16-eyes-and-layout.js:178-179` | `layout_border_boxes_and_measured_flow`: (576,296,128,128); `spinner_uses_unlagged_clamped_clock_and_fades_out`: first alpha5/86, scale0.13424, completed scale0.9/fade |
| Outgoing elapsed/25 *0.05 cap1; login1-p, worldp | `source/06-login-and-hud.js:10-94` | `outgoing_elapsed_clock_and_rounding`: 250ms gives0.5, scale0.98839 at960x540; `early_death_does_not_stop_the_independent_outgoing_clock` |
| Return strictly >1600ms; progress -0.004delta; Play extra alpha1-0.5clamp(6p); focus at0 | `source/13-update-frame.js:522-618` | `strict_return_boundary_lag_floor_recovery_and_focus`: 1600 hold, 1601 return, p0.9995, laggedp0.9825, exactly one focus |
| Normalize UTF-16 units inclusive32..127; truncate24 | `source/20a-game-over-and-codes.js:48-71`; `source/20b-skins-and-cosmetics.js:513-521`; `source/19-websocket-and-protocol.js:2091-2105` | `nickname_utf16_del_surrogates_and_limit`: DEL retained, surrogate pair two spaces, unit24 splits a pair |
| Final prefix alpha0.45; ungrouped number; ! strictly >1000; replay labels in four locales | `source/20a-game-over-and-codes.js:3-43` | `lengths_1000_and_1001_and_translated_replay`; `persistent_last_life_second_life_and_disconnect_has_no_result_authority` |
| All four locale placeholders, Play labels and three ordered tips | `source/06-login-and-hud.js:254-293` | `translations_and_source_order`: exact plain text for en/de/fr/pt |
| Quality high unless stored "0"; low/high persisted as "0"/"1" | `source/20a-game-over-and-codes.js:99-123` | `quality_memory_survives_missing_storage_and_snapshots_on_play`: absent/invalid reads, ignored failed write, frozen Play choice |

The plan's phrase “rounded base scale” is imprecise: source retains the base ratio
and rounds the combined scale. The plan's general per-step delta wording does
not apply to button overlays or the separate logo/outgoing timer. Spinner scale
also depends on the source's rounding grouping: it completes at 0.9. Tests
preserve these distinctions. Font metrics and actual raster/art parity remain
unverified; synthetic numeric coverage does not establish visual acceptance.
