#![cfg(feature = "networking")]

//! Loopback checks of the raw protocol-19 host. No outside server is contacted.
use slither_sim::{
    Action,
    client::{
        Backend, BotConfig, ClientFrame, ClientStatus, Host, HostConfig, LatencyConfig,
        NetworkBackend, NetworkConfig, PlayerConfig, RunningHost, SimulationConfig,
        challenge::solve_challenge, network::Handshake, network::login_packet,
    },
};
use std::{
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpStream},
    time::{Duration, Instant},
};

const WAIT: Duration = Duration::from_secs(5);

struct Running {
    address: SocketAddr,
    host: RunningHost,
}

impl Running {
    fn start(config: HostConfig) -> Self {
        let host = Host::bind(config).unwrap().start().unwrap();
        Self {
            address: host.local_addr(),
            host,
        }
    }

    fn url(&self) -> String {
        format!("ws://{}/slither", self.address)
    }

    /// Stop the host and require that no simulator error occurred.
    fn finish(self) -> u64 {
        let ticks = self.host.ticks();
        self.host.stop().unwrap();
        ticks
    }
}

fn config(max_clients: usize, bots: usize) -> HostConfig {
    let mut simulation = SimulationConfig {
        world: slither_sim::Config::sandbox(),
        ..Default::default()
    };
    // Every snake is covered, so all players receive each other.
    simulation.world.vision = 3000.;
    HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        seed: 7,
        players: PlayerConfig {
            max: max_clients,
            ..Default::default()
        },
        bots: BotConfig {
            count: bots,
            ..Default::default()
        },
        simulation,
    }
}

fn network(url: String, nickname: &str, handshake: Handshake) -> NetworkBackend {
    NetworkBackend::connect(NetworkConfig {
        url,
        nickname: nickname.into(),
        handshake,
        decision_interval_seconds: 0.02,
        ..NetworkConfig::default()
    })
    .unwrap()
}

fn until(
    backend: &mut NetworkBackend,
    what: &str,
    done: impl Fn(&ClientFrame) -> bool,
) -> ClientFrame {
    let start = Instant::now();
    loop {
        let frame = backend.next_frame().unwrap();
        if done(&frame) {
            return frame;
        }
        assert!(start.elapsed() < WAIT, "timed out waiting for {what}");
    }
}

#[test]
fn dense_initial_sector_listings_fit_bounded_socket_transports() {
    let mut config = config(1, 0);
    config.simulation.world.radius = 1500.;
    config.simulation.world.food_count = 100_000;
    config.simulation.world.ambient_food.enabled = false;
    config.simulation.world.ambient_replenish_per_second = 0.;
    let host = Running::start(config);
    let mut bot = NetworkBackend::connect(NetworkConfig {
        url: host.url(),
        handshake: Handshake::Direct,
        record_wire: true,
        decision_interval_seconds: 0.02,
        ..Default::default()
    })
    .unwrap();
    let mut received_bytes = 0;
    let start = Instant::now();
    loop {
        let frame = bot.next_frame().unwrap();
        assert_ne!(frame.status, ClientStatus::Disconnected);
        for record in bot.drain_records().unwrap() {
            if record.event == slither_sim::client::WireEvent::Receive {
                assert!(record.bytes.len() + 10 <= 1 << 20);
                received_bytes += record.bytes.len();
            }
        }
        // Food interest is bounded independently of vision. Concentrate enough
        // food inside the real stencil to require multiple listing packets;
        // whole-world receipt of 100,000 pellets is no longer the host policy.
        // The backend's aggregate-emission test covers the 1 MiB split itself.
        if received_bytes > 8192
            && frame
                .observation
                .as_ref()
                .is_some_and(|o| o.food.len() > 10_000)
        {
            break;
        }
        assert!(start.elapsed() < WAIT, "incomplete initial broadcast");
    }
    bot.disconnect();
    assert!(host.finish() > 0);
}

#[test]
fn native_client_joins_plays_and_measures_ping() {
    let host = Running::start(config(2, 2));
    let mut bot = network(host.url(), "Native", Handshake::Challenge);
    let frame = until(&mut bot, "self", |f| f.status == ClientStatus::Playing);
    assert_eq!(frame.settings.as_ref().unwrap().protocol, 19);
    bot.send_action(Action {
        heading: 1.,
        boost: false,
    })
    .unwrap();
    let frame = until(&mut bot, "ping and food", |f| {
        f.timing.ping_seconds.is_some()
            && f.observation.as_ref().is_some_and(|o| !o.food.is_empty())
    });
    // Two background bots share the world with this client.
    assert!(frame.observation.unwrap().opponents.len() >= 2);
    bot.disconnect();
    assert!(host.finish() > 0);
}

