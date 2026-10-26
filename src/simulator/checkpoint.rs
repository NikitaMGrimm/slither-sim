//! Validated, versioned core continuations. Debug snapshots are not checkpoints.
use super::*;
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    mem::size_of,
};

pub const FORMAT: &str = "slither-sim-checkpoint-v2";
pub const RULES_REVISION: &str = "slither-sim-rules-v2";

#[derive(Clone, Copy, Debug, Default)]
pub struct EventRecording {
    pub pickups: bool,
    pub food_births: bool,
}

/// Non-authoritative observation/event scheduling is chosen by the restoring caller.
#[derive(Clone, Copy, Debug, Default)]
pub struct RestoreOptions {
    pub events: EventRecording,
    #[cfg(feature = "parallel")]
    pub parallel: bool,
}

/// Owned validated payload. Serialize this value; load bytes through `from_checkpoint`.
/// No unchecked public DTO conversion or `Deserialize` implementation is provided.
///
/// ```compile_fail
/// let _: slither_sim::Simulator = serde_json::from_str("{}").unwrap();
/// ```
/// ```compile_fail
/// let _: slither_sim::Checkpoint = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Clone, Debug, Serialize)]
#[serde(transparent)]
pub struct Checkpoint(Envelope);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    format: String,
    rules_revision: String,
    producer: Producer,
    #[serde(with = "ConfigRecord")]
    config: Config,
    state: State,
}

