//! Authored host-event traces. No window, clipboard, GPU or IME is opened.
#![cfg(not(target_arch = "wasm32"))]
use slither_client::{
    menu_input::*,
    menu_preferences::MenuQuality,
    start_page::{MenuLocale, StartMenuIntent, StartMenuState},
};
use slither_wgpu_slice::menu_input_native::*;
use winit::{
    event::{ElementState, Ime, WindowEvent},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
};

#[derive(Default)]
struct Host {
    text: String,
    fail: bool,
    calls: Vec<String>,
}
impl EditHost for Host {
    fn ime_allowed(&mut self, allowed: bool) {
        self.calls.push(format!("allowed {allowed}"));
    }
    fn ime_purpose_normal(&mut self) {
        self.calls.push("normal".into());
    }
    fn ime_cursor_area(&mut self, position: [f64; 2], size: [f64; 2]) {
        self.calls.push(format!("area {position:?} {size:?}"));
    }
    fn read_clipboard(&mut self) -> Result<String, String> {
        self.calls.push("read".into());
        if self.fail {
            Err("injected busy".into())
        } else {
            Ok(self.text.clone())
        }
    }
    fn write_clipboard(&mut self, text: &str) -> Result<(), String> {
        self.calls.push("write".into());
        if self.fail {
            Err("injected busy".into())
        } else {
            self.text = text.into();
            Ok(())
        }
    }
}
fn focused() -> NativeMenuInput {
    let mut native = NativeMenuInput::default();
    native
        .state
        .apply(EditIntent::Focus(Some(TextField::Nickname)));
    native
}
fn key(
    native: &mut NativeMenuInput,
    logical: Key,
    code: KeyCode,
    text: Option<&str>,
    pressed: bool,
    repeat: bool,
) -> NativeInputResult {
    native.key(KeyInput {
        logical: &logical,
        physical: PhysicalKey::Code(code),
        text,
        state: if pressed {
            ElementState::Pressed
        } else {
            ElementState::Released
        },
        repeat,
    })
}
fn character(
    native: &mut NativeMenuInput,
    c: &str,
    code: KeyCode,
    text: Option<&str>,
) -> NativeInputResult {
    key(native, Key::Character(c.into()), code, text, true, false)
}
fn named(
    native: &mut NativeMenuInput,
    name: NamedKey,
    code: KeyCode,
    pressed: bool,
    repeat: bool,
) -> NativeInputResult {
    key(native, Key::Named(name), code, Some("\r"), pressed, repeat)
}

#[test]
fn synthetic_key_fields_use_logical_editing_physical_gameplay_and_altgr_text() {
    let mut native = focused();
    assert!(character(&mut native, "a", KeyCode::KeyQ, Some("a")).consumed);
    character(&mut native, "é", KeyCode::Quote, Some("é"));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "a ");
    native.modifiers(ModifiersState::CONTROL | ModifiersState::ALT);
    character(&mut native, "@", KeyCode::KeyQ, Some("@"));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "a @");
    native.modifiers(ModifiersState::SHIFT);
    named(
        &mut native,
        NamedKey::ArrowLeft,
        KeyCode::ArrowLeft,
        true,
        false,
    );
    assert_eq!(native.state.editor(TextField::Nickname).selection(), (3, 2));
    named(
        &mut native,
        NamedKey::Backspace,
        KeyCode::Backspace,
        true,
        false,
    );
    assert_eq!(native.state.editor(TextField::Nickname).text(), "a ");
    native.window_event(&WindowEvent::ModifiersChanged(
        ModifiersState::CONTROL.into(),
    ));
    character(&mut native, "z", KeyCode::KeyZ, Some("z"));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "a @");
    native.modifiers(ModifiersState::CONTROL | ModifiersState::SHIFT);
    character(&mut native, "Z", KeyCode::KeyZ, Some("Z"));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "a ");
    let up = named(
        &mut native,
        NamedKey::ArrowLeft,
        KeyCode::ArrowLeft,
        false,
        false,
    );
    assert_eq!(
        up.effects,
        vec![HostEditEffect::Gameplay {
            key: GameplayKey::Left,
            pressed: false
        }]
    );
    native.state.apply(EditIntent::Focus(None));
    let down = named(
        &mut native,
        NamedKey::ArrowRight,
        KeyCode::ArrowRight,
        true,
        false,
    );
    assert_eq!(
        down.effects,
        vec![HostEditEffect::Gameplay {
            key: GameplayKey::Right,
            pressed: true
        }]
    );
    assert!(down.consumed);
    assert!(!named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).consumed);
}

