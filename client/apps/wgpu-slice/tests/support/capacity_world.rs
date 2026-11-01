//! Authored circular bodies. No sockets, external artwork or simulation claims.
#![allow(dead_code)]
use super::tape;
use slither_wgpu_slice::{Result, game::LocalGame};

pub const EXTENT: [u32; 2] = [1920, 1080];

fn drain(game: &mut LocalGame) -> Result<()> {
    for _ in 0..16 {
        tape::pictures(game);
        let mut effects = false;
        while let Some(effect) = game.effect() {
            if matches!(effect.kind, slither_wgpu_slice::game::EffectKind::Close) {
                return Err("capacity fixture unexpectedly queued Close".into());
            }
            game.acknowledge(effect.generation, effect.id, true)?;
            effects = true;
        }
        game.service_ingress()?;
        if !effects && !game.has_frame() {
            return Ok(());
        }
    }
    Err("capacity fixture drain limit".into())
}

pub fn snake(id: u16, points: usize) -> Vec<u8> {
    let mut p = tape::snake((id % 9) as u8);
    p[1..3].copy_from_slice(&id.to_be_bytes());
    let angle = f64::from(id) * 2.399963229728653;
    let centre = [16000. + 7000. * angle.cos(), 16000. + 7000. * angle.sin()];
    let radius = points as f64 * 6. / std::f64::consts::TAU;
    let position = |i: usize| {
        let a = i as f64 * std::f64::consts::TAU / points as f64;
        [
            (centre[0] + radius * a.cos()) * 2.,
            (centre[1] + radius * a.sin()) * 2.,
        ]
        .map(|x| x.round() / 2.)
    };
    let head = position(points - 1);
    p[16..19].copy_from_slice(&tape::u24((head[0] * 5.) as u32));
    p[19..22].copy_from_slice(&tape::u24((head[1] * 5.) as u32));
    p.truncate(25);
    let first = position(0);
    p.extend(tape::u24((first[0] * 5.) as u32));
    p.extend(tape::u24((first[1] * 5.) as u32));
    for i in 1..points {
        let previous = position(i - 1);
        let current = position(i);
        let delta = [current[0] - previous[0], current[1] - previous[1]];
        p.extend(delta.map(|d| (127. + d * 2.) as u8));
    }
    p
}

pub fn world(population: usize, points: usize) -> Result<LocalGame> {
    let mut game = tape::negotiating();
    let mut setup = tape::setup();
    setup[1..4].copy_from_slice(&tape::u24(16000));
    setup[24] = 6;
    setup.extend([0, 0]);
    setup.extend(tape::u24(15000));
    game.receive(game.generation(), &tape::bundle(&[setup]), 3.)?;
    drain(&mut game)?;
    let packets: Vec<_> = (1..=population as u16)
        .map(|id| snake(id, points))
        .collect();
    if packets[0].len() >= 8192 {
        // A bundled command has a 13-bit length prefix. Maximum-length bodies
        // use bare appearance frames, retaining the existing wire contract.
        for packet in &packets {
            game.receive(game.generation(), packet, 3.)?;
            drain(&mut game)?;
        }
    } else {
        for chunk in packets.chunks(64) {
            game.receive(game.generation(), &tape::bundle(chunk), 3.)?;
            drain(&mut game)?;
        }
    }
    let mut board = tape::board();
    board[4..6].copy_from_slice(&(population as u16).to_be_bytes());
    game.receive(game.generation(), &tape::bundle(&[board]), 3.)?;
    drain(&mut game)?;
    assert_eq!(game.client().received().snakes.len(), population);
    Ok(game)
}
