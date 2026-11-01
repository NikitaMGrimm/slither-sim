//! Authored CPU-only rules using the production LocalGame and native menu host.
use super::{bundle, challenge, effects, negotiating, pictures, playing, pump, setup, snake};
use egui::{Event, Key, Modifiers, PointerButton, RawInput, Rect, pos2, vec2};
use slither_client::{session_policy::Phase, start_page::MenuPhase};
use slither_wgpu_slice::{
    game::{EffectKind, LocalGame},
    ingame_egui::leave_id,
    login_native::NativeLogin,
    menu_input_native::KeyInput,
};
use winit::{
    dpi::PhysicalPosition,
    event::{DeviceId, ElementState, WindowEvent},
    keyboard::{Key as NativeKey, KeyCode, NamedKey, PhysicalKey},
};

fn login(game: &LocalGame) -> NativeLogin {
    let mut login = NativeLogin::new("Player").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(8., true, false).unwrap();
    login.observe_game(game).unwrap();
    login.step(20., false, true).unwrap();
    login
}

fn key_event(key: Key, pressed: bool, repeat: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat,
        modifiers: Modifiers::NONE,
    }
}
fn input(login: &mut NativeLogin, game: &mut LocalGame, events: Vec<Event>) {
    login
        .ingame_input(
            game,
            RawInput {
                events,
                ..Default::default()
            },
        )
        .unwrap();
}
fn escape(login: &mut NativeLogin, game: &mut LocalGame, repeat: bool) -> bool {
    let native = NativeKey::Named(NamedKey::Escape);
    login.egui.window.key(&native, ElementState::Pressed);
    if game.ingame_menu_open() {
        input(login, game, vec![key_event(Key::Escape, true, repeat)]);
        true
    } else {
        login
            .ingame_key(
                game,
                KeyInput {
                    logical: &native,
                    physical: PhysicalKey::Code(KeyCode::Escape),
                    text: None,
                    state: ElementState::Pressed,
                    repeat,
                },
                30.,
            )
            .unwrap()
    }
}
fn release_escape(login: &mut NativeLogin, game: &mut LocalGame) {
    login
        .egui
        .window
        .key(&NativeKey::Named(NamedKey::Escape), ElementState::Released);
    input(login, game, vec![key_event(Key::Escape, false, false)]);
}
fn click(login: &mut NativeLogin, game: &mut LocalGame, point: egui::Pos2) {
    input(
        login,
        game,
        vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ],
    );
}

#[test]
fn esc_toggle_repeat_outside_click_and_single_keyboard_focus() {
    let mut game = playing();
    let mut login = login(&game);
    login
        .egui
        .window
        .key(&NativeKey::Named(NamedKey::Space), ElementState::Pressed);
    assert!(escape(&mut login, &mut game, false));
    assert!(game.ingame_menu_open());
    assert!(login.egui.active());
    assert!(login.blocks_gameplay());
    assert!(login.ingame_ui.focused);
    assert_eq!(
        login.egui.context().unwrap().memory(|m| m.focused()),
        Some(leave_id())
    );
    assert_eq!(login.controls.focus(), None);
    assert_eq!(login.input.state.focus(), None);
    input(
        &mut login,
        &mut game,
        vec![key_event(Key::Space, true, false)],
    );
    assert!(
        game.ingame_menu_open(),
        "a held boost key cannot activate Leave on entry"
    );
    input(
        &mut login,
        &mut game,
        vec![key_event(Key::Space, false, false)],
    );
    assert!(escape(&mut login, &mut game, true));
    assert!(
        game.ingame_menu_open(),
        "held Esc cannot immediately close the menu"
    );
    release_escape(&mut login, &mut game);
    for shift in [false, true] {
        let modifiers = Modifiers {
            shift,
            ..Modifiers::NONE
        };
        let mut tab = key_event(Key::Tab, true, false);
        if let Event::Key { modifiers: m, .. } = &mut tab {
            *m = modifiers;
        }
        input(
            &mut login,
            &mut game,
            vec![tab, key_event(Key::Tab, false, false)],
        );
        assert!(login.ingame_ui.focused);
        assert_eq!(
            login.egui.context().unwrap().memory(|m| m.focused()),
            Some(leave_id())
        );
    }
    // Panel padding consumes clicks without dismissal or activation.
    let padding = login.ingame_ui.panel_rect.min + vec2(3., 3.);
    click(&mut login, &mut game, padding);
    assert!(game.ingame_menu_open());
    escape(&mut login, &mut game, false);
    assert!(!game.ingame_menu_open());
    assert!(!login.egui.active());
    release_escape(&mut login, &mut game);
    escape(&mut login, &mut game, false);
    release_escape(&mut login, &mut game);
    click(&mut login, &mut game, pos2(2., 2.));
    assert!(!game.ingame_menu_open());
    assert_eq!(game.phase(), Phase::Playing);
    assert!(!login.egui.active());
}

