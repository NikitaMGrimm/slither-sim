//! Champion editor input through the raw egui route, with an authored victory
//! tape. No window, GPU, clipboard or game server.
#![cfg(not(any(target_arch = "wasm32", target_os = "linux")))]
#[path = "support/announcement_tape.rs"]
mod tape;

use egui::{Event, Key, Pos2, TouchPhase, pos2, vec2};
use slither_client::{
    champion::{MESSAGE, SAVE},
    login_controls::NICKNAME,
    menu_controls::{Activation, ControlId},
    menu_theme::MenuTheme,
    start_page::{MenuAction, MenuPhase, StartMenuIntent},
};
use slither_wgpu_slice::{champion_egui::widget_id, game::LocalGame, login_native::NativeLogin};

const EXTENT: [u32; 2] = [960, 720];

fn victory_at(end: u32) -> (LocalGame, NativeLogin) {
    let mut game = tape::playing();
    let mut login = NativeLogin::new("Player").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(8., true, false).unwrap();
    tape::pump(&mut game, 700.);
    login.observe_game(&game).unwrap();
    login.step(700., false, true).unwrap();
    game.receive(game.generation(), &[b'v', 1], 800.).unwrap();
    for now in (808..=end).step_by(40) {
        game.receive(game.generation(), b"p", f64::from(now))
            .unwrap();
        tape::pump(&mut game, f64::from(now));
        login.observe_game(&game).unwrap();
        login.step(f64::from(now), false, false).unwrap();
    }
    (game, login)
}
/// The returned editor at full opacity, laid out in the shared logical window.
fn opened() -> (LocalGame, NativeLogin) {
    let (game, mut login) = victory_at(6008);
    assert_eq!(login.champion.panel.alpha, 1.);
    login.prepare_host(EXTENT, 1., false, None).unwrap();
    login.step(6048., false, false).unwrap();
    pass(&mut login);
    assert_eq!(focus(&login), Some(MESSAGE));
    (game, login)
}
/// Adjacent preedits and pointer moves wait for the redraw's egui pass.
fn pass(login: &mut NativeLogin) {
    login.prepare_host(EXTENT, 1., false, None).unwrap();
}
fn raw(login: &mut NativeLogin, events: Vec<Event>) {
    login.settings_input(egui::RawInput {
        events,
        ..Default::default()
    });
}
/// egui-winit 0.37 reports native auto-repeats as `repeat: false`.
fn key_event(key: Key, pressed: bool, shift: bool) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers {
            shift,
            ..Default::default()
        },
    }
}
fn press(login: &mut NativeLogin, key: Key) {
    raw(login, vec![key_event(key, true, false)]);
}
fn release(login: &mut NativeLogin, key: Key) {
    raw(login, vec![key_event(key, false, false)]);
}
fn tap(login: &mut NativeLogin, key: Key) {
    press(login, key);
    release(login, key);
}
fn text(login: &mut NativeLogin, value: &str) {
    raw(login, vec![Event::Text(value.into())]);
}
fn preedit(login: &mut NativeLogin, value: &str) {
    raw(
        login,
        vec![Event::Ime(egui::ImeEvent::Preedit {
            text: value.into(),
            active_range_chars: None,
        })],
    );
    pass(login);
}
fn commit(login: &mut NativeLogin, value: &str) {
    raw(
        login,
        vec![Event::Ime(egui::ImeEvent::Commit(value.into()))],
    );
}
fn focus(login: &NativeLogin) -> Option<ControlId> {
    login.focused()
}
fn center(login: &NativeLogin, id: ControlId) -> Pos2 {
    login
        .champion
        .ui
        .controls
        .iter()
        .find(|c| c.0 == id)
        .unwrap()
        .1
        .center()
}
fn at(pos: Pos2) -> [f64; 2] {
    [f64::from(pos.x), f64::from(pos.y)]
}
fn click(login: &mut NativeLogin, id: ControlId) {
    let point = at(center(login, id));
    login.pointer(point, Some(true), false);
    login.pointer(point, Some(false), false);
}
fn selection(login: &NativeLogin) -> Option<egui::text::CCursorRange> {
    egui::text_edit::TextEditState::load(login.egui.context().unwrap(), widget_id(MESSAGE))
        .and_then(|s| s.cursor.char_range())
}
fn held(login: &NativeLogin) -> bool {
    login
        .egui
        .context()
        .unwrap()
        .input(|i| i.pointer.any_down())
}
fn touch(id: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch {
        device_id: egui::TouchDeviceId(1),
        id: egui::TouchId(id),
        phase,
        pos,
        force: None,
    }
}
fn primary(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    }
}
fn owner_down(login: &mut NativeLogin, id: u64, pos: Pos2) {
    raw(
        login,
        vec![
            touch(id, TouchPhase::Start, pos),
            Event::PointerMoved(pos),
            primary(pos, true),
        ],
    );
}

