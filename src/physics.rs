//! Server rules measured from protocol-19 captures (`tools/fidelity/slither-headless.mjs`,
//! analyzed on the server's elapsed-ms clock) and constants the server sends to clients.
//! Speeds are world units per second; protocol speed converts as `sp / 0.032`.
//! Turning and boost progress use a separate 8 ms reference interval. Neither
//! conversion is the physics tick. See docs/server-rules.md for each fit's evidence.
use crate::model::Point;
use crate::{Constraint, SimError};
use serde::{Deserialize, Serialize};
use std::{
    f64::consts::{PI, TAU},
    sync::LazyLock,
};

/// Average server loop period: frame gaps are multiples of ~7 ms (6.94-7.07 ms per server).
/// Snake motion scales with elapsed time. Prey coordinate rounding occurs once
/// per loop, so its effective velocity also depends on this period.
pub const LOOP_SECONDS: f64 = 0.007;
/// The per-server average loop periods behind `LOOP_SECONDS`. Other physics
/// steps are valid but unmeasured.
pub(crate) const MEASURED_LOOP_SECONDS: [f64; 2] = [0.00694, 0.00707];
/// Protocol speed divided by this factor gives world units/s; not a server tick.
pub(crate) const SPEED_UNIT_SECONDS: f64 = 0.032;
/// Nominal interval for turning and boost-progress coefficients, separate from LOOP_SECONDS.
pub(crate) const REFERENCE_FRAME_SECONDS: f64 = 0.008;
/// Distance between body anchors (`default_msl`).
pub const ANCHOR_SPACING: f64 = 42.0;
/// Radius of drawn bodies and heads per scale (`sc * 29 / 2`).
pub const BODY_RADIUS: f64 = 14.5;
/// Boost speed (`nsp3`).
pub const BOOST_SPEED: f64 = 14.0 / SPEED_UNIT_SECONDS;
/// Nominal interval between boost pellets while boosting.
pub const BOOST_DROP_SECONDS: f64 = 0.155;
/// Food nutrition per squared pellet size before the mass multiplier.
pub const NUTRITION: f64 = 0.0031535;
/// Maximum counted segments (`mscps`).
pub const MAX_SEGMENTS: usize = 430;
/// Scale of the smallest snake, two segments.
pub const MIN_SCALE: f64 = 1.0;

pub fn scale(segments: usize) -> f64 {
    (1.0 + (segments as f64 - 2.0) / 106.0).clamp(MIN_SCALE, 6.0)
}
/// Empirical server turn factor. Sustained anchor turns support a size denominator
/// of 111 (validated through 293 segments); the client's drawn scale still uses 106.
/// Evidence and packaged regressions: docs/server-rules.md#movement-and-geometry.
pub fn scang(segments: usize) -> f64 {
    let turn_scale = (1.0 + (segments as f64 - 2.0) / 111.0).clamp(1.0, 6.0);
    0.13 + 0.87 * ((7.0 - turn_scale) / 6.0).powi(2)
}
/// Cruise speed `nsp1 + nsp2 * sc`.
pub fn base_speed(segments: usize) -> f64 {
    (5.39 + 0.4 * scale(segments)) / SPEED_UNIT_SECONDS
}
/// Maximum turn in radians per second: `mamu * scang` per 8 ms, independent of boost
/// (speed never drops below `spangdv`, so `spang` is always 1).
pub fn turn_rate(segments: usize) -> f64 {
    0.033 / REFERENCE_FRAME_SECONDS * scang(segments)
}
/// Speed ramp derived from the current APK's normalized boost progress, using nominal
/// time. The fixed-size slope is independently supported by online speed reports;
/// latent progress retention during growth and offline saturation quirks are unverified.
/// See docs/server-rules.md#movement-and-geometry. Network delay and the offline 1.3 clock are excluded.
pub fn approach_speed(current: f64, target: f64, segments: usize, dt: f64) -> f64 {
    let up = (BOOST_SPEED - base_speed(segments)) * (0.015 / REFERENCE_FRAME_SECONDS)
        / (1.0 + 0.305 * (scale(segments) - 1.0));
    let limit = if target > current { up } else { up * 4.0 / 3.0 } * dt;
    current + (target - current).clamp(-limit, limit)
}

/// Retained idle cruise is measured online. Progress through size changes is an
/// APK-derived candidate; direct speed remains available for differential runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedModel {
    #[default]
    RetainedProgress,
    DirectSpeed,
}

