//! Authored settings/control regressions, without storage, renderer or transport.
use slither_client::{
    menu_controls::*,
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
    let studio = slither_client::skin_studio::StudioState::new(Default::default())
        .unwrap()
        .panel([960., 540.], "");
    let mut intent = menu
        .settings
        .intent(QUALITY, Some(false), Activation::Space);
    assert_ne!(intent.panel, studio.id);
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
    let intent = m.settings.intent(RESOLUTION, Some(true), Activation::Enter);
    m.settings.activate(intent);
    m.settings.activate(intent);
    assert_eq!(m.settings.take_write(), Some("2:0:native"));
    m.settings.native_available(false);
    m.settings.storage_result(false);
    assert_eq!(m.settings.preferences().quality, Q::Low);
    assert_eq!(m.settings.preferences().resolution, WorldResolution::Native);
    assert!(m.settings.native_unavailable());
    assert!(m.settings.not_saved());
    assert_eq!(m.settings.take_write(), None);
    m.settings.native_available(true);
    assert!(!m.settings.native_unavailable());
    m.settings
        .activate(m.settings.intent(QUALITY, Some(true), Activation::Space));
    assert_eq!(m.settings.take_write(), Some("2:1:native"));
}

#[test]
fn storage_failure_and_done_preserve_quality_next_play_and_generation() {
    let mut m = menu(Q::High);
    m.edit_nickname(&"Player".encode_utf16().collect::<Vec<_>>());
    let view = m.view(true);
    assert!(!m.settings.open(&m.view(false)));
    assert!(m.settings.open(&view));
    let toggle = m.settings.intent(QUALITY, Some(false), Activation::Space);
    m.settings.activate(toggle);
    m.action(MenuAction::Play, true);
    assert!(!m.view(true).spinner_visible);
    assert_eq!(m.settings.take_write(), Some("2:0:original"));
    m.settings.storage_result(false);
    assert!(m.settings.not_saved());
    assert_eq!(m.settings.take_write(), None);
    m.settings
        .activate(m.settings.intent(DONE, None, Activation::Enter));
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

#[test]
fn modal_clock_matches_picker_rates_caps_and_atomic_rejection() {
    let mut m = menu(Q::High);
    m.settings.step(0.).unwrap();
    m.settings.open(&m.view(true));
    for now in (16..=400).step_by(16) {
        m.settings.step(now as f64).unwrap();
    }
    assert_eq!(m.settings.alpha(), 1.);
    m.settings.close();
    assert!(m.settings.visible());
    for now in (416..=720).step_by(16) {
        m.settings.step(now as f64).unwrap();
    }
    assert!(!m.settings.visible());
    m.settings.open(&m.view(true));
    m.settings.step(10_000.).unwrap();
    assert_eq!(m.settings.alpha(), 0.1);
    let saved = m.settings.clone();
    for now in [9999., f64::NAN, f64::INFINITY] {
        assert!(m.settings.step(now).is_err());
        assert_eq!(m.settings, saved);
    }
}
