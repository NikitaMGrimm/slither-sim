//! Authored synthetic panel/editor/policy regressions. No host or transport.
use slither_client::{
    champion::*,
    menu_controls::*,
    menu_input::*,
    menu_layout::MenuRect,
    menu_theme::MenuTheme,
    protocol::encode_champion,
    session_policy::{Command, Phase, SessionPolicy},
};

fn victory() -> SessionPolicy {
    let mut s = SessionPolicy::new();
    s.prepare_controls(
        &[
            Command {
                opcode: b'a',
                argument: 0,
            },
            Command {
                opcode: b'v',
                argument: 1,
            },
        ],
        0.,
    )
    .unwrap();
    s.commit_controls().unwrap();
    s
}
fn panel() -> PanelView {
    let s = victory();
    let mut c = ChampionState::default();
    assert!(c.observe(3, &s.view()));
    c.step(0., 1.).unwrap();
    c.panel(&s.view(), [960., 720.])
}
fn center(r: MenuRect) -> [f64; 2] {
    [r.x + r.width / 2., r.y + r.height / 2.]
}

#[test]
fn placements_inverse_half_open_clips_and_reverse_order() {
    let mut p = panel();
    p.placement.scale = 1.5;
    let r = p.resolve([960., 720.]).unwrap();
    let field = &r[2];
    assert_eq!(field.rect.width, 444. * 1.5);
    assert_eq!(field.local(center(field.rect)), [222., 24.]);
    assert!(field.hit(center(field.rect)));
    assert!(!field.hit([field.rect.x + field.rect.width, field.rect.y]));
    p.controls[2].clip = Some(MenuRect {
        x: 18.,
        y: 88.,
        width: 20.,
        height: 48.,
    });
    let r = p.resolve([960., 720.]).unwrap();
    assert!(!r[2].hit(center(r[2].rect)));
    p.controls[3].placement = p.controls[2].placement;
    p.controls[2].clip = None;
    let r = p.resolve([960., 720.]).unwrap();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p, &mut edit);
    input.pointer(&p, &r, center(r[2].rect), Some(true));
    assert_eq!(input.capture(), Some(SAVE));
    p.controls[3].id = MESSAGE;
    assert!(p.resolve([960., 720.]).is_err());
    p.controls.pop();
    p.placement.scale = f64::INFINITY;
    assert!(p.resolve([960., 720.]).is_err());
}

#[test]
fn capture_requires_matching_release_and_never_retargets_tab_or_generation() {
    let mut p = panel();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p, &mut edit);
    let r = p.resolve([960., 720.]).unwrap();
    let save = center(r[3].rect);
    let field = center(r[2].rect);
    input.pointer(&p, &r, save, Some(true));
    assert!(input.pointer(&p, &r, field, Some(false)).is_none());
    input.pointer(&p, &r, save, Some(true));
    input.tab(&p, true);
    assert_eq!(
        input.pointer(&p, &r, save, Some(false)).unwrap().control,
        SAVE
    );
    input.pointer(&p, &r, save, Some(true));
    p.generation += 1;
    input.sync(&p, &mut edit);
    assert!(input.pointer(&p, &r, save, Some(false)).is_none());
    input.pointer(&p, &r, save, Some(true));
    p.controls.pop();
    input.sync(&p, &mut edit);
    assert_eq!(input.capture(), None);
    input.blur();
    assert_eq!(input.capture(), None);
}

