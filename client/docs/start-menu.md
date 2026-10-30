# Opt-in start menu core

`start_page::StartMenuState` publishes copied `StartMenuView` values and
`StartMenuIntent` effects. `MenuLayout` takes logical viewport pixels and measured
text/inline-flow metrics; no world backing size or DPR enters its geometry.
Existing Canvas `project_start_page`, preferences, bindings and adapters are
unchanged. No new Wasm binding is needed until an adapter consumes this API.

Adapters later consume the localized wording, separate last-length prefix/bold
number/regular punctuation, geometry, reveal/transition clocks, requested quality
and deferred Play/focus/link/storage intents. Measure the padded label again when
Play becomes Play Again. Nickname holder dimensions include the CSS border; its
input rectangle specifies the declared input content area. A DOM adapter must
normalize input UA padding/border so its content matches this rectangle.
The 50px Play bottom margin is trailing flow space, not part of its hit rectangle.

Supply normal last-length line-box height and nickname row baseline space from
the chosen platform font/flow implementation. The numeric tests use authored
18px/4px/52.2px metrics, not measured font parity. Login scales about the center
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
the next `step` returns one frozen nickname/quality request. It does not grant
route/profile authority. The existing admission owner must authorize execution.
Call `begin_outgoing` only after that request has been emitted and gameplay
accepted, at the same clock seam. Effects execute outside core borrows.

`accepted_game_over` accepts only committed Dead/Victory outcomes during a life,
with the command-time `HudFinal` capture. It never reads the current displayed
score. Duplicate outcomes and close-only phases cannot rewrite it. Missing
captures stay blank; no disconnect supplies a score. Last-life content survives
the next Play and disconnect; `EndScreenState` remains independent. Champion
outcomes expose an explicitly incomplete handoff with ordinary controls disabled.
PR10 must connect these projection clocks to committed SessionPolicy lifecycle
and fence transport/reset effects there; this opt-in policy executes none itself.

`MenuQuality` retains high/low in memory and emits best-effort storage writes;
storage failure cannot undo a toggle. It has an independent key and does not
modify adaptive `QualityState` or migrate the existing appearance record.

Logo assets/curve settling, renderer styles, chooser/builder/server modal policy,
champion editing/acknowledgement and all adapter activation remain later work.
Incoming/API skin 65 and the existing chooser are untouched; the later official
chooser owns the 0..64 inventory and 42 gate. Code redemption remains hidden.

## Opt-in text editing and native input

`menu_input::MenuInputState` owns two `TextEditState` values, directional UTF-8
byte selections, focus, modality, composition and bounded undo/redo (64 edits).
`EditIntent` and `HostEditEffect` contain copied data only. No DOM, window,
clipboard, transport or accessibility-library handle enters the shared core.
Movement and deletion use Unicode scalar boundaries, including independently
editable combining marks. Word movement uses whitespace boundaries; this is a
small native editing policy, not a locale-specific word/grapheme segmenter.
`byte_to_utf16` and `utf16_to_byte` reject offsets inside a UTF-8 scalar or UTF-16
surrogate pair rather than silently interpreting different coordinate systems.
Selection intents clamp invalid byte positions backwards to a scalar boundary.

Typing, paste and IME commit all replace the selection, normalize the complete
result and apply the field limit. Nicknames reuse #300's inclusive 32..127 UTF-16
unit rule and 24-unit cap: DEL survives and an emoji becomes two spaces, even at
the last available unit. Champion editing preserves Unicode and caps at 140
UTF-16 units, stopping before a split surrogate pair. **This is a requested
deviation:** the private `source/20b-skins-and-cosmetics.js:528-536` handler also
applies ASCII substitution to champion input, and `06-login-and-hud.js:321-340`
does so on Save. The plan's Unicode inference must not be treated as an official
input or wire rule. This PR adds no champion submission or wire encoding.

Raw Unicode preedit and its optional UTF-8 byte selection stay separate from
the committed value, field limit and undo history. Empty preedit is retained
across winit's mandatory pre-Commit clear. An empty preview without an adjacent
commit cancels at the dispatch boundary (also on hosts with long-lived Enabled).
Disabled, explicit cancellation,
focus changes, modal open and blur discard uncommitted text. The OS owns Enter's
composition commit: the core waits for `Ime::Commit`, never inserts the preview
or activates Play/Save on that key. IME availability alone does not suppress
ordinary key text (important for hosts with long-lived Enabled notifications).
Text during composition is ignored; an identical trailing `KeyEvent.text` and
Enter are suppressed for the commit's dispatch batch. Direct commits without
preedit also deduplicate earlier identical key text in that batch, restoring
the prior selection/history before applying the one committed edit. The native event loop
ends that batch in `about_to_wait`. A consumed Enter stays held until key-up,
so repeats cannot activate after the batch ends. Real OS ordering still needs
physical-host validation; synthetic traces are not that acceptance.

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

The native adapter maps winit 0.30 logical editing keys, physical gameplay keys,
modifiers (including Windows AltGr), IME and focus events. The host executes
IME admission, normal purpose, logical candidate rectangles and clipboard work
outside core borrows. `main.rs` has an optional hook initialized to `None`:
there is no visible menu, window startup change, enabled editor or new draw.
The later compositor must initialize/synchronize the menu and editor, supply
the measured caret rectangle and clock/focus/admission intents, and consume the
existing deferred Play effects. Browsers will use real DOM inputs separately.

The authored traces in `tests/menu_input.rs` and
`apps/wgpu-slice/tests/menu_input_native.rs` exercise offsets, normalization,
limits, selection/history, IME lifecycle and both Enter/commit orders, injected
clipboard success/failure, gameplay releases and login semantic snapshots.
winit's `KeyEvent` has a private platform payload, so synthetic key events inject
the exact public fields production extracts through `KeyInput`; no fabricated
platform payload or unsafe constructor is used. IME and focus tests use actual
`WindowEvent` values. No test opens a window, real clipboard, IME or server.
Real OS candidate placement, IME/keyboard interaction and screen readers remain
unverified. Native accessibility bridging and browser inputs are later PRs.

## Label width

The width metric is the padded label's untransformed, zoom-adjusted CSS layout
width, not an ink or transformed bounding box. Before adding 35px, convert it to
the integer `offsetWidth` that the source reads at button creation and relabeling
(`source/03-buttons.js:441-442,491-492`). The source's subsequent ceiling is then
redundant. Authored widths 52.2, 52.49, 52.5 and 52.51 yield button widths
87, 87, 88 and 88; 90.01 yields 125, not 126.

Blink is pinned to commit `4dfbf1c0f8e59669a8d578f02c258a759d5c0b70`:
[`HTMLElement::offsetWidthForBinding` and `AdjustedOffsetForZoom`, lines4458-4462/4503-4511](https://chromium.googlesource.com/chromium/src/+/4dfbf1c0f8e59669a8d578f02c258a759d5c0b70/third_party/blink/renderer/core/html/html_element.cc#4458)
read the layout width, adjust for zoom and round it.
[`LayoutUnit::Round`, lines294-297](https://chromium.googlesource.com/chromium/src/+/4dfbf1c0f8e59669a8d578f02c258a759d5c0b70/third_party/blink/renderer/platform/geometry/layout_unit.h#294)
rounds to nearest with positive half ties upward. This binding path neither
floors the width nor snaps its two edges. Native metrics must already represent
CSS layout units after zoom adjustment; browser adapters may pass the already
integer `offsetWidth` directly. Font shaping and layout quantization remain the
metric provider's responsibility. No browser execution is claimed by this audit.

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
