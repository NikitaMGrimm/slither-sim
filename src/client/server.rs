//! Raw protocol-13/19 WebSocket host: remote clients and in-process bots share
//! one real-time world.
//!
//! Three thread roles share one wall-clock schedule: simulation owns physics
//! and encoding; I/O owns the listener and sockets, sending frames at their
//! latency-model delivery times; bots decode, decide, ping and model processing
//! for their own slots, reaching the world through timestamped commands.
//! Running 480 seekers on the simulation thread delayed remote frames by up to
//! ~20 ms depending on tick phase ([jitter study], 2026-10-25).
//!
//! [jitter study]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-tail-jitter/REPORT.md
//!
//! Nonblocking sockets retain interrupted upgrades, partial messages and pending
//! output in tungstenite state, so stalled peers cannot block physics or other
//! connections. Upgrade/login deadlines, message/send-buffer limits and per-poll
//! read budgets bound each socket; client faults close only that connection.
//!
//! Login accepts optional one-byte framing requests, `[99, 0]`, a `6` challenge
//! generated from a fixed literal, a 27-byte reply and `s` login. The challenge
//! provides stock-client compatibility, not authentication. A direct `s` login
//! is also accepted. Legacy `[99]` / `[99,3|4]` use a client-compatible 24-letter
//! challenge; selectors 11/12 choose protocol 13, selector 31 uses a short login
//! with protocol-19 output.
use super::{
    Bot, ClientState, ClientStatus, ForagerBot, SimulationConfig, challenge,
    latency::{LatencyConfig, Link, Random},
    protocol,
    simulation::{
        MAX_FRAME_BYTES, MAX_NICKNAME, MAX_REMOTE_OUTPUT_BYTES, MAX_VICTORY_TEXT, PlayerAppearance,
        REMOTE_IN_FLIGHT, RawSimulator, output_cost,
    },
};
use crate::Action;
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    io,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tungstenite::{
    Bytes, HandshakeError, Message, WebSocket,
    handshake::{
        MidHandshake,
        server::{ErrorResponse, Request, Response, ServerHandshake},
    },
    http::StatusCode,
    protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
};

#[cfg(target_os = "linux")]
mod readiness;
#[cfg(windows)]
mod wake;

pub const PATH: &str = "/slither";
pub const DEFAULT_PORT: u16 = 8766;
/// Automatic bot opt-in to physical food and the SlitherSim rules extension.
/// Stock browser sockets never request this header.
pub(crate) const PHYSICAL_FOOD_HEADER: &str = "x-slithersim-food";
// The budgets and timeouts below are host resource policies, not protocol or
// physics limits.
/// Largest accepted client message or frame. Logins with a custom skin are the
/// largest stock-client messages and stay far below this.
const MAX_INPUT_BYTES: usize = 4 << 10;
/// Output batched per socket write.
const WRITE_BUFFER_BYTES: usize = 16 << 10;
/// Queued output beyond this closes the connection as too slow. tungstenite
/// refuses a frame before writing anything if it would overflow this, so leave
/// room for the largest message and its 10-byte header behind an unflushed batch.
const MAX_SEND_BUFFER_BYTES: usize = WRITE_BUFFER_BYTES + MAX_FRAME_BYTES + 10;
/// Frames waiting for their delivery time on one background bot. Socket queues
/// use the shared byte budget, so legacy single messages have comparable room.
const MAX_QUEUED_FRAMES: usize = 4096;
/// Host policy: an unadmitted bot waits a wall second after refusal instead of
/// retrying every decision (normally 42 ms). Socket joins keep their own cadence.
/// See research/performance/2026-10-25-battledome-spawn-cost/REPORT.md.
const BOT_ADMISSION_RETRY_SECONDS: f64 = 1.;
/// Connections that have not joined, including those being closed.
const MAX_PENDING: usize = 32;
/// Fairness budgets per poll, so one busy listener or socket cannot starve
/// the other connections.
const MAX_ACCEPTS_PER_POLL: usize = 16;
const MAX_READS_PER_POLL: usize = 32;
const MAX_RAW_READ_BYTES: usize = 64 << 10;
const MAX_RAW_READ_CALLS: usize = 32;
/// Lifecycle deadlines from acceptance: upgrade, login and close handshakes.
const HTTP_TIMEOUT: Duration = Duration::from_secs(5);
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10);
const CLOSE_TIMEOUT: Duration = Duration::from_secs(1);
/// Physics ticks run per wake while catching up with the wall clock.
const MAX_CATCHUP_TICKS: u64 = 8;
/// Backlog beyond this is deferred: world time falls behind the wall clock
/// rather than skipping physics or running an unbounded burst.
const MAX_BACKLOG_SECONDS: f64 = 0.25;
/// Longest sleep of the socket and bot threads, which also bounds how late a
/// frame handed over during a sleep is noticed.
const MAX_IDLE: Duration = Duration::from_millis(1);
/// Appearances carry the custom skin after a u8 length.
const MAX_CUSTOM_SKIN: usize = u8::MAX as usize;
/// WebSocket control frames carry at most 125 payload bytes (RFC 6455 5.5),
/// two of which hold the close code.
const MAX_CLOSE_REASON: usize = 125 - 2;
/// Printable letters only: both accepted replies stay valid stock-client text.
const CHALLENGE_LITERAL: (&[u8; 5], &[u8; 22]) = (b"Slith", b"erSimLocalHostChalleng");

/// Real-time host configuration.
///
/// `simulation.agents` and `simulation.controlledSlot` are derived: the world
/// has `players.max` socket slots followed by `bots.count` background seekers, and no
/// in-process controlled slot. Values given for those two fields are replaced.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct HostConfig {
    pub bind: SocketAddr,
    pub seed: u32,
    pub players: PlayerConfig,
    pub bots: BotConfig,
    pub simulation: SimulationConfig,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEFAULT_PORT)),
            seed: 1,
            players: PlayerConfig::default(),
            bots: BotConfig::default(),
            simulation: SimulationConfig::default(),
        }
    }
}

/// Socket player slots and artificial server-side link delay.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct PlayerConfig {
    pub max: usize,
    /// Players already incur their own compute time; processing must be zero.
    pub latency: LatencyConfig,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            max: 4,
            latency: LatencyConfig::default(),
        }
    }
}

/// Built-in bot slots, workers, and modeled link and processing delay.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct BotConfig {
    pub count: usize,
    /// Workers apart from the simulation and socket threads.
    pub threads: usize,
    pub latency: LatencyConfig,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self {
            count: 4,
            threads: 2,
            latency: LatencyConfig::default(),
        }
    }
}

impl HostConfig {
    pub fn validate(&self) -> Result<(), String> {
        self.world_config().map(|_| ())
    }

    /// Valid settings outside measured behavior; see [`SimulationConfig::warnings`].
    pub fn warnings(&self) -> Vec<String> {
        self.simulation.warnings()
    }

    fn world_config(&self) -> Result<SimulationConfig, String> {
        let agents = self
            .players
            .max
            .checked_add(self.bots.count)
            .ok_or("players.max + bots.count must fit in usize")?;
        if self.players.max == 0 {
            return Err("Host needs 1 or more players.max".into());
        }
        if self.bots.count > 0 && self.bots.threads == 0 {
            return Err("Bots need 1 or more bots.threads".into());
        }
        if self.simulation.record_wire {
            return Err("The host does not retain wire records; set recordWire to false".into());
        }
        if self.simulation.latency != LatencyConfig::default() {
            return Err("Host simulation.latency must stay at its default; use players.latency or bots.latency".into());
        }
        if self.players.latency.processing_seconds != [0.; 2]
            || self.players.latency.processing_jitter_seconds != 0.
        {
            return Err("players.latency.processingSeconds and processingJitterSeconds must be zero; socket players incur their own compute time".into());
        }
        self.players
            .latency
            .validate()
            .map_err(|e| format!("players.latency: {e}"))?;
        self.bots
            .latency
            .validate()
            .map_err(|e| format!("bots.latency: {e}"))?;
        let mut config = SimulationConfig {
            agents,
            controlled_slot: 0,
            latency: self.bots.latency.clone(),
            ..self.simulation.clone()
        };
        config.world.size_for_population(self.bots.count);
        config.validate_for_population(self.bots.count)?;
        Ok(config)
    }
}

/// Threads exchange wall seconds since `epoch`. Only the simulation thread maps
/// them to world time, which runs `deferred × dt` behind once ticks were
/// deferred, so a deferral never moves times other threads already hold.
struct Clock {
    epoch: Instant,
    dt: f64,
    /// Published copy of the simulation's deferred ticks, for reporting.
    deferred: AtomicU64,
}

impl Clock {
    /// Wall seconds at `at`.
    fn wall(&self, at: Instant) -> f64 {
        at.saturating_duration_since(self.epoch).as_secs_f64()
    }

    /// Sleep until wall time `due`, for at most `limit`; an infinite `due`
    /// sleeps for `limit`.
    fn sleep_until(&self, due: f64, limit: Duration) {
        let wait = (due - self.wall(Instant::now())).min(limit.as_secs_f64());
        if wait > 0. {
            thread::sleep(Duration::from_secs_f64(wait));
        }
    }
}

/// State every host thread can see.
struct Shared {
    #[cfg(windows)]
    socket_wake: wake::Wake,
    clock: Clock,
    stop: AtomicBool,
    ticks: AtomicU64,
    /// Nanoseconds the simulation thread spent running ticks.
    busy: AtomicU64,
    players: AtomicUsize,
    /// Commands per socket slot sent to the simulation and not yet consumed.
    pending: Vec<AtomicUsize>,
    failure: Mutex<Option<String>>,
}

impl Shared {
    fn wake_socket(&self) -> Result<(), String> {
        #[cfg(windows)]
        self.socket_wake.notify().map_err(|e| e.to_string())?;
        Ok(())
    }

    fn stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

/// Sets the stop flag when its thread ends, including by panic, so the other
/// threads finish too. Senders to a thread that has ended therefore always see
/// the flag: they ignore send errors and exit at their next check.
struct StopOnExit(Arc<Shared>);

impl Drop for StopOnExit {
    fn drop(&mut self) {
        self.0.stop.store(true, Ordering::Relaxed);
    }
}

/// From a slot's owner to the simulation thread, in send order. A generation
/// numbers one owner's sessions in a slot; commands for an older one are
/// ignored.
enum Command {
    Connect {
        slot: usize,
        generation: u64,
        appearance: PlayerAppearance,
        session_id: bool,
        physical_food: bool,
        protocol13: bool,
        full_minimap: bool,
    },
    /// A game command sent at wall time `sent`.
    Message {
        slot: usize,
        generation: u64,
        bytes: Vec<u8>,
        sent: f64,
    },
    Victory {
        slot: usize,
        generation: u64,
        text: Vec<u8>,
        sent: f64,
    },
    Disconnect {
        slot: usize,
        generation: u64,
    },
}

impl Command {
    fn slot(&self) -> usize {
        match self {
            Command::Connect { slot, .. }
            | Command::Message { slot, .. }
            | Command::Victory { slot, .. }
            | Command::Disconnect { slot, .. } => *slot,
        }
    }
}

/// From the simulation thread to a slot's owner.
enum Delivery {
    /// Server payloads with their delivery times in wall seconds, in order.
    Frames {
        slot: usize,
        generation: u64,
        frames: Vec<(f64, Vec<u8>)>,
    },
    /// The world refused or ended this session.
    Closed {
        slot: usize,
        generation: u64,
        reason: String,
    },
}

impl Delivery {
    fn session(&self) -> (usize, u64) {
        match self {
            Self::Frames {
                slot, generation, ..
            }
            | Self::Closed {
                slot, generation, ..
            } => (*slot, *generation),
        }
    }
}

/// A bound protocol-13/19 server. `start` serves it on its own threads; `run`
/// serves it until a failure.
pub struct Host {
    listener: TcpListener,
    world: RawSimulator,
    max_clients: usize,
    bots: usize,
    bot_threads: usize,
    seed: u32,
    latency: LatencyConfig,
    decision_seconds: f64,
}

impl Host {
    pub fn bind(config: HostConfig) -> Result<Self, String> {
        let simulation = config.world_config()?;
        let latency = simulation.latency.clone();
        let decision_seconds = simulation.decision_ticks as f64 * simulation.world.dt;
        // Bots are clients of their own slots, like sockets.
        let mut world = RawSimulator::new(
            config.seed,
            simulation,
            config.players.max + config.bots.count,
        )?;
        world.set_player_latency(config.players.max, config.players.latency);
        let listener = TcpListener::bind(config.bind).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Self {
            listener,
            world,
            max_clients: config.players.max,
            bots: config.bots.count,
            bot_threads: config.bots.threads.min(config.bots.count),
            seed: config.seed,
            latency,
            decision_seconds,
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, String> {
        self.listener.local_addr().map_err(|e| e.to_string())
    }

    /// Serve on new threads; the world clock starts now.
    pub fn start(self) -> Result<RunningHost, String> {
        let address = self.local_addr()?;
        let (encoded, replies) = challenge_parts()?;
        let mut challenge = vec![0, 0, b'6'];
        challenge.extend(encoded);
        let shared = Arc::new(Shared {
            #[cfg(windows)]
            socket_wake: wake::Wake::new().map_err(|e| e.to_string())?,
            clock: Clock {
                epoch: Instant::now(),
                dt: self.world.dt(),
                deferred: AtomicU64::new(0),
            },
            stop: AtomicBool::new(false),
            ticks: AtomicU64::new(0),
            busy: AtomicU64::new(0),
            players: AtomicUsize::new(0),
            pending: (0..self.max_clients).map(|_| AtomicUsize::new(0)).collect(),
            failure: Mutex::new(None),
        });
        let mut running = RunningHost {
            address,
            shared: shared.clone(),
            threads: Vec::new(),
        };
        let (commands, inbox) = mpsc::channel();
        let (to_sockets, socket_inbox) = mpsc::channel();
        let mut routes = vec![to_sockets];
        for worker in 0..self.bot_threads {
            let (sender, receiver) = mpsc::channel();
            routes.push(sender);
            let seats = (self.max_clients + worker..self.max_clients + self.bots)
                .step_by(self.bot_threads)
                .map(|slot| Seat::new(slot, slot - self.max_clients, self.decision_seconds))
                .collect();
            let bots = Bots {
                seats,
                policy: ForagerBot::new(),
                stride: self.bot_threads,
                deliveries: receiver,
                commands: commands.clone(),
                shared: shared.clone(),
                latency: self.latency.clone(),
                random: Random::new(u64::from(self.seed) ^ 0x626f_7473_0000 ^ worker as u64),
                interval: self.decision_seconds,
            };
            running.spawn(&format!("slither-bots-{worker}"), bots, Bots::run)?;
        }
        let gateway = Gateway {
            listener: self.listener,
            sessions: Vec::new(),
            #[cfg(target_os = "linux")]
            readiness: readiness::Readiness::default(),
            gate: Gate {
                occupied: vec![false; self.max_clients],
                generations: vec![0; self.max_clients],
                challenge,
                replies,
                commands,
                shared: shared.clone(),
            },
            deliveries: socket_inbox,
        };
        let agents = self.max_clients + self.bots;
        let simulation = Simulation {
            world: self.world,
            commands: inbox,
            batches: routes.iter().map(|_| Vec::new()).collect(),
            routes,
            max_clients: self.max_clients,
            generations: vec![0; agents],
            waiting: VecDeque::new(),
            next_spawn_tick: 0,
            last_spawn_tick: None,
            shared,
            ticks: 0,
            deferred: 0,
        };
        running.spawn("slither-simulation", simulation, Simulation::run)?;
        running.spawn("slither-sockets", gateway, Gateway::run)?;
        Ok(running)
    }

    /// Serve until a thread fails, and return its error.
    pub fn run(self) -> Result<(), String> {
        self.start()?.wait()
    }
}

/// A serving host. Dropping it stops and joins its threads.
pub struct RunningHost {
    address: SocketAddr,
    shared: Arc<Shared>,
    threads: Vec<JoinHandle<Result<(), String>>>,
}

impl RunningHost {
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// Physics ticks run since starting.
    pub fn ticks(&self) -> u64 {
        self.shared.ticks.load(Ordering::Relaxed)
    }

    /// Wall-clock ticks postponed because the simulation could not keep up.
    pub fn deferred_ticks(&self) -> u64 {
        self.shared.clock.deferred.load(Ordering::Relaxed)
    }

    /// Wall seconds the simulation thread has spent running ticks; its load is
    /// the increase over elapsed time.
    pub fn simulation_seconds(&self) -> f64 {
        self.shared.busy.load(Ordering::Relaxed) as f64 * 1e-9
    }

    /// Connected socket players in the shared world.
    pub fn players(&self) -> usize {
        self.shared.players.load(Ordering::Relaxed)
    }

    /// Whether a host thread has stopped. Call [`Self::wait`] for its result.
    pub fn is_finished(&self) -> bool {
        self.shared.stopped()
    }

    /// Block until a thread fails, and return its error.
    pub fn wait(mut self) -> Result<(), String> {
        self.join()
    }

    /// Stop every thread; an error reports a failure that happened first.
    pub fn stop(mut self) -> Result<(), String> {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.join()
    }

    fn spawn<T: Send + 'static>(
        &mut self,
        name: &str,
        task: T,
        run: fn(&mut T) -> Result<(), String>,
    ) -> Result<(), String> {
        let shared = self.shared.clone();
        let thread = thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                let mut task = task;
                // Declared after the task, so the flag is set before its
                // channels close, on return and on unwind.
                let exit = StopOnExit(shared);
                let result = run(&mut task);
                if let Err(error) = &result {
                    let mut failure = exit.0.failure.lock().unwrap_or_else(|e| e.into_inner());
                    failure.get_or_insert_with(|| error.clone());
                }
                result
            })
            .map_err(|e| e.to_string())?;
        self.threads.push(thread);
        Ok(())
    }

