//! Authored synthetic addresses, in-memory inputs and isolated files. No game/network effects.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{menu_controls::*, server_picker::*};
use slither_wgpu_slice::{
    endpoint_login::{self, Connection},
    login_native::NativeLogin,
    menu_input_native::KeyInput,
    server_picker_native::*,
};
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
use winit::{
    event::ElementState,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
};

/// A settled login with the picker opened through its shared action.
fn opened() -> NativeLogin {
    let mut login = NativeLogin::new("Player").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    login.prepare([960, 720], true, None).unwrap();
    login.open_picker();
    login.picker.policy.alpha = 1.;
    login.prepare([960, 720], true, None).unwrap();
    login
}

#[test]
fn automatic_returns_from_custom_without_a_directory_loading_action() {
    let mut login = opened();
    login.picker.policy.start_directory();
    assert!(login.picker.policy.take_directory_request());
    login
        .picker
        .policy
        .add_custom("192.0.2.9:445".into())
        .unwrap();
    click(&mut login, ControlId(16));
    assert!(!login.picker.policy.official_context());
    assert_eq!(
        login.picker.policy.selection.custom_address(),
        Some("192.0.2.9:445")
    );
    login
        .picker
        .policy
        .directory_failed("Injected cancellation");
    login.picker.policy.alpha = 0.;
    login.step(9616., true, false).unwrap();
    login.open_picker();
    login.picker.policy.alpha = 1.;
    login.prepare([960, 720], true, None).unwrap();
    assert!(
        !login
            .picker
            .panel
            .controls
            .iter()
            .any(|control| control.id == ControlId(11))
    );
    click(&mut login, AUTOMATIC);
    assert!(login.picker.policy.official_context());
    assert!(login.picker.policy.can_play());
    assert_eq!(login.picker.policy.selection, ServerPreference::Automatic);
    assert!(login.picker.policy.take_directory_request());
    assert!(!login.picker.policy.take_directory_request());
    assert!(!login.picker.policy.is_open());
    assert!(login.step(9632., true, false).unwrap().0.is_empty());
}
fn input(login: &mut NativeLogin, event: egui::Event) {
    login.settings_input(egui::RawInput {
        events: vec![event],
        ..Default::default()
    });
}
fn key(login: &mut NativeLogin, key: egui::Key, pressed: bool, modifiers: egui::Modifiers) {
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
fn press(login: &mut NativeLogin, k: egui::Key) {
    key(login, k, true, Default::default());
    key(login, k, false, Default::default());
}
fn click(login: &mut NativeLogin, id: ControlId) {
    login.prepare([960, 720], true, None).unwrap();
    let r = login
        .picker
        .ui
        .controls
        .iter()
        .find(|c| c.0 == id)
        .unwrap_or_else(|| panic!("missing picker control {id:?}"))
        .1;
    let p = [f64::from(r.center().x), f64::from(r.center().y)];
    login.pointer(p, Some(true), true);
    login.pointer(p, Some(false), true);
}
/// Replace the address through the field's ordinary select-all editing path.
fn address(login: &mut NativeLogin, value: &str) {
    click(login, ADDRESS);
    let command = egui::Modifiers {
        ctrl: true,
        command: true,
        ..Default::default()
    };
    key(login, egui::Key::A, true, command);
    key(login, egui::Key::A, false, command);
    press(login, egui::Key::Delete);
    assert_eq!(login.picker.ui.address, "");
    if !value.is_empty() {
        input(login, egui::Event::Text(value.into()));
    }
}

#[test]
fn add_validate_select_and_remove_custom_servers_without_acquisition() {
    let mut login = opened();
    assert!(!login.picker.policy.take_directory_request());
    for invalid in [
        "",
        "host",
        "host:0",
        "host:65536",
        "user@host:80",
        "ws://host:80/slither",
        "host:80/path",
        "bad host:445",
    ] {
        address(&mut login, invalid);
        click(&mut login, ADD_CUSTOM);
        assert!(login.picker.policy.custom.is_empty());
        assert!(!login.picker.policy.custom_error.is_empty());
        assert!(login.picker.policy.is_open());
        assert!(!login.picker.policy.take_directory_request());
        // A rejected address stays editable.
        assert_eq!(login.picker.ui.address, invalid);
    }
    for valid in [
        "192.0.2.1:445",
        "192.0.2.1:80",
        "[2001:db8::1]:446",
        "bücher.example:443",
    ] {
        address(&mut login, valid);
        click(&mut login, ADD_CUSTOM);
        assert!(
            login
                .picker
                .policy
                .custom
                .contains(&normalize_custom_server(valid).unwrap())
        );
        assert!(login.picker.policy.custom_error.is_empty());
        assert_eq!(login.picker.ui.address, "");
    }
    let old = login.picker.policy.custom.clone();
    address(&mut login, "192.0.2.1:80");
    click(&mut login, ADD_CUSTOM);
    assert_eq!(login.picker.policy.custom, old);
    login.picker.policy.scroll(isize::MIN);
    click(&mut login, ControlId(16));
    let fetches = Cell::new(0);
    let work = endpoint_login::prepare_selection::<()>(&login.picker.policy.selection, |_| {
        fetches.set(fetches.get() + 1);
        Err("unexpected official fetch/probe".into())
    })
    .unwrap();
    let Connection::Direct(endpoint) = work else {
        panic!("custom must connect directly")
    };
    let url = slither_wgpu_slice::game::server_endpoint(&endpoint).unwrap();
    assert_eq!(url.host_str(), Some("192.0.2.1"));
    assert_eq!(url.port(), Some(445));
    assert_eq!(fetches.get(), 0);
    assert!(!login.picker.policy.official_context());
    login.picker.policy.alpha = 0.;
    login.step(9616., true, false).unwrap();
    login.open_picker();
    login.picker.policy.alpha = 1.;
    assert!(!login.picker.policy.take_directory_request());
    let remove = login
        .picker
        .panel
        .controls
        .iter()
        .find(|c| c.id.0 >= 8192)
        .unwrap()
        .id;
    click(&mut login, remove);
    assert_eq!(login.picker.policy.custom.len(), 3);
    assert!(login.picker.policy.is_open());
    assert_eq!(login.picker.policy.selection, ServerPreference::Automatic);
    assert!(!login.picker.policy.take_directory_request());
    // Removal changes the generation: focus returns to the initial field.
    login.prepare([960, 720], true, None).unwrap();
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
}

#[test]
fn pasted_address_trims_outer_whitespace_but_rejects_it_inside() {
    let mut login = opened();
    for value in [
        "192.0.2.9:445\n",
        " \r\n192.0.2.9:445\t",
        "\u{2003}192.0.2.9:445\u{2003}",
        "192.0.2.\n9:445",
        "192.0.2.9 :445",
    ] {
        address(&mut login, "");
        input(&mut login, egui::Event::Paste(value.into()));
        assert!(!login.picker.ui.address.chars().any(char::is_control));
        click(&mut login, ADD_CUSTOM);
        assert_eq!(login.picker.policy.custom, ["192.0.2.9:445"]);
        assert_eq!(
            login.picker.policy.custom_error.is_empty(),
            !value.trim().chars().any(char::is_whitespace)
        );
        assert!(!login.picker.policy.take_directory_request());
    }
}

#[test]
fn address_field_is_bounded_in_utf16_units_with_its_caret_in_chars() {
    let mut login = opened();
    address(&mut login, &"\u{1F600}".repeat(300));
    // 512 UTF-16 units: each surrogate pair consumes two.
    assert_eq!(login.picker.ui.address.encode_utf16().count(), 512);
    assert_eq!(login.picker.ui.address.chars().count(), 256);
    input(&mut login, egui::Event::Text("x".into()));
    assert_eq!(login.picker.ui.address.chars().count(), 256);
    // The caret was clamped in chars; Home and an insert still edit in place.
    press(&mut login, egui::Key::Home);
    input(&mut login, egui::Event::Text("ab".into()));
    assert!(login.picker.ui.address.starts_with("ab"));
    assert_eq!(login.picker.ui.address.encode_utf16().count(), 512);
}

#[test]
fn address_field_keyboard_mouse_and_visible_focus_share_normal_editing() {
    let mut login = opened();
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
    address(&mut login, "192.0.2.8:445");
    press(&mut login, egui::Key::Home);
    input(&mut login, egui::Event::Text("x".into()));
    assert!(login.picker.ui.address.starts_with('x'));
    address(&mut login, "192.0.2.8:445");
    click(&mut login, ADDRESS);
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
    assert!(!login.picker.ui.keyboard_focus);
    press(&mut login, egui::Key::Tab);
    assert_eq!(login.picker.ui.focused(), Some(ADD_CUSTOM));
    assert!(login.picker.ui.keyboard_focus);
    click(&mut login, ADDRESS);
    press(&mut login, egui::Key::Enter);
    assert_eq!(login.picker.policy.custom.len(), 1);
    assert!(login.picker.policy.is_open());
    assert_eq!(login.picker.ui.address, "");
    // Enter adds without surrendering the field.
    assert_eq!(login.picker.ui.focused(), Some(ADDRESS));
    assert!(!login.picker.policy.take_directory_request());
    // A held Enter's repeats cannot add the next typed entry.
    address(&mut login, "192.0.2.9:445");
    key(&mut login, egui::Key::Enter, true, Default::default());
    assert_eq!(login.picker.policy.custom.len(), 2);
    address(&mut login, "192.0.2.10:445");
    for _ in 0..3 {
        input(
            &mut login,
            egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: Some(egui::Key::Enter),
                pressed: true,
                repeat: true,
                modifiers: Default::default(),
            },
        );
    }
    key(&mut login, egui::Key::Enter, false, Default::default());
    assert_eq!(login.picker.policy.custom.len(), 2);
    assert_eq!(login.picker.ui.address, "192.0.2.10:445");
}
#[test]
fn official_choices_keep_the_existing_acquisition_arguments() {
    for choice in [
        ServerPreference::Automatic,
        ServerPreference::Official(0),
        ServerPreference::Official(65535),
    ] {
        let calls = Cell::new(0);
        let work = endpoint_login::prepare_selection(&choice, |id| {
            calls.set(calls.get() + 1);
            Ok(id)
        })
        .unwrap();
        assert!(matches!(work,Connection::Official(id) if id==choice.official_id()));
        assert_eq!(calls.get(), 1);
    }
}
#[test]
fn custom_storage_is_bounded_atomic_and_ignores_invalid_records() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "custom-servers-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("custom-servers.txt");
    let expected = (1..=MAX_CUSTOM_SERVERS)
        .map(|n| format!("192.0.2.{n}:80"))
        .collect::<Vec<_>>();
    write_custom(&path, &expected).unwrap();
    let mut current = Vec::new();
    assert!(read_custom(&path, &mut current));
    assert_eq!(current, expected);
    let invalid = [
        b"2\n192.0.2.1:80\n".to_vec(),
        b"1\ninvalid\n".to_vec(),
        b"1\n192.0.2.1:80\n192.0.2.1:80\n".to_vec(),
        vec![255],
        vec![b'x'; MAX_CUSTOM_SERVERS * (MAX_CUSTOM_ADDRESS + 1) + 3],
        format!(
            "1\n{}",
            (1..=33)
                .map(|n| format!("192.0.2.{n}:80\n"))
                .collect::<String>()
        )
        .into_bytes(),
    ];
    for bytes in invalid {
        std::fs::write(&path, bytes).unwrap();
        assert!(!read_custom(&path, &mut current));
        assert_eq!(current, expected);
    }
    let choice = root.join("server-choice.txt");
    for record in ["1:auto", "1:0", "1:65535", "2:custom:[2001:db8::1]:80"] {
        write_preference(&choice, record).unwrap();
        assert_eq!(
            read_preference(&choice),
            ServerPreference::restore(Some(record))
        );
    }
    for record in [
        "2:custom:invalid",
        "2:custom:ws://host:80/slither",
        "9:future",
    ] {
        std::fs::write(&choice, record).unwrap();
        assert_eq!(read_preference(&choice), ServerPreference::Automatic);
    }
    let blocked = root.join("blocked");
    std::fs::create_dir(&blocked).unwrap();
    assert!(write_custom(&blocked, &current).is_err());
    assert_eq!(current, expected);
    assert!(blocked.is_dir());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(choice).unwrap();
    std::fs::remove_dir(blocked).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn separate_storage_handles_merge_operations_from_stale_instance_lists() {
    let root = std::env::temp_dir().join(format!("custom-operations-{}", std::process::id()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("custom-servers.txt");
    let first = CustomServerStorage::new(path.clone());
    let second = CustomServerStorage::new(path.clone());
    let mut a = Vec::new();
    let mut b = Vec::new();
    let add = |n| CustomServerOperation::Add(format!("192.0.2.{n}:80"));
    first.apply(&add(1), &mut a).unwrap();
    second.apply(&add(2), &mut b).unwrap();
    assert_eq!(b, ["192.0.2.1:80", "192.0.2.2:80"]);
    first.apply(&add(1), &mut a).unwrap();
    assert_eq!(a, b); // A duplicate Add refreshes the stale list too.
    second
        .apply(&CustomServerOperation::Remove(a[0].clone()), &mut b)
        .unwrap();
    first.apply(&add(3), &mut a).unwrap();
    assert_eq!(a, ["192.0.2.2:80", "192.0.2.3:80"]); // Do not resurrect a removed entry.
    for n in 4..=33 {
        first.apply(&add(n), &mut a).unwrap();
    }
    assert!(second.apply(&add(34), &mut b).is_err());
    assert_eq!(b, a);
    assert_eq!(b.len(), MAX_CUSTOM_SERVERS);
    let mut disk = Vec::new();
    assert!(read_custom(&path, &mut disk));
    assert_eq!(disk, a);

    // Malformed storage falls back to this instance's list and still applies the edit.
    std::fs::write(&path, "1\ninvalid\n").unwrap();
    second
        .apply(&CustomServerOperation::Remove(b[0].clone()), &mut b)
        .unwrap();
    assert_eq!(b.len(), MAX_CUSTOM_SERVERS - 1);
    assert!(read_custom(&path, &mut disk));
    assert_eq!(disk, b);
    std::fs::remove_file(&path).unwrap();
    first.apply(&add(1), &mut a).unwrap_err(); // Missing file uses the full in-memory fallback.
    a.truncate(1);
    std::fs::create_dir(&path).unwrap();
    assert!(first.apply(&add(34), &mut a).is_err());
    assert_eq!(a, ["192.0.2.2:80", "192.0.2.34:80"]);
    std::fs::remove_dir(path).unwrap();
    std::fs::remove_file(root.join("custom-servers.lock")).unwrap();
    std::fs::remove_dir(root).unwrap();
}
#[test]
fn cosmetics_arrow_focus_uses_shared_modality() {
    use slither_client::skin_studio::{COSMETIC_BASE, COSMETICS_TAB};
    use slither_wgpu_slice::login_native::NativeLogin;
    let mut login = NativeLogin::new("Player").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
    assert!(login.studio.open(true));
    login.step(9830., true, false).unwrap();
    login.studio.activate(COSMETICS_TAB, "Player");
    login.step(10020., true, false).unwrap();
    login.prepare_host([960, 720], 1., true, None).unwrap();
    let resolved = login.studio_panel.resolve([960., 720.]).unwrap();
    let cell = resolved
        .iter()
        .find(|r| r.control.id == ControlId(COSMETIC_BASE + 1))
        .unwrap();
    let point = [
        cell.rect.x + cell.rect.width / 2.,
        cell.rect.y + cell.rect.height / 2.,
    ];
    for down in [true, false] {
        login.pointer(point, Some(down), true);
    }
    assert_eq!(login.studio.state.preview_cosmetic(), Some(0));
    assert!(!login.studio_ui.keyboard_focus);
    let key = |login: &mut NativeLogin, named| {
        let logical = Key::Named(named);
        login.key(
            KeyInput {
                logical: &logical,
                physical: PhysicalKey::Code(KeyCode::ArrowRight),
                text: None,
                state: ElementState::Pressed,
                repeat: false,
            },
            true,
        );
    };
    key(&mut login, NamedKey::ArrowRight);
    assert_eq!(login.focused(), Some(ControlId(COSMETIC_BASE + 2)));
    assert!(login.studio_ui.keyboard_focus);
    let shapes = |login: &mut NativeLogin| {
        login.prepare_host([960, 720], 1., true, None).unwrap();
        login
            .egui
            .prepared_shapes()
            .iter()
            .map(|s| s.shape.visual_bounding_rect())
            .collect::<Vec<_>>()
    };
    let before = shapes(&mut login);
    // Pressing the focused cell and releasing outside it changes only the
    // input modality: the selected cosmetic and the focused cell stay put.
    let target = login
        .studio_ui
        .controls
        .iter()
        .find(|c| c.0 == ControlId(COSMETIC_BASE + 2))
        .unwrap()
        .1
        .center();
    let surface = slither_client::skin_studio::StudioState::bounds([960., 720.]);
    login.pointer([target.x.into(), target.y.into()], Some(true), true);
    login.pointer([surface.x + 8., surface.y + 8.], Some(false), true);
    assert!(!login.studio_ui.keyboard_focus);
    assert_eq!(login.studio.state.preview_cosmetic(), Some(0));
    assert_eq!(login.focused(), Some(ControlId(COSMETIC_BASE + 2)));
    let mouse = shapes(&mut login);
    assert!(before.len() > mouse.len(), "the focus ring hides");
    // Arrow keys show the ring again; Left then Right return to the cell.
    key(&mut login, NamedKey::ArrowLeft);
    key(&mut login, NamedKey::ArrowRight);
    assert_eq!(login.focused(), Some(ControlId(COSMETIC_BASE + 2)));
    assert_eq!(shapes(&mut login), before);
}
#[test]
fn dense_custom_and_mixed_windows_stay_inside_the_window() {
    use slither_client::server_directory::Server;
    let mut login = opened();
    for n in 1..=32 {
        login
            .picker
            .policy
            .add_custom(format!("192.0.2.{n}:445"))
            .unwrap();
    }
    let servers = (1..=2000)
        .map(|id| Server {
            id,
            cluster: 1,
            address: ([192, 0, 2, 100], id).into(),
            population: 40,
            active: true,
        })
        .collect::<Vec<_>>();
    login
        .picker
        .policy
        .replace(&servers, &std::collections::BTreeMap::from([(1, 12)]))
        .unwrap();
    login.picker.set_address(&"a".repeat(512));
    // Glyphs accumulate per DPR, so the resize history matters: up through
    // every supported step, then back down.
    for dpr in [1., 1.5, 2., 3., 4., 5., 3., 1.5, 1.] {
        for logical in [[360., 640.], [960., 720.], [960., 540.]] {
            for offset in [0, 27, 32, 2032] {
                login.picker.policy.scroll(isize::MIN);
                login.picker.policy.scroll(offset);
                login
                    .prepare_host(logical.map(|v| (v * dpr) as u32), dpr, true, None)
                    .unwrap();
                let window = egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(logical[0] as f32, logical[1] as f32),
                );
                let ui = &login.picker.ui;
                assert_eq!(
                    ui.controls
                        .iter()
                        .filter(|c| (16..8192).contains(&c.0.0))
                        .count(),
                    6
                );
                for (id, r, _) in &ui.controls {
                    assert!(
                        window.expand(0.01).contains_rect(*r),
                        "{dpr}, {logical:?}, {offset}: {id:?} {r:?}"
                    );
                }
                assert_eq!(ui.address.len(), 512);
                // An atlas-only budget, not the retained toolkit's total raster
                // bound. Picker text lives in the shared egui font atlas: 2048
                // wide (the host leaves max_texture_side at egui's default),
                // growing in height and cleared above 80% fill. 16 MiB (2048²×4)
                // is the maximum measured over this history, not a structural
                // guarantee. The login wordmark textures belong to #418 and are
                // excluded. Counts keep the old 64-resource/512-draw ceilings.
                let textures = login.egui.context().unwrap().tex_manager();
                let textures = textures.read();
                let atlas: usize = textures
                    .allocated()
                    .filter(|(_, m)| m.name == "egui_font_texture")
                    .map(|(_, m)| m.bytes_used())
                    .sum();
                let shapes = login.egui.prepared_shapes().len();
                assert!(
                    textures.num_allocated() <= 64 && atlas <= 16 * 1024 * 1024 && shapes <= 512,
                    "{dpr}, {logical:?}, {offset}: {} textures, {atlas} atlas bytes, {shapes} shapes",
                    textures.num_allocated()
                );
            }
        }
    }
}
