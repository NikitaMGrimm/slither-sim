pub mod checkpoint;
mod initialization;
#[cfg(test)]
mod initialization_tests;
#[cfg(test)]
mod observation_tests;
#[cfg(test)]
mod pit_food_tests;
mod spawn_bounds;
#[cfg(test)]
mod spawn_bounds_tests;
use crate::bounds_index::BoundsIndex;
use crate::food_grid::{EaterGrid, FoodCache, PelletIndex};
use crate::kiosk::{KioskConfig, KioskState};
use crate::prey::{OUTWARD_SPEED, Prey, PreyConfig, PreyMode};
use crate::profiling::StepProfile;
use crate::{Constraint, ErrorKind, SimError};
use crate::{CoreStepError, WorldStatus};
use crate::{model::*, physics::*};
pub use initialization::{
    InitialPopulation, InitialSpeed, Placement, ReplayLife, ReplayPrey, Scenario, WorldOptions,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::OnceLock;
use std::time::Instant;

/// Provisional density limit inferred from received death-food packets.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct DeathFoodConfig {
    pub enabled: bool,
    pub cell_size: f64,
    pub max_per_cell: usize,
}
impl Default for DeathFoodConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cell_size: 75.0,
            max_per_cell: 22,
        }
    }
}
impl DeathFoodConfig {
    pub fn validate(&self) -> Result<(), SimError> {
        if let Some(rule) = Constraint::positive(self.cell_size) {
            return Err(SimError::config(
                rule,
                "deathFood.cellSize",
                "Death-food cellSize must be positive and finite",
            ));
        }
        Ok(())
    }
}

/// Provisional admission limit inferred from received ambient-food packets.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct AmbientFoodConfig {
    pub enabled: bool,
    pub cell_size: f64,
    pub max_per_cell: usize,
}
impl Default for AmbientFoodConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cell_size: 75.0,
            max_per_cell: 2,
        }
    }
}
impl AmbientFoodConfig {
    pub fn validate(&self) -> Result<(), SimError> {
        if let Some(rule) = Constraint::positive(self.cell_size) {
            return Err(SimError::config(
                rule,
                "ambientFood.cellSize",
                "Ambient-food cellSize must be positive and finite",
            ));
        }
        Ok(())
    }
}

/// Provisional admission limit inferred from received small boost-food packets.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct BoostFoodConfig {
    pub enabled: bool,
    pub cell_size: f64,
    pub max_per_cell: usize,
}
impl Default for BoostFoodConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cell_size: 75.0,
            max_per_cell: 9,
        }
    }
}
impl BoostFoodConfig {
    pub fn validate(&self) -> Result<(), SimError> {
        if let Some(rule) = Constraint::positive(self.cell_size) {
            return Err(SimError::config(
                rule,
                "boostFood.cellSize",
                "Boost-food cellSize must be positive and finite",
            ));
        }
        Ok(())
    }
}

/// A one-way inner boundary: entering is free, but a snake whose head center
/// has been inside dies when its head center reaches `radius` again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PitConfig {
    pub radius: f64,
}

/// Named starting point for a JSON world config; explicit fields override it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Preset {
    /// `Config::official(1)`, which omitted fields also use without a preset.
    Official,
    Sandbox,
    Battledome,
}
impl Preset {
    pub fn config(self) -> Config {
        match self {
            Self::Official => Config::official(1),
            Self::Sandbox => Config::sandbox(),
            Self::Battledome => Config::battledome(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    /// Initial boundary radius. Runtime updates are reported by `Simulator::radius`.
    pub radius: f64,
    /// Follow occupied seats: grow at once, shrink gradually, with a 3% dead zone.
    /// Scale ecology with live area. A product model, not an identified official law; see
    /// research/fidelity/2026-10-26-arena-radius/REPORT.md.
    pub dynamic_radius: bool,
    /// Runtimes enlarge this arena to fit their population at creation, scaling
    /// food with area. Explicit JSON `radius` opts out unless `sizeToPopulation`
    /// is also supplied; a fixed Rust radius disables this and `dynamic_radius`.
    pub size_to_population: bool,
    pub vision: f64,
    /// Initial-area refill target; counts ambient pellets unless
    /// `ambient_refill_counts_all_food`. Seat-following scales it with live area.
    pub food_count: usize,
    /// Count every pellet toward the refill target. A custom ecology choice,
    /// enabled by Battledome; independent of a snake's size floor.
    pub ambient_refill_counts_all_food: bool,
    /// Custom pit ecology: scale ambient supply by pit area, use full death
    /// budgets and discard ambient/death/boost pellets outside the live pit.
    /// Requires `pit`; disabled worlds retain their original food rules.
    pub food_only_inside_pit: bool,
    /// Initial-area ambient attempts/s below target; seat-following scales this rate.
    pub ambient_replenish_per_second: f64,
    /// Physics step. The server loop is ~7 ms; larger steps trade event resolution for speed.
    pub dt: f64,
    /// Disable only for differential debugging; auto indexing preserves exact pickup rules.
    pub food_index: bool,
    /// Recorded-world diagnostics can supply observed boost pellets instead. Disabling
    /// emission preserves boost cost, timing, and RNG consumption, including that tick.
    pub emit_boost_food: bool,
    /// Idle retention is measured; normalized progress transitions remain provisional.
    pub speed_model: SpeedModel,
    pub prey: PreyConfig,
    /// Optional received-radius trace. Overrides the fixed or dynamic radius at each time.
    pub radius_updates: Vec<crate::RadiusUpdate>,
    pub boundary_mode: crate::BoundaryMode,
    /// Provisional 2024 anti-kiosk model. Disable for recorded motion replays.
    pub kiosk: KioskConfig,
    /// Provisional local density cap on new death pellets.
    pub death_food: DeathFoodConfig,
    /// Provisional local admission cap on new ambient pellets.
    pub ambient_food: AmbientFoodConfig,
    /// Provisional local admission cap on new boost pellets.
    pub boost_food: BoostFoodConfig,
    /// Optional one-way inner boundary; leaving it kills with `DeathReason::Pit`.
    pub pit: Option<PitConfig>,
    /// Inner distance of circular newborn bodies; `None` keeps official admission.
    pub spawn_radius: Option<f64>,
    /// Outer distance, sampled uniformly by area from `spawn_radius`. None keeps
    /// a single circle. Circular lives require full clearance and head-path safety.
    pub spawn_radius_max: Option<f64>,
    /// Window of `Simulator::kill_score`: each credited kill counts for this
    /// many seconds, across the slot's lives. `None` keeps no kill score.
    pub kill_score_seconds: Option<f64>,
    /// Least displayed score: new lives start at the least mass that displays
    /// it, and boosting or the kiosk penalty spend only mass above that floor;
    /// boosting at the floor is free. Death food excludes free floor mass unless
    /// `food_only_inside_pit` selects full, spatially filtered death budgets.
    /// Requires `spawn_radius`, the ring that new bodies lie along. Read when a
    /// world is created.
    pub min_score: Option<u32>,
}

// Private wire representation; public serde calls must go through preset handling.
#[derive(Serialize, Deserialize)]
#[serde(
    remote = "Config",
    default = "Config::default",
    rename_all = "camelCase",
    deny_unknown_fields
)]
struct ConfigWire {
    radius: f64,
    dynamic_radius: bool,
    size_to_population: bool,
    vision: f64,
    food_count: usize,
    #[serde(skip_serializing_if = "is_false")]
    ambient_refill_counts_all_food: bool,
    #[serde(skip_serializing_if = "is_false")]
    food_only_inside_pit: bool,
    ambient_replenish_per_second: f64,
    dt: f64,
    food_index: bool,
    emit_boost_food: bool,
    speed_model: SpeedModel,
    prey: PreyConfig,
    radius_updates: Vec<crate::RadiusUpdate>,
    boundary_mode: crate::BoundaryMode,
    kiosk: KioskConfig,
    death_food: DeathFoodConfig,
    ambient_food: AmbientFoodConfig,
    boost_food: BoostFoodConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pit: Option<PitConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spawn_radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spawn_radius_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kill_score_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_score: Option<u32>,
}

fn is_false(value: &bool) -> bool {
    !value
}

impl Default for Config {
    fn default() -> Self {
        Self::official(1)
    }
}
impl Serialize for Config {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ConfigWire::serialize(self, serializer)
    }
}
impl<'de> Deserialize<'de> for Config {
    /// Fields given beside `preset` replace its values; nested objects merge
    /// field by field. Without a preset, omitted fields use `Config::official(1)`.
    /// Input passes through `serde_json::Value`, so a repeated key keeps its last value.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        use serde_json::{Map, Value};
        fn merge(target: &mut Map<String, Value>, fields: Map<String, Value>) {
            for (key, value) in fields {
                match (target.get_mut(&key), value) {
                    (Some(Value::Object(target)), Value::Object(fields)) => merge(target, fields),
                    (_, value) => {
                        target.insert(key, value);
                    }
                }
            }
        }
        let mut fields = Map::deserialize(deserializer)?;
        // Preserve custom radii without changing the preset's other defaults.
        // Serialized configs include the flag, so resolved round trips retain it.
        let explicit_radius = fields.contains_key("radius");
        let explicit_dynamic_radius = fields.contains_key("dynamicRadius");
        let explicit_population_sizing = fields.contains_key("sizeToPopulation");
        let fields = match fields.remove("preset") {
            None => fields,
            Some(preset) => {
                let preset = Preset::deserialize(preset).map_err(D::Error::custom)?;
                let Value::Object(mut base) =
                    serde_json::to_value(preset.config()).map_err(D::Error::custom)?
                else {
                    unreachable!("configs serialize as objects")
                };
                merge(&mut base, fields);
                base
            }
        };
        let mut config =
            ConfigWire::deserialize(Value::Object(fields)).map_err(D::Error::custom)?;
        if explicit_radius && !explicit_population_sizing {
            config.size_to_population = false;
        }
        if explicit_radius && !explicit_dynamic_radius {
            config.dynamic_radius = false;
        }
        Ok(config)
    }
}
/// Whether segment `a`-`b` passes within `radius` of the center. Endpoints
/// are measured unscaled, as the head test is, since rescaling can move a
/// point across the circle. An interior closest point is found with
/// coordinates divided by their largest magnitude, so a tiny path's squared
/// length cannot underflow to zero and look like a point.
fn reaches_center(a: Point, b: Point, radius: f64) -> bool {
    let origin = Point::default();
    // The segment is never farther from the center than either endpoint.
    if a.distance(origin) < radius || b.distance(origin) < radius {
        return true;
    }
    let scale = a.x.abs().max(a.y.abs()).max(b.x.abs()).max(b.y.abs());
    let (ax, ay) = (a.x / scale, a.y / scale);
    let (dx, dy) = (b.x / scale - ax, b.y / scale - ay);
    let length = dx * dx + dy * dy;
    // An endpoint closest point was measured above.
    if length == 0.0 {
        return false;
    }
    let t = -(ax * dx + ay * dy) / length;
    t > 0.0 && t < 1.0 && (ax + t * dx).hypot(ay + t * dy) * scale < radius
}
/// Distance every spawn anchor keeps from the wall.
pub(crate) const SPAWN_WALL_MARGIN: f64 = 60.0;
/// Protocol sector area used by the modeled official ecology below.
const SECTOR_AREA: f64 = 300.0 * 300.0;
/// Official food target and ambient attempts/s for an arena of `radius`:
/// about 7 pellets and 0.1024 attempts/s per sector.
fn official_food(radius: f64) -> (usize, f64) {
    let sectors = PI * radius * radius / SECTOR_AREA;
    ((7.0 * sectors) as usize, 0.1024 * sectors)
}
/// Official prey target for an arena of `radius`: one per ten million square
/// units, at least one.
fn official_prey(radius: f64) -> usize {
    (PI * radius * radius / 10_000_000.0).round().max(1.0) as usize
}
impl Config {
    /// Startup capacity policy, independent of later boundary motion:
    /// research/fidelity/2026-10-26-arena-radius/REPORT.md.
    pub(crate) fn size_for_population(&mut self, population: usize) {
        // Circular seat-following keeps radius/pit/spawn settings as reference
        // geometry. Scaling them here would move the controller's base too.
        if self.dynamic_radius && self.spawn_radius.is_some() && self.min_score.is_some() {
            return;
        }
        let radius = population_radius(population);
        if self.size_to_population && self.radius > 0. && self.radius < radius {
            let (food, rate, prey) = self.ecology_at_radius(radius);
            self.radius = radius;
            self.food_count = food;
            self.ambient_replenish_per_second = rate;
            self.prey.count = prey;
        }
    }

    /// Circular arenas keep their reference size through 32 occupied seats.
    /// Above it, preserve area per seat. Queue age never increases this target.
    /// Provisional mode choice, iterated through local playtesting.
    /// research/battledome/2026-10-27-arena-playtest/REPORT.md
    pub(crate) fn seat_radius(&self, seats: usize) -> f64 {
        if self.dynamic_radius && self.spawn_radius.is_some() && self.min_score.is_some() {
            (self.radius * (seats.max(32) as f64 / 32.).sqrt()).min(checkpoint::MAGNITUDE)
        } else {
            population_radius(seats)
        }
    }

    pub(crate) fn circular_scale(&self, radius: f64) -> f64 {
        if self.dynamic_radius && self.spawn_radius.is_some() && self.min_score.is_some() {
            radius / self.radius
        } else {
            1.
        }
    }

    /// Same density policy at startup and during seat-following motion. Preserve
    /// custom targets, including zero. Official prey uses its density before
    /// rounding/flooring, rather than multiplying the one-prey minimum by area.
    pub(crate) fn ecology_at_radius(&self, radius: f64) -> (usize, f64, usize) {
        if radius == self.radius {
            return (
                self.food_count,
                self.ambient_replenish_per_second,
                self.prey.count,
            );
        }
        let area = (radius / self.radius).powi(2);
        let scale = |value: f64| if value == 0.0 { 0.0 } else { value * area };
        (
            scale(self.food_count as f64) as usize,
            scale(self.ambient_replenish_per_second),
            if self.prey.count == official_prey(self.radius) {
                official_prey(radius)
            } else {
                scale(self.prey.count as f64).round() as usize
            },
        )
    }

    /// Best-current official-server approximation, initialized for `snakes`.
    /// Includes provisional models; this is not an identified server preset.
    /// Starts population-sized, then follows occupied seats with a 3% dead zone.
    /// This product model omits real official waves, whose controller remains
    /// unidentified. See
    /// research/fidelity/2026-10-26-arena-radius/REPORT.md.
    /// Food follows live area. Prey uses an explicit
    /// working estimate of one per ten million square units, with at least one.
    /// Received prey density does not establish a global census; see
    /// docs/configuration.md#official-and-sandbox.
    pub fn official(snakes: usize) -> Self {
        let radius = population_radius(snakes);
        let (food_count, ambient_replenish_per_second) = official_food(radius);
        Self {
            radius,
            dynamic_radius: true,
            size_to_population: true,
            // A finite receipt approximation: the paired visibility census
            // rejects 500, while 1500 substantially reduces missed snakes/prey.
            // Exact server entry/exit geometry and size scaling remain unknown.
            vision: 1500.0,
            food_count,
            ambient_refill_counts_all_food: false,
            food_only_inside_pit: false,
            ambient_replenish_per_second,
            dt: LOOP_SECONDS,
            food_index: true,
            emit_boost_food: true,
            speed_model: SpeedModel::default(),
            prey: PreyConfig {
                count: official_prey(radius),
                ..PreyConfig::default()
            },
            radius_updates: vec![],
            boundary_mode: crate::BoundaryMode::default(),
            kiosk: KioskConfig::default(),
            death_food: DeathFoodConfig::default(),
            ambient_food: AmbientFoodConfig::default(),
            boost_food: BoostFoodConfig::default(),
            pit: None,
            spawn_radius: None,
            spawn_radius_max: None,
            kill_score_seconds: None,
            min_score: None,
        }
    }

    /// Compact fixed-size scene using the official mechanics and model settings.
    /// Population-dependent radius changes are disabled; food and prey counts are
    /// fixed for inexpensive examples and controlled tests. Prey can be enabled
    /// explicitly with `prey.count`.
    pub fn sandbox() -> Self {
        Self {
            radius: 1000.0,
            dynamic_radius: false,
            size_to_population: false,
            food_count: 350,
            ambient_replenish_per_second: 5.0,
            prey: PreyConfig {
                count: 0,
                ..PreyConfig::default()
            },
            ..Self::official(1)
        }
    }