    fn join(&mut self) -> Result<(), String> {
        let mut panicked = false;
        for thread in self.threads.drain(..) {
            panicked |= thread.join().is_err();
        }
        let failure = self
            .shared
            .failure
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        match (failure.clone(), panicked) {
            (Some(error), _) => Err(error),
            (None, true) => Err("A host thread panicked".into()),
            (None, false) => Ok(()),
        }
    }
}

impl Drop for RunningHost {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        let _ = self.join();
    }
}

/// The simulation thread: the world, its tick schedule and command intake.
struct Simulation {
    world: RawSimulator,
    commands: Receiver<Command>,
    /// Owner threads: sockets first, then bot workers.
    routes: Vec<Sender<Vec<Delivery>>>,
    batches: Vec<Vec<Delivery>>,
    max_clients: usize,
    /// Current session per slot; zero while closed.
    generations: Vec<u64>,
    waiting: VecDeque<PendingJoin>,
    next_spawn_tick: u64,
    /// Host queue state, not core checkpoint state. Expanding the pit must
    /// leave the last newborn its existing departure window before it moves.
    last_spawn_tick: Option<u64>,
    shared: Arc<Shared>,
    ticks: u64,
    deferred: u64,
}

struct PendingJoin {
    slot: usize,
    generation: u64,
    appearance: PlayerAppearance,
    session_id: bool,
    physical_food: bool,
    protocol13: bool,
    full_minimap: bool,
    requested_tick: u64,
    /// A new socket connection replaces the previous player's rolling score.
    reset_kills: bool,
}

impl Simulation {
    fn run(&mut self) -> Result<(), String> {
        let dt = self.shared.clock.dt;
        while !self.shared.stopped() {
            let due = self.due_ticks(Instant::now());
            if due == 0 {
                let next = (self.ticks + self.deferred + 1) as f64 * dt;
                self.shared.clock.sleep_until(next, MAX_IDLE);
                continue;
            }
            let started = Instant::now();
            for _ in 0..due {
                // A command applies in the tick during which it arrives. Its send
                // time, not this loop, picks that tick, so each frame reflects the
                // input that reached the world by its due time.
                self.apply_commands()?;
                self.advance_tick()?;
                self.ticks += 1;
                self.shared.ticks.store(self.ticks, Ordering::Relaxed);
                self.hand_over()?;
            }
            let busy = started.elapsed().as_nanos().try_into().unwrap_or(u64::MAX);
            self.shared.busy.fetch_add(busy, Ordering::Relaxed);
        }
        Ok(())
    }

    fn due_ticks(&mut self, now: Instant) -> u64 {
        let dt = self.shared.clock.dt;
        // Casts saturate, and ticks + deferred never exceeds an earlier schedule.
        let scheduled = (now.duration_since(self.shared.clock.epoch).as_secs_f64() / dt) as u64;
        let backlog = scheduled.saturating_sub(self.deferred + self.ticks);
        let limit = (MAX_BACKLOG_SECONDS / dt).ceil() as u64;
        if backlog > limit {
            self.defer(backlog - limit);
        }
        backlog.min(limit).min(MAX_CATCHUP_TICKS)
    }

    /// Run world time `ticks` further behind the wall clock. Messages already
    /// in flight keep their wall arrival times.
    fn defer(&mut self, ticks: u64) {
        self.deferred += ticks;
        self.shared
            .clock
            .deferred
            .store(self.deferred, Ordering::Relaxed);
        self.world.shift_links(ticks as f64 * self.shared.clock.dt);
    }

    /// World time of wall time `wall`, behind it by the deferred ticks.
    fn world_time(&self, wall: f64) -> f64 {
        wall - self.deferred as f64 * self.shared.clock.dt
    }

    fn route(&self, slot: usize) -> usize {
        if slot < self.max_clients {
            0
        } else {
            1 + (slot - self.max_clients) % (self.routes.len() - 1)
        }
    }

    fn apply_commands(&mut self) -> Result<(), String> {
        while let Ok(command) = self.commands.try_recv() {
            self.apply(command)?;
        }
        self.admit_waiting()?;
        Ok(())
    }

    /// Circular worlds wait for a safe pose. A waiting socket gets the next
    /// attempt before any bot, including bots whose commands arrived first.
    /// One failed search per 14 ms of world time bounds total retry work,
    /// replacing per-bot refusal retries. Successful joins allow the next tick.
    /// research/battledome/2026-10-26-safe-spawn/REPORT.md
    fn admit_waiting(&mut self) -> Result<(), String> {
        if self.ticks < self.next_spawn_tick || self.waiting.is_empty() {
            return Ok(());
        }
        let first = self
            .waiting
            .iter()
            .position(|join| join.slot < self.max_clients)
            .unwrap_or(0);
        let protected_until = self.last_spawn_tick.map_or(0, |tick| {
            tick.saturating_add((crate::spawn::DEPARTURE_SECONDS / self.world.dt()).ceil() as u64)
        });
        if !self
            .world
            .grow_for_waiters(self.waiting.len(), self.ticks >= protected_until)
        {
            // Stop admissions too, so fresh births cannot postpone expansion
            // indefinitely. No search work is spent during this bounded hold.
            self.next_spawn_tick = protected_until;
            return Ok(());
        }
        let join = self.waiting.remove(first).unwrap();
        if !self.join(&join)? {
            self.waiting.insert(first, join);
            self.next_spawn_tick =
                self.ticks + (crate::spawn::RETRY_SECONDS / self.world.dt()).ceil() as u64;
        }
        Ok(())
    }

    fn join(&mut self, join: &PendingJoin) -> Result<bool, String> {
        let slot = join.slot;
        let waited = self.ticks.saturating_sub(join.requested_tick) as f64 * self.world.dt();
        let admitted = if join.reset_kills {
            self.world
                .connect_new_player_waited(slot, join.appearance.clone(), waited)
        } else {
            self.world
                .connect_waited(slot, join.appearance.clone(), waited)
        };
        match admitted {
            Ok(false) => return Ok(false),
            Ok(true) => {
                self.last_spawn_tick = Some(self.ticks);
                if join.protocol13 {
                    self.world.protocol13(slot);
                    if join.full_minimap {
                        self.world.full_minimap(slot);
                    }
                }
                if join.physical_food {
                    self.world.physical_food(slot);
                }
                if join.session_id
                    && let Err(reason) = self.world.request_session_id(slot)
                {
                    self.close(slot, reason)?;
                }
            }
            Err(reason) => self.close(slot, reason)?,
        }
        Ok(true)
    }

    /// Client faults close only that session; errors are simulator failures.
    fn apply(&mut self, command: Command) -> Result<(), String> {
        let slot = command.slot();
        if slot < self.max_clients {
            self.shared.pending[slot].fetch_sub(1, Ordering::Relaxed);
        }
        match command {
            Command::Connect {
                slot,
                generation,
                appearance,
                session_id,
                physical_food,
                protocol13,
                full_minimap,
            } => {
                let reset_kills = slot < self.max_clients
                    && (generation != self.generations[slot]
                        || self.waiting.iter().any(|join| {
                            join.slot == slot && join.generation == generation && join.reset_kills
                        }));
                self.generations[slot] = generation;
                let join = PendingJoin {
                    slot,
                    generation,
                    appearance,
                    session_id,
                    physical_food,
                    protocol13,
                    full_minimap,
                    requested_tick: self.ticks,
                    reset_kills,
                };
                if self.world.waits_for_spawn() {
                    // A bot has reset its decoder for this generation. Retire
                    // the previous life and queued frames before it waits, or
                    // old death/gameplay frames would arrive before new setup.
                    self.world.disconnect(slot)?;
                    self.waiting.retain(|pending| pending.slot != slot);
                    self.waiting.push_back(join);
                } else {
                    self.join(&join)?;
                }
            }
            Command::Message {
                slot,
                generation,
                bytes,
                sent,
            } if generation == self.generations[slot] => {
                if self.waiting.iter().any(|join| join.slot == slot) {
                    return Ok(());
                }
                let sent = self.world_time(sent);
                if let Err(reason) = self.world.receive_at(slot, &bytes, sent) {
                    self.close(slot, reason)?;
                }
            }
            Command::Victory {
                slot,
                generation,
                text,
                sent,
            } if generation == self.generations[slot] => {
                let sent = self.world_time(sent);
                if let Err(reason) = self.world.victory_at(slot, &text, sent) {
                    self.close(slot, reason)?;
                }
            }
            Command::Disconnect { slot, generation } if generation == self.generations[slot] => {
                self.waiting
                    .retain(|join| join.slot != slot || join.generation != generation);
                self.generations[slot] = 0;
                self.world.disconnect(slot)?;
            }
            Command::Message { .. } | Command::Victory { .. } | Command::Disconnect { .. } => {}
        }
        Ok(())
    }

    fn close(&mut self, slot: usize, reason: String) -> Result<(), String> {
        self.waiting.retain(|join| join.slot != slot);
        self.world.disconnect(slot)?;
        let generation = std::mem::take(&mut self.generations[slot]);
        let route = self.route(slot);
        self.batches[route].push(Delivery::Closed {
            slot,
            generation,
            reason,
        });
        Ok(())
    }

    /// One tick. Socket slots come first in the broadcast, and their frames
    /// reach the socket thread as soon as they are encoded: waiting for every
    /// bot's frame would add that encoding time to each player's delay.
    fn advance_tick(&mut self) -> Result<(), String> {
        let Self {
            world,
            routes,
            batches,
            generations,
            shared,
            deferred,
            max_clients,
            ..
        } = self;
        let offset = *deferred as f64 * shared.clock.dt;
        world.advance_tick_with(*max_clients, &mut |early| {
            for (slot, frames) in early {
                // Socket slots share route 0.
                batches[0].push(delivery(slot, generations[slot], frames, offset));
            }
            // A closed route means its thread ended and the host is stopping.
            let _ = routes[0].send(std::mem::take(&mut batches[0]));
            shared.wake_socket()?;
            Ok(())
        })
    }

    /// Hand every queued frame to its slot's owner, which sends it on time.
    fn hand_over(&mut self) -> Result<(), String> {
        let offset = self.deferred as f64 * self.shared.clock.dt;
        for slot in 0..self.generations.len() {
            let frames = match self.world.take(slot) {
                Ok(frames) => frames,
                Err(reason) => {
                    self.close(slot, reason)?;
                    continue;
                }
            };
            if !frames.is_empty() {
                let route = self.route(slot);
                let generation = self.generations[slot];
                self.batches[route].push(delivery(slot, generation, frames, offset));
            }
        }
        for (index, (route, batch)) in self.routes.iter().zip(&mut self.batches).enumerate() {
            if !batch.is_empty() {
                // A closed route means its thread ended and the host is stopping.
                let _ = route.send(std::mem::take(batch));
                if index == 0 {
                    self.shared.wake_socket()?;
                }
            }
        }
        Ok(())
    }
}

/// Frames for a slot's owner, moved from world to wall time by `offset`.
fn delivery(
    slot: usize,
    generation: u64,
    mut frames: Vec<(f64, Vec<u8>)>,
    offset: f64,
) -> Delivery {
    for (at, _) in &mut frames {
        *at += offset;
    }
    Delivery::Frames {
        slot,
        generation,
        frames,
    }
}

/// Header placed on every outgoing message of one connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Framing {
    /// Two-byte elapsed milliseconds, as generated by the simulator.
    Elapsed,
    /// No header; requested by the stock client's default `[1]`.
    Bare,
    /// Two-byte message counter; requested by `[2]`.
    Sequence(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Before `[99, 0]` or a direct login; framing requests are accepted here.
    Hello {
        framed: bool,
    },
    Reply,
    Login,
    Playing(usize),
    Closing(Instant),
}

struct Upgrade(Arc<AtomicBool>);

impl tungstenite::handshake::server::Callback for Upgrade {
    fn on_request(self, request: &Request, response: Response) -> Result<Response, ErrorResponse> {
        self.0.store(
            request
                .headers()
                .get(PHYSICAL_FOOD_HEADER)
                .is_some_and(|value| value == "physical"),
            Ordering::Relaxed,
        );
        upgrade(request, response)
    }
}

#[expect(
    clippy::result_large_err,
    reason = "tungstenite's header callback fixes the error type"
)]
fn upgrade(request: &Request, response: Response) -> Result<Response, ErrorResponse> {
    if request.uri().path() == PATH {
        return Ok(response);
    }
    let mut error = ErrorResponse::new(Some("Not found".into()));
    *error.status_mut() = StatusCode::NOT_FOUND;
    Err(error)
}

fn socket_config() -> WebSocketConfig {
    WebSocketConfig::default()
        .read_buffer_size(MAX_INPUT_BYTES)
        .write_buffer_size(WRITE_BUFFER_BYTES)
        .max_write_buffer_size(MAX_SEND_BUFFER_BYTES)
        .max_message_size(Some(MAX_INPUT_BYTES))
        .max_frame_size(Some(MAX_INPUT_BYTES))
}

/// The socket thread: listener, sessions and socket-slot admission.
struct Gateway {
    listener: TcpListener,
    sessions: Vec<Session>,
    #[cfg(target_os = "linux")]
    readiness: readiness::Readiness,
    gate: Gate,
    deliveries: Receiver<Vec<Delivery>>,
}

/// Admission state shared by all sessions.
struct Gate {
    occupied: Vec<bool>,
    /// Last session generation per socket slot.
    generations: Vec<u64>,
    /// The `6` message with an elapsed header, transformed per connection.
    challenge: Vec<u8>,
    /// Exact accepted replies: the literal-copy compatibility profile and the
    /// rotated reply observed from the native and tested stock clients.
    replies: [Vec<u8>; 2],
    commands: Sender<Command>,
    shared: Arc<Shared>,
}

impl Gate {
    /// Send a socket slot's command; the slot counts it until consumed. Only
    /// this thread adds to the count, so a read of it is never too low.
    fn send(&self, command: Command) {
        self.shared.pending[command.slot()].fetch_add(1, Ordering::Relaxed);
        // A closed channel means the simulation ended and the host is stopping.
        let _ = self.commands.send(command);
    }

