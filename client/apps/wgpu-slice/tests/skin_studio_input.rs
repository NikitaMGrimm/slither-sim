//! Native event translation with authored inputs; no windows or game servers.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{
    login_controls::CHANGE_SKIN,
    menu_controls::ControlId,
    skin_studio::*,
    start_page::{MenuAction, MenuPhase, StartMenuIntent},
};
use slither_wgpu_slice::{login_native::NativeLogin, menu_input_native::KeyInput};
use winit::{
    event::{ElementState, WindowEvent},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
};
fn key(login: &mut NativeLogin, named: NamedKey) {
    key_admitted(login, named, false);
}
fn key_admitted(login: &mut NativeLogin, named: NamedKey, admission: bool) {
    let logical = Key::Named(named);
    let result = login.key(
        KeyInput {
            logical: &logical,
            physical: PhysicalKey::Code(KeyCode::Enter),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
        },
        admission,
    );
    assert!(result.consumed);
}
fn click(login: &mut NativeLogin, id: ControlId, down: bool) {
    let point = if login.studio.state.visible() {
        let resolved = login.studio_panel.resolve([960., 540.]).unwrap();
        let r = resolved.iter().find(|c| c.control.id == id).unwrap().rect;
        [r.x + r.width / 2., r.y + r.height / 2.]
    } else {
        let r = login
            .login_ui
            .controls
            .iter()
            .find(|c| c.0 == id)
            .unwrap()
            .1;
        [f64::from(r.center().x), f64::from(r.center().y)]
    };
    login.pointer(point, Some(down), false);
}
#[test]
fn login_change_skin_opens_without_endpoint_and_save_returns_focus_after_fade() {
    let mut login = NativeLogin::new("Current").unwrap();
    login.step(0., false, false).unwrap();
    login.prepare([960, 540], false, None).unwrap();
    login.focus(CHANGE_SKIN);
    key(&mut login, NamedKey::Enter);
    assert!(login.studio.state.visible());
    assert_eq!(login.state.view(false).phase, MenuPhase::Login);
    let (intents, _) = login.step(230., false, false).unwrap();
    assert!(
        !intents
            .iter()
            .any(|i| matches!(i, StartMenuIntent::Play { .. }))
    );
    assert_eq!(login.focused(), Some(BUILT_IN_TAB));
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(PREVIOUS));
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(NEXT));
    key(&mut login, NamedKey::ArrowRight);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(CANCEL));
    key(&mut login, NamedKey::ArrowRight);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(SAVE));
    key(&mut login, NamedKey::Enter);
    key(&mut login, NamedKey::Space);
    assert_eq!(login.studio.state.committed().builtin, 1);
    assert_eq!(login.studio.state.committed().nickname, "Current");
    assert_eq!(login.studio.status, "Saved for this run");
    assert!(login.step(459., false, false).unwrap().0.is_empty());
    assert!(login.studio.state.visible());
    login.sync_return_focus(false);
    assert_ne!(login.focused(), Some(CHANGE_SKIN));
    assert!(login.step(460., false, false).unwrap().0.is_empty());
    login.sync_return_focus(false);
    assert_eq!(login.focused(), Some(CHANGE_SKIN));
    assert_eq!(login.state.view(false).phase, MenuPhase::Login);
}
#[test]
fn blur_resize_matching_release_outside_cancel_and_tab_trap_preserve_draft() {
    let mut login = NativeLogin::new("").unwrap();
    login.step(0., false, false).unwrap();
    login.open_studio();
    login.step(230., false, false).unwrap();
    click(&mut login, NEXT, true);
    login.event(&WindowEvent::Focused(false), false);
    click(&mut login, NEXT, false);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    login.event(&WindowEvent::Focused(true), false);
    click(&mut login, NEXT, true);
    login.event(
        &WindowEvent::Resized(winit::dpi::PhysicalSize::new(900, 540)),
        false,
    );
    click(&mut login, NEXT, false);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    click(&mut login, NEXT, true);
    click(&mut login, NEXT, false);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    login.focus(BUILT_IN_TAB);
    login.input.modifiers(ModifiersState::SHIFT);
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(SAVE));
    login.input.modifiers(ModifiersState::empty());
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.focused(), Some(BUILT_IN_TAB));
    login.pointer([0., 0.], Some(true), false);
    login.pointer([0., 0.], Some(false), false);
    login.step(460., false, false).unwrap();
    assert!(!login.studio.state.visible());
    assert_eq!(login.studio.state.committed().builtin, 0);
    login.open_studio();
    login.step(690., false, false).unwrap();
    key(&mut login, NamedKey::Escape);
    login.step(920., false, false).unwrap();
    assert!(!login.studio.state.visible());
    assert_eq!(login.studio.state.committed().builtin, 0);
}
#[test]
fn pending_play_cannot_open_studio_and_preview_clock_pauses_without_catch_up() {
    let mut login = NativeLogin::new("").unwrap();
    login.step(0., true, false).unwrap();
    login.state.action(MenuAction::Play, true);
    login.open_studio();
    assert!(!login.studio.state.visible());
    let mut preview = slither_client::studio_preview::AppearancePreview::default();
    preview.tick(0.);
    preview.tick(16.);
    preview.pause();
    preview.tick(9000.);
    assert_eq!(preview.time(), 16.);
    preview.tick(9016.);
    assert_eq!(preview.time(), 32.);
}