// Full checkpoint configuration, independent of request defaults and presets.
#[derive(Serialize, Deserialize)]
#[serde(remote = "Config", rename_all = "camelCase", deny_unknown_fields)]
struct ConfigRecord {
    radius: f64,
    dynamic_radius: bool,
    size_to_population: bool,
    vision: f64,
    food_count: usize,
    ambient_refill_counts_all_food: bool,
    ambient_replenish_per_second: f64,
    dt: f64,
    food_index: bool,
    emit_boost_food: bool,
    speed_model: SpeedModel,
    prey: PreyConfig,
    radius_updates: Vec<crate::RadiusUpdate>,
    boundary_mode: crate::BoundaryMode,
    kiosk: KioskConfig,
    death_food: DeathFoodConfig,
    ambient_food: AmbientFoodConfig,
    boost_food: BoostFoodConfig,
    #[serde(deserialize_with = "required_option")]
    pit: Option<PitConfig>,
    #[serde(deserialize_with = "required_option")]
    spawn_radius: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    spawn_radius_max: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    kill_score_seconds: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    min_score: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Producer {
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    commit: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct State {
    time: f64,
    radius: f64,
    radius_update_index: usize,
    next_join: u64,
    random: u32,
    prey_random: u32,
    next_prey_id: u64,
    ambient_clock: f64,
    prey_clock: f64,
    prey_motion_clock: f64,
    #[serde(deserialize_with = "required_option")]
    min_mass: Option<FloorRecord>,
    #[serde(deserialize_with = "required_option")]
    failure: Option<SimError>,
    slots: Vec<Slot>,
    food: Vec<FoodRecord>,
    prey: Vec<PreyRecord>,
    deaths: Vec<DeathRecord>,
}

// Option fields in authoritative state must be present, even when null. Serde's
// usual missing-Option default would silently invent absent companion state.
fn required_option<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}

fn fields(value: &Value, names: &[&str], path: &str) -> Result<(), SimError> {
    let object = value.as_object().ok_or_else(|| {
        invalid(
            Constraint::InconsistentState,
            path,
            "Checkpoint record must be an object",
        )
    })?;
    for name in names {
        require(
            object.contains_key(*name),
            Constraint::Missing,
            &format!("{path}.{name}"),
        )?;
    }
    for name in object.keys() {
        require(
            names.contains(&name.as_str()),
            Constraint::InconsistentState,
            &format!("{path}.{name}"),
        )?;
    }
    Ok(())
}

fn state_schema(raw: &Value) -> Result<(), SimError> {
    fields(
        raw,
        &["format", "rulesRevision", "producer", "config", "state"],
        "checkpoint",
    )?;
    // The record's own serialization names every config key, so a new setting
    // cannot be left out of this list.
    let canonical = ConfigRecord::serialize(&Config::default(), serde_json::value::Serializer)
        .map_err(|e| invalid(Constraint::InconsistentState, "config", e.to_string()))?;
    let keys: Vec<&str> = canonical
        .as_object()
        .into_iter()
        .flat_map(|config| config.keys().map(String::as_str))
        .collect();
    fields(&raw["config"], &keys, "config")?;
    let state = &raw["state"];
    fields(
        state,
        &[
            "time",
            "radius",
            "radiusUpdateIndex",
            "nextJoin",
            "random",
            "preyRandom",
            "nextPreyId",
            "ambientClock",
            "preyClock",
            "preyMotionClock",
            "minMass",
            "failure",
            "slots",
            "food",
            "prey",
            "deaths",
        ],
        "state",
    )?;
    for (i, slot) in state["slots"].as_array().into_iter().flatten().enumerate() {
        let path = format!("state.slots[{i}]");
        fields(
            slot,
            &[
                "snake",
                "boost",
                "speed",
                "kiosk",
                "pitEntered",
                "deathTime",
                "join",
                "killTimes",
            ],
            &path,
        )?;
        for (name, keys) in [
            (
                "snake",
                &[
                    "id", "head", "body", "heading", "speed", "radius", "length", "segments",
                    "fam", "score", "kills", "alive",
                ][..],
            ),
            (
                "boost",
                &[
                    "held",
                    "active",
                    "exhausted",
                    "clock",
                    "activeBoostSeconds",
                    "boostMassSpent",
                ],
            ),
            ("speed", &["speed", "progress"]),
            ("kiosk", &["anchor", "heldSeconds", "scoreCredit"]),
        ] {
            fields(&slot[name], keys, &format!("{path}.{name}"))?;
        }
    }
    for (name, keys) in [
        ("food", &["x", "y", "value", "size", "origin"][..]),
        (
            "prey",
            &[
                "id",
                "position",
                "heading",
                "targetHeading",
                "speed",
                "size",
                "nextDecision",
                "mode",
            ],
        ),
        ("deaths", &["time", "victim", "killer", "reason", "food"]),
    ] {
        for (i, value) in state[name].as_array().into_iter().flatten().enumerate() {
            fields(value, keys, &format!("state.{name}[{i}]"))?;
        }
    }
    if !state["minMass"].is_null() {
        fields(&state["minMass"], &["segments", "fam"], "state.minMass")?;
    }
    if !state["failure"].is_null() {
        fields(
            &state["failure"],
            &["kind", "world", "slot", "field", "message"],
            "state.failure",
        )?;
    }
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Slot {
    snake: SnakeRecord,
    boost: BoostRecord,
    speed: SpeedRecord,
    kiosk: KioskRecord,
    pit_entered: bool,
    #[serde(deserialize_with = "required_option")]
    death_time: Option<f64>,
    join: u64,
    kill_times: Vec<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Position {
    x: f64,
    y: f64,
}
impl From<Point> for Position {
    fn from(p: Point) -> Self {
        Self { x: p.x, y: p.y }
    }
}
impl From<Position> for Point {
    fn from(p: Position) -> Self {
        Self { x: p.x, y: p.y }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnakeRecord {
    id: String,
    head: Position,
    body: Vec<Position>,
    heading: f64,
    speed: f64,
    radius: f64,
    length: f64,
    segments: usize,
    fam: f64,
    score: f64,
    kills: usize,
    alive: bool,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BoostRecord {
    held: bool,
    active: bool,
    exhausted: bool,
    clock: f64,
    active_boost_seconds: f64,
    boost_mass_spent: f64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpeedRecord {
    speed: f64,
    progress: f64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct KioskRecord {
    #[serde(deserialize_with = "required_option")]
    anchor: Option<Position>,
    held_seconds: f64,
    score_credit: f64,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FloorRecord {
    segments: usize,
    fam: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FoodRecord {
    x: f64,
    y: f64,
    value: f64,
    #[serde(deserialize_with = "required_option")]
    size: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    origin: Option<Origin>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PreyRecord {
    id: u64,
    position: Position,
    heading: f64,
    target_heading: f64,
    speed: f64,
    size: f64,
    #[serde(deserialize_with = "required_option")]
    next_decision: Option<f64>,
    mode: PreyMode,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DeathRecord {
    time: f64,
    victim: String,
    #[serde(deserialize_with = "required_option")]
    killer: Option<String>,
    reason: DeathReason,
    food: usize,
}

fn invalid(rule: Constraint, field: impl Into<String>, message: impl Into<String>) -> SimError {
    SimError::new(ErrorKind::InvalidCheckpoint { rule }, message).at_field(field)
}
fn require(ok: bool, rule: Constraint, field: &str) -> Result<(), SimError> {
    if ok {
        Ok(())
    } else {
        Err(invalid(
            rule,
            field,
            format!("Invalid checkpoint {field}: {rule:?}"),
        ))
    }
}
fn finite(value: f64, field: &str) -> Result<(), SimError> {
    require(value.is_finite(), Constraint::NonFinite, field)
}
fn nonnegative(value: f64, field: &str) -> Result<(), SimError> {
    finite(value, field)?;
    require(value >= 0., Constraint::Negative, field)
}
fn positive(value: f64, field: &str) -> Result<(), SimError> {
    finite(value, field)?;
    require(value > 0., Constraint::NonPositive, field)
}
/// Largest accepted radius, heading and one tick of snake travel. Individually
/// finite values near f64::MAX would overflow on the next step.
pub(super) const MAGNITUDE: f64 = 1e150;
/// Living snakes lie within this. A tick moves a head at most one more
/// MAGNITUDE before the boundary constrains or kills it, so dead snakes and
/// their food stay within DEAD and continuation remains exportable. Squared
/// distances between such points stay finite.
pub(super) const LIVING: f64 = 2. * MAGNITUDE;
pub(super) const DEAD: f64 = LIVING + MAGNITUDE;
fn bounded(value: f64, limit: f64, field: &str) -> Result<(), SimError> {
    finite(value, field)?;
    require(value.abs() <= limit, Constraint::OutOfRange, field)
}
fn position(p: Position, limit: f64, field: &str) -> Result<(), SimError> {
    bounded(p.x, limit, &format!("{field}.x"))?;
    bounded(p.y, limit, &format!("{field}.y"))
}
fn timestamp(value: f64, time: f64, field: &str) -> Result<(), SimError> {
    nonnegative(value, field)?;
    require(value <= time, Constraint::OutOfRange, field)
}
fn reserve<T>(v: &mut Vec<T>, count: usize, field: &str) -> Result<(), SimError> {
    let bytes = count
        .checked_mul(size_of::<T>())
        .filter(|&n| n <= isize::MAX as usize);
    require(bytes.is_some(), Constraint::OutOfRange, field)?;
    v.try_reserve_exact(count).map_err(|_| {
        invalid(
            Constraint::OutOfRange,
            field,
            "Checkpoint storage cannot be reserved",
        )
    })
}
fn collect<T, U>(values: &[T], field: &str, f: impl FnMut(&T) -> U) -> Result<Vec<U>, SimError> {
    let mut out = Vec::new();
    reserve(&mut out, values.len(), field)?;
    out.extend(values.iter().map(f));
    Ok(out)
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SimError> {
    let mut de = serde_json::Deserializer::from_slice(bytes);
    let out = serde_path_to_error::deserialize(&mut de).map_err(|error| {
        invalid(
            Constraint::InconsistentState,
            error.path().to_string(),
            error.inner().to_string(),
        )
    })?;
    de.end()
        .map_err(|e| invalid(Constraint::InconsistentState, "checkpoint", e.to_string()))?;
    Ok(out)
}

// Config's request decoder supplies defaults/presets. A checkpoint instead needs
// all canonical fields, including nested configuration; reject fields it ignored.
// Serde also reads a struct from an array positionally, filling missing entries
// from container defaults, so every value must keep its canonical JSON type.
fn config_shape(raw: &Value, resolved: &Value, path: &str) -> Result<(), SimError> {
    if std::mem::discriminant(raw) != std::mem::discriminant(resolved) {
        return Err(invalid(
            Constraint::InconsistentState,
            path,
            "Checkpoint config value has a noncanonical JSON type",
        ));
    }
    match (raw, resolved) {
        (Value::Object(raw), Value::Object(resolved)) => {
            for (key, expected) in resolved {
                let field = format!("{path}.{key}");
                let value = raw.get(key).ok_or_else(|| {
                    invalid(
                        Constraint::Missing,
                        &field,
                        "Missing checkpoint config field",
                    )
                })?;
                config_shape(value, expected, &field)?;
            }
            for key in raw.keys() {
                require(
                    resolved.contains_key(key),
                    Constraint::InconsistentState,
                    &format!("{path}.{key}"),
                )?;
            }
        }
        (Value::Array(raw), Value::Array(resolved)) => {
            require(
                raw.len() == resolved.len(),
                Constraint::InconsistentState,
                path,
            )?;
            for (index, (value, expected)) in raw.iter().zip(resolved).enumerate() {
                config_shape(value, expected, &format!("{path}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

impl Simulator {
    pub fn checkpoint(&self) -> Result<Checkpoint, SimError> {
        let n = self.snakes.len();
        for (field, actual) in [
            ("boost", self.boosts.len()),
            ("speed", self.speeds.len()),
            ("kiosk", self.kiosks.len()),
            ("pitEntered", self.pit_entered.len()),
            ("deathTime", self.death_times.len()),
            ("join", self.joins.len()),
            ("killTimes", self.kill_times.len()),
        ] {
            require(
                actual == n,
                Constraint::LengthMismatch,
                &format!("state.slots.{field}"),
            )?;
        }
        let ids: HashSet<_> = self.prey.iter().map(|p| p.id).collect();
        require(
            self.prey_decision_times.keys().all(|id| ids.contains(id)),
            Constraint::InconsistentState,
            "state.prey.nextDecision",
        )?;
        require(
            self.prey_outward.iter().all(|id| ids.contains(id)),
            Constraint::InconsistentState,
            "state.prey.mode",
        )?;
        // Validate Config before its JSON serialization can turn NaN into null.
        self.config.validate().map_err(checkpoint_config_error)?;
        let config = self.config.clone();
        let mut slots = Vec::new();
        reserve(&mut slots, n, "state.slots")?;
        for (i, s) in self.snakes.iter().enumerate() {
            let b = &self.boosts[i];
            let k = &self.kiosks[i];
            slots.push(Slot {
                snake: SnakeRecord {
                    id: s.id.clone(),
                    head: s.head.into(),
                    body: collect(&s.body, "body", |&p| p.into())?,
                    heading: s.heading,
                    speed: s.speed,
                    radius: s.radius,
                    length: s.length,
                    segments: s.segments,
                    fam: s.fam,
                    score: s.score,
                    kills: s.kills,
                    alive: s.alive,
                },
                boost: BoostRecord {
                    held: b.held,
                    active: b.active,
                    exhausted: b.exhausted,
                    clock: b.clock,
                    active_boost_seconds: b.active_boost_seconds,
                    boost_mass_spent: b.boost_mass_spent,
                },
                speed: SpeedRecord {
                    speed: self.speeds[i].speed,
                    progress: self.speeds[i].progress(),
                },
                kiosk: KioskRecord {
                    anchor: k.anchor.map(Into::into),
                    held_seconds: k.held_seconds,
                    score_credit: k.score_credit,
                },
                pit_entered: self.pit_entered[i],
                death_time: self.death_times[i],
                join: self.joins[i],
                kill_times: self.kill_times[i].iter().copied().collect(),
            });
        }
        let state = State {
            time: self.time,
            radius: self.radius,
            radius_update_index: self.radius_update_index,
            next_join: self.next_join,
            random: self.random.0,
            prey_random: self.prey_random.0,
            next_prey_id: self.next_prey_id,
            ambient_clock: self.ambient_clock,
            prey_clock: self.prey_clock,
            prey_motion_clock: self.prey_motion_clock,
            min_mass: self.min_mass.map(|f| FloorRecord {
                segments: f.segments,
                fam: f.fam,
            }),
            failure: self.failure.clone(),
            slots,
            food: collect(&self.food, "state.food", |f| FoodRecord {
                x: f.x,
                y: f.y,
                value: f.value,
                size: f.size,
                origin: f.origin,
            })?,
            prey: collect(&self.prey, "state.prey", |p| PreyRecord {
                id: p.id,
                position: p.position.into(),
                heading: p.heading,
                target_heading: p.target_heading,
                speed: p.speed,
                size: p.size,
                next_decision: self.prey_decision_times.get(&p.id).copied(),
                mode: if self.prey_outward.contains(&p.id) {
                    PreyMode::Outward
                } else {
                    PreyMode::Roaming
                },
            })?,
            deaths: collect(&self.deaths, "state.deaths", |d| DeathRecord {
                time: d.time,
                victim: d.victim.clone(),
                killer: d.killer.clone(),
                reason: d.reason,
                food: d.food,
            })?,
        };
        state.validate(&self.config)?;
        Ok(Checkpoint(Envelope {
            format: FORMAT.into(),
            rules_revision: RULES_REVISION.into(),
            producer: Producer {
                version: env!("CARGO_PKG_VERSION").into(),
                commit: None,
            },
            config,
            state,
        }))
    }

    pub fn from_checkpoint(bytes: &[u8], options: RestoreOptions) -> Result<Self, SimError> {
        let raw: Value = decode(bytes)?;
        let header = |field: &str| -> Result<String, SimError> {
            let value = raw
                .get(field)
                .ok_or_else(|| invalid(Constraint::Missing, field, "Missing checkpoint header"))?;
            value.as_str().map(str::to_owned).ok_or_else(|| {
                invalid(
                    Constraint::InconsistentState,
                    field,
                    "Checkpoint header must be a string",
                )
            })
        };
        let format = header("format")?;
        let rules_revision = header("rulesRevision")?;
        if format != FORMAT || rules_revision != RULES_REVISION {
            let field = if format != FORMAT {
                "format"
            } else {
                "rulesRevision"
            };
            return Err(SimError::new(
                ErrorKind::UnsupportedCheckpoint {
                    format,
                    rules_revision,
                },
                "Unsupported checkpoint format or rules revision",
            )
            .at_field(field));
        }
        // JSON arrays have no length prefix: Serde allocates from actual encoded
        // elements, never from a supplied segment/population count. Its recursion
        // limit remains enabled. Runtime capacity arithmetic is checked below.
        state_schema(&raw)?;
        let envelope: Envelope = decode(bytes)?;
        envelope
            .config
            .validate()
            .map_err(checkpoint_config_error)?;
        let canonical = ConfigRecord::serialize(&envelope.config, serde_json::value::Serializer)
            .map_err(|e| invalid(Constraint::InconsistentState, "config", e.to_string()))?;
        config_shape(&raw["config"], &canonical, "config")?;
        envelope.state.validate(&envelope.config)?;
        envelope.state.into_world(envelope.config, options)
    }

    /// Build and validate a complete candidate before replacing the incumbent.
    pub fn restore(&mut self, bytes: &[u8], options: RestoreOptions) -> Result<(), SimError> {
        let candidate = Self::from_checkpoint(bytes, options)?;
        *self = candidate;
        Ok(())
    }
}

fn checkpoint_config_error(error: SimError) -> SimError {
    let rule = match error.kind {
        ErrorKind::InvalidConfig { rule } => rule,
        _ => Constraint::InconsistentState,
    };
    invalid(
        rule,
        error
            .field
            .as_deref()
            .map_or("config".into(), |field| format!("config.{field}")),
        error.to_string(),
    )
}

impl State {
    fn validate(&self, config: &Config) -> Result<(), SimError> {
        nonnegative(self.time, "state.time")?;
        finite(self.time + config.dt, "state.time")?;
        positive(self.radius, "state.radius")?;
        bounded(self.radius, MAGNITUDE, "state.radius")?;
        for (i, update) in config.radius_updates.iter().enumerate() {
            let field = format!("config.radiusUpdates[{i}].radius");
            bounded(update.radius, MAGNITUDE, &field)?;
        }
        let through = if self.time == 0. {
            0.
        } else {
            self.time + 1e-9
        };
        let cursor = config.radius_updates.partition_point(|u| u.time <= through);
        require(
            self.radius_update_index == cursor,
            Constraint::Ordering,
            "state.radiusUpdateIndex",
        )?;
        if let Some(update) = cursor.checked_sub(1).map(|i| &config.radius_updates[i]) {
            let this_tick = self.time == 0. || update.time > (self.time - config.dt).max(0.) + 1e-9;
            if !config.dynamic_radius || this_tick {
                require(
                    self.radius == update.radius,
                    Constraint::InconsistentState,
                    "state.radius",
                )?;
            }
        } else if !config.dynamic_radius || self.time == 0. {
            // Only the controller and recorded updates move the live radius,
            // and the controller first runs in a step.
            require(
                self.radius == config.radius,
                Constraint::InconsistentState,
                "state.radius",
            )?;
        }
        nonnegative(self.prey_clock, "state.preyClock")?;
        require(
            self.prey_clock < config.prey.respawn_seconds,
            Constraint::OutOfRange,
            "state.preyClock",
        )?;
        finite(self.prey_clock + config.dt, "state.preyClock")?;
        nonnegative(self.prey_motion_clock, "state.preyMotionClock")?;
        // At the largest supported batched dt, subtracting the rounded substep
        // product can leave more than one period by a few ulps of dt. Keep the
        // actual finite phase, including that roundoff; ordinary ticks remain
        // strictly below a period. tests/checkpoint.rs pins the phase boundaries.
        let roundoff = config.dt * f64::EPSILON * 4.;
        let phase_valid = if roundoff <= 1e-12 {
            self.prey_motion_clock < LOOP_SECONDS
        } else {
            self.prey_motion_clock <= LOOP_SECONDS + roundoff
        };
        require(phase_valid, Constraint::OutOfRange, "state.preyMotionClock")?;
        finite(self.ambient_clock, "state.ambientClock")?;
        finite(self.ambient_clock + config.dt, "state.ambientClock")?;
        let rate = config.ambient_replenish_per_second;
        if rate == 0. {
            require(
                self.ambient_clock == 0.,
                Constraint::InconsistentState,
                "state.ambientClock",
            )?;
        } else {
            let credit = self.ambient_clock * rate;
            finite(credit, "state.ambientClock")?;
            // The 1e-12 s refill tolerance can spend one interval early; rate
            // rescaling preserves that signed fractional credit, not seconds.
            // Tiny dt can round an early subtraction to exactly -interval;
            // multiplication/rescaling can likewise round credit to an endpoint.
            require(
                (-1. ..=1.).contains(&credit),
                Constraint::OutOfRange,
                "state.ambientClock",
            )?;
        }
        let expected_floor = config.min_score.and_then(MassFloor::for_score);
        require(
            self.min_mass.map(|f| (f.segments, f.fam))
                == expected_floor.map(|f| (f.segments, f.fam)),
            Constraint::InconsistentState,
            "state.minMass",
        )?;
        let mut ids = HashSet::new();
        let mut joins = HashSet::new();
        for (i, slot) in self.slots.iter().enumerate() {
            slot.validate(config, self.time, expected_floor)
                .map_err(|mut e| {
                    e.field = e.field.map(|field| format!("state.slots[{i}].{field}"));
                    e.at_slot(i)
                })?;
            require(
                ids.insert(slot.snake.id.as_str()),
                Constraint::Duplicate,
                &format!("state.slots[{i}].snake.id"),
            )
            .map_err(|e| e.at_slot(i))?;
            require(
                joins.insert(slot.join),
                Constraint::Duplicate,
                &format!("state.slots[{i}].join"),
            )
            .map_err(|e| e.at_slot(i))?;
            require(
                slot.join < self.next_join,
                Constraint::Ordering,
                "state.nextJoin",
            )?;
        }
        require(
            self.next_join < u64::MAX,
            Constraint::OutOfRange,
            "state.nextJoin",
        )?;
        let mut last = 0.;
        for (i, death) in self.deaths.iter().enumerate() {
            let path = format!("state.deaths[{i}]");
            timestamp(death.time, self.time, &format!("{path}.time"))?;
            require(
                death.time >= last,
                Constraint::Ordering,
                &format!("{path}.time"),
            )?;
            last = death.time;
            require(
                ids.contains(death.victim.as_str()),
                Constraint::InconsistentState,
                &format!("{path}.victim"),
            )?;
            if let Some(killer) = &death.killer {
                require(
                    ids.contains(killer.as_str()),
                    Constraint::InconsistentState,
                    &format!("{path}.killer"),
                )?;
            }
            if death.reason == DeathReason::Pit {
                require(
                    config.pit.is_some(),
                    Constraint::InconsistentState,
                    &format!("{path}.reason"),
                )?;
            }
        }
        // The death log is never trimmed: index each victim's latest death once.
        let mut last_death = HashMap::new();
        for death in &self.deaths {
            last_death.insert(death.victim.as_str(), death.time);
        }
        for (i, slot) in self.slots.iter().enumerate() {
            if let Some(time) = slot.death_time {
                require(
                    last_death.get(slot.snake.id.as_str()) == Some(&time),
                    Constraint::InconsistentState,
                    &format!("state.slots[{i}].deathTime"),
                )?;
            }
        }
        for (i, food) in self.food.iter().enumerate() {
            let path = format!("state.food[{i}]");
            bounded(food.x, DEAD, &format!("{path}.x"))?;
            bounded(food.y, DEAD, &format!("{path}.y"))?;
            nonnegative(food.value, &format!("{path}.value"))?;
            if let Some(size) = food.size {
                nonnegative(size, &format!("{path}.size"))?;
            }
        }
        let mut prey_ids = HashSet::new();
        for (i, prey) in self.prey.iter().enumerate() {
            let path = format!("state.prey[{i}]");
            require(
                prey_ids.insert(prey.id),
                Constraint::Duplicate,
                &format!("{path}.id"),
            )?;
            // Prey motion has its own stopped path for overflow.
            finite(prey.position.x, &format!("{path}.position.x"))?;
            finite(prey.position.y, &format!("{path}.position.y"))?;
            bounded(prey.heading, MAGNITUDE, &format!("{path}.heading"))?;
            let field = format!("{path}.targetHeading");
            bounded(prey.target_heading, MAGNITUDE, &field)?;
            nonnegative(prey.speed, &format!("{path}.speed"))?;
            positive(prey.size, &format!("{path}.size"))?;
            finite(prey.speed * LOOP_SECONDS, &format!("{path}.speed"))?;
            // This is the existing per-catch bound, not a cap on resumed body
            // mass. Accumulated gains can still reach the core stopped path.
            finite(
                nutrition(prey.size, 2.) * config.prey.nutrition_factor + MAX_SEGMENTS as f64,
                &format!("{path}.size"),
            )?;
            if let Some(time) = prey.next_decision {
                nonnegative(time, &format!("{path}.nextDecision"))?;
            }
        }
        if let Some(cause) = &self.failure {
            require(
                matches!(cause.kind, ErrorKind::NumericOverflow),
                Constraint::InconsistentState,
                "state.failure.kind",
            )?;
            require(
                cause.world.is_none(),
                Constraint::InconsistentState,
                "state.failure.world",
            )?;
            require(
                cause.slot.is_none_or(|i| i < self.slots.len()),
                Constraint::OutOfRange,
                "state.failure.slot",
            )?;
        }
        Ok(())
    }

    fn into_world(self, config: Config, options: RestoreOptions) -> Result<Simulator, SimError> {
        let mut snakes = Vec::new();
        let mut boosts = Vec::new();
        let mut speeds = Vec::new();
        let mut kiosks = Vec::new();
        let mut pit_entered = Vec::new();
        let mut death_times = Vec::new();
        let mut joins = Vec::new();
        let mut kill_times = Vec::new();
        let n = self.slots.len();
        reserve(&mut snakes, n, "state.slots")?;
        reserve(&mut boosts, n, "state.slots.boost")?;
        reserve(&mut speeds, n, "state.slots.speed")?;
        reserve(&mut kiosks, n, "state.slots.kiosk")?;
        reserve(&mut pit_entered, n, "state.slots.pitEntered")?;
        reserve(&mut death_times, n, "state.slots.deathTime")?;
        reserve(&mut joins, n, "state.slots.join")?;
        reserve(&mut kill_times, n, "state.slots.killTimes")?;
        for slot in self.slots {
            let s = slot.snake;
            let b = slot.boost;
            let k = slot.kiosk;
            let body = collect(&s.body, "state.slots.snake.body", |&p| p.into())?;
            snakes.push(Snake {
                id: s.id,
                head: s.head.into(),
                body,
                heading: s.heading,
                speed: s.speed,
                radius: s.radius,
                length: s.length,
                segments: s.segments,
                fam: s.fam,
                score: s.score,
                kills: s.kills,
                alive: s.alive,
            });
            boosts.push(Boost {
                held: b.held,
                active: b.active,
                exhausted: b.exhausted,
                clock: b.clock,
                active_boost_seconds: b.active_boost_seconds,
                boost_mass_spent: b.boost_mass_spent,
            });
            speeds.push(
                SpeedState::with_progress(slot.speed.speed, slot.speed.progress)
                    .expect("validated speed"),
            );
            kiosks.push(KioskState {
                anchor: k.anchor.map(Into::into),
                held_seconds: k.held_seconds,
                score_credit: k.score_credit,
            });
            pit_entered.push(slot.pit_entered);
            death_times.push(slot.death_time);
            joins.push(slot.join);
            let mut times = VecDeque::new();
            times
                .try_reserve_exact(slot.kill_times.len())
                .map_err(|_| {
                    invalid(
                        Constraint::OutOfRange,
                        "state.slots.killTimes",
                        "Checkpoint history cannot be reserved",
                    )
                })?;
            times.extend(slot.kill_times);
            kill_times.push(times);
        }
        let mut prey_decision_times = HashMap::new();
        let mut prey_outward = HashSet::new();
        prey_decision_times
            .try_reserve(self.prey.len())
            .map_err(|_| {
                invalid(
                    Constraint::OutOfRange,
                    "state.prey",
                    "Checkpoint decisions cannot be reserved",
                )
            })?;
        prey_outward.try_reserve(self.prey.len()).map_err(|_| {
            invalid(
                Constraint::OutOfRange,
                "state.prey",
                "Checkpoint modes cannot be reserved",
            )
        })?;
        let prey = collect(&self.prey, "state.prey", |p| {
            if let Some(time) = p.next_decision {
                prey_decision_times.insert(p.id, time);
            }
            if p.mode == PreyMode::Outward {
                prey_outward.insert(p.id);
            }
            Prey {
                id: p.id,
                position: p.position.into(),
                heading: p.heading,
                target_heading: p.target_heading,
                speed: p.speed,
                size: p.size,
            }
        })?;
        let food = collect(&self.food, "state.food", |f| Food {
            x: f.x,
            y: f.y,
            value: f.value,
            size: f.size,
            origin: f.origin,
        })?;
        let deaths = collect(&self.deaths, "state.deaths", |d| DeathEvent {
            time: d.time,
            victim: d.victim.clone(),
            killer: d.killer.clone(),
            reason: d.reason,
            food: d.food,
        })?;
        Ok(Simulator {
            config,
            radius: self.radius,
            snakes,
            food,
            prey,
            deaths,
            time: self.time,
            pickup_events: options.events.pickups.then(Vec::new),
            food_spawn_events: options.events.food_births.then(Vec::new),
            death_times,
            joins,
            next_join: self.next_join,
            random: Random(self.random),
            boosts,
            speeds,
            kiosks,
            pit_entered,
            kill_times,
            min_mass: self.min_mass.map(|f| MassFloor {
                segments: f.segments,
                fam: f.fam,
            }),
            ambient_clock: self.ambient_clock,
            prey_random: Random(self.prey_random),
            next_prey_id: self.next_prey_id,
            prey_clock: self.prey_clock,
            prey_decision_times,
            prey_outward,
            prey_motion_clock: self.prey_motion_clock,
            radius_update_index: self.radius_update_index,
            failure: self.failure,
            food_cache: FoodCache::default(),
            eater_grid: EaterGrid::default(),
            buffers: StepBuffers::default(),
            #[cfg(feature = "parallel")]
            parallel: options.parallel,
        })
    }
}

impl Slot {
    fn validate(
        &self,
        config: &Config,
        time: f64,
        floor: Option<MassFloor>,
    ) -> Result<(), SimError> {
        let s = &self.snake;
        require(
            s.body.len() == s.segments,
            Constraint::LengthMismatch,
            "snake.body",
        )?;
        require(
            s.segments >= 2 && s.segments.checked_add(1).is_some(),
            Constraint::OutOfRange,
            "snake.segments",
        )?;
        let limit = if s.alive { LIVING } else { DEAD };
        position(s.head, limit, "snake.head")?;
        for (i, &p) in s.body.iter().enumerate() {
            position(p, limit, &format!("snake.body[{i}]"))?;
        }
        bounded(s.heading, MAGNITUDE, "snake.heading")?;
        nonnegative(s.speed, "snake.speed")?;
        bounded(s.speed * config.dt, MAGNITUDE, "snake.speed")?;
        nonnegative(s.fam, "snake.fam")?;
        finite(s.segments as f64 + s.fam, "snake.fam")?;
        nonnegative(self.speed.speed, "speed.speed")?;
        bounded(self.speed.speed * config.dt, MAGNITUDE, "speed.speed")?;
        finite(self.speed.progress, "speed.progress")?;
        require(
            (0. ..=1.).contains(&self.speed.progress),
            Constraint::OutOfRange,
            "speed.progress",
        )?;
        if let Some(f) = floor {
            require(
                (s.segments, s.fam) >= (f.segments, f.fam),
                Constraint::InconsistentState,
                "snake.fam",
            )?;
        }
        if let Some(p) = self.kiosk.anchor {
            position(p, DEAD, "kiosk.anchor")?;
        }
        nonnegative(self.kiosk.held_seconds, "kiosk.heldSeconds")?;
        nonnegative(self.kiosk.score_credit, "kiosk.scoreCredit")?;
        require(
            self.kiosk.anchor.is_some() || self.kiosk.held_seconds == 0.,
            Constraint::InconsistentState,
            "kiosk.anchor",
        )?;
        finite(self.kiosk.held_seconds + config.dt, "kiosk.heldSeconds")?;
        let mut derived = Snake {
            id: String::new(),
            head: Point::default(),
            body: Vec::new(),
            heading: 0.,
            speed: 0.,
            radius: 0.,
            length: 0.,
            segments: s.segments,
            fam: s.fam,
            score: 0.,
            kills: 0,
            alive: false,
        };
        Simulator::update_mass(&mut derived);
        KioskState {
            anchor: None,
            held_seconds: 0.,
            score_credit: self.kiosk.score_credit,
        }
        .finish_score(&mut derived);
        for (field, actual, expected) in [
            ("snake.radius", s.radius, derived.radius),
            ("snake.length", s.length, derived.length),
            ("snake.score", s.score, derived.score),
        ] {
            finite(actual, field)?;
            finite(expected, field)?;
            require(actual == expected, Constraint::InconsistentState, field)?;
        }
        let b = self.boost;
        nonnegative(b.clock, "boost.clock")?;
        nonnegative(b.active_boost_seconds, "boost.activeBoostSeconds")?;
        nonnegative(b.boost_mass_spent, "boost.boostMassSpent")?;
        finite(
            b.active_boost_seconds + config.dt,
            "boost.activeBoostSeconds",
        )?;
        require(
            !b.active || b.held && !b.exhausted,
            Constraint::InconsistentState,
            "boost.active",
        )?;
        require(
            !b.exhausted || b.held && !b.active,
            Constraint::InconsistentState,
            "boost.exhausted",
        )?;
        // Exhaustion can retain the undropped remainder of a batched tick,
        // rather than a sub-period phase; advance() resets it on the next tick.
        require(
            if b.active {
                b.clock < BOOST_DROP_SECONDS
            } else if b.exhausted {
                b.clock <= config.dt + BOOST_DROP_SECONDS
            } else {
                b.clock == 0.
            },
            Constraint::OutOfRange,
            "boost.clock",
        )?;
        require(
            !self.pit_entered || config.pit.is_some(),
            Constraint::InconsistentState,
            "pitEntered",
        )?;
        if let Some(death) = self.death_time {
            timestamp(death, time, "deathTime")?;
            require(!s.alive, Constraint::InconsistentState, "deathTime")?;
        }
        require(
            self.kill_times.is_empty() || config.kill_score_seconds.is_some(),
            Constraint::InconsistentState,
            "killTimes",
        )?;
        let mut previous = 0.;
        for &kill in &self.kill_times {
            timestamp(kill, time, "killTimes")?;
            require(kill >= previous, Constraint::Ordering, "killTimes")?;
            previous = kill;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_rejects_each_short_runtime_companion_before_indexing() {
        let mut source = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            ..Default::default()
        })
        .unwrap();
        source.push_ring("a".into(), 0, 1).unwrap();
        for shorten in [
            (|w: &mut Simulator| w.boosts.clear()) as fn(&mut Simulator),
            |w| w.speeds.clear(),
            |w| w.kiosks.clear(),
            |w| w.pit_entered.clear(),
            |w| w.death_times.clear(),
            |w| w.joins.clear(),
            |w| w.kill_times.clear(),
        ] {
            let mut bad = source.clone();
            shorten(&mut bad);
            assert_eq!(
                bad.checkpoint().unwrap_err().kind,
                ErrorKind::InvalidCheckpoint {
                    rule: Constraint::LengthMismatch
                }
            );
        }
    }
}
