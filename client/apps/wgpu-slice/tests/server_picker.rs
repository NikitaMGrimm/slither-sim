//! Authored synthetic directory/ping injection; no windows, GPU, or sockets.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{
    desktop_input::mouse_client_position, menu_controls::*, menu_input::*,
    menu_layout::MenuElement as E, server_directory::Server, server_picker::*,
    start_page::StartMenuIntent,
};
use slither_wgpu_slice::{
    login_native::NativeLogin, menu_input_native::KeyInput, server_picker_native::*,
};
use std::{collections::BTreeMap, net::SocketAddr};
use winit::{
    event::ElementState,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
};
fn servers() -> Vec<Server> {
    (1..=2000)
        .map(|id| Server {
            id,
            cluster: id % 5,
            address: SocketAddr::from(([192, 0, 2, 1], id)),
            population: 40,
            active: true,
        })
        .collect()
}
fn key(p: &mut NativeServerPicker, key: NamedKey, code: KeyCode) -> Vec<HostEditEffect> {
    let logical = Key::Named(key);
    p.key(KeyInput {
        logical: &logical,
        physical: PhysicalKey::Code(code),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
    })
    .effects
}
#[test]
fn scaled_icon_and_row_hits_are_isolated_from_play_and_gameplay() {
    for dpr in [1., 1.5, 2.] {
        let mut login = NativeLogin::new("Player").unwrap();
        login.step(0., true, false).unwrap();
        let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
        login.prepare_host(extent, dpr, true, None).unwrap();
        let r = login.layout.viewport_rect(E::Server);
        let physical = [(r.x + r.width / 2.) * dpr, (r.y + r.height / 2.) * dpr];
        let point = mouse_client_position(physical, dpr);
        login.pointer(point, Some(true), true);
        let effects = login.pointer(point, Some(false), true);
        assert!(effects.contains(&HostEditEffect::ReleasePointerBoost));
        assert!(login.picker.policy.is_open());
        assert!(login.picker.policy.take_directory_request());
        assert_eq!(login.picker.controls.focus(), Some(SEARCH));
        login
            .picker
            .policy
            .replace(&servers(), &BTreeMap::from([(0, 12)]))
            .unwrap();
        for now in (16..=640).step_by(16) {
            let (intents, _) = login.step(now as f64, true, false).unwrap();
            assert!(
                !intents
                    .iter()
                    .any(|i| matches!(i, StartMenuIntent::Play { .. }))
            );
        }
        login.prepare_host(extent, dpr, true, None).unwrap();
        let rows = login.picker.panel.resolve([960., 720.]).unwrap();
        let row = rows
            .iter()
            .find(|r| matches!(r.control.kind, ControlKind::ListRow { .. }))
            .unwrap();
        let point =
            mouse_client_position([(row.rect.x + 12.) * dpr, (row.rect.y + 12.) * dpr], dpr);
        assert!(row.hit(point));
        assert!(!row.hit([row.rect.x + row.rect.width, row.rect.y]));
        login.pointer(point, Some(true), true);
        login.pointer(point, Some(false), true);
        assert_eq!(login.picker.policy.selection.0, Some(5));
        for now in (656..=1200).step_by(16) {
            login.step(now as f64, true, false).unwrap();
        }
        assert!(!login.picker.policy.visible());
        assert_eq!(login.input.state.focus(), Some(EditorId::Nickname));
        assert!(!login.state.view(true).spinner_visible);
    }
}
#[test]
fn picker_is_available_without_a_play_route_and_immediate_close_restores_focus() {
    let mut login = NativeLogin::new("").unwrap();
    login.step(0., false, false).unwrap();
    login.prepare([960, 720], false, None).unwrap();
    let r = login.layout.viewport_rect(E::Server);
    let point = [r.x + 10., r.y + 10.];
    login.pointer(point, Some(true), false);
    login.pointer(point, Some(false), false);
    assert!(login.picker.policy.is_open());
    login.picker.policy.close();
    login.step(16., false, false).unwrap();
    assert!(!login.picker.policy.visible());
    assert_eq!(login.input.state.focus(), Some(EditorId::Nickname));
    assert!(!login.state.view(false).spinner_visible);
}
#[test]
fn offline_tab_finds_shared_server_link_and_paints_its_focus_without_enabling_play() {
    use slither_client::login_controls::{PLAY, QUALITY, SERVER};
    let mut login = NativeLogin::new("").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, false, false).unwrap();
    }
    let unfocused = login.prepare([960, 720], false, None).unwrap();
    let logical = Key::Named(NamedKey::Tab);
    let result = login.key(
        KeyInput {
            logical: &logical,
            physical: PhysicalKey::Code(KeyCode::Tab),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
        },
        false,
    );
    assert!(result.consumed);
    assert_eq!(login.controls.focus(), Some(SERVER));
    let focused = login.prepare([960, 720], false, None).unwrap();
    assert_eq!(focused.frame.draws.len(), unfocused.frame.draws.len() + 4);
    for id in [PLAY, QUALITY] {
        assert!(
            !login
                .panel
                .controls
                .iter()
                .find(|c| c.id == id)
                .unwrap()
                .enabled
        );
    }
    assert!(login.state.view(false).play_disabled);
}
#[test]
fn retained_picker_resize_stays_within_quotas_at_supported_dprs() {
    for dpr in [1., 1.5, 2., 3., 4., 5.] {
        let mut login = NativeLogin::new("").unwrap();
        for now in (0..=9600).step_by(16) {
            login.step(now as f64, true, false).unwrap();
        }
        login
            .picker
            .policy
            .replace(&servers(), &BTreeMap::from([(0, 12), (1, 65)]))
            .unwrap();
        login.picker.open();
        login.picker.policy.alpha = 1.;
        for logical in [
            [320., 240.],
            [640., 480.],
            [960., 540.],
            [960., 720.],
            [1280., 720.],
            [1600., 900.],
            [1920., 1080.],
            [960., 540.],
        ] {
            let extent = logical.map(|v| (v * dpr) as u32);
            if extent
                .iter()
                .any(|n| *n > slither_wgpu_slice::frame::MAX_EXTENT)
            {
                assert!(slither_wgpu_slice::render::dimensions(extent, dpr).is_err());
                continue; // The physical backing is outside the supported contract.
            }
            login.picker.policy.scroll(isize::MAX);
            login.picker.sync();
            let p = login
                .prepare_host(extent, dpr, true, None)
                .unwrap_or_else(|e| panic!("DPR {dpr}, {logical:?}: {e}"));
            let bytes = p
                .resources
                .iter()
                .map(|r| r.image.pixels.len())
                .sum::<usize>();
            assert!(
                p.resources.len() <= 64 && bytes <= 13 * 1024 * 1024,
                "DPR {dpr}, {logical:?}: {} resources, {bytes} bytes",
                p.resources.len()
            );
            assert!(
                p.frame.draws.len()
                    + usize::from(p.backdrop.is_some())
                    + usize::from(p.world_cover.is_some())
                    <= 512
            );
        }
    }
}
#[test]
fn list_window_raster_and_draw_cost_stays_bounded_across_refresh_and_scrolling() {
    for dpr in [1., 1.5, 2.] {
        let mut login = NativeLogin::new("").unwrap();
        login
            .picker
            .policy
            .replace(&servers(), &BTreeMap::from([(0, 12), (1, 55), (2, 160)]))
            .unwrap();
        login.step(0., true, false).unwrap();
        login.picker.open();
        for now in (16..=9600).step_by(16) {
            login.step(now as f64, true, false).unwrap();
        }
        for offset in [0, 994, 2000, -2000] {
            login.picker.policy.scroll(offset);
            login.picker.sync();
            let picture = login
                .prepare_host([(960. * dpr) as u32, (720. * dpr) as u32], dpr, true, None)
                .unwrap();
            let bytes = picture
                .resources
                .iter()
                .map(|r| r.image.pixels.len())
                .sum::<usize>();
            assert!(bytes <= 13 * 1024 * 1024, "{dpr}: {bytes}");
            assert!(picture.resources.len() <= 64);
            assert!(picture.frame.draws.len() <= 512);
            let backdrop = picture
                .frame
                .draws
                .iter()
                .position(|d| {
                    picture
                        .resources
                        .iter()
                        .any(|r| r.asset == d.asset && r.image.pixels == [24, 34, 48, 255])
                })
                .unwrap();
            let logo = picture
                .frame
                .draws
                .iter()
                .enumerate()
                .filter(|(_, d)| {
                    picture.resources.iter().any(|r| {
                        r.asset == d.asset
                            && r.asset.handle >= slither_wgpu_slice::bank::MENU_TEXT_HANDLE_BASE
                            && r.image
                                .pixels
                                .as_chunks::<4>()
                                .0
                                .contains(&[86, 172, 129, 255])
                    })
                })
                .map(|(i, _)| i)
                .collect::<Vec<_>>();
            assert!(!logo.is_empty());
            assert!(
                logo.iter().all(|i| *i < backdrop),
                "Login text must be below modal dimming and surface"
            );
            assert_eq!(
                login
                    .picker
                    .panel
                    .controls
                    .iter()
                    .filter(|c| matches!(c.kind, ControlKind::ListRow { .. }))
                    .count(),
                6
            );
        }
    }
}
#[test]
fn keyboard_traps_focus_numeric_edits_cannot_play_and_preference_failure_is_in_memory() {
    let mut p = NativeServerPicker::default();
    p.policy.replace(&servers(), &BTreeMap::new()).unwrap();
    p.open();
    let effects = key(&mut p, NamedKey::ArrowDown, KeyCode::ArrowDown);
    p.commit_effects(effects);
    assert_eq!(p.controls.focus(), Some(ControlId(16)));
    let effects = key(&mut p, NamedKey::End, KeyCode::End);
    p.commit_effects(effects);
    assert_eq!(p.controls.focus(), Some(ControlId(2015)));
    let effects = key(&mut p, NamedKey::Tab, KeyCode::Tab);
    p.commit_effects(effects);
    assert_eq!(p.controls.focus(), Some(CLOSE));
    let effects = key(&mut p, NamedKey::Tab, KeyCode::Tab);
    p.commit_effects(effects);
    assert_eq!(p.controls.focus(), Some(SEARCH));
    let effects = p.input.state.apply(EditIntent::KeyText("19x99".into()));
    p.commit_effects(effects);
    assert_eq!(p.policy.selection.0, Some(1999));
    assert!(!p.policy.is_open());
    let directory =
        std::env::temp_dir().join(format!("slither-server-pref-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("choice");
    assert_eq!(read_preference(&path), ServerPreference(None));
    write_preference(&path, &p.policy.take_preference().unwrap()).unwrap();
    assert_eq!(read_preference(&path), ServerPreference(Some(1999)));
    assert!(write_preference(&directory, "1:auto").is_err());
    assert_eq!(p.policy.selection.0, Some(1999));
    std::fs::write(&path, "1:77".repeat(1000)).unwrap();
    assert_eq!(read_preference(&path), ServerPreference(None));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
