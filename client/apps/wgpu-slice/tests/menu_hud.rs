//! Authored session/picture transitions; no window, GPU, sockets or external art.
#![cfg(windows)]
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::{menu_layout::MenuElement, session_policy::Phase, start_page::MenuPhase};
use slither_wgpu_slice::{game::LocalGame, login_native::NativeLogin, render::PreparedFrame};

fn frame(game: &mut LocalGame, now: f64) -> PreparedFrame {
    game.begin_redraw(now, [960, 720], 1.).unwrap();
    tape::effects(game);
    game.take_frame().unwrap()
}
fn present(game: &mut LocalGame, picture: PreparedFrame) {
    game.presented(true).unwrap();
    game.bank.recycle_frame(picture);
}
fn playing() -> (LocalGame, NativeLogin) {
    let mut game = tape::playing();
    game.window_viewport([960, 720], 1.).unwrap();
    let mut login = NativeLogin::new("Player").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(16., true, false).unwrap();
    let mut announcement = vec![b'm', 0, 0, 1, 255, 255, 255, 3];
    announcement.extend(b"BotAuthored record");
    game.receive(
        game.generation(),
        &tape::bundle(&[vec![b'M', 0, 8, 64], announcement]),
        31.,
    )
    .unwrap();
    for now in (32..=1008).step_by(16) {
        game.receive(
            game.generation(),
            &tape::bundle(&[vec![b'p']]),
            f64::from(now),
        )
        .unwrap();
        let picture = frame(&mut game, f64::from(now));
        login.observe_game(&game).unwrap();
        login.step(f64::from(now), false, true).unwrap();
        present(&mut game, picture);
    }
    (game, login)
}

#[test]
fn terminal_login_never_emits_retained_game_hud() {
    let mut results = Vec::new();
    for transition in [
        "remote reset",
        "server close",
        "local stop",
        "connect failure",
    ] {
        let (mut game, mut login) = playing();
        let picture = frame(&mut game, 1024.);
        assert!(
            !picture.overlay.as_ref().unwrap().draws.is_empty(),
            "live HUD positive control"
        );
        present(&mut game, picture);
        match transition {
            "remote reset" => game
                .transport_failed(
                    game.generation(),
                    "IO error: remote host forcibly closed connection (os error 10054)",
                )
                .unwrap(),
            "server close" => game.closed(game.generation()),
            "local stop" => game.request_stop(),
            _ => {
                game.closed(game.generation());
                tape::effects(&mut game);
                let picture = frame(&mut game, 1040.);
                present(&mut game, picture);
                game.play("ws://127.0.0.1:19491/slither", "Player", 0)
                    .unwrap();
                let picture = frame(&mut game, 1056.);
                present(&mut game, picture);
                tape::effects(&mut game);
                game.transport_failed(game.generation(), "Authored connect failure")
                    .unwrap();
            }
        }
        tape::effects(&mut game);
        let mut emitted = 0;
        for now in [1088., 1104., 1120.] {
            login.observe_game(&game).unwrap();
            login.step(now, game.can_play(), false).unwrap();
            let picture = frame(&mut game, now);
            assert_eq!(login.state.view(false).phase, MenuPhase::Login);
            emitted += picture.overlay.as_ref().unwrap().draws.len();
            present(&mut game, picture);
        }
        println!("{transition}: {emitted} game HUD draws on Login");
        results.push(emitted);
    }
    assert_eq!(results, [0; 4]);
}

