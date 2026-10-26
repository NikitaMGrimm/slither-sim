//! Measurement adapters only. Workload definitions stay in the shared harness.
#![allow(dead_code)]
use slither_sim::{Config, Food, Simulator, Snake};

pub fn ring(seed: u32, config: Config, count: usize) -> Result<Simulator, String> {
    #[cfg(slither_private_state)]
    {
        use slither_sim::{InitialPopulation, Placement, WorldOptions};
        Simulator::new(WorldOptions {
            seed,
            config,
            population: InitialPopulation::Placed {
                ids: (0..count).map(|s| s.to_string()).collect(),
                placement: Placement::TestRing,
            },
        })
        .map_err(|e| e.to_string())
    }
    #[cfg(not(slither_private_state))]
    {
        let mut world = Simulator::new(seed, config).map_err(|e| e.to_string())?;
        for slot in 0..count {
            world
                .add(slot.to_string(), slot, count)
                .map_err(|e| e.to_string())?;
        }
        Ok(world)
    }
}

/// Historical benchmark fixtures changed mass/pose and initialized idle speed
/// without starting a new life. A checked checkpoint preserves their original
/// join order, RNG, and every other timer. This is untimed time-zero setup only;
/// normal replay imports deliberately start a new life and advance join order.
pub fn initial_snakes(world: &mut Simulator, mut snakes: Vec<Snake>) -> Result<(), String> {
    use slither_sim::physics;
    for snake in &mut snakes {
        snake.length = snake.segments as f64 + snake.fam;
        snake.score = physics::score(snake.segments, snake.fam);
        snake.radius = 14.5 * physics::scale(snake.segments);
    }
    #[cfg(slither_private_state)]
    {
        assert_eq!(world.time(), 0.);
        assert_eq!(snakes.len(), world.snakes().len());
        let mut checkpoint = serde_json::to_value(world.checkpoint().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        for (slot, snake) in checkpoint["state"]["slots"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .zip(snakes)
        {
            slot["speed"] = serde_json::json!({"speed": snake.speed, "progress": 0.});
            slot["snake"] = serde_json::to_value(snake).map_err(|e| e.to_string())?;
        }
        let bytes = serde_json::to_vec(&checkpoint).map_err(|e| e.to_string())?;
        world
            .restore(&bytes, slither_sim::RestoreOptions::default())
            .map_err(|e| e.to_string())
    }
    #[cfg(not(slither_private_state))]
    {
        assert_eq!(world.time, 0.);
        assert_eq!(snakes.len(), world.snakes.len());
        world.snakes = snakes;
        for slot in 0..world.snakes.len() {
            world
                .initialize_speed_state(slot, Some(0.))
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

pub fn record_events(world: &mut Simulator) {
    #[cfg(slither_private_state)]
    world.record_events(slither_sim::EventRecording {
        pickups: true,
        food_births: true,
    });
    #[cfg(not(slither_private_state))]
    {
        world.pickup_events = Some(Vec::new());
        world.food_spawn_events = Some(Vec::new());
    }
}

pub fn replace_food(world: &mut Simulator, food: Vec<Food>) -> Result<(), String> {
    #[cfg(slither_private_state)]
    {
        world.replace_food(food).map_err(|e| e.to_string())
    }
    #[cfg(not(slither_private_state))]
    {
        world.food = food;
        Ok(())
    }
}

pub fn retire(world: &mut Simulator, slot: usize) -> Result<(), String> {
    #[cfg(slither_private_state)]
    {
        world.retire(slot).map_err(|e| e.to_string())
    }
    #[cfg(not(slither_private_state))]
    {
        world.snakes[slot].alive = false;
        Ok(())
    }
}

#[cfg(not(slither_private_state))]
pub trait LegacyState {
    fn correct_mass(
        &mut self,
        slot: usize,
        body: Vec<slither_sim::Point>,
        fam: f64,
    ) -> Result<(), String>;
    fn correct_pose(
        &mut self,
        slot: usize,
        head: slither_sim::Point,
        body: Vec<slither_sim::Point>,
        heading: f64,
    ) -> Result<(), String>;
    fn retire(&mut self, slot: usize) -> Result<(), String>;
    fn insert_food(&mut self, food: Food) -> Result<usize, String>;
    fn radius(&self) -> f64;
    fn config(&self) -> &Config;
    fn time(&self) -> f64;
    fn snakes(&self) -> &[Snake];
    fn food(&self) -> &[Food];
    fn prey(&self) -> &[slither_sim::Prey];
    fn deaths(&self) -> &[slither_sim::DeathEvent];
    fn pickup_events(&self) -> Option<&[slither_sim::PickupEvent]>;
    fn food_spawn_events(&self) -> Option<&[slither_sim::FoodSpawnEvent]>;
}
#[cfg(not(slither_private_state))]
impl LegacyState for Simulator {
    fn correct_mass(
        &mut self,
        slot: usize,
        body: Vec<slither_sim::Point>,
        fam: f64,
    ) -> Result<(), String> {
        let snake = &mut self.snakes[slot];
        snake.segments = body.len();
        snake.body = body;
        snake.fam = fam;
        Simulator::update_mass(snake);
        Ok(())
    }
    fn correct_pose(
        &mut self,
        slot: usize,
        head: slither_sim::Point,
        body: Vec<slither_sim::Point>,
        heading: f64,
    ) -> Result<(), String> {
        let snake = &mut self.snakes[slot];
        snake.head = head;
        snake.body = body;
        snake.heading = heading;
        Ok(())
    }
    fn retire(&mut self, slot: usize) -> Result<(), String> {
        retire(self, slot)
    }
    fn insert_food(&mut self, food: Food) -> Result<usize, String> {
        let index = self.food.len();
        self.food.push(food);
        Ok(index)
    }
    fn radius(&self) -> f64 {
        self.config.radius
    }

    #[inline]
    fn config(&self) -> &Config {
        &self.config
    }
    #[inline]
    fn time(&self) -> f64 {
        self.time
    }
    #[inline]
    fn snakes(&self) -> &[Snake] {
        &self.snakes
    }
    #[inline]
    fn food(&self) -> &[Food] {
        &self.food
    }
    #[inline]
    fn prey(&self) -> &[slither_sim::Prey] {
        &self.prey
    }
    #[inline]
    fn deaths(&self) -> &[slither_sim::DeathEvent] {
        &self.deaths
    }
    #[inline]
    fn pickup_events(&self) -> Option<&[slither_sim::PickupEvent]> {
        self.pickup_events.as_deref()
    }
    #[inline]
    fn food_spawn_events(&self) -> Option<&[slither_sim::FoodSpawnEvent]> {
        self.food_spawn_events.as_deref()
    }
}

/// The old helper read the live radius from Config; the split API takes it explicitly.
pub fn spawn_distance(world: &Simulator, unit: f64) -> f64 {
    #[cfg(slither_runtime_radius)]
    {
        slither_sim::spawn::spawn_distance(world.config(), world.radius(), unit)
    }
    #[cfg(not(slither_runtime_radius))]
    {
        slither_sim::spawn::spawn_distance(world.config(), unit)
    }
}

/// Project only the intended geometric v4 contract/radius split onto v3 for comparison.
/// Full original replies are retained by the separate ordered radius comparison.
pub fn geometric_reply(
    bytes: &[u8],
    expected_rules: &serde_json::Value,
) -> Result<Vec<u8>, String> {
    let mut value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    match value["protocol"].as_str() {
        Some("slither-sim-json-v3" | "slither-sim-json-v4" | "slither-sim-json-v5") => {}
        _ => return Err("Unknown geometric comparison contract".into()),
    }
    value["protocol"] = serde_json::json!("slither-sim-json-v3");
    if let Some(worlds) = value
        .get_mut("result")
        .and_then(|result| result.get_mut("worlds"))
        .and_then(serde_json::Value::as_array_mut)
    {
        for world in worlds {
            if let Some(rules) = world.as_object_mut().unwrap().remove("rules") {
                if &rules != expected_rules {
                    return Err("Geometric frame rules differ from effective configuration".into());
                }
            } else if cfg!(slither_world_rules) {
                return Err("Geometric frame has no rules".into());
            }
        }
    }
    if value.get("error").is_some_and(serde_json::Value::is_object) {
        let error = &value["error"];
        let message = error["message"].as_str().ok_or("Error has no message")?;
        let expected = "World 0: Initial spawn body extends outside arena";
        if message != expected
            || error["code"] != 9
            || error["rule"] != 9
            || error["world"] != 0
            || error["slot"] != 0
            || error["field"] != "body"
        {
            return Err("Unexpected coded error in geometric comparison".into());
        }
        value["error"] = serde_json::json!("Initial spawn body extends outside arena");
    }
    if let Some(result) = value
        .get_mut("result")
        .and_then(serde_json::Value::as_object_mut)
        && result.contains_key("snakes")
        && let Some(radius) = result.remove("radius")
    {
        result.get_mut("config").ok_or("Snapshot has no config")?["radius"] = radius;
    }
    serde_json::to_vec(&value).map_err(|e| e.to_string())
}

/// Config-derived legacy rules projection; validate new metadata before normalization.
pub fn rules(config: &Config) -> serde_json::Value {
    serde_json::json!({"boundaryMode":config.boundary_mode, "pitRadius":config.pit.as_ref().map(|p|p.radius),
        "minScore":config.min_score, "killScoreSeconds":config.kill_score_seconds,
        "freeBoostAtFloor":config.min_score.is_some()})
}
pub fn frame_debug(
    frame: slither_sim::transport::FrameRef<'_>,
    expected_rules: &serde_json::Value,
) -> String {
    #[cfg(slither_world_rules)]
    assert_eq!(&serde_json::to_value(frame.rules).unwrap(), expected_rules);
    #[cfg(not(slither_world_rules))]
    let _ = expected_rules;
    format!(
        "FrameRef {{ time: {:?}, ticks: {:?}, living: {:?}, radius: {:?}, vision: {:?}, snakes: {:?}, food: {:?}, prey: {:?}, views: {:?}, deaths: {:?} }}",
        frame.time,
        frame.ticks,
        frame.living,
        frame.radius,
        frame.vision,
        frame.snakes,
        frame.food,
        frame.prey,
        frame.views,
        frame.deaths
    )
}
