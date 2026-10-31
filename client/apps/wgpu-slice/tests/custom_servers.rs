//! Authored synthetic addresses, in-memory inputs and isolated files. No game/network effects.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
use slither_client::{menu_controls::*, menu_input::*, server_picker::*};
use slither_wgpu_slice::{
    endpoint_login::{self, Connection},
    menu_input_native::KeyInput,
    server_picker_native::*,
    text::TextCache,
};
use std::{
    cell::Cell,
    sync::atomic::{AtomicUsize, Ordering},
};
use winit::{
    event::ElementState,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
};

fn key(p: &mut NativeServerPicker, logical: Key, code: KeyCode, state: ElementState) {
    let result = p.key(KeyInput {
        logical: &logical,
        physical: PhysicalKey::Code(code),
        text: None,
        state,
        repeat: false,
    });
    p.commit_effects(result.effects);
}
fn click(p: &mut NativeServerPicker, text: &mut TextCache, id: ControlId) {
    p.sync();
    let controls = p.panel.resolve(p.viewport).unwrap();
    let r = controls.iter().find(|r| r.control.id == id).unwrap();
    let point = [r.rect.x + r.rect.width / 2., r.rect.y + r.rect.height / 2.];
    for down in [true, false] {
        let effects = p.pointer(text, point, Some(down)).unwrap();
        p.commit_effects(effects);
    }
}
fn address(p: &mut NativeServerPicker, value: &str) {
    p.input.state.reset_editor(ADDRESS_EDITOR, "");
    p.controls.focus_control(&p.panel, ADDRESS);
    p.controls.sync_focus(&p.panel, &mut p.input.state);
    let effects = p.input.state.apply(EditIntent::KeyText(value.into()));
    p.commit_effects(effects);
}
#[test]
fn add_validate_select_and_remove_custom_servers_without_acquisition() {
    let mut p = NativeServerPicker::default();
    let mut text = TextCache::menu();
    p.open();
    p.policy.alpha = 1.;
    p.sync();
    assert!(!p.policy.take_directory_request());
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
        address(&mut p, invalid);
        click(&mut p, &mut text, ADD_CUSTOM);
        assert!(p.policy.custom.is_empty());
        assert!(!p.policy.custom_error.is_empty());
        assert!(p.policy.is_open());
        assert!(!p.policy.take_directory_request());
    }
    for valid in [
        "192.0.2.1:445",
        "192.0.2.1:80",
        "[2001:db8::1]:446",
        "bücher.example:443",
    ] {
        address(&mut p, valid);
        click(&mut p, &mut text, ADD_CUSTOM);
        assert!(
            p.policy
                .custom
                .contains(&normalize_custom_server(valid).unwrap())
        );
        assert!(p.policy.custom_error.is_empty());
    }
    let old = p.policy.custom.clone();
    address(&mut p, "192.0.2.1:80");
    click(&mut p, &mut text, ADD_CUSTOM);
    assert_eq!(p.policy.custom, old);
    p.policy.scroll(isize::MIN);
    click(&mut p, &mut text, ControlId(16));
    let fetches = Cell::new(0);
    let work = endpoint_login::prepare_selection::<()>(&p.policy.selection, |_| {
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
    assert!(!p.policy.official_context());
    p.open();
    assert!(!p.policy.take_directory_request());
    let remove = p.panel.controls.iter().find(|c| c.id.0 >= 8192).unwrap().id;
    click(&mut p, &mut text, remove);
    assert_eq!(p.policy.custom.len(), 3);
    assert!(p.policy.is_open());
    assert_eq!(p.policy.selection, ServerPreference::Automatic);
    assert!(!p.policy.take_directory_request());
}
#[test]
fn address_field_keyboard_mouse_and_focus_visible_share_normal_editing() {
    let mut p = NativeServerPicker::default();
    let mut text = TextCache::menu();
    p.open();
    p.policy.alpha = 1.;
    p.sync();
    assert_eq!(p.input.state.focus(), Some(ADDRESS_EDITOR));
    address(&mut p, "192.0.2.8:445");
    key(
        &mut p,
        Key::Named(NamedKey::Home),
        KeyCode::Home,
        ElementState::Pressed,
    );
    let effects = p.input.state.apply(EditIntent::KeyText("x".into()));
    p.commit_effects(effects);
    assert!(p.input.state.editor(ADDRESS_EDITOR).text().starts_with('x'));
    address(&mut p, "192.0.2.8:445");
    click(&mut p, &mut text, ADDRESS);
    assert_eq!(p.controls.focus(), Some(ADDRESS));
    assert!(p.controls.visible_focus().is_none());
    assert_eq!(p.input.state.focus(), Some(ADDRESS_EDITOR));
    key(
        &mut p,
        Key::Named(NamedKey::Tab),
        KeyCode::Tab,
        ElementState::Pressed,
    );
    assert_eq!(p.controls.focus(), Some(ADD_CUSTOM));
    assert_eq!(p.controls.visible_focus(), Some(ADD_CUSTOM));
    click(&mut p, &mut text, ADDRESS);
    key(
        &mut p,
        Key::Named(NamedKey::Enter),
        KeyCode::Enter,
        ElementState::Pressed,
    );
    key(
        &mut p,
        Key::Named(NamedKey::Enter),
        KeyCode::Enter,
        ElementState::Released,
    );
    assert_eq!(p.policy.custom.len(), 1);
    assert!(p.policy.is_open());
    assert!(p.input.state.editor(ADDRESS_EDITOR).text().is_empty());
    assert!(!p.policy.take_directory_request());
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
fn dense_custom_and_mixed_windows_fit_the_existing_native_menu_limits() {
    use slither_client::server_directory::Server;
    use slither_wgpu_slice::login_native::NativeLogin;
    let mut login = NativeLogin::new("Player").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(now as f64, true, false).unwrap();
    }
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
    login.picker.open();
    login.picker.policy.alpha = 1.;
    login
        .picker
        .input
        .state
        .reset_editor(ADDRESS_EDITOR, &"a".repeat(512));
    for dpr in [1., 1.5, 2., 5.] {
        for logical in [[360., 640.], [960., 720.], [960., 540.]] {
            for offset in [0, 27, 32, 2032] {
                login.picker.policy.scroll(isize::MIN);
                login.picker.policy.scroll(offset);
                let picture = login
                    .prepare_host(logical.map(|v| (v * dpr) as u32), dpr, true, None)
                    .unwrap();
                assert!(picture.frame.draws.len() <= 512);
                assert!(
                    picture.resources.len() <= 64,
                    "{dpr}, {logical:?}, {offset}: {} resources ({} distinct), {} draws, {} bytes",
                    picture.resources.len(),
                    picture
                        .resources
                        .iter()
                        .map(|r| slither_wgpu_slice::frame::ResourceKey::from(r.asset))
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                    picture.frame.draws.len(),
                    picture
                        .resources
                        .iter()
                        .map(|r| r.image.pixels.len())
                        .sum::<usize>()
                );
                assert!(
                    picture
                        .resources
                        .iter()
                        .map(|r| r.image.pixels.len())
                        .sum::<usize>()
                        <= 13 * 1024 * 1024
                );
            }
        }
    }
}
