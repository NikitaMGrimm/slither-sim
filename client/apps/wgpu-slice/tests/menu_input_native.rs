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
    let text = (name == NamedKey::Enter && pressed).then_some("\r");
    key(native, Key::Named(name), code, text, pressed, repeat)
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
fn windows_preedit_process_enter_commit_and_release() {
    for release_as_process in [false, true] {
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
        assert!(
            key(
                &mut native,
                Key::Named(NamedKey::Process),
                KeyCode::Enter,
                None,
                true,
                false
            )
            .effects
            .is_empty()
        );
        native.window_event(&WindowEvent::Ime(Ime::Preedit("".into(), None)));
        native.window_event(&WindowEvent::Ime(Ime::Commit("😀é".into())));
        native.window_event(&WindowEvent::Ime(Ime::Disabled));
        assert_eq!(native.state.editor(TextField::Nickname).text(), "   ");
        // Windows provides no duplicate KeyEvent.text for the composed result.
        let released = if release_as_process {
            NamedKey::Process
        } else {
            NamedKey::Enter
        };
        assert!(
            key(
                &mut native,
                Key::Named(released),
                KeyCode::Enter,
                None,
                false,
                false
            )
            .effects
            .is_empty()
        );
        assert_eq!(
            named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
            vec![HostEditEffect::Activate(TextField::Nickname)]
        );
        named(&mut native, NamedKey::Enter, KeyCode::Enter, false, false);
        native.window_event(&WindowEvent::Ime(Ime::Enabled));
        native.window_event(&WindowEvent::Ime(Ime::Preedit("discard".into(), None)));
        named(&mut native, NamedKey::Escape, KeyCode::Escape, true, false);
        native.window_event(&WindowEvent::Ime(Ime::Disabled));
        assert!(!native.state.editor(TextField::Nickname).composing());
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

fn windows_enter_commit(native: &mut NativeMenuInput, text: &str) {
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    native.window_event(&WindowEvent::Ime(Ime::Preedit(
        text.into(),
        Some((text.len(), text.len())),
    )));
    let process = key(
        native,
        Key::Named(NamedKey::Process),
        KeyCode::Enter,
        None,
        true,
        false,
    );
    assert!(process.consumed && process.effects.is_empty());
    native.window_event(&WindowEvent::Ime(Ime::Preedit("".into(), None)));
    native.window_event(&WindowEvent::Ime(Ime::Commit(text.into())));
    native.window_event(&WindowEvent::Ime(Ime::Disabled));
}

#[test]
fn round3_windows_enter_release_then_fresh_a_inserts_both() {
    let mut native = focused();
    windows_enter_commit(&mut native, "a");
    named(&mut native, NamedKey::Enter, KeyCode::Enter, false, false);
    character(&mut native, "a", KeyCode::KeyA, Some("a"));
    key(
        &mut native,
        Key::Character("a".into()),
        KeyCode::KeyA,
        None,
        false,
        false,
    );
    assert_eq!(native.state.editor(TextField::Nickname).text(), "aa");
}

#[test]
fn round3_windows_a_release_then_direct_commit_inserts_both() {
    let mut native = focused();
    character(&mut native, "a", KeyCode::KeyA, Some("a"));
    key(
        &mut native,
        Key::Character("a".into()),
        KeyCode::KeyA,
        None,
        false,
        false,
    );
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    native.window_event(&WindowEvent::Ime(Ime::Preedit("".into(), None)));
    native.window_event(&WindowEvent::Ime(Ime::Commit("a".into())));
    native.window_event(&WindowEvent::Ime(Ime::Disabled));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "aa");
}

#[test]
fn round3_windows_earlier_aa_and_aba_traces_preserve_every_edit() {
    let mut native = focused();
    character(&mut native, "a", KeyCode::KeyA, Some("a"));
    key(
        &mut native,
        Key::Character("a".into()),
        KeyCode::KeyA,
        None,
        false,
        false,
    );
    windows_enter_commit(&mut native, "a");
    named(&mut native, NamedKey::Enter, KeyCode::Enter, false, false);
    assert_eq!(native.state.editor(TextField::Nickname).text(), "aa");
    let mut native = focused();
    windows_enter_commit(&mut native, "a");
    named(&mut native, NamedKey::Enter, KeyCode::Enter, false, false);
    for (c, code) in [("b", KeyCode::KeyB), ("a", KeyCode::KeyA)] {
        character(&mut native, c, code, Some(c));
        key(
            &mut native,
            Key::Character(c.into()),
            code,
            None,
            false,
            false,
        );
    }
    assert_eq!(native.state.editor(TextField::Nickname).text(), "aba");
}

#[test]
fn round3_process_keys_never_insert_or_activate_shortcuts() {
    let mut native = focused();
    for (c, code) in [
        ("k", KeyCode::KeyK),
        ("e", KeyCode::KeyE),
        ("e", KeyCode::KeyE),
        ("p", KeyCode::KeyP),
    ] {
        character(&mut native, c, code, Some(c));
        key(
            &mut native,
            Key::Character(c.into()),
            code,
            None,
            false,
            false,
        );
    }
    native.modifiers(ModifiersState::CONTROL);
    let process = key(
        &mut native,
        Key::Named(NamedKey::Process),
        KeyCode::KeyA,
        None,
        true,
        false,
    );
    assert!(process.consumed && process.effects.is_empty());
    assert_eq!(native.state.editor(TextField::Nickname).selection(), (4, 4));
    // Process can carry text on Windows; that payload is still IME-owned.
    native.modifiers(ModifiersState::CONTROL | ModifiersState::ALT);
    let process = key(
        &mut native,
        Key::Named(NamedKey::Process),
        KeyCode::KeyV,
        Some("a"),
        true,
        false,
    );
    assert!(process.consumed && process.effects.is_empty());
    assert_eq!(native.state.editor(TextField::Nickname).text(), "keep");
    native.state.apply(EditIntent::Focus(None));
    let process = key(
        &mut native,
        Key::Named(NamedKey::Process),
        KeyCode::Space,
        None,
        true,
        false,
    );
    assert!(process.consumed && process.effects.is_empty());
    let release = key(
        &mut native,
        Key::Named(NamedKey::Process),
        KeyCode::Space,
        None,
        false,
        false,
    );
    assert_eq!(
        release.effects,
        vec![HostEditEffect::Gameplay {
            key: GameplayKey::Space,
            pressed: false
        }]
    );
}

#[test]
fn windows_plain_typing_and_repeats_work_with_ime_enabled_without_preedit() {
    let mut native = focused();
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    character(&mut native, "a", KeyCode::KeyA, Some("a"));
    for _ in 0..2 {
        key(
            &mut native,
            Key::Character("a".into()),
            KeyCode::KeyA,
            Some("a"),
            true,
            true,
        );
    }
    key(
        &mut native,
        Key::Character("a".into()),
        KeyCode::KeyA,
        None,
        false,
        false,
    );
    character(&mut native, "b", KeyCode::KeyB, Some("b"));
    assert_eq!(native.state.editor(TextField::Nickname).text(), "aaab");
    assert!(native.state.editor(TextField::Nickname).preedit().is_none());
}

#[test]
fn windows_unrelated_keys_do_not_release_a_consumed_enter() {
    let mut native = focused();
    windows_enter_commit(&mut native, "a");
    // Neither an unrelated press nor a non-Enter release releases the physical
    // Enter owned by IME. This also guards against a blanket release workaround.
    key(
        &mut native,
        Key::Named(NamedKey::Shift),
        KeyCode::ShiftLeft,
        None,
        true,
        false,
    );
    key(
        &mut native,
        Key::Named(NamedKey::Space),
        KeyCode::Space,
        None,
        false,
        false,
    );
    assert!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false)
            .effects
            .is_empty()
    );
    named(&mut native, NamedKey::Enter, KeyCode::Enter, false, false);
    assert_eq!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
        vec![HostEditEffect::Activate(TextField::Nickname)]
    );
}

