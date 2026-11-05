//! Nickname editing as the desktop host delivers it: each authored
//! egui-winit event is routed alone and committed through the host's
//! edit-effect path before any frame runs. No window or clipboard.
use slither_client::menu_layout::MenuElement as E;
use slither_wgpu_slice::{login_native::NativeLogin, menu_input_native::EditHost};

struct NoWindow;
impl EditHost for NoWindow {
    fn ime_allowed(&mut self, _: bool) {}
    fn ime_purpose_normal(&mut self) {}
    fn ime_cursor_area(&mut self, _: [f64; 2], _: [f64; 2]) {}
    fn read_clipboard(&mut self) -> Result<String, String> {
        Err("no clipboard".into())
    }
    fn write_clipboard(&mut self, _: &str) -> Result<(), String> {
        Err("no clipboard".into())
    }
}

fn route(login: &mut NativeLogin, event: egui::Event, admission: bool) {
    login.set_admission(admission);
    login.settings_input(egui::RawInput {
        events: vec![event],
        ..Default::default()
    });
    let effects = login.take_egui_effects();
    login.commit_edit_effects(effects, &mut NoWindow, admission);
}

fn key(login: &mut NativeLogin, key: egui::Key, modifiers: egui::Modifiers, admission: bool) {
    for pressed in [true, false] {
        route(
            login,
            egui::Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            },
            admission,
        );
    }
}

/// egui-winit sends a printable press as Key then Text, then the release.
fn type_text(login: &mut NativeLogin, text: &str, admission: bool) {
    for ch in text.chars() {
        let k = egui::Key::from_name(&ch.to_uppercase().to_string()).unwrap();
        let event = |pressed| egui::Event::Key {
            key: k,
            physical_key: Some(k),
            pressed,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        };
        route(login, event(true), admission);
        route(login, egui::Event::Text(ch.into()), admission);
        route(login, event(false), admission);
    }
}

fn assert_nickname(login: &mut NativeLogin, expected: &str, extent: [u32; 2], admission: bool) {
    assert_eq!(login.state.view(admission).nickname, expected, "stored");
    login.prepare_host(extent, 1., admission, None).unwrap();
    assert_eq!(login.displayed_nickname(), expected, "field value");
    assert!(
        login
            .egui
            .prepared_shapes()
            .iter()
            .any(|s| matches!(&s.shape, egui::Shape::Text(t) if t.galley.job.text == expected)),
        "painted {expected:?}"
    );
}

/// Click the field, then type, delete and commit an IME composition the way
/// the desktop delivers them; every edit must be kept, shown and painted.
pub fn edit_nickname(login: &mut NativeLogin, extent: [u32; 2], admission: bool) {
    login.prepare_host(extent, 1., admission, None).unwrap();
    let r = login.layout.viewport_rect(E::NicknameHolder);
    let point = [r.x + r.width / 2., r.y + r.height / 2.];
    for down in [true, false] {
        let effects = login.pointer(point, Some(down), admission);
        login.commit_edit_effects(effects, &mut NoWindow, admission);
    }
    assert_eq!(
        login.focused(),
        Some(slither_client::login_controls::NICKNAME)
    );
    key(
        login,
        egui::Key::A,
        egui::Modifiers {
            ctrl: true,
            command: true,
            ..Default::default()
        },
        admission,
    );
    type_text(login, "Ada", admission);
    assert_nickname(login, "Ada", extent, admission);
    key(
        login,
        egui::Key::Backspace,
        egui::Modifiers::NONE,
        admission,
    );
    assert_nickname(login, "Ad", extent, admission);
    route(
        login,
        egui::Event::Ime(egui::ImeEvent::Preedit {
            text: "m".into(),
            active_range_chars: Some(1..1),
        }),
        admission,
    );
    route(
        login,
        egui::Event::Ime(egui::ImeEvent::Commit("m".into())),
        admission,
    );
    assert_nickname(login, "Adm", extent, admission);
}
