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
    Backend, ClientFrame, ClientState, ClientStatus, WireEvent, WireRecord, latency::Random,
    presentation::FoodPresentation,
};
use crate::{Action, Simulator, model::Point, spawn};
use std::{collections::BTreeMap, f64::consts::TAU, sync::Arc};

/// Clear-pose draws per spawn before it is deferred. Placement policy: a clear
/// pose may exist after this many failures.
const SPAWN_ATTEMPTS: usize = 64;
/// Undelivered messages per remote connection, counting both directions. Each
/// uplink message yields at most one downlink frame (a pong), so this cap leaves
/// the rest of `connection::MAX_IN_FLIGHT` for broadcasts while the host drains.
pub(super) const REMOTE_IN_FLIGHT: usize = connection::MAX_IN_FLIGHT / 2;
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
mod remote;
mod snakes;

pub use config::SimulationConfig;
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
    world: Simulator,
    slots: Vec<Slot>,
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

impl SimulatorBackend {
    pub fn new(seed: u32, config: SimulationConfig) -> Result<Self, String> {
        Self::build(seed, config, 0)
    }

    /// The first `remote` slots start absent and unconnected until a remote
    /// client connects; the rest join and connect natively.
    fn build(seed: u32, config: SimulationConfig, remote: usize) -> Result<Self, String> {
        config.validate()?;
        let mut world = Simulator::new(seed, config.world.clone())?;
        world.food_spawn_events = Some(Vec::new());
        world.pickup_events = Some(Vec::new());
        world.add_slots(config.agents)?;
        for snake in &mut world.snakes {
            // Native experiments retain their synthetic ring initialization.
            // Hosted lives enter through the same admission rule as rejoins.
            snake.alive = remote == 0;
        }
        let mut backend = Self {
            random: Random::new(u64::from(seed) ^ 0x6c61_7465_6e63_7900),
            spawn_random: Random::new(u64::from(seed) ^ 0x7370_6177_6e00),
            food_presentation: FoodPresentation::new(seed, config.agents),
            food_eaters: BTreeMap::new(),
            prey_eaters: BTreeMap::new(),
            prey_wire: BTreeMap::new(),
            food_sectors: Vec::new(),
            food_index: FoodIndex::default(),
            food_epoch: 0,
            next_minimap: 1,
            next_leaderboard: 1,
            prey_clocks: BTreeMap::new(),
            reports: vec![SnakeReports::default(); config.agents],
            listed: Vec::new(),
            food_buckets: Vec::new(),
            slots: Vec::with_capacity(config.agents),
            world,
            config,
            tick: 0,
            remote,
            profiles: vec![PlayerAppearance::default(); remote],
            record: None,
            connections: 0,
            session_key: seed.rotate_left(16) ^ 0x5e55_1d00,
        };
        backend.food_presentation.sync(&backend.world.food)?;
        for slot in 0..backend.config.agents {
            let heading = backend.world.snakes[slot].heading;
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
                seeker: None,
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
                backend.world.respawn_body(slot, body, heading)?;
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
        self.tick as f64 * self.world.config.dt
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
        self.world.snakes[slot].alive = false;
        self.world.respawn_body(slot, body, heading)?;
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
        s.seeker = None;
        self.connect(slot)?;
        self.client_until(slot, now)
    }

    /// Body and heading for a new life, chosen with this slot treated as
    /// absent; `Simulator::spawn_body` shapes it. The first candidate clear by
    /// 600 units wins. When none is, the candidate with the largest
    /// `Simulator::spawn_margin` does: no official join has been seen refused,
    /// and refusing would disconnect a socket player. That candidate must still
    /// clear every contact range, or the first tick could kill the newborn or
    /// its neighbor; only an arena too full for that refuses. Evidence:
    /// research/fidelity/2026-10-25-spawn-admission/REPORT.md.
    fn spawn_pose(&mut self, slot: usize) -> Result<(Vec<Point>, f64), String> {
        let alive = self.world.snakes[slot].alive;
        self.world.snakes[slot].alive = false;
        let pose = (|| {
            let mut open: Option<(f64, Vec<Point>, f64)> = None;
            for _ in 0..SPAWN_ATTEMPTS {
                let distance = spawn::spawn_distance(&self.world.config, self.spawn_random.unit());
                let angle = self.spawn_random.unit() * TAU;
                let heading = self.spawn_random.unit() * TAU;
                let head = Point {
                    x: angle.cos() * distance,
                    y: angle.sin() * distance,
                };
                // Local policy: official joins keep a median 925 units from
                // visible bodies, but three came within 550-600.
                let (body, heading) = self.world.spawn_body(head, heading)?;
                if self.world.body_is_clear(&body, 600.) {
                    return Ok((body, heading));
                }
                if let Some(margin) = self.world.spawn_margin(&body)
                    && open.as_ref().is_none_or(|(best, ..)| margin > *best)
                {
                    open = Some((margin, body, heading));
                }
            }
            match open {
                Some((margin, body, heading)) if margin > 0. => Ok((body, heading)),
                Some(_) => Err("No spawn position is clear of other snakes".into()),
                None => Err("No spawn position fits inside the arena".into()),
            }
        })();
        self.world.snakes[slot].alive = alive;
        pose
    }

    /// Drive non-controlled slots with a food seeker over their own frames.
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
                    if let Some(action) = seeker_action(&frame, &mut self.slots[slot].seeker) {
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

/// The background seeker's decision. It sends only when its heading byte
/// changes; `sent` holds the last byte sent on this connection.
pub(crate) fn seeker_action(frame: &ClientFrame, sent: &mut Option<u8>) -> Option<Action> {
    let heading = seek(frame)?;
    let byte = (heading.rem_euclid(TAU) * 251. / TAU).floor() as u8;
    if *sent == Some(byte) {
        return None;
    }
    *sent = Some(byte);
    Some(Action {
        heading,
        boost: false,
    })
}

/// Turn inward near the rim, otherwise toward the nearest received food.
fn seek(frame: &ClientFrame) -> Option<f64> {
    let observation = frame.observation.as_ref()?;
    let head = observation.me.head;
    let arena = &observation.arena;
    let from_center = head.distance(arena.center);
    if from_center > arena.radius * 0.8 {
        return Some((arena.center.y - head.y).atan2(arena.center.x - head.x));
    }
    observation
        .food
        .iter()
        .min_by(|a, b| {
            a.point()
                .distance(head)
                .total_cmp(&b.point().distance(head))
        })
        .map(|food| (food.y - head.y).atan2(food.x - head.x))
}

#[cfg(test)]
mod tests;