/// Stored physical speed and boost progress. This hidden state is not a policy input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedState {
    pub speed: f64,
    progress: f64,
}
impl SpeedState {
    pub fn idle(speed: f64) -> Self {
        Self {
            speed,
            progress: 0.0,
        }
    }
    /// Snapshot inference cannot distinguish every retained idle speed from a ramp.
    /// Below current-size cruise, retain the observed speed rather than accelerating.
    pub fn from_observed_speed(speed: f64, segments: usize) -> Self {
        let base = base_speed(segments);
        Self {
            speed,
            progress: ((speed - base) / (BOOST_SPEED - base)).clamp(0.0, 1.0),
        }
    }
    pub fn with_progress(speed: f64, progress: f64) -> Result<Self, SimError> {
        let message = "Speed state requires finite nonnegative speed and progress in 0..1";
        if let Some(rule) = Constraint::nonnegative(speed) {
            return Err(SimError::state(rule, "speed", message));
        }
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(SimError::state(
                if progress.is_finite() {
                    Constraint::OutOfRange
                } else {
                    Constraint::NonFinite
                },
                "progress",
                message,
            ));
        }
        Ok(Self { speed, progress })
    }
    pub fn progress(self) -> f64 {
        self.progress
    }
    pub fn step(&mut self, boosting: bool, segments: usize, dt: f64, model: SpeedModel) -> f64 {
        if dt == 0.0 {
            return self.speed;
        }
        let base = base_speed(segments);
        if model == SpeedModel::DirectSpeed {
            self.speed = approach_speed(
                self.speed,
                if boosting { BOOST_SPEED } else { base },
                segments,
                dt,
            );
            *self = Self::from_observed_speed(self.speed, segments);
        } else if (boosting && self.progress < 1.0) || (!boosting && self.progress > 0.0) {
            let rate = if boosting {
                0.015 / REFERENCE_FRAME_SECONDS
            } else {
                -0.02 / REFERENCE_FRAME_SECONDS
            };
            self.progress = (self.progress + rate * dt / (1.0 + 0.305 * (scale(segments) - 1.0)))
                .clamp(0.0, 1.0);
            self.speed = base + (BOOST_SPEED - base) * self.progress;
        }
        self.speed
    }
}
/// Clients send the wanted heading as one byte in 251 steps; the server steers to that angle.
pub fn quantize_heading(heading: f64) -> f64 {
    (heading.rem_euclid(TAU) * 251.0 / TAU).floor().min(250.0) * TAU / 251.0
}
/// Growth multiplier at continuous mass `segments + fam`: (1 - (mass - 2) / 430)^2.25.
/// Matches isolated original-game pickups to 5e-5, including the decline within a segment.
pub fn multiplier(mass: f64) -> f64 {
    (1.0 - (mass - 2.0) / MAX_SEGMENTS as f64)
        .max(1.0 / MAX_SEGMENTS as f64)
        .powf(2.25)
}
/// Growth (`fam`) from eating a pellet of `size` at `mass = segments + fam`.
pub fn nutrition(size: f64, mass: f64) -> f64 {
    size * size * NUTRITION * multiplier(mass)
}
/// Base mass per boost pellet; fitted 0.0812 (1 - sct/430)^1.66.
/// The simulator applies a `[0.94, 1.08)` jitter multiplier, capped by remaining mass.
pub fn boost_cost(segments: usize) -> f64 {
    0.0812
        * (1.0 - segments as f64 / MAX_SEGMENTS as f64)
            .max(0.0)
            .powf(1.66)
}
/// Food pickup region: a circle ahead of the head. Its center leads by the distance covered
/// in ~242 ms at scale 1 (longer for larger snakes), and its radius grows with scale.
/// Calibrated with spatial anchor tangents: packet headings lag during turns and
/// previously biased the fit toward a larger circle; see docs/server-rules.md#movement-and-geometry.
pub fn pickup_circle(head: Point, heading: f64, speed: f64, segments: usize) -> (Point, f64) {
    let sc = scale(segments) - 1.0;
    let lead = speed * SPEED_UNIT_SECONDS * (7.55 + 1.8 * sc);
    (
        Point {
            x: head.x + heading.cos() * lead,
            y: head.y + heading.sin() * lead,
        },
        36.0 + 19.0 * sc,
    )
}
/// The same pickup geometry with this heading's `(cos, sin)` already computed.
/// Keep the arithmetic order of `pickup_circle`; food and prey share the result
/// within a step because pickups change fam, not the head, speed or segments.
/// research/performance/2026-10-27-core-heading-geometry/REPORT.md
pub(crate) fn pickup_circle_directed(
    head: Point,
    (cos, sin): (f64, f64),
    speed: f64,
    segments: usize,
) -> (Point, f64) {
    let sc = scale(segments) - 1.0;
    let lead = speed * SPEED_UNIT_SECONDS * (7.55 + 1.8 * sc);
    (
        Point {
            x: head.x + cos * lead,
            y: head.y + sin * lead,
        },
        36.0 + 19.0 * sc,
    )
}
/// How far ahead of a head center another snake's centerline is lethal: the head's
/// tip plus the probe's extension by the body radius (see [`strikes`]).
pub fn collision_distance(head_segments: usize, body_segments: usize) -> f64 {
    BODY_RADIUS * (scale(head_segments) + scale(body_segments))
}
/// A head's tip: its center advanced by its own radius along its heading.
pub fn head_tip(head: Point, heading: f64, segments: usize) -> Point {
    let r = BODY_RADIUS * scale(segments);
    Point {
        x: head.x + heading.cos() * r,
        y: head.y + heading.sin() * r,
    }
}
/// Whether a moving head strikes another snake's line, as the official client's
/// offline collision code tests it (Android 3.06 `oef`, see
/// docs/server-rules.md#movement-and-geometry). `last_tip` and `tip` are the
/// head's tips before and after this tick's move; `line` starts at the other snake's
/// tip, then its head center, then its body from newest to oldest anchor. Only line
/// points within 3.666 summed radii of the tip on both axes count. The head dies
/// when its forward probe, from the last tip to the tip extended by the other's
/// radius, crosses a line segment, or when its tip lies within 0.975 of the other's
/// radius of a line point. Contact beside the head or along the body is harmless.
pub fn strikes(
    last_tip: Point,
    tip: Point,
    heading: f64,
    head_segments: usize,
    body_segments: usize,
    line: impl IntoIterator<Item = Point>,
) -> bool {
    strikes_scaled(
        last_tip,
        tip,
        (heading.cos(), heading.sin()),
        scale(head_segments),
        scale(body_segments),
        line,
    )
}
/// [`strikes`] with the head's heading as `(cos, sin)` and both scales precomputed,
/// so a caller testing many pairs evaluates each snake's terms once.
pub(crate) fn strikes_scaled(
    last_tip: Point,
    tip: Point,
    (cos, sin): (f64, f64),
    head_scale: f64,
    body_scale: f64,
    line: impl IntoIterator<Item = Point>,
) -> bool {
    let body_radius = BODY_RADIUS * body_scale;
    // `3.666 * collision_distance(head, body)` with the same operation order.
    let reach = 3.666 * (BODY_RADIUS * (head_scale + body_scale));
    let end = Point {
        x: tip.x + cos * body_radius,
        y: tip.y + sin * body_radius,
    };
    let mut points = line.into_iter();
    let Some(mut a) = points.next() else {
        return false;
    };
    for b in points {
        if (b.x - tip.x).abs().max((b.y - tip.y).abs()) < reach
            && (segments_intersect(last_tip, end, a, b) || tip.distance_lt(b, 0.975 * body_radius))
        {
            return true;
        }
        a = b;
    }
    false
}
/// The official client's segment test: the two lines' intersection lies within
/// both segments' coordinate ranges, inclusive. Parallel segments never intersect.
pub fn segments_intersect(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    let (aa1, bb1) = (a0.y - a1.y, a1.x - a0.x);
    let cc1 = aa1 * a1.x + bb1 * a1.y;
    let (aa2, bb2) = (b1.y - b0.y, b0.x - b1.x);
    let cc2 = aa2 * b0.x + bb2 * b0.y;
    let det = aa1 * bb2 - aa2 * bb1;
    if det == 0.0 {
        return false;
    }
    let x = (bb2 * cc1 - bb1 * cc2) / det;
    let y = (aa1 * cc2 - aa2 * cc1) / det;
    let within = |v: f64, p: f64, q: f64| {
        if p < q {
            p <= v && v <= q
        } else if p > q {
            q <= v && v <= p
        } else {
            v == p
        }
    };
    within(x, a0.x, a1.x) && within(y, a0.y, a1.y) && within(x, b0.x, b1.x) && within(y, b0.y, b1.y)
}
pub fn advance(
    head: Point,
    heading: f64,
    target: f64,
    speed: f64,
    turn: f64,
    dt: f64,
) -> (Point, f64) {
    let d = target - heading;
    let next = heading + d.sin().atan2(d.cos()).clamp(-turn * dt, turn * dt);
    (
        Point {
            x: head.x + next.cos() * speed * dt,
            y: head.y + next.sin() * speed * dt,
        },
        next,
    )
}
/// Advance with the original operation order, retaining the direction already
/// needed for displacement. Later core phases reuse it for the unchanged heading.
/// The public `advance` remains the independent reference for bit-exact tests.
pub(crate) fn advance_directed(
    head: Point,
    heading: f64,
    target: f64,
    speed: f64,
    turn: f64,
    dt: f64,
) -> (Point, f64, (f64, f64)) {
    let d = target - heading;
    let next = heading + d.sin().atan2(d.cos()).clamp(-turn * dt, turn * dt);
    let direction = (next.cos(), next.sin());
    (
        Point {
            x: head.x + direction.0 * speed * dt,
            y: head.y + direction.1 * speed * dt,
        },
        next,
        direction,
    )
}