#[test]
fn secondary_actions_share_a_row_and_focus_order_without_crossing_modals() {
    use slither_client::login_controls::{NICKNAME, PLAY, SERVER, SETTINGS};
    let mut login = NativeLogin::new("").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    login.prepare([960, 540], true, None).unwrap();
    let view = login.state.view(true);
    let resolved = slither_client::login_controls::panel(&view, &login.layout, [960., 540.])
        .resolve([960., 540.])
        .unwrap();
    let skin = resolved
        .iter()
        .find(|r| r.control.id == CHANGE_SKIN)
        .unwrap();
    let server = resolved.iter().find(|r| r.control.id == SERVER).unwrap();
    let settings = resolved.iter().find(|r| r.control.id == SETTINGS).unwrap();
    assert!(skin.control.visible && skin.control.enabled);
    assert!(server.control.visible && server.control.enabled);
    assert_eq!(skin.rect.y, server.rect.y);
    assert!(settings.control.visible && settings.control.enabled);
    assert_eq!(skin.rect.y, settings.rect.y);
    assert!(!skin.rect.contains(
        server.rect.x + server.rect.width / 2.,
        server.rect.y + server.rect.height / 2.
    ));
    login.focus(NICKNAME);
    for id in [PLAY, CHANGE_SKIN, SERVER, SETTINGS, NICKNAME] {
        key_admitted(&mut login, NamedKey::Tab, true);
        assert_eq!(login.focused(), Some(id));
    }
    login.input.modifiers(ModifiersState::SHIFT);
    for id in [SETTINGS, SERVER, CHANGE_SKIN, PLAY, NICKNAME] {
        key_admitted(&mut login, NamedKey::Tab, true);
        assert_eq!(login.focused(), Some(id));
    }
    login.input.modifiers(ModifiersState::empty());
    login.focus(SERVER);
    key_admitted(&mut login, NamedKey::Enter, true);
    assert!(login.picker.policy.visible());
    login.open_studio();
    assert!(!login.studio.state.visible());

    let mut login = NativeLogin::new("").unwrap();
    login.step(0., false, false).unwrap();
    login.open_studio();
    login.step(230., false, false).unwrap();
    // Control ids belong to the active panel; a login action cannot pierce it.
    login.focus(SERVER);
    login.activate(false);
    assert!(!login.picker.policy.visible());
    login.pointer([100., 100.], Some(true), false);
    login.pointer([100., 100.], Some(false), false);
    assert!(login.studio.state.interactive());
    // Releasing outside after pressing the surface must not dismiss the modal.
    login.pointer([100., 100.], Some(true), false);
    login.pointer([0., 0.], Some(false), false);
    assert!(login.studio.state.interactive());
}

