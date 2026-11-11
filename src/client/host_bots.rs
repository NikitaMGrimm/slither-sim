//! World-thread host bots. `center` and `follow` slots never connect, decode,
//! receive frames or model latency: the world thread respawns and steers them
//! from world state, so they cost only their steering and physics.
//!
//! Follow bots steer at the owner's current head with no prediction, offset or
//! avoidance, so they can kill the owner. Collisions, death food and every
//! other rule stay the same for everyone. Synthetic regressions live in
//! simulation/host_bot_tests.rs.
use crate::{Point, Simulator};
use serde::{Deserialize, Serialize};
use std::ops::Range;

/// What the host's bot slots do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BotBehavior {
    /// A4 food seekers: protocol clients of their slots on bot worker threads.
    #[default]
    Forager,
    /// World-thread bots that steer toward the arena center.
    Center,
    /// World-thread bots that steer at the owner's head every tick; `center`
    /// while no owner is alive.
    Follow,
}

/// Center bots re-aim and dead bots respawn on staggered six-tick decisions.
const DECISION_TICKS: u64 = 6;

pub(super) struct InWorldBots {
    slots: Range<usize>,
    behavior: BotBehavior,
}

impl InWorldBots {
    pub(super) fn new(slots: Range<usize>, behavior: BotBehavior) -> Self {
        Self { slots, behavior }
    }

    pub(super) fn slots(&self) -> Range<usize> {
        self.slots.clone()
    }

    /// Staggered decision ticks for aiming and respawning.
    pub(super) fn decides(&self, slot: usize, tick: u64) -> bool {
        (tick + (slot - self.slots.start) as u64).is_multiple_of(DECISION_TICKS)
    }

    /// The wanted heading for a living bot this tick, or `None` to keep the
    /// last one. `owner` is the living owner's slot.
    pub(super) fn heading(
        &self,
        world: &Simulator,
        slot: usize,
        tick: u64,
        owner: Option<usize>,
    ) -> Option<f64> {
        let head = world.snakes()[slot].head;
        if self.behavior == BotBehavior::Follow
            && let Some(owner) = owner
        {
            return Some(toward(head, world.snakes()[owner].head));
        }
        self.decides(slot, tick)
            .then(|| toward(head, Point::default()))
    }
}

fn toward(from: Point, to: Point) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}
