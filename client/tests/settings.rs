//! Authored settings/control regressions, without storage, renderer or transport.
use slither_client::{
    menu_controls::*,
    menu_input::*,
    menu_preferences::MenuQuality as Q,
    settings::*,
    start_page::{MenuAction, MenuLocale, StartMenuIntent, StartMenuState},
};

fn menu(quality: Q) -> StartMenuState {
    StartMenuState::new(MenuLocale::English, false, quality)
}

#[test]
fn strict_record_and_exact_legacy_precedence_never_schedule_startup_writes() {
    for (legacy, expected) in [
        (None, Q::High),
        (Some("0"), Q::Low),
        (Some(""), Q::High),
        (Some("00"), Q::High),
        (Some("false"), Q::High),
        (Some(" 0"), Q::High),
        (Some("1"), Q::High),
    ] {
        for record in [
            None,
            Some(""),
            Some("2:0:original"),
            Some("1:0:extra"),
            Some("1:2"),
            Some("1:0\n"),
            Some("１:0"),
        ] {
            let preferences = SettingsPreferences::restore(record, legacy);
            assert_eq!(preferences.quality, expected);
            assert_eq!(SettingsState::new(preferences).take_write(), None);
        }
        for (record, quality) in [("1:0", Q::Low), ("1:1", Q::High)] {
            let restored = SettingsPreferences::restore(Some(record), legacy);
            assert_eq!(restored.quality, quality);
            assert_eq!(restored.serialize(), record);
        }
    }
    assert!(SettingsPreferences::parse(&"1".repeat(MAX_SETTINGS_BYTES + 1)).is_none());
}

#[test]
fn toggle_shared_capture_semantics_focus_and_explicit_set() {
    let mut m = menu(Q::High);
    assert!(m.settings.open(&m.view(true)));
    let mut panel = m.settings.panel([960., 540.]);
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&panel, &mut edit);
    assert_eq!(input.focus(), Some(QUALITY));
    let resolved = panel.resolve([960., 540.]).unwrap();
    let toggle = resolved.iter().find(|r| r.control.id == QUALITY).unwrap();
    let p = [toggle.rect.x + 10., toggle.rect.y + 10.];
    input.pointer(&panel, &resolved, p, Some(true));
    assert!(
        input
            .pointer(&panel, &resolved, [0., 0.], Some(false))
            .is_none()
    );
    input.pointer(&panel, &resolved, p, Some(true));
    input.blur();
    assert!(input.pointer(&panel, &resolved, p, Some(false)).is_none());
    input.pointer(&panel, &resolved, p, Some(true));
    panel.generation += 1;
    input.sync(&panel, &mut edit);
    assert!(input.pointer(&panel, &resolved, p, Some(false)).is_none());
    panel = m.settings.panel([960., 540.]);
    input.sync(&panel, &mut edit);
    let nodes = panel.semantics([960., 540.], &input, &edit).unwrap();
    let node = nodes.iter().find(|n| n.id == QUALITY).unwrap();
    assert_eq!(node.role, AccessibleRole::Checkbox);
    assert_eq!(node.checked, Some(true));
    assert!(node.actions.contains(&PanelAction::SetChecked));
    for activation in [Activation::Pointer, Activation::Enter, Activation::Space] {
        input.focus_control(&panel, QUALITY);
        let intent = if activation == Activation::Pointer {
            input.pointer(&panel, &resolved, p, Some(true));
            input.pointer(&panel, &resolved, p, Some(false)).unwrap()
        } else {
            input.activate(&panel, activation).unwrap()
        };
        assert_eq!(intent.checked, Some(false));
        m.settings.activate(intent);
        m.settings.activate(intent); // repeated Set cannot invert or queue a second write.
        assert_eq!(m.settings.preferences().quality, Q::Low);
    }
    assert_eq!(m.settings.take_write(), Some("1:0"));
    assert_eq!(m.settings.take_write(), None);
    input.tab(&panel, false);
    assert_eq!(input.focus(), Some(DONE));
    input.tab(&panel, false);
    assert_eq!(input.focus(), Some(QUALITY));
    input.tab(&panel, true);
    assert_eq!(input.focus(), Some(DONE));
    panel
        .controls
        .iter_mut()
        .find(|c| c.id == QUALITY)
        .unwrap()
        .enabled = false;
    input.sync(&panel, &mut edit);
    input.focus_control(&panel, QUALITY);
    assert_ne!(input.focus(), Some(QUALITY));
}

#[test]
fn storage_failure_and_done_preserve_quality_next_play_and_generation() {
    let mut m = menu(Q::High);
    m.edit_nickname(&"Player".encode_utf16().collect::<Vec<_>>());
    let view = m.view(true);
    assert!(!m.settings.open(&m.view(false)));
    assert!(m.settings.open(&view));
    let old = m.settings.panel([960., 540.]);
    let mut input = PanelInputState::default();
    let mut edit = MenuInputState::default();
    input.sync(&old, &mut edit);
    let toggle = input.activate(&old, Activation::Space).unwrap();
    m.settings.activate(toggle);
    m.action(MenuAction::Play, true);
    assert!(!m.view(true).spinner_visible);
    assert_eq!(m.settings.take_write(), Some("1:0"));
    m.settings.storage_result(false);
    let panel = m.settings.panel([360., 640.]);
    assert!(
        panel.controls.iter().any(
            |c| matches!(&c.kind, ControlKind::Label { text } if text == "applies this session")
        )
    );
    assert_eq!(m.settings.take_write(), None);
    input.focus_control(&old, DONE);
    m.settings
        .activate(input.activate(&old, Activation::Enter).unwrap());
    assert!(!m.settings.visible());
    assert!(m.settings.open(&m.view(true)));
    m.settings.activate(toggle); // previous generation is inert.
    assert_eq!(m.settings.take_write(), None);
    m.settings.close();
    m.action(MenuAction::Play, true);
    assert!(!m.settings.open(&m.view(true)));
    assert!(
        m.step_with_admission(16., false, true)
            .unwrap()
            .contains(&StartMenuIntent::Play {
                nickname: "Player".into(),
                quality: Q::Low
            })
    );
}