    /// Bound queued commands by `REMOTE_IN_FLIGHT` per slot, even while the
    /// simulation stalls: the world's own limit applies only on consumption.
    fn forward(&self, command: Command) -> Result<(), String> {
        if self.shared.pending[command.slot()].load(Ordering::Relaxed) >= REMOTE_IN_FLIGHT {
            return Err("Remote connection has too many undelivered messages".into());
        }
        self.send(command);
        Ok(())
    }

    /// A free slot whose earlier commands were all consumed, so login churn
    /// cannot queue commands faster than the simulation takes them.
    fn free_slot(&self) -> Option<usize> {
        (0..self.occupied.len()).find(|&slot| {
            !self.occupied[slot] && self.shared.pending[slot].load(Ordering::Relaxed) == 0
        })
    }
}

/// Bound work below the parser: one message may contain arbitrarily many
/// fragments. WouldBlock preserves its partial frame/message for the next poll.
struct ReadBudget<S> {
    inner: S,
    bytes: usize,
    calls: usize,
    #[cfg(target_os = "linux")]
    read_blocked: bool,
}

impl<S> ReadBudget<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            bytes: MAX_RAW_READ_BYTES,
            calls: MAX_RAW_READ_CALLS,
            #[cfg(target_os = "linux")]
            read_blocked: false,
        }
    }

    fn rearm(&mut self) {
        self.bytes = MAX_RAW_READ_BYTES;
        self.calls = MAX_RAW_READ_CALLS;
        // Keep read_blocked: a fairness yield does not establish that the
        // transport is empty, and rearming must not trigger another empty read.
    }
}

impl<S: io::Read> io::Read for ReadBudget<S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.bytes == 0 || self.calls == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.calls -= 1;
        let limit = bytes.len().min(self.bytes);
        let result = self.inner.read(&mut bytes[..limit]);
        #[cfg(target_os = "linux")]
        {
            self.read_blocked = result
                .as_ref()
                .is_err_and(|error| error.kind() == io::ErrorKind::WouldBlock);
        }
        let read = result?;
        self.bytes -= read;
        Ok(read)
    }
}

impl<S: io::Write> io::Write for ReadBudget<S> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

struct Open<S = TcpStream> {
    socket: WebSocket<ReadBudget<S>>,
    phase: Phase,
    framing: Framing,
    want_session_id: bool,
    legacy_reply: Option<Vec<u8>>,
    full_minimap: bool,
    physical_food: bool,
    generation: u64,
    /// Frames from the world waiting for their delivery time, in order.
    queued: VecDeque<(f64, Vec<u8>)>,
    queued_bytes: usize,
}

enum Stage {
    Http(Box<MidHandshake<ServerHandshake<ReadBudget<TcpStream>, Upgrade>>>),
    Open(Box<Open>),
    Closed,
}

struct Session {
    accepted: Instant,
    physical_food: Arc<AtomicBool>,
    stage: Stage,
}

impl Gateway {
    fn run(&mut self) -> Result<(), String> {
        while !self.gate.shared.stopped() {
            self.poll()?;
            let next = self
                .sessions
                .iter()
                .filter_map(|session| match &session.stage {
                    Stage::Open(open) => open.queued.front().map(|(at, _)| *at),
                    _ => None,
                })
                .fold(f64::INFINITY, f64::min);
            // Measured loopback polling added ~1 ms after encoding a frame.
            // Wake when a frame arrives instead of waiting out the poll sleep;
            // future frames still wait for their modeled due time. Windows uses
            // a high-resolution timer because channel timeouts delayed inputs.
            // research/fidelity/2026-10-27-host-frame-timing/REPORT.md
            if next.is_finite() {
                self.gate.shared.clock.sleep_until(next, MAX_IDLE);
            } else {
                #[cfg(windows)]
                self.gate
                    .shared
                    .socket_wake
                    .wait(MAX_IDLE)
                    .map_err(|e| e.to_string())?;
                #[cfg(not(windows))]
                if let Ok(batch) = self.deliveries.recv_timeout(MAX_IDLE) {
                    let now = Instant::now();
                    for delivery in batch {
                        self.deliver(delivery, now);
                    }
                }
            }
        }
        Ok(())
    }

    /// Accept and service sockets, take the world's frames and send those due.
    /// An error is a host failure; client faults only close that client.
    fn poll(&mut self) -> Result<(), String> {
        // Newly accepted sessions keep this poll's budget through first service.
        for session in &mut self.sessions {
            session.rearm();
        }
        let now = Instant::now();
        self.accept(now);
        while let Ok(batch) = self.deliveries.try_recv() {
            for delivery in batch {
                self.deliver(delivery, now);
            }
        }
        let wall = self.gate.shared.clock.wall(now);
        #[cfg(target_os = "linux")]
        {
            use std::os::fd::AsRawFd;
            self.readiness
                .poll(self.sessions.iter().map(|session| match &session.stage {
                    Stage::Open(open) => Some(open.socket.get_ref().inner.as_raw_fd()),
                    _ => None,
                }));
        }
        for (index, session) in self.sessions.iter_mut().enumerate() {
            #[cfg(target_os = "linux")]
            let readable = self.readiness.readable(index);
            #[cfg(not(target_os = "linux"))]
            let readable = {
                let _ = index;
                true
            };
            session.service(&mut self.gate, now, wall, readable)?;
        }
        for session in &mut self.sessions {
            let Stage::Open(open) = &mut session.stage else {
                continue;
            };
            if !open.flush(&mut self.gate) {
                session.stage = Stage::Closed;
            }
        }
        self.sessions
            .retain(|session| !matches!(session.stage, Stage::Closed));
        let players = self.gate.occupied.iter().filter(|&&used| used).count();
        self.gate.shared.players.store(players, Ordering::Relaxed);
        Ok(())
    }

    /// Queue a delivery on its session. Deliveries for a session that already
    /// ended are dropped.
    fn deliver(&mut self, delivery: Delivery, now: Instant) {
        let (slot, generation) = delivery.session();
        let Some(open) = self
            .sessions
            .iter_mut()
            .find_map(|session| match &mut session.stage {
                Stage::Open(open)
                    if open.phase == Phase::Playing(slot) && open.generation == generation =>
                {
                    Some(open)
                }
                _ => None,
            })
        else {
            return;
        };
        match delivery {
            Delivery::Frames { frames, .. } => {
                let used = frames.iter().fold(open.queued_bytes, |used, (_, bytes)| {
                    used.saturating_add(output_cost(bytes))
                });
                if used > MAX_REMOTE_OUTPUT_BYTES {
                    open.reject(&mut self.gate, now, "Client is reading too slowly");
                } else {
                    open.queued_bytes = used;
                    open.queued.extend(frames);
                }
            }
            Delivery::Closed { reason, .. } => open.reject(&mut self.gate, now, &reason),
        }
    }

    fn accept(&mut self, now: Instant) {
        for _ in 0..MAX_ACCEPTS_PER_POLL {
            let stream = match self.listener.accept() {
                Ok((stream, _)) => stream,
                // Transient accept failures (aborted peers, descriptor limits)
                // are retried on a later poll without affecting the world.
                Err(_) => break,
            };
            let pending = self
                .sessions
                .iter()
                .filter(|session| !session.joined())
                .count();
            if pending >= MAX_PENDING
                || stream.set_nonblocking(true).is_err()
                || stream.set_nodelay(true).is_err()
            {
                continue;
            }
            let physical_food = Arc::new(AtomicBool::new(false));
            let stage = match tungstenite::accept_hdr_with_config(
                ReadBudget::new(stream),
                Upgrade(physical_food.clone()),
                Some(socket_config()),
            ) {
                Ok(socket) => Stage::Open(Box::new(Open::new(
                    socket,
                    physical_food.load(Ordering::Relaxed),
                ))),
                Err(HandshakeError::Interrupted(mid)) => Stage::Http(Box::new(mid)),
                Err(HandshakeError::Failure(_)) => Stage::Closed,
            };
            self.sessions.push(Session {
                accepted: now,
                physical_food,
                stage,
            });
        }
    }
}

impl Session {
    fn rearm(&mut self) {
        match &mut self.stage {
            Stage::Http(mid) => mid.get_mut().get_mut().rearm(),
            Stage::Open(open) => open.socket.get_mut().rearm(),
            Stage::Closed => {}
        }
    }

    fn joined(&self) -> bool {
        matches!(&self.stage, Stage::Open(open) if matches!(open.phase, Phase::Playing(_)))
    }

    fn service(
        &mut self,
        gate: &mut Gate,
        now: Instant,
        wall: f64,
        readable: bool,
    ) -> Result<(), String> {
        let age = now.duration_since(self.accepted);
        self.stage = match std::mem::replace(&mut self.stage, Stage::Closed) {
            Stage::Http(_) if age >= HTTP_TIMEOUT => Stage::Closed,
            Stage::Http(mid) => match mid.handshake() {
                Ok(socket) => Stage::Open(Box::new(Open::new(
                    socket,
                    self.physical_food.load(Ordering::Relaxed),
                ))),
                Err(HandshakeError::Interrupted(mid)) => Stage::Http(Box::new(mid)),
                Err(HandshakeError::Failure(_)) => Stage::Closed,
            },
            Stage::Open(mut open) => {
                if open.service(gate, now, wall, age, readable)? {
                    Stage::Open(open)
                } else {
                    Stage::Closed
                }
            }
            Stage::Closed => Stage::Closed,
        };
        Ok(())
    }
}

impl<S: io::Read + io::Write> Open<S> {
    fn new(socket: WebSocket<ReadBudget<S>>, physical_food: bool) -> Self {
        #[cfg(target_os = "linux")]
        let mut socket = socket;
        // An upgrade may already hold WebSocket bytes from the handshake.
        #[cfg(target_os = "linux")]
        {
            socket.get_mut().read_blocked = false;
        }
        Self {
            socket,
            phase: Phase::Hello { framed: false },
            framing: Framing::Elapsed,
            want_session_id: false,
            legacy_reply: None,
            full_minimap: false,
            physical_food,
            generation: 0,
            queued: VecDeque::new(),
            queued_bytes: 0,
        }
    }

    /// Read within budget and send due frames. Returns false once closed.
    fn service(
        &mut self,
        gate: &mut Gate,
        now: Instant,
        wall: f64,
        age: Duration,
        readable: bool,
    ) -> Result<bool, String> {
        if let Phase::Closing(deadline) = self.phase {
            return Ok(now < deadline && self.drain_close());
        }
        if !matches!(self.phase, Phase::Playing(_)) && age >= LOGIN_TIMEOUT {
            self.reject(gate, now, "Login timeout");
            return Ok(true);
        }
        #[cfg(target_os = "linux")]
        let reads = if readable || !self.socket.get_ref().read_blocked {
            MAX_READS_PER_POLL
        } else {
            0
        };
        #[cfg(not(target_os = "linux"))]
        let reads = {
            let _ = readable;
            MAX_READS_PER_POLL
        };
        for _ in 0..reads {
            // Only a real transport WouldBlock permits waiting for readiness.
            // A parser write stall or a fairness budget can leave buffered input.
            #[cfg(target_os = "linux")]
            {
                self.socket.get_mut().read_blocked = false;
            }
            match self.socket.read() {
                Ok(Message::Binary(bytes)) => {
                    if let Err(reason) = self.receive(gate, &bytes, wall) {
                        self.reject(gate, now, &reason);
                        return Ok(true);
                    }
                }
                Ok(Message::Text(_)) => {
                    self.reject(gate, now, "Expected binary messages");
                    return Ok(true);
                }
                Ok(Message::Close(_)) => {
                    // tungstenite queued the close reply; flush it while closing.
                    self.release(gate);
                    self.phase = Phase::Closing(now + CLOSE_TIMEOUT);
                    return Ok(true);
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
                    break;
                }
                Err(tungstenite::Error::Capacity(_)) => {
                    // tungstenite keeps the oversized frame header after this
                    // error; reading again trips its buffer invariant. Send
                    // the close once and drop the socket without another read.
                    self.reject(gate, now, "Message too large");
                    let _ = self.socket.flush();
                    return Ok(false);
                }
                Err(_) => {
                    self.release(gate);
                    return Ok(false);
                }
            }
        }
        self.send_due(gate, now, wall)?;
        Ok(true)
    }

    /// Apply one client message received at wall time `wall`. An error
    /// closes only this connection.
    fn receive(&mut self, gate: &mut Gate, bytes: &[u8], wall: f64) -> Result<(), String> {
        match (self.phase, bytes) {
            (Phase::Hello { framed: false }, [1]) => self.request(Framing::Bare),
            (Phase::Hello { framed: false }, [2]) => self.request(Framing::Sequence(1)),
            (Phase::Hello { .. }, [1 | 2]) => Err("Repeated framing request".into()),
            (Phase::Hello { .. }, [4]) if !self.want_session_id => {
                self.want_session_id = true;
                Ok(())
            }
            (Phase::Hello { .. }, [4]) => Err("Repeated session-ID request".into()),
            (Phase::Hello { .. }, [99, 0]) => {
                self.phase = Phase::Reply;
                let challenge = gate.challenge.clone();
                self.send(challenge)
            }
            // Client-derived: NTL's protocol toggle uses [99], and disabling
            // its dynamic map uses [99,3]. All share the same reply solver.
            (Phase::Hello { .. }, [99] | [99, 3 | 4]) => {
                // Client-derived: NTL requests [99,3] with dynmm off. Its dot
                // then uses setup grd, so only this connection needs a full map.
                self.full_minimap = bytes == [99, 3];
                let (challenge, reply) = legacy_challenge();
                self.legacy_reply = Some(reply);
                self.phase = Phase::Reply;
                self.send(challenge)
            }
            (Phase::Hello { .. } | Phase::Login, [b's', ..]) => self.join(gate, bytes),
            (Phase::Hello { .. }, _) => Err("Expected a login request".into()),
            (Phase::Reply, _)
                if self.legacy_reply.as_ref().map_or_else(
                    || gate.replies.iter().any(|reply| reply == bytes),
                    |reply| reply == bytes,
                ) =>
            {
                self.phase = Phase::Login;
                Ok(())
            }
            (Phase::Reply, _) => Err("Invalid challenge reply".into()),
            (Phase::Login, _) => Err("Expected a login".into()),
            (Phase::Playing(slot), [255, 118, text @ ..])
                if text.len() <= MAX_VICTORY_TEXT && text.is_ascii() =>
            {
                gate.forward(Command::Victory {
                    slot,
                    generation: self.generation,
                    text: text.to_vec(),
                    sent: wall,
                })
            }
            (Phase::Playing(slot), _) if super::simulation::client_message(bytes) => {
                gate.forward(Command::Message {
                    slot,
                    generation: self.generation,
                    bytes: bytes.to_vec(),
                    sent: wall,
                })
            }
            (Phase::Playing(_), _) => Err("Unsupported client message".into()),
            (Phase::Closing(_), _) => Ok(()),
        }
    }

    fn request(&mut self, framing: Framing) -> Result<(), String> {
        self.framing = framing;
        self.phase = Phase::Hello { framed: true };
        Ok(())
    }

    /// Claim a free slot and ask the world to connect it. The world answers
    /// with setup, or closes the session if it cannot admit the snake.
    fn join(&mut self, gate: &mut Gate, login: &[u8]) -> Result<(), String> {
        let appearance = parse_login(login)?;
        let slot = gate.free_slot().ok_or("Server is full")?;
        gate.occupied[slot] = true;
        gate.generations[slot] += 1;
        self.generation = gate.generations[slot];
        self.phase = Phase::Playing(slot);
        gate.send(Command::Connect {
            slot,
            generation: self.generation,
            appearance,
            session_id: self.want_session_id,
            physical_food: self.physical_food,
            protocol13: legacy_login(login),
            full_minimap: self.full_minimap,
        });
        Ok(())
    }

