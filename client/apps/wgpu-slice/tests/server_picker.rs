//! Authored synthetic directory/ping injection; no windows, GPU, or sockets.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{
    desktop_input::mouse_client_position,
    login_controls::{NICKNAME, SERVER},
    menu_controls::*,
    menu_input::HostEditEffect,
    menu_layout::MenuElement as E,
    server_directory::Server,
    server_picker::*,
    start_page::StartMenuIntent,
};
use slither_wgpu_slice::{login_native::NativeLogin, server_picker_native::*};
use std::{collections::BTreeMap, net::SocketAddr};

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
fn settled(admission: bool) -> NativeLogin {
    let mut login = NativeLogin::new("Player").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, admission, false).unwrap();
    }
    for _ in 0..2 {
        login.prepare([960, 720], admission, None).unwrap();
    }
    login
}
/// Open through the shared Server action and finish the entry fade.
fn opened(official: bool) -> NativeLogin {
    let mut login = settled(true);
    if official {
        login.picker.policy.start_directory();
    }
    login.open_picker();
    login.picker.policy.alpha = 1.;
    login.prepare([960, 720], true, None).unwrap();
    login
}
fn input(login: &mut NativeLogin, event: egui::Event) {
    login.settings_input(egui::RawInput {
        events: vec![event],
        ..Default::default()
    });
}
fn key(login: &mut NativeLogin, key: egui::Key, pressed: bool) {
    input(
        login,
        egui::Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers: Default::default(),
        },
    );
}
fn press(login: &mut NativeLogin, k: egui::Key) {
    key(login, k, true);
    key(login, k, false);
}
fn widget(login: &NativeLogin, id: ControlId) -> egui::Rect {
    login
        .picker
        .ui
        .controls
        .iter()
        .find(|c| c.0 == id)
        .unwrap_or_else(|| panic!("missing picker control {id:?}"))
        .1
}
fn click_at(login: &mut NativeLogin, point: [f64; 2]) -> Vec<HostEditEffect> {
    let mut effects = login.pointer(point, Some(true), true);
    effects.extend(login.pointer(point, Some(false), true));
    effects
}
fn click(login: &mut NativeLogin, id: ControlId) {
    let r = widget(login, id);
    click_at(login, [f64::from(r.center().x), f64::from(r.center().y)]);
}
fn fade(login: &mut NativeLogin, from: u32, to: u32) {
    for now in (from..=to).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
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
        let effects = click_at(&mut login, mouse_client_position(physical, dpr));
        assert!(effects.contains(&HostEditEffect::ReleasePointerBoost));
        // egui-winit owns IME admission for the picker field it focuses next.
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, HostEditEffect::ImeAllowed(_)))
        );
        assert!(login.picker.policy.is_open());
        assert!(!login.picker.policy.take_directory_request());
        assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
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
        // The egui row covers exactly the shared hit rectangle.
        let painted = widget(&login, row.control.id);
        assert!(painted.contains(egui::pos2(point[0] as f32, point[1] as f32)));
        assert!(!painted.contains(egui::pos2(
            (row.rect.x + row.rect.width) as f32 + 0.5,
            row.rect.y as f32
        )));
        click_at(&mut login, point);
        assert_eq!(login.picker.policy.selection.official_id(), Some(5));
        fade(&mut login, 656, 1200);
        assert!(!login.picker.policy.visible());
        assert_eq!(login.focused(), Some(NICKNAME));
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
    assert_eq!(login.focused(), Some(NICKNAME));
    assert!(!login.state.view(false).spinner_visible);
}

#[test]
fn offline_tab_finds_shared_server_link_and_paints_its_focus_without_enabling_play() {
    use slither_client::login_controls::{CHANGE_SKIN, PLAY, SETTINGS};
    use slither_wgpu_slice::menu_input_native::KeyInput;
    use winit::{
        event::ElementState,
        keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    };
    let mut login = NativeLogin::new("").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, false, false).unwrap();
    }
    login.prepare([960, 720], false, None).unwrap();
    let unfocused = login.egui.prepared_shapes().len();
    let logical = Key::Named(NamedKey::Tab);
    let tab = KeyInput {
        logical: &logical,
        physical: PhysicalKey::Code(KeyCode::Tab),
        text: None,
        state: ElementState::Pressed,
        repeat: false,
    };
    assert!(login.key(tab.clone(), false).consumed);
    assert_eq!(login.focused(), Some(CHANGE_SKIN));
    login.key(tab.clone(), false);
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
fn picker_controls_and_ime_cursor_stay_inside_windows_at_supported_dprs() {
    for dpr in [1., 1.5, 2., 3., 4., 5.] {
        let mut login = settled(true);
        login
            .picker
            .policy
            .replace(&servers(), &BTreeMap::from([(0, 12), (1, 65)]))
            .unwrap();
        login.open_picker();
        login.picker.policy.alpha = 1.;
        for logical in [
            [320., 240.],
            [640., 480.],
            [960., 540.],
            [960., 720.],
            [1280., 720.],
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
            for offset in [isize::MAX, isize::MIN] {
                login.picker.policy.scroll(offset);
                login
                    .prepare_host(extent, dpr, true, None)
                    .unwrap_or_else(|e| panic!("DPR {dpr}, {logical:?}: {e}"));
                let window = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(logical[0] as f32, logical[1] as f32),
                );
                let rows = login
                    .picker
                    .ui
                    .controls
                    .iter()
                    .filter(|c| c.0.0 >= 16 && c.0.0 < 8192)
                    .count();
                assert_eq!(rows, 6, "DPR {dpr}, {logical:?}");
                for (id, r, _) in &login.picker.ui.controls {
                    assert!(
                        r.is_positive() && window.expand(0.01).contains_rect(*r),
                        "DPR {dpr}, {logical:?}, {id:?} {r:?}"
                    );
                }
                assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
                let ime = login.egui.output().and_then(|o| o.ime).unwrap();
                assert!(
                    widget(&login, ADDRESS)
                        .expand(0.01)
                        .contains_rect(ime.cursor_rect),
                    "DPR {dpr}, {logical:?}, {ime:?}"
                );
            }
        }
    }
}