#[test]
fn pit_markers_reach_stock_sockets_but_not_portable_bots() {
    // Spare seats allow reconnect to overlap the old socket's asynchronous close.
    let mut config = config(4, 0);
    config.simulation.world = slither_sim::Config::battledome();
    config.simulation.world.food_count = 0;
    config.simulation.world.ambient_replenish_per_second = 0.;
    let host = Running::start(config);
    // No special header: this follows stock framing and food parsing.
    let mut stock = Raw::connect(host.address, "/slither");
    assert!(stock.status().starts_with("HTTP/1.1 101"));
    stock.send(&[1]);
    stock.send(&stock_login(b"Stock", 0, &[]));
    let start = Instant::now();
    loop {
        let received = packets(&stock.binary());
        assert!(received.iter().all(|p| p[0] != b'~'));
        if received.iter().any(|p| p[0] == b'F' && p.len() > 3) {
            for packet in received.iter().filter(|p| p[0] == b'F') {
                for marker in packet[3..].as_chunks::<4>().0 {
                    assert_eq!((marker[0], marker[3]), (8, 31));
                }
            }
            break;
        }
        assert!(start.elapsed() < WAIT, "no marker listing");
    }
    // The native network backend requests physical food automatically on both
    // login paths and every reconnect. With no real food, its view stays empty.
    for handshake in [Handshake::Challenge, Handshake::Direct] {
        let mut bot = network(host.url(), "Bot", handshake);
        for life in 0..2 {
            if life > 0 {
                bot.reconnect().unwrap();
            }
            let frame = until(&mut bot, &format!("self {handshake:?} life {life}"), |f| {
                assert_ne!(
                    f.status,
                    ClientStatus::Disconnected,
                    "{handshake:?} life {life}"
                );
                assert_ne!(f.status, ClientStatus::Dead, "{handshake:?} life {life}");
                f.status == ClientStatus::Playing
            });
            assert!(frame.rules.is_some());
            assert!(frame.observation.unwrap().food.is_empty());
            for _ in 0..5 {
                assert!(
                    bot.next_frame()
                        .unwrap()
                        .observation
                        .unwrap()
                        .food
                        .is_empty()
                );
            }
        }
        bot.disconnect();
        // A close is handled asynchronously; wait for the remote seat to free.
        let start = Instant::now();
        while host.host.players() > 1 {
            assert!(start.elapsed() < WAIT, "bot connection did not close");
            std::thread::sleep(Duration::from_millis(2));
        }
    }
    host.finish();
}

#[test]
fn bot_threads_steer_their_snakes() {
    let mut config = config(1, 4);
    config.bots.threads = 2;
    let host = Running::start(config);
    let mut watcher = network(host.url(), "Watcher", Handshake::Direct);
    let first = until(&mut watcher, "every bot", |f| {
        f.observation
            .as_ref()
            .is_some_and(|o| o.opponents.len() == 4)
    })
    .observation
    .unwrap();
    // A hosted snake keeps its spawn heading until its client sends a command,
    // so a turn shows that bot's thread decoded frames and steered.
    let mut turned = std::collections::BTreeSet::new();
    let start = Instant::now();
    while turned.len() < 4 {
        assert!(start.elapsed() < WAIT, "only {} bots turned", turned.len());
        let frame = watcher.next_frame().unwrap();
        for snake in frame.observation.iter().flat_map(|o| &o.opponents) {
            let spawned = first.opponents.iter().find(|o| o.id == snake.id);
            if spawned.is_some_and(|o| (o.heading - snake.heading).abs() > 0.2) {
                turned.insert(snake.id.clone());
            }
        }
    }
    watcher.disconnect();
    assert!(host.finish() > 0);
}

#[test]
fn portable_client_requests_session_id_and_sends_custom_profile() {
    let host = Running::start(config(1, 0));
    let mut bot = NetworkBackend::connect(NetworkConfig {
        url: host.url(),
        nickname: "Café".into(),
        skin: 7,
        custom_skin: vec![8, 0, 255],
        request_session_id: true,
        record_wire: true,
        decision_interval_seconds: 0.02,
        ..NetworkConfig::default()
    })
    .unwrap();
    let (mut session_snake, mut profile_snake) = (None, None);
    let deadline = Instant::now() + WAIT;
    loop {
        let frame = bot.next_frame().unwrap();
        for record in bot.drain_records().unwrap() {
            if record.event != slither_sim::client::WireEvent::Receive {
                continue;
            }
            for p in slither_sim::client::protocol::split_frame(&record.bytes).unwrap() {
                match p[0] {
                    b'S' => {
                        assert_eq!(p.len(), 7);
                        session_snake = Some(u16::from_be_bytes([p[1], p[2]]));
                    }
                    b's' if p.len() > 31 => {
                        assert_eq!(p[15], 7);
                        assert_eq!(&p[22..31], &[4, b'C', b'a', b'f', 233, 3, 8, 0, 255]);
                        profile_snake = Some(u16::from_be_bytes([p[1], p[2]]));
                    }
                    _ => {}
                }
            }
        }
        if session_snake.is_some() && profile_snake.is_some() {
            assert_eq!(session_snake, profile_snake);
            assert_eq!(frame.status, ClientStatus::Playing);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "missing optional session/profile data"
        );
    }
    bot.disconnect();
    host.finish();
}

