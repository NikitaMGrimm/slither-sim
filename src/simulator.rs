use crate::bounds_index::BoundsIndex;
use crate::food_grid::{EaterGrid, FoodCache};
use crate::kiosk::{KioskConfig, KioskState};
use crate::prey::{OUTWARD_SPEED, Prey, PreyConfig, PreyMode};
use crate::profiling::StepProfile;
use crate::{model::*, physics::*};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::consts::{FRAC_PI_2, PI, TAU};
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
    pub fn validate(&self) -> Result<(), String> {
        if !self.cell_size.is_finite() || self.cell_size <= 0.0 || self.max_per_cell == 0 {
            return Err("Invalid death-food density configuration".into());
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
    pub fn validate(&self) -> Result<(), String> {
        if !self.cell_size.is_finite() || self.cell_size <= 0.0 || self.max_per_cell == 0 {
            return Err("Invalid ambient-food density configuration".into());
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
    pub fn validate(&self) -> Result<(), String> {
        if !self.cell_size.is_finite() || self.cell_size <= 0.0 || self.max_per_cell == 0 {
            return Err("Invalid boost-food density configuration".into());
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

// `remote = "Self"` turns the derived impls into inherent functions, which the
// trait impls below wrap to accept `preset`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    remote = "Self",
    default,
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub struct Config {
    /// Boundary radius; the starting radius when `dynamicRadius` is set.
    pub radius: f64,
    /// Enable the population-driven boundary model used by [`Config::official`].
    pub dynamic_radius: bool,
    pub vision: f64,
    /// Global ambient pellet target; boost and death do not count against it.
    pub food_count: usize,
    /// Ambient placement attempts per simulated second while below food_count.
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pit: Option<PitConfig>,
    /// Fixed distance of new lives from the center; `None` keeps the official
    /// radial grid of `spawn::sample_spawn_distance`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spawn_radius: Option<f64>,
    /// Window of `Simulator::kill_score`: each credited kill counts for this
    /// many seconds, across the slot's lives. `None` keeps no kill score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kill_score_seconds: Option<f64>,
    /// Displayed-score floor: new lives start at this score and boosting or the
    /// kiosk penalty never shrink a snake below it. Read when a world is created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_score: Option<f64>,
}
impl Default for Config {
    fn default() -> Self {
        Self::official(1)
    }
}
impl Serialize for Config {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Config::serialize(self, serializer)
    }
}
impl<'de> Deserialize<'de> for Config {
    /// Fields given beside `preset` replace its values; nested objects merge
    /// field by field. Without a preset, omitted fields use `Config::official(1)`.
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
        Config::deserialize(Value::Object(fields)).map_err(D::Error::custom)
    }
}
/// Protocol sector area used by the modeled official ecology below.
const SECTOR_AREA: f64 = 300.0 * 300.0;
/// Official food target and ambient attempts/s for an arena of `radius`:
/// about 7 pellets and 0.1024 attempts/s per sector.
fn official_food(radius: f64) -> (usize, f64) {
    let sectors = PI * radius * radius / SECTOR_AREA;
    ((7.0 * sectors) as usize, 0.1024 * sectors)
}
impl Config {
    /// Best-current official-server approximation, initialized for `snakes`.
    /// Includes provisional models; this is not an identified server preset.
    /// Radius and food use the existing population model. Prey uses an explicit
    /// working estimate of one per ten million square units, with at least one.
    /// Received prey density does not establish a global census; see
    /// docs/configuration.md#official-and-sandbox.
    pub fn official(snakes: usize) -> Self {
        let radius = population_radius(snakes);
        let area = PI * radius * radius;
        let (food_count, ambient_replenish_per_second) = official_food(radius);
        Self {
            radius,
            dynamic_radius: true,
            // A finite receipt approximation: the paired visibility census
            // rejects 500, while 1500 substantially reduces missed snakes/prey.
            // Exact server entry/exit geometry and size scaling remain unknown.
            vision: 1500.0,
            food_count,
            ambient_replenish_per_second,
            dt: LOOP_SECONDS,
            food_index: true,
            emit_boost_food: true,
            speed_model: SpeedModel::default(),
            prey: PreyConfig {
                count: (area / 10_000_000.0).round().max(1.0) as usize,
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
            food_count: 350,
            ambient_replenish_per_second: 5.0,
            prey: PreyConfig {
                count: 0,
                ..PreyConfig::default()
            },
            ..Self::official(1)
        }
    }

    /// Battledome: official physics and ecology in a fixed 3584-unit arena,
    /// with a one-way pit of radius 1784, spawns midway between the two, a
    /// 15-minute kill score, a 400-point size floor and no prey. See docs/configuration.md#battledome.
    pub fn battledome() -> Self {
        const RADIUS: f64 = 3584.0;
        const PIT: f64 = 1784.0;
        let (food_count, ambient_replenish_per_second) = official_food(RADIUS);
        Self {
            radius: RADIUS,
            dynamic_radius: false,
            food_count,
            ambient_replenish_per_second,
            prey: PreyConfig {
                count: 0,
                ..PreyConfig::default()
            },
            pit: Some(PitConfig { radius: PIT }),
            spawn_radius: Some((PIT + RADIUS) / 2.0),
            kill_score_seconds: Some(900.0),
            min_score: Some(400.0),
            ..Self::official(1)
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.food_count > 100_000 {
            return Err("foodCount must be at most 100000".into());
        }
        if [self.radius, self.vision, self.dt]
            .iter()
            .any(|n| !n.is_finite() || *n <= 0.0)
        {
            return Err("Settings must be positive finite numbers".into());
        }
        if !self.ambient_replenish_per_second.is_finite()
            || !(0.0..=10_000.0).contains(&self.ambient_replenish_per_second)
        {
            return Err("ambientReplenishPerSecond must be finite and between 0 and 10000".into());
        }
        if self.dt > 0.032 {
            return Err("dt must be at most 32ms".into());
        }
        self.prey.validate()?;
        self.kiosk.validate()?;
        self.death_food.validate()?;
        self.ambient_food.validate()?;
        self.boost_food.validate()?;
        self.validate_mode()?;
        crate::boundary::validate(&self.radius_updates)
    }
    /// Pit, spawn ring and kill-score window: only the limits the rules need.
    fn validate_mode(&self) -> Result<(), String> {
        let pit = self.pit.as_ref().map(|pit| pit.radius);
        if pit.is_some_and(|pit| !(pit > 0.0 && pit < self.radius)) {
            return Err("pit.radius must be positive and less than radius, so the pit lies inside the arena".into());
        }
        if self.spawn_radius.is_some_and(|spawn| {
            !(spawn >= 0.0 && spawn < self.radius && pit.is_none_or(|pit| spawn > pit))
        }) {
            return Err("spawnRadius must lie inside radius and outside pit.radius, where new snakes can live".into());
        }
        if self
            .kill_score_seconds
            .is_some_and(|window| !(window.is_finite() && window > 0.0))
        {
            return Err(
                "killScoreSeconds must be positive and finite, so kills count and then expire"
                    .into(),
            );
        }
        let scores = MassFloor::score_range();
        if self.min_score.is_some_and(|score| !scores.contains(&score)) {
            return Err(format!(
                "minScore must be finite and from a new snake's {} to the {MAX_SEGMENTS}-segment \
                 score {}, so a snake can be built at it",
                scores.start(),
                scores.end()
            ));
        }
        Ok(())
    }
}
/// Constant-density boundary heuristic. Population/radius pairs do not identify
/// the server's controller; measured replay uses `Config::radius_updates` instead.
pub fn population_radius(snakes: usize) -> f64 {
    670.0 * (snakes.max(1) as f64).sqrt()
}
/// Local controller rate limit; later received expansions exceed this value.
/// It must not be applied to measured `radius_updates` replay.
const RADIUS_RATE: f64 = 3.0 / 0.128;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Random(u32);
impl Random {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        f64::from(self.0) / 4294967296.0
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

/// Serializes losslessly (JSON with exact floats), so a running world can move
/// between processes. Per-step event buffers are not saved.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Simulator {
    pub config: Config,
    /// Stable slots: use add(), never remove or reorder snakes during a run.
    pub snakes: Vec<Snake>,
    pub food: Vec<Food>,
    /// Publicly injected prey must have unique IDs; decision phases follow IDs.
    pub prey: Vec<Prey>,
    pub deaths: Vec<DeathEvent>,
    /// Latest step only. None avoids event allocation/copying.
    #[serde(skip)]
    pub pickup_events: Option<Vec<PickupEvent>>,
    /// Latest step only. Disabled by default.
    #[serde(skip)]
    pub food_spawn_events: Option<Vec<FoodSpawnEvent>>,
    pub time: f64,
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
    ambient_clock: f64,
    prey_random: Random,
    next_prey_id: u64,
    prey_clock: f64,
    prey_decision_times: HashMap<u64, f64>,
    prey_outward: HashSet<u64>,
    prey_motion_clock: f64,
    radius_update_index: usize,
    #[serde(skip)]
    food_cache: FoodCache,
    #[serde(skip)]
    eater_grid: EaterGrid,
    #[serde(skip)]
    buffers: StepBuffers,
    /// Batch observation on the rayon pool; see `set_parallel`.
    #[cfg(feature = "parallel")]
    #[serde(skip)]
    parallel: bool,
}
impl Simulator {
    /// Let `observe_all` and `observe_selected` split large batches over the
    /// current rayon thread pool: the global pool, sized by `RAYON_NUM_THREADS`
    /// or the core count, unless called inside `ThreadPool::install`. Results are
    /// identical either way. Off by default, so worlds already run in parallel do
    /// not compete for cores. Like the event buffers, this is not serialized.
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
    pub fn set_ambient_rate(&mut self, rate: f64) -> Result<(), String> {
        if !rate.is_finite() || !(0.0..=10_000.0).contains(&rate) {
            return Err("Ambient rate must be finite and between 0 and 10000".into());
        }
        if rate != self.config.ambient_replenish_per_second {
            let credit = self.ambient_clock * self.config.ambient_replenish_per_second;
            let clock = if rate > 0.0 { credit / rate } else { 0.0 };
            if !clock.is_finite() {
                return Err("Ambient refill credit cannot be represented at this rate".into());
            }
            self.ambient_clock = clock;
            self.config.ambient_replenish_per_second = rate;
        }
        Ok(())
    }
    pub fn new(seed: u32, config: Config) -> Result<Self, String> {
        config.validate()?;
        let min_mass = config.min_score.and_then(MassFloor::for_score);
        let mut world = Self {
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
            eater_grid: EaterGrid::default(),
            buffers: StepBuffers::default(),
            #[cfg(feature = "parallel")]
            parallel: false,
        };
        world.update_radius(0.0);
        world.food.reserve(world.config.food_count);
        world.food_cache.begin();
        // food_count is a target number of attempts at startup. A rejected
        // candidate is not retried, so crowded small worlds can start below it.
        for _ in 0..world.config.food_count {
            let f = world.new_food();
            if admit_ambient_food(
                &f,
                &world.config.ambient_food,
                &mut world.food_cache,
                &world.food,
            ) {
                world.food_cache.push(&f);
                world.food.push(f);
            }
        }
        world.food_cache.end();
        for _ in 0..world.config.prey.count {
            world.spawn_prey();
        }
        Ok(world)
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
    /// Provisional radial profile fitted to received additions. Recipient coverage
    /// and admission censor those counts; this is not a recovered generation law.
    fn new_food(&mut self) -> Food {
        let a = self.random.next() * TAU;
        let r = loop {
            let f = self.random.next().sqrt();
            if self.random.next() * 0.217 <= 0.217 - 0.172 * f {
                break f * (self.config.radius - 20.0).max(0.0);
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
    pub fn add(
        &mut self,
        id: impl Into<String>,
        index: usize,
        count: usize,
    ) -> Result<usize, String> {
        let id = id.into();
        if count == 0 || index >= count {
            return Err("Invalid spawn slot".into());
        }
        if self.snakes.iter().any(|s| s.id == id) {
            return Err("Duplicate agent id".into());
        }
        let a = index as f64 / count as f64 * TAU;
        let distance = self.config.spawn_radius.unwrap_or(self.config.radius * 0.5);
        let head = Point {
            x: a.cos() * distance,
            y: a.sin() * distance,
        };
        // Two segments face inward; a `minScore` body follows the ring counterclockwise.
        let (body, heading) = if self.spawn_segments() > 2 {
            self.spawn_body(head, a + FRAC_PI_2)
        } else {
            let tail = Point {
                x: head.x + a.cos() * 42.0,
                y: head.y + a.sin() * 42.0,
            };
            (vec![head, tail], a + PI)
        };
        let segments = body.len();
        let mut snake = Snake {
            id,
            head,
            body,
            heading,
            speed: base_speed(segments),
            radius: 0.0,
            length: 0.0,
            segments,
            fam: self.min_mass.map_or(0.0, |floor| floor.fam),
            score: 0.0,
            kills: 0,
            alive: true,
        };
        Self::update_mass(&mut snake);
        self.snakes.push(snake);
        self.boosts.push(Boost::default());
        self.speeds.push(SpeedState::idle(base_speed(segments)));
        self.kiosks.push(KioskState::default());
        self.pit_entered.push(false);
        self.kill_times.push(VecDeque::new());
        self.death_times.push(None);
        self.joins.push(self.next_join);
        self.next_join += 1;
        Ok(self.snakes.len() - 1)
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
    /// Synchronize an imported/replaced snake's hidden motion state. None infers
    /// progress from its public speed; Some supplies known progress (0 means idle).
    /// Call after replacement even when the new public speed equals the old value.
    pub fn initialize_speed_state(
        &mut self,
        index: usize,
        progress: Option<f64>,
    ) -> Result<(), String> {
        let snake = self.snakes.get(index).ok_or("Unknown speed-state slot")?;
        let state = match progress {
            Some(q) => SpeedState::with_progress(snake.speed, q)?,
            None => {
                SpeedState::with_progress(snake.speed, 0.0)?;
                SpeedState::from_observed_speed(snake.speed, snake.segments)
            }
        };
        self.speeds[index] = state;
        Ok(())
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
    pub fn spawn_segments(&self) -> usize {
        self.min_mass.map_or(2, |floor| floor.segments)
    }
    /// Head-first body and heading of a new life of `spawn_segments` with its
    /// head at `head`. Two segments lie straight behind `heading`. A larger
    /// `minScore` body follows the circle through `head` around the center,
    /// turning the way `heading` leans, so it keeps the head's distance from
    /// the wall and pit; it is straight if that circle is too small to hold it.
    /// Pass the result to `body_is_clear` and `respawn_body`.
    pub fn spawn_body(&self, head: Point, heading: f64) -> (Vec<Point>, f64) {
        let segments = self.spawn_segments();
        if let Some(ring) = (segments > 2)
            .then(|| crate::spawn::ring_body(head, heading, segments))
            .flatten()
        {
            return ring;
        }
        let body = (0..segments)
            .map(|i| Point {
                x: head.x - heading.cos() * 42.0 * i as f64,
                y: head.y - heading.sin() * 42.0 * i as f64,
            })
            .collect();
        (body, heading)
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
        self.line_is_clear(&[head, tail], segments, clearance)
    }
    /// Check a supplied head-first body with one anchor per segment (2..=430).
    /// Uses the same arena margin and centerline exclusion as straight spawns.
    /// This tests other living snakes, not self-intersections in the supplied body.
    pub fn body_is_clear(&self, body: &[Point], clearance: f64) -> bool {
        (2..=430).contains(&body.len())
            && body.iter().all(|p| p.finite())
            && self.line_is_clear(body, body.len(), clearance)
    }
    fn line_is_clear(&self, line: &[Point], segments: usize, clearance: f64) -> bool {
        if !clearance.is_finite()
            || clearance < 0.0
            || line
                .iter()
                .any(|p| p.distance(Point::default()) >= self.config.radius - 60.)
        {
            return false;
        }
        self.snakes.iter().filter(|s| s.alive).all(|s| {
            let width = (collision_distance(s.segments, segments) + 40.).max(clearance);
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
    ) -> Result<(), String> {
        let s = self.snakes.get(index).ok_or("Unknown respawn slot")?;
        if s.alive || !head.finite() || !heading.is_finite() || !(2..=430).contains(&segments) {
            return Err("Respawn requires a dead slot, finite pose and 2..430 segments".into());
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
    /// With `minScore`, the body needs at least `spawn_segments` anchors; at
    /// exactly that many, the life starts at the floor's score.
    pub fn respawn_body(
        &mut self,
        index: usize,
        body: Vec<Point>,
        heading: f64,
    ) -> Result<(), String> {
        let s = self.snakes.get(index).ok_or("Unknown respawn slot")?;
        if s.alive
            || !heading.is_finite()
            || !(2..=430).contains(&body.len())
            || body.iter().any(|p| !p.finite())
        {
            return Err("Respawn requires a dead slot, finite pose and 2..430 segments".into());
        }
        if body
            .iter()
            .any(|p| p.distance(Point::default()) >= self.config.radius)
        {
            return Err("Spawn body extends outside arena".into());
        }
        let floor = self.min_mass;
        if let Some(floor) = floor.filter(|floor| body.len() < floor.segments) {
            return Err(format!(
                "minScore needs a spawn body of at least {} segments",
                floor.segments
            ));
        }
        let s = &mut self.snakes[index];
        s.head = body[0];
        s.segments = body.len();
        s.body = body;
        // Large finite angles lose per-tick turn increments to rounding.
        s.heading = if heading.abs() > TAU {
            heading.sin().atan2(heading.cos())
        } else {
            heading
        };
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
        self.pit_entered[index] = false;
        self.death_times[index] = None;
        self.joins[index] = self.next_join;
        self.next_join += 1;
        Ok(())
    }
    pub fn living(&self) -> usize {
        self.snakes.iter().filter(|s| s.alive).count()
    }
    /// One physics tick (`config.dt`, the 7 ms server loop by default). Actions use stable
    /// spawn slots; dead slots may be None.
    pub fn step(&mut self, actions: &[Option<Action>]) -> Result<(), String> {
        self.validate_actions(actions)?;
        self.advance_validated(actions);
        Ok(())
    }
    pub fn validate_actions(&self, actions: &[Option<Action>]) -> Result<(), String> {
        // The public configuration may have changed since construction. Reject
        // invalid prey settings before movement, clocks or refill mutate state.
        self.config.prey.validate()?;
        if actions.len() != self.snakes.len() {
            return Err("Action batch must have one slot per spawned agent".into());
        }
        for (s, a) in self.snakes.iter().zip(actions) {
            if s.alive && a.is_none_or(|a| !a.heading.is_finite()) {
                return Err(format!("Missing or invalid action for {}", s.id));
            }
        }
        Ok(())
    }
    pub fn step_profiled(
        &mut self,
        actions: &[Option<Action>],
        profile: &mut StepProfile,
    ) -> Result<(), String> {
        self.validate_actions(actions)?;
        self.advance::<true>(actions, profile);
        Ok(())
    }
    pub(crate) fn advance_validated(&mut self, actions: &[Option<Action>]) {
        self.advance::<false>(actions, &mut StepProfile::default());
    }
    fn advance<const PROFILE: bool>(
        &mut self,
        actions: &[Option<Action>],
        profile: &mut StepProfile,
    ) {
        let mut clock = if PROFILE { Some(Instant::now()) } else { None };
        if let Some(events) = &mut self.pickup_events {
            events.clear();
        }
        if let Some(events) = &mut self.food_spawn_events {
            events.clear();
        }
        self.update_radius(self.time + self.config.dt);
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
            eaten,
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
        // Every food edit from here to the refill is mirrored into the cache, so
        // occupancy is compared with the food list at most once per step.
        self.food_cache.begin();
        for &i in alive.iter() {
            let a = actions[i].expect("validated action");
            let s = &mut self.snakes[i];
            let state = &mut self.boosts[i];
            state.held = a.boost;
            if !a.boost {
                state.exhausted = false;
            }
            // Boosting needs mass above the two-segment minimum; real 2-segment snakes keep
            // boosting down to fam ~0. A `minScore` floor makes boosting there free.
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
            // Public snapshot replacement can change speed; mass-only updates
            // must preserve the prior progress and retained idle speed.
            if speed.speed != s.speed {
                *speed = SpeedState::from_observed_speed(s.speed, s.segments);
            }
            s.speed = speed.step(boosting, s.segments, c.dt, c.speed_model);
            if boosting {
                let mut elapsed = state.clock + c.dt;
                while elapsed >= BOOST_DROP_SECONDS && state.active {
                    elapsed -= BOOST_DROP_SECONDS;
                    let tail = s.body.last().copied().unwrap_or(s.head);
                    let mult = multiplier(s.segments as f64 + s.fam);
                    // Food comes only from removed mass: nothing at the floor and a
                    // smaller pellet for a partial step down to it.
                    let available = floor.map_or(s.segments as f64 - 2.0 + s.fam, |floor| {
                        floor.excess(s.segments, s.fam)
                    });
                    let cost =
                        available.min(boost_cost(s.segments) * (0.94 + 0.14 * self.random.next()));
                    let removed = remove_mass(&mut s.segments, &mut s.fam, cost, floor);
                    state.boost_mass_spent += cost;
                    s.body.truncate(s.body.len() - removed);
                    let size = ((cost * 0.95 / (NUTRITION * mult)).sqrt() * 5.0).floor() / 5.0;
                    if size > 0.0 {
                        let jitter = sc * 16.0;
                        let x = tail.x + (self.random.next() - 0.5) * jitter;
                        let y = tail.y + (self.random.next() - 0.5) * jitter;
                        let pellet = Self::pellet(Point { x, y }, size, Origin::Boost);
                        if c.emit_boost_food {
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
                                        food: pellet.clone(),
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
                state.clock = elapsed;
            } else {
                state.clock = 0.0;
            }
            (s.head, s.heading) = advance(
                s.head,
                s.heading,
                quantize_heading(a.heading),
                s.speed,
                turn_rate(s.segments),
                c.dt,
            );
            let (head, lethal) =
                crate::boundary::constrain(s.head, s.heading, sc, c.radius, c.boundary_mode);
            s.head = head;
            if lethal {
                boundary_deaths.push((i, DeathReason::Boundary));
            } else if let Some(pit) = &c.pit {
                // A mode rule, not an official mechanic: docs/configuration.md#battledome.
                if s.head.distance_lt(Point::default(), pit.radius) {
                    self.pit_entered[i] = true;
                } else if self.pit_entered[i] {
                    boundary_deaths.push((i, DeathReason::Pit));
                }
            }
            if let Some(anchor) = crossed_anchor(previous[i], s.head, s.body[0]) {
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
        heads.clear();
        heads.extend(self.snakes.iter().map(|s| {
            if !s.alive {
                return (Point::default(), (0.0, 0.0), 0.0);
            }
            let scale = scale(s.segments);
            let direction = (s.heading.cos(), s.heading.sin());
            let r = BODY_RADIUS * scale;
            let tip = Point {
                x: s.head.x + direction.0 * r,
                y: s.head.y + direction.1 * r,
            };
            (tip, direction, scale)
        }));
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
                self.snakes[k].kills += 1;
                if self.config.kill_score_seconds.is_some() {
                    self.kill_times[k].push_back(self.time);
                }
            }
            let food = match reason {
                DeathReason::Body => self.death_food(i),
                // Outward browser controls produced no drops, including from
                // body anchors still inside the arena. See ecology evidence.
                DeathReason::Boundary => 0,
                // A mode rule: pit deaths drop no food, like the outer wall.
                DeathReason::Pit => 0,
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
                let (center, radius) = pickup_circle(s.head, s.heading, s.speed, s.segments);
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
            dense_hits::<PROFILE>(
                &self.eater_grid,
                &self.snakes,
                eaters,
                &self.food,
                hits,
                profile,
            );
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
                    item: PickupItem::Food(f.clone()),
                });
            }
            let s = &mut self.snakes[i];
            s.fam += f
                .size
                .map_or(f.value, |size| nutrition(size, s.segments as f64 + s.fam));
            Self::update_mass(s);
        }
        if !hits.is_empty() {
            eaten.clear();
            eaten.resize(self.food.len(), false);
            for &(findex, _) in hits.iter() {
                eaten[findex] = true;
            }
            self.food_cache.remove(eaten);
            let mut i = 0;
            self.food.retain(|_| {
                let keep = !eaten[i];
                i += 1;
                keep
            });
        }
        if PROFILE {
            profile.food_seconds += clock.unwrap().elapsed().as_secs_f64();
            clock = Some(Instant::now());
        }
        self.advance_prey(previous, prey_buffers);
        if PROFILE {
            profile.prey_seconds += clock.unwrap().elapsed().as_secs_f64();
            clock = Some(Instant::now());
        }
        let dt = self.config.dt;
        let target = self.config.food_count;
        let replenish = self.config.ambient_replenish_per_second;
        if replenish == 0.0 {
            self.ambient_clock = 0.0;
        } else {
            let interval = 1.0 / replenish;
            self.ambient_clock += dt;
            if self.ambient_clock + 1e-12 >= interval {
                let mut ambient_food_count = self.food_cache.ambient(&self.food);
                while self.ambient_clock + 1e-12 >= interval {
                    self.ambient_clock -= interval;
                    if ambient_food_count < target {
                        let f = self.new_food();
                        // Cell occupancy includes every origin.
                        if admit_ambient_food(
                            &f,
                            &self.config.ambient_food,
                            &mut self.food_cache,
                            &self.food,
                        ) {
                            if let Some(events) = &mut self.food_spawn_events {
                                events.push(FoodSpawnEvent {
                                    source: None,
                                    food: f.clone(),
                                });
                            }
                            self.food_cache.push(&f);
                            self.food.push(f);
                            ambient_food_count += 1;
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
        if self.config.dynamic_radius {
            let target = population_radius(self.living());
            let step = RADIUS_RATE * self.config.dt;
            self.config.radius += (target - self.config.radius).clamp(-step, step);
        }
        while let Some(update) = self.config.radius_updates.get(self.radius_update_index) {
            if update.time > time + 1e-9 {
                break;
            }
            self.config.radius = update.radius;
            self.radius_update_index += 1;
        }
    }

    fn spawn_prey(&mut self) {
        while self.prey.iter().any(|prey| prey.id == self.next_prey_id) {
            self.next_prey_id = self.next_prey_id.wrapping_add(1);
        }
        let angle = self.prey_random.next() * TAU;
        let radius = self.prey_random.next().sqrt() * self.config.radius * 0.95;
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
    pub fn set_prey_mode(&mut self, id: u64, mode: PreyMode) -> Result<(), String> {
        if self.prey.iter().filter(|prey| prey.id == id).count() != 1 {
            return Err("prey mode requires exactly one matching identity".into());
        }
        let prey = self.prey.iter_mut().find(|prey| prey.id == id).unwrap();
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
        if !outward && old.distance(Point::default()) > c.radius * 0.9 {
            prey.target_heading = (-old.y).atan2(-old.x);
        }
    }

    fn advance_prey(&mut self, previous: &[Point], buffers: &mut PreyBuffers) {
        let c = &self.config;
        let p = &c.prey;
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
        } = buffers;
        live_ids.clear();
        live_ids.extend(self.prey.iter().map(|prey| prey.id));
        self.prey_decision_times
            .retain(|id, _| live_ids.contains(id));
        self.prey_outward.retain(|id| live_ids.contains(id));
        // The global motion-tick phase above survives empty periods.
        if self.prey.is_empty() && p.count == 0 {
            return;
        }
        // The same pickup circle as food. Eating prey changes only mass, so each
        // living snake's circle and lead stay fixed for this whole phase.
        circles.clear();
        circles.extend(self.snakes.iter().map(|s| {
            s.alive.then(|| {
                let (center, radius) = pickup_circle(s.head, s.heading, s.speed, s.segments);
                let lead = Point {
                    x: center.x - s.head.x,
                    y: center.y - s.head.y,
                };
                (center, radius, lead)
            })
        }));
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
                Self::decide_prey(prey, outward, &self.snakes, c, &mut self.prey_random);
            }
            for _ in 0..motion_ticks {
                prey.advance_server_tick(p.turn_rate);
            }
            let distance = prey.position.distance(Point::default());
            if distance >= c.radius {
                let ratio = c.radius * (1.0 - 1e-9) / distance;
                prey.position.x *= ratio;
                prey.position.y *= ratio;
            }
            // Sweep relative motion, so fast prey cannot tunnel through a mouth.
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
                if sweep_misses(a, b, radius) {
                    return None;
                }
                let d = segment_distance(Point::default(), a, b);
                (d <= radius).then_some((i, d))
            };
            let nearest = |(i, a): &(usize, f64), (j, b): &(usize, f64)| {
                a.total_cmp(b)
                    .then_with(|| self.snakes[*i].id.cmp(&self.snakes[*j].id))
            };
            let query = Bounds::between(old, prey.position);
            let indexed = index.as_ref().is_some_and(|index| {
                small(&[old.x, old.y, prey.position.x, prey.position.y])
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
                s.fam += nutrition(prey.size, s.segments as f64 + s.fam) * p.nutrition_factor;
                Self::update_mass(s);
                false
            } else {
                true
            }
        });
        if self.prey.len() >= p.count {
            self.prey_clock = 0.0;
        } else {
            self.prey_clock += c.dt;
            if self.prey_clock >= p.respawn_seconds {
                self.prey_clock -= p.respawn_seconds;
                self.spawn_prey();
            }
        }
    }

    pub fn update_mass(s: &mut Snake) {
        s.length = s.segments as f64 + s.fam;
        s.score = score(s.segments, s.fam);
        s.radius = 14.5 * scale(s.segments);
    }
    fn death_food(&mut self, index: usize) -> usize {
        let s = &self.snakes[index];
        let density = &self.config.death_food;
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
        for site in death_food_sites(&s.body, s.head).rev() {
            let pellet = Self::pellet(
                Point {
                    x: site.x + (self.random.next() * 2.0 - 1.0) * spread,
                    y: site.y + (self.random.next() * 2.0 - 1.0) * spread,
                },
                size,
                Origin::Death,
            );
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
                    food: pellet.clone(),
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
/// allocation per buffer per tick. Every buffer is cleared before use and
/// nothing in it outlives a step, so clones and deserialized worlds start empty.
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
    eaten: Vec<bool>,
    prey: PreyBuffers,
}
#[derive(Default)]
struct PreyBuffers {
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
mod death_food_density_tests {
    use super::*;
    use crate::food_grid::food_cell;

    fn world(enabled: bool) -> Simulator {
        let mut world = Simulator::new(
            7,
            Config {
                food_count: 0,
                ambient_replenish_per_second: 0.0,
                death_food: DeathFoodConfig {
                    enabled,
                    ..DeathFoodConfig::default()
                },
                ..Config::sandbox()
            },
        )
        .unwrap();
        add_corpse(&mut world, "a", Point { x: 37.5, y: 37.5 });
        world
    }

    fn add_corpse(world: &mut Simulator, id: &str, point: Point) {
        let index = world.add(id, 0, 1).unwrap();
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
                .all(|f| food_cell(f.x, f.y, 75.0) == (0, 0))
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
        assert_eq!(food_cell(-37.5, -37.5, 75.0), (-1, -1));
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
        let mut world = Simulator::new(123, config).unwrap();
        world.add("0", 0, 1).unwrap();
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
        world.initialize_speed_state(0, Some(0.0)).unwrap();
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
