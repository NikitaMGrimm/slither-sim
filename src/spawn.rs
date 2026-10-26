use crate::{Config, Point, physics};
use std::f64::consts::{FRAC_PI_2, PI, SQRT_2, TAU};

#[cfg(feature = "networking")]
mod safety;
#[cfg(feature = "networking")]
pub(crate) use safety::Safety;

// Custom circular policy, not official evidence: retain the strict 600-unit,
// two-second head-path zone; keep 400 from bodies plus a three-second normal
// head runway and departure corridors. The 120-second grid rejected cheaper
// safety settings; cached queries fund the larger candidate budget.
// research/battledome/2026-10-26-safe-spawn/REPORT.md
#[cfg(feature = "networking")]
pub(crate) const CLEARANCE: f64 = 600.0;
#[cfg(feature = "networking")]
pub(crate) const HEAD_PATH_SECONDS: f64 = 2.0;
// Circular admission spends more cheap cached queries so a failed random
// search does not monopolize admission. Official admission keeps its 64 draws.
#[cfg(feature = "networking")]
pub(crate) const CIRCULAR_ATTEMPTS: usize = 4096;
#[cfg(feature = "networking")]
pub(crate) const CIRCULAR_CLEARANCE: f64 = 400.0;
#[cfg(feature = "networking")]
pub(crate) const RETRY_SECONDS: f64 = 0.014;
#[cfg(feature = "networking")]
pub(crate) const DEPARTURE_SECONDS: f64 = 3.0;
#[cfg(feature = "networking")]
pub(crate) const DEPARTURE_PATH_CLEARANCE: f64 = 400.0;
#[cfg(feature = "networking")]
pub(crate) const WAITING: &str = "Spawn candidate search exhausted: no safe circular pose sampled";

/// Circular lives sample their configured band uniformly by area; a missing
/// outer radius keeps a single circle. None keeps the official grid and stream.
pub fn spawn_distance(config: &Config, unit: f64) -> f64 {
    match config.spawn_radius {
        Some(inner) => config.spawn_radius_max.map_or(inner, |outer| {
            // Interpolate normalized squared radii to avoid squaring a large
            // finite custom radius. Validation guarantees 0 < inner <= outer.
            outer * ((inner / outer).powi(2) * (1.0 - unit) + unit).sqrt()
        }),
        None => sample_spawn_distance(config.radius, unit),
    }
}

/// A `segments`-anchor body along the circle through `head` around the center,
/// trailing the head at the 42-unit anchor spacing, and its tangent heading.
/// It turns counterclockwise when `heading` leans that way, else clockwise.
/// `None` when the circle cannot hold the body without wrapping onto itself.
/// A mode design choice for `spawnRadius` lives; see docs/configuration.md#battledome.
pub fn ring_body(head: Point, heading: f64, segments: usize) -> Option<(Vec<Point>, f64)> {
    let radius = head.distance(Point::default());
    let step = ring_step(radius, segments)?;
    let angle = head.y.atan2(head.x);
    let turn = if (heading - angle - FRAC_PI_2).cos() >= 0.0 {
        1.0
    } else {
        -1.0
    };
    let body = (0..segments)
        .map(|i| {
            if i == 0 {
                return head;
            }
            let a = angle - turn * step * i as f64;
            Point {
                x: a.cos() * radius,
                y: a.sin() * radius,
            }
        })
        .collect();
    Some((body, angle + turn * FRAC_PI_2))
}

/// One-ulp error budgets in world units, including both components' Euclidean
/// error (sqrt(2)). Angles stay within +/-3*pi for a body shorter than a turn.
pub(crate) fn ring_roundoff(radius: f64) -> f64 {
    // Initial sin/cos, coordinate multiply and hypot radius round trip.
    let radial = 2.0 * SQRT_2 + 1.0;
    // atan2, step*index, angle subtraction, then sin/cos and multiply.
    let position = PI + TAU + 3.0 * PI + 2.0 * SQRT_2;
    // Division perturbs radius by epsilon; asin's absolute error is at most
    // (pi/2)*epsilon and changes a chord midpoint by at most radius times that.
    let chord = 1.0 + FRAC_PI_2;
    radius * (f64::EPSILON * (radial + position + chord))
}

/// Angle between neighboring anchors 42 units apart on a circle of `radius`,
/// leaving a head-tail gap wider than the body's actual diameter.
pub(crate) fn ring_step(radius: f64, segments: usize) -> Option<f64> {
    let half_spacing = physics::ANCHOR_SPACING / 2.0;
    if !radius.is_finite() || radius <= half_spacing || segments < 2 {
        return None;
    }
    let step = 2.0 * (half_spacing / radius).asin();
    // The tail is (segments - 1) steps behind the head. Reserve a chord of
    // two body radii, with one position-error budget at each end, rather than
    // one anchor spacing: large floor bodies are wider than 42 units.
    let body_radius = physics::BODY_RADIUS * physics::scale(segments);
    let gap = 2.0 * ((body_radius + ring_roundoff(radius)) / radius).asin();
    let occupied = (segments - 1) as f64 * step;
    let angle_roundoff = f64::EPSILON * (occupied + gap + TAU);
    (occupied > gap + angle_roundoff && occupied + gap < TAU - angle_roundoff).then_some(step)
}

/// Observed own-spawn radii lie on `0.009 + 0.011*n` of the arena radius.
/// The probability of each band is not identified: keep the area-weighted
/// draw and quantize its radius to the observed grid.
/// Callers supply a positive finite arena radius and a unit draw in [0, 1).
pub fn sample_spawn_distance(arena_radius: f64, unit: f64) -> f64 {
    let edge = arena_radius - 100.0;
    let continuous = unit.sqrt() * edge;
    const OFFSET: f64 = 0.009;
    const STEP: f64 = 0.011;
    // Tiny custom arenas may have no band inside the 100-unit edge margin.
    if edge < OFFSET * arena_radius {
        return 0.0;
    }
    let max_index = ((edge / arena_radius - OFFSET) / STEP).floor().max(0.0);
    let index = ((continuous / arena_radius - OFFSET) / STEP)
        .round()
        .clamp(0.0, max_index);
    (OFFSET + STEP * index) * arena_radius
}
