//! Opt-in seams for offline catalog fixtures; normal host operations stay in the library.
use super::super::{RawSimulator, SimulationConfig};
use crate::client::latency::Random;
use crate::{Action, Point, Simulator};

#[doc(hidden)]
pub struct RawWorld(RawSimulator);

impl RawWorld {
    pub fn new(seed: u32, config: SimulationConfig, world: Simulator) -> Result<Self, String> {
        if config.agents != world.snakes().len() || world.time() != 0. {
            return Err("Raw fixture needs matching slots and a time-zero world".into());
        }
        let count = config.agents;
        let mut raw = RawSimulator::new(seed, config, count)?;
        raw.backend.world = world;
        raw.backend
            .food_presentation
            .sync(raw.backend.world.food())?;
        for slot in 0..count {
            if raw.backend.world.snakes()[slot].alive {
                raw.backend.slots[slot].control = Action {
                    heading: raw.backend.world.snakes()[slot].heading,
                    boost: false,
                };
                raw.backend.connect(slot)?;
                raw.backend.slots[slot].connection.decorations = true;
                raw.backend.ingest(slot, 0.)?;
            }
        }
        raw.backend.broadcast_with(0, &mut |_| Ok(()), true)?;
        Ok(Self(raw))
    }

    pub fn world(&self) -> &Simulator {
        &self.0.backend.world
    }

    pub fn controls(&mut self, actions: &[Option<Action>]) -> Result<(), String> {
        if actions.len() != self.0.backend.slots.len()
            || actions.iter().flatten().any(|a| !a.heading.is_finite())
        {
            return Err("Invalid raw fixture controls".into());
        }
        for (slot, action) in self.0.backend.slots.iter_mut().zip(actions) {
            if let Some(action) = action {
                slot.control = *action;
            }
        }
        Ok(())
    }

    pub fn tick(&mut self) -> Result<(), String> {
        self.0.advance_tick_with(0, &mut |_| Ok(()))
    }

    pub fn search(&mut self, seed: u64) -> Result<(Vec<Point>, f64), String> {
        self.0.backend.spawn_random = Random::new(seed);
        self.0.backend.spawn_pose_waited(usize::MAX, 0.)
    }

    pub fn food_epoch(&self) -> u64 {
        self.0.backend.food_epoch
    }

    pub fn food_update_next(&self) -> bool {
        let b = &self.0.backend;
        (((b.tick + 1) as f64 * b.world.config().dt / crate::client::visibility::CLEANUP_SECONDS)
            .floor() as u64)
            > b.food_epoch
    }

    pub fn composition(&self) -> (usize, usize, usize) {
        let mut scratch = Vec::new();
        let (mut recipients, mut known, mut food) = (0, 0, 0);
        for slot in &self.0.backend.slots {
            if slot.connection.setup {
                recipients += 1;
                slot.connection.snakes.candidates(&[], &mut scratch);
                known += scratch.len();
                food += slot.connection.food.perf_len();
            }
        }
        (recipients, known, food)
    }

    pub fn drain(&mut self) -> Result<Vec<(usize, f64, Vec<u8>)>, String> {
        let mut records = Vec::new();
        for slot in 0..self.0.backend.slots.len() {
            records.extend(
                self.0
                    .take(slot)?
                    .into_iter()
                    .map(|(at, bytes)| (slot, at, bytes)),
            );
        }
        Ok(records)
    }
}
