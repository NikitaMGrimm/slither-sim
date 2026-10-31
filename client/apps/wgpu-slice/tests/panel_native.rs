#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
//! Authored pointer/capture regressions, without a window or clipboard.
use slither_client::{
    champion::MESSAGE,
    menu_input::{EditIntent, EditorId},
    session_policy::{Command, SessionPolicy},
};
use slither_wgpu_slice::{panel_native::NativePanel, text::TextCache};
fn panel() -> NativePanel {
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
    let mut p = NativePanel::new(&s.view());
    p.viewport = [960., 720.];
    p.policy.observe(1, &s.view());
    p.step(0., 1., &s.view()).unwrap();
    p.input.state.apply(EditIntent::KeyText("abcdef".into()));
    p
}
#[test]
fn outside_press_never_starts_a_selection_drag() {
    let mut p = panel();
    let mut text = TextCache::menu();
    let before = p.input.state.editor(EditorId::Champion).selection();
    p.pointer(&mut text, [0., 0.], Some(true)).unwrap();
    assert_eq!(p.controls.capture(), None);
    p.pointer(&mut text, [100., 100.], None).unwrap();
    assert_eq!(p.input.state.editor(EditorId::Champion).selection(), before);
}
#[test]
fn preedit_cannot_swallow_release_or_activate_a_control() {
    let mut p = panel();
    let mut text = TextCache::menu();
    let r = p.panel.resolve(p.viewport).unwrap()[2].rect;
    let point = [r.x + 12., r.y + r.height / 2.];
    p.pointer(&mut text, point, Some(true)).unwrap();
    assert_eq!(p.controls.capture(), Some(MESSAGE));
    p.input.state.apply(EditIntent::Preedit("x".into(), None));
    let before = p.input.state.editor(EditorId::Champion).selection();
    p.pointer(&mut text, [point[0] + 50., point[1]], Some(false))
        .unwrap();
    assert_eq!(p.controls.capture(), None);
    assert_eq!(p.input.state.editor(EditorId::Champion).selection(), before);
    assert!(p.take_submission().is_none());
    p.input.state.apply(EditIntent::ImeCommit("z".into()));
    let committed = p.input.state.editor(EditorId::Champion).selection();
    p.pointer(&mut text, [point[0] + 100., point[1]], None)
        .unwrap();
    assert_eq!(
        p.input.state.editor(EditorId::Champion).selection(),
        committed
    );
}

#[path = "support/announcement_tape.rs"]
mod tape;
#[test]
fn returned_champion_empty_full_and_selected_compose_at_high_dpr() {
    use slither_wgpu_slice::login_native::NativeLogin;
    let mut game = tape::playing();
    let mut login = NativeLogin::new("Player").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(8., true, false).unwrap();
    tape::pump(&mut game, 700.);
    login.observe_game(&game).unwrap();
    login.step(700., false, true).unwrap();
    game.receive(game.generation(), &[b'v', 1], 800.).unwrap();
    for now in (808..=6008).step_by(40) {
        game.receive(game.generation(), b"p", now as f64).unwrap();
        tape::pump(&mut game, now as f64);
        login.observe_game(&game).unwrap();
        login.step(now as f64, false, false).unwrap();
    }
    assert_eq!(login.champion.panel.alpha, 1.);
    assert!(!login.state.view(false).last_life.label.is_empty());
    for dpr in [4., 5.] {
        for n in [0, 140, 50] {
            login
                .champion
                .input
                .state
                .reset_editor(EditorId::Champion, &"W".repeat(n));
            if n == 50 {
                login.champion.input.state.apply(EditIntent::SelectAll);
            }
            let frame = login
                .prepare_host([(960. * dpr) as u32, (720. * dpr) as u32], dpr, false, None)
                .unwrap();
            let bytes: usize = frame.resources.iter().map(|r| r.image.pixels.len()).sum();
            assert!(bytes <= 4 * 1024 * 1024);
            assert!(frame.frame.draws.len() <= 512);
            println!("returned DPR {dpr}, {n} units: {bytes} bytes");
        }
    }
}
