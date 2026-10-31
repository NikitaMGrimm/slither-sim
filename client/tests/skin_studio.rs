//! Authored offline transaction, migration and wire fixtures.
use slither_client::{menu_preferences::MenuPreferences, skin_studio::*};

fn state(skin: u8) -> StudioState {
    StudioState::new(AppearanceRecord {
        builtin: skin,
        ..Default::default()
    })
    .unwrap()
}
#[test]
fn cycle_wrap_skip_and_saved_65_are_exact() {
    for (skin, next, expected) in [
        (0, false, 64),
        (64, true, 0),
        (41, true, 43),
        (43, false, 41),
        (65, false, 64),
        (65, true, 0),
    ] {
        let mut studio = state(skin);
        studio.open(0., true);
        studio.cycle(next, UnlockSnapshot::default());
        assert_eq!(studio.draft().unwrap().record.selection().skin, expected);
    }
    let mut studio = state(65);
    studio.open(0., true);
    assert!(
        studio
            .panel([960., 540.], "")
            .controls
            .iter()
            .any(|c| c.name == "Saved skin 65")
    );
    assert_eq!(
        studio
            .save("", 230., UnlockSnapshot::default(), &mut || panic!(
                "retained Save sampled entropy"
            ))
            .unwrap()
            .unwrap()
            .selection()
            .skin,
        65
    );
    let mut studio = state(0);
    studio.open(0., true);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        seen.insert(studio.draft().unwrap().record.builtin);
        studio.cycle(true, UnlockSnapshot::default());
    }
    assert_eq!(seen.len(), 64);
    assert!(!seen.contains(&42));
    assert!(!seen.contains(&65));
}
#[test]
fn save_is_one_commit_cancel_is_none_and_focus_waits_for_return() {
    let mut studio = state(7);
    assert!(!studio.open(0., false));
    assert!(studio.open(0., true));
    studio.step(115.);
    assert_eq!(studio.alpha(), 0.5);
    studio.cycle(true, UnlockSnapshot::default());
    assert_eq!(studio.committed().builtin, 7);
    studio.cancel(115.);
    studio.step(344.);
    assert!(studio.visible());
    assert!(!studio.take_return_focus());
    studio.step(345.);
    assert!(!studio.visible());
    assert!(studio.take_return_focus());
    assert!(!studio.take_return_focus());
    assert_eq!(studio.committed().builtin, 7);
    studio.open(400., true);
    studio.step(630.);
    studio.cycle(true, UnlockSnapshot::default());
    let saved = studio
        .save("Current", 630., UnlockSnapshot::default(), &mut || {
            panic!("built-in Save sampled entropy")
        })
        .unwrap()
        .unwrap();
    assert_eq!(saved.builtin, 8);
    assert_eq!(saved.nickname, "Current");
    assert!(
        studio
            .save("Other", 631., UnlockSnapshot::default(), &mut || panic!(
                "repeated Save sampled entropy"
            ))
            .unwrap()
            .is_none()
    );
    assert_eq!(studio.committed(), &saved);
}
#[test]
fn v1_v2_unknown_and_corrupt_migrate_without_normalizing_custom() {
    let mut legacy = MenuPreferences::new(false);
    let mut custom = vec![0; 8];
    custom.extend([255, 41, 1, 37, 3, 0]);
    legacy.select(65, &custom).unwrap();
    legacy.set_nickname("Stored").unwrap();
    let v1 = legacy.serialize();
    let restored =
        RestoredAppearance::restore(PreferenceInput::Missing, PreferenceInput::Text(v1.clone()));
    assert_eq!(
        restored.record.selection(),
        AppearanceSelection {
            skin: 65,
            custom_bytes: custom.clone()
        }
    );
    let v2 = restored.record.serialize().unwrap();
    assert_eq!(
        RestoredAppearance::restore(PreferenceInput::Text(v2), PreferenceInput::Corrupt).record,
        restored.record
    );
    let unknown = RestoredAppearance::restore(
        PreferenceInput::Text("9:future".into()),
        PreferenceInput::Text(v1.clone()),
    );
    assert!(!unknown.persistence_enabled);
    assert_eq!(unknown.record, AppearanceRecord::default());
    for corrupt in [
        "2:",
        "2:41:b:00:00:gg:--",
        "2::c:00:00::--",
        "2::b:42:00::--",
    ] {
        let result = RestoredAppearance::restore(
            PreferenceInput::Text(corrupt.into()),
            PreferenceInput::Text(v1.clone()),
        );
        assert!(result.persistence_enabled);
        assert_eq!(result.record, AppearanceRecord::default());
    }
    let mut large = vec![0; 8];
    for _ in 0..123 {
        large.extend([255, 41]);
    }
    let record = AppearanceRecord {
        custom_bytes: large,
        active_custom: true,
        ..Default::default()
    };
    assert_eq!(
        AppearanceRecord::parse(&record.serialize().unwrap()),
        Some(record)
    );
    let mut builder = custom;
    builder[..8].copy_from_slice(&[255, 255, 255, 0, 0, 0, 17, 249]);
    assert!(
        AppearanceSelection {
            skin: 65,
            custom_bytes: builder
        }
        .validate_authored()
        .is_ok()
    );
}
#[test]
fn cli_only_overrides_explicit_values_and_builtin_selection_retains_inactive_custom() {
    let mut restored = RestoredAppearance::restore(
        PreferenceInput::Missing,
        PreferenceInput::Text("1:4e616d65:41:00000000000000000102".into()),
    );
    let old = restored.record.clone();
    restored.override_cli(None, None).unwrap();
    assert_eq!(restored.record, old);
    restored.override_cli(Some("CLI"), Some(65)).unwrap();
    assert_eq!(restored.record.nickname, "CLI");
    assert_eq!(restored.record.selection().skin, 65);
    assert!(!restored.record.active_custom);
    assert_eq!(restored.record.custom_bytes, old.custom_bytes);
    let before = restored.record.clone();
    assert!(restored.override_cli(None, Some(66)).is_err());
    assert_eq!(restored.record, before);
}
#[test]
fn every_existing_login_is_byte_identical_after_unchanged_save() {
    for skin in 0..=65 {
        for custom in [vec![], vec![0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 2, 41]] {
            let mut old = MenuPreferences::new(false);
            old.select(skin, &custom).unwrap();
            old.set_nickname("Bytes").unwrap();
            let record = RestoredAppearance::restore(
                PreferenceInput::Missing,
                PreferenceInput::Text(old.serialize()),
            )
            .record;
            let snapshot = record.selection();
            let mut studio = StudioState::new(record).unwrap();
            studio.open(0., true);
            let saved = studio
                .save("Bytes", 230., UnlockSnapshot::default(), &mut || {
                    panic!("retained Save sampled entropy")
                })
                .unwrap()
                .unwrap();
            assert_eq!(saved.selection(), snapshot);
            let before =
                slither_client::protocol::encode_login(291, &[7; 20], "Bytes", skin, Some(&custom))
                    .unwrap();
            let after = saved.selection();
            assert_eq!(
                slither_client::protocol::encode_login(
                    291,
                    &[7; 20],
                    "Bytes",
                    after.skin,
                    Some(&after.custom_bytes)
                )
                .unwrap(),
                before
            );
        }
    }
}