#[test]
fn two_clients_share_one_world_and_rejoin_a_freed_slot() {
    let host = Running::start(config(2, 0));
    let mut first = network(host.url(), "First", Handshake::Challenge);
    let mut second = network(host.url(), "Second", Handshake::Direct);
    let a = until(&mut first, "opponent", |f| {
        f.observation
            .as_ref()
            .is_some_and(|o| o.opponents.len() == 1)
    });
    let b = until(&mut second, "opponent", |f| {
        f.observation
            .as_ref()
            .is_some_and(|o| o.opponents.len() == 1)
    });
    let (a, b) = (a.observation.unwrap(), b.observation.unwrap());
    assert_eq!(a.opponents[0].id, b.me.id);
    assert_eq!(b.opponents[0].id, a.me.id);

    // A third client is refused while both slots are taken.
    let mut third = network(host.url(), "Third", Handshake::Challenge);
    let start = Instant::now();
    loop {
        match third.next_frame() {
            Ok(frame) if frame.status == ClientStatus::Disconnected => break,
            Ok(frame) => assert_ne!(frame.status, ClientStatus::Playing),
            Err(_) => break,
        }
        assert!(start.elapsed() < WAIT, "full server kept the client");
    }

    // Leaving frees the slot for a fresh session with its own setup.
    first.disconnect();
    let start = Instant::now();
    loop {
        if third.reconnect().is_ok()
            && (0..50).any(|_| {
                third
                    .next_frame()
                    .is_ok_and(|frame| frame.status == ClientStatus::Playing)
            })
        {
            break;
        }
        assert!(start.elapsed() < WAIT, "freed slot was not reused");
    }
    until(&mut second, "still playing", |f| {
        f.status == ClientStatus::Playing
    });
    drop((second, third));
    host.finish();
}

/// A hand-written client, to control exact bytes and TCP segment boundaries.
struct Raw {
    stream: TcpStream,
    buffer: Vec<u8>,
}

impl Raw {
    fn connect(address: SocketAddr, path: &str) -> Self {
        let mut raw = Self::open(address);
        raw.write(
            format!(
                "GET {path} HTTP/1.1\r\nHost: {address}\r\nUpgrade: websocket\r\n\
                 Connection: Upgrade\r\nSec-WebSocket-Version: 13\r\n\
                 Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n"
            )
            .as_bytes(),
        );
        raw
    }

    fn open(address: SocketAddr) -> Self {
        let stream = TcpStream::connect(address).unwrap();
        stream.set_read_timeout(Some(WAIT)).unwrap();
        stream.set_nodelay(true).unwrap();
        Self {
            stream,
            buffer: Vec::new(),
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.stream.write_all(bytes).unwrap();
    }

    /// HTTP status line of the upgrade response.
    fn status(&mut self) -> String {
        while !self.buffer.windows(4).any(|w| w == b"\r\n\r\n") {
            self.fill().expect("closed before an HTTP response");
        }
        let end = self
            .buffer
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap()
            + 4;
        let head = String::from_utf8(self.buffer.drain(..end).collect()).unwrap();
        head.lines().next().unwrap().to_string()
    }

    fn fill(&mut self) -> Option<()> {
        let mut chunk = [0; 4096];
        match self.stream.read(&mut chunk) {
            Ok(0) => None,
            Ok(n) => {
                self.buffer.extend(&chunk[..n]);
                Some(())
            }
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted
                ) =>
            {
                None
            }
            Err(error) => panic!("{error}"),
        }
    }