#[test]
fn overlay_geometry_is_centred_bounded_and_has_one_control_across_dpi() {
    for extent in [
        [320, 240],
        [640, 120],
        [640, 360],
        [1920, 1080],
        [3840, 2160],
    ] {
        for dpr in [1., 1.5, 2.] {
            let mut game = playing();
            let mut login = login(&game);
            escape(&mut login, &mut game, false);
            for _ in 0..2 {
                login.prepare_ingame(&mut game, extent, dpr).unwrap();
            }
            let viewport = Rect::from_min_size(
                pos2(0., 0.),
                vec2(extent[0] as f32 / dpr as f32, extent[1] as f32 / dpr as f32),
            );
            let panel = login.ingame_ui.panel_rect;
            let control = login.ingame_ui.leave_rect;
            assert!(
                viewport.contains_rect(panel),
                "{extent:?} DPR {dpr}: {panel:?}"
            );
            assert!(panel.contains_rect(control));
            assert!((panel.center() - viewport.center()).length() <= 1.);
            assert!(panel.width() <= 257. && panel.height() <= 85.);
            assert!(login.ingame_ui.focused);
            assert_eq!(
                login.egui.context().unwrap().memory(|m| m.focused()),
                Some(leave_id())
            );
        }
    }
}

#[test]
fn inactive_desktop_menu_path_never_runs_an_egui_pass() {
    let mut game = playing();
    let mut login = login(&game);
    for now in 21..30 {
        login.step(f64::from(now), false, true).unwrap();
        login.prepare_ingame(&mut game, [960, 540], 1.).unwrap();
    }
    assert!(!login.egui.initialized());
    escape(&mut login, &mut game, false);
    release_escape(&mut login, &mut game);
    escape(&mut login, &mut game, false);
    let passes = login.egui.context().unwrap().cumulative_pass_nr();
    for now in 31..100 {
        login.step(f64::from(now), false, true).unwrap();
        login.prepare_ingame(&mut game, [960, 540], 1.).unwrap();
    }
    assert_eq!(login.egui.context().unwrap().cumulative_pass_nr(), passes);
}

#[test]
fn open_releases_boost_immediately_freezes_heading_and_keeps_pinging() {
    let mut game = playing();
    let mut login = login(&game);
    game.key("Space", true);
    game.key("ArrowLeft", true);
    game.pointer(900., 450., 960., 540., false).unwrap();
    assert!(pump(&mut game, 100.).contains(&vec![253]));
    assert!(game.wheel_zoom(1.));
    assert!(login.open_ingame_menu(&mut game, 101.).unwrap());
    assert_eq!(
        effects(&mut game),
        [vec![254]],
        "release bypasses the 50 ms gate"
    );
    let before = game.client().control_view().own.unwrap().raw_head;
    let mut pings = 0;
    for now in (120..=1000).step_by(40) {
        assert!(!game.key("Space", true));
        assert!(!game.key("ArrowLeft", true));
        game.pointer(100., 100., 960., 540., true).unwrap();
        assert!(!game.wheel_zoom(1.));
        let packets = pump(&mut game, f64::from(now));
        assert!(
            packets.iter().all(|p| p == &[251]),
            "unexpected control: {packets:?}"
        );
        if packets.contains(&vec![251]) {
            pings += 1;
            game.receive(game.generation(), b"p", f64::from(now) + 1.)
                .unwrap();
        }
    }
    assert!(
        pings >= 3,
        "normal ping cadence continues while the overlay owns input"
    );
    assert_ne!(
        game.client().control_view().own.unwrap().raw_head,
        before,
        "the game keeps running"
    );
    let event = WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(100., 100.),
    };
    let _ = login.egui.route_window_event(&event, 1., |_| ());
    release_escape(&mut login, &mut game);
    escape(&mut login, &mut game, false);
    let packets = pump(&mut game, 1040.);
    assert_eq!(
        packets
            .iter()
            .filter(|p| p.len() == 1 && p[0] < 251)
            .count(),
        1
    );
    assert!(packets.iter().all(|p| p.len() == 1 && p[0] <= 251));
    assert!(
        pump(&mut game, 1041.).is_empty(),
        "no turn debt or heading catch-up"
    );
}

#[test]
fn death_while_open_retires_the_overlay_and_esc_cannot_reopen_it() {
    let mut game = playing();
    let mut login = login(&game);
    escape(&mut login, &mut game, false);
    game.receive(game.generation(), &[b'v', 0], 31.).unwrap();
    login.observe_game(&game).unwrap();
    assert_eq!(game.phase(), Phase::Dead);
    assert!(!login.egui.active());
    assert!(!game.ingame_menu_open());
    assert!(!escape(&mut login, &mut game, false));
}

