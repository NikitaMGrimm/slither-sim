//! Authored synthetic protocol-13 packets; no recorded server data.
#![allow(dead_code)]

pub fn u24(n: u32) -> [u8; 3] {
    let b = n.to_be_bytes();
    [b[1], b[2], b[3]]
}
pub fn bundle(commands: &[Vec<u8>]) -> Vec<u8> {
    commands
        .iter()
        .flat_map(|c| {
            let mut p = (c.len() as u16).to_be_bytes().to_vec();
            p.extend(c);
            p
        })
        .collect()
}
pub fn setup() -> Vec<u8> {
    let mut p = vec![b'a'];
    p.extend(u24(1000));
    for n in [430u16, 300, 100] {
        p.extend(n.to_be_bytes());
    }
    p.push(48);
    for n in [539u16, 40, 1400, 33, 28, 430] {
        p.extend(n.to_be_bytes());
    }
    p.extend([13, 42]);
    p
}
pub fn snake(id: u16) -> Vec<u8> {
    let mut p = vec![b's'];
    p.extend(id.to_be_bytes());
    p.extend(u24(0x400000));
    p.push(b'2');
    p.extend(u24(0x800000));
    p.extend(4250u16.to_be_bytes());
    p.extend(u24(0x123456));
    p.push(3);
    p.extend(u24(5500));
    p.extend(u24(5000));
    p.extend([3, b'S', b'Y', b'N', 2, 9, 8, 0]);
    p.extend(u24(4501));
    p.extend(u24(5002));
    p.extend([211, 127, 129, 125]);
    p
}
pub fn anchor(code: u8, id: u16, xy: &[u8], fam: Option<u32>) -> Vec<u8> {
    let mut p = vec![code];
    p.extend(id.to_be_bytes());
    p.extend(xy);
    if let Some(fam) = fam {
        p.extend(u24(fam));
    }
    p
}
pub fn food(code: u8, color: u8, x: u16, y: u16, size: u8) -> Vec<u8> {
    let mut p = vec![code, color];
    p.extend(x.to_be_bytes());
    p.extend(y.to_be_bytes());
    p.push(size);
    p
}
pub fn eaten(x: u16, y: u16, eater: Option<u16>) -> Vec<u8> {
    let mut p = vec![b'c'];
    p.extend(x.to_be_bytes());
    p.extend(y.to_be_bytes());
    if let Some(id) = eater {
        p.extend(id.to_be_bytes());
    }
    p
}
pub fn board() -> Vec<u8> {
    vec![b'l', 1, 0, 2, 0, 9, 0, 12, 0, 0, 0, 41, 3, b'B', b'O', b'T']
}
pub fn challenge() -> Vec<u8> {
    let source = format!(
        "var a='{}';for(var i=0;i<24;i++)idba[i]=a.charCodeAt(i);",
        "A".repeat(24)
    );
    let mut p = vec![b'6'];
    let mut key = 23;
    for b in source.bytes() {
        for n in [b >> 4, b & 15] {
            p.push(b'a' + (n + key) % 26);
            key = (key + 17) % 26;
        }
    }
    p
}
