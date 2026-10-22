use slither_sim::{Point, physics::append_anchor};

#[test]
fn continuously_updated_bodies_match_fresh_official_server_snapshots() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/server-body-snapshots.json")).unwrap();
    let tolerance = fixture["provenance"]["tolerance_world_units"]
        .as_f64()
        .unwrap();
    let points = |value: &serde_json::Value| -> Vec<Point> {
        value
            .as_array()
            .unwrap()
            .iter()
            .rev()
            .map(|p| Point {
                x: p[0].as_f64().unwrap(),
                y: p[1].as_f64().unwrap(),
            })
            .collect()
    };
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 38);
    for case in cases {
        let mut body = points(&case["initial_body"]);
        for step in case["steps"].as_array().unwrap() {
            if step["kind"] == "tail" {
                body.pop();
            } else {
                append_anchor(
                    &mut body,
                    Point {
                        x: step["x"].as_f64().unwrap(),
                        y: step["y"].as_f64().unwrap(),
                    },
                    step["grow"].as_bool().unwrap(),
                );
            }
        }
        let expected = points(&case["expected_body"]);
        assert_eq!(body.len(), expected.len());
        for (index, (actual, reference)) in body.iter().zip(expected).enumerate() {
            assert!(
                actual.distance(reference) <= tolerance,
                "{} snake {} at {} point {index}: {:?} vs {:?}",
                case["group"],
                case["snake_id"],
                case["receipt_ms"],
                actual,
                reference
            );
        }
    }
}
