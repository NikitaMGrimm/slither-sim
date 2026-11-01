//! Retained, redacted official catalog bytes and independent Python-decoder
//! expectations. No simulator encoder or production JSON parsing is involved.
#![cfg(feature = "networking")]
use serde_json::{Value, json};
use slither_protocol::{DecodeAdmission::*, frame::*, scalar, v13::*};

const CATALOG: &str = include_str!("fixtures/protocol-catalog-13.json");

fn base64(source: &str) -> Vec<u8> {
    let mut bits = 0_u32;
    let mut count = 0;
    let mut bytes = Vec::new();
    for c in source.bytes().take_while(|&c| c != b'=') {
        let n = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
            .iter()
            .position(|&b| b == c)
            .unwrap() as u32;
        bits = (bits << 6) | n;
        count += 6;
        if count >= 8 {
            count -= 8;
            bytes.push((bits >> count) as u8);
        }
    }
    bytes
}

fn fields(m: Message<'_>) -> Value {
    match m {
        Message::Setup(s) => json!({"grid":s.grid_center, "maxSegments":s.max_segments,
            "sectorSize":s.sector_size, "sectorCount":s.sector_count, "protocol":s.protocol,
            "spacing":s.body_spacing, "radius":s.radius, "extensionBytes":s.command.len()-30}),
        Message::SnakeAppearance(s) => json!({"id":s.id, "fam":s.fraction, "skin":s.skin,
            "head":s.head.map(scalar::position5), "nameBytes":s.name.len(),
            "customBytes":s.custom_skin.len(), "bodyCount":usize::from(s.body.first.is_some())+s.body.deltas().len(),
            "bodyFirst":s.body.first.map(|p| p.map(scalar::position5))}),
        Message::SnakeRemoval(s) => {
            json!({"id":s.id,"removed":true,"died":s.raw_status.map(|b| b!=0)})
        }
        Message::Anchor(a) => {
            let mut v = json!({"id":a.id,"grow":a.grew,"own":false,"fam":a.fraction});
            match a.coordinates {
                Coordinates::Absolute(p) => v["position"] = json!(p),
                Coordinates::Relative(_) => v["delta"] = json!(a.coordinates.delta().unwrap()),
                _ => panic!("unexpected coordinate family"),
            }
            v
        }
        Message::Motion(m) => json!({"id":m.id,"own":false,"headingByte":m.heading,
            "wantedByte":m.wanted_heading,"speedByte":m.speed}),
        Message::Mass { id, fraction } => json!({"id":id,"fam":fraction}),
        Message::Tail { id, fraction } => json!({"id":id,"fam":fraction}),
        Message::FoodBatch(f) => {
            let first = f.foods().next().unwrap();
            json!({"foodsCount":f.foods().len(),"foodsFirst":[u16::from(first.color),first.position[0],first.position[1],u16::from(first.size)]})
        }
        Message::FoodSpawn(f) => json!({"color":f.color,"position":f.position,"size":f.size}),
        Message::FoodEaten(f) => json!({"position":f.position,"eater":f.eater}),
        Message::SectorClear(s) => json!({"sector":s}),
        Message::PreyAppearance(p) => json!({"id":p.id,"color":p.color,
            "position":p.position.map(scalar::position5),"size":p.size,"direction":p.raw_direction,
            "angleA":p.wanted_heading,"angleB":p.heading,"speed":p.speed}),
        Message::PreyRemoval { id, eater } => json!({"id":id,"removed":true,"eater":eater}),
        Message::PreyMotion(p) => {
            json!({"id":p.id,"position":p.position.map(|v| 1 + 3 * u32::from(v)),
            "direction":p.raw_direction,"heading":p.heading,"wanted":p.wanted_heading,"speed":p.speed})
        }
        Message::Leaderboard(l) => {
            let row = l.rows().next().unwrap();
            json!({"ownRank":l.local_row(),"rank":l.rank(),"population":l.population(),
                "rowsCount":l.rows().len(),"rowsFirst":{"segments":row.count(),"fam":row.raw_fraction(),
                "color":row.raw_color(),"nameBytes":row.name().len()}})
        }
        Message::Minimap(m) => {
            let side = m.dimension().unwrap();
            // Independently count raw run records and represented cells; these
            // catalog summaries are not new public parser fields.
            let bytes = &m.command()[3..];
            let (mut at, mut cells, mut runs) = (0, 0, 0);
            while at < bytes.len() {
                let b = bytes[at];
                at += 1;
                cells += if b == 255 {
                    let n = 126 * usize::from(bytes[at]);
                    at += 1;
                    runs += 1;
                    n
                } else if b >= 128 {
                    runs += 1;
                    usize::from(b - 128)
                } else {
                    7
                };
            }
            json!({"side":side,"encoding":"reverse-seven-bit-runs","dataBytes":bytes.len(),
                "encodedCells":cells,"setBits":m.selected(side).unwrap().count(),"zeroRuns":runs})
        }
        Message::Kills { id, count } => json!({"id":id,"kills":count}),
        Message::LifeControl { status, .. } => json!({"status":status}),
        Message::Radius(r) => json!({"radius":r}),
        Message::Challenge(c) => json!({"encodedBytes":c.len()}),
        Message::Session { .. } | Message::Pong(_) => json!({}),
        _ => panic!("catalog family must have explicit assertions"),
    }
}

#[test]
fn all_33_catalog_records_match_independent_expectations_under_each_admission() {
    let catalog: Value = serde_json::from_str(CATALOG).unwrap();
    assert_eq!(catalog["wireProtocol"], 13);
    assert_eq!(catalog["records"].as_array().unwrap().len(), 33);
    for record in catalog["records"].as_array().unwrap() {
        let bytes = base64(record["frameBase64"].as_str().unwrap());
        let frame = decode_frame(
            &bytes,
            Header::TwoBytes,
            Limits::CLIENT,
            FrameAdmission::Bounded,
        )
        .unwrap();
        let expected = record["expected"].as_array().unwrap();
        assert_eq!(frame.command_count(), expected.len());
        for admission in [Sim, ClientReceived, ClientPresentation] {
            for (command, expected) in frame.commands().zip(expected) {
                assert_eq!(
                    command.bytes[0],
                    expected["code"].as_str().unwrap().as_bytes()[0]
                );
                assert_eq!(
                    command.bytes.len() as u64,
                    expected["bytes"].as_u64().unwrap()
                );
                let actual = fields(decode_command(command, admission).unwrap());
                for (key, value) in expected.as_object().unwrap() {
                    if matches!(key.as_str(), "code" | "bytes") {
                        continue;
                    }
                    assert_eq!(
                        &actual[key], value,
                        "record {} field {key} ({admission:?})",
                        record["record"]
                    );
                }
            }
        }
    }
}