#[test]
fn tab_order_visibility_alpha_damping_and_editor_sync() {
    let mut p = panel();
    p.alpha = 0.;
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    let effects = input.sync(&p, &mut edit);
    assert!(effects.contains(&HostEditEffect::ReleasePointerBoost));
    assert_eq!(input.focus(), Some(MESSAGE));
    input.tab(&p, false);
    assert_eq!(input.focus(), Some(SAVE));
    input.tab(&p, false);
    assert_eq!(input.focus(), Some(MESSAGE));
    edit.apply(EditIntent::Preedit("日".into(), None));
    input.sync(&p, &mut edit);
    assert!(edit.editor(EditorId::Champion).composing());
    p.controls[3].enabled = false;
    input.tab(&p, true);
    assert_eq!(input.focus(), Some(MESSAGE));
    let r = p.resolve([960., 720.]).unwrap();
    let point = center(r[2].rect);
    input.step(&p, 0., MenuTheme::default()).unwrap();
    input.pointer(&p, &r, point, Some(true));
    input.step(&p, 8., MenuTheme::default()).unwrap();
    assert_eq!(input.progress(MESSAGE), [0.33, 0.33]);
    assert!(input.step(&p, 7., MenuTheme::default()).is_err());
    p.visible = false;
    input.sync(&p, &mut edit);
    assert_eq!(edit.focus(), None);
    assert_eq!(input.capture(), None);
}

#[test]
fn registry_preserves_ime_undo_clipboard_tokens_and_numeric_rule() {
    let id = EditorId(7);
    let mut e = MenuInputState::default();
    assert!(e.register(id, TextRule::NumericServer, "12a3"));
    e.apply(EditIntent::Focus(Some(id)));
    assert_eq!(e.editor(id).text(), "123");
    e.apply(EditIntent::KeyText("456".into()));
    assert_eq!(e.editor(id).text(), "1234");
    e.apply(EditIntent::Undo);
    assert_eq!(e.editor(id).text(), "123");
    let effects = e.apply(EditIntent::Paste);
    let HostEditEffect::ReadClipboard(token) = effects[0] else {
        panic!()
    };
    e.apply(EditIntent::Focus(Some(EditorId::Champion)));
    e.apply(EditIntent::ClipboardRead {
        token,
        result: Ok("stale".into()),
    });
    assert_eq!(e.editor(EditorId::Champion).text(), "");
    e.apply(EditIntent::ImeEnterPressed);
    e.apply(EditIntent::Preedit("日".into(), None));
    e.register(EditorId::Champion, TextRule::Champion, "ignored");
    e.apply(EditIntent::ImeCommit("😀".into()));
    assert_eq!(e.editor(EditorId::Champion).text(), "  ");
    assert!(
        e.apply(EditIntent::Enter {
            pressed: true,
            repeat: false
        })
        .is_empty()
    );
    e.apply(EditIntent::Enter {
        pressed: false,
        repeat: false,
    });
    assert_eq!(
        e.apply(EditIntent::Enter {
            pressed: true,
            repeat: false
        }),
        vec![HostEditEffect::Activate(EditorId::Champion)]
    );
}

#[test]
fn exact_wire_units_truncation_and_isolated_surrogates() {
    for n in [139, 140, 141] {
        assert_eq!(
            encode_champion(&vec![65; n]),
            [vec![255, 118], vec![65; n.min(140)]].concat()
        );
    }
    assert_eq!(
        encode_champion(&[0, 31, 32, 126, 127, 128, 0xd800, 0xdc00, 65]),
        [255, 118, 32, 32, 32, 126, 127, 32, 32, 32, 65]
    );
    let mut units = vec![65; 139];
    units.extend([0xd83d, 0xde00, 66]);
    let bytes = encode_champion(&units);
    assert_eq!(bytes.len(), 142);
    assert_eq!(bytes[141], 32);
}