#[test]
fn entry_return_overlap_and_rotated_preview_fit_shared_quotas_at_dpr_one_to_eight() {
    let mut peak = 0;
    for (section, save) in [
        (StudioSection::BuiltIn, false),
        (StudioSection::BuiltIn, true),
        (StudioSection::Builder, false),
        (StudioSection::Builder, true),
    ] {
        for dpr in 1..=8 {
            let dpr = f64::from(dpr);
            let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
            let mut login = NativeLogin::new("Player").unwrap();
            for time in (0..=9600).step_by(16) {
                login.step(f64::from(time), false, false).unwrap();
            }
            login.prepare_host(extent, dpr, false, None).unwrap();
            login.open_studio();
            for time in [0., 115., 230., 320., 410., 525., 639., 640.] {
                login.step(9600. + time, false, false).unwrap();
                if time == 230. && section == StudioSection::Builder {
                    login.studio.activate(BUILDER_TAB, "Player");
                    for n in 0..47 {
                        login.studio.state.edit_builder(|b| {
                            b.append(slither_client::skin_builder::PALETTES[n % 39]);
                        });
                    }
                }
                if time == 410. {
                    if save {
                        login.studio.activate_with_random(SAVE, "Player", &mut || 0);
                    } else {
                        login.studio.state.cancel(9600. + time);
                    }
                    login.step(9600. + time, false, false).unwrap();
                }
                let menu = login.prepare_host(extent, dpr, false, None).unwrap();
                assert!(
                    menu.resources.len() <= 1,
                    "{section:?}, Save {save}, DPR {dpr}"
                );
                if login.studio.state.visible() {
                    let (preview, resources) =
                        login.studio.prepare_preview(extent, dpr, None).unwrap();
                    peak = peak.max(preview.draws.len());
                    assert!(preview.draws.len() <= 1024);
                    preview
                        .validate(
                            &resources
                                .iter()
                                .map(|r| (r.asset.into(), r.asset))
                                .collect(),
                        )
                        .unwrap();
                }
                login.studio.presented();
            }
        }
    }
    println!("Studio entry/return preview: {peak}/1024 draws");
}

#[test]
fn login_constructor_and_champion_return_retain_their_existing_normalization_and_fade() {
    let login = NativeLogin::new("A😀\n").unwrap();
    assert_eq!(login.state.view(true).nickname, "A   ");
    let mut login = NativeLogin::new("Player").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    let mut session = slither_client::session_policy::SessionPolicy::new().view();
    session.outcome = Some(slither_client::session_policy::Phase::Victory);
    login.champion.policy.observe(1, &session);
    login.champion.policy.step(0., 1.).unwrap();
    session.champion_acknowledged = true;
    login.champion.policy.observe(1, &session);
    let mut time = 0.;
    while !login.champion.policy.finished() {
        time += 40.;
        login.champion.policy.step(time, 1.).unwrap();
    }
    login.prepare_host([960, 540], 1., true, None).unwrap();
    // The returning controls start invisible and fade in on the policy's clock.
    assert_eq!(login.champion.policy.ordinary_alpha(), 0.);
}

