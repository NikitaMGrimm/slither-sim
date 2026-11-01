//! Loopback-only CPU presentation sink. No window, adapter, device or GPU submission.
#![forbid(unsafe_code)]
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod probe {
    use slither_client::{
        protocol::{self, ElapsedHeader},
        session_policy::Phase,
        state::Point,
    };
    use slither_wgpu_slice::{
        Result,
        game::{EffectKind, LocalGame, loopback_endpoint},
        native::{Event, Transport},
    };
    use std::time::{Duration, Instant};

    const EXTENT: [u32; 2] = [960, 540];

    #[derive(Default)]
    struct Summary {
        opened: bool,
        challenge: bool,
        login: bool,
        protocol: u8,
        own: bool,
        steering: u64,
        boost_on: bool,
        boost_off: bool,
        moved: bool,
        turned: bool,
        accelerated: bool,
        decelerated: bool,
        leaderboard: bool,
        minimap: bool,
        pictures: u64,
        closed: bool,
        baseline: Option<(Point, f64, f64)>,
        peak_speed: f64,
    }
    impl Summary {
        fn observe(&mut self, game: &LocalGame, active: f64) -> Result<()> {
            let state = game.client().received();
            if let Some(setup) = &state.setup {
                self.protocol = setup.protocol;
            }
            if let Some(snake) = state.own_snake_id.and_then(|id| state.snakes.get(&id)) {
                self.own = true;
                let (head, heading, speed) =
                    *self
                        .baseline
                        .get_or_insert((snake.head, snake.heading, snake.speed));
                self.moved |=
                    self.steering > 0 && (snake.head.x - head.x).hypot(snake.head.y - head.y) > 10.;
                let turn = (snake.heading - heading)
                    .sin()
                    .atan2((snake.heading - heading).cos())
                    .abs();
                self.turned |= self.steering > 0 && turn > 0.2;
                if self.boost_on && !self.boost_off {
                    self.peak_speed = self.peak_speed.max(snake.speed);
                    self.accelerated |= snake.speed > speed + 20.;
                }
                if self.boost_off && active > 3.5 {
                    self.decelerated |= self.accelerated && snake.speed < self.peak_speed - 20.;
                }
            }
            if game.phase() == Phase::Failed || game.failure.is_some() {
                return Err(format!(
                    "session: {}",
                    game.failure.as_deref().unwrap_or("failed")
                ));
            }
            Ok(())
        }
        fn print(&self, seconds: f64) {
            println!(
                "{{\"websocket\":{},\"challenge\":{},\"loginSent\":{},\"protocol\":{},\"setup\":{},\"ownSnake\":{},\"steeringCommands\":{},\"boostOn\":{},\"boostOff\":{},\"moved\":{},\"turned\":{},\"accelerated\":{},\"decelerated\":{},\"leaderboard\":{},\"minimap\":{},\"preparedFrames\":{},\"cleanClose\":{},\"seconds\":{:.3}}}",
                self.opened,
                self.challenge,
                self.login,
                self.protocol,
                self.protocol != 0,
                self.own,
                self.steering,
                self.boost_on,
                self.boost_off,
                self.moved,
                self.turned,
                self.accelerated,
                self.decelerated,
                self.leaderboard,
                self.minimap,
                self.pictures,
                self.closed,
                seconds
            );
        }
    }

    struct Host {
        game: LocalGame,
        transport: Transport,
        summary: Summary,
        started: Instant,
        playing: Option<Instant>,
    }
    impl Host {
        fn now(&self) -> f64 {
            self.started.elapsed().as_secs_f64() * 1000.
        }
        fn active(&self) -> f64 {
            self.playing.map_or(0., |t| t.elapsed().as_secs_f64())
        }
        fn effects(&mut self) -> Result<()> {
            for _ in 0..64 {
                let Some(e) = self.game.effect() else {
                    return Ok(());
                };
                if matches!(e.kind, EffectKind::Connect(_)) {
                    self.game.construct(e.generation, e.id, self.now())?;
                }
                self.transport
                    .execute(&e)
                    .map_err(|e| format!("transport: {e}"))?;
                self.game.acknowledge(e.generation, e.id, true)?;
                if let EffectKind::Send(bytes) = e.kind {
                    if self.game.phase() == Phase::Playing && matches!(bytes.as_slice(), [0..=250])
                    {
                        self.summary.steering += 1;
                    }
                    self.summary.boost_on |= bytes == [253];
                    self.summary.boost_off |= self.summary.boost_on && bytes == [254];
                    // Heading 115 is also the byte 's'; only a full message is login.
                    self.summary.login |= bytes.len() > 1 && bytes.first() == Some(&b's');
                }
            }
            Err("effects: drain budget".into())
        }
        fn pictures(&mut self) -> Result<()> {
            for _ in 0..16 {
                let Some(_picture) = self.game.take_frame() else {
                    return Ok(());
                };
                // CPU sink accepts the actual production prepared picture. This
                // commits LocalGame's transaction, but makes no GPU/pixel claim.
                self.game.presented(true)?;
                self.summary.pictures += 1;
            }
            if self.game.has_frame() {
                Err("presentation: drain budget".into())
            } else {
                Ok(())
            }
        }
        fn flush(&mut self) -> Result<()> {
            self.game.observe_time(self.now())?;
            self.pictures()?;
            self.effects()?;
            self.summary.observe(&self.game, self.active())
        }
        fn tick(&mut self) -> Result<()> {
            for event in self.transport.poll() {
                match event {
                    Event::Opened(g) => {
                        self.summary.opened = true;
                        self.game.opened(g, self.now())?;
                    }
                    Event::Binary(g, bytes) => {
                        self.game
                            .receive(g, &bytes, self.now())
                            .map_err(|e| format!("ingress: {e}"))?;
                        self.flush()?;
                        let decoded = protocol::read_frame(&bytes, ElapsedHeader::Absent)
                            .map_err(|e| format!("decode: {e}"))?;
                        for c in decoded.commands() {
                            let c = c.bytes;
                            self.summary.challenge |= c[0] == b'6';
                            self.summary.leaderboard |= c[0] == b'l';
                            self.summary.minimap |= matches!(c[0], b'M' | b'V' | b'U');
                        }
                    }
                    Event::Closed(_) => return Err("session: unexpected remote close".into()),
                    Event::Failed(_, e) => return Err(format!("transport: {e}")),
                }
                self.flush()?;
            }
            if self.game.phase() == Phase::Playing {
                if self.summary.own {
                    self.playing.get_or_insert_with(Instant::now);
                }
                let heading = self.summary.baseline.map_or(0., |(_, h, _)| h) + 0.8;
                self.game.pointer(
                    480. + 220. * heading.cos(),
                    270. + 220. * heading.sin(),
                    960.,
                    540.,
                    false,
                )?;
                self.game.key("Space", (1.5..3.).contains(&self.active()));
            }
            self.flush()?;
            self.game
                .begin_frame(self.now(), EXTENT, 1.)
                .map_err(|e| format!("presentation: {e}"))?;
            self.flush()
        }
        fn run(&mut self, seconds: f64) -> Result<()> {
            // Login/setup have a separate wall-clock budget; gameplay never depends on frame counts.
            let admission = Duration::from_secs(20);
            while self.playing.is_none() || self.active() < seconds {
                self.tick()?;
                if self.playing.is_none() && self.started.elapsed() > admission {
                    return Err("admission: no playing session within 20 seconds".into());
                }
                std::thread::sleep(Duration::from_millis(16));
            }
            self.transport
                .begin_close()
                .map_err(|e| format!("close: {e}"))?;
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                for event in self.transport.poll() {
                    match event {
                        Event::Closed(g) => {
                            self.game.closed(g);
                            self.summary.closed = true;
                            return Ok(());
                        }
                        Event::Failed(_, e) => return Err(format!("close: {e}")),
                        // In-flight game data can precede the peer's Close. No new controls are issued.
                        Event::Binary(_, _) => {}
                        Event::Opened(_) => return Err("close: unexpected open".into()),
                    }
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err("close: no peer acknowledgement within 5 seconds".into())
        }
    }

    fn arguments(args: &[String]) -> Result<(String, f64)> {
        if args.len() != 4 || args[0] != "--server" || args[2] != "--seconds" {
            return Err("arguments: --server 127.0.0.1:<port> --seconds 5..60".into());
        }
        let endpoint = slither_wgpu_slice::official::explicit_endpoint(&args[1])?;
        // Preserve the probe's literal loopback gate before URL normalization
        // removes :80 or expands IPv4 aliases such as 127.1.
        let port = loopback_endpoint(&format!("ws://{}/slither", args[1]))?;
        if [8766, 18884, 18885].contains(&port) {
            return Err("arguments: reserved port".into());
        }
        let seconds: f64 = args[3].parse().map_err(|_| "arguments: invalid seconds")?;
        if !seconds.is_finite() || !(5. ..=60.).contains(&seconds) {
            return Err("arguments: seconds must be 5..60".into());
        }
        Ok((endpoint, seconds))
    }
    pub fn run() -> Result<()> {
        let (endpoint, seconds) = arguments(&std::env::args().skip(1).collect::<Vec<_>>())?;
        let started = Instant::now();
        let mut host = Host {
            game: LocalGame::new(0.)?,
            transport: Transport::default(),
            summary: Summary::default(),
            started,
            playing: None,
        };
        host.game.play(&endpoint, "Local E2E", 0)?;
        let result = host.run(seconds);
        host.transport.close();
        host.summary.print(started.elapsed().as_secs_f64());
        result
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn server_arguments_keep_literal_loopback_and_time_bounds() {
            for port in [1, 80, 49152, 65535] {
                for seconds in [5., 8.5, 60.] {
                    let args = [
                        "--server".into(),
                        format!("127.0.0.1:{port}"),
                        "--seconds".into(),
                        seconds.to_string(),
                    ];
                    let (endpoint, duration) = arguments(&args).unwrap();
                    let url = url::Url::parse(&endpoint).unwrap();
                    assert_eq!(url.host_str(), Some("127.0.0.1"));
                    assert_eq!(url.port_or_known_default(), Some(port));
                    assert_eq!(duration, seconds);
                }
            }
            for server in [
                "127.1:49152",
                "localhost:49152",
                "192.0.2.1:49152",
                "127.0.0.1:0",
                "127.0.0.1:8766",
                "127.0.0.1:18884",
                "127.0.0.1:18885",
            ] {
                assert!(
                    arguments(&[
                        "--server".into(),
                        server.into(),
                        "--seconds".into(),
                        "5".into()
                    ])
                    .is_err()
                );
            }
            for seconds in ["0", "4.9", "60.1", "NaN", "inf"] {
                assert!(
                    arguments(&[
                        "--server".into(),
                        "127.0.0.1:49152".into(),
                        "--seconds".into(),
                        seconds.into()
                    ])
                    .is_err()
                );
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(error) = probe::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
