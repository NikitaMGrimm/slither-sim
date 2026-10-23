//! Virtual-time backend: the authoritative simulator acts as a protocol-19 server
//! and every slot is a client fed only by the bytes delivered to it.
//!
//! Each slot owns one connection. Commands and pings travel its uplink and are
//! applied at the next server tick after arrival; frames emitted at a tick travel
//! its downlink. Both directions keep their order like one TCP stream, so jitter
//! and congestion delay messages but never reorder them. Configured delays stay
//! private: a client only measures its pings and knows its own reply time.
//!
//! Snake/prey visibility uses approximate circular coverage: a
//! snake is covered while any part of its centerline is within `world.vision` of
//! the client's head, and prey within it. Food uses a separate retained
//! 300-unit-sector interest model. A covered snake appears once per entry or
//! life; afterwards only changed anchors, tail removals, mass and motion fields
//! are sent. Native and remote recipients use the same coverage policies.
use super::{
    Backend, ClientFrame, ClientState, ClientStatus, Minimap, WireEvent, WireRecord, encode,
    latency::{LatencyConfig, Link, Random},
    presentation::FoodPresentation,
    protocol,
    visibility::SectorInterest,
};
use crate::{
    Action, Config, Simulator,
    model::{Arena, Origin, Point, Snake},
    physics,
    simulator::PickupItem,
    spawn,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    f64::consts::TAU,
};

