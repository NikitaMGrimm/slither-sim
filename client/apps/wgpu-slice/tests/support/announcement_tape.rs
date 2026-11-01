//! Authored synthetic packet/session helpers. Every effect is an in-memory acknowledgement.
#![allow(dead_code)]
use slither_client::session_policy::Phase;
use slither_wgpu_slice::game::{EffectKind, LocalGame};
pub fn u24(n: u32) -> [u8; 3] {
    let b = n.to_be_bytes();
    [b[1], b[2], b[3]]
}
pub fn bundle(cs: &[Vec<u8>]) -> Vec<u8> {
    cs.iter()
        .flat_map(|p| {
            let mut b = (p.len() as u16).to_be_bytes().to_vec();
            b.extend(p);
            b
        })
        .collect()
}
pub fn setup() -> Vec<u8> {
    let mut p = vec![b'a'];
    p.extend(u24(1000));
    for v in [430u16, 100, 20] {
        p.extend(v.to_be_bytes());
    }
    p.push(48);
    for v in [539u16, 40, 1400, 33, 28, 430] {
        p.extend(v.to_be_bytes());
    }
    p.extend([19, 42]);
    p
}
pub fn snake(skin: u8) -> Vec<u8> {
    let mut p = vec![b's', 0, 7];
    p.extend(u24(0));
    p.push(b'0');
    p.extend(u24(0));
    p.extend(4250u16.to_be_bytes());
    p.extend(u24(0));
    p.push(skin);
    p.extend(u24(5500));
    p.extend(u24(5000));
    p.extend([0, 0, 255]);
    p.extend(u24(4780));
    p.extend(u24(5000));
    for _ in 2..12 {
        p.extend([151, 127]);
    }
    p.extend([0, 0]);
    p
}
pub fn food() -> Vec<u8> {
    vec![b'F', 41, 4, 76, 3, 232, 20]
}
pub fn prey() -> Vec<u8> {
    let mut p = vec![b'y', 0, 31, 9];
    p.extend(u24(5500));
    p.extend(u24(5000));
    p.extend([20, b'0']);
    p.extend(u24(0));
    p.extend(u24(0));
    p.extend(1000u16.to_be_bytes());
    p
}
pub fn board() -> Vec<u8> {
    let mut p = vec![b'l', 1, 0, 2, 0, 9, 0, 12];
    p.extend(u24(0));
    p.extend([41, 3, b'B', b'O', b'T']);
    p
}
pub fn challenge() -> Vec<u8> {
    // Original grammar-token generator, not a copied challenge.
    let first = format!("\"{}\"", "A".repeat(5));
    let second = format!("\"{}\"", "A".repeat(22));
    let tokens = [
        "var",
        "a",
        "=",
        &first,
        ";",
        "a",
        "+=",
        &second,
        ";",
        "for",
        "(",
        "var",
        "i",
        "=",
        "0",
        ";",
        "i",
        "<",
        "27",
        ";",
        "i",
        "++",
        ")",
        "{",
        "idba",
        "[",
        "i",
        "]",
        "=",
        "a",
        ".",
        "charCodeAt",
        "(",
        "i",
        ")",
        ";",
        "}",
    ];
    let mut p = vec![54];
    let mut key = 23;
    for b in tokens.join(" ").bytes() {
        for n in [b >> 4, b & 15] {
            p.push(b'a' + (n + key) % 26);
            key = (key + 17) % 26;
        }
    }
    p
}
pub fn effects(g: &mut LocalGame) -> Vec<Vec<u8>> {
    let mut packets = Vec::new();
    for _ in 0..16 {
        let Some(e) = g.effect() else { return packets };
        if let EffectKind::Send(ref b) = e.kind {
            packets.push(b.clone());
        }
        if matches!(&e.kind, EffectKind::Connect(_)) {
            g.construct(e.generation, e.id, 1.).unwrap();
        }
        g.acknowledge(e.generation, e.id, true).unwrap();
    }
    panic!("effect loop")
}
pub fn pictures(g: &mut LocalGame) {
    for _ in 0..16 {
        let Some(p) = g.take_frame() else { return };
        let r = p
            .resources
            .iter()
            .map(|r| (r.asset.into(), r.asset))
            .collect();
        p.frame.quads(&r).unwrap();
        g.presented(true).unwrap();
    }
    assert!(!g.has_frame(), "picture loop");
}
pub fn pump(g: &mut LocalGame, t: f64) -> Vec<Vec<u8>> {
    g.begin_frame(t, [960, 540], 1.).unwrap();
    let p = effects(g);
    pictures(g);
    p
}
pub fn negotiating() -> LocalGame {
    let mut g = LocalGame::new(0.).unwrap();
    g.draw_diagnostics.strict_validation = true;
    g.play("ws://127.0.0.1:19491/slither", "Player", 44)
        .unwrap();
    pump(&mut g, 1.);
    let e = g.effect().unwrap();
    assert!(matches!(e.kind, EffectKind::Connect(_)));
    g.construct(e.generation, e.id, 1.).unwrap();
    g.acknowledge(e.generation, e.id, true).unwrap();
    g.opened(g.generation(), 2.).unwrap();
    assert_eq!(g.phase(), Phase::Opening);
    assert_eq!(effects(&mut g), [vec![1], vec![99, 0]]);
    assert_eq!(g.phase(), Phase::Challenge);
    g.receive(g.generation(), &bundle(&[challenge()]), 2.)
        .unwrap();
    let packets = effects(&mut g);
    assert_eq!(packets[0], b"DHLPTXBFJNRVZDHLPTXBFJNRVZD");
    assert_eq!(packets[1][24], 44);
    assert_eq!(&packets[1][26..32], b"Player");
    assert_eq!(&packets[1][32..], [0, 255]);
    assert_eq!(g.phase(), Phase::Setup);
    g
}
pub fn playing() -> LocalGame {
    let mut g = negotiating();
    g.receive(
        g.generation(),
        &bundle(&[setup(), snake(44), food(), prey(), board()]),
        3.,
    )
    .unwrap();
    pictures(&mut g);
    pump(&mut g, 20.);
    g
}
