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
    let panel = if login.studio.state.visible() {
        &login.studio_panel
    } else {
        &login.panel
    };
    let resolved = panel.resolve([960., 540.]).unwrap();
    let r = resolved.iter().find(|c| c.control.id == id).unwrap();
    login.pointer(
        [r.rect.x + r.rect.width / 2., r.rect.y + r.rect.height / 2.],
        Some(down),
        false,
    );
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
    let resolved = login.panel.resolve([960., 540.]).unwrap();
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
    assert!(!skin.hit([
        server.rect.x + server.rect.width / 2.,
        server.rect.y + server.rect.height / 2.
    ]));
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
    let mut admitted_peak = 0;
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
                let draws = menu.frame.draws.len() + usize::from(menu.backdrop.is_some());
                let counts = menu.draw_counts();
                assert_eq!(counts.components.total(), menu.frame.draws.len());
                assert_eq!(counts.reported_menu(), draws);
                assert_eq!(
                    counts.admitted_menu(),
                    draws + usize::from(menu.world_cover.is_some())
                );
                admitted_peak = admitted_peak.max(counts.admitted_menu());
                assert!(counts.admitted_menu() <= 512);
                peak = peak.max(draws);
                assert!(
                    draws <= 512,
                    "{section:?}, Save {save}, DPR {dpr}, t {time}: {draws}"
                );
                assert!(menu.resources.len() <= 64);
                assert!(
                    menu.resources
                        .iter()
                        .map(|r| r.image.pixels.len())
                        .sum::<usize>()
                        <= 13 * 1024 * 1024
                );
                let mut picture = slither_wgpu_slice::render::PreparedFrame {
                    frame: menu.frame,
                    overlay: None,
                    resources: menu.resources,
                };
                if login.studio.state.visible() {
                    login.studio.append_preview(&mut picture, None).unwrap();
                }
                picture
                    .frame
                    .validate(
                        &picture
                            .resources
                            .iter()
                            .map(|r| (r.asset.into(), r.asset))
                            .collect(),
                    )
                    .unwrap();
                assert!(picture.draw_count() <= 1536);
                login.studio.presented();
            }
        }
    }
    println!(
        "Studio entry/return with Settings: {peak}/512 reported; {admitted_peak}/512 admitted, including cover"
    );
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
    for id in [
        slither_client::login_controls::NICKNAME,
        slither_client::login_controls::PLAY,
        CHANGE_SKIN,
        slither_client::login_controls::SERVER,
    ] {
        assert_eq!(
            login
                .panel
                .controls
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .alpha,
            0.
        );
    }
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
                assert_eq!(menu.frame.viewport_scale, dpr);
                assert_eq!(menu.frame.extent, extent);
                let resolved = login.studio_panel.resolve([960., height]).unwrap();
                let next = resolved.iter().find(|r| r.control.id == NEXT).unwrap();
                let r = next.rect;
                let point = mouse_client_position(
                    [(r.x + r.width / 2.) * dpr, (r.y + r.height / 2.) * dpr],
                    dpr,
                );
                assert!(next.hit(point));
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
                let mut picture = slither_wgpu_slice::render::PreparedFrame {
                    frame: menu.frame,
                    overlay: None,
                    resources: menu.resources,
                };
                login.studio.append_preview(&mut picture, None).unwrap();
                assert!(picture.draw_count() <= 1536);
                assert_eq!(picture.ui_frame().viewport_scale, dpr);
                picture
                    .frame
                    .validate(
                        &picture
                            .resources
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
        assert!(login.controls.capture().is_none());
        assert!(
            !login
                .egui
                .context()
                .unwrap()
                .input(|i| i.pointer.any_down())
        );
        assert!(login.egui.context().unwrap().dragged_id().is_none());
        let returned = login.prepare([960, 540], false, None).unwrap();
        // Moving the pointer away cannot remove any additional stale ring/press paint.
        login.pointer([-1., -1.], None, false);
        let away = login.prepare([960, 540], false, None).unwrap();
        assert_eq!(returned.frame.draws.len(), away.frame.draws.len());
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

/// egui-winit turns a cancelled touch into Touch(Cancel) and PointerGone with
/// no release. The press neither clicks nor stays held on the returned login.
#[test]
fn a_cancelled_touch_releases_the_pointer_without_a_click() {
    let mut login = opened_studio();
    let pos = login
        .studio_ui
        .controls
        .iter()
        .find(|c| c.0 == NEXT)
        .unwrap()
        .1
        .center();
    let touch = |phase| egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(7),
        phase,
        pos,
        force: None,
    };
    raw(
        &mut login,
        vec![
            touch(egui::TouchPhase::Start),
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ],
    );
    raw(
        &mut login,
        vec![touch(egui::TouchPhase::Cancel), egui::Event::PointerGone],
    );
    let context = login.egui.context().unwrap().clone();
    assert!(!context.input(|i| i.pointer.any_down()));
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 0);
    login.studio.state.cancel(230.);
    login.step(460., false, false).unwrap();
    login.sync_return_focus(false);
    login.prepare([960, 540], false, None).unwrap();
    assert!(!login.studio.state.visible());
    assert!(!context.input(|i| i.pointer.any_down()));
}
