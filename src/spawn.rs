use crate::{Config, Point};
use std::f64::consts::{FRAC_PI_2, TAU};

/// Head distance of a new life: `config.spawn_radius`, or the official grid of
/// `sample_spawn_distance`. Callers draw `unit` either way, so the setting never
/// shifts the rest of their spawn stream.
pub fn spawn_distance(config: &Config, unit: f64) -> f64 {
    config
        .spawn_radius
        .unwrap_or_else(|| sample_spawn_distance(config.radius, unit))
}

/// A `segments`-anchor body along the circle through `head` around the center,
/// trailing the head at the 42-unit anchor spacing, and its tangent heading.
/// It turns counterclockwise when `heading` leans that way, else clockwise.
/// `None` when the circle cannot hold the body without wrapping onto itself.
/// A mode design choice for multi-segment spawns; see docs/configuration.md#battledome.
pub fn ring_body(head: Point, heading: f64, segments: usize) -> Option<(Vec<Point>, f64)> {
    let radius = head.distance(Point::default());
    if !radius.is_finite() || radius <= 21.0 {
        return None;
    }
    // Angle between neighboring anchors 42 units apart on the circle.
    let step = 2.0 * (21.0 / radius).asin();
    if segments as f64 * step >= TAU {
        return None;
    }
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