#[test]
fn login_paints_below_modal_dimming_and_only_the_window_has_rows() {
    let mut login = settled(true);
    login
        .picker
        .policy
        .replace(&servers(), &BTreeMap::from([(0, 12), (1, 55), (2, 160)]))
        .unwrap();
    login.open_picker();
    fade(&mut login, 9616, 10000);
    for offset in [0, 994, 2000, -2000] {
        login.picker.policy.scroll(offset);
        login.prepare([960, 720], true, None).unwrap();
        let shapes = login.egui.prepared_shapes();
        let backdrop = egui::Color32::from_rgb(24, 34, 48).gamma_multiply(0.88);
        let dim = shapes
            .iter()
            .position(|s| {
                matches!(&s.shape, egui::Shape::Rect(r)
                    if r.fill == backdrop && r.rect.size() == egui::vec2(960., 720.))
            })
            .expect("modal dimming");
        // Login art (wordmark tiles, skin) is the only textured content.
        let textured = |s: &egui::epaint::ClippedShape| match &s.shape {
            egui::Shape::Rect(r) => r.fill_texture_id() != egui::TextureId::default(),
            egui::Shape::Mesh(m) => m.texture_id != egui::TextureId::default(),
            _ => false,
        };
        assert!(shapes[..dim].iter().any(textured), "login wordmark");
        assert!(
            !shapes[dim..].iter().any(textured),
            "Login art must be below modal dimming and surface"
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

#[test]
fn clicking_numeric_search_deliberately_clears_it_even_when_already_focused() {
    let mut login = opened(true);
    login.picker.policy.replace(&[], &BTreeMap::new()).unwrap();
    login.prepare([960, 720], true, None).unwrap();
    click(&mut login, SEARCH);
    for query in ["909", "808"] {
        input(&mut login, egui::Event::Text(query.into()));
        assert_eq!(login.picker.ui.search, query);
        assert_eq!(login.picker.ui.focused(), Some(SEARCH));
        click(&mut login, SEARCH);
        assert_eq!(login.picker.ui.search, "");
        assert_eq!(login.picker.policy.search, "");
        assert!(login.picker.policy.is_open());
        assert_eq!(login.picker.policy.selection, ServerPreference::Automatic);
    }
}

#[test]
fn keyboard_traps_focus_numeric_edits_cannot_play_and_preference_failure_is_in_memory() {
    let mut login = opened(true);
    login
        .picker
        .policy
        .replace(&servers(), &BTreeMap::new())
        .unwrap();
    login.prepare([960, 720], true, None).unwrap();
    click(&mut login, SEARCH);
    press(&mut login, egui::Key::ArrowDown);
    assert_eq!(login.picker.ui.focused(), Some(ControlId(16)));
    press(&mut login, egui::Key::End);
    assert_eq!(login.picker.ui.focused(), Some(ControlId(2015)));
    press(&mut login, egui::Key::Tab);
    assert_eq!(login.picker.ui.focused(), Some(CLOSE));
    press(&mut login, egui::Key::Tab);
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
    for _ in 0..2 {
        press(&mut login, egui::Key::Tab);
    }
    assert_eq!(login.picker.ui.focused(), Some(SEARCH));
    input(&mut login, egui::Event::Text("19x99".into()));
    assert_eq!(login.picker.policy.selection.official_id(), Some(1999));
    assert!(!login.picker.policy.is_open());
    let directory =
        std::env::temp_dir().join(format!("slither-server-pref-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("choice");
    assert_eq!(read_preference(&path), ServerPreference::Automatic);
    write_preference(&path, &login.picker.policy.take_preference().unwrap()).unwrap();
    assert_eq!(read_preference(&path), ServerPreference::Official(1999));
    assert!(write_preference(&directory, "1:auto").is_err());
    assert_eq!(login.picker.policy.selection.official_id(), Some(1999));
    std::fs::write(&path, "1:77".repeat(1000)).unwrap();
    assert_eq!(read_preference(&path), ServerPreference::Automatic);
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[test]
fn open_picker_rows_stay_clickable_at_dpr_five_to_eight() {
    for height in [720., 540.] {
        let mut login = settled(true);
        login
            .picker
            .policy
            .replace(&servers(), &BTreeMap::from([(0, 12), (1, 65)]))
            .unwrap();
        login.open_picker();
        login.picker.policy.alpha = 1.;
        for dpr in [5., 6., 7., 8., 5.] {
            for offset in [0, isize::MAX, isize::MIN] {
                login.picker.policy.scroll(offset);
                let extent = [(960. * dpr) as u32, (height * dpr) as u32];
                let picture = login
                    .prepare_host(extent, dpr, true, None)
                    .unwrap_or_else(|e| panic!("picker 960x{height}, DPR {dpr}: {e}"));
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
                    widget(&login, row.control.id)
                        .contains(egui::pos2(point[0] as f32, point[1] as f32))
                );
            }
        }
        // The last window is the top one; its first row is the fastest server.
        let resolved = login.picker.panel.resolve([960., height]).unwrap();
        let row = resolved
            .iter()
            .find(|r| matches!(r.control.kind, ControlKind::ListRow { .. }))
            .unwrap();
        let point = mouse_client_position([(row.rect.x + 12.) * 5., (row.rect.y + 12.) * 5.], 5.);
        click_at(&mut login, point);
        assert_eq!(login.picker.policy.selection.official_id(), Some(5));
    }
}

#[test]
fn picker_reopening_inherits_the_current_opener_modality() {
    let mut login = settled(false);
    press(&mut login, egui::Key::Tab);
    login.focus(SERVER);
    press(&mut login, egui::Key::Enter);
    assert!(login.picker.policy.is_open());
    assert!(login.picker.ui.keyboard_focus);
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
    login.picker.policy.close();
    fade(&mut login, 9616, 10000);
    assert!(!login.picker.policy.visible());
    login.prepare([960, 720], false, None).unwrap();
    let r = login
        .login_ui
        .controls
        .iter()
        .find(|c| c.0 == SERVER)
        .unwrap()
        .1;
    click_at(
        &mut login,
        [f64::from(r.center().x), f64::from(r.center().y)],
    );
    assert!(login.picker.policy.is_open());
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
    assert!(!login.picker.ui.keyboard_focus);
    press(&mut login, egui::Key::Tab);
    assert!(login.picker.ui.keyboard_focus);
    assert_eq!(login.picker.ui.focused(), Some(ADD_CUSTOM));
}

#[test]
fn text_navigation_reveals_pointer_focused_fields_without_activating() {
    for (field, value, official) in [(SEARCH, "123", true), (ADDRESS, "192.0.2.9:445", false)] {
        for navigation in [
            egui::Key::ArrowLeft,
            egui::Key::ArrowRight,
            egui::Key::Home,
            egui::Key::End,
        ] {
            let mut login = opened(official);
            click(&mut login, field);
            input(&mut login, egui::Event::Text(value.into()));
            assert_eq!(login.picker.ui.focused(), Some(field));
            assert!(!login.picker.ui.keyboard_focus);
            press(&mut login, navigation);
            assert_eq!(login.picker.ui.focused(), Some(field));
            assert!(login.picker.ui.keyboard_focus);
            assert_eq!(login.picker.ui.text(field), Some(value));
            assert!(login.picker.policy.is_open());
            assert_eq!(login.picker.policy.selection, ServerPreference::Automatic);
            assert!(login.picker.policy.custom.is_empty());
            assert_eq!(login.picker.policy.take_directory_request(), official);
        }
    }
}

#[test]
fn arrows_never_move_focus_out_of_the_address_or_between_buttons() {
    let mut login = opened(false);
    for key in [
        egui::Key::ArrowUp,
        egui::Key::ArrowDown,
        egui::Key::PageUp,
        egui::Key::PageDown,
    ] {
        press(&mut login, key);
        assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
    }
    press(&mut login, egui::Key::Tab);
    for key in [egui::Key::ArrowLeft, egui::Key::ArrowRight] {
        press(&mut login, key);
        assert_eq!(login.picker.ui.focused(), Some(ADD_CUSTOM));
    }
    // Without rows, list navigation reaches the address in custom context.
    press(&mut login, egui::Key::ArrowDown);
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
}

#[test]
fn wheel_bursts_scroll_without_a_pass_per_event_and_keep_fractions() {
    let mut login = opened(true);
    login
        .picker
        .policy
        .replace(&servers(), &BTreeMap::new())
        .unwrap();
    login.prepare([960, 720], true, None).unwrap();
    let context = login.egui.context().unwrap().clone();
    let passes = context.cumulative_pass_nr();
    let wheel = |unit, y| egui::Event::MouseWheel {
        unit,
        delta: egui::vec2(0., y),
        phase: egui::TouchPhase::Move,
        modifiers: Default::default(),
        source: egui::MouseWheelSource::Unknown,
    };
    login.settings_input(egui::RawInput {
        events: (0..100)
            .map(|_| wheel(egui::MouseWheelUnit::Line, -1.))
            .collect(),
        ..Default::default()
    });
    assert_eq!(context.cumulative_pass_nr(), passes);
    assert_eq!(login.picker.policy.list.window(2000).start, 100);
    // Point deltas are logical; one row is 46 physical pixels at any DPR.
    for _ in 0..3 {
        input(&mut login, wheel(egui::MouseWheelUnit::Point, -23.));
    }
    assert_eq!(context.cumulative_pass_nr(), passes);
    assert_eq!(login.picker.policy.list.window(2000).start, 101);
    login.prepare([960, 720], true, None).unwrap();
    assert!(
        login
            .picker
            .ui
            .controls
            .iter()
            .any(|c| c.0 == ControlId(16 + 101))
    );
}

#[test]
fn backdrop_and_escape_close_while_blank_panel_space_shields() {
    for close in ["backdrop", "escape", "close"] {
        let mut login = opened(false);
        let surface = login.picker.panel.resolve([960., 720.]).unwrap()[1].rect;
        click_at(
            &mut login,
            [surface.x + 4., surface.y + surface.height - 4.],
        );
        assert!(login.picker.policy.is_open());
        match close {
            "backdrop" => {
                click_at(&mut login, [4., 4.]);
            }
            "escape" => press(&mut login, egui::Key::Escape),
            _ => click(&mut login, CLOSE),
        }
        assert!(!login.picker.policy.is_open(), "{close}");
        // Outgoing controls are disabled: clicks during the fade cannot act.
        click(&mut login, ADD_CUSTOM);
        assert!(login.picker.policy.custom_error.is_empty());
        assert!(login.intercepts_pointer([4., 4.]));
        fade(&mut login, 9616, 10000);
        assert!(!login.picker.policy.visible());
        assert_eq!(login.focused(), Some(NICKNAME));
        assert!(!login.state.settings.visible());
    }
}

#[test]
fn intercepts_pointer_agrees_with_dispatch_through_open_and_close() {
    let mut login = settled(true);
    for now in [9616, 9632] {
        assert_eq!(
            login.intercepts_pointer([480., 360.]),
            login.egui_owns_input()
        );
        login.step(f64::from(now), true, false).unwrap();
    }
    login.open_picker();
    for now in (9648..=10400).step_by(16) {
        if now == 10000 {
            login.picker.policy.close();
        }
        login.step(f64::from(now), true, false).unwrap();
        login.prepare([960, 720], true, None).unwrap();
        assert!(login.egui.active());
        assert_eq!(
            login.intercepts_pointer([480., 360.]),
            login.egui_owns_input()
        );
        assert!(login.egui_owns_input());
    }
    assert!(!login.picker.policy.visible());
}

#[test]
fn handoff_round_trip_keeps_login_state_and_reopens_with_a_fresh_search() {
    let mut login = settled(true);
    select_all_and_type(&mut login, "Nick");
    assert_eq!(login.state.view(true).nickname, "Nick");
    login.picker.policy.start_directory();
    login
        .picker
        .policy
        .replace(&servers(), &BTreeMap::new())
        .unwrap();
    login.focus(SERVER);
    let effects = {
        press(&mut login, egui::Key::Enter);
        login.take_egui_effects()
    };
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, HostEditEffect::ImeAllowed(_)))
    );
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
    input(&mut login, egui::Event::Text("192.0.2.3:445".into()));
    click(&mut login, SEARCH);
    // No directory ID matches, so search neither selects nor closes.
    input(&mut login, egui::Event::Text("9999".into()));
    assert!(login.picker.policy.is_open());
    press(&mut login, egui::Key::Escape);
    for now in (9616..=10000).step_by(16) {
        let (_, effects) = login.step(f64::from(now), true, false).unwrap();
        // egui-winit keeps IME admission for the nickname it refocuses.
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, HostEditEffect::ImeAllowed(_)))
        );
    }
    assert!(!login.picker.policy.visible());
    assert_eq!(login.focused(), Some(NICKNAME));
    login.prepare([960, 720], true, None).unwrap();
    assert!(login.egui.output().unwrap().ime.is_some());
    assert_eq!(login.displayed_nickname(), "Nick");
    input(&mut login, egui::Event::Text("!".into()));
    assert_eq!(login.state.view(true).nickname, "Nick!");
    login.focus(SERVER);
    press(&mut login, egui::Key::Enter);
    assert!(login.picker.policy.is_open());
    assert_eq!(login.picker.ui.address, "192.0.2.3:445");
    assert_eq!(login.picker.ui.search, "");
    assert_eq!(login.picker.policy.selection, ServerPreference::Automatic);
    // Choosing a custom server closes and makes direct Play available.
    press(&mut login, egui::Key::Enter);
    click(&mut login, ControlId(16));
    assert_eq!(
        login.picker.policy.selection.custom_address(),
        Some("192.0.2.3:445")
    );
    assert!(login.picker.policy.can_play());
    fade(&mut login, 10016, 10400);
    press(&mut login, egui::Key::Enter);
    assert!(matches!(
        login.step(10416., true, false).unwrap().0.as_slice(),
        [StartMenuIntent::Play { nickname, .. }] if nickname == "Nick!"
    ));
}

