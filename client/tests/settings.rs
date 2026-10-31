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
fn settings_identity_rejects_studio_activations() {
    let mut menu = menu(Q::High);
    menu.settings.open(&menu.view(true));
    let panel = menu.settings.panel([960., 540.]);
    let studio = slither_client::skin_studio::StudioState::new(Default::default())
        .unwrap()
        .panel([960., 540.], "");
    assert_ne!(panel.id, studio.id);
    let mut input = PanelInputState::default();
    input.sync(&panel, &mut MenuInputState::default());
    let mut intent = input.activate(&panel, Activation::Space).unwrap();
    intent.panel = studio.id;
    menu.settings.activate(intent);
    assert_eq!(menu.settings.preferences().quality, Q::High);
    assert_eq!(menu.settings.take_write(), None);
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
            Some("3:0:original"),
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
            assert_eq!(
                restored.serialize(),
                if quality == Q::Low {
                    "2:0:original"
                } else {
                    "2:1:original"
                }
            );
        }
    }
    assert!(SettingsPreferences::parse(&"1".repeat(MAX_SETTINGS_BYTES + 1)).is_none());
}

#[test]
fn v2_roundtrips_strictly_and_resolution_is_independent_of_quality_and_capability() {
    for quality in [Q::Low, Q::High] {
        for resolution in [WorldResolution::Original, WorldResolution::Native] {
            let p = SettingsPreferences {
                quality,
                resolution,
            };
            assert_eq!(SettingsPreferences::parse(p.serialize()), Some(p));
            assert_eq!(
                SettingsPreferences::restore(Some(p.serialize()), Some("0")),
                p
            );
            let mut state = SettingsState::new(p);
            assert_eq!(state.take_write(), None);
            state.legacy_toggle();
            assert_ne!(state.preferences().quality, quality);
            assert_eq!(state.preferences().resolution, resolution);
        }
    }
    for invalid in [
        "2:0",
        "2:0:Native",
        "2:0:sharp",
        "2:0:native:",
        "2:2:original",
        "2:1:native\n",
        " 2:0:original",
        "3:1:native",
    ] {
        assert!(SettingsPreferences::parse(invalid).is_none(), "{invalid}");
        assert_eq!(
            SettingsPreferences::restore(Some(invalid), Some("0")).resolution,
            WorldResolution::Original
        );
    }
    let mut m = menu(Q::Low);
    m.settings.open(&m.view(true));
    let panel = m.settings.panel([960., 540.]);
    let mut input = PanelInputState::default();
    input.sync(&panel, &mut MenuInputState::default());
    input.focus_control(&panel, RESOLUTION);
    let intent = input.activate(&panel, Activation::Enter).unwrap();
    assert_eq!(intent.checked, Some(true));
    m.settings.activate(intent);
    m.settings.activate(intent);
    assert_eq!(m.settings.take_write(), Some("2:0:native"));
    m.settings.native_available(false);
    m.settings.storage_result(false);
    assert_eq!(m.settings.preferences().quality, Q::Low);
    assert_eq!(m.settings.preferences().resolution, WorldResolution::Native);
    assert!(
        m.settings
            .panel([960., 540.])
            .controls
            .iter()
            .any(|c| c.name == "Native unavailable at this size")
    );
    assert_eq!(m.settings.take_write(), None);
    m.settings.native_available(true);
    assert!(
        !m.settings
            .panel([960., 540.])
            .controls
            .iter()
            .any(|c| c.name == "Native unavailable at this size")
    );
    input.focus_control(&panel, QUALITY);
    m.settings
        .activate(input.activate(&panel, Activation::Space).unwrap());
    assert_eq!(m.settings.take_write(), Some("2:1:native"));
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
    assert_eq!(m.settings.take_write(), Some("2:0:original"));
    assert_eq!(m.settings.take_write(), None);
    input.tab(&panel, false);
    assert_eq!(input.focus(), Some(RESOLUTION));
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
    assert_eq!(m.settings.take_write(), Some("2:0:original"));
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
