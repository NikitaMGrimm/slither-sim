"""Run current client functions against shared scalars and print f64-bit goldens.

Offline, Linux only for A0. The temporary package is outside the workspace and
does not edit client sources. No production dependency on either consumer is
introduced. Keep generated expected bits fixed in the protocol package tests.

After PR B delegates client numerics to slither-protocol, regeneration MUST use
--client-root pointing to an independent pre-adoption client checkout at
8b36a91cafe6878ccedf0d7da70a4008ae7ef108 (merged #391). Using B's delegating
client would make the comparison circular. Never refresh old expected bits to
accept a shared-reader change.
"""
import argparse
from pathlib import Path
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parents[1]
PRE_ADOPTION_COMMIT = "8b36a91cafe6878ccedf0d7da70a4008ae7ef108"
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
    for raw in [0_u16, 1, 127, 128, 255, 16384, 32767, 32768, 65534, 65535] {
        let mut p = common::appearance(); let end = p.len();
        p[end-2..].copy_from_slice(&raw.to_be_bytes());
        let event = apply(&mut state, &p);
        let PresentationEvent::SnakeAppearance(a) = &event[0] else { panic!() };
        pin("body_angle16", u32::from(raw), a.heading, s::body_angle16(raw));
        // The private advance helper does not expose its angle. Run its exact
        // source with a cos seam that returns the argument, before any trig.
        pin("anchor_angle16", u32::from(raw), anchor_reference::angle(raw), s::anchor_angle16(raw));
        let [hi, lo] = raw.to_be_bytes();
        let event = apply(&mut state, &[b'j', 0, 1, hi, lo, hi, lo]);
        let PresentationEvent::PreyMotion(a) = &event[0] else { panic!() };
        pin("prey_position", u32::from(raw), a.absolute_position.x, s::prey_position(raw));
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
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--client-root", type=Path, default=ROOT / "client",
                        help=f"independent client source; after B use {PRE_ADOPTION_COMMIT}")
    args = parser.parse_args()
    client = args.client_root.resolve()
    manifest = tomllib.loads((client / "Cargo.toml").read_text(encoding="utf-8"))
    if "slither-protocol" in manifest.get("dependencies", {}):
        parser.error(f"circular oracle: use --client-root from pre-adoption {PRE_ADOPTION_COMMIT}")
    # Copy these two actual private functions without rewriting their arithmetic.
    # Only math::cos/sin are replaced in the temporary module: cos returns its
    # argument and spacing=1, origin=0 makes advance().x that exact angle. This
    # observes the client's intermediate value instead of duplicating its formula
    # or recovering an angle through platform-dependent inverse trigonometry.
    state_source = (client / "src/state.rs").read_text(encoding="utf-8")
    def function(name):
        start = state_source.index(f"fn {name}(")
        return state_source[start:state_source.index("\n}", start) + 2]
    anchor_reference = '''
mod anchor_reference {
    use slither_client::state::{Point, StateError, MAX_COORDINATE};
    use std::f64::consts::TAU;
    mod math {
        pub fn cos(angle: f64) -> f64 { angle }
        pub fn sin(_: f64) -> f64 { 0. }
    }
    pub fn angle(raw: u16) -> f64 { advance(Point::default(), raw, 1.).unwrap().x }
'''+function("checked_point")+"\n"+function("advance")+"\n}\n"
    # Read only the existing literal witness array, preserving historical paths.
    witnesses = (ROOT / "tests/client_challenge.rs").read_text(encoding="utf-8")
    witnesses = witnesses[witnesses.index("const CAPTURED:"):witnesses.index("fn encode_source")]
    with tempfile.TemporaryDirectory(prefix="protocol-client-oracle-") as directory:
        path = Path(directory)
        (path / "src").mkdir()
        (path / "Cargo.toml").write_text(
            '[package]\nname="protocol-client-oracle"\nversion="0.0.0"\nedition="2024"\n'
            '[dependencies]\n'
            f'slither-client={{path="{client.as_posix()}"}}\n'
            f'slither-protocol={{path="{(ROOT / "crates/slither-protocol").as_posix()}"}}\n',
            encoding="utf-8")
        source = SOURCE.replace("COMMON", (ROOT / "crates/slither-protocol/tests/common/mod.rs").as_posix())
        (path / "src/main.rs").write_text(source + witnesses + anchor_reference, encoding="utf-8")
        return subprocess.run(["rustup", "run", "1.99.0", "cargo", "run", "--offline", "--manifest-path", str(path / "Cargo.toml")], check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
