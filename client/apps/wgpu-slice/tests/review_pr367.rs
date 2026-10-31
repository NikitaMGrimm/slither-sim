//! Reviewer-authored opaque-prefix repro, ported to assert retained-wire Play succeeds.
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/announcement_tape.rs"]
mod support;
use slither_client::skin_studio::*;
use slither_wgpu_slice::game::{LocalGame, LoginProfile};

fn sent(selection: &AppearanceSelection) -> Vec<u8> {
    let mut game = LocalGame::new(0.).unwrap();
    game.play_with_appearance(
        "ws://127.0.0.1:19491/slither",
        "Bytes",
        selection,
        &LoginProfile {
            client_version: 291,
            page_echo: [7; 20],
        },
    )
    .unwrap();
    support::pump(&mut game, 1.);
    support::effects(&mut game);
    game.opened(game.generation(), 2.).unwrap();
    support::effects(&mut game);
    game.receive(
        game.generation(),
        &support::bundle(&[support::challenge()]),
        3.,
    )
    .unwrap();
    support::effects(&mut game)[1].clone()
}

#[test]
fn opaque_prefix_repro() {
    let selection = AppearanceSelection {
        skin: 7,
        custom_bytes: vec![1, 2, 3, 4, 5, 6, 7, 8, 2, 1],
    };
    let encoded = slither_client::protocol::encode_login(
        291,
        &[7; 20],
        "Bytes",
        7,
        Some(&selection.custom_bytes),
    )
    .unwrap();
    assert_eq!(&encoded[33..], selection.custom_bytes);
    assert_eq!(sent(&selection), encoded);
    assert!(selection.validate_authored().is_err());
    let saved = "2:4279746573:c:07:07:01020304050607080201:--";
    let restored = RestoredAppearance::restore(
        PreferenceInput::Text(saved.into()),
        PreferenceInput::Missing,
    );
    assert_eq!(restored.record.selection(), selection);
    assert_eq!(restored.record.serialize().unwrap(), saved);
    let mut studio = StudioState::new(restored.record).unwrap();
    studio.open(0., true);
    studio.step(230.);
    let committed = studio
        .save("Bytes", 230., &mut || {
            panic!("retained Save sampled entropy")
        })
        .unwrap()
        .unwrap();
    assert_eq!(committed.selection(), selection);
    assert_eq!(sent(&committed.selection()), encoded);
}

#[test]
fn retained_maximum_raw_payload_is_sent_without_pattern_reinterpretation() {
    let selection = AppearanceSelection {
        skin: 65,
        custom_bytes: vec![9; 255],
    };
    assert_eq!(
        sent(&selection),
        slither_client::protocol::encode_login(
            291,
            &[7; 20],
            "Bytes",
            65,
            Some(&selection.custom_bytes)
        )
        .unwrap()
    );
}