#[test]
fn committed_victory_only_and_committed_ack_drives_editor_exit() {
    let mut s = SessionPolicy::new();
    let mut c = ChampionState::default();
    assert!(!c.observe(1, &s.view()));
    assert!(!c.visible());
    s.prepare_controls(
        &[
            Command {
                opcode: b'a',
                argument: 0,
            },
            Command {
                opcode: b'v',
                argument: 1,
            },
        ],
        0.,
    )
    .unwrap();
    assert!(!c.observe(1, &s.view()));
    s.commit_controls().unwrap();
    assert_eq!(s.view().outcome, Some(Phase::Victory));
    assert!(c.observe(1, &s.view()));
    assert!(s.can_send_champion());
    c.step(0., 1.).unwrap();
    assert!(c.panel(&s.view(), [960., 720.]).enabled);
    s.champion_enqueued();
    assert!(!s.can_send_champion());
    assert!(!c.panel(&s.view(), [960., 720.]).enabled);
    s.prepare_controls(
        &[Command {
            opcode: b'v',
            argument: 2,
        }],
        8.,
    )
    .unwrap();
    assert!(!s.view().champion_acknowledged);
    s.commit_controls().unwrap();
    assert!(s.view().champion_acknowledged);
    c.observe(1, &s.view());
    for now in (8..=408).step_by(8) {
        c.step(f64::from(now), 1.).unwrap();
    }
    assert!(c.finished());
    assert!(!c.visible());
    assert!(!c.observe(1, &s.view()));
    let mut death = SessionPolicy::new();
    death
        .prepare_controls(
            &[
                Command {
                    opcode: b'a',
                    argument: 0,
                },
                Command {
                    opcode: b'v',
                    argument: 0,
                },
                Command {
                    opcode: b'v',
                    argument: 2,
                },
            ],
            0.,
        )
        .unwrap();
    death.commit_controls().unwrap();
    assert!(!death.view().champion_acknowledged);
    assert!(!ChampionState::default().observe(2, &death.view()));
}

#[test]
fn modal_restores_surviving_parent_focus_and_semantics_exclude_raw_preedit() {
    let mut parent = panel();
    parent.modal = false;
    parent.id = 10;
    let mut modal = panel();
    modal.id = 11;
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&parent, &mut edit);
    input.tab(&parent, false);
    assert_eq!(input.focus(), Some(SAVE));
    input.sync(&modal, &mut edit);
    assert_eq!(input.focus(), Some(MESSAGE));
    edit.apply(EditIntent::KeyText("Value".into()));
    edit.apply(EditIntent::Preedit("日".into(), None));
    let nodes = modal.semantics([960., 720.], &input, &edit).unwrap();
    let field = &nodes[2];
    assert!(field.focused);
    assert_eq!(field.value, "Value");
    assert_eq!(field.actions, vec![PanelAction::Focus, PanelAction::Edit]);
    input.sync(&parent, &mut edit);
    assert_eq!(input.focus(), Some(SAVE));
    assert_eq!(edit.focus(), None);
    input.sync(&modal, &mut edit);
    parent.controls[3].visible = false;
    input.sync(&parent, &mut edit);
    assert_eq!(input.focus(), None);
}

#[test]
fn focused_generic_modal_field_uses_the_shared_caret_clock() {
    let p = panel();
    assert!(p.modal);
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&p, &mut edit);
    edit.step_caret(0.);
    assert!(edit.caret_visible());
    edit.step_caret(530.);
    input.sync(&p, &mut edit);
    assert!(!edit.caret_visible());
    edit.apply(EditIntent::Preedit("日".into(), None));
    assert!(edit.caret_visible());
    edit.step_caret(1000.);
    assert!(edit.caret_visible());
}