fn select_all_and_type(login: &mut NativeLogin, text: &str) {
    for pressed in [true, false] {
        input(
            login,
            egui::Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed,
                repeat: false,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    command: true,
                    ..Default::default()
                },
            },
        );
    }
    input(login, egui::Event::Text(text.into()));
}

#[test]
fn address_ime_is_bounded_cancellable_and_outside_undo_history() {
    for cancel in ["disabled", "empty", "focus", "escape", "commit"] {
        let mut login = opened(false);
        input(&mut login, egui::Event::Text("192.0.2.1:80".into()));
        login.step(11200., true, false).unwrap();
        login.prepare([960, 720], true, None).unwrap();
        let context = login.egui.context().unwrap().clone();
        let passes = context.cumulative_pass_nr();
        for n in 0..200 {
            input(
                &mut login,
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: format!("候補{n}"),
                    active_range_chars: Some(0..1),
                }),
            );
        }
        // Adjacent preedits coalesce until redraw.
        assert_eq!(context.cumulative_pass_nr(), passes);
        login.prepare([960, 720], true, None).unwrap();
        assert!(login.picker.ui.composing);
        assert!(login.picker.ui.address.ends_with("候補199"));
        // Composition owns Enter and Tab.
        press(&mut login, egui::Key::Enter);
        press(&mut login, egui::Key::Tab);
        assert!(login.picker.policy.custom.is_empty());
        assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
        match cancel {
            "disabled" => {
                let response = login.egui.window_event(
                    &winit::event::WindowEvent::Ime(winit::event::Ime::Disabled),
                    1.,
                    || panic!("native Disabled must be handled before the window adapter"),
                );
                assert!(response.repaint);
                login.settings_input(egui::RawInput::default());
            }
            "empty" => input(
                &mut login,
                egui::Event::Ime(egui::ImeEvent::Preedit {
                    text: String::new(),
                    active_range_chars: None,
                }),
            ),
            "focus" => {
                input(&mut login, egui::Event::WindowFocused(false));
                input(&mut login, egui::Event::WindowFocused(true));
            }
            "escape" => press(&mut login, egui::Key::Escape),
            _ => input(
                &mut login,
                egui::Event::Ime(egui::ImeEvent::Commit("\u{7f}x".into())),
            ),
        }
        login.prepare([960, 720], true, None).unwrap();
        assert!(!login.picker.ui.composing, "{cancel}");
        assert!(login.picker.policy.is_open(), "{cancel}");
        let committed = if cancel == "commit" {
            "192.0.2.1:80 x"
        } else {
            "192.0.2.1:80"
        };
        assert_eq!(login.picker.ui.address, committed, "{cancel}");
        let command = egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        };
        for _ in 0..4 {
            input(
                &mut login,
                egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: Some(egui::Key::Z),
                    pressed: true,
                    repeat: false,
                    modifiers: command,
                },
            );
            let text = login.picker.ui.address.clone();
            assert!(!text.contains('候'), "{cancel}: {text:?}");
        }
        assert_eq!(login.picker.ui.address, "", "{cancel}");
    }
}