#[test]
fn consumed_physical_enter_repeat_release_and_blur() {
    for code in [KeyCode::Enter, KeyCode::NumpadEnter] {
        for logical in [NamedKey::Process, NamedKey::Enter] {
            let mut native = focused();
            native.window_event(&WindowEvent::Ime(Ime::Enabled));
            native.window_event(&WindowEvent::Ime(Ime::Preedit("a".into(), None)));
            let consumed = key(
                &mut native,
                Key::Named(logical),
                code,
                Some("\r"),
                true,
                false,
            );
            assert!(consumed.consumed && consumed.effects.is_empty());
            native.window_event(&WindowEvent::Ime(Ime::Preedit("".into(), None)));
            native.window_event(&WindowEvent::Ime(Ime::Commit("a".into())));
            native.window_event(&WindowEvent::Ime(Ime::Disabled));
            assert!(
                named(&mut native, NamedKey::Enter, code, true, true)
                    .effects
                    .is_empty()
            );
            assert!(
                named(&mut native, NamedKey::Enter, code, true, false)
                    .effects
                    .is_empty()
            );
            key(
                &mut native,
                Key::Named(NamedKey::Process),
                code,
                Some("\r"),
                false,
                false,
            );
            assert_eq!(
                named(&mut native, NamedKey::Enter, code, true, false).effects,
                vec![HostEditEffect::Activate(TextField::Nickname)]
            );
            assert!(
                named(&mut native, NamedKey::Enter, code, true, true)
                    .effects
                    .is_empty()
            );
        }
    }
    let mut native = focused();
    windows_enter_commit(&mut native, "a");
    native.window_event(&WindowEvent::Focused(false));
    native.window_event(&WindowEvent::Focused(true));
    assert_eq!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
        vec![HostEditEffect::Activate(TextField::Nickname)]
    );
}