/// Derived steering results only; position and speed still advance every tick.
/// Bit keys preserve signed zero and invalidate after action, size or dt edits,
/// including a new life in a reused slot. Nothing here belongs in a checkpoint.
#[derive(Clone, Copy, Default)]
pub(crate) struct SteeringCache {
    entry: Option<SteeringEntry>,
}

#[derive(Clone, Copy)]
struct SteeringEntry {
    heading: u64,
    target: u64,
    segments: usize,
    dt: u64,
    next: f64,
    direction: (f64, f64),
}

impl SteeringCache {
    pub(crate) fn advance(
        &mut self,
        head: Point,
        heading: f64,
        target: f64,
        speed: f64,
        segments: usize,
        dt: f64,
    ) -> (Point, f64, (f64, f64)) {
        let entry = match self.entry {
            Some(entry)
                if entry.heading == heading.to_bits()
                    && entry.target == target.to_bits()
                    && entry.segments == segments
                    && entry.dt == dt.to_bits() =>
            {
                entry
            }
            _ => {
                let (_, next, direction) = advance_directed(
                    Point::default(),
                    heading,
                    quantize_heading(target),
                    0.0,
                    turn_rate(segments),
                    dt,
                );
                let entry = SteeringEntry {
                    heading: heading.to_bits(),
                    target: target.to_bits(),
                    segments,
                    dt: dt.to_bits(),
                    next,
                    direction,
                };
                self.entry = Some(entry);
                entry
            }
        };
        (
            Point {
                x: head.x + entry.direction.0 * speed * dt,
                y: head.y + entry.direction.1 * speed * dt,
            },
            entry.next,
            entry.direction,
        )
    }
}
pub fn append_anchor(body: &mut Vec<Point>, anchor: Point, grow: bool) {
    // Fresh server body snapshots match contraction of the existing body before
    // insertion. The stock client does the opposite; see docs/server-rules.md#movement-and-geometry.
    contract_body(body);
    body.insert(0, anchor);
    if !grow {
        body.pop();
    }
}

