//! Virtual-time backend: the authoritative simulator acts as a protocol-19 server
//! and every slot is a client fed only by the bytes delivered to it.
//!
//! Commands and pings arrive over each slot's ordered uplink and apply in that
//! server tick; emitted frames travel its ordered downlink. Variation and pauses
//! delay messages without reordering them. Configured delays stay private:
//! clients measure only pings and their own reply time.
//!
//! Snake/prey coverage approximates a circle of `world.vision` around the head;
//! any centerline part keeps a snake covered. Food has separate retained
//! 300-unit-sector interest. Snakes appear once per entry or life, then receive
//! anchor, tail, mass and motion deltas. Native and remote coverage is shared.
use super::{
    Backend, Bot, ClientFrame, ClientState, ClientStatus, ForagerBot, WireEvent, WireRecord,
    latency::{LatencyConfig, Random},
    presentation::FoodPresentation,
};
use crate::{Action, Simulator, model::Point, spawn};
use std::{collections::BTreeMap, f64::consts::TAU, sync::Arc};

/// Official candidate budget. A failed random search does not prove that no
/// legal pose exists; circular worlds use their larger cached-query budget.
const SPAWN_ATTEMPTS: usize = 64;
/// Undelivered messages per remote connection, counting both directions. Each
/// uplink message yields at most one downlink frame (a pong), so this cap leaves
/// the rest of `connection::MAX_IN_FLIGHT` for broadcasts while the host drains.
pub(super) const REMOTE_IN_FLIGHT: usize = connection::MAX_IN_FLIGHT / 2;
/// Largest framed message a connection is sent.
pub(super) const MAX_FRAME_BYTES: usize = connection::MAX_FRAME_BYTES;
/// Snake IDs are the slots, sent as u16, so 65,536 would fit; but the
/// leaderboard's u16 rank and living population must also count every snake.
pub(crate) const MAX_AGENTS: usize = u16::MAX as usize;
/// Prey IDs are sent as u16; `assign_prey_wire` gives each living prey one.
const MAX_PREY: usize = u16::MAX as usize + 1;
/// Stock-client compatibility: its nickname input keeps 24 characters, while
/// the u8 length field could carry 255 Latin-1 bytes.
pub(crate) const MAX_NICKNAME: usize = 24;
/// Stock-client compatibility: its victory input keeps 140 ASCII characters;
/// the record packet carries the remainder without a length field.
pub(crate) const MAX_VICTORY_TEXT: usize = 140;
static NATIVE_PROFILE: PlayerAppearance = PlayerAppearance {
    nickname: Vec::new(),
    skin: 0,
    custom_skin: Vec::new(),
};

/// Profile a remote player sends at login, repeated in its snake's appearance.
#[derive(Clone, Debug, Default)]
pub(crate) struct PlayerAppearance {
    pub nickname: Vec<u8>,
    pub skin: u8,
    pub custom_skin: Vec<u8>,
}

mod broadcast;
mod config;
mod connection;
mod food;
mod markers;
#[cfg(test)]
mod pit_tests;
mod prey_visibility;
mod remote;
mod snakes;

pub use config::{DeliveryProfile, SimulationConfig};
pub(crate) use remote::RawSimulator;

use broadcast::{PreyClock, Record};
use connection::{Connection, Slot};
use food::{FoodIndex, SectorFood};
use snakes::SnakeReports;