#[test]
fn death_return_panels_never_emit_game_hud_and_keep_the_world_fade() {
    for panel in [None, Some(MenuElement::Server), Some(MenuElement::Settings)] {
        let (mut game, mut login) = playing();
        game.receive(game.generation(), &tape::bundle(&[vec![b'v', 0]]), 1024.)
            .unwrap();
        let mut returning = false;
        for now in (1040..=10000).step_by(16) {
            game.receive(
                game.generation(),
                &tape::bundle(&[vec![b'p']]),
                f64::from(now),
            )
            .unwrap();
            login.observe_game(&game).unwrap();
            login.step(f64::from(now), game.can_play(), false).unwrap();
            let view = login.state.view(false);
            if !returning && view.phase == MenuPhase::Returning {
                if let Some(button) = panel {
                    login.prepare_host([960, 720], 1., false, None).unwrap();
                    let r = login.layout.viewport_rect(button);
                    let point = [r.x + r.width / 2., r.y + r.height / 2.];
                    login.pointer(point, Some(true), false);
                    login.pointer(point, Some(false), false);
                    assert!(if button == MenuElement::Server {
                        login.picker.policy.visible()
                    } else {
                        login.state.settings.visible()
                    });
                    // A rendering/protocol failure can occur with the panel retained.
                    game.fail("Authored failure while panel is open");
                    tape::effects(&mut game);
                }
                returning = true;
            }
            let picture = frame(&mut game, f64::from(now));
            if returning {
                assert!(
                    picture.overlay.as_ref().unwrap().draws.is_empty(),
                    "{panel:?} at {now}"
                );
                if panel.is_none() && view.world_alpha > 0. && game.phase() == Phase::Dead {
                    assert!(
                        !picture.frame.draws.is_empty(),
                        "intentional death world fade remains"
                    );
                }
            }
            present(&mut game, picture);
        }
        assert!(returning);
        assert_eq!(login.state.view(false).phase, MenuPhase::Login);
    }
}

#[test]
fn terminal_panels_and_in_game_retry_keep_all_hud_layers_out() {
    for button in [
        MenuElement::Server,
        MenuElement::Settings,
        MenuElement::Skin,
    ] {
        let (mut game, mut login) = playing();
        let picture = frame(&mut game, 1024.);
        let overlay = picture.overlay.as_ref().unwrap();
        assert!(
            overlay.draws.iter().any(|d| d.asset.handle >> 60 == 5),
            "minimap positive control"
        );
        assert!(!overlay.draws.is_empty());
        game.retry_frame(picture).unwrap();
        assert!(game.open_ingame_menu(1025.).unwrap());
        tape::effects(&mut game);
        let retry = game.take_frame().unwrap();
        assert!(
            retry.overlay.as_ref().unwrap().draws.is_empty(),
            "retry rechecks modal ownership"
        );
        present(&mut game, retry);
        game.closed(game.generation());
        tape::effects(&mut game);
        login.observe_game(&game).unwrap();
        login.step(1040., game.can_play(), false).unwrap();
        login.prepare_host([960, 720], 1., true, None).unwrap();
        let r = login.layout.viewport_rect(button);
        let point = [r.x + r.width / 2., r.y + r.height / 2.];
        login.pointer(point, Some(true), true);
        login.pointer(point, Some(false), true);
        assert!(
            login.picker.policy.visible()
                || login.state.settings.visible()
                || login.studio.state.visible()
        );
        game.fail("Authored terminal panel failure");
        let picture = frame(&mut game, 1056.);
        assert!(picture.frame.draws.is_empty());
        assert!(picture.overlay.as_ref().unwrap().draws.is_empty());
        present(&mut game, picture);
        // The browser/single-backing path has no independent overlay to clear.
        game.bank.overlay_viewport = None;
        let picture = frame(&mut game, 1072.);
        assert!(picture.frame.draws.is_empty());
        assert!(picture.overlay.is_none());
        present(&mut game, picture);
    }
}

#[test]
fn visible_login_handoff_suppresses_hud_even_when_session_is_playing() {
    let (mut game, mut login) = playing();
    // The native host may still be drawing an outgoing login over accepted play.
    login.state = slither_client::start_page::StartMenuState::new(
        slither_client::start_page::MenuLocale::English,
        false,
        slither_client::menu_preferences::MenuQuality::Low,
    );
    login.step(1024., false, false).unwrap();
    assert!(login.needs_menu_pass(false));
    let picture = frame(&mut game, 1024.);
    assert!(!picture.overlay.as_ref().unwrap().draws.is_empty());
    game.retry_frame(picture).unwrap();
    let picture = game
        .take_frame_with_menu(login.needs_menu_pass(false))
        .unwrap();
    assert!(
        !picture.frame.draws.is_empty(),
        "world can crossfade under login"
    );
    assert!(picture.overlay.as_ref().unwrap().draws.is_empty());
    present(&mut game, picture);
    let picture = frame(&mut game, 1040.);
    assert!(
        !picture.overlay.as_ref().unwrap().draws.is_empty(),
        "fresh gameplay restores HUD"
    );
    present(&mut game, picture);
}

