//! Authored offline builder/codec transactions. No reference code or art.
use slither_client::{skin_builder::*, skin_studio::*};

fn studio(record: AppearanceRecord) -> StudioState {
    let mut s = StudioState::new(record).unwrap();
    assert!(s.open(0., true));
    s.step(230.);
    s.set_section(StudioSection::Builder, 230.);
    s.step(410.);
    s
}
fn saved(s: &mut StudioState) -> AppearanceRecord {
    s.save("Bytes", 410., UnlockSnapshot::default(), &mut || {
        panic!("retained/empty sampled entropy")
    })
    .unwrap()
    .unwrap()
}
#[test]
fn swatch_order_cap_and_adjacent_rle_are_exact() {
    assert_eq!(
        PALETTES.to_vec(),
        (0..=35).chain([37, 39, 41]).collect::<Vec<_>>()
    );
    let mut b = BuilderDraft::new(&[]);
    assert!(!b.append(40));
    for n in 0..46 {
        assert!(b.append(PALETTES[n % 39]));
    }
    assert_eq!(b.pattern().colours(), 46);
    assert!(b.append(6));
    let at_cap = b.pattern().clone();
    assert_eq!(at_cap.colours(), 47);
    assert!(!b.append(41));
    assert_eq!(b.pattern(), &at_cap);
    b.reset();
    for palette in [41, 41, 37, 37, 37, 39] {
        assert!(b.append(palette));
    }
    assert_eq!(
        b.pattern().encode([0; 8]).unwrap(),
        [vec![0; 8], vec![2, 41, 3, 37, 1, 39]].concat()
    );
}
#[test]
fn long_legacy_runs_are_bounded_editable_and_split_at_255() {
    let mut legacy = vec![0; 8];
    for _ in 0..123 {
        legacy.extend([255, 1]);
    }
    let mut b = BuilderDraft::new(&legacy);
    assert_eq!(
        b.pattern().runs(),
        &[PatternRun {
            count: 31_365,
            palette: 1
        }]
    );
    assert!(!b.append(2));
    b.recolour(41).unwrap();
    let bytes = b.pattern().encode([255, 255, 255, 0, 0, 0, 7, 8]).unwrap();
    assert_eq!(bytes.len(), 254);
    assert!(
        bytes[8..]
            .as_chunks::<2>()
            .0
            .iter()
            .all(|p| *p == [255, 41])
    );
    assert!(b.remove());
    assert_eq!(b.pattern().colours(), 0);
    assert!(b.append(39));
    let mut merge = BuilderDraft::new(&[0, 0, 0, 0, 0, 0, 0, 0, 255, 1, 1, 2, 255, 1]);
    merge.selected = 1;
    merge.recolour(1).unwrap();
    assert_eq!(
        &merge.pattern().encode([0; 8]).unwrap()[8..],
        &[255, 1, 255, 1, 1, 1]
    );
    let mut preferences = slither_client::menu_preferences::MenuPreferences::new(false);
    preferences.select(65, &legacy).unwrap();
    let restored = RestoredAppearance::restore(
        PreferenceInput::Missing,
        PreferenceInput::Text(preferences.serialize()),
    )
    .record;
    let mut s = studio(restored);
    assert_eq!(saved(&mut s).custom_bytes, legacy);
    let mut s = studio(AppearanceRecord {
        builtin: 65,
        custom_base: 65,
        custom_bytes: legacy,
        active_custom: true,
        ..Default::default()
    });
    s.edit_builder(|b| b.recolour(41).unwrap());
    let mut samples = [4, 5].into_iter();
    let changed = s
        .save("Bytes", 410., UnlockSnapshot::default(), &mut || {
            samples.next().unwrap()
        })
        .unwrap()
        .unwrap();
    assert_eq!(&changed.custom_bytes[..8], &[255, 255, 255, 0, 0, 0, 4, 5]);
    assert_eq!(changed.custom_bytes.len(), 254);
    assert_eq!(changed.custom_base, 65);
}
#[test]
fn changed_save_samples_two_bytes_once_and_preview_never_serializes_a_spacer() {
    let mut s = studio(AppearanceRecord {
        builtin: 65,
        ..Default::default()
    });
    for palette in PALETTES {
        s.edit_builder(|b| {
            assert!(b.append(palette));
        });
    }
    let preview = s.preview_selection().unwrap();
    assert_eq!(preview.skin, 65);
    assert_eq!(&preview.custom_bytes[..8], &[255, 255, 255, 0, 0, 0, 0, 0]);
    assert!(
        preview.custom_bytes[8..]
            .as_chunks::<2>()
            .0
            .iter()
            .all(|p| p[0] == 1 && p[1] != 40)
    );
    let resolved =
        slither_client::presentation::resolve_skin(65, &preview.custom_bytes, false).unwrap();
    assert!(resolved.custom && resolved.antenna.is_none());
    let mut calls = 0;
    let mut random = || {
        calls += 1;
        if calls == 1 { 0 } else { 255 }
    };
    assert!(
        s.save("😀", 410., UnlockSnapshot::default(), &mut random)
            .is_err()
    );
    let record = s
        .save("Bytes", 410., UnlockSnapshot::default(), &mut random)
        .unwrap()
        .unwrap();
    assert!(
        s.save("Bytes", 411., UnlockSnapshot::default(), &mut random)
            .unwrap()
            .is_none()
    );
    assert_eq!(calls, 2);
    assert_eq!(&record.custom_bytes[..8], &[255, 255, 255, 0, 0, 0, 0, 255]);
    assert_eq!(&record.custom_bytes[8..], &preview.custom_bytes[8..]);
    assert_eq!(record.custom_base, 65);
    record.selection().validate_authored().unwrap();
}
#[test]
fn reset_empty_save_reuses_committed_custom_or_builtin_and_cancel_discards_all() {
    for active_custom in [false, true] {
        let record = AppearanceRecord {
            nickname: "Old".into(),
            builtin: 64,
            custom_base: 7,
            custom_bytes: vec![1, 2, 3, 4, 5, 6, 7, 8, 2, 41],
            active_custom,
            cosmetic: Some(3),
        };
        let mut s = studio(record.clone());
        s.edit_builder(|b| b.reset());
        assert_eq!(s.draft().unwrap().record, record);
        let got = saved(&mut s);
        assert_eq!(got.custom_bytes, record.custom_bytes);
        assert_eq!(got.custom_base, 7);
        assert!(got.active_custom);
        assert_eq!(got.builtin, 64);
        assert_eq!(got.cosmetic, Some(3));
        let mut s = studio(record.clone());
        s.edit_builder(|b| {
            b.reset();
            b.append(39);
        });
        s.cancel(410.);
        s.step(640.);
        assert_eq!(s.committed(), &record);
    }
    let record = AppearanceRecord {
        builtin: 65,
        ..Default::default()
    };
    let mut s = studio(record.clone());
    s.edit_builder(|b| {
        b.append(41);
        b.reset();
    });
    assert_eq!(saved(&mut s).selection(), record.selection());
    let mut s = studio(AppearanceRecord {
        builtin: 7,
        ..Default::default()
    });
    s.edit_builder(|b| {
        b.append(41);
    });
    s.set_section(StudioSection::BuiltIn, 410.);
    s.cycle(true, UnlockSnapshot::default());
    let mut calls = 0;
    let got = s
        .save("Bytes", 410., UnlockSnapshot::default(), &mut || {
            calls += 1;
            9
        })
        .unwrap()
        .unwrap();
    assert!(!got.active_custom);
    assert_eq!(got.custom_base, 7);
    assert_eq!(got.custom_bytes, vec![255, 255, 255, 0, 0, 0, 9, 9, 1, 41]);
    assert_eq!(got.builtin, 8);
    assert_eq!(calls, 2);
}
#[test]
fn every_retained_prefix_and_raw_tail_survives_unchanged_builder_save() {
    let mut patterns = vec![
        vec![9; 255],
        vec![1, 2, 3],
        vec![0; 8],
        vec![0, 0, 0, 0, 0, 0, 0, 0, 0, 40],
    ];
    for header in [
        [0; 8],
        [255, 255, 255, 0, 0, 0, 19, 237],
        [1, 2, 3, 4, 5, 6, 7, 8],
    ] {
        for pairs in [1, 23, 123] {
            let mut bytes = header.to_vec();
            for index in 0..pairs {
                bytes.extend([if index % 2 == 0 { 255 } else { 1 }, PALETTES[index % 39]]);
            }
            patterns.push(bytes);
        }
    }
    for skin in 0..=65 {
        for bytes in &patterns {
            let record = AppearanceRecord {
                nickname: "Bytes".into(),
                builtin: skin,
                custom_base: skin,
                active_custom: true,
                custom_bytes: bytes.clone(),
                cosmetic: None,
            };
            let mut s = studio(record.clone());
            assert_eq!(s.preview_selection().unwrap(), record.selection());
            let got = saved(&mut s);
            assert_eq!(got, record);
            assert_eq!(
                AppearanceRecord::parse(&got.serialize().unwrap()).unwrap(),
                record
            );
        }
    }
    assert!(PatternRuns::from_wire(&vec![0; 256]).is_none());
    assert!(
        AppearanceSelection {
            skin: 7,
            custom_bytes: vec![0; 256]
        }
        .validate_wire()
        .is_err()
    );
}
