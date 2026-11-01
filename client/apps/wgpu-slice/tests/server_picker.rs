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
        assert!(!login.picker.policy.take_directory_request());
        assert_eq!(login.picker.controls.focus(), Some(ADDRESS));
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
        assert_eq!(login.picker.policy.selection.official_id(), Some(5));
        for now in (656..=1200).step_by(16) {
            login.step(now as f64, true, false).unwrap();
        }
        assert!(!login.picker.policy.visible());
        assert_eq!(
            login.focused(),
            Some(slither_client::login_controls::NICKNAME)
        );
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
    assert_eq!(
        login.focused(),
        Some(slither_client::login_controls::NICKNAME)
    );
    assert!(!login.state.view(false).spinner_visible);
}
#[test]
fn offline_tab_finds_shared_server_link_and_paints_its_focus_without_enabling_play() {
    use slither_client::login_controls::{CHANGE_SKIN, PLAY, SERVER, SETTINGS};
    let mut login = NativeLogin::new("").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, false, false).unwrap();
    }
    login.prepare([960, 720], false, None).unwrap();
    let unfocused = login.egui.prepared_shapes().len();
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
    assert_eq!(login.focused(), Some(CHANGE_SKIN));
    login.key(
        KeyInput {
            logical: &logical,
            physical: PhysicalKey::Code(KeyCode::Tab),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
        },
        false,
    );
    assert_eq!(login.focused(), Some(SERVER));
    login.prepare([960, 720], false, None).unwrap();
    assert_eq!(login.focused(), Some(SERVER));
    assert!(login.login_ui.keyboard_focus);
    assert!(login.egui.prepared_shapes().len() > unfocused);
    let order = slither_wgpu_slice::login_egui::focus_order(&login.state.view(false));
    for id in [PLAY, SETTINGS] {
        assert!(!order.contains(&slither_wgpu_slice::login_egui::widget_id(id)));
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
                            && r.image.pixels.as_chunks::<4>().0.iter().any(|p| {
                                p[3] == 255
                                    && (p[..3] == slither_wgpu_slice::menu::WORDMARK_GREEN
                                        || (p[1] > 180 && p[0] < 140 && p[2] < 170))
                            })
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
fn clicking_numeric_search_deliberately_clears_it_even_when_already_focused() {
    let mut picker = NativeServerPicker::default();
    picker.policy.replace(&[], &BTreeMap::new()).unwrap();
    picker.open();
    picker.policy.alpha = 1.;
    picker.sync();
    let mut text = slither_wgpu_slice::text::TextCache::menu();
    let resolved = picker.panel.resolve(picker.viewport).unwrap();
    let search = resolved.iter().find(|r| r.control.id == SEARCH).unwrap();
    let point = [search.rect.x + 20., search.rect.y + search.rect.height / 2.];
    picker.pointer(&mut text, point, Some(true)).unwrap();
    picker.pointer(&mut text, point, Some(false)).unwrap();
    for query in ["909", "808"] {
        let effects = picker.input.state.apply(EditIntent::KeyText(query.into()));
        picker.commit_effects(effects);
        assert_eq!(picker.input.state.editor(SEARCH_EDITOR).text(), query);
        assert_eq!(picker.controls.focus(), Some(SEARCH));
        let effects = picker.pointer(&mut text, point, Some(true)).unwrap();
        picker.commit_effects(effects);
        let effects = picker.pointer(&mut text, point, Some(false)).unwrap();
        picker.commit_effects(effects);
        assert_eq!(picker.input.state.editor(SEARCH_EDITOR).text(), "");
        assert_eq!(picker.policy.search, "");
        assert!(picker.policy.is_open());
        assert_eq!(picker.policy.selection, ServerPreference::Automatic);
    }
}
#[test]
fn keyboard_traps_focus_numeric_edits_cannot_play_and_preference_failure_is_in_memory() {
    let mut p = NativeServerPicker::default();
    p.policy.replace(&servers(), &BTreeMap::new()).unwrap();
    p.open();
    p.controls.focus_control(&p.panel, SEARCH);
    p.controls.sync_focus(&p.panel, &mut p.input.state);
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
    assert_eq!(p.controls.focus(), Some(ADDRESS));
    for _ in 0..3 {
        let effects = key(&mut p, NamedKey::Tab, KeyCode::Tab);
        p.commit_effects(effects);
    }
    assert_eq!(p.controls.focus(), Some(SEARCH));
    let effects = p.input.state.apply(EditIntent::KeyText("19x99".into()));
    p.commit_effects(effects);
    assert_eq!(p.policy.selection.official_id(), Some(1999));
    assert!(!p.policy.is_open());
    let directory =
        std::env::temp_dir().join(format!("slither-server-pref-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("choice");
    assert_eq!(read_preference(&path), ServerPreference::Automatic);
    write_preference(&path, &p.policy.take_preference().unwrap()).unwrap();
    assert_eq!(read_preference(&path), ServerPreference::Official(1999));
    assert!(write_preference(&directory, "1:auto").is_err());
    assert_eq!(p.policy.selection.official_id(), Some(1999));
    std::fs::write(&path, "1:77".repeat(1000)).unwrap();
    assert_eq!(read_preference(&path), ServerPreference::Automatic);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn open_picker_with_stroke_logo_never_drops_at_dpr_five_to_eight() {
    let mut failures = Vec::new();
    for height in [720., 540.] {
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
        for dpr in [5., 6., 7., 8., 5.] {
            for offset in [0, isize::MAX, isize::MIN] {
                login.picker.policy.scroll(offset);
                login.picker.sync();
                let picture = login.prepare_host(
                    [(960. * dpr) as u32, (height * dpr) as u32],
                    dpr,
                    true,
                    None,
                );
                let picture = match picture {
                    Ok(picture) => picture,
                    Err(error) => {
                        failures.push(format!("picker 960x{height}, DPR {dpr}: {error}"));
                        break;
                    }
                };
                let draws = picture.frame.draws.len()
                    + usize::from(picture.backdrop.is_some())
                    + usize::from(picture.world_cover.is_some());
                let bytes = picture
                    .resources
                    .iter()
                    .map(|r| r.image.pixels.len())
                    .sum::<usize>();
                assert!(draws <= 512);
                assert!(bytes <= 13 * 1024 * 1024);
                assert!(
                    picture
                        .resources
                        .iter()
                        .all(|r| r.asset.width <= 2048 && r.asset.height <= 2048)
                );
                assert_eq!(picture.frame.viewport_scale, dpr);
                let resolved = login.picker.panel.resolve([960., height]).unwrap();
                let row = resolved
                    .iter()
                    .find(|r| matches!(r.control.kind, ControlKind::ListRow { .. }))
                    .unwrap();
                let r = row.rect;
                let point = mouse_client_position([(r.x + 12.) * dpr, (r.y + 12.) * dpr], dpr);
                assert!(row.hit(point));
                assert!(
                    picture
                        .frame
                        .draws
                        .iter()
                        .any(|d| point[0] >= d.destination.x
                            && point[0] < d.destination.x + d.destination.width
                            && point[1] >= d.destination.y
                            && point[1] < d.destination.y + d.destination.height)
                );
                let mut hits = PanelInputState::default();
                hits.sync(&login.picker.panel, &mut MenuInputState::default());
                hits.pointer(&login.picker.panel, &resolved, point, Some(true));
                assert_eq!(
                    hits.pointer(&login.picker.panel, &resolved, point, Some(false))
                        .unwrap()
                        .control,
                    row.control.id
                );
                eprintln!("picker 960x{height}, DPR {dpr}: {draws} draws, {bytes} bytes");
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn picker_reopening_inherits_the_current_opener_modality() {
    use slither_client::login_controls::SERVER;
    let mut login = NativeLogin::new("Focus").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, false, false).unwrap();
    }
    login.prepare([960, 720], false, None).unwrap();
    login.controls.keyboard_navigation();
    login.focus(SERVER);
    login.activate(false);
    login.step(9616., false, false).unwrap();
    assert!(login.picker.controls.focus().is_some());
    assert_eq!(
        login.picker.controls.visible_focus(),
        login.picker.controls.focus()
    );
    login.picker.policy.close();
    for now in (9632..=10000).step_by(16) {
        login.step(now as f64, false, false).unwrap();
    }
    assert!(!login.picker.policy.visible());
    login.prepare([960, 720], false, None).unwrap();
    let controls = login.panel.resolve([960., 720.]).unwrap();
    let r = controls.iter().find(|r| r.control.id == SERVER).unwrap();
    let point = [r.rect.x + r.rect.width / 2., r.rect.y + r.rect.height / 2.];
    login.pointer(point, Some(true), false);
    login.pointer(point, Some(false), false);
    assert!(login.picker.policy.is_open());
    assert!(login.picker.controls.focus().is_some());
    assert!(login.picker.controls.visible_focus().is_none());
    login.picker.controls.tab(&login.picker.panel, false);
    assert_eq!(
        login.picker.controls.visible_focus(),
        login.picker.controls.focus()
    );
}

#[test]
fn text_navigation_reveals_pointer_focused_fields_without_activating() {
    for (field, editor, value, official) in [
        (SEARCH, SEARCH_EDITOR, "123", true),
        (ADDRESS, ADDRESS_EDITOR, "192.0.2.9:445", false),
    ] {
        for (navigation, code) in [
            (NamedKey::ArrowLeft, KeyCode::ArrowLeft),
            (NamedKey::ArrowRight, KeyCode::ArrowRight),
            (NamedKey::Home, KeyCode::Home),
            (NamedKey::End, KeyCode::End),
        ] {
            let mut p = NativeServerPicker::default();
            p.policy.start_directory(official);
            p.open();
            p.policy.alpha = 1.;
            p.sync();
            let resolved = p.panel.resolve(p.viewport).unwrap();
            let r = resolved.iter().find(|r| r.control.id == field).unwrap();
            let point = [r.rect.x + r.rect.width / 2., r.rect.y + r.rect.height / 2.];
            let mut text = slither_wgpu_slice::text::TextCache::menu();
            p.pointer(&mut text, point, Some(true)).unwrap();
            p.pointer(&mut text, point, Some(false)).unwrap();
            p.input.state.apply(EditIntent::KeyText(value.into()));
            assert_eq!(p.controls.focus(), Some(field));
            assert_eq!(p.controls.visible_focus(), None);
            let effects = key(&mut p, navigation, code);
            assert!(
                effects
                    .iter()
                    .all(|e| !matches!(e, HostEditEffect::Activate(_)))
            );
            assert_eq!(p.controls.visible_focus(), Some(field));
            assert_eq!(p.input.state.editor(editor).text(), value);
            assert!(p.policy.is_open());
            assert_eq!(p.policy.selection, ServerPreference::Automatic);
            assert!(p.policy.custom.is_empty());
            assert_eq!(p.policy.take_directory_request(), official);
        }
    }
}
