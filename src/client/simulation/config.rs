//! Configuration and protocol bounds for the virtual-time backend.
use super::{MAX_AGENTS, MAX_PREY, snakes::FRESH_ANCHORS};
use crate::{
    Config,
    client::{encode, latency::LatencyConfig},
    physics,
};
use serde::{Deserialize, Serialize};

/// Packet release policy, independent of the link's delivery delay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeliveryProfile {
    /// Measured official pacing; train bots meant for official servers here.
    Official,
    /// Our design for players: prompt steering, speed and mass reports.
    #[default]
    Responsive,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulationConfig {
    pub world: Config,
    /// When visible state is sent; independent of `latency`.
    pub delivery: DeliveryProfile,
    /// Snakes joined at start on the deterministic `Placement::TestRing`.
    pub agents: usize,
    pub controlled_slot: usize,
    /// Physics ticks between `next_frame`/`step_all` decisions.
    pub decision_ticks: usize,
    /// Seconds between minimaps on one server-wide clock; connections join its
    /// running phase. The default approximates the observed period.
    pub minimap_interval_seconds: f64,
    /// None selects 256 for pit worlds and 136 otherwise; Some overrides it.
    pub minimap_side: Option<usize>,
    /// Seconds between leaderboards on their own server-wide clock. The
    /// default approximates the observed period.
    pub leaderboard_interval_seconds: f64,
    /// Offer a world-run record and victory entry. This local rule is not an
    /// established official-server qualification policy.
    pub local_records: bool,
    pub latency: LatencyConfig,
    /// Retain each slot's connect/send/receive/disconnect records for replay.
    pub record_wire: bool,
}

// Derived struct deserializers also accept positional arrays, which would
// silently start defaults; `Deserialize` below admits objects only.
#[derive(Deserialize)]
#[serde(
    remote = "SimulationConfig",
    default = "SimulationConfig::default",
    deny_unknown_fields,
    rename_all = "camelCase"
)]
struct SimulationConfigWire {
    world: Config,
    delivery: DeliveryProfile,
    agents: usize,
    controlled_slot: usize,
    decision_ticks: usize,
    minimap_interval_seconds: f64,
    minimap_side: Option<usize>,
    leaderboard_interval_seconds: f64,
    local_records: bool,
    latency: LatencyConfig,
    record_wire: bool,
}

impl<'de> Deserialize<'de> for SimulationConfig {
    /// Input passes through `serde_json::Value`, so a repeated key keeps its
    /// last value, as in the world [`Config`].
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let fields = serde_json::Map::deserialize(deserializer)?;
        SimulationConfigWire::deserialize(serde_json::Value::Object(fields))
            .map_err(D::Error::custom)
    }
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            world: Config::default(),
            delivery: DeliveryProfile::Responsive,
            agents: 1,
            controlled_slot: 0,
            decision_ticks: 6,
            // Official HUD clocks are shared by all recipients and average
            // 2503.994/3008.062 ms, not their nominal 2.5/3 s; see the
            // scheduler study linked from `super::snakes::MOTION_REPORT_TICKS`.
            // Clean #111 medians remain 2504/3008 ms; these fixed clocks are
            // models, not universal gaps. See the cadence-near-server fixture
            // and research/fidelity/2026-10-26-vps-timing/REPORT.md.
            minimap_interval_seconds: 2.504,
            minimap_side: None,
            leaderboard_interval_seconds: 3.008,
            local_records: false,
            latency: LatencyConfig::default(),
            record_wire: false,
        }
    }
}
impl SimulationConfig {
    /// Effective stock minimap resolution, including the pit-world default.
    pub fn minimap_side(&self) -> usize {
        self.minimap_side
            .unwrap_or(if self.world.pit.is_some() { 256 } else { 136 })
    }

    /// Checks the world as backends and hosts run it, after sizing it for
    /// `agents`; see [`Config::size_to_population`].
    pub fn validate(&self) -> Result<(), String> {
        self.validate_for_population(self.agents)
    }