#[test]
fn numeric_search_ime_commit_uses_the_digit_rule_and_selects_once() {
    let mut login = opened(true);
    login
        .picker
        .policy
        .replace(&servers(), &BTreeMap::new())
        .unwrap();
    login.prepare([960, 720], true, None).unwrap();
    click(&mut login, SEARCH);
    input(
        &mut login,
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "４２".into(),
            active_range_chars: None,
        }),
    );
    login.prepare([960, 720], true, None).unwrap();
    // Provisional text never selects a server.
    assert!(login.picker.policy.is_open());
    input(
        &mut login,
        egui::Event::Ime(egui::ImeEvent::Commit("４2a7".into())),
    );
    assert_eq!(login.picker.policy.selection.official_id(), Some(27));
    assert!(!login.picker.policy.is_open());
}

/// A pointer close can end the native composition without any IME event the
/// picker sees (`set_ime_allowed(false)` need not report Disabled). The closed
/// picker restores the snapshot, and the next open starts without composition.
#[test]
fn closing_while_composing_restores_the_field_and_reopens_clean() {
    for close in ["backdrop", "close"] {
        let mut login = opened(false);
        input(&mut login, egui::Event::Text("192.0.2.1:80".into()));
        login.step(11200., true, false).unwrap();
        login.prepare([960, 720], true, None).unwrap();
        input(
            &mut login,
            egui::Event::Ime(egui::ImeEvent::Preedit {
                text: "候補".into(),
                active_range_chars: Some(0..1),
            }),
        );
        login.prepare([960, 720], true, None).unwrap();
        assert!(login.picker.ui.composing, "{close}");
        match close {
            "backdrop" => {
                click_at(&mut login, [4., 4.]);
            }
            _ => click(&mut login, CLOSE),
        }
        assert!(!login.picker.policy.is_open(), "{close}");
        assert!(!login.picker.ui.composing, "{close}");
        assert_eq!(login.picker.ui.address, "192.0.2.1:80", "{close}");
        fade(&mut login, 11216, 12000);
        assert!(!login.picker.policy.visible(), "{close}");
        login.open_picker();
        login.picker.policy.alpha = 1.;
        login.prepare([960, 720], true, None).unwrap();
        assert!(!login.picker.ui.composing, "{close}");
        assert_eq!(login.picker.ui.focused(), Some(ADDRESS), "{close}");
        // Enter adds, and Escape closes, as in any fresh open.
        press(&mut login, egui::Key::Enter);
        assert_eq!(login.picker.policy.custom, ["192.0.2.1:80"], "{close}");
        press(&mut login, egui::Key::Escape);
        assert!(!login.picker.policy.is_open(), "{close}");
    }
}

