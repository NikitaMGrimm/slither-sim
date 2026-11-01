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
    assert_eq!(login.egui.active(), login.state.view(true).visible);
    release_escape(&mut login, &mut game);
    escape(&mut login, &mut game, false);
    release_escape(&mut login, &mut game);
    click(&mut login, &mut game, pos2(2., 2.));
    assert!(!game.ingame_menu_open());
    assert_eq!(game.phase(), Phase::Playing);
    assert_eq!(login.egui.active(), login.state.view(true).visible);
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
    for now in (600..1000).step_by(16) {
        pump(&mut game, now as f64);
        login.observe_game(&game).unwrap();
        login.step(now as f64, false, true).unwrap();
    }
    assert!(!login.egui.active(), "{:?}", login.state.view(false));
    let initial_passes = login.egui.context().unwrap().cumulative_pass_nr();
    for now in 1001..1010 {
        login.step(f64::from(now), false, true).unwrap();
        login.prepare_ingame(&mut game, [960, 540], 1.).unwrap();
    }
    assert_eq!(
        login.egui.context().unwrap().cumulative_pass_nr(),
        initial_passes
    );
    escape(&mut login, &mut game, false);
    release_escape(&mut login, &mut game);
    escape(&mut login, &mut game, false);
    let passes = login.egui.context().unwrap().cumulative_pass_nr();
    for now in 1011..1080 {
        login.step(f64::from(now), false, true).unwrap();
        login.prepare_ingame(&mut game, [960, 540], 1.).unwrap();
    }
    assert_eq!(login.egui.context().unwrap().cumulative_pass_nr(), passes);
}

#[test]
fn retained_login_egui_input_keeps_enter_during_socket_close() {
    let mut game = playing();
    let mut login = login(&game);
    assert!(login.egui.active());
    login.cancel_game(&mut game);
    assert!(game.stopping());
    login.settings_input(RawInput {
        events: vec![
            key_event(Key::Enter, true, false),
            key_event(Key::Enter, false, false),
        ],
        ..Default::default()
    });
    let close = game.effect().unwrap();
    assert_eq!(close.kind, EffectKind::Close);
    game.acknowledge(close.generation, close.id, true).unwrap();
    pump(&mut game, 40.);
    login.observe_game(&game).unwrap();
    let intents = login.step(40., game.can_play(), false).unwrap().0;
    assert!(matches!(
        intents.as_slice(),
        [slither_client::start_page::StartMenuIntent::Play { .. }]
    ));
    assert!(login.step(48., false, false).unwrap().0.is_empty());
}

#[test]
fn outgoing_login_keeps_drawing_while_gameplay_owns_keys_pointer_zoom_and_esc() {
    use slither_client::{
        login_controls::PLAY,
        menu_input::{GameplayKey, HostEditEffect},
    };
    let mut game = playing();
    let mut login = login(&game);
    login.prepare([960, 540], false, None).unwrap();
    assert!(login.egui.active());
    assert!(!login.egui_owns_input());
    let result = login.key(
        KeyInput {
            logical: &NativeKey::Named(NamedKey::ArrowLeft),
            physical: PhysicalKey::Code(KeyCode::ArrowLeft),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
        },
        false,
    );
    assert!(result.effects.contains(&HostEditEffect::Gameplay {
        key: GameplayKey::Left,
        pressed: true
    }));
    assert!(!login.intercepts_pointer([0., 0.]));
    let r = login
        .login_ui
        .controls
        .iter()
        .find(|c| c.0 == PLAY)
        .unwrap()
        .1;
    assert!(!login.intercepts_pointer([r.center().x as f64, r.center().y as f64]));
    assert!(login.wheel_zoom(&mut game, -1.));
    assert!(escape(&mut login, &mut game, false));
    assert!(game.ingame_menu_open());
    assert!(login.egui_owns_input());
}

