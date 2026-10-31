#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{
    menu_input::{EditIntent, HostEditEffect, TextField},
    menu_layout::MenuElement as E,
    menu_preferences::MenuQuality,
    start_page::{MenuPhase, StartMenuIntent},
};
use slither_wgpu_slice::login_native::NativeLogin;

#[test]
fn shared_field_placement_keeps_placeholder_value_preedit_and_ime_together() {
    for dpr in [1., 1.25, 1.5, 1.75, 2.] {
        for value in ["", "Player", "WWWWWWWWWWWWWWWWWWWWWWWW"] {
            let mut login = NativeLogin::new(value).unwrap();
            login.step(0., true, false).unwrap();
            login.step(16., true, false).unwrap();
            for preedit in [false, true] {
                if preedit {
                    login
                        .input
                        .state
                        .apply(EditIntent::Preedit("é".into(), Some((0, 2))));
                }
                let picture = login
                    .prepare_host([(960. * dpr) as u32, (540. * dpr) as u32], dpr, true, None)
                    .unwrap();
                let caret = picture
                    .frame
                    .draws
                    .iter()
                    .find(|d| {
                        d.destination.height > 5.
                            && picture.resources.iter().any(|r| {
                                r.asset == d.asset
                                    && r.image
                                        .pixels
                                        .as_chunks::<4>()
                                        .0
                                        .iter()
                                        .all(|p| *p == [224, 224, 255, 255])
                            })
                    })
                    .unwrap();
                assert!(login.cursor_area().unwrap().iter().any(|e| matches!(e,
                    HostEditEffect::ImeCursorArea { position, size }
                    if *position == [caret.destination.x, caret.destination.y]
                    && *size == [caret.destination.width, caret.destination.height]
                )));
            }
        }
    }
}

#[test]
fn selected_font_flow_matches_initial_and_retained_login() {
    for (extent, dpr) in [([1920, 1080], 1.), ([960, 540], 1.), ([1920, 1080], 2.)] {
        let mut login = NativeLogin::new("Player").unwrap();
        login.step(9600., true, false).unwrap();
        login.prepare_host(extent, dpr, true, None).unwrap();
        let first = login.layout.clone();
        login.prepare_host(extent, dpr, true, None).unwrap();
        assert_eq!(login.layout, first);
        let nick = first.viewport_rect(E::NicknameHolder);
        let play = first.viewport_rect(E::Play);
        assert_eq!(first.logical_rect(E::Logo).height, 115.);
        assert!((first.logical_rect(E::NicknameHolder).y + 64. - 360.).abs() < 0.01);
        assert!((first.logical_rect(E::Play).y + 64. - 427.).abs() < 0.01);
        assert!((play.width / first.scale - 87.).abs() < 0.5);
        assert!((play.y - nick.y - 67. * first.scale).abs() < 0.01);
    }
}

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
fn tab_focus_releases_all_holds_and_settings_has_no_play_effect() {
    let mut login = NativeLogin::new("").unwrap();
    login.step(0., true, false).unwrap();
    for focus in [
        slither_client::login_controls::PLAY,
        slither_client::login_controls::SETTINGS,
        slither_client::login_controls::NICKNAME,
    ] {
        let effects = login.focus(focus);
        assert!(effects.contains(&HostEditEffect::ReleasePointerBoost));
        assert_eq!(
            effects
                .iter()
                .filter(|e| matches!(e, HostEditEffect::Gameplay { pressed: false, .. }))
                .count(),
            4
        );
    }
    login.focus(slither_client::login_controls::SETTINGS);
    login.activate(true);
    assert!(login.state.settings.visible());
    assert_eq!(login.state.view(true).quality, MenuQuality::High);
    assert!(login.step(16., true, false).unwrap().0.is_empty());
    login.close_settings(true);
    assert_eq!(login.input.state.focus(), None);
    for frame in 2..256 {
        let (intents, _) = login.step(f64::from(frame) * 16., true, false).unwrap();
        assert!(
            !intents
                .iter()
                .any(|i| matches!(i, StartMenuIntent::Play { .. }))
        );
        if !login.state.settings.visible() {
            break;
        }
        assert_eq!(login.input.state.focus(), None);
    }
    assert!(!login.state.settings.visible());
    assert_eq!(
        login.controls.focus(),
        Some(slither_client::login_controls::SETTINGS)
    );
    login.focus(slither_client::login_controls::NICKNAME);
    assert_eq!(login.input.state.focus(), Some(TextField::Nickname));
}