    /// Battledome: official physics in a seat-following, initially 3584-unit arena,
    /// with a one-way pit of radius 1784, spawns in an outer band, a
    /// 15-minute kill score, a 400-point size floor and no prey. See docs/configuration.md#battledome.
    pub fn battledome() -> Self {
        const RADIUS: f64 = 3584.0;
        const PIT: f64 = 1784.0;
        let (food_count, ambient_replenish_per_second) = official_food(RADIUS);
        Self {
            radius: RADIUS,
            dynamic_radius: true,
            size_to_population: false,
            food_count,
            // Design choice: death/boost food already feeds snakes, so it also
            // fills the ambient target. Independent of minScore for custom worlds.
            // research/battledome/2026-10-25-food-balance/REPORT.md
            ambient_refill_counts_all_food: true,
            food_only_inside_pit: true,
            ambient_replenish_per_second,
            prey: PreyConfig {
                count: 0,
                ..PreyConfig::default()
            },
            pit: Some(PitConfig { radius: PIT }),
            spawn_radius: Some((PIT + RADIUS) / 2.0),
            // Custom policy: sqrt((3584 - 60)^2 - 3400^2) / 437.5 = 2.12 s
            // of straight tangent travel at maximum boost before the spawn wall
            // margin. Never place a life between the old ring and the middle.
            // research/battledome/2026-10-26-safe-spawn/REPORT.md
            spawn_radius_max: Some(3400.0),
            kill_score_seconds: Some(900.0),
            min_score: Some(400),
            ..Self::official(1)
        }
    }
    /// Checks the initial rules. Checkpoints validate the live radius separately,
    /// so a world that has shrunk inside its pit or spawn band still restores.
    pub fn validate(&self) -> Result<(), SimError> {
        for (field, value) in [
            ("radius", self.radius),
            ("vision", self.vision),
            ("dt", self.dt),
        ] {
            if let Some(rule) = Constraint::positive(value) {
                return Err(SimError::config(
                    rule,
                    field,
                    "Settings must be positive finite numbers",
                ));
            }
        }
        let radius_limit_error = |field: &str| {
            SimError::config(
                Constraint::OutOfRange,
                field,
                format!(
                    "{field} must be at most {:e} for checkpoint-safe continuation",
                    checkpoint::MAGNITUDE
                ),
            )
        };
        if self.radius > checkpoint::MAGNITUDE {
            return Err(radius_limit_error("radius"));
        }
        check_ambient_rate(self.ambient_replenish_per_second, self.dt)?;
        // A slide leaves the head one contact lead behind the rim along its
        // heading. Moving at least that far in one tick puts a head that keeps
        // its heading on or outside the rim, which is lethal, so shallow slides
        // of a boosting minimum-size snake would become deaths. The old 32 ms
        // cap claimed body-radius tunneling, but collision tests are swept.
        let lead = crate::boundary::CONTACT_LEAD * MIN_SCALE;
        if self.boundary_mode == crate::BoundaryMode::Sliding && self.dt * BOOST_SPEED >= lead {
            return Err(SimError::config(
                Constraint::OutOfRange,
                "dt",
                format!(
                    "With sliding boundaries dt must be below {lead} / {BOOST_SPEED} = {} s: a \
                 boosting minimum-size snake would pass its {lead}-unit forward contact \
                 lead in one tick and die on shallow rim contact. Use boundaryMode \"hard\" \
                 for longer steps",
                    lead / BOOST_SPEED
                ),
            ));
        }
        // Prey advance in LOOP_SECONDS substeps and boosting snakes drop food
        // every BOOST_DROP_SECONDS, so one tick counts dt plus a carried clock
        // below one period of each. Counting the shorter period below
        // `max_tick_count` keeps both counts exact integers that fit usize,
        // about 6.3e13 s on 64-bit targets. It also keeps a tick of boost
        // travel, dt × BOOST_SPEED, far inside f64, so heads stay finite.
        if (self.dt + 2.0 * LOOP_SECONDS) / LOOP_SECONDS >= max_tick_count() {
            return Err(SimError::config(
                Constraint::OutOfRange,
                "dt",
                format!(
                    "dt {} is too long: prey advance in {LOOP_SECONDS} s substeps, and one \
                 tick's count of them must be an exact integer that fits usize (below {})",
                    self.dt,
                    max_tick_count()
                ),
            ));
        }
        self.prey.validate()?;
        self.kiosk.validate()?;
        self.death_food.validate()?;
        self.ambient_food.validate()?;
        self.boost_food.validate()?;
        self.validate_mode()?;
        crate::boundary::validate(&self.radius_updates)?;
        if let Some(index) = self
            .radius_updates
            .iter()
            .position(|update| update.radius > checkpoint::MAGNITUDE)
        {
            return Err(radius_limit_error(&format!(
                "radiusUpdates[{index}].radius"
            )));
        }
        Ok(())
    }
    /// Pit, spawn ring and kill-score window: only the limits the rules need.
    fn validate_mode(&self) -> Result<(), SimError> {
        let pit = self.pit.as_ref().map(|pit| pit.radius);
        if self.food_only_inside_pit && pit.is_none() {
            return Err(SimError::config(
                Constraint::Missing,
                "pit",
                "foodOnlyInsidePit requires pit",
            ));
        }
        if pit.is_some_and(|pit| !(pit > 0.0 && pit.is_finite() && pit < self.radius)) {
            return Err(SimError::config(
                Constraint::positive(pit.unwrap()).unwrap_or(Constraint::OutOfRange),
                "pit.radius",
                "pit.radius must be positive and less than radius, so the pit lies inside the arena",
            ));
        }
        if self
            .kill_score_seconds
            .is_some_and(|window| !(window.is_finite() && window > 0.0))
        {
            return Err(SimError::config(
                Constraint::positive(self.kill_score_seconds.unwrap()).unwrap(),
                "killScoreSeconds",
                "killScoreSeconds must be positive and finite, so kills count and then expire",
            ));
        }
        let scores = MassFloor::score_range();
        if self.min_score.is_some_and(|score| !scores.contains(&score)) {
            return Err(SimError::config(
                Constraint::OutOfRange,
                "minScore",
                format!(
                    "minScore must be from a newborn's displayed {} to {}, the score at \
                 {MAX_SEGMENTS} segments, where the score table ends and larger snakes \
                 display less, so growth never falls below the floor",
                    scores.start(),
                    scores.end()
                ),
            ));
        }
        if self.min_score.is_some() && self.spawn_radius.is_none() {
            return Err(SimError::config(
                Constraint::Missing,
                "spawnRadius",
                "minScore requires spawnRadius, the ring new lives lie along",
            ));
        }
        // The whole centerline must clear the pit: the midpoint of a 42-unit
        // chord is sqrt(spawn^2 - 21^2) from the center. Check both rounded
        // radii: small rings can wrap, but large ones lose angular resolution.
        let segments = self
            .min_score
            .and_then(MassFloor::for_score)
            .map_or(2, |floor| floor.segments);
        if self.spawn_radius_max.is_some_and(|outer| {
            !outer.is_finite() || self.spawn_radius.is_none_or(|inner| outer < inner)
        }) {
            return Err(SimError::config(
                if !self.spawn_radius_max.unwrap().is_finite() {
                    Constraint::NonFinite
                } else if self.spawn_radius.is_none() {
                    Constraint::Missing
                } else {
                    Constraint::Ordering
                },
                "spawnRadiusMax",
                "spawnRadiusMax requires spawnRadius and must be finite and at least spawnRadius, so the spawn band is ordered",
            ));
        }
        let invalid = [
            ("spawnRadius", self.spawn_radius),
            ("spawnRadiusMax", self.spawn_radius_max),
        ]
        .into_iter()
        .find_map(|(name, spawn)| {
            let spawn = spawn?;
            let roundoff = crate::spawn::ring_roundoff(spawn);
            let lower = spawn - roundoff;
            let upper = spawn + roundoff;
            let fits = spawn.is_finite()
                && upper < self.radius - SPAWN_WALL_MARGIN
                && pit.is_none_or(|pit| lower > pit.hypot(ANCHOR_SPACING / 2.0))
                && crate::spawn::ring_step(lower, segments).is_some()
                && crate::spawn::ring_step(upper, segments).is_some();
            (!fits).then_some(name)
        });
        if let Some(name) = invalid {
            return Err(SimError::config(
                if name == "spawnRadius" && !self.spawn_radius.unwrap().is_finite() {
                    Constraint::NonFinite
                } else {
                    Constraint::OutsideArena
                },
                name,
                format!(
                    "{name} must lie outside pit.radius and more than {SPAWN_WALL_MARGIN} inside \
                 radius, with body chords outside the pit and room for a new life's \
                 {segments} anchors 42 apart plus its head-tail body clearance"
                ),
            ));
        }
        Ok(())
    }

    /// Valid settings outside measured behavior, as sentences. The command-line
    /// tools print them to stderr when they load a config; `serve` resets also
    /// return them.
    pub fn warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();
        let [low, high] = MEASURED_LOOP_SECONDS;
        if !(low..=high).contains(&self.dt) {
            warnings.push(format!(
                "dt {} s differs from the measured ~7 ms server loop ({}-{} ms): contact, \
                 pickup and event timing may differ; prey still move in fixed 7 ms substeps",
                self.dt,
                low * 1000.,
                high * 1000.
            ));
        }
        warnings
    }
}
/// Refill subtracts one attempt interval at a time from a positive clock below
/// `interval + dt` until it runs out. Unless that subtraction changes the
/// largest such clock (and so every smaller one), the loop never ends, so f64
/// resolution, not a policy cap, bounds the rate.
fn check_ambient_rate(rate: f64, dt: f64) -> Result<(), SimError> {
    if let Some(rule) = Constraint::nonnegative(rate) {
        return Err(SimError::config(
            rule,
            "ambientReplenishPerSecond",
            "ambientReplenishPerSecond must be finite and nonnegative",
        ));
    }
    let interval = 1.0 / rate;
    let clock = dt + interval;
    if rate > 0.0 && clock - interval >= clock {
        return Err(SimError::config(
            Constraint::OutOfRange,
            "ambientReplenishPerSecond",
            format!(
                "ambientReplenishPerSecond {rate} with dt {dt} is too high: one attempt interval \
             no longer changes the refill clock in f64"
            ),
        ));
    }
    Ok(())
}

/// Constant-density boundary heuristic. Population/radius pairs do not identify
/// the server's controller; measured replay uses `Config::radius_updates` instead.
pub fn population_radius(snakes: usize) -> f64 {
    670.0 * (snakes.max(1) as f64).sqrt()
}
/// Local shrink rate limit; growth applies immediately outside the dead zone.
/// It must not be applied to measured `radius_updates` replay.
const RADIUS_RATE: f64 = 3.0 / 0.128;
const RADIUS_DEAD_ZONE: f64 = 0.03;

#[derive(Clone, Debug)]
struct Random(u32);
impl Random {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        f64::from(self.0) / 4294967296.0
    }
}
#[derive(Clone, Debug, Default)]
struct Boost {
    held: bool,
    active: bool,
    /// Exhausting a held boost requires release before newly eaten mass can fuel it.
    exhausted: bool,
    clock: f64,
    active_boost_seconds: f64,
    boost_mass_spent: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BoostAccounting {
    /// Simulated seconds whose movement used active boost for this life.
    pub active_boost_seconds: f64,
    /// Growth-mass units passed to `remove_mass` for this life.
    pub boost_mass_spent: f64,
}

/// Optional presentation telemetry. Captures are already applied to world state.
#[derive(Clone, Debug, PartialEq)]
pub enum PickupItem {
    Food(Food),
    Prey(Prey),
}

#[derive(Clone, Debug, PartialEq)]
pub struct PickupEvent {
    pub eater: usize,
    pub item: PickupItem,
}

/// Optional birth provenance for client presentation; never consulted by physics.
#[derive(Clone, Debug, PartialEq)]
pub struct FoodSpawnEvent {
    /// Stable snake slot for boost/death food; None for ambient replenishment.
    pub source: Option<usize>,
    pub food: Food,
}

/// A running world. Use the validated checkpoint boundary for exact continuation.
///
/// ```compile_fail
/// let mut world = slither_sim::Simulator::new(slither_sim::WorldOptions::default()).unwrap();
/// world.snakes.clear();
/// ```
/// ```compile_fail
/// let mut world = slither_sim::Simulator::new(slither_sim::WorldOptions::default()).unwrap();
/// world.config().radius = 10.0;
/// ```
#[derive(Clone, Debug)]
pub struct Simulator {
    config: Config,
    /// Stable slots: checked operations never remove or reorder snakes.
    snakes: Vec<Snake>,
    food: Vec<Food>,
    prey: Vec<Prey>,
    deaths: Vec<DeathEvent>,
    /// Latest step only. None avoids event allocation/copying.
    pickup_events: Option<Vec<PickupEvent>>,
    /// Latest step only. Disabled by default.
    food_spawn_events: Option<Vec<FoodSpawnEvent>>,
    time: f64,
    /// Live boundary, independent of the configured initial radius.
    radius: f64,
    death_times: Vec<Option<f64>>,
    /// Join order of each slot's current life; collisions resolve oldest first.
    joins: Vec<u64>,
    next_join: u64,
    random: Random,
    boosts: Vec<Boost>,
    speeds: Vec<SpeedState>,
    kiosks: Vec<KioskState>,
    /// Whether each slot's current life has had its head center inside the pit.
    pit_entered: Vec<bool>,
    /// Times of each slot's credited kills still inside `killScoreSeconds`.
    kill_times: Vec<VecDeque<f64>>,
    /// `config.min_score` as mass, converted when the world is created.
    min_mass: Option<MassFloor>,
    /// Fractional refill credit in initial-area seconds, including during motion.
    ambient_clock: f64,
    prey_random: Random,
    next_prey_id: u64,
    prey_clock: f64,
    prey_decision_times: HashMap<u64, f64>,
    prey_outward: HashSet<u64>,
    prey_motion_clock: f64,
    radius_update_index: usize,
    /// Why the world stopped: a pickup whose gain would overflow the eater's
    /// mass, or non-finite prey motion. Validation rejects every later step.
    failure: Option<SimError>,
    food_cache: FoodCache,
    /// Derived admission boxes, rebuilt lazily after motion/growth/deaths.
    spawn_bounds: OnceLock<Vec<Option<Bounds>>>,
    eater_grid: EaterGrid,
    buffers: StepBuffers,
    /// Batch observation on the rayon pool; see `set_parallel`.
    #[cfg(feature = "parallel")]
    parallel: bool,
}
impl Simulator {
    /// Split large `observe_all` and `observe_selected` batches over the current
    /// rayon pool, with identical results. The global pool uses `RAYON_NUM_THREADS`
    /// or the core count; `ThreadPool::install` overrides it. Off by default so
    /// parallel worlds do not compete for cores. Not serialized, like event buffers.
    #[cfg(feature = "parallel")]
    pub fn set_parallel(&mut self, parallel: bool) {
        self.parallel = parallel;
    }
    /// The `set_parallel` value.
    #[cfg(feature = "parallel")]
    pub fn parallel(&self) -> bool {
        self.parallel
    }
    /// Set ambient placement attempts per second, preserving fractional refill credit.
    /// Zero disables refill and discards pending credit. Invalid rates or unrepresentable
    /// clock rescaling leave the world unchanged. Use this instead of editing the rate directly.
    pub fn set_ambient_rate(&mut self, rate: f64) -> Result<(), SimError> {
        check_ambient_rate(rate, self.config.dt)?;
        if rate != self.config.ambient_replenish_per_second {
            let credit = self.ambient_clock * self.config.ambient_replenish_per_second;
            let clock = if rate > 0.0 { credit / rate } else { 0.0 };
            if !clock.is_finite() {
                return Err(SimError::new(
                    ErrorKind::NumericOverflow,
                    "Ambient refill credit cannot be represented at this rate",
                )
                .at_field("ambientReplenishPerSecond"));
            }
            self.ambient_clock = clock;
            self.config.ambient_replenish_per_second = rate;
        }
        Ok(())
    }
    fn empty(seed: u32, config: Config) -> Result<Self, SimError> {
        config.validate()?;
        let min_mass = config.min_score.and_then(MassFloor::for_score);
        let mut world = Self {
            radius: config.radius,
            config,
            snakes: vec![],
            food: vec![],
            prey: vec![],
            deaths: vec![],
            pickup_events: None,
            food_spawn_events: None,
            time: 0.0,
            death_times: vec![],
            joins: vec![],
            next_join: 0,
            random: Random(seed),
            boosts: vec![],
            speeds: vec![],
            kiosks: vec![],
            pit_entered: vec![],
            kill_times: vec![],
            min_mass,
            ambient_clock: 0.0,
            prey_random: Random(seed ^ 0x706_7265),
            next_prey_id: 0,
            prey_clock: 0.0,
            prey_decision_times: HashMap::new(),
            prey_outward: HashSet::new(),
            prey_motion_clock: 0.0,
            radius_update_index: 0,
            food_cache: FoodCache::default(),
            spawn_bounds: OnceLock::new(),
            eater_grid: EaterGrid::default(),
            buffers: StepBuffers::default(),
            #[cfg(feature = "parallel")]
            parallel: false,
            failure: None,
        };
        // Construction applies measured time-zero overrides without spending a
        // population-controller tick before any snakes have joined.
        world.apply_radius_updates(0.0);
        Ok(world)
    }
    fn initialize_ecology(&mut self) -> Result<(), SimError> {
        let world = self;
        // A zero cell limit rejects every candidate, so there is nothing to store.
        let ambient = &world.config.ambient_food;
        let admits = !ambient.enabled || ambient.max_per_cell > 0;
        let (food, _, prey) = if world.config.dynamic_radius {
            world.config.ecology_at_radius(world.radius)
        } else {
            (
                world.config.food_count,
                world.config.ambient_replenish_per_second,
                world.config.prey.count,
            )
        };
        let food = if world.config.food_only_inside_pit {
            (food as f64 * world.food_area_fraction()) as usize
        } else {
            food
        };
        let startup = if admits { food } else { 0 };
        world.food.try_reserve(startup).map_err(|_| {
            SimError::new(
                ErrorKind::AllocationFailed,
                format!(
                    "foodCount {} does not fit in memory",
                    world.config.food_count
                ),
            )
            .at_field("foodCount")
        })?;
        world.food_cache.begin();
        // food_count is a target number of attempts at startup. A rejected
        // candidate is not retried, so crowded small worlds can start below it.
        for _ in 0..startup {
            let f = world.new_food();
            if world.food_position_allowed(f.point())
                && admit_ambient_food(
                    &f,
                    &world.config.ambient_food,
                    &mut world.food_cache,
                    &world.food,
                )
            {
                world.food_cache.push(&f);
                world.food.push(f);
            }
        }
        world.food_cache.end();
        world.prey.try_reserve(prey).map_err(|_| {
            SimError::new(
                ErrorKind::AllocationFailed,
                format!(
                    "prey count {} does not fit in memory",
                    world.config.prey.count
                ),
            )
            .at_field("prey.count")
        })?;
        // A new world has no prey, so IDs 0..count need no collision scan.
        for _ in 0..prey {
            world.push_prey();
        }
        Ok(())
    }
    fn pellet(p: Point, size: f64, origin: Origin) -> Food {
        Food {
            x: quantize_food(p.x),
            y: quantize_food(p.y),
            size: Some(size),
            value: nutrition(size, 2.0),
            origin: Some(origin),
        }
    }
    /// Provisional decreasing area-density profile. The inherited coefficients
    /// have no local fit citation; recipient coverage and admission censor counts.
    /// This is a model choice, not a recovered generation law.
    fn new_food(&mut self) -> Food {
        let radius = self.food_radius();
        let a = self.random.next() * TAU;
        let r = loop {
            let f = self.random.next().sqrt();
            if self.random.next() * 0.217 <= 0.217 - 0.172 * f {
                break f * (radius - 20.0).max(0.0);
            }
        };
        let size = ((3.0 + self.random.next() * 6.4) * 5.0).round() / 5.0;
        Self::pellet(
            Point {
                x: a.cos() * r,
                y: a.sin() * r,
            },
            size,
            Origin::Ambient,
        )
    }
    fn push_ring(&mut self, id: String, index: usize, count: usize) -> Result<usize, SimError> {
        let a = index as f64 / count as f64 * TAU;
        let (body, heading) = match self.config.spawn_radius {
            // A ring life lies along the ring, counterclockwise.
            Some(distance) => self.spawn_body(
                Point {
                    x: a.cos() * distance * self.config.circular_scale(self.radius),
                    y: a.sin() * distance * self.config.circular_scale(self.radius),
                },
                a + FRAC_PI_2,
            )?,
            // Two segments halfway to the wall, facing inward.
            None => {
                let distance = self.radius * 0.5;
                let head = Point {
                    x: a.cos() * distance,
                    y: a.sin() * distance,
                };
                let tail = Point {
                    x: head.x + a.cos() * 42.0,
                    y: head.y + a.sin() * 42.0,
                };
                (vec![head, tail], a + PI)
            }
        };
        self.push_spawn(id, body, heading)
            .map_err(|e| e.at_slot(self.snakes.len()))
    }
    pub fn death_time(&self, index: usize) -> Option<f64> {
        self.death_times.get(index).copied().flatten()
    }
    /// Credited kills of this slot, across its lives, within the last
    /// `killScoreSeconds`: a kill at time `t` counts while `time < t + window`.
    /// Zero without a window; `None` for unknown slots.
    pub fn kill_score(&self, index: usize) -> Option<usize> {
        let times = self.kill_times.get(index)?;
        Some(self.config.kill_score_seconds.map_or(0, |window| {
            times.len() - times.partition_point(|&t| t + window <= self.time)
        }))
    }