#[test]
fn clicking_fading_play_reaches_gameplay_without_queuing_another_login() {
    let mut game = playing();
    let mut login = login(&game);
    login.prepare([960, 540], false, None).unwrap();
    let view = login.state.view(false);
    assert_eq!(view.phase, MenuPhase::Outgoing);
    assert!(view.visible && view.play_disabled);
    let r = login
        .login_ui
        .controls
        .iter()
        .find(|c| c.0 == slither_client::login_controls::PLAY)
        .unwrap()
        .1;
    let point = [r.center().x as f64, r.center().y as f64];
    let generation = game.generation();
    // Exercise the desktop callback's menu-or-world click boundary.
    if login.intercepts_pointer(point) {
        login.pointer(point, Some(true), false);
        game.mouse_up();
    } else {
        slither_wgpu_slice::world_resolution::WorldWindow {
            physical: [960, 540],
            dpr: 1.,
            requested: login.state.settings.preferences().resolution,
        }
        .pointer(&mut game, point, true)
        .unwrap();
    }
    assert!(
        pump(&mut game, 100.).contains(&vec![253]),
        "outgoing login swallowed the boost click"
    );
    game.mouse_up();
    assert!(pump(&mut game, 180.).contains(&vec![254]));
    assert!(login.step(28., false, true).unwrap().0.is_empty());
    assert_eq!(login.state.view(false).phase, MenuPhase::Outgoing);
    assert_eq!(game.generation(), generation);
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
    assert_eq!(
        login.egui.active(),
        login.state.view(true).visible || login.state.view(true).spinner_visible
    );
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
        assert_eq!(game.phase(), Phase::Playing);
        assert!(game.stopping());
        let close = game.effect().expect("existing teardown queues Close");
        assert_eq!(close.kind, EffectKind::Close);
        game.acknowledge(close.generation, close.id, true).unwrap();
        assert_eq!(game.phase(), Phase::Disconnected);
        pump(&mut game, 40.);
        login.observe_game(&game).unwrap();
        login.step(40., game.can_play(), false).unwrap();
        assert_eq!(login.state.view(true).phase, MenuPhase::Login);
        assert!(login.egui.active());
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
    assert!(start.egui.initialized());
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
        assert!(start.egui.initialized());
    }
}

#[test]
fn unadmitted_playing_escape_uses_the_same_pending_close_and_retains_replay() {
    let mut game = negotiating();
    game.receive(game.generation(), &bundle(&[setup()]), 3.)
        .unwrap();
    pictures(&mut game);
    pump(&mut game, 20.);
    assert_eq!(game.phase(), Phase::Playing);
    assert!(game.client().control_view().own.is_none());
    let mut login = login(&game);
    assert!(!escape(&mut login, &mut game, false));
    game.begin_redraw(25., [960, 540], 1.).unwrap();
    let retained = game.take_frame().unwrap();
    game.retry_frame(retained).unwrap();
    login.cancel_game(&mut game); // The desktop's unconsumed Esc fallback.
    assert!(game.stopping());
    let close = game.effect().unwrap();
    assert_eq!(close.kind, EffectKind::Close);
    assert!(!game.can_play());
    for now in [30., 40., 50.] {
        game.begin_redraw(now, [960, 540], 1.).unwrap();
        assert!(
            game.take_frame().is_some(),
            "pending close must not block presentation"
        );
        game.presented(true).unwrap();
        assert_eq!(game.effect().unwrap().id, close.id);
    }
    let enter = NativeKey::Named(NamedKey::Enter);
    assert!(
        login
            .ingame_key(
                &mut game,
                KeyInput {
                    logical: &enter,
                    physical: PhysicalKey::Code(KeyCode::Enter),
                    text: None,
                    state: ElementState::Pressed,
                    repeat: false,
                },
                51.
            )
            .unwrap()
    );
    game.acknowledge(close.generation, close.id, true).unwrap();
    login.observe_game(&game).unwrap();
    let intents = login.step(60., game.can_play(), false).unwrap().0;
    assert_eq!(
        intents
            .iter()
            .filter(|i| matches!(i, slither_client::start_page::StartMenuIntent::Play { .. }))
            .count(),
        1
    );
    assert!(!game.stopping());
}