#[test]
fn studio_density_cap_preserves_real_dpr_paint_hits_and_preview() {
    use slither_client::desktop_input::mouse_client_position;
    for height in [540., 720.] {
        for dpr in [5., 6., 7., 8.] {
            let mut login = NativeLogin::new("Player").unwrap();
            for time in (0..=9600).step_by(16) {
                login.step(time as f64, false, false).unwrap();
            }
            let extent = [(960. * dpr) as u32, (height * dpr) as u32];
            login.prepare_host(extent, dpr, false, None).unwrap();
            login.open_studio();
            for (time, skin) in [(9715., 1), (9830., 2)] {
                login.step(time, false, false).unwrap();
                let menu = login.prepare_host(extent, dpr, false, None).unwrap();
                assert_eq!(menu.viewport_scale, dpr);
                assert_eq!(menu.extent, extent);
                let resolved = login.studio_panel.resolve([960., height]).unwrap();
                let next = resolved.iter().find(|r| r.control.id == NEXT).unwrap();
                let r = next.rect;
                let point = mouse_client_position(
                    [(r.x + r.width / 2.) * dpr, (r.y + r.height / 2.) * dpr],
                    dpr,
                );
                // The egui widget is drawn and hit at the core rectangle.
                let widget = login
                    .studio_ui
                    .controls
                    .iter()
                    .find(|c| c.0 == NEXT)
                    .unwrap()
                    .1;
                assert!(widget.contains(egui::pos2(point[0] as f32, point[1] as f32)));
                login.pointer(point, Some(true), false);
                login.pointer(point, Some(false), false);
                assert_eq!(login.studio.state.draft().unwrap().record.builtin, skin);
                let (preview, resources) = login.studio.prepare_preview(extent, dpr, None).unwrap();
                assert!(preview.draws.len() <= 1024);
                assert_eq!(preview.viewport_scale, dpr);
                preview
                    .validate(
                        &resources
                            .iter()
                            .map(|r| (r.asset.into(), r.asset))
                            .collect(),
                    )
                    .unwrap();
                login.studio.presented();
            }
        }
    }
}

#[test]
fn studio_round_trip_restores_opener_modality_and_clears_pointer_feedback() {
    for keyboard in [false, true] {
        let mut login = NativeLogin::new("Focus").unwrap();
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, false, false).unwrap();
        }
        login.prepare([960, 540], false, None).unwrap();
        if keyboard {
            while login.focused() != Some(CHANGE_SKIN) {
                key(&mut login, NamedKey::Tab);
            }
            key(&mut login, NamedKey::Enter);
        } else {
            click(&mut login, CHANGE_SKIN, true);
            login.step(9616., false, false).unwrap();
            click(&mut login, CHANGE_SKIN, false);
        }
        login.step(9900., false, false).unwrap();
        assert!(login.studio.state.visible());
        click(&mut login, CANCEL, true);
        login.step(9916., false, false).unwrap();
        click(&mut login, CANCEL, false);
        login.step(10200., false, false).unwrap();
        login.sync_return_focus(false);
        assert!(!login.studio.state.visible());
        assert_eq!(login.focused(), Some(CHANGE_SKIN));
        assert_eq!(login.login_ui.keyboard_focus, keyboard);
        assert!(
            !login
                .egui
                .context()
                .unwrap()
                .input(|i| i.pointer.any_down())
        );
        assert!(login.egui.context().unwrap().dragged_id().is_none());
    }
}

fn center(login: &NativeLogin, id: ControlId) -> [f64; 2] {
    let resolved = login.studio_panel.resolve([960., 540.]).unwrap();
    let r = resolved.iter().find(|c| c.control.id == id).unwrap().rect;
    [r.x + r.width / 2., r.y + r.height / 2.]
}
fn opened_studio() -> NativeLogin {
    let mut login = NativeLogin::new("").unwrap();
    login.step(0., false, false).unwrap();
    login.prepare([960, 540], false, None).unwrap();
    login.open_studio();
    login.step(230., false, false).unwrap();
    login
}

/// As with the old panel input, a press focuses its control without showing
/// the keyboard ring, and the release activates it.
#[test]
fn a_press_focuses_its_control_and_the_release_activates_it() {
    let mut login = opened_studio();
    assert_eq!(login.focused(), Some(BUILT_IN_TAB));
    let next = center(&login, NEXT);
    login.pointer(next, Some(true), false);
    assert_eq!(login.focused(), Some(NEXT));
    assert!(!login.studio_ui.keyboard_focus);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    login.pointer(next, Some(false), false);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    // The keyboard continues from the pressed control.
    key(&mut login, NamedKey::ArrowRight);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 2);
    assert!(login.studio_ui.keyboard_focus);
}