// Pointer hits below are byte offsets supplied by the platform's painted geometry.
fn mouse_fixture() -> (PanelView, PanelInputState, MenuInputState) {
    let panel = panel();
    let mut controls = PanelInputState::default();
    let mut edit = MenuInputState::default();
    controls.sync(&panel, &mut edit);
    edit.reset_editor(EditorId::Champion, "one two, three");
    (panel, controls, edit)
}
#[allow(clippy::too_many_arguments)]
fn gesture(
    p: &PanelView,
    c: &mut PanelInputState,
    e: &mut MenuInputState,
    hit: usize,
    down: Option<bool>,
    shift: bool,
    now: f64,
    delta: [f64; 2],
) {
    let r = p.resolve([960., 720.]).unwrap();
    let point = [r[2].rect.x + 20. + delta[0], r[2].rect.y + 20. + delta[1]];
    c.pointer(p, &r, point, down);
    c.select_text(
        e,
        EditorId::Champion,
        TextHit {
            caret: hit,
            character: hit,
        },
        point,
        down == Some(true),
        shift,
        now,
        ClickSettings::default(),
    );
}
#[test]
fn mouse_drag_and_shift_click_keep_the_anchor_and_replace_selection() {
    for replacement in [
        EditIntent::KeyText("X".into()),
        EditIntent::Delete,
        EditIntent::Backspace,
    ] {
        let (p, mut c, mut e) = mouse_fixture();
        gesture(&p, &mut c, &mut e, 4, Some(true), false, 0., [0.; 2]);
        gesture(&p, &mut c, &mut e, 7, None, false, 10., [12., 0.]);
        assert_eq!(e.editor(EditorId::Champion).selection(), (4, 7));
        gesture(&p, &mut c, &mut e, 7, Some(false), false, 20., [12., 0.]);
        gesture(&p, &mut c, &mut e, 0, None, false, 30., [0.; 2]);
        assert_eq!(
            e.editor(EditorId::Champion).selection(),
            (4, 7),
            "release ends dragging"
        );
        e.apply(replacement.clone());
        assert_eq!(
            e.editor(EditorId::Champion).text(),
            if matches!(replacement, EditIntent::KeyText(_)) {
                "one X, three"
            } else {
                "one , three"
            }
        );
    }
    let (p, mut c, mut e) = mouse_fixture();
    e.apply(EditIntent::Select {
        anchor: 7,
        caret: 7,
    });
    gesture(&p, &mut c, &mut e, 4, Some(true), true, 0., [0.; 2]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (7, 4));
    gesture(&p, &mut c, &mut e, 0, None, true, 10., [-10., 0.]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (7, 0));
    gesture(&p, &mut c, &mut e, 12, None, true, 20., [20., 0.]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (7, 12));
}
#[test]
fn double_word_and_triple_all_obey_time_distance_and_control_identity() {
    for (time, delta, multi) in [
        (500., [2., 2.], true),
        (501., [0.; 2], false),
        (100., [2.01, 0.], false),
        (100., [0., 2.01], false),
        (-1., [0.; 2], false),
    ] {
        let (p, mut c, mut e) = mouse_fixture();
        gesture(&p, &mut c, &mut e, 5, Some(true), false, 0., [0.; 2]);
        gesture(&p, &mut c, &mut e, 5, Some(false), false, 1., [0.; 2]);
        gesture(&p, &mut c, &mut e, 5, Some(true), false, time, delta);
        assert_eq!(
            e.editor(EditorId::Champion).selection(),
            if multi { (4, 7) } else { (5, 5) }
        );
        gesture(&p, &mut c, &mut e, 5, Some(false), false, time + 1., delta);
        gesture(&p, &mut c, &mut e, 5, Some(true), false, time + 20., delta);
        assert_eq!(
            e.editor(EditorId::Champion).selection(),
            if multi { (0, 14) } else { (4, 7) }
        );
    }
    let (mut p, mut c, mut e) = mouse_fixture();
    gesture(&p, &mut c, &mut e, 5, Some(true), false, 0., [0.; 2]);
    gesture(&p, &mut c, &mut e, 5, Some(false), false, 1., [0.; 2]);
    p.generation += 1;
    c.sync(&p, &mut e);
    gesture(&p, &mut c, &mut e, 5, Some(true), false, 20., [0.; 2]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (5, 5));
}
#[test]
fn mouse_word_classes_clamping_and_preedit_are_bounded() {
    for (hit, expected) in [
        (3, (3, 4)),
        (7, (7, 8)),
        (14, (9, 14)),
        (usize::MAX, (9, 14)),
    ] {
        let (p, mut c, mut e) = mouse_fixture();
        for time in [0., 10.] {
            gesture(&p, &mut c, &mut e, hit, Some(true), false, time, [0.; 2]);
            gesture(
                &p,
                &mut c,
                &mut e,
                hit,
                Some(false),
                false,
                time + 1.,
                [0.; 2],
            );
        }
        assert_eq!(e.editor(EditorId::Champion).selection(), expected);
    }
    let (p, mut c, mut e) = mouse_fixture();
    gesture(&p, &mut c, &mut e, 4, Some(true), false, 0., [0.; 2]);
    gesture(&p, &mut c, &mut e, usize::MAX, None, false, 1., [0.; 2]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (4, 14));
    e.apply(EditIntent::Preedit("composing".into(), None));
    gesture(&p, &mut c, &mut e, 0, None, false, 2., [0.; 2]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (4, 14));
    gesture(&p, &mut c, &mut e, 0, Some(false), false, 3., [0.; 2]);
    assert_eq!(c.capture(), None);
    e.apply(EditIntent::ImeCommit("X".into()));
    gesture(&p, &mut c, &mut e, 0, None, false, 4., [0.; 2]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (5, 5));
}

