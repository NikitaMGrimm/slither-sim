#![cfg(feature = "feel-trace")]
//! Authored synthetic boundary identity checks; no sockets or GPU.
use slither_client::{feel_trace::Recorder, presentation::AnimatedClient};

#[test]
fn unsupported_face_records_an_error_without_publishing_a_partial_row() {
    use slither_client::{
        presentation::{
            ClientPresentationError, FaceGeometry, FacePresentation, MouthFaceGeometry,
        },
        protocol::ElapsedHeader,
    };
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
    let bytes: Vec<u8> = [setup, own]
        .into_iter()
        .flat_map(|p| {
            let mut v = (p.len() as u16).to_be_bytes().to_vec();
            v.extend(p);
            v
        })
        .collect();
    let trace = Recorder::default();
    let mut client = AnimatedClient::new(0.).unwrap();
    client.record_feel(trace.clone());
    client
        .apply_frame_with_face_presentation::<ClientPresentationError, _>(
            &bytes,
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
