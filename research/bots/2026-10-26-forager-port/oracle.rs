//! Standalone oracle exporter. Build against the untouched be3a035 crate.
//! Arguments: bit-encoded selected inputs, policy-data directory, output file.
use serde_json::{Value, json};
use slither_sim::{
    Action,
    client::{Bot, ClientFrame, ForagerBot, ReceivedMetadata},
    forager::{Intent, Params},
};
use std::{fs, path::Path};

fn bits(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}
fn decode(value: &Value) -> Value {
    match value {
        Value::Object(map) if map.len() == 1 && map.contains_key("f64") => {
            json!(f64::from_bits(
                u64::from_str_radix(map["f64"].as_str().unwrap(), 16).unwrap()
            ))
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
        Some(value) => json!({"present":true,"heading":bits(value.heading),"boost":value.boost}),
        None => json!({"present":false}),
    }
}
fn fill_intents(
    intents: &mut Vec<Intent>,
    observation: &slither_sim::Observation,
    received: &ReceivedMetadata,
) {
    intents.clear();
    let rows = &received.snakes;
    let mut next = 0;
    for opponent in &observation.opponents {
        let mut intent = Intent::default();
        let found = (0..rows.len())
            .map(|i| (next + i) % rows.len())
            .find(|&i| rows[i].snake_id == opponent.id);
        if let Some(i) = found {
            next = i + 1;
            let row = &rows[i];
            if let Some(wanted) = row.wanted_history.first() {
                // Motion reports round the target to 256 steps; the server
                // steers to the client's 251-step command, so take the nearest.
                let step = std::f64::consts::TAU / 251.0;
                intent.wanted =
                    (wanted.value.rem_euclid(std::f64::consts::TAU) / step).round() % 251.0 * step;
            }
            if row.heading.valid {
                intent.heading_age = row.heading.age_seconds;
            }
        }
        intents.push(intent);
    }
}

fn derived(bot: &ForagerBot, frame: &ClientFrame) -> Value {
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

fn params(data: &Path, name: &str) -> Params {
    let manifest: Value =
        serde_json::from_str(&fs::read_to_string(data.join("resolved.json")).unwrap()).unwrap();
    let entry = &manifest["policies"][name];
    let overrides: Value = serde_json::from_str(
        &fs::read_to_string(data.join(entry["overridesFile"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    let resolved = &entry["resolved"];
    let mut p = Params::default();
    if let Some(value) = overrides["cell"].as_f64() {
        p.cell = value;
    }
    assert_eq!(bits(p.cell), resolved["cell"].as_str().unwrap(), "cell");
    if let Some(value) = overrides["cells"].as_u64() {
        p.cells = value as usize;
    }
    assert_eq!(json!(p.cells), resolved["cells"]);
    if let Some(value) = overrides["settle_seconds"].as_f64() {
        p.settle_seconds = value;
    }
    assert_eq!(
        bits(p.settle_seconds),
        resolved["settle_seconds"].as_str().unwrap(),
        "settle_seconds"
    );
    if let Some(value) = overrides["occupied_factor"].as_f64() {
        p.occupied_factor = value;
    }
    assert_eq!(
        bits(p.occupied_factor),
        resolved["occupied_factor"].as_str().unwrap(),
        "occupied_factor"
    );
    if let Some(value) = overrides["orbit_factor"].as_f64() {
        p.orbit_factor = value;
    }
    assert_eq!(
        bits(p.orbit_factor),
        resolved["orbit_factor"].as_str().unwrap(),
        "orbit_factor"
    );
    if let Some(value) = overrides["rim_food_margin"].as_f64() {
        p.rim_food_margin = value;
    }
    assert_eq!(
        bits(p.rim_food_margin),
        resolved["rim_food_margin"].as_str().unwrap(),
        "rim_food_margin"
    );
    if let Some(value) = overrides["head_path_seconds"].as_f64() {
        p.head_path_seconds = value;
    }
    assert_eq!(
        bits(p.head_path_seconds),
        resolved["head_path_seconds"].as_str().unwrap(),
        "head_path_seconds"
    );
    if let Some(value) = overrides["head_path_steps"].as_u64() {
        p.head_path_steps = value as usize;
    }
    assert_eq!(json!(p.head_path_steps), resolved["head_path_steps"]);
    if let Some(value) = overrides["lookahead_seconds"].as_f64() {
        p.lookahead_seconds = value;
    }
    assert_eq!(
        bits(p.lookahead_seconds),
        resolved["lookahead_seconds"].as_str().unwrap(),
        "lookahead_seconds"
    );
    if let Some(value) = overrides["reach_extra"].as_f64() {
        p.reach_extra = value;
    }
    assert_eq!(
        bits(p.reach_extra),
        resolved["reach_extra"].as_str().unwrap(),
        "reach_extra"
    );
    if let Some(value) = overrides["rim_ahead_seconds"].as_f64() {
        p.rim_ahead_seconds = value;
    }
    assert_eq!(
        bits(p.rim_ahead_seconds),
        resolved["rim_ahead_seconds"].as_str().unwrap(),
        "rim_ahead_seconds"
    );
    if let Some(value) = overrides["rim_ahead_credit"].as_f64() {
        p.rim_ahead_credit = value;
    }
    assert_eq!(
        bits(p.rim_ahead_credit),
        resolved["rim_ahead_credit"].as_str().unwrap(),
        "rim_ahead_credit"
    );
    if let Some(value) = overrides["safe"].as_f64() {
        p.safe = value;
    }
    assert_eq!(bits(p.safe), resolved["safe"].as_str().unwrap(), "safe");
    if let Some(value) = overrides["danger_weight"].as_f64() {
        p.danger_weight = value;
    }
    assert_eq!(
        bits(p.danger_weight),
        resolved["danger_weight"].as_str().unwrap(),
        "danger_weight"
    );
    if let Some(value) = overrides["roam_fraction"].as_f64() {
        p.roam_fraction = value;
    }
    assert_eq!(
        bits(p.roam_fraction),
        resolved["roam_fraction"].as_str().unwrap(),
        "roam_fraction"
    );
    if let Some(fan) = overrides["fan"].as_array() {
        p.fan = [0.0; 16];
        p.fan_len = fan.len();
        for (i, value) in fan.iter().enumerate() {
            p.fan[i] = value.as_f64().unwrap();
        }
    }
    for (i, value) in p.fan.iter().enumerate() {
        assert_eq!(bits(*value), resolved["fan"][i].as_str().unwrap());
    }
    assert_eq!(json!(p.fan_len), resolved["fan_len"]);
    if let Some(value) = overrides["uplink_share"].as_f64() {
        p.uplink_share = value;
    }
    assert_eq!(
        bits(p.uplink_share),
        resolved["uplink_share"].as_str().unwrap(),
        "uplink_share"
    );
    if let Some(value) = overrides["lead_scale"].as_f64() {
        p.lead_scale = value;
    }
    assert_eq!(
        bits(p.lead_scale),
        resolved["lead_scale"].as_str().unwrap(),
        "lead_scale"
    );
    if let Some(value) = overrides["uncertainty_margin"].as_f64() {
        p.uncertainty_margin = value;
    }
    assert_eq!(
        bits(p.uncertainty_margin),
        resolved["uncertainty_margin"].as_str().unwrap(),
        "uncertainty_margin"
    );
    if let Some(value) = overrides["intent_weight"].as_f64() {
        p.intent_weight = value;
    }
    assert_eq!(
        bits(p.intent_weight),
        resolved["intent_weight"].as_str().unwrap(),
        "intent_weight"
    );
    if let Some(value) = overrides["stale_margin"].as_f64() {
        p.stale_margin = value;
    }
    assert_eq!(
        bits(p.stale_margin),
        resolved["stale_margin"].as_str().unwrap(),
        "stale_margin"
    );
    if let Some(value) = overrides["own_width_factor"].as_f64() {
        p.own_width_factor = value;
    }
    assert_eq!(
        bits(p.own_width_factor),
        resolved["own_width_factor"].as_str().unwrap(),
        "own_width_factor"
    );
    if let Some(value) = overrides["kill_weight"].as_f64() {
        p.kill_weight = value;
    }
    assert_eq!(
        bits(p.kill_weight),
        resolved["kill_weight"].as_str().unwrap(),
        "kill_weight"
    );
    if let Some(value) = overrides["kill_range"].as_f64() {
        p.kill_range = value;
    }
    assert_eq!(
        bits(p.kill_range),
        resolved["kill_range"].as_str().unwrap(),
        "kill_range"
    );
    if let Some(value) = overrides["kill_horizon"].as_f64() {
        p.kill_horizon = value;
    }
    assert_eq!(
        bits(p.kill_horizon),
        resolved["kill_horizon"].as_str().unwrap(),
        "kill_horizon"
    );
    if let Some(value) = overrides["kill_samples"].as_u64() {
        p.kill_samples = value as usize;
    }
    assert_eq!(json!(p.kill_samples), resolved["kill_samples"]);
    if let Some(value) = overrides["kill_gap"].as_f64() {
        p.kill_gap = value;
    }
    assert_eq!(
        bits(p.kill_gap),
        resolved["kill_gap"].as_str().unwrap(),
        "kill_gap"
    );
    if let Some(value) = overrides["kill_window"].as_f64() {
        p.kill_window = value;
    }
    assert_eq!(
        bits(p.kill_window),
        resolved["kill_window"].as_str().unwrap(),
        "kill_window"
    );
    if let Some(value) = overrides["kill_cross_angle"].as_f64() {
        p.kill_cross_angle = value;
    }
    assert_eq!(
        bits(p.kill_cross_angle),
        resolved["kill_cross_angle"].as_str().unwrap(),
        "kill_cross_angle"
    );
    if let Some(value) = overrides["kill_trap_weight"].as_f64() {
        p.kill_trap_weight = value;
    }
    assert_eq!(
        bits(p.kill_trap_weight),
        resolved["kill_trap_weight"].as_str().unwrap(),
        "kill_trap_weight"
    );
    if let Some(value) = overrides["kill_trap_seconds"].as_f64() {
        p.kill_trap_seconds = value;
    }
    assert_eq!(
        bits(p.kill_trap_seconds),
        resolved["kill_trap_seconds"].as_str().unwrap(),
        "kill_trap_seconds"
    );
    if let Some(value) = overrides["coil_weight"].as_f64() {
        p.coil_weight = value;
    }
    assert_eq!(
        bits(p.coil_weight),
        resolved["coil_weight"].as_str().unwrap(),
        "coil_weight"
    );
    if let Some(value) = overrides["coil_mass_ratio"].as_f64() {
        p.coil_mass_ratio = value;
    }
    assert_eq!(
        bits(p.coil_mass_ratio),
        resolved["coil_mass_ratio"].as_str().unwrap(),
        "coil_mass_ratio"
    );
    if let Some(value) = overrides["coil_range"].as_f64() {
        p.coil_range = value;
    }
    assert_eq!(
        bits(p.coil_range),
        resolved["coil_range"].as_str().unwrap(),
        "coil_range"
    );
    if let Some(value) = overrides["coil_radius_widths"].as_f64() {
        p.coil_radius_widths = value;
    }
    assert_eq!(
        bits(p.coil_radius_widths),
        resolved["coil_radius_widths"].as_str().unwrap(),
        "coil_radius_widths"
    );
    if let Some(value) = overrides["coil_lead_angle"].as_f64() {
        p.coil_lead_angle = value;
    }
    assert_eq!(
        bits(p.coil_lead_angle),
        resolved["coil_lead_angle"].as_str().unwrap(),
        "coil_lead_angle"
    );
    if let Some(value) = overrides["coil_close_angle"].as_f64() {
        p.coil_close_angle = value;
    }
    assert_eq!(
        bits(p.coil_close_angle),
        resolved["coil_close_angle"].as_str().unwrap(),
        "coil_close_angle"
    );
    if let Some(value) = overrides["coil_shrink"].as_f64() {
        p.coil_shrink = value;
    }
    assert_eq!(
        bits(p.coil_shrink),
        resolved["coil_shrink"].as_str().unwrap(),
        "coil_shrink"
    );
    if let Some(value) = overrides["coil_progress_power"].as_f64() {
        p.coil_progress_power = value;
    }
    assert_eq!(
        bits(p.coil_progress_power),
        resolved["coil_progress_power"].as_str().unwrap(),
        "coil_progress_power"
    );
    if let Some(value) = overrides["boost_min_score"].as_f64() {
        p.boost_min_score = value;
    }
    assert_eq!(
        bits(p.boost_min_score),
        resolved["boost_min_score"].as_str().unwrap(),
        "boost_min_score"
    );
    if let Some(value) = overrides["kill_boost"].as_bool() {
        p.kill_boost = value;
    }
    assert_eq!(json!(p.kill_boost), resolved["kill_boost"]);
    if let Some(value) = overrides["escape_boost"].as_bool() {
        p.escape_boost = value;
    }
    assert_eq!(json!(p.escape_boost), resolved["escape_boost"]);
    if let Some(value) = overrides["food_boost_value"].as_f64() {
        p.food_boost_value = value;
    }
    assert_eq!(
        bits(p.food_boost_value),
        resolved["food_boost_value"].as_str().unwrap(),
        "food_boost_value"
    );
    if let Some(value) = overrides["safe_width_factor"].as_f64() {
        p.safe_width_factor = value;
    }
    assert_eq!(
        bits(p.safe_width_factor),
        resolved["safe_width_factor"].as_str().unwrap(),
        "safe_width_factor"
    );
    if let Some(value) = overrides["exit_weight"].as_f64() {
        p.exit_weight = value;
    }
    assert_eq!(
        bits(p.exit_weight),
        resolved["exit_weight"].as_str().unwrap(),
        "exit_weight"
    );
    if let Some(value) = overrides["exit_seconds"].as_f64() {
        p.exit_seconds = value;
    }
    assert_eq!(
        bits(p.exit_seconds),
        resolved["exit_seconds"].as_str().unwrap(),
        "exit_seconds"
    );
    if let Some(value) = overrides["prey_weight"].as_f64() {
        p.prey_weight = value;
    }
    assert_eq!(
        bits(p.prey_weight),
        resolved["prey_weight"].as_str().unwrap(),
        "prey_weight"
    );
    if let Some(value) = overrides["prey_range"].as_f64() {
        p.prey_range = value;
    }
    assert_eq!(
        bits(p.prey_range),
        resolved["prey_range"].as_str().unwrap(),
        "prey_range"
    );
    if let Some(value) = overrides["prey_horizon"].as_f64() {
        p.prey_horizon = value;
    }
    assert_eq!(
        bits(p.prey_horizon),
        resolved["prey_horizon"].as_str().unwrap(),
        "prey_horizon"
    );
    if let Some(value) = overrides["race_sigma"].as_f64() {
        p.race_sigma = value;
    }
    assert_eq!(
        bits(p.race_sigma),
        resolved["race_sigma"].as_str().unwrap(),
        "race_sigma"
    );
    if let Some(value) = overrides["race_min_value"].as_f64() {
        p.race_min_value = value;
    }
    assert_eq!(
        bits(p.race_min_value),
        resolved["race_min_value"].as_str().unwrap(),
        "race_min_value"
    );
    if let Some(value) = overrides["race_cells"].as_u64() {
        p.race_cells = value as usize;
    }
    assert_eq!(json!(p.race_cells), resolved["race_cells"]);
    if let Some(value) = overrides["boost_free_score"].as_f64() {
        p.boost_free_score = value;
    }
    assert_eq!(
        bits(p.boost_free_score),
        resolved["boost_free_score"].as_str().unwrap(),
        "boost_free_score"
    );
    p.validate().unwrap();
    p
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let data = Path::new(&args[2]);
    let mut fixture: Value = serde_json::from_str(&fs::read_to_string(&args[1]).unwrap()).unwrap();
    for name in ["A1", "A4-mean"] {
        let mut bot = ForagerBot::new(params(data, name)).unwrap();
        for case in fixture["cases"].as_array_mut().unwrap() {
            let frame: ClientFrame = serde_json::from_value(decode(&case["frame"])).unwrap();
            let input = derived(&bot, &frame);
            let output = action(bot.decide(&frame));
            case["expected"][name] = json!({"derived": input, "action": output});
        }
    }
    fs::write(&args[3], serde_json::to_string(&fixture).unwrap() + "\n").unwrap();
}
