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

#[test]
fn host_backing_drawn_controls_use_logical_pointer_and_ime_coordinates() {
    for (physical, dpr) in [([960, 540], 1.), ([1200, 675], 1.25), ([1920, 1080], 2.)] {
        for element in [E::Play, E::NicknameHolder, E::Quality] {
            let mut login = NativeLogin::new("Test").unwrap();
            login.step(0., true, false).unwrap();
            let logical = login
                .prepare(physical.map(|n| (f64::from(n) / dpr) as u32), true, None)
                .unwrap();
            let rect = login.layout.viewport_rect(element);
            let center = [rect.x + rect.width / 2., rect.y + rect.height / 2.];
            // Select a real painted quad, then project its centre exactly as the
            // host blits the backing into the physical window and routes input.
            let index = logical
                .frame
                .draws
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let distance = |d: &slither_wgpu_slice::frame::Draw| {
                        (d.destination.x + d.destination.width / 2. + d.translation[0] - center[0])
                            .powi(2)
                            + (d.destination.y + d.destination.height / 2. + d.translation[1]
                                - center[1])
                                .powi(2)
                    };
                    distance(a).total_cmp(&distance(b))
                })
                .unwrap()
                .0;
            let host = login.prepare_host(physical, dpr, true, None).unwrap();
            assert_eq!(host.frame.extent, [1500, 845]);
            let draw = &host.frame.draws[index];
            let point = [
                (draw.destination.x + draw.destination.width / 2. + draw.translation[0])
                    * f64::from(physical[0])
                    / f64::from(host.frame.extent[0])
                    / dpr,
                (draw.destination.y + draw.destination.height / 2. + draw.translation[1])
                    * f64::from(physical[1])
                    / f64::from(host.frame.extent[1])
                    / dpr,
            ];
            assert!(
                (point[0] - center[0]).abs() < 2.,
                "{element:?}: {point:?} {center:?}"
            );
            assert!((point[1] - center[1]).abs() < 2.);
            login.pointer(point, Some(true), true);
            login.pointer(point, Some(false), true);
            match element {
                E::Play => {
                    assert_eq!(
                        login
                            .step(16., true, false)
                            .unwrap()
                            .0
                            .iter()
                            .filter(|i| matches!(i, StartMenuIntent::Play { .. }))
                            .count(),
                        1
                    );
                    assert!(login.step(32., true, false).unwrap().0.is_empty());
                }
                E::NicknameHolder => {
                    assert_eq!(login.input.state.focus(), Some(TextField::Nickname));
                    let nick = login.layout.viewport_rect(E::Nickname);
                    let effects = login.cursor_area().unwrap();
                    assert!(effects.iter().any(|e| matches!(e,HostEditEffect::ImeCursorArea{position,..} if position[0]>=nick.x && position[0]<=nick.x+nick.width && position[1]==nick.y)));
                }
                E::Quality => assert_eq!(login.state.view(true).quality, MenuQuality::Low),
                _ => unreachable!(),
            }
        }
    }
}

#[test]
fn terminal_connecting_attempt_returns_without_automatic_retry() {
    use slither_client::session_policy::Phase;
    for phase in [Phase::Failed, Phase::Disconnected] {
        let mut login = NativeLogin::new("Retry").unwrap();
        login.step(0., true, false).unwrap();
        login.activate(true);
        assert!(!login.observe_session(phase)); // queued retry ignores old terminal phase
        assert!(matches!(
            login.step(16., true, false).unwrap().0.as_slice(),
            [StartMenuIntent::Play { .. }]
        ));
        assert!(login.observe_session(phase));
        assert!(!login.state.view(true).disabled);
        assert!(!login.state.view(true).spinner_visible);
        assert!(
            login
                .step(32., true, false)
                .unwrap()
                .0
                .iter()
                .all(|i| !matches!(i, StartMenuIntent::Play { .. }))
        );
        login.activate(true);
        assert!(matches!(
            login.step(48., true, false).unwrap().0.as_slice(),
            [StartMenuIntent::Play { .. }]
        ));
    }
}

#[test]
fn acquisition_cancel_and_pre_game_socket_outcomes_release_native_admission() {
    use slither_wgpu_slice::game::{EffectKind, LocalGame};
    for outcome in [
        "acquisition-failed",
        "acquisition-cancelled",
        "socket-closed",
        "socket-failed",
    ] {
        let mut login = NativeLogin::new("Retry").unwrap();
        let mut game = LocalGame::new(0.).unwrap();
        login.step(0., true, false).unwrap();
        login.activate(true);
        login.step(16., true, false).unwrap();
        if outcome.starts_with("socket") {
            game.play("ws://127.0.0.1:19491/slither", "Retry", 0)
                .unwrap();
            game.begin_frame(1., [960, 540], 1.).unwrap();
            while game.take_frame().is_some() {
                game.presented(true).unwrap();
            }
            let effect = game.effect().unwrap();
            assert!(matches!(effect.kind, EffectKind::Connect(_)));
            game.construct(effect.generation, effect.id, 1.).unwrap();
            game.acknowledge(effect.generation, effect.id, true)
                .unwrap();
            let stale_generation = effect.generation;
            if outcome == "socket-closed" {
                game.closed(stale_generation);
            } else {
                game.fail("Synthetic socket failure");
            }
        } else if outcome == "acquisition-failed" {
            game.fail("Synthetic acquisition failure");
        } else {
            game.stop();
        }
        while let Some(effect) = game.effect() {
            game.acknowledge(effect.generation, effect.id, true)
                .unwrap();
        }
        assert!(game.can_play(), "{outcome}");
        assert!(login.observe_session(game.phase()), "{outcome}");
        let (intents, _) = login.step(32., game.can_play(), false).unwrap();
        assert!(
            intents
                .iter()
                .all(|i| !matches!(i, StartMenuIntent::Play { .. }))
        );
        assert!(game.effect().is_none());
        assert!(!login.state.view(true).spinner_visible);
        login.activate(game.can_play());
        assert!(matches!(
            login.step(48., true, false).unwrap().0.as_slice(),
            [StartMenuIntent::Play { .. }]
        ));
    }
}

#[test]
fn empty_field_uses_placeholder_colour_and_shadow_free_caret() {
    let mut login = NativeLogin::new("").unwrap();
    login.step(0., true, false).unwrap();
    let picture = login.prepare([960, 540], true, None).unwrap();
    let nick = login.layout.viewport_rect(E::Nickname);
    let caret = picture
        .frame
        .draws
        .iter()
        .find(|draw| draw.destination.width == 1. && draw.destination.height == nick.height)
        .unwrap();
    assert_eq!(caret.destination.x, nick.x);
    assert_eq!(caret.destination.y, nick.y);
    let material = picture
        .resources
        .iter()
        .find(|r| r.asset.handle == caret.asset.handle)
        .unwrap();
    assert_eq!((material.image.width, material.image.height), (1, 1));
    assert_eq!(material.image.pixels, [224, 224, 255, 255]);
    // Text tint is baked into its padded raster, independently of caret material.
    assert!(picture.resources.iter().any(|r| {
        r.image
            .pixels
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[3] > 0 && p[..3] == [112, 112, 176])
    }));
}