#[test]
fn browser_login_fade_excludes_hud_on_setup_flush_frames_and_retries() {
    let mut game = tape::negotiating();
    game.window_viewport([960, 720], 1.).unwrap();
    let mut login = NativeLogin::new("Player").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(16., true, false).unwrap();
    game.receive(
        game.generation(),
        &tape::bundle(&[
            tape::setup(),
            tape::snake(44),
            tape::food(),
            tape::board(),
            vec![b'M', 0, 8, 64],
        ]),
        31.,
    )
    .unwrap();
    tape::effects(&mut game);
    // Receive/Setup flush has not advanced a frame or the login fade.
    while let Some(picture) = game.take_browser_frame() {
        assert!(
            picture.overlay.as_ref().unwrap().draws.is_empty(),
            "Setup flush excludes all HUD draws"
        );
        present(&mut game, picture);
    }
    let mut overlaps = Vec::new();
    let mut restored = false;
    for now in (32..=800).step_by(16) {
        game.receive(
            game.generation(),
            &tape::bundle(&[vec![b'p']]),
            f64::from(now),
        )
        .unwrap();
        game.begin_redraw(f64::from(now), [960, 720], 1.).unwrap();
        tape::effects(&mut game);
        login.observe_game(&game).unwrap();
        login.step(f64::from(now), false, true).unwrap();
        let view = login.state.view(false);
        let picture = game.take_browser_frame().unwrap();
        let count = picture
            .overlay
            .as_ref()
            .unwrap()
            .draws
            .iter()
            .filter(|d| d.effective_alpha > 0.)
            .count();
        if view.visible && view.login_alpha > 0. && count > 0 {
            overlaps.push((now, count));
        }
        restored |= !view.visible && count > 0;
        game.retry_frame(picture).unwrap();
        let retry = game.take_browser_frame().unwrap();
        assert_eq!(
            retry
                .overlay
                .as_ref()
                .unwrap()
                .draws
                .iter()
                .filter(|d| d.effective_alpha > 0.)
                .count(),
            count
        );
        present(&mut game, retry);
    }
    println!("browser login/HUD overlaps: {overlaps:?}");
    assert!(overlaps.is_empty());
    assert!(restored, "HUD returns after the DOM login fade completes");
}

#[test]
fn dismissed_menu_restores_the_same_retry_picture_without_a_fresh_frame() {
    for separate_overlay in [false, true] {
        let (mut game, _) = playing();
        if !separate_overlay {
            game.bank.overlay_viewport = None;
        }
        assert!(game.open_ingame_menu(1024.).unwrap());
        let picture = frame(&mut game, 1040.);
        assert_eq!(
            picture.ui_frame().draws.len(),
            if separate_overlay {
                0
            } else {
                picture.world_draws
            }
        );
        let identity = picture.frame.identity;
        game.retry_frame(picture).unwrap();
        let picture = game.take_frame().unwrap();
        game.retry_frame(picture).unwrap();
        game.close_ingame_menu();
        let picture = game.take_frame().unwrap();
        assert_eq!(picture.frame.identity, identity);
        let hud = picture.ui_frame().draws.len()
            - if separate_overlay {
                0
            } else {
                picture.world_draws
            };
        println!("separate overlay={separate_overlay}: restored HUD draws={hud}");
        present(&mut game, picture);
        assert!(hud > 0, "dismissing a menu must restore the retained HUD");
    }
}

