#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{
    login_controls::{CHANGE_SKIN, NICKNAME, PLAY, SERVER, SETTINGS},
    menu_input::{EditIntent, HostEditEffect},
    menu_layout::MenuElement as E,
    start_page::{MenuPhase, StartMenuIntent},
};
use slither_wgpu_slice::login_native::NativeLogin;

fn settled(nickname: &str) -> NativeLogin {
    let mut login = NativeLogin::new(nickname).unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    for _ in 0..2 {
        login.prepare([960, 540], true, None).unwrap();
    }
    login
}

#[test]
fn connection_errors_replace_tips_without_covering_controls_and_clear_on_next_attempt() {
    use slither_wgpu_slice::game::LocalGame;
    let error = "WebSocket protocol error: Connection reset without closing handshake";
    for (extent, dpr) in [([360, 640], 1.), ([960, 540], 1.), ([1920, 1080], 2.)] {
        let mut login = settled("Player");
        let mut game = LocalGame::new(0.).unwrap();
        game.fail(error);
        login.observe_game(&game).unwrap();
        for _ in 0..2 {
            login.prepare_host(extent, dpr, true, None).unwrap();
        }
        let shape = login.egui.prepared_shapes().iter().find(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == error)
        }).expect("visible connection error");
        let logo = login.layout.viewport_rect(E::Logo);
        let nickname = login.layout.viewport_rect(E::NicknameHolder);
        assert!(f64::from(shape.clip_rect.top()) >= logo.y + logo.height);
        assert!(f64::from(shape.clip_rect.bottom()) <= nickname.y);
        assert!(shape.clip_rect.left() >= 0.);
        assert!(shape.clip_rect.right() <= extent[0] as f32 / dpr as f32);
        game.play("ws://127.0.0.1:19491/slither", "Player", 0)
            .unwrap();
        login.observe_game(&game).unwrap();
        login.prepare_host(extent, dpr, true, None).unwrap();
        assert!(!login.egui.prepared_shapes().iter().any(|shape| {
            matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == error)
        }));
    }
}

#[test]
fn connection_error_clears_before_acquisition_and_new_failure_replaces_it() {
    use slither_wgpu_slice::game::LocalGame;
    let mut login = settled("Player");
    let mut game = LocalGame::new(0.).unwrap();
    game.fail("Previous connection failed");
    login.observe_game(&game).unwrap();
    login.begin_play_attempt(&mut game).unwrap();
    assert!(game.failure.is_none());
    login.prepare([960, 540], true, None).unwrap();
    assert!(!login.egui.prepared_shapes().iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "Previous connection failed")
    }));
    game.fail("New profile acquisition failed");
    login.observe_game(&game).unwrap();
    assert_eq!(
        game.failure.as_deref(),
        Some("New profile acquisition failed")
    );
    login.prepare([960, 540], true, None).unwrap();
    assert!(login.egui.prepared_shapes().iter().any(|shape| {
        matches!(&shape.shape, egui::Shape::Text(text) if text.galley.job.text == "New profile acquisition failed")
    }));
}

#[test]
fn complete_bounded_connection_error_is_visible_without_hover_after_play() {
    use slither_client::menu_layout::{MenuLayout, MenuMetrics};
    let mut login = settled("Player");
    let mut view = login.state.view(true);
    view.tips_visible = false;
    let error = "Connection reset while acquiring a profile: ".repeat(6);
    let bounded: String = error.chars().take(256).collect();
    let ctx = login.egui.context().unwrap().clone();
    for (width, height, dpr) in [
        (280., 480., 1.),
        (360., 640., 1.),
        (960., 540., 1.),
        (960., 540., 2.),
    ] {
        let layout = MenuLayout::new(
            width,
            height,
            0.,
            false,
            MenuMetrics {
                last_length_line_height: 18.,
                nickname_baseline_space: 0.,
                padded_play_label_width: 80.,
            },
        )
        .unwrap();
        let mut output = ctx.run_pass(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width as f32, height as f32),
                )),
                viewport_id: egui::ViewportId::ROOT,
                viewports: [(
                    egui::ViewportId::ROOT,
                    egui::ViewportInfo {
                        native_pixels_per_point: Some(dpr),
                        ..Default::default()
                    },
                )]
                .into_iter()
                .collect(),
                ..Default::default()
            },
            |ctx| {
                login
                    .login_ui
                    .show(ctx, &view, &layout, true, Some(&error))
                    .unwrap();
            },
        );
        let shape = output
            .shapes
            .iter()
            .find(|shape| {
                matches!(&shape.shape,
            egui::Shape::Text(text) if text.galley.job.text == bounded)
            })
            .expect("complete bounded message");
        let egui::Shape::Text(text) = &shape.shape else {
            unreachable!()
        };
        let bounds = text.galley.rect.translate(text.pos.to_vec2());
        assert!(
            shape.clip_rect.contains_rect(bounds),
            "unclipped text at {width}x{height}, DPR {dpr}: {bounds:?}"
        );
        let logo = layout.viewport_rect(E::Logo);
        let nickname = layout.viewport_rect(E::NicknameHolder);
        assert!(bounds.top() as f64 >= logo.y + logo.height);
        assert!(bounds.bottom() as f64 <= nickname.y);
        // This CPU-only pass deliberately has no GPU texture owner.
        output.textures_delta.clear();
    }
}
fn input(login: &mut NativeLogin, event: egui::Event) {
    login.settings_input(egui::RawInput {
        events: vec![event],
        ..Default::default()
    });
}
fn press(login: &mut NativeLogin, key: egui::Key, modifiers: egui::Modifiers) {
    for pressed in [true, false] {
        input(
            login,
            egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            },
        );
    }
}
fn select_all(login: &mut NativeLogin) {
    press(
        login,
        egui::Key::A,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
    );
}