    /// End the previous player's rolling history after networking validates
    /// and admits a new connection to this slot. Respawning alone retains it.
    #[cfg(feature = "networking")]
    pub(crate) fn reset_kill_score(&mut self, index: usize) {
        self.kill_times[index].clear();
    }
    /// Read-only replay diagnostic: successful active boost, not the held button.
    pub fn is_boosting(&self, index: usize) -> bool {
        self.snakes.get(index).is_some_and(|s| s.alive)
            && self.boosts.get(index).is_some_and(|b| b.active)
    }
    /// Return cumulative per-life boost usage. The seconds count includes a
    /// full physics tick whenever movement used boost, including a tick whose
    /// fuel reached zero; mass is the exact unscored amount passed to
    /// `remove_mass`. Respawning resets both values.
    pub fn boost_accounting(&self, index: usize) -> Option<BoostAccounting> {
        self.boosts.get(index).map(|b| BoostAccounting {
            active_boost_seconds: b.active_boost_seconds,
            boost_mass_spent: b.boost_mass_spent,
        })
    }
    /// Segments of a new life: two, or the `minScore` floor's.
    fn spawn_segments(&self) -> usize {
        self.min_mass.map_or(2, |floor| floor.segments)
    }
    /// Head-first body and heading of a new life at `head`. With `spawnRadius`,
    /// its anchors lie 42 apart along the circle through `head` around the
    /// center, turning the way `heading` leans, and it heads along the circle;
    /// an error if that circle cannot hold them. Otherwise two segments lie
    /// straight behind `heading`. Pass the result to `body_is_clear` and `respawn_body`.
    pub fn spawn_body(&self, head: Point, heading: f64) -> Result<(Vec<Point>, f64), SimError> {
        if self.config.spawn_radius.is_none() {
            let tail = Point {
                x: head.x - heading.cos() * 42.0,
                y: head.y - heading.sin() * 42.0,
            };
            return Ok((vec![head, tail], heading));
        }
        crate::spawn::ring_body(head, heading, self.spawn_segments()).ok_or_else(|| {
            SimError::state(
                Constraint::OutOfRange,
                "body",
                "Spawn circle is too small for a new life's body",
            )
        })
    }
    /// Calibrated spawn exclusion, not collision immunity. Check the whole new
    /// body so its tail cannot appear in front of an existing player either.
    /// `clearance` is a minimum centerline distance (600 in the browser lobby).
    /// Zero retains the historical capsule-width + 40 placement margin.
    pub fn spawn_is_clear(
        &self,
        head: Point,
        heading: f64,
        segments: usize,
        clearance: f64,
    ) -> bool {
        if !head.finite()
            || !heading.is_finite()
            || !(2..=430).contains(&segments)
            || !clearance.is_finite()
            || clearance < 0.
        {
            return false;
        }
        let tail = Point {
            x: head.x - heading.cos() * 42. * (segments - 1) as f64,
            y: head.y - heading.sin() * 42. * (segments - 1) as f64,
        };
        self.line_is_clear(&[head, tail], segments, clearance, None)
    }
    /// Check a supplied head-first body with one anchor per segment (2..=430).
    /// Uses the same arena margin and centerline exclusion as straight spawns.
    /// This tests other living snakes, not self-intersections in the supplied body.
    pub fn body_is_clear(&self, body: &[Point], clearance: f64) -> bool {
        (2..=430).contains(&body.len())
            && body.iter().all(|p| p.finite())
            && self.line_is_clear(body, body.len(), clearance, None)
    }
    // The host's clearance probes used to temporarily mark its slot dead. An
    // explicit exclusion preserves that query without exposing mutable state.
    #[cfg(feature = "networking")]
    pub(crate) fn body_is_clear_excluding(
        &self,
        body: &[Point],
        clearance: f64,
        excluded: Option<usize>,
    ) -> bool {
        (2..=430).contains(&body.len())
            && body.iter().all(|p| p.finite())
            && self.line_is_clear(body, body.len(), clearance, excluded)
    }
    /// Custom circular admission: the newborn must also miss each living
    /// head's straight two-second path by the full preferred clearance. This
    /// protects a player continuing its current motion; later turns are unknown.
    /// research/battledome/2026-10-26-safe-spawn/REPORT.md
    #[cfg(all(feature = "networking", test))]
    pub(crate) fn body_clears_head_paths(&self, body: &[Point]) -> bool {
        self.snakes.iter().filter(|s| s.alive).all(|s| {
            let travel = s.speed * crate::spawn::HEAD_PATH_SECONDS;
            let end = Point {
                x: s.head.x + s.heading.cos() * travel,
                y: s.head.y + s.heading.sin() * travel,
            };
            let width =
                (collision_distance(s.segments, body.len()) + 40.).max(crate::spawn::CLEARANCE);
            body.windows(2)
                .all(|edge| swept_distance(edge[0], edge[1], s.head, end) > width)
        })
    }
    /// Least distance between a supplied head-first body's centerline and any
    /// living snake, beyond that snake's contact range (collision distance plus
    /// 40), as `body_is_clear` measures it; infinite without other snakes.
    /// `None` for a body that check rejects whatever the neighbors: a length
    /// outside 2..=430, a non-finite point, or a point within the wall margin.
    pub fn spawn_margin(&self, body: &[Point]) -> Option<f64> {
        self.spawn_margin_bounded(body, f64::NEG_INFINITY)
    }

    /// As `spawn_margin`, but may return an upper bound at most `stop_at` as
    /// soon as an edge rules out beating it. Larger results remain exact.
    pub(crate) fn spawn_margin_bounded(&self, body: &[Point], stop_at: f64) -> Option<f64> {
        self.spawn_margin_bounded_excluding(body, stop_at, None)
    }
    pub(crate) fn spawn_margin_bounded_excluding(
        &self,
        body: &[Point],
        stop_at: f64,
        excluded: Option<usize>,
    ) -> Option<f64> {
        let limit = self.radius - SPAWN_WALL_MARGIN;
        if !(2..=430).contains(&body.len())
            || body
                .iter()
                .any(|p| !p.finite() || p.distance(Point::default()) >= limit)
        {
            return None;
        }
        let mut margin = f64::INFINITY;
        let query = spawn_bounds::bounds(body.iter().copied());
        let bounds = self.spawn_bounds();
        for (i, s) in self
            .snakes
            .iter()
            .enumerate()
            .filter(|(i, s)| s.alive && Some(*i) != excluded)
        {
            let contact = collision_distance(s.segments, body.len()) + 40.;
            // Skip only boxes that cannot lower the accumulated exact minimum.
            // Using stop_at here would change early returns and fallback ties.
            if let (Some(query), Some(bounds)) = (query, bounds[i])
                && spawn_bounds::gap(query, bounds) - contact - 1. > margin
            {
                continue;
            }
            for edge in body.windows(2) {
                let (head, tail) = (edge[0], edge[1]);
                margin = margin.min(segment_distance(s.head, head, tail) - contact);
                if margin <= stop_at {
                    return Some(margin);
                }
                let mut a = s.head;
                for &b in &s.body {
                    margin = margin.min(swept_distance(head, tail, a, b) - contact);
                    if margin <= stop_at {
                        return Some(margin);
                    }
                    a = b;
                }
            }
        }
        Some(margin)
    }
    fn line_is_clear(
        &self,
        line: &[Point],
        segments: usize,
        clearance: f64,
        excluded: Option<usize>,
    ) -> bool {
        if !clearance.is_finite()
            || clearance < 0.0
            || line
                .iter()
                .any(|p| p.distance(Point::default()) >= self.radius - SPAWN_WALL_MARGIN)
        {
            return false;
        }
        let query = spawn_bounds::bounds(line.iter().copied());
        let bounds = self.spawn_bounds();
        self.snakes
            .iter()
            .enumerate()
            .filter(|(i, s)| s.alive && Some(*i) != excluded)
            .all(|(i, s)| {
                let width = (collision_distance(s.segments, segments) + 40.).max(clearance);
                // Include every head/body edge in the box and the requested
                // clearance, with slack for exact-distance roundoff. Unusual
                // coordinates retain the exhaustive path. See spawn_bounds.rs.
                if let (Some(query), Some(bounds)) = (query, bounds[i])
                    && !bounds.near(query, width + 1.)
                {
                    return true;
                }
                line.windows(2).all(|edge| {
                    let (head, tail) = (edge[0], edge[1]);
                    // Include the head even if an externally supplied body is empty.
                    if segment_distance(s.head, head, tail) <= width {
                        return false;
                    }
                    let mut a = s.head;
                    s.body.iter().all(|&b| {
                        let distance = swept_distance(head, tail, a, b);
                        a = b;
                        distance > width
                    })
                })
            })
    }

    /// Replace a dead snake in its existing slot and clear all per-life internal state.
    /// Population runners own spawn placement; this does not create death food.
    pub fn respawn(
        &mut self,
        index: usize,
        head: Point,
        heading: f64,
        segments: usize,
    ) -> Result<(), SimError> {
        let s = self.snakes.get(index).ok_or_else(|| {
            SimError::new(ErrorKind::UnknownSlot, "Unknown respawn slot").at_slot(index)
        })?;
        if s.alive || !head.finite() || !heading.is_finite() || !(2..=430).contains(&segments) {
            return Err(SimError::state(
                if s.alive {
                    Constraint::InconsistentState
                } else if !head.finite() || !heading.is_finite() {
                    Constraint::NonFinite
                } else {
                    Constraint::OutOfRange
                },
                if s.alive {
                    "alive"
                } else if !head.finite() {
                    "head"
                } else if !heading.is_finite() {
                    "heading"
                } else {
                    "segments"
                },
                "Respawn requires a dead slot, finite pose and 2..430 segments",
            )
            .at_slot(index));
        }
        let body = (0..segments)
            .map(|i| Point {
                x: head.x - heading.cos() * 42.0 * i as f64,
                y: head.y - heading.sin() * 42.0 * i as f64,
            })
            .collect();
        self.respawn_body(index, body, heading)
    }
    /// Respawn a dead slot with a supplied head-first body (2..=430 finite anchors).
    /// Every anchor must be inside the arena. Resets the same per-life state as
    /// `respawn`; call `body_is_clear` separately if placement needs exclusion.
    /// With `minScore`, the body needs at least the floor's segments; at exactly
    /// that many, the life starts at the floor's mass.
    pub fn respawn_body(
        &mut self,
        index: usize,
        body: Vec<Point>,
        heading: f64,
    ) -> Result<(), SimError> {
        let s = self.snakes.get(index).ok_or_else(|| {
            SimError::new(ErrorKind::UnknownSlot, "Unknown respawn slot").at_slot(index)
        })?;
        if s.alive
            || !heading.is_finite()
            || !(2..=430).contains(&body.len())
            || body.iter().any(|p| !p.finite())
        {
            return Err(SimError::state(
                if s.alive {
                    Constraint::InconsistentState
                } else if !heading.is_finite() || body.iter().any(|p| !p.finite()) {
                    Constraint::NonFinite
                } else {
                    Constraint::OutOfRange
                },
                if s.alive {
                    "alive"
                } else if !heading.is_finite() {
                    "heading"
                } else {
                    "body"
                },
                "Respawn requires a dead slot, finite pose and 2..430 segments",
            )
            .at_slot(index));
        }
        if body
            .iter()
            .any(|p| p.distance(Point::default()) >= self.radius)
        {
            return Err(SimError::state(
                Constraint::OutsideArena,
                "body",
                "Spawn body extends outside arena",
            )
            .at_slot(index));
        }
        // Large finite angles lose per-tick turn increments to rounding.
        let heading = if heading.abs() > TAU {
            heading.sin().atan2(heading.cos())
        } else {
            heading
        };
        self.validate_spawn(&body, heading)
            .map_err(|e| e.at_slot(index))?;
        let floor = self.min_mass;
        if let Some(floor) = floor.filter(|floor| body.len() < floor.segments) {
            return Err(SimError::state(
                Constraint::OutOfRange,
                "body",
                format!(
                    "minScore needs a spawn body of at least {} segments",
                    floor.segments
                ),
            )
            .at_slot(index));
        }
        self.check_join().map_err(|e| e.at_slot(index))?;
        let pit_entered = self.initial_pit_entry(body[0], true);
        let s = &mut self.snakes[index];
        s.head = body[0];
        s.segments = body.len();
        s.body = body;
        s.heading = heading;
        s.speed = base_speed(s.segments);
        s.fam = floor
            .filter(|floor| floor.segments == s.segments)
            .map_or(0.0, |floor| floor.fam);
        s.kills = 0;
        s.alive = true;
        Self::update_mass(s);
        self.boosts[index] = Boost::default();
        self.speeds[index] = SpeedState::idle(s.speed);
        self.kiosks[index] = KioskState::default();
        self.pit_entered[index] = pit_entered;
        self.death_times[index] = None;
        self.joins[index] = self.next_join;
        self.next_join += 1;
        self.refresh_spawn_bounds(index);
        Ok(())
    }
    pub fn living(&self) -> usize {
        self.snakes.iter().filter(|s| s.alive).count()
    }
    /// Living seats plus physics-dead seats waiting to respawn. Dormant and
    /// explicitly retired slots are vacant; respawning reoccupies them.
    pub fn occupied(&self) -> usize {
        self.snakes
            .iter()
            .zip(&self.death_times)
            .filter(|(snake, death)| snake.alive || death.is_some())
            .count()
    }

