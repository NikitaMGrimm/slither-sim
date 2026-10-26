//! Opt-in phase timing. Normal stepping compiles out clocks and counters.
use serde::Serialize;
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepProfile {
    pub ticks: u64,
    pub movement_seconds: f64,
    pub collision_seconds: f64,
    pub death_seconds: f64,
    pub food_seconds: f64,
    pub prey_seconds: f64,
    pub ambient_seconds: f64,
    pub body_pairs: u64,
    pub segment_tests: u64,
    pub food_pairs: u64,
    pub pickup_tests: u64,
}

/// Read-only world counters for diagnostics.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldStats {
    pub time: f64,
    pub physics_tick_ms: f64,
    pub arena_radius: f64,
    pub spawned: usize,
    pub living: usize,
    pub living_segments: usize,
    pub food: usize,
    pub prey: usize,
    pub ambient_food: usize,
    pub boost_food: usize,
    pub death_food: usize,
    pub deaths: usize,
}
impl WorldStats {
    pub fn from_world(world: &crate::Simulator) -> Self {
        use crate::Origin;
        let mut food_counts = [0; 3];
        for f in world.food() {
            match f.origin {
                Some(Origin::Ambient) => food_counts[0] += 1,
                Some(Origin::Boost) => food_counts[1] += 1,
                Some(Origin::Death) => food_counts[2] += 1,
                None => {}
            }
        }
        Self {
            time: world.time(),
            physics_tick_ms: world.config().dt * 1000.0,
            arena_radius: world.config().radius,
            spawned: world.snakes().len(),
            living: world.living(),
            living_segments: world
                .snakes()
                .iter()
                .filter(|s| s.alive)
                .map(|s| s.body.len())
                .sum(),
            food: world.food().len(),
            prey: world.prey().len(),
            ambient_food: food_counts[0],
            boost_food: food_counts[1],
            death_food: food_counts[2],
            deaths: world.deaths().len(),
        }
    }
}
