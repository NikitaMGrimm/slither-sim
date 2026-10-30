#![cfg(feature = "feel-trace")]
//! Authored synthetic boundary identity checks; no sockets or GPU.
use slither_client::{feel_trace::Recorder, presentation::AnimatedClient};

fn authored_own() -> Vec<u8> {
    // Authored synthetic Setup and own appearance, same grammar as the browser fixture.
    let u24 = |n: u32| [(n >> 16) as u8, (n >> 8) as u8, n as u8];
    let mut setup = vec![97];
    setup.extend(u24(1000));
    for n in [430_u16, 100, 20] {
        setup.extend(n.to_be_bytes());
    }
    setup.push(48);
    for n in [539_u16, 40, 1400, 33, 28, 430] {
        setup.extend(n.to_be_bytes());
    }
    setup.extend([19, 42]);
    let mut own = vec![115, 0, 7];
    own.extend(u24(0));
    own.push(48);
    own.extend(u24(0));
    own.extend(4250_u16.to_be_bytes());
    own.extend(u24(0));
    own.push(0);
    own.extend(u24(5500));
    own.extend(u24(5000));
    own.extend([0, 0, 255]);
    own.extend(u24(4780));
    own.extend(u24(5000));
    own.extend([151, 127, 0, 0]);
    [setup, own]
        .into_iter()
        .flat_map(|p| {
            let mut v = (p.len() as u16).to_be_bytes().to_vec();
            v.extend(p);
            v
        })
        .collect()
}

#[test]
fn unsupported_face_records_an_error_without_publishing_a_partial_row() {
    use slither_client::{
        presentation::{
            ClientPresentationError, FaceGeometry, FacePresentation, MouthFaceGeometry,
        },
        protocol::ElapsedHeader,
    };
    let trace = Recorder::default();
    let mut client = AnimatedClient::new(0.).unwrap();
    client.record_feel(trace.clone());
    client
        .apply_frame_with_face_presentation::<ClientPresentationError, _>(
            &authored_own(),
            ElapsedHeader::Absent,
            |_| {
                Ok(FacePresentation {
                    face: FaceGeometry::Mouth(MouthFaceGeometry { pupil_travel: 4. }),
                    antenna: None,
                })
            },
        )
        .unwrap();
    assert!(trace.take().is_empty());
    assert_eq!(trace.take_errors(), ["update: unsupported face layout"]);
    assert!(trace.has_failed());
    assert!(trace.take_errors().is_empty());
    assert!(trace.has_failed());
    assert_eq!(client.received().frames_applied, 1);
    assert_eq!(client.draw_serial(), 0);
}

#[test]
fn death_through_draw_expiry_and_restart_keep_complete_observations() {
    use slither_client::protocol::ElapsedHeader;
    let recorder = Recorder::default();
    let mut client = AnimatedClient::new(0.).unwrap();
    client.record_feel(recorder.clone());
    client
        .apply_frame(&authored_own(), ElapsedHeader::Absent)
        .unwrap();
    let first = client.own_token().unwrap();
    client
        .apply_frame(&[115, 0, 7, 1], ElapsedHeader::Absent)
        .unwrap();
    for frame in 1..=20 {
        client.advance(f64::from(frame) * 40., false).unwrap();
        client.prepare_draw(1500, 845).unwrap();
    }
    let own = client.own_snake().unwrap().metrics();
    assert!(own.death_progress >= 1. && own.expired);
    assert!(client.drawn_own_snake().is_none());
    assert!(
        recorder.take_errors().is_empty(),
        "expiry must follow the production draw skip"
    );
    assert!(!recorder.has_failed());
    let rows = recorder.take();
    assert_eq!(rows.len(), 42);
    let expired: Vec<_> = rows.iter().filter(|r| r.values[137] == 1.).collect();
    assert!(!expired.is_empty());
    // Camera prefix is 13 + 124 cells. Expired own retains all raw channels
    // but omits the six head/pupil projection cells skipped by production draw.
    assert!(
        expired
            .iter()
            .all(|r| r.values[138] >= 1. && r.values.len() == 405)
    );
    client.reset();
    client
        .apply_frame(&authored_own(), ElapsedHeader::Absent)
        .unwrap();
    client.advance(840., false).unwrap();
    client.prepare_draw(1500, 845).unwrap();
    assert_ne!(client.own_token().unwrap(), first);
    assert!(!client.own_snake().unwrap().metrics().expired);
    assert!(!recorder.has_failed());
    let restarted = recorder.take();
    assert_eq!(restarted.len(), 3);
    assert!(
        restarted
            .iter()
            .all(|r| r.values[137] == 0. && r.values.len() == 411)
    );
}

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