/// Simulator backend whose clients reconstruct frames from protocol-19 bytes.
///
/// `reconnect` drops the old connection's in-flight messages and respawns via
/// `Simulator::respawn_body` at a clear pose drawn from `spawn::spawn_distance`.
/// A living snake is first removed without death food.
pub struct SimulatorBackend {
    config: SimulationConfig,
    player_latency: PlayerLatency,
    world: Simulator,
    slots: Vec<Slot>,
    /// Stateless scratch shared by sequential background decisions, never per slot.
    background_policy: Option<ForagerBot>,
    random: Random,
    spawn_random: Random,
    food_presentation: FoodPresentation,
    /// Exact pickup positions since the last broadcast, before buffers reset.
    food_eaters: BTreeMap<(u64, u64), u16>,
    /// Eater slot by simulator prey ID since the last broadcast.
    prey_eaters: BTreeMap<u64, u16>,
    /// Wire ID of each living prey. Simulator IDs are 64-bit; see `assign_prey_wire`.
    prey_wire: BTreeMap<u64, u16>,
    /// Listings of the subscribed sectors at the last broadcast, indexed by
    /// `u16::from_be_bytes(sector)`. An unchanged listing keeps its allocation,
    /// so a connection already holding it can skip the comparison.
    food_sectors: Vec<Option<Arc<SectorFood>>>,
    /// Identity of the latest shared food snapshot, including failed broadcasts.
    food_revision: Arc<()>,
    /// Socket presentation only; native clients keep the physical listings.
    decorated_food_sectors: Vec<Option<Arc<SectorFood>>>,
    pit_markers: Option<markers::PitMarkers>,
    food_index: FoodIndex,
    /// Food maintenance periods begun since virtual zero.
    food_epoch: u64,
    /// Next minimap and leaderboard periods; see `broadcast::clock_due`.
    next_minimap: u64,
    next_leaderboard: u64,
    prey_clocks: BTreeMap<u64, PreyClock>,
    reports: Vec<SnakeReports>,
    /// Sectors listed at the last broadcast.
    listed: Vec<u16>,
    /// Scratch sector-to-bucket table, all `u32::MAX` between broadcasts.
    food_buckets: Vec<u32>,
    tick: u64,
    /// Leading slots owned by external clients; zero for the native backend.
    remote: usize,
    /// Profiles of the remote slots; native slots use `NATIVE_PROFILE`.
    profiles: Vec<PlayerAppearance>,
    record: Option<Record>,
    /// Connections opened so far; numbers session IDs.
    connections: u32,
    session_key: u32,
}

/// Only hosted socket slots override the simulation's link model. Native
/// backends and raw simulations without an override retain one shared model.
#[derive(Default)]
struct PlayerLatency {
    slots: usize,
    config: LatencyConfig,
}

impl PlayerLatency {
    fn for_slot<'a>(&'a self, slot: usize, simulation: &'a LatencyConfig) -> &'a LatencyConfig {
        if slot < self.slots {
            &self.config
        } else {
            simulation
        }
    }
}

impl SimulatorBackend {
    pub fn new(seed: u32, config: SimulationConfig) -> Result<Self, String> {
        Self::build(seed, config, 0)
    }

    /// The first `remote` slots start absent and unconnected until a remote
    /// client connects; the rest join and connect natively.
    fn build(seed: u32, mut config: SimulationConfig, remote: usize) -> Result<Self, String> {
        // Validate the world that will run, as the host and resets do.
        let population = config.agents - remote;
        config.world.size_for_population(population);
        config.validate_for_population(population)?;
        let ids = (0..config.agents).map(|slot| slot.to_string()).collect();
        let mut world = Simulator::new(crate::WorldOptions {
            seed,
            config: config.world.clone(),
            population: crate::InitialPopulation::Placed {
                ids,
                placement: crate::Placement::TestRing,
            },
        })
        .map_err(|error| error.to_string())?;
        if remote > 0 {
            for slot in 0..config.agents {
                world.retire(slot).map_err(|error| error.to_string())?;
            }
            world.grow_for_seats(population, 1., true);
        }
        world.record_events(crate::EventRecording {
            pickups: true,
            food_births: true,
        });
        let mut backend = Self {
            random: Random::new(u64::from(seed) ^ 0x6c61_7465_6e63_7900),
            spawn_random: Random::new(u64::from(seed) ^ 0x7370_6177_6e00),
            food_presentation: FoodPresentation::new(seed, config.agents),
            food_eaters: BTreeMap::new(),
            prey_eaters: BTreeMap::new(),
            prey_wire: BTreeMap::new(),
            food_sectors: Vec::new(),
            food_revision: Arc::default(),
            decorated_food_sectors: Vec::new(),
            pit_markers: config
                .world
                .pit
                .as_ref()
                .map(|pit| markers::PitMarkers::new(pit.radius))
                .transpose()?,
            food_index: FoodIndex::default(),
            food_epoch: 0,
            next_minimap: 1,
            next_leaderboard: 1,
            prey_clocks: BTreeMap::new(),
            reports: vec![SnakeReports::default(); config.agents],
            listed: Vec::new(),
            food_buckets: Vec::new(),
            slots: Vec::with_capacity(config.agents),
            background_policy: None,
            player_latency: PlayerLatency::default(),
            world,
            config,
            tick: 0,
            remote,
            profiles: vec![PlayerAppearance::default(); remote],
            record: None,
            connections: 0,
            session_key: seed.rotate_left(16) ^ 0x5e55_1d00,
        };
        backend.food_presentation.sync(backend.world.food())?;
        for slot in 0..backend.config.agents {
            let heading = backend.world.snakes()[slot].heading;
            let remote = slot < remote;
            backend.slots.push(Slot {
                state: ClientState::new(),
                connection: Connection::default(),
                life: 0,
                outbox: None,
                processing_override: None,
                previous_boost: None,
                control: Action {
                    heading,
                    boost: false,
                },
                pinged_at: None,
                records: (backend.config.record_wire && !remote).then(Vec::new),
                record_bytes: 0,
                remote,
                living: None,
            });
            if remote {
                continue;
            }
            // A crowded host leaves the slot dead; the normal background
            // lifecycle retries admission after its first death report.
            if backend.remote > 0
                && let Ok((body, heading)) = backend.spawn_pose(slot)
            {
                backend
                    .world
                    .respawn_body(slot, body, heading)
                    .map_err(|error| error.to_string())?;
                backend.slots[slot].control.heading = heading;
            }
            backend.connect(slot)?;
            backend.client_until(slot, 0.)?;
        }
        Ok(backend)
    }

