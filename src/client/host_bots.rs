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
use std::{collections::BTreeMap, fmt, str::FromStr};

/// What a host bot slot does.
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

impl BotBehavior {
    pub const ALL: [Self; 3] = [Self::Forager, Self::Center, Self::Follow];

    /// The configuration and console name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Forager => "forager",
            Self::Center => "center",
            Self::Follow => "follow",
        }
    }
}

impl fmt::Display for BotBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.name())
    }
}

impl FromStr for BotBehavior {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|behavior| behavior.name() == name)
            .ok_or_else(|| format!("Unknown bot behavior {name}; use forager, center or follow"))
    }
}

/// Center bots re-aim and dead bots respawn on staggered six-tick decisions.
const DECISION_TICKS: u64 = 6;

/// The world-thread behavior of each assigned bot slot. Slots are kept in
/// ascending order, so driving them is deterministic.
pub(super) struct InWorldBots {
    /// The first bot slot; decisions stagger from it.
    first: usize,
    behaviors: BTreeMap<usize, BotBehavior>,
}

impl InWorldBots {
    pub(super) fn new(first: usize) -> Self {
        Self {
            first,
            behaviors: BTreeMap::new(),
        }
    }

    pub(super) fn assign(&mut self, slot: usize, behavior: BotBehavior) {
        debug_assert!(slot >= self.first && behavior != BotBehavior::Forager);
        self.behaviors.insert(slot, behavior);
    }

    pub(super) fn unassign(&mut self, slot: usize) -> bool {
        self.behaviors.remove(&slot).is_some()
    }

    pub(super) fn is_empty(&self) -> bool {
        self.behaviors.is_empty()
    }

    /// Assigned slots and their behaviors in ascending slot order.
    pub(super) fn slots(&self) -> impl Iterator<Item = (usize, BotBehavior)> + '_ {
        self.behaviors
            .iter()
            .map(|(&slot, &behavior)| (slot, behavior))
    }

    /// Staggered decision ticks for aiming and respawning.
    pub(super) fn decides(&self, slot: usize, tick: u64) -> bool {
        (tick + (slot - self.first) as u64).is_multiple_of(DECISION_TICKS)
    }

    /// The wanted heading for a living bot this tick, or `None` to keep the
    /// last one. `owner` is the living owner's slot.
    pub(super) fn heading(
        &self,
        world: &Simulator,
        slot: usize,
        behavior: BotBehavior,
        tick: u64,
        owner: Option<usize>,
    ) -> Option<f64> {
        let head = world.snakes()[slot].head;
        if behavior == BotBehavior::Follow
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