/// Messages in flight per direction of one connection before the run fails.
const MAX_IN_FLIGHT: usize = 4096;
/// Fit the host's 1 MiB send buffer and native client's message limit, leaving
/// room for the largest unmasked WebSocket header.
const MAX_FRAME_BYTES: usize = (1 << 20) - 10;
/// Retained wire bytes per slot before the run fails; drain to continue. Each
/// record also counts a fixed overhead so empty events stay bounded.
const MAX_RECORD_BYTES: usize = 32 << 20;
const RECORD_OVERHEAD: usize = 32;
/// New anchors per broadcast that keep an exact incremental encoding; the
/// fourth-newest anchor has already moved by the body contraction.
const FRESH_ANCHORS: usize = 3;
const SPAWN_ATTEMPTS: usize = 64;
/// Undelivered messages per remote connection, counting both directions. Each
/// uplink message yields at most one downlink frame (a pong), so this cap leaves
/// the rest of `MAX_IN_FLIGHT` for broadcasts while the host drains.
const REMOTE_IN_FLIGHT: usize = MAX_IN_FLIGHT / 2;
const MAX_NICKNAME: usize = 24;
/// Rows the stock leaderboard shows.
const LEADERBOARD_ROWS: usize = 10;
/// The stock client's victory input keeps 140 ASCII characters.
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct SimulationConfig {
    pub world: Config,
    /// Snakes joined at start on the deterministic test ring of `Simulator::add`.
    pub agents: usize,
    pub controlled_slot: usize,
    /// Physics ticks between `next_frame`/`step_all` decisions.
    pub decision_ticks: usize,
    /// Physics ticks between update frames.
    pub broadcast_ticks: usize,
    pub minimap_interval_seconds: f64,
    pub minimap_side: usize,
    /// Seconds between leaderboards to each playing connection, starting with
    /// its own appearance. A local schedule, not a measured official interval.
    pub leaderboard_interval_seconds: f64,
    /// Offer a world-run record and victory entry. This local rule is not an
    /// established official-server qualification policy.
    pub local_records: bool,
    pub latency: LatencyConfig,
    /// Retain each slot's connect/send/receive/disconnect records for replay.
    pub record_wire: bool,
}
impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            world: Config::default(),
            agents: 1,
            controlled_slot: 0,
            decision_ticks: 6,
            broadcast_ticks: 6,
            minimap_interval_seconds: 2.5,
            // Retained protocol-19 minimap receipts and the earlier browser
            // bridge use 136 cells per side; 32 was a local backend shortcut.
            minimap_side: 136,
            leaderboard_interval_seconds: 3.,
            local_records: false,
            latency: LatencyConfig::default(),
            record_wire: false,
        }
    }
}
impl SimulationConfig {
    pub fn validate(&self) -> Result<(), String> {
        self.world.validate()?;
        self.latency.validate()?;
        for speed in [
            self.world.prey.cruise_speed,
            self.world.prey.escape_speed,
            self.world.prey.sprint_speed,
        ] {
            encode::speed_u16(speed).map_err(|error| format!("Prey {error}"))?;
        }
        if !(1..=128).contains(&self.agents) || self.controlled_slot >= self.agents {
            return Err("Simulation needs 1 to 128 agents and a controlled slot among them".into());
        }
        if !(1..=1000).contains(&self.decision_ticks) || !(1..=1000).contains(&self.broadcast_ticks)
        {
            return Err("Decision and broadcast ticks must be 1 through 1000".into());
        }
        // Anchors farther behind the front have been contracted and cannot be
        // encoded incrementally; boost speed must not outrun that window.
        if self.broadcast_ticks as f64 * self.world.dt * physics::BOOST_SPEED
            >= (FRESH_ANCHORS - 1) as f64 * physics::ANCHOR_SPACING
        {
            return Err("Broadcast interval is too long for incremental body updates".into());
        }
        if !self.minimap_interval_seconds.is_finite() || self.minimap_interval_seconds <= 0. {
            return Err("Minimap interval must be positive".into());
        }
        if !self.leaderboard_interval_seconds.is_finite() || self.leaderboard_interval_seconds <= 0.
        {
            return Err("Leaderboard interval must be positive".into());
        }
        if !(1..=512).contains(&self.minimap_side) {
            return Err("Minimap side must be 1 through 512".into());
        }
        let largest = self
            .world
            .radius_updates
            .iter()
            .map(|update| update.radius)
            .chain([self.world.radius])
            .chain(
                self.world
                    .dynamic_radius
                    .then(|| crate::simulator::population_radius(self.agents)),
            )
            .fold(0., f64::max);
        if largest > encode::MAX_RADIUS {
            return Err(format!(
                "Protocol-19 simulation supports arena radii up to {}",
                encode::MAX_RADIUS
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
enum Uplink {
    Connect,
    Message(Vec<u8>),
    /// Victory text for the record life; still read after game over.
    Victory(Vec<u8>),
}

/// What one connection has been told about a snake, in wire terms.
#[derive(Clone, Debug)]
struct KnownSnake {
    life: u64,
    front: Point,
    len: usize,
    heading: u8,
    wanted: u8,
    speed: u8,
    fam: u32,
    kills: usize,
}
#[derive(Clone, Debug, PartialEq)]
struct KnownPrey {
    grid: [u16; 2],
    motion: [u32; 3],
}

/// The physical identity disambiguates food collapsed onto the same wire grid.
#[derive(Clone, Copy, Debug, PartialEq)]
struct KnownFood {
    point: Point,
    style: encode::FoodStyle,
}

/// Server and transport state of one connection.
#[derive(Clone, Debug, Default)]
struct Connection {
    /// Opened by a connect; a closed remote slot has no session.
    open: bool,
    link: Link,
    uplink: VecDeque<(f64, Uplink)>,
    downlink: VecDeque<(f64, Vec<u8>)>,
    /// The server has received the connect and sent setup.
    setup: bool,
    /// Game over was sent; afterwards only pings and victory text are accepted.
    over: bool,
    /// Setup emission time and milliseconds of elapsed headers sent since then.
    /// Headers difference quantized absolute times, so rounding never drifts.
    setup_at: Option<f64>,
    header_ms: u64,
    snakes: BTreeMap<usize, KnownSnake>,
    food: BTreeMap<[u8; 2], BTreeMap<[u8; 2], KnownFood>>,
    food_interest: SectorInterest,
    prey: BTreeMap<u16, KnownPrey>,
    radius: u32,
    minimap_at: f64,
    /// Next leaderboard, scheduled once the connection's own snake appears.
    leaderboard_at: Option<f64>,
    /// Rank frozen when the connection's snake died.
    final_rank: Option<usize>,
    /// Version of the session record last sent; zero before any.
    record_version: u64,
    /// Score credit last sent for the connection's own snake.
    credit: u8,
    /// Opaque per-connection session ID, unique across slot reuse.
    session: u32,
    /// The client asked for its session ID; `session_pending` until it is sent.
    session_requested: bool,
    session_pending: bool,
}

/// Highest-scoring completed life in this world run. It names the slot and
/// life so a later session in a reused slot cannot change it.
#[derive(Clone, Debug)]
struct Record {
    slot: usize,
    life: u64,
    segments: usize,
    fam: u32,
    score: f64,
    name: Vec<u8>,
    message: Vec<u8>,
    message_set: bool,
    version: u64,
}

/// Living snakes ranked once per broadcast that needs a leaderboard: wire
/// display score descending, then slot ascending.
struct Board {
    /// `(display score, slot)` in rank order.
    order: Vec<(f64, usize)>,
    /// Encoded top rows shared by every connection's packet.
    rows: Vec<u8>,
}
impl Board {
    fn new(world: &Simulator, profiles: &[PlayerAppearance]) -> Result<Self, String> {
        let mut order: Vec<(f64, usize)> = world
            .snakes
            .iter()
            .enumerate()
            .filter(|(_, snake)| snake.alive)
            .map(|(slot, snake)| (wire_score(snake), slot))
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut rows = Vec::new();
        for &(_, slot) in order.iter().take(LEADERBOARD_ROWS) {
            let (segments, fam, _) = represented(&world.snakes[slot]);
            let (color, name) = match profiles.get(slot) {
                Some(profile) => (profile.skin, profile.nickname.clone()),
                None => (0, slot.to_string().into_bytes()),
            };
            encode::leaderboard_row(&mut rows, segments, fam, color, &name)?;
        }
        Ok(Self { order, rows })
    }
    /// One-based rank a snake with this final score would hold among the living.
    fn rank(&self, score: f64, slot: usize) -> usize {
        1 + self
            .order
            .partition_point(|&(other, i)| other > score || (other == score && i < slot))
    }
}

/// Wire representation of a snake's score: segments plus the byte credit the
/// stock client adds to its own snake (`R`), the quantized fam and the display
/// score they produce. Leaderboards and records rank and show this score; its
/// precision is bounded by whole credited segments.
fn represented(snake: &Snake) -> (usize, u32, f64) {
    let fam = encode::fam_u24(snake.fam);
    let credit = encode::segment_credit(snake.score, snake.segments, fam);
    let segments = snake.segments + usize::from(credit);
    (segments, fam, encode::display_score(segments, fam))
}
fn wire_score(snake: &Snake) -> f64 {
    represented(snake).2
}

struct Slot {
    state: ClientState,
    connection: Connection,
    life: u64,
    /// Command decided at `.0`, leaving the client after `.2` seconds of processing.
    outbox: Option<(f64, Action, f64)>,
    /// Measured processing of the decision made at `.0`. It replaces the
    /// configured draw for an action sent at that same time; a decision without
    /// an action keeps the slot busy until `.0 + .1`.
    processing_override: Option<(f64, f64)>,
    previous_boost: Option<bool>,
    /// Command in force on the server.
    control: Action,
    pinged_at: Option<f64>,
    records: Option<Vec<WireRecord>>,
    record_bytes: usize,
    /// Heading byte last requested by the built-in background seeker.
    seeker: Option<u8>,
    /// An external client exchanges this slot's raw payloads; the in-process
    /// decoder, pings, records and processing model stay idle.
    remote: bool,
    /// Life seen alive at the last tick; its later absence is a completed life
    /// unless a disconnect or respawn cleared it first.
    living: Option<u64>,
}
impl Slot {
    /// Fail before events are applied that their records could not retain.
    fn room(&self, sizes: impl IntoIterator<Item = usize>) -> Result<(), String> {
        let needed: usize = sizes.into_iter().map(|size| size + RECORD_OVERHEAD).sum();
        if self.records.is_some() && self.record_bytes + needed > MAX_RECORD_BYTES {
            return Err("Wire record buffer is full; drain records first".into());
        }
        Ok(())
    }
    /// Fail before sending messages the uplink queue could not hold.
    fn uplink_room(&self, messages: usize) -> Result<(), String> {
        if self.connection.uplink.len() + messages > MAX_IN_FLIGHT {
            return Err("Uplink queue is full".into());
        }
        Ok(())
    }
    fn record(&mut self, time: f64, event: WireEvent, bytes: &[u8]) -> Result<(), String> {
        self.room([bytes.len()])?;
        if let Some(records) = &mut self.records {
            self.record_bytes += bytes.len() + RECORD_OVERHEAD;
            records.push(WireRecord {
                time,
                event,
                bytes: bytes.to_vec(),
            });
        }
        Ok(())
    }
}

/// Simulator backend whose clients reconstruct frames from protocol-19 bytes.
///
/// `reconnect` opens a new connection and respawns the slot with the core
/// `Simulator::respawn` at a clear pose drawn from `spawn::sample_spawn_distance`;
/// a living snake is first removed without death food. In-flight messages of the
/// old connection are dropped.
pub struct SimulatorBackend {
    config: SimulationConfig,
    world: Simulator,
    slots: Vec<Slot>,
    random: Random,
    spawn_random: Random,
    food_presentation: FoodPresentation,
    /// Exact pickup positions since the last broadcast, before buffers reset.
    food_eaters: BTreeMap<(u64, u64), u16>,
    prey_eaters: BTreeMap<u16, u16>,
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
        for slot in 0..config.agents {
            world.add(slot.to_string(), slot, config.agents)?;
            // Native experiments retain their synthetic ring initialization.
            // Hosted lives enter through the same admission rule as rejoins.
            world.snakes[slot].alive = remote == 0;
        }
        let mut backend = Self {
            random: Random::new(u64::from(seed) ^ 0x6c61_7465_6e63_7900),
            spawn_random: Random::new(u64::from(seed) ^ 0x7370_6177_6e00),
            food_presentation: FoodPresentation::new(seed, config.agents),
            food_eaters: BTreeMap::new(),
            prey_eaters: BTreeMap::new(),
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
            if backend.remote > 0 {
                if let Ok((head, heading)) = backend.spawn_pose(slot) {
                    backend.world.respawn(slot, head, heading, 2)?;
                    backend.slots[slot].control.heading = heading;
                }
                // A crowded host leaves the slot dead; the normal background
                // lifecycle retries admission after its first death report.
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
        if !seconds.is_finite() || !(0. ..=2.).contains(&seconds) {
            return Err("Processing must be 0 to 2 seconds".into());
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
        let (head, heading) = self.spawn_pose(slot)?;
        let s = &mut self.slots[slot];
        s.state.disconnect();
        s.record(now, WireEvent::Disconnect, &[])?;
        s.living = None;
        self.world.snakes[slot].alive = false;
        self.world.respawn(slot, head, heading, 2)?;
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

    /// Clear pose for a new life, chosen with this slot treated as absent.
    fn spawn_pose(&mut self, slot: usize) -> Result<(Point, f64), String> {
        let alive = self.world.snakes[slot].alive;
        self.world.snakes[slot].alive = false;
        let radius = self.world.config.radius;
        let mut pose = None;
        for _ in 0..SPAWN_ATTEMPTS {
            let distance = spawn::sample_spawn_distance(radius, self.spawn_random.unit());
            let angle = self.spawn_random.unit() * TAU;
            let heading = self.spawn_random.unit() * TAU;
            let head = Point {
                x: angle.cos() * distance,
                y: angle.sin() * distance,
            };
            // Restore the earlier browser lobby's separation model. Own-join
            // receipts support a large exclusion zone, but do not identify the
            // exact official test. Never fall back to near-contact placement
            // when the compact arena is crowded: defer this spawn instead.
            if self.world.spawn_is_clear(head, heading, 2, 600.) {
                pose = Some((head, heading));
                break;
            }
        }
        self.world.snakes[slot].alive = alive;
        pose.ok_or_else(|| "No clear spawn position".into())
    }

    fn connect(&mut self, slot: usize) -> Result<(), String> {
        self.slots[slot].room([0])?;
        let now = self.time();
        let appearance = self.profiles.get(slot).unwrap_or(&NATIVE_PROFILE);
        self.food_presentation
            .register(slot, appearance.skin, &appearance.custom_skin);
        let latency = &self.config.latency;
        self.connections = self.connections.wrapping_add(1);
        let mut connection = Connection {
            open: true,
            session: session_id(self.connections, self.session_key),
            link: Link::connect(latency, &mut self.random),
            ..Default::default()
        };
        connection.link.update(latency, &mut self.random, now);
        let arrival = connection.link.uplink(latency, &mut self.random, now);
        connection.uplink.push_back((arrival, Uplink::Connect));
        let s = &mut self.slots[slot];
        s.connection = connection;
        if s.remote {
            return Ok(());
        }
        s.state.new_connection(now)?;
        s.record(now, WireEvent::Connect, &[])
    }

    /// Client side up to `now`: deliveries and departures in time order, then a
    /// ping if the client's own cadence asks for one.
    fn client_until(&mut self, slot: usize, now: f64) -> Result<(), String> {
        // A remote client drains its own frames and sends its own pings.
        if self.slots[slot].remote {
            return Ok(());
        }
        loop {
            let s = &self.slots[slot];
            let frame = s
                .connection
                .downlink
                .front()
                .map(|(at, _)| *at)
                .filter(|at| *at <= now);
            let leave = s
                .outbox
                .map(|(at, _, processing)| at + processing)
                .filter(|at| *at <= now);
            match (frame, leave) {
                (Some(at), leave) if leave.is_none_or(|leave| at <= leave) => {
                    let s = &mut self.slots[slot];
                    // A full record buffer leaves the frame queued, undelivered.
                    s.room([s.connection.downlink[0].1.len()])?;
                    let (_, bytes) = s.connection.downlink.pop_front().unwrap();
                    s.state.receive_frame(&bytes, at)?;
                    s.record(at, WireEvent::Receive, &bytes)?;
                }
                (_, Some(at)) => self.transmit(slot, at)?,
                _ => break,
            }
        }
        let s = &mut self.slots[slot];
        if s.pinged_at.is_none_or(|at| at < now) {
            s.pinged_at = Some(now);
            if s.state.ping_due(now)? {
                s.room([1])?;
                s.uplink_room(1)?;
                s.state.record_sent(&[251], now)?;
                s.record(now, WireEvent::Send, &[251])?;
                self.uplink(slot, now, Uplink::Message(vec![251]))?;
            }
        }
        Ok(())
    }

    fn transmit(&mut self, slot: usize, at: f64) -> Result<(), String> {
        let s = &mut self.slots[slot];
        let Some((_, action, processing)) = s.outbox else {
            return Ok(());
        };
        let packets = protocol::action_packets(action, s.previous_boost)?;
        // Reserve everything first: a rejected departure stays queued with the
        // client's sent history, boost state and server control untouched.
        s.room(packets.iter().map(Vec::len))?;
        s.uplink_room(packets.len())?;
        s.outbox = None;
        s.previous_boost = Some(action.boost);
        s.state.set_processing_seconds(processing)?;
        for packet in packets {
            let s = &mut self.slots[slot];
            s.state.record_sent(&packet, at)?;
            s.record(at, WireEvent::Send, &packet)?;
            self.uplink(slot, at, Uplink::Message(packet))?;
        }
        Ok(())
    }

    fn uplink(&mut self, slot: usize, sent: f64, message: Uplink) -> Result<(), String> {
        self.slots[slot].uplink_room(1)?;
        let connection = &mut self.slots[slot].connection;
        let arrival = connection
            .link
            .uplink(&self.config.latency, &mut self.random, sent);
        connection.uplink.push_back((arrival, message));
        Ok(())
    }

    /// Frame packets emitted at `now`. A packet too long for a length prefix (a
    /// dense large minimap) travels alone in its own frame, in stream order.
    /// Aggregate broadcasts are split to fit the bounded socket transports.
    fn emit(&mut self, slot: usize, now: f64, packets: &[Vec<u8>]) -> Result<(), String> {
        let mut groups: Vec<&[Vec<u8>]> = Vec::new();
        let mut start = 0;
        let mut bytes = 2; // Elapsed-ms header.
        for (index, packet) in packets.iter().enumerate() {
            if !encode::frameable(packet) {
                if start < index {
                    groups.push(&packets[start..index]);
                }
                groups.push(&packets[index..=index]);
                start = index + 1;
                bytes = 2;
            } else {
                let packet_bytes = packet.len() + 2;
                if bytes + packet_bytes > MAX_FRAME_BYTES && start < index {
                    groups.push(&packets[start..index]);
                    start = index;
                    bytes = 2;
                }
                bytes += packet_bytes;
            }
        }
        if start < packets.len() {
            groups.push(&packets[start..]);
        }
        let connection = &mut self.slots[slot].connection;
        if connection.downlink.len() + groups.len() > MAX_IN_FLIGHT {
            return Err("Downlink queue is full".into());
        }
        for group in groups {
            let setup = *connection.setup_at.get_or_insert(now);
            let target = ((now - setup) * 1000.).round().max(0.) as u64;
            let elapsed = target.saturating_sub(connection.header_ms).min(65535);
            connection.header_ms += elapsed;
            let bytes = encode::frame(elapsed as u16, group)?;
            let arrival = connection
                .link
                .downlink(&self.config.latency, &mut self.random, now);
            connection.downlink.push_back((arrival, bytes));
        }
        Ok(())
    }

    /// Server side at `now`: arrived connects, pings and commands, in stream order.
    fn ingest(&mut self, slot: usize, now: f64) -> Result<(), String> {
        while let Some((at, _)) = self.slots[slot].connection.uplink.front() {
            if *at > now {
                break;
            }
            let (_, message) = self.slots[slot].connection.uplink.pop_front().unwrap();
            let over = self.slots[slot].connection.over;
            match message {
                Uplink::Victory(text) => self.victory(slot, text, now)?,
                // The stock browser keeps pinging during its death fade and
                // victory entry. Missing pongs trips its lag detector and slows
                // the animation to one fifth speed, delaying Play Again.
                Uplink::Message(ref bytes) if bytes == &[251] => {
                    self.emit(slot, now, &[encode::pong()])?;
                }
                _ if over => {}
                Uplink::Connect => {
                    let radius = self.world.config.radius;
                    let connection = &mut self.slots[slot].connection;
                    connection.setup = true;
                    connection.radius = radius.round() as u32;
                    connection.minimap_at = now;
                    self.emit(slot, now, &[encode::setup(radius)?])?;
                }
                Uplink::Message(bytes) => match bytes[..] {
                    [heading @ 0..=250] => {
                        self.slots[slot].control.heading = (f64::from(heading) + 0.5) * TAU / 251.;
                    }
                    [252, amount] => {
                        // Keyboard turn: amounts below 128 turn the wanted heading
                        // down, 128 and above up, by the source bridge's 0.033 rad
                        // per count scaled by the size turn factor. An
                        // approximation; the closed server's rule is unaudited.
                        let (count, sign) = if amount < 128 {
                            (amount, -1.)
                        } else {
                            (amount - 128, 1.)
                        };
                        let turn = 0.033 * physics::scang(self.world.snakes[slot].segments);
                        let control = &mut self.slots[slot].control;
                        control.heading =
                            (control.heading + sign * turn * f64::from(count)).rem_euclid(TAU);
                    }
                    [253] => self.slots[slot].control.boost = true,
                    [254] => self.slots[slot].control.boost = false,
                    _ => return Err("Unsupported client message".into()),
                },
            }
        }
        Ok(())
    }

    /// Text from the client whose dead life holds the record, accepted once.
    /// Anything else is ignored: this is a local session rule, not an official
    /// qualification check.
    fn victory(&mut self, slot: usize, text: Vec<u8>, now: f64) -> Result<(), String> {
        let life = self.slots[slot].life;
        let dead = !self.world.snakes[slot].alive;
        if self.record.as_ref().is_some_and(|record| {
            record.slot == slot && record.life == life && dead && !record.message_set
        }) {
            // The stock client dismisses its victory entry on v2. Reserve and
            // queue that acknowledgement before accepting the message.
            self.emit(slot, now, &[encode::victory_accepted()])?;
            let record = self.record.as_mut().unwrap();
            record.message = text;
            record.message_set = true;
            record.version += 1;
        }
        Ok(())
    }

    /// A life that ended by death since the last tick may set the record; a
    /// tie keeps the earlier holder.
    fn settle_lives(&mut self) {
        if !self.config.local_records {
            return;
        }
        for slot in 0..self.slots.len() {
            let snake = &self.world.snakes[slot];
            let s = &mut self.slots[slot];
            if snake.alive {
                s.living = Some(s.life);
                continue;
            }
            if s.living.take() != Some(s.life) {
                continue;
            }
            let (segments, fam, score) = represented(snake);
            if self
                .record
                .as_ref()
                .is_some_and(|record| record.score >= score)
            {
                continue;
            }
            let name = match self.profiles.get(slot) {
                Some(profile) => profile.nickname.clone(),
                None => slot.to_string().into_bytes(),
            };
            let version = self.record.as_ref().map_or(1, |record| record.version + 1);
            self.record = Some(Record {
                slot,
                life: s.life,
                segments,
                fam,
                score,
                name,
                message: Vec::new(),
                message_set: false,
                version,
            });
        }
    }

    /// One server tick. Client sides are then brought up to the new time, so
    /// frames polled between ticks are already complete.
    fn advance(&mut self) -> Result<(), String> {
        let now = self.time();
        for slot in 0..self.slots.len() {
            self.ingest(slot, now)?;
        }
        let actions: Vec<_> = self
            .world
            .snakes
            .iter()
            .zip(&self.slots)
            .map(|(snake, slot)| snake.alive.then_some(slot.control))
            .collect();
        self.world.step(&actions)?;
        let births = self.world.food_spawn_events.as_deref().unwrap_or(&[]);
        // Physics appends boost/death food, visits pickups in reverse food
        // order, then appends ambient replenishment. Preserve that ordering for
        // co-located lifetimes, including a refill at a just-eaten point.
        for event in births.iter().filter(|event| event.source.is_some()) {
            self.food_presentation.births(std::slice::from_ref(event))?;
        }
        for event in self.world.pickup_events.as_deref().unwrap_or(&[]) {
            if let PickupItem::Prey(prey) = &event.item {
                self.prey_eaters
                    .entry(encode::prey_id(prey))
                    .or_insert(event.eater as u16);
            }
            if let PickupItem::Food(food) = &event.item {
                // Only the oldest co-located lifetime is on the wire. Eating
                // a hidden newer pellet must not animate that visible pellet.
                if self.food_presentation.picked_up(food)? {
                    self.food_eaters
                        .entry(food_identity(food.point()))
                        .or_insert(event.eater as u16);
                }
            }
        }
        for event in births.iter().filter(|event| event.source.is_none()) {
            self.food_presentation.births(std::slice::from_ref(event))?;
        }
        self.settle_lives();
        self.tick += 1;
        let now = self.time();
        for (slot, snake) in self.slots.iter_mut().zip(&self.world.snakes) {
            if slot.connection.setup {
                slot.connection
                    .food_interest
                    .advance(snake.head, snake.heading, now);
            }
        }
        for slot in &mut self.slots {
            // A decision that sent nothing is complete once its duration elapses.
            if slot
                .processing_override
                .is_some_and(|(at, seconds)| at < now && now >= at + seconds)
            {
                slot.processing_override = None;
            }
        }
        if self.tick % self.config.broadcast_ticks as u64 == 0 {
            self.broadcast()?;
        } else {
            self.minimaps()?;
        }
        for slot in 0..self.slots.len() {
            self.client_until(slot, now)?;
        }
        Ok(())
    }

    fn broadcast(&mut self) -> Result<(), String> {
        let now = self.time();
        let latency = self.config.latency.clone();
        for slot in &mut self.slots {
            slot.connection.link.update(&latency, &mut self.random, now);
        }
        self.food_presentation.sync(&self.world.food)?;
        if !self.slots.iter().any(|slot| slot.connection.setup) {
            self.food_eaters.clear();
            self.prey_eaters.clear();
            return Ok(());
        }
        // Dead clients still render the world during the stock death fade;
        // withholding deltas leaves opponents extrapolating stale motion.
        let mut sectors: BTreeMap<[u8; 2], BTreeMap<[u8; 2], KnownFood>> = BTreeMap::new();
        for food in &self.world.food {
            let key = encode::food_key(food.point())?;
            let size = food
                .size
                .unwrap_or_else(|| (food.value / physics::nutrition(1., 2.)).max(0.).sqrt());
            sectors
                .entry([key[0], key[1]])
                .or_default()
                .entry([key[2], key[3]])
                .or_insert(KnownFood {
                    point: food.point(),
                    style: encode::FoodStyle {
                        color: self.food_presentation.color(food),
                        size: encode::food_size(size),
                        snake_food: matches!(food.origin, Some(Origin::Boost | Origin::Death)),
                    },
                });
        }
        let bounds: Vec<[f64; 4]> = self
            .world
            .snakes
            .iter()
            .map(|snake| {
                snake.body.iter().fold(
                    [snake.head.x, snake.head.y, snake.head.x, snake.head.y],
                    |[x0, y0, x1, y1], p| [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)],
                )
            })
            .collect();
        let mut board = None;
        for slot in 0..self.slots.len() {
            let connection = &self.slots[slot].connection;
            if !connection.setup {
                continue;
            }
            let alive = self.world.snakes[slot].alive;
            let was_over = connection.over;
            // The board is needed when a leaderboard is due, on first
            // appearance or to freeze the rank at death.
            let due = connection
                .leaderboard_at
                .map_or(alive && !connection.over, |at| now + 1e-9 >= at)
                || !alive && !connection.over;
            if board.is_none() && due {
                board = Some(Board::new(&self.world, &self.profiles)?);
            }
            if !alive && !was_over {
                let snake = &self.world.snakes[slot];
                let rank = board.as_ref().unwrap().rank(wire_score(snake), slot);
                self.slots[slot].connection.final_rank = Some(rank);
            }
            let mut packets = self.updates(slot, now, &sectors, &bounds)?;
            // The stock client ignores a leaderboard after game over, so the
            // death broadcast omits it; a record may follow.
            let live_board = board.as_ref().filter(|_| alive || was_over);
            packets.extend(self.hud(slot, now, live_board)?);
            if !packets.is_empty() {
                self.emit(slot, now, &packets)?;
            }
        }
        self.food_eaters.clear();
        self.prey_eaters.clear();
        self.minimaps()?;
        Ok(())
    }

    /// Maps have their own nominal clock; gameplay broadcasts must not round
    /// 2.5-second map deadlines up to 2.52 seconds on every update.
    fn minimaps(&mut self) -> Result<(), String> {
        let now = self.time();
        let interval = self.config.minimap_interval_seconds;
        let mut map = None;
        for slot in 0..self.slots.len() {
            let connection = &self.slots[slot].connection;
            if !connection.setup
                || !self.world.snakes[slot].alive
                || !connection.snakes.contains_key(&slot)
                || now + 1e-9 < connection.minimap_at
            {
                continue;
            }
            let deadline = connection.minimap_at;
            if map.is_none() {
                map = Some(Minimap::from_snakes(
                    &self.world.snakes,
                    &Arena {
                        center: Point::default(),
                        radius: self.world.config.radius,
                    },
                    self.config.minimap_side,
                ));
            }
            self.emit(slot, now, &[encode::minimap(map.as_ref().unwrap())])?;
            self.slots[slot].connection.minimap_at = if deadline == 0. {
                now + interval
            } else {
                // Skip missed opportunities in bounded time, preserving the
                // connection's deadline phase instead of accumulating rounding.
                deadline + (((now - deadline).max(0.) / interval).floor() + 1.) * interval
            };
        }
        Ok(())
    }

    /// Changes since this connection's last frame, own snake first.
    fn updates(
        &mut self,
        slot: usize,
        now: f64,
        sectors: &BTreeMap<[u8; 2], BTreeMap<[u8; 2], KnownFood>>,
        bounds: &[[f64; 4]],
    ) -> Result<Vec<Vec<u8>>, String> {
        let world = &self.world;
        let me = &world.snakes[slot];
        let record = self
            .record
            .as_ref()
            .is_some_and(|record| record.slot == slot && record.life == self.slots[slot].life);
        let mut packets = Vec::new();
        let connection = &mut self.slots[slot].connection;
        if !me.alive && !connection.over {
            // Game over alone leaves the stock client predicting its snake: send
            // the final mass, then game over, then the death removal.
            connection.over = true;
            let id = slot as u16;
            let known = connection.snakes.remove(&slot);
            let fam = encode::fam_u24(me.fam);
            if known.as_ref().is_some_and(|known| known.fam != fam) {
                packets.push(encode::fam(id, fam));
            }
            // The stock game-over score adds the credit to the final mass.
            let credit = encode::segment_credit(me.score, me.segments, fam);
            if connection.credit != credit {
                connection.credit = credit;
                packets.push(encode::credit(credit));
            }
            // The local record rule qualifies this life; the stock v1 flag
            // offers its player a victory-message input.
            packets.push(encode::game_over(record));
            if known.is_some() {
                packets.push(encode::removal(id, true));
            }
        }
        let radius = world.config.radius.round() as u32;
        if connection.radius != radius {
            connection.radius = radius;
            packets.push(encode::radius(world.config.radius)?);
        }
        let vision = world.config.vision;
        let head = me.head;
        let order = std::iter::once(slot).chain((0..world.snakes.len()).filter(|&i| i != slot));
        for i in order {
            let snake = &world.snakes[i];
            let life = self.slots[i].life;
            let wanted = physics::quantize_heading(self.slots[i].control.heading);
            let eligible = snake.alive && (i == slot || covered(head, snake, bounds[i], vision));
            let own = i == slot;
            let id = i as u16;
            let profile = self.profiles.get(i).unwrap_or(&NATIVE_PROFILE);
            let connection = &mut self.slots[slot].connection;
            let known = connection.snakes.get_mut(&i);
            match (known, eligible) {
                (Some(known), true) if known.life == life => {
                    changes(&mut packets, own, id, known, snake, wanted, profile)?;
                }
                (Some(known), false) => {
                    let died = !snake.alive || known.life != life;
                    packets.push(encode::removal(id, died));
                    connection.snakes.remove(&i);
                }
                (None, false) => {}
                (known, true) => {
                    if known.is_some() {
                        packets.push(encode::removal(id, true));
                    }
                    full(&mut packets, own, id, snake, wanted, profile)?;
                    connection
                        .snakes
                        .insert(i, KnownSnake::new(life, snake, wanted)?);
                }
            }
            let connection = &mut self.slots[slot].connection;
            if own && connection.snakes.contains_key(&slot) {
                // Leaderboards start with the client's own appearance; a
                // requested session ID follows it so the client knows itself.
                connection.leaderboard_at.get_or_insert(now);
                let fam = encode::fam_u24(snake.fam);
                let credit = encode::segment_credit(snake.score, snake.segments, fam);
                if connection.credit != credit {
                    connection.credit = credit;
                    packets.push(encode::credit(credit));
                }
                if std::mem::take(&mut connection.session_pending) {
                    packets.push(encode::session(id, connection.session));
                }
            }
        }
        let connection = &mut self.slots[slot].connection;
        // Also covers explicit diagnostic broadcasts between physics ticks.
        connection.food_interest.advance(me.head, me.heading, now);
        let food_subscriptions = connection.food_interest.subscribed().clone();
        food_updates(
            &mut packets,
            connection,
            sectors,
            &self.food_eaters,
            &food_subscriptions,
        )?;
        let mut visible = BTreeMap::new();
        for prey in &world.prey {
            if prey.position.distance(head) <= vision {
                visible.entry(encode::prey_id(prey)).or_insert(prey);
            }
        }
        let gone: Vec<u16> = connection
            .prey
            .keys()
            .filter(|id| !visible.contains_key(id))
            .copied()
            .collect();
        for id in gone {
            connection.prey.remove(&id);
            packets.push(encode::prey_removal(id, self.prey_eaters.get(&id).copied()));
        }
        for (id, prey) in visible {
            let current = KnownPrey {
                grid: encode::prey_grid(prey.position)?,
                motion: [
                    encode::angle_u24(prey.heading),
                    encode::angle_u24(prey.target_heading),
                    u32::from(encode::speed_u16(prey.speed)?),
                ],
            };
            match connection.prey.get(&id) {
                None => packets.push(encode::prey_appearance(prey)?),
                Some(known) if *known == current => continue,
                Some(known) => {
                    packets.push(encode::prey_motion(prey, known.motion != current.motion)?)
                }
            }
            connection.prey.insert(id, current);
        }
        Ok(packets)
    }

    /// Leaderboard when `board` is given and due, then an unsent record. Both
    /// wait for the connection's own appearance.
    fn hud(
        &mut self,
        slot: usize,
        now: f64,
        board: Option<&Board>,
    ) -> Result<Vec<Vec<u8>>, String> {
        let mut packets = Vec::new();
        let interval = self.config.leaderboard_interval_seconds;
        let connection = &mut self.slots[slot].connection;
        let Some(at) = connection.leaderboard_at else {
            return Ok(packets);
        };
        if let Some(board) = board.filter(|_| now + 1e-9 >= at) {
            let (row, rank) = match board.order.iter().position(|&(_, i)| i == slot) {
                Some(index) if index < LEADERBOARD_ROWS => (index as u8 + 1, index + 1),
                Some(index) => (0, index + 1),
                None => (0, connection.final_rank.unwrap_or(0)),
            };
            packets.push(encode::leaderboard(
                row,
                rank,
                board.order.len(),
                &board.rows,
            ));
            connection.leaderboard_at = Some(now + interval);
        }
        if let Some(record) = self
            .record
            .as_ref()
            .filter(|record| record.version != connection.record_version)
        {
            packets.push(encode::record(
                record.segments,
                record.fam,
                &record.name,
                &record.message,
            )?);
            connection.record_version = record.version;
        }
        Ok(packets)
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
                    if let Some(heading) = seek(&frame) {
                        let byte = (heading.rem_euclid(TAU) * 251. / TAU).floor() as u8;
                        if self.slots[slot].seeker != Some(byte) {
                            self.slots[slot].seeker = Some(byte);
                            self.send_to(
                                slot,
                                Action {
                                    heading,
                                    boost: false,
                                },
                            )?;
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// One shared world whose leading slots are external protocol-19 clients.
///
/// Remote slots exchange exact WebSocket payloads through the same server-side
/// scheduling, coverage and encoders as native slots. This process never
/// decodes their frames, pings for them or models their processing: a remote
/// client's compute time has already elapsed in real time. Configured latency
/// still delays both directions in order. Remaining slots are background food
/// seekers that reconnect after death; a remote slot stays dead until the host
/// connects it again.
pub(crate) struct RawSimulator {
    backend: SimulatorBackend,
}

impl RawSimulator {
    pub(crate) fn new(
        seed: u32,
        config: SimulationConfig,
        remote_slots: usize,
    ) -> Result<Self, String> {
        if !(1..=config.agents).contains(&remote_slots) {
            return Err("Remote slots must be 1 through the agent count".into());
        }
        if config.record_wire {
            return Err("Raw simulation does not record wire messages".into());
        }
        Ok(Self {
            backend: SimulatorBackend::build(seed, config, remote_slots)?,
        })
    }

    pub(crate) fn time(&self) -> f64 {
        self.backend.time()
    }
    pub(crate) fn dt(&self) -> f64 {
        self.backend.world.config.dt
    }

    fn remote(&self, slot: usize) -> Result<(), String> {
        if slot < self.backend.remote {
            Ok(())
        } else {
            Err("Unknown remote slot".into())
        }
    }
    fn open(&self, slot: usize) -> Result<(), String> {
        self.remote(slot)?;
        if self.backend.slots[slot].connection.open {
            Ok(())
        } else {
            Err("Remote slot is not connected".into())
        }
    }

    /// Start a new session and life in `slot`, replacing any existing one and
    /// its in-flight messages. The server answers with setup once the connect
    /// arrives; the first broadcast after it begins with the slot's own snake.
    pub(crate) fn connect(
        &mut self,
        slot: usize,
        appearance: PlayerAppearance,
    ) -> Result<(), String> {
        self.remote(slot)?;
        if appearance.nickname.len() > MAX_NICKNAME || appearance.custom_skin.len() > 255 {
            return Err("Nickname or custom skin is too long".into());
        }
        let b = &mut self.backend;
        let (head, heading) = b.spawn_pose(slot)?;
        b.slots[slot].living = None;
        b.world.snakes[slot].alive = false;
        b.world.respawn(slot, head, heading, 2)?;
        b.profiles[slot] = appearance;
        let s = &mut b.slots[slot];
        s.life += 1;
        s.control = Action {
            heading,
            boost: false,
        };
        b.connect(slot)
    }

    /// Queue one client message on the uplink. Accepts heading bytes 0..=250,
    /// ping 251, keyboard turns `[252, amount]` and boost 253/254; a rejected
    /// message changes nothing.
    pub(crate) fn receive(&mut self, slot: usize, bytes: &[u8]) -> Result<(), String> {
        self.open(slot)?;
        if !matches!(bytes, [0..=251 | 253 | 254] | [252, _]) {
            return Err("Unsupported client message".into());
        }
        let connection = &self.backend.slots[slot].connection;
        if connection.uplink.len() + connection.downlink.len() >= REMOTE_IN_FLIGHT {
            return Err("Remote connection has too many undelivered messages".into());
        }
        let now = self.time();
        self.backend
            .uplink(slot, now, Uplink::Message(bytes.to_vec()))
    }

    /// Server payloads delivered by now, in order. Each is one WebSocket message.
    pub(crate) fn drain(&mut self, slot: usize) -> Result<Vec<Vec<u8>>, String> {
        self.open(slot)?;
        let now = self.time();
        let downlink = &mut self.backend.slots[slot].connection.downlink;
        let mut delivered = Vec::new();
        while downlink.front().is_some_and(|(at, _)| *at <= now) {
            delivered.push(downlink.pop_front().unwrap().1);
        }
        Ok(delivered)
    }

    /// Close the session and remove its living snake without death food.
    /// Observers see a removal at their next broadcast. Closing a closed slot
    /// does nothing.
    pub(crate) fn disconnect(&mut self, slot: usize) -> Result<(), String> {
        self.remote(slot)?;
        let b = &mut self.backend;
        b.slots[slot].living = None;
        b.world.snakes[slot].alive = false;
        b.slots[slot].connection = Connection::default();
        Ok(())
    }

    /// Answer the client's optional pre-login session-ID request `[4]` once:
    /// `S` follows its own appearance on this connection's ordered downlink.
    /// The ID is opaque and does not authenticate or resume anything.
    pub(crate) fn request_session_id(&mut self, slot: usize) -> Result<(), String> {
        self.open(slot)?;
        let connection = &mut self.backend.slots[slot].connection;
        if !connection.session_requested {
            connection.session_requested = true;
            connection.session_pending = true;
        }
        Ok(())
    }

    /// Queue a playing client's `[255, 118, text]` victory text. Only the
    /// connection whose dead life holds the session record can set it, once;
    /// other text is ignored when it arrives.
    pub(crate) fn victory(&mut self, slot: usize, text: &[u8]) -> Result<(), String> {
        self.open(slot)?;
        if text.len() > MAX_VICTORY_TEXT || !text.is_ascii() {
            return Err("Victory text must be at most 140 ASCII bytes".into());
        }
        let connection = &self.backend.slots[slot].connection;
        if connection.uplink.len() + connection.downlink.len() >= REMOTE_IN_FLIGHT {
            return Err("Remote connection has too many undelivered messages".into());
        }
        let now = self.time();
        self.backend
            .uplink(slot, now, Uplink::Victory(text.to_vec()))
    }

    /// Shared server ticks; background slots decide at the decision cadence.
    pub(crate) fn advance_ticks(&mut self, ticks: usize) -> Result<(), String> {
        let b = &mut self.backend;
        for _ in 0..ticks {
            if b.tick % b.config.decision_ticks as u64 == 0 {
                b.drive_background()?;
            }
            b.advance()?;
        }
        Ok(())
    }
}

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

impl KnownSnake {
    fn new(life: u64, snake: &Snake, wanted: f64) -> Result<Self, String> {
        Ok(Self {
            life,
            front: snake.body[0],
            len: snake.body.len(),
            heading: encode::angle_byte(snake.heading),
            wanted: encode::angle_byte(wanted),
            speed: encode::speed_byte(snake.speed)?,
            fam: encode::fam_u24(snake.fam),
            kills: snake.kills,
        })
    }
}

/// Bijective mix of a connection serial, so IDs stay distinct for 2^32
/// connections without revealing their order. Opaque; it authenticates nothing.
fn session_id(serial: u32, key: u32) -> u32 {
    let mut x = (serial ^ key).wrapping_mul(0x9e37_79b1);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85eb_ca6b);
    x ^ x >> 13
}

/// Coverage: any centerline part within `vision` of `head`.
fn covered(head: Point, snake: &Snake, [x0, y0, x1, y1]: [f64; 4], vision: f64) -> bool {
    let reach = vision + 1.;
    if head.x < x0 - reach || head.x > x1 + reach || head.y < y0 - reach || head.y > y1 + reach {
        return false;
    }
    snake.head.distance(head) <= vision
        || snake
            .body
            .first()
            .is_some_and(|&point| physics::segment_distance(head, snake.head, point) <= vision)
        || snake
            .body
            .windows(2)
            .any(|pair| physics::segment_distance(head, pair[0], pair[1]) <= vision)
}

/// Full snake, then its complete motion with the turn direction: the stock
/// appearance handler leaves the heading at the last body anchor's direction.
/// A nonzero kill count follows, since appearances do not carry it.
fn full(
    packets: &mut Vec<Vec<u8>>,
    own: bool,
    id: u16,
    snake: &Snake,
    wanted: f64,
    profile: &PlayerAppearance,
) -> Result<(), String> {
    packets.push(encode::appearance(id, snake, wanted, profile)?);
    packets.extend(encode::motion(
        own,
        id,
        Some(encode::angle_byte(snake.heading)),
        Some(encode::angle_byte(wanted)),
        Some(encode::speed_byte(snake.speed)?),
        encode::direction(snake.heading, wanted),
    ));
    if snake.kills > 0 {
        packets.push(encode::kills(id, snake.kills));
    }
    Ok(())
}

/// Incremental body, mass and motion reports for a known snake.
fn changes(
    packets: &mut Vec<Vec<u8>>,
    own: bool,
    id: u16,
    known: &mut KnownSnake,
    snake: &Snake,
    wanted: f64,
    profile: &PlayerAppearance,
) -> Result<(), String> {
    let body = &snake.body;
    let fam = encode::fam_u24(snake.fam);
    let mut fam_sent = false;
    let change = body.len() as isize - known.len as isize;
    match body
        .iter()
        .take(FRESH_ANCHORS)
        .position(|&point| point == known.front)
        .filter(|&fresh| change <= fresh as isize)
    {
        Some(fresh) => {
            // Growth and tail removal commute geometrically: contraction never
            // reads the tail, so report removals first, then the new anchors.
            let grown = change.clamp(0, fresh as isize);
            for _ in 0..grown - change {
                packets.push(encode::tail(id, fam));
                fam_sent = true;
            }
            for j in (0..fresh).rev() {
                let grows = (j as isize) < grown;
                let (point, previous) = (body[j], body[j + 1]);
                let angle = encode::angle_u16((point.y - previous.y).atan2(point.x - previous.x));
                let radians = f64::from(angle) * TAU / 65536.;
                let step = Point {
                    x: previous.x + radians.cos() * physics::ANCHOR_SPACING,
                    y: previous.y + radians.sin() * physics::ANCHOR_SPACING,
                };
                let carried = grows.then_some(fam);
                fam_sent |= grows;
                packets.push(if step.distance(point) < 1e-6 {
                    encode::anchor(own, id, angle, carried)
                } else {
                    encode::anchor_at(own, id, angle, point, carried)?
                });
            }
        }
        None => {
            // Unrecognizable continuity (e.g. an externally edited body): resend.
            full(packets, own, id, snake, wanted, profile)?;
            *known = KnownSnake::new(known.life, snake, wanted)?;
            return Ok(());
        }
    }
    known.front = body[0];
    known.len = body.len();
    if fam_sent {
        known.fam = fam;
    } else if known.fam != fam {
        known.fam = fam;
        packets.push(encode::fam(id, fam));
    }
    let heading = encode::angle_byte(snake.heading);
    let target = encode::angle_byte(wanted);
    let speed = encode::speed_byte(snake.speed)?;
    let report = encode::motion(
        own,
        id,
        (heading != known.heading).then_some(heading),
        (target != known.wanted).then_some(target),
        (speed != known.speed).then_some(speed),
        encode::direction(snake.heading, wanted),
    );
    packets.extend(report);
    known.heading = heading;
    known.wanted = target;
    known.speed = speed;
    if known.kills != snake.kills {
        known.kills = snake.kills;
        packets.push(encode::kills(id, snake.kills));
    }
    Ok(())
}

/// Sector subscriptions around the head: `W` and listings on entry, `f`/`c` changes
/// while subscribed and a sector clear on exit.
fn food_updates(
    packets: &mut Vec<Vec<u8>>,
    connection: &mut Connection,
    sectors: &BTreeMap<[u8; 2], BTreeMap<[u8; 2], KnownFood>>,
    eaters: &BTreeMap<(u64, u64), u16>,
    subscribed: &BTreeSet<[u8; 2]>,
) -> Result<(), String> {
    let left: Vec<_> = connection
        .food
        .keys()
        .filter(|sector| !subscribed.contains(*sector))
        .copied()
        .collect();
    for sector in left {
        connection.food.remove(&sector);
        packets.push(encode::sector_clear(sector));
    }
    let empty = BTreeMap::new();
    for &sector in subscribed {
        let current = sectors.get(&sector).unwrap_or(&empty);
        let key = |offset: [u8; 2]| [sector[0], sector[1], offset[0], offset[1]];
        match connection.food.get_mut(&sector) {
            None => {
                let foods: Vec<_> = current.iter().map(|(k, v)| (*k, v.style)).collect();
                packets.push(encode::sector_enter(sector));
                packets.extend(encode::food_sector(sector, &foods));
                connection.food.insert(sector, current.clone());
            }
            Some(known) => {
                for (offset, food) in known.iter() {
                    let eater = eaters.get(&food_identity(food.point)).copied();
                    if current.get(offset) != Some(food) || eater.is_some() {
                        packets.push(encode::food_remove(key(*offset), eater));
                    }
                }
                for (offset, food) in current {
                    if known.get(offset) != Some(food)
                        || known
                            .get(offset)
                            .is_some_and(|old| eaters.contains_key(&food_identity(old.point)))
                    {
                        packets.push(encode::food_add(key(*offset), food.style));
                    }
                }
                known.clone_from(current);
            }
        }
    }
    Ok(())
}

fn food_identity(point: Point) -> (u64, u64) {
    let bits = |v: f64| if v == 0. { 0 } else { v.to_bits() };
    (bits(point.x), bits(point.y))
}

/// Background policy from the slot's own frame: turn inward near the rim,
/// otherwise toward the nearest received food.
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
mod tests {
    use super::*;

    #[test]
    fn food_replacement_removes_the_old_item_with_its_actual_eater() {
        let point = Point::default();
        let [sx, sy, rx, ry] = encode::food_key(point).unwrap();
        let first = KnownFood {
            point,
            style: encode::FoodStyle {
                color: 3,
                size: 20,
                snake_food: true,
            },
        };
        let second = KnownFood {
            point,
            style: encode::FoodStyle {
                color: 7,
                size: 40,
                snake_food: true,
            },
        };
        let mut connection = Connection::default();
        let mut sectors = BTreeMap::from([([sx, sy], BTreeMap::from([([rx, ry], first)]))]);
        let mut packets = Vec::new();
        food_updates(
            &mut packets,
            &mut connection,
            &sectors,
            &BTreeMap::new(),
            &BTreeSet::from([[sx, sy]]),
        )
        .unwrap();
        assert!(packets.contains(&vec![b'F', sx, sy, 3, rx, ry, 20]));
        sectors.get_mut(&[sx, sy]).unwrap().insert([rx, ry], second);
        let eaters = BTreeMap::from([(food_identity(point), 2)]);
        packets.clear();
        food_updates(
            &mut packets,
            &mut connection,
            &sectors,
            &eaters,
            &BTreeSet::from([[sx, sy]]),
        )
        .unwrap();
        assert_eq!(
            packets,
            [
                encode::food_remove([sx, sy, rx, ry], Some(2)),
                encode::food_add([sx, sy, rx, ry], second.style)
            ]
        );
        packets.clear();
        food_updates(
            &mut packets,
            &mut connection,
            &sectors,
            &BTreeMap::new(),
            &BTreeSet::from([[sx, sy]]),
        )
        .unwrap();
        assert!(packets.is_empty());
    }

    #[test]
    fn presentation_telemetry_preserves_physics_and_source_colors() {
        let mut config = raw_config(2);
        config.world.boost_food.enabled = false;
        let mut sim = RawSimulator::new(7, config, 2).unwrap();
        let mut profile = profile("Boost");
        profile.skin = 3;
        sim.connect(0, profile).unwrap();
        let mut reference = sim.backend.world.clone();
        reference.food_spawn_events = None;
        reference.pickup_events = None;
        let mut births = 0;
        for _ in 0..160 {
            sim.backend.slots[0].control.boost = true;
            let actions: Vec<_> = sim
                .backend
                .world
                .snakes
                .iter()
                .zip(&sim.backend.slots)
                .map(|(snake, slot)| snake.alive.then_some(slot.control))
                .collect();
            reference.step(&actions).unwrap();
            sim.backend.advance().unwrap();
            assert_eq!(sim.backend.world.snakes, reference.snakes);
            assert_eq!(sim.backend.world.food, reference.food);
            assert_eq!(sim.backend.world.prey, reference.prey);
            for event in sim.backend.world.food_spawn_events.as_ref().unwrap() {
                if event.source == Some(0) {
                    births += 1;
                    assert_eq!(sim.backend.food_presentation.color(&event.food), 3);
                }
            }
        }
        assert!(births > 0);
    }

    #[test]
    fn sector_announcements_follow_subscription_entries_and_exits() {
        let mut connection = Connection::default();
        let mut packets = Vec::new();
        let sectors = BTreeMap::new();
        let eaters = BTreeMap::new();
        food_updates(
            &mut packets,
            &mut connection,
            &sectors,
            &eaters,
            &BTreeSet::from([[108, 108]]),
        )
        .unwrap();
        assert_eq!(packets, [vec![b'W', 108, 108], vec![b'F', 108, 108]]);
        packets.clear();
        food_updates(
            &mut packets,
            &mut connection,
            &sectors,
            &eaters,
            &BTreeSet::from([[108, 108]]),
        )
        .unwrap();
        assert!(packets.is_empty());
        food_updates(
            &mut packets,
            &mut connection,
            &sectors,
            &eaters,
            &BTreeSet::from([[110, 108]]),
        )
        .unwrap();
        assert_eq!(
            packets,
            [
                vec![b'w', 108, 108],
                vec![b'W', 110, 108],
                vec![b'F', 110, 108]
            ]
        );
    }
    use std::f64::consts::PI;

    fn raw_config(agents: usize) -> SimulationConfig {
        SimulationConfig {
            agents,
            world: Config {
                vision: 3000.,
                ..Config::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn hosted_and_native_snakes_use_finite_whole_body_coverage() {
        let mut config = raw_config(3);
        config.world.radius = 6000.;
        config.world.vision = Config::default().vision;
        assert_eq!(config.world.vision, 1500.);
        let mut sim = RawSimulator::new(7, config, 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "Browser");
        for (slot, x) in [(0, 0.), (1, 1000.), (2, 4000.)] {
            sim.backend.world.snakes[slot].alive = false;
            sim.backend
                .world
                .respawn(slot, Point { x, y: 0. }, 0., 2)
                .unwrap();
            sim.backend.slots[slot].control.heading = 0.;
        }
        run(&mut sim, &mut [(0, &mut remote)], 12);
        assert!(sim.backend.slots[0].connection.snakes.contains_key(&1));
        assert!(!sim.backend.slots[0].connection.snakes.contains_key(&2));
        assert!(sim.backend.slots[1].connection.snakes.contains_key(&0));
        assert!(!sim.backend.slots[1].connection.snakes.contains_key(&2));

        // A distant head does not hide a snake whose tail reaches the view.
        sim.backend.world.snakes[2].alive = false;
        sim.backend
            .world
            .respawn(2, Point { x: 2500., y: 0. }, 0., 40)
            .unwrap();
        sim.backend.broadcast().unwrap();
        remote.pump(&mut sim, 0);
        assert!(sim.backend.slots[0].connection.snakes.contains_key(&2));
        for point in &mut sim.backend.world.snakes[2].body {
            point.x += 3000.;
        }
        sim.backend.world.snakes[2].head.x += 3000.;
        sim.backend.broadcast().unwrap();
        assert!(
            remote
                .pump(&mut sim, 0)
                .contains(&encode::removal(2, false))
        );
        sim.backend.world.snakes[1].alive = false;
        sim.backend.broadcast().unwrap();
        assert!(remote.pump(&mut sim, 0).contains(&encode::removal(1, true)));
    }

    #[test]
    fn ground_colors_are_shared_across_connections_and_reconnects() {
        let mut sim = RawSimulator::new(7, raw_config(2), 2).unwrap();
        let mut a = Remote::connect(&mut sim, 0, "A");
        let mut b = Remote::connect(&mut sim, 1, "B");
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 12);
        let food = sim
            .backend
            .world
            .food
            .iter()
            .find(|food| {
                let key = encode::food_key(food.point()).unwrap();
                (0..2).all(|slot| {
                    sim.backend.slots[slot]
                        .connection
                        .food
                        .get(&[key[0], key[1]])
                        .is_some_and(|sector| sector.contains_key(&[key[2], key[3]]))
                        && food.point().distance(sim.backend.world.snakes[slot].head) > 100.
                })
            })
            .expect("shared ground food")
            .clone();
        let key = encode::food_key(food.point()).unwrap();
        let style = |sim: &RawSimulator, slot: usize| {
            sim.backend.slots[slot].connection.food[&[key[0], key[1]]][&[key[2], key[3]]].style
        };
        let original = style(&sim, 0);
        assert_eq!(original, style(&sim, 1));
        let previous_head = sim.backend.world.snakes[1].head;
        let mut b = Remote::connect(&mut sim, 1, "New life");
        // Hold the observer location for this subscription/color regression;
        // random new spawn locations can legitimately see a different sector.
        let snake = &mut sim.backend.world.snakes[1];
        let delta = Point {
            x: previous_head.x - snake.head.x,
            y: previous_head.y - snake.head.y,
        };
        snake.head = previous_head;
        for point in &mut snake.body {
            point.x += delta.x;
            point.y += delta.y;
        }
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 6);
        assert_eq!(original, style(&sim, 1));
    }

    #[test]
    fn hosted_background_lives_use_safe_admission_from_startup() {
        for seed in 1..=16 {
            let mut config = raw_config(5);
            config.world.radius = 3000.;
            let mut sim = RawSimulator::new(seed, config, 1).unwrap();
            assert!(!sim.backend.world.snakes[0].alive);
            for slot in 1..5 {
                let snake = sim.backend.world.snakes[slot].clone();
                assert!(snake.alive, "seed {seed}, slot {slot}");
                sim.backend.world.snakes[slot].alive = false;
                assert!(sim.backend.world.spawn_is_clear(
                    snake.head,
                    snake.heading,
                    snake.segments,
                    600.
                ));
                sim.backend.world.snakes[slot].alive = true;
            }
        }
    }

    #[test]
    fn background_respawn_cannot_appear_close_to_a_living_player() {
        for seed in 1..=32 {
            let mut sim = SimulatorBackend::new(seed, raw_config(2)).unwrap();
            sim.world.snakes[1].alive = false;
            sim.reconnect_slot(1).unwrap();
            let new_life = sim.world.snakes[1].clone();
            // Check the complete new body against the existing player's head
            // and body, rather than only preventing overlap at the new head.
            sim.world.snakes[1].alive = false;
            assert!(
                sim.world
                    .spawn_is_clear(new_life.head, new_life.heading, 2, 600.),
                "seed {seed}"
            );
        }
    }

    #[test]
    fn minimaps_keep_their_own_clock_and_reset_after_death_and_reconnect() {
        let mut config = raw_config(1);
        config.world.radius = 10_000.;
        config.world.food_count = 0;
        config.world.ambient_replenish_per_second = 0.;
        let mut sim = RawSimulator::new(7, config, 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "Map");
        // Keep this timing fixture away from the rim and random food.
        sim.backend.world.snakes[0].alive = false;
        sim.backend
            .world
            .respawn(0, Point::default(), 0., 2)
            .unwrap();
        sim.backend.slots[0].control = Action {
            heading: 0.,
            boost: false,
        };
        let mut times = Vec::new();
        for _ in 0..760 {
            sim.advance_ticks(1).unwrap();
            for packet in remote.pump(&mut sim, 0) {
                if packet[0] == b'M' {
                    assert_eq!(&packet[1..3], &136_u16.to_be_bytes());
                    times.push(sim.time());
                }
            }
        }
        assert_eq!(times.len(), 3);
        for pair in times.windows(2) {
            assert!(((pair[1] - pair[0]) - 2.5).abs() <= sim.dt() + 1e-9);
        }
        assert!(
            times
                .iter()
                .any(|time| (time / sim.dt()).round() as u64 % 6 != 0)
        );

        sim.backend.world.snakes[0].alive = false;
        sim.advance_ticks(400).unwrap();
        assert!(remote.pump(&mut sim, 0).iter().all(|p| p[0] != b'M'));
        let mut next_life = Remote::connect(&mut sim, 0, "New map");
        run(&mut sim, &mut [(0, &mut next_life)], 7);
        assert!(next_life.packets.iter().any(|p| p[0] == b'M'));
    }

    #[test]
    fn retained_food_keeps_updating_after_leaving_the_entry_region() {
        let mut config = raw_config(1);
        config.world.radius = 5000.;
        config.world.food_count = 0;
        config.world.ambient_replenish_per_second = 0.;
        let mut sim = RawSimulator::new(7, config, 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "Food");
        sim.backend.world.snakes[0].alive = false;
        sim.backend
            .world
            .respawn(0, Point::default(), 0., 2)
            .unwrap();
        sim.backend.slots[0].control = Action {
            heading: 0.,
            boost: false,
        };
        run(&mut sim, &mut [(0, &mut remote)], 7);
        let food = crate::Food {
            x: 750.,
            y: 0.,
            value: 1.,
            size: Some(2.),
            origin: None,
        };
        let key = encode::food_key(food.point()).unwrap();
        sim.backend.world.food.push(food);
        sim.backend.broadcast().unwrap();
        remote.pump(&mut sim, 0);
        assert!(
            sim.backend.slots[0].connection.food[&[key[0], key[1]]].contains_key(&[key[2], key[3]])
        );

        // Synthetic pose change isolates retention from navigation and pickup.
        sim.backend.world.snakes[0].alive = false;
        sim.backend
            .world
            .respawn(0, Point { x: 3000., y: 0. }, 0., 2)
            .unwrap();
        sim.advance_ticks(1).unwrap();
        sim.backend.world.food[0].size = Some(3.);
        sim.backend.broadcast().unwrap();
        let changed = remote.pump(&mut sim, 0);
        assert!(
            changed
                .iter()
                .any(|p| matches!(p[0], b'f' | b'b') && p[1..5] == key)
        );
        assert!(!changed.contains(&encode::sector_clear([key[0], key[1]])));
        sim.advance_ticks(450).unwrap();
        assert!(sim.backend.world.snakes[0].alive);
        assert!(
            remote
                .pump(&mut sim, 0)
                .contains(&encode::sector_clear([key[0], key[1]]))
        );
    }

    #[test]
    fn prey_pickup_reports_the_actual_eater_without_inventing_one_for_visibility_exit() {
        let mut sim = RawSimulator::new(7, raw_config(1), 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "Prey");
        run(&mut sim, &mut [(0, &mut remote)], 7);
        let snake = &sim.backend.world.snakes[0];
        let (position, _) =
            physics::pickup_circle(snake.head, snake.heading, snake.speed, snake.segments);
        let prey = crate::prey::Prey {
            id: 123,
            position,
            heading: 0.,
            target_heading: 0.,
            speed: 118.75,
            size: 6.,
        };
        sim.backend.world.prey.push(prey.clone());
        sim.backend.broadcast().unwrap();
        remote.pump(&mut sim, 0);
        run(&mut sim, &mut [(0, &mut remote)], 6);
        assert!(remote.packets.contains(&encode::prey_removal(123, Some(0))));

        sim.backend.world.prey.push(prey);
        sim.backend.broadcast().unwrap();
        remote.pump(&mut sim, 0);
        sim.backend.world.prey.clear();
        sim.backend.broadcast().unwrap();
        assert!(
            remote
                .pump(&mut sim, 0)
                .contains(&encode::prey_removal(123, None))
        );
    }
    fn profile(nickname: &str) -> PlayerAppearance {
        PlayerAppearance {
            nickname: nickname.as_bytes().to_vec(),
            skin: 7,
            custom_skin: vec![1, 2, 3],
        }
    }
    fn angle_close(a: f64, b: f64, tolerance: f64) -> bool {
        ((a - b + PI).rem_euclid(TAU) - PI).abs() < tolerance
    }

    /// A remote client: the shared decoder fed only with drained payloads.
    struct Remote {
        state: ClientState,
        packets: Vec<Vec<u8>>,
    }
    impl Remote {
        fn connect(sim: &mut RawSimulator, slot: usize, nickname: &str) -> Self {
            sim.connect(slot, profile(nickname)).unwrap();
            let mut state = ClientState::new();
            state.new_connection(sim.time()).unwrap();
            Self {
                state,
                packets: Vec::new(),
            }
        }
        fn pump(&mut self, sim: &mut RawSimulator, slot: usize) -> Vec<Vec<u8>> {
            let now = sim.time();
            let mut packets = Vec::new();
            for frame in sim.drain(slot).unwrap() {
                self.state.receive_frame(&frame, now).unwrap();
                for packet in protocol::split_frame(&frame).unwrap() {
                    packets.push(packet.to_vec());
                }
            }
            self.packets.extend(packets.iter().cloned());
            packets
        }
        fn frame(&self, sim: &RawSimulator) -> ClientFrame {
            self.state.frame(sim.time()).unwrap()
        }
    }
    fn run(sim: &mut RawSimulator, remotes: &mut [(usize, &mut Remote)], ticks: usize) {
        for _ in 0..ticks {
            sim.advance_ticks(1).unwrap();
            for (slot, remote) in remotes.iter_mut() {
                remote.pump(sim, *slot);
            }
        }
    }
    fn is_full(packet: &[u8]) -> bool {
        packet[0] == b's' && packet.len() > 6
    }

    #[test]
    fn raw_join_sends_setup_then_own_appearance_and_shares_the_decoder() {
        let mut sim = RawSimulator::new(3, raw_config(3), 2).unwrap();
        sim.advance_ticks(60).unwrap();
        // Unconnected remote slots stay absent; the background slot plays.
        let world = &sim.backend.world;
        assert!(!world.snakes[0].alive && !world.snakes[1].alive && world.snakes[2].alive);
        assert!(sim.drain(0).is_err() && sim.receive(0, &[1]).is_err());
        assert!(sim.drain(2).is_err() && sim.connect(2, profile("")).is_err());
        let long = PlayerAppearance {
            nickname: vec![b'x'; 25],
            ..profile("")
        };
        assert!(sim.connect(0, long).is_err());
        assert!(!sim.backend.world.snakes[0].alive);

        // Slot 0's id differs from the setup server number; slot 1's equals it.
        assert_eq!(encode::SERVER_NUMBER, 1);
        let mut a = Remote::connect(&mut sim, 0, "Zero");
        assert!(a.pump(&mut sim, 0).is_empty());
        sim.advance_ticks(1).unwrap();
        let setup = a.pump(&mut sim, 0);
        assert_eq!(
            setup,
            [encode::setup(sim.backend.world.config.radius).unwrap()]
        );
        assert!(sim.drain(0).unwrap().is_empty());
        let mut b = Remote::connect(&mut sim, 1, "One");
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 12);

        for (slot, remote, nickname) in [(0, &a, "Zero"), (1, &b, "One")] {
            assert_eq!(remote.packets[0][0], b'a');
            assert_eq!(remote.packets.iter().filter(|p| p[0] == b'a').count(), 1);
            // The first full appearance is the client's own, with its profile.
            let own = &remote.packets[1];
            assert!(is_full(own));
            assert_eq!(u16::from_be_bytes([own[1], own[2]]), slot as u16);
            assert_eq!(own[15], 7);
            let n = nickname.len();
            assert_eq!(own[22], n as u8);
            assert_eq!(&own[23..23 + n], nickname.as_bytes());
            assert_eq!(own[23 + n..27 + n], [3, 1, 2, 3]);
            let frame = remote.frame(&sim);
            assert_eq!(frame.status, ClientStatus::Playing);
            let observation = frame.observation.unwrap();
            assert_eq!(observation.me.id, slot.to_string());
            // Within the movement since the last broadcast, six ticks ago.
            let actual = &sim.backend.world.snakes[slot];
            assert!(observation.me.head.distance(actual.head) < 6. * sim.dt() * actual.speed + 1.);
            // One shared world: each client sees the other remote and the bot.
            let mut seen: Vec<_> = observation.opponents.iter().map(|o| o.id.clone()).collect();
            seen.sort();
            let others: Vec<_> = (0..3)
                .filter(|&i| i != slot)
                .map(|i| i.to_string())
                .collect();
            assert_eq!(seen, others);
        }
    }

    #[test]
    fn raw_slots_never_ping_decode_or_record_on_their_own() {
        let config = SimulationConfig {
            record_wire: true,
            ..raw_config(2)
        };
        assert!(RawSimulator::new(1, config, 1).is_err());
        assert!(RawSimulator::new(1, raw_config(2), 0).is_err());
        assert!(RawSimulator::new(1, raw_config(2), 3).is_err());
        let mut sim = RawSimulator::new(1, raw_config(2), 2).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "");
        // Longer than the client's ping interval.
        for _ in 0..1500 {
            sim.advance_ticks(1).unwrap();
            remote.pump(&mut sim, 0);
            assert!(sim.backend.slots[0].connection.uplink.is_empty());
        }
        assert!(!remote.packets.contains(&encode::pong()));
        let s = &sim.backend.slots[0];
        assert!(s.records.is_none() && s.outbox.is_none());
        let internal = s.state.frame(sim.time()).unwrap();
        assert_eq!(internal.status, ClientStatus::Disconnected);
        assert!(remote.frame(&sim).status != ClientStatus::Waiting);
    }

    #[test]
    fn raw_controls_travel_the_uplink_and_reject_without_mutation() {
        let mut sim = RawSimulator::new(5, raw_config(2), 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "");
        run(&mut sim, &mut [(0, &mut remote)], 7);
        for (message, heading, boost) in [
            (10, 10.5, false),
            (253, 10.5, true),
            (250, 250.5, true),
            (254, 250.5, false),
        ] {
            sim.receive(0, &[message]).unwrap();
            sim.advance_ticks(1).unwrap();
            let control = sim.backend.slots[0].control;
            assert_eq!(control.heading, heading * TAU / 251.);
            assert_eq!(control.boost, boost);
        }
        sim.receive(0, &[251]).unwrap();
        sim.advance_ticks(1).unwrap();
        assert!(remote.pump(&mut sim, 0).contains(&encode::pong()));
        let control = sim.backend.slots[0].control;
        for message in [&[252][..], &[255], &[], &[1, 2], &[252, 1, 2]] {
            let error = sim.receive(0, message).unwrap_err();
            assert_eq!(error, "Unsupported client message");
            assert!(sim.backend.slots[0].connection.uplink.is_empty());
        }
        assert!(sim.receive(1, &[1]).is_err());
        sim.advance_ticks(1).unwrap();
        assert_eq!(sim.backend.slots[0].control, control);

        // The steered snake reports the requested heading to its own decoder.
        sim.receive(0, &[100]).unwrap();
        run(&mut sim, &mut [(0, &mut remote)], 120);
        let me = remote.frame(&sim).observation.unwrap().me;
        let world = sim.backend.world.snakes[0].heading;
        assert!(angle_close(me.heading, world, 0.03));
        assert!(angle_close(world, 100.5 * TAU / 251., 0.03));
    }

    #[test]
    fn keyboard_turns_move_the_wanted_heading_both_ways() {
        let mut sim = RawSimulator::new(5, raw_config(1), 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "");
        run(&mut sim, &mut [(0, &mut remote)], 7);
        sim.receive(0, &[0]).unwrap();
        sim.advance_ticks(1).unwrap();
        let start = sim.backend.slots[0].control.heading;
        let step = 0.033 * physics::scang(sim.backend.world.snakes[0].segments);
        for (amount, turns) in [(0, 0.), (128, 0.), (10, -10.), (138, 10.), (255, 127.)] {
            let before = sim.backend.slots[0].control;
            sim.receive(0, &[252, amount]).unwrap();
            sim.advance_ticks(1).unwrap();
            let after = sim.backend.slots[0].control;
            assert!(angle_close(
                after.heading,
                before.heading + turns * step,
                1e-12
            ));
            assert!((0. ..TAU).contains(&after.heading) && !after.boost);
        }
        assert!(angle_close(
            sim.backend.slots[0].control.heading,
            start + 127. * step,
            1e-12
        ));
        // Turning down past zero wraps into [0, TAU).
        sim.receive(0, &[252, 127]).unwrap();
        sim.receive(0, &[252, 127]).unwrap();
        sim.advance_ticks(1).unwrap();
        let heading = sim.backend.slots[0].control.heading;
        assert!(angle_close(heading, start - 127. * step, 1e-12));
        assert!((0. ..TAU).contains(&heading));
    }

    #[test]
    fn raw_queues_are_bounded_and_rejections_are_atomic() {
        let mut sim = RawSimulator::new(2, raw_config(1), 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "");
        let mut accepted = 0;
        while sim.receive(0, &[251]).is_ok() {
            accepted += 1;
        }
        // The connect itself occupies one place.
        assert_eq!(accepted, REMOTE_IN_FLIGHT - 1);
        let uplink = sim.backend.slots[0].connection.uplink.len();
        assert!(sim.receive(0, &[3]).is_err());
        assert_eq!(sim.backend.slots[0].connection.uplink.len(), uplink);
        // Pongs fill the downlink until drained; undrained frames still count.
        sim.advance_ticks(1).unwrap();
        assert!(sim.receive(0, &[251]).is_err());
        let packets = remote.pump(&mut sim, 0);
        let pongs = packets.iter().filter(|p| **p == encode::pong()).count();
        assert_eq!(pongs, accepted);
        sim.receive(0, &[251]).unwrap();
    }

    #[test]
    fn raw_latency_keeps_order_without_processing_delay() {
        let mut config = raw_config(1);
        config.latency.round_trip_seconds = vec![0.1];
        config.latency.processing_seconds = [0.5, 0.5];
        let mut sim = RawSimulator::new(4, config, 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "");
        let start = sim.time();
        let (at, setup) = loop {
            sim.advance_ticks(1).unwrap();
            if let Some(packet) = remote.pump(&mut sim, 0).first() {
                break (sim.time(), packet.clone());
            }
        };
        assert_eq!(setup[0], b'a');
        // One round trip, rounded up to the ticks of uplink and downlink.
        assert!(at >= start + 0.1 - 1e-9 && at < start + 0.1 + 2. * sim.dt());
        run(&mut sim, &mut [(0, &mut remote)], 30);
        sim.receive(0, &[251]).unwrap();
        let sent = sim.time();
        let pong = loop {
            sim.advance_ticks(1).unwrap();
            if remote.pump(&mut sim, 0).contains(&encode::pong()) {
                break sim.time();
            }
        };
        assert!(pong >= sent + 0.1 - 1e-9 && pong < sent + 0.1 + 2. * sim.dt());
    }

    #[test]
    fn every_full_appearance_is_followed_by_its_directed_motion() {
        let mut sim = RawSimulator::new(9, raw_config(4), 2).unwrap();
        let mut a = Remote::connect(&mut sim, 0, "");
        let mut b = Remote::connect(&mut sim, 1, "");
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 13);
        let mut logs = Vec::new();
        // Slot 1 rejoins while slot 0 turns up, then down: its new session
        // first sees slot 0 in the middle of each turn.
        for offset in [60, 251 - 60] {
            let heading = sim.backend.world.snakes[0].heading;
            let byte = (heading.rem_euclid(TAU) * 251. / TAU).floor() as usize;
            sim.receive(0, &[((byte + offset) % 251) as u8]).unwrap();
            run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 2);
            logs.push((1, std::mem::take(&mut b.packets)));
            b = Remote::connect(&mut sim, 1, "");
            run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 8);
        }
        // Break continuity so the fallback path resends a known snake.
        let resent = a.packets.len();
        for point in &mut sim.backend.world.snakes[2].body {
            point.x += 5.;
        }
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 6);
        assert!(
            a.packets[resent..]
                .iter()
                .any(|p| is_full(p) && p[1..3] == [0, 2])
        );
        logs.push((0, a.packets));
        logs.push((1, b.packets));
        let mut seen = [0; 3];
        for (slot, packets) in &logs {
            for k in (0..packets.len()).filter(|&k| is_full(&packets[k])) {
                let (s, motion) = (&packets[k], &packets[k + 1]);
                let id = u16::from_be_bytes([s[1], s[2]]);
                let own = usize::from(id) == *slot;
                let direction = s[6] - b'0';
                seen[usize::from(direction)] += 1;
                let code = match (own, direction == 2) {
                    (true, false) => b'd',
                    (true, true) => b'7',
                    (false, false) => b'e',
                    (false, true) => b'E',
                };
                assert_eq!(motion[0], code);
                assert_eq!(motion.len(), if own { 4 } else { 6 });
                let at = if own { 1 } else { 3 };
                if !own {
                    assert_eq!(u16::from_be_bytes([motion[1], motion[2]]), id);
                }
                let angle = |m: usize| {
                    f64::from(u32::from_be_bytes([0, s[m], s[m + 1], s[m + 2]])) * TAU / 16777215.
                };
                assert_eq!(motion[at], encode::angle_byte(angle(3)));
                assert_eq!(motion[at + 1], encode::angle_byte(angle(7)));
                if direction > 0 {
                    assert_eq!(encode::direction(angle(3), angle(7)), direction);
                }
            }
        }
        assert!(seen.iter().all(|&count| count > 0), "{seen:?}");
    }

    #[test]
    fn post_death_opponents_keep_the_live_broadcast_cadence() {
        for rtt in [0., 0.1] {
            let mut config = raw_config(2);
            config.latency.round_trip_seconds = vec![rtt];
            config.minimap_interval_seconds = 0.01;
            let interval = config.broadcast_ticks as f64 * config.world.dt;
            let mut sim = RawSimulator::new(6, config, 2).unwrap();
            let mut a = Remote::connect(&mut sim, 0, "Dead");
            let mut b = Remote::connect(&mut sim, 1, "Moving");
            run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 60);
            sim.backend.world.snakes[0].alive = false;
            let mut over = false;
            let mut deaths = 0;
            let mut motion_at = Vec::new();
            for _ in 0..120 {
                // Synthetic steering keeps the opponent's wanted angle changing
                // on every broadcast, so packet gaps expose a stopped stream.
                sim.backend.slots[1].control.heading += 0.02;
                sim.advance_ticks(1).unwrap();
                b.pump(&mut sim, 1);
                for packet in a.pump(&mut sim, 0) {
                    if packet[0] == b'v' {
                        over = true;
                        deaths += 1;
                    }
                    if !over {
                        continue;
                    }
                    assert!(
                        !matches!(packet[0], b'M' | b'V' | b'u'),
                        "post-death minimap"
                    );
                    assert!(!matches!(packet[0], b'G' | b'N' | b'd' | b'7'));
                    if matches!(packet[0], b'e' | b'E' | b'3' | b'4' | b'5')
                        && packet.get(1..3) == Some(&[0, 1])
                    {
                        motion_at.push(sim.time());
                    }
                }
            }
            assert_eq!(deaths, 1);
            assert!(
                motion_at.len() >= 10,
                "missing post-death motion: {motion_at:?}"
            );
            for gap in motion_at.windows(2) {
                assert!((gap[1] - gap[0] - interval).abs() < 1e-9, "{gap:?}");
            }
            assert_eq!(a.frame(&sim).status, ClientStatus::Dead);
            assert!(a.frame(&sim).observation.is_none());
        }
    }

    #[test]
    fn sole_dead_session_still_receives_food_and_prey() {
        let mut sim = RawSimulator::new(6, raw_config(1), 1).unwrap();
        let mut a = Remote::connect(&mut sim, 0, "");
        run(&mut sim, &mut [(0, &mut a)], 12);
        sim.backend.world.snakes[0].alive = false;
        assert!(
            next_broadcast(&mut sim, 0, &mut a)
                .iter()
                .any(|p| p[0] == b'v')
        );
        let head = sim.backend.world.snakes[0].head;
        let food = crate::model::Food {
            x: head.x + 100.,
            y: head.y,
            value: 1.,
            size: Some(2.),
            origin: None,
        };
        let key = encode::food_key(food.point()).unwrap();
        let prey = crate::prey::Prey {
            id: 123,
            position: Point {
                x: head.x,
                y: head.y + 100.,
            },
            heading: 0.,
            target_heading: 0.,
            speed: 100.,
            size: 6.,
        };
        let appear = encode::prey_appearance(&prey).unwrap();
        // Synthetic world edits isolate visibility even with no living sessions.
        sim.backend.world.food.push(food);
        sim.backend.world.prey.push(prey);
        sim.backend.slots[0].connection.minimap_at = 0.;
        sim.backend.broadcast().unwrap();
        let packets = a.pump(&mut sim, 0);
        assert!(
            packets.contains(&encode::food_add(
                key,
                encode::FoodStyle {
                    color: sim
                        .backend
                        .food_presentation
                        .color(sim.backend.world.food.last().unwrap()),
                    size: encode::food_size(2.),
                    snake_food: false,
                }
            ))
        );
        assert!(packets.contains(&appear));
        assert!(
            packets
                .iter()
                .all(|p| !matches!(p[0], b'v' | b'M' | b'V' | b'u'))
        );
        assert_eq!(a.frame(&sim).status, ClientStatus::Dead);
    }

    #[test]
    fn remote_death_sends_mass_game_over_then_removal_and_stays_dead() {
        let mut sim = RawSimulator::new(6, raw_config(3), 2).unwrap();
        let mut a = Remote::connect(&mut sim, 0, "");
        let mut b = Remote::connect(&mut sim, 1, "");
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 13);
        while sim.backend.tick % 6 != 0 {
            run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 1);
        }
        let snake = &mut sim.backend.world.snakes[0];
        snake.fam = 0.75;
        snake.alive = false;
        sim.advance_ticks(6).unwrap();
        let fam = encode::fam_u24(0.75);
        let death = [
            encode::fam(0, fam),
            encode::game_over(false),
            encode::removal(0, true),
        ];
        let packets = a.pump(&mut sim, 0);
        assert!(packets.starts_with(&death));
        assert!(packets.iter().all(|p| p[0] != b'm'));
        assert!(sim.backend.record.is_none());
        assert_eq!(a.frame(&sim).status, ClientStatus::Dead);
        let seen = b.pump(&mut sim, 1);
        assert!(seen.contains(&encode::removal(0, true)));
        assert!(seen.iter().all(|p| p[0] != b'm'));
        // A dead session must still answer pings during the stock client's
        // death fade, without accepting controls or reviving the snake.
        let control = sim.backend.slots[0].control;
        for bytes in [&[0][..], &[252, 100], &[253], &[251]] {
            sim.receive(0, bytes).unwrap();
        }
        let rank = sim.backend.slots[0].connection.final_rank.unwrap();
        let mut boards = 0;
        let mut pongs = 0;
        for _ in 0..600 {
            sim.advance_ticks(1).unwrap();
            b.pump(&mut sim, 1);
            let living = sim.backend.world.snakes.iter().filter(|s| s.alive).count();
            for packet in a.pump(&mut sim, 0) {
                if packet == encode::pong() {
                    pongs += 1;
                    continue;
                }
                assert!(!matches!(packet[0], b'v' | b'M' | b'V' | b'u'));
                if packet[0] != b'l' {
                    continue;
                }
                let (position, own_rank, count, _) = board_packet(&packet);
                assert_eq!((position, own_rank, count), (0, rank, living));
                boards += 1;
            }
        }
        assert!(boards > 0);
        assert_eq!(pongs, 1);
        assert_eq!(sim.backend.slots[0].control, control);
        assert!(!sim.backend.world.snakes[0].alive);
    }

    /// `(segments, fam, color, name)` of one leaderboard row.
    type Row = (usize, u32, u8, Vec<u8>);
    /// Fields of the stock `l` handler: own row, rank, population and rows.
    fn board_packet(p: &[u8]) -> (u8, usize, usize, Vec<Row>) {
        assert_eq!(p[0], b'l');
        let u16_at = |m: usize| usize::from(u16::from_be_bytes([p[m], p[m + 1]]));
        let mut rows = Vec::new();
        let mut m = 6;
        while m < p.len() {
            let fam = u32::from_be_bytes([0, p[m + 2], p[m + 3], p[m + 4]]);
            let n = usize::from(p[m + 6]);
            rows.push((u16_at(m), fam, p[m + 5], p[m + 7..m + 7 + n].to_vec()));
            m += 7 + n;
        }
        (p[1], u16_at(2), u16_at(4), rows)
    }
    fn set_mass(snake: &mut Snake, segments: usize, fam: f64) {
        snake.segments = segments;
        snake.fam = fam;
        snake.score = physics::score(segments, fam);
    }
    /// Run until the next broadcast has been delivered; returns its packets.
    fn next_broadcast(sim: &mut RawSimulator, slot: usize, remote: &mut Remote) -> Vec<Vec<u8>> {
        let mut packets = Vec::new();
        loop {
            sim.advance_ticks(1).unwrap();
            packets.extend(remote.pump(sim, slot));
            if sim.backend.tick % sim.backend.config.broadcast_ticks as u64 == 0 {
                return packets;
            }
        }
    }

    #[test]
    fn leaderboard_ranks_living_snakes_with_slot_ties() {
        let mut config = raw_config(13);
        config.world.radius = 3000.;
        let mut sim = RawSimulator::new(1, config, 2).unwrap();
        let mut zero = Remote::connect(&mut sim, 0, "Zero");
        run(&mut sim, &mut [(0, &mut zero)], 7);
        // Slot 1 never connected and slot 4 is dead: neither is counted.
        let world = &mut sim.backend.world;
        set_mass(&mut world.snakes[0], 30, 0.5);
        for slot in 2..13 {
            set_mass(&mut world.snakes[slot], 10 + slot, 0.25);
        }
        set_mass(&mut world.snakes[3], 40, 0.);
        set_mass(&mut world.snakes[5], 40, 0.);
        world.snakes[4].alive = false;
        let board = Board::new(&sim.backend.world, &sim.backend.profiles).unwrap();
        let order: Vec<_> = board.order.iter().map(|&(_, slot)| slot).collect();
        assert_eq!(order, [3, 5, 0, 12, 11, 10, 9, 8, 7, 6, 2]);
        let p = encode::leaderboard(3, 3, 11, &board.rows);
        let (_, _, count, rows) = board_packet(&p);
        assert_eq!(count, 11);
        assert_eq!(rows.len(), LEADERBOARD_ROWS);
        assert_eq!(rows[0], (40, 0, 0, b"3".to_vec()));
        assert_eq!(rows[1].3, b"5");
        assert_eq!(rows[2], (30, encode::fam_u24(0.5), 7, b"Zero".to_vec()));
        assert_eq!(rows[9].3, b"6");
        // A dead snake ranks after living ones it does not outscore; slot breaks ties.
        let score = |segments| encode::display_score(segments, 0);
        assert_eq!(board.rank(score(40), 4), 2);
        assert_eq!(board.rank(score(40), 1), 1);
        assert_eq!(board.rank(score(2), 4), 12);

        // Each connection's packet shares the rows, with its own position.
        while sim.backend.tick % 6 != 5 {
            sim.advance_ticks(1).unwrap();
        }
        zero.pump(&mut sim, 0);
        sim.backend.slots[0].connection.leaderboard_at = Some(0.);
        sim.advance_ticks(1).unwrap();
        let board = Board::new(&sim.backend.world, &sim.backend.profiles).unwrap();
        let index = board.order.iter().position(|&(_, i)| i == 0).unwrap();
        let count = board.order.len();
        let expected = encode::leaderboard(index as u8 + 1, index + 1, count, &board.rows);
        assert!(zero.pump(&mut sim, 0).contains(&expected));
    }

    #[test]
    fn leaderboards_start_with_own_appearance_and_follow_the_interval() {
        let mut config = raw_config(2);
        config.leaderboard_interval_seconds = 1.;
        let mut sim = RawSimulator::new(4, config, 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "Me");
        let mut times = Vec::new();
        while sim.time() < 5.5 {
            sim.advance_ticks(1).unwrap();
            let packets = remote.pump(&mut sim, 0);
            if times.is_empty() && packets.iter().any(|p| p[0] == b'l') {
                let own = packets.iter().position(|p| is_full(p) && p[1..3] == [0, 0]);
                let board = packets.iter().position(|p| p[0] == b'l');
                assert!(own.unwrap() < board.unwrap());
            }
            let living = sim.backend.world.snakes.iter().filter(|s| s.alive).count();
            let own = &sim.backend.slots[0].connection;
            for packet in packets.iter().filter(|p| p[0] == b'l') {
                let (position, rank, count, rows) = board_packet(packet);
                assert_eq!((count, rows.len()), (living, living));
                // This seed's remote dies; it keeps its frozen rank without a row.
                if own.over {
                    assert_eq!((position, Some(rank)), (0, own.final_rank));
                    times.push(sim.time());
                    continue;
                }
                assert!(position == rank as u8 && (1..=living).contains(&rank));
                assert_eq!(rows[rank - 1].3, b"Me");
                times.push(sim.time());
            }
        }
        assert!(times[0] < 0.1);
        assert_eq!(times.len(), 6);
        let broadcast = 6. * sim.dt();
        for pair in times.windows(2) {
            let gap = pair[1] - pair[0];
            assert!((1. - 1e-9..1. + broadcast).contains(&gap), "{gap}");
        }
    }

    #[test]
    fn credit_and_hud_carry_the_represented_score() {
        let mut sim = RawSimulator::new(2, raw_config(2), 1).unwrap();
        let mut remote = Remote::connect(&mut sim, 0, "Me");
        run(&mut sim, &mut [(0, &mut remote)], 7);
        // A score above the shown mass, as kiosk credit produces; overridden
        // here between ticks, then broadcast without a physics step.
        let world = &mut sim.backend.world;
        let own = &mut world.snakes[0];
        let fam = encode::fam_u24(own.fam);
        own.score = encode::display_score(own.segments + 5, fam);
        let own_segments = own.segments;
        let bot = &mut world.snakes[1];
        let bot_fam = encode::fam_u24(bot.fam);
        bot.score = encode::display_score(bot.segments + 3, bot_fam);
        let bot_segments = bot.segments;
        sim.backend.slots[0].connection.leaderboard_at = Some(0.);
        sim.backend.broadcast().unwrap();
        let packets = remote.pump(&mut sim, 0);
        assert!(packets.contains(&encode::credit(5)));
        let board = packets.iter().find(|p| p[0] == b'l').unwrap();
        let (_, _, _, rows) = board_packet(board);
        let mut rows: Vec<_> = rows.into_iter().map(|row| (row.0, row.1)).collect();
        rows.sort();
        let mut expected = vec![(own_segments + 5, fam), (bot_segments + 3, bot_fam)];
        expected.sort();
        assert_eq!(rows, expected);
        // The decoder applies the transmitted own credit to its own score.
        let me = remote.frame(&sim).observation.unwrap().me;
        assert_eq!(me.score, encode::display_score(me.segments + 5, fam));
        // The next physics step recomputes the score; the credit resets to zero.
        let packets = next_broadcast(&mut sim, 0, &mut remote);
        assert!(packets.contains(&encode::credit(0)));
        let packets = next_broadcast(&mut sim, 0, &mut remote);
        assert!(packets.iter().all(|p| p[0] != b'R'));
    }

    #[test]
    fn kill_counts_follow_appearances_and_changes() {
        let mut sim = RawSimulator::new(3, raw_config(3), 1).unwrap();
        sim.backend.world.snakes[1].kills = 3;
        let mut remote = Remote::connect(&mut sim, 0, "");
        run(&mut sim, &mut [(0, &mut remote)], 7);
        let packets = &remote.packets;
        let appear = packets.iter().position(|p| is_full(p) && p[1..3] == [0, 1]);
        let kill = packets.iter().position(|p| *p == encode::kills(1, 3));
        // After the appearance and its motion; zero counts are not sent.
        assert_eq!(kill.unwrap(), appear.unwrap() + 2);
        assert_eq!(packets.iter().filter(|p| p[0] == b'k').count(), 1);
        sim.backend.world.snakes[2].kills = 1;
        sim.backend.world.snakes[0].kills = 2;
        let packets = next_broadcast(&mut sim, 0, &mut remote);
        let kills: Vec<_> = packets.iter().filter(|p| p[0] == b'k').cloned().collect();
        assert_eq!(kills, [encode::kills(0, 2), encode::kills(2, 1)]);
        let packets = next_broadcast(&mut sim, 0, &mut remote);
        assert!(packets.iter().all(|p| p[0] != b'k'));
    }

    #[test]
    fn sole_dead_record_holder_sets_text_once_and_slot_reuse_cannot() {
        let mut config = raw_config(1);
        config.local_records = true;
        let mut sim = RawSimulator::new(7, config, 1).unwrap();
        let mut ann = Remote::connect(&mut sim, 0, "Ann");
        run(&mut sim, &mut [(0, &mut ann)], 7);
        // Text before a record exists, and invalid text, change nothing.
        sim.victory(0, b"early").unwrap();
        assert!(sim.victory(0, &[b'x'; MAX_VICTORY_TEXT + 1]).is_err());
        assert!(sim.victory(0, "\u{e9}".as_bytes()).is_err());
        sim.victory(0, &[b'x'; MAX_VICTORY_TEXT]).unwrap();
        sim.advance_ticks(1).unwrap();
        assert!(sim.backend.record.is_none());
        let die = |sim: &mut RawSimulator, segments| {
            let snake = &mut sim.backend.world.snakes[0];
            set_mass(snake, segments, 0.5);
            snake.alive = false;
        };
        die(&mut sim, 30);
        let fam = encode::fam_u24(0.5);
        let packets = next_broadcast(&mut sim, 0, &mut ann);
        assert!(packets.contains(&encode::game_over(true)));
        assert!(packets.contains(&encode::record(30, fam, b"Ann", b"").unwrap()));

        // The only connection is over; its accepted text still reaches it.
        sim.victory(0, b"gg").unwrap();
        sim.receive(0, &[251]).unwrap();
        let packets = next_broadcast(&mut sim, 0, &mut ann);
        assert_eq!(
            packets,
            [
                encode::victory_accepted(),
                encode::pong(),
                encode::record(30, fam, b"Ann", b"gg").unwrap(),
            ]
        );
        sim.victory(0, b"again").unwrap();
        ann.packets.clear();
        run(&mut sim, &mut [(0, &mut ann)], 700);
        assert!(
            ann.packets
                .iter()
                .all(|p| p[0] != b'm' || p.ends_with(b"gg"))
        );
        assert!(!ann.packets.contains(&encode::pong()));

        // A new session in the slot sees the record but cannot edit it, and a
        // lower or equal life does not replace it.
        sim.disconnect(0).unwrap();
        let mut bob = Remote::connect(&mut sim, 0, "Bob");
        run(&mut sim, &mut [(0, &mut bob)], 7);
        assert!(
            bob.packets
                .contains(&encode::record(30, fam, b"Ann", b"gg").unwrap())
        );
        die(&mut sim, 30);
        let packets = next_broadcast(&mut sim, 0, &mut bob);
        assert!(packets.contains(&encode::game_over(false)));
        sim.victory(0, b"mine").unwrap();
        run(&mut sim, &mut [(0, &mut bob)], 30);
        assert_eq!(bob.packets.iter().filter(|p| p[0] == b'm').count(), 1);
        let record = sim.backend.record.as_ref().unwrap();
        assert_eq!(
            (&record.name[..], &record.message[..]),
            (&b"Ann"[..], &b"gg"[..])
        );

        let mut bob = Remote::connect(&mut sim, 0, "Bob");
        run(&mut sim, &mut [(0, &mut bob)], 7);
        die(&mut sim, 31);
        let packets = next_broadcast(&mut sim, 0, &mut bob);
        assert!(packets.contains(&encode::game_over(true)));
        assert!(packets.contains(&encode::record(31, fam, b"Bob", b"").unwrap()));
        // Submitting an empty message still dismisses the stock input once.
        sim.victory(0, b"").unwrap();
        let packets = next_broadcast(&mut sim, 0, &mut bob);
        assert!(packets.contains(&encode::victory_accepted()));
        sim.victory(0, b"late replacement").unwrap();
        let packets = next_broadcast(&mut sim, 0, &mut bob);
        assert!(!packets.contains(&encode::victory_accepted()));
        assert!(sim.backend.record.as_ref().unwrap().message.is_empty());
        assert!(sim.backend.record.as_ref().unwrap().message_set);
        // A new world run starts without a record.
        assert!(
            RawSimulator::new(7, raw_config(1), 1)
                .unwrap()
                .backend
                .record
                .is_none()
        );
    }

    #[test]
    fn session_id_follows_own_appearance_once_per_connection() {
        let mut sim = RawSimulator::new(5, raw_config(2), 2).unwrap();
        assert!(sim.request_session_id(0).is_err());
        let mut a = Remote::connect(&mut sim, 0, "");
        sim.request_session_id(0).unwrap();
        sim.request_session_id(0).unwrap();
        let mut b = Remote::connect(&mut sim, 1, "");
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 20);
        let session = sim.backend.slots[0].connection.session;
        let sessions: Vec<_> = a.packets.iter().filter(|p| p[0] == b'S').collect();
        assert_eq!(sessions, [&encode::session(0, session)]);
        let at = a.packets.iter().position(|p| p[0] == b'S').unwrap();
        assert!(a.packets[0][0] == b'a' && is_full(&a.packets[1]) && at > 1);
        assert_eq!(a.frame(&sim).status, ClientStatus::Playing);
        assert!(b.packets.iter().all(|p| p[0] != b'S'));

        // Slot reuse gets a fresh ID; IDs stay distinct per connection.
        let mut seen = BTreeSet::from([session, sim.backend.slots[1].connection.session]);
        for _ in 0..200 {
            sim.disconnect(0).unwrap();
            sim.connect(0, profile("")).unwrap();
            sim.request_session_id(0).unwrap();
            assert!(seen.insert(sim.backend.slots[0].connection.session));
        }
        // Deterministic for a seed.
        let mut again = RawSimulator::new(5, raw_config(2), 2).unwrap();
        again.connect(0, profile("")).unwrap();
        assert_eq!(again.backend.slots[0].connection.session, session);
    }

    #[test]
    fn disconnect_removes_the_snake_and_a_reused_slot_is_a_new_life() {
        let mut sim = RawSimulator::new(8, raw_config(3), 2).unwrap();
        let mut a = Remote::connect(&mut sim, 0, "Old");
        let mut b = Remote::connect(&mut sim, 1, "");
        run(&mut sim, &mut [(0, &mut a), (1, &mut b)], 13);
        sim.receive(0, &[251]).unwrap();
        sim.disconnect(0).unwrap();
        assert!(!sim.backend.world.snakes[0].alive);
        assert!(sim.drain(0).is_err() && sim.receive(0, &[1]).is_err());
        sim.disconnect(0).unwrap();
        run(&mut sim, &mut [(1, &mut b)], 6);
        assert!(b.packets.contains(&encode::removal(0, true)));
        let observation = b.frame(&sim).observation.unwrap();
        assert!(observation.opponents.iter().all(|o| o.id != "0"));

        // Replace a life before any broadcast: observers drop the stale one.
        Remote::connect(&mut sim, 0, "Old");
        run(&mut sim, &mut [(1, &mut b)], 6);
        let life = sim.backend.slots[0].life;
        sim.disconnect(0).unwrap();
        b.packets.clear();
        let mut c = Remote::connect(&mut sim, 0, "New");
        assert_eq!(sim.backend.slots[0].life, life + 1);
        run(&mut sim, &mut [(0, &mut c), (1, &mut b)], 7);
        let removed = b
            .packets
            .iter()
            .position(|p| *p == encode::removal(0, true));
        let full = b
            .packets
            .iter()
            .position(|p| is_full(p) && p[1..3] == [0, 0]);
        assert!(removed.unwrap() < full.unwrap());
        assert_eq!(b.packets[full.unwrap()][22..26], [3, b'N', b'e', b'w']);
        // The new session starts from its own setup, without the old pong.
        assert_eq!(c.packets[0][0], b'a');
        assert!(is_full(&c.packets[1]) && c.packets[1][22..26] == [3, b'N', b'e', b'w']);
        assert!(!c.packets.contains(&encode::pong()));
        assert_eq!(c.frame(&sim).status, ClientStatus::Playing);
    }

    #[test]
    fn aggregate_broadcasts_fit_transport_limits_and_preserve_packet_order() {
        let mut backend = SimulatorBackend::new(1, SimulationConfig::default()).unwrap();
        let packets: Vec<_> = (0..130)
            .map(|index| {
                let mut packet = vec![b'F'; 8191];
                packet[1] = index;
                packet
            })
            .collect();
        backend.emit(0, 0., &packets).unwrap();
        let frames = &backend.slots[0].connection.downlink;
        assert!(frames.len() > 1);
        let mut decoded = Vec::new();
        for (_, bytes) in frames {
            // Reserve the maximum unmasked WebSocket header inside the host's
            // 1 MiB send buffer, also staying below the native receive limit.
            assert!(bytes.len() + 10 <= 1 << 20);
            decoded.extend(protocol::split_frame(bytes).unwrap());
        }
        assert_eq!(
            decoded,
            packets.iter().map(Vec::as_slice).collect::<Vec<_>>()
        );
    }

    #[test]
    fn oversized_packets_travel_alone_in_stream_order() {
        let config = SimulationConfig {
            minimap_side: 512,
            record_wire: true,
            ..Default::default()
        };
        let mut backend = SimulatorBackend::new(1, config).unwrap();
        while backend.frame().unwrap().status != ClientStatus::Playing {
            backend.advance_ticks(1).unwrap();
        }
        backend.drain_records(0).unwrap();
        let cells = (0..512 * 512).map(|i| u8::from(i % 2 == 0)).collect();
        let dense = Minimap {
            side: 512,
            cells,
            age_seconds: 0.,
        };
        let packet = encode::minimap(&dense);
        assert!(!encode::frameable(&packet));
        let now = backend.time();
        backend
            .emit(0, now, &[encode::pong(), packet, encode::pong()])
            .unwrap();
        assert_eq!(backend.slots[0].connection.downlink.len(), 3);
        backend.advance_ticks(1).unwrap();
        let records = backend.drain_records(0).unwrap();
        let sizes: Vec<_> = records
            .iter()
            .filter(|r| r.event == WireEvent::Receive)
            .map(|r| r.bytes.len())
            .take(3)
            .collect();
        assert_eq!(sizes[0], 3);
        assert!(sizes[1] > 8192);
        assert_eq!(sizes[2], 3);
        let received = backend.frame().unwrap().minimap.unwrap();
        assert_eq!(received.cells, dense.cells);
        // Split frames share one emission time: later ones carry zero elapsed.
        let headers: Vec<_> = records
            .iter()
            .filter(|r| r.event == WireEvent::Receive)
            .take(3)
            .map(|r| u16::from_be_bytes([r.bytes[0], r.bytes[1]]))
            .collect();
        assert_eq!(headers[1..], [0, 0]);
        let connection = &backend.slots[0].connection;
        let since_setup = (now - connection.setup_at.unwrap()) * 1000.;
        assert!((connection.header_ms as f64 - since_setup).abs() <= 1.);
    }

    #[test]
    fn recording_exhaustion_preserves_reconnect_state_and_allows_retry() {
        let snapshot = |backend: &SimulatorBackend| {
            let s = &backend.slots[0];
            (
                backend.frame().unwrap(),
                serde_json::to_value(&backend.world).unwrap(),
                format!(
                    "{:?}",
                    (
                        (&s.state, &s.connection),
                        (
                            s.life,
                            s.outbox,
                            s.processing_override,
                            s.previous_boost,
                            s.control,
                            s.pinged_at,
                            s.seeker,
                            s.living,
                        ),
                        (&s.records, s.record_bytes),
                        (
                            backend.connections,
                            &backend.random,
                            &backend.spawn_random,
                            &backend.record,
                        ),
                    )
                ),
            )
        };
        for used in [MAX_RECORD_BYTES, MAX_RECORD_BYTES - RECORD_OVERHEAD] {
            let mut backend = SimulatorBackend::new(
                1,
                SimulationConfig {
                    record_wire: true,
                    latency: LatencyConfig {
                        processing_seconds: [0.1; 2],
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
            .unwrap();
            while backend.frame().unwrap().status != ClientStatus::Playing {
                backend.advance_ticks(1).unwrap();
            }
            backend
                .send_to(
                    0,
                    Action {
                        heading: 1.,
                        boost: true,
                    },
                )
                .unwrap();
            backend.slots[0].record_bytes = used;
            let before = snapshot(&backend);
            if used == MAX_RECORD_BYTES {
                assert_eq!(
                    backend.connect(0).unwrap_err(),
                    "Wire record buffer is full; drain records first"
                );
                assert_eq!(snapshot(&backend), before);
            }
            assert_eq!(
                backend.reconnect_slot(0).unwrap_err(),
                "Wire record buffer is full; drain records first"
            );
            assert_eq!(snapshot(&backend), before);

            let life = backend.slots[0].life;
            let connections = backend.connections;
            let mut records = backend.drain_records(0).unwrap();
            backend.reconnect_slot(0).unwrap();
            assert_eq!(backend.slots[0].life, life + 1);
            assert_eq!(backend.connections, connections + 1);
            while backend.frame().unwrap().status != ClientStatus::Playing {
                backend.advance_ticks(1).unwrap();
            }
            records.extend(backend.drain_records(0).unwrap());
            let mut replay = ClientState::new();
            for record in &records {
                super::super::replay_record(&mut replay, record).unwrap();
            }
            assert_eq!(
                replay.frame(backend.time()).unwrap(),
                backend.frame().unwrap()
            );
        }
    }

    #[test]
    fn reconnect_rechecks_record_room_after_queued_delivery() {
        let mut backend = SimulatorBackend::new(
            1,
            SimulationConfig {
                record_wire: true,
                ..Default::default()
            },
        )
        .unwrap();
        while backend.frame().unwrap().status != ClientStatus::Playing {
            backend.advance_ticks(1).unwrap();
        }
        let now = backend.time();
        let s = &mut backend.slots[0];
        s.connection
            .downlink
            .push_front((now, vec![0, 0, b'e', 64]));
        s.record_bytes = MAX_RECORD_BYTES - 2 * RECORD_OVERHEAD;
        let mut connection = s.connection.clone();
        connection.downlink.pop_front();
        let life = s.life;
        let connections = backend.connections;
        let world = serde_json::to_value(&backend.world).unwrap();
        let random = format!("{:?}", (&backend.random, &backend.spawn_random));
        let records = backend.slots[0].records.as_ref().unwrap().len();

        assert_eq!(
            backend.reconnect_slot(0).unwrap_err(),
            "Wire record buffer is full; drain records first"
        );
        assert_eq!(backend.frame().unwrap().status, ClientStatus::Playing);
        assert_eq!(backend.slots[0].life, life);
        assert_eq!(backend.connections, connections);
        assert_eq!(serde_json::to_value(&backend.world).unwrap(), world);
        assert_eq!(
            format!("{:?}", (&backend.random, &backend.spawn_random)),
            random
        );
        assert_eq!(
            format!("{:?}", backend.slots[0].connection),
            format!("{connection:?}")
        );
        assert_eq!(
            backend.slots[0].records.as_ref().unwrap().len(),
            records + 1
        );
        assert_eq!(
            backend.slots[0]
                .records
                .as_ref()
                .unwrap()
                .last()
                .unwrap()
                .event,
            WireEvent::Receive
        );
    }

    #[test]
    fn rejected_departure_leaves_every_side_unchanged() {
        let config = SimulationConfig {
            record_wire: true,
            ..Default::default()
        };
        let mut backend = SimulatorBackend::new(1, config).unwrap();
        while backend.frame().unwrap().status != ClientStatus::Playing {
            backend.advance_ticks(1).unwrap();
        }
        backend.drain_records(0).unwrap();
        // Room for the heading record alone, not heading plus boost.
        backend.slots[0].record_bytes = MAX_RECORD_BYTES - 2 * RECORD_OVERHEAD - 1;
        let uplink = backend.slots[0].connection.uplink.len();
        let control = backend.slots[0].control;
        let action = Action {
            heading: 1.,
            boost: true,
        };
        assert!(backend.send_to(0, action).is_err());
        let s = &backend.slots[0];
        assert!(s.outbox.is_some_and(|(_, queued, _)| queued == action));
        assert_eq!(s.previous_boost, None);
        assert_eq!(s.connection.uplink.len(), uplink);
        assert_eq!(s.control, control);
        assert!(s.records.as_ref().unwrap().is_empty());
        assert!(backend.frame().unwrap().timing.sent_commands.is_empty());
        assert!(backend.decision_pending(0).unwrap());

        // The queued decision departs once records are drained.
        backend.drain_records(0).unwrap();
        backend.advance_ticks(1).unwrap();
        let sent: Vec<_> = backend
            .drain_records(0)
            .unwrap()
            .into_iter()
            .filter(|r| r.event == WireEvent::Send && r.bytes != [251])
            .map(|r| r.bytes)
            .collect();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[1], [253]);
        assert_eq!(backend.slots[0].previous_boost, Some(true));
        let commands = backend.frame().unwrap().timing.sent_commands;
        assert!(commands.len() == 1 && commands[0].action.boost);
    }
}