    /// Live pit geometry; circular seat-following scales it with the wall so
    /// its minimap position and the stock player's own marker agree.
    pub fn pit_radius(&self) -> Option<f64> {
        self.config
            .pit
            .as_ref()
            .map(|pit| pit.radius * self.config.circular_scale(self.radius))
    }

    pub(crate) fn food_radius(&self) -> f64 {
        if self.config.food_only_inside_pit {
            self.pit_radius().unwrap().min(self.radius)
        } else {
            self.radius
        }
    }

    fn food_area_fraction(&self) -> f64 {
        (self.food_radius() / self.radius).powi(2)
    }

    fn food_position_allowed(&self, point: Point) -> bool {
        !self.config.food_only_inside_pit || point.distance_lt(Point::default(), self.food_radius())
    }

    fn initial_pit_entry(&self, head: Point, alive: bool) -> bool {
        // Moving pits count physical entry, so a life explicitly initialized
        // inside must start entered. Outside newborns are not trapped when
        // growth carries the pit over them. Fixed worlds retain their rules.
        alive
            && self.config.dynamic_radius
            && self.config.min_score.is_some()
            && self
                .pit_radius()
                .is_some_and(|pit| head.distance_lt(Point::default(), pit))
    }

    /// Give queued host seats room before searching, without saving a second
    /// population/controller state. Shrink still belongs to update_radius.
    #[cfg(feature = "networking")]
    /// False leaves the radius untouched when a recent host birth holds growth.
    pub(crate) fn grow_for_seats(&mut self, seats: usize, permitted: bool) -> bool {
        if self.config.dynamic_radius
            && self.config.spawn_radius.is_some()
            && self.config.min_score.is_some()
        {
            let target = self.config.seat_radius(seats).min(checkpoint::MAGNITUDE);
            if target - self.radius > RADIUS_DEAD_ZONE * self.radius {
                if !permitted {
                    return false;
                }
                self.radius = target;
            }
        }
        true
    }
    /// Advance one physics tick (`config.dt`, the 7 ms server loop by default).
    /// Actions use stable spawn slots; dead slots may be None. Rejected actions
    /// change nothing. If a pickup would overflow its eater's mass, the tick
    /// completes without that gain, returns an error and stops the world;
    /// every later step is rejected.
    pub fn step(&mut self, actions: &[Option<Action>]) -> Result<(), CoreStepError> {
        self.validate_actions(actions)
            .map_err(CoreStepError::Rejected)?;
        self.advance_validated(actions)
            .map_err(CoreStepError::Stopped)
    }
    /// Persistent terminal cause, including after later rejected retries.
    pub fn status(&self) -> WorldStatus<'_> {
        self.failure
            .as_ref()
            .map_or(WorldStatus::Running, WorldStatus::Stopped)
    }
    pub fn validate_actions(&self, actions: &[Option<Action>]) -> Result<(), SimError> {
        if let Some(failure) = &self.failure {
            let mut error = failure.clone();
            error.kind = ErrorKind::WorldStopped;
            return Err(error);
        }
        if actions.len() != self.snakes.len() {
            return Err(SimError::new(
                ErrorKind::ActionShape {
                    expected: self.snakes.len(),
                    actual: actions.len(),
                },
                "Action batch must have one slot per spawned agent",
            )
            .at_field("actions"));
        }
        for (slot, (s, a)) in self.snakes.iter().zip(actions).enumerate() {
            if s.alive && a.is_none_or(|a| !a.heading.is_finite()) {
                return Err(SimError::new(
                    ErrorKind::InvalidAction {
                        rule: if a.is_none() {
                            Constraint::Missing
                        } else {
                            Constraint::NonFinite
                        },
                    },
                    format!("Missing or invalid action for {}", s.id),
                )
                .at_slot(slot)
                .at_field("heading"));
            }
        }
        Ok(())
    }
    pub fn step_profiled(
        &mut self,
        actions: &[Option<Action>],
        profile: &mut StepProfile,
    ) -> Result<(), CoreStepError> {
        self.validate_actions(actions)
            .map_err(CoreStepError::Rejected)?;
        self.advance::<true>(actions, profile);
        self.failure
            .clone()
            .map_or(Ok(()), |cause| Err(CoreStepError::Stopped(cause)))
    }
    /// Advance validated actions; errors only when the tick stopped the world.
    pub(crate) fn advance_validated(&mut self, actions: &[Option<Action>]) -> Result<(), SimError> {
        self.advance::<false>(actions, &mut StepProfile::default());
        self.failure.clone().map_or(Ok(()), Err)
    }
    fn advance<const PROFILE: bool>(
        &mut self,
        actions: &[Option<Action>],
        profile: &mut StepProfile,
    ) {
        self.spawn_bounds.take();
        let mut clock = if PROFILE { Some(Instant::now()) } else { None };
        if let Some(events) = &mut self.pickup_events {
            events.clear();
        }
        if let Some(events) = &mut self.food_spawn_events {
            events.clear();
        }
        let previous_pit_radius = self.pit_radius();
        self.update_radius(self.time + self.config.dt);
        let pit_radius = self.pit_radius();
        let mut buffers = std::mem::take(&mut self.buffers);
        let StepBuffers {
            alive,
            previous,
            last_tips,
            boundary_deaths,
            bodies,
            heads,
            order,
            candidates,
            dead,
            fallen,
            eaters,
            hits,
            retained_food: _,
            prey: prey_buffers,
        } = &mut buffers;
        let c = &self.config;
        alive.clear();
        alive.extend(
            self.snakes
                .iter()
                .enumerate()
                .filter(|(_, s)| s.alive)
                .map(|(i, _)| i),
        );
        previous.clear();
        previous.extend(self.snakes.iter().map(|s| s.head));
        last_tips.clear();
        last_tips.extend(
            self.snakes
                .iter()
                .map(|s| head_tip(s.head, s.heading, s.segments)),
        );
        boundary_deaths.clear();
        // Public food edits reset the cache; radius retention and the internal
        // pushes/pickups are mirrored. Reuse verification while that ownership
        // invariant and the mirrored length still hold.
        self.food_cache.begin_mirrored(&self.food);
        // Every living slot writes its current direction during movement below.
        // Retained values are never used for a new step or a dead slot.
        heads.resize(self.snakes.len(), (Point::default(), (0.0, 0.0), 0.0));
        for &i in alive.iter() {
            let a = actions[i].expect("validated action");
            let s = &mut self.snakes[i];
            let state = &mut self.boosts[i];
            state.held = a.boost;
            if !a.boost {
                state.exhausted = false;
            }
            // Boosting needs mass above the two-segment minimum; real 2-segment snakes keep
            // boosting down to fam ~0. A `minScore` floor keeps boosting allowed:
            // at the floor it is free speed, a mode design choice.
            let floor = self.min_mass;
            let can_boost = floor.is_some() || s.segments > 2 || s.fam > 0.0;
            if a.boost && can_boost && !state.exhausted && !state.active {
                // The first pellet usually drops within a loop of the boost request.
                state.clock = BOOST_DROP_SECONDS;
            }
            state.active = a.boost && can_boost && !state.exhausted;
            let boosting = state.active;
            if boosting {
                state.active_boost_seconds += c.dt;
            }
            let sc = scale(s.segments);
            let speed = &mut self.speeds[i];
            // Checkpoint v1 can retain a pending observed-speed inference from
            // the former public-state API. Preserve that continuation behavior.
            if speed.speed != s.speed {
                *speed = SpeedState::from_observed_speed(s.speed, s.segments);
            }
            s.speed = speed.step(boosting, s.segments, c.dt, c.speed_model);
            if boosting {
                let elapsed = state.clock + c.dt;
                // Counted, not subtracted until the clock runs out: at a long dt
                // the subtraction stops changing the clock, and a 430-segment
                // snake's boost costs no mass, so nothing else ends the loop.
                // With at most one drop, as at any measured dt, this matches
                // that subtraction exactly.
                let (drops, rest) = drops_in(elapsed);
                let mut dropped = 0;
                while dropped < drops && state.active {
                    dropped += 1;
                    let tail = s.body.last().copied().unwrap_or(s.head);
                    let mult = multiplier(s.segments as f64 + s.fam);
                    // Pellets match the mass spent: nothing at the floor, so free
                    // boosting never makes food, and a smaller last pellet above it.
                    let available = floor.map_or(s.segments as f64 - 2.0 + s.fam, |floor| {
                        floor.excess(s.segments, s.fam)
                    });
                    let cost =
                        available.min(boost_cost(s.segments) * (0.94 + 0.14 * self.random.next()));
                    let removed = remove_mass_above_floor(&mut s.segments, &mut s.fam, cost, floor);
                    state.boost_mass_spent += cost;
                    s.body.truncate(s.body.len() - removed);
                    let size = ((cost * 0.95 / (NUTRITION * mult)).sqrt() * 5.0).floor() / 5.0;
                    if size > 0.0 {
                        let jitter = sc * 16.0;
                        let x = tail.x + (self.random.next() - 0.5) * jitter;
                        let y = tail.y + (self.random.next() - 0.5) * jitter;
                        let pellet = Self::pellet(Point { x, y }, size, Origin::Boost);
                        if c.emit_boost_food
                            && (!c.food_only_inside_pit
                                || pellet.point().distance_lt(
                                    Point::default(),
                                    pit_radius.unwrap().min(self.radius),
                                ))
                        {
                            // Earlier admitted drops in this same step share the capacity.
                            let admitted = !c.boost_food.enabled
                                || self.food_cache.has_room(
                                    &self.food,
                                    &pellet,
                                    c.boost_food.cell_size,
                                    c.boost_food.max_per_cell,
                                );
                            if admitted {
                                if let Some(events) = &mut self.food_spawn_events {
                                    events.push(FoodSpawnEvent {
                                        source: Some(i),
                                        food: pellet,
                                    });
                                }
                                self.food_cache.push(&pellet);
                                self.food.push(pellet);
                            }
                        }
                    }
                    if floor.is_none() && s.segments == 2 && s.fam <= 0.0 {
                        state.active = false;
                        state.exhausted = true;
                    }
                }
                state.clock = if dropped == drops {
                    rest
                } else {
                    elapsed - dropped as f64 * BOOST_DROP_SECONDS
                };
            } else {
                state.clock = 0.0;
            }
            (s.head, s.heading, heads[i].1) = advance_directed(
                s.head,
                s.heading,
                quantize_heading(a.heading),
                s.speed,
                turn_rate(s.segments),
                c.dt,
            );
            let (head, lethal) =
                crate::boundary::constrain(s.head, s.heading, sc, self.radius, c.boundary_mode);
            s.head = head;
            if lethal {
                boundary_deaths.push((i, DeathReason::Boundary));
            } else if let Some(pit) = pit_radius {
                // A mode rule, not an official mechanic: docs/configuration.md#battledome.
                // The swept head-center path counts, so the rule does not depend on
                // dt: crossing the pit or clipping its edge within a tick also kills.
                // Growth overtaking a head is not physical entry. The existing
                // entry bit keeps engulfed newborns free to leave, while lives
                // initialized inside and later inward crossings remain one-way.
                // research/battledome/2026-10-26-spawn-capacity/REPORT.md
                let can_enter = !c.dynamic_radius
                    || c.min_score.is_none()
                    || !previous[i].distance_lt(
                        Point::default(),
                        pit.max(previous_pit_radius.unwrap_or(pit)),
                    );
                let touched =
                    self.pit_entered[i] || (can_enter && reaches_center(previous[i], s.head, pit));
                if touched {
                    if s.head.distance_lt(Point::default(), pit) {
                        self.pit_entered[i] = true;
                    } else {
                        boundary_deaths.push((i, DeathReason::Pit));
                    }
                }
            }
            if let Some(anchor) = crossed_anchor(s.head, s.body[0]) {
                let grow = s.fam >= 1.0;
                if grow {
                    s.fam -= 1.0;
                    s.segments += 1;
                }
                append_anchor(&mut s.body, anchor, grow);
            }
            Self::update_mass(s);
            self.kiosks[i].step(s, &c.kiosk, c.dt, self.min_mass);
            Self::update_mass(s);
        }
        if PROFILE {
            profile.movement_seconds += clock.unwrap().elapsed().as_secs_f64();
            clock = Some(Instant::now());
        }
        // Stable-slot indexing avoids searching the living list for every pair.
        bodies.clear();
        bodies.extend(self.snakes.iter().map(|s| {
            if !s.alive {
                return None;
            }
            let mut bounds = Bounds::between(s.head, s.head);
            for &p in &s.body {
                bounds.include(p);
            }
            Some(bounds)
        }));
        // Each living snake's tip, heading direction and scale, as `head_tip` and
        // `strikes` compute them, evaluated once rather than per pair. Dead
        // slots are never probed or tested.
        for (s, head) in self.snakes.iter().zip(heads.iter_mut()) {
            if !s.alive {
                *head = (Point::default(), (0.0, 0.0), 0.0);
                continue;
            }
            // Boundary and kiosk processing leave the moved heading unchanged.
            // Recompute scale after boost/kiosk mass changes, but reuse direction.
            let scale = scale(s.segments);
            let direction = head.1;
            debug_assert_eq!(
                (direction.0.to_bits(), direction.1.to_bits()),
                (s.heading.cos().to_bits(), s.heading.sin().to_bits()),
                "movement must supply the current heading direction"
            );
            let r = BODY_RADIUS * scale;
            let tip = Point {
                x: s.head.x + direction.0 * r,
                y: s.head.y + direction.1 * r,
            };
            *head = (tip, direction, scale);
        }
        // The offline Android client resolves collisions oldest first against
        // newer snakes first, skipping those already dead. We preserve that
        // resolution after moving all snakes; online move/test phasing is unverified.
        order.clear();
        order.extend_from_slice(alive);
        order.sort_unstable_by_key(|&i| self.joins[i]);
        // Small worlds test every pair faster than they build an index.
        let index = (order.len() >= 32).then(|| {
            let bounds: Vec<Bounds> = order
                .iter()
                .map(|&j| bodies[j].expect("living snake has bounds"))
                .collect();
            let longest = order.iter().map(|&j| self.snakes[j].segments).max();
            (BoundsIndex::new(&bounds), longest.unwrap_or(0))
        });
        candidates.clear();
        dead.clear();
        fallen.clear();
        fallen.resize(self.snakes.len(), false);
        for &i in order.iter() {
            if let Ok(k) = boundary_deaths.binary_search_by_key(&i, |&(j, _)| j) {
                dead.push((i, None, boundary_deaths[k].1));
                fallen[i] = true;
                continue;
            }
            let s = &self.snakes[i];
            let (tip, direction, head_scale) = heads[i];
            let probe = Bounds::between(tip, tip);
            let indexed = index.as_ref().is_some_and(|(index, longest)| {
                // Scale grows with segments, so this bounds every pair's reach below.
                let reach = 3.666 * collision_distance(s.segments, *longest);
                index.candidates(probe, reach, candidates)
            });
            let test = |j: usize, profile: &mut StepProfile| {
                if i == j || fallen[j] {
                    return false;
                }
                let other_bounds = bodies[j].expect("living snake has bounds");
                let other = &self.snakes[j];
                let (other_tip, _, body_scale) = heads[j];
                if PROFILE {
                    profile.body_pairs += 1;
                }
                // `3.666 * collision_distance(head, body)` with the same operation order.
                let reach = 3.666 * (BODY_RADIUS * (head_scale + body_scale));
                if !other_bounds.near(probe, reach) {
                    return false;
                }
                if PROFILE {
                    profile.segment_tests += self.snakes[j].body.len() as u64;
                }
                let line = [other_tip, other.head]
                    .into_iter()
                    .chain(other.body.iter().copied());
                strikes_scaled(last_tips[i], tip, direction, head_scale, body_scale, line)
            };
            // Both visit positions newest first; the index omits only snakes
            // whose bounds fail the test above.
            let killer = if indexed {
                candidates
                    .iter()
                    .map(|&k| order[k as usize])
                    .find(|&j| test(j, profile))
            } else {
                order.iter().rev().copied().find(|&j| test(j, profile))
            };
            if killer.is_some() {
                dead.push((i, killer, DeathReason::Body));
                fallen[i] = true;
            }
        }
        if PROFILE {
            profile.collision_seconds += clock.unwrap().elapsed().as_secs_f64();
            clock = Some(Instant::now());
        }
        self.time += c.dt;
        if let Some(window) = c.kill_score_seconds {
            for times in &mut self.kill_times {
                while times.front().is_some_and(|&t| t + window <= self.time) {
                    times.pop_front();
                }
            }
        }
        for &(i, killer, reason) in dead.iter() {
            self.snakes[i].alive = false;
            self.death_times[i] = Some(self.time);
            if let Some(k) = killer {
                // A restored checkpoint may carry any count, and one tick can
                // credit several kills.
                self.snakes[k].kills = self.snakes[k].kills.saturating_add(1);
                if self.config.kill_score_seconds.is_some() {
                    self.kill_times[k].push_back(self.time);
                }
            }
            let food = match reason {
                // Pit deaths drop normal death food: a mode design choice,
                // see docs/configuration.md#battledome.
                DeathReason::Body | DeathReason::Pit => self.death_food(i),
                // Outward browser controls produced no drops, including from
                // body anchors still inside the arena. See ecology evidence.
                DeathReason::Boundary => 0,
            };
            self.deaths.push(DeathEvent {
                time: self.time,
                victim: self.snakes[i].id.clone(),
                killer: killer.map(|k| self.snakes[k].id.clone()),
                reason,
                food,
            });
        }
        if PROFILE {
            profile.death_seconds += clock.unwrap().elapsed().as_secs_f64();
            clock = Some(Instant::now());
        }
        // Without food there is nothing to index or eat.
        eaters.clear();
        if !self.food.is_empty() {
            eaters.extend(alive.iter().filter(|&&i| self.snakes[i].alive).map(|&i| {
                let s = &self.snakes[i];
                let (center, radius) =
                    pickup_circle_directed(s.head, heads[i].1, s.speed, s.segments);
                (i, center, radius, Bounds::between(center, center))
            }));
        }
        // Index the few eater circles, then visit food in reverse order; ties go to the
        // nearest head, then the lexically smaller id.
        const CELL: f64 = 256.0;
        // The dense grid lists exactly the eaters whose cover holds a pellet's cell,
        // so it agrees with both paths below. They remain for circles it cannot index.
        // A grid much larger than the food list costs more to allocate than it saves.
        let dense = self.config.food_index
            && !eaters.is_empty()
            && self.eater_grid.build(
                eaters
                    .iter()
                    .map(|&(_, center, radius, _)| (center, radius)),
                self.food.len().saturating_mul(4).max(4096),
            );
        let indexed = !dense
            && self.config.food_index
            && eaters.len().saturating_mul(self.food.len()) >= 32000;
        let groups = eaters.len().div_ceil(64);
        // Hash collisions add candidates only; they can never remove a real candidate.
        let bucket = |x: i64, y: i64| {
            ((x as u64).wrapping_mul(73856093) ^ (y as u64).wrapping_mul(19349663)) as usize & 255
        };
        let mut grid = if indexed {
            vec![0u64; 256 * groups]
        } else {
            vec![]
        };
        if indexed {
            for (e, &(_, _, width, bounds)) in eaters.iter().enumerate() {
                let lo_x = ((bounds.min.x - width - 1e-9) / CELL).floor() as i64;
                let hi_x = ((bounds.max.x + width + 1e-9) / CELL).floor() as i64;
                let lo_y = ((bounds.min.y - width - 1e-9) / CELL).floor() as i64;
                let hi_y = ((bounds.max.y + width + 1e-9) / CELL).floor() as i64;
                for x in lo_x..=hi_x {
                    for y in lo_y..=hi_y {
                        grid[bucket(x, y) * groups + e / 64] |= 1u64 << (e % 64);
                    }
                }
            }
        }
        // Eating changes only mass, which no pickup test reads, so eaters are found
        // first and then applied in the same reverse food order.
        hits.clear();
        if dense {
            if let Some(pellets) = self.food_cache.pellets(&self.food) {
                indexed_hits::<PROFILE>(
                    &self.eater_grid,
                    pellets,
                    &self.snakes,
                    eaters,
                    &self.food,
                    hits,
                    profile,
                );
            } else {
                dense_hits::<PROFILE>(
                    &self.eater_grid,
                    &self.snakes,
                    eaters,
                    &self.food,
                    hits,
                    profile,
                );
            }
        } else {
            let mut indexed_candidates = Vec::with_capacity(eaters.len());
            for (findex, f) in self.food.iter().enumerate().rev() {
                let p = f.point();
                let eater = if indexed {
                    indexed_candidates.clear();
                    let offset =
                        bucket((p.x / CELL).floor() as i64, (p.y / CELL).floor() as i64) * groups;
                    for g in 0..groups {
                        let mut mask = grid[offset + g];
                        while mask != 0 {
                            indexed_candidates.push(g * 64 + mask.trailing_zeros() as usize);
                            mask &= mask - 1;
                        }
                    }
                    find_eater::<PROFILE>(
                        &self.snakes,
                        p,
                        indexed_candidates.iter().map(|&e| eaters[e]),
                        profile,
                    )
                } else {
                    find_eater::<PROFILE>(&self.snakes, p, eaters.iter().copied(), profile)
                };
                if let Some(i) = eater {
                    hits.push((findex, i));
                }
            }
        }
        for &(findex, i) in hits.iter() {
            let f = &self.food[findex];
            if let Some(events) = &mut self.pickup_events {
                events.push(PickupEvent {
                    eater: i,
                    item: PickupItem::Food(*f),
                });
            }
            let s = &mut self.snakes[i];
            let gain = f
                .size
                .map_or(f.value, |size| nutrition(size, s.segments as f64 + s.fam));
            gain_mass(s, i, gain, "food", &mut self.failure);
        }
        if !hits.is_empty() {
            // Both pickup paths append hits in reverse food order. Reversing them
            // lets the cache compact survivor ranges without changing that order.
            self.food_cache.remove(
                self.food.len(),
                hits.iter().rev().map(|&(findex, _)| findex),
            );
            compact_food(&mut self.food, hits.iter().rev().map(|&(findex, _)| findex));
        }
        if PROFILE {
            profile.food_seconds += clock.unwrap().elapsed().as_secs_f64();
            clock = Some(Instant::now());
        }
        self.advance_prey(previous, heads, eaters, prey_buffers);
        if PROFILE {
            profile.prey_seconds += clock.unwrap().elapsed().as_secs_f64();
            clock = Some(Instant::now());
        }
        let dt = self.config.dt;
        let target = if self.config.dynamic_radius {
            self.config.ecology_at_radius(self.radius).0
        } else {
            self.config.food_count
        };
        let target = if self.config.food_only_inside_pit {
            (target as f64 * self.food_area_fraction()) as usize
        } else {
            target
        };
        let replenish = self.config.ambient_replenish_per_second;
        // Store credit in initial-area seconds. Weight only this tick's earned
        // credit by live area, so boundary motion never creates/discards credit
        // and checkpoints need no previous-radius/controller state.
        let refill_dt = if replenish > 0.0 && self.config.dynamic_radius {
            dt * (self.radius / self.config.radius).powi(2)
        } else {
            dt
        };
        // Keep the original interval/credit units for checkpoint continuation;
        // only new credit is reduced by the live pit's share of arena area.
        let refill_dt = if self.config.food_only_inside_pit {
            refill_dt * self.food_area_fraction()
        } else {
            refill_dt
        };
        if replenish == 0.0 {
            self.ambient_clock = 0.0;
        } else if !(self.ambient_clock + refill_dt).is_finite()
            || check_ambient_rate(replenish, refill_dt).is_err()
        {
            self.failure.get_or_insert_with(|| {
                SimError::new(
                    ErrorKind::NumericOverflow,
                    "Live-area ambient refill cannot advance its clock safely",
                )
                .at_field("ambientReplenishPerSecond")
            });
        } else {
            let interval = 1.0 / replenish;
            self.ambient_clock += refill_dt;
            if self.ambient_clock + 1e-12 >= interval {
                let mut refill_count = if self.config.ambient_refill_counts_all_food {
                    self.food.len()
                } else {
                    self.food_cache.ambient(&self.food)
                };
                // The tolerance spends an attempt a rounding error early, but only
                // while credit remains: below a 1e-12 s interval it would otherwise
                // drive the clock negative until subtraction stopped changing it.
                // Above that interval a qualifying clock is always positive.
                while self.ambient_clock > 0.0 && self.ambient_clock + 1e-12 >= interval {
                    self.ambient_clock -= interval;
                    if refill_count < target {
                        let f = self.new_food();
                        // Cell occupancy includes every origin.
                        if self.food_position_allowed(f.point())
                            && admit_ambient_food(
                                &f,
                                &self.config.ambient_food,
                                &mut self.food_cache,
                                &self.food,
                            )
                        {
                            if let Some(events) = &mut self.food_spawn_events {
                                events.push(FoodSpawnEvent {
                                    source: None,
                                    food: f,
                                });
                            }
                            self.food_cache.push(&f);
                            self.food.push(f);
                            refill_count += 1;
                        }
                    }
                }
            }
        }
        self.food_cache.end();
        if PROFILE {
            profile.ambient_seconds += clock.unwrap().elapsed().as_secs_f64();
            profile.ticks += 1;
        }
        for (snake, kiosk) in self.snakes.iter_mut().zip(&self.kiosks) {
            kiosk.finish_score(snake);
        }
        self.buffers = buffers;
    }
    fn update_radius(&mut self, time: f64) {
        // Product/feel model, not an identified official law. Occupied seats
        // avoid death/respawn jitter; the stateless dead zone absorbs small churn.
        // research/fidelity/2026-10-26-arena-radius/SEAT-FOLLOWING.md
        let previous = self.radius;
        if self.config.dynamic_radius {
            let target = self.config.seat_radius(self.occupied());
            if (target - self.radius).abs() > RADIUS_DEAD_ZONE * self.radius {
                // Expansion gives joining players room without a multi-minute
                // wait. Shrink stays slower than ordinary snake movement.
                self.radius = if target > self.radius {
                    target
                } else {
                    (self.radius - RADIUS_RATE * self.config.dt).max(target)
                };
            }
        }
        // Tick comparisons retain their tolerance; construction uses exact zero
        // so a near-future recorded sample cannot affect the initial frame.
        self.apply_radius_updates(time + 1e-9);
        if self.config.food_only_inside_pit && self.radius != previous {
            // Custom pit food disappears at a moving lethal edge; never move
            // stranded pellets inward or leave them suppressing total-food refill.
            // docs/configuration.md#battledome and pit_food_tests
            let radius = self.food_radius();
            self.retain_food(radius);
        } else if self.config.dynamic_radius && self.radius < previous {
            // Stranded pellets must not suppress live-area refill. Fixed replay
            // keeps its historical retention; all food origins leave with this
            // model's wall. Mirror removals to preserve the warm cache.
            // Use squared distances except at numerically ambiguous boundaries.
            self.retain_food(self.radius);
        }
    }