#[test]
fn egui_focus_is_unique_and_every_control_is_reachable_in_both_directions() {
    for area in [[360., 640.], [960., 540.], [1920., 1080.]] {
        for dpr in [1., 1.5, 2.] {
            let extent = area.map(|n| (n * dpr) as u32);
            let mut login = settled("Player");
            login.prepare_host(extent, dpr, true, None).unwrap();
            login.prepare_host(extent, dpr, true, None).unwrap();
            assert_eq!(login.focused(), Some(NICKNAME));
            for (modifiers, order) in [
                (
                    egui::Modifiers::NONE,
                    [PLAY, CHANGE_SKIN, SERVER, SETTINGS, NICKNAME],
                ),
                (
                    egui::Modifiers::SHIFT,
                    [SETTINGS, SERVER, CHANGE_SKIN, PLAY, NICKNAME],
                ),
            ] {
                for id in order {
                    press(&mut login, egui::Key::Tab, modifiers);
                    assert_eq!(login.focused(), Some(id), "{area:?}, DPR {dpr}");
                    assert_eq!(login.login_ui.controls.iter().filter(|c| c.2).count(), 1);
                }
            }
        }
    }
}

#[test]
fn egui_controls_and_ime_cursor_stay_inside_small_and_large_windows() {
    for area in [[320., 240.], [360., 640.], [960., 540.], [1920., 1080.]] {
        for dpr in [1., 1.5, 2.] {
            let mut login = settled("WWWWWWWWWWWWWWWWWWWWWWWW");
            let extent = area.map(|n| (n * dpr) as u32);
            for _ in 0..2 {
                login.prepare_host(extent, dpr, true, None).unwrap();
            }
            let window = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(area[0] as f32, area[1] as f32),
            );
            for (_, r, _) in &login.login_ui.controls {
                assert!(
                    r.is_positive() && window.contains_rect(*r),
                    "{area:?} {dpr} {r:?}"
                );
            }
            let ime = login.egui.output().and_then(|o| o.ime).unwrap();
            assert!(window.contains_rect(ime.rect));
            let field = login
                .login_ui
                .controls
                .iter()
                .find(|c| c.0 == NICKNAME)
                .unwrap()
                .1;
            assert!(
                field.contains_rect(ime.cursor_rect),
                "{area:?} DPR {dpr}, IME {ime:?}"
            );
        }
    }
}

