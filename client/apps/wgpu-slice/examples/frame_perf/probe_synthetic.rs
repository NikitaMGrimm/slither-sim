//! Authored synthetic retained-food workloads, without a GPU, window or socket.
use super::tape;
use slither_wgpu_slice::{Result, game::LocalGame};

pub fn world(stress: bool) -> Result<LocalGame> {
    let mut game = tape::negotiating();
    game.receive(
        game.generation(),
        &tape::bundle(&[tape::setup(), tape::snake(44)]),
        3.,
    )?;
    tape::pictures(&mut game);
    let mut packets = Vec::new();
    for i in 0..if stress { 39_u16 } else { 7 } {
        let x = 900 + (i % 8) * 65;
        let y = 800 + (i / 8) * 60;
        let mut p = tape::snake(0);
        p[1..3].copy_from_slice(&(100 + i).to_be_bytes());
        // The helper's head/anchor coordinates are protocol 19 fifth-units.
        p[16..19].copy_from_slice(&tape::u24(u32::from(x) * 5));
        p[19..22].copy_from_slice(&tape::u24(u32::from(y) * 5));
        packets.push(p);
    }
    let (count, columns, spacing) = if stress {
        (30_000, 256, 8)
    } else {
        (324, 18, 114)
    };
    for i in 0..count {
        let x = 100 + (i % columns) * spacing;
        let y = 100 + (i / columns) * spacing;
        packets.push(vec![
            b'F',
            (x / 100) as u8,
            (y / 100) as u8,
            (i % 10) as u8,
            ((x % 100) * 255 / 100) as u8,
            ((y % 100) * 255 / 100) as u8,
            20,
        ]);
    }
    // Bounded ingress bundles; no transport is constructed.
    for chunk in packets.chunks(4000) {
        game.receive(game.generation(), &tape::bundle(chunk), 21.)?;
        tape::pictures(&mut game);
    }
    tape::pump(&mut game, 1000.);
    game.window_viewport([960, 540], 1.)?;
    assert_eq!(game.client().foods().foods().len(), count);
    assert_eq!(
        game.client().snake_objects().count(),
        if stress { 40 } else { 8 }
    );
    Ok(game)
}