#[test]
fn studio_controls_and_backdrop_answer_a_long_hold_and_a_drag_out_and_back() {
    for drag in [false, true] {
        let mut login = opened_studio();
        let hold = |login: &mut NativeLogin, p: [f64; 2]| {
            login.pointer(p, Some(true), false);
            if drag {
                login.pointer([p[0] + 200., 2.], None, false);
                login.prepare([960, 540], false, None).unwrap();
                login.pointer(p, None, false);
            } else {
                login.step(1500., false, false).unwrap();
            }
            login.prepare([960, 540], false, None).unwrap();
            login.pointer(p, Some(false), false);
        };
        let next = center(&login, NEXT);
        hold(&mut login, next);
        assert_eq!(
            login.studio.state.draft().unwrap().record.builtin,
            1,
            "{drag}"
        );
        hold(&mut login, [2., 2.]);
        login.step(5000., false, false).unwrap();
        assert!(!login.studio.state.visible(), "{drag}");
    }
    // A press on Next released elsewhere changes nothing.
    let mut login = opened_studio();
    let next = center(&login, NEXT);
    login.pointer(next, Some(true), false);
    login.pointer([next[0] + 200., 2.], Some(false), false);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    // A backdrop press released over the panel keeps the studio.
    login.pointer([2., 2.], Some(true), false);
    login.prepare([960, 540], false, None).unwrap();
    login.pointer(next, Some(false), false);
    login.step(5000., false, false).unwrap();
    assert!(login.studio.state.visible());
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
}

/// A key the studio ignores leaves the held press; one it handles retires it, so
/// the release can't act on top of the key.
#[test]
fn only_keys_the_studio_handles_retire_the_held_press() {
    let tap = |key: egui::Key| {
        let mut login = opened_studio();
        let next = center(&login, NEXT);
        login.pointer(next, Some(true), false);
        for pressed in [true, false] {
            login.settings_input(egui::RawInput {
                events: vec![arrow(key, pressed)],
                ..Default::default()
            });
        }
        login.prepare([960, 540], false, None).unwrap();
        let after_key = login.studio.state.draft().unwrap().record.builtin;
        login.pointer(next, Some(false), false);
        (
            after_key,
            login.studio.state.draft().unwrap().record.builtin,
        )
    };
    for key in [egui::Key::A, egui::Key::F1] {
        assert_eq!(tap(key), (0, 1), "{key:?}");
    }
    assert_eq!(tap(egui::Key::Enter), (1, 1));
    let (left, released) = tap(egui::Key::ArrowLeft);
    assert_ne!(left, 0);
    assert_eq!(released, left);
}

/// Arrow keys can move the cosmetic window under a press, as the wheel can.
#[test]
fn a_keyboard_moved_cosmetic_window_retires_the_held_press() {
    let mut login = opened_studio();
    login.studio.activate(COSMETICS_TAB, "Player");
    login.step(500., false, false).unwrap();
    login.prepare([960, 540], false, None).unwrap();
    let cell = ControlId(COSMETIC_BASE + 5);
    let middle = |login: &NativeLogin| {
        let c = login
            .studio_ui
            .controls
            .iter()
            .find(|c| c.0 == cell)
            .unwrap();
        [f64::from(c.1.center().x), f64::from(c.1.center().y)]
    };
    let before = login.studio.state.preview_cosmetic();
    let start = middle(&login);
    login.pointer(start, Some(true), false);
    assert_eq!(login.focused(), Some(cell));
    for _ in 0..2 {
        key(&mut login, NamedKey::ArrowDown);
    }
    login.prepare([960, 540], false, None).unwrap();
    let end = middle(&login);
    assert_ne!(start, end);
    login.pointer(end, Some(false), false);
    assert_eq!(login.studio.state.preview_cosmetic(), before);
    // A fresh click on the same cell acts.
    login.prepare([960, 540], false, None).unwrap();
    login.pointer(end, Some(true), false);
    login.pointer(end, Some(false), false);
    assert_ne!(login.studio.state.preview_cosmetic(), before);
}

