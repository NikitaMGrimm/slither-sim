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