#[test]
fn handoff_owns_input_on_the_empty_message_without_a_ring() {
    let (_, mut login) = victory_at(808);
    assert!(login.champion.policy.visible());
    assert!(login.egui_owns_input() && login.intercepts_pointer());
    assert!(login.blocks_gameplay());
    pass(&mut login);
    assert_eq!(focus(&login), Some(MESSAGE));
    assert!(login.champion.ui.message.is_empty());
    assert!(!login.champion.ui.keyboard_focus);
    // The other screens stay shut behind the mounted editor.
    assert!(login.open_settings(true).is_empty());
    assert!(!login.state.settings.visible());
    login.open_studio();
    assert!(!login.studio.state.visible());
    login.activate(true);
    let (intents, _) = login.step(848., true, false).unwrap();
    assert!(
        !intents
            .iter()
            .any(|i| matches!(i, StartMenuIntent::Play { .. }))
    );
}

#[test]
fn entry_releases_held_gameplay_keys() {
    use slither_client::menu_input::{GameplayKey, HostEditEffect};
    use slither_wgpu_slice::menu_input_native::KeyInput;
    use winit::{
        event::ElementState,
        keyboard::{Key as WinitKey, KeyCode, NamedKey, PhysicalKey},
    };
    let mut game = tape::playing();
    let mut login = NativeLogin::new("Player").unwrap();
    login.step(0., true, false).unwrap();
    login.activate(true);
    login.step(8., true, false).unwrap();
    tape::pump(&mut game, 700.);
    login.observe_game(&game).unwrap();
    login.step(700., false, true).unwrap();
    let logical = WinitKey::Named(NamedKey::ArrowLeft);
    let down = login.key(
        KeyInput {
            logical: &logical,
            physical: PhysicalKey::Code(KeyCode::ArrowLeft),
            text: None,
            state: ElementState::Pressed,
            repeat: false,
            synthetic: false,
        },
        false,
    );
    assert!(down.effects.contains(&HostEditEffect::Gameplay {
        key: GameplayKey::Left,
        pressed: true
    }));
    game.receive(game.generation(), &[b'v', 1], 800.).unwrap();
    login.observe_game(&game).unwrap();
    assert!(login.champion.policy.visible());
    let (_, effects) = login.step(808., false, false).unwrap();
    assert!(effects.contains(&HostEditEffect::Gameplay {
        key: GameplayKey::Left,
        pressed: false
    }));
}

#[test]
fn enter_refuses_an_empty_message_but_space_and_pointer_send_it() {
    // The shared seam refuses an empty Enter; Save itself is not disabled.
    let (mut game, mut login) = opened();
    let generation = game.generation();
    tap(&mut login, Key::Enter);
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert_eq!(source, Activation::Enter);
    assert!(!game.save_champion(g, &units, source).unwrap());
    assert!(tape::effects(&mut game).is_empty());
    assert_eq!(focus(&login), Some(MESSAGE));

    tap(&mut login, Key::Tab);
    assert_eq!(focus(&login), Some(SAVE));
    tap(&mut login, Key::Enter);
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert_eq!(source, Activation::Enter);
    assert!(!game.save_champion(g, &units, source).unwrap());

    tap(&mut login, Key::Space);
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert_eq!(source, Activation::Space);
    assert_eq!(g, generation);
    assert!(game.save_champion(g, &units, source).unwrap());
    assert_eq!(tape::effects(&mut game), [vec![255, 118]]);

    let (mut game, mut login) = opened();
    click(&mut login, SAVE);
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert_eq!(source, Activation::Pointer);
    assert!(game.save_champion(g, &units, source).unwrap());
    assert_eq!(tape::effects(&mut game), [vec![255, 118]]);
}

#[test]
fn a_typed_message_sends_on_field_enter_and_a_held_key_saves_once() {
    let (mut game, mut login) = opened();
    text(&mut login, "AB");
    // The host's gate keeps a held Enter to one press.
    for _ in 0..3 {
        press(&mut login, Key::Enter);
    }
    assert_eq!(focus(&login), Some(MESSAGE));
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert!(login.champion.take_submission().is_none());
    assert_eq!(
        (units.as_slice(), source),
        (&[65, 66][..], Activation::Enter)
    );
    assert!(game.save_champion(g, &units, source).unwrap());
    assert_eq!(tape::effects(&mut game), [vec![255, 118, 65, 66]]);
    login.observe_game(&game).unwrap();
    assert!(!login.champion.panel.enabled);
    release(&mut login, Key::Enter);
    press(&mut login, Key::Enter);
    assert!(login.champion.take_submission().is_none());

    let (_, mut login) = opened();
    tap(&mut login, Key::Tab);
    for _ in 0..3 {
        press(&mut login, Key::Space);
    }
    assert!(login.champion.take_submission().is_some());
    assert!(login.champion.take_submission().is_none());
    release(&mut login, Key::Space);
    press(&mut login, Key::Space);
    assert!(login.champion.take_submission().is_some());
}

#[test]
fn only_navigation_keys_reveal_the_ring_and_a_press_clears_it() {
    let (_, mut login) = opened();
    text(&mut login, "abc");
    tap(&mut login, Key::Backspace);
    assert!(!login.champion.ui.keyboard_focus);
    for key in [Key::ArrowLeft, Key::ArrowRight, Key::Home, Key::End] {
        let point = at(center(&login, MESSAGE));
        login.pointer(point, Some(true), false);
        login.pointer(point, Some(false), false);
        assert!(!login.champion.ui.keyboard_focus);
        tap(&mut login, key);
        assert!(login.champion.ui.keyboard_focus, "{key:?}");
    }
    assert!(login.champion.take_submission().is_none());
    click(&mut login, MESSAGE);
    tap(&mut login, Key::Tab);
    assert!(login.champion.ui.keyboard_focus);
    assert_eq!(focus(&login), Some(SAVE));
    let point = at(center(&login, SAVE));
    login.pointer(point, Some(true), false);
    assert!(!login.champion.ui.keyboard_focus);
    login.pointer(point, None, false);
}