/// A section switch starts a new panel generation: a press held across it
/// never activates the control under its release, as the old capture rules.
#[test]
fn a_press_held_across_a_section_switch_never_clicks() {
    let mut login = opened_studio();
    let cancel = center(&login, CANCEL);
    login.pointer(cancel, Some(true), false);
    login.studio.activate(BUILDER_TAB, "");
    login.step(300., false, false).unwrap();
    assert_eq!(center(&login, CANCEL), cancel);
    login.pointer(cancel, Some(false), false);
    login.step(600., false, false).unwrap();
    assert!(login.studio.state.visible());
    assert_eq!(login.studio.state.section, StudioSection::Builder);
    assert_eq!(login.focused(), Some(BUILDER_TAB));
    // The next full press and release does.
    login.pointer(cancel, Some(true), false);
    login.pointer(cancel, Some(false), false);
    login.step(900., false, false).unwrap();
    assert!(!login.studio.state.visible());
}

/// Touch arrives as egui-winit translates it: a tap activates once; a tap
/// that leaves its control before lifting does not.
#[test]
fn touch_taps_activate_through_egui() {
    let mut login = opened_studio();
    let tap = |at: [f64; 2], start: bool, end: bool| {
        let pos = egui::pos2(at[0] as f32, at[1] as f32);
        let touch = |phase| egui::Event::Touch {
            device_id: egui::TouchDeviceId(1),
            id: egui::TouchId(7),
            phase,
            pos,
            force: None,
        };
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let mut events = Vec::new();
        if start {
            events.extend([
                touch(egui::TouchPhase::Start),
                egui::Event::PointerMoved(pos),
                button(true),
            ]);
        }
        if end {
            events.extend([
                touch(egui::TouchPhase::End),
                egui::Event::PointerMoved(pos),
                button(false),
                egui::Event::PointerGone,
            ]);
        }
        egui::RawInput {
            events,
            ..Default::default()
        }
    };
    let next = center(&login, NEXT);
    login.settings_input(tap(next, true, true));
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    login.settings_input(tap(next, true, false));
    login.settings_input(tap(center(&login, PREVIOUS), false, true));
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    login.settings_input(tap(center(&login, PREVIOUS), true, true));
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    assert!(!login.studio_ui.keyboard_focus);
}

fn raw(login: &mut NativeLogin, events: Vec<egui::Event>) {
    login.settings_input(egui::RawInput {
        events,
        ..Default::default()
    });
}
fn arrow(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        // egui-winit 0.37 reports native auto-repeats as repeat: false.
        repeat: false,
        modifiers: Default::default(),
    }
}

/// The adapter route, which the `key` helper's repeat filter would mask:
/// a held arrow cycles once per physical press, as the old studio input did.
#[test]
fn held_navigation_keys_act_once_per_physical_press() {
    let mut login = opened_studio();
    login.focus(NEXT);
    for _ in 0..3 {
        raw(&mut login, vec![arrow(egui::Key::ArrowRight, true)]);
    }
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    raw(&mut login, vec![arrow(egui::Key::ArrowRight, false)]);
    raw(
        &mut login,
        vec![
            arrow(egui::Key::ArrowRight, true),
            arrow(egui::Key::ArrowRight, true),
        ],
    );
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 2);
    // A held key is forgotten when the window loses focus.
    raw(&mut login, vec![egui::Event::WindowFocused(false)]);
    raw(&mut login, vec![egui::Event::WindowFocused(true)]);
    login.focus(NEXT);
    raw(&mut login, vec![arrow(egui::Key::ArrowLeft, true)]);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    // Holding Right on the tab switches one section, not all of them.
    raw(&mut login, vec![arrow(egui::Key::ArrowLeft, false)]);
    login.focus(BUILT_IN_TAB);
    for _ in 0..3 {
        raw(&mut login, vec![arrow(egui::Key::ArrowRight, true)]);
    }
    assert_eq!(login.studio.state.section, StudioSection::Builder);
}

