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
| Play height47, top20/bottom50; width ceil(padded label+35) | `source/06-login-and-hud.js:264-273`; `source/03-buttons.js:416-451` | `layout_border_boxes_and_measured_flow`: width88/y426, remeasured width126/y431; bottom margin documented as trailing space |
| Skin left20/bottom16, 95x89; server right20/bottom16, 107x89; quality right20/top16, 90x87 | `site/index.html:288-301,347-350` | `layout_border_boxes_and_measured_flow`: corners (20,615), (1153,615), (1170,16) |
| Height threshold560, floor50; combined scale and top rounded to 1e-5; scale multiplier 1+0.1p² | `source/16-eyes-and-layout.js:194-206`; `source/06-login-and-hud.js:70-72` | `resize_small_height_rounding_and_transform_origin`: heights559/280/40, scale0.99821/0.5/0.08929; `inverse_hit_test_round_trips` |
| Logo elapsed/25, alpha +0.05, reveal +0.00375, nickname progress +0.006; nickname min(1,6p), skin clamp(5(p-0.05)), Play clamp(5(p-0.01)) | `source/21-logo.js:137-184,292-302` | `logo_clock_checkpoints_are_not_clamped_or_lagged`: 25/250/10000ms, animated versus initially complete |
| Tips initial phase1.9/index-1; phase +0.017delta, wrap2π; alpha sqrt((1-cos phase)/2), round1e-5 | `source/06-login-and-hud.js:273-293`; `source/13-update-frame.js:495-508` | `tip_cycle_clamp_lag_and_rounding`: first tip at step52, alpha0.01841, full three-tip cycle; initial blank/alpha0 |
| Frame elapsed/8, clamp0..5; lag *=0.85 floor0.2, recovery +0.05 cap1 | `source/13-update-frame.js:15-52` | `tip_cycle_clamp_lag_and_rounding`: 8000ms clamps to5 then4.25, alpha0.83389, negative elapsed0; `strict_return_boundary_lag_floor_recovery_and_focus` |
| Hover/press +0.33 or -0.2 per callback; disabled rendered layers0 | `source/03-buttons.js:355-408` | `overlays_are_per_callback_and_disabled_layers_are_zero`: 0.33/0.66/0.99/1/0.8 |
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
