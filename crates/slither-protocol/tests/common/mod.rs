#![allow(dead_code)]
//! Authored synthetic commands; no encoders from either consumer.
pub fn u24(value: u32) -> [u8; 3] {
    let b = value.to_be_bytes();
    [b[1], b[2], b[3]]
}
pub fn setup() -> Vec<u8> {
    let mut p = vec![b'a'];
    p.extend(u24(1000));
    for n in [430_u16, 100, 20] {
        p.extend(n.to_be_bytes());
    }
    p.push(48);
    for n in [539_u16, 40, 1400, 33, 28, 430] {
        p.extend(n.to_be_bytes());
    }
    p.extend([19, 10]);
    p
}
pub fn appearance() -> Vec<u8> {
    let mut p = vec![b's', 0, 17];
    p.extend(u24(1));
    p.push(b'2');
    p.extend(u24(16_777_215));
    p.extend(321_u16.to_be_bytes());
    p.extend(u24(8_388_607));
    p.push(55);
    p.extend(u24(5001));
    p.extend(u24(5002));
    p.extend([2, 0, 255, 2, 123, 234, 7]); // name/custom/cosmetic
    p.extend(u24(4999));
    p.extend(u24(5000));
    p.extend([0, 255, 0x12, 0x34]);
    p
}
pub fn encode_source(source: &[u8]) -> Vec<u8> {
    let mut key = 23;
    let mut out = Vec::new();
    for &b in source {
        for nibble in [b >> 4, b & 15] {
            out.push(b'a' + (nibble + key) % 26);
            key = (key + 17) % 26;
        }
    }
    out
}