    pub fn config(&self) -> &SimulationConfig {
        &self.config
    }
    /// Local and server virtual time in seconds.
    pub fn time(&self) -> f64 {
        self.tick as f64 * self.world.config().dt
    }
    pub fn slots(&self) -> usize {
        self.slots.len()
    }
    /// Authoritative world for diagnostics and tests. Never a bot input.
    pub fn world(&self) -> &Simulator {
        &self.world
    }

    fn slot(&self, slot: usize) -> Result<(), String> {
        if slot < self.slots.len() {
            Ok(())
        } else {
            Err("Unknown simulation slot".into())
        }
    }

    pub(super) fn check_send(&self, slot: usize, action: Action) -> Result<(), String> {
        self.slot(slot)?;
        if !action.heading.is_finite() {
            return Err("Action heading must be finite".into());
        }
        if self.pending(slot) {
            return Err("Slot is still processing its previous decision".into());
        }
        // Like the network backend, only a client that sees its own snake steers.
        if self.slots[slot].state.status() != ClientStatus::Playing {
            return Err("Slot is not playing".into());
        }
        Ok(())
    }

    fn pending(&self, slot: usize) -> bool {
        let now = self.time();
        let s = &self.slots[slot];
        s.outbox.is_some()
            || s.processing_override
                .is_some_and(|(at, seconds)| at < now && now < at + seconds)
    }

    /// Whether this slot's client is still processing an earlier decision: a
    /// queued action, or a measured duration set for a decision sent no action.
    pub fn decision_pending(&self, slot: usize) -> Result<bool, String> {
        self.slot(slot)?;
        Ok(self.pending(slot))
    }

    /// Queue a decision. It leaves after the slot's processing time, then travels
    /// the uplink. Rejected unless the slot's client is playing and finished
    /// processing its previous decision.
    pub fn send_to(&mut self, slot: usize, action: Action) -> Result<(), String> {
        self.check_send(slot, action)?;
        let now = self.time();
        let s = &mut self.slots[slot];
        let processing = match s.processing_override.take() {
            Some((at, seconds)) if at == now => seconds,
            _ => s
                .connection
                .link
                .processing(&self.config.latency, &mut self.random),
        };
        s.outbox = Some((now, action, processing));
        self.client_until(slot, now)
    }

    /// Report the measured duration of the decision made now. An action sent
    /// at this same time departs after it instead of the configured draw; with
    /// no action the slot stays busy for it. The client knows it like its ping.
    pub fn set_slot_processing_seconds(&mut self, slot: usize, seconds: f64) -> Result<(), String> {
        self.slot(slot)?;
        if !seconds.is_finite() || seconds < 0. {
            return Err("Processing must be finite and nonnegative".into());
        }
        if self.pending(slot) {
            return Err("Slot is still processing its previous decision".into());
        }
        let now = self.time();
        let s = &mut self.slots[slot];
        s.state.set_processing_seconds(seconds)?;
        s.processing_override = Some((now, seconds));
        Ok(())
    }
    pub fn set_processing_seconds(&mut self, seconds: f64) -> Result<(), String> {
        self.set_slot_processing_seconds(self.config.controlled_slot, seconds)
    }

