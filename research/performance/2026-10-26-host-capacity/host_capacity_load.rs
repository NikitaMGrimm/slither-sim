//! Headless protocol-19 load generator for a study-owned `slither-sim host`.
//!
//! host_capacity_load COUNT PORT DURATION_SECONDS WARMUP_SECONDS OUTPUT_JSON CLIENT_OFFSET
//!
//! Keeps COUNT clients (1..=1000) on the explicit private/loopback study endpoint from
//! one thread: nonblocking sockets, one service pass per millisecond at most, no
//! thread per client. Each client uses the stock login (`[99, 0]`, solved `6`
//! challenge, `login_packet`), the default elapsed-header framing, decodes every
//! message with `ClientState::receive_frame`, pings like the stock client and
//! steers every 100 ms from one `ClientState::frame`. A dead client closes and
//! reconnects, as do rejected logins (full host, no clear spawn) with backoff.
//!
//! Measurements cover [WARMUP, WARMUP + DURATION) seconds after process start.
//! Raw records are buffered in memory and written only after the run, so slow
//! output filesystems cannot stall the measured loop:
//! - OUTPUT_JSON: summary (lifecycle, close reasons, time-weighted clients,
//!   traffic, interval and generator statistics, per-client summaries).
//! - OUTPUT with extension `intervals.csv`: one row per measured own gameplay
//!   update: `client,connection,receipt_ms,interval_ms,header_interval_ms`.
//! - OUTPUT with extension `telemetry.jsonl`: one line per elapsed second.
//!
//! CLIENT_OFFSET makes client ids and nicknames unique across processes; give
//! each process a distinct range. Process CPU is measured by the caller.

