//! Measured radius changes and an approximate, locally calibrated rim constraint.
use crate::model::Point;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryMode {
    Hard,
    /// Tangential projection with a forward contact point. See boundary-evidence.md.
    #[default]
    Sliding,
}

/// Returns the corrected head and whether contact is lethal. Most snakes only
/// take the squared-distance fast path; no body/food scans or allocations.
pub fn constrain(
    head: Point,
    heading: f64,
    scale: f64,
    radius: f64,
    mode: BoundaryMode,
) -> (Point, bool) {
    let distance_squared = head.x * head.x + head.y * head.y;
    if mode == BoundaryMode::Hard {
        return (head, distance_squared >= radius * radius);
    }
    // Fit to received anchors, not the rendered/predicted head outside the rim.
    let lead = 16.0 * scale;
    let inner = (radius - lead).max(0.0);
    if distance_squared < inner * inner {
        return (head, false);
    }
    // A large radius discontinuity or an already-outside head is not a teleport.
    if distance_squared >= radius * radius {
        return (head, true);
    }
    let (dy, dx) = heading.sin_cos();
    let tip = Point {
        x: head.x + lead * dx,
        y: head.y + lead * dy,
    };
    let tip_squared = tip.x * tip.x + tip.y * tip.y;
    if tip_squared < radius * radius {
        return (head, false);
    }
    let distance = tip_squared.sqrt();
    // sin(0.7): the observed transition is near 40 degrees outward from tangent.
    // Input quantization and packet timing limit the precision of this estimate.
    if (tip.x * dx + tip.y * dy) > 0.644217687237691 * distance {
        return (head, true);
    }
    let ratio = radius / distance;
    (
        Point {
            x: tip.x * ratio - lead * dx,
            y: tip.y * ratio - lead * dy,
        },
        false,
    )
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RadiusUpdate {
    pub time: f64,
    pub radius: f64,
}

pub(crate) fn validate(updates: &[RadiusUpdate]) -> Result<(), String> {
    if updates
        .iter()
        .any(|u| !u.time.is_finite() || u.time < 0. || !u.radius.is_finite() || u.radius <= 0.)
        || updates.windows(2).any(|w| w[0].time >= w[1].time)
    {
        return Err(
            "Radius updates require increasing nonnegative times and positive finite radii".into(),
        );
    }
    Ok(())
}