#[test]
fn ordinary_panels_keep_blink_and_cancel_click_chains_on_other_targets() {
    let (mut p, mut c, mut e) = mouse_fixture();
    p.modal = false;
    c.sync(&p, &mut e);
    e.step_caret(0.);
    assert!(e.caret_visible());
    for now in [100., 300., 530.] {
        e.step_caret(now);
        c.sync(&p, &mut e);
    }
    assert!(
        !e.caret_visible(),
        "equal nonmodal sync must not restart blink"
    );
    gesture(&p, &mut c, &mut e, 5, Some(true), false, 1000., [0.; 2]);
    gesture(&p, &mut c, &mut e, 5, Some(false), false, 1001., [0.; 2]);
    let r = p.resolve([960., 720.]).unwrap();
    c.pointer(&p, &r, center(r[3].rect), Some(true));
    c.pointer(&p, &r, center(r[3].rect), Some(false));
    gesture(&p, &mut c, &mut e, 5, Some(true), false, 1020., [0.; 2]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (5, 5));
}

#[test]
fn double_click_drag_preserves_the_word_until_crossing_its_boundary() {
    let (p, mut c, mut e) = mouse_fixture();
    gesture(&p, &mut c, &mut e, 5, Some(true), false, 0., [0.; 2]);
    gesture(&p, &mut c, &mut e, 5, Some(false), false, 1., [0.; 2]);
    gesture(&p, &mut c, &mut e, 5, Some(true), false, 20., [0.; 2]);
    gesture(&p, &mut c, &mut e, 5, None, false, 21., [0.; 2]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (4, 7));
    gesture(&p, &mut c, &mut e, 11, None, false, 30., [20., 0.]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (4, 14));
    gesture(&p, &mut c, &mut e, 1, None, false, 40., [-10., 0.]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (7, 0));
}

#[test]
fn word_selection_uses_character_while_single_click_uses_nearest_caret() {
    let (p, mut c, mut e) = mouse_fixture();
    e.reset_editor(EditorId::Champion, "one_12 two");
    gesture(&p, &mut c, &mut e, 6, Some(true), false, 0., [0.; 2]);
    assert_eq!(e.editor(EditorId::Champion).selection(), (6, 6));
    gesture(&p, &mut c, &mut e, 6, Some(false), false, 1., [0.; 2]);
    let resolved = p.resolve([960., 720.]).unwrap();
    let point = [resolved[2].rect.x + 20., resolved[2].rect.y + 20.];
    c.pointer(&p, &resolved, point, Some(true));
    c.select_text(
        &mut e,
        EditorId::Champion,
        TextHit {
            caret: 6,
            character: 5,
        },
        point,
        true,
        false,
        20.,
        ClickSettings::default(),
    );
    assert_eq!(e.editor(EditorId::Champion).selection(), (0, 6));
}

#[test]
fn pointer_focus_is_silent_and_keyboard_navigation_exposes_the_target() {
    let panel = panel();
    let resolved = panel.resolve([960., 720.]).unwrap();
    let target = resolved.iter().find(|r| r.control.id == SAVE).unwrap();
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&panel, &mut edit);
    input.tab(&panel, false);
    assert_eq!(input.visible_focus(), input.focus());
    input.pointer(&panel, &resolved, center(target.rect), Some(true));
    input.pointer(&panel, &resolved, center(target.rect), Some(false));
    assert_eq!(input.focus(), Some(SAVE));
    assert!(input.visible_focus().is_none());
    input.pointer(&panel, &resolved, center(target.rect), None);
    assert!(input.visible_focus().is_none());
    input.tab(&panel, false);
    assert_eq!(input.visible_focus(), input.focus());
}