/// Touches as egui-winit translates them: only the touch that owns the
/// synthetic pointer gets pointer events, and its cancel has no release.
fn touch(id: u64, phase: egui::TouchPhase, pos: egui::Pos2) -> egui::Event {
    egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    }
}
fn primary(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
fn owner_down(login: &mut NativeLogin, id: u64, pos: egui::Pos2) {
    raw(
        login,
        vec![
            touch(id, egui::TouchPhase::Start, pos),
            egui::Event::PointerMoved(pos),
            primary(pos, true),
        ],
    );
}
fn owner_up(login: &mut NativeLogin, id: u64, pos: egui::Pos2) {
    raw(
        login,
        vec![
            touch(id, egui::TouchPhase::End, pos),
            primary(pos, false),
            egui::Event::PointerGone,
        ],
    );
}
fn owner_cancel(login: &mut NativeLogin, id: u64, pos: egui::Pos2) {
    raw(
        login,
        vec![
            touch(id, egui::TouchPhase::Cancel, pos),
            egui::Event::PointerGone,
        ],
    );
}
fn next_center(login: &NativeLogin) -> egui::Pos2 {
    login
        .studio_ui
        .controls
        .iter()
        .find(|c| c.0 == NEXT)
        .unwrap()
        .1
        .center()
}
fn held(login: &NativeLogin) -> bool {
    login
        .egui
        .context()
        .unwrap()
        .input(|i| i.pointer.any_down())
}

/// The owner's cancelled press neither clicks nor stays held on the returned
/// login.
#[test]
fn a_cancelled_touch_releases_the_pointer_without_a_click() {
    let mut login = opened_studio();
    let pos = next_center(&login);
    owner_down(&mut login, 7, pos);
    owner_cancel(&mut login, 7, pos);
    assert!(!held(&login));
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    login.studio.state.cancel(230.);
    login.step(460., false, false).unwrap();
    login.sync_return_focus(false);
    login.prepare([960, 540], false, None).unwrap();
    assert!(!login.studio.state.visible());
    assert!(!held(&login));
}

/// A second finger gets no pointer events while the first owns the pointer, so
/// cancelling it leaves the first finger's press to click on release.
#[test]
fn a_cancelled_secondary_touch_leaves_the_primary_press() {
    let mut login = opened_studio();
    let pos = next_center(&login);
    let other = pos + egui::vec2(100., 100.);
    owner_down(&mut login, 7, pos);
    raw(&mut login, vec![touch(8, egui::TouchPhase::Start, other)]);
    raw(&mut login, vec![touch(8, egui::TouchPhase::Cancel, other)]);
    assert!(held(&login));
    owner_up(&mut login, 7, pos);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    assert!(!held(&login));
}

#[test]
fn the_owner_still_cancels_after_a_secondary_touch_comes_and_goes() {
    let mut login = opened_studio();
    let pos = next_center(&login);
    let other = pos + egui::vec2(100., 100.);
    owner_down(&mut login, 7, pos);
    raw(&mut login, vec![touch(8, egui::TouchPhase::Start, other)]);
    raw(&mut login, vec![touch(8, egui::TouchPhase::Cancel, other)]);
    raw(&mut login, vec![touch(9, egui::TouchPhase::Start, other)]);
    owner_cancel(&mut login, 7, pos);
    assert!(!held(&login));
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    // Touch 9 started under the old owner, so it never owned the pointer, and
    // the cancel freed ownership for the next touch.
    raw(&mut login, vec![touch(9, egui::TouchPhase::End, other)]);
    owner_down(&mut login, 10, pos);
    owner_up(&mut login, 10, pos);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
}

/// A new egui-winit adapter starts with no owner, whichever touch was down.
#[test]
fn a_dropped_adapter_forgets_the_owning_touch() {
    let resets: [fn(&mut NativeLogin); 2] = [
        |login| login.egui.observe_only(&WindowEvent::Focused(true), 1.),
        |login| {
            login.egui.set_active(false);
            login.egui.set_active(true);
        },
    ];
    for reset in resets {
        let mut login = opened_studio();
        let pos = next_center(&login);
        owner_down(&mut login, 7, pos);
        reset(&mut login);
        owner_down(&mut login, 8, pos);
        owner_cancel(&mut login, 8, pos);
        assert!(!held(&login));
        assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    }
}