#[test]
fn ime_events_keep_raw_bytes_cancel_and_deduplicate_enter_in_both_orders() {
    for enter_before_commit in [true, false] {
        let mut native = focused();
        native.window_event(&WindowEvent::Ime(Ime::Enabled));
        native.window_event(&WindowEvent::Ime(Ime::Preedit("😀é".into(), Some((4, 6)))));
        assert_eq!(
            native
                .state
                .editor(TextField::Nickname)
                .preedit()
                .unwrap()
                .text,
            "😀é"
        );
        character(&mut native, "😀é", KeyCode::KeyA, Some("😀é"));
        if enter_before_commit {
            assert!(
                named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false)
                    .effects
                    .is_empty()
            );
        }
        native.window_event(&WindowEvent::Ime(Ime::Preedit("".into(), None)));
        native.window_event(&WindowEvent::Ime(Ime::Commit("😀é".into())));
        native.window_event(&WindowEvent::Ime(Ime::Disabled));
        character(&mut native, "😀é", KeyCode::KeyA, Some("😀é"));
        if !enter_before_commit {
            assert!(
                named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false)
                    .effects
                    .is_empty()
            );
        }
        assert_eq!(native.state.editor(TextField::Nickname).text(), "   ");
        native.state.apply(EditIntent::EndEventBatch);
        assert!(
            named(&mut native, NamedKey::Enter, KeyCode::Enter, true, true)
                .effects
                .is_empty()
        );
        named(&mut native, NamedKey::Enter, KeyCode::Enter, false, false);
        assert_eq!(
            named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
            vec![HostEditEffect::Activate(TextField::Nickname)]
        );
        native.window_event(&WindowEvent::Ime(Ime::Enabled));
        native.window_event(&WindowEvent::Ime(Ime::Preedit("discard".into(), None)));
        named(&mut native, NamedKey::Escape, KeyCode::Escape, true, false);
        assert!(!native.state.editor(TextField::Nickname).composing());
        native.window_event(&WindowEvent::Ime(Ime::Commit("late".into())));
        assert_eq!(native.state.editor(TextField::Nickname).text(), "   ");
    }
}

#[test]
fn injected_clipboard_shortcuts_cover_success_failure_and_non_destructive_cut() {
    let mut native = focused();
    let mut host = Host {
        text: "é😀 Ada".into(),
        ..Host::default()
    };
    native.modifiers(ModifiersState::CONTROL);
    let effects = character(&mut native, "v", KeyCode::KeyV, Some("v")).effects;
    assert!(native.execute(effects, &mut host).is_empty());
    assert_eq!(native.state.editor(TextField::Nickname).text(), "    Ada");
    character(&mut native, "a", KeyCode::KeyA, Some("a"));
    let effects = character(&mut native, "c", KeyCode::KeyC, Some("c")).effects;
    native.execute(effects, &mut host);
    assert_eq!(host.text, "    Ada");
    assert_eq!(native.state.editor(TextField::Nickname).text(), "    Ada");
    host.fail = true;
    let effects = character(&mut native, "x", KeyCode::KeyX, Some("x")).effects;
    assert_eq!(
        native.execute(effects, &mut host),
        vec![HostEditEffect::ClipboardFailed("injected busy".into())]
    );
    assert_eq!(native.state.editor(TextField::Nickname).text(), "    Ada");
    let effects = character(&mut native, "v", KeyCode::KeyV, Some("v")).effects;
    assert_eq!(
        native.execute(effects, &mut host),
        vec![HostEditEffect::ClipboardFailed("injected busy".into())]
    );
    host.fail = false;
    let effects = character(&mut native, "x", KeyCode::KeyX, Some("x")).effects;
    native.execute(effects, &mut host);
    assert_eq!(native.state.editor(TextField::Nickname).text(), "");
    character(&mut native, "z", KeyCode::KeyZ, Some("z"));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "    Ada");
    character(&mut native, "y", KeyCode::KeyY, Some("y"));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "");
    assert_eq!(host.calls, ["read", "write", "write", "read", "write"]);
}

