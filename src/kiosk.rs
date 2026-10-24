//! Provisional model of the 2024 kiosk penalty. Its phases come from the WORM
//! Labs study; the detector, rates and recovery curve await current captures.

use crate::model::{Point, Snake};
use crate::physics::{MAX_SEGMENTS, MassFloor, remove_mass_above_floor, score_exact};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct KioskConfig {
    pub enabled: bool,
    /// Minimum physical and displayed score, approximately 8,500 in the study.
    pub floor_score: f64,
    pub grace_seconds: f64,
    /// A head staying within this distance of its first qualifying position is
    /// considered to be kiosking. This is a proxy, not the server's detector.
    pub stationary_radius: f64,
    /// Exponential physical score decay while kiosking after the grace period.
    pub shrink_per_second: f64,
    /// Exponential displayed score decay after physical size reaches the floor.
    pub score_decay_per_second: f64,
    /// Exponential restoration of physical score after leaving the kiosk.
    pub recovery_per_second: f64,
}

impl Default for KioskConfig {
    fn default() -> Self {
        // Calibration data from the historical chart; runtime settings remain independent.
        let reference_score = 62_000.0_f64;
        let floor_score = 8_500.0;
        let shrink_seconds = 6.0 * 60.0;
        let score_decay_seconds = 120.0 * 60.0;
        let log_score_ratio = (reference_score / floor_score).ln();
        let shrink_rate = log_score_ratio / shrink_seconds;
        Self {
            enabled: true,
            floor_score,
            grace_seconds: 20.0 * 60.0,
            stationary_radius: 800.0,
            shrink_per_second: shrink_rate,
            score_decay_per_second: log_score_ratio / score_decay_seconds,
            // No measured recovery curve; use the same time scale as shrinkage.
            recovery_per_second: shrink_rate,
        }
    }
}

