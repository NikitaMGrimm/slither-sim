//! Checked scene construction and targeted replay edits, outside the tick path.
use super::checkpoint::{DEAD, LIVING, MAGNITUDE};
use super::*;
use crate::EventRecording;
use std::num::NonZeroU32;

#[derive(Clone, Debug)]
pub struct WorldOptions {
    pub seed: u32,
    pub config: Config,
    pub population: InitialPopulation,
}

impl Default for WorldOptions {
    fn default() -> Self {
        Self {
            seed: 1,
            config: Config::default(),
            population: InitialPopulation::Empty,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub enum InitialPopulation {
    #[default]
    Empty,
    Dormant {
        ids: Vec<String>,
    },
    Placed {
        ids: Vec<String>,
        placement: Placement,
    },
}

/// Synthetic ring placement is the historical default. Admission is a bounded
/// search, not a proof that an arena is full and not the host's retry policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Placement {
    #[default]
    TestRing,
    Admitted {
        #[serde(default = "default_attempts")]
        attempts: NonZeroU32,
        #[serde(default = "default_clearance")]
        preferred_clearance: f64,
    },
}

fn default_attempts() -> NonZeroU32 {
    NonZeroU32::new(64).unwrap()
}
fn default_clearance() -> f64 {
    600.
}

#[derive(Clone, Copy, Debug)]
pub enum InitialSpeed {
    Infer(f64),
    Known(SpeedState),
}

/// A new replay life. Segment count and display values are derived from mass;
/// the slot identity is retained. Exact historical continuation uses checkpoints.
#[derive(Clone, Debug)]
pub struct ReplayLife {
    pub head: Point,
    pub body: Vec<Point>,
    pub heading: f64,
    pub fam: f64,
    pub kills: usize,
    pub alive: bool,
    pub speed: InitialSpeed,
}

impl ReplayLife {
    pub fn from_observed(snake: Snake, speed: InitialSpeed) -> Self {
        Self {
            head: snake.head,
            body: snake.body,
            heading: snake.heading,
            fam: snake.fam,
            kills: snake.kills,
            alive: snake.alive,
            speed,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReplayPrey {
    pub prey: Prey,
    pub mode: PreyMode,
}

/// Supplied time-zero state. No generated food, prey, or placement RNG draws.
#[derive(Clone, Debug, Default)]
pub struct Scenario {
    pub snakes: Vec<(String, ReplayLife)>,
    pub food: Vec<Food>,
    pub prey: Vec<ReplayPrey>,
}

fn allocation() -> SimError {
    SimError::new(ErrorKind::AllocationFailed, "State does not fit in memory")
}
fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), SimError> {
    values.try_reserve(additional).map_err(|_| allocation())
}
fn finite(value: f64, field: &str) -> Result<(), SimError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(SimError::state(
            Constraint::NonFinite,
            field,
            "State must be finite",
        ))
    }
}
fn nonnegative(value: f64, field: &str) -> Result<(), SimError> {
    if let Some(rule) = Constraint::nonnegative(value) {
        Err(SimError::state(
            rule,
            field,
            "State must be finite and nonnegative",
        ))
    } else {
        Ok(())
    }
}
// Share the checkpoint continuation envelope for all checked replay ingress.
fn bounded(value: f64, limit: f64, field: &str) -> Result<(), SimError> {
    finite(value, field)?;
    if value.abs() > limit {
        return Err(SimError::state(
            Constraint::OutOfRange,
            field,
            "State exceeds the finite continuation envelope",
        ));
    }
    Ok(())
}
fn point(point: Point, limit: f64, field: &str) -> Result<(), SimError> {
    bounded(point.x, limit, field)?;
    bounded(point.y, limit, field)
}
fn body(body: &[Point], limit: f64) -> Result<(), SimError> {
    if body.len() < 2
        || body
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .is_none()
    {
        return Err(SimError::state(
            Constraint::OutOfRange,
            "body",
            "A body needs at least two anchors",
        ));
    }
    for &p in body {
        point(p, limit, "body")?;
    }
    Ok(())
}
fn food(food: &Food) -> Result<(), SimError> {
    bounded(food.x, DEAD, "food.x")?;
    bounded(food.y, DEAD, "food.y")?;
    nonnegative(food.value, "food.value")?;
    if let Some(size) = food.size {
        nonnegative(size, "food.size")?;
    }
    Ok(())
}

impl Simulator {
    pub fn new(options: WorldOptions) -> Result<Self, SimError> {
        let WorldOptions {
            seed,
            config,
            population,
        } = options;
        let mut world = Self::empty(seed, config)?;
        // Validate IDs and placement before spending any ecology draws.
        let (ids, placement) = match population {
            InitialPopulation::Empty => (Vec::new(), None),
            InitialPopulation::Dormant { ids } => (ids, None),
            InitialPopulation::Placed { ids, placement } => (ids, Some(placement)),
        };
        let mut unique = HashSet::new();
        unique.try_reserve(ids.len()).map_err(|_| allocation())?;
        for (slot, id) in ids.iter().enumerate() {
            if !unique.insert(id.as_str()) {
                return Err(SimError::new(ErrorKind::DuplicateId, "Duplicate agent id")
                    .at_slot(slot)
                    .at_field("id"));
            }
        }
        if let Some(Placement::Admitted {
            preferred_clearance,
            ..
        }) = placement
        {
            nonnegative(preferred_clearance, "placement.preferredClearance")?;
        }
        world.initialize_ecology()?;
        world.reserve_slots(ids.len())?;
        let count = ids.len();
        // Domain-separated LCG, independent of the food and prey streams. This
        // stream belongs only to construction; callers own later admission.
        let mut random = Random(seed ^ 0x706c_6163); // "plac"
        for (slot, id) in ids.into_iter().enumerate() {
            match placement {
                None => {
                    world.push_dormant(id)?;
                }
                Some(Placement::TestRing) => {
                    world.push_ring(id, slot, count)?;
                }
                Some(Placement::Admitted {
                    attempts,
                    preferred_clearance,
                }) => {
                    let mut best = None;
                    let mut margin = 0.0;
                    for _ in 0..attempts.get() {
                        let angle = random.next() * TAU;
                        let distance = crate::spawn::spawn_distance(&world.config, random.next());
                        let heading = random.next() * TAU;
                        let head = Point {
                            x: angle.cos() * distance,
                            y: angle.sin() * distance,
                        };
                        let Ok((body, heading)) = world.spawn_body(head, heading) else {
                            continue;
                        };
                        if world.body_is_clear(&body, preferred_clearance) {
                            best = Some((body, heading));
                            break;
                        }
                        if let Some(candidate) = world.spawn_margin(&body).filter(|&m| m > margin) {
                            margin = candidate;
                            best = Some((body, heading));
                        }
                    }
                    let (body, heading) = best.ok_or_else(|| {
                        SimError::new(
                            ErrorKind::PlacementExhausted {
                                attempts: attempts.get(),
                            },
                            "Spawn candidate search exhausted",
                        )
                        .at_slot(slot)
                    })?;
                    world.push_spawn(id, body, heading)?;
                }
            }
        }
        Ok(world)
    }

    pub fn from_scenario(seed: u32, config: Config, scenario: Scenario) -> Result<Self, SimError> {
        let mut world = Self::empty(seed, config)?;
        let mut unique = HashSet::new();
        unique
            .try_reserve(scenario.snakes.len())
            .map_err(|_| allocation())?;
        for (slot, (id, _)) in scenario.snakes.iter().enumerate() {
            if !unique.insert(id.as_str()) {
                return Err(SimError::new(ErrorKind::DuplicateId, "Duplicate agent id")
                    .at_slot(slot)
                    .at_field("id"));
            }
        }
        world.reserve_slots(scenario.snakes.len())?;
        for (slot, (id, life)) in scenario.snakes.into_iter().enumerate() {
            let (snake, speed) = world.replay_snake(id, life).map_err(|e| e.at_slot(slot))?;
            world.push_life(snake, speed).map_err(|e| e.at_slot(slot))?;
        }
        world.replace_food(scenario.food)?;
        world.replace_prey(scenario.prey)?;
        Ok(world)
    }

    #[inline]
    pub fn config(&self) -> &Config {
        &self.config
    }
    #[inline]
    pub fn radius(&self) -> f64 {
        self.config.radius
    }
    #[inline]
    pub fn time(&self) -> f64 {
        self.time
    }
    #[inline]
    pub fn snakes(&self) -> &[Snake] {
        &self.snakes
    }
    #[inline]
    pub fn food(&self) -> &[Food] {
        &self.food
    }
    #[inline]
    pub fn prey(&self) -> &[Prey] {
        &self.prey
    }
    #[inline]
    pub fn deaths(&self) -> &[DeathEvent] {
        &self.deaths
    }
    #[inline]
    pub fn pickup_events(&self) -> Option<&[PickupEvent]> {
        self.pickup_events.as_deref()
    }
    #[inline]
    pub fn food_spawn_events(&self) -> Option<&[FoodSpawnEvent]> {
        self.food_spawn_events.as_deref()
    }

    /// Enabling an already enabled recorder retains its events and capacity.
    pub fn record_events(&mut self, options: EventRecording) {
        if options.pickups {
            self.pickup_events.get_or_insert_with(Vec::new);
        } else {
            self.pickup_events = None;
        }
        if options.food_births {
            self.food_spawn_events.get_or_insert_with(Vec::new);
        } else {
            self.food_spawn_events = None;
        }
    }

    fn check_id(&self, id: &str) -> Result<(), SimError> {
        if self.snakes.iter().any(|snake| snake.id == id) {
            Err(SimError::new(ErrorKind::DuplicateId, "Duplicate agent id").at_field("id"))
        } else {
            Ok(())
        }
    }
    fn check_slot(&self, slot: usize) -> Result<(), SimError> {
        if slot < self.snakes.len() {
            Ok(())
        } else {
            Err(SimError::new(ErrorKind::UnknownSlot, "Unknown agent slot").at_slot(slot))
        }
    }
    pub(super) fn check_join(&self) -> Result<(), SimError> {
        if self.next_join < u64::MAX - 1 {
            Ok(())
        } else {
            Err(
                SimError::new(ErrorKind::RepresentationLimit, "Join order exhausted")
                    .at_field("nextJoin"),
            )
        }
    }
    fn reserve_slots(&mut self, additional: usize) -> Result<(), SimError> {
        reserve(&mut self.snakes, additional)?;
        reserve(&mut self.boosts, additional)?;
        reserve(&mut self.speeds, additional)?;
        reserve(&mut self.kiosks, additional)?;
        reserve(&mut self.pit_entered, additional)?;
        reserve(&mut self.kill_times, additional)?;
        reserve(&mut self.death_times, additional)?;
        reserve(&mut self.joins, additional)
    }
    fn push_life(&mut self, snake: Snake, speed: SpeedState) -> Result<usize, SimError> {
        self.check_join()?;
        self.reserve_slots(1)?;
        let slot = self.snakes.len();
        self.snakes.push(snake);
        self.boosts.push(Boost::default());
        self.speeds.push(speed);
        self.kiosks.push(KioskState::default());
        self.pit_entered.push(false);
        self.kill_times.push(VecDeque::new());
        self.death_times.push(None);
        self.joins.push(self.next_join);
        self.next_join += 1;
        Ok(slot)
    }
    fn push_dormant(&mut self, id: String) -> Result<usize, SimError> {
        let segments = self.spawn_segments();
        let mut body = Vec::new();
        reserve(&mut body, segments)?;
        body.resize(segments, Point::default());
        let (snake, speed) = self.replay_snake(
            id,
            ReplayLife {
                head: Point::default(),
                body,
                heading: 0.,
                fam: self.min_mass.map_or(0., |floor| floor.fam),
                kills: 0,
                alive: false,
                speed: InitialSpeed::Known(SpeedState::idle(base_speed(segments))),
            },
        )?;
        self.push_life(snake, speed)
    }
    pub fn append_dormant(&mut self, id: String) -> Result<usize, SimError> {
        self.check_id(&id)?;
        self.push_dormant(id)
    }
    pub fn spawn(&mut self, id: String, body: Vec<Point>, heading: f64) -> Result<usize, SimError> {
        self.check_id(&id)?;
        bounded(heading, MAGNITUDE, "heading")?;
        // Match respawn without changing synthetic ring or replay representations.
        let heading = if heading.abs() > TAU {
            heading.sin().atan2(heading.cos())
        } else {
            heading
        };
        self.push_spawn(id, body, heading)
    }
    pub(super) fn push_spawn(
        &mut self,
        id: String,
        body: Vec<Point>,
        heading: f64,
    ) -> Result<usize, SimError> {
        self.validate_spawn(&body, heading)?;
        let segments = body.len();
        let (snake, speed) = self.replay_snake(
            id,
            ReplayLife {
                head: body[0],
                body,
                heading,
                fam: self
                    .min_mass
                    .filter(|f| f.segments == segments)
                    .map_or(0., |f| f.fam),
                kills: 0,
                alive: true,
                speed: InitialSpeed::Known(SpeedState::idle(base_speed(segments))),
            },
        )?;
        self.push_life(snake, speed)
    }
    pub(super) fn validate_spawn(&self, points: &[Point], heading: f64) -> Result<(), SimError> {
        body(points, LIVING)?;
        bounded(heading, MAGNITUDE, "heading")?;
        if points.len() > MAX_SEGMENTS {
            return Err(SimError::state(
                Constraint::OutOfRange,
                "body",
                "Spawn requires 2..430 anchors",
            ));
        }
        if points
            .iter()
            .any(|p| p.distance(Point::default()) >= self.radius())
        {
            return Err(SimError::state(
                Constraint::OutsideArena,
                "body",
                "Spawn body extends outside arena",
            ));
        }
        Ok(())
    }
    fn replay_snake(&self, id: String, life: ReplayLife) -> Result<(Snake, SpeedState), SimError> {
        let limit = if life.alive { LIVING } else { DEAD };
        body(&life.body, limit)?;
        point(life.head, limit, "head")?;
        bounded(life.heading, MAGNITUDE, "heading")?;
        self.validate_mass(life.body.len(), life.fam)?;
        let speed = match life.speed {
            InitialSpeed::Infer(speed) => {
                SpeedState::with_progress(speed, 0.)?;
                SpeedState::from_observed_speed(speed, life.body.len())
            }
            InitialSpeed::Known(state) => SpeedState::with_progress(state.speed, state.progress())?,
        };
        bounded(speed.speed * self.config.dt, MAGNITUDE, "speed")?;
        let mut snake = Snake {
            id,
            head: life.head,
            segments: life.body.len(),
            body: life.body,
            heading: life.heading,
            fam: life.fam,
            speed: speed.speed,
            kills: life.kills,
            alive: life.alive,
            radius: 0.,
            length: 0.,
            score: 0.,
        };
        Self::update_mass(&mut snake);
        Ok((snake, speed))
    }
    fn validate_mass(&self, segments: usize, fam: f64) -> Result<(), SimError> {
        nonnegative(fam, "fam")?;
        finite(segments as f64 + fam, "fam")?;
        if self
            .min_mass
            .is_some_and(|floor| (segments, fam) < (floor.segments, floor.fam))
        {
            return Err(SimError::state(
                Constraint::OutOfRange,
                "fam",
                "Mass is below the configured floor",
            ));
        }
        Ok(())
    }
    /// Retire without physics death, food, or slot removal. The next import or
    /// respawn resets per-life controls; rolling kill history survives.
    pub fn retire(&mut self, slot: usize) -> Result<(), SimError> {
        self.check_slot(slot)?;
        self.snakes[slot].alive = false;
        self.death_times[slot] = None;
        Ok(())
    }
    pub fn import_life(&mut self, slot: usize, life: ReplayLife) -> Result<(), SimError> {
        self.check_slot(slot)?;
        self.check_join()?;
        let (mut snake, speed) = self
            .replay_snake(String::new(), life)
            .map_err(|e| e.at_slot(slot))?;
        snake.id = std::mem::take(&mut self.snakes[slot].id);
        self.snakes[slot] = snake;
        self.speeds[slot] = speed;
        self.boosts[slot] = Boost::default();
        self.kiosks[slot] = KioskState::default();
        self.pit_entered[slot] = false;
        self.death_times[slot] = None;
        self.joins[slot] = self.next_join;
        self.next_join += 1;
        Ok(())
    }
    pub fn correct_mass(
        &mut self,
        slot: usize,
        points: Vec<Point>,
        fam: f64,
    ) -> Result<(), SimError> {
        self.check_slot(slot)?;
        body(
            &points,
            if self.snakes[slot].alive {
                LIVING
            } else {
                DEAD
            },
        )
        .map_err(|e| e.at_slot(slot))?;
        self.validate_mass(points.len(), fam)
            .map_err(|e| e.at_slot(slot))?;
        let snake = &mut self.snakes[slot];
        snake.segments = points.len();
        snake.body = points;
        snake.fam = fam;
        Self::update_mass(snake);
        self.kiosks[slot].finish_score(snake);
        Ok(())
    }
    pub fn correct_pose(
        &mut self,
        slot: usize,
        head: Point,
        points: Vec<Point>,
        heading: f64,
    ) -> Result<(), SimError> {
        self.check_slot(slot)?;
        point(
            head,
            if self.snakes[slot].alive {
                LIVING
            } else {
                DEAD
            },
            "head",
        )
        .map_err(|e| e.at_slot(slot))?;
        body(
            &points,
            if self.snakes[slot].alive {
                LIVING
            } else {
                DEAD
            },
        )
        .map_err(|e| e.at_slot(slot))?;
        bounded(heading, MAGNITUDE, "heading").map_err(|e| e.at_slot(slot))?;
        if points.len() != self.snakes[slot].segments {
            return Err(SimError::state(
                Constraint::LengthMismatch,
                "body",
                "Pose correction must retain segment count",
            )
            .at_slot(slot));
        }
        let snake = &mut self.snakes[slot];
        snake.head = head;
        snake.body = points;
        snake.heading = heading;
        Ok(())
    }
    pub fn replace_food(&mut self, values: Vec<Food>) -> Result<(), SimError> {
        for value in &values {
            food(value)?;
        }
        self.food = values;
        self.food_cache = FoodCache::default();
        Ok(())
    }
    pub fn insert_food(&mut self, value: Food) -> Result<usize, SimError> {
        food(&value)?;
        reserve(&mut self.food, 1)?;
        let index = self.food.len();
        self.food.push(value);
        self.food_cache = FoodCache::default();
        Ok(index)
    }
    pub fn remove_food(&mut self, index: usize) -> Result<Food, SimError> {
        if index >= self.food.len() {
            return Err(
                SimError::new(ErrorKind::UnknownEntity, "Unknown food index").at_field("food"),
            );
        }
        let food = self.food.remove(index);
        self.food_cache = FoodCache::default();
        Ok(food)
    }
    fn validate_prey(&self, prey: &Prey) -> Result<(), SimError> {
        finite(prey.position.x, "prey.position.x")?;
        finite(prey.position.y, "prey.position.y")?;
        bounded(prey.heading, MAGNITUDE, "prey.heading")?;
        bounded(prey.target_heading, MAGNITUDE, "prey.targetHeading")?;
        nonnegative(prey.speed, "prey.speed")?;
        if let Some(rule) = Constraint::positive(prey.size) {
            return Err(SimError::state(
                rule,
                "prey.size",
                "Prey size must be positive and finite",
            ));
        }
        finite(
            nutrition(prey.size, 2.) * self.config.prey.nutrition_factor + MAX_SEGMENTS as f64,
            "prey.size",
        )
    }
    pub fn replace_prey(&mut self, values: Vec<ReplayPrey>) -> Result<(), SimError> {
        let mut prey = Vec::new();
        let mut ids = HashSet::new();
        let mut outward = HashSet::new();
        let mut decisions = HashMap::new();
        reserve(&mut prey, values.len())?;
        ids.try_reserve(values.len()).map_err(|_| allocation())?;
        outward
            .try_reserve(values.len())
            .map_err(|_| allocation())?;
        decisions
            .try_reserve(values.len())
            .map_err(|_| allocation())?;
        for value in values {
            self.validate_prey(&value.prey)?;
            let id = value.prey.id;
            if !ids.insert(id) {
                return Err(
                    SimError::new(ErrorKind::DuplicateId, "Duplicate prey id").at_field("prey.id")
                );
            }
            if value.mode == PreyMode::Outward {
                outward.insert(id);
                decisions.insert(id, self.time + 1. / 3.);
            }
            prey.push(value.prey);
        }
        self.prey = prey;
        self.prey_outward = outward;
        self.prey_decision_times = decisions;
        Ok(())
    }
    pub fn insert_prey(&mut self, value: ReplayPrey) -> Result<(), SimError> {
        self.validate_prey(&value.prey)?;
        let id = value.prey.id;
        if self.prey.iter().any(|p| p.id == id) {
            return Err(
                SimError::new(ErrorKind::DuplicateId, "Duplicate prey id").at_field("prey.id")
            );
        }
        reserve(&mut self.prey, 1)?;
        if value.mode == PreyMode::Outward {
            self.prey_outward.try_reserve(1).map_err(|_| allocation())?;
            self.prey_decision_times
                .try_reserve(1)
                .map_err(|_| allocation())?;
            self.prey_outward.insert(id);
            self.prey_decision_times.insert(id, self.time + 1. / 3.);
        }
        self.prey.push(value.prey);
        Ok(())
    }
    pub fn remove_prey(&mut self, id: u64) -> Result<Prey, SimError> {
        let index = self.prey.iter().position(|p| p.id == id).ok_or_else(|| {
            SimError::new(ErrorKind::UnknownEntity, "Unknown prey id").at_field("prey.id")
        })?;
        self.prey_outward.remove(&id);
        self.prey_decision_times.remove(&id);
        Ok(self.prey.remove(index))
    }
}