#[test]
fn nickname_uses_utf16_substitution_and_full_value_truncation_before_play() {
    for text in ["A😀\u{7f}\n日", "12345678901234567890123😀tail"] {
        let mut login = settled("Initial");
        select_all(&mut login);
        input(&mut login, egui::Event::Paste(text.into()));
        let expected = slither_client::start_page::normalize_nickname(text);
        assert_eq!(login.state.view(true).nickname, expected);
        press(&mut login, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(
            matches!(login.step(9616.,true,false).unwrap().0.as_slice(),[StartMenuIntent::Play{nickname,..}] if nickname==&expected)
        );
        assert!(login.step(9632., true, false).unwrap().0.is_empty());
    }
    let mut login = settled("123456789012345678901234");
    press(&mut login, egui::Key::Home, egui::Modifiers::NONE);
    input(&mut login, egui::Event::Text("AB".into()));
    assert_eq!(login.state.view(true).nickname, "AB1234567890123456789012");
}

#[test]
fn normalized_cjk_and_emoji_pastes_keep_both_caret_endpoints_within_text() {
    for text in ["日".repeat(32), "😀".repeat(32), "日😀".repeat(16)] {
        let mut login = settled("12345678901234567890123");
        press(&mut login, egui::Key::End, egui::Modifiers::NONE);
        input(&mut login, egui::Event::Paste(text));
        assert_eq!(login.state.view(true).nickname, "12345678901234567890123 ");
        let ctx = login.egui.context().unwrap();
        let state =
            egui::TextEdit::load_state(ctx, slither_wgpu_slice::login_egui::widget_id(NICKNAME))
                .unwrap();
        let range = state.cursor.char_range().unwrap();
        let chars = login.login_ui.nickname.chars().count();
        assert_eq!(chars, 24);
        assert!(range.primary.index.0 <= chars, "{range:?}");
        assert!(range.secondary.index.0 <= chars, "{range:?}");
    }
}

#[test]
fn sustained_ime_coalesces_before_redraw_and_commit_does_not_play() {
    let mut login = settled("Initial");
    select_all(&mut login);
    let context = login.egui.context().unwrap().clone();
    let passes = context.cumulative_pass_nr();
    for n in 0..1000 {
        input(
            &mut login,
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: format!("日{n}"),
                active_range_chars: Some(0..1),
            }),
        );
    }
    assert_eq!(context.cumulative_pass_nr(), passes);
    assert_eq!(login.state.view(true).nickname, "Initial");
    login.prepare([960, 540], true, None).unwrap();
    assert_eq!(context.cumulative_pass_nr(), passes + 1);
    assert_eq!(login.displayed_nickname(), "日999");
    press(&mut login, egui::Key::Tab, egui::Modifiers::NONE);
    assert_eq!(login.focused(), Some(NICKNAME));
    press(&mut login, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(login.state.view(true).phase, MenuPhase::Login);
    input(
        &mut login,
        egui::Event::Ime(egui::ImeEvent::Commit("A😀\u{7f}".into())),
    );
    assert!(!login.login_ui.composing);
    assert_eq!(login.state.view(true).nickname, "A  \u{7f}");
    assert_eq!(login.state.view(true).phase, MenuPhase::Login);
    press(&mut login, egui::Key::Enter, egui::Modifiers::NONE);
    assert!(
        matches!(login.step(9616.,true,false).unwrap().0.as_slice(),[StartMenuIntent::Play{nickname,..}] if nickname=="A  \u{7f}")
    );
}

#[test]
fn ime_cancellation_restores_committed_text_and_full_field_commit_uses_the_same_limit() {
    for cancel in [
        egui::Event::WindowFocused(false),
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: String::new(),
            active_range_chars: None,
        }),
    ] {
        let mut login = settled("Initial");
        select_all(&mut login);
        input(
            &mut login,
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "日".into(),
                active_range_chars: None,
            }),
        );
        login.prepare([960, 540], true, None).unwrap();
        input(&mut login, cancel);
        login.prepare([960, 540], true, None).unwrap();
        assert_eq!(login.state.view(true).nickname, "Initial");
        assert!(!login.login_ui.composing);
    }
    let mut login = settled("123456789012345678901234");
    press(&mut login, egui::Key::Home, egui::Modifiers::NONE);
    input(
        &mut login,
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "日".into(),
            active_range_chars: None,
        }),
    );
    input(
        &mut login,
        egui::Event::Ime(egui::ImeEvent::Commit("AB".into())),
    );
    assert_eq!(login.state.view(true).nickname, "AB1234567890123456789012");
}

#[test]
fn native_ime_disabled_restores_preedit_before_and_after_redraw() {
    for redraw in [false, true] {
        let mut login = settled("Initial");
        select_all(&mut login);
        input(
            &mut login,
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "候補".into(),
                active_range_chars: None,
            }),
        );
        if redraw {
            login.prepare([960, 540], true, None).unwrap();
        }
        assert!(login.login_ui.composing);
        let response = login.egui.window_event(
            &winit::event::WindowEvent::Ime(winit::event::Ime::Disabled),
            1.,
            || panic!("native Disabled must be handled before the window adapter"),
        );
        assert!(response.repaint);
        login.settings_input(egui::RawInput::default());
        assert!(!login.login_ui.composing);
        assert_eq!(login.displayed_nickname(), "Initial");
        login.prepare([960, 540], true, None).unwrap();
        assert_eq!(login.displayed_nickname(), "Initial");
        press(&mut login, egui::Key::Tab, egui::Modifiers::NONE);
        assert_eq!(login.focused(), Some(PLAY));
        press(&mut login, egui::Key::Enter, egui::Modifiers::NONE);
        assert!(matches!(
            login.step(9616., true, false).unwrap().0.as_slice(),
            [StartMenuIntent::Play { nickname, .. }] if nickname == "Initial"
        ));
    }
}