fn undo(login: &mut NativeLogin) {
    let command = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    input(
        login,
        egui::Event::Key {
            key: egui::Key::Z,
            physical_key: Some(egui::Key::Z),
            pressed: true,
            repeat: false,
            modifiers: command,
        },
    );
}
fn preedit(login: &mut NativeLogin, text: &str) {
    input(
        login,
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: text.into(),
            active_range_chars: None,
        }),
    );
    login.prepare([960, 720], true, None).unwrap();
}

/// A press held across a panel generation change cannot click the control
/// that now reuses its ID, as the shared panel input cancelled capture.
#[test]
fn a_press_held_across_a_generation_change_never_clicks() {
    // Keyboard Remove while the mouse holds the same Remove button.
    for redraw in [false, true] {
        let mut login = opened(false);
        for address in ["192.0.2.1:80", "192.0.2.2:80"] {
            login.picker.policy.add_custom(address.into()).unwrap();
        }
        login.prepare([960, 720], true, None).unwrap();
        login.prepare([960, 720], true, None).unwrap();
        for _ in 0..12 {
            if login.picker.ui.focused() == Some(ControlId(8192)) {
                break;
            }
            press(&mut login, egui::Key::Tab);
        }
        assert_eq!(login.picker.ui.focused(), Some(ControlId(8192)));
        let r = widget(&login, ControlId(8192));
        let p = [f64::from(r.center().x), f64::from(r.center().y)];
        login.pointer(p, Some(true), true);
        press(&mut login, egui::Key::Space);
        assert_eq!(login.picker.policy.custom, ["192.0.2.2:80"]);
        if redraw {
            login.prepare([960, 720], true, None).unwrap();
        }
        login.pointer(p, Some(false), true);
        assert_eq!(login.picker.policy.custom, ["192.0.2.2:80"], "{redraw}");
        // The next ordinary click on the same button acts.
        login.prepare([960, 720], true, None).unwrap();
        click(&mut login, ControlId(8192));
        assert!(login.picker.policy.custom.is_empty(), "{redraw}");
    }
    // Directory replacement under a held row press.
    let mut login = opened(true);
    login
        .picker
        .policy
        .replace(&servers(), &BTreeMap::new())
        .unwrap();
    login.prepare([960, 720], true, None).unwrap();
    login.prepare([960, 720], true, None).unwrap();
    let r = widget(&login, ControlId(16));
    let p = [f64::from(r.center().x), f64::from(r.center().y)];
    login.pointer(p, Some(true), true);
    let replacement = [Server {
        id: 3000,
        cluster: 0,
        address: SocketAddr::from(([192, 0, 2, 2], 3000)),
        population: 1,
        active: true,
    }];
    login
        .picker
        .policy
        .replace(&replacement, &BTreeMap::new())
        .unwrap();
    login.prepare([960, 720], true, None).unwrap();
    login.pointer(p, Some(false), true);
    assert!(login.picker.policy.is_open());
    login.prepare([960, 720], true, None).unwrap();
    click(&mut login, ControlId(16));
    assert_eq!(login.picker.policy.selection.official_id(), Some(3000));
}

