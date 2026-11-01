//! Authored synthetic v13 sessions through native/browser's production LocalGame.
#[path = "support/announcement_tape.rs"]
mod host;
#[path = "../../../tests/support/protocol13.rs"]
mod wire;
use slither_client::{presentation::ScoreStatus, session_policy::Phase};
use slither_wgpu_slice::game::{EffectKind, LocalGame};
use wire::*;

#[test]
fn stock_login_then_setup13_drives_world_hud_death_and_picture_acknowledgements() {
    let mut g = LocalGame::new(0.).unwrap();
    g.play("ws://127.0.0.1:19491/slither", "Player", 3).unwrap();
    host::pump(&mut g, 1.);
    let e = g.effect().unwrap();
    assert!(matches!(e.kind, EffectKind::Connect(_)));
    g.construct(e.generation, e.id, 1.).unwrap();
    g.acknowledge(e.generation, e.id, true).unwrap();
    g.opened(g.generation(), 2.).unwrap();
    assert_eq!(host::effects(&mut g), [vec![1], vec![99, 0]]);
    g.receive(g.generation(), &challenge(), 3.).unwrap();
    let sent = host::effects(&mut g);
    assert_eq!(sent[0], b"DHLPTXBFJNRVZDHLPTXBFJNR");
    let mut login = vec![b's', 30];
    login.extend([0; 22]);
    login.extend([3, 6]);
    login.extend(b"Player");
    login.extend([0, 255]);
    assert_eq!(sent[1], login);
    assert_eq!(g.phase(), Phase::Setup);
    g.receive(
        g.generation(),
        &bundle(&[
            setup(),
            snake(0),
            snake(300),
            food(b'F', 7, 1100, 1000, 20),
            food(b'f', 8, 1101, 1000, 25),
            vec![b'M', 0, 2, 15],
            board(),
            vec![b'k', 0, 0, 0, 0, 1],
            vec![b'S', 0, 2, 0, 0, 0, 3],
            vec![b'R', 0],
            // Authored record announcement: shared segments/fraction/name/text.
            vec![
                b'm', 0, 0, 12, 0, 0, 0, 3, b'S', b'Y', b'N', b'R', b'E', b'C',
            ],
        ]),
        4.,
    )
    .unwrap();
    host::pictures(&mut g);
    host::pump(&mut g, 20.);
    assert_eq!(g.phase(), Phase::Playing);
    assert_eq!(g.client().received().snakes.len(), 2);
    assert_eq!(g.client().received().foods.len(), 2);
    assert!(g.client().minimap_view().toggles.iter().any(|&c| c != 0));
    assert!(g.hud_text().contains("9"), "{}", g.hud_text());
    let ScoreStatus::Ready(before) = g.client().score_view().status else {
        panic!()
    };
    g.receive(
        g.generation(),
        &bundle(&[
            anchor(b'g', 0, &[4, 76, 3, 232], None),
            anchor(b'N', 0, &[170, 128], Some(0x800000)),
            anchor(b'G', 300, &[130, 129], None),
            vec![b'E', 0, 0, 64, 96],
            eaten(1100, 1000, Some(0)),
        ]),
        30.,
    )
    .unwrap();
    host::pictures(&mut g);
    host::pump(&mut g, 40.);
    assert_eq!(g.client().received().foods.len(), 1);
    let ScoreStatus::Ready(after) = g.client().score_view().status else {
        panic!()
    };
    assert!(after > before);
    g.receive(
        g.generation(),
        &bundle(&[vec![b's', 1, 44, 1], vec![b's', 0, 0, 1], vec![b'v', 0]]),
        50.,
    )
    .unwrap();
    host::pictures(&mut g);
    host::effects(&mut g);
    assert_eq!(g.phase(), Phase::Dead);
    assert!(g.client().received().snakes.is_empty());
    assert_eq!(g.client().received().own_snake_id, Some(0));
    assert!(g.failure.is_none());
}

#[test]
fn same_protocol_setup_boundaries_preserve_world_and_reject_a_version_switch_atomically() {
    let mut g = host::negotiating();
    g.receive(
        g.generation(),
        &bundle(&[setup(), snake(7), setup(), snake(300)]),
        4.,
    )
    .unwrap();
    host::pictures(&mut g);
    assert_eq!(g.client().received().setup.as_ref().unwrap().protocol, 13);
    assert_eq!(g.client().received().snakes.len(), 2);
    assert_eq!(g.phase(), Phase::Playing);
    let before = g.client().received().clone();
    let mut changed = setup();
    changed[23] = 19;
    assert!(
        g.receive(
            g.generation(),
            &bundle(&[food(b'b', 9, 1, 2, 3), changed]),
            5.
        )
        .is_err()
    );
    assert_eq!(g.client().received(), &before);
}

#[test]
fn v13_pong_clears_the_pending_ping_and_the_next_due_ping_is_sent() {
    let mut g = host::negotiating();
    g.receive(g.generation(), &bundle(&[setup(), snake(0)]), 4.)
        .unwrap();
    host::pictures(&mut g);
    assert_eq!(g.client().received().setup.as_ref().unwrap().protocol, 13);
    let first = host::pump(&mut g, 300.);
    assert_eq!(first.iter().filter(|b| b.as_slice() == [251]).count(), 1);
    assert!(g.champion_session().ping_pending);
    assert!(
        host::pump(&mut g, 1200.)
            .iter()
            .all(|b| b.as_slice() != [251])
    );
    g.receive(g.generation(), b"p", 1210.).unwrap();
    host::pictures(&mut g);
    assert!(!g.champion_session().ping_pending);
    assert!(!g.champion_session().lagging);
    let resumed = host::pump(&mut g, 1250.);
    assert_eq!(resumed.iter().filter(|b| b.as_slice() == [251]).count(), 1);
    assert!(g.champion_session().ping_pending);
    g.receive(g.generation(), b"p", 1260.).unwrap();
    host::pictures(&mut g);
    assert!(!g.champion_session().ping_pending);
    assert!(
        host::pump(&mut g, 1500.)
            .iter()
            .all(|b| b.as_slice() != [251])
    );
    assert_eq!(
        host::pump(&mut g, 1501.)
            .iter()
            .filter(|b| b.as_slice() == [251])
            .count(),
        1
    );
}