#[test]
fn round4_mouse_candidate_commit_does_not_consume_fresh_enter() {
    let mut native = focused();
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    native.window_event(&WindowEvent::Ime(Ime::Preedit("a".into(), Some((1, 1)))));
    // Candidate selected by pointer: no physical Enter event.
    native.window_event(&WindowEvent::Ime(Ime::Preedit("".into(), None)));
    native.window_event(&WindowEvent::Ime(Ime::Commit("a".into())));
    native.window_event(&WindowEvent::Ime(Ime::Disabled));
    assert_eq!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
        vec![HostEditEffect::Activate(TextField::Nickname)]
    );
}

#[test]
fn round4_direct_commit_does_not_consume_fresh_enter() {
    let mut native = focused();
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    native.window_event(&WindowEvent::Ime(Ime::Commit("a".into())));
    assert_eq!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
        vec![HostEditEffect::Activate(TextField::Nickname)]
    );
}

#[test]
fn round4_empty_preedit_cancel_does_not_consume_fresh_enter() {
    let mut native = focused();
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    native.window_event(&WindowEvent::Ime(Ime::Preedit("cancel".into(), None)));
    native.window_event(&WindowEvent::Ime(Ime::Preedit("".into(), None)));
    assert_eq!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
        vec![HostEditEffect::Activate(TextField::Nickname)]
    );
}

#[test]
fn round4_ime_disable_cancel_does_not_consume_fresh_enter() {
    let mut native = focused();
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    native.window_event(&WindowEvent::Ime(Ime::Preedit("cancel".into(), None)));
    native.window_event(&WindowEvent::Ime(Ime::Disabled));
    assert_eq!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
        vec![HostEditEffect::Activate(TextField::Nickname)]
    );
}

#[test]
fn round4_process_space_candidate_then_enter_activates_nickname() {
    let mut native = focused();
    native.window_event(&WindowEvent::Ime(Ime::Enabled));
    native.window_event(&WindowEvent::Ime(Ime::Preedit("a".into(), Some((1, 1)))));
    let process = key(
        &mut native,
        Key::Named(NamedKey::Process),
        KeyCode::Space,
        Some(" "),
        true,
        false,
    );
    assert!(process.consumed && process.effects.is_empty());
    native.window_event(&WindowEvent::Ime(Ime::Preedit("".into(), None)));
    native.window_event(&WindowEvent::Ime(Ime::Commit("a".into())));
    key(
        &mut native,
        Key::Named(NamedKey::Space),
        KeyCode::Space,
        None,
        false,
        false,
    );
    assert_eq!(native.state.editor(TextField::Nickname).text(), "a");
    assert_eq!(
        named(&mut native, NamedKey::Enter, KeyCode::Enter, true, false).effects,
        vec![HostEditEffect::Activate(TextField::Nickname)]
    );
}