    /// Send queued frames due by wall time `wall`. Only host faults are errors.
    fn send_due(&mut self, gate: &mut Gate, now: Instant, wall: f64) -> Result<(), String> {
        while self.queued.front().is_some_and(|(at, _)| *at <= wall) {
            let (_, message) = self.queued.pop_front().unwrap();
            self.queued_bytes -= output_cost(&message);
            if message.len() < 3 {
                return Err("Simulator emitted a message without a framed payload".into());
            }
            if let Err(reason) = self.send(message) {
                self.reject(gate, now, &reason);
                break;
            }
        }
        Ok(())
    }

    /// Queue one elapsed-framed message with this connection's header.
    fn send(&mut self, mut framed: Vec<u8>) -> Result<(), String> {
        if let Framing::Sequence(next) = &mut self.framing {
            framed[..2].copy_from_slice(&next.to_be_bytes());
            *next = next.wrapping_add(1);
        }
        // The delivery queue owns this allocation. Keep it through tungstenite;
        // bare framing can also drop its header by slicing the owned bytes.
        let mut bytes = Bytes::from(framed);
        if self.framing == Framing::Bare {
            bytes = bytes.slice(2..);
        }
        match self.socket.write(Message::Binary(bytes)) {
            Ok(()) => Ok(()),
            // The frame stays buffered; a later flush continues it.
            Err(tungstenite::Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
                Ok(())
            }
            Err(tungstenite::Error::WriteBufferFull(_)) => {
                Err("Client is reading too slowly".into())
            }
            Err(error) => Err(error.to_string()),
        }
    }

    /// Write buffered output. Returns false once the connection ended.
    fn flush(&mut self, gate: &mut Gate) -> bool {
        match self.socket.flush() {
            Ok(()) => true,
            Err(tungstenite::Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => true,
            Err(_) => {
                self.release(gate);
                false
            }
        }
    }

    /// Continue a close handshake. Returns false once the socket is finished.
    fn drain_close(&mut self) -> bool {
        for _ in 0..MAX_READS_PER_POLL {
            match self.socket.read() {
                Ok(_) => {}
                Err(tungstenite::Error::Io(error)) if error.kind() == io::ErrorKind::WouldBlock => {
                    return true;
                }
                Err(_) => return false,
            }
        }
        true
    }

    fn release(&mut self, gate: &mut Gate) {
        if let Phase::Playing(slot) = self.phase {
            gate.send(Command::Disconnect {
                slot,
                generation: self.generation,
            });
            gate.occupied[slot] = false;
            self.queued.clear();
            self.queued_bytes = 0;
        }
    }

    fn reject(&mut self, gate: &mut Gate, now: Instant, reason: &str) {
        self.release(gate);
        self.phase = Phase::Closing(now + CLOSE_TIMEOUT);
        let mut end = reason.len().min(MAX_CLOSE_REASON);
        while !reason.is_char_boundary(end) {
            end -= 1;
        }
        let _ = self.socket.close(Some(CloseFrame {
            code: CloseCode::Policy,
            reason: reason[..end].to_owned().into(),
        }));
    }
}

/// One bot thread: received-frame clients sharing stateless policy scratch.
struct Bots {
    /// Every `stride`th slot from the first seat's.
    seats: Vec<Seat>,
    policy: ForagerBot,
    stride: usize,
    deliveries: Receiver<Vec<Delivery>>,
    commands: Sender<Command>,
    shared: Arc<Shared>,
    latency: LatencyConfig,
    random: Random,
    /// Seconds between one bot's decisions.
    interval: f64,
}

/// A background client that decides from delivered frames every interval, sends
/// after processing, pings on the client's cadence and rejoins after death,
/// matching the simulator's native background client.
struct Seat {
    slot: usize,
    /// Position among bot seats, which follow the player slots; picks the
    /// seat's skin and name.
    bot: usize,
    generation: u64,
    state: ClientState,
    /// Processing draws for this connection.
    link: Link,
    frames: VecDeque<(f64, Vec<u8>)>,
    /// Latest local time used; the client clock never runs backwards.
    clock: f64,
    decide_at: f64,
    /// Decision time, action and processing time of the pending command.
    outbox: Option<(f64, Action, f64)>,
    previous_boost: Option<bool>,
    /// The world closed the session; rejoin at the next decision.
    closed: bool,
}

impl Bots {
    fn run(&mut self) -> Result<(), String> {
        let start = self.shared.clock.wall(Instant::now());
        for seat in &mut self.seats {
            seat.decide_at += start;
            seat.join(start, &self.commands, &self.latency, &mut self.random)?;
        }
        while !self.shared.stopped() {
            while let Ok(batch) = self.deliveries.try_recv() {
                for delivery in batch {
                    self.deliver(delivery);
                }
            }
            let now = self.shared.clock.wall(Instant::now());
            let mut next = f64::INFINITY;
            for seat in &mut self.seats {
                seat.run(
                    now,
                    &self.commands,
                    &self.latency,
                    &mut self.random,
                    self.interval,
                    &mut self.policy,
                )?;
                next = next.min(seat.next_event());
            }
            self.shared.clock.sleep_until(next, MAX_IDLE);
        }
        Ok(())
    }

    fn deliver(&mut self, delivery: Delivery) {
        let (slot, generation) = delivery.session();
        let first = self.seats[0].slot;
        let Some(seat) = self
            .seats
            .get_mut((slot - first) / self.stride)
            .filter(|seat| seat.generation == generation)
        else {
            return;
        };
        match delivery {
            Delivery::Frames { .. } if seat.closed => {}
            Delivery::Frames { frames, .. } => {
                if seat.frames.len() + frames.len() > MAX_QUEUED_FRAMES {
                    // A worker this far behind ends the session and rejoins,
                    // as a socket client reading too slowly is closed.
                    let _ = self.commands.send(Command::Disconnect { slot, generation });
                    seat.close(self.shared.clock.wall(Instant::now()));
                } else {
                    seat.frames.extend(frames);
                }
            }
            Delivery::Closed { .. } => seat.close(self.shared.clock.wall(Instant::now())),
        }
    }
}

impl Seat {
    /// `decide_at` is relative to the start until the thread runs.
    fn new(slot: usize, bot: usize, interval: f64) -> Self {
        Self {
            slot,
            bot,
            generation: 0,
            state: ClientState::new(),
            link: Link::default(),
            frames: VecDeque::new(),
            clock: 0.,
            // Spread decisions over the interval, as independent clients do.
            decide_at: interval * (slot % 16) as f64 / 16.,
            outbox: None,
            previous_boost: None,
            closed: false,
        }
    }

    /// Drop in-flight work. Unadmitted bots back off; established lives retain
    /// their normal next-decision rejoin after a closure or death.
    fn close(&mut self, now: f64) {
        if self.state.status() == ClientStatus::Waiting {
            self.decide_at = self.decide_at.max(now + BOT_ADMISSION_RETRY_SECONDS);
        }
        self.closed = true;
        self.frames.clear();
        self.outbox = None;
        self.state.disconnect();
    }

    fn next_event(&self) -> f64 {
        let frame = self.frames.front().map_or(f64::INFINITY, |(at, _)| *at);
        let leave = self
            .outbox
            .map_or(f64::INFINITY, |(at, _, processing)| at + processing);
        frame.min(leave).min(self.decide_at)
    }

    /// Advance to the local time `at`, never backwards.
    fn tick(&mut self, at: f64) -> f64 {
        self.clock = self.clock.max(at);
        self.clock
    }

    /// Client events up to world time `now`, in time order: deliveries, then
    /// departures, then decisions; then a ping if its cadence asks for one.
    fn run(
        &mut self,
        now: f64,
        commands: &Sender<Command>,
        latency: &LatencyConfig,
        random: &mut Random,
        interval: f64,
        policy: &mut ForagerBot,
    ) -> Result<(), String> {
        loop {
            let frame = self.frames.front().map_or(f64::INFINITY, |(at, _)| *at);
            let leave = self
                .outbox
                .map_or(f64::INFINITY, |(at, _, processing)| at + processing);
            let decide = self.decide_at;
            let first = frame.min(leave).min(decide);
            if first > now {
                break;
            }
            let at = self.tick(first);
            if frame == first {
                let (_, bytes) = self.frames.pop_front().unwrap();
                if self.state.receive_frame(&bytes, at).is_err() {
                    // Decoder capacity and malformed frames are this client's
                    // failure, just like its delivery-queue overflow.
                    let _ = commands.send(Command::Disconnect {
                        slot: self.slot,
                        generation: self.generation,
                    });
                    self.close(at);
                }
            } else if leave == first {
                self.transmit(at, commands)?;
            } else {
                // A late thread makes at most one catch-up decision.
                self.decide_at = (decide + interval).max(now);
                self.decide(at, commands, latency, random, policy)?;
            }
        }
        let at = self.tick(now);
        if !self.closed && self.state.ping_due(at)? {
            self.state.record_sent(&[251], at)?;
            self.command(commands, vec![251], at);
        }
        Ok(())
    }

    fn decide(
        &mut self,
        at: f64,
        commands: &Sender<Command>,
        latency: &LatencyConfig,
        random: &mut Random,
        policy: &mut ForagerBot,
    ) -> Result<(), String> {
        if self.closed || self.state.status() == ClientStatus::Dead {
            return self.join(at, commands, latency, random);
        }
        if self.state.status() != ClientStatus::Playing || self.outbox.is_some() {
            return Ok(());
        }
        let frame = self.state.frame(at)?;
        if let Some(action) = policy.decide(&frame) {
            let processing = self.link.processing(latency, random);
            self.outbox = Some((at, action, processing));
        }
        Ok(())
    }

    fn transmit(&mut self, at: f64, commands: &Sender<Command>) -> Result<(), String> {
        let Some((_, action, processing)) = self.outbox.take() else {
            return Ok(());
        };
        let packets = protocol::action_packets(action, self.previous_boost)?;
        self.previous_boost = Some(action.boost);
        self.state.set_processing_seconds(processing)?;
        for packet in packets {
            self.state.record_sent(&packet, at)?;
            self.command(commands, packet, at);
        }
        Ok(())
    }

    /// Start a new session and life. A full world may refuse it; the bot
    /// then backs off before its next decision.
    fn join(
        &mut self,
        at: f64,
        commands: &Sender<Command>,
        latency: &LatencyConfig,
        random: &mut Random,
    ) -> Result<(), String> {
        // The world may stamp this session's first frames earlier than `at`
        // while it catches up on ticks; the client clock never goes back.
        let at = self.tick(at);
        self.state.disconnect();
        self.state.new_connection(at)?;
        self.generation += 1;
        self.link = Link::connect(latency, random);
        self.frames.clear();
        self.outbox = None;
        self.previous_boost = None;
        self.closed = false;
        // A closed channel means the simulation ended and the host is stopping.
        let _ = commands.send(Command::Connect {
            slot: self.slot,
            generation: self.generation,
            appearance: PlayerAppearance {
                nickname: bot_nickname(self.bot),
                skin: bot_skin(self.bot),
                custom_skin: Vec::new(),
            },
            session_id: false,
            physical_food: true,
            protocol13: false,
            full_minimap: false,
        });
        Ok(())
    }

    fn command(&self, commands: &Sender<Command>, bytes: Vec<u8>, sent: f64) {
        let _ = commands.send(Command::Message {
            slot: self.slot,
            generation: self.generation,
            bytes,
            sent,
        });
    }
}

/// Each bot seat keeps one stock skin across lives so the world isn't uniform.
/// The pinned client offers skins 0..=64 (`max_skin_cv`) but its picker skips
/// 42, an event skin, unless that event is active.
fn bot_skin(bot: usize) -> u8 {
    let skin = (bot % 64) as u8;
    if skin >= 42 { skin + 1 } else { skin }
}

/// Our own base names, at most 12 bytes so `bot_nickname` always fits.
const BOT_NAMES: [&str; 64] = [
    "Noodle", "Viper", "Mamba", "Ziggy", "Pixel", "Comet", "Biscuit", "Pretzel", "Wiggles",
    "Sprout", "Nimbus", "Pebble", "Zephyr", "Juniper", "Marble", "Waffle", "Bandit", "Tango",
    "Echo", "Maple", "Rocket", "Clover", "Gizmo", "Sunny", "Tofu", "Mochi", "Pickle", "Basil",
    "Indigo", "Saffron", "Cobalt", "Ember", "Frost", "Hazel", "Jasper", "Kiwi", "Lumen", "Mango",
    "Nova", "Olive", "Pepper", "Quill", "Rascal", "Sable", "Taffy", "Umber", "Velvet", "Willow",
    "Yeti", "Zinc", "Acorn", "Bolt", "Cinder", "Dune", "Fennel", "Glimmer", "Hopper", "Inky",
    "Jinx", "Koda", "Lark", "Moss", "Nugget", "Orbit",
];

/// Official servers label their bots by appending "(bot)" to an ordinary
/// player name, as seen in 2026-10 live captures. Each seat keeps one name
/// across lives so its rolling kills stay readable; seats past the list get a
/// round number, keeping names distinct up to `MAX_AGENTS`.
fn bot_nickname(bot: usize) -> Vec<u8> {
    let name = BOT_NAMES[bot % BOT_NAMES.len()];
    match bot / BOT_NAMES.len() {
        0 => format!("{name} (bot)"),
        round => format!("{name} {} (bot)", round + 1),
    }
    .into_bytes()
}

/// These legacy selectors describe the login layout, not the output version.
fn legacy_login(bytes: &[u8]) -> bool {
    matches!(bytes.get(1), Some(11 | 12))
}

/// Stock: skin/name length at 24/25; legacy: at 2/3. All end with the name,
/// two reserved bytes and custom skin. Stock bytes are interpreted as before.
fn parse_login(bytes: &[u8]) -> Result<PlayerAppearance, String> {
    let (skin, name) = if legacy_login(bytes) {
        (2, 3)
    } else if bytes.get(1) == Some(&31) {
        // Client-derived compatibility: Frontier 6.4.2 sends two version bytes
        // before skin/name, without the stock 20-byte password. Its parser has
        // the full modern (>=15) body/appearance and (>=14) food branches, so
        // this layout selector keeps protocol 19. See tests/fixtures/browser-admission.json.
        (4, 5)
    } else {
        (24, 25)
    };
    if bytes.len() < name + 3 || bytes[0] != b's' {
        return Err("Truncated login".into());
    }
    let length = usize::from(bytes[name]);
    if length > MAX_NICKNAME {
        return Err("Nickname longer than 24 bytes".into());
    }
    let start = name + 1;
    let end = start + length;
    let custom = bytes.get(end + 2..).ok_or("Truncated login")?;
    if custom.len() > MAX_CUSTOM_SKIN {
        return Err("Custom skin longer than 255 bytes".into());
    }
    Ok(PlayerAppearance {
        nickname: bytes[start..end].to_vec(),
        skin: bytes[skin],
        custom_skin: custom.to_vec(),
    })
}

/// Client-derived compatibility: NTL 9.78 extracts the first quoted literal;
/// Frontier 6.4.2 reads 24 letters at source offset 7; Precise 0.6 evaluates the
/// source with a zero-filled `idba` in scope, then rotates its contents. The
/// compact prefix and buffer fill support all three with the same reply.
/// See research/fidelity/2026-10-27-browser-admission/REPORT.md.
/// Official #177 captures instead returned the stock
/// 27-copy source even for [4], [99,4]. This deliberately serves the supported
/// client syntax, not an invented claim about official behavior. The generated
/// source is fixed data, never executed; the challenge is not authentication.
fn legacy_challenge() -> (Vec<u8>, Vec<u8>) {
    let source = b"var a='ABCDEFGHIJKLMNOPQRSTUVWX';for(var i=0;i<24;i++)idba[i]=a.charCodeAt(i);";
    let mut framed = vec![0, 0, b'6'];
    let mut key = 23_u8;
    for byte in source {
        for nibble in [byte >> 4, byte & 15] {
            framed.push(b'a' + (nibble + key) % 26);
            key = (key + 17) % 26;
        }
    }
    let mut reply = Vec::with_capacity(24);
    let mut rotation = 2;
    for letter in 0..24 {
        reply.push(b'A' + (letter + rotation) % 26);
        rotation = (rotation + 3 + letter) % 26;
    }
    (framed, reply)
}

/// Encoded challenge source and its two accepted replies.
fn challenge_parts() -> Result<(Vec<u8>, [Vec<u8>; 2]), String> {
    let (first, second) = CHALLENGE_LITERAL;
    let mut source = b"var a = \"".to_vec();
    source.extend(first);
    source.extend(b"\";a+=\"");
    source.extend(second);
    source.extend(b"\";for (var i=0;i<27;i++){idba[i]=a.charCodeAt(i);}");
    // Inverse of the solver's letter cipher: two shifted nibbles per byte.
    let mut key = 23_u8;
    let mut encoded = Vec::with_capacity(source.len() * 2);
    for byte in source {
        for nibble in [byte >> 4, byte & 15] {
            encoded.push(b'a' + (nibble + key) % 26);
            key = (key + 17) % 26;
        }
    }
    let literal = [first.as_slice(), second.as_slice()].concat();
    let solved = challenge::solve_challenge(&encoded)?;
    Ok((encoded, [literal, solved]))
}

#[cfg(test)]
mod bot_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::simulation::MAX_AGENTS;

