#![cfg(feature = "feel-trace")]
//! Authored synthetic staged-frame test; no window, socket or GPU.
use slither_client::feel_trace::Recorder;
use slither_wgpu_slice::game::LocalGame;

#[test]
fn issued_picture_carries_its_observation_before_commit_and_through_retry() {
    let mut game = LocalGame::new(0.).unwrap();
    let trace = Recorder::default();
    game.record_feel(trace.clone());
    game.play("ws://127.0.0.1:19090/slither", "Synthetic", 0)
        .unwrap();
    game.begin_frame(8., [1500, 845], 1.).unwrap();
    assert_eq!(game.client().draw_serial(), 0);
    let picture = game.take_frame().unwrap();
    let observation = game.issued_feel_observation().unwrap().clone();
    assert_eq!(observation.phase, "draw");
    assert_eq!(observation.values[0], 1.);
    // Draining the journal must not erase the copy owned by this picture.
    assert_eq!(trace.take().len(), 2);
    game.retry_frame(picture).unwrap();
    let _picture = game.take_frame().unwrap();
    assert_eq!(game.issued_feel_observation(), Some(&observation));
    assert_eq!(game.client().draw_serial(), 0);
    game.presented(true).unwrap();
    assert_eq!(game.client().draw_serial(), 1);
    assert!(trace.take().is_empty());
}