    /// Hosts start with bots only; capacity bounds still cover every seat and
    /// recorded radius, including live-area prey growth.
    pub(crate) fn validate_for_population(&self, population: usize) -> Result<(), String> {
        if !(1..=MAX_AGENTS).contains(&self.agents) || self.controlled_slot >= self.agents {
            return Err(format!(
                "Simulation needs 1 to {MAX_AGENTS} agents, since protocol-19 snake IDs, \
                 leaderboard rank and population are u16, and a controlled slot among them"
            ));
        }
        let mut world = self.world.clone();
        world.size_for_population(population);
        world.validate().map_err(|error| error.to_string())?;
        // Live prey refill to `count`, and each needs one of the u16 wire IDs.
        if world.prey.count > MAX_PREY {
            return Err(format!(
                "Protocol-19 prey IDs are u16: prey count must be at most {MAX_PREY}"
            ));
        }
        self.latency.validate()?;
        for speed in [
            world.prey.cruise_speed,
            world.prey.escape_speed,
            world.prey.sprint_speed,
        ] {
            encode::speed_u16(speed).map_err(|error| format!("Prey {error}"))?;
        }
        // Appearances encode size in one byte of fifth-units. Check the spawn
        // grid, allowing range endpoints above 51 that still only produce 51.
        if world
            .prey
            .size_steps()
            .is_some_and(|(_, upper)| upper > f64::from(u8::MAX))
        {
            return Err("Protocol-19 simulation supports prey sizes up to 51".into());
        }
        if self.decision_ticks == 0 {
            return Err("Decision ticks must be at least 1".into());
        }
        if !self.minimap_interval_seconds.is_finite() || self.minimap_interval_seconds <= 0. {
            return Err("Minimap interval must be positive".into());
        }
        if !self.leaderboard_interval_seconds.is_finite() || self.leaderboard_interval_seconds <= 0.
        {
            return Err("Leaderboard interval must be positive".into());
        }
        if self
            .minimap_side
            .is_some_and(|side| !(1..=crate::client::minimap::MAX_SIDE).contains(&side))
        {
            return Err(format!(
                "Minimap side must be 1 through {}: the stock client clamps larger sides",
                crate::client::minimap::MAX_SIDE
            ));
        }
        let largest = world
            .radius_updates
            .iter()
            .map(|update| update.radius)
            .chain([world.radius])
            .chain(
                (world.dynamic_radius || world.size_to_population)
                    .then(|| world.seat_radius(self.agents)),
            )
            .fold(0., f64::max);
        if largest > encode::MAX_RADIUS {
            return Err(format!(
                "Protocol-19 simulation supports arena radii up to {}, a margin inside the \
                 ±32,550 u16 anchor coordinates",
                encode::MAX_RADIUS
            ));
        }
        if world.dynamic_radius && world.ecology_at_radius(largest).2 > MAX_PREY {
            return Err(format!(
                "Protocol-19 prey IDs are u16: live-area prey count must be at most {MAX_PREY}"
            ));
        }
        // After the radius limit, whose message is clearer than setup's range error.
        encode::setup(world.radius, world.prey.turn_rate)?;
        Ok(())
    }

    /// Valid settings outside measured behavior; see [`Config::warnings`].
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = self.world.warnings();
        // A head less than one spacing past its front anchor that travels less
        // than `FRESH_ANCHORS - 1` spacings crosses at most that many anchors.
        let window = (FRESH_ANCHORS - 1) as f64 * physics::ANCHOR_SPACING;
        if self.world.dt * physics::BOOST_SPEED >= window {
            warnings.push(format!(
                "dt × {} exceeds the {window}-unit incremental body-update window: snakes \
                 that outrun it in one tick are resent in full (more bandwidth)",
                physics::BOOST_SPEED
            ));
        }
        warnings
    }
}