    #[test]
    fn owned_frames_preserve_websocket_bytes_and_sequence_wrap() {
        for framing in [Framing::Elapsed, Framing::Bare, Framing::Sequence(u16::MAX)] {
            let socket = WebSocket::from_raw_socket(
                ReadBudget::new(io::Cursor::new(Vec::new())),
                tungstenite::protocol::Role::Server,
                Some(socket_config()),
            );
            let mut open = Open::new(socket, false);
            open.framing = framing;
            let mut expected = Vec::new();
            let mut sequence = u16::MAX;
            for size in [1, 123, 124, 125, 126, 65536] {
                let mut framed = vec![0x12, 0x34];
                framed.extend((0..size).map(|i| i as u8));
                let mut payload = framed.clone();
                match framing {
                    Framing::Elapsed => {}
                    Framing::Bare => {
                        payload.drain(..2);
                    }
                    Framing::Sequence(_) => {
                        payload[..2].copy_from_slice(&sequence.to_be_bytes());
                        sequence = sequence.wrapping_add(1);
                    }
                }
                expected.push(0x82);
                match payload.len() {
                    n @ 0..=125 => expected.push(n as u8),
                    n @ 126..=65535 => {
                        expected.push(126);
                        expected.extend((n as u16).to_be_bytes());
                    }
                    n => {
                        expected.push(127);
                        expected.extend((n as u64).to_be_bytes());
                    }
                }
                expected.extend(payload);
                open.send(framed).unwrap();
            }
            open.socket.flush().unwrap();
            assert_eq!(open.socket.get_ref().inner.get_ref(), &expected);
        }
    }

    #[test]
    fn notified_frames_keep_due_times_elapsed_bytes_and_order() {
        use tungstenite::protocol::Role;
        let socket = WebSocket::from_raw_socket(
            ReadBudget::new(io::Cursor::new(Vec::new())),
            Role::Server,
            Some(socket_config()),
        );
        let mut open = Open::new(socket, false);
        let (_, mut gate, _) = connected(LatencyConfig::default(), 0);
        let first = vec![0, 7, b'p'];
        let second = vec![0, 14, b'p'];
        open.queued = VecDeque::from([(10., first.clone()), (11., second.clone())]);
        open.queued_bytes = output_cost(&first) + output_cost(&second);
        // A notification permits a poll; it never advances a frame's deadline.
        gate.shared.wake_socket().unwrap();
        open.send_due(&mut gate, Instant::now(), 9.).unwrap();
        assert_eq!(open.queued.len(), 2);
        open.send_due(&mut gate, Instant::now(), 10.).unwrap();
        assert_eq!(open.queued.front(), Some(&(11., second.clone())));
        assert_eq!(open.queued_bytes, output_cost(&second));
        open.send_due(&mut gate, Instant::now(), 11.).unwrap();
        assert!(open.queued.is_empty());
        assert_eq!(open.queued_bytes, 0);
        open.socket.flush().unwrap();
        let bytes = open.socket.into_inner().inner.into_inner();
        let mut client = WebSocket::from_raw_socket(io::Cursor::new(bytes), Role::Client, None);
        assert_eq!(client.read().unwrap().into_data().as_ref(), &first);
        assert_eq!(client.read().unwrap().into_data().as_ref(), &second);
    }

    #[test]
    fn largest_message_fits_behind_an_unflushed_batch() {
        let socket = WebSocket::from_raw_socket(
            ReadBudget::new(io::Cursor::new(Vec::new())),
            tungstenite::protocol::Role::Server,
            Some(socket_config()),
        );
        let mut open = Open::new(socket, false);
        // With its 4-byte header this fills the batch exactly, so it stays
        // buffered without a write.
        let batch = vec![1; WRITE_BUFFER_BYTES - 4];
        let largest = vec![2; MAX_FRAME_BYTES];
        open.send(batch.clone()).unwrap();
        open.send(largest.clone()).unwrap();
        open.socket.flush().unwrap();
        let written = open.socket.get_ref().inner.get_ref();
        assert_eq!(written.len(), WRITE_BUFFER_BYTES + MAX_FRAME_BYTES + 10);
        assert_eq!(&written[4..WRITE_BUFFER_BYTES], &batch[..]);
        assert_eq!(&written[WRITE_BUFFER_BYTES + 10..], &largest[..]);
    }

    /// Finite, always-readable input until its selected bytes have been read.
    struct Readable {
        bytes: VecDeque<u8>,
        read: usize,
        first_read: Option<usize>,
    }