    pub fn advance_ticks(&mut self, ticks: usize) -> Result<(), String> {
        for _ in 0..ticks {
            self.advance()?;
        }
        Ok(())
    }

    /// The controlled slot's client frame at the current time.
    pub fn frame(&self) -> Result<ClientFrame, String> {
        self.frame_for(self.config.controlled_slot)
    }
    /// A slot's client frame at the current time. Polling processes nothing:
    /// only advancing time delivers frames or sends messages.
    pub fn frame_for(&self, slot: usize) -> Result<ClientFrame, String> {
        self.slot(slot)?;
        self.slots[slot].state.frame(self.time())
    }

    /// Send `Some` actions, hold the previous request for `None`, advance one
    /// decision and return every slot's frame. No slot is driven automatically.
    /// The whole batch is rejected if a `Some` targets a slot whose previous
    /// decision is pending; pass `None` for it until `decision_pending` clears.
    /// A duration set before a `None` keeps that slot pending until it elapses.
    pub fn step_all(&mut self, actions: &[Option<Action>]) -> Result<Vec<ClientFrame>, String> {
        if actions.len() != self.slots.len() {
            return Err("Action batch must have one entry per slot".into());
        }
        for (slot, action) in actions.iter().enumerate() {
            if let Some(action) = action {
                self.check_send(slot, *action)?;
            }
        }
        for (slot, action) in actions.iter().enumerate() {
            if let Some(action) = action {
                self.send_to(slot, *action)?;
            }
        }
        self.advance_ticks(self.config.decision_ticks)?;
        (0..self.slots.len())
            .map(|slot| self.frame_for(slot))
            .collect()
    }

    pub fn drain_records(&mut self, slot: usize) -> Result<Vec<WireRecord>, String> {
        self.slot(slot)?;
        let s = &mut self.slots[slot];
        s.record_bytes = 0;
        s.records
            .as_mut()
            .map(std::mem::take)
            .ok_or_else(|| "Wire recording is disabled".into())
    }

    /// Close this slot's connection and play a new life on a new one.
    pub fn reconnect_slot(&mut self, slot: usize) -> Result<(), String> {
        self.slot(slot)?;
        self.slots[slot].room([0, 0])?;
        let now = self.time();
        self.client_until(slot, now)?;
        // Deliveries and departures may have consumed the lifecycle records' room.
        self.slots[slot].room([0, 0])?;
        let (body, heading) = self.spawn_pose(slot)?;
        let s = &mut self.slots[slot];
        s.state.disconnect();
        s.record(now, WireEvent::Disconnect, &[])?;
        s.living = None;
        self.world.retire(slot).map_err(|error| error.to_string())?;
        self.world
            .respawn_body(slot, body, heading)
            .map_err(|error| error.to_string())?;
        let s = &mut self.slots[slot];
        s.life += 1;
        s.control = Action {
            heading,
            boost: false,
        };
        s.outbox = None;
        s.processing_override = None;
        s.previous_boost = None;
        s.pinged_at = None;
        self.connect(slot)?;
        self.client_until(slot, now)
    }

