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
    #[default]
    Official,
    Responsive,
}
impl DeliveryProfile {
    fn is_official(&self) -> bool {
        *self == Self::Official
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    remote = "Self",
    default,
    deny_unknown_fields,
    rename_all = "camelCase"
)]
pub struct SimulationConfig {
    pub world: Config,
    /// When visible state is sent; independent of `latency`.
    // Preserve the official profile's existing config/trace JSON.
    #[serde(skip_serializing_if = "DeliveryProfile::is_official")]
    pub delivery: DeliveryProfile,
    /// Snakes joined at start on the deterministic test ring of `Simulator::add`.
    pub agents: usize,
    pub controlled_slot: usize,
    /// Physics ticks between `next_frame`/`step_all` decisions.
    pub decision_ticks: usize,
    /// Seconds between minimaps on one server-wide clock; connections join its
    /// running phase. The default is the official mean period.
    pub minimap_interval_seconds: f64,
    pub minimap_side: usize,
    /// Seconds between leaderboards on their own server-wide clock. The
    /// default is the official mean period.
    pub leaderboard_interval_seconds: f64,
    /// Offer a world-run record and victory entry. This local rule is not an
    /// established official-server qualification policy.
    pub local_records: bool,
    pub latency: LatencyConfig,
    /// Retain each slot's connect/send/receive/disconnect records for replay.
    pub record_wire: bool,
}
impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            world: Config::default(),
            delivery: DeliveryProfile::Official,
            agents: 1,
            controlled_slot: 0,
            decision_ticks: 6,
            // Official HUD clocks are shared by all recipients and average
            // 2503.994/3008.062 ms, not their nominal 2.5/3 s; see the
            // scheduler study linked from `super::snakes::MOTION_REPORT_TICKS`.
            minimap_interval_seconds: 2.504,
            // Retained protocol-19 minimap receipts and the earlier browser
            // bridge use 136 cells per side; 32 was a local backend shortcut.
            minimap_side: 136,
            leaderboard_interval_seconds: 3.008,
            local_records: false,
            latency: LatencyConfig::default(),
            record_wire: false,
        }
    }
}
impl Serialize for SimulationConfig {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Self::serialize(self, serializer)
    }
}
impl<'de> Deserialize<'de> for SimulationConfig {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        use serde_json::{Map, Value};
        let mut fields = Map::deserialize(deserializer)?;
        // Resolve while the preset name still exists: Config deserialization
        // expands it into world fields. Explicit delivery always wins, and a
        // custom pit/floor alone does not silently opt into responsive delivery.
        if !fields.contains_key("delivery")
            && fields
                .get("world")
                .and_then(|world| world.get("preset"))
                .and_then(Value::as_str)
                == Some("battledome")
        {
            fields.insert("delivery".into(), Value::String("responsive".into()));
        }
        Self::deserialize(Value::Object(fields)).map_err(D::Error::custom)
    }
}
impl SimulationConfig {
    pub fn validate(&self) -> Result<(), String> {
        self.world.validate()?;
        // Live prey refill to `count`, and each needs one of the u16 wire IDs.
        if self.world.prey.count > MAX_PREY {
            return Err(format!(
                "Protocol-19 prey IDs are u16: prey count must be at most {MAX_PREY}"
            ));
        }
        self.latency.validate()?;
        encode::setup(self.world.radius, self.world.prey.turn_rate)?;
        for speed in [
            self.world.prey.cruise_speed,
            self.world.prey.escape_speed,
            self.world.prey.sprint_speed,
        ] {
            encode::speed_u16(speed).map_err(|error| format!("Prey {error}"))?;
        }
        // Appearances encode size in one byte of fifth-units. Check the spawn
        // grid, allowing range endpoints above 51 that still only produce 51.
        if self
            .world
            .prey
            .size_steps()
            .is_some_and(|(_, upper)| upper > f64::from(u8::MAX))
        {
            return Err("Protocol-19 simulation supports prey sizes up to 51".into());
        }
        if !(1..=MAX_AGENTS).contains(&self.agents) || self.controlled_slot >= self.agents {
            return Err(format!(
                "Simulation needs 1 to {MAX_AGENTS} agents, since protocol-19 snake IDs, \
                 leaderboard rank and population are u16, and a controlled slot among them"
            ));
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
        if !(1..=crate::client::minimap::MAX_SIDE).contains(&self.minimap_side) {
            return Err(format!(
                "Minimap side must be 1 through {}: the stock client clamps larger sides",
                crate::client::minimap::MAX_SIDE
            ));
        }
        let largest = self
            .world
            .radius_updates
            .iter()
            .map(|update| update.radius)
            .chain([self.world.radius])
            .chain(
                self.world
                    .dynamic_radius
                    .then(|| crate::simulator::population_radius(self.agents)),
            )
            .fold(0., f64::max);
        if largest > encode::MAX_RADIUS {
            return Err(format!(
                "Protocol-19 simulation supports arena radii up to {}, a margin inside the \
                 ±32,550 u16 anchor coordinates",
                encode::MAX_RADIUS
            ));
        }
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
