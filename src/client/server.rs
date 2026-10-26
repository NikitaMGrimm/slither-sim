//! Raw protocol-19 WebSocket host: remote clients and in-process bots share
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
//! is also accepted.
use super::{
    ClientState, ClientStatus, SimulationConfig, challenge,
    latency::{LatencyConfig, Link, Random},
    protocol,
    simulation::{
        MAX_NICKNAME, MAX_VICTORY_TEXT, PlayerAppearance, REMOTE_IN_FLIGHT, RawSimulator,
    },
};
use crate::{Action, simulator::population_radius};
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
    HandshakeError, Message, WebSocket,
    handshake::{
        MidHandshake,
        server::{ErrorResponse, Request, Response, ServerHandshake},
    },
    http::StatusCode,
    protocol::{CloseFrame, WebSocketConfig, frame::coding::CloseCode},
};

pub const PATH: &str = "/slither";
pub const DEFAULT_PORT: u16 = 8766;
/// Automatic bot handshake: preserve physical food even at decorative wire keys.
/// An HTTP header keeps the stock binary protocol and other servers unchanged.
pub(crate) const PHYSICAL_FOOD_HEADER: &str = "x-slithersim-food";
// The budgets and timeouts below are host resource policies, not protocol or
// physics limits.
/// Largest accepted client message or frame. Logins with a custom skin are the
/// largest stock-client messages and stay far below this.
const MAX_INPUT_BYTES: usize = 4 << 10;
/// Output batched per socket write.
const WRITE_BUFFER_BYTES: usize = 16 << 10;
/// Queued output beyond this closes the connection as too slow.
const MAX_SEND_BUFFER_BYTES: usize = 1 << 20;
/// Frames waiting for their delivery time on one connection, socket or bot.
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
/// has `maxClients` socket slots followed by `bots` background seekers, and no
/// in-process controlled slot. Values given for those two fields are replaced.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct HostConfig {
    pub bind: SocketAddr,
    pub seed: u32,
    pub max_clients: usize,
    pub bots: usize,
    /// Threads that run the background seekers, apart from the simulation and
    /// socket threads.
    pub bot_threads: usize,
    pub simulation: SimulationConfig,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            bind: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, DEFAULT_PORT)),
            seed: 1,
            max_clients: 4,
            bots: 4,
            bot_threads: 2,
            simulation: SimulationConfig::default(),
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
            .max_clients
            .checked_add(self.bots)
            .ok_or("maxClients + bots must fit in usize")?;
        if self.max_clients == 0 {
            return Err("Host needs 1 or more maxClients".into());
        }
        if self.bots > 0 && self.bot_threads == 0 {
            return Err("Bots need 1 or more botThreads".into());
        }
        if self.simulation.record_wire {
            return Err("The host does not retain wire records; set recordWire to false".into());
        }
        let mut config = SimulationConfig {
            agents,
            controlled_slot: 0,
            ..self.simulation.clone()
        };
        // A dynamic arena follows its living population, but presets and
        // omitted fields start at `Config::official(1)`'s 670 units: too small
        // to admit a host's first lives clear of each other. Start no smaller
        // than the population radius of every seat, scaling the food target and
        // ambient rate with the area to keep their density.
        let world = &mut config.world;
        let seats = population_radius(agents);
        if world.dynamic_radius && world.radius > 0. && world.radius < seats {
            let area = (seats / world.radius).powi(2);
            world.radius = seats;
            world.food_count = (world.food_count as f64 * area) as usize;
            world.ambient_replenish_per_second *= area;
        }
        config.validate()?;
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