#[test]
fn replay_during_close_waits_for_the_existing_death_return_admission() {
    let mut game = playing();
    let mut login = login(&game);
    game.receive(game.generation(), &[b'v', 0], 30.).unwrap();
    login.observe_game(&game).unwrap();
    login.cancel_game(&mut game);
    let enter = NativeKey::Named(NamedKey::Enter);
    assert!(
        login
            .ingame_key(
                &mut game,
                KeyInput {
                    logical: &enter,
                    physical: PhysicalKey::Code(KeyCode::Enter),
                    text: None,
                    state: ElementState::Pressed,
                    repeat: false,
                },
                31.
            )
            .unwrap()
    );
    let close = game.effect().unwrap();
    game.acknowledge(close.generation, close.id, true).unwrap();
    login.observe_game(&game).unwrap();
    assert!(login.state.view(true).play_disabled);
    assert!(
        login
            .step(40., game.can_play(), false)
            .unwrap()
            .0
            .iter()
            .all(|i| !matches!(i, slither_client::start_page::StartMenuIntent::Play { .. }))
    );
    for now in (40..=4000).step_by(40) {
        pump(&mut game, f64::from(now));
        login.observe_game(&game).unwrap();
    }
    assert_eq!(
        login
            .step(4001., game.can_play(), false)
            .unwrap()
            .0
            .iter()
            .filter(|i| matches!(i, slither_client::start_page::StartMenuIntent::Play { .. }))
            .count(),
        1
    );
}