    fn retain_food(&mut self, radius: f64) {
        let removed = &mut self.buffers.retained_food;
        removed.clear();
        removed.extend(
            self.food.iter().enumerate().filter_map(|(i, f)| {
                (!f.point().distance_lt(Point::default(), radius)).then_some(i)
            }),
        );
        if !removed.is_empty() {
            // Keep retain's strict boundary and survivor order, but mirror the
            // ascending original indices before compaction. Warm counts and
            // stable pellet handles then survive a moving wall; cold/reset
            // caches still undergo their normal verification on the next use.
            // research/performance/2026-10-27-mirrored-food-retention/REPORT.md
            self.food_cache
                .remove(self.food.len(), removed.iter().copied());
            compact_food(&mut self.food, removed.iter().copied());
        }
        removed.clear();
    }

    fn apply_radius_updates(&mut self, through: f64) {
        while let Some(update) = self.config.radius_updates.get(self.radius_update_index) {
            if update.time > through {
                break;
            }
            self.radius = update.radius;
            self.radius_update_index += 1;
        }
    }

    fn spawn_prey(&mut self) {
        while self.prey.iter().any(|prey| prey.id == self.next_prey_id) {
            self.next_prey_id = self.next_prey_id.wrapping_add(1);
        }
        self.push_prey();
    }

    /// Spawn a prey with `next_prey_id`, which the caller has made unique.
    fn push_prey(&mut self) {
        let angle = self.prey_random.next() * TAU;
        let radius = self.prey_random.next().sqrt() * self.radius * 0.95;
        let heading = self.prey_random.next() * TAU;
        let (min_size, max_size) = self
            .config
            .prey
            .size_steps()
            .expect("validated prey size range");
        self.prey.push(Prey {
            id: self.next_prey_id,
            position: Point {
                x: angle.cos() * radius,
                y: angle.sin() * radius,
            },
            heading,
            target_heading: heading,
            speed: self.config.prey.cruise_speed,
            size: ((self.config.prey.size
                + self.prey_random.next() * (self.config.prey.max_size - self.config.prey.size))
                * 5.0)
                .round()
                .clamp(min_size, max_size)
                / 5.0,
        });
        self.next_prey_id = self.next_prey_id.wrapping_add(1);
    }

    /// Initialize an observed prey mode without inventing its server-side trigger.
    /// Existing ordinary decision phase is preserved. A newly initialized
    /// outward clip defers its next decision by one third of a second; a new
    /// roaming prey keeps its immediate first-step decision. Mode remains internal,
    /// so observations retain their public schema. The exact-11 pulse is not
    /// modeled: its advertised speed does not reproduce recorded displacement.
    pub fn set_prey_mode(&mut self, id: u64, mode: PreyMode) -> Result<(), SimError> {
        let index = self
            .prey
            .iter()
            .position(|prey| prey.id == id)
            .ok_or_else(|| {
                SimError::new(ErrorKind::UnknownEntity, "Unknown prey identity").at_field("prey.id")
            })?;
        if mode == PreyMode::Outward {
            self.prey_outward.try_reserve(1).map_err(|_| {
                SimError::new(
                    ErrorKind::AllocationFailed,
                    "Prey modes do not fit in memory",
                )
            })?;
            self.prey_decision_times.try_reserve(1).map_err(|_| {
                SimError::new(
                    ErrorKind::AllocationFailed,
                    "Prey decisions do not fit in memory",
                )
            })?;
        }
        let prey = &mut self.prey[index];
        match mode {
            PreyMode::Roaming => {
                self.prey_outward.remove(&id);
                prey.speed = self.config.prey.cruise_speed;
            }
            PreyMode::Outward => {
                self.prey_decision_times
                    .entry(id)
                    .or_insert(self.time + 1.0 / 3.0);
                self.prey_outward.insert(id);
                prey.speed = OUTWARD_SPEED;
                prey.target_heading = prey.position.y.atan2(prey.position.x);
            }
        }
        Ok(())
    }

    fn decide_prey(
        prey: &mut Prey,
        outward: bool,
        snakes: &[Snake],
        c: &Config,
        radius: f64,
        random: &mut Random,
    ) {
        let p = &c.prey;
        let old = prey.position;
        // Only live heads trigger ordinary flight. The outward-state activation
        // comes separately from recorded evidence through set_prey_mode.
        let threat = snakes
            .iter()
            .filter(|s| {
                s.alive
                    && (old.x - s.head.x).abs() < p.flee_half_width
                    && (old.y - s.head.y).abs() < p.flee_half_width
            })
            .map(|s| (s, old.distance(s.head)))
            .min_by(|(a, x), (b, y)| x.total_cmp(y).then_with(|| a.id.cmp(&b.id)));
        if let Some((s, _)) = threat {
            prey.target_heading = (old.y - s.head.y).atan2(old.x - s.head.x);
            prey.target_heading += (random.next() - 0.5) * 0.6;
        } else if outward {
            prey.target_heading = old.y.atan2(old.x);
        } else if random.next() < 0.3 {
            prey.target_heading += (random.next() - 0.5) * 2.4;
        }
        prey.speed = threat
            .map(|(snake, _)| p.flight_speed(snake.speed))
            .unwrap_or(if outward {
                OUTWARD_SPEED
            } else {
                p.cruise_speed
            });
        // Roaming rim behavior remains provisional. Observed outward prey must
        // not be forced inward here; removal at the boundary is still unknown.
        if !outward && old.distance(Point::default()) > radius * 0.9 {
            prey.target_heading = (-old.y).atan2(-old.x);
        }
    }