#[test]
fn timed_ime_candidates_never_enter_cancelled_or_committed_undo_history() {
    for commit in [false, true] {
        let mut login = settled("Before");
        select_all(&mut login);
        input(&mut login, egui::Event::Text("Committed".into()));
        login.step(11200., true, false).unwrap();
        login.prepare([960, 540], true, None).unwrap();
        select_all(&mut login);
        input(
            &mut login,
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "候補".into(),
                active_range_chars: None,
            }),
        );
        login.prepare([960, 540], true, None).unwrap();
        login.step(12800., true, false).unwrap();
        login.prepare([960, 540], true, None).unwrap();
        assert_eq!(login.displayed_nickname(), "候補");
        input(
            &mut login,
            if commit {
                egui::Event::Ime(egui::ImeEvent::Commit("Final".into()))
            } else {
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: String::new(),
                    active_range_chars: None,
                })
            },
        );
        login.prepare([960, 540], true, None).unwrap();
        assert_eq!(
            login.displayed_nickname(),
            if commit { "Final" } else { "Committed" }
        );
        let command = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        let mut saw_committed = !commit;
        for _ in 0..6 {
            press(&mut login, egui::Key::Z, command);
            let text = login.displayed_nickname();
            assert!(matches!(text.as_str(), "Before" | "Committed"), "{text:?}");
            saw_committed |= text == "Committed";
        }
        assert!(saw_committed);
        assert_eq!(login.displayed_nickname(), "Before");
        for _ in 0..6 {
            press(
                &mut login,
                egui::Key::Z,
                egui::Modifiers {
                    shift: true,
                    ..command
                },
            );
            let text = login.displayed_nickname();
            assert!(
                matches!(text.as_str(), "Before" | "Committed" | "Final"),
                "{text:?}"
            );
        }
        assert_eq!(
            login.displayed_nickname(),
            if commit { "Final" } else { "Committed" }
        );
    }
}

#[test]
fn ime_commit_after_undo_discards_old_redo_while_cancellation_preserves_it() {
    for commit in [false, true] {
        let mut login = settled("Before");
        let command = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        select_all(&mut login);
        input(&mut login, egui::Event::Text("Committed".into()));
        login.step(11200., true, false).unwrap();
        login.prepare([960, 540], true, None).unwrap();
        for _ in 0..6 {
            press(&mut login, egui::Key::Z, command);
        }
        assert_eq!(login.displayed_nickname(), "Before");
        input(
            &mut login,
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "候補".into(),
                active_range_chars: None,
            }),
        );
        login.prepare([960, 540], true, None).unwrap();
        login.step(12800., true, false).unwrap();
        login.prepare([960, 540], true, None).unwrap();
        input(
            &mut login,
            if commit {
                egui::Event::Ime(egui::ImeEvent::Commit("Final".into()))
            } else {
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: String::new(),
                    active_range_chars: None,
                })
            },
        );
        login.prepare([960, 540], true, None).unwrap();
        let committed = login.displayed_nickname();
        if commit {
            assert!(committed.contains("Final"));
        }
        for _ in 0..6 {
            press(
                &mut login,
                egui::Key::Z,
                egui::Modifiers {
                    shift: true,
                    ..command
                },
            );
        }
        assert_eq!(
            login.displayed_nickname(),
            if commit {
                committed
            } else {
                "Committed".into()
            }
        );
    }
}

#[test]
fn windows_process_enter_cannot_insert_payload_or_activate_until_key_up() {
    use slither_wgpu_slice::menu_input_native::KeyInput;
    use winit::{
        event::ElementState,
        keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    };
    let mut login = settled("Player");
    let logical = Key::Named(NamedKey::Process);
    let physical = PhysicalKey::Code(KeyCode::Enter);
    assert!(login.egui.process_key(KeyInput {
        logical: &logical,
        physical,
        text: Some("uncommitted"),
        state: ElementState::Pressed,
        repeat: false
    }));
    input(
        &mut login,
        egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        },
    );
    assert_eq!(login.state.view(true).phase, MenuPhase::Login);
    assert_eq!(login.state.view(true).nickname, "Player");
    assert!(login.egui.process_key(KeyInput {
        logical: &logical,
        physical,
        text: None,
        state: ElementState::Released,
        repeat: false
    }));
    login.prepare([960, 540], true, None).unwrap();
    press(&mut login, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(login.state.view(true).phase, MenuPhase::Connecting);
}