#[test]
fn tab_cycles_even_while_held_and_composition_consumes_it() {
    let (_, mut login) = opened();
    // No release between presses: the editor cycles on auto-repeat.
    for expected in [SAVE, MESSAGE, SAVE, MESSAGE] {
        press(&mut login, Key::Tab);
        assert_eq!(focus(&login), Some(expected));
    }
    raw(&mut login, vec![key_event(Key::Tab, true, true)]);
    assert_eq!(focus(&login), Some(SAVE));
    raw(&mut login, vec![key_event(Key::Tab, true, true)]);
    assert_eq!(focus(&login), Some(MESSAGE));
    release(&mut login, Key::Tab);
    preedit(&mut login, "日");
    assert!(login.champion.ui.composing);
    tap(&mut login, Key::Tab);
    assert_eq!(focus(&login), Some(MESSAGE));
    assert!(login.champion.ui.composing);
}

#[test]
fn unowned_keys_stay_inert_on_both_controls() {
    let (_, mut login) = opened();
    text(&mut login, "abcdef");
    let before = selection(&login);
    for key in [Key::ArrowUp, Key::ArrowDown, Key::Escape] {
        tap(&mut login, key);
        assert_eq!(focus(&login), Some(MESSAGE), "{key:?}");
        assert_eq!(selection(&login), before, "{key:?}");
    }
    assert_eq!(login.champion.ui.message, "abcdef");
    tap(&mut login, Key::Tab);
    for key in [
        Key::ArrowLeft,
        Key::ArrowRight,
        Key::Home,
        Key::End,
        Key::ArrowUp,
        Key::ArrowDown,
        Key::Escape,
    ] {
        // Held keys report as repeated presses and move nothing either.
        press(&mut login, key);
        press(&mut login, key);
        release(&mut login, key);
        assert_eq!(focus(&login), Some(SAVE), "{key:?}");
    }
    assert!(login.champion.take_submission().is_none());
    assert!(login.champion.policy.visible());
    assert!(!login.state.settings.visible());
}

#[test]
fn held_caret_and_delete_keys_repeat_in_the_field() {
    let (_, mut login) = opened();
    text(&mut login, "abcd");
    press(&mut login, Key::ArrowLeft);
    press(&mut login, Key::ArrowLeft);
    release(&mut login, Key::ArrowLeft);
    press(&mut login, Key::Backspace);
    press(&mut login, Key::Backspace);
    release(&mut login, Key::Backspace);
    assert_eq!(login.champion.ui.message, "cd");
    text(&mut login, "x");
    assert_eq!(login.champion.ui.message, "xcd");
}

#[test]
fn pointer_press_moves_focus_only_on_controls() {
    let (_, mut login) = opened();
    text(&mut login, "abcdef");
    let before = selection(&login);
    // A press outside keeps the field focused and starts no selection drag.
    login.pointer([2., 2.], Some(true), false);
    login.pointer([100., 100.], None, false);
    login.pointer([100., 100.], Some(false), false);
    assert_eq!(focus(&login), Some(MESSAGE));
    assert_eq!(selection(&login), before);
    text(&mut login, "g");
    assert_eq!(login.champion.ui.message, "abcdefg");
    // A release off Save cancels its activation but keeps the moved focus.
    let save = at(center(&login, SAVE));
    login.pointer(save, Some(true), false);
    assert_eq!(focus(&login), Some(SAVE));
    login.pointer([2., 2.], None, false);
    login.pointer([2., 2.], Some(false), false);
    assert!(login.champion.take_submission().is_none());
    assert_eq!(focus(&login), Some(SAVE));
    click(&mut login, MESSAGE);
    assert_eq!(focus(&login), Some(MESSAGE));
    assert!(login.champion.take_submission().is_none());
}

#[test]
fn pointer_input_during_preedit_keeps_the_selection_and_activates_nothing() {
    let (_, mut login) = opened();
    text(&mut login, "abcdef");
    let field = center(&login, MESSAGE);
    login.pointer(at(field), Some(true), false);
    preedit(&mut login, "x");
    let before = selection(&login);
    login.pointer(
        [f64::from(field.x) + 50., f64::from(field.y)],
        Some(false),
        false,
    );
    assert_eq!(selection(&login), before);
    assert!(!held(&login));
    click(&mut login, SAVE);
    assert!(login.champion.ui.composing);
    assert_eq!(focus(&login), Some(MESSAGE));
    assert!(login.champion.take_submission().is_none());
    commit(&mut login, "z");
    assert!(!login.champion.ui.composing);
    let committed = selection(&login);
    // The retired press cannot extend a selection once composition ends.
    login.pointer([f64::from(field.x) + 100., f64::from(field.y)], None, false);
    assert_eq!(selection(&login), committed);
}