#[test]
fn leave_by_mouse_enter_and_space_returns_to_login_and_can_play_again() {
    for key in [None, Some(Key::Enter), Some(Key::Space)] {
        let mut game = playing();
        let mut login = login(&game);
        escape(&mut login, &mut game, false);
        release_escape(&mut login, &mut game);
        if let Some(key) = key {
            input(
                &mut login,
                &mut game,
                vec![key_event(key, true, false), key_event(key, false, false)],
            );
        } else {
            let point = login.ingame_ui.leave_rect.center();
            click(&mut login, &mut game, point);
        }
        assert_eq!(game.phase(), Phase::Disconnected);
        let close = game.effect().expect("existing teardown queues Close");
        assert_eq!(close.kind, EffectKind::Close);
        game.acknowledge(close.generation, close.id, true).unwrap();
        pump(&mut game, 40.);
        login.observe_game(&game).unwrap();
        login.step(40., game.can_play(), false).unwrap();
        assert_eq!(login.state.view(true).phase, MenuPhase::Login);
        assert!(!login.egui.active());
        assert!(game.can_play());
        let old_generation = game.generation();
        login.activate(true);
        assert!(
            login
                .step(48., true, false)
                .unwrap()
                .0
                .iter()
                .any(|i| matches!(i, slither_client::start_page::StartMenuIntent::Play { .. }))
        );
        game.play("ws://127.0.0.1:19491/slither", "Player", 44)
            .unwrap();
        pump(&mut game, 49.);
        effects(&mut game);
        game.opened(game.generation(), 50.).unwrap();
        effects(&mut game);
        game.receive(game.generation(), &bundle(&[challenge()]), 51.)
            .unwrap();
        effects(&mut game);
        game.receive(game.generation(), &bundle(&[setup(), snake(44)]), 52.)
            .unwrap();
        pictures(&mut game);
        pump(&mut game, 60.);
        login.observe_game(&game).unwrap();
        assert_eq!(game.phase(), Phase::Playing);
        assert_eq!(game.generation(), old_generation + 1);
        assert!(!game.ingame_menu_open());
        assert!(login.open_ingame_menu(&mut game, 61.).unwrap());
    }
}

#[test]
fn dismissal_retains_heading_cadence_and_samples_the_current_pointer() {
    let mut game = playing();
    let mut login = login(&game);
    game.pointer(480., 0., 960., 540., false).unwrap();
    assert_eq!(pump(&mut game, 100.).len(), 1);
    assert!(login.open_ingame_menu(&mut game, 101.).unwrap());
    let event = WindowEvent::CursorMoved {
        device_id: DeviceId::dummy(),
        position: PhysicalPosition::new(900., 450.),
    };
    let _ = login.egui.route_window_event(&event, 1., |_| ());
    escape(&mut login, &mut game, false);
    assert!(!game.ingame_menu_open());
    assert!(
        pump(&mut game, 110.).is_empty(),
        "closing does not reset the 33 ms heading gate"
    );
    let packets = pump(&mut game, 135.);
    assert_eq!(packets.len(), 1);
    assert!(packets[0].len() == 1 && packets[0][0] < 251);
    assert!(pump(&mut game, 136.).is_empty());
}

#[test]
fn esc_does_not_open_on_login_picker_settings_connecting_dead_or_terminal() {
    let mut ready = LocalGame::new(0.).unwrap();
    let mut start = NativeLogin::new("Player").unwrap();
    start.step(0., true, false).unwrap();
    assert!(!escape(&mut start, &mut ready, false));
    assert!(!start.egui.initialized());
    start.picker.open();
    assert!(!escape(&mut start, &mut ready, false));
    let mut start = NativeLogin::new("Player").unwrap();
    start.step(0., true, false).unwrap();
    start.open_settings(true);
    assert!(
        !start
            .ingame_key(
                &mut ready,
                KeyInput {
                    logical: &NativeKey::Named(NamedKey::Escape),
                    physical: PhysicalKey::Code(KeyCode::Escape),
                    text: None,
                    state: ElementState::Pressed,
                    repeat: false,
                },
                30.
            )
            .unwrap()
    );
    assert!(start.state.settings.visible());
    let mut connecting = negotiating();
    let mut start = login(&connecting);
    assert!(!escape(&mut start, &mut connecting, false));
    let mut opening = LocalGame::new(0.).unwrap();
    opening
        .play("ws://127.0.0.1:19491/slither", "Player", 44)
        .unwrap();
    assert!(!opening.open_ingame_menu(1.).unwrap());
    pump(&mut opening, 1.);
    effects(&mut opening);
    opening.opened(opening.generation(), 2.).unwrap();
    effects(&mut opening);
    assert_eq!(opening.phase(), Phase::Challenge);
    assert!(!opening.open_ingame_menu(3.).unwrap());
    for state in [
        Phase::Dead,
        Phase::Victory,
        Phase::Disconnected,
        Phase::Failed,
    ] {
        let mut game = playing();
        let mut start = login(&game);
        match state {
            Phase::Dead | Phase::Victory => game
                .receive(
                    game.generation(),
                    &[b'v', u8::from(state == Phase::Victory)],
                    30.,
                )
                .unwrap(),
            Phase::Disconnected => game.stop(),
            Phase::Failed => game.fail("authored test"),
            _ => unreachable!(),
        }
        start.observe_game(&game).unwrap();
        assert!(!escape(&mut start, &mut game, false));
        assert!(!game.ingame_menu_open());
        assert!(!start.egui.initialized());
    }
}