#[test]
fn reset_during_death_hold_keeps_background_art_but_no_entities_or_hud() {
    use slither_wgpu_slice::cosmetics::{BackgroundArt, Cosmetics};
    let (mut game, mut login) = playing();
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(8, 8, image::Rgba([32, 64, 96, 255]))
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let png = bytes.into_inner();
    game.bank.cosmetics = Some(
        Cosmetics::load(
            |path| {
                if path == BackgroundArt::HexTile.path() {
                    Ok(png.clone())
                } else {
                    Err("No authored image for this role".into())
                }
            },
            None,
        )
        .unwrap(),
    );
    let tile = game.bank.background_art(BackgroundArt::HexTile).unwrap();
    let picture = frame(&mut game, 1024.);
    assert!(picture.frame.draws.iter().any(|d| d.asset == tile));
    present(&mut game, picture);
    game.receive(game.generation(), &tape::bundle(&[vec![b'v', 0]]), 1040.)
        .unwrap();
    login.observe_game(&game).unwrap();
    game.transport_failed(game.generation(), "Authored reset during death hold")
        .unwrap();
    tape::effects(&mut game);
    login.observe_game(&game).unwrap();
    login.step(1056., game.can_play(), false).unwrap();
    assert_eq!(login.state.view(false).phase, MenuPhase::DeathHold);
    assert_eq!(login.state.view(false).world_alpha, 1.);
    let picture = frame(&mut game, 1056.);
    assert!(picture.frame.snakes.is_empty());
    assert!(picture.frame.food_batch.is_none());
    assert!(picture.overlay.as_ref().unwrap().draws.is_empty());
    let backgrounds = picture
        .frame
        .draws
        .iter()
        .filter(|d| d.asset == tile)
        .count();
    assert_eq!(
        picture.frame.draws.len(),
        backgrounds,
        "only authored background remains"
    );
    println!("background draws during terminal death hold: {backgrounds}");
    present(&mut game, picture);
    assert!(backgrounds > 0);
}

#[test]
fn in_game_menu_and_its_settings_never_emit_game_hud() {
    use egui::{Event, Key, Modifiers, RawInput};
    let key = |key, pressed, shift| Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: Modifiers {
            shift,
            ..Modifiers::NONE
        },
    };
    let send = |login: &mut NativeLogin, game: &mut LocalGame, events| {
        login
            .ingame_input(
                game,
                RawInput {
                    events,
                    ..Default::default()
                },
            )
            .unwrap();
    };
    let (mut game, mut login) = playing();
    let mut emitted = Vec::new();
    let mut settings_frames = 0;
    for now in (1024..=2400).step_by(16) {
        let now = f64::from(now);
        match now as u32 {
            1040 => assert!(login.open_ingame_menu(&mut game, now).unwrap()),
            1104 => send(
                &mut login,
                &mut game,
                vec![
                    key(Key::Tab, true, false),
                    key(Key::Tab, false, false),
                    key(Key::Enter, true, false),
                    key(Key::Enter, false, false),
                ],
            ),
            1600 => send(
                &mut login,
                &mut game,
                vec![
                    key(Key::Escape, true, false),
                    key(Key::Escape, false, false),
                ],
            ),
            2048 => send(
                &mut login,
                &mut game,
                vec![
                    key(Key::Escape, true, false),
                    key(Key::Escape, false, false),
                ],
            ),
            _ => {}
        }
        game.receive(game.generation(), &tape::bundle(&[vec![b'p']]), now)
            .unwrap();
        login.observe_game(&game).unwrap();
        login.step(now, false, true).unwrap();
        login.prepare_ingame(&mut game, [960, 720], 1.).unwrap();
        let picture = frame(&mut game, now);
        let hud = picture.overlay.as_ref().unwrap().draws.len();
        if login.state.settings.visible() {
            settings_frames += 1;
        }
        if game.ingame_menu_open() {
            emitted.push((now, hud));
        } else if now > 2048. {
            assert!(hud > 0, "Resume restores the HUD at {now}");
        }
        present(&mut game, picture);
    }
    assert!(
        settings_frames > 20,
        "Settings was open and fading over the game"
    );
    assert!(emitted.len() > 60);
    assert!(
        emitted.iter().all(|(_, hud)| *hud == 0),
        "HUD drawn under a visible menu: {emitted:?}"
    );
}
