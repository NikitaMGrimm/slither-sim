//! Raw protocol-19 WebSocket host: remote clients and in-process bots share
//! one real-time world.
//!
//! Three kinds of threads share one wall-clock schedule. The simulation thread
//! owns the world and only runs physics ticks and encodes their messages. The
//! I/O thread owns the listener and every socket, and sends each frame at the
//! time the latency model delivers it rather than whenever the tick that made
//! it finished. Bot threads run the background seekers as ordinary clients of
//! their own slots: they decode frames, decide, ping and model processing time
//! off the simulation thread, and reach the world only through timestamped
//! commands. Running 480 seekers on the host thread delayed remote frames by up
//! to ~20 ms depending on tick phase (jitter study, 2026-10-25).
//!
//! Sockets stay nonblocking: interrupted HTTP upgrades, partial WebSocket
//! messages and unflushed output stay inside their tungstenite state, so a slow
//! or stalled peer never blocks physics or another connection. Each socket is
//! bounded: upgrade and login deadlines, small client messages, a capped send
//! buffer and a per-poll read budget. A misbehaving client is closed without
//! touching other slots.
//!
//! Login follows the stock client: optional one-byte framing requests, the
//! `[99, 0]` request, a `6` challenge generated here from a fixed literal, a
//! 27-byte reply and the `s` login. The challenge is a compatibility step, not
//! authentication. A login sent first (the native `direct` handshake) is also
//! accepted.
use super::{
    ClientState, ClientStatus, SimulationConfig, challenge,
    latency::{LatencyConfig, Link, Random},
    protocol,
    simulation::{MAX_NICKNAME, MAX_VICTORY_TEXT, PlayerAppearance, RawSimulator},
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
// The budgets and timeouts below are host resource policies, not protocol or
// physics limits.
/// Largest accepted client message or frame. Logins with a custom skin are the
/// largest stock-client messages and stay far below this.
const MAX_INPUT_BYTES: usize = 4 << 10;
/// Output batched per socket write.
const WRITE_BUFFER_BYTES: usize = 16 << 10;
/// Queued output beyond this closes the connection as too slow.
const MAX_SEND_BUFFER_BYTES: usize = 1 << 20;
/// Frames waiting for their delivery time on one connection.
const MAX_QUEUED_FRAMES: usize = 4096;
/// Connections that have not joined, including those being closed.
const MAX_PENDING: usize = 32;
/// Fairness budgets per poll, so one busy listener or socket cannot starve
/// the other connections.
const MAX_ACCEPTS_PER_POLL: usize = 16;
const MAX_READS_PER_POLL: usize = 32;
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
        let config = SimulationConfig {
            agents,
            controlled_slot: 0,
            ..self.simulation.clone()
        };
        config.validate()?;
        Ok(config)
    }
}

/// World time `t` is due at `epoch + t + deferred × dt`. Ticks the simulation
/// could not keep up with shift the whole schedule, so queued frames keep
/// their spacing.
struct Clock {
    epoch: Instant,
    dt: f64,
    deferred: AtomicU64,
}

impl Clock {
    /// World time at `at`.
    fn world(&self, at: Instant) -> f64 {
        let offset = self.deferred.load(Ordering::Relaxed) as f64 * self.dt;
        at.saturating_duration_since(self.epoch).as_secs_f64() - offset
    }