#[test]
fn unreadable_storage_blocks_writes_but_corrupt_storage_can_be_replaced() {
    let legacy = || PreferenceInput::Text("1:4f6c64:41:00000000000000000102".into());
    let unreadable = RestoredAppearance::restore(PreferenceInput::Unreadable, legacy());
    assert!(!unreadable.persistence_enabled);
    assert_eq!(unreadable.record, AppearanceRecord::default());
    assert!(
        !RestoredAppearance::restore(PreferenceInput::Missing, PreferenceInput::Unreadable)
            .persistence_enabled
    );
    let corrupt = RestoredAppearance::restore(PreferenceInput::Corrupt, legacy());
    assert!(corrupt.persistence_enabled);
    assert_eq!(corrupt.record, AppearanceRecord::default());
}

#[test]
fn retained_wire_admission_does_not_validate_new_pattern_structure() {
    for bytes in [vec![1, 2, 3, 4, 5, 6, 7, 8, 2, 1], vec![9; 255]] {
        let selection = AppearanceSelection {
            skin: 7,
            custom_bytes: bytes,
        };
        assert!(selection.validate_wire().is_ok());
        assert!(selection.validate_authored().is_err());
    }
    assert!(
        AppearanceSelection {
            skin: 7,
            custom_bytes: vec![0; 256]
        }
        .validate_wire()
        .is_err()
    );
}

#[test]
fn status_band_does_not_overlap_navigation_footer_or_preview() {
    use slither_client::menu_controls::{ControlId, intersection};
    let mut studio = state(0);
    studio.open(0., true);
    studio.step(230.);
    for width in [280., 360., 640., 672., 960., 1920.] {
        for height in [240., 320., 360., 540., 720.] {
            let viewport = [width, height];
            let panel = studio.panel(viewport, "Saved for this run");
            let resolved = panel.resolve(viewport).unwrap();
            let status = resolved
                .iter()
                .find(|c| c.control.id == ControlId(4))
                .unwrap()
                .rect;
            for id in [ControlId(3), PREVIOUS, NEXT, CANCEL, SAVE] {
                let r = resolved.iter().find(|c| c.control.id == id).unwrap().rect;
                let overlap = intersection(status, r);
                assert!(
                    overlap.width == 0. || overlap.height == 0.,
                    "{viewport:?}: status overlaps {id:?}"
                );
            }
            let overlap = intersection(status, StudioState::preview_bounds(viewport));
            assert!(
                overlap.width == 0. || overlap.height == 0.,
                "{viewport:?}: preview overlaps status"
            );
        }
    }
}
