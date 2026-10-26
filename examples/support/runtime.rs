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
