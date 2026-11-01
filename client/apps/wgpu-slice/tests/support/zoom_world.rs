//! Authored dense synthetic world, shared by budget tests and offscreen zoom perf.
//! No socket is created; all transport effects are acknowledged in memory.
#![allow(dead_code)]
use super::tape;
use slither_wgpu_slice::{Result, game::LocalGame};

pub const EXTENT: [u32; 2] = [960, 540];
pub const FOOD_COUNT: u16 = 65_535;

pub fn world() -> Result<LocalGame> {
    let mut g = tape::negotiating();
    let mut setup = tape::setup();
    setup[1..4].copy_from_slice(&tape::u24(12000));
    // Explicit radius after the protocol/skin-count setup fields.
    setup.extend([0, 0]);
    setup.extend(tape::u24(11500));
    let own = snake(7, 12000, 12000, 12);
    let mut packets = vec![setup, own, tape::prey(), tape::board()];
    for i in 0..64_u16 {
        let (x, y) = if i < 16 {
            (11700 + (i % 4) * 180, 11700 + (i / 4) * 180)
        } else {
            let angle = f64::from(i - 16) * std::f64::consts::TAU / 48.;
            (
                (12000. + 9000. * angle.cos()) as u16,
                (12000. + 9000. * angle.sin()) as u16,
            )
        };
        packets.push(snake(100 + i, x, y, 120));
    }
    for i in 0..FOOD_COUNT {
        let (x, y) = if i < 4000 {
            (11600 + (i % 80) * 10, 11600 + (i / 80) * 16)
        } else {
            (
                1000 + ((u32::from(i) * 73) % 22000) as u16,
                1000 + ((u32::from(i) * 137) % 22000) as u16,
            )
        };
        packets.push(vec![
            b'F',
            (x / 100) as u8,
            (y / 100) as u8,
            (i % 9) as u8,
            ((x % 100) * 255 / 100) as u8,
            ((y % 100) * 255 / 100) as u8,
            20,
        ]);
    }
    // Respect the production decoder's 4,096-command message limit.
    for chunk in packets.chunks(4000) {
        g.receive(g.generation(), &tape::bundle(chunk), 3.)?;
        tape::pictures(&mut g);
    }
    // Settle spawn and camera correction using production updates, not injected art.
    for i in 1..=60 {
        tape::pump(&mut g, 3. + f64::from(i) * 16.);
    }
    assert_eq!(g.client().foods().foods().len(), usize::from(FOOD_COUNT));
    Ok(g)
}

fn snake(id: u16, x: u16, y: u16, points: usize) -> Vec<u8> {
    let mut p = tape::snake((id % 9) as u8);
    p[1..3].copy_from_slice(&id.to_be_bytes());
    p[16..19].copy_from_slice(&tape::u24(u32::from(x) * 5));
    p[19..22].copy_from_slice(&tape::u24(u32::from(y) * 5));
    p[25..28].copy_from_slice(&tape::u24(u32::from(x) * 5 - 1500));
    p[28..31].copy_from_slice(&tape::u24(u32::from(y) * 5));
    p.truncate(31);
    for _ in 2..points {
        p.extend([132, 127]);
    }
    p.extend([0, 0]);
    p
}
