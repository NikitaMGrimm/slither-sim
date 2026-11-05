//! Authored CPU picture controls; no GPU, window, socket or external artwork.
#![cfg(not(target_arch = "wasm32"))]
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::presentation::{QualityMode, QualityState};
use slither_wgpu_slice::{cosmetics::Cosmetics, render};

#[test]
fn auto_degradation_retains_legacy_shadows_while_explicit_low_removes_them() {
    let mut game = tape::playing();
    game.bank.cosmetics = Some(Cosmetics::from_images(Default::default(), None).unwrap());
    let extent = game.window_viewport([1920, 1080], 1.).unwrap();
    let mut degraded = game.quality();
    degraded.high_quality = false;
    degraded.quality_blend = 0.;
    degraded.quality_scale = 1.7;
    assert_eq!(degraded.requested, QualityMode::High);
    let shadow = game.bank.assets[game.bank.shadow.asset].handle;
    let mut world = game.client().clone();
    let auto =
        render::prepare_gameplay(&mut world, &mut game.bank, extent, 1., 100, degraded).unwrap();
    assert!(
        auto.frame
            .draws
            .iter()
            .any(|draw| draw.asset.handle == shadow)
    );
    game.bank.recycle_frame(auto);
    let mut explicit = QualityState::new(QualityMode::Low, 0.).unwrap();
    explicit.begin_connection();
    degraded = explicit.snapshot();
    let mut world = game.client().clone();
    let low =
        render::prepare_gameplay(&mut world, &mut game.bank, extent, 1., 100, degraded).unwrap();
    assert!(
        !low.frame
            .draws
            .iter()
            .any(|draw| draw.asset.handle == shadow)
    );
}

#[test]
fn pending_presets_keep_the_current_picture_until_connection_initialization() {
    let mut game = tape::playing();
    game.bank.cosmetics = Some(Cosmetics::from_images(Default::default(), None).unwrap());
    let extent = game.window_viewport([1920, 1080], 1.).unwrap();
    let full = game.quality();
    let mut world = game.client().clone();
    let picture =
        render::prepare_gameplay(&mut world, &mut game.bank, extent, 1., 100, full).unwrap();
    let expected = picture.frame.canonical().unwrap();
    game.bank.recycle_frame(picture);
    for requested in [QualityMode::Low, QualityMode::Medium] {
        let mut pending = full;
        pending.requested = requested;
        let mut world = game.client().clone();
        let picture =
            render::prepare_gameplay(&mut world, &mut game.bank, extent, 1., 100, pending).unwrap();
        assert_eq!(
            picture.frame.canonical().unwrap(),
            expected,
            "{requested:?} must wait for connection initialization"
        );
        game.bank.recycle_frame(picture);
    }
}
