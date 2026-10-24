//! Small, deterministic reference policy using only one bot's observation.
//!
//! The kernel has no persistent state, heap allocation or networking dependency.
//! Its safety margins and short prediction horizon are policy choices, not fitted
//! server rules. See the portable qualification study:
//! <https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-24-reference-policy/REPORT.md>.

use crate::{Action, Observation, Point, physics};
use std::f64::consts::{PI, TAU};

/// Seek food while steering away from nearby walls, heads and body edges.
///
/// Food is reselected each decision using distance plus turn cost. A fixed fan
/// of 13 headings is scored over four turn-limited samples spanning 0.36 seconds.
/// Ties keep the first food/heading in observation/fan order. Boost is always off.
/// The observation must use the crate's finite coordinates, radians and seconds;
/// received-state callers should use `client::ReferenceBot` with the
/// `networking` feature. This heuristic does not guarantee collision avoidance.
pub fn decide(observation: &Observation) -> Action {
    let me = &observation.me;
    let head = me.head;
    let center_heading = toward(head, observation.arena.center);
    let turn_rate = physics::turn_rate(me.segments);
    let turn_radius = me.speed / turn_rate;
    let mut target_heading = center_heading;
    let mut best_food_cost = f64::INFINITY;
    for food in &observation.food {
        let heading = toward(head, food.point());
        let cost = distance_squared(head, food.point()).sqrt()
            + 2.0 * turn_radius * wrap(heading - me.heading).abs();
        if cost < best_food_cost {
            best_food_cost = cost;
            target_heading = heading;
        }
    }
    if distance_squared(head, observation.arena.center).sqrt() > observation.arena.radius - 220.0 {
        target_heading = center_heading;
    }

    let mut best_score = f64::NEG_INFINITY;
    let mut best_heading = me.heading;
    for delta in [
        wrap(target_heading - me.heading),
        0.0,
        -0.3,
        0.3,
        -0.6,
        0.6,
        -1.0,
        1.0,
        -1.5,
        1.5,
        -2.2,
        2.2,
        PI,
    ] {
        let wanted = me.heading + delta;
        let mut position = head;
        let mut heading = me.heading;
        let mut clearance = f64::INFINITY;
        for _ in 0..4 {
            heading += wrap(wanted - heading).clamp(-turn_rate * 0.09, turn_rate * 0.09);
            position.x += heading.cos() * me.speed * 0.09;
            position.y += heading.sin() * me.speed * 0.09;
            clearance = clearance.min(
                observation.arena.radius
                    - distance_squared(position, observation.arena.center).sqrt()
                    - me.radius
                    - 20.0,
            );
            for opponent in &observation.opponents {
                if distance_squared(head, opponent.head) < 500.0_f64.powi(2) {
                    clearance = clearance.min(
                        distance_squared(position, opponent.head).sqrt()
                            - me.radius
                            - opponent.radius
                            - 25.0,
                    );
                }
                let mut previous = opponent.head;
                for &anchor in &opponent.body {
                    if distance_squared(head, previous) < 500.0_f64.powi(2)
                        || distance_squared(head, anchor) < 500.0_f64.powi(2)
                    {
                        clearance = clearance.min(
                            physics::segment_distance(position, previous, anchor)
                                - me.radius
                                - opponent.radius
                                - 15.0,
                        );
                    }
                    previous = anchor;
                }
            }
        }
        let danger = if clearance < 0.0 {
            100.0 - clearance * 5.0
        } else {
            0.0
        };
        let score = 2.0 * wrap(wanted - target_heading).cos() - 0.08 * delta.abs()
            + 0.004 * clearance.clamp(-1000.0, 150.0)
            - danger;
        if score > best_score {
            best_score = score;
            best_heading = wanted;
        }
    }
    Action {
        heading: best_heading,
        boost: false,
    }
}

fn wrap(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn toward(from: Point, to: Point) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}

fn distance_squared(a: Point, b: Point) -> f64 {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2)
}