fn contract_body(body: &mut [Point]) {
    for i in 3..body.len() {
        let weight = 0.43 * ((i - 2) as f64 / 4.0).min(1.0);
        body[i].x += (body[i - 1].x - body[i].x) * weight;
        body[i].y += (body[i - 1].y - body[i].y) * weight;
    }
}
pub fn crossed_anchor(head: Point, last: Point) -> Option<Point> {
    if head.distance_lt(last, 42.0) {
        return None;
    }
    // The authoritative anchor points along the current head ray when the
    // 42-unit threshold is crossed, even if the head overshot during this tick.
    let angle = (head.y - last.y).atan2(head.x - last.x);
    let normalized = (angle + PI * 2.0) % TAU;
    let quantized = (normalized / TAU * 65536.0).floor() / 65536.0 * TAU;
    Some(Point {
        x: last.x + quantized.cos() * 42.0,
        y: last.y + quantized.sin() * 42.0,
    })
}
/// The stock client's `(fmlts, fpsls)` score tables over `0..=MAX_SEGMENTS`.
fn score_tables() -> &'static ([f64; 431], [f64; 431]) {
    static TABLE: LazyLock<([f64; 431], [f64; 431])> = LazyLock::new(|| {
        let mut weights = [0.0; 431];
        let mut sums = [0.0; 431];
        for i in 0..=430 {
            weights[i] = (1.0 - i.min(429) as f64 / 430.0).powf(2.25);
            if i > 0 {
                sums[i] = sums[i - 1] + 1.0 / weights[i - 1];
            }
        }
        (weights, sums)
    });
    &TABLE
}
/// Continuous score used by the provisional kiosk penalty. The public score
/// still rounds down exactly as the official score display does.
pub(crate) fn score_exact(segments: usize, fam: f64) -> f64 {
    let (weights, sums) = score_tables();
    let n = segments.min(430);
    (sums[n] + fam.min(1.0) / weights[n] - 1.0) * 15.0 - 5.0
}
pub fn score(segments: usize, fam: f64) -> f64 {
    score_exact(segments, fam).floor()
}
/// Steps of protocol-19 fam: a 24-bit fraction that clients read as `u24 / 16777215`.
const FAM_STEPS: u32 = 16_777_215;
fn wire_fam(step: u32) -> f64 {
    f64::from(step) / f64::from(FAM_STEPS)
}

