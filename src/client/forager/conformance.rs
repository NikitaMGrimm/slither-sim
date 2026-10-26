use super::{ForagerBot, fill_intents};
use crate::{Action, bot::PolicyId, client::Bot};
use serde_json::{Value, json};
#[cfg(any(
    all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"),
    all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"),
))]
use sha2::{Digest, Sha256};

#[cfg(any(
    all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"),
    all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"),
))]
fn input_sha256(frame: &Value) -> String {
    use std::fmt::Write;
    // Match project_oracle.py: compact UTF-8 bit-encoded JSON, recursively
    // sorted keys, no trailing newline. This hashes the input before decoding.
    let mut canonical = frame.clone();
    canonical.sort_all_objects();
    let digest = Sha256::digest(serde_json::to_vec(&canonical).unwrap());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        write!(hex, "{byte:02x}").unwrap();
    }
    hex
}

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

// Only the exact deployment targets on which the untouched kernel ran have
// an equality oracle. In particular, macOS conformance is unverified because
// no macOS oracle exists; macOS and other targets run determinism/invariants.
#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
const PLATFORM_ORACLE: Option<&str> = Some(include_str!(
    "../../../tests/fixtures/forager-oracle-windows-x86_64-msvc.json"
));
#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
const PLATFORM_ORACLE: Option<&str> = Some(include_str!(
    "../../../tests/fixtures/forager-oracle-linux-x86_64-gnu.json"
));
#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"),
    all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"),
)))]
const PLATFORM_ORACLE: Option<&str> = None;

fn fixture() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/forager-conformance.json"
    ))
    .unwrap();
    assert_eq!(fixture["clientContract"], "slither-client-v2");
    assert_eq!(fixture["wireProtocol"], 19);
    assert_eq!(fixture["cases"].as_array().unwrap().len(), 140);
    fixture
}

fn make_bot(name: &str) -> ForagerBot {
    let mut bot = ForagerBot::new(PolicyId::A1);
    if name == "A4-mean" {
        bot.policy = crate::bot::Forager::a4_mean_for_test();
    }
    bot
}

fn check_structure(frame: &crate::client::ClientFrame, result: Option<Action>) {
    let playing = frame.contract == crate::client::CONTRACT
        && frame.status == crate::client::ClientStatus::Playing
        && frame.observation.is_some();
    assert_eq!(result.is_some(), playing, "action presence");
    if let Some(result) = result {
        assert!(result.heading.is_finite(), "heading must be finite");
        assert!(action(Some(result))["boost"].is_boolean(), "boolean boost");
    }
}

#[test]
fn full_frames_are_deterministic_with_fresh_and_interleaved_shared_instances() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().unwrap();
    for name in ["A1", "A4-mean"] {
        let mut shared = make_bot(name);
        // Both orders reuse one instance across interleaved slots, lives,
        // connections and invalid frames, without resetting between decisions.
        for case in cases.iter().chain(cases.iter().rev()) {
            let frame = serde_json::from_value(decode(&case["frame"])).unwrap();
            let before = serde_json::to_value(&frame).unwrap();
            let mut first = make_bot(name);
            let mut second = make_bot(name);
            let inputs = derived(&first, &frame);
            assert_eq!(
                derived(&second, &frame),
                inputs,
                "{} {name}: fresh inputs",
                case["id"]
            );
            assert_eq!(
                derived(&shared, &frame),
                inputs,
                "{} {name}: shared inputs",
                case["id"]
            );
            let first = first.decide(&frame);
            let second = second.decide(&frame);
            let reused = shared.decide(&frame);
            for result in [first, second, reused] {
                check_structure(&frame, result);
            }
            assert_eq!(
                action(first),
                action(second),
                "{} {name}: fresh bits",
                case["id"]
            );
            assert_eq!(
                action(first),
                action(reused),
                "{} {name}: shared bits",
                case["id"]
            );
            assert_eq!(
                serde_json::to_value(&frame).unwrap(),
                before,
                "input mutated"
            );
        }
    }
}

#[cfg(any(
    all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"),
    all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"),
))]
#[test]
fn platform_oracle_equality_zero_mismatches() {
    let raw = PLATFORM_ORACLE.expect("this exact target has a frozen-kernel oracle");
    let oracle: Value = serde_json::from_str(raw).unwrap();
    assert_eq!(oracle["schemaVersion"], 2);
    assert_eq!(oracle["oracleStatus"], "complete");
    assert_eq!(oracle["oraclePlatform"]["os"], std::env::consts::OS);
    assert_eq!(oracle["oraclePlatform"]["arch"], std::env::consts::ARCH);
    let fixture = fixture();
    let cases = fixture["cases"].as_array().unwrap();
    let expected = oracle["cases"].as_array().unwrap();
    assert_eq!(expected.len(), cases.len());
    for name in ["A1", "A4-mean"] {
        let mut shared = make_bot(name);
        for (case, oracle_case) in cases.iter().zip(expected) {
            assert_eq!(case["id"], oracle_case["id"], "oracle case ID");
            assert_eq!(
                input_sha256(&case["frame"]),
                oracle_case["inputSha256"].as_str().unwrap(),
                "{}: oracle input hash",
                case["id"]
            );
            let frame = serde_json::from_value(decode(&case["frame"])).unwrap();
            let expected = &oracle_case["expected"][name];
            // Compare every derived bit before any action, including all 64
            // sent slots, truncation length and unknown-Intent NaN payloads.
            assert_eq!(
                derived(&shared, &frame),
                expected["derived"],
                "{} {name}: derived inputs",
                case["id"]
            );
            assert_eq!(
                action(shared.decide(&frame)),
                expected["action"],
                "{} {name}: action bits",
                case["id"]
            );
        }
    }
}

#[cfg(not(any(
    all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"),
    all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"),
)))]
#[test]
fn macos_and_other_targets_without_oracles_have_unverified_conformance() {
    assert!(PLATFORM_ORACLE.is_none());
    eprintln!(
        "macOS conformance is unverified because no macOS oracle exists. This target runs fresh/shared determinism and structural invariants, not oracle equality."
    );
}
