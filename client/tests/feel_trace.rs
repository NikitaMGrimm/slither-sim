#![cfg(feature = "feel-trace")]
//! Authored synthetic boundary identity checks; no sockets or GPU.
use slither_client::{feel_trace::Recorder, presentation::AnimatedClient};

#[test]
fn packet_boundary_does_not_invent_an_animation_update_or_draw() {
    let recorder = Recorder::default();
    let mut client = AnimatedClient::new(0.).unwrap();
    client.record_feel(recorder.clone());
    client
        .apply_frame(&[112], slither_client::protocol::ElapsedHeader::Absent)
        .unwrap();
    let rows = recorder.take();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].phase, "update");
    assert_eq!(rows[0].values[0], 0.);
    assert_eq!(client.draw_serial(), 0);
    assert_eq!(client.received().frames_applied, 1);
}

#[test]
fn staged_draw_observations_are_owned_copies_and_never_sample_by_drawing() {
    let recorder = Recorder::default();
    let mut committed = AnimatedClient::new(0.).unwrap();
    committed.record_feel(recorder.clone());
    let original = committed.clone();
    let mut staged = committed.clone();
    staged.advance(8., false).unwrap();
    staged.prepare_draw(1500, 845).unwrap();
    staged.advance(24., false).unwrap();
    staged.prepare_draw(1500, 845).unwrap();
    let rows = recorder.take();
    assert_eq!(committed, original);
    assert_eq!(
        rows.iter().map(|r| r.phase).collect::<Vec<_>>(),
        ["update", "draw", "update", "draw"]
    );
    assert_eq!(rows[0].values[0], 1.);
    assert_eq!(rows[1].values[0], 1.);
    assert_eq!(rows[2].values[0], 2.);
    assert!(recorder.take().is_empty());
}