/// The least mass that displays a `minScore`, on the protocol fam grid so
/// that clients decode exactly the same mass and score.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MassFloor {
    pub segments: usize,
    pub fam: f64,
}
impl MassFloor {
    /// Displayed scores a floor can take: a newborn's, up to the score at
    /// `MAX_SEGMENTS`. Below that size the display rises with mass. The score
    /// table ends there, so a larger snake displays `score(MAX_SEGMENTS, fam)`
    /// again, as little as the upper bound. Growth never undercuts a floor in this range.
    pub fn score_range() -> std::ops::RangeInclusive<u32> {
        score(2, 0.0) as u32..=score(MAX_SEGMENTS, 0.0) as u32
    }
    /// The least segments and wire fam step whose displayed score is at least
    /// `target`. Displayed score never decreases with mass, so a linear scan
    /// over segments and a bisection over the 24-bit steps find it.
    pub fn for_score(target: u32) -> Option<Self> {
        if !Self::score_range().contains(&target) {
            return None;
        }
        let shows =
            |segments: usize, step: u32| score(segments, wire_fam(step)) >= f64::from(target);
        let segments = (2..=MAX_SEGMENTS).find(|&n| shows(n, FAM_STEPS - 1))?;
        // `hi` always displays the target; `lo` does not, unless it is zero.
        let (mut lo, mut hi) = (0, FAM_STEPS - 1);
        if shows(segments, 0) {
            hi = 0;
        }
        while hi - lo > 1 {
            let mid = lo + (hi - lo) / 2;
            if shows(segments, mid) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        Some(Self {
            segments,
            fam: wire_fam(hi),
        })
    }
    /// Mass above the floor, or zero at and below it.
    pub fn excess(self, segments: usize, fam: f64) -> f64 {
        (segments as f64 - self.segments as f64 + (fam - self.fam)).max(0.0)
    }
}

/// Remove up to `amount` mass, keeping at least two segments.
/// Returns the segments removed.
pub fn remove_mass(segments: &mut usize, fam: &mut f64, amount: f64) -> usize {
    remove_mass_above_floor(segments, fam, amount, None)
}

/// The optional mode floor is internal so the default public API and overlaid
/// baseline tests keep the same three-argument mass-removal contract.
pub(crate) fn remove_mass_above_floor(
    segments: &mut usize,
    fam: &mut f64,
    amount: f64,
    floor: Option<MassFloor>,
) -> usize {
    let amount = floor.map_or(amount, |f| amount.min(f.excess(*segments, *fam)));
    let before = *segments;
    *fam -= amount;
    while *fam < 0.0 && *segments > 2 {
        *segments -= 1;
        *fam += 1.0;
    }
    *fam = fam.max(0.0);
    // `amount` stops at the floor, so landing below it is only rounding.
    if let Some(f) = floor
        && amount > 0.0
        && (*segments, *fam) < (f.segments, f.fam)
    {
        (*segments, *fam) = (f.segments, f.fam);
    }
    before.saturating_sub(*segments)
}
pub fn quantize_food(value: f64) -> f64 {
    (value / (300.0 / 256.0)).floor() * (300.0 / 256.0)
}
/// Candidate death-food sites on anchors and their midpoints. Received small
/// and stretched deaths fit this count; the optional density limit filters
/// these candidates after scattering.
pub fn death_food_sites(
    body: &[Point],
    head: Point,
) -> impl DoubleEndedIterator<Item = Point> + '_ {
    std::iter::once(head).chain(body.iter().enumerate().flat_map(move |(index, &point)| {
        let previous = if index == 0 { head } else { body[index - 1] };
        let midpoint = Point {
            x: (previous.x + point.x) * 0.5,
            y: (previous.y + point.y) * 0.5,
        };
        [midpoint, point]
    }))
}

/// Empirical size fit, quantized to the observed 0.2 units. Validated deaths span
/// 2..244 segments; beyond that this extrapolates, not recovered server code.
pub fn death_food_size(segments: usize) -> f64 {
    let n = segments as f64;
    ((13.6 + 1.375e-7 * n * n * n) * 5.0).round() / 5.0
}
pub fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    p.distance(closest_on_segment(p, a, b))
}

/// The same projection and boundary result as `segment_distance(p, a, b) <= r`.
/// Keep ambiguous distances on Point's original hypot path.
#[cfg(feature = "networking")]
pub(crate) fn segment_distance_le(p: Point, a: Point, b: Point, r: f64) -> bool {
    p.distance_le(closest_on_segment(p, a, b), r)
}