#[test]
fn window_overlay_controls_use_logical_pointer_and_ime_coordinates() {
    for (physical, dpr) in [
        ([960, 540], 1.),
        ([1200, 675], 1.25),
        ([1920, 1080], 2.),
        ([3840, 2160], 1.5),
        ([5760, 3240], 6.),
        ([6720, 3780], 7.),
        ([7680, 4320], 8.),
    ] {
        for element in [E::Play, E::NicknameHolder, E::Settings] {
            let mut login = NativeLogin::new("Test").unwrap();
            for now in (0..=9600).step_by(16) {
                login.step(now as f64, true, false).unwrap();
            }
            login
                .prepare(physical.map(|n| (f64::from(n) / dpr) as u32), true, None)
                .unwrap();
            let rect = login.layout.viewport_rect(element);
            let center = [
                rect.x + rect.width / 2.,
                rect.y
                    + if element == E::Settings {
                        46.
                    } else {
                        rect.height / 2.
                    },
            ];
            let host = login.prepare_host(physical, dpr, true, None).unwrap();
            assert_eq!(host.frame.extent, physical);
            assert_eq!(host.frame.viewport_scale, dpr);
            // Slicing changes quad topology with DPR. Verify the physical target
            // lies in an actual painted tile, then route its logical inverse.
            let physical_point = center.map(|n| (n * dpr).round());
            let point = physical_point.map(|n| n / host.frame.viewport_scale);
            assert!(
                host.frame.draws.iter().any(|d| {
                    let x = d.destination.x + d.translation[0];
                    let y = d.destination.y + d.translation[1];
                    point[0] >= x
                        && point[0] < x + d.destination.width
                        && point[1] >= y
                        && point[1] < y + d.destination.height
                }),
                "{element:?} at DPR {dpr}: {point:?}"
            );
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
                            .step(9616., true, false)
                            .unwrap()
                            .0
                            .iter()
                            .filter(|i| matches!(i, StartMenuIntent::Play { .. }))
                            .count(),
                        1
                    );
                    assert!(login.step(9632., true, false).unwrap().0.is_empty());
                }
                E::NicknameHolder => {
                    assert_eq!(login.input.state.focus(), Some(TextField::Nickname));
                    let nick = login.layout.viewport_rect(E::Nickname);
                    let effects = login.cursor_area().unwrap();
                    assert!(effects.iter().any(|e| matches!(e,HostEditEffect::ImeCursorArea{position,..} if position[0]>=nick.x - 0.5/dpr && position[0]<=nick.x+nick.width && position[1]>=nick.y - 0.5/dpr && position[1]<nick.y+nick.height)));
                    let painted = login.prepare_host(physical, dpr, true, None).unwrap();
                    let caret = painted
                        .frame
                        .draws
                        .iter()
                        .find(|d| {
                            painted.resources.iter().any(|r| {
                                r.asset == d.asset
                                    && r.image
                                        .pixels
                                        .as_chunks::<4>()
                                        .0
                                        .iter()
                                        .all(|p| *p == [224, 224, 255, 255])
                            })
                        })
                        .unwrap();
                    assert!(
                        effects.iter().any(
                            |e| matches!(e, HostEditEffect::ImeCursorArea{position,size}
                        if *position == [caret.destination.x,caret.destination.y]
                        && *size == [caret.destination.width,caret.destination.height])
                        ),
                        "DPR {dpr}: IME {effects:?}, caret {:?}",
                        caret.destination
                    );
                }
                E::Settings => assert!(login.state.settings.visible()),
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
    login.step(16., true, false).unwrap();
    let picture = login.prepare([960, 540], true, None).unwrap();
    let nick = login.layout.viewport_rect(E::Nickname);
    let caret = picture
        .frame
        .draws
        .iter()
        .find(|draw| {
            draw.destination.width == 1.
                && draw.destination.height < nick.height
                && draw.destination.height > 10.
        })
        .unwrap();
    assert_eq!(
        caret.destination.x,
        (nick.x + 2. * login.layout.scale).round()
    );
    assert!(caret.destination.y > nick.y);
    let material = picture
        .resources
        .iter()
        .find(|r| r.asset.handle == caret.asset.handle)
        .unwrap();
    assert_eq!(
        (material.image.width, material.image.height),
        (1, caret.destination.height.round() as u32)
    );
    assert_eq!(
        material.image.pixels,
        [224, 224, 255, 255].repeat(material.image.height as usize)
    );
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

#[test]
fn physical_menu_reuses_rasters_and_bounds_controls_at_hidpi() {
    for dpr in [1., 1.5, 2.] {
        let mut login = NativeLogin::new("Player").unwrap();
        login.step(0., true, false).unwrap();
        login.step(16., true, false).unwrap();
        let first = login.prepare_host([3840, 2160], dpr, true, None).unwrap();
        let second = login.prepare_host([3840, 2160], dpr, true, None).unwrap();
        let resized = login.prepare_host([3600, 2025], dpr, true, None).unwrap();
        let assets = |p: &slither_wgpu_slice::menu::MenuFrame| {
            p.resources.iter().map(|r| r.asset).collect::<Vec<_>>()
        };
        assert_eq!(assets(&first), assets(&second));
        assert_eq!(assets(&first), assets(&resized));
        assert!(
            first
                .resources
                .iter()
                .all(|r| r.asset.width <= 2048 && r.asset.height <= 2048)
        );
        assert!(
            first
                .resources
                .iter()
                .map(|r| r.image.pixels.len())
                .sum::<usize>()
                < slither_wgpu_slice::text::MENU_CACHE_BYTES
        );
        if dpr == 2. {
            // A 2x field/shadow still exceeds the old 1.5x density, while
            // bounded tiles replace the former full-width material.
            assert!(first.resources.iter().any(|r| r.asset.handle >> 60 == 7
                && r.asset.height > 200
                && r.asset.width <= 512));
        }
    }
}

#[path = "support/announcement_tape.rs"]
mod endpoint_tape;

#[test]
fn early_return_click_uses_animated_hit_rect_and_waits_for_close_ack() {
    use endpoint_tape::{bundle, pump};
    use slither_wgpu_slice::{endpoint_login, game::EffectKind};
    let mut game = endpoint_tape::playing();
    let mut login = NativeLogin::new("Stored").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(16., true, false).unwrap();
    pump(&mut game, 600.);
    login.observe_game(&game).unwrap();
    login.step(600., false, false).unwrap();
    game.receive(game.generation(), &bundle(&[vec![b'v', 0]]), 700.)
        .unwrap();
    login.observe_game(&game).unwrap();
    login.prepare_host([1920, 1080], 1., false, None).unwrap();
    let hold = login.layout.viewport_rect(E::Play);
    let hold_point = [hold.x + hold.width / 2., hold.y + hold.height / 2.];
    login.pointer(hold_point, Some(true), false);
    login.pointer(hold_point, Some(false), false);
    assert_eq!(login.state.view(false).phase, MenuPhase::DeathHold);
    let generation = game.generation();
    let mut clicked = false;
    let mut close_checked = false;
    let mut previous = 1.;
    for frame in 0..=150 {
        let now = 2301. + f64::from(frame) * 16.;
        game.receive(generation, &bundle(&[vec![b'p']]), now)
            .unwrap();
        game.begin_frame(now, [960, 540], 1.).unwrap();
        login.observe_game(&game).unwrap();
        let view = login.state.view(game.can_play());
        assert_eq!(view.transition, game.login_session().return_fraction);
        assert!(view.transition <= previous);
        previous = view.transition;
        assert!(login.step(now, false, false).unwrap().0.is_empty());
        if !clicked && view.phase == MenuPhase::Returning && view.transition < 0.8 {
            assert!(view.disabled && !view.play_disabled);
            assert_eq!(login.input.state.focus(), None);
            login.input.state.apply_menu(
                EditIntent::KeyText("Changed".into()),
                &mut login.state,
                false,
            );
            assert_eq!(login.state.view(false).nickname, "Stored");
            login.prepare_host([1920, 1080], 1., false, None).unwrap();
            let r = login.layout.viewport_rect(E::Play);
            let point = [r.x + r.width - 0.1, r.y + r.height / 2.];
            let mut settled = login.layout.clone();
            settled.scale = settled.base_scale;
            assert!(!settled.hit_test(E::Play, point[0], point[1]));
            assert!(login.layout.hit_test(E::Play, point[0], point[1]));
            for _ in 0..2 {
                login.pointer(point, Some(true), false);
                login.pointer(point, Some(false), false);
            }
            let queued = login.state.view(false);
            assert_eq!(queued.phase, MenuPhase::Connecting);
            assert!(queued.disabled && queued.play_disabled && queued.spinner_visible);
            assert_eq!(queued.transition, view.transition);
            clicked = true;
        }
        while let Some(effect) = game.effect() {
            assert!(!matches!(effect.kind, EffectKind::Connect(_)));
            if matches!(effect.kind, EffectKind::Close) {
                assert!(clicked && !game.can_play());
                assert!(
                    login
                        .step(now, game.can_play(), false)
                        .unwrap()
                        .0
                        .is_empty()
                );
                close_checked = true;
            }
            game.acknowledge(effect.generation, effect.id, true)
                .unwrap();
        }
        login.observe_game(&game).unwrap();
        if let Some(_picture) = game.take_frame() {
            assert!(!game.can_play());
            assert!(
                login
                    .step(now, game.can_play(), false)
                    .unwrap()
                    .0
                    .is_empty()
            );
            game.presented(true).unwrap();
        }
        if close_checked && game.can_play() {
            assert_eq!(game.generation(), generation);
            assert_eq!(login.state.view(false).transition, 0.);
            let intents = login.step(now + 1., true, false).unwrap().0;
            assert!(
                matches!(intents.as_slice(), [StartMenuIntent::Play { nickname, .. }] if nickname == "Stored")
            );
            assert!(login.step(now + 2., true, false).unwrap().0.is_empty());
            endpoint_login::play(&mut game, "ws://127.0.0.1:19491/slither", "Stored", 0).unwrap();
            pump(&mut game, now + 3.);
            let connect = game.effect().unwrap();
            assert!(matches!(connect.kind, EffectKind::Connect(_)));
            game.construct(connect.generation, connect.id, now + 3.)
                .unwrap();
            game.acknowledge(connect.generation, connect.id, true)
                .unwrap();
            assert!(game.effect().is_none());
            assert_eq!(game.generation(), generation + 1);
            return;
        }
    }
    panic!("return must finish and dispatch exactly one admitted replay");
}

#[test]
fn accepted_death_uses_receipt_score_and_session_return_then_replays_once() {
    use endpoint_tape::{bundle, effects, pump};
    use slither_client::{presentation::ScoreTables, start_page::MenuAction};
    use slither_wgpu_slice::endpoint_login;
    let mut game = endpoint_tape::playing();
    game.window_viewport([1920, 1080], 2.).unwrap();
    let mut login = NativeLogin::new("Again").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(16., true, false).unwrap();
    pump(&mut game, 600.);
    login.observe_game(&game).unwrap();
    login.step(600., false, false).unwrap();
    let own = game.client().score_view().own.unwrap();
    // Received credit before v belongs to the final capture; the later R in
    // the same envelope must not change the last-length line.
    let expected = ScoreTables::new(game.client().score_view().table_max.unwrap())
        .unwrap()
        .score(own.logical_segments + 3, own.fractional_mass)
        .unwrap();
    game.receive(
        game.generation(),
        &bundle(&[vec![b'R', 3], vec![b'v', 0], vec![b'R', 7]]),
        700.,
    )
    .unwrap();
    login.observe_game(&game).unwrap();
    let captured = login.state.view(false).last_life;
    assert_eq!(captured.number, format!("{expected:.0}"));
    assert!(!login.state.view(false).visible);
    game.receive(game.generation(), &bundle(&[vec![b'p']]), 2300.)
        .unwrap();
    pump(&mut game, 2300.);
    login.observe_game(&game).unwrap();
    assert_eq!(login.state.view(true).phase, MenuPhase::DeathHold);
    for _ in 0..2 {
        login.state.action(MenuAction::Play, true);
    }
    game.begin_frame(2301., [960, 540], 1.).unwrap();
    effects(&mut game);
    login.observe_game(&game).unwrap();
    let midpoint = login.state.view(true);
    assert!((midpoint.transition - 0.9995).abs() < 1e-10);
    assert!(midpoint.disabled);
    // An unacknowledged picture retries without advancing return or focus.
    let picture = game.take_frame().unwrap();
    let identity = picture.frame.identity;
    let mut composed = picture.clone();
    let menu = login.prepare_host([1920, 1080], 2., false, None).unwrap();
    let menu_draws = menu.frame.draws.clone();
    menu.append_to(&mut composed).unwrap();
    let overlay = composed.overlay.as_ref().unwrap();
    assert_eq!(overlay.extent, [1920, 1080]);
    assert_eq!(overlay.viewport_scale, 2.);
    assert!(overlay.draws.ends_with(&menu_draws));
    let world_draws = composed.frame.draws.len();
    assert_eq!(
        world_draws,
        picture.frame.draws.len() + 1,
        "only the backdrop enters the world"
    );
    game.retry_frame(picture).unwrap();
    game.begin_frame(2310., [960, 540], 1.).unwrap();
    login.observe_game(&game).unwrap();
    assert_eq!(login.state.view(true), midpoint);
    assert_eq!(game.take_frame().unwrap().frame.identity, identity);
    game.presented(true).unwrap();
    let mut focus = 0;
    let mut completion_retried = false;
    for frame in 1..=130 {
        let now = 2301. + f64::from(frame) * 16.;
        if game.login_session().outcome.is_some() && game.login_session().death_pending {
            game.receive(game.generation(), &bundle(&[vec![b'p']]), now)
                .unwrap();
        }
        game.begin_frame(now, [960, 540], 1.).unwrap();
        effects(&mut game);
        login.observe_game(&game).unwrap();
        if !game.login_session().death_pending && !completion_retried {
            assert!(!game.can_play());
            let picture = game.take_frame().unwrap();
            let identity = picture.frame.identity;
            game.retry_frame(picture).unwrap();
            assert!(
                !login
                    .step(now, false, false)
                    .unwrap()
                    .0
                    .contains(&StartMenuIntent::FocusNickname)
            );
            game.begin_frame(now + 1., [960, 540], 1.).unwrap();
            login.observe_game(&game).unwrap();
            assert!(
                !login
                    .step(now + 1., false, false)
                    .unwrap()
                    .0
                    .contains(&StartMenuIntent::FocusNickname)
            );
            assert_eq!(game.take_frame().unwrap().frame.identity, identity);
            game.presented(true).unwrap();
            login.sync_return_focus(game.can_play());
            assert_eq!(login.input.state.focus(), Some(TextField::Nickname));
            completion_retried = true;
            focus += 1;
        } else {
            while game.take_frame().is_some() {
                game.presented(true).unwrap();
            }
            let (intents, _) = login.step(now, game.can_play(), false).unwrap();
            focus += intents
                .iter()
                .filter(|i| **i == StartMenuIntent::FocusNickname)
                .count();
        }
    }
    assert!(completion_retried);
    assert_eq!(focus, 1);
    assert!(game.can_play());
    assert_eq!(login.state.view(true).last_life, captured);
    let generation = game.generation();
    login.activate(true);
    login.activate(true);
    let (intents, _) = login.step(4500., true, false).unwrap();
    assert!(matches!(intents.as_slice(), [StartMenuIntent::Play { .. }]));
    // A failed fresh official acquisition has not replaced the old session.
    game.fail("Synthetic replay acquisition failure");
    login.observe_game(&game).unwrap();
    assert_eq!(login.state.view(true).phase, MenuPhase::Login);
    assert!(!login.state.view(true).spinner_visible);
    assert_eq!(login.state.view(true).last_life, captured);
    login.activate(true);
    assert!(matches!(
        login.step(4516., true, false).unwrap().0.as_slice(),
        [StartMenuIntent::Play { .. }]
    ));
    endpoint_login::play(&mut game, "ws://127.0.0.1:19491/slither", "Again", 0).unwrap();
    assert_eq!(game.generation(), generation + 1);
    assert_eq!(game.login_session().outcome, None);
    assert_eq!(login.state.view(false).last_life, captured);
}

#[test]
fn champion_requires_v1_and_close_restores_ordinary_login() {
    use endpoint_tape::{bundle, effects, pump};
    for flag in [None, Some(0), Some(1), Some(2)] {
        let mut game = endpoint_tape::playing();
        let mut login = NativeLogin::new("Player").unwrap();
        login.step(0., true, false).unwrap();
        login.activate(true);
        login.step(16., true, false).unwrap();
        pump(&mut game, 600.);
        login.observe_game(&game).unwrap();
        let mut first = endpoint_tape::board();
        first[2..4].copy_from_slice(&1u16.to_be_bytes());
        game.receive(game.generation(), &bundle(&[first]), 650.)
            .unwrap();
        if let Some(flag) = flag {
            game.receive(game.generation(), &bundle(&[vec![b'v', flag]]), 700.)
                .unwrap();
        }
        // Observe only after close: accepted outcome must survive the terminal
        // phase, while close-only and flag 2 have no score/champion authority.
        game.closed(game.generation());
        effects(&mut game);
        login.observe_game(&game).unwrap();
        for frame in 0..=240 {
            let now = 700. + f64::from(frame) * 16.;
            pump(&mut game, now);
            login.observe_game(&game).unwrap();
            let (intents, _) = login.step(now, game.can_play(), false).unwrap();
            if flag == Some(1)
                && login.state.view(true).phase != slither_client::start_page::MenuPhase::Login
            {
                assert!(!intents.contains(&StartMenuIntent::FocusNickname));
            }
        }
        let view = login.state.view(true);
        assert!(!view.champion_pending);
        assert!(view.ordinary_controls_visible);
        assert_eq!(
            view.last_life.label.is_empty(),
            !matches!(flag, Some(0 | 1))
        );
        assert!(!view.disabled);
        assert_eq!(login.input.state.focus(), Some(TextField::Nickname));
        assert!(!game.champion_session().champion_acknowledged);
    }
}

#[test]
fn native_login_endpoint_enter_pointer_and_terminal_retry_send_fixed_profile() {
    use endpoint_tape::{bundle, challenge, effects, pump};
    use slither_wgpu_slice::{endpoint_login, game::LocalGame};
    for cancel in [false, true] {
        let mut game = LocalGame::new(0.).unwrap();
        let mut login = NativeLogin::new("Fixed").unwrap();
        for attempt in 0..2 {
            let now = f64::from(attempt) * 32.;
            login.step(now, game.can_play(), false).unwrap();
            if attempt == 0 {
                login.activate(game.can_play());
            } else {
                login
                    .prepare_host([960, 540], 1., game.can_play(), None)
                    .unwrap();
                let play = login.layout.viewport_rect(E::Play);
                let point = [play.x + play.width / 2., play.y + play.height / 2.];
                login.pointer(point, Some(true), game.can_play());
                login.pointer(point, Some(false), game.can_play());
            }
            let (intents, _) = login.step(now + 16., game.can_play(), false).unwrap();
            let [StartMenuIntent::Play { nickname, .. }] = intents.as_slice() else {
                panic!("one explicit native Play");
            };
            // The native host dispatches the shared menu intent through this
            // same desktop endpoint helper; no socket or acquisition is needed.
            assert_eq!(
                endpoint_login::play(&mut game, "ws://127.0.0.1:19491/slither", nickname, 9)
                    .unwrap(),
                291
            );
            pump(&mut game, now + 1.);
            effects(&mut game);
            game.opened(game.generation(), now + 2.).unwrap();
            effects(&mut game);
            game.receive(game.generation(), &bundle(&[challenge()]), now + 3.)
                .unwrap();
            let packets = effects(&mut game);
            assert_eq!(packets.len(), 2);
            assert_eq!(
                &packets[1][..24],
                &[
                    115, 30, 1, 35, 54, 206, 204, 169, 97, 178, 74, 136, 124, 117, 14, 210, 106,
                    236, 8, 208, 136, 213, 140, 111,
                ]
            );
            assert_eq!(&packets[1][24..], b"\x09\x05Fixed\x00\xff");
            if cancel {
                game.stop();
            } else {
                game.fail("Synthetic failed login");
            }
            effects(&mut game);
            assert!(game.can_play());
            assert!(login.observe_session(game.phase()));
            assert!(!login.state.view(true).spinner_visible);
        }
    }
}

#[test]
fn login_menu_composes_over_the_world_picture() {
    let mut game = slither_wgpu_slice::game::LocalGame::new(0.).unwrap();
    let mut login = NativeLogin::new("Player").unwrap();
    // Same sizes as the desktop redraw: world at the backing size, menu at physical.
    let extent = game.window_viewport([960, 540], 1.).unwrap();
    for t in [16., 32., 48.] {
        game.begin_redraw(t, extent, 1.).unwrap();
        while let Some(e) = game.effect() {
            game.acknowledge(e.generation, e.id, true).unwrap();
        }
        let p = game.take_frame().expect("world picture");
        let mut picture = p.clone();
        login
            .prepare_host([960, 540], 1., true, game.menu_cosmetics())
            .unwrap()
            .append_to(&mut picture)
            .unwrap();
        let resident = picture
            .resources
            .iter()
            .map(|r| (r.asset.into(), r.asset))
            .collect();
        picture.frame.quads(&resident).unwrap();
        picture.overlay.as_ref().unwrap().quads(&resident).unwrap();
        game.presented(true).unwrap();
    }
    game.begin_redraw(64., extent, 1.).unwrap();
    while let Some(e) = game.effect() {
        game.acknowledge(e.generation, e.id, true).unwrap();
    }
    let mut picture = game.take_frame().expect("world picture");
    let mut menu = login.prepare_host([960, 540], 1., true, None).unwrap();
    menu.resources[0].asset = picture.resources[0].asset;
    menu.resources[0].asset.revision += 1;
    assert!(menu.append_to(&mut picture).is_err());
}

#[test]
fn shared_login_hits_fitted_text_and_drags_clamp_at_hidpi() {
    use slither_client::{
        login_controls::NICKNAME, menu_controls::ControlKind, menu_theme::MenuTheme,
    };
    use slither_wgpu_slice::{editor_geometry::field_geometry, text::TextCache};
    for dpr in [1.5, 2.] {
        for value in ["abcdef", "WWWWWWWWWWWWWWWWWWWWWWWW"] {
            let mut login = NativeLogin::new(value).unwrap();
            login.step(0., true, false).unwrap();
            let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
            login.prepare_host(extent, dpr, true, None).unwrap();
            let resolved = login.panel.resolve([960., 540.]).unwrap();
            let r = resolved.iter().find(|r| r.control.id == NICKNAME).unwrap();
            let ControlKind::TextField { fit, .. } = r.control.kind else {
                unreachable!()
            };
            assert!(fit);
            let mut text = TextCache::menu();
            let (_, g) = field_geometry(&mut text, r, &login.input.state, &mut Default::default())
                .unwrap()
                .unwrap();
            let base = MenuTheme::default().text_style(r.control.style).size * r.scale;
            if value.starts_with('W') {
                assert!(g.style.size < base);
            }
            let advance = |text: &mut TextCache, count| {
                text.measure_panel(&value[..count], g.style)
                    .unwrap()
                    .advance
            };
            let point = |text: &mut TextCache, count| {
                [
                    g.placement.origin[0] + advance(text, count),
                    g.rect.y + g.rect.height / 2.,
                ]
            };
            // Native coordinates arrive as snapped physical pixels divided by DPR.
            let start = point(&mut text, 2).map(|v| (v * dpr).round() / dpr);
            login.pointer(start, Some(true), true);
            assert_eq!(
                login.input.state.editor(TextField::Nickname).selection(),
                (2, 2)
            );
            let end = point(&mut text, 5).map(|v| (v * dpr).round() / dpr);
            login.pointer(end, None, true);
            assert_eq!(
                login.input.state.editor(TextField::Nickname).selection(),
                (2, 5)
            );
            let outside = [g.rect.x - 20., start[1]];
            assert!(login.intercepts_pointer(outside));
            login.pointer(outside, None, true);
            assert_eq!(
                login.input.state.editor(TextField::Nickname).selection(),
                (2, 0)
            );
            let outside = [g.rect.x + g.rect.width + 20., start[1]];
            login.pointer(outside, None, true);
            assert_eq!(
                login.input.state.editor(TextField::Nickname).selection(),
                (2, value.len())
            );
            login.pointer(outside, Some(false), true);
            assert_eq!(login.controls.capture(), None);
            login.pointer(start, None, true);
            assert_eq!(
                login.input.state.editor(TextField::Nickname).selection(),
                (2, value.len())
            );
            login.input.state.apply(EditIntent::KeyText("X".into()));
            assert_eq!(login.displayed_nickname(), format!("{}X", &value[..2]));
        }
    }
}
#[test]
fn login_preedit_suppresses_pointer_selection_and_activation_but_releases_capture() {
    use slither_client::login_controls::NICKNAME;
    let mut login = NativeLogin::new("abcdef").unwrap();
    login.step(0., true, false).unwrap();
    login.prepare([960, 540], true, None).unwrap();
    let r = login.layout.viewport_rect(E::Nickname);
    let p = [r.x + 30., r.y + 12.];
    login.pointer(p, Some(true), true);
    assert_eq!(login.controls.capture(), Some(NICKNAME));
    let selected = login.input.state.editor(TextField::Nickname).selection();
    login
        .input
        .state
        .apply(EditIntent::Preedit("é".into(), None));
    let play = login.layout.viewport_rect(E::Play);
    login.pointer([play.x + 10., play.y + 10.], Some(false), true);
    assert_eq!(login.controls.capture(), None);
    assert_eq!(
        login.input.state.editor(TextField::Nickname).selection(),
        selected
    );
    assert_eq!(login.state.view(true).phase, MenuPhase::Login);
    login.input.state.apply(EditIntent::ImeCommit("X".into()));
    let after = login.input.state.editor(TextField::Nickname).selection();
    login.pointer([r.x, r.y], None, true);
    assert_eq!(
        login.input.state.editor(TextField::Nickname).selection(),
        after
    );
}
#[cfg(windows)]
#[test]
fn windows_click_preferences_are_converted_to_logical_half_extents() {
    use slither_wgpu_slice::menu_input_native::click_settings;
    for dpr in [1., 1.5, 2.] {
        let settings = click_settings(dpr);
        assert_eq!(settings.time_ms, f64::from(winsafe::GetDoubleClickTime()));
        assert_eq!(
            settings.distance,
            [winsafe::co::SM::CXDOUBLECLK, winsafe::co::SM::CYDOUBLECLK]
                .map(|m| f64::from(winsafe::GetSystemMetrics(m).max(1)) / (2. * dpr))
        );
    }
}

#[test]
fn shared_login_return_enter_keeps_terminal_guard_and_once_only_queue() {
    use slither_client::{session_policy::Phase, start_page::MenuSession};
    use slither_wgpu_slice::menu_input_native::KeyInput;
    use winit::{
        event::ElementState,
        keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    };
    for terminal in [false, true] {
        let mut login = NativeLogin::new("Stored").unwrap();
        login.step(0., true, false).unwrap();
        login.activate(true);
        login.step(16., true, false).unwrap();
        login.step(32., true, true).unwrap();
        assert!(login.state.accepted_game_over(Phase::Dead, None).unwrap());
        let mut session = MenuSession {
            phase: if terminal {
                Phase::Disconnected
            } else {
                Phase::Dead
            },
            outcome: Some(Phase::Dead),
            playing: false,
            death_pending: true,
            return_active: true,
            return_fraction: 0.6,
        };
        login.state.observe_session(session, None).unwrap();
        login.step(48., false, false).unwrap();
        assert_eq!(login.input.state.focus(), None);
        let key = Key::Named(NamedKey::Enter);
        for (index, repeat) in [false, true, false].into_iter().enumerate() {
            let result = login.key(
                KeyInput {
                    logical: &key,
                    physical: PhysicalKey::Code(KeyCode::Enter),
                    text: Some("\r"),
                    state: ElementState::Pressed,
                    repeat,
                },
                false,
            );
            if !repeat && (!terminal || index == 0) {
                assert!(result.consumed);
            }
            assert!(
                result
                    .effects
                    .iter()
                    .all(|e| !matches!(e, HostEditEffect::Activate(_)))
            );
        }
        assert_eq!(
            login.state.view(false).phase,
            if terminal {
                MenuPhase::Connecting
            } else {
                MenuPhase::Returning
            }
        );
        assert!(login.step(64., false, false).unwrap().0.is_empty());
        if terminal {
            session.death_pending = false;
            session.return_active = false;
            session.return_fraction = 0.;
            login.state.observe_session(session, None).unwrap();
            let intents = login.step(80., true, false).unwrap().0;
            assert!(
                matches!(intents.as_slice(), [StartMenuIntent::Play { nickname, .. }] if nickname == "Stored")
            );
            assert!(login.step(96., true, false).unwrap().0.is_empty());
        }
    }
}

#[test]
fn settled_login_density_cap_keeps_real_viewport_layout_and_hits() {
    use slither_client::desktop_input::mouse_client_position;
    for height in [540., 720.] {
        let mut login = NativeLogin::new("Player").unwrap();
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, true, false).unwrap();
        }
        let mut previous = None;
        for dpr in [5., 6., 7., 8., 5.] {
            let extent = [(960. * dpr) as u32, (height * dpr) as u32];
            let frame = login.prepare_host(extent, dpr, true, None).unwrap();
            assert_eq!(frame.frame.extent, extent);
            assert_eq!(frame.frame.viewport_scale, dpr);
            assert!(
                frame.frame.draws.len()
                    + usize::from(frame.backdrop.is_some())
                    + usize::from(frame.world_cover.is_some())
                    <= 512
            );
            assert!(
                frame
                    .resources
                    .iter()
                    .map(|r| r.image.pixels.len())
                    .sum::<usize>()
                    <= 13 * 1024 * 1024
            );
            let rects = [E::Logo, E::NicknameHolder, E::Play, E::Settings, E::Server]
                .map(|e| login.layout.viewport_rect(e));
            if let Some(old) = previous {
                assert_eq!(rects, old);
            }
            previous = Some(rects);
            let rect = login.layout.viewport_rect(E::Settings);
            let point = mouse_client_position(
                [(rect.x + rect.width / 2.) * dpr, (rect.y + 46.) * dpr],
                dpr,
            );
            assert!(frame.frame.draws.iter().any(|draw| {
                let r = draw.destination;
                point[0] >= r.x
                    && point[0] < r.x + r.width
                    && point[1] >= r.y
                    && point[1] < r.y + r.height
            }));
            let quality = login.state.view(true).quality;
            login.pointer(point, Some(true), true);
            login.pointer(point, Some(false), true);
            assert!(login.state.settings.visible());
            assert_eq!(login.state.view(true).quality, quality);
            login.close_settings(true);
        }
    }
}
