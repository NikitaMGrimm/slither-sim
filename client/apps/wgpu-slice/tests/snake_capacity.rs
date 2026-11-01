//! Rule-based capacity checks on authored packets; no GPU, window or socket.
#[path = "support/capacity_world.rs"]
mod synthetic;
#[path = "support/announcement_tape.rs"]
mod tape;
use slither_client::{presentation::*, protocol::ElapsedHeader, session_policy::Phase, state::*};
use slither_wgpu_slice::{frame::MAX_DRAWS, game::EffectKind, render};

#[test]
fn session_admits_512_and_rejects_513_atomically_then_closes_cleanly() {
    let mut game = synthetic::world(MAX_SNAKES, 26).unwrap();
    assert_eq!(MAX_SNAKES, 512);
    assert_eq!(game.phase(), Phase::Playing);
    assert!(game.failure.is_none());
    assert_eq!(game.draw_diagnostics.skipped_frames, 0);
    assert_eq!(game.client().received().snakes.len(), MAX_SNAKES);
    assert!(game.client().received().snakes.contains_key(&257));
    assert!(game.client().received().snakes.contains_key(&501));
    for _ in 0..64 {
        let Some(effect) = game.effect() else { break };
        assert!(!matches!(effect.kind, EffectKind::Close));
        game.acknowledge(effect.generation, effect.id, true)
            .unwrap();
    }
    let before = game.client().clone();
    let error = game
        .receive(
            game.generation(),
            &tape::bundle(&[synthetic::snake(513, 26)]),
            4.,
        )
        .unwrap_err();
    assert_eq!(error, StateError::Limit.to_string());
    assert_eq!(game.client(), &before);
    // Match the native and WebGPU checked-error adapters, not just ClientState.
    game.fail(&error);
    assert_eq!(game.phase(), Phase::Failed);
    let mut closed = false;
    while let Some(effect) = game.effect() {
        closed |= matches!(effect.kind, EffectKind::Close);
        game.acknowledge(effect.generation, effect.id, true)
            .unwrap();
    }
    assert!(closed);
}

#[test]
fn maximum_length_full_map_retains_received_anchors_and_own_detail_in_bounded_pictures() {
    let mut game = synthetic::world(MAX_SNAKES, MAX_BODY_POINTS).unwrap();
    let mut world = game.client().clone();
    assert_eq!(MAX_TOTAL_BODY_POINTS, MAX_SNAKES * MAX_BODY_POINTS);
    assert_eq!(
        world
            .received()
            .snakes
            .values()
            .map(|s| s.body.len())
            .sum::<usize>(),
        MAX_TOTAL_BODY_POINTS
    );
    assert_eq!(
        world.own_snake().unwrap().body().points().len(),
        MAX_BODY_POINTS
    );
    assert!(!world.own_snake().unwrap().body().detail_reduced());
    assert!(world.detail_pressure());
    assert!(world.retained_body_points() <= PRESSURE_BODY_POINTS);
    assert!(world.body_storage_bytes() <= PRESSURE_BODY_POINTS * size_of::<BodyPoint>());
    let own = world.own_snake().unwrap().body().clone();
    world.wheel_zoom(-1000.);
    let quality = game.quality();
    for _ in 0..110 {
        world
            .prepare_draw_with_style(synthetic::EXTENT[0], synthetic::EXTENT[1], 1e6, false)
            .unwrap();
    }
    for identity in 0..3 {
        let p = render::prepare_gameplay(
            &mut world,
            &mut game.bank,
            synthetic::EXTENT,
            1.,
            identity,
            quality,
        )
        .unwrap();
        assert!(p.draw_count() <= MAX_DRAWS);
        assert!(p.pixel_bytes() <= slither_wgpu_slice::bank::PIXEL_BUDGET);
        if identity == 2 {
            assert_eq!(p.frame.snakes.len(), MAX_SNAKES);
            assert!(p.frame.snakes.iter().all(|s| s.body_commands > 0));
        }
        game.bank.recycle_frame(p);
    }
    assert_eq!(world.own_snake().unwrap().body(), &own);
    assert_eq!(world.received(), game.client().received());
    assert_eq!(game.draw_diagnostics.skipped_frames, 0);
}