impl KioskConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !self.floor_score.is_finite()
            || self.floor_score <= 0.0
            || !self.grace_seconds.is_finite()
            || self.grace_seconds < 0.0
            || !self.stationary_radius.is_finite()
            || self.stationary_radius <= 0.0
            // A zero rate freezes that phase: exp(-0 * dt) keeps the score.
            || !self.shrink_per_second.is_finite()
            || self.shrink_per_second < 0.0
            || !self.score_decay_per_second.is_finite()
            || self.score_decay_per_second < 0.0
            || !self.recovery_per_second.is_finite()
            || self.recovery_per_second < 0.0
        {
            return Err("Invalid kiosk penalty configuration".into());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct KioskState {
    anchor: Option<Point>,
    held_seconds: f64,
    /// Displayed score minus the score implied by physical segment mass.
    score_credit: f64,
}

impl KioskState {
    /// Shrinking stops at `floor`, the world's `minScore` mass.
    pub(crate) fn step(
        &mut self,
        snake: &mut Snake,
        config: &KioskConfig,
        dt: f64,
        floor: Option<MassFloor>,
    ) {
        if !snake.alive {
            return;
        }
        if !config.enabled {
            self.anchor = None;
            self.held_seconds = 0.0;
            self.recover(snake, config, dt);
            return;
        }
        let physical = score_exact(snake.segments, snake.fam);
        // Physical shrinking ends at the higher of the kiosk and mass floors;
        // the displayed-score decay below keeps the kiosk floor.
        let shrink_floor = floor.map_or(config.floor_score, |floor| {
            config
                .floor_score
                .max(score_exact(floor.segments, floor.fam))
        });
        if physical <= shrink_floor && self.score_credit <= 0.0 {
            self.anchor = None;
            self.held_seconds = 0.0;
            return;
        }
        let anchor = *self.anchor.get_or_insert(snake.head);
        if snake.head.distance(anchor) > config.stationary_radius {
            self.anchor = Some(snake.head);
            self.held_seconds = 0.0;
            self.recover(snake, config, dt);
            return;
        }
        self.held_seconds += dt;
        if self.held_seconds + 1e-9 < config.grace_seconds {
            self.recover(snake, config, dt);
            return;
        }
        if physical > shrink_floor + 0.5 {
            let target = (physical * (-config.shrink_per_second * dt).exp()).max(shrink_floor);
            // At the mass floor, use its exact mass rather than a bisection estimate.
            let target_mass = match floor {
                Some(floor) if target <= score_exact(floor.segments, floor.fam) => {
                    floor.segments as f64 + floor.fam
                }
                _ => mass_for_score(target),
            };
            let old_mass = snake.segments as f64 + snake.fam;
            if target_mass < old_mass {
                let removed = remove_mass_above_floor(
                    &mut snake.segments,
                    &mut snake.fam,
                    old_mass - target_mass,
                    floor,
                );
                snake
                    .body
                    .truncate(snake.body.len().saturating_sub(removed));
                self.score_credit += physical - score_exact(snake.segments, snake.fam);
            }
        } else if self.score_credit > 0.0 {
            let display = physical + self.score_credit;
            let next_display = (display * (-config.score_decay_per_second * dt).exp())
                .max(config.floor_score.min(display));
            // Ordinary boost can put the physical body below the kiosk floor.
            // The penalty must never refund that ordinary score cost.
            self.score_credit = self.score_credit.min((next_display - physical).max(0.0));
        }
    }

    fn recover(&mut self, snake: &mut Snake, config: &KioskConfig, dt: f64) {
        if self.score_credit <= 0.0 || snake.fam >= 1.0 {
            return;
        }
        let physical = score_exact(snake.segments, snake.fam);
        let target =
            physical + self.score_credit * (1.0 - (-config.recovery_per_second * dt).exp());
        let mass = mass_for_score(target).min(snake.segments as f64 + 1.0);
        snake.fam = snake.fam.max(mass - snake.segments as f64);
        self.score_credit =
            (self.score_credit - (score_exact(snake.segments, snake.fam) - physical)).max(0.0);
    }

    pub(crate) fn finish_score(&self, snake: &mut Snake) {
        snake.length = snake.segments as f64 + snake.fam;
        snake.score = (score_exact(snake.segments, snake.fam) + self.score_credit).floor();
    }
}

fn mass_for_score(target: f64) -> f64 {
    let mut lo = 2.0;
    let mut hi = MAX_SEGMENTS as f64;
    for _ in 0..48 {
        let mid = (lo + hi) * 0.5;
        let segments = mid.floor() as usize;
        if score_exact(segments, mid - segments as f64) < target {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Simulator};

    #[test]
    fn historical_twenty_minute_grace_has_no_effect_before_its_boundary() {
        let mut world = Simulator::new(1, Config::sandbox()).unwrap();
        world.add("player", 0, 1).unwrap();
        let snake = &mut world.snakes[0];
        snake.segments = 380;
        snake.body = vec![Point::default(); 380];
        snake.fam = 0.0;
        let mut state = KioskState::default();
        let config = KioskConfig::default();
        let before = score_exact(snake.segments, snake.fam).floor();
        state.step(snake, &config, 1_199.0, None);
        state.finish_score(snake);
        assert_eq!(snake.segments, 380);
        assert_eq!(snake.score, before);
        state.step(snake, &config, 1.0, None);
        state.finish_score(snake);
        assert!(snake.segments < 380);
        assert_eq!(snake.score, before);
    }

    #[test]
    fn physical_boost_below_floor_cannot_create_new_score_credit() {
        let mut world = Simulator::new(1, Config::sandbox()).unwrap();
        world.add("player", 0, 1).unwrap();
        let snake = &mut world.snakes[0];
        let mass = mass_for_score(8_450.0);
        snake.segments = mass.floor() as usize;
        snake.fam = mass - snake.segments as f64;
        let mut state = KioskState {
            anchor: Some(snake.head),
            held_seconds: 1_200.0,
            score_credit: 10.0,
        };
        let previous_display = (score_exact(snake.segments, snake.fam) + 10.0).floor();
        state.step(snake, &KioskConfig::default(), 1.0, None);
        state.finish_score(snake);
        assert!(state.score_credit <= 10.0);
        assert!(snake.score <= previous_display);
    }
}