#[cfg(feature = "networking")]
mod load {
    use serde_json::{Value, json};
    use slither_sim::{
        Action, Observation,
        client::{
            ClientState, ClientStatus, challenge,
            network::login_packet,
            protocol::{action_packets, split_frame},
        },
    };
    use std::{
        collections::BTreeMap,
        f64::consts::{PI, TAU},
        fs::File,
        io::{self, BufWriter, Write},
        net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream},
        path::PathBuf,
        thread,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };
    use tungstenite::{
        HandshakeError, Message, WebSocket,
        handshake::{MidHandshake, client::ClientHandshake},
        protocol::WebSocketConfig,
    };

    const USAGE: &str = "host_capacity_load COUNT PORT DURATION_SECONDS WARMUP_SECONDS OUTPUT_JSON CLIENT_OFFSET; COUNT=1..1000, PORT=1..65535, DURATION>0, WARMUP>=0, DURATION+WARMUP<=7200, CLIENT_OFFSET=0..1000000";
    const NOMINAL_MS: f64 = 42.; // Legacy deviation only; capacity uses measured gaps.
    /// The host refuses connections beyond 32 unjoined sessions. Three per
    /// process keeps eight generators below that, closing sessions included.
    const PENDING_LIMIT: usize = 3;
    const START_GAP: f64 = 0.004;
    /// std has no nonblocking connect; loopback connects return immediately
    /// unless the listen backlog is full, so the blocking call is bounded.
    const CONNECT_TIMEOUT: Duration = Duration::from_millis(50);
    const HANDSHAKE_TIMEOUT: f64 = 5.;
    const SETUP_TIMEOUT: f64 = 12.;
    const IDLE_TIMEOUT: f64 = 10.;
    const CONTROL_INTERVAL: f64 = 0.1;
    const DEATH_RETRY: f64 = 0.25;
    const RETRY_BASE: f64 = 0.25;
    const MAX_RETRY: f64 = 4.;
    const MAX_READS: usize = 64;
    const POLL: Duration = Duration::from_millis(1);
    const MAX_MESSAGE_BYTES: usize = 1 << 20;
    const MAX_REASONS: usize = 64;
    /// Own motion is sent only when the heading, wanted angle or speed byte
    /// changes, so the bot never aims straight: it always keeps turning.
    const MIN_TURN: f64 = 0.6;
    const MAX_TURN: f64 = 1.5;
    const CATEGORIES: [&str; 6] = ["idle", "connecting", "login", "setup", "playing", "dead"];

    struct Args {
        count: usize,
        port: u16,
        duration: f64,
        warmup: f64,
        output: PathBuf,
        offset: u64,
    }

    fn bad<T>(_: T) -> String {
        USAGE.into()
    }

    fn parse_args() -> Result<Args, String> {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let [count, port, duration, warmup, output, offset] = args.as_slice() else {
            return Err(USAGE.into());
        };
        let args = Args {
            count: count.parse().map_err(bad)?,
            port: port.parse().map_err(bad)?,
            duration: duration.parse().map_err(bad)?,
            warmup: warmup.parse().map_err(bad)?,
            output: PathBuf::from(output),
            offset: offset.parse().map_err(bad)?,
        };
        if !(1..=1000).contains(&args.count)
            || args.port == 0
            || !args.duration.is_finite()
            || args.duration <= 0.
            || !args.warmup.is_finite()
            || args.warmup < 0.
            || args.duration + args.warmup > 7200.
            || args.offset > 1_000_000
        {
            return Err(USAGE.into());
        }
        Ok(args)
    }

    fn socket_config() -> WebSocketConfig {
        WebSocketConfig::default()
            .read_buffer_size(16 << 10)
            .max_message_size(Some(MAX_MESSAGE_BYTES))
            .max_frame_size(Some(MAX_MESSAGE_BYTES))
    }

    fn r3(value: f64) -> f64 {
        (value * 1000.).round() / 1000.
    }

    /// Nearest-rank summary; sorts in place.
    fn stats(values: &mut [f32]) -> Value {
        if values.is_empty() {
            return json!({"count": 0});
        }
        values.sort_unstable_by(f32::total_cmp);
        let values: &[f32] = values;
        let n = values.len();
        let rank = |p: f64| {
            r3(f64::from(
                values[((p * n as f64).ceil() as usize).clamp(1, n) - 1],
            ))
        };
        let mean = values.iter().map(|&v| f64::from(v)).sum::<f64>() / n as f64;
        json!({
            "count": n, "mean": r3(mean), "min": r3(f64::from(values[0])),
            "p50": rank(0.5), "p99": rank(0.99), "max": r3(f64::from(values[n - 1])),
        })
    }

    fn bump(map: &mut BTreeMap<String, u64>, key: String) {
        let key = if map.len() >= MAX_REASONS && !map.contains_key(&key) {
            "other".to_string()
        } else {
            key
        };
        *map.entry(key).or_default() += 1;
    }

    /// Own-snake motion and anchors, using the decoder's length rules: own
    /// packets omit the two-byte snake id (`encode::with_id`).
    fn own_motion(p: &[u8]) -> bool {
        match p[0] {
            b'G' | b'N' | b'd' | b'7' => true,
            b'=' => p.len() == 7,
            b'+' => p.len() == 10,
            b'e' | b'E' | b'3' | b'4' | b'5' => p.len() <= 3,
            _ => false,
        }
    }

    /// Snake, food and prey updates; excludes pongs, maps, HUD and lifecycle.
    fn world_update(p: &[u8]) -> bool {
        matches!(
            p[0],
            b's' | b'g'
                | b'n'
                | b'G'
                | b'N'
                | b'+'
                | b'='
                | b'e'
                | b'E'
                | b'3'
                | b'4'
                | b'5'
                | b'd'
                | b'7'
                | b'h'
                | b'r'
                | b'F'
                | b'b'
                | b'f'
                | b'c'
                | b'C'
                | b'<'
                | b'w'
                | b'y'
                | b'j'
        )
    }

    /// Repel from nearby opponents, turn back from the rim, else seek food;
    /// always keep at least MIN_TURN so the snake weaves.
    fn steer(o: &Observation, weave: &mut f64) -> f64 {
        let me = &o.me;
        let (head, heading) = (me.head, me.heading);
        if !heading.is_finite() {
            return 0.;
        }
        let reach = 200. + me.radius;
        let (mut ax, mut ay) = (0., 0.);
        for other in &o.opponents {
            let limit = reach + other.radius;
            for p in std::iter::once(&other.head).chain(other.body.iter()) {
                let (dx, dy) = (head.x - p.x, head.y - p.y);
                let d = dx.hypot(dy);
                if d < limit && d > 1e-6 {
                    let w = (limit - d) / (limit * d);
                    ax += dx * w;
                    ay += dy * w;
                }
            }
        }
        let center = o.arena.center;
        let desired = if ax != 0. || ay != 0. {
            Some(ay.atan2(ax))
        } else if head.distance(center) > 0.8 * o.arena.radius {
            Some((center.y - head.y).atan2(center.x - head.x))
        } else {
            o.food
                .iter()
                .filter_map(|f| {
                    let d = f.point().distance(head);
                    (d < 600.).then_some((f.value / (d + 40.), f))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, f)| (f.y - head.y).atan2(f.x - head.x))
        };
        let turn = desired.map_or(0., |target| (target - heading + PI).rem_euclid(TAU) - PI);
        let turn = if !turn.is_finite() || turn.abs() < MIN_TURN {
            *weave = -*weave;
            MIN_TURN * *weave
        } else {
            turn.clamp(-MAX_TURN, MAX_TURN)
        };
        heading + turn
    }

    #[derive(Default)]
    struct Window {
        frames: u64,
        bytes_in: u64,
        messages_out: u64,
        bytes_out: u64,
        own_frames: u64,
        world_frames: u64,
        /// Messages read after the first in one service call (queued receipts).
        burst_frames: u64,
    }

    struct Interval {
        client: u64,
        connection: u64,
        at: f64,
        interval: f32,
        header: u32,
    }

    struct Ctx {
        epoch: Instant,
        start: f64,
        end: f64,
        addr: SocketAddr,
        url: String,
        stopping: bool,
        pending: usize,
        next_start: f64,
        counters: BTreeMap<&'static str, u64>,
        closes: BTreeMap<String, u64>,
        errors: BTreeMap<String, u64>,
        window: Window,
        records: Vec<Interval>,
        frame_records: Vec<(u64, u64, f64, u64, usize)>,
        window_counters: BTreeMap<&'static str, u64>,
        own: Vec<f32>,
        own_header: Vec<f32>,
        own_deviation: Vec<f32>,
        world: Vec<f32>,
        world_deviation: Vec<f32>,
        lateness: Vec<f32>,
        connect_ms: Vec<f32>,
        poll: Vec<f32>,
        second_own: Vec<f32>,
        second_late_max: f64,
    }

    impl Ctx {
        fn now(&self) -> f64 {
            self.epoch.elapsed().as_secs_f64()
        }
        fn in_window(&self, t: f64) -> bool {
            t >= self.start && t < self.end
        }
        fn count(&mut self, key: &'static str) {
            self.add(key, 1);
        }
        fn add(&mut self, key: &'static str, n: u64) {
            *self.counters.entry(key).or_default() += n;
            if self.in_window(self.now()) {
                *self.window_counters.entry(key).or_default() += n;
            }
        }
    }

    enum Link {
        Idle,
        Http(Box<MidHandshake<ClientHandshake<TcpStream>>>, f64),
        Open(Box<WebSocket<TcpStream>>),
    }

    enum Cause {
        Death,
        ServerClose,
        Error,
        Shutdown,
    }

    struct Client {
        id: u64,
        nickname: String,
        link: Link,
        retry_at: f64,
        failures: u32,
        rng: u64,
        state: ClientState,
        connections: u64,
        opened_at: f64,
        last_received: f64,
        challenged: bool,
        admitted: bool,
        ever_admitted: bool,
        playing: bool,
        boost: Option<bool>,
        next_control: f64,
        weave: f64,
        last_own: Option<f64>,
        own_header_ms: u64,
        server_ms: u64,
        last_world: Option<f64>,
        // Measured-window totals.
        frames: u64,
        bytes: u64,
        own_intervals: u64,
        own_max: f64,
        admissions: u64,
        deaths: u64,
    }

    impl Client {
        fn new(id: u64) -> Self {
            Self {
                id,
                nickname: format!("L{id}"),
                link: Link::Idle,
                retry_at: 0.,
                failures: 0,
                rng: id.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1,
                state: ClientState::new(),
                connections: 0,
                opened_at: 0.,
                last_received: 0.,
                challenged: false,
                admitted: false,
                ever_admitted: false,
                playing: false,
                boost: None,
                next_control: 0.,
                weave: if id % 2 == 0 { 1. } else { -1. },
                last_own: None,
                own_header_ms: 0,
                server_ms: 0,
                last_world: None,
                frames: 0,
                bytes: 0,
                own_intervals: 0,
                own_max: 0.,
                admissions: 0,
                deaths: 0,
            }
        }

        fn unit(&mut self) -> f64 {
            let mut x = self.rng;
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            self.rng = x;
            (x >> 11) as f64 / (1_u64 << 53) as f64
        }

        fn backoff(&mut self) -> f64 {
            let exponent = self.failures.saturating_sub(1).min(8) as i32;
            (RETRY_BASE * 2_f64.powi(exponent)).min(MAX_RETRY) * (0.75 + 0.5 * self.unit())
        }

        /// Index into CATEGORIES. An admitted dead client is never "playing".
        fn category(&self) -> usize {
            match &self.link {
                Link::Idle => 0,
                Link::Http(..) => 1,
                Link::Open(_) if !self.admitted => 2,
                Link::Open(_) => match self.state.status() {
                    ClientStatus::Playing => 4,
                    ClientStatus::Dead => 5,
                    _ => 3,
                },
            }
        }

        fn service(&mut self, ctx: &mut Ctx) {
            match std::mem::replace(&mut self.link, Link::Idle) {
                Link::Idle => self.start(ctx),
                Link::Http(mid, started) => {
                    let now = ctx.now();
                    if now - started > HANDSHAKE_TIMEOUT {
                        self.end(ctx, Cause::Error, "handshake timeout", now);
                        return;
                    }
                    match mid.handshake() {
                        Ok((socket, _)) => self.opened(ctx, socket),
                        Err(HandshakeError::Interrupted(mid)) => {
                            self.link = Link::Http(Box::new(mid), started)
                        }
                        Err(HandshakeError::Failure(e)) => {
                            self.end(ctx, Cause::Error, &format!("handshake: {e}"), now)
                        }
                    }
                }
                Link::Open(socket) => {
                    self.link = Link::Open(socket);
                    self.serve_open(ctx);
                }
            }
        }

        fn start(&mut self, ctx: &mut Ctx) {
            let now = ctx.now();
            if ctx.stopping
                || now < self.retry_at
                || now < ctx.next_start
                || ctx.pending >= PENDING_LIMIT
            {
                return;
            }
            ctx.next_start = now + START_GAP;
            ctx.pending += 1;
            ctx.count("connectAttempts");
            let started = Instant::now();
            let stream = TcpStream::connect_timeout(&ctx.addr, CONNECT_TIMEOUT).and_then(|s| {
                s.set_nodelay(true)?;
                s.set_nonblocking(true)?;
                Ok(s)
            });
            ctx.connect_ms
                .push((started.elapsed().as_secs_f64() * 1000.) as f32);
            let stream = match stream {
                Ok(stream) => stream,
                Err(e) => {
                    let detail = format!("tcp connect: {:?}", e.kind());
                    self.end(ctx, Cause::Error, &detail, now);
                    return;
                }
            };
            match tungstenite::client::client_with_config(
                ctx.url.as_str(),
                stream,
                Some(socket_config()),
            ) {
                Ok((socket, _)) => self.opened(ctx, socket),
                Err(HandshakeError::Interrupted(mid)) => self.link = Link::Http(Box::new(mid), now),
                Err(HandshakeError::Failure(e)) => {
                    self.end(ctx, Cause::Error, &format!("handshake: {e}"), now)
                }
            }
        }

        fn opened(&mut self, ctx: &mut Ctx, socket: WebSocket<TcpStream>) {
            let now = ctx.now();
            self.link = Link::Open(Box::new(socket));
            self.connections += 1;
            if self.ever_admitted {
                ctx.count("rejoins");
            }
            ctx.count("opened");
            self.opened_at = now;
            self.last_received = now;
            self.challenged = false;
            self.admitted = false;
            self.playing = false;
            self.boost = None;
            self.last_own = None;
            self.own_header_ms = 0;
            self.server_ms = 0;
            self.last_world = None;
            let result = self
                .state
                .new_connection(now)
                .and_then(|()| self.send(ctx, &[99, 0], now));
            if let Err(e) = result {
                self.end(ctx, Cause::Error, &e, now);
            }
        }

        fn send(&mut self, ctx: &mut Ctx, bytes: &[u8], at: f64) -> Result<(), String> {
            let Link::Open(socket) = &mut self.link else {
                return Err("send without an open socket".into());
            };
            match socket.write(Message::Binary(bytes.to_vec().into())) {
                Ok(()) => {}
                // tungstenite keeps the frame buffered; a later flush sends it.
                Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(format!("write: {e}")),
            }
            self.state.record_sent(bytes, at)?;
            ctx.count("messagesOut");
            ctx.add("bytesOut", bytes.len() as u64);
            if ctx.in_window(at) {
                ctx.window.messages_out += 1;
                ctx.window.bytes_out += bytes.len() as u64;
            }
            Ok(())
        }

        fn serve_open(&mut self, ctx: &mut Ctx) {
            let mut burst = 0;
            for _ in 0..MAX_READS {
                let Link::Open(socket) = &mut self.link else {
                    return;
                };
                let message = socket.read();
                // Each message keeps its own receipt time, even within a burst.
                let at = ctx.now();
                match message {
                    Ok(Message::Binary(bytes)) => {
                        burst += 1;
                        if burst > 1 && ctx.in_window(at) {
                            ctx.window.burst_frames += 1;
                        }
                        self.last_received = at;
                        if let Err(e) = self.receive(ctx, &bytes, at) {
                            self.end(ctx, Cause::Error, &e, at);
                            return;
                        }
                        if self.state.status() == ClientStatus::Dead {
                            ctx.count("deaths");
                            if ctx.in_window(at) {
                                self.deaths += 1;
                            }
                            self.end(ctx, Cause::Death, "own death", at);
                            return;
                        }
                    }
                    Ok(Message::Close(frame)) => {
                        let reason = frame
                            .map(|f| f.reason.to_string())
                            .filter(|r| !r.is_empty())
                            .unwrap_or_else(|| "close without reason".into());
                        self.end(ctx, Cause::ServerClose, &reason, at);
                        return;
                    }
                    Ok(Message::Text(_)) => {
                        self.end(ctx, Cause::Error, "unexpected text message", at);
                        return;
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {
                        break;
                    }
                    Err(
                        tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed,
                    ) => {
                        self.end(ctx, Cause::ServerClose, "closed without close frame", at);
                        return;
                    }
                    Err(e) => {
                        self.end(ctx, Cause::Error, &format!("read: {e}"), at);
                        return;
                    }
                }
            }
            let now = ctx.now();
            if now - self.last_received > IDLE_TIMEOUT {
                self.end(ctx, Cause::Error, "receive timeout", now);
                return;
            }
            if !self.admitted && now - self.opened_at > SETUP_TIMEOUT {
                self.end(ctx, Cause::Error, "setup timeout", now);
                return;
            }
            if let Err(e) = self.tick(ctx, now) {
                self.end(ctx, Cause::Error, &e, now);
                return;
            }
            let Link::Open(socket) = &mut self.link else {
                return;
            };
            match socket.flush() {
                Ok(()) => {}
                Err(tungstenite::Error::Io(e)) if e.kind() == io::ErrorKind::WouldBlock => {}
                Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                    self.end(ctx, Cause::ServerClose, "closed without close frame", now)
                }
                Err(e) => self.end(ctx, Cause::Error, &format!("flush: {e}"), now),
            }
        }

        fn receive(&mut self, ctx: &mut Ctx, bytes: &[u8], at: f64) -> Result<(), String> {
            let packets = split_frame(bytes)?;
            self.state.receive_frame(bytes, at)?;
            ctx.count("framesIn");
            ctx.add("bytesIn", bytes.len() as u64);
            let measured = ctx.in_window(at);
            if measured {
                self.frames += 1;
                self.bytes += bytes.len() as u64;
                ctx.window.frames += 1;
                ctx.window.bytes_in += bytes.len() as u64;
            }
            if let Some(prompt) = packets.iter().find(|p| p[0] == b'6') {
                if self.challenged || self.admitted {
                    return Err("unexpected repeated challenge".into());
                }
                self.challenged = true;
                let reply = challenge::solve_challenge(&prompt[1..])?;
                self.send(ctx, &reply, at)?;
                let login = login_packet(&self.nickname, (self.id % 9) as u8)?;
                self.send(ctx, &login, at)?;
                ctx.count("loginsSent");
            }
            let header = u64::from(bytes[0]) << 8 | u64::from(bytes[1]);
            if packets.iter().any(|p| p[0] == b'a') {
                // The setup header is the game-clock epoch, not an interval.
                self.admitted = true;
                self.ever_admitted = true;
                self.failures = 0;
                ctx.count("admitted");
                if measured {
                    self.admissions += 1;
                }
                self.last_own = None;
                self.last_world = None;
                self.own_header_ms = 0;
            } else if self.admitted {
                self.own_header_ms += header;
                self.server_ms += header;
            }
            if self.admitted && measured {
                ctx.frame_records.push((
                    self.id,
                    self.connections,
                    at * 1000.,
                    self.server_ms,
                    bytes.len(),
                ));
            }
            if packets.iter().any(|p| own_motion(p)) {
                if measured {
                    ctx.window.own_frames += 1;
                }
                if let Some(previous) = self.last_own {
                    if measured && previous >= ctx.start {
                        let interval = (at - previous) * 1000.;
                        ctx.own.push(interval as f32);
                        ctx.own_header.push(self.own_header_ms as f32);
                        ctx.own_deviation.push((interval - NOMINAL_MS).abs() as f32);
                        ctx.second_own.push(interval as f32);
                        ctx.records.push(Interval {
                            client: self.id,
                            connection: self.connections,
                            at,
                            interval: interval as f32,
                            header: self.own_header_ms.min(u64::from(u32::MAX)) as u32,
                        });
                        self.own_intervals += 1;
                        self.own_max = self.own_max.max(interval);
                    }
                }
                self.last_own = Some(at);
                self.own_header_ms = 0;
            }
            if packets.iter().any(|p| world_update(p)) {
                if measured {
                    ctx.window.world_frames += 1;
                }
                if let Some(previous) = self.last_world {
                    if measured && previous >= ctx.start {
                        let interval = (at - previous) * 1000.;
                        ctx.world.push(interval as f32);
                        ctx.world_deviation
                            .push((interval - NOMINAL_MS).abs() as f32);
                    }
                }
                self.last_world = Some(at);
            }
            Ok(())
        }

        fn tick(&mut self, ctx: &mut Ctx, now: f64) -> Result<(), String> {
            if self.state.ping_due(now)? {
                self.send(ctx, &[251], now)?;
            }
            if self.state.status() != ClientStatus::Playing {
                return Ok(());
            }
            if !self.playing {
                self.playing = true;
                ctx.count("lifeStarts");
                self.next_control = now + self.unit() * CONTROL_INTERVAL;
                return Ok(());
            }
            if now < self.next_control {
                return Ok(());
            }
            let late = (now - self.next_control) * 1000.;
            if ctx.in_window(now) {
                ctx.lateness.push(late as f32);
                ctx.second_late_max = ctx.second_late_max.max(late);
            }
            self.next_control += CONTROL_INTERVAL;
            if self.next_control <= now {
                self.next_control = now + CONTROL_INTERVAL;
                ctx.count("controlSkips");
            }
            let frame = self.state.frame(now)?;
            let Some(observation) = frame.observation.as_ref() else {
                return Ok(());
            };
            let heading = if std::env::var_os("STUDY_WATCHER").is_some() {
                now * 1.2
            } else {
                steer(observation, &mut self.weave)
            };
            for packet in action_packets(
                Action {
                    heading,
                    boost: false,
                },
                self.boost,
            )? {
                self.send(ctx, &packet, now)?;
            }
            self.boost = Some(false);
            ctx.count("controls");
            Ok(())
        }

        fn end(&mut self, ctx: &mut Ctx, cause: Cause, detail: &str, now: f64) {
            if let Link::Open(mut socket) = std::mem::replace(&mut self.link, Link::Idle) {
                // Close and drop at once: a lingering close would hold one of
                // the host's limited pending sessions.
                let _ = socket.close(None);
                let _ = socket.flush();
            }
            self.state.disconnect();
            let admitted = std::mem::replace(&mut self.admitted, false);
            self.playing = false;
            let stage = if admitted {
                "after admission"
            } else {
                "before admission"
            };
            let (key, delay) = match cause {
                Cause::Death => {
                    self.failures = 0;
                    ("local: own death".to_string(), DEATH_RETRY)
                }
                Cause::Shutdown => ("local: shutdown".to_string(), 0.),
                Cause::ServerClose => {
                    if !admitted && (detail.contains("full") || detail.contains("spawn")) {
                        ctx.count("refusals");
                    }
                    ctx.count(if admitted {
                        "closedAfterAdmission"
                    } else {
                        "rejectedBeforeAdmission"
                    });
                    if admitted {
                        self.failures = 0;
                    }
                    self.failures += 1;
                    (format!("server {stage}: {detail}"), self.backoff())
                }
                Cause::Error => {
                    ctx.count("errors");
                    bump(&mut ctx.errors, detail.to_string());
                    self.failures += 1;
                    (format!("local error {stage}"), self.backoff())
                }
            };
            bump(&mut ctx.closes, key);
            self.retry_at = now + delay;
        }
    }

    pub fn run() -> Result<(), String> {
        let args = parse_args()?;
        let epoch = Instant::now();
        let epoch_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_secs_f64()
            * 1000.;
        let common: f64 = std::env::var("STUDY_EPOCH_UNIX_S")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(epoch_unix_ms / 1000.);
        let measured_start = (common + args.warmup - epoch_unix_ms / 1000.).max(0.);
        let csv_path = args.output.with_extension("intervals.csv");
        let telemetry_path = args.output.with_extension("telemetry.jsonl");
        // Create outputs up front so a bad path fails before any load.
        let output = File::create(&args.output).map_err(|e| format!("{e}: {:?}", args.output))?;
        let csv = File::create(&csv_path).map_err(|e| format!("{e}: {csv_path:?}"))?;
        let telemetry_file =
            File::create(&telemetry_path).map_err(|e| format!("{e}: {telemetry_path:?}"))?;
        let ip: Ipv4Addr = std::env::var("STUDY_SERVER_IP")
            .unwrap_or_else(|_| "127.0.0.1".into())
            .parse()
            .map_err(|_| "bad study IPv4")?;
        if !ip.is_loopback() && !ip.is_private() {
            return Err("Only loopback or study private IPv4 is permitted".into());
        }
        let addr = SocketAddr::V4(SocketAddrV4::new(ip, args.port));
        let mut ctx = Ctx {
            epoch,
            start: measured_start,
            end: measured_start + args.duration,
            addr,
            url: format!("ws://{addr}/slither"),
            stopping: false,
            pending: 0,
            next_start: 0.,
            counters: BTreeMap::new(),
            closes: BTreeMap::new(),
            errors: BTreeMap::new(),
            window: Window::default(),
            records: Vec::new(),
            frame_records: Vec::new(),
            window_counters: BTreeMap::new(),
            own: Vec::new(),
            own_header: Vec::new(),
            own_deviation: Vec::new(),
            world: Vec::new(),
            world_deviation: Vec::new(),
            lateness: Vec::new(),
            connect_ms: Vec::new(),
            poll: Vec::new(),
            second_own: Vec::new(),
            second_late_max: 0.,
        };
        let mut clients: Vec<Client> = (0..args.count as u64)
            .map(|i| Client::new(args.offset + i))
            .collect();
        let (start, end) = (ctx.start, ctx.end);
        let mut telemetry: Vec<Value> = Vec::new();
        let mut counts = [0_usize; CATEGORIES.len()];
        let mut weighted = [0_f64; CATEGORIES.len()];
        let mut previous_pass: Option<f64> = None;
        let (mut busy, mut busy_window, mut passes) = (0_f64, 0_f64, 0_u64);
        let (mut second, mut second_busy, mut second_passes, mut second_poll_max) =
            (1_u64, 0_f64, 0_u64, 0_f64);
        let mut last_counters = ctx.counters.clone();
        let mut rotate = 0;
        let n = clients.len();
        loop {
            let pass_start = Instant::now();
            let now = ctx.now();
            if let Some(previous) = previous_pass {
                let overlap = (now.min(end) - previous.max(start)).max(0.);
                for (total, &count) in weighted.iter_mut().zip(&counts) {
                    *total += count as f64 * overlap;
                }
                let interval = (now - previous) * 1000.;
                if previous >= start && now < end {
                    ctx.poll.push(interval as f32);
                }
                second_poll_max = second_poll_max.max(interval);
            }
            previous_pass = Some(now);
            if now >= second as f64 {
                let delta: BTreeMap<&str, u64> = ctx
                    .counters
                    .iter()
                    .map(|(&k, &v)| (k, v - last_counters.get(k).copied().unwrap_or(0)))
                    .filter(|&(_, d)| d > 0)
                    .collect();
                let clients_now: BTreeMap<&str, usize> =
                    CATEGORIES.iter().copied().zip(counts).collect();
                telemetry.push(json!({
                    "second": second, "time": r3(now), "measuring": ctx.in_window(now),
                    "clients": clients_now, "delta": delta,
                    "ownIntervalMs": stats(&mut ctx.second_own),
                    "passes": second_passes, "busySeconds": r3(second_busy),
                    "pollMaxMs": r3(second_poll_max),
                    "controlLateMaxMs": r3(ctx.second_late_max),
                }));
                last_counters = ctx.counters.clone();
                ctx.second_own.clear();
                ctx.second_late_max = 0.;
                (second_busy, second_passes, second_poll_max) = (0., 0, 0.);
                second = now.floor() as u64 + 1;
            }
            if now >= end {
                break;
            }
            ctx.pending = clients
                .iter()
                .filter(|c| matches!(c.category(), 1 | 2))
                .count();
            for k in 0..n {
                clients[(rotate + k) % n].service(&mut ctx);
            }
            rotate = (rotate + 1) % n;
            counts = [0; CATEGORIES.len()];
            for client in &clients {
                counts[client.category()] += 1;
            }
            passes += 1;
            second_passes += 1;
            let spent = pass_start.elapsed();
            busy += spent.as_secs_f64();
            second_busy += spent.as_secs_f64();
            if now >= start {
                busy_window += spent.as_secs_f64();
            }
            if spent < POLL {
                thread::sleep(POLL - spent);
            }
        }
        let finished = ctx.now();
        // Preserve live processes through the external resource end sample.
        thread::sleep(Duration::from_secs(2));
        ctx.stopping = true;
        let final_counts: BTreeMap<&str, usize> = CATEGORIES.iter().copied().zip(counts).collect();
        for client in &mut clients {
            if !matches!(client.link, Link::Idle) {
                client.end(&mut ctx, Cause::Shutdown, "shutdown", finished);
            }
        }

        let mut csv = BufWriter::with_capacity(1 << 20, csv);
        let write = |csv: &mut BufWriter<File>, records: &[Interval]| -> io::Result<()> {
            writeln!(
                csv,
                "client,connection,receipt_ms,interval_ms,header_interval_ms"
            )?;
            for r in records {
                writeln!(
                    csv,
                    "{},{},{:.3},{:.3},{}",
                    r.client,
                    r.connection,
                    r.at * 1000.,
                    r.interval,
                    r.header
                )?;
            }
            csv.flush()
        };
        write(&mut csv, &ctx.records).map_err(|e| format!("{e}: {csv_path:?}"))?;
        let mut frames_out = BufWriter::new(
            File::create(args.output.with_extension("frames.csv")).map_err(|e| e.to_string())?,
        );
        writeln!(frames_out, "client,connection,receipt_ms,server_ms,bytes")
            .map_err(|e| e.to_string())?;
        for (client, connection, receipt, server, bytes) in &ctx.frame_records {
            writeln!(
                frames_out,
                "{client},{connection},{receipt:.6},{server},{bytes}"
            )
            .map_err(|e| e.to_string())?;
        }
        frames_out.flush().map_err(|e| e.to_string())?;
        let mut telemetry_out = BufWriter::new(telemetry_file);
        for line in &telemetry {
            serde_json::to_writer(&mut telemetry_out, line).map_err(|e| e.to_string())?;
            telemetry_out.write_all(b"\n").map_err(|e| e.to_string())?;
        }
        telemetry_out.flush().map_err(|e| e.to_string())?;

        let measured = (finished.min(end) - start).max(1e-9);
        let mean_clients: BTreeMap<&str, f64> = CATEGORIES
            .iter()
            .copied()
            .zip(weighted.iter().map(|w| r3(w / measured)))
            .collect();
        let per_client: Vec<Value> = clients
            .iter()
            .map(|c| {
                json!({
                    "client": c.id, "connections": c.connections, "admissions": c.admissions,
                    "deaths": c.deaths, "frames": c.frames, "bytesIn": c.bytes,
                    "ownIntervals": c.own_intervals, "ownMaxMs": r3(c.own_max),
                })
            })
            .collect();
        let w = &ctx.window;
        let report = json!({
            "tool": "host_capacity_load",
            "args": {
                "count": args.count, "port": args.port, "durationSeconds": args.duration,
                "warmupSeconds": args.warmup, "clientOffset": args.offset,
            },
            "endpoint": "study-owned server",
            "epochUnixMs": r3(epoch_unix_ms),
            "constants": {
                "pendingLimit": PENDING_LIMIT, "startGapMs": START_GAP * 1000.,
                "connectTimeoutMs": CONNECT_TIMEOUT.as_millis() as u64,
                "controlIntervalMs": CONTROL_INTERVAL * 1000., "pollSleepMs": 1,
                "nominalIntervalMs": NOMINAL_MS, "deathRetryMs": DEATH_RETRY * 1000.,
                "retryBaseMs": RETRY_BASE * 1000., "maxRetryMs": MAX_RETRY * 1000.,
                "handshake": "challenge", "framing": "elapsed header",
            },
            "window": {
                "startSeconds": start, "endSeconds": end, "finishedSeconds": r3(finished),
                "measuredSeconds": r3(measured),
            },
            "clients": {"timeWeightedMean": mean_clients, "final": final_counts},
            "counters": ctx.counters,
            "holdCounters": ctx.window_counters,
            "closeReasons": ctx.closes,
            "errors": ctx.errors,
            "traffic": {
                "frames": w.frames, "bytesIn": w.bytes_in, "messagesOut": w.messages_out,
                "bytesOut": w.bytes_out, "ownFrames": w.own_frames, "worldFrames": w.world_frames,
                "burstFrames": w.burst_frames,
            },
            "intervals": {
                "definition": "own: receipt gap between frames with own motion or anchor packets ('G','N','d','7', id-less 'e','E','3','4','5','=','+'); world: frames with any snake/food/prey packet; pongs, maps, HUD and lifecycle excluded; per connection, both receipts inside the window",
                "ownMs": stats(&mut ctx.own),
                "ownAbsDeviationMs": stats(&mut ctx.own_deviation),
                "ownHeaderMs": stats(&mut ctx.own_header),
                "worldMs": stats(&mut ctx.world),
                "worldAbsDeviationMs": stats(&mut ctx.world_deviation),
            },
            "generator": {
                "passes": passes,
                "pollIntervalMs": stats(&mut ctx.poll),
                "controlLatenessMs": stats(&mut ctx.lateness),
                "connectCallMs": stats(&mut ctx.connect_ms),
                "busySeconds": r3(busy), "busySecondsInWindow": r3(busy_window),
                "wallSeconds": r3(finished),
            },
            "perClient": per_client,

            "limitations": [
                "Receipt times are taken when this process reads each message (at most one service pass per ms plus pass time), not kernel arrival; burstFrames counts messages read back-to-back.",
                "Own-motion frames exist only when own heading, wanted angle, speed or anchors change; the bot keeps turning, but a missing change lengthens an interval. worldMs is the cadence of any gameplay packet.",
                "TCP connect is a blocking call bounded at 50 ms; connectCallMs reports it.",
                "Dead clients close immediately and reconnect, unlike the stock death fade.",
                "CPU time is measured by the runner. Excess uses all post-setup frame receipts and summed headers, minimum-normalized within each connection in the hold.",
            ],
        });
        let mut output = BufWriter::new(output);
        serde_json::to_writer_pretty(&mut output, &report).map_err(|e| e.to_string())?;
        output.write_all(b"\n").map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())
    }
}

fn main() {
    #[cfg(feature = "networking")]
    if let Err(error) = load::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
    #[cfg(not(feature = "networking"))]
    {
        eprintln!("host_capacity_load requires the networking feature");
        std::process::exit(1);
    }
}