#[test]
fn loopback_leave_waits_for_normal_close_ack_keeps_frames_and_starts_second_game() {
    use slither_wgpu_slice::native::{Event as NetworkEvent, Transport};
    use std::{
        net::TcpListener,
        sync::mpsc,
        thread,
        time::{Duration, Instant},
    };
    use tungstenite::{Message, protocol::frame::coding::CloseCode};
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let endpoint = format!("ws://{}/slither", listener.local_addr().unwrap());
    let (closed_tx, closed_rx) = mpsc::channel();
    let (ack_tx, ack_rx) = mpsc::channel();
    let peer = thread::spawn(move || {
        for round in 0..2 {
            let deadline = Instant::now() + Duration::from_secs(10);
            let stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline, "accept deadline");
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => panic!("accept: {e}"),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut socket = tungstenite::accept(stream).unwrap();
            assert!(matches!(socket.read().unwrap(), Message::Binary(b) if b.as_ref() == [1]));
            assert!(matches!(socket.read().unwrap(), Message::Binary(b) if b.as_ref() == [99, 0]));
            socket
                .send(Message::Binary(bundle(&[challenge()]).into()))
                .unwrap();
            assert!(matches!(socket.read().unwrap(), Message::Binary(_)));
            assert!(matches!(socket.read().unwrap(), Message::Binary(b) if b[0] == b's'));
            socket
                .send(Message::Binary(bundle(&[setup(), snake(44)]).into()))
                .unwrap();
            loop {
                match socket.read().unwrap() {
                    Message::Close(Some(frame)) => {
                        assert_eq!(frame.code, CloseCode::Normal);
                        closed_tx.send(round).unwrap();
                        ack_rx.recv_timeout(Duration::from_secs(10)).unwrap();
                        match socket.flush() {
                            Ok(()) | Err(tungstenite::Error::ConnectionClosed) => {}
                            other => panic!("close acknowledgement: {other:?}"),
                        }
                        break;
                    }
                    Message::Binary(b) if b.as_ref() == [251] => {
                        socket.send(Message::Binary(vec![b'p'].into())).unwrap();
                    }
                    Message::Binary(_) => {}
                    other => panic!("unexpected peer message: {other:?}"),
                }
            }
        }
    });
    fn drain(game: &mut LocalGame, transport: &mut Transport, now: f64) {
        for _ in 0..64 {
            let Some(effect) = game.effect() else { return };
            if matches!(effect.kind, EffectKind::Connect(_)) {
                game.construct(effect.generation, effect.id, now).unwrap();
            }
            if !transport
                .execute_pending(&effect, std::time::Instant::now())
                .unwrap()
            {
                return;
            }
            game.acknowledge(effect.generation, effect.id, true)
                .unwrap();
        }
        panic!("effect budget");
    }
    fn tick(
        game: &mut LocalGame,
        transport: &mut Transport,
        started: Instant,
        close_acks: &mut Vec<u32>,
    ) -> bool {
        let now = started.elapsed().as_secs_f64() * 1000.;
        for event in transport.poll() {
            match event {
                NetworkEvent::Opened(g) => game.opened(g, now).unwrap(),
                NetworkEvent::Binary(g, bytes) => game.receive(g, &bytes, now).unwrap(),
                NetworkEvent::Closed(g) => {
                    close_acks.push(g);
                    game.closed(g);
                }
                NetworkEvent::Failed(_, e) => panic!("transport: {e}"),
            }
            drain(game, transport, now);
        }
        drain(game, transport, now);
        game.begin_redraw(now, [960, 540], 1.).unwrap();
        drain(game, transport, now);
        let presented = game.take_frame().is_some();
        if presented {
            game.presented(true).unwrap();
        }
        drain(game, transport, now);
        presented
    }
    let started = Instant::now();
    let mut game = LocalGame::new(0.).unwrap();
    let mut transport = Transport::default();
    let mut close_acks = Vec::new();
    game.play(&endpoint, "Player", 44).unwrap();
    for round in 0..2 {
        let deadline = Instant::now() + Duration::from_secs(10);
        while game.phase() != Phase::Playing || game.client().control_view().own.is_none() {
            assert!(Instant::now() < deadline, "playing deadline");
            tick(&mut game, &mut transport, started, &mut close_acks);
            thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(game.generation(), round + 1);
        let mut login = login(&game);
        escape(&mut login, &mut game, false);
        release_escape(&mut login, &mut game);
        input(
            &mut login,
            &mut game,
            vec![key_event(Key::Enter, true, false)],
        );
        assert!(game.stopping());
        assert!(!game.can_play());
        // Execute the actual Leave/Close effect, while retaining transport ownership.
        drain(
            &mut game,
            &mut transport,
            started.elapsed().as_secs_f64() * 1000.,
        );
        assert_eq!(
            closed_rx.recv_timeout(Duration::from_secs(10)).unwrap(),
            round
        );
        for _ in 0..3 {
            assert!(tick(&mut game, &mut transport, started, &mut close_acks));
            login.observe_game(&game).unwrap();
            assert_ne!(login.state.view(false).phase, MenuPhase::Login);
            assert!(!game.can_play());
        }
        if round == 0 {
            let enter = NativeKey::Named(NamedKey::Enter);
            assert!(
                login
                    .ingame_key(
                        &mut game,
                        KeyInput {
                            logical: &enter,
                            physical: PhysicalKey::Code(KeyCode::Enter),
                            text: None,
                            state: ElementState::Pressed,
                            repeat: false,
                        },
                        100.
                    )
                    .unwrap()
            );
        }
        ack_tx.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !game.can_play() {
            assert!(Instant::now() < deadline, "close completion deadline");
            tick(&mut game, &mut transport, started, &mut close_acks);
            thread::sleep(Duration::from_millis(1));
        }
        assert!(transport.close_deadline().is_none());
        assert_eq!(
            close_acks,
            (1..=round + 1).collect::<Vec<_>>(),
            "deadline fallback is not a completed handshake"
        );
        login.observe_game(&game).unwrap();
        if round == 0 {
            assert!(
                login
                    .step(110., true, false)
                    .unwrap()
                    .0
                    .iter()
                    .any(|i| matches!(i, slither_client::start_page::StartMenuIntent::Play { .. }))
            );
            game.play(&endpoint, "Player", 44).unwrap();
        }
    }
    peer.join().unwrap();
}