fn closest_on_segment(p: Point, a: Point, b: Point) -> Point {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let denom = dx * dx + dy * dy;
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / if denom == 0.0 { 1.0 } else { denom })
        .clamp(0.0, 1.0);
    Point {
        x: a.x + t * dx,
        y: a.y + t * dy,
    }
}
pub fn swept_distance(a: Point, b: Point, c: Point, d: Point) -> f64 {
    fn cross(p: Point, q: Point, r: Point) -> f64 {
        (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)
    }
    if cross(a, b, c) * cross(a, b, d) < 0.0 && cross(c, d, a) * cross(c, d, b) < 0.0 {
        return 0.0;
    }
    segment_distance(a, c, d)
        .min(segment_distance(b, c, d))
        .min(segment_distance(c, a, b))
        .min(segment_distance(d, a, b))
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: Point,
    pub max: Point,
}
impl Bounds {
    pub fn between(a: Point, b: Point) -> Self {
        Self {
            min: Point {
                x: a.x.min(b.x),
                y: a.y.min(b.y),
            },
            max: Point {
                x: a.x.max(b.x),
                y: a.y.max(b.y),
            },
        }
    }
    pub fn include(&mut self, p: Point) {
        self.min.x = self.min.x.min(p.x);
        self.min.y = self.min.y.min(p.y);
        self.max.x = self.max.x.max(p.x);
        self.max.y = self.max.y.max(p.y);
    }
    pub fn near(self, other: Self, r: f64) -> bool {
        !(self.max.x < other.min.x - r
            || self.min.x > other.max.x + r
            || self.max.y < other.min.y - r
            || self.min.y > other.max.y + r)
    }
}

#[cfg(test)]
mod directed_geometry_tests {
    use super::*;