#[test]
fn composition_owns_enter_and_commit_does_not_double_it() {
    let (mut game, mut login) = opened();
    raw(&mut login, vec![Event::Paste("A😀".into())]);
    assert_eq!(login.champion.ui.message, "A  ");
    preedit(&mut login, "日");
    assert_eq!(login.champion.ui.message, "A  日");
    // egui-winit turns an IME's Process key with physical Enter into this press.
    press(&mut login, Key::Enter);
    commit(&mut login, "B");
    assert_eq!(login.champion.ui.message, "A  B");
    // Still the same physical press.
    press(&mut login, Key::Enter);
    assert!(login.champion.take_submission().is_none());
    release(&mut login, Key::Enter);
    press(&mut login, Key::Enter);
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert_eq!(source, Activation::Enter);
    assert!(game.save_champion(g, &units, source).unwrap());
    assert_eq!(tape::effects(&mut game), [vec![255, 118, 65, 32, 32, 66]]);
}

#[test]
fn escape_and_blur_restore_the_composition_snapshot() {
    let (_, mut login) = opened();
    text(&mut login, "abc");
    preedit(&mut login, "日本");
    assert_eq!(login.champion.ui.message, "abc日本");
    tap(&mut login, Key::Escape);
    assert_eq!(login.champion.ui.message, "abc");
    assert!(!login.champion.ui.composing);
    assert_eq!(focus(&login), Some(MESSAGE));
    preedit(&mut login, "語");
    raw(&mut login, vec![Event::WindowFocused(false)]);
    assert_eq!(login.champion.ui.message, "abc");
    assert!(!login.champion.ui.composing);
    raw(&mut login, vec![Event::WindowFocused(true)]);
    // An IME that clears its own preview leaves the committed text.
    preedit(&mut login, "語");
    preedit(&mut login, "");
    assert_eq!(login.champion.ui.message, "abc");
    assert!(!login.champion.ui.composing);
}

#[test]
fn window_blur_keeps_focus_and_retires_a_press() {
    let (_, mut login) = opened();
    tap(&mut login, Key::Tab);
    let save = at(center(&login, SAVE));
    login.pointer(save, Some(true), false);
    raw(&mut login, vec![Event::WindowFocused(false)]);
    assert!(!held(&login));
    raw(&mut login, vec![Event::WindowFocused(true)]);
    login.pointer(save, Some(false), false);
    assert!(login.champion.take_submission().is_none());
    assert_eq!(focus(&login), Some(SAVE));
}

#[test]
fn long_preedit_and_message_stay_bounded_in_utf16_units() {
    use slither_client::menu_input::MAX_PREEDIT_UNITS;
    let (mut game, mut login) = opened();
    preedit(&mut login, &"語".repeat(MAX_PREEDIT_UNITS * 4));
    assert_eq!(
        login.champion.ui.message.encode_utf16().count(),
        MAX_PREEDIT_UNITS
    );
    tap(&mut login, Key::Escape);
    assert!(login.champion.ui.message.is_empty());
    text(&mut login, &"W".repeat(200));
    assert_eq!(login.champion.ui.message.encode_utf16().count(), 140);
    text(&mut login, "X");
    assert_eq!(login.champion.ui.message, "W".repeat(140));
    tap(&mut login, Key::Enter);
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert_eq!(units.len(), 140);
    assert!(game.save_champion(g, &units, source).unwrap());
    let sent = tape::effects(&mut game);
    assert_eq!(sent[0].len(), 142);
}

#[test]
fn the_touch_that_owns_the_pointer_decides_the_click() {
    let up = |login: &mut NativeLogin, id, pos| {
        raw(
            login,
            vec![
                touch(id, TouchPhase::End, pos),
                primary(pos, false),
                Event::PointerGone,
            ],
        );
    };
    // A second finger's cancel leaves the first finger's press to click once.
    let (_, mut login) = opened();
    let pos = center(&login, SAVE);
    let other = pos + vec2(100., 100.);
    owner_down(&mut login, 7, pos);
    raw(&mut login, vec![touch(8, TouchPhase::Start, other)]);
    raw(&mut login, vec![touch(8, TouchPhase::Cancel, other)]);
    assert!(held(&login));
    up(&mut login, 7, pos);
    assert!(login.champion.take_submission().is_some());
    assert!(login.champion.take_submission().is_none());
    assert!(!held(&login));
    // The owner's cancel drops the press: no click and nothing held.
    let (_, mut login) = opened();
    let pos = center(&login, SAVE);
    owner_down(&mut login, 7, pos);
    raw(
        &mut login,
        vec![touch(7, TouchPhase::Cancel, pos), Event::PointerGone],
    );
    assert!(!held(&login));
    assert!(login.champion.take_submission().is_none());
}

/// The editor is up while the login below it is hidden: the desktop frame loop
/// must keep giving it egui passes when no input arrives.
#[test]
fn frames_without_input_keep_the_editor_running_while_the_login_is_hidden() {
    let (_, mut login) = victory_at(808);
    let view = login.state.view(false);
    assert!(!view.visible && !view.spinner_visible);
    assert!(login.champion.policy.visible());
    pass(&mut login);
    let save = at(center(&login, SAVE));
    login.pointer(save, None, false);
    let context = login.egui.context().unwrap().clone();
    let start = context.cumulative_pass_nr();
    let mut now = 840.;
    for _ in 0..4 {
        login.step(now, false, false).unwrap();
        assert!(login.needs_menu_pass(false));
        pass(&mut login);
        now += 16.;
    }
    assert_eq!(context.cumulative_pass_nr(), start + 4);
    assert!(login.champion.ui.fade[0] > 0.);
}

