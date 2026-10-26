//! Measured radius changes and an approximate, locally calibrated rim constraint.
use crate::model::Point;
use crate::{Constraint, SimError};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryMode {
    Hard,
    /// Tangential projection with a forward contact point. See boundary-evidence.md.
    #[default]
    Sliding,
}

/// Forward contact lead per unit of snake scale.
pub const CONTACT_LEAD: f64 = 16.0;

/// sin(0.7): fitted lethal outward component; see the constraint rationale below.
pub const LETHAL_OUTWARD: f64 = 0.644217687237691;

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
        return (head, head.distance(Point::default()) >= radius);
    }
    // Fit to received anchors, not the rendered/predicted head outside the rim.
    let lead = CONTACT_LEAD * scale;
    let inner = (radius - lead).max(0.0);
    if distance_squared < inner * inner {
        return (head, false);
    }
    // A large radius discontinuity or an already-outside head is not a teleport.
    if head.distance(Point::default()) >= radius {
        return (head, true);
    }
    let (dy, dx) = heading.sin_cos();
    let tip = Point {
        x: head.x + lead * dx,
        y: head.y + lead * dy,
    };
    let distance = tip.distance(Point::default());
    if distance < radius {
        return (head, false);
    }
    // sin(0.7): the observed transition is near 40 degrees outward from tangent.
    // Input quantization and packet timing limit the precision of this estimate.
    if (tip.x * dx + tip.y * dy) > LETHAL_OUTWARD * distance {
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

pub(crate) fn validate(updates: &[RadiusUpdate]) -> Result<(), SimError> {
    let message = "Radius updates require increasing nonnegative times and positive finite radii";
    for (index, update) in updates.iter().enumerate() {
        for (field, rule) in [
            ("time", Constraint::nonnegative(update.time)),
            ("radius", Constraint::positive(update.radius)),
        ] {
            if let Some(rule) = rule {
                return Err(SimError::config(
                    rule,
                    &format!("radiusUpdates[{index}].{field}"),
                    message,
                ));
            }
        }
    }
    if let Some(index) = updates.windows(2).position(|w| w[0].time >= w[1].time) {
        return Err(SimError::config(
            Constraint::Ordering,
            &format!("radiusUpdates[{}].time", index + 1),
            message,
        ));
    }
    Ok(())
}