fn pointer_button(
    login: &mut NativeLogin,
    point: [f64; 2],
    button: egui::PointerButton,
    pressed: bool,
) {
    input(
        login,
        egui::Event::PointerButton {
            pos: egui::pos2(point[0] as f32, point[1] as f32),
            button,
            pressed,
            modifiers: Default::default(),
        },
    );
}
/// One official row at the first list position; returns its centre.
fn single_row(login: &mut NativeLogin, id: u16) -> [f64; 2] {
    replace_row(login, id);
    login.prepare([960, 720], true, None).unwrap();
    login.prepare([960, 720], true, None).unwrap();
    let r = widget(login, ControlId(16));
    let p = [f64::from(r.center().x), f64::from(r.center().y)];
    login.pointer(p, None, true);
    login.prepare([960, 720], true, None).unwrap();
    p
}
fn replace_row(login: &mut NativeLogin, id: u16) {
    let row = [Server {
        id,
        cluster: 0,
        address: SocketAddr::from(([192, 0, 2, 2], id)),
        population: 1,
        active: true,
    }];
    login.picker.policy.replace(&row, &BTreeMap::new()).unwrap();
}

/// Only a primary press held across the generation is stale: a held secondary
/// button, or a primary press the host already cancelled, never suppresses a
/// fresh primary click, with or without a redraw first.
#[test]
fn a_fresh_primary_click_after_a_generation_change_acts() {
    use egui::PointerButton::{Primary, Secondary};
    for cancelled in [false, true] {
        for replace in [false, true] {
            let mut login = opened(true);
            let p = single_row(&mut login, 2000);
            if cancelled {
                pointer_button(&mut login, p, Primary, true);
                // The native resize path cancels the egui pointer without a
                // picker pass.
                login.event(
                    &winit::event::WindowEvent::Resized(winit::dpi::PhysicalSize::new(960, 720)),
                    true,
                );
            } else {
                pointer_button(&mut login, p, Secondary, true);
            }
            if replace {
                replace_row(&mut login, 3000);
                if !cancelled {
                    login.prepare([960, 720], true, None).unwrap();
                }
            }
            pointer_button(&mut login, p, Primary, true);
            pointer_button(&mut login, p, Primary, false);
            assert_eq!(
                login.picker.policy.selection.official_id(),
                Some(if replace { 3000 } else { 2000 }),
                "cancelled={cancelled} replace={replace}"
            );
        }
    }
}