    fn advance_prey(
        &mut self,
        previous: &[Point],
        heads: &[(Point, (f64, f64), f64)],
        food_circles: &[(usize, Point, f64, Bounds)],
        buffers: &mut PreyBuffers,
    ) {
        let c = &self.config;
        let p = &c.prey;
        let target = if c.dynamic_radius {
            c.ecology_at_radius(self.radius).2
        } else {
            p.count
        };
        // A shrinking model arena must not retain its old population compressed
        // against the rim. Stable insertion order makes surplus removal deterministic.
        if c.dynamic_radius {
            self.prey.truncate(target);
        }
        let below_target = self.prey.len() < target;
        // A full population carries no saved replenishment credit.
        if !below_target {
            self.prey_clock = 0.0;
        }
        // Preserve integer-coordinate steps even when a caller batches snake
        // simulation into a larger dt. Rounding one large displacement would
        // invent movement for stationary, sub-unit-per-tick prey.
        self.prey_motion_clock += c.dt;
        let motion_ticks = ((self.prey_motion_clock + 1e-12) / LOOP_SECONDS).floor() as usize;
        self.prey_motion_clock =
            (self.prey_motion_clock - motion_ticks as f64 * LOOP_SECONDS).max(0.0);
        // Public callers may insert/remove prey. Retain clocks only for present
        // identities, including through empty periods and after consumption.
        let PreyBuffers {
            live_ids,
            circles,
            slots,
            sweeps,
            candidates,
            path,
        } = buffers;
        live_ids.clear();
        live_ids.extend(self.prey.iter().map(|prey| prey.id));
        self.prey_decision_times
            .retain(|id, _| live_ids.contains(id));
        self.prey_outward.retain(|id| live_ids.contains(id));
        // The global motion-tick phase above survives empty periods.
        if self.prey.is_empty() && target == 0 {
            return;
        }
        // The same pickup circle as food. Eating prey changes only mass, so each
        // living snake's circle and lead stay fixed for this whole phase.
        circles.clear();
        if food_circles.is_empty() {
            // Empty food skips its circle pass; prey still needs current geometry.
            circles.extend(self.snakes.iter().enumerate().map(|(i, s)| {
                s.alive.then(|| {
                    let (center, radius) =
                        pickup_circle_directed(s.head, heads[i].1, s.speed, s.segments);
                    let lead = Point {
                        x: center.x - s.head.x,
                        y: center.y - s.head.y,
                    };
                    (center, radius, lead)
                })
            }));
        } else {
            // Food only changes fam. All current lives, heads, headings, speeds
            // and integer segment counts still match the food-circle pass.
            // Keep slot indexing and recompute the same center-minus-head lead.
            // research/performance/2026-10-27-core-heading-geometry/REPORT.md
            circles.resize(self.snakes.len(), None);
            for &(i, center, radius, _) in food_circles {
                let s = &self.snakes[i];
                #[cfg(debug_assertions)]
                {
                    let expected = pickup_circle(s.head, s.heading, s.speed, s.segments);
                    assert_eq!(
                        (center.x.to_bits(), center.y.to_bits(), radius.to_bits()),
                        (
                            expected.0.x.to_bits(),
                            expected.0.y.to_bits(),
                            expected.1.to_bits()
                        ),
                        "food pickup must preserve prey-circle geometry"
                    );
                    assert!(s.alive, "food pickup must preserve living slots");
                }
                circles[i] = Some((
                    center,
                    radius,
                    Point {
                        x: center.x - s.head.x,
                        y: center.y - s.head.y,
                    },
                ));
            }
        }
        #[cfg(debug_assertions)]
        for (snake, circle) in self.snakes.iter().zip(circles.iter()) {
            assert_eq!(
                snake.alive,
                circle.is_some(),
                "shared pickup geometry must cover every living slot"
            );
        }
        // Index each living snake's swept circle path for many prey. Beyond the
        // largest radius plus one unit, a sweep's both ends lie past the
        // `sweep_misses` limit on one axis, so skipped pairs never eat. Capping
        // every operand of the exact test, not only the sums indexed here, keeps
        // rounding far inside that unit; huge operands could cancel differently.
        const MAGNITUDE: f64 = 1e7;
        let small = |values: &[f64]| values.iter().all(|v| v.abs() <= MAGNITUDE);
        slots.clear();
        sweeps.clear();
        let mut reach = 0.0f64;
        let mut regular = true;
        for (i, circle) in circles.iter().enumerate() {
            if let Some((center, radius, lead)) = *circle {
                let from = Point {
                    x: previous[i].x + lead.x,
                    y: previous[i].y + lead.y,
                };
                regular &= small(&[
                    previous[i].x,
                    previous[i].y,
                    lead.x,
                    lead.y,
                    center.x,
                    center.y,
                    radius,
                ]);
                slots.push(i);
                sweeps.push(Bounds::between(from, center));
                reach = reach.max(radius + 1.0);
            }
        }
        let index =
            (regular && slots.len() * self.prey.len() >= 512).then(|| BoundsIndex::new(sweeps));
        candidates.clear();
        self.prey.retain_mut(|prey| {
            let old = prey.position;
            // Ordinary decisions are per prey, with independently phased first
            // presence. 333 ms checked on simulation steps approximates observed
            // ~338 ms intervals; the hidden decision timer/counter is unknown.
            let next_decision = self.prey_decision_times.entry(prey.id).or_insert(self.time);
            let decide = self.time + 1e-12 >= *next_decision;
            if decide {
                *next_decision = self.time + 1.0 / 3.0;
            }
            let outward = self.prey_outward.contains(&prey.id);
            if decide {
                Self::decide_prey(prey, outward, &self.snakes, c, self.radius, &mut self.prey_random);
            }
            // Keep the default one-substep path allocation-free and preserve its
            // exact endpoint arithmetic. Coarse ticks need every quantized bend.
            path.clear();
            if motion_ticks > 1 {
                path.push(old);
            }
            let mut finite_motion = true;
            for _ in 0..motion_ticks {
                prey.advance_server_tick(p.turn_rate);
                if !prey.position.finite() {
                    finite_motion = false;
                    break;
                }
                if motion_ticks > 1 {
                    path.push(prey.position);
                }
            }
            let distance = prey.position.distance(Point::default());
            // Finite speeds need not produce finite accumulated motion. Preserve
            // the last tick's valid position and use the existing terminal path,
            // rather than allowing rim correction to multiply infinity by zero.
            if !finite_motion || !distance.is_finite() {
                prey.position = old;
                self.failure.get_or_insert_with(|| {
                    SimError::new(ErrorKind::NumericOverflow, "Prey motion overflowed f64; the tick completed without that motion and the world cannot advance further").at_field("prey.position")
                });
                return true;
            }
            if distance >= self.radius {
                let ratio = self.radius * (1.0 - 1e-9) / distance;
                prey.position.x *= ratio;
                prey.position.y *= ratio;
            }
            if let Some(last) = path.last_mut() {
                *last = prey.position;
            }
            // Sweep each computed prey substep relative to linearly interpolated
            // snake motion. Steering and the end-of-tick rim clamp remain models;
            // this tests the computed polyline rather than its endpoint chord.
            // tests/prey.rs pins a curved path whose chord misses the mouth.
            let sweep = |i: usize| {
                // Swept from the previous head's circle to the current one.
                let (center, radius, lead) = circles[i]?;
                let a = Point {
                    x: old.x - previous[i].x - lead.x,
                    y: old.y - previous[i].y - lead.y,
                };
                let b = Point {
                    x: prey.position.x - center.x,
                    y: prey.position.y - center.y,
                };
                let distance = |a, b| {
                    if sweep_misses(a, b, radius) {
                        f64::INFINITY
                    } else {
                        segment_distance(Point::default(), a, b)
                    }
                };
                let d = if motion_ticks > 1 {
                    let mut prior = a;
                    let mut closest = f64::INFINITY;
                    for (k, point) in path.iter().enumerate().skip(1) {
                        let relative = if k == motion_ticks {
                            b
                        } else {
                            let phase = k as f64 / motion_ticks as f64;
                            Point {
                                x: point.x
                                    - (previous[i].x + (self.snakes[i].head.x - previous[i].x) * phase)
                                    - lead.x,
                                y: point.y
                                    - (previous[i].y + (self.snakes[i].head.y - previous[i].y) * phase)
                                    - lead.y,
                            }
                        };
                        closest = closest.min(distance(prior, relative));
                        prior = relative;
                    }
                    closest
                } else {
                    distance(a, b)
                };
                (d <= radius).then_some((i, d))
            };
            let nearest = |(i, a): &(usize, f64), (j, b): &(usize, f64)| {
                a.total_cmp(b)
                    .then_with(|| self.snakes[*i].id.cmp(&self.snakes[*j].id))
            };
            let mut query = Bounds::between(old, prey.position);
            for &point in path.iter() {
                query.include(point);
            }
            let indexed = index.as_ref().is_some_and(|index| {
                small(&[query.min.x, query.min.y, query.max.x, query.max.y])
                    && index.candidates(query, reach, candidates)
            });
            // Both visit slots in ascending order, so ties resolve as before.
            let eater = if indexed {
                candidates
                    .iter()
                    .rev()
                    .filter_map(|&k| sweep(slots[k as usize]))
                    .min_by(nearest)
            } else {
                (0..circles.len()).filter_map(sweep).min_by(nearest)
            }
            .map(|(i, _)| i);
            if let Some(i) = eater {
                self.prey_decision_times.remove(&prey.id);
                self.prey_outward.remove(&prey.id);
                if let Some(events) = &mut self.pickup_events {
                    events.push(PickupEvent {
                        eater: i,
                        item: PickupItem::Prey(prey.clone()),
                    });
                }
                let s = &mut self.snakes[i];
                let gain = nutrition(prey.size, s.segments as f64 + s.fam) * p.nutrition_factor;
                gain_mass(s, i, gain, "prey", &mut self.failure);
                false
            } else {
                true
            }
        });
        if self.prey.len() >= target {
            self.prey_clock = 0.0;
        } else if below_target {
            // Provisional population policy: credit starts AFTER the tick that
            // drops below target, so a catch waits a full respawn interval.
            self.prey_clock += c.dt;
            // Spend all due intervals. The deficit bounds work even when a tiny
            // interval is too small to change the floating-point clock.
            for _ in self.prey.len()..target {
                if self.prey_clock < self.config.prey.respawn_seconds {
                    break;
                }
                self.prey_clock -= self.config.prey.respawn_seconds;
                self.spawn_prey();
            }
            if self.prey.len() >= target {
                self.prey_clock = 0.0;
            }
        }
    }

    fn update_mass(s: &mut Snake) {
        s.length = s.segments as f64 + s.fam;
        s.score = score(s.segments, s.fam);
        s.radius = 14.5 * scale(s.segments);
    }
    fn death_food(&mut self, index: usize) -> usize {
        let s = &self.snakes[index];
        // Legacy custom floor rule, preserved when pit-only food is disabled:
        // map the earned fraction of continuous
        // mass to the same fraction of the ordinary death-food nutrition.
        // Keep normal size/scatter, selecting evenly spaced sites when the
        // cumulative budget crosses an integer in tail-first emission order.
        // The final site carries any fractional nutrition, sqrt-scaled in size.
        // Admission still discards rejected shares rather than redistributing.
        // research/battledome/2026-10-25-food-balance/REPORT.md
        // Pit-only mode instead drops the ordinary full-body budget, including
        // floor-size deaths, then discards outside positions; owner-directed
        // design in docs/configuration.md#battledome, tests in pit_food_tests.
        let site_count = 2 * s.body.len() + 1;
        let budget = self
            .min_mass
            .filter(|_| !self.config.food_only_inside_pit)
            .map(|floor| {
                let mass = s.segments as f64 + s.fam;
                let earned = floor.excess(s.segments, s.fam);
                site_count as f64 * (earned / mass)
            });
        if budget == Some(0.0) {
            return 0;
        }
        let density = &self.config.death_food;
        let food_radius = self.config.food_only_inside_pit.then(|| self.food_radius());
        // Occupancy counts current food, including earlier deaths this same tick.
        let outside_step = !self.food_cache.is_open();
        if outside_step {
            self.food_cache.begin();
        }
        let mut count = 0;
        // Ordered official packets favor independent coordinate offsets in a
        // square; 15 is an empirical half-width (see death-jitter-audit.md).
        let spread = 15.0 * scale(s.segments);
        let size = death_food_size(s.segments);
        let mut next_whole = 1.0;
        let sites = death_food_sites(&s.body, s.head)
            .rev()
            .enumerate()
            .filter_map(|(index, site)| {
                if let Some(budget) = budget {
                    // Compute cumulative credit from the index to avoid drift;
                    // the last site reaches the exact budget, even at integers.
                    let credit = if index + 1 == site_count {
                        budget
                    } else {
                        budget * ((index + 1) as f64 / site_count as f64)
                    };
                    if credit < next_whole {
                        return None;
                    }
                    next_whole += 1.0;
                }
                Some((site, size))
            });
        let remainder = budget.and_then(|budget| {
            let fraction = budget.fract();
            (fraction > 0.0).then_some((s.head, size * fraction.sqrt()))
        });
        for (site, size) in sites.chain(remainder) {
            let pellet = Self::pellet(
                Point {
                    x: site.x + (self.random.next() * 2.0 - 1.0) * spread,
                    y: site.y + (self.random.next() * 2.0 - 1.0) * spread,
                },
                size,
                Origin::Death,
            );
            // Filter the scattered, quantized position, retaining the ordinary
            // full-body budget and RNG draws. Rejected shares are discarded.
            if food_radius
                .is_some_and(|radius| !pellet.point().distance_lt(Point::default(), radius))
            {
                continue;
            }
            if density.enabled
                && !self.food_cache.has_room(
                    &self.food,
                    &pellet,
                    density.cell_size,
                    density.max_per_cell,
                )
            {
                continue;
            }
            if let Some(events) = &mut self.food_spawn_events {
                events.push(FoodSpawnEvent {
                    source: Some(index),
                    food: pellet,
                });
            }
            self.food_cache.push(&pellet);
            self.food.push(pellet);
            count += 1;
        }
        if outside_step {
            self.food_cache.end();
        }
        count
    }
}

/// Working memory kept between steps, so that small worlds do not pay for an
/// allocation per buffer per tick. Buffers are reset or overwritten before use; no value is trusted across steps.
/// Cloned and restored worlds start empty.
#[derive(Default)]
struct StepBuffers {
    alive: Vec<usize>,
    previous: Vec<Point>,
    last_tips: Vec<Point>,
    /// Ascending slots killed by the outer wall or the pit during movement.
    boundary_deaths: Vec<(usize, DeathReason)>,
    bodies: Vec<Option<Bounds>>,
    heads: Vec<(Point, (f64, f64), f64)>,
    order: Vec<usize>,
    candidates: Vec<u32>,
    dead: Vec<(usize, Option<usize>, DeathReason)>,
    fallen: Vec<bool>,
    eaters: Vec<(usize, Point, f64, Bounds)>,
    hits: Vec<(usize, usize)>,
    /// Ascending original indices pruned by a moving outer wall or pit edge.
    retained_food: Vec<usize>,
    prey: PreyBuffers,
}
#[derive(Default)]
struct PreyBuffers {
    /// Reused for one prey at a time, only on ticks with multiple motion steps.
    path: Vec<Point>,
    live_ids: HashSet<u64>,
    circles: Vec<Option<(Point, f64, Point)>>,
    slots: Vec<usize>,
    sweeps: Vec<Bounds>,
    candidates: Vec<u32>,
}
impl Clone for StepBuffers {
    fn clone(&self) -> Self {
        Self::default()
    }
}
impl std::fmt::Debug for StepBuffers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StepBuffers").finish_non_exhaustive()
    }
}

/// Largest per-tick count of a fixed-period event that is an exact f64 integer
/// and fits usize.
fn max_tick_count() -> f64 {
    (2f64)
        .powi(f64::MANTISSA_DIGITS as i32)
        .min(usize::MAX as f64)
}

/// Boost drops due in `elapsed` seconds and the clock left after them. The dt
/// bound keeps the count below `max_tick_count`; the corrections make the
/// rest lie in `0..BOOST_DROP_SECONDS` despite rounding.
fn drops_in(elapsed: f64) -> (usize, f64) {
    let mut drops = (elapsed / BOOST_DROP_SECONDS).floor();
    let mut rest = elapsed - drops * BOOST_DROP_SECONDS;
    if rest < 0.0 && drops > 0.0 {
        drops -= 1.0;
        rest += BOOST_DROP_SECONDS;
    } else if rest >= BOOST_DROP_SECONDS {
        drops += 1.0;
        rest -= BOOST_DROP_SECONDS;
    }
    (drops as usize, rest.max(0.0))
}

/// Add `gain` to `s`'s mass, or record why not when the sum overflows f64.
/// Validation bounds one catch, not the mass that catches accumulate.
fn gain_mass(s: &mut Snake, slot: usize, gain: f64, item: &str, failure: &mut Option<SimError>) {
    let fam = s.fam + gain;
    if fam.is_finite() {
        s.fam = fam;
        Simulator::update_mass(s);
    } else if failure.is_none() {
        *failure = Some(SimError::new(ErrorKind::NumericOverflow, format!(
            "Snake {}'s mass would overflow f64 after eating {item} worth {gain:e}; the tick \
             completed without that gain and the world cannot advance further",
            s.id
        )).at_slot(slot).at_field("fam"));
    }
}

fn admit_ambient_food(
    food: &Food,
    config: &AmbientFoodConfig,
    cache: &mut FoodCache,
    current: &[Food],
) -> bool {
    !config.enabled || cache.has_room(current, food, config.cell_size, config.max_per_cell)
}