/// A bound protocol-19 server. `start` serves it on its own threads; `run`
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
        let world = RawSimulator::new(config.seed, simulation, config.max_clients + config.bots)?;
        let listener = TcpListener::bind(config.bind).map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        Ok(Self {
            listener,
            world,
            max_clients: config.max_clients,
            bots: config.bots,
            bot_threads: config.bot_threads.min(config.bots),
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
                .map(|slot| Seat::new(slot, self.decision_seconds))
                .collect();
            let bots = Bots {
                seats,
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
        match self.world.connect(slot, join.appearance.clone()) {
            Ok(false) => return Ok(false),
            Ok(true) => {
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
            } => {
                self.generations[slot] = generation;
                let join = PendingJoin {
                    slot,
                    generation,
                    appearance,
                    session_id,
                    physical_food,
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
            Ok(())
        })
    }

    /// Hand every queued frame to its slot's owner, which sends it on time.
    fn hand_over(&mut self) -> Result<(), String> {
        let offset = self.deferred as f64 * self.shared.clock.dt;
        for slot in 0..self.generations.len() {
            let frames = self.world.take(slot)?;
            if !frames.is_empty() {
                let route = self.route(slot);
                let generation = self.generations[slot];
                self.batches[route].push(delivery(slot, generation, frames, offset));
            }
        }
        for (route, batch) in self.routes.iter().zip(&mut self.batches) {
            if !batch.is_empty() {
                // A closed route means its thread ended and the host is stopping.
                let _ = route.send(std::mem::take(batch));
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
}

impl<S> ReadBudget<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            bytes: MAX_RAW_READ_BYTES,
            calls: MAX_RAW_READ_CALLS,
        }
    }

    fn rearm(&mut self) {
        self.bytes = MAX_RAW_READ_BYTES;
        self.calls = MAX_RAW_READ_CALLS;
    }
}

impl<S: io::Read> io::Read for ReadBudget<S> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.bytes == 0 || self.calls == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        self.calls -= 1;
        let limit = bytes.len().min(self.bytes);
        let read = self.inner.read(&mut bytes[..limit])?;
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
    physical_food: bool,
    generation: u64,
    /// Frames from the world waiting for their delivery time, in order.
    queued: VecDeque<(f64, Vec<u8>)>,
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
            self.gate.shared.clock.sleep_until(next, MAX_IDLE);
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
        for session in &mut self.sessions {
            session.service(&mut self.gate, now, wall)?;
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
                if open.queued.len() + frames.len() > MAX_QUEUED_FRAMES {
                    open.reject(&mut self.gate, now, "Client is reading too slowly");
                } else {
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

    fn service(&mut self, gate: &mut Gate, now: Instant, wall: f64) -> Result<(), String> {
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
                if open.service(gate, now, wall, age)? {
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
        Self {
            socket,
            phase: Phase::Hello { framed: false },
            framing: Framing::Elapsed,
            want_session_id: false,
            physical_food,
            generation: 0,
            queued: VecDeque::new(),
        }
    }

    /// Read within budget and send due frames. Returns false once closed.
    fn service(
        &mut self,
        gate: &mut Gate,
        now: Instant,
        wall: f64,
        age: Duration,
    ) -> Result<bool, String> {
        if let Phase::Closing(deadline) = self.phase {
            return Ok(now < deadline && self.drain_close());
        }
        if !matches!(self.phase, Phase::Playing(_)) && age >= LOGIN_TIMEOUT {
            self.reject(gate, now, "Login timeout");
            return Ok(true);
        }
        for _ in 0..MAX_READS_PER_POLL {
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
                self.send(&challenge)
            }
            (Phase::Hello { .. } | Phase::Login, [b's', ..]) => self.join(gate, bytes),
            (Phase::Hello { .. }, _) => Err("Expected a login request".into()),
            (Phase::Reply, _) if gate.replies.iter().any(|reply| reply == bytes) => {
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
        });
        Ok(())
    }

    /// Send queued frames due by wall time `wall`. Only host faults are errors.
    fn send_due(&mut self, gate: &mut Gate, now: Instant, wall: f64) -> Result<(), String> {
        while self.queued.front().is_some_and(|(at, _)| *at <= wall) {
            let (_, message) = self.queued.pop_front().unwrap();
            if message.len() < 3 {
                return Err("Simulator emitted a message without a framed payload".into());
            }
            if let Err(reason) = self.send(&message) {
                self.reject(gate, now, &reason);
                break;
            }
        }
        Ok(())
    }

    /// Queue one elapsed-framed message with this connection's header.
    fn send(&mut self, framed: &[u8]) -> Result<(), String> {
        let payload = &framed[2..];
        let bytes = match &mut self.framing {
            Framing::Elapsed => framed.to_vec(),
            Framing::Bare => payload.to_vec(),
            Framing::Sequence(next) => {
                let mut bytes = next.to_be_bytes().to_vec();
                *next = next.wrapping_add(1);
                bytes.extend(payload);
                bytes
            }
        };
        match self.socket.write(Message::Binary(bytes.into())) {
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

/// One bot thread: background seekers in their own slots.
struct Bots {
    /// Every `stride`th slot from the first seat's.
    seats: Vec<Seat>,
    stride: usize,
    deliveries: Receiver<Vec<Delivery>>,
    commands: Sender<Command>,
    shared: Arc<Shared>,
    latency: LatencyConfig,
    random: Random,
    /// Seconds between one bot's decisions.
    interval: f64,
}

/// A background seeker that decides from delivered frames every interval, sends
/// after processing, pings on the client's cadence and rejoins after death,
/// matching the simulator's native background client.
struct Seat {
    slot: usize,
    generation: u64,
    state: ClientState,
    /// Last heading byte sent; see `seeker_action`.
    sent: Option<u8>,
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
    fn new(slot: usize, interval: f64) -> Self {
        Self {
            slot,
            generation: 0,
            state: ClientState::new(),
            sent: None,
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
                self.decide(at, commands, latency, random)?;
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
    ) -> Result<(), String> {
        if self.closed || self.state.status() == ClientStatus::Dead {
            return self.join(at, commands, latency, random);
        }
        if self.state.status() != ClientStatus::Playing || self.outbox.is_some() {
            return Ok(());
        }
        let frame = self.state.frame(at)?;
        if let Some(action) = super::simulation::seeker_action(&frame, &mut self.sent) {
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
        self.sent = None;
        self.frames.clear();
        self.outbox = None;
        self.previous_boost = None;
        self.closed = false;
        // A closed channel means the simulation ended and the host is stopping.
        let _ = commands.send(Command::Connect {
            slot: self.slot,
            generation: self.generation,
            appearance: PlayerAppearance::default(),
            session_id: false,
            physical_food: true,
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

/// Stock-client login: protocol and client-version bytes, password, skin at
/// 24, nickname length at 25, nickname, two reserved bytes, custom skin.
fn parse_login(bytes: &[u8]) -> Result<PlayerAppearance, String> {
    if bytes.len() < 28 || bytes[0] != b's' {
        return Err("Truncated login".into());
    }
    let length = usize::from(bytes[25]);
    if length > MAX_NICKNAME {
        return Err("Nickname longer than 24 bytes".into());
    }
    let end = 26 + length;
    let custom = bytes.get(end + 2..).ok_or("Truncated login")?;
    if custom.len() > MAX_CUSTOM_SKIN {
        return Err("Custom skin longer than 255 bytes".into());
    }
    Ok(PlayerAppearance {
        nickname: bytes[26..end].to_vec(),
        skin: bytes[24],
        custom_skin: custom.to_vec(),
    })
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
mod tests {
    use super::*;

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
        open.service(&mut gateway.gate, now, 0., Duration::ZERO)
            .unwrap();
        assert!(open.socket.get_ref().inner.read <= MAX_RAW_READ_BYTES);
        assert_eq!(open.phase, Phase::Hello { framed: false });
        // Later polls resume the parser and eventually complete the request.
        for _ in 0..32 {
            if open.phase == (Phase::Hello { framed: true }) {
                break;
            }
            open.socket.get_mut().rearm();
            open.service(&mut gateway.gate, now, 0., Duration::ZERO)
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
        open.service(&mut gateway.gate, now, 0., Duration::ZERO)
            .unwrap();
        assert!(open.socket.get_ref().inner.read <= 64 << 10);
        assert_eq!(open.phase, Phase::Hello { framed: false });
        for _ in 0..32 {
            if open.phase == (Phase::Hello { framed: true }) {
                break;
            }
            open.socket.get_mut().rearm();
            open.service(&mut gateway.gate, now, 0., Duration::ZERO)
                .unwrap();
        }
        assert_eq!(open.phase, Phase::Hello { framed: true });
        assert_eq!(open.framing, Framing::Bare);
        // Deadlines still apply with a partial fragmented message.
        open.service(&mut gateway.gate, now, 0., LOGIN_TIMEOUT)
            .unwrap();
        assert!(matches!(open.phase, Phase::Closing(_)));
    }

    #[test]
    fn bot_decode_failure_disconnects_only_its_generation_and_rejoins() {
        let (commands, received) = mpsc::channel();
        let latency = LatencyConfig::default();
        let mut random = Random::new(1);
        let mut seat = Seat::new(4, 1.);
        seat.join(0., &commands, &latency, &mut random).unwrap();
        let _ = received.recv().unwrap();
        seat.decide_at = 1.;
        let mut world = crate::Simulator::new(1, crate::Config::sandbox()).unwrap();
        world.add("large", 0, 1).unwrap();
        let snake = &mut world.snakes[0];
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
        seat.run(0.1, &commands, &latency, &mut random, 1.).unwrap();
        assert!(seat.closed && seat.frames.is_empty() && seat.outbox.is_none());
        assert!(matches!(
            received.recv().unwrap(),
            Command::Disconnect {
                slot: 4,
                generation: 1
            }
        ));
        // A different seat on the same worker still runs normally.
        let mut healthy = Seat::new(5, 1.);
        healthy.join(0.1, &commands, &latency, &mut random).unwrap();
        healthy
            .run(0.1, &commands, &latency, &mut random, 1.)
            .unwrap();
        seat.run(1., &commands, &latency, &mut random, 1.).unwrap();
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
                max_clients: 1,
                bots: 0,
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
        let seats = host.max_clients + host.bots;
        let official = crate::Config::official(seats);
        assert_eq!(config.world.radius, official.radius);
        // The area scale floors 109 pellets × 8 rather than the 877.4 target.
        assert!(official.food_count - config.world.food_count < seats);
        assert!(
            (config.world.ambient_replenish_per_second - official.ambient_replenish_per_second)
                .abs()
                < 1e-9
        );
        // Before, `Config::official(1)`'s 670-unit start refused these joins.
        let mut world = RawSimulator::new(host.seed, config, seats).unwrap();
        for slot in 0..seats {
            world.connect(slot, PlayerAppearance::default()).unwrap();
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

    fn shared(dt: f64, sockets: usize) -> Arc<Shared> {
        Arc::new(Shared {
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
        let mut seat = Seat::new(4, 0.042);
        seat.join(0.1, &commands, &latency, &mut random).unwrap();
        assert!(matches!(
            received.try_recv().unwrap(),
            Command::Connect { generation: 1, .. }
        ));
        seat.close(0.2);
        seat.run(1.19, &commands, &latency, &mut random, 0.042)
            .unwrap();
        assert!(received.try_recv().is_err());
        seat.run(1.2, &commands, &latency, &mut random, 0.042)
            .unwrap();
        assert!(matches!(
            received.try_recv().unwrap(),
            Command::Connect { generation: 2, .. }
        ));
        // A second refusal starts a fresh delay from its actual receipt time,
        // including when the simulation has fallen behind the wall clock.
        seat.close(2.);
        seat.run(2.99, &commands, &latency, &mut random, 0.042)
            .unwrap();
        assert!(received.try_recv().is_err());
        seat.run(3., &commands, &latency, &mut random, 0.042)
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
        seat.run(3.1, &commands, &latency, &mut random, 0.042)
            .unwrap();
        assert!(matches!(
            received.try_recv().unwrap(),
            Command::Connect { generation: 4, .. }
        ));
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
        let mut seat = Seat::new(4, 0.042);
        seat.join(0.1, &commands, &latency, &mut random).unwrap();
        let mut bots = Bots {
            seats: vec![seat],
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
        let mut seat = Seat::new(0, 0.042);
        seat.join(0.5, &commands, &latency, &mut random).unwrap();
        // A world catching up on ticks stamps the setup before the join.
        let mut frame = vec![0, 0];
        frame.extend(super::super::encode::setup(1000., 0.029).unwrap());
        seat.frames.push_back((0.1, frame));
        seat.run(0.6, &commands, &latency, &mut random, 0.042)
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
        seat.run(0.7, &commands, &latency, &mut random, 0.042)
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
        gateway.accept(Instant::now());
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
            max_clients: usize::MAX,
            bots: 1,
            ..HostConfig::default()
        };
        assert!(config.validate().unwrap_err().contains("maxClients + bots"));
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
}
