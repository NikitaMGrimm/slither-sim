//! Replay recorded controls and food through the production simulator.
//!
//! cargo run --locked --example fidelity_replay -- fixture.json free > free.jsonl
//! cargo run --locked --example fidelity_replay -- fixture.json motion_only > motion.jsonl
//!
//! This is a counterfactual replay: it never replaces the simulated head, heading,
//! body, or speed with later observations. `motion_only` replaces mass at recorded
//! mass events so pickup/drop errors do not change the subsequent turn rate.

use serde::Deserialize;
use serde_json::{Value, json};
use slither_sim::{Action, Config, Food, PickupItem, Point, Simulator, Snake};
use std::collections::{HashMap, HashSet};
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, BufWriter, Write};

const DT: f64 = 0.007;
const EPS: f64 = 1e-9;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: u32,
    #[serde(default)]
    name: String,
    #[serde(default)]
    provenance: Value,
    #[serde(default = "default_seed")]
    seed: u32,
    duration_seconds: f64,
    initial: InitialState,
    #[serde(default)]
    controls: Vec<Control>,
    #[serde(default)]
    food_events: Vec<FoodEvent>,
    #[serde(default)]
    mass_events: Vec<MassEvent>,
    #[serde(default)]
    observations: Vec<Observed>,
    #[serde(default)]
    expected_pickups: Option<Vec<ExpectedPickup>>,
    #[serde(default)]
    options: Options,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InitialState {
    snake: Snake,
    action: Action,
    #[serde(default)]
    food: Vec<NamedFood>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedPickup {
    time: f64,
    id: String,
}

fn default_seed() -> u32 {
    1
}
fn default_radius() -> f64 {
    50_000.0
}
fn default_sample_interval() -> f64 {
    0.05
}
fn default_head_tolerance() -> f64 {
    8.0
}
fn default_anchor_tolerance() -> f64 {
    6.0
}
fn default_speed_tolerance() -> f64 {
    20.0
}
fn default_mass_tolerance() -> f64 {
    0.03
}
fn default_pickup_time_tolerance() -> f64 {
    0.1
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NamedFood {
    id: String,
    food: Food,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Control {
    time: f64,
    action: Action,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum FoodEventKind {
    Add,
    Remove,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FoodEvent {
    time: f64,
    kind: FoodEventKind,
    id: String,
    #[serde(default)]
    food: Option<Food>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MassEvent {
    time: f64,
    segments: usize,
    fam: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observed {
    time: f64,
    #[serde(default)]
    anchor_index: Option<usize>,
    #[serde(default)]
    anchor: Option<Point>,
    #[serde(default)]
    head: Option<Point>,
    #[serde(default)]
    heading: Option<f64>,
    #[serde(default)]
    heading_source: Option<String>,
    #[serde(default)]
    speed: Option<f64>,
    #[serde(default)]
    segments: Option<usize>,
    #[serde(default)]
    fam: Option<f64>,
    #[serde(default)]
    boosting: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DropMode {
    #[default]
    Simulated,
    Recorded,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    #[serde(default)]
    boost_drops: DropMode,
    #[serde(default = "default_radius")]
    radius: f64,
    #[serde(default)]
    input_time_offset_ms: f64,
    #[serde(default = "default_sample_interval")]
    sample_interval_seconds: f64,
    #[serde(default = "default_head_tolerance")]
    head_tolerance: f64,
    #[serde(default = "default_anchor_tolerance")]
    anchor_tolerance: f64,
    #[serde(default = "default_speed_tolerance")]
    speed_tolerance: f64,
    #[serde(default = "default_mass_tolerance")]
    mass_tolerance: f64,
    #[serde(default = "default_pickup_time_tolerance")]
    pickup_time_tolerance: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            boost_drops: DropMode::default(),
            radius: default_radius(),
            input_time_offset_ms: 0.0,
            sample_interval_seconds: default_sample_interval(),
            head_tolerance: default_head_tolerance(),
            anchor_tolerance: default_anchor_tolerance(),
            speed_tolerance: default_speed_tolerance(),
            mass_tolerance: default_mass_tolerance(),
            pickup_time_tolerance: default_pickup_time_tolerance(),
        }
    }
}

struct AnchorEvent {
    index: usize,
    time: f64,
    point: Point,
}
struct EatEvent {
    time: f64,
    id: String,
}

/// Restore body length after a mass intervention by extending simulated tail
/// geometry. This does not copy recorded positions, but motion_only body and
/// drop locations are conditional on these synthetic tail anchors.
fn align_tail_to_segments(snake: &mut Snake) -> usize {
    let mut appended = 0;
    snake.body.truncate(snake.segments);
    while snake.body.len() < snake.segments {
        let tail = *snake.body.last().expect("validated nonempty body");
        let previous = if snake.body.len() > 1 {
            snake.body[snake.body.len() - 2]
        } else {
            snake.head
        };
        let dx = tail.x - previous.x;
        let dy = tail.y - previous.y;
        let length = dx.hypot(dy);
        let (ux, uy) = if length > 1e-9 {
            (dx / length, dy / length)
        } else {
            (-snake.heading.cos(), -snake.heading.sin())
        };
        snake.body.push(Point {
            x: tail.x + ux * 42.0,
            y: tail.y + uy * 42.0,
        });
        appended += 1;
    }
    appended
}

fn fail(message: impl Into<String>) -> Box<dyn Error> {
    message.into().into()
}

fn ordered_times<'a>(
    label: &str,
    times: impl Iterator<Item = &'a f64>,
    duration: f64,
) -> Result<(), Box<dyn Error>> {
    let mut last = -EPS;
    for &time in times {
        if !time.is_finite() || time < 0.0 || time > duration + EPS || time + EPS < last {
            return Err(fail(format!(
                "{label} times must be finite, sorted, and within replay duration"
            )));
        }
        last = time;
    }
    Ok(())
}

fn validate(f: &Fixture) -> Result<(), Box<dyn Error>> {
    if f.schema_version != 1 {
        return Err(fail("unsupported replay schema_version"));
    }
    if !f.duration_seconds.is_finite() || f.duration_seconds <= 0.0 {
        return Err(fail("invalid duration_seconds"));
    }
    let initial = &f.initial.snake;
    if !initial.alive
        || initial.body.is_empty()
        || initial.segments < 2
        || !initial.head.finite()
        || !initial.heading.is_finite()
        || !initial.speed.is_finite()
        || !initial.fam.is_finite()
        || initial.fam < 0.0
        || initial.body.iter().any(|point| !point.finite())
    {
        return Err(fail(
            "invalid initial snake; body must be active anchors in head-to-tail order",
        ));
    }
    if initial.body.len() != initial.segments {
        return Err(fail(
            "initial body anchor count must equal segments; remove dying anchors before replay",
        ));
    }
    if !f.initial.action.heading.is_finite() {
        return Err(fail("invalid initial action heading"));
    }
    ordered_times(
        "control",
        f.controls.iter().map(|e| &e.time),
        f.duration_seconds,
    )?;
    ordered_times(
        "food event",
        f.food_events.iter().map(|e| &e.time),
        f.duration_seconds,
    )?;
    ordered_times(
        "mass event",
        f.mass_events.iter().map(|e| &e.time),
        f.duration_seconds,
    )?;
    ordered_times(
        "observation",
        f.observations.iter().map(|e| &e.time),
        f.duration_seconds,
    )?;
    if let Some(expected) = &f.expected_pickups {
        ordered_times(
            "expected pickup",
            expected.iter().map(|e| &e.time),
            f.duration_seconds,
        )?;
    }
    if f.controls.iter().any(|e| !e.action.heading.is_finite()) {
        return Err(fail("invalid control heading"));
    }
    if f.mass_events
        .iter()
        .any(|e| e.segments < 2 || !e.fam.is_finite() || e.fam < 0.0)
    {
        return Err(fail("invalid mass event"));
    }
    for o in &f.observations {
        if o.anchor.is_some() != o.anchor_index.is_some()
            || o.anchor_index == Some(0)
            || o.anchor.is_some_and(|point| !point.finite())
            || o.head.is_some_and(|point| !point.finite())
            || o.heading.is_some_and(|value| !value.is_finite())
            || o.speed.is_some_and(|value| !value.is_finite())
            || o.fam.is_some_and(|value| !value.is_finite())
        {
            return Err(fail("invalid observation"));
        }
    }
    let options = &f.options;
    if !options.radius.is_finite()
        || options.radius <= 0.0
        || !options.input_time_offset_ms.is_finite()
        || !options.sample_interval_seconds.is_finite()
        || options.sample_interval_seconds <= 0.0
        || [
            options.head_tolerance,
            options.anchor_tolerance,
            options.speed_tolerance,
            options.mass_tolerance,
            options.pickup_time_tolerance,
        ]
        .iter()
        .any(|x| !x.is_finite() || *x < 0.0)
    {
        return Err(fail("invalid replay options"));
    }
    let mut ids = HashSet::new();
    for named in &f.initial.food {
        if !ids.insert(named.id.as_str()) || !named.food.point().finite() {
            return Err(fail("duplicate initial food ID or invalid food position"));
        }
    }
    for event in &f.food_events {
        match event.kind {
            FoodEventKind::Add if event.food.is_none() => {
                return Err(fail("food add requires food"));
            }
            FoodEventKind::Remove if event.food.is_some() => {
                return Err(fail("food remove must omit food"));
            }
            _ => {}
        }
        if matches!(event.reason.as_deref(), Some("self_pickup" | "self-pickup")) {
            return Err(fail(
                "recorded self pickups must be expected pickups, never forced food removals",
            ));
        }
    }
    Ok(())
}

fn write(out: &mut impl Write, value: Value) -> io::Result<()> {
    serde_json::to_writer(&mut *out, &value)?;
    out.write_all(b"\n")
}

fn angle_error(a: f64, b: f64) -> f64 {
    ((a - b + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI).abs()
}

fn divergence(
    first: &mut HashMap<&'static str, Value>,
    key: &'static str,
    time: f64,
    error: f64,
    tolerance: f64,
) {
    if error > tolerance {
        first.entry(key).or_insert_with(
            || json!({"metric":key,"time":time,"error":error,"tolerance":tolerance}),
        );
    }
}

fn run(f: Fixture, mode: &str, mut out: impl Write) -> Result<(), Box<dyn Error>> {
    validate(&f)?;
    if mode != "free" && mode != "motion_only" {
        return Err(fail("mode must be free or motion_only"));
    }
    let mut config = Config {
        radius: f.options.radius,
        food_count: 0,
        ambient_replenish_per_second: 0.0,
        dt: DT,
        ..Config::sandbox()
    };
    config.prey.count = 0;
    config.kiosk.enabled = false;
    config.death_food.enabled = false;
    config.ambient_food.enabled = false;
    config.boost_food.enabled = false;
    config.emit_boost_food = matches!(f.options.boost_drops, DropMode::Simulated);
    let speed = f.initial.snake.speed;
    let mut world = Simulator::from_scenario(
        f.seed,
        config,
        slither_sim::Scenario {
            snakes: vec![(
                f.initial.snake.id.clone(),
                slither_sim::ReplayLife::from_observed(
                    f.initial.snake,
                    slither_sim::InitialSpeed::Infer(speed),
                ),
            )],
            food: f
                .initial
                .food
                .iter()
                .map(|named| named.food.clone())
                .collect(),
            prey: Vec::new(),
        },
    )?;
    world.record_events(slither_sim::EventRecording {
        pickups: true,
        food_births: false,
    });

    write(
        &mut out,
        json!({"kind":"meta","schema_version":1,"name":f.name,"mode":mode,
        "dt":DT,"seed":f.seed,"provenance":f.provenance,
        "boost_drops":if matches!(f.options.boost_drops, DropMode::Recorded) {"recorded"} else {"simulated"},
        "input_time_offset_ms":f.options.input_time_offset_ms,
        "limits":["client pose and input receipt may not equal server state/time",
            "hidden initial boost phase and RNG are not reconstructed",
            "no opponent movement, competing consumption or prey unless encoded as food events"]}),
    )?;
    let mut known: Vec<(String, Food)> =
        f.initial.food.into_iter().map(|n| (n.id, n.food)).collect();
    let mut action = f.initial.action;
    let (mut control_i, mut food_i, mut mass_i, mut observation_i) = (0, 0, 0, 0);
    let (mut next_sample, mut generated_id, mut anchor_index) = (0.0, 0usize, 0usize);
    let mut first = HashMap::new();
    // Targets belong to the capture, including those after a simulated death.
    let observed_anchors: Vec<_> = f
        .observations
        .iter()
        .filter_map(|o| {
            o.anchor_index
                .zip(o.anchor)
                .map(|(index, point)| AnchorEvent {
                    index,
                    time: o.time,
                    point,
                })
        })
        .collect();
    let mut simulated_anchors = Vec::new();
    let mut observed_eats = Vec::new();
    let mut simulated_eats = Vec::new();
    let mut external_food_events = 0usize;
    let offset = f.options.input_time_offset_ms / 1000.0;

    while world.time() + EPS < f.duration_seconds && world.snakes()[0].alive {
        while control_i < f.controls.len()
            && f.controls[control_i].time + offset <= world.time() + EPS
        {
            action = f.controls[control_i].action;
            control_i += 1;
        }
        while food_i < f.food_events.len() && f.food_events[food_i].time <= world.time() + EPS {
            let event = &f.food_events[food_i];
            match event.kind {
                FoodEventKind::Add => {
                    if known.iter().any(|(id, _)| id == &event.id) {
                        return Err(fail(format!("food ID already live: {}", event.id)));
                    }
                    let food = event.food.as_ref().expect("validated add food").clone();
                    if !food.point().finite() {
                        return Err(fail("invalid added food position"));
                    }
                    world.insert_food(food.clone())?;
                    known.push((event.id.clone(), food));
                    write(
                        &mut out,
                        json!({"kind":"food_event","time":world.time(),"event":"add","id":event.id,"reason":event.reason}),
                    )?;
                }
                FoodEventKind::Remove => {
                    if let Some(index) = known.iter().position(|(id, _)| id == &event.id) {
                        let (_, food) = known.remove(index);
                        if let Some(index) =
                            world.food().iter().position(|candidate| *candidate == food)
                        {
                            world.remove_food(index)?;
                        } else {
                            return Err(fail(format!(
                                "food ID present in sidecar but not simulator: {}",
                                event.id
                            )));
                        }
                        write(
                            &mut out,
                            json!({"kind":"food_event","time":world.time(),"event":"remove","id":event.id,"reason":event.reason}),
                        )?;
                    } else {
                        write(
                            &mut out,
                            json!({"kind":"food_event","time":world.time(),"event":"remove_missing","id":event.id,"reason":event.reason}),
                        )?;
                    }
                }
            }
            external_food_events += 1;
            food_i += 1;
        }
        if mode == "motion_only" {
            while mass_i < f.mass_events.len() && f.mass_events[mass_i].time <= world.time() + EPS {
                let event = &f.mass_events[mass_i];
                let mut corrected = world.snakes()[0].clone();
                corrected.segments = event.segments;
                corrected.fam = event.fam;
                let tail_anchors_synthesized = align_tail_to_segments(&mut corrected);
                world.correct_mass(0, corrected.body, corrected.fam)?;
                write(
                    &mut out,
                    json!({"kind":"intervention","time":world.time(),
                    "source_time":event.time,"field":"mass","segments":event.segments,
                    "fam":event.fam,"tail_anchors_synthesized":tail_anchors_synthesized}),
                )?;
                mass_i += 1;
            }
        }
        let previous_anchor = world.snakes()[0].body[0];
        let before = std::mem::take(&mut known);
        world.step(&[Some(action)])?;
        if world.snakes()[0].body[0] != previous_anchor {
            anchor_index += 1;
            let event = AnchorEvent {
                index: anchor_index,
                time: world.time(),
                point: world.snakes()[0].body[0],
            };
            write(
                &mut out,
                json!({"kind":"anchor","time":event.time,"index":event.index,"point":event.point}),
            )?;
            simulated_anchors.push(event);
        }

        // A sidecar tracks wire food IDs without changing production physics. Exact
        // duplicate pellets cannot be uniquely identified, so mark those ambiguous.
        let ambiguous_pickup_values: Vec<Food> = world
            .pickup_events()
            .expect("enabled pickup events")
            .iter()
            .filter_map(|event| match &event.item {
                PickupItem::Food(food) => Some(food),
                PickupItem::Prey(_) => None,
            })
            .filter(|food| {
                before.iter().filter(|(_, prior)| prior == *food).count() > 1
                    || world.food().iter().any(|remaining| remaining == *food)
            })
            .cloned()
            .collect();
        let mut used = vec![false; before.len()];
        for food in world.food() {
            if let Some(index) = before
                .iter()
                .enumerate()
                .position(|(index, (_, candidate))| !used[index] && candidate == food)
            {
                used[index] = true;
                known.push(before[index].clone());
            } else {
                generated_id += 1;
                known.push((format!("sim:{generated_id}"), food.clone()));
            }
        }
        let mut eaten_before: Vec<_> = before
            .into_iter()
            .enumerate()
            .filter_map(|(index, entry)| (!used[index]).then_some(entry))
            .collect();
        for pickup in world.pickup_events().expect("enabled pickup events") {
            if let PickupItem::Food(food) = &pickup.item {
                let matching: Vec<_> = eaten_before
                    .iter()
                    .enumerate()
                    .filter_map(|(index, (_, candidate))| (candidate == food).then_some(index))
                    .collect();
                let ambiguous = matching.len() > 1 || ambiguous_pickup_values.contains(food);
                let id = if let Some(index) = matching.last() {
                    eaten_before.remove(*index).0
                } else {
                    generated_id += 1;
                    format!("sim:{generated_id}")
                };
                write(
                    &mut out,
                    json!({"kind":"pickup","time":world.time(),"id":id,"food":food,
                    "eater":pickup.eater,"id_ambiguous":ambiguous}),
                )?;
                if pickup.eater == 0 {
                    simulated_eats.push(EatEvent {
                        time: world.time(),
                        id,
                    });
                }
            }
        }
        if !eaten_before.is_empty() {
            return Err(fail(
                "food sidecar lost pellets without a pickup; cannot assign stable IDs",
            ));
        }

        while observation_i < f.observations.len()
            && f.observations[observation_i].time <= world.time() + EPS
        {
            let o = &f.observations[observation_i];
            let s = &world.snakes()[0];
            let head_error = o.head.map(|point| s.head.distance(point));
            let speed_error = o.speed.map(|speed| (s.speed - speed).abs());
            let mass_error = (mode == "free")
                .then(|| {
                    o.segments.zip(o.fam).map(|(segments, fam)| {
                        ((s.segments as f64 + s.fam) - (segments as f64 + fam)).abs()
                    })
                })
                .flatten();
            let heading_error = o.heading.map(|heading| angle_error(s.heading, heading));
            let boost_match = o.boosting.map(|boosting| boosting == world.is_boosting(0));
            if let Some(error) = head_error {
                divergence(&mut first, "head", o.time, error, f.options.head_tolerance);
            }
            if let Some(error) = speed_error {
                divergence(
                    &mut first,
                    "speed",
                    o.time,
                    error,
                    f.options.speed_tolerance,
                );
            }
            if let Some(error) = mass_error {
                divergence(&mut first, "mass", o.time, error, f.options.mass_tolerance);
            }
            // Wire headings may be stale and quantized; report them but do not
            // treat them as authoritative per-frame heading divergence.
            if boost_match == Some(false) {
                first
                    .entry("boost")
                    .or_insert_with(|| json!({"metric":"boost","time":o.time}));
            }
            write(
                &mut out,
                json!({"kind":"observation","time":o.time,"sampled_sim_time":world.time(),
                "head_error":head_error,"speed_error":speed_error,"mass_error":mass_error,
                "mass_comparison_intervened":mode == "motion_only",
                "heading_error_diagnostic":heading_error,"heading_source":o.heading_source,
                "boost_match":boost_match,"anchor_index":o.anchor_index}),
            )?;
            observation_i += 1;
        }
        if world.time() + EPS >= next_sample {
            let s = &world.snakes()[0];
            write(
                &mut out,
                json!({"kind":"sample","time":world.time(),"head":s.head,
                "heading":s.heading,"speed":s.speed,"segments":s.segments,"fam":s.fam,
                "score":s.score,"body":s.body,"boosting":world.is_boosting(0),
                "action":action,"food_count":world.food().len()}),
            )?;
            next_sample += f.options.sample_interval_seconds;
        }
    }

    if let Some(observation) = f.observations.get(observation_i) {
        first.insert(
            "observation_missing",
            json!({"metric":"observation_missing","time":observation.time}),
        );
    }
    let mut anchor_errors = Vec::new();
    for observed in &observed_anchors {
        if let Some(simulated) = simulated_anchors
            .iter()
            .find(|event| event.index == observed.index)
        {
            let distance = simulated.point.distance(observed.point);
            let time_error = simulated.time - observed.time;
            divergence(
                &mut first,
                "anchor",
                observed.time,
                distance,
                f.options.anchor_tolerance,
            );
            anchor_errors.push(distance);
            write(
                &mut out,
                json!({"kind":"anchor_comparison","index":observed.index,
                "observed_time":observed.time,"simulated_time":simulated.time,
                "time_error":time_error,"distance":distance,"observed":observed.point,
                "simulated":simulated.point}),
            )?;
        } else {
            first.entry("anchor_missing").or_insert_with(
                || json!({"metric":"anchor_missing","time":observed.time,"index":observed.index}),
            );
            write(
                &mut out,
                json!({"kind":"anchor_comparison","index":observed.index,
                "observed_time":observed.time,"simulated_time":null,"distance":null}),
            )?;
        }
    }
    if let Some(expected) = &f.expected_pickups {
        observed_eats.extend(expected.iter().map(|eat| EatEvent {
            time: eat.time,
            id: eat.id.clone(),
        }));
    }
    let mut matched = HashSet::new();
    let mut missed_pickups = 0usize;
    for observed in &observed_eats {
        let candidate = simulated_eats
            .iter()
            .enumerate()
            .filter(|(index, eat)| {
                !matched.contains(index)
                    && eat.id == observed.id
                    && (eat.time - observed.time).abs() <= f.options.pickup_time_tolerance
            })
            .min_by(|(_, a), (_, b)| {
                (a.time() - observed.time)
                    .abs()
                    .total_cmp(&(b.time() - observed.time).abs())
            });
        if let Some((index, _)) = candidate {
            matched.insert(index);
        } else {
            missed_pickups += 1;
            first.entry("pickup_missed").or_insert_with(
                || json!({"metric":"pickup_missed","time":observed.time,"id":observed.id}),
            );
        }
    }
    let extra_pickups = if f.expected_pickups.is_some() {
        if let Some(eat) = simulated_eats
            .iter()
            .enumerate()
            .find_map(|(index, event)| (!matched.contains(&index)).then_some(event))
        {
            first
                .entry("pickup_extra")
                .or_insert_with(|| json!({"metric":"pickup_extra","time":eat.time,"id":eat.id}));
        }
        Some(simulated_eats.len() - matched.len())
    } else {
        None
    };
    let mut first_divergences: Vec<_> = first.into_values().collect();
    first_divergences.sort_by(|a, b| {
        a["time"]
            .as_f64()
            .unwrap_or(f64::INFINITY)
            .total_cmp(&b["time"].as_f64().unwrap_or(f64::INFINITY))
    });
    let anchor_mean_error = (!anchor_errors.is_empty())
        .then(|| anchor_errors.iter().sum::<f64>() / anchor_errors.len() as f64);
    write(
        &mut out,
        json!({"kind":"summary","mode":mode,"replayed_seconds":world.time(),
        "ended_alive":world.snakes()[0].alive,"anchor_events_observed":observed_anchors.len(),
        "anchor_events_simulated":simulated_anchors.len(),"anchor_mean_error":anchor_mean_error,
        "observed_pickups":observed_eats.len(),"simulated_pickups":simulated_eats.len(),
        "missed_pickups":missed_pickups,"extra_pickups":extra_pickups,
        "observed_pickups_complete":f.expected_pickups.is_some(),
        "external_food_events_applied":external_food_events,
        "external_food_events_pending":f.food_events.len() - food_i,
        "observations_total":f.observations.len(),
        "observations_compared":observation_i,
        "observations_pending":f.observations.len() - observation_i,
        "mass_interventions":if mode == "motion_only" {mass_i} else {0},
        "first_divergences":first_divergences}),
    )?;
    out.flush()?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 3 {
        return Err(fail(
            "usage: fidelity_replay <fixture.json> <free|motion_only>",
        ));
    }
    let fixture: Fixture = serde_json::from_str(&fs::read_to_string(&args[1])?)?;
    run(fixture, &args[2], BufWriter::new(io::stdout().lock()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_value() -> Value {
        json!({
            "schema_version":1,"duration_seconds":1.0,
            "initial":{"snake":{"id":"self","head":{"x":0.0,"y":0.0},
                "body":[{"x":0.0,"y":0.0},{"x":-42.0,"y":0.0}],
                "heading":0.0,"speed":180.0,"radius":14.5,"length":84.0,
                "segments":2,"fam":0.0,"score":10.0,"alive":true},
                "action":{"heading":0.0,"boost":false},"food":[]},
            "observations":[{"time":0.1,"anchor_index":1,"anchor":{"x":42.0,"y":0.0}}],
            "expected_pickups":[],"options":{"boost_drops":"recorded"}
        })
    }

    #[test]
    fn fixture_accepts_indexed_anchors_and_complete_empty_pickup_target() {
        let fixture: Fixture = serde_json::from_value(fixture_value()).unwrap();
        validate(&fixture).unwrap();
        assert_eq!(fixture.observations[0].anchor_index, Some(1));
        assert!(fixture.expected_pickups.is_some());
        assert!(matches!(fixture.options.boost_drops, DropMode::Recorded));
    }

    #[test]
    fn early_death_keeps_later_observation_and_anchor_targets() {
        let mut value = fixture_value();
        value["options"]["radius"] = json!(100.0);
        value["observations"] = json!([
            {"time":0.25,"anchor_index":1,"anchor":{"x":42.0,"y":0.0}},
            {"time":0.9,"anchor_index":3,"anchor":{"x":126.0,"y":0.0}}
        ]);
        let fixture: Fixture = serde_json::from_value(value).unwrap();
        let mut output = Vec::new();
        run(fixture, "free", &mut output).unwrap();
        let rows: Vec<Value> = String::from_utf8(output)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let summary = rows.last().unwrap();
        assert_eq!(summary["ended_alive"], false);
        assert!(summary["replayed_seconds"].as_f64().unwrap() < 0.9);
        assert_eq!(summary["observations_total"], 2);
        assert_eq!(summary["observations_compared"], 1);
        assert_eq!(summary["observations_pending"], 1);
        assert_eq!(summary["anchor_events_observed"], 2);
        for metric in ["observation_missing", "anchor_missing"] {
            assert!(
                summary["first_divergences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["metric"] == metric && row["time"] == 0.9)
            );
        }
        assert!(rows.iter().any(|row| row["kind"] == "anchor_comparison"
            && row["index"] == 3
            && row["simulated_time"].is_null()));
    }

    #[test]
    fn replay_refuses_forced_self_pickup_and_preserves_front_on_mass_intervention() {
        let mut value = fixture_value();
        value["food_events"] = json!([{"time":0.1,"kind":"remove","id":"wire:1",
            "reason":"self-pickup"}]);
        let fixture: Fixture = serde_json::from_value(value).unwrap();
        assert!(
            validate(&fixture)
                .unwrap_err()
                .to_string()
                .contains("never forced")
        );

        let mut snake = fixture.initial.snake;
        let front = snake.body[0];
        let head = snake.head;
        snake.segments = 4;
        assert_eq!(align_tail_to_segments(&mut snake), 2);
        assert_eq!(snake.head, head);
        assert_eq!(snake.body[0], front);
        assert_eq!(snake.body.len(), 4);
        assert_eq!(snake.body[3], Point { x: -126.0, y: 0.0 });
    }
}