/// Whether `segment_distance(origin, a, b) > radius` follows from both ends lying
/// beyond `radius` on one side of an axis. The computed closest point stays within
/// a few ulps of the segment's bounding box and `hypot` is at least its larger
/// coordinate, so the margin covers rounding. NaN comparisons never skip.
fn sweep_misses(a: Point, b: Point, radius: f64) -> bool {
    let limit = radius + 1e-9 * (radius + a.x.abs() + a.y.abs() + b.x.abs() + b.y.abs());
    (a.x > limit && b.x > limit)
        || (a.x < -limit && b.x < -limit)
        || (a.y > limit && b.y > limit)
        || (a.y < -limit && b.y < -limit)
}

/// Remove strictly ascending original food indices, preserving survivor order.
fn compact_food(food: &mut Vec<Food>, removed: impl Iterator<Item = usize>) {
    let len = food.len();
    let (mut read, mut write) = (0, 0);
    for end in removed.chain(std::iter::once(len)) {
        debug_assert!(read <= end && end <= len);
        let kept = end - read;
        let gap = read - write;
        if gap != 0 && kept != 0 {
            // Food is Copy and removed values are discarded. Copy only the
            // survivors; overlap is safe and the unread suffix stays intact.
            food.copy_within(read..end, write);
        }
        write += kept;
        read = end + 1;
    }
    food.truncate(write);
}

/// Query occupied eater cells, then restore the original reverse pellet order.
fn indexed_hits<const PROFILE: bool>(
    grid: &EaterGrid,
    pellets: &PelletIndex,
    snakes: &[Snake],
    eaters: &[(usize, Point, f64, Bounds)],
    food: &[Food],
    hits: &mut Vec<(usize, usize)>,
    profile: &mut StepProfile,
) {
    // Keep the original single-index iterator in ordinary ticks. Dispatch
    // once per pickup phase, so a dormant migration adds no per-pellet union
    // iterator work. Both paths use the same exact tests and final ordering.
    if pellets.rebuilding() {
        indexed_hits_by::<PROFILE, _>(
            grid,
            |cell| pellets.food(cell),
            snakes,
            eaters,
            food,
            hits,
            profile,
        );
    } else {
        indexed_hits_by::<PROFILE, _>(
            grid,
            |cell| pellets.own_food(cell),
            snakes,
            eaters,
            food,
            hits,
            profile,
        );
    }
}

fn indexed_hits_by<const PROFILE: bool, I: Iterator<Item = usize>>(
    grid: &EaterGrid,
    food_in_cell: impl Fn(u32) -> I,
    snakes: &[Snake],
    eaters: &[(usize, Point, f64, Bounds)],
    food: &[Food],
    hits: &mut Vec<(usize, usize)>,
    profile: &mut StepProfile,
) {
    // Unique cells and one handle per pellet give at most one hit per pellet.
    // Reuse the exact tests and eater order; sorting restores the original
    // reverse-food event/mass/removal order before anything is applied.
    for (cell, candidates) in grid.occupied() {
        for findex in food_in_cell(cell) {
            if let Some(eater) = find_eater::<PROFILE>(
                snakes,
                food[findex].point(),
                candidates.clone().map(|e| eaters[e]),
                profile,
            ) {
                hits.push((findex, eater));
            }
        }
    }
    hits.sort_unstable_by_key(|hit| std::cmp::Reverse(hit.0));
}

/// Visit food in reverse, recording each pellet's eater.
fn dense_hits<const PROFILE: bool>(
    grid: &EaterGrid,
    snakes: &[Snake],
    eaters: &[(usize, Point, f64, Bounds)],
    food: &[Food],
    hits: &mut Vec<(usize, usize)>,
    profile: &mut StepProfile,
) {
    for (findex, f) in food.iter().enumerate().rev() {
        let p = f.point();
        if let Some(candidates) = grid.candidates(p) {
            let eater = find_eater::<PROFILE>(snakes, p, candidates.map(|e| eaters[e]), profile);
            if let Some(i) = eater {
                hits.push((findex, i));
            }
        }
    }
}

fn find_eater<const PROFILE: bool>(
    snakes: &[Snake],
    p: Point,
    eaters: impl Iterator<Item = (usize, Point, f64, Bounds)>,
    profile: &mut StepProfile,
) -> Option<usize> {
    let mut eater = None;
    let mut best = f64::INFINITY;
    for (i, center, radius, bounds) in eaters {
        if PROFILE {
            profile.food_pairs += 1;
        }
        if !bounds.near(Bounds::between(p, p), radius + 1e-9) {
            continue;
        }
        let s = &snakes[i];
        if PROFILE {
            profile.pickup_tests += 1;
        }
        if p.distance(center) > radius {
            continue;
        }
        let d = p.distance(s.head);
        if d < best || (d == best && eater.is_some_and(|k: usize| s.id < snakes[k].id)) {
            eater = Some(i);
            best = d;
        }
    }
    eater
}

#[cfg(test)]
mod food_pickup_order_tests {
    use super::*;
    use crate::RadiusUpdate;

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "shared pickup geometry must cover every living slot")]
    fn partial_food_geometry_is_rejected() {
        let mut config = Config::sandbox();
        config.food_count = 0;
        config.prey.count = 1;
        let mut world = Simulator::new(WorldOptions {
            seed: 47,
            config,
            population: InitialPopulation::Placed {
                ids: vec!["first".into(), "missing".into()],
                placement: Placement::TestRing,
            },
        })
        .unwrap();
        let previous: Vec<_> = world.snakes.iter().map(|s| s.head).collect();
        let heads: Vec<_> = world
            .snakes
            .iter()
            .map(|s| {
                (
                    head_tip(s.head, s.heading, s.segments),
                    (s.heading.cos(), s.heading.sin()),
                    scale(s.segments),
                )
            })
            .collect();
        let s = &world.snakes[0];
        let (center, radius) = pickup_circle(s.head, s.heading, s.speed, s.segments);
        world.advance_prey(
            &previous,
            &heads,
            &[(0, center, radius, Bounds::between(center, center))],
            &mut PreyBuffers::default(),
        );
    }

    #[test]
    fn shared_prey_geometry_matches_fresh_circles_after_food_gain() {
        for dt in [LOOP_SECONDS, LOOP_SECONDS * 2.0, LOOP_SECONDS * 4.0] {
            let mut config = Config::sandbox();
            config.radius = 4096.;
            config.dt = dt;
            config.food_count = 0;
            config.ambient_replenish_per_second = 0.;
            config.prey.count = 24;
            let mut world = Simulator::new(WorldOptions {
                seed: 47,
                config,
                population: InitialPopulation::Placed {
                    ids: (0..40).map(|i| i.to_string()).collect(),
                    placement: Placement::TestRing,
                },
            })
            .unwrap();
            world.record_events(checkpoint::EventRecording {
                pickups: true,
                food_births: true,
            });
            for i in (2..40).step_by(7) {
                world.retire(i).unwrap();
            }
            let previous: Vec<_> = world
                .snakes
                .iter()
                .map(|s| Point {
                    x: s.head.x - 3.,
                    y: s.head.y + 4.,
                })
                .collect();
            let heads: Vec<_> = world
                .snakes
                .iter()
                .map(|s| {
                    (
                        head_tip(s.head, s.heading, s.segments),
                        (s.heading.cos(), s.heading.sin()),
                        scale(s.segments),
                    )
                })
                .collect();
            let food_circles: Vec<_> = world
                .snakes
                .iter()
                .enumerate()
                .filter(|(_, s)| s.alive)
                .map(|(i, s)| {
                    let (center, radius) = pickup_circle(s.head, s.heading, s.speed, s.segments);
                    (i, center, radius, Bounds::between(center, center))
                })
                .collect();
            world.prey[0].position = food_circles[0].1;
            // Food can leave several pending anchors' worth of fam. It does not
            // advance those anchors or alter the circle's integer segment input.
            for (i, s) in world.snakes.iter_mut().enumerate() {
                if s.alive {
                    s.fam += [0., 0.999, 1.001, 20.][i % 4];
                    Simulator::update_mass(s);
                }
            }
            let mut reference = world.clone();
            world.advance_prey(
                &previous,
                &heads,
                &food_circles,
                &mut PreyBuffers::default(),
            );
            reference.advance_prey(&previous, &heads, &[], &mut PreyBuffers::default());
            assert_eq!(world.pickup_events(), reference.pickup_events());
            assert!(
                !world.pickup_events().unwrap().is_empty(),
                "exercise consumption"
            );
            assert_eq!(
                serde_json::to_vec(&world.checkpoint().unwrap()).unwrap(),
                serde_json::to_vec(&reference.checkpoint().unwrap()).unwrap(),
                "dt {dt}"
            );
        }
    }

    #[test]
    fn ordered_food_compaction_matches_stable_retention_bit_for_bit() {
        let foods = |len| {
            (0..len)
                .map(|i| Food {
                    x: [0.0, -0.0, 1e300, f64::NAN, f64::INFINITY][i % 5],
                    y: i as f64,
                    value: i as f64 + 1.0,
                    size: (i % 2 == 0).then_some(i as f64 * 0.25),
                    origin: [None, Some(Origin::Ambient), Some(Origin::Boost)][i % 3],
                })
                .collect::<Vec<_>>()
        };
        let bits = |food: &[Food]| {
            food.iter()
                .map(|f| {
                    (
                        f.x.to_bits(),
                        f.y.to_bits(),
                        f.value.to_bits(),
                        f.size.map(f64::to_bits),
                        f.origin,
                    )
                })
                .collect::<Vec<_>>()
        };
        for len in 0..=12 {
            for mask in 0..1usize << len {
                let mut actual = foods(len);
                let expected: Vec<_> = actual
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) == 0)
                    .map(|(_, f)| *f)
                    .collect();
                compact_food(&mut actual, (0..len).filter(|i| mask & (1 << i) != 0));
                assert_eq!(bits(&actual), bits(&expected), "len {len}, mask {mask}");
            }
        }
        // Cover long survivor ranges and large gaps with tiny survivor ranges.
        for removed in [
            vec![],
            vec![0],
            vec![1023],
            vec![1, 17, 127, 511, 997],
            (0..1024).step_by(2).collect(),
            (0..1024).filter(|i| i % 19 != 0).collect(),
            (0..1024).collect(),
        ] {
            let mut actual = foods(1024);
            let expected: Vec<_> = actual
                .iter()
                .enumerate()
                .filter(|(i, _)| !removed.contains(i))
                .map(|(_, f)| *f)
                .collect();
            compact_food(&mut actual, removed.iter().copied());
            assert_eq!(bits(&actual), bits(&expected));
        }
    }

    #[test]
    fn public_food_edits_reset_reusable_mirror_verification() {
        let mut world = Simulator::new(WorldOptions {
            seed: 7,
            config: Config {
                food_count: 4,
                ambient_replenish_per_second: 0.0,
                ..Config::sandbox()
            },
            population: InitialPopulation::Empty,
        })
        .unwrap();
        world.step(&[]).unwrap();
        let mut food = world.food().to_vec();
        for f in &mut food {
            f.origin = Some(Origin::Boost);
        }
        // Same length defeats a length-only check; the API must reset the cache.
        world.replace_food(food).unwrap();
        world.food_cache.begin_mirrored(&world.food);
        assert_eq!(world.food_cache.ambient(&world.food), 0);
        world.food_cache.end();
        let mut added = world.remove_food(0).unwrap();
        added.origin = Some(Origin::Ambient);
        added.x = 1234.0;
        world.insert_food(added).unwrap();
        world.food_cache.begin_mirrored(&world.food);
        assert_eq!(world.food_cache.ambient(&world.food), 1);
        world.food_cache.end();
        let mut restored = Simulator::from_checkpoint(
            &serde_json::to_vec(&world.checkpoint().unwrap()).unwrap(),
            checkpoint::RestoreOptions::default(),
        )
        .unwrap();
        restored.food_cache.begin_mirrored(&restored.food);
        assert_eq!(restored.food_cache.ambient(&restored.food), 1);
    }

    #[test]
    fn radius_retention_preserves_warm_handles_and_matches_recounts() {
        for pit_only in [false, true] {
            let mut config = if pit_only {
                Config::battledome()
            } else {
                Config::official(32)
            };
            config.radius = 3584.;
            config.food_count = 0;
            config.ambient_replenish_per_second = 0.;
            config.prey.count = 0;
            let radii = [3500., 3000., 2500., 3600., 2200.];
            config.radius_updates = radii
                .iter()
                .enumerate()
                .map(|(i, &radius)| RadiusUpdate {
                    time: (i + 1) as f64 * config.dt,
                    radius,
                })
                .collect();
            let mut world = Simulator::new(WorldOptions {
                seed: 47,
                config,
                population: InitialPopulation::Empty,
            })
            .unwrap();
            let mut food = Vec::new();
            for radius in radii {
                let edge = if pit_only {
                    1784. * (radius / 3584.)
                } else {
                    radius
                };
                for distance in [0., edge.next_down(), edge, edge.next_up(), 3700.] {
                    for sign in [-1., 1.] {
                        food.push(Food {
                            x: sign * distance,
                            y: 0.,
                            value: food.len() as f64 + 1.,
                            size: Some(1.),
                            origin: [
                                None,
                                Some(Origin::Ambient),
                                Some(Origin::Boost),
                                Some(Origin::Death),
                            ][food.len() % 4],
                        });
                    }
                }
            }
            let probes = food.clone();
            world.replace_food(food).unwrap();
            world.food_cache.begin_mirrored(&world.food);
            world.food_cache.pellets(&world.food).unwrap();
            for size in [40., 75., 128.] {
                world
                    .food_cache
                    .has_room(&world.food, &probes[0], size, usize::MAX);
            }
            world.food_cache.end();
            for tick in 0..radii.len() {
                let before = world.food.clone();
                let previous = world.radius;
                let (matched, handles, free, grids) = world.food_cache.retention_state();
                assert!(matched);
                assert_eq!(grids, 3);
                let mut cold = world.clone();
                cold.food_cache = FoodCache::default();
                world.update_radius((tick + 1) as f64 * world.config.dt);
                cold.update_radius((tick + 1) as f64 * cold.config.dt);
                let mut expected = before.clone();
                if (pit_only && world.radius != previous) || (!pit_only && world.radius < previous)
                {
                    let radius = world.food_radius();
                    expected.retain(|f| f.point().distance_lt(Point::default(), radius));
                }
                assert_eq!(
                    serde_json::to_vec(&world.food).unwrap(),
                    serde_json::to_vec(&expected).unwrap()
                );
                assert_eq!(world.food, cold.food);
                let removed = before.len() - expected.len();
                assert_eq!(
                    world.food_cache.retention_state(),
                    (true, handles, free + removed, 3)
                );
                world.food_cache.begin_mirrored(&world.food);
                world.food_cache.pellets(&world.food).unwrap();
                assert_eq!(
                    world.food_cache.ambient(&world.food),
                    world
                        .food
                        .iter()
                        .filter(|f| f.origin == Some(Origin::Ambient))
                        .count()
                );
                for size in [40., 75., 128.] {
                    for probe in &probes {
                        let cell = crate::food_grid::food_cell(probe.x, probe.y, size);
                        let count = world
                            .food
                            .iter()
                            .filter(|f| crate::food_grid::food_cell(f.x, f.y, size) == cell)
                            .count();
                        for max in [0, count, count + 1] {
                            assert_eq!(
                                world.food_cache.has_room(&world.food, probe, size, max),
                                count < max
                            );
                        }
                    }
                }
                // Verification, occupancy grids and freed handles survive the
                // next access; rebuilding would lose the old free-list holes.
                assert_eq!(
                    world.food_cache.retention_state(),
                    (true, handles, free + removed, 3)
                );
                world.food_cache.end();
            }
            let (_, handles, free, _) = world.food_cache.retention_state();
            let remaining = world.food.len();
            world.retain_food(0.);
            assert!(world.food.is_empty());
            world.food_cache.begin_mirrored(&world.food);
            assert_eq!(world.food_cache.ambient(&world.food), 0);
            world.food_cache.pellets(&world.food).unwrap();
            assert_eq!(
                world.food_cache.retention_state(),
                (true, handles, free + remaining, 3)
            );
            world.food_cache.end();
        }
    }

    #[test]
    fn reused_food_cache_matches_fresh_caches_through_radius_and_pit_retention() {
        for pit_only in [false, true] {
            let mut warm = Simulator::new(WorldOptions {
                seed: 47,
                config: Config {
                    dynamic_radius: true,
                    food_count: 500,
                    ambient_replenish_per_second: 80.0,
                    food_only_inside_pit: pit_only,
                    pit: pit_only.then_some(PitConfig { radius: 900.0 }),
                    radius_updates: vec![
                        RadiusUpdate {
                            time: 0.021,
                            radius: 800.0,
                        },
                        RadiusUpdate {
                            time: 0.084,
                            radius: 600.0,
                        },
                        RadiusUpdate {
                            time: 0.140,
                            radius: 1000.0,
                        },
                    ],
                    ..Config::sandbox()
                },
                population: InitialPopulation::Placed {
                    ids: (0..8).map(|i| format!("s{i}")).collect(),
                    placement: Placement::TestRing,
                },
            })
            .unwrap();
            warm.record_events(checkpoint::EventRecording {
                pickups: true,
                food_births: true,
            });
            let mut fresh = warm.clone();
            for tick in 0..96 {
                if tick == 20 {
                    let mut food = warm.food().to_vec();
                    food[0].x = 0.0;
                    food[0].y = 0.0;
                    for world in [&mut warm, &mut fresh] {
                        world.replace_food(food.clone()).unwrap();
                    }
                }
                let actions: Vec<_> = warm
                    .snakes
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        s.alive.then_some(Action {
                            heading: i as f64 * 2.4 + tick as f64 * 0.003,
                            boost: (tick / 12 + i) % 4 == 0,
                        })
                    })
                    .collect();
                fresh.food_cache = FoodCache::default();
                warm.step(&actions).unwrap();
                fresh.step(&actions).unwrap();
                assert_eq!(
                    warm.pickup_events(),
                    fresh.pickup_events(),
                    "tick {tick}, pit {pit_only}"
                );
                assert_eq!(
                    warm.food_spawn_events(),
                    fresh.food_spawn_events(),
                    "tick {tick}, pit {pit_only}"
                );
                assert_eq!(warm.deaths(), fresh.deaths(), "tick {tick}, pit {pit_only}");
                assert_eq!(
                    serde_json::to_value(warm.checkpoint().unwrap()).unwrap(),
                    serde_json::to_value(fresh.checkpoint().unwrap()).unwrap(),
                    "tick {tick}, pit {pit_only}"
                );
            }
        }
    }

    #[test]
    fn dense_hits_keep_reverse_pellet_order_nearest_head_and_lexical_ties() {
        let mut world = Simulator::new(WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: InitialPopulation::Placed {
                ids: vec!["z".into(), "a".into()],
                placement: Placement::TestRing,
            },
        })
        .unwrap();
        world.snakes[0].head = Point { x: -10.0, y: 0.0 };
        world.snakes[1].head = Point { x: 10.0, y: 0.0 };
        // Synthetic overlapping pickup circles isolate the existing selection
        // contract: nearest head, lexical ID on ties, reverse pellet order.
        let center = Point { x: 0.0, y: 0.0 };
        let eaters: Vec<_> = (0..2)
            .map(|i| (i, center, 36.0, Bounds::between(center, center)))
            .collect();
        let food: Vec<_> = [0.0, 100.0, -1.0, 1.0]
            .into_iter()
            .map(|x| Food {
                x,
                y: 0.0,
                value: 1.0,
                size: None,
                origin: None,
            })
            .collect();
        let mut grid = EaterGrid::default();
        assert!(grid.build(eaters.iter().map(|&(_, c, r, _)| (c, r)), 4096));
        let mut profile = StepProfile::default();
        let mut hits = Vec::new();
        dense_hits::<false>(
            &grid,
            &world.snakes,
            &eaters,
            &food,
            &mut hits,
            &mut profile,
        );
        assert_eq!(hits, [(3, 1), (2, 0), (0, 1)]);
        let mut cache = FoodCache::default();
        let mut indexed = Vec::new();
        indexed_hits::<false>(
            &grid,
            cache.pellets(&food).unwrap(),
            &world.snakes,
            &eaters,
            &food,
            &mut indexed,
            &mut profile,
        );
        assert_eq!(indexed, hits);
        let scanned: Vec<_> = food
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(findex, f)| {
                find_eater::<false>(
                    &world.snakes,
                    f.point(),
                    eaters.iter().copied(),
                    &mut profile,
                )
                .map(|eater| (findex, eater))
            })
            .collect();
        assert_eq!(hits, scanned);
    }
}