#[test]
fn save_answers_the_matching_release_however_long_or_far_the_press_went() {
    // Longer than egui's 0.8 s click limit.
    let (_, mut login) = opened();
    let save = at(center(&login, SAVE));
    login.pointer(save, Some(true), false);
    login.step(7100., false, false).unwrap();
    pass(&mut login);
    login.pointer(save, Some(false), false);
    let (_, _, source) = login.champion.take_submission().unwrap();
    assert_eq!(source, Activation::Pointer);
    assert!(login.champion.take_submission().is_none());
    // Further than egui's 6 point click limit, then back.
    let (_, mut login) = opened();
    let save = at(center(&login, SAVE));
    login.pointer(save, Some(true), false);
    login.pointer([2., 2.], None, false);
    pass(&mut login);
    login.pointer(save, None, false);
    pass(&mut login);
    login.pointer(save, Some(false), false);
    assert!(login.champion.take_submission().is_some());
    assert!(login.champion.take_submission().is_none());
    // Released elsewhere, and a later release there, send nothing.
    let (_, mut login) = opened();
    let save = at(center(&login, SAVE));
    login.pointer(save, Some(true), false);
    login.pointer([2., 2.], Some(false), false);
    assert!(login.champion.take_submission().is_none());
    login.pointer(save, None, false);
    pass(&mut login);
    login.pointer(save, Some(false), false);
    assert!(login.champion.take_submission().is_none());
    // A press elsewhere never captures Save.
    login.pointer([2., 2.], Some(true), false);
    login.pointer(save, None, false);
    pass(&mut login);
    login.pointer(save, Some(false), false);
    assert!(login.champion.take_submission().is_none());
}

