//! Measurement-only projection shared by old/new runtime API adapters.
//! Every authoritative field, ordered entity and timer is retained. Only the
//! checkpoint envelope, slot grouping and field spelling are normalized.
use serde_json::Value;
#[cfg(slither_checkpoint)]
use serde_json::{Map, json};
use slither_sim::Simulator;

#[cfg(not(slither_checkpoint))]
pub fn projection(world: &Simulator) -> Result<Value, String> {
    let mut value = serde_json::to_value(world).map_err(|e| e.to_string())?;
    if let Some(outward) = value.get_mut("prey_outward").and_then(Value::as_array_mut) {
        outward.sort_unstable_by_key(Value::as_u64);
    }
    Ok(value)
}

#[cfg(slither_checkpoint)]
pub fn projection(world: &Simulator) -> Result<Value, String> {
    project(
        serde_json::to_value(world.checkpoint().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
    )
}

#[cfg(slither_checkpoint)]
fn take(object: &mut Map<String, Value>, key: &str) -> Result<Value, String> {
    object
        .remove(key)
        .ok_or_else(|| format!("Checkpoint projection is missing {key}"))
}
#[cfg(slither_checkpoint)]
fn object(value: Value) -> Result<Map<String, Value>, String> {
    match value {
        Value::Object(value) => Ok(value),
        _ => Err("Checkpoint projection expected an object".into()),
    }
}
#[cfg(slither_checkpoint)]
fn array(value: Value) -> Result<Vec<Value>, String> {
    match value {
        Value::Array(value) => Ok(value),
        _ => Err("Checkpoint projection expected an array".into()),
    }
}
#[cfg(slither_checkpoint)]
fn empty(object: &Map<String, Value>) -> Result<(), String> {
    if object.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "Unprojected checkpoint fields: {:?}",
            object.keys().collect::<Vec<_>>()
        ))
    }
}

#[cfg(slither_checkpoint)]
fn project(value: Value) -> Result<Value, String> {
    let mut envelope = object(value)?;
    // Producer data is provenance, explicitly outside the authoritative state.
    for key in ["format", "rulesRevision", "producer"] {
        take(&mut envelope, key)?;
    }
    let mut config = object(take(&mut envelope, "config")?)?;
    // Checkpoint config records require nullable/false members that the legacy
    // Config serializer omitted. This changes representation, not their values.
    for key in [
        "pit",
        "spawnRadius",
        "spawnRadiusMax",
        "killScoreSeconds",
        "minScore",
    ] {
        if config.get(key) == Some(&Value::Null) {
            config.remove(key);
        }
    }
    if config.get("ambientRefillCountsAllFood") == Some(&Value::Bool(false)) {
        config.remove("ambientRefillCountsAllFood");
    }
    let mut state = object(take(&mut envelope, "state")?)?;
    empty(&envelope)?;
    let radius = take(&mut state, "radius")?;
    if config.get("radius") != Some(&radius) {
        return Err("Flat runtime Config radius differs from checkpoint live radius".into());
    }
    let mut result = Map::new();
    result.insert("config".into(), Value::Object(config));
    for (new, old) in [
        ("time", "time"),
        ("radiusUpdateIndex", "radius_update_index"),
        ("nextJoin", "next_join"),
        ("random", "random"),
        ("preyRandom", "prey_random"),
        ("nextPreyId", "next_prey_id"),
        ("ambientClock", "ambient_clock"),
        ("preyClock", "prey_clock"),
        ("preyMotionClock", "prey_motion_clock"),
        ("minMass", "min_mass"),
        ("failure", "failure"),
        ("deaths", "deaths"),
    ] {
        result.insert(old.into(), take(&mut state, new)?);
    }
    let mut snakes = Vec::new();
    let mut boosts = Vec::new();
    let mut speeds = Vec::new();
    let mut kiosks = Vec::new();
    let mut pit_entered = Vec::new();
    let mut death_times = Vec::new();
    let mut joins = Vec::new();
    let mut kill_times = Vec::new();
    for value in array(take(&mut state, "slots")?)? {
        let mut slot = object(value)?;
        snakes.push(take(&mut slot, "snake")?);
        let mut boost = object(take(&mut slot, "boost")?)?;
        for (new, old) in [
            ("activeBoostSeconds", "active_boost_seconds"),
            ("boostMassSpent", "boost_mass_spent"),
        ] {
            let value = take(&mut boost, new)?;
            boost.insert(old.into(), value);
        }
        boosts.push(Value::Object(boost));
        speeds.push(take(&mut slot, "speed")?);
        let mut kiosk = object(take(&mut slot, "kiosk")?)?;
        for (new, old) in [
            ("heldSeconds", "held_seconds"),
            ("scoreCredit", "score_credit"),
        ] {
            let value = take(&mut kiosk, new)?;
            kiosk.insert(old.into(), value);
        }
        kiosks.push(Value::Object(kiosk));
        pit_entered.push(take(&mut slot, "pitEntered")?);
        death_times.push(take(&mut slot, "deathTime")?);
        joins.push(take(&mut slot, "join")?);
        kill_times.push(take(&mut slot, "killTimes")?);
        empty(&slot)?;
    }
    for (key, values) in [
        ("snakes", snakes),
        ("boosts", boosts),
        ("speeds", speeds),
        ("kiosks", kiosks),
        ("pit_entered", pit_entered),
        ("death_times", death_times),
        ("joins", joins),
        ("kill_times", kill_times),
    ] {
        result.insert(key.into(), Value::Array(values));
    }
    let food = array(take(&mut state, "food")?)?
        .into_iter()
        .map(|value| {
            let mut food = object(value)?;
            for key in ["size", "origin"] {
                if food.get(key) == Some(&Value::Null) {
                    food.remove(key);
                }
            }
            Ok(Value::Object(food))
        })
        .collect::<Result<Vec<_>, String>>()?;
    result.insert("food".into(), Value::Array(food));
    let mut prey = Vec::new();
    let mut decisions = Map::new();
    let mut outward = Vec::new();
    for value in array(take(&mut state, "prey")?)? {
        let mut p = object(value)?;
        let id = p
            .get("id")
            .and_then(Value::as_u64)
            .ok_or("Checkpoint prey lacks an id")?;
        let decision = take(&mut p, "nextDecision")?;
        if !decision.is_null() {
            decisions.insert(id.to_string(), decision);
        }
        match take(&mut p, "mode")?.as_str() {
            Some("outward") => outward.push(json!(id)),
            Some("roaming") => {}
            _ => return Err("Unknown prey mode in projection".into()),
        }
        prey.push(Value::Object(p));
    }
    outward.sort_unstable_by_key(Value::as_u64);
    result.insert("prey".into(), Value::Array(prey));
    result.insert("prey_decision_times".into(), Value::Object(decisions));
    result.insert("prey_outward".into(), Value::Array(outward));
    empty(&state)?;
    Ok(Value::Object(result))
}