    fn frame(payload: &[u8]) -> Vec<u8> {
        let mask = [0x12, 0x34, 0x56, 0x78];
        let mut frame = vec![0x82];
        if payload.len() < 126 {
            frame.push(0x80 | payload.len() as u8);
        } else {
            frame.push(0x80 | 126);
            frame.extend((payload.len() as u16).to_be_bytes());
        }
        frame.extend(mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        frame
    }

    fn send(&mut self, payload: &[u8]) {
        self.write(&Self::frame(payload));
    }

    /// Next server message as `(opcode, payload)`; `None` when the socket ended.
    fn next(&mut self) -> Option<(u8, Vec<u8>)> {
        loop {
            if let Some((opcode, header, length)) = self.header()
                && self.buffer.len() >= header + length
            {
                let payload = self.buffer[header..header + length].to_vec();
                self.buffer.drain(..header + length);
                return Some((opcode, payload));
            }
            self.fill()?;
        }
    }

    fn header(&self) -> Option<(u8, usize, usize)> {
        let b = &self.buffer;
        if b.len() < 2 {
            return None;
        }
        assert_eq!(b[1] & 0x80, 0, "server frames are unmasked");
        match b[1] & 0x7f {
            126 if b.len() >= 4 => Some((b[0] & 15, 4, usize::from(b[2]) << 8 | usize::from(b[3]))),
            126 => None,
            127 if b.len() >= 10 => Some((
                b[0] & 15,
                10,
                usize::try_from(u64::from_be_bytes(b[2..10].try_into().unwrap())).unwrap(),
            )),
            127 => None,
            n => Some((b[0] & 15, 2, usize::from(n))),
        }
    }

    fn binary(&mut self) -> Vec<u8> {
        loop {
            match self.next() {
                Some((2, payload)) => return payload,
                Some((8, payload)) => panic!("closed: {:?}", String::from_utf8_lossy(&payload)),
                Some(_) => {}
                None => panic!("socket ended"),
            }
        }
    }

    /// Wait for a server close, skipping game messages already queued.
    fn closed(&mut self) -> String {
        loop {
            match self.next() {
                Some((8, payload)) => {
                    return String::from_utf8_lossy(payload.get(2..).unwrap_or(&[])).into();
                }
                Some(_) => {}
                None => return String::new(),
            }
        }
    }
}

fn stock_login(nickname: &[u8], skin: u8, custom: &[u8]) -> Vec<u8> {
    let mut login = vec![115, 30, 1, 35];
    login.extend([
        54, 206, 204, 169, 97, 178, 74, 136, 124, 117, 14, 210, 106, 236, 8, 208, 136, 213, 140,
        111,
    ]);
    login.extend([skin, nickname.len() as u8]);
    login.extend(nickname);
    login.extend([0, 255]);
    login.extend(custom);
    login
}

/// The stock template copies the host's fixed literal into its reply.
const LITERAL: &[u8; 27] = b"SlitherSimLocalHostChalleng";

/// Stock-client packet split of one header-free message.
fn packets(message: &[u8]) -> Vec<Vec<u8>> {
    if message[0] >= 32 {
        return vec![message.to_vec()];
    }
    let (mut m, mut packets) = (0, Vec::new());
    while m < message.len() {
        let length = if message[m] < 32 {
            m += 2;
            usize::from(message[m - 2]) << 8 | usize::from(message[m - 1])
        } else {
            m += 1;
            usize::from(message[m - 1] - 32)
        };
        packets.push(message[m..m + length].to_vec());
        m += length;
    }
    packets
}

#[test]
fn stock_default_login_uses_bare_messages_and_routes_controls() {
    let host = Running::start(config(2, 0));
    let mut stock = Raw::connect(host.address, "/slither");
    assert!(stock.status().starts_with("HTTP/1.1 101"));
    stock.send(&[1]);
    stock.send(&[99, 0]);
    let challenge = stock.binary();
    assert_eq!(challenge[0], b'6', "bare framing has no elapsed header");
    solve_challenge(&challenge[1..]).unwrap();
    stock.send(LITERAL);
    stock.send(&stock_login(b"Stock", 5, &[1, 2, 3]));
    let setup = packets(&stock.binary()).remove(0);
    assert_eq!(setup[0], b'a');
    assert_eq!(setup[23], 19, "protocol version");

    // The stock appearance carries the login nickname and custom skin.
    let start = Instant::now();
    let appearance = loop {
        let message = stock.binary();
        if let Some(at) = message
            .windows(b"Stock".len() + 1)
            .position(|w| w == b"\x05Stock")
        {
            break message[at..].to_vec();
        }
        assert!(start.elapsed() < WAIT, "no own appearance");
    };
    assert_eq!(&appearance[6..10], [3, 1, 2, 3]);

    // Heading, boost, keyboard turns and a victory text are accepted; the
    // following ping is answered on this connection.
    for control in [
        &[100][..],
        &[253],
        &[254],
        &[252, 10],
        &[252, 200],
        &[255, 118, b'w', b'o', b'n'],
    ] {
        stock.send(control);
    }
    stock.send(&[251]);
    let start = Instant::now();
    while !stock.binary().contains(&b'p') {
        assert!(start.elapsed() < WAIT, "no pong");
    }
    host.finish();
}

#[test]
fn legacy_handshake_login_controls_and_reconnect_use_individual_protocol13_messages() {
    let host = Running::start(config(1, 0));
    for (selector, request_session, request) in [
        (12, true, &[99, 4][..]),
        (11, false, &[99][..]),
        (12, false, &[99, 3][..]),
    ] {
        let mut client = Raw::connect(host.address, "/slither");
        assert!(client.status().starts_with("HTTP/1.1 101"));
        if request_session {
            client.send(&[4]);
        }
        client.send(request);
        let challenge = client.binary();
        assert_eq!(&challenge[..3], &[0, 0, b'6']);
        client.send(b"CGLRYGPZKWJXMCTLEYTPMKJJ");
        // Synthetic legacy layout; no third-party client bytes or code.
        let mut login = vec![b's', selector, 7, 6];
        login.extend(b"Legacy");
        login.extend([0, 255, 9, 8]);
        client.send(&login);
        let setup = client.binary();
        assert_eq!(setup[2], b'a');
        assert_eq!(setup[25], 13);
        let start = Instant::now();
        loop {
            let message = client.binary();
            assert!(message[2] >= 32, "legacy clients cannot split bundles");
            if message[2] == b's' && message.len() > 9 {
                let p = &message[2..];
                assert_eq!(p[15], 7);
                assert_eq!(&p[22..33], b"\x06Legacy\x02\x09\x08\x00");
                break;
            }
            assert!(start.elapsed() < WAIT);
        }
        for control in [&[100][..], &[253], &[254], &[252, 10], &[251]] {
            client.send(control);
        }
        let mut session = false;
        loop {
            let message = client.binary();
            assert!(message[2] >= 32);
            session |= message[2] == b'S';
            if message[2] == b'p' {
                break;
            }
            assert!(start.elapsed() < WAIT);
        }
        assert_eq!(session, request_session);
        let mut close = Raw::frame(&[]);
        close[0] = 0x88;
        client.write(&close);
        // Give the socket thread its close before reclaiming the single seat.
        client.closed();
        let closing = Instant::now();
        let retire_tick = host.host.ticks();
        while host.host.players() != 0 || host.host.ticks() < retire_tick + 2 {
            assert!(closing.elapsed() < WAIT, "close did not free the seat");
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    host.finish();
}

/// Synthetic format fixture, not third-party code or a recorded login.
#[test]
fn short_modern_login_uses_protocol19_after_the_shared_challenge() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/browser-admission.json")).unwrap();
    let bytes = |key: &str| -> Vec<u8> {
        fixture[key]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect()
    };
    let host = Running::start(config(1, 0));
    let mut client = Raw::connect(host.address, "/slither");
    assert!(client.status().starts_with("HTTP/1.1 101"));
    client.send(&[4]);
    client.send(&bytes("request"));
    let challenge = client.binary();
    assert_eq!(&challenge[..3], &[0, 0, b'6']);
    client.send(fixture["replyAscii"].as_str().unwrap().as_bytes());
    client.send(&bytes("login"));
    let setup = packets(&client.binary()[2..]).remove(0);
    assert_eq!(setup[0], b'a');
    assert_eq!(setup[23], fixture["setupProtocol"].as_u64().unwrap() as u8);
    let started = Instant::now();
    let mut session = false;
    let mut appearance = false;
    client.send(&[100]);
    client.send(&[253]);
    client.send(&[254]);
    client.send(&[251]);
    loop {
        for packet in packets(&client.binary()[2..]) {
            session |= packet[0] == b'S';
            if packet[0] == b's' && packet.len() > 9 {
                let name = fixture["nickname"].as_str().unwrap().as_bytes();
                assert_eq!(packet[15], fixture["skin"].as_u64().unwrap() as u8);
                assert_eq!(usize::from(packet[22]), name.len());
                assert_eq!(&packet[23..23 + name.len()], name);
                let custom = bytes("customSkin");
                let at = 23 + name.len();
                assert_eq!(usize::from(packet[at]), custom.len());
                assert_eq!(&packet[at + 1..at + 1 + custom.len()], custom);
                appearance = true;
            }
            if packet[0] == b'p' && session && appearance {
                host.finish();
                return;
            }
        }
        assert!(started.elapsed() < WAIT, "short modern login did not join");
    }
}

#[test]
fn dense_legacy_prey_burst_keeps_its_socket_and_the_host_alive() {
    let mut config = config(2, 0);
    config.simulation.world.radius = 1000.;
    config.simulation.world.food_count = 0;
    config.simulation.world.ambient_replenish_per_second = 0.;
    config.simulation.world.prey.count = 5000;
    let host = Running::start(config);
    let mut legacy = Raw::connect(host.address, "/slither");
    assert!(legacy.status().starts_with("HTTP/1.1 101"));
    legacy.send(&[115, 12, 7, 0, 0, 255]);
    let setup = legacy.binary();
    assert_eq!(setup[2], b'a');
    assert_eq!(setup[25], 13);
    let mut prey = std::collections::BTreeSet::new();
    let started = Instant::now();
    // A few prey can be eaten before their first appearance. Still require
    // comfortably more individual frames than the former 4,096-frame limit.
    while prey.len() < 4500 {
        let frame = legacy.binary();
        assert!(frame[2] >= 32, "legacy messages remain individual");
        if frame[2] == b'y' && frame.len() == 22 {
            prey.insert(u16::from_be_bytes([frame[3], frame[4]]));
        }
        assert!(
            started.elapsed() < WAIT,
            "incomplete prey listing: {}",
            prey.len()
        );
    }
    legacy.send(&[251]);
    while legacy.binary()[2] != b'p' {
        assert!(started.elapsed() < WAIT, "legacy socket stopped responding");
    }
    let mut stock = network(host.url(), "Healthy", Handshake::Direct);
    until(&mut stock, "other stock recipient", |frame| {
        frame.status == ClientStatus::Playing && frame.timing.ping_seconds.is_some()
    });
    assert!(!host.host.is_finished());
    stock.disconnect();
    assert!(host.finish() > 0);
}

#[test]
fn oversized_legacy_cull_disconnects_that_player_while_stock_still_gets_pongs() {
    let mut config = config(2, 0);
    config.simulation.world.radius = 1000.;
    config.simulation.world.food_count = 100_000;
    config.simulation.world.ambient_food.enabled = false;
    config.simulation.world.ambient_replenish_per_second = 0.;
    config.simulation.world.dynamic_radius = true;
    config.simulation.world.size_to_population = false;
    config.simulation.world.radius_updates = vec![slither_sim::RadiusUpdate {
        time: 0.5,
        radius: 100.,
    }];
    let host = Running::start(config);
    let mut stock = Raw::connect(host.address, "/slither");
    assert!(stock.status().starts_with("HTTP/1.1 101"));
    stock.send(&stock_login(b"Healthy", 0, &[]));
    let mut stock_writer = stock.stream.try_clone().unwrap();
    // Keep this healthy recipient reading while the other socket receives its
    // large listing/cull. All threads inherit the test process's CPU affinity.
    let reader = std::thread::spawn(move || {
        let started = Instant::now();
        loop {
            let frame = stock.binary();
            if packets(&frame[2..]).iter().any(|p| p == b"p") {
                return;
            }
            assert!(started.elapsed() < WAIT, "stock did not receive its pong");
        }
    });
    let mut legacy = Raw::connect(host.address, "/slither");
    assert!(legacy.status().starts_with("HTTP/1.1 101"));
    legacy.send(&[115, 12, 7, 0, 0, 255]);
    let started = Instant::now();
    let reason = loop {
        if let Some((8, payload)) = legacy.next() {
            break String::from_utf8(payload[2..].to_vec()).unwrap();
        }
        assert!(
            started.elapsed() < WAIT,
            "over-budget legacy session stayed open"
        );
    };
    assert_eq!(reason, "Client output queue is full");
    assert!(!host.host.is_finished());
    stock_writer.write_all(&Raw::frame(&[251])).unwrap();
    reader.join().unwrap();
    assert!(host.finish() > 0);
}

#[test]
fn sequence_framing_and_native_reply_are_supported() {
    let host = Running::start(config(1, 0));
    let mut client = Raw::connect(host.address, "/slither");
    assert!(client.status().starts_with("HTTP/1.1 101"));
    client.send(&[2]);
    client.send(&[99, 0]);
    let challenge = client.binary();
    assert_eq!(&challenge[..3], [0, 1, b'6']);
    client.send(&solve_challenge(&challenge[3..]).unwrap());
    client.send(&login_packet("Seq", 0).unwrap());
    let setup = client.binary();
    assert_eq!(&setup[..2], [0, 2]);
    assert_eq!(packets(&setup[2..])[0][0], b'a');
    let next = client.binary();
    assert_eq!(&next[..2], [0, 3]);
    host.finish();
}

#[test]
fn session_ids_and_leaderboards_follow_self_in_all_framing_modes() {
    // Session negotiation is independent of the optional framing request.
    for requests in [
        vec![vec![4], vec![1]],
        vec![vec![2], vec![4]],
        vec![vec![4]],
        vec![vec![1]],
    ] {
        let host = Running::start(config(1, 0));
        let mut client = Raw::connect(host.address, "/slither");
        assert!(client.status().starts_with("HTTP/1.1 101"));
        let bare = requests.iter().any(|r| r == &[1]);
        let sequence = requests.iter().any(|r| r == &[2]);
        let want_session = requests.iter().any(|r| r == &[4]);
        for request in &requests {
            client.send(request);
        }
        client.send(&[99, 0]);
        let challenge = client.binary();
        let offset = if bare { 0 } else { 2 };
        assert_eq!(packets(&challenge[offset..])[0][0], b'6');
        client.send(LITERAL);
        client.send(&stock_login(b"Board", 7, &[1, 2, 3]));
        let mut self_id = None;
        let mut session = None;
        let mut board = false;
        let mut next_sequence = 2u16;
        let deadline = Instant::now() + WAIT;
        while !board || (want_session && session.is_none()) {
            let message = client.binary();
            if sequence {
                assert_eq!(u16::from_be_bytes([message[0], message[1]]), next_sequence);
                next_sequence = next_sequence.wrapping_add(1);
            }
            for packet in packets(&message[offset..]) {
                match packet[0] {
                    b's' if packet.len() > 6 && self_id.is_none() => {
                        self_id = Some(u16::from_be_bytes([packet[1], packet[2]]));
                    }
                    b'S' => {
                        assert!(want_session, "unsolicited session identifier");
                        assert_eq!(packet.len(), 7);
                        assert_eq!(Some(u16::from_be_bytes([packet[1], packet[2]])), self_id);
                        session = Some(u32::from_be_bytes(packet[3..7].try_into().unwrap()));
                    }
                    b'l' => {
                        assert!(self_id.is_some(), "leaderboard before self appearance");
                        assert_eq!(&packet[1..6], [1, 0, 1, 0, 1]);
                        assert!(packet.windows(5).any(|w| w == b"Board"));
                        board = true;
                    }
                    _ => {}
                }
            }
            assert!(
                Instant::now() < deadline,
                "missing leaderboard/session response"
            );
        }
        assert_eq!(session.is_some(), want_session);
        host.finish();
    }
}

#[test]
fn malformed_and_unsupported_clients_are_closed_alone() {
    let host = Running::start(config(2, 0));
    let mut bot = network(host.url(), "Steady", Handshake::Challenge);
    until(&mut bot, "self", |f| f.status == ClientStatus::Playing);

    let mut wrong_path = Raw::connect(host.address, "/other");
    assert!(wrong_path.status().starts_with("HTTP/1.1 404"));

    let cases: [&[&[u8]]; 6] = [
        &[&[4], &[4]],
        &[&[1], &[1]],
        &[&[1], &[2]],
        &[&[100]],
        &[&[99, 0], b"not the challenge reply...."],
        &[&[115, 30, 1]],
    ];
    for messages in cases {
        let mut client = Raw::connect(host.address, "/slither");
        assert!(client.status().starts_with("HTTP/1.1 101"));
        for message in messages {
            client.send(message);
        }
        assert!(!client.closed().is_empty(), "{messages:?} was not refused");
    }
    // Oversized input is refused before it is buffered as a message.
    let mut large = Raw::connect(host.address, "/slither");
    assert!(large.status().starts_with("HTTP/1.1 101"));
    large.send(&vec![0; 5000]);
    large.closed();

    // A logged-in client sending an unsupported command loses only its slot.
    let mut player = Raw::connect(host.address, "/slither");
    assert!(player.status().starts_with("HTTP/1.1 101"));
    player.send(&login_packet("Bad", 0).unwrap());
    assert_eq!(packets(&player.binary()[2..])[0][0], b'a');
    player.send(&[200, 1, 2, 3, 4]);
    player.closed();

    let frame = until(&mut bot, "continued play", |f| {
        f.status == ClientStatus::Playing && f.timing.ping_seconds.is_some()
    });
    assert_eq!(frame.status, ClientStatus::Playing);
    drop(bot);
    host.finish();
}

#[test]
fn partial_and_stalled_peers_do_not_block_others() {
    let host = Running::start(config(2, 1));
    // Half an HTTP request, and a peer that never sends anything.
    let mut partial_http = Raw::open(host.address);
    partial_http.write(b"GET /slither HTTP/1.1\r\nHost: x\r\nUpgrade: web");
    let _silent = Raw::open(host.address);
    // Half of a WebSocket frame after a completed upgrade.
    let mut partial_frame = Raw::connect(host.address, "/slither");
    assert!(partial_frame.status().starts_with("HTTP/1.1 101"));
    let request = Raw::frame(&[99, 0]);
    partial_frame.write(&request[..3]);
    // A joined client that never reads its messages.
    let mut stalled = Raw::connect(host.address, "/slither");
    assert!(stalled.status().starts_with("HTTP/1.1 101"));
    stalled.send(&login_packet("Stalled", 0).unwrap());

    let mut bot = network(host.url(), "Steady", Handshake::Challenge);
    let frame = until(&mut bot, "play beside stalled peers", |f| {
        f.observation
            .as_ref()
            .is_some_and(|o| o.opponents.len() == 2)
    });
    assert_eq!(frame.status, ClientStatus::Playing);

    // The interrupted messages resume where they stopped.
    partial_frame.write(&request[3..]);
    assert_eq!(partial_frame.binary()[2], b'6');
    partial_http.write(
        b"socket\r\nConnection: Upgrade\r\nSec-WebSocket-Version: 13\r\n\
          Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n",
    );
    assert!(partial_http.status().starts_with("HTTP/1.1 101"));
    drop(bot);
    host.finish();
}

#[test]
fn login_deadline_closes_idle_upgrades() {
    let host = Running::start(config(1, 0));
    let mut idle = Raw::connect(host.address, "/slither");
    idle.stream
        .set_read_timeout(Some(Duration::from_secs(15)))
        .unwrap();
    assert!(idle.status().starts_with("HTTP/1.1 101"));
    let start = Instant::now();
    assert_eq!(idle.closed(), "Login timeout");
    assert!(start.elapsed() >= Duration::from_secs(9));
    host.finish();
}

#[test]
fn configuration_is_bounded_and_documents_derived_fields() {
    let default = HostConfig::default();
    assert_eq!(default.bind, "127.0.0.1:8766".parse().unwrap());
    assert_eq!(default.players.max, 4);
    assert_eq!(default.bots.count, 4);
    assert_eq!(default.bots.threads, 2);
    assert_eq!(default.players.latency, LatencyConfig::default());
    assert_eq!(default.bots.latency, LatencyConfig::default());
    default.validate().unwrap();
    assert!(config(0, 4).validate().is_err());
    // Protocol-19 rank and population are u16: 65535 slots, not a policy cap of 128.
    config(100, 29).validate().unwrap();
    config(65_000, 535).validate().unwrap();
    let error = config(65_000, 536).validate().unwrap_err();
    assert!(error.contains("u16"), "{error}");
    let mut recording = config(1, 0);
    recording.simulation.record_wire = true;
    assert!(recording.validate().is_err());
    let mut derived = config(2, 2);
    derived.simulation.agents = 99;
    derived.simulation.controlled_slot = 50;
    derived.validate().unwrap();
    let mut threadless = config(1, 2);
    threadless.bots.threads = 0;
    assert!(threadless.validate().unwrap_err().contains("bots.threads"));
    threadless.bots.count = 0;
    threadless.validate().unwrap();
    let parsed: HostConfig =
        serde_json::from_str(r#"{"bind":"0.0.0.0:9000","players":{"max":3},"bots":{"count":0}}"#)
            .unwrap();
    assert_eq!(parsed.bind.port(), 9000);
    assert!(serde_json::from_str::<HostConfig>(r#"{"port":1}"#).is_err());
}

#[test]
fn host_latency_validation_names_the_role_and_rejects_player_processing() {
    let mut host = config(1, 1);
    host.simulation.latency = LatencyConfig::fixed(0.06);
    let error = host.validate().unwrap_err();
    for field in ["simulation.latency", "players.latency", "bots.latency"] {
        assert!(error.contains(field), "{error}");
    }
    host.simulation.latency = LatencyConfig::default();
    for (processing, jitter) in [([0.01, 0.01], 0.), ([0., 0.], 0.01)] {
        host.players.latency.processing_seconds = processing;
        host.players.latency.processing_jitter_seconds = jitter;
        let error = host.validate().unwrap_err();
        assert!(error.contains("players.latency"), "{error}");
        assert!(error.contains("must be zero"), "{error}");
    }
    host.players.latency = LatencyConfig::fixed(0.06);
    host.bots.latency.processing_seconds = [0.01, 0.02];
    host.bots.latency.processing_jitter_seconds = 0.003;
    host.validate().unwrap();
    for player in [true, false] {
        let mut invalid = host.clone();
        let (latency, field) = if player {
            (&mut invalid.players.latency, "players.latency")
        } else {
            (&mut invalid.bots.latency, "bots.latency")
        };
        latency.round_trip_seconds = vec![-1.];
        assert!(invalid.validate().unwrap_err().contains(field));
    }
}

#[test]
fn legacy_host_keys_are_rejected_with_grouped_config_names() {
    for (json, field) in [
        (r#"{"maxClients":4,"bots":4,"botThreads":2}"#, "maxClients"),
        (r#"{"botThreads":2}"#, "botThreads"),
    ] {
        let error = serde_json::from_str::<HostConfig>(json)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains(&format!("unknown field `{field}`")),
            "{error}"
        );
        assert!(
            error.contains("players") && error.contains("bots"),
            "{error}"
        );
    }
    let error = serde_json::from_str::<HostConfig>(r#"{"bots":4}"#)
        .unwrap_err()
        .to_string();
    assert!(error.contains("BotConfig"), "{error}");
    assert!(error.contains("expected"), "{error}");
}

#[test]
fn socket_bots_receive_exact_live_battledome_rules_while_stock_sockets_do_not() {
    let mut cfg = config(2, 0);
    cfg.simulation.world = slither_sim::Config::battledome();
    cfg.simulation.world.food_count = 0;
    cfg.simulation.world.ambient_replenish_per_second = 0.;
    cfg.simulation.world.radius_updates = vec![slither_sim::RadiusUpdate {
        time: 0.7,
        radius: 7168.,
    }];
    let host = Running::start(cfg);
    let mut bot = network(host.url(), "Rules", Handshake::Direct);
    let initial = until(&mut bot, "initial rules", |f| f.rules.is_some());
    assert_eq!(initial.rules.unwrap().pit_radius, Some(1784.));
    assert_eq!(initial.rules.unwrap().min_score, Some(400));
    assert_eq!(initial.rules.unwrap().kill_score_seconds, Some(900.));
    let grown = until(&mut bot, "grown pit", |f| {
        f.rules.is_some_and(|r| r.pit_radius.unwrap() > 3500.)
    });
    let pit = grown.rules.unwrap().pit_radius.unwrap();
    let shrunk = until(&mut bot, "shrinking pit", |f| {
        f.rules.is_some_and(|r| r.pit_radius.unwrap() < pit)
    });
    assert_eq!(shrunk.rules.unwrap().min_score, Some(400));
    // A stock-compatible socket provides no custom header and uses bare framing.
    let mut stock = Raw::connect(host.address, "/slither");
    assert!(stock.status().starts_with("HTTP/1.1 101"));
    stock.send(&[1]);
    stock.send(&stock_login(b"StockRules", 0, &[]));
    let mut saw_self = false;
    for _ in 0..40 {
        let received = packets(&stock.binary());
        assert!(received.iter().all(|p| p[0] != b'~'));
        saw_self |= received.iter().any(|p| p[0] == b's' && p.len() > 7);
    }
    assert!(saw_self);
    bot.disconnect();
    drop(stock);
    host.finish();
}
