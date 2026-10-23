//! Server rules measured from protocol-19 captures (`tools/fidelity/slither-headless.mjs`,
//! analyzed on the server's elapsed-ms clock) and constants the server sends to clients.
//! Speeds are world units per second; protocol speed converts as `sp / 0.032`.
//! Turning and boost progress use a separate 8 ms reference interval. Neither
//! conversion is the physics tick. See docs/server-rules.md for each fit's evidence.
use crate::model::Point;
use serde::{Deserialize, Serialize};
use std::{
    f64::consts::{PI, TAU},
    sync::OnceLock,
};

/// Average server loop period: frame gaps are multiples of ~7 ms (6.94-7.07 ms per server).
/// Snake motion scales with elapsed time. Prey coordinate rounding occurs once
/// per loop, so its effective velocity also depends on this period.
pub const LOOP_SECONDS: f64 = 0.007;
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

pub fn scale(segments: usize) -> f64 {
    (1.0 + (segments as f64 - 2.0) / 106.0).clamp(1.0, 6.0)
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
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
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
    pub fn with_progress(speed: f64, progress: f64) -> Result<Self, String> {
        if !speed.is_finite()
            || speed < 0.0
            || !progress.is_finite()
            || !(0.0..=1.0).contains(&progress)
        {
            return Err(
                "Speed state requires finite nonnegative speed and progress in 0..1".into(),
            );
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
/// Mass removed per boost pellet before its +-7% jitter; fitted 0.0812 (1 - sct/430)^1.66.
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
    let body_radius = BODY_RADIUS * scale(body_segments);
    let reach = 3.666 * collision_distance(head_segments, body_segments);
    let end = Point {
        x: tip.x + heading.cos() * body_radius,
        y: tip.y + heading.sin() * body_radius,
    };
    let mut points = line.into_iter();
    let Some(mut a) = points.next() else {
        return false;
    };
    for b in points {
        if (b.x - tip.x).abs().max((b.y - tip.y).abs()) < reach
            && (segments_intersect(last_tip, end, a, b) || tip.distance(b) < 0.975 * body_radius)
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
pub fn append_anchor(body: &mut Vec<Point>, anchor: Point, grow: bool) {
    // Fresh server body snapshots match contraction of the existing body before
    // insertion. The stock client does the opposite; see docs/server-rules.md#movement-and-geometry.
    contract_body(body);
    body.insert(0, anchor);
    if !grow {
        body.pop();
    }
}

/// Stock-client reconstruction, retained separately from authoritative geometry.
pub fn append_client_anchor(body: &mut Vec<Point>, anchor: Point, grow: bool) {
    body.insert(0, anchor);
    if !grow {
        body.pop();
    }
    contract_body(body);
}

fn contract_body(body: &mut [Point]) {
    for i in 3..body.len() {
        let weight = 0.43 * ((i - 2) as f64 / 4.0).min(1.0);
        body[i].x += (body[i - 1].x - body[i].x) * weight;
        body[i].y += (body[i - 1].y - body[i].y) * weight;
    }
}
pub fn crossed_anchor(_previous: Point, head: Point, last: Point) -> Option<Point> {
    if head.distance(last) < 42.0 {
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
/// Continuous score used by the provisional kiosk penalty. The public score
/// still rounds down exactly as the official score display does.
pub(crate) fn score_exact(segments: usize, fam: f64) -> f64 {
    static TABLE: OnceLock<([f64; 431], [f64; 431])> = OnceLock::new();
    let (weights, sums) = TABLE.get_or_init(|| {
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
    let n = segments.min(430);
    (sums[n] + fam.min(1.0) / weights[n] - 1.0) * 15.0 - 5.0
}
pub fn score(segments: usize, fam: f64) -> f64 {
    score_exact(segments, fam).floor()
}
pub fn remove_mass(segments: &mut usize, fam: &mut f64, amount: f64) -> usize {
    *fam -= amount;
    let mut removed = 0;
    while *fam < 0.0 && *segments > 2 {
        *segments -= 1;
        *fam += 1.0;
        removed += 1;
    }
    *fam = fam.max(0.0);
    removed
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
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let denom = dx * dx + dy * dy;
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / if denom == 0.0 { 1.0 } else { denom })
        .clamp(0.0, 1.0);
    p.distance(Point {
        x: a.x + t * dx,
        y: a.y + t * dy,
    })
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