    #[cfg(feature = "networking")]
    #[test]
    fn segment_comparison_matches_original_projection_and_hypot() {
        // Freeze the original scalar projection independently of the shared
        // helper, including its zero-length denominator and clamp order.
        fn original(p: Point, a: Point, b: Point) -> f64 {
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let denom = dx * dx + dy * dy;
            let t = (((p.x - a.x) * dx + (p.y - a.y) * dy)
                / if denom == 0.0 { 1.0 } else { denom })
            .clamp(0.0, 1.0);
            p.distance(Point {
                x: a.x + t * dx,
                y: a.y + t * dy,
            })
        }
        let mut state = 0x73ac_128d_5479_u64;
        let mut coordinate = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 65536.
        };
        let special = [
            0.,
            -0.,
            f64::MIN_POSITIVE,
            1e-200,
            1e200,
            f64::MAX,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];
        for i in 0..100_000 {
            let p = Point {
                x: coordinate(),
                y: coordinate(),
            };
            let mut a = Point {
                x: coordinate(),
                y: coordinate(),
            };
            let mut b = Point {
                x: coordinate(),
                y: coordinate(),
            };
            match i % 5 {
                0 => b = a,
                1 => {
                    a.x = special[i / 5 % special.len()];
                    b.y = a.x;
                }
                _ => {}
            }
            let distance = original(p, a, b);
            assert_eq!(segment_distance(p, a, b).to_bits(), distance.to_bits());
            for r in [
                distance,
                distance.next_down(),
                distance.next_up(),
                coordinate().abs(),
                special[i % special.len()],
                -1.,
            ] {
                assert_eq!(
                    segment_distance_le(p, a, b, r),
                    distance <= r,
                    "p={p:?}, a={a:?}, b={b:?}, r={r:?}"
                );
            }
        }
    }

    fn point_bits(point: Point) -> (u64, u64) {
        (point.x.to_bits(), point.y.to_bits())
    }

    #[test]
    fn cached_steering_matches_uncached_motion_across_input_changes() {
        let mut cache = SteeringCache::default();
        let angles = [0.0, -0.0, PI, -PI, TAU, -TAU, 1e6, -1e6, 1e100, -1e100];
        let sizes = [2, 3, 106, 430, 512, usize::MAX];
        let steps = [0.0, -0.0, LOOP_SECONDS, LOOP_SECONDS * 4.0, 1.0];
        for i in 0..2048 {
            let heading = angles[i % angles.len()];
            let target = angles[(i / angles.len()) % angles.len()];
            let segments = sizes[(i / 3) % sizes.len()];
            let dt = steps[(i / 7) % steps.len()];
            // The first call changes keys; subsequent calls reuse steering
            // while varying the position and speed that must never be cached.
            for j in 0..3 {
                let head = Point {
                    x: if j == 0 { -0.0 } else { i as f64 * 123.0 },
                    y: -(j as f64) * 128.0,
                };
                let speed = [0.0, base_speed(segments), BOOST_SPEED][j];
                let expected = advance(
                    head,
                    heading,
                    quantize_heading(target),
                    speed,
                    turn_rate(segments),
                    dt,
                );
                let actual = cache.advance(head, heading, target, speed, segments, dt);
                assert_eq!(
                    point_bits(actual.0),
                    point_bits(expected.0),
                    "position {i}/{j}"
                );
                assert_eq!(actual.1.to_bits(), expected.1.to_bits(), "heading {i}/{j}");
                assert_eq!(
                    (actual.2.0.to_bits(), actual.2.1.to_bits()),
                    (expected.1.cos().to_bits(), expected.1.sin().to_bits()),
                    "direction {i}/{j}"
                );
            }
        }
    }

    #[test]
    fn directed_geometry_matches_public_oracles_bit_for_bit() {
        let angles = [0.0, -0.0, PI, -PI, TAU, -TAU, 1e6, -1e6, 1e100, -1e100];
        let coordinates = [
            0.0,
            -0.0,
            128.0f64.next_down(),
            128.0,
            128.0f64.next_up(),
            -128.0f64.next_down(),
            -128.0,
            -128.0f64.next_up(),
            -((1u64 << 20) as f64),
            (1u64 << 20) as f64,
            -1e7,
            1e7,
        ];
        let sizes = [2, 3, 106, 430, 512, 10_000, usize::MAX];
        let mut random = 0x6a09_e667_f3bc_c909u64;
        let mut unit = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            (random >> 11) as f64 / (1u64 << 53) as f64
        };
        for i in 0..65_536 {
            let heading = if i < 4096 {
                angles[i % angles.len()]
            } else {
                (unit() - 0.5) * 1e6
            };
            let head = Point {
                x: coordinates[i % coordinates.len()],
                y: (unit() - 0.5) * 2e7,
            };
            let target = if i < 4096 {
                angles[(i / angles.len()) % angles.len()]
            } else {
                (unit() - 0.5) * TAU * 4.0
            };
            let speed = [0.0, base_speed(2), BOOST_SPEED, 1000.0][i % 4];
            let turn = [0.0, turn_rate(2), turn_rate(430)][i % 3];
            let dt = [0.0, LOOP_SECONDS, LOOP_SECONDS * 4.0, 1.0][i % 4];
            let segments = sizes[i % sizes.len()];
            let expected = advance(head, heading, target, speed, turn, dt);
            let actual = advance_directed(head, heading, target, speed, turn, dt);
            assert_eq!(
                point_bits(actual.0),
                point_bits(expected.0),
                "motion case {i}"
            );
            assert_eq!(actual.1.to_bits(), expected.1.to_bits(), "heading case {i}");
            assert_eq!(
                (actual.2.0.to_bits(), actual.2.1.to_bits()),
                (expected.1.cos().to_bits(), expected.1.sin().to_bits()),
                "direction case {i}"
            );
            let expected_tip = head_tip(expected.0, expected.1, segments);
            let body_radius = BODY_RADIUS * scale(segments);
            let actual_tip = Point {
                x: actual.0.x + actual.2.0 * body_radius,
                y: actual.0.y + actual.2.1 * body_radius,
            };
            assert_eq!(
                point_bits(actual_tip),
                point_bits(expected_tip),
                "tip case {i}"
            );
            let expected = pickup_circle(expected.0, expected.1, speed, segments);
            let actual = pickup_circle_directed(actual.0, actual.2, speed, segments);
            assert_eq!(
                point_bits(actual.0),
                point_bits(expected.0),
                "circle case {i}"
            );
            assert_eq!(actual.1.to_bits(), expected.1.to_bits(), "radius case {i}");
        }
    }
    // Frozen scalar reference: do not share the new threshold helper.
    fn original_strikes_threshold(
        last: Point,
        tip: Point,
        direction: (f64, f64),
        head_scale: f64,
        body_scale: f64,
        line: &[Point],
    ) -> bool {
        fn intersect(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
            let (a1c, b1c) = (a0.y - a1.y, a1.x - a0.x);
            let c1 = a1c * a1.x + b1c * a1.y;
            let (a2c, b2c) = (b1.y - b0.y, b0.x - b1.x);
            let c2 = a2c * b0.x + b2c * b0.y;
            let det = a1c * b2c - a2c * b1c;
            if det == 0.0 {
                return false;
            }
            let x = (b2c * c1 - b1c * c2) / det;
            let y = (a1c * c2 - a2c * c1) / det;
            let within = |v: f64, p: f64, q: f64| {
                if p < q {
                    p <= v && v <= q
                } else if p > q {
                    q <= v && v <= p
                } else {
                    v == p
                }
            };
            within(x, a0.x, a1.x)
                && within(y, a0.y, a1.y)
                && within(x, b0.x, b1.x)
                && within(y, b0.y, b1.y)
        }
        let radius = 14.5 * body_scale;
        let reach = 3.666 * (14.5 * (head_scale + body_scale));
        let end = Point {
            x: tip.x + direction.0 * radius,
            y: tip.y + direction.1 * radius,
        };
        for edge in line.windows(2) {
            let b = edge[1];
            if (b.x - tip.x).abs().max((b.y - tip.y).abs()) < reach
                && (intersect(last, end, edge[0], b)
                    || (tip.x - b.x).hypot(tip.y - b.y) < 0.975 * radius)
            {
                return true;
            }
        }
        false
    }

    #[test]
    fn strikes_threshold_matches_original_at_float_boundaries() {
        for origin in [
            0.0,
            -0.0,
            512.0,
            -512.0,
            1e15,
            -1e15,
            1e300,
            f64::MAX,
            f64::INFINITY,
            f64::NAN,
        ] {
            for body_scale in [
                -1.0,
                -0.0,
                0.0,
                1e-202,
                1e-50,
                1.0,
                1.25,
                6.0,
                1e50,
                1e200,
                f64::INFINITY,
                f64::NAN,
            ] {
                let r = 0.975 * (14.5 * body_scale);
                let tip = Point {
                    x: origin,
                    y: origin,
                };
                for d in [
                    0.0,
                    r.next_down(),
                    r,
                    r.next_up(),
                    -r,
                    f64::MIN_POSITIVE,
                    f64::MAX,
                    f64::INFINITY,
                    f64::NAN,
                ] {
                    for head_scale in [1.0, 6.0, -1.0, f64::INFINITY, f64::NAN] {
                        let b = Point {
                            x: tip.x,
                            y: tip.y + d,
                        };
                        let line = [b, b];
                        let expected = original_strikes_threshold(
                            tip,
                            tip,
                            (1.0, 0.0),
                            head_scale,
                            body_scale,
                            &line,
                        );
                        assert_eq!(
                            strikes_scaled(tip, tip, (1.0, 0.0), head_scale, body_scale, line),
                            expected,
                            "origin={origin:?} scale={body_scale:?} d={d:?} head={head_scale:?}"
                        );
                    }
                }
            }
        }
        let reach = 3.666 * (14.5 * (1.0 + 1.0));
        let line = [
            Point {
                x: -reach,
                y: -10.0,
            },
            Point { x: reach, y: 10.0 },
        ];
        let last = Point { x: -3.0, y: 0.0 };
        let tip = Point::default();
        assert!(!original_strikes_threshold(
            last,
            tip,
            (1.0, 0.0),
            1.0,
            1.0,
            &line
        ));
        assert!(!strikes_scaled(last, tip, (1.0, 0.0), 1.0, 1.0, line));
    }

    #[test]
    fn strikes_threshold_matches_original_random_lines() {
        let mut seed = 0x4a19_c33b_6d83_7f21_u64;
        let mut unit = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 11) as f64 / (1_u64 << 53) as f64
        };
        let sizes = [2, 3, 10, 106, 430, 512, usize::MAX];
        for case in 0_usize..4096 {
            let extent = [1e-200, 1.0, 2e4, 1e150][case % 4];
            let tip = Point {
                x: (unit() - 0.5) * extent,
                y: (unit() - 0.5) * extent,
            };
            let last = Point {
                x: tip.x + (unit() - 0.5) * 20.0,
                y: tip.y + (unit() - 0.5) * 20.0,
            };
            let angle = unit() * TAU;
            let direction = (angle.cos(), angle.sin());
            let head_scale = scale(sizes[case % sizes.len()]);
            let body_scale = scale(sizes[(case / 7) % sizes.len()]);
            let r = 0.975 * (14.5 * body_scale);
            let mut line = Vec::new();
            for k in 0..case % 17 {
                let d = match k % 4 {
                    0 => r.next_down(),
                    1 => r,
                    2 => r.next_up(),
                    _ => unit() * 500.0,
                };
                let angle = unit() * TAU;
                line.push(Point {
                    x: tip.x + angle.cos() * d,
                    y: tip.y + angle.sin() * d,
                });
            }
            let expected =
                original_strikes_threshold(last, tip, direction, head_scale, body_scale, &line);
            assert_eq!(
                strikes_scaled(last, tip, direction, head_scale, body_scale, line),
                expected,
                "case {case}"
            );
        }
    }
}
