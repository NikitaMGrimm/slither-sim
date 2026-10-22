//! A literal external JSON example guards the current private-preview shape.
//! Intentional revisions remain possible; this is not a permanent API freeze.
use serde_json::{Value, json};
use slither_sim::client::{CONTRACT, ClientFrame};

fn example() -> Value {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/client-contract-v2.json")).unwrap();
    fixture["frame"].clone()
}

/// Existing fields must retain their names and values; extra output fields are
/// allowed so an additive report does not require rewriting this baseline.
fn preserves_fields(expected: &Value, actual: &Value, path: &str) {
    match expected {
        Value::Object(fields) => {
            let object = actual.as_object().unwrap_or_else(|| panic!("{path}"));
            for (name, value) in fields {
                let next = format!("{path}/{name}");
                preserves_fields(
                    value,
                    object.get(name).unwrap_or_else(|| panic!("Missing {next}")),
                    &next,
                );
            }
        }
        Value::Array(values) => {
            let array = actual.as_array().unwrap_or_else(|| panic!("{path}"));
            assert_eq!(array.len(), values.len(), "{path}");
            for (i, (expected, actual)) in values.iter().zip(array).enumerate() {
                preserves_fields(expected, actual, &format!("{path}/{i}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn literal_preview_frame_preserves_json_names_types_and_optional_reports() {
    let expected = example();
    assert_eq!(expected["contract"], CONTRACT);
    let frame: ClientFrame = serde_json::from_value(expected.clone()).unwrap();
    frame.received.validate().unwrap();
    frame.minimap.as_ref().unwrap().validate().unwrap();
    preserves_fields(&expected, &serde_json::to_value(frame).unwrap(), "frame");
}

fn add_future_fields(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for value in fields.values_mut() {
                add_future_fields(value);
            }
            fields.insert("futureField".into(), json!({"version": 3}));
        }
        Value::Array(values) => values.iter_mut().for_each(add_future_fields),
        _ => {}
    }
}

#[test]
fn added_response_fields_are_ignored_at_every_object_level() {
    let expected: ClientFrame = serde_json::from_value(example()).unwrap();
    let mut extended = example();
    add_future_fields(&mut extended);
    assert_eq!(
        serde_json::from_value::<ClientFrame>(extended).unwrap(),
        expected
    );
}

#[test]
fn known_response_fields_still_require_their_declared_types() {
    for (path, invalid) in [
        ("/status", json!("futureStatus")),
        ("/life", json!(-1)),
        ("/settings/protocol", json!("19")),
        ("/observation/self/id", json!(7)),
        ("/received/snakes/0/position/ageSeconds", json!("old")),
        ("/minimap/cells/0", json!(256)),
        ("/hud/leaderboard/entries/0/segments", json!("two")),
        ("/hud/session/sessionId", Value::Null),
    ] {
        let mut value = example();
        *value.pointer_mut(path).unwrap() = invalid;
        assert!(
            serde_json::from_value::<ClientFrame>(value).is_err(),
            "{path}"
        );
    }
}