/// Add reads committed text only: a click that takes focus from a composing
/// field restores its snapshot before Add runs.
#[test]
fn add_during_composition_uses_only_committed_text() {
    for committed in ["", "192.0.2.1:80"] {
        let mut login = opened(false);
        if !committed.is_empty() {
            input(&mut login, egui::Event::Text(committed.into()));
        }
        login.step(11200., true, false).unwrap();
        login.prepare([960, 720], true, None).unwrap();
        preedit(&mut login, "192.0.2.99:445");
        assert!(login.picker.ui.composing);
        click(&mut login, ADD_CUSTOM);
        assert!(!login.picker.ui.composing, "{committed:?}");
        let saved: &[&str] = if committed.is_empty() {
            &[]
        } else {
            &[committed]
        };
        assert_eq!(login.picker.policy.custom, saved, "{committed:?}");
        assert!(
            !login.picker.policy.custom.iter().any(|c| c.contains("99")),
            "{committed:?}"
        );
        assert!(!login.picker.ui.address.contains("99"), "{committed:?}");
    }
}

/// Moving focus to the other field cancels the composition like any other
/// cancellation: committed text, no candidate in undo history.
#[test]
fn focus_moving_to_another_field_cancels_the_composition() {
    let mut login = opened(true);
    input(&mut login, egui::Event::Text("192.0.2.1:80".into()));
    login.step(11200., true, false).unwrap();
    login.prepare([960, 720], true, None).unwrap();
    preedit(&mut login, "候補");
    assert!(login.picker.ui.address.contains('候'));
    click(&mut login, SEARCH);
    assert!(!login.picker.ui.composing);
    assert_eq!(login.picker.ui.address, "192.0.2.1:80");
    assert_eq!(login.picker.ui.focused(), Some(SEARCH));
    click(&mut login, ADDRESS);
    for _ in 0..3 {
        undo(&mut login);
        assert!(!login.picker.ui.address.contains('候'));
    }
    assert_eq!(login.picker.ui.address, "");
}

/// Queued wheel input moves the row window before redraw; the next key must
/// not navigate from a row that has already left it.
#[test]
fn keys_after_a_queued_wheel_use_the_current_row_window() {
    let mut login = opened(true);
    login
        .picker
        .policy
        .replace(&servers(), &BTreeMap::new())
        .unwrap();
    login.prepare([960, 720], true, None).unwrap();
    click(&mut login, SEARCH);
    press(&mut login, egui::Key::ArrowDown);
    press(&mut login, egui::Key::End);
    assert_eq!(login.picker.ui.focused(), Some(ControlId(2015)));
    input(
        &mut login,
        egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0., 1.),
            phase: egui::TouchPhase::Move,
            modifiers: Default::default(),
            source: egui::MouseWheelSource::Unknown,
        },
    );
    assert_eq!(login.picker.policy.list.window(2000).start, 1993);
    // The focused last row left the window: ArrowDown starts from the top,
    // as the shared panel input did after dropping that focus.
    press(&mut login, egui::Key::ArrowDown);
    assert_eq!(login.picker.ui.focused(), Some(ControlId(16)));
}

/// A tap as egui-winit translates it: Touch, PointerMoved and a primary press
/// on start; Touch, the primary release and PointerGone on end.
fn tap(id: u64, p: [f64; 2], start: bool, end: bool) -> Vec<egui::Event> {
    let pos = egui::pos2(p[0] as f32, p[1] as f32);
    let touch = |phase| egui::Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
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
            button(false),
            egui::Event::PointerGone,
        ]);
    }
    events
}
fn batch(login: &mut NativeLogin, events: Vec<egui::Event>) {
    login.settings_input(egui::RawInput {
        events,
        ..Default::default()
    });
}