#[test]
fn injected_host_records_ime_admission_purpose_area_and_blur_releases() {
    let mut native = NativeMenuInput::default();
    let mut host = Host::default();
    let effects = native
        .state
        .apply(EditIntent::Focus(Some(TextField::Nickname)));
    let releases = native.execute(effects, &mut host);
    assert_eq!(releases.len(), 5);
    let effects = native.state.apply(EditIntent::CursorArea {
        position: [12.5, 28.],
        size: [1., 24.],
    });
    native.execute(effects, &mut host);
    native.modifiers(ModifiersState::CONTROL);
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    native.window_event(&WindowEvent::Ime(Ime::Preedit("discard".into(), None)));
    let result = native.window_event(&WindowEvent::Focused(false));
    let remaining = native.execute(result.effects, &mut host);
    assert_eq!(remaining.len(), 5);
    assert!(remaining.contains(&HostEditEffect::Gameplay {
        key: GameplayKey::Space,
        pressed: false
    }));
    assert!(native.state.editor(TextField::Nickname).preedit().is_none());
    let result = native.window_event(&WindowEvent::Focused(true));
    native.execute(result.effects, &mut host);
    character(&mut native, "a", KeyCode::KeyA, Some("a"));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "a");
    assert_eq!(
        host.calls,
        [
            "allowed true",
            "normal",
            "area [12.5, 28.0] [1.0, 24.0]",
            "allowed false",
            "allowed true",
            "normal"
        ]
    );
    assert!(
        native
            .state
            .apply(EditIntent::CursorArea {
                position: [f64::NAN, 0.],
                size: [1., 24.]
            })
            .is_empty()
    );
}

#[test]
fn modal_suppresses_gameplay_and_login_activation_but_always_routes_releases() {
    let mut native = focused();
    native.state.apply(EditIntent::Modal(true));
    assert!(
        named(
            &mut native,
            NamedKey::ArrowRight,
            KeyCode::ArrowRight,
            true,
            false
        )
        .effects
        .is_empty()
    );
    assert!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false)
            .effects
            .is_empty()
    );
    assert_eq!(
        named(
            &mut native,
            NamedKey::ArrowRight,
            KeyCode::ArrowRight,
            false,
            false
        )
        .effects,
        vec![HostEditEffect::Gameplay {
            key: GameplayKey::Right,
            pressed: false
        }]
    );
    native.state.apply(EditIntent::Modal(false));
    assert_eq!(native.state.focus(), Some(TextField::Nickname));
}

#[test]
fn native_text_clipboard_and_enter_feed_existing_deferred_menu_request() {
    let mut native = NativeMenuInput::default();
    let mut menu = StartMenuState::new(MenuLocale::English, false, MenuQuality::High);
    let intents = menu.step(0., false).unwrap();
    native.state.sync_menu(&menu.view(true), &intents);
    let mut host = Host::default();
    character(&mut native, "Ada", KeyCode::KeyA, Some("Ada"));
    let effects = named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects;
    let effects = native.execute(effects, &mut host);
    let effects = native.state.commit_menu(effects, &mut menu, true);
    assert_eq!(effects, vec![HostEditEffect::ImeAllowed(false)]);
    assert_eq!(
        menu.step(8., false).unwrap(),
        vec![StartMenuIntent::Play {
            nickname: "Ada".into(),
            quality: MenuQuality::High
        }]
    );
    assert!(menu.step(16., false).unwrap().is_empty());
}
