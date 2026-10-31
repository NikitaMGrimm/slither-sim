"""Run current client functions against shared scalars and print f64-bit goldens.

Offline, Linux only for A0. The temporary package is outside the workspace and
does not edit client sources. No production dependency on either consumer is
introduced. Keep generated expected bits fixed in the protocol package tests.
"""
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = r'''
use slither_client::{protocol::ElapsedHeader, state::{ClientState, PresentationEvent}};
use slither_protocol::{scalar as s, v19::{self, Message}, DecodeAdmission};
#[path = "COMMON"] mod common;
fn apply(state: &mut ClientState, p: &[u8]) -> Vec<PresentationEvent> {
    state.apply_frame_with_events(p, ElapsedHeader::Absent).unwrap().events
}
fn pin(name: &str, input: u32, client: f64, shared: f64) {
    assert_eq!(client.to_bits(), shared.to_bits(), "{name} {input}");
    println!("({name:?}, {input}, 0x{:016x}),", client.to_bits());
}
fn main() {
    let mut state = ClientState::default(); apply(&mut state, &common::setup());
    for raw in [0, 1, 127, 128, 255, 65535, 8388607, 16777214, 16777215] {
        let mut p = common::appearance(); p[3..6].copy_from_slice(&common::u24(raw));
        p[7..10].copy_from_slice(&common::u24(raw)); p[12..15].copy_from_slice(&common::u24(raw));
        p[16..19].copy_from_slice(&common::u24(raw));
        let event = apply(&mut state, &p);
        let PresentationEvent::SnakeAppearance(a) = &event[0] else { panic!() };
        pin("angle24", raw, state.snakes[&17].heading, s::angle24(raw));
        pin("fraction24", raw, a.fam, s::fraction24(raw));
        pin("position5", raw, a.absolute_head.x, s::position5(raw));
        let mut prey = vec![0; 20]; prey[0] = b'y'; prey[11] = b'0';
        prey[12..15].copy_from_slice(&common::u24(raw)); prey[15..18].copy_from_slice(&common::u24(raw));
        let event = apply(&mut state, &prey);
        let PresentationEvent::PreySpawn(a) = event[0] else { panic!() };
        pin("prey_angle24", raw, a.heading, s::prey_angle24(raw));
    }
    for raw in [0, 1, 17, 18, 127, 128, 254, 255] {
        let event = apply(&mut state, &[b'd', raw, raw, raw]);
        let PresentationEvent::SnakeMotion { heading: Some(h), speed_per_32ms: Some(v), .. } = event[0] else { panic!() };
        pin("angle8", u32::from(raw), h, s::angle8(raw));
        pin("compact_speed_per_32ms", u32::from(raw), v, s::compact_speed_per_32ms(raw));
        pin("compact_speed", u32::from(raw), state.snakes[&17].speed, s::compact_speed(raw));
        let mut p = common::appearance();
        let first = p.len() - 10; p[first..first+6].fill(0);
        p[first+6] = raw; p[first+7] = 127;
        let event = apply(&mut state, &p);
        let PresentationEvent::SnakeAppearance(a) = &event[0] else { panic!() };
        pin("delta_half", u32::from(raw), a.absolute_body[1].x, s::delta_half(raw));
    }
    for raw in [0_u16, 1, 31, 32, 321, 32767, 65535] {
        let mut p = common::appearance(); p[10..12].copy_from_slice(&raw.to_be_bytes());
        let event = apply(&mut state, &p);
        let PresentationEvent::SnakeAppearance(a) = &event[0] else { panic!() };
        pin("full_speed_per_32ms", u32::from(raw), a.speed_per_32ms, s::full_speed_per_32ms(raw));
        pin("full_speed", u32::from(raw), state.snakes[&17].speed, s::full_speed(raw));
    }
    // Run the eleven historical solver witnesses through both actual solvers.
    for (_, _, encoded, hex) in CAPTURED {
        let current = slither_client::challenge::solve_challenge(encoded.as_bytes()).unwrap();
        let shared = slither_protocol::challenge::stock(encoded.as_bytes(), slither_protocol::challenge::Grammar::TokenWhitespace).unwrap();
        assert_eq!(&current[..], shared);
        let expected: Vec<_> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(&hex[i..i+2],16).unwrap()).collect();
        assert_eq!(current, expected);
    }
    // Compare current login construction after #391, with explicit profiles/tails.
    for version in [0, 291, 65535] { for custom in [&[][..], &[0, 128, 255]] {
        let old = slither_client::protocol::encode_login(version, &[9;20], "Fixed", 9, Some(custom)).unwrap();
        let mut out = [0; 400];
        let n = v19::client::write_login(v19::client::ModernLogin { client_version: version, page_echo: &[9;20], nickname: b"Fixed", skin:9, custom_skin:custom }, &mut out).unwrap();
        assert_eq!(&out[..n], old);
    }}
    let Message::SnakeAppearance(_) = v19::decode(&common::appearance(), DecodeAdmission::ClientPresentation).unwrap() else { panic!() };
    eprintln!("Client scalar bits, 11 retained challenges and login profiles agree.");
}
'''


def main():
    # Read only the existing literal witness array, preserving historical paths.
    witnesses = (ROOT / "tests/client_challenge.rs").read_text(encoding="utf-8")
    witnesses = witnesses[witnesses.index("const CAPTURED:"):witnesses.index("fn encode_source")]
    with tempfile.TemporaryDirectory(prefix="protocol-client-oracle-") as directory:
        path = Path(directory)
        (path / "src").mkdir()
        (path / "Cargo.toml").write_text(
            '[package]\nname="protocol-client-oracle"\nversion="0.0.0"\nedition="2024"\n'
            '[dependencies]\n'
            f'slither-client={{path="{(ROOT / "client").as_posix()}"}}\n'
            f'slither-protocol={{path="{(ROOT / "crates/slither-protocol").as_posix()}"}}\n',
            encoding="utf-8")
        source = SOURCE.replace("COMMON", (ROOT / "crates/slither-protocol/tests/common/mod.rs").as_posix())
        (path / "src/main.rs").write_text(source + witnesses, encoding="utf-8")
        return subprocess.run(["rustup", "run", "1.99.0", "cargo", "run", "--offline", "--manifest-path", str(path / "Cargo.toml")], check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