    impl io::Read for Readable {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            if self.bytes.is_empty() {
                return Err(io::ErrorKind::WouldBlock.into());
            }
            let limit = if self.read == 0 {
                self.first_read.unwrap_or(usize::MAX)
            } else {
                usize::MAX
            };
            let n = bytes.len().min(self.bytes.len()).min(limit);
            for byte in &mut bytes[..n] {
                *byte = self.bytes.pop_front().unwrap();
            }
            self.read += n;
            Ok(n)
        }
    }

    impl io::Write for Readable {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn buffered_messages_resume_without_readiness_then_empty_reads_stop() {
        let (mut gateway, _simulation) = pit_gateway();
        let count = MAX_READS_PER_POLL + 1;
        // Masked binary pings, all loaded by the first transport read.
        let bytes = [130, 129, 0, 0, 0, 0, 251].repeat(count);
        let socket = WebSocket::from_raw_socket(
            ReadBudget::new(Readable {
                bytes: bytes.into(),
                read: 0,
                first_read: None,
            }),
            tungstenite::protocol::Role::Server,
            Some(socket_config()),
        );
        let mut open = Open::new(socket, false);
        open.phase = Phase::Playing(0);
        let now = Instant::now();
        open.service(&mut gateway.gate, now, 0., Duration::ZERO, true)
            .unwrap();
        assert_eq!(
            gateway.gate.shared.pending[0].load(Ordering::Relaxed),
            MAX_READS_PER_POLL
        );
        assert!(!open.socket.get_ref().read_blocked);
        open.socket.get_mut().rearm();
        open.service(&mut gateway.gate, now, 0., Duration::ZERO, false)
            .unwrap();
        assert_eq!(
            gateway.gate.shared.pending[0].load(Ordering::Relaxed),
            count
        );
        assert!(open.socket.get_ref().read_blocked);
        open.socket.get_mut().rearm();
        assert!(open.socket.get_ref().read_blocked);
        let remaining_reads = open.socket.get_ref().calls;
        let frame = vec![0, 7, b'p'];
        open.queued_bytes = output_cost(&frame);
        open.queued.push_back((0., frame));
        open.service(&mut gateway.gate, now, 0., Duration::ZERO, false)
            .unwrap();
        assert_eq!(open.socket.get_ref().calls, remaining_reads);
        assert!(
            open.queued.is_empty(),
            "unreadable sockets still send due output"
        );
    }

    fn fragmented_framing_request() -> Vec<u8> {
        let mut bytes = vec![2, 128, 0, 0, 0, 0];
        for _ in 0..100_000 {
            bytes.extend([0, 128, 0, 0, 0, 0]);
        }
        // Complete a framing request only after all the empty continuations.
        bytes.extend([128, 129, 0, 0, 0, 0, 1]);
        bytes
    }

    #[test]
    fn pipelined_upgrade_and_input_share_the_first_read_budget() {
        let (mut gateway, _simulation) = pit_gateway();
        let mut bytes = b"GET /slither HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n".to_vec();
        let header_bytes = bytes.len();
        bytes.extend(fragmented_framing_request());
        let socket = tungstenite::accept_hdr_with_config(
            ReadBudget::new(Readable {
                bytes: bytes.into(),
                read: 0,
                first_read: Some(header_bytes),
            }),
            Upgrade(Arc::new(AtomicBool::new(false))),
            Some(socket_config()),
        )
        .unwrap();
        let mut open = Open::new(socket, false);
        assert!(open.socket.get_ref().inner.read > 0);
        let now = Instant::now();
        // Acceptance and first service belong to the same gateway poll.
        open.service(&mut gateway.gate, now, 0., Duration::ZERO, true)
            .unwrap();
        assert!(open.socket.get_ref().inner.read <= MAX_RAW_READ_BYTES);
        assert_eq!(open.phase, Phase::Hello { framed: false });
        // Later polls resume the parser and eventually complete the request.
        for _ in 0..32 {
            if open.phase == (Phase::Hello { framed: true }) {
                break;
            }
            open.socket.get_mut().rearm();
            open.service(&mut gateway.gate, now, 0., Duration::ZERO, false)
                .unwrap();
        }
        assert_eq!(open.phase, Phase::Hello { framed: true });
        assert_eq!(open.framing, Framing::Bare);
    }

    #[test]
    fn fragmented_input_yields_and_resumes_within_raw_read_budget() {
        let (mut gateway, _simulation) = pit_gateway();
        let bytes = fragmented_framing_request();
        let socket = WebSocket::from_raw_socket(
            ReadBudget::new(Readable {
                bytes: bytes.into(),
                read: 0,
                first_read: None,
            }),
            tungstenite::protocol::Role::Server,
            Some(socket_config()),
        );
        let mut open = Open::new(socket, false);
        let now = Instant::now();
        open.service(&mut gateway.gate, now, 0., Duration::ZERO, true)
            .unwrap();
        assert!(open.socket.get_ref().inner.read <= 64 << 10);
        assert_eq!(open.phase, Phase::Hello { framed: false });
        for _ in 0..32 {
            if open.phase == (Phase::Hello { framed: true }) {
                break;
            }
            open.socket.get_mut().rearm();
            open.service(&mut gateway.gate, now, 0., Duration::ZERO, false)
                .unwrap();
        }
        assert_eq!(open.phase, Phase::Hello { framed: true });
        assert_eq!(open.framing, Framing::Bare);
        // Deadlines still apply with a partial fragmented message.
        open.service(&mut gateway.gate, now, 0., LOGIN_TIMEOUT, false)
            .unwrap();
        assert!(matches!(open.phase, Phase::Closing(_)));
    }

    #[test]
    fn bot_decode_failure_disconnects_only_its_generation_and_rejoins() {
        let (commands, received) = mpsc::channel();
        let latency = LatencyConfig::default();
        let mut random = Random::new(1);
        let mut policy = ForagerBot::new();
        let mut seat = Seat::new(4, 0, 1.);
        seat.join(0., &commands, &latency, &mut random).unwrap();
        let _ = received.recv().unwrap();
        seat.decide_at = 1.;
        let world = crate::Simulator::new(crate::WorldOptions {
            seed: 1,
            config: crate::Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["large".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut detached_snake = world.snakes()[0].clone();
        let snake = &mut detached_snake;
        snake.body = vec![crate::Point::default(); 430];
        let mut packets = vec![super::super::encode::setup(32000., 0.029).unwrap()];
        for id in 1..=152 {
            packets.push(
                super::super::encode::appearance(id, snake, 0., &PlayerAppearance::default())
                    .unwrap(),
            );
        }
        let full = super::super::encode::frame(0, &packets).unwrap();
        seat.frames.push_back((0., full));
        let overflow = super::super::encode::frame(
            0,
            &[
                super::super::encode::appearance(153, snake, 0., &PlayerAppearance::default())
                    .unwrap(),
            ],
        )
        .unwrap();
        let mut probe = ClientState::new();
        probe.new_connection(0.).unwrap();
        probe.receive_frame(&seat.frames[0].1, 0.).unwrap();
        assert!(
            probe
                .receive_frame(&overflow, 0.1)
                .unwrap_err()
                .contains("total body limit")
        );
        seat.frames.push_back((0.1, overflow));
        seat.outbox = Some((0., Action::default(), 0.2));
        seat.run(0.1, &commands, &latency, &mut random, 1., &mut policy)
            .unwrap();
        assert!(seat.closed && seat.frames.is_empty() && seat.outbox.is_none());
        assert!(matches!(
            received.recv().unwrap(),
            Command::Disconnect {
                slot: 4,
                generation: 1
            }
        ));
        // A different seat on the same worker still runs normally.
        let mut healthy = Seat::new(5, 0, 1.);
        healthy.join(0.1, &commands, &latency, &mut random).unwrap();
        healthy
            .run(0.1, &commands, &latency, &mut random, 1., &mut policy)
            .unwrap();
        seat.run(1., &commands, &latency, &mut random, 1., &mut policy)
            .unwrap();
        assert!(!seat.closed);
        assert_eq!(seat.generation, 2);
        assert!(received.try_iter().any(|command| matches!(
            command,
            Command::Connect {
                slot: 4,
                generation: 2,
                ..
            }
        )));
    }

    #[test]
    fn long_tick_shutdown_is_prompt() {
        const CHILD: &str = "SLITHER_LONG_TICK_SHUTDOWN_CHILD";
        if let Ok(mode) = std::env::var(CHILD) {
            let mut config = HostConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                players: PlayerConfig {
                    max: 1,
                    ..Default::default()
                },
                bots: BotConfig {
                    count: 0,
                    ..Default::default()
                },
                ..Default::default()
            };
            config.simulation.world = crate::Config::sandbox();
            config.simulation.world.dt = 60.;
            config.simulation.world.boundary_mode = crate::BoundaryMode::Hard;
            config.simulation.world.food_count = 0;
            config.simulation.world.ambient_replenish_per_second = 0.;
            let host = Host::bind(config).unwrap().start().unwrap();
            thread::sleep(Duration::from_millis(100));
            assert_eq!(host.ticks(), 0);
            assert!(!host.is_finished());
            match mode.as_str() {
                "stop" => host.stop().unwrap(),
                "drop" => drop(host),
                "failure" => {
                    host.shared
                        .failure
                        .lock()
                        .unwrap()
                        .replace("synthetic worker failure".into());
                    drop(StopOnExit(host.shared.clone()));
                    assert!(host.is_finished());
                    assert_eq!(host.wait().unwrap_err(), "synthetic worker failure");
                }
                _ => panic!("unknown shutdown mode"),
            }
            return;
        }
        for mode in ["stop", "drop", "failure"] {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "client::server::tests::long_tick_shutdown_is_prompt",
                    "--nocapture",
                ])
                .env(CHILD, mode)
                .spawn()
                .unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(status) = child.try_wait().unwrap() {
                    assert!(status.success(), "{mode}: {status}");
                    break;
                }
                if Instant::now() >= deadline {
                    child.kill().unwrap();
                    child.wait().unwrap();
                    panic!("{mode} did not stop within the watchdog deadline");
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
    }

    #[test]
    fn an_infinite_deadline_sleeps_for_the_limit() {
        let clock = Clock {
            epoch: Instant::now(),
            dt: 0.007,
            deferred: AtomicU64::new(0),
        };
        let started = Instant::now();
        clock.sleep_until(f64::INFINITY, Duration::from_millis(20));
        assert!(started.elapsed() >= Duration::from_millis(15));
    }

    #[test]
    fn a_default_host_admits_every_seat() {
        let host = HostConfig::default();
        let config = host.world_config().unwrap();
        let seats = host.players.max + host.bots.count;
        let official = crate::Config::official(host.bots.count);
        assert_eq!(config.world.radius, official.radius);
        // Startup sizing scales the floored one-seat food target by area.
        assert!(official.food_count - config.world.food_count < seats);
        assert!(
            (config.world.ambient_replenish_per_second - official.ambient_replenish_per_second)
                .abs()
                < 1e-9
        );
        let mut world = RawSimulator::new(host.seed, config, seats).unwrap();
        for slot in 0..seats {
            world.connect(slot, PlayerAppearance::default()).unwrap();
        }
    }

    #[test]
    fn host_player_and_bot_links_schedule_independently() {
        // Clock the actual host's world manually: slots 0 and 1 are a socket
        // player and a hosted bot, without wall-clock or socket jitter.
        for rtts in [[0.084, 0.], [0., 0.084]] {
            let config = HostConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                players: PlayerConfig {
                    max: 1,
                    latency: LatencyConfig::fixed(rtts[0]),
                },
                bots: BotConfig {
                    count: 1,
                    latency: LatencyConfig {
                        processing_seconds: [0.03, 0.03],
                        ..LatencyConfig::fixed(rtts[1])
                    },
                    ..Default::default()
                },
                simulation: SimulationConfig {
                    world: crate::Config::sandbox(),
                    ..Default::default()
                },
                ..Default::default()
            };
            let host = Host::bind(config.clone()).unwrap();
            assert_eq!(host.latency, config.bots.latency);
            let mut world = host.world;
            for slot in 0..2 {
                assert!(world.connect(slot, PlayerAppearance::default()).unwrap());
            }
            // Both setup and subsequent pongs traverse uplink and downlink.
            let mut tick = 0;
            for packet_type in *b"ap" {
                let sent = tick as f64 * world.dt();
                if packet_type == b'p' {
                    for slot in 0..2 {
                        world.receive_at(slot, &[251], sent).unwrap();
                    }
                }
                let mut arrivals = [None; 2];
                for _ in 0..20 {
                    world.advance_tick_with(0, &mut |_| Ok(())).unwrap();
                    tick += 1;
                    for (slot, arrival) in arrivals.iter_mut().enumerate() {
                        for (at, bytes) in world.take(slot).unwrap() {
                            if protocol::split_frame(&bytes)
                                .unwrap()
                                .iter()
                                .any(|p| p[0] == packet_type)
                            {
                                assert!(arrival.replace(at).is_none());
                            }
                        }
                    }
                }
                for (slot, arrival) in arrivals.into_iter().enumerate() {
                    let delay = arrival.expect("setup or pong") - sent;
                    assert!(delay >= rtts[slot] - 1e-9, "slot {slot}: {delay}");
                    assert!(
                        delay <= rtts[slot] + world.dt() + 1e-9,
                        "slot {slot}: {delay}"
                    );
                }
            }
        }
    }

    #[test]
    fn explicit_and_fixed_host_arenas_keep_their_radius() {
        let mut host = HostConfig::default();
        host.simulation.world.radius = 5000.;
        assert_eq!(host.world_config().unwrap().world.radius, 5000.);
        host.simulation.world = crate::Config::sandbox();
        let config = host.world_config().unwrap();
        assert_eq!(config.world.radius, 1000.);
        assert_eq!(config.world.food_count, crate::Config::sandbox().food_count);
    }

    #[test]
    fn omitted_and_official_host_worlds_start_sized_for_bots() {
        for simulation in [
            "{}",
            r#"{"world":{}}"#,
            r#"{"world":{"preset":"official"}}"#,
        ] {
            let host: HostConfig = serde_json::from_str(&format!(
                r#"{{"players":{{"max":4}},"bots":{{"count":480}},"simulation":{simulation}}}"#
            ))
            .unwrap();
            let config = host.world_config().unwrap().world;
            let radius = crate::simulator::population_radius(480);
            assert_eq!(config.radius, radius);
            assert!(config.dynamic_radius);
            assert_eq!(config.food_count, 109 * 480);
            assert_eq!(config.prey.count, 68);
            assert_eq!(config.prey.count, crate::Config::official(480).prey.count);
            assert!(
                (config.ambient_replenish_per_second
                    - crate::Config::official(480).ambient_replenish_per_second)
                    .abs()
                    < 1e-9
            );
        }
    }

    #[test]
    fn custom_host_worlds_opt_out_of_seat_sizing_and_round_trip() {
        for world in [
            r#"{"preset":"battledome","dynamicRadius":false}"#,
            r#"{"preset":"sandbox"}"#,
            r#"{"radius":900,"dynamicRadius":false}"#,
            r#"{"preset":"official","radius":900}"#,
            r#"{"sizeToPopulation":false}"#,
        ] {
            let host: HostConfig = serde_json::from_str(&format!(
                r#"{{"players":{{"max":4}},"bots":{{"count":480}},"simulation":{{"world":{world}}}}}"#
            ))
            .unwrap();
            let original = serde_json::to_value(&host.simulation.world).unwrap();
            for host in [
                host.clone(),
                serde_json::from_value(serde_json::to_value(host).unwrap()).unwrap(),
            ] {
                let resolved = host.world_config().unwrap().world;
                assert!(!resolved.size_to_population);
                assert_eq!(serde_json::to_value(resolved).unwrap(), original);
            }
        }
        // Callers may explicitly request startup sizing even with a radius.
        let host: HostConfig = serde_json::from_str(
            r#"{"simulation":{"world":{"radius":900,"sizeToPopulation":true,"dynamicRadius":true}}}"#,
        )
        .unwrap();
        assert_eq!(
            host.world_config().unwrap().world.radius,
            crate::simulator::population_radius(4)
        );
        assert!(host.world_config().unwrap().world.dynamic_radius);
    }

    #[test]
    fn population_sizing_keeps_custom_prey_density() {
        for (prey, expected) in [(0, 0), (2, 960)] {
            let host: HostConfig = serde_json::from_str(&format!(
                r#"{{"players":{{"max":4}},"bots":{{"count":480}},"simulation":{{"world":{{"prey":{{"count":{prey}}}}}}}}}"#
            ))
            .unwrap();
            let world = host.world_config().unwrap().world;
            assert_eq!(world.radius, crate::simulator::population_radius(480));
            assert_eq!(world.prey.count, expected);
        }
    }

    fn shared(dt: f64, sockets: usize) -> Arc<Shared> {
        Arc::new(Shared {
            #[cfg(windows)]
            socket_wake: wake::Wake::new().unwrap(),
            clock: Clock {
                epoch: Instant::now(),
                dt,
                deferred: AtomicU64::new(0),
            },
            stop: AtomicBool::new(false),
            ticks: AtomicU64::new(0),
            busy: AtomicU64::new(0),
            players: AtomicUsize::new(0),
            pending: (0..sockets).map(|_| AtomicUsize::new(0)).collect(),
            failure: Mutex::new(None),
        })
    }

    /// A one-socket simulation already connected in slot 0, generation 1,
    /// with the gate that sends its commands.
    fn connected(
        latency: LatencyConfig,
        deferred: u64,
    ) -> (Simulation, Gate, Receiver<Vec<Delivery>>) {
        let config = SimulationConfig {
            agents: 1,
            world: crate::Config::sandbox(),
            latency,
            ..Default::default()
        };
        let world = RawSimulator::new(1, config, 1).unwrap();
        let dt = world.dt();
        let (commands, inbox) = mpsc::channel();
        let (route, deliveries) = mpsc::channel();
        let mut simulation = Simulation {
            world,
            commands: inbox,
            routes: vec![route],
            batches: vec![Vec::new()],
            max_clients: 1,
            generations: vec![0],
            waiting: VecDeque::new(),
            next_spawn_tick: 0,
            last_spawn_tick: None,
            shared: shared(dt, 1),
            ticks: 0,
            deferred,
        };
        let gate = Gate {
            occupied: vec![true],
            generations: vec![1],
            challenge: Vec::new(),
            replies: [Vec::new(), Vec::new()],
            commands,
            shared: simulation.shared.clone(),
        };
        gate.send(Command::Connect {
            slot: 0,
            generation: 1,
            appearance: PlayerAppearance::default(),
            session_id: false,
            physical_food: true,
            protocol13: false,
            full_minimap: false,
        });
        for _ in 0..40 {
            step(&mut simulation);
        }
        while deliveries.try_recv().is_ok() {}
        (simulation, gate, deliveries)
    }

    fn step(simulation: &mut Simulation) {
        simulation.apply_commands().unwrap();
        simulation.advance_tick().unwrap();
        simulation.ticks += 1;
        simulation.hand_over().unwrap();
    }

    #[test]
    fn remote_output_overflow_closes_only_its_session_for_both_protocols() {
        for legacy in [false, true] {
            let (mut simulation, _gate, deliveries) = connected(LatencyConfig::default(), 0);
            assert!(
                simulation
                    .world
                    .connect(0, PlayerAppearance::default())
                    .unwrap()
            );
            if legacy {
                simulation.world.protocol13(0);
            }
            // Withhold handover to model an undrained world-side queue. Stock
            // bundles and legacy single pongs must reach the same byte policy,
            // without either protocol's overflow escaping the shared tick.
            for _ in 0..if legacy { 20 } else { 400 } {
                for _ in 0..REMOTE_IN_FLIGHT - 1 {
                    simulation
                        .world
                        .receive_at(0, &[251], simulation.ticks as f64 * simulation.world.dt())
                        .unwrap();
                }
                simulation
                    .world
                    .advance_tick_with(0, &mut |_| Ok(()))
                    .unwrap();
                simulation.ticks += 1;
            }
            assert_eq!(
                simulation.world.take(0).unwrap_err(),
                "Client output queue is full"
            );
            simulation.hand_over().unwrap();
            assert!(deliveries.try_iter().flatten().any(|delivery| matches!(
                delivery,
                Delivery::Closed { slot: 0, generation: 1, ref reason }
                    if reason == "Client output queue is full"
            )));
            assert_eq!(simulation.generations[0], 0);
            assert!(simulation.world.take(0).unwrap().is_empty());
            let before = simulation.ticks;
            step(&mut simulation);
            assert_eq!(simulation.ticks, before + 1);
            assert!(!simulation.shared.stopped());
        }
    }

    /// Wall delivery time of the first pong handed over, stepping at most
    /// `ticks` ticks.
    fn pong_after(
        simulation: &mut Simulation,
        deliveries: &Receiver<Vec<Delivery>>,
        ticks: usize,
    ) -> Option<f64> {
        for _ in 0..ticks {
            step(simulation);
            let pong = deliveries
                .try_iter()
                .flatten()
                .filter_map(|delivery| match delivery {
                    Delivery::Frames { frames, .. } => Some(frames),
                    Delivery::Closed { .. } => None,
                })
                .flatten()
                .find(|(_, bytes)| protocol::split_frame(bytes).unwrap().contains(&&b"p"[..]));
            if let Some((at, _)) = pong {
                return Some(at);
            }
        }
        None
    }

    fn ping(sent: f64) -> Command {
        Command::Message {
            slot: 0,
            generation: 1,
            bytes: vec![251],
            sent,
        }
    }

    #[test]
    fn deferred_ticks_keep_wall_times_other_threads_hold() {
        // World time already runs 100 ticks behind the wall clock.
        let (mut simulation, gate, deliveries) = connected(LatencyConfig::default(), 100);
        let dt = simulation.shared.clock.dt;
        // A ping sent mid-tick by the wall clock is answered in that tick, in
        // a frame due at the tick's wall time.
        let wall = (simulation.ticks + 100) as f64 * dt + dt / 2.;
        gate.send(ping(wall));
        let at = pong_after(&mut simulation, &deliveries, 1).expect("no pong in the arrival tick");
        assert!(
            (at - (simulation.ticks + 100) as f64 * dt).abs() < 1e-9,
            "{at}"
        );
    }

    #[test]
    fn deferring_keeps_in_flight_uplinks_on_their_wall_arrival() {
        let (mut simulation, gate, deliveries) = connected(LatencyConfig::one_way(0.07, 0.), 0);
        let dt = simulation.shared.clock.dt;
        let sent = simulation.ticks;
        // Arrives 10.5 ticks after it is sent, by the wall clock.
        gate.send(ping((sent as f64 + 0.5) * dt));
        step(&mut simulation);
        simulation.defer(5);
        let at = pong_after(&mut simulation, &deliveries, 20).expect("no pong");
        // Answered in the tick that ends after its wall arrival.
        assert!((at - (sent + 11) as f64 * dt).abs() < 1e-9, "{at}");
        assert_eq!(simulation.ticks, sent + 6);
    }

    #[test]
    fn a_socket_slot_waiting_on_too_many_messages_is_refused() {
        let (mut simulation, gate, _deliveries) = connected(LatencyConfig::default(), 0);
        // A stalled simulation consumes nothing.
        for _ in 0..REMOTE_IN_FLIGHT {
            gate.forward(ping(0.)).unwrap();
        }
        assert!(gate.forward(ping(0.)).is_err());
        simulation.apply_commands().unwrap();
        assert_eq!(simulation.shared.pending[0].load(Ordering::Relaxed), 0);
        gate.forward(ping(0.)).unwrap();
    }

    #[test]
    fn a_released_socket_slot_waits_for_the_simulation_to_take_its_commands() {
        let (mut simulation, mut gate, _deliveries) = connected(LatencyConfig::default(), 0);
        // A login and close, as `release` sends them, while simulation stalls.
        gate.send(Command::Disconnect {
            slot: 0,
            generation: 1,
        });
        gate.occupied[0] = false;
        assert_eq!(gate.free_slot(), None);
        simulation.apply_commands().unwrap();
        assert_eq!(gate.free_slot(), Some(0));
    }

    #[test]
    fn socket_frames_leave_before_the_bots_frames_are_handed_over() {
        let config = SimulationConfig {
            agents: 2,
            world: crate::Config::sandbox(),
            ..Default::default()
        };
        let world = RawSimulator::new(1, config, 2).unwrap();
        let dt = world.dt();
        let (commands, inbox) = mpsc::channel();
        let (socket_route, socket_deliveries) = mpsc::channel();
        let (bot_route, bot_deliveries) = mpsc::channel();
        let mut simulation = Simulation {
            world,
            commands: inbox,
            routes: vec![socket_route, bot_route],
            batches: vec![Vec::new(), Vec::new()],
            max_clients: 1,
            generations: vec![0, 0],
            waiting: VecDeque::new(),
            next_spawn_tick: 0,
            last_spawn_tick: None,
            shared: shared(dt, 1),
            ticks: 0,
            deferred: 0,
        };
        for slot in 0..2 {
            commands
                .send(Command::Connect {
                    slot,
                    generation: 1,
                    appearance: PlayerAppearance::default(),
                    session_id: false,
                    physical_food: true,
                    protocol13: false,
                    full_minimap: false,
                })
                .unwrap();
        }
        // Only the socket slot's command is counted, as `Gate::send` would.
        simulation.shared.pending[0].store(1, Ordering::Relaxed);
        for _ in 0..40 {
            step(&mut simulation);
        }
        while socket_deliveries.try_recv().is_ok() {}
        while bot_deliveries.try_recv().is_ok() {}
        for _ in 0..20 {
            simulation.apply_commands().unwrap();
            simulation.advance_tick().unwrap();
        }
        let socket_frames = socket_deliveries
            .try_iter()
            .flatten()
            .filter(|delivery| matches!(delivery, Delivery::Frames { slot: 0, .. }))
            .count();
        assert!(socket_frames > 0);
        assert!(simulation.world.take(0).unwrap().is_empty());
        // The bot's frames wait for `hand_over`.
        assert!(bot_deliveries.try_recv().is_err());
        assert!(!simulation.world.take(1).unwrap().is_empty());
    }

    #[test]
    fn refused_bot_admission_waits_a_wall_second_before_retrying() {
        let (commands, received) = mpsc::channel();
        let latency = LatencyConfig::default();
        let mut random = Random::new(1);
        let mut policy = ForagerBot::new();
        let mut seat = Seat::new(4, 0, 0.042);
        seat.join(0.1, &commands, &latency, &mut random).unwrap();
        assert!(matches!(
            received.try_recv().unwrap(),
            Command::Connect { generation: 1, .. }
        ));
        seat.close(0.2);
        seat.run(1.19, &commands, &latency, &mut random, 0.042, &mut policy)
            .unwrap();
        assert!(received.try_recv().is_err());
        seat.run(1.2, &commands, &latency, &mut random, 0.042, &mut policy)
            .unwrap();
        assert!(matches!(
            received.try_recv().unwrap(),
            Command::Connect { generation: 2, .. }
        ));
        // A second refusal starts a fresh delay from its actual receipt time,
        // including when the simulation has fallen behind the wall clock.
        seat.close(2.);
        seat.run(2.99, &commands, &latency, &mut random, 0.042, &mut policy)
            .unwrap();
        assert!(received.try_recv().is_err());
        seat.run(3., &commands, &latency, &mut random, 0.042, &mut policy)
            .unwrap();
        assert!(matches!(
            received.try_recv().unwrap(),
            Command::Connect { generation: 3, .. }
        ));
        // A reported death keeps the ordinary next-decision rejoin cadence.
        seat.state.receive_frame(&[0, 0, b'v', 0], 3.01).unwrap();
        let next_decision = seat.decide_at;
        seat.close(3.02);
        assert_eq!(seat.decide_at, next_decision);
        seat.run(3.1, &commands, &latency, &mut random, 0.042, &mut policy)
            .unwrap();
        assert!(matches!(
            received.try_recv().unwrap(),
            Command::Connect { generation: 4, .. }
        ));
    }

    fn fenced_spawn(gap: f64) -> (Simulation, Gate, Receiver<Vec<Delivery>>) {
        let (mut simulation, gate, deliveries) = connected(LatencyConfig::default(), 0);
        let mut world = crate::Config::battledome();
        world.dynamic_radius = false;
        world.spawn_radius_max = None;
        world.food_count = 0;
        world.ambient_replenish_per_second = 0.;
        simulation.world = RawSimulator::new(
            7,
            SimulationConfig {
                agents: 2,
                world,
                ..Default::default()
            },
            2,
        )
        .unwrap();
        simulation.ticks = 0;
        simulation.generations = vec![0; 2];
        simulation.next_spawn_tick = 0;
        simulation.last_spawn_tick = None;
        place_spawn_fence(&mut simulation, gap);
        (simulation, gate, deliveries)
    }

    fn place_spawn_fence(simulation: &mut Simulation, gap: f64) {
        let world = simulation.world.fixture_world();
        let radius = world.config().spawn_radius.unwrap() + gap;
        let body: Vec<_> = (0..=64)
            .map(|i| {
                let angle = std::f64::consts::TAU * i as f64 / 64.;
                crate::Point {
                    x: radius * angle.cos(),
                    y: radius * angle.sin(),
                }
            })
            .collect();
        // A closed replay body blankets the single spawn circle: every strict
        // pose is within 400 of it. Gap 350 leaves inward fallback corridors;
        // gap 150 blocks even the 200-unit fallback body floor.
        world
            .import_life(
                1,
                crate::ReplayLife {
                    head: body[0],
                    speed: crate::InitialSpeed::Infer(crate::physics::base_speed(body.len())),
                    body,
                    heading: std::f64::consts::FRAC_PI_2,
                    fam: 0.,
                    kills: 0,
                    alive: true,
                },
            )
            .unwrap();
    }

    fn advance_fenced_spawn(simulation: &mut Simulation, until: u64, gap: f64) {
        // Advance real physics/world ticks, holding only the synthetic obstacle
        // pose fixed. Admission is polled at the threshold boundaries below.
        while simulation.ticks < until {
            simulation.advance_tick().unwrap();
            simulation.ticks += 1;
            simulation.hand_over().unwrap();
            place_spawn_fence(simulation, gap);
        }
    }

    fn request_fenced_spawn(simulation: &mut Simulation, gate: &Gate, generation: u64) {
        gate.send(Command::Connect {
            slot: 0,
            generation,
            appearance: PlayerAppearance::default(),
            session_id: false,
            physical_food: true,
            protocol13: false,
            full_minimap: false,
        });
        simulation.apply_commands().unwrap();
    }

    #[test]
    fn fallback_clock_admits_only_after_two_world_seconds() {
        let (mut simulation, gate, _deliveries) = fenced_spawn(350.);
        // A nonzero request tick also distinguishes request age from world age.
        advance_fenced_spawn(&mut simulation, 16, 350.);
        let requested = simulation.ticks;
        let ready = requested + (2. / simulation.world.dt()).ceil() as u64;
        request_fenced_spawn(&mut simulation, &gate, 2);
        assert_eq!(simulation.waiting.len(), 1, "no fallback at request time");

        advance_fenced_spawn(&mut simulation, ready - 1, 350.);
        simulation.apply_commands().unwrap();
        assert_eq!(
            simulation.waiting.len(),
            1,
            "no fallback before two seconds"
        );
        assert!(!simulation.world.fixture_world().snakes()[0].alive);

        // A failed search retains the normal 14ms retry interval.
        let attempt = simulation.next_spawn_tick.max(ready);
        advance_fenced_spawn(&mut simulation, attempt, 350.);
        simulation.apply_commands().unwrap();
        assert!(
            simulation.waiting.is_empty(),
            "fallback must admit after the gate"
        );
        assert_eq!(simulation.last_spawn_tick, Some(attempt));
        let world = simulation.world.fixture_world();
        let body = &world.snakes()[0].body;
        assert!(world.snakes()[0].alive);
        assert!(!world.body_is_clear_excluding(body, 400., Some(0)));
        assert!(world.body_is_clear_excluding(body, 200., Some(0)));
    }

    #[test]
    fn fallback_clock_restarts_when_a_waiting_socket_reconnects() {
        let (mut simulation, gate, _deliveries) = fenced_spawn(150.);
        advance_fenced_spawn(&mut simulation, 16, 150.);
        let old_requested = simulation.ticks;
        request_fenced_spawn(&mut simulation, &gate, 2);
        let old_ready = old_requested + (2. / simulation.world.dt()).ceil() as u64 + 2;
        advance_fenced_spawn(&mut simulation, old_ready, 150.);
        simulation.apply_commands().unwrap();
        assert_eq!(simulation.waiting.len(), 1, "tight fence blocks both tiers");
        assert!((simulation.ticks - old_requested) as f64 * simulation.world.dt() > 2.);

        // Replace the socket through the same command path as a real reconnect.
        // The world now offers fallback-only poses, so stale age would admit it.
        gate.send(Command::Disconnect {
            slot: 0,
            generation: 2,
        });
        place_spawn_fence(&mut simulation, 350.);
        let reconnected = simulation.ticks;
        request_fenced_spawn(&mut simulation, &gate, 3);
        let ready = reconnected + (2. / simulation.world.dt()).ceil() as u64;
        assert_eq!(simulation.waiting.len(), 1);
        assert_eq!(simulation.waiting[0].generation, 3);

        advance_fenced_spawn(&mut simulation, ready - 1, 350.);
        simulation.apply_commands().unwrap();
        assert_eq!(
            simulation.waiting.len(),
            1,
            "reconnect must start a fresh two-second fallback wait"
        );
        assert!(!simulation.world.fixture_world().snakes()[0].alive);

        let attempt = simulation.next_spawn_tick.max(ready);
        advance_fenced_spawn(&mut simulation, attempt, 350.);
        simulation.apply_commands().unwrap();
        assert!(simulation.waiting.is_empty());
        assert!(simulation.world.fixture_world().snakes()[0].alive);
        assert_eq!(simulation.generations[0], 3);
        assert_eq!(simulation.last_spawn_tick, Some(attempt));
    }

    #[test]
    fn queued_expansion_leaves_recent_births_their_departure_window() {
        let (mut simulation, _gate, _deliveries) = connected(LatencyConfig::default(), 0);
        let mut world = crate::Config::battledome();
        world.food_count = 0;
        world.ambient_replenish_per_second = 0.;
        simulation.world = RawSimulator::new(
            7,
            SimulationConfig {
                agents: 40,
                world,
                ..Default::default()
            },
            40,
        )
        .unwrap();
        simulation.ticks = 0;
        simulation.next_spawn_tick = 0;
        simulation.last_spawn_tick = None;
        simulation.generations = vec![1; 40];
        let request = |slot, requested_tick| PendingJoin {
            slot,
            requested_tick,
            generation: 1,
            appearance: PlayerAppearance::default(),
            session_id: false,
            physical_food: true,
            protocol13: false,
            full_minimap: false,
            reset_kills: false,
        };
        simulation.waiting.push_back(request(0, 0));
        simulation.admit_waiting().unwrap();
        assert_eq!(simulation.last_spawn_tick, Some(0));
        for _ in 0..100 {
            step(&mut simulation);
        }
        for slot in 1..40 {
            simulation
                .waiting
                .push_back(request(slot, simulation.ticks));
        }
        simulation.admit_waiting().unwrap();
        let ready = (crate::spawn::DEPARTURE_SECONDS / simulation.world.dt()).ceil() as u64;
        assert_eq!(simulation.next_spawn_tick, ready);
        assert_eq!(simulation.waiting.len(), 39);
        while simulation.ticks < ready {
            step(&mut simulation);
            assert_eq!(simulation.waiting.len(), 39);
        }
        simulation.admit_waiting().unwrap();
        assert_eq!(simulation.waiting.len(), 38);
        assert_eq!(simulation.last_spawn_tick, Some(ready));
    }

    #[test]
    fn waiting_sockets_precede_bots_and_cancel_without_spawning() {
        let (mut simulation, gate, _deliveries) = connected(LatencyConfig::default(), 0);
        let mut world = crate::Config::sandbox();
        world.spawn_radius = Some(400.);
        world.min_score = Some(400);
        world.food_count = 0;
        world.ambient_replenish_per_second = 0.;
        simulation.world = RawSimulator::new(
            7,
            SimulationConfig {
                agents: 3,
                world,
                ..Default::default()
            },
            3,
        )
        .unwrap();
        assert!(
            simulation
                .world
                .connect(2, PlayerAppearance::default())
                .unwrap()
        );
        simulation.generations = vec![0, 0, 1];
        for slot in [1, 0] {
            let command = Command::Connect {
                slot,
                generation: 2,
                appearance: PlayerAppearance::default(),
                session_id: false,
                physical_food: true,
                protocol13: false,
                full_minimap: false,
            };
            if slot == 0 {
                gate.send(command);
            } else {
                gate.commands.send(command).unwrap();
            }
        }
        simulation.apply_commands().unwrap();
        assert_eq!(simulation.waiting.len(), 2);
        assert!(simulation.next_spawn_tick > simulation.ticks);
        // Input while waiting does not close or accidentally connect the socket.
        gate.send(Command::Message {
            slot: 0,
            generation: 2,
            bytes: vec![251],
            sent: 0.,
        });
        simulation.apply_commands().unwrap();
        assert_eq!(simulation.waiting.len(), 2);
        simulation.world.disconnect(2).unwrap();
        simulation.ticks = simulation.next_spawn_tick;
        simulation.apply_commands().unwrap();
        assert_eq!(simulation.waiting.len(), 1);
        assert_eq!(simulation.waiting[0].slot, 1);
        // A disconnected waiting generation must never be admitted later.
        gate.commands
            .send(Command::Disconnect {
                slot: 1,
                generation: 2,
            })
            .unwrap();
        simulation.apply_commands().unwrap();
        assert!(simulation.waiting.is_empty());
        assert_eq!(simulation.generations[1], 0);
    }

    #[test]
    fn a_queued_bot_rejoin_retires_its_old_connection_before_waiting() {
        let (mut simulation, gate, _deliveries) = connected(LatencyConfig::default(), 0);
        let mut world = crate::Config::sandbox();
        world.spawn_radius = Some(400.);
        world.min_score = Some(400);
        simulation.world = RawSimulator::new(
            7,
            SimulationConfig {
                agents: 2,
                world,
                ..Default::default()
            },
            2,
        )
        .unwrap();
        assert!(
            simulation
                .world
                .connect(1, PlayerAppearance::default())
                .unwrap()
        );
        simulation.generations = vec![0, 1];
        for slot in [1, 0] {
            let command = Command::Connect {
                slot,
                generation: 2,
                appearance: PlayerAppearance::default(),
                session_id: false,
                physical_food: true,
                protocol13: false,
                full_minimap: false,
            };
            if slot == 0 {
                gate.send(command);
            } else {
                gate.commands.send(command).unwrap();
            }
        }
        simulation.apply_commands().unwrap();
        assert_eq!(simulation.waiting.len(), 1);
        assert_eq!(simulation.waiting[0].slot, 1);
        assert!(simulation.world.take(1).unwrap().is_empty());
        simulation
            .world
            .advance_tick_with(0, &mut |_| Ok(()))
            .unwrap();
        assert!(!simulation.world.take(0).unwrap().is_empty());
        assert!(simulation.world.take(1).unwrap().is_empty());
    }

    #[test]
    fn a_bot_seat_too_far_behind_ends_its_session() {
        let (commands, received) = mpsc::channel();
        let (_route, deliveries) = mpsc::channel();
        let latency = LatencyConfig::default();
        let mut random = Random::new(1);
        let mut seat = Seat::new(4, 0, 0.042);
        seat.join(0.1, &commands, &latency, &mut random).unwrap();
        let mut bots = Bots {
            seats: vec![seat],
            policy: ForagerBot::new(),
            stride: 1,
            deliveries,
            commands,
            shared: shared(0.007, 0),
            latency,
            random,
            interval: 0.042,
        };
        let frames = |count| Delivery::Frames {
            slot: 4,
            generation: 1,
            frames: vec![(1., vec![0, 0, b'p']); count],
        };
        bots.deliver(frames(MAX_QUEUED_FRAMES));
        assert_eq!(bots.seats[0].frames.len(), MAX_QUEUED_FRAMES);
        bots.deliver(frames(1));
        assert!(bots.seats[0].closed && bots.seats[0].frames.is_empty());
        assert!(received.try_iter().any(|command| matches!(
            command,
            Command::Disconnect {
                slot: 4,
                generation: 1
            }
        )));
        bots.deliver(frames(1));
        assert!(bots.seats[0].frames.is_empty());
    }

    #[test]
    fn seats_survive_early_frames_and_closes_while_processing() {
        let (commands, _received) = mpsc::channel();
        let latency = LatencyConfig {
            processing_seconds: [0.05, 0.05],
            ..Default::default()
        };
        let mut random = Random::new(1);
        let mut policy = ForagerBot::new();
        let mut seat = Seat::new(0, 0, 0.042);
        seat.join(0.5, &commands, &latency, &mut random).unwrap();
        // A world catching up on ticks stamps the setup before the join.
        let mut frame = vec![0, 0];
        frame.extend(super::super::encode::setup(1000., 0.029).unwrap());
        seat.frames.push_back((0.1, frame));
        seat.run(0.6, &commands, &latency, &mut random, 0.042, &mut policy)
            .unwrap();
        // A close while a command is processing cancels its departure, due
        // here before the next decision would rejoin.
        assert!(seat.decide_at > 0.61);
        seat.outbox = Some((
            0.6,
            Action {
                heading: 0.,
                boost: false,
            },
            0.01,
        ));
        seat.close(0.6);
        seat.run(0.7, &commands, &latency, &mut random, 0.042, &mut policy)
            .unwrap();
    }

    #[test]
    fn challenge_round_trips_through_the_restricted_solver() {
        let (encoded, [literal, solved]) = challenge_parts().unwrap();
        assert!(encoded.iter().all(u8::is_ascii_lowercase));
        assert_eq!(literal, b"SlitherSimLocalHostChalleng");
        assert_eq!(solved.len(), 27);
        assert_ne!(literal, solved);
        assert!(solved.iter().all(u8::is_ascii_alphabetic));
    }

    /// Manually clocked socket gateway and world: no worker races or sleeps are
    /// needed to interrupt an upgrade or deliver an obsolete generation late.
    fn pit_gateway() -> (Gateway, Simulation) {
        let mut config = SimulationConfig {
            world: crate::Config::battledome(),
            latency: LatencyConfig::one_way(0., 0.2),
            ..Default::default()
        };
        config.world.food_count = 0;
        config.world.ambient_replenish_per_second = 0.;
        // Keep this presentation fixture within pit-marker food interest.
        config.world.spawn_radius_max = None;
        let world = RawSimulator::new(7, config, 1).unwrap();
        let shared = shared(world.dt(), 1);
        let (commands, inbox) = mpsc::channel();
        let (route, deliveries) = mpsc::channel();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let gateway = Gateway {
            listener,
            sessions: Vec::new(),
            #[cfg(target_os = "linux")]
            readiness: readiness::Readiness::default(),
            gate: Gate {
                occupied: vec![false],
                generations: vec![0],
                challenge: Vec::new(),
                replies: [Vec::new(), Vec::new()],
                commands,
                shared: shared.clone(),
            },
            deliveries,
        };
        let simulation = Simulation {
            world,
            commands: inbox,
            routes: vec![route],
            batches: vec![Vec::new()],
            max_clients: 1,
            generations: vec![0],
            waiting: VecDeque::new(),
            next_spawn_tick: 0,
            last_spawn_tick: None,
            shared,
            ticks: 0,
            deferred: 0,
        };
        (gateway, simulation)
    }

    #[test]
    fn pit_markers_fragmented_upgrade_preserves_physical_food_header() {
        use std::io::Write;
        let (mut gateway, mut simulation) = pit_gateway();
        let mut client = TcpStream::connect(gateway.listener.local_addr().unwrap()).unwrap();
        client.set_nodelay(true).unwrap();
        // The upgrade is first interrupted before any bytes, then inside both
        // the header name and its value, forcing the saved HTTP state to resume.
        // A nonblocking listener may not see a fresh loopback connection on the
        // first accept, notably on macOS.
        let deadline = Instant::now() + Duration::from_secs(2);
        while gateway.sessions.is_empty() && Instant::now() < deadline {
            gateway.accept(Instant::now());
            thread::yield_now();
        }
        assert!(
            !gateway.sessions.is_empty(),
            "gateway did not accept the loopback connection within two seconds"
        );
        assert!(matches!(gateway.sessions[0].stage, Stage::Http(_)));
        for fragment in [
            &b"GET /slither HTTP/1.1\r\nHost: localhost\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nX-SlitherSim-F"[..],
            &b"ood: phys"[..],
        ] {
            client.write_all(fragment).unwrap();
            gateway.poll().unwrap();
            assert!(matches!(gateway.sessions[0].stage, Stage::Http(_)));
        }
        client.write_all(b"ical\r\n\r\n").unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            gateway.poll().unwrap();
            if matches!(gateway.sessions[0].stage, Stage::Open(_)) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "fragmented upgrade never completed"
            );
            thread::yield_now();
        }
        let Stage::Open(open) = &mut gateway.sessions[0].stage else {
            unreachable!()
        };
        assert!(open.physical_food);
        open.join(
            &mut gateway.gate,
            &crate::client::network::login_packet("Fragmented", 0).unwrap(),
        )
        .unwrap();
        step(&mut simulation);
        // Apply the actual Connect command and decode the host's queued frames:
        // this empty physical world must contain no decorative food for the bot.
        let mut reader = ClientState::new();
        reader.new_connection(0.).unwrap();
        let mut receipt: f64 = 0.;
        for delivery in gateway.deliveries.try_iter().flatten() {
            let Delivery::Frames { frames, .. } = delivery else {
                panic!("join failed")
            };
            for (at, bytes) in frames {
                reader.receive_frame(&bytes, at).unwrap();
                receipt = at;
            }
        }
        assert!(
            reader
                .frame(receipt)
                .unwrap()
                .observation
                .unwrap()
                .food
                .is_empty()
        );
    }

    #[test]
    fn pit_markers_reused_slot_rejects_late_deliveries_between_food_modes() {
        use tungstenite::protocol::Role;
        struct LateFrames {
            generation: u64,
            frames: Vec<(f64, Vec<u8>)>,
        }
        let (mut gateway, mut simulation) = pit_gateway();
        let mut late: Option<LateFrames> = None;
        // One slot traverses both directions, resetting its listings each time.
        for physical_food in [false, true, false] {
            if let Some(session) = gateway.sessions.first_mut() {
                let Stage::Open(open) = &mut session.stage else {
                    panic!("session closed")
                };
                assert!(
                    !open.queued.is_empty(),
                    "old generation has a queued future delivery"
                );
                open.release(&mut gateway.gate);
                assert!(open.queued.is_empty());
                gateway.sessions.clear();
                simulation.apply_commands().unwrap();
            }
            let client = TcpStream::connect(gateway.listener.local_addr().unwrap()).unwrap();
            client
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            // The gateway listener is nonblocking, and a fresh connection may
            // not be queued yet (macOS CI returned WouldBlock here).
            let deadline = Instant::now() + Duration::from_secs(2);
            let server = loop {
                match gateway.listener.accept() {
                    Ok((server, _)) => break server,
                    Err(error)
                        if error.kind() == io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::yield_now()
                    }
                    Err(error) => panic!("accept failed: {error}"),
                }
            };
            server.set_nonblocking(true).unwrap();
            let mut client = WebSocket::from_raw_socket(client, Role::Client, None);
            let mut open = Open::new(
                WebSocket::from_raw_socket(
                    ReadBudget::new(server),
                    Role::Server,
                    Some(socket_config()),
                ),
                physical_food,
            );
            open.join(
                &mut gateway.gate,
                &crate::client::network::login_packet("Reuse", 0).unwrap(),
            )
            .unwrap();
            let generation = open.generation;
            assert_eq!(open.phase, Phase::Playing(0));
            gateway.sessions.push(Session {
                accepted: Instant::now(),
                physical_food: Arc::new(AtomicBool::new(physical_food)),
                stage: Stage::Open(Box::new(open)),
            });
            simulation.apply_commands().unwrap();
            assert!(simulation.world.take(0).unwrap().is_empty());
            step(&mut simulation);
            let mut current = Vec::new();
            for delivery in gateway.deliveries.try_iter().flatten() {
                let Delivery::Frames {
                    slot,
                    generation: actual,
                    frames,
                } = delivery
                else {
                    panic!("join failed")
                };
                assert_eq!((slot, actual), (0, generation));
                current.extend(frames);
            }
            assert!(!current.is_empty());
            if let Some(old) = &late {
                gateway.deliver(
                    Delivery::Frames {
                        slot: 0,
                        generation: old.generation,
                        frames: old.frames.clone(),
                    },
                    Instant::now(),
                );
            }
            let Stage::Open(open) = &gateway.sessions[0].stage else {
                panic!("session closed")
            };
            assert!(
                open.queued.is_empty(),
                "obsolete mode leaked before the new listing"
            );
            gateway.deliver(
                Delivery::Frames {
                    slot: 0,
                    generation,
                    frames: current.clone(),
                },
                Instant::now(),
            );
            if let Some(old) = &late {
                gateway.deliver(
                    Delivery::Frames {
                        slot: 0,
                        generation: old.generation,
                        frames: old.frames.clone(),
                    },
                    Instant::now(),
                );
                gateway.deliver(
                    Delivery::Closed {
                        slot: 0,
                        generation: old.generation,
                        reason: "obsolete close".into(),
                    },
                    Instant::now(),
                );
            }
            let Stage::Open(open) = &mut gateway.sessions[0].stage else {
                panic!("session closed")
            };
            assert_eq!(open.phase, Phase::Playing(0));
            assert_eq!(open.queued.iter().cloned().collect::<Vec<_>>(), current);
            open.send_due(&mut gateway.gate, Instant::now(), 1000.)
                .unwrap();
            assert!(open.flush(&mut gateway.gate));
            let mut reader = ClientState::new();
            reader.new_connection(0.).unwrap();
            let mut receipt = 0.;
            for (at, expected) in &current {
                let Message::Binary(bytes) = client.read().unwrap() else {
                    panic!("expected frame")
                };
                assert_eq!(&bytes[..], expected);
                reader.receive_frame(&bytes, *at).unwrap();
                receipt = *at;
            }
            let frame = reader.frame(receipt).unwrap();
            assert_eq!(frame.status, ClientStatus::Playing);
            assert_eq!(frame.observation.unwrap().food.is_empty(), physical_food);
            // Keep another real delivery queued until the old session releases;
            // an identical delayed batch also arrives after slot reuse.
            let future = current
                .into_iter()
                .map(|(_, bytes)| (1001., bytes))
                .collect::<Vec<_>>();
            gateway.deliver(
                Delivery::Frames {
                    slot: 0,
                    generation,
                    frames: future.clone(),
                },
                Instant::now(),
            );
            late = Some(LateFrames {
                generation,
                frames: future,
            });
        }
    }

    #[test]
    fn overflowing_client_and_bot_counts_are_errors() {
        let config = HostConfig {
            players: PlayerConfig {
                max: usize::MAX,
                ..Default::default()
            },
            bots: BotConfig {
                count: 1,
                ..Default::default()
            },
            ..HostConfig::default()
        };
        assert!(
            config
                .validate()
                .unwrap_err()
                .contains("players.max + bots.count")
        );
    }

    #[test]
    fn login_layout_matches_the_stock_client() {
        let mut login = vec![115, 30, 1, 35];
        login.extend([0; 20]);
        login.extend([7, 3]);
        login.extend(b"Bot");
        login.extend([0, 255, 9, 8]);
        let appearance = parse_login(&login).unwrap();
        assert_eq!(appearance.nickname, b"Bot");
        assert_eq!(appearance.skin, 7);
        assert_eq!(appearance.custom_skin, [9, 8]);
        login[25] = 25;
        assert!(parse_login(&login).is_err());
        assert!(parse_login(&login[..27]).is_err());
    }

    #[test]
    fn legacy_login_boundaries_and_selector_choose_the_wire_profile() {
        for selector in [11, 12, 31] {
            for length in [0, MAX_NICKNAME] {
                let name = vec![b'x'; length];
                let custom = vec![7; MAX_CUSTOM_SKIN];
                let mut login = vec![b's', selector];
                if selector == 31 {
                    login.extend([1, 161]);
                }
                login.extend([23, length as u8]);
                login.extend(&name);
                login.extend([0, 255]);
                login.extend(&custom);
                let parsed = parse_login(&login).unwrap();
                assert_eq!(legacy_login(&login), selector != 31);
                assert_eq!(parsed.nickname, name);
                assert_eq!(parsed.skin, 23);
                assert_eq!(parsed.custom_skin, custom);
                login.push(0);
                assert!(parse_login(&login).is_err());
                let minimum = length + if selector == 31 { 8 } else { 6 };
                for end in 0..minimum {
                    assert!(parse_login(&login[..end]).is_err(), "length {end}");
                }
            }
            let mut too_long = vec![b's', selector];
            if selector == 31 {
                too_long.extend([1, 161]);
            }
            too_long.extend([0, 25, 0, 0]);
            assert!(parse_login(&too_long).is_err());
        }
        assert!(!legacy_login(&[b's', 30]));
    }

    #[test]
    fn legacy_challenge_is_fixed_data_with_the_observed_client_reply() {
        let (challenge, reply) = legacy_challenge();
        assert_eq!(&challenge[..3], &[0, 0, b'6']);
        let mut key = 23;
        let source: Vec<u8> = challenge[3..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let mut byte = 0;
                for &letter in pair {
                    let nibble = (letter - b'a' + 26 - key) % 26;
                    assert!(nibble < 16);
                    byte = (byte << 4) | nibble;
                    key = (key + 17) % 26;
                }
                byte
            })
            .collect();
        assert_eq!(&source[..7], b"var a='");
        let quoted = source.split(|&b| b == b'\'').nth(1).unwrap();
        assert_eq!(quoted, b"ABCDEFGHIJKLMNOPQRSTUVWX");
        assert_eq!(&source[7..31], quoted, "fixed-offset solver input");
        // Pin the entire buffer-filling statement without evaluating source.
        // The real eval-based client is tested only in the isolated browser.
        assert_eq!(
            &source[31..],
            b"';for(var i=0;i<24;i++)idba[i]=a.charCodeAt(i);"
        );
        // Client-derived local-sandbox observation, not an official capture.
        assert_eq!(reply, b"CGLRYGPZKWJXMCTLEYTPMKJJ");
        assert_ne!(reply, challenge_parts().unwrap().1[1]);
    }

    #[test]
    fn bot_seats_wear_distinct_pickable_stock_skins() {
        let skins: std::collections::BTreeSet<u8> = (0..64).map(bot_skin).collect();
        assert_eq!(skins.len(), 64);
        assert!(skins.iter().all(|&skin| skin <= 64 && skin != 42));
        assert_eq!(bot_skin(64), bot_skin(0));
    }

    #[test]
    fn bot_seats_have_distinct_stock_sized_names() {
        let names: std::collections::BTreeSet<Vec<u8>> =
            (0..MAX_AGENTS).map(bot_nickname).collect();
        assert_eq!(names.len(), MAX_AGENTS);
        assert!(names.iter().all(|name| name.len() <= MAX_NICKNAME));
        assert_eq!(bot_nickname(0), b"Noodle (bot)");
        assert_eq!(bot_nickname(64), b"Noodle 2 (bot)");
    }

    #[test]
    fn first_bot_after_player_slots_gets_the_first_name_and_skin() {
        let (commands, received) = mpsc::channel();
        let mut seat = Seat::new(8, 0, 1.);
        seat.join(
            0.,
            &commands,
            &LatencyConfig::default(),
            &mut Random::new(1),
        )
        .unwrap();
        let Command::Connect { appearance, .. } = received.recv().unwrap() else {
            panic!("expected a connect");
        };
        assert_eq!(appearance.nickname, b"Noodle (bot)");
        assert_eq!(appearance.skin, 0);
    }

    #[test]
    fn rolling_kill_reset_follows_socket_ownership_not_respawn_or_bot_generation() {
        let (mut simulation, gate, _) = connected(LatencyConfig::default(), 0);
        let config = SimulationConfig {
            world: crate::Config::battledome(),
            agents: 2,
            ..Default::default()
        };
        simulation.world = RawSimulator::new(7, config, 2).unwrap();
        simulation.generations = vec![0; 2];
        let queue = |simulation: &mut Simulation, slot, generation| {
            let command = Command::Connect {
                slot,
                generation,
                appearance: PlayerAppearance::default(),
                session_id: false,
                physical_food: true,
                protocol13: false,
                full_minimap: false,
            };
            if slot < simulation.max_clients {
                gate.send(command);
            } else {
                gate.commands.send(command).unwrap();
            }
            let command = simulation.commands.try_recv().unwrap();
            simulation.apply(command).unwrap();
            simulation
                .waiting
                .iter()
                .find(|join| join.slot == slot)
                .unwrap()
                .reset_kills
        };
        assert!(queue(&mut simulation, 0, 2));
        // A retried request must retain the pending reset until admission.
        assert!(queue(&mut simulation, 0, 2));
        let first = simulation.waiting.pop_front().unwrap();
        assert!(simulation.join(&first).unwrap());
        assert!(!queue(&mut simulation, 0, 2), "same connection respawn");
        assert!(queue(&mut simulation, 0, 3), "new socket owner");
        assert!(!queue(&mut simulation, 1, 1), "first bot life");
        assert!(!queue(&mut simulation, 1, 2), "later bot life");
    }
}