    /// Body and heading for a new life, chosen with this slot treated as
    /// absent; `Simulator::spawn_body` shapes it. The first candidate clear by
    /// 600 units wins. When none is, the candidate with the largest
    /// `Simulator::spawn_margin` does: no official join has been seen refused,
    /// and refusing would disconnect a socket player. That candidate must still
    /// clear every contact range, or the first tick could kill the newborn or
    /// its neighbor. Failure exhausts the configured candidate search, which
    /// does not establish that no legal pose exists. Circular worlds instead
    /// require full body/head-path clearance and wait without a tight fallback.
    /// Evidence:
    /// research/fidelity/2026-10-25-spawn-admission/REPORT.md.
    fn spawn_pose(&mut self, slot: usize) -> Result<(Vec<Point>, f64), String> {
        let circular = self.world.config().spawn_radius.is_some();
        let mut safety = None;
        let mut open: Option<(f64, Vec<Point>, f64)> = None;
        let attempts = if circular {
            spawn::CIRCULAR_ATTEMPTS
        } else {
            SPAWN_ATTEMPTS
        };
        for _ in 0..attempts {
            let distance = spawn::spawn_distance(
                self.world.config(),
                self.world.radius(),
                self.spawn_random.unit(),
            );
            let angle = self.spawn_random.unit() * TAU;
            let heading = self.spawn_random.unit() * TAU;
            let head = Point {
                x: angle.cos() * distance,
                y: angle.sin() * distance,
            };
            // Local policy: official joins keep a median 925 units from
            // visible bodies, but three came within 550-600.
            let (body, heading) = self
                .world
                .spawn_body(head, heading)
                .map_err(|error| error.to_string())?;
            let clear = if circular {
                safety
                    .get_or_insert_with(|| {
                        spawn::Safety::new_excluding(
                            &self.world,
                            Some(slot),
                            body.len(),
                            spawn::CIRCULAR_CLEARANCE,
                            spawn::HEAD_PATH_SECONDS,
                        )
                    })
                    .clears(&body)
            } else {
                self.world
                    .body_is_clear_excluding(&body, spawn::CLEARANCE, Some(slot))
            };
            if clear && (!circular || safety.as_ref().unwrap().clears_departures(body[0], heading))
            {
                return Ok((body, heading));
            }
            if circular {
                continue;
            }
            // A candidate's margin only decreases while scanning neighbors.
            // Stop once it cannot beat zero (admission) or the incumbent;
            // keep strict ties and all random draws unchanged. See:
            // research/performance/2026-10-25-battledome-spawn-cost/REPORT.md.
            let best = open.as_ref().map_or(0., |(best, ..)| best.max(0.));
            if let Some(margin) = self
                .world
                .spawn_margin_bounded_excluding(&body, best, Some(slot))
                && open.as_ref().is_none_or(|(best, ..)| margin > *best)
            {
                open = Some((margin, body, heading));
            }
        }
        if circular {
            return Err(spawn::WAITING.into());
        }
        match open {
            Some((margin, body, heading)) if margin > 0. => Ok((body, heading)),
            Some(_) => Err(
                "Spawn candidate search exhausted: no sampled position is clear of other snakes"
                    .into(),
            ),
            None => Err(
                "Spawn candidate search exhausted: no sampled position fits inside the arena"
                    .into(),
            ),
        }
    }

    /// Drive non-controlled slots with the shared policy over their own frames.
    fn drive_background(&mut self) -> Result<(), String> {
        for slot in 0..self.slots.len() {
            // Remote slots replace the controlled slot; they are never driven.
            let driven = if self.remote > 0 {
                slot >= self.remote
            } else {
                slot != self.config.controlled_slot
            };
            if !driven {
                continue;
            }
            match self.slots[slot].state.status() {
                ClientStatus::Dead => {
                    // A crowded arena may have no clear pose yet; retry next decision.
                    let _ = self.reconnect_slot(slot);
                }
                ClientStatus::Playing if !self.pending(slot) => {
                    let frame = self.frame_for(slot)?;
                    if let Some(action) = self
                        .background_policy
                        .get_or_insert_with(ForagerBot::new)
                        .decide(&frame)
                    {
                        self.send_to(slot, action)?;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// Receives `(slot, frames)` for early slots during a tick; see
/// `RawSimulator::advance_tick_with`.
pub(crate) type EarlyFrames<'a> =
    dyn FnMut(Vec<(usize, Vec<(f64, Vec<u8>)>)>) -> Result<(), String> + 'a;

impl Backend for SimulatorBackend {
    /// Advance one decision interval, and further while the controlled client is
    /// still processing, so a bot never decides faster than its virtual reply
    /// time, whether or not its last decision sent an action.
    fn next_frame(&mut self) -> Result<ClientFrame, String> {
        self.drive_background()?;
        self.advance_ticks(self.config.decision_ticks)?;
        while self.pending(self.config.controlled_slot) {
            self.advance()?;
        }
        self.frame()
    }
    fn send_action(&mut self, action: Action) -> Result<(), String> {
        self.send_to(self.config.controlled_slot, action)
    }
    fn reconnect(&mut self) -> Result<(), String> {
        self.reconnect_slot(self.config.controlled_slot)
    }
    fn set_processing_seconds(&mut self, seconds: f64) -> Result<(), String> {
        SimulatorBackend::set_processing_seconds(self, seconds)
    }
}

/// Game commands a playing client may send: heading bytes 0..=250, ping 251,
/// keyboard turns `[252, amount]` and boost 253/254.
pub(crate) fn client_message(bytes: &[u8]) -> bool {
    matches!(bytes, [0..=251 | 253 | 254] | [252, _])
}

#[cfg(test)]
mod bot_tests;

#[cfg(test)]
mod tests;
