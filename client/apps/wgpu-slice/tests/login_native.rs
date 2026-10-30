#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{
    menu_input::{EditIntent, HostEditEffect, TextField},
    menu_layout::MenuElement as E,
    menu_preferences::MenuQuality,
    start_page::{MenuPhase, StartMenuIntent},
};
use slither_wgpu_slice::login_native::NativeLogin;

#[test]
fn native_login_commit_focus_pointer_and_deferred_play() {
    let mut login = NativeLogin::new("Initial").unwrap();
    let (intents, effects) = login.step(0., true, false).unwrap();
    assert_eq!(intents, vec![StartMenuIntent::FocusNickname]);
    assert!(effects.contains(&HostEditEffect::ImeAllowed(true)));
    login.input.state.apply(EditIntent::SelectAll);
    login
        .input
        .state
        .apply(EditIntent::Preedit("日".into(), Some((0, 3))));
    assert_eq!(login.displayed_nickname(), "日");
    login.prepare([960, 540], true, None).unwrap();
    let r = login.layout.viewport_rect(E::Play);
    let point = [r.x + r.width / 2., r.y + r.height / 2.];
    login.pointer(point, Some(true), true);
    login.pointer(point, Some(false), true);
    assert_eq!(login.state.view(true).phase, MenuPhase::Login);
    let effects = login
        .input
        .state
        .apply(EditIntent::ImeCommit("A😀\u{7f}".into()));
    login
        .input
        .state
        .commit_menu(effects, &mut login.state, true);
    assert_eq!(login.state.view(true).nickname, "A  \u{7f}");
    login.pointer(point, Some(true), true);
    login.pointer(point, Some(false), true);
    login.activate(true);
    assert!(login.state.view(true).spinner_visible);
    let (effects, _) = login.step(16., true, false).unwrap();
    assert_eq!(
        effects,
        vec![StartMenuIntent::Play {
            nickname: "A  \u{7f}".into(),
            quality: MenuQuality::High
        }]
    );
    assert!(login.step(32., true, false).unwrap().0.is_empty());
    assert_eq!(login.state.view(true).phase, MenuPhase::Connecting);
    assert!(login.blocks_gameplay());
    login.step(48., true, true).unwrap();
    assert_eq!(login.state.view(true).phase, MenuPhase::Outgoing);
    assert!(!login.blocks_gameplay());
    assert!(!login.intercepts_pointer([0., 0.]));
    assert!(login.intercepts_pointer(point));
}

#[test]
fn tab_focus_releases_all_holds_and_quality_has_no_play_effect() {
    let mut login = NativeLogin::new("").unwrap();
    login.step(0., true, false).unwrap();
    for focus in [1, 2, 0] {
        let effects = login.focus(focus);
        assert!(effects.contains(&HostEditEffect::ReleasePointerBoost));
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, HostEditEffect::Gameplay { pressed: false, .. }))
                .count(),
            if focus == 0 { 8 } else { 4 }
        );
    }
    login.focus(2);
    login.activate(true);
    assert_eq!(login.state.view(true).quality, MenuQuality::Low);
    assert!(login.step(16., true, false).unwrap().0.is_empty());
    assert_eq!(login.input.state.focus(), None);
    login.focus(0);
    assert_eq!(login.input.state.focus(), Some(TextField::Nickname));
}
