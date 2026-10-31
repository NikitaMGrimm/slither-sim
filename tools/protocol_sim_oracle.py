"""Compare the simulator with its fixed pre-adoption reader, offline.

Run on a build host: python3 tools/protocol_sim_oracle.py. Temporary test-only
modules come from the immutable baseline, never from generated expectations.
They are removed even on failure; no second reader ships in the final tree.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
BASE = "ef733563130caec44ef3da9234a2f13ca478fc36"


def fixture_bytes():
    """Retain every wire field in all JSON fixtures, including v13 rejection inputs."""
    cases = []

    def walk(value, path):
        if isinstance(value, dict):
            for key, child in value.items():
                if isinstance(child, str) and key.lower().endswith(("hex", "base64")):
                    try:
                        data = (base64.b64decode(child, validate=True) if key.lower().endswith("base64")
                                else bytes.fromhex(child))
                    except ValueError:
                        continue
                    cases.append((f"{path}:{key}", list(data), "frame" in key.lower()))
                else:
                    walk(child, f"{path}:{key}")
        elif isinstance(value, list):
            for index, child in enumerate(value):
                walk(child, f"{path}:{index}")

    for path in sorted((ROOT / "tests/fixtures").rglob("*.json")):
        walk(json.loads(path.read_text()), str(path.relative_to(ROOT)))
    return cases


SOURCE = r'''
use super::{protocol as new, oracle_protocol as old, oracle_challenge, challenge};
use crate::Action;

fn packet(p: &[u8]) {
    assert_eq!(old::oracle_packet(p), new::oracle_packet(p), "packet {p:?}");
}
fn frame(p: &[u8]) {
    assert_eq!(old::split_frame(p), new::split_frame(p), "frame {p:?}");
}
fn challenge(p: &[u8]) {
    assert_eq!(oracle_challenge::solve_challenge(p), challenge::solve_challenge(p), "challenge {p:?}");
}
fn encode(source: &[u8]) -> Vec<u8> {
    let mut key = 23;
    source.iter().flat_map(|byte| [byte >> 4, byte & 15]).map(|nibble| {
        let out = b'a' + (nibble + key) % 26; key = (key + 17) % 26; out
    }).collect()
}

#[test]
fn differential_probe() {
    let fixtures: Vec<(String, Vec<u8>, bool)> = serde_json::from_str(include_str!("oracle_fixtures.json")).unwrap();
    let mut commands = 0;
    for (name, bytes, framed) in &fixtures {
        if *framed {
            frame(bytes);
            if let Ok(packets) = old::split_frame(bytes) {
                for p in packets { packet(p); commands += 1; }
            }
        } else { packet(bytes); commands += 1; }
        let _ = name;
    }
    let mut synthetic = 0;
    // Every opcode, short and extended body sizes, five edge/random patterns.
    let mut random = 0x8f210ab9_u32;
    for opcode in 0..=255 {
        for len in 1..=80 {
            for pattern in 0..5 {
                let mut p = vec![opcode; len];
                for byte in &mut p[1..] {
                    random ^= random << 13; random ^= random >> 17; random ^= random << 5;
                    *byte = match pattern { 0 => 0, 1 => 255, 2 => 127, 3 => 1, _ => random as u8 };
                }
                packet(&p); synthetic += 1;
                let mut f = vec![0, 0, 0, len as u8]; f.extend(&p); frame(&f);
                f.pop(); frame(&f);
            }
        }
    }
    packet(&[]); frame(&[]); frame(&[0]); frame(&[0,0]);
    for (_, bytes, framed) in &fixtures {
        let parts = if *framed { old::split_frame(bytes).unwrap_or_default() } else { vec![bytes.as_slice()] };
        for p in parts {
            // Every retained command's truncations and one-byte mutations.
            for len in 0..p.len().min(256) { packet(&p[..len]); synthetic += 1; }
            for at in 0..p.len().min(256) {
                for value in [0, 1, 127, 255] { let mut q = p.to_vec(); q[at] = value; packet(&q); synthetic += 1; }
            }
        }
    }
    let mut large = vec![0,0,0,1,b'p'];
    for _ in 1..65_537 { large.extend([33,b'p']); }
    frame(&large);
    let mut map_count = 0;
    for side in [0_u16, 1, 2, 80, 512, 513, 65535] {
        for command in [b'M', b'U', b'u', b'L', b'V'] {
            for tail in [&[][..], &[255][..], &[64, 255][..], &[255,255,255,255][..], &[127,0,128,255,1][..]] {
                let [hi,lo] = side.to_be_bytes();
                let mut p = match command { b'L' => vec![command,2,hi,lo], b'M' | b'U' => vec![command,hi,lo], _ => vec![command] };
                p.extend(tail);
                let mut a = super::oracle_minimap::MinimapDecoder::default();
                let mut b = super::minimap::MinimapDecoder::default();
                let mut ca = super::minimap::MAX_FRAME_MINIMAP_CELLS;
                let mut cb = ca;
                for q in [&b"M\0\x02\x40"[..], &p, &b"V\x40"[..]] {
                    assert_eq!(a.apply(q,0.,&mut ca), b.apply(q,0.,&mut cb), "map {q:?}");
                    assert_eq!(format!("{a:?}"), format!("{b:?}"), "map {q:?}");
                    assert_eq!(ca,cb); map_count += 1;
                }
            }
        }
    }
    let mut challenge_count = 0;
    for encoded in CAPTURED { challenge(encoded.as_bytes()); challenge_count += 1; }
    let source = b"var a = \"ABCDE\";a+=\"FGHIJKLMNOPQRSTUVWXYZa\";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}";
    for len in 0..=source.len() { challenge(&encode(&source[..len])); challenge_count += 1; }
    for at in 0..source.len() {
        for value in [0, b' ', b'"', b'\\', 127, 255] {
            let mut q = source.to_vec(); q[at] = value; challenge(&encode(&q)); challenge_count += 1;
            for len in 0..q.len() { challenge(&encode(&q[..len])); challenge_count += 1; }
        }
    }
    // Paired state histories exercise clocking, setup, caches and atomic failure.
    let setup = super::encode::setup(3584., 4.).unwrap();
    let mut states = 0;
    for (_, bytes, framed) in &fixtures {
        let mut a = old::ClientState::new(); let mut b = new::ClientState::new();
        a.new_connection(0.).unwrap(); b.new_connection(0.).unwrap();
        a.receive_packet(&setup, 0., 0.).unwrap(); b.receive_packet(&setup, 0., 0.).unwrap();
        let result_a = if *framed { a.receive_frame(bytes, 0.1) } else { a.receive_packet(bytes, 0.1, 0.1) };
        let result_b = if *framed { b.receive_frame(bytes, 0.1) } else { b.receive_packet(bytes, 0.1, 0.1) };
        assert_eq!(result_a, result_b, "state input {bytes:?}");
        assert_eq!(format!("{:?}", a.frame(0.2)), format!("{:?}", b.frame(0.2)));
        states += 1;
    }
    for index in 0..100_000_u64 {
        let heading = f64::from_bits(index.wrapping_mul(0x9e3779b97f4a7c15));
        for boost in [false, true] {
            let action = Action { heading, boost };
            assert_eq!(old::action_packets(action, None), new::action_packets(action, None));
        }
    }
    println!("differential: fixture fields={}, commands={commands}, synthetic={synthetic}, challenges={challenge_count}, state histories={states}, minimap histories={map_count}, actions=200000; zero mismatches", fixtures.len());
}

#[test]
fn differential_receive_cost() {
    use std::{hint::black_box, time::Instant};
    for (density, snakes, foods) in [("sparse", 1, 16), ("dense", 8, 256), ("food-limit", 1, 65536)] {
        let setup = super::encode::setup(3584., 4.).unwrap();
        let mut initial = vec![setup];
        for id in 1..=snakes {
            // Authored minimal full appearance with a first and directed final anchor.
            let mut p = vec![0; 33]; p[0] = b's'; p[2] = id; p[6] = b'0';
            p[10..12].copy_from_slice(&5000_u16.to_be_bytes());
            p[16..22].copy_from_slice(&[1,0,0,1,0,0]);
            p[25..31].copy_from_slice(&[1,0,0,1,0,0]); initial.push(p);
        }
        let mut updates = vec![vec![b'd',32,33,90]];
        for id in 1..=snakes { updates.extend([vec![b'e',0,id,32,33,90], vec![b'g',0,id,0,0]]); }
        let mut food = vec![b'F',0,0];
        for id in 0..foods { food.extend([1,(id >> 8) as u8,id as u8,5]); }
        // A near-limit listing uses repeated sector packets, as the server does.
        for tuples in food[3..].chunks(4 * 2000) { let mut p = vec![b'F',0,0]; p.extend(tuples); updates.push(p); }
        let bytes = super::encode::frame(1,&updates).unwrap();
        let initial = super::encode::frame(0,&initial).unwrap();
        let mut a = old::ClientState::new(); let mut b = new::ClientState::new();
        a.new_connection(0.).unwrap(); b.new_connection(0.).unwrap();
        a.receive_frame(&initial,0.).unwrap(); b.receive_frame(&initial,0.).unwrap();
        a.receive_frame(&bytes,0.01).unwrap(); b.receive_frame(&bytes,0.01).unwrap();
        assert_eq!(format!("{:?}",a.frame(0.01)),format!("{:?}",b.frame(0.01)));
        for seats in if density == "food-limit" { vec![1] } else { vec![1,100,500] } {
            macro_rules! run {
                ($state:expr) => {{
                    let mut states = vec![$state.clone(); seats];
                    let start = Instant::now();
                    let allocations = crate::bot::oracle_allocations(|| {
                        for step in 1..=16 { for state in &mut states {
                            state.receive_frame(black_box(&bytes),0.01 + step as f64 * 0.001).unwrap();
                        } }
                    });
                    (start.elapsed().as_secs_f64(), allocations)
                }};
            }
            for round in 0..6 {
                let (old,new) = if round % 2 == 0 { (run!(a),run!(b)) }
                                else { let new = run!(b); (run!(a),new) };
                assert!(new.1 <= old.1, "receive allocations increased");
                println!("receive-cost {density} seats={seats} snakes={snakes} foods={foods} bytes={} commands={} frames={} pair={round} old_s={:.9} new_s={:.9} old_alloc={} new_alloc={}", bytes.len(), updates.len(), seats*16, old.0,new.0,old.1,new.1);
            }
        }
    }
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bench", action="store_true", help="paired optimized receive cost and allocation counts")
    args = parser.parse_args()
    client = ROOT / "src/client"
    paths = [client / "mod.rs", client / "protocol.rs", ROOT / "src/bot/mod.rs", ROOT / "src/bot/tests.rs"]
    originals = {path: path.read_bytes() for path in paths}
    created = []
    try:
        for module in ("protocol", "minimap", "rules_packet", "challenge"):
            source = subprocess.check_output(["git", "show", f"{BASE}:src/client/{module}.rs"], cwd=ROOT, text=True)
            source = source.split("#[cfg(test)]\nmod tests")[0]
            source = source.replace("#[cfg(test)]\nmod pit_projection_tests;", "")
            if module == "protocol":
                # Minimap has a separate paired retained-state check below.
                source = source.replace("super::rules_packet::", "super::oracle_rules_packet::")
                source = source.replace("super::challenge::", "super::oracle_challenge::")
                source += '\npub(super) fn oracle_packet(p: &[u8]) -> String { format!("{:?}", parse(p)) }\n'
            path = client / f"oracle_{module}.rs"
            if path.exists():
                raise FileExistsError(path)
            path.write_text(source)
            created.append(path)
        # Rust Debug round-trips every finite f64, including signed zero; all
        # accepted packet scalars are finite. Comparing the full representation
        # therefore checks exact numeric bits as well as options/order/errors.
        with (client / "protocol.rs").open("a") as output:
            output.write('\n#[cfg(test)]\npub(super) fn oracle_packet(p: &[u8]) -> String { format!("{:?}", parse(p)) }\n')
        fixtures = client / "oracle_fixtures.json"
        fixtures.write_text(json.dumps(fixture_bytes()))
        created.append(fixtures)
        captured = re.findall(r'"([a-zA-Z]{184})"', (ROOT / "tests/client_challenge.rs").read_text())
        source = SOURCE + '\nconst CAPTURED: &[&str] = &' + json.dumps(captured) + ';\n'
        path = client / "oracle_probe.rs"
        path.write_text(source)
        created.append(path)
        with (client / "mod.rs").open("a") as output:
            output.write('\n#[cfg(test)]\n#[allow(dead_code)]\nmod oracle_protocol;\n')
            for module in ("minimap", "rules_packet", "challenge", "probe"):
                output.write(f'#[cfg(test)]\n#[allow(dead_code)]\nmod oracle_{module};\n')
        with (ROOT / "src/bot/mod.rs").open("a") as output:
            output.write('\n#[cfg(test)] pub(crate) use tests::oracle_allocations;\n')
        with (ROOT / "src/bot/tests.rs").open("a") as output:
            output.write('''
pub(crate) fn oracle_allocations(run: impl FnOnce()) -> usize {
    ALLOCATIONS.set(0); COUNTING.set(true); run(); COUNTING.set(false); ALLOCATIONS.get()
}
''')
        mode = ["--release"] if args.bench else []
        test = "differential_receive_cost" if args.bench else "differential_probe"
        subprocess.run(["cargo", "+1.99.0", "test", "--locked", "--lib", *mode, test, "--", "--nocapture", "--test-threads=1"],
                       cwd=ROOT, env={**os.environ, "CARGO_BUILD_JOBS": "2"}, check=True)
    finally:
        for path, content in originals.items():
            path.write_bytes(content)
        for path in created:
            path.unlink()


if __name__ == "__main__":
    main()