/// Desktop touch reaches the owning menu immediately and in order, like mouse
/// input, so a press is judged against the panel generation it started in.
#[test]
fn touch_input_is_dispatched_immediately_and_in_order() {
    use slither_wgpu_slice::egui_native::immediate_menu_input;
    use winit::event::{DeviceId, Touch, TouchPhase, WindowEvent};
    for phase in [
        TouchPhase::Started,
        TouchPhase::Moved,
        TouchPhase::Ended,
        TouchPhase::Cancelled,
    ] {
        assert!(immediate_menu_input(&WindowEvent::Touch(Touch {
            device_id: DeviceId::dummy(),
            phase,
            location: winit::dpi::PhysicalPosition::new(4., 4.),
            force: None,
            id: 1,
        })));
    }
    // A touch press dispatched before a directory replacement cannot select
    // the replacement; the next tap can.
    let mut login = opened(true);
    let p = single_row(&mut login, 2000);
    batch(&mut login, tap(1, p, true, false));
    replace_row(&mut login, 3000);
    login.prepare([960, 720], true, None).unwrap();
    batch(&mut login, tap(1, p, false, true));
    assert!(login.picker.policy.is_open());
    batch(&mut login, tap(2, p, true, true));
    assert_eq!(login.picker.policy.selection.official_id(), Some(3000));
    // A stale touch release and a fresh tap in one batch: only the release
    // is ignored.
    let mut login = opened(true);
    let p = single_row(&mut login, 2000);
    batch(&mut login, tap(1, p, true, false));
    replace_row(&mut login, 3000);
    login.prepare([960, 720], true, None).unwrap();
    let mut events = tap(1, p, false, true);
    events.extend(tap(2, p, true, true));
    batch(&mut login, events);
    assert_eq!(login.picker.policy.selection.official_id(), Some(3000));
    // The same batch after a keyboard Remove under the held touch, with no
    // directory replacement: the fresh tap removes the remaining entry.
    let mut login = opened(false);
    for address in ["192.0.2.1:80", "192.0.2.2:80"] {
        login.picker.policy.add_custom(address.into()).unwrap();
    }
    login.prepare([960, 720], true, None).unwrap();
    login.prepare([960, 720], true, None).unwrap();
    for _ in 0..12 {
        if login.picker.ui.focused() == Some(ControlId(8192)) {
            break;
        }
        press(&mut login, egui::Key::Tab);
    }
    let r = widget(&login, ControlId(8192));
    let p = [f64::from(r.center().x), f64::from(r.center().y)];
    batch(&mut login, tap(1, p, true, false));
    press(&mut login, egui::Key::Space);
    assert_eq!(login.picker.policy.custom, ["192.0.2.2:80"]);
    let mut events = tap(1, p, false, true);
    events.extend(tap(2, p, true, true));
    batch(&mut login, events);
    assert!(login.picker.policy.custom.is_empty());
}

/// Login text painted below the picker's full-window dimming (all of it while
/// the picker is closed): string, origin and galley bounds.
fn login_text(login: &NativeLogin, window: egui::Rect) -> Vec<(String, egui::Pos2, egui::Rect)> {
    let shapes = login.egui.prepared_shapes();
    let dim = shapes
        .iter()
        .position(|s| matches!(&s.shape, egui::Shape::Rect(r) if r.rect == window))
        .unwrap_or(shapes.len());
    shapes[..dim]
        .iter()
        .filter_map(|s| match &s.shape {
            egui::Shape::Text(t) => Some((t.galley.job.text.clone(), t.pos, t.galley.rect)),
            _ => None,
        })
        .collect()
}

/// The login under the picker stays on the egui renderer through entry, the
/// fades and exit, so its links and captions never shift between renderers.
#[test]
fn login_text_is_pixel_stable_while_the_picker_enters_and_leaves() {
    for dpr in [1., 1.5] {
        let mut login = settled(true);
        let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(960., 720.));
        login.prepare_host(extent, dpr, true, None).unwrap();
        let closed = login_text(&login, window);
        for caption in ["Change Skin", "Settings"] {
            assert!(
                closed.iter().any(|t| t.0.contains(caption)),
                "{dpr}: {caption} in {closed:?}"
            );
        }
        let r = login.layout.viewport_rect(E::Server);
        let physical = [(r.x + r.width / 2.) * dpr, (r.y + r.height / 2.) * dpr];
        click_at(&mut login, mouse_client_position(physical, dpr));
        assert!(login.picker.policy.is_open());
        let check = |login: &mut NativeLogin, now: u32, phase: &str| {
            login.step(f64::from(now), true, false).unwrap();
            login.prepare_host(extent, dpr, true, None).unwrap();
            assert_eq!(login_text(login, window), closed, "{dpr} {phase} {now}");
        };
        for now in (9616..=10000).step_by(16) {
            check(&mut login, now, "enter");
        }
        assert!(login.picker.policy.alpha >= 1.);
        press(&mut login, egui::Key::Escape);
        for now in (10016..=10400).step_by(16) {
            check(&mut login, now, "leave");
        }
        assert!(!login.picker.policy.visible());
        check(&mut login, 10416, "closed");
    }
}