#[test]
fn maximum_length_burst_releases_prepared_work_without_losing_live_state() {
    let mut game = synthetic::world(1, MAX_BODY_POINTS).unwrap();
    // Score-table coverage is independent of received-anchor capacity. Keep the
    // maximum own appearance inside its advertised HUD domain for this session.
    let mut setup = tape::setup();
    setup[1..4].copy_from_slice(&tape::u24(16000));
    setup[4..6].copy_from_slice(&(MAX_BODY_POINTS as u16).to_be_bytes());
    setup[24] = 6;
    setup.extend([0, 0]);
    setup.extend(tape::u24(15000));
    game.receive(game.generation(), &tape::bundle(&[setup]), 4.)
        .unwrap();
    tape::pictures(&mut game);
    game.begin_frame(10., synthetic::EXTENT, 1.).unwrap();
    tape::pictures(&mut game);
    game.service_ingress().unwrap();
    game.begin_frame(12., synthetic::EXTENT, 1.).unwrap();
    assert!(game.has_frame());
    // Leave the prepared picture unissued while more than one deferred-queue
    // capacity of maximum-length bare appearances arrives in the same wake.
    for id in 2..=MAX_SNAKES as u16 {
        game.receive(
            game.generation(),
            &synthetic::snake(id, MAX_BODY_POINTS),
            13.,
        )
        .unwrap();
    }
    tape::pictures(&mut game);
    game.service_ingress().unwrap();
    assert_eq!(game.client().received().snakes.len(), MAX_SNAKES);
    assert_eq!(
        game.client()
            .received()
            .snakes
            .values()
            .map(|s| s.body.len())
            .sum::<usize>(),
        MAX_TOTAL_BODY_POINTS
    );
    assert!(game.failure.is_none());
    assert_eq!(game.draw_diagnostics.skipped_frames, 0);
    assert!(game.wheel_zoom(-1000.));
    for frame in 0..110 {
        game.begin_frame(20. + f64::from(frame) * 16., synthetic::EXTENT, 1.)
            .unwrap();
        tape::effects(&mut game);
        let picture = game.take_frame().expect("complete session picture");
        assert!(picture.draw_count() <= MAX_DRAWS);
        if frame == 109 {
            assert_eq!(picture.frame.snakes.len(), MAX_SNAKES);
            assert!(picture.frame.snakes.iter().all(|s| s.body_commands > 0));
        }
        game.presented(true).unwrap();
    }
    assert!(!game.client().own_snake().unwrap().body().detail_reduced());
    assert_eq!(game.draw_diagnostics.skipped_frames, 0);
}

#[test]
fn pressure_quota_relationships_fit_the_fixed_draw_and_device_envelopes() {
    const {
        assert!(
            MAX_RETAINED_SNAKES * 8 + MAX_RETAINED_BODY_POINTS <= MAX_RETAINED_TOTAL_BODY_POINTS
        );
        assert!(MAX_RETAINED_SNAKES * 8 <= PRESSURE_RIVAL_DRAW_SAMPLES);
        assert!(MAX_CURVE_SAMPLES + PRESSURE_RIVAL_DRAW_SAMPLES <= MAX_DRAW_SAMPLES);
    }
    // A fresh maximum-length stock own appearance starts with spacing that
    // emits at most two detailed samples per segment plus head/terminal samples. Rivals share
    // their numeric quota. Reserve complete faces, bulbs, edge, text and HUD.
    let body =
        8 * (2 * MAX_BODY_POINTS + 2) + 2 * PRESSURE_RIVAL_DRAW_SAMPLES + 16 * MAX_RETAINED_SNAKES;
    let other = PRESSURE_FOOD_DRAWS + PRESSURE_PREY_DRAWS + 10 * MAX_RETAINED_SNAKES + 16_384;
    assert!(body + other <= MAX_DRAWS);
    assert_eq!(
        slither_wgpu_slice::frame::DRAW_RECORD_BUDGET,
        64 * 1024 * 1024
    );
    assert_eq!(
        slither_wgpu_slice::frame::VERTEX_CHUNK_QUADS
            * slither_wgpu_slice::frame::QUAD_VERTEX_BYTES,
        983_040
    );
    // A complete duplicate replacement remains legal at the live boundary.
    let mut state = ClientState::default();
    state
        .apply_frame(&tape::bundle(&[tape::setup()]), ElapsedHeader::Absent)
        .unwrap();
    for id in 1..=MAX_SNAKES as u16 {
        state
            .apply_frame(
                &tape::bundle(&[synthetic::snake(id, 2)]),
                ElapsedHeader::Absent,
            )
            .unwrap();
    }
    state
        .apply_frame(&synthetic::snake(1, MAX_BODY_POINTS), ElapsedHeader::Absent)
        .unwrap();
    assert_eq!(state.snakes.len(), MAX_SNAKES);
}