    /// Sleep until world time `due`, for at most `limit`.
    fn sleep_until(&self, due: f64, limit: Duration) {
        let wait = Duration::try_from_secs_f64(due - self.world(Instant::now()))
            .map_or(Duration::ZERO, |wait| wait.min(limit));
        if !wait.is_zero() {
            thread::sleep(wait);
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
    },
    /// A game command sent at world time `sent`.
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

/// From the simulation thread to a slot's owner.
enum Delivery {
    /// Server payloads with their delivery times in world seconds, in order.
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
    shared: Arc<Shared>,
    ticks: u64,
    deferred: u64,
}

impl Simulation {
    fn run(&mut self) -> Result<(), String> {
        let dt = self.shared.clock.dt;
        while !self.shared.stopped() {
            let due = self.due_ticks(Instant::now());
            if due == 0 {
                let next = (self.ticks + 1) as f64 * dt;
                self.shared
                    .clock
                    .sleep_until(next, Duration::from_secs_f64(dt));
                continue;
            }
            let started = Instant::now();
            for _ in 0..due {
                // Commands take effect at the first tick after they arrive;
                // their send times, not this loop, set when that is.
                self.apply_commands()?;
                self.world.advance_ticks(1)?;
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
            self.deferred += backlog - limit;
            self.shared
                .clock
                .deferred
                .store(self.deferred, Ordering::Relaxed);
        }
        backlog.min(limit).min(MAX_CATCHUP_TICKS)
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
        Ok(())
    }

    /// Client faults close only that session; errors are simulator failures.
    fn apply(&mut self, command: Command) -> Result<(), String> {
        match command {
            Command::Connect {
                slot,
                generation,
                appearance,
                session_id,
            } => {
                self.generations[slot] = generation;
                let mut joined = self.world.connect(slot, appearance);
                if joined.is_ok() && session_id {
                    joined = self.world.request_session_id(slot);
                }
                if let Err(reason) = joined {
                    self.close(slot, reason)?;
                }
            }
            Command::Message {
                slot,
                generation,
                bytes,
                sent,
            } if generation == self.generations[slot] => {
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
                if let Err(reason) = self.world.victory_at(slot, &text, sent) {
                    self.close(slot, reason)?;
                }
            }
            Command::Disconnect { slot, generation } if generation == self.generations[slot] => {
                self.generations[slot] = 0;
                self.world.disconnect(slot)?;
            }
            Command::Message { .. } | Command::Victory { .. } | Command::Disconnect { .. } => {}
        }
        Ok(())
    }

    fn close(&mut self, slot: usize, reason: String) -> Result<(), String> {
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

    /// Hand every queued frame to its slot's owner, which sends it on time.
    fn hand_over(&mut self) -> Result<(), String> {
        for slot in 0..self.generations.len() {
            let frames = self.world.take(slot)?;
            if !frames.is_empty() {
                let route = self.route(slot);
                self.batches[route].push(Delivery::Frames {
                    slot,
                    generation: self.generations[slot],
                    frames,
                });
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

type Upgrade = fn(&Request, Response) -> Result<Response, ErrorResponse>;

#[allow(
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
    fn send(&self, command: Command) {
        // A closed channel means the simulation ended and the host is stopping.
        let _ = self.commands.send(command);
    }
}

struct Open {
    socket: WebSocket<TcpStream>,
    phase: Phase,
    framing: Framing,
    want_session_id: bool,
    generation: u64,
    /// Frames from the world waiting for their delivery time, in order.
    queued: VecDeque<(f64, Vec<u8>)>,
}

enum Stage {
    Http(Box<MidHandshake<ServerHandshake<TcpStream, Upgrade>>>),
    Open(Box<Open>),
    Closed,
}

struct Session {
    accepted: Instant,
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
        let now = Instant::now();
        self.accept(now);
        while let Ok(batch) = self.deliveries.try_recv() {
            for delivery in batch {
                self.deliver(delivery, now);
            }
        }
        let world = self.gate.shared.clock.world(now);
        for session in &mut self.sessions {
            session.service(&mut self.gate, now, world)?;
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
        let (slot, generation) = match &delivery {
            Delivery::Frames {
                slot, generation, ..
            }
            | Delivery::Closed {
                slot, generation, ..
            } => (*slot, *generation),
        };
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
            let stage = match tungstenite::accept_hdr_with_config(
                stream,
                upgrade as Upgrade,
                Some(socket_config()),
            ) {
                Ok(socket) => Stage::Open(Box::new(Open::new(socket))),
                Err(HandshakeError::Interrupted(mid)) => Stage::Http(Box::new(mid)),
                Err(HandshakeError::Failure(_)) => Stage::Closed,
            };
            self.sessions.push(Session {
                accepted: now,
                stage,
            });
        }
    }
}

impl Session {
    fn joined(&self) -> bool {
        matches!(&self.stage, Stage::Open(open) if matches!(open.phase, Phase::Playing(_)))
    }

    fn service(&mut self, gate: &mut Gate, now: Instant, world: f64) -> Result<(), String> {
        let age = now.duration_since(self.accepted);
        self.stage = match std::mem::replace(&mut self.stage, Stage::Closed) {
            Stage::Http(_) if age >= HTTP_TIMEOUT => Stage::Closed,
            Stage::Http(mid) => match mid.handshake() {
                Ok(socket) => Stage::Open(Box::new(Open::new(socket))),
                Err(HandshakeError::Interrupted(mid)) => Stage::Http(Box::new(mid)),
                Err(HandshakeError::Failure(_)) => Stage::Closed,
            },
            Stage::Open(mut open) => {
                if open.service(gate, now, world, age)? {
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

impl Open {
    fn new(socket: WebSocket<TcpStream>) -> Self {
        Self {
            socket,
            phase: Phase::Hello { framed: false },
            framing: Framing::Elapsed,
            want_session_id: false,
            generation: 0,
            queued: VecDeque::new(),
        }
    }

    /// Read within budget and send due frames. Returns false once closed.
    fn service(
        &mut self,
        gate: &mut Gate,
        now: Instant,
        world: f64,
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
                    if let Err(reason) = self.receive(gate, &bytes, world) {
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
        self.send_due(gate, now, world)?;
        Ok(true)
    }

    /// Apply one client message received at world time `world`. An error
    /// closes only this connection.
    fn receive(&mut self, gate: &mut Gate, bytes: &[u8], world: f64) -> Result<(), String> {
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
                gate.send(Command::Victory {
                    slot,
                    generation: self.generation,
                    text: text.to_vec(),
                    sent: world,
                });
                Ok(())
            }
            (Phase::Playing(slot), _) if super::simulation::client_message(bytes) => {
                gate.send(Command::Message {
                    slot,
                    generation: self.generation,
                    bytes: bytes.to_vec(),
                    sent: world,
                });
                Ok(())
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
        let slot = gate
            .occupied
            .iter()
            .position(|used| !used)
            .ok_or("Server is full")?;
        gate.occupied[slot] = true;
        gate.generations[slot] += 1;
        self.generation = gate.generations[slot];
        self.phase = Phase::Playing(slot);
        gate.send(Command::Connect {
            slot,
            generation: self.generation,
            appearance,
            session_id: self.want_session_id,
        });
        Ok(())
    }

    /// Send queued frames due by world time `world`. Only host faults are errors.
    fn send_due(&mut self, gate: &mut Gate, now: Instant, world: f64) -> Result<(), String> {
        while self.queued.front().is_some_and(|(at, _)| *at <= world) {
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

/// A background seeker as a client of its slot. It follows the simulator's
/// native background client: it decides every decision interval from the
/// frames delivered by then, sends after its processing time, pings on the
/// client's cadence and rejoins after it dies.
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
        let start = self.shared.clock.world(Instant::now());
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
            let now = self.shared.clock.world(Instant::now());
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
        let (slot, generation) = match &delivery {
            Delivery::Frames {
                slot, generation, ..
            }
            | Delivery::Closed {
                slot, generation, ..
            } => (*slot, *generation),
        };
        let first = self.seats[0].slot;
        let Some(seat) = self
            .seats
            .get_mut((slot - first) / self.stride)
            .filter(|seat| seat.generation == generation)
        else {
            return;
        };
        match delivery {
            Delivery::Frames { frames, .. } => seat.frames.extend(frames),
            Delivery::Closed { .. } => {
                seat.closed = true;
                seat.frames.clear();
                seat.state.disconnect();
            }
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
                self.state.receive_frame(&bytes, at)?;
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

    /// Start a new session and life. A crowded world may refuse it; the bot
    /// then retries at its next decision.
    fn join(
        &mut self,
        at: f64,
        commands: &Sender<Command>,
        latency: &LatencyConfig,
        random: &mut Random,
    ) -> Result<(), String> {
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

    #[test]
    fn challenge_round_trips_through_the_restricted_solver() {
        let (encoded, [literal, solved]) = challenge_parts().unwrap();
        assert!(encoded.iter().all(u8::is_ascii_lowercase));
        assert_eq!(literal, b"SlitherSimLocalHostChalleng");
        assert_eq!(solved.len(), 27);
        assert_ne!(literal, solved);
        assert!(solved.iter().all(u8::is_ascii_alphabetic));
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
