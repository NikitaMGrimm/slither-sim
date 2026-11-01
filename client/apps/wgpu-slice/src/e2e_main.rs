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
    const MAP_CAMERA_MOTION_MIN_UNITS: f64 = 1.;

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
        peak_received: usize,
        peak_prepared: usize,
        skipped: u64,
        full_map: bool,
        best_map_overflow: Option<f64>,
        worst_map_overflow: Option<f64>,
        map_fit_samples: usize,
        map_camera_origin: Option<Point>,
        map_camera_max_displacement: f64,
        closed: bool,
        baseline: Option<(Point, f64, f64)>,
        peak_speed: f64,
    }
    impl Summary {
        fn observe_map_fit(&mut self, overflow: f64, position: Point) {
            let origin = *self.map_camera_origin.get_or_insert(position);
            self.map_camera_max_displacement = self
                .map_camera_max_displacement
                .max((position.x - origin.x).hypot(position.y - origin.y));
            self.map_fit_samples += 1;
            self.best_map_overflow =
                Some(self.best_map_overflow.map_or(overflow, |p| p.min(overflow)));
            self.worst_map_overflow = Some(
                self.worst_map_overflow
                    .map_or(overflow, |p| p.max(overflow)),
            );
            self.full_map = self.worst_map_overflow.is_some_and(|p| p <= 1e-6);
        }
        fn map_camera_moved(&self) -> bool {
            self.map_camera_max_displacement > MAP_CAMERA_MOTION_MIN_UNITS
        }
        fn observe(&mut self, game: &LocalGame, active: f64) -> Result<()> {
            let state = game.client().received();
            self.peak_received = self.peak_received.max(state.snakes.len());
            self.skipped = game.draw_diagnostics.skipped_frames;
            if self.skipped != 0 {
                return Err("presentation: picture skipped".into());
            }
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
                "{{\"websocket\":{},\"challenge\":{},\"loginSent\":{},\"protocol\":{},\"setup\":{},\"ownSnake\":{},\"steeringCommands\":{},\"boostOn\":{},\"boostOff\":{},\"moved\":{},\"turned\":{},\"accelerated\":{},\"decelerated\":{},\"leaderboard\":{},\"minimap\":{},\"preparedFrames\":{},\"cleanClose\":{},\"peakReceivedSnakes\":{},\"peakPreparedSnakes\":{},\"pictureSkips\":{},\"fullMap\":{},\"mapFitSamples\":{},\"bestMapOverflowPixels\":{},\"worstMapOverflowPixels\":{},\"mapCameraMoved\":{},\"mapCameraMaxDisplacementWorldUnits\":{},\"seconds\":{:.3}}}",
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
                self.peak_received,
                self.peak_prepared,
                self.skipped,
                self.full_map,
                self.map_fit_samples,
                self.best_map_overflow
                    .map_or("null".to_owned(), |p| p.to_string()),
                self.worst_map_overflow
                    .map_or("null".to_owned(), |p| p.to_string()),
                self.map_camera_moved(),
                self.map_camera_origin.map_or("null".to_owned(), |_| self
                    .map_camera_max_displacement
                    .to_string()),
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
        legacy: bool,
        full_map_requested: bool,
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
                let Some(mut e) = self.game.effect() else {
                    return Ok(());
                };
                // Test harness only: the simulator chooses 13 on a legacy login.
                // Production LocalGame keeps its stock login for every server.
                if self.legacy
                    && let EffectKind::Send(bytes) = &mut e.kind
                    && bytes.starts_with(&[b's', 30])
                {
                    let mut login = vec![b's', 12];
                    login.extend_from_slice(&bytes[24..]);
                    *bytes = login;
                }
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
                let Some(picture) = self.game.take_frame() else {
                    return Ok(());
                };
                self.summary.peak_prepared =
                    self.summary.peak_prepared.max(picture.frame.snakes.len());
                // Accept the picture before observing its committed camera;
                // the candidate's draw transform is not yet in game.client().
                self.game.presented(true)?;
                // Only post-wheel committed pictures establish fit. An early
                // Setup or a single lucky frame cannot accept the moving view.
                if self.full_map_requested
                    && self.playing.is_some()
                    && let Some(setup) = &self.game.client().received().setup
                {
                    let camera = self.game.client().camera();
                    let radius = setup
                        .radius
                        .max(self.game.client().boundary().map_or(0., |b| b.displayed()));
                    let overflow = [camera.position().x, camera.position().y]
                        .into_iter()
                        .zip(EXTENT)
                        .map(|(position, side)| {
                            2. * (radius + (position - setup.grid_center).abs()) * camera.zoom()
                                - f64::from(side)
                        })
                        .fold(0., f64::max);
                    self.summary.observe_map_fit(overflow, camera.position());
                }
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
                if self.summary.own && self.playing.is_none() {
                    // Wheel admission requires a playing, live own snake.
                    if self.full_map_requested && !self.game.wheel_zoom(-1000.) {
                        return Err("full-map: zoom admission rejected".into());
                    }
                    self.playing = Some(Instant::now());
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
            let deadline = Instant::now() + slither_wgpu_slice::native::CLOSE_TIMEOUT;
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
        let legacy = match std::env::var("SLITHER_CLIENT_E2E_PROTOCOL").as_deref() {
            Err(std::env::VarError::NotPresent) | Ok("19") => false,
            Ok("13") => true,
            _ => return Err("SLITHER_CLIENT_E2E_PROTOCOL must be 13 or 19".into()),
        };
        let started = Instant::now();
        let mut host = Host {
            game: LocalGame::new(0.)?,
            transport: Transport::default(),
            summary: Summary::default(),
            started,
            playing: None,
            legacy,
            full_map_requested: std::env::var("SLITHER_CLIENT_E2E_FULL_MAP").as_deref() == Ok("1"),
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
        fn whole_map_requires_every_post_zoom_picture_to_fit() {
            let mut summary = Summary::default();
            assert_eq!(summary.map_fit_samples, 0);
            assert_eq!(summary.best_map_overflow, None);
            assert_eq!(summary.worst_map_overflow, None);
            assert!(!summary.full_map);
            summary.observe_map_fit(0., Point::default());
            assert!(summary.full_map);
            summary.observe_map_fit(3.56, Point { x: 2., y: 0. });
            summary.observe_map_fit(0., Point { x: 4., y: 0. });
            assert_eq!(summary.map_fit_samples, 3);
            assert_eq!(summary.best_map_overflow, Some(0.));
            assert_eq!(summary.worst_map_overflow, Some(3.56));
            assert!(!summary.full_map);
            assert!(summary.map_camera_moved());
        }
        #[test]
        fn authored_motion_sequence_keeps_both_clipped_endpoints() {
            let mut summary = Summary::default();
            for (index, overflow) in [3.56, 0., 4.].into_iter().enumerate() {
                summary.observe_map_fit(
                    overflow,
                    Point {
                        x: index as f64 * 2.,
                        y: 0.,
                    },
                );
            }
            assert_eq!(summary.map_fit_samples, 3);
            assert_eq!(summary.best_map_overflow, Some(0.));
            assert_eq!(summary.worst_map_overflow, Some(4.));
            assert!(summary.map_camera_moved());
            assert!(!summary.full_map);
        }
        #[test]
        fn stationary_post_zoom_summary_does_not_prove_camera_motion() {
            // Lifetime snake movement is deliberately true: it cannot stand
            // in for camera movement during the fitted post-wheel pictures.
            let mut summary = Summary {
                moved: true,
                ..Summary::default()
            };
            for _ in 0..3 {
                summary.observe_map_fit(0., Point { x: 20., y: 30. });
            }
            assert!(summary.full_map);
            assert_eq!(summary.map_camera_max_displacement, 0.);
            assert!(!summary.map_camera_moved());
        }
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