#[cfg(test)]
mod death_food_density_tests {
    use super::*;
    use crate::food_grid::{Axis, food_cell};

    fn world(enabled: bool) -> Simulator {
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config {
                food_count: 0,
                ambient_replenish_per_second: 0.0,
                death_food: DeathFoodConfig {
                    enabled,
                    ..DeathFoodConfig::default()
                },
                ..Config::sandbox()
            },
            ..Default::default()
        })
        .unwrap();
        add_corpse(&mut world, "a", Point { x: 37.5, y: 37.5 });
        world
    }

    fn add_corpse(world: &mut Simulator, id: &str, point: Point) {
        let index = world.push_ring(id.into(), 0, 1).unwrap();
        let snake = &mut world.snakes[index];
        snake.head = point;
        snake.body = vec![point; 30];
        snake.segments = 30;
        Simulator::update_mass(snake);
    }

    #[test]
    fn birth_provenance_reports_admitted_death_food_only() {
        let mut world = world(true);
        world.food_spawn_events = Some(Vec::new());
        let count = world.death_food(0);
        let events = world.food_spawn_events.as_ref().unwrap();
        assert_eq!(events.len(), count);
        assert!(
            events
                .iter()
                .all(|e| e.source == Some(0) && e.food.origin == Some(Origin::Death))
        );
        assert_eq!(world.death_food(0), 0);
        assert_eq!(world.food_spawn_events.as_ref().unwrap().len(), count);
    }

    #[test]
    fn floor_budget_spans_the_body_and_preserves_nutrition_and_random_draws() {
        let floor = MassFloor::for_score(400).unwrap();
        for (min_score, segments, fam, full_sites) in [
            // Full pellets are 7 or 8 sites apart, from tail toward head.
            (400, 30, 0.5, vec![7, 14, 21, 28, 35, 42, 49, 56]),
            // Both the last whole crossing and a remainder fall at the head.
            (400, 30, 0.12, vec![7, 15, 22, 30, 38, 45, 53, 60]),
            // Sub-pellet growth still returns its nutrition near the head.
            (400, floor.segments, floor.fam + 0.01, vec![]),
            (400, floor.segments, floor.fam, vec![]),
            // Exactly one pellet: the final crossing must not round away.
            (10, 2, 0.5, vec![4]),
        ] {
            let mut world = Simulator::new(crate::WorldOptions {
                seed: 7,
                config: Config {
                    radius: 5000.0,
                    food_count: 0,
                    ambient_replenish_per_second: 0.0,
                    spawn_radius: Some(2684.0),
                    min_score: Some(min_score),
                    ..Config::sandbox()
                },
                ..Default::default()
            })
            .unwrap();
            world.config.death_food.enabled = false;
            world.push_ring("victim".into(), 0, 1).unwrap();
            let snake = &mut world.snakes[0];
            snake.head = Point { x: -75.0, y: 0.0 };
            snake.body = (0..segments)
                .map(|i| Point {
                    x: i as f64 * 75.0,
                    y: 0.0,
                })
                .collect();
            snake.segments = segments;
            snake.fam = fam;
            Simulator::update_mass(snake);
            let sites: Vec<_> = death_food_sites(&snake.body, snake.head).rev().collect();
            let budget = sites.len() as f64
                * (world.min_mass.unwrap().excess(segments, fam) / (segments as f64 + fam));
            assert_eq!(full_sites.len(), budget.floor() as usize);
            let size = death_food_size(segments);
            let spread = 15.0 * scale(segments);
            let mut random = world.random.clone();
            let mut expected = Vec::new();
            for (index, share) in full_sites
                .into_iter()
                .map(|index| (index, 1.0))
                .chain((budget.fract() > 0.0).then_some((sites.len() - 1, budget.fract())))
            {
                let site = sites[index];
                expected.push(Simulator::pellet(
                    Point {
                        x: site.x + (random.next() * 2.0 - 1.0) * spread,
                        y: site.y + (random.next() * 2.0 - 1.0) * spread,
                    },
                    size * share.sqrt(),
                    Origin::Death,
                ));
            }
            assert_eq!(world.death_food(0), budget.ceil() as usize);
            assert_eq!(world.food, expected);
            let nutrition: f64 = world.food.iter().map(|food| food.value).sum();
            assert!((nutrition - budget * crate::physics::nutrition(size, 2.0)).abs() < 1e-12);
            assert_eq!(world.random.next(), random.next());
        }
    }

    fn existing(x: f64, y: f64, origin: Origin) -> Food {
        Food {
            x,
            y,
            value: 1.0,
            size: Some(5.0),
            origin: Some(origin),
        }
    }

    #[test]
    fn compact_death_stops_at_twenty_two_total_pellets_in_one_cell() {
        let mut world = world(true);
        assert_eq!(world.death_food(0), 22);
        assert_eq!(world.food.len(), 22);
        assert!(
            world
                .food
                .iter()
                .all(|f| food_cell(f.x, f.y, 75.0) == (Axis::Index(0), Axis::Index(0)))
        );
    }

    #[test]
    fn prior_food_and_sequential_deaths_share_the_same_limit() {
        let mut world = world(true);
        for i in 0..10 {
            let origin = match i % 3 {
                0 => Origin::Ambient,
                1 => Origin::Boost,
                _ => Origin::Death,
            };
            world.food.push(existing(20.0, 20.0, origin));
        }
        assert_eq!(world.death_food(0), 12);
        add_corpse(&mut world, "b", Point { x: 37.5, y: 37.5 });
        assert_eq!(world.death_food(1), 0);
        assert_eq!(world.food.len(), 22);
        // Food is publicly mutable; occupancy must be rebuilt for a later death.
        world.food.pop();
        assert_eq!(world.death_food(1), 1);
    }

    #[test]
    fn disabled_cap_retains_count_and_rejections_do_not_shift_random_stream() {
        let mut capped = world(true);
        let mut uncapped = world(false);
        assert_eq!(capped.death_food(0), 22);
        assert_eq!(uncapped.death_food(0), 61);
        assert_eq!(capped.new_food(), uncapped.new_food());
    }

    #[test]
    fn negative_coordinates_use_floor_cells_instead_of_truncation() {
        let mut world = world(true);
        let negative = Point { x: -37.5, y: -37.5 };
        world.snakes[0].head = negative;
        world.snakes[0].body.fill(negative);
        world.food = vec![existing(2.0, 2.0, Origin::Ambient); 22];
        assert_eq!(
            food_cell(-37.5, -37.5, 75.0),
            (Axis::Index(-1), Axis::Index(-1))
        );
        assert_eq!(world.death_food(0), 22);
        assert_eq!(world.food.len(), 44);
    }
}

#[cfg(test)]
mod prey_sweep_tests {
    use super::*;

    #[test]
    fn rejected_sweeps_are_beyond_the_pickup_radius() {
        let mut state = 0x1234_5678_9abc_u64;
        let mut unit = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut rejected = 0;
        for i in 0..400_000 {
            let scale = [1.0, 50.0, 1e4, 1e9][i % 4];
            let radius = 36.0 + unit() * 95.0;
            let mut a = Point {
                x: (unit() - 0.5) * scale,
                y: (unit() - 0.5) * scale,
            };
            let b = Point {
                x: a.x + (unit() - 0.5) * scale,
                y: a.y + (unit() - 0.5) * scale,
            };
            if i % 1000 == 7 {
                a.x = [f64::NAN, f64::INFINITY, -f64::INFINITY][i % 3];
            }
            if sweep_misses(a, b, radius) {
                rejected += 1;
                let distance = segment_distance(Point::default(), a, b);
                assert!(
                    distance > radius || distance.is_nan(),
                    "{a:?} {b:?} {radius}"
                );
            }
        }
        assert!(rejected > 100_000);
    }
}

#[cfg(test)]
mod prey_index_tests {
    use super::*;

    #[test]
    fn sweeps_outside_the_index_reach_always_miss() {
        let mut state = 0xdead_beef_u64;
        let mut unit = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut skipped = 0;
        for i in 0..400_000 {
            let magnitude = [1e3, 1e5, 1e7][i % 3] * (2.0 * unit() - 1.0);
            let previous = Point {
                x: magnitude,
                y: -magnitude * unit(),
            };
            let lead = Point {
                x: (unit() - 0.5) * 400.0,
                y: (unit() - 0.5) * 400.0,
            };
            let from = Point {
                x: previous.x + lead.x,
                y: previous.y + lead.y,
            };
            let center = Point {
                x: from.x + (unit() - 0.5) * 20.0,
                y: from.y + (unit() - 0.5) * 20.0,
            };
            let radius = 36.0 + unit() * 95.0;
            // Prey paths straddle the index reach around the circle path.
            let offset = (radius + 1.0) * (0.9 + unit() * 0.3);
            let angle = unit() * std::f64::consts::TAU;
            let old = Point {
                x: from.x + offset * angle.cos(),
                y: from.y + offset * angle.sin(),
            };
            let position = Point {
                x: old.x + (unit() - 0.5) * 4.0,
                y: old.y + (unit() - 0.5) * 4.0,
            };
            if Bounds::between(from, center).near(Bounds::between(old, position), radius + 1.0) {
                continue;
            }
            skipped += 1;
            let a = Point {
                x: old.x - previous.x - lead.x,
                y: old.y - previous.y - lead.y,
            };
            let b = Point {
                x: position.x - center.x,
                y: position.y - center.y,
            };
            assert!(sweep_misses(a, b, radius), "{a:?} {b:?} {radius}");
        }
        assert!(skipped > 10_000, "{skipped}");
    }

    /// Indexed sums near the origin can come from huge operands that cancel
    /// differently in the exact test, which then still eats every prey.
    #[test]
    fn huge_cancelling_operands_skip_the_index() {
        let mut config = Config::sandbox();
        config.radius = 1e30;
        config.dt = 1e-300;
        config.food_count = 0;
        config.ambient_replenish_per_second = 0.0;
        config.kiosk.enabled = false;
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 123,
            config,
            ..Default::default()
        })
        .unwrap();
        world.push_ring("0".into(), 0, 1).unwrap();
        let head = Point {
            x: -(2f64.powi(70)),
            y: 0.0,
        };
        let s = &mut world.snakes[0];
        s.head = head;
        s.heading = 0.0;
        s.body = vec![head; s.segments];
        // The pickup lead almost cancels the head, leaving the circle near the prey.
        s.speed = f64::from_bits((-head.x / (0.032 * 7.55)).to_bits() + 1);
        world.speeds[0] = SpeedState::idle(world.snakes[0].speed);
        world.prey = (0..512)
            .map(|id| Prey {
                id,
                position: Point { x: 2048.0, y: 0.0 },
                heading: 0.0,
                target_heading: 0.0,
                speed: 0.0,
                size: 6.0,
            })
            .collect();
        world.step(&[Some(Action::default())]).unwrap();
        assert!(world.prey.is_empty(), "{} prey left", world.prey.len());
    }
}

#[cfg(test)]
mod pit_sweep_tests {
    use super::*;

    #[test]
    fn a_one_ulp_step_inside_counts_although_rescaling_collapses_it() {
        // Near the reviewer's repro, find a one-ulp step whose ends rescale to
        // the same point while its end is measurably closer. `hypot` rounding
        // differs across platforms, so search rather than fix the points.
        let origin = Point::default();
        let mut x: f64 = -7190.000000000015;
        let mut tested = 0;
        for _ in 0..1000 {
            let (a, b) = (
                Point { x, y: 10_000.0 },
                Point {
                    x: x.next_up(),
                    y: 10_000.0,
                },
            );
            x = x.next_down();
            let (far, near) = (a.distance(origin), b.distance(origin));
            if a.x / 10_000.0 != b.x / 10_000.0 || near >= far {
                continue;
            }
            // A radius between the two: the start is outside, the end inside.
            let radius = near.next_up();
            assert!(far >= radius);
            assert!(reaches_center(a, b, radius), "{a:?} -> {b:?}");
            tested += 1;
        }
        assert!(tested > 0);
    }
}

#[cfg(test)]
mod slot_tests {
    use super::*;

    #[test]
    fn placed_population_keeps_order_and_unique_slots() {
        let mut checked = Simulator::new(crate::WorldOptions {
            seed: 3,
            config: Config::sandbox(),
            ..Default::default()
        })
        .unwrap();
        for slot in 0..5 {
            checked.push_ring(slot.to_string(), slot, 5).unwrap();
        }
        let mut generated = Simulator::new(crate::WorldOptions {
            seed: 3,
            config: Config::sandbox(),
            population: InitialPopulation::Placed {
                ids: (0..5).map(|s| s.to_string()).collect(),
                placement: Placement::default(),
            },
        })
        .unwrap();
        assert_eq!(
            serde_json::to_value(generated.checkpoint().unwrap()).unwrap(),
            serde_json::to_value(checked.checkpoint().unwrap()).unwrap()
        );
        // Caller-supplied IDs are still checked.
        assert_eq!(
            generated.append_dormant("3".into()).unwrap_err().kind,
            ErrorKind::DuplicateId
        );
        // The largest protocol-19 ring is linear work: the old per-slot scan
        // made 2.1 billion ID comparisons here.
        let ring = Simulator::new(crate::WorldOptions {
            seed: 3,
            config: Config::sandbox(),
            population: InitialPopulation::Placed {
                ids: (0..65_535).map(|s| s.to_string()).collect(),
                placement: Placement::default(),
            },
        })
        .unwrap();
        assert_eq!(ring.snakes.len(), 65_535);
        assert_eq!(ring.snakes[65_534].id, "65534");
    }
}