#[test]
fn egui_nickname_refocus_keeps_exclusive_ime_ownership() {
    let mut login = settled("Initial");
    for focused in [false, true] {
        input(&mut login, egui::Event::WindowFocused(focused));
        let result = login.event(&winit::event::WindowEvent::Focused(focused), true);
        assert!(
            !result
                .effects
                .iter()
                .any(|e| matches!(e, HostEditEffect::ImeAllowed(_)))
        );
    }
    login.prepare([960, 540], true, None).unwrap();
    assert!(login.egui.output().unwrap().ime.is_some());
    press(&mut login, egui::Key::Escape, egui::Modifiers::NONE);
    assert_eq!(login.focused(), Some(NICKNAME));
    assert!(!login.take_cancel_request());
}

#[test]
fn pointer_and_keyboard_entry_points_keep_the_legacy_screens() {
    for keyboard in [false, true] {
        for id in [PLAY, SETTINGS, SERVER, CHANGE_SKIN] {
            let mut login = settled("Player");
            if keyboard {
                login.focus(id);
                press(&mut login, egui::Key::Enter, egui::Modifiers::NONE);
            } else {
                let r = login
                    .login_ui
                    .controls
                    .iter()
                    .find(|c| c.0 == id)
                    .unwrap()
                    .1;
                let p = [r.center().x as f64, r.center().y as f64];
                login.pointer(p, Some(true), true);
                login.pointer(p, Some(false), true);
            }
            assert_eq!(login.state.settings.is_open(), id == SETTINGS);
            assert_eq!(login.picker.policy.visible(), id == SERVER);
            assert_eq!(login.studio.state.visible(), id == CHANGE_SKIN);
            assert_eq!(
                login.state.view(true).phase == MenuPhase::Connecting,
                id == PLAY
            );
        }
    }
}

