use super::{ForagerBot, fill_intents};
use crate::{Action, bot::PolicyId, client::Bot};
use serde_json::{Value, json};

fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

fn decode(value: &Value) -> Value {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("f64") => {
            let raw = u64::from_str_radix(map["f64"].as_str().unwrap(), 16).unwrap();
            json!(f64::from_bits(raw))
        }
        Value::Object(map) => map
            .iter()
            .map(|(key, value)| (key.clone(), decode(value)))
            .collect(),
        Value::Array(values) => values.iter().map(decode).collect(),
        value => value.clone(),
    }
}

fn action(value: Option<Action>) -> Value {
    match value {
        Some(value) => {
            json!({"present": true, "heading": bits(value.heading), "boost": value.boost})
        }
        None => json!({"present": false}),
    }
}

fn derived(bot: &ForagerBot, frame: &crate::client::ClientFrame) -> Value {
    let timing = bot.timing(frame);
    let mut intents = Vec::new();
    if let Some(observation) = &frame.observation {
        fill_intents(&mut intents, observation, &frame.received);
    }
    json!({
        "timing": {
            "observationAge": bits(timing.observation_age),
            "commandDelay": bits(timing.command_delay),
            "processing": bits(timing.processing),
            "uncertainty": bits(timing.uncertainty),
            "sentLen": timing.sent_len,
            "sent": timing.sent.iter().map(|sent| json!({
                "secondsAgo": bits(sent.seconds_ago),
                "heading": bits(sent.action.heading),
                "boost": sent.action.boost,
            })).collect::<Vec<_>>(),
        },
        "intents": intents.iter().map(|intent| json!({
            "wanted": bits(intent.wanted), "headingAge": bits(intent.heading_age),
        })).collect::<Vec<_>>(),
    })
}

#[test]
fn frozen_oracle_full_frames_match_before_and_after_scratch_reuse() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/forager-conformance.json"
    ))
    .unwrap();
    assert_eq!(fixture["clientContract"], "slither-client-v2");
    assert_eq!(fixture["wireProtocol"], 19);
    assert_eq!(
        fixture["oracleStatus"], "complete",
        "requires verified Windows/Linux oracle outputs"
    );
    let cases = fixture["cases"].as_array().unwrap();
    assert!(!cases.is_empty());
    for name in ["A1", "A4-mean"] {
        let make_bot = || {
            let mut bot = ForagerBot::new(PolicyId::A1);
            if name == "A4-mean" {
                bot.policy = crate::bot::Forager::a4_mean_for_test();
            }
            bot
        };
        let mut shared = make_bot();
        // Reverse replay also alternates slots/lives and crosses every lifecycle
        // boundary in the opposite order; scratch never carries policy state.
        for case in cases.iter().chain(cases.iter().rev()) {
            let frame = serde_json::from_value(decode(&case["frame"])).unwrap();
            let before = serde_json::to_value(&frame).unwrap();
            let expected = &case["expected"][name];
            assert_eq!(
                derived(&shared, &frame),
                expected["derived"],
                "{} {name}: derived inputs",
                case["id"]
            );
            assert_eq!(
                action(shared.decide(&frame)),
                expected["action"],
                "{} {name}: shared action",
                case["id"]
            );
            assert_eq!(
                action(make_bot().decide(&frame)),
                expected["action"],
                "{} {name}: fresh action",
                case["id"]
            );
            shared.reset();
            assert_eq!(
                serde_json::to_value(&frame).unwrap(),
                before,
                "input mutated"
            );
        }
    }
}
