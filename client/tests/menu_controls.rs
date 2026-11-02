//! Authored synthetic panel/editor/policy regressions. No host or transport.
use slither_client::{
    champion::*,
    menu_controls::*,
    menu_layout::MenuRect,
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
fn placements_half_open_clips_and_reverse_order() {
    let mut p = panel();
    p.placement.scale = 1.5;
    let r = p.resolve([960., 720.]).unwrap();
    let field = &r[2];
    assert_eq!(field.rect.width, 444. * 1.5);
    p.controls[2].clip = Some(MenuRect {
        x: 18.,
        y: 88.,
        width: 20.,
        height: 48.,
    });
    let r = p.resolve([960., 720.]).unwrap();
    let [x, y] = center(r[2].rect);
    assert!(!r[2].clip.contains(x, y));
    p.controls[3].id = MESSAGE;
    assert!(p.resolve([960., 720.]).is_err());
    p.controls.pop();
    p.placement.scale = f64::INFINITY;
    assert!(p.resolve([960., 720.]).is_err());
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