#[test]
fn nickname_persists_at_the_existing_skin_save_boundary() {
    use slither_wgpu_slice::menu_storage::MenuStorage;
    let directory =
        std::env::temp_dir().join(format!("egui-login-preferences-{}", std::process::id()));
    assert!(!directory.exists());
    let mut login =
        NativeLogin::with_preferences(Default::default(), MenuStorage::at(Some(directory.clone())))
            .unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    for _ in 0..2 {
        login.prepare([960, 540], true, None).unwrap();
    }
    select_all(&mut login);
    input(&mut login, egui::Event::Paste("Saved😀".into()));
    assert!(
        !directory.exists(),
        "editing alone never writes preferences"
    );
    login.open_studio();
    login.step(9830., true, false).unwrap();
    login.studio.activate_with_random(
        slither_client::skin_studio::SAVE,
        &login.state.view(true).nickname,
        &mut || 7,
    );
    let mut storage = MenuStorage::at(Some(directory.clone()));
    assert_eq!(storage.restore().record.nickname, "Saved  ");
    std::fs::remove_file(directory.join("menu.v2")).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn failed_preference_save_keeps_the_run_only_message_on_login() {
    let mut login = settled("Memory");
    login.open_studio();
    login.step(9830., true, false).unwrap();
    login
        .studio
        .activate_with_random(slither_client::skin_studio::SAVE, "Memory", &mut || 7);
    login.step(10060., true, false).unwrap();
    login.sync_return_focus(true);
    for _ in 0..2 {
        login.prepare([960, 540], true, None).unwrap();
    }
    assert_eq!(login.studio.status, "Saved for this run");
    assert!(login.egui.prepared_shapes().iter().any(|shape| matches!(&shape.shape,egui::Shape::Text(text) if text.galley.job.text=="Saved for this run")));
}

#[test]
fn held_and_refocus_enter_never_queue_a_second_play() {
    use winit::{
        event::ElementState,
        keyboard::{Key, NamedKey},
    };
    for synthetic in [false, true] {
        let mut login = settled("Once");
        if synthetic {
            login
                .egui
                .synthetic_key(&Key::Named(NamedKey::Enter), ElementState::Pressed);
        }
        for _ in 0..10 {
            input(
                &mut login,
                egui::Event::Key {
                    key: egui::Key::Enter,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                },
            );
        }
        let intents = login.step(9616., true, false).unwrap().0;
        assert_eq!(
            intents
                .iter()
                .filter(|i| matches!(i, StartMenuIntent::Play { .. }))
                .count(),
            usize::from(!synthetic)
        );
        assert!(login.step(9632., true, false).unwrap().0.is_empty());
    }
}

#[test]
fn clipboard_commands_survive_input_passes_until_platform_output() {
    let mut login = settled("Copy me");
    select_all(&mut login);
    input(&mut login, egui::Event::Copy);
    login.prepare([960, 540], true, None).unwrap();
    assert!(
        login.egui.output().unwrap().commands.iter().any(
            |command| matches!(command,egui::OutputCommand::CopyText(text) if text=="Copy me")
        )
    );
    input(&mut login, egui::Event::Cut);
    assert_eq!(login.state.view(true).nickname, "");
    input(&mut login, egui::Event::Paste("Restored".into()));
    assert_eq!(login.state.view(true).nickname, "Restored");
}

#[test]
fn callback_admission_changes_do_not_queue_play_or_retarget_the_press() {
    let mut login = settled("Player");
    login.set_admission(false);
    press(&mut login, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(login.state.view(false).phase, MenuPhase::Login);
    assert!(!login.studio.state.visible());
    assert!(!login.state.settings.visible());
    assert!(!login.picker.policy.visible());
    login.set_admission(true);
    login.focus(NICKNAME);
    press(&mut login, egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(login.state.view(true).phase, MenuPhase::Connecting);
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
            assert_eq!(
                login.focused(),
                Some(slither_client::login_controls::NICKNAME)
            );
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
    login.prepare_host([1920, 1080], 2., true, None).unwrap();
    fn shows(shape: &egui::Shape, text: &str) -> bool {
        match shape {
            egui::Shape::Text(shape) => shape.galley.job.text.contains(text),
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| shows(shape, text)),
            _ => false,
        }
    }
    assert!(login.egui.prepared_shapes().iter().any(|shape| shows(
        &shape.shape,
        &format!("Your final length was {}", captured.number)
    )));
    assert!(
        login
            .egui
            .prepared_shapes()
            .iter()
            .any(|shape| shows(&shape.shape, "Play Again"))
    );
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
        assert_eq!(
            login.focused(),
            Some(slither_client::login_controls::NICKNAME)
        );
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
    // Empty black worlds have no textures; inject the resource collision explicitly.
    picture.resources.push(menu.resources[0].clone());
    menu.resources[0].asset.revision += 1;
    assert!(menu.append_to(&mut picture).is_err());
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

/// Login text egui paints: string, origin and galley bounds. The cycling tip
/// changes on its own clock, so its row is left out, and so is the text of
/// an egui studio drawn above the login.
fn login_text(login: &NativeLogin) -> Vec<(String, egui::Pos2, egui::Rect)> {
    let tips = login.layout.viewport_rect(E::Tips);
    let studio = studio_strings(login);
    egui_text(login)
        .into_iter()
        .filter(|t| {
            !(tips.y..tips.y + tips.height).contains(&f64::from(t.1.y)) && !studio.contains(&t.0)
        })
        .collect()
}
fn egui_text(login: &NativeLogin) -> Vec<(String, egui::Pos2, egui::Rect)> {
    login
        .egui
        .prepared_shapes()
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((t.galley.job.text.clone(), t.pos, t.galley.rect)),
            _ => None,
        })
        .collect()
}
/// Every string the visible studio panel can paint.
fn studio_strings(login: &NativeLogin) -> Vec<String> {
    use slither_client::menu_controls::ControlKind as K;
    if !login.studio.state.visible() {
        return Vec::new();
    }
    let mut strings: Vec<String> = login
        .studio_panel
        .controls
        .iter()
        .filter_map(|c| match &c.kind {
            K::Label { text } => Some(text.clone()),
            K::Button { label } | K::SectionTab { label, .. } => Some(label.clone()),
            K::Swatch {
                count: Some(count), ..
            } => Some(count.to_string()),
            K::CosmeticCell { cosmetic: None, .. } => Some("None".into()),
            _ => None,
        })
        .collect();
    strings.retain(|s| !s.is_empty());
    strings
}
/// Studio text egui paints, keyed like login text.
fn studio_text(login: &NativeLogin) -> Vec<(String, egui::Pos2, egui::Rect)> {
    let studio = studio_strings(login);
    let tips = login.layout.viewport_rect(E::Tips);
    egui_text(login)
        .into_iter()
        .filter(|t| {
            studio.contains(&t.0) && !(tips.y..tips.y + tips.height).contains(&f64::from(t.1.y))
        })
        .collect()
}
/// While the login is meaningfully visible the whole settled login is on
/// egui at its settled place; once faded out, any text left is still there.
fn assert_stable(
    login: &NativeLogin,
    settled: &[(String, egui::Pos2, egui::Rect)],
    visible: bool,
    at: &str,
) {
    let text = login_text(login);
    if visible {
        assert_eq!(text, settled, "{at}");
    } else {
        assert!(text.iter().all(|t| settled.contains(t)), "{at}: {text:?}");
    }
}

/// The skin studio and the login below it are both egui through entry,
/// the fades and exit: the login's links and captions never change
/// renderer, so they never shift, and the studio's own text holds still
/// while it fades.
#[test]
fn login_text_stays_put_while_the_skin_studio_enters_and_leaves() {
    use slither_client::skin_studio::CANCEL;
    for dpr in [1., 1.5] {
        let mut login = settled("Player");
        let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
        login.prepare_host(extent, dpr, true, None).unwrap();
        let settled = login_text(&login);
        for caption in ["Change Skin", "Choose Server", "Settings"] {
            assert!(settled.iter().any(|t| t.0 == caption), "{dpr}: {settled:?}");
        }
        let r = login
            .login_ui
            .controls
            .iter()
            .find(|c| c.0 == CHANGE_SKIN)
            .unwrap()
            .1;
        let p = [r.center().x as f64, r.center().y as f64];
        login.pointer(p, Some(true), true);
        login.pointer(p, Some(false), true);
        assert!(login.studio.state.visible());
        let mut studio = Vec::new();
        let mut frame = |login: &mut NativeLogin, now: u32, phase: &str| {
            login.step(f64::from(now), true, false).unwrap();
            login.prepare_host(extent, dpr, true, None).unwrap();
            let visible = 1. - login.studio.state.alpha() > 0.05;
            let at = format!("{dpr} {phase} {now}");
            assert_stable(login, &settled, visible, &at);
            if login.studio.state.visible() {
                studio.push((at, studio_text(login)));
            }
        };
        for now in (9616..=10400).step_by(16) {
            frame(&mut login, now, "enter");
        }
        assert!(login.studio.state.alpha() >= 1.);
        let open = studio_text(&login);
        for caption in [
            "Skin studio",
            "Built-in",
            "Previous",
            "Next",
            "Cancel",
            "Save",
        ] {
            assert!(open.iter().any(|t| t.0 == caption), "{dpr}: {open:?}");
        }
        let cancel = login
            .studio_panel
            .resolve([960., 720.])
            .unwrap()
            .into_iter()
            .find(|c| c.control.id == CANCEL)
            .unwrap()
            .rect;
        let p = [cancel.x + cancel.width / 2., cancel.y + cancel.height / 2.];
        login.pointer(p, Some(true), true);
        login.pointer(p, Some(false), true);
        for now in (10416..=11200).step_by(16) {
            frame(&mut login, now, "leave");
        }
        assert!(!login.studio.state.visible());
        frame(&mut login, 11216, "closed");
        // The studio's text sits where the open studio paints it on every
        // frame of its entry and exit fades.
        for (at, text) in &studio {
            assert!(text.iter().all(|t| open.contains(t)), "{at}: {text:?}");
        }
        assert!(studio.iter().filter(|s| s.1 == open).count() > 50);
        // egui owns input again; the host's post-present sync returns focus
        // to the opener.
        assert!(login.egui_owns_input());
        login.sync_return_focus(true);
        assert_eq!(login.focused(), Some(CHANGE_SKIN));
    }
}

/// The champion editor still draws on the old toolkit. The returning login
/// below it is on egui throughout, so the final length stays put when the
/// editor exits and the ordinary controls return.
#[test]
fn login_text_stays_put_while_the_champion_editor_exits() {
    use endpoint_tape::{bundle, effects, pump};
    for dpr in [1., 1.5] {
        let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
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
        game.receive(game.generation(), &bundle(&[vec![b'v', 1]]), 700.)
            .unwrap();
        let mut frames = Vec::new();
        for frame in 0..=1300 {
            let now = 700. + f64::from(frame) * 16.;
            // Close only once the returning login has settled below the editor.
            if frame == 800 {
                game.closed(game.generation());
                effects(&mut game);
            }
            pump(&mut game, now);
            login.observe_game(&game).unwrap();
            login.step(now, game.can_play(), false).unwrap();
            login
                .prepare_host(extent, dpr, game.can_play(), None)
                .unwrap();
            let view = login.state.view(true);
            frames.push((
                frame,
                login.champion.policy.visible(),
                view.visible && view.login_alpha >= 1.,
                login_text(&login),
            ));
        }
        let (.., settled) = frames.last().unwrap().clone();
        let length = settled
            .iter()
            .find(|t| t.0.starts_with("Your final length"))
            .unwrap()
            .clone();
        assert!(
            frames.iter().any(|f| f.1 && f.2),
            "{dpr}: settled under the editor"
        );
        assert!(frames.iter().any(|f| !f.1), "{dpr}: the editor exits");
        for (frame, editor, settled_login, text) in &frames {
            if *settled_login {
                assert!(
                    text.contains(&length),
                    "{dpr} {frame} editor={editor}: {text:?}"
                );
                assert!(
                    text.iter().all(|t| settled.contains(t)),
                    "{dpr} {frame} editor={editor}: {text:?}"
                );
            }
        }
    }
}

/// A section switch fades only the section's own content: the title, tabs
/// and footer text and widgets hold still on every frame, and the incoming
/// section's text appears where it settles.
#[test]
fn studio_text_and_widgets_stay_put_across_a_section_fade() {
    use slither_client::skin_studio::{BUILDER_TAB, CANCEL, COSMETICS_TAB, SAVE};
    for dpr in [1., 1.5, 2.] {
        let mut login = settled("Player");
        let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
        login.prepare_host(extent, dpr, true, None).unwrap();
        login.open_studio();
        let mut now = 9616;
        while login.studio.state.alpha() < 1. {
            login.step(f64::from(now), true, false).unwrap();
            login.prepare_host(extent, dpr, true, None).unwrap();
            now += 16;
        }
        let chrome = [
            "Skin studio",
            "Built-in",
            "Builder",
            "Cosmetics",
            "Cancel",
            "Save",
        ];
        let fixed = |login: &NativeLogin| {
            let text: Vec<_> = studio_text(login)
                .into_iter()
                .filter(|t| chrome.contains(&t.0.as_str()))
                .collect();
            let widgets: Vec<_> = login
                .studio_ui
                .controls
                .iter()
                .filter(|c| [BUILDER_TAB, COSMETICS_TAB, CANCEL, SAVE].contains(&c.0))
                .map(|c| (c.0, c.1))
                .collect();
            (text, widgets)
        };
        let open = fixed(&login);
        assert_eq!(open.0.len(), chrome.len(), "{dpr}: {:?}", open.0);
        assert_eq!(open.1.len(), 4);
        login.studio.activate(BUILDER_TAB, "Player");
        let mut frames = Vec::new();
        for _ in 0..16 {
            login.step(f64::from(now), true, false).unwrap();
            login.prepare_host(extent, dpr, true, None).unwrap();
            assert_eq!(fixed(&login), open, "{dpr} {now}");
            frames.push((now, studio_text(&login)));
            now += 16;
        }
        let settled = studio_text(&login);
        assert!(settled.iter().any(|t| t.0 == "Add"), "{dpr}: {settled:?}");
        for (at, text) in &frames {
            assert!(
                text.iter()
                    .filter(|t| !["Previous", "Next"].contains(&t.0.as_str()))
                    .all(|t| settled.contains(t)),
                "{dpr} {at}: {text:?}"
            );
        }
    }
}

/// Only the touch that owns egui-winit's synthetic pointer gets pointer
/// events, so only its cancel may drop the press.
#[test]
fn change_skin_tap_survives_a_cancelled_secondary_touch() {
    let touch = |id, phase, pos| egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    };
    let button = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    let send = |login: &mut NativeLogin, events| {
        login.settings_input(egui::RawInput {
            events,
            ..Default::default()
        })
    };
    let mut login = settled("Player");
    let pos = login
        .login_ui
        .controls
        .iter()
        .find(|c| c.0 == CHANGE_SKIN)
        .unwrap()
        .1
        .center();
    let down = vec![
        touch(7, egui::TouchPhase::Start, pos),
        egui::Event::PointerMoved(pos),
        button(pos, true),
    ];
    // The owner's cancel drops the press.
    send(&mut login, down.clone());
    send(
        &mut login,
        vec![
            touch(7, egui::TouchPhase::Cancel, pos),
            egui::Event::PointerGone,
        ],
    );
    assert!(!login.studio.state.visible());
    let context = login.egui.context().unwrap().clone();
    assert!(!context.input(|i| i.pointer.any_down()));
    // A secondary touch's cancel does not.
    send(&mut login, down);
    let other = pos + egui::vec2(100., 100.);
    send(&mut login, vec![touch(8, egui::TouchPhase::Start, other)]);
    send(&mut login, vec![touch(8, egui::TouchPhase::Cancel, other)]);
    send(
        &mut login,
        vec![
            touch(7, egui::TouchPhase::End, pos),
            button(pos, false),
            egui::Event::PointerGone,
        ],
    );
    assert!(login.studio.state.visible());
}
