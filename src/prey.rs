//! Moving edible entities. Authoritative positions are quantized each server tick;
//! client prediction is smooth. Steering and population control remain estimates.
use crate::{
    model::*,
    physics::{REFERENCE_FRAME_SECONDS, SPEED_UNIT_SECONDS, advance},
};
use serde::{Deserialize, Serialize};

/// Explicit replay state. The official trigger for outward departure is unknown.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PreyMode {
    #[default]
    Roaming,
    Outward,
}

pub(crate) const OUTWARD_SPEED: f64 = 6.060 / SPEED_UNIT_SECONDS;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Prey {
    pub id: u64,
    pub position: Point,
    pub heading: f64,
    pub target_heading: f64,
    /// World units per second (client `sp / .032`).
    pub speed: f64,
    pub size: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct PreyConfig {
    pub count: usize,
    pub cruise_speed: f64,
    /// Flight target near a minimum-speed snake (protocol speed 5.79).
    pub escape_speed: f64,
    /// Flight target near a full-boost snake (protocol speed 14).
    pub sprint_speed: f64,
    pub turn_rate: f64,
    /// Half-width of the axis-aligned threat square, not a Euclidean radius.
    pub flee_half_width: f64,
    pub size: f64,
    pub max_size: f64,
    pub nutrition_factor: f64,
    pub respawn_seconds: f64,
}
impl Default for PreyConfig {
    fn default() -> Self {
        Self {
            // One moving edible entity even in the smallest official world.
            // Config::official scales its initial target; sandbox opts out.
            count: 1,
            cruise_speed: 3.8 / SPEED_UNIT_SECONDS,
            escape_speed: 6.153 / SPEED_UNIT_SECONDS,
            sprint_speed: 10.997 / SPEED_UNIT_SECONDS,
            turn_rate: 0.029 / REFERENCE_FRAME_SECONDS,
            flee_half_width: 320.0,
            size: 6.0,
            // Observed range; the server's spawn distribution remains unknown.
            max_size: 9.2,
            // Rounded empirical estimate: isolated catch ~30.22 food equivalents.
            nutrition_factor: 30.0,
            respawn_seconds: 1.0,
        }
    }
}
impl PreyConfig {
    pub(crate) fn size_steps(&self) -> Option<(f64, f64)> {
        let mut lower = (self.size * 5.0).ceil().max(1.0);
        let mut upper = (self.max_size * 5.0).floor();
        // Multiplication can round an adjacent float onto a grid endpoint.
        if lower / 5.0 < self.size {
            lower += 1.0;
        }
        if upper / 5.0 > self.max_size {
            upper -= 1.0;
        }
        (lower.is_finite()
            && upper.is_finite()
            && lower <= upper
            && lower / 5.0 >= self.size
            && upper / 5.0 <= self.max_size)
            .then_some((lower, upper))
    }

    /// Empirical flight speed follows the threatening snake's protocol speed.
    /// The measured default endpoints are (5.79, 6.153) and (14, 10.997).
    pub fn flight_speed(&self, snake_speed: f64) -> f64 {
        let fraction = ((snake_speed * SPEED_UNIT_SECONDS - 5.79) / (14.0 - 5.79)).clamp(0.0, 1.0);
        self.escape_speed + (self.sprint_speed - self.escape_speed) * fraction
    }

    pub fn validate(&self) -> Result<(), String> {
        let values = [
            self.cruise_speed,
            self.escape_speed,
            self.sprint_speed,
            self.turn_rate,
            self.flee_half_width,
            self.size,
            self.max_size,
            self.nutrition_factor,
            self.respawn_seconds,
        ];
        if self.count > 10000
            || values.iter().any(|x| !x.is_finite() || *x <= 0.0)
            || self.escape_speed < self.cruise_speed
            || self.sprint_speed < self.escape_speed
            || self.max_size < self.size
            || self.size_steps().is_none()
        {
            return Err("Invalid prey settings".into());
        }
        Ok(())
    }
}

impl Prey {
    /// Client prediction with a shortest-turn target. Rendering offsets are excluded.
    pub fn advance(&mut self, dt: f64, turn_rate: f64) {
        (self.position, self.heading) = advance(
            self.position,
            self.heading,
            self.target_heading,
            self.speed,
            turn_rate,
            dt,
        );
    }

    /// Authoritative movement measured from incoming prey positions. The official
    /// server floors absolute coordinates after each ~7 ms step. Arena-centered
    /// coordinates require floor too, including negative values, because the
    /// original positive world origin is an integer translation.
    pub fn advance_server_tick(&mut self, turn_rate: f64) {
        self.advance(crate::physics::LOOP_SECONDS, turn_rate);
        self.position.x = self.position.x.floor();
        self.position.y = self.position.y.floor();
    }
}
