//! Authored 512 x 4096 circular bodies; no assets, sockets, windows or GPU.
use slither_client::{presentation::*, protocol::ElapsedHeader, state::MAX_BODY_POINTS};

fn u24(n: u32) -> [u8; 3] {
    let b = n.to_be_bytes();
    [b[1], b[2], b[3]]
}

fn snake(id: u16) -> Vec<u8> {
    let angle = f64::from(id) * 2.399963229728653;
    let centre = [16000. + 7000. * angle.cos(), 16000. + 7000. * angle.sin()];
    let radius = MAX_BODY_POINTS as f64 * 6. / std::f64::consts::TAU;
    let position = |i: usize| {
        let a = i as f64 * std::f64::consts::TAU / MAX_BODY_POINTS as f64;
        [centre[0] + radius * a.cos(), centre[1] + radius * a.sin()].map(|x| (x * 2.).round() / 2.)
    };
    let mut p = vec![b's'];
    p.extend(id.to_be_bytes());
    p.extend(u24(0));
    p.push(b'0');
    p.extend(u24(0));
    p.extend(4250_u16.to_be_bytes());
    p.extend(u24(0));
    p.push((id % 9) as u8);
    for v in position(MAX_BODY_POINTS - 1) {
        p.extend(u24((v * 5.) as u32));
    }
    p.extend([0, 0, 255]);
    for v in position(0) {
        p.extend(u24((v * 5.) as u32));
    }
    for i in 1..MAX_BODY_POINTS {
        let a = position(i - 1);
        let b = position(i);
        p.extend([b[0] - a[0], b[1] - a[1]].map(|d| (127. + d * 2.) as u8));
    }
    p
}

#[test]
fn full_map_rival_curve_allocations_fit_the_aggregate_sample_budget() {
    let mut world = AnimatedClient::new(0.).unwrap();
    let mut setup = vec![b'a'];
    setup.extend(u24(16000));
    for v in [430_u16, 100, 20] {
        setup.extend(v.to_be_bytes());
    }
    setup.push(48);
    for v in [539_u16, 40, 1400, 33, 28, 430] {
        setup.extend(v.to_be_bytes());
    }
    setup.extend([19, 6, 0, 0]);
    setup.extend(u24(15000));
    world.apply_frame(&setup, ElapsedHeader::Absent).unwrap();
    for id in 1..=512 {
        world
            .apply_frame(&snake(id), ElapsedHeader::Absent)
            .unwrap();
    }
    assert!(world.detail_pressure());
    world.wheel_zoom(-1000.);
    for _ in 0..2 {
        world
            .prepare_draw_with_style(1920, 1080, 1e6, false)
            .unwrap();
    }
    for _ in 0..3 {
        let prepared = world
            .prepare_draw_with_style(1920, 1080, 1., false)
            .unwrap();
        let rivals: Vec<_> = prepared
            .iter()
            .filter(|s| !s.own)
            .filter_map(|s| s.curve.as_ref())
            .collect();
        assert_eq!(rivals.len(), 511);
        let active: usize = rivals.iter().map(|c| c.samples.len()).sum();
        let capacity: usize = rivals.iter().map(|c| c.samples.capacity()).sum();
        assert!(active <= PRESSURE_RIVAL_DRAW_SAMPLES);
        assert!(
            capacity <= PRESSURE_RIVAL_DRAW_SAMPLES,
            "active {active}, retained sample slots {capacity}"
        );
        let bytes: usize = rivals.iter().map(|c| c.storage_bytes()).sum();
        assert!(
            bytes
                <= PRESSURE_RIVAL_DRAW_SAMPLES
                    * (size_of::<CurveSample>() + size_of::<CurveVertex>()),
            "retained curve payload {bytes}"
        );
        assert_eq!(
            rivals.iter().map(|c| c.vertices().len()).sum::<usize>(),
            active
        );
        println!(
            "rivals {}, active {active}, sample capacity {capacity}, curve payload {bytes} bytes",
            rivals.len()
        );
    }
}
