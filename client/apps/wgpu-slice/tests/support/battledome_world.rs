//! Authored Battledome-shaped fixture, not a capture or a server-mechanics oracle.
#![allow(dead_code)]
use super::tape;
use slither_wgpu_slice::{Result, game::LocalGame};

pub fn world(bots: u16) -> Result<LocalGame> {
    make_world(bots, false, false)
}

pub fn far_world(bots: u16) -> Result<LocalGame> {
    make_world(bots, bots == 500, true)
}

fn make_world(bots: u16, normal: bool, decorated: bool) -> Result<LocalGame> {
    let mut g = tape::negotiating();
    if decorated {
        g.bank.cosmetics = Some(authored_cosmetics()?);
    }
    let radius = if normal {
        670. * f64::from(bots + 1).sqrt()
    } else {
        3584. * (f64::from(bots + 1) / 32.).sqrt()
    };
    let sector: u16 = if normal { 200 } else { 100 };
    let center = 16000_u16;
    let mut setup = tape::setup();
    setup[1..4].copy_from_slice(&tape::u24(u32::from(center)));
    setup[6..8].copy_from_slice(&sector.to_be_bytes());
    setup.extend([0, 0]);
    setup.extend(tape::u24(radius.ceil() as u32));
    // Off-centre own snake, as on the host's spawn band. The whole-map zoom
    // endpoint must account for that offset rather than centring the arena.
    let mut packets = vec![
        setup,
        snake(7, center, radius * 0.74, 0., 0),
        tape::board(),
        vec![b'M', 0, 8, 64],
    ];
    // The state owner bounds live snakes. Larger hosts send
    // subscriptions rather than their whole world; stress that largest valid
    // live set without increasing the unrelated admission contract.
    let retained_bots = bots.min(slither_client::state::MAX_SNAKES as u16 - 1);
    for i in 0..retained_bots {
        let angle = f64::from(i) * 2.399963229728653;
        let radial =
            radius * (0.08 + 0.86 * ((f64::from(i) + 0.5) / f64::from(retained_bots)).sqrt());
        packets.push(snake(
            100 + i,
            center,
            radial,
            angle,
            if decorated && i.is_multiple_of(5) {
                24 + (i % 2) as u8
            } else {
                (i % 9) as u8
            },
        ));
    }
    // Constant-density pit food: ~777 pellets at 32 seats. All are retained,
    // including those culled at stock zoom. No private or external artwork.
    let target = if normal {
        (3. * 7. * std::f64::consts::PI * radius * radius / 90_000.).floor() as u32
    } else {
        (777. * f64::from(bots + 1) / 32.).round() as u32
    };
    // Keep the production ingress cap and represent a subscribed central disk at
    // the requested 3x density rather than changing the client food contract.
    let food_count = target.min(65_535);
    let food_radius = if normal {
        radius * (f64::from(food_count) / f64::from(target)).sqrt()
    } else {
        radius * 0.49
    };
    for i in 0..food_count {
        let angle = f64::from(i) * 2.399963229728653;
        let radial = food_radius * ((f64::from(i) + 0.5) / f64::from(food_count)).sqrt();
        let x = (f64::from(center) + radial * angle.cos()) as u16;
        let y = (f64::from(center) + radial * angle.sin()) as u16;
        packets.push(vec![
            b'F',
            (x / sector) as u8,
            (y / sector) as u8,
            (i % 9) as u8,
            ((x % sector) * 255 / sector) as u8,
            ((y % sector) * 255 / sector) as u8,
            20,
        ]);
    }
    for chunk in packets.chunks(4000) {
        g.receive(g.generation(), &tape::bundle(chunk), 3.)?;
        tape::pictures(&mut g);
    }
    for i in 1..=60 {
        tape::pump(&mut g, 3. + f64::from(i) * 16.);
    }
    assert_eq!(
        g.client().snake_objects().count(),
        usize::from(retained_bots) + 1
    );
    assert_eq!(g.client().foods().foods().len(), food_count as usize);
    assert_eq!(
        g.client().score_view().status,
        slither_client::presentation::ScoreStatus::Ready(400.)
    );
    Ok(g)
}

fn snake(id: u16, center: u16, radius: f64, angle: f64, skin: u8) -> Vec<u8> {
    let mut p = tape::snake(skin);
    p[1..3].copy_from_slice(&id.to_be_bytes());
    // Half cruise, half boosting; pit/circular body geometry is authored.
    p[10..12].copy_from_slice(
        &(if id.is_multiple_of(2) {
            12000_u16
        } else {
            4250_u16
        })
        .to_be_bytes(),
    );
    // Safely inside the wire-fam interval displaying score 400 at 26 anchors.
    p[12..15].copy_from_slice(&tape::u24(2_666_000));
    let heading = ((angle + std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU)
        / std::f64::consts::TAU
        * 16_777_215.) as u32;
    p[3..6].copy_from_slice(&tape::u24(heading));
    p[7..10].copy_from_slice(&tape::u24(heading));
    let at = |i: usize| {
        let a = angle - i as f64 * 42. / radius.max(500.);
        [
            f64::from(center) + radius * a.cos(),
            f64::from(center) + radius * a.sin(),
        ]
    };
    let head = at(0);
    p[16..19].copy_from_slice(&tape::u24((head[0] * 5.) as u32));
    p[19..22].copy_from_slice(&tape::u24((head[1] * 5.) as u32));
    p.truncate(22);
    let name = format!("Bot {id}");
    p.push(name.len() as u8);
    p.extend(name.bytes());
    p.extend([0, 255]);
    // Wire body is tail-to-head, with half-unit signed deltas.
    let tail = at(26);
    p.extend(tape::u24((tail[0] * 5.) as u32));
    p.extend(tape::u24((tail[1] * 5.) as u32));
    let mut previous = tail;
    for i in (1..26).rev() {
        let point = at(i);
        p.extend(
            std::array::from_fn::<_, 2, _>(|axis| {
                ((point[axis] - previous[axis]) * 2.).round() as i16 + 127
            })
            .map(|v| v as u8),
        );
        previous = point;
    }
    p
}

/// Only the built-in procedural face/bulb rasters; no image files are opened.
pub fn authored_cosmetics() -> Result<slither_wgpu_slice::cosmetics::Cosmetics> {
    slither_wgpu_slice::cosmetics::Cosmetics::from_images(Default::default(), None)
}
