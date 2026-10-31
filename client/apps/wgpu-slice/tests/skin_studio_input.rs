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
    let logical = Key::Named(named);
    let result = login.key(
        KeyInput {
            logical: &logical,
            physical: PhysicalKey::Code(KeyCode::Enter),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
        },
        false,
    );
    assert!(result.consumed);
}
fn click(login: &mut NativeLogin, id: ControlId, down: bool) {
    let resolved = login.panel.resolve([960., 540.]).unwrap();
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
    assert_eq!(login.controls.focus(), Some(NEXT));
    key(&mut login, NamedKey::ArrowRight);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.controls.focus(), Some(CANCEL));
    key(&mut login, NamedKey::ArrowRight);
    assert_eq!(login.studio.state.draft().unwrap().record.builtin, 1);
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.controls.focus(), Some(SAVE));
    key(&mut login, NamedKey::Enter);
    key(&mut login, NamedKey::Space);
    assert_eq!(login.studio.state.committed().builtin, 1);
    assert_eq!(login.studio.state.committed().nickname, "Current");
    assert_eq!(login.studio.status, "Saved for this run");
    assert!(login.step(459., false, false).unwrap().0.is_empty());
    assert!(login.studio.state.visible());
    login.sync_return_focus(false);
    assert_ne!(login.controls.focus(), Some(CHANGE_SKIN));
    assert!(login.step(460., false, false).unwrap().0.is_empty());
    login.sync_return_focus(false);
    assert_eq!(login.controls.focus(), Some(CHANGE_SKIN));
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
    login.focus(PREVIOUS);
    login.input.modifiers(ModifiersState::SHIFT);
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.controls.focus(), Some(SAVE));
    login.input.modifiers(ModifiersState::empty());
    key(&mut login, NamedKey::Tab);
    assert_eq!(login.controls.focus(), Some(PREVIOUS));
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
    use slither_client::login_controls::{NICKNAME, PLAY, QUALITY, SERVER};
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
    assert!(skin.control.visible && skin.control.enabled);
    assert!(server.control.visible && server.control.enabled);
    assert_eq!(skin.rect.y, server.rect.y);
    assert!(!skin.hit([
        server.rect.x + server.rect.width / 2.,
        server.rect.y + server.rect.height / 2.
    ]));
    login.focus(NICKNAME);
    for id in [PLAY, QUALITY, CHANGE_SKIN, SERVER, NICKNAME] {
        key(&mut login, NamedKey::Tab);
        assert_eq!(login.controls.focus(), Some(id));
    }
    login.input.modifiers(ModifiersState::SHIFT);
    for id in [SERVER, CHANGE_SKIN, QUALITY, PLAY, NICKNAME] {
        key(&mut login, NamedKey::Tab);
        assert_eq!(login.controls.focus(), Some(id));
    }
    login.input.modifiers(ModifiersState::empty());
    login.focus(SERVER);
    key(&mut login, NamedKey::Enter);
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
    for dpr in 1..=8 {
        let dpr = f64::from(dpr);
        let extent = [(960. * dpr) as u32, (540. * dpr) as u32];
        let mut login = NativeLogin::new("Player").unwrap();
        for time in (0..=9600).step_by(16) {
            login.step(f64::from(time), false, false).unwrap();
        }
        login.prepare_host(extent, dpr, false, None).unwrap();
        login.open_studio();
        for time in [0., 115., 230., 345., 459., 460.] {
            if time == 345. {
                login.studio.state.cancel(9830.);
            }
            login.step(9600. + time, false, false).unwrap();
            let menu = login.prepare_host(extent, dpr, false, None).unwrap();
            assert!(menu.frame.draws.len() <= 512);
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
    use slither_client::{desktop_input::mouse_client_position, menu_controls::*, menu_input::*};
    for height in [540., 720.] {
        for dpr in [5., 6., 7., 8.] {
            let mut login = NativeLogin::new("Player").unwrap();
            for time in (0..=9600).step_by(16) {
                login.step(time as f64, false, false).unwrap();
            }
            let extent = [(960. * dpr) as u32, (height * dpr) as u32];
            login.prepare_host(extent, dpr, false, None).unwrap();
            login.open_studio();
            for time in [9715., 9830.] {
                login.step(time, false, false).unwrap();
                let menu = login.prepare_host(extent, dpr, false, None).unwrap();
                assert_eq!(menu.frame.viewport_scale, dpr);
                assert_eq!(menu.frame.extent, extent);
                assert!(menu.frame.draws.len() + usize::from(menu.backdrop.is_some()) <= 512);
                assert!(
                    menu.resources
                        .iter()
                        .map(|r| r.image.pixels.len())
                        .sum::<usize>()
                        <= 13 * 1024 * 1024
                );
                let resolved = login.panel.resolve([960., height]).unwrap();
                let next = resolved.iter().find(|r| r.control.id == NEXT).unwrap();
                let r = next.rect;
                let point = mouse_client_position(
                    [(r.x + r.width / 2.) * dpr, (r.y + r.height / 2.) * dpr],
                    dpr,
                );
                assert!(next.hit(point));
                assert!(menu.frame.draws.iter().any(|d| point[0] >= d.destination.x
                    && point[0] < d.destination.x + d.destination.width
                    && point[1] >= d.destination.y
                    && point[1] < d.destination.y + d.destination.height));
                let mut hit = PanelInputState::default();
                hit.sync(&login.panel, &mut MenuInputState::default());
                hit.pointer(&login.panel, &resolved, point, Some(true));
                assert_eq!(
                    hit.pointer(&login.panel, &resolved, point, Some(false))
                        .unwrap()
                        .control,
                    NEXT
                );
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