#[test]
fn another_button_cannot_hand_a_primary_press_to_save() {
    use egui::PointerButton::{Middle, Primary, Secondary};
    let button = |login: &mut NativeLogin, pos, button, pressed| {
        raw(
            login,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    };
    for other in [Secondary, Middle] {
        // Primary went down in the message, so its release is not Save's.
        let (_, mut login) = opened();
        text(&mut login, "message");
        let (field, save) = (center(&login, MESSAGE), center(&login, SAVE));
        button(&mut login, field, Primary, true);
        button(&mut login, save, other, true);
        button(&mut login, save, Primary, false);
        button(&mut login, save, other, false);
        assert!(login.champion.take_submission().is_none(), "{other:?}");
        // Another button leaves Save's own primary press to click once.
        button(&mut login, save, Primary, true);
        button(&mut login, save, other, true);
        button(&mut login, save, other, false);
        button(&mut login, save, Primary, false);
        let (_, _, source) = login.champion.take_submission().unwrap();
        assert_eq!(source, Activation::Pointer, "{other:?}");
        assert!(login.champion.take_submission().is_none(), "{other:?}");
    }
}

/// egui forgets the press on each of these, but still reports the later release.
#[test]
fn a_cancelled_or_disabled_save_press_never_clicks_after_a_long_hold() {
    let hold = |login: &mut NativeLogin| {
        let save = center(login, SAVE);
        login.pointer(at(save), Some(true), false);
        login.step(7100., false, false).unwrap();
        pass(login);
        save
    };
    // Window blur.
    let (_, mut login) = opened();
    let save = hold(&mut login);
    raw(&mut login, vec![Event::WindowFocused(false)]);
    raw(&mut login, vec![Event::WindowFocused(true)]);
    login.pointer(at(save), Some(false), false);
    assert!(login.champion.take_submission().is_none());
    // The owning touch's cancel.
    let (_, mut login) = opened();
    let save = center(&login, SAVE);
    owner_down(&mut login, 7, save);
    login.step(7100., false, false).unwrap();
    pass(&mut login);
    raw(
        &mut login,
        vec![touch(7, TouchPhase::Cancel, save), Event::PointerGone],
    );
    login.pointer(at(save), None, false);
    pass(&mut login);
    login.pointer(at(save), Some(false), false);
    assert!(login.champion.take_submission().is_none());
    // Sending by key disables Save under the held press.
    for redraw in [false, true] {
        let (mut game, mut login) = opened();
        text(&mut login, "A");
        let save = hold(&mut login);
        tap(&mut login, Key::Space);
        let (g, units, source) = login.champion.take_submission().unwrap();
        assert!(game.save_champion(g, &units, source).unwrap());
        login.observe_game(&game).unwrap();
        assert!(!login.champion.panel.enabled);
        if redraw {
            pass(&mut login);
        }
        login.pointer(at(save), Some(false), false);
        assert!(login.champion.take_submission().is_none());
    }
    // Screen replacement.
    let (mut game, mut login) = opened();
    let save = hold(&mut login);
    let generation = game.generation();
    game.closed(generation);
    login.observe_game(&game).unwrap();
    let mut now = 7140.;
    while !login.champion.policy.finished() {
        tape::pump(&mut game, now);
        login.observe_game(&game).unwrap();
        login.step(now, game.can_play(), false).unwrap();
        now += 40.;
        assert!(now < 20_000.);
    }
    login.observe_game(&game).unwrap();
    login.sync_return_focus(game.can_play());
    login
        .prepare_host(EXTENT, 1., game.can_play(), None)
        .unwrap();
    let before = login.state.view(game.can_play()).phase;
    login.pointer(at(save), Some(false), game.can_play());
    let (intents, _) = login.step(now, game.can_play(), false).unwrap();
    assert!(login.champion.take_submission().is_none());
    assert!(
        !intents
            .iter()
            .any(|i| matches!(i, StartMenuIntent::Play { .. }))
    );
    assert_eq!(login.state.view(game.can_play()).phase, before);
}

#[test]
fn two_clicks_within_half_a_second_select_a_word() {
    for (gap, selects) in [(100., true), (400., true), (600., false)] {
        let (_, mut login) = opened();
        text(&mut login, "abcdef");
        let point = at(center(&login, MESSAGE));
        login.pointer(point, Some(true), false);
        login.pointer(point, Some(false), false);
        login.step(6048. + gap, false, false).unwrap();
        pass(&mut login);
        login.pointer(point, Some(true), false);
        login.pointer(point, Some(false), false);
        assert_eq!(!selection(&login).unwrap().is_empty(), selects, "{gap}");
    }
}

#[test]
fn a_press_across_the_handoff_or_the_exit_does_not_click() {
    // Pressed on the returning login, released over Save once the editor is up.
    let mut game = tape::playing();
    let mut login = NativeLogin::new("Player").unwrap();
    for now in (0..=9600).step_by(16) {
        login.step(f64::from(now), true, false).unwrap();
    }
    login.prepare_host(EXTENT, 1., true, None).unwrap();
    let pos = pos2(480., 500.);
    assert!(login.egui.active());
    raw(
        &mut login,
        vec![Event::PointerMoved(pos), primary(pos, true)],
    );
    tape::pump(&mut game, 9700.);
    game.receive(game.generation(), &[b'v', 1], 9710.).unwrap();
    login.observe_game(&game).unwrap();
    assert!(login.champion.policy.visible());
    login.step(9720., false, false).unwrap();
    login.prepare_host(EXTENT, 1., false, None).unwrap();
    let save = center(&login, SAVE);
    raw(
        &mut login,
        vec![Event::PointerMoved(save), primary(save, false)],
    );
    assert!(login.champion.take_submission().is_none());
    assert!(login.champion.policy.visible());

    // Pressed on Save, released after the editor finished, over the login.
    let (mut game, mut login) = opened();
    let save = at(center(&login, SAVE));
    login.pointer(save, Some(true), false);
    let generation = game.generation();
    game.closed(generation);
    login.observe_game(&game).unwrap();
    let mut now = 6088.;
    while !login.champion.policy.finished() {
        tape::pump(&mut game, now);
        login.observe_game(&game).unwrap();
        login.step(now, game.can_play(), false).unwrap();
        now += 40.;
        assert!(now < 20_000.);
    }
    login.observe_game(&game).unwrap();
    login.sync_return_focus(game.can_play());
    login
        .prepare_host(EXTENT, 1., game.can_play(), None)
        .unwrap();
    let before = login.state.view(game.can_play()).phase;
    login.pointer(save, Some(false), game.can_play());
    assert!(login.champion.take_submission().is_none());
    let (intents, _) = login.step(now, game.can_play(), false).unwrap();
    assert!(
        !intents
            .iter()
            .any(|i| matches!(i, StartMenuIntent::Play { .. }))
    );
    assert_eq!(login.state.view(game.can_play()).phase, before);
    assert!(!held(&login));
}

#[test]
fn widgets_sit_at_the_core_rectangles_through_the_fade_and_every_scale() {
    let (mut game, mut login) = victory_at(808);
    // Across the return, the panel fade only changes opacity, never geometry.
    for now in (848..=6008).step_by(160) {
        game.receive(game.generation(), b"p", f64::from(now))
            .unwrap();
        tape::pump(&mut game, f64::from(now));
        login.observe_game(&game).unwrap();
        login.step(f64::from(now), false, false).unwrap();
        pass(&mut login);
        let panel = &login.champion.panel;
        let resolved = panel.resolve([960., 720.]).unwrap();
        for id in [MESSAGE, SAVE] {
            let r = resolved.iter().find(|r| r.control.id == id).unwrap().rect;
            let widget = login
                .champion
                .ui
                .controls
                .iter()
                .find(|c| c.0 == id)
                .unwrap()
                .1;
            assert_eq!(
                [widget.min.x, widget.min.y, widget.width(), widget.height()],
                [r.x as f32, r.y as f32, r.width as f32, r.height as f32],
                "{id:?} at {now}"
            );
        }
    }
    // Shrunk windows use the same core scale; a DPR change keeps logical hits.
    for (logical, dpr) in [([960., 720.], 1.5), ([960., 720.], 2.), ([360., 400.], 1.)] {
        let extent = [(logical[0] * dpr) as u32, (logical[1] * dpr) as u32];
        login.step(6200., false, false).unwrap();
        login.prepare_host(extent, dpr, false, None).unwrap();
        let area = [f64::from(extent[0]) / dpr, f64::from(extent[1]) / dpr];
        assert_eq!(login.champion.viewport, area);
        let resolved = login.champion.panel.resolve(area).unwrap();
        let r = resolved.iter().find(|r| r.control.id == SAVE).unwrap().rect;
        let save = center(&login, SAVE);
        assert_eq!(save.x, (r.x + r.width / 2.) as f32);
        assert_eq!(save.y, (r.y + r.height / 2.) as f32);
    }
}

#[test]
fn save_overlays_step_linearly_on_the_callback_clock() {
    let tokens = MenuTheme::default();
    let (_, mut login) = opened();
    let save = at(center(&login, SAVE));
    let mut now = 6100.;
    let mut step = |login: &mut NativeLogin| {
        now += 16.;
        login.step(now, false, false).unwrap();
        login.champion.ui.fade
    };
    login.pointer(save, None, false);
    pass(&mut login);
    assert_eq!(step(&mut login), [tokens.hover_rate, 0.]);
    assert_eq!(step(&mut login), [2. * tokens.hover_rate, 0.]);
    login.pointer(save, Some(true), false);
    let held_fade = step(&mut login);
    assert_eq!(held_fade[1], tokens.press_rate);
    login.pointer(save, Some(false), false);
    assert!(login.champion.take_submission().is_some());
    let released = step(&mut login);
    assert_eq!(released[1], (held_fade[1] - tokens.release_rate).max(0.));
    login.pointer([2., 2.], None, false);
    pass(&mut login);
    let away = step(&mut login);
    assert!(away[0] < released[0]);
    for _ in 0..16 {
        step(&mut login);
    }
    assert_eq!(step(&mut login), [0., 0.]);
}

#[test]
fn sent_or_terminal_editor_takes_no_input() {
    let (mut game, mut login) = opened();
    text(&mut login, "A");
    click(&mut login, SAVE);
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert!(game.save_champion(g, &units, source).unwrap());
    login.observe_game(&game).unwrap();
    assert!(!login.champion.panel.enabled);
    text(&mut login, "B");
    assert_eq!(login.champion.ui.message, "A");
    click(&mut login, SAVE);
    tap(&mut login, Key::Enter);
    tap(&mut login, Key::Space);
    assert!(login.champion.take_submission().is_none());
    // A duplicate would never reach the wire even if the panel still took it.
    assert!(!game.save_champion(g, &units, source).unwrap());
}

#[test]
fn a_fresh_victory_starts_empty_without_the_old_undo_history() {
    let (mut game, mut login) = opened();
    text(&mut login, "old");
    tap(&mut login, Key::Enter);
    let (g, units, source) = login.champion.take_submission().unwrap();
    assert!(game.save_champion(g, &units, source).unwrap());
    tape::effects(&mut game);
    game.receive(g, &[b'v', 2], 6100.).unwrap();
    let mut now = 6108.;
    while !login.champion.policy.finished() {
        tape::pump(&mut game, now);
        login.observe_game(&game).unwrap();
        login.step(now, game.can_play(), false).unwrap();
        now += 40.;
        assert!(now < 20_000.);
    }
    for _ in 0..200 {
        if game.can_play() {
            break;
        }
        game.receive(g, b"p", now).unwrap();
        tape::pump(&mut game, now);
        login.observe_game(&game).unwrap();
        login.step(now, game.can_play(), false).unwrap();
        now += 40.;
    }
    assert!(game.can_play());
    assert!(game.can_play());
    login.sync_return_focus(true);
    assert_eq!(login.focused(), Some(NICKNAME));
    tape::rejoin(&mut game);
    assert_ne!(game.generation(), g);
    game.receive(game.generation(), &[b'v', 1], now).unwrap();
    login.observe_game(&game).unwrap();
    assert!(login.champion.policy.visible());
    login.step(now + 8., false, false).unwrap();
    pass(&mut login);
    assert_eq!(focus(&login), Some(MESSAGE));
    assert!(login.champion.ui.message.is_empty());
    raw(
        &mut login,
        vec![Event::Key {
            key: Key::Z,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        }],
    );
    assert!(login.champion.ui.message.is_empty());
}

#[test]
fn early_play_waits_for_champion_ack_or_disconnect_and_editor_exit() {
    for disconnect in [false, true] {
        let (mut game, mut login) = victory_at(2408);
        let generation = game.generation();
        let returning = login.state.view(false);
        assert_eq!(returning.phase, MenuPhase::Returning);
        assert!(returning.transition > 0. && returning.transition < 1.);
        assert!(
            game.save_champion(generation, &[65], Activation::Pointer)
                .unwrap()
        );
        assert_eq!(tape::effects(&mut game), [vec![255, 118, 65]]);

        // Even with admission supplied, neither an explicit Play nor native
        // Enter can bypass the mounted editor or queue a replay behind it.
        let reject_play = |login: &mut NativeLogin, now| {
            assert!(login.champion.policy.visible());
            let view = login.state.view(true);
            assert!(view.play_disabled && !view.ordinary_controls_visible);
            login.state.action(MenuAction::Play, true);
            login.activate(true);
            let (intents, _) = login.step(now, true, false).unwrap();
            assert!(
                !intents
                    .iter()
                    .any(|i| matches!(i, StartMenuIntent::Play { .. }))
            );
            assert_ne!(login.state.view(true).phase, MenuPhase::Connecting);
        };
        reject_play(&mut login, 2408.);
        for now in (2448..=6008).step_by(40) {
            game.receive(generation, b"p", f64::from(now)).unwrap();
            tape::pump(&mut game, f64::from(now));
            login.observe_game(&game).unwrap();
            reject_play(&mut login, f64::from(now));
        }
        assert_eq!(login.state.view(false).transition, 0.);
        assert!(game.champion_session().champion_sent);
        assert!(!game.champion_session().champion_acknowledged);
        if disconnect {
            game.closed(generation);
        } else {
            game.receive(generation, &[b'v', 2], 6048.).unwrap();
        }
        for now in (6048..=7208).step_by(40) {
            tape::pump(&mut game, f64::from(now));
            login.observe_game(&game).unwrap();
            if login.champion.policy.visible() {
                reject_play(&mut login, f64::from(now));
            } else {
                let (intents, _) = login.step(f64::from(now), game.can_play(), false).unwrap();
                assert!(
                    !intents
                        .iter()
                        .any(|i| matches!(i, StartMenuIntent::Play { .. }))
                );
            }
        }
        assert!(login.champion.policy.finished() && game.can_play());
        assert_eq!(game.champion_session().champion_acknowledged, !disconnect);
        assert_eq!(game.generation(), generation);
        assert!(!login.state.view(true).play_disabled);
        login.state.action(MenuAction::Play, game.can_play());
        let (intents, _) = login.step(7248., game.can_play(), false).unwrap();
        assert!(matches!(intents.as_slice(), [StartMenuIntent::Play { .. }]));
    }
}

#[test]
fn returned_champion_empty_full_and_selected_paint_at_high_dpr() {
    let (_, mut login) = victory_at(6008);
    assert_eq!(login.champion.panel.alpha, 1.);
    assert!(!login.state.view(false).last_life.label.is_empty());
    for dpr in [4., 5.] {
        let extent = [(960. * dpr) as u32, (720. * dpr) as u32];
        login.prepare_host(extent, dpr, false, None).unwrap();
        for n in [0, 140, 50] {
            if n > 0 {
                text(&mut login, &"W".repeat(n));
            }
            if n == 50 {
                raw(
                    &mut login,
                    vec![Event::Key {
                        key: Key::A,
                        physical_key: None,
                        pressed: true,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    }],
                );
            }
            let picture = login.prepare_host(extent, dpr, false, None).unwrap();
            assert_eq!(picture.viewport_scale, dpr);
            let textures = login.egui.context().unwrap().tex_manager();
            let textures = textures.read();
            assert!(
                textures
                    .allocated()
                    .all(|(_, meta)| meta.size.into_iter().all(|side| side <= 2048))
            );
            assert!(
                textures
                    .allocated()
                    .map(|(_, meta)| meta.bytes_used())
                    .sum::<usize>()
                    <= 32 * 1024 * 1024
            );
        }
        login.champion.ui.message.clear();
    }
}

#[test]
fn terminal_transport_before_ack_ends_editor_without_resend_or_early_login() {
    for end in [808, 6008] {
        for failure in 0..3 {
            let (mut game, mut login) = victory_at(end);
            let generation = game.generation();
            let last = login.state.view(false).last_life.clone();
            assert!(login.champion.policy.visible());
            assert!(
                game.save_champion(generation, &[65], Activation::Pointer)
                    .unwrap()
            );
            let pending = game.effect().unwrap();
            match failure {
                0 => game.closed(generation),
                1 => game.fail("Synthetic socket error"),
                _ => game.acknowledge(generation, pending.id, false).unwrap(),
            }
            assert!(tape::effects(&mut game).is_empty());
            login.observe_game(&game).unwrap();
            assert!(!login.champion.panel.enabled);
            if end == 808 {
                assert!(game.login_session().death_pending);
                assert!(!login.state.view(true).visible);
            }
            assert!(login.needs_redraw());
            assert!(
                !game
                    .save_champion(generation, &[66], Activation::Pointer)
                    .unwrap()
            );
            for now in (end + 40..=end + 4800).step_by(40) {
                assert!(tape::pump(&mut game, f64::from(now)).is_empty());
                login.observe_game(&game).unwrap();
                login.step(f64::from(now), game.can_play(), false).unwrap();
                let session = game.login_session();
                if session.death_pending {
                    assert!(login.needs_redraw());
                }
                if session.death_pending || session.return_fraction > 0. {
                    assert_ne!(login.state.view(true).phase, MenuPhase::Login);
                }
            }
            assert!(login.champion.policy.finished());
            assert!(!login.champion.policy.visible());
            assert_eq!(login.champion.policy.ordinary_alpha(), 1.);
            assert!(!game.champion_session().champion_acknowledged);
            assert_eq!(game.generation(), generation);
            assert!(game.can_play());
            let view = login.state.view(game.can_play());
            assert_eq!(view.phase, MenuPhase::Login);
            assert!(!view.champion_pending && view.ordinary_controls_visible && !view.disabled);
            assert_eq!(view.last_life, last);
            login.sync_return_focus(game.can_play());
            assert_eq!(login.focused(), Some(NICKNAME));
            assert!(login.champion.take_submission().is_none());
            login
                .prepare_host(EXTENT, 1., game.can_play(), None)
                .unwrap();
        }
    }
}
