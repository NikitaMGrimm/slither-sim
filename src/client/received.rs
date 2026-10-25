//! Semantic client receipt state. Clocks are local receipt clocks, never inferred
//! server send times. Bodies are reconstructed from appearance and anchor events.
use crate::{
    model::{Point, Snake},
    physics,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ReceivedField {
    pub age_seconds: f64,
    pub valid: bool,
    pub predicted: bool,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ReceivedHistory {
    pub value: f64,
    pub age_seconds: f64,
    pub valid: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ReceivedSnake {
    pub snake_id: String,
    pub incarnation: u64,
    pub appearance: ReceivedField,
    pub position: ReceivedField,
    pub heading: ReceivedField,
    pub wanted_heading: ReceivedField,
    pub speed: ReceivedField,
    pub body: ReceivedField,
    pub mass: ReceivedField,
    /// Recent *changes* of reported wanted heading, newest first.
    pub wanted_history: Vec<ReceivedHistory>,
    /// Earlier reports near 84 and 252 ms, not regularly sampled predictions.
    pub speed_history: Vec<ReceivedHistory>,
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct CategoryKnowledge {
    pub valid: bool,
    /// Explicit category-specific evidence of completeness, not absence of positives.
    pub complete: bool,
    pub age_seconds: f64,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct ReceivedMetadata {
    pub connection_generation: u64,
    pub snakes: Vec<ReceivedSnake>,
    pub bodies: CategoryKnowledge,
    pub food: CategoryKnowledge,
    pub prey: CategoryKnowledge,
    pub minimap: ReceivedField,
}
impl ReceivedMetadata {
    pub fn snake(&self, id: &str) -> Option<&ReceivedSnake> {
        self.snakes.iter().find(|snake| snake.snake_id == id)
    }
    pub fn validate(&self) -> Result<(), String> {
        let age = |value: f64| value.is_finite() && value >= 0.;
        let mut ids = std::collections::HashSet::new();
        for snake in &self.snakes {
            if snake.snake_id.is_empty()
                || snake.incarnation == 0
                || !ids.insert(&snake.snake_id)
                || ![
                    snake.position,
                    snake.heading,
                    snake.wanted_heading,
                    snake.speed,
                    snake.body,
                    snake.mass,
                    snake.appearance,
                ]
                .iter()
                .all(|field| {
                    age(field.age_seconds)
                        && (field.valid || (field.age_seconds == 0. && !field.predicted))
                })
                || !snake
                    .wanted_history
                    .iter()
                    .chain(&snake.speed_history)
                    .all(|entry| {
                        age(entry.age_seconds)
                            && entry.value.is_finite()
                            && (!entry.valid || entry.age_seconds <= 0.5)
                            && (entry.valid || (entry.age_seconds == 0. && entry.value == 0.))
                    })
                || snake.wanted_history.len() > 3
                || snake.speed_history.len() > 2
            {
                return Err("Invalid received snake metadata".into());
            }
        }
        if ![
            self.bodies.age_seconds,
            self.food.age_seconds,
            self.prey.age_seconds,
            self.minimap.age_seconds,
        ]
        .into_iter()
        .all(age)
            || [self.bodies, self.food, self.prey].iter().any(|category| {
                !category.valid && (category.complete || category.age_seconds != 0.)
            })
            || (!self.minimap.valid && (self.minimap.predicted || self.minimap.age_seconds != 0.))
        {
            return Err("Invalid received category metadata".into());
        }
        Ok(())
    }
    /// Advance only time since delivery; transport latency is not a receipt age.
    pub fn age_by(&mut self, elapsed: f64) {
        let elapsed = elapsed.max(0.);
        for snake in &mut self.snakes {
            for field in [
                &mut snake.position,
                &mut snake.heading,
                &mut snake.wanted_heading,
                &mut snake.speed,
                &mut snake.body,
                &mut snake.mass,
                &mut snake.appearance,
            ] {
                if field.valid {
                    field.age_seconds += elapsed;
                }
            }
            for history in snake
                .wanted_history
                .iter_mut()
                .chain(&mut snake.speed_history)
            {
                if history.valid {
                    history.age_seconds += elapsed;
                }
            }
            snake
                .wanted_history
                .retain(|entry| entry.valid && entry.age_seconds <= 0.5);
            for history in &mut snake.speed_history {
                if history.valid && history.age_seconds > 0.5 {
                    *history = ReceivedHistory::default();
                }
            }
        }
        for category in [&mut self.bodies, &mut self.food, &mut self.prey] {
            if category.valid {
                category.age_seconds += elapsed;
            }
        }
        if self.minimap.valid {
            self.minimap.age_seconds += elapsed;
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Field(Option<f64>);
impl Field {
    fn receipt(&mut self, now: f64) {
        self.0 = Some(now);
    }
    fn metadata(&self, now: f64) -> ReceivedField {
        ReceivedField {
            valid: self.0.is_some(),
            predicted: false,
            age_seconds: self.0.map_or(0., |at| (now - at).max(0.)),
        }
    }
}
#[derive(Clone, Debug)]
struct Track {
    snake: Snake,
    target: f64,
    incarnation: u64,
    position: Field,
    heading: Field,
    wanted_heading: Field,
    speed: Field,
    body: Field,
    mass: Field,
    appearance: Field,
    wanted: VecDeque<(f64, f64)>,
    speeds: VecDeque<(f64, f64)>,
}
const DEFAULT_CONTRACTION: f64 = 0.43;
/// `physics::append_anchor` with a configurable coefficient; at 0.43 it is the
/// same computation. Contraction precedes insertion, as in received snapshots.
fn append_anchor(body: &mut Vec<Point>, anchor: Point, grow: bool, contraction: f64) {
    for i in 3..body.len() {
        let weight = contraction * ((i - 2) as f64 / 4.0).min(1.0);
        body[i].x += (body[i - 1].x - body[i].x) * weight;
        body[i].y += (body[i - 1].y - body[i].y) * weight;
    }
    body.insert(0, anchor);
    if !grow {
        body.pop();
    }
}

/// One connection's ordered received events. Slots are internal bookkeeping only.
#[derive(Clone, Debug, Default)]
pub struct ReceivedTracker {
    connection_generation: u64,
    tracks: Vec<Option<Track>>,
    generations: Vec<u64>,
    last_time: Option<f64>,
}
impl ReceivedTracker {
    pub fn new(slots: usize) -> Self {
        Self {
            tracks: vec![None; slots],
            generations: vec![0; slots],
            last_time: None,
            connection_generation: 0,
        }
    }
    pub fn reconnect(&mut self) {
        self.tracks.fill(None);
        self.connection_generation += 1;
        for generation in &mut self.generations {
            *generation += 1;
        }
        self.last_time = None;
    }
    fn clock(&mut self, now: f64) -> Result<(), String> {
        if !now.is_finite() || now < 0. {
            return Err("Invalid receipt clock".into());
        }
        if self.last_time.is_some_and(|last| now < last) {
            self.reconnect();
        }
        self.last_time = Some(now);
        Ok(())
    }
    pub fn appearance(
        &mut self,
        slot: usize,
        snake: &Snake,
        target: f64,
        now: f64,
    ) -> Result<(), String> {
        self.clock(now)?;
        if slot >= self.tracks.len()
            || !target.is_finite()
            || !snake.head.finite()
            || snake.body.iter().any(|point| !point.finite())
        {
            return Err("Invalid received appearance".into());
        }
        self.generations[slot] += 1;
        self.tracks[slot] = Some(Track {
            snake: snake.clone(),
            target,
            incarnation: self.generations[slot],
            position: Field(Some(now)),
            heading: Field(Some(now)),
            wanted_heading: Field(Some(now)),
            speed: Field(Some(now)),
            body: Field(Some(now)),
            mass: Field(Some(now)),
            appearance: Field(Some(now)),
            wanted: VecDeque::new(),
            speeds: VecDeque::from([(now, snake.speed)]),
        });
        // Received appearance packets contain mass, not private score credits or kills.
        if let Some(track) = &mut self.tracks[slot] {
            track.snake.score = physics::score(snake.segments, snake.fam);
            track.snake.kills = 0;
        }
        Ok(())
    }
    pub fn removal(&mut self, slot: usize, now: f64) -> Result<(), String> {
        self.clock(now)?;
        let track = self.tracks.get_mut(slot).ok_or("Invalid received slot")?;
        if let Some(track) = track {
            track.snake.alive = false;
        }
        Ok(())
    }
    pub fn anchor(
        &mut self,
        slot: usize,
        point: Point,
        grow: bool,
        now: f64,
    ) -> Result<(), String> {
        self.anchor_with(slot, point, grow, DEFAULT_CONTRACTION, now)
    }
    /// `anchor` with the server's contraction coefficient, restricted to 0..=1:
    /// larger values can overflow sequential reconstruction of finite wire points.
    pub fn anchor_with(
        &mut self,
        slot: usize,
        point: Point,
        grow: bool,
        contraction: f64,
        now: f64,
    ) -> Result<(), String> {
        // Validate before the clock, which may reconnect on a backward receipt.
        if !(0. ..=1.).contains(&contraction) {
            return Err("Body contraction must be from 0 to 1".into());
        }
        self.clock(now)?;
        if !point.finite() {
            return Err("Invalid received anchor".into());
        }
        let track = self.track_mut(slot)?;
        append_anchor(&mut track.snake.body, point, grow, contraction);
        track.body.receipt(now);
        Ok(())
    }
    /// Grow to at least `slots`; existing tracks and incarnations are kept.
    pub fn ensure_slots(&mut self, slots: usize) {
        if slots > self.tracks.len() {
            self.tracks.resize(slots, None);
            self.generations.resize(slots, 0);
        }
    }
    pub fn tail(&mut self, slot: usize, length: usize, now: f64) -> Result<(), String> {
        self.clock(now)?;
        let track = self.track_mut(slot)?;
        track.snake.body.truncate(length);
        track.body.receipt(now);
        Ok(())
    }
    pub fn position(&mut self, slot: usize, point: Point, now: f64) -> Result<(), String> {
        self.clock(now)?;
        if !point.finite() {
            return Err("Invalid received position".into());
        }
        let track = self.track_mut(slot)?;
        track.snake.head = point;
        track.position.receipt(now);
        Ok(())
    }
    /// Optional fields are independent reports, not a refreshed pose from rendering.
    pub fn motion(
        &mut self,
        slot: usize,
        heading: Option<f64>,
        wanted: Option<f64>,
        speed: Option<f64>,
        now: f64,
    ) -> Result<(), String> {
        self.clock(now)?;
        if heading
            .into_iter()
            .chain(wanted)
            .chain(speed)
            .any(|value| !value.is_finite())
        {
            return Err("Invalid received motion".into());
        }
        let track = self.track_mut(slot)?;
        if let Some(heading) = heading {
            track.snake.heading = heading;
            track.heading.receipt(now);
        }
        if let Some(wanted) = wanted {
            if wanted != track.target {
                track.wanted.push_front((now, wanted));
                track.wanted.truncate(3);
            }
            track.target = wanted;
            track.wanted_heading.receipt(now);
        }
        if let Some(speed) = speed {
            track.snake.speed = speed;
            track.speed.receipt(now);
            track.speeds.push_front((now, speed));
            while track.speeds.len() > 1
                && track.speeds.back().is_some_and(|(at, _)| now - at > 0.5)
            {
                track.speeds.pop_back();
            }
            // Bounded even if malformed sources produce excessive equal-time reports.
            track.speeds.truncate(80);
        }
        Ok(())
    }
    pub fn mass(&mut self, slot: usize, source: &Snake, now: f64) -> Result<(), String> {
        self.mass_values(
            slot,
            source.segments,
            source.fam,
            source.radius,
            source.length,
            now,
        )
    }
    /// Wire mass reports carry scalars, so callers need not copy a snake's body.
    pub(crate) fn mass_values(
        &mut self,
        slot: usize,
        segments: usize,
        fam: f64,
        radius: f64,
        length: f64,
        now: f64,
    ) -> Result<(), String> {
        self.clock(now)?;
        let track = self.track_mut(slot)?;
        track.snake.segments = segments;
        track.snake.fam = fam;
        track.snake.radius = radius;
        track.snake.length = length;
        // Opponent private credits/kills are not reconstructed from mass reports.
        track.snake.score = physics::score(segments, fam);
        track.mass.receipt(now);
        Ok(())
    }
    /// Set the receiver's displayed score. Use only the receiver's own slot:
    /// opponents' private credits are unknown. Mass receipts are not refreshed,
    /// and a later `mass` report restores the mass-derived score until the own
    /// score arrives again.
    pub fn own_score(&mut self, slot: usize, score: f64) -> Result<(), String> {
        if !score.is_finite() || score < 0. {
            return Err("Invalid received own score".into());
        }
        self.track_mut(slot)?.snake.score = score;
        Ok(())
    }
    fn track_mut(&mut self, slot: usize) -> Result<&mut Track, String> {
        self.tracks
            .get_mut(slot)
            .and_then(Option::as_mut)
            .filter(|track| track.snake.alive)
            .ok_or_else(|| "Received update before appearance".into())
    }
    pub fn snake(&self, slot: usize) -> Option<&Snake> {
        self.tracks.get(slot)?.as_ref().map(|track| &track.snake)
    }
    pub fn target(&self, slot: usize) -> Option<f64> {
        self.tracks.get(slot)?.as_ref().map(|track| track.target)
    }
    /// Snakes in metadata order. `snake` additionally keeps a removed snake.
    pub fn alive_snakes(&self) -> impl Iterator<Item = (usize, &Snake)> {
        self.tracks.iter().enumerate().filter_map(|(slot, track)| {
            track
                .as_ref()
                .map(|track| (slot, &track.snake))
                .filter(|(_, snake)| snake.alive)
        })
    }
    pub fn metadata(&self, now: f64) -> ReceivedMetadata {
        let mut metadata = ReceivedMetadata {
            connection_generation: self.connection_generation,
            ..Default::default()
        };
        for track in self
            .tracks
            .iter()
            .flatten()
            .filter(|track| track.snake.alive)
        {
            let history = |&(at, value): &(f64, f64)| ReceivedHistory {
                value,
                age_seconds: (now - at).max(0.),
                valid: now - at <= 0.5,
            };
            let mut speed_history = Vec::new();
            for age in [0.084, 0.252] {
                if let Some(report) = track
                    .speeds
                    .iter()
                    .skip(1)
                    .filter(|(at, _)| now - at <= 0.5)
                    .min_by(|a, b| {
                        ((now - a.0) - age)
                            .abs()
                            .total_cmp(&((now - b.0) - age).abs())
                    })
                {
                    speed_history.push(history(report));
                } else {
                    speed_history.push(ReceivedHistory::default());
                }
            }
            metadata.snakes.push(ReceivedSnake {
                snake_id: track.snake.id.clone(),
                incarnation: track.incarnation,
                position: track.position.metadata(now),
                appearance: track.appearance.metadata(now),
                heading: track.heading.metadata(now),
                wanted_heading: track.wanted_heading.metadata(now),
                speed: track.speed.metadata(now),
                body: track.body.metadata(now),
                mass: track.mass.metadata(now),
                wanted_history: track
                    .wanted
                    .iter()
                    .filter(|(at, _)| now - at <= 0.5)
                    .map(history)
                    .collect(),
                speed_history,
            });
        }
        metadata
    }
}
