//! Manual loopback measurement, never invoked by tests or CI. No desktop window.
#![forbid(unsafe_code)]
#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
mod rig {
    use slither_client::{session_policy::Phase, state::Point};
    use slither_wgpu_slice::{
        Result,
        game::{EffectKind, LocalGame, loopback_endpoint},
        gpu::{Renderer, instance},
        latency, latency_rig,
        native::{Event, Transport},
        render::PreparedFrame,
    };
    use std::{
        io::Write,
        path::PathBuf,
        sync::mpsc,
        time::{Duration, Instant},
    };

    const EXTENT: [u32; 2] = [960, 540];
    const FRAME: Duration = Duration::from_nanos(16_666_667);
    // Config::battledome's proportional geometry; the runner freezes this preset.
    const PIT_RATIO: f64 = 1784. / 3584.;

    struct Options {
        endpoint: String,
        output: PathBuf,
        seconds: f64,
        zoom: f64,
    }
    impl Options {
        fn parse(args: &[String]) -> Result<Self> {
            if args.len() != 8 {
                return Err(
                    "--server 127.0.0.1:PORT --output NEWDIR --seconds 5..150 --zoom 0|-17".into(),
                );
            }
            let mut server = None;
            let mut output = None;
            let mut seconds = None;
            let mut zoom = None;
            for pair in args.chunks_exact(2) {
                match pair[0].as_str() {
                    "--server" if server.is_none() => {
                        let endpoint = format!("ws://{}/slither", pair[1]);
                        loopback_endpoint(&endpoint)?;
                        server = Some(endpoint);
                    }
                    "--output" if output.is_none() => output = Some(PathBuf::from(&pair[1])),
                    "--seconds" if seconds.is_none() => seconds = pair[1].parse::<f64>().ok(),
                    "--zoom" if zoom.is_none() => zoom = pair[1].parse::<f64>().ok(),
                    _ => return Err("unknown/repeated measurement option".into()),
                }
            }
            let seconds = seconds
                .filter(|s| s.is_finite() && (5. ..=150.).contains(s))
                .ok_or("invalid seconds")?;
            let zoom = zoom
                .filter(|z| *z == 0. || *z == -17.)
                .ok_or("zoom must be 0 or -17 notches")?;
            let endpoint = server.ok_or("missing server")?;
            let port = url::Url::parse(&endpoint)
                .map_err(|e| e.to_string())?
                .port()
                .ok_or("missing port")?;
            if [8766, 18884, 18885].contains(&port) {
                return Err("reserved port".into());
            }
            Ok(Self {
                endpoint,
                output: output.ok_or("missing output")?,
                seconds,
                zoom,
            })
        }
    }

    struct Offscreen {
        renderer: Renderer,
        _texture: wgpu::Texture,
        view: wgpu::TextureView,
    }
    impl Offscreen {
        fn new() -> Result<Self> {
            let renderer = pollster::block_on(Renderer::new(
                &instance(),
                None,
                wgpu::TextureFormat::Rgba8Unorm,
            ))?;
            println!(
                "Offscreen adapter {:?}; MSAA{}",
                renderer.adapter_info, renderer.sample_count
            );
            let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("latency rig offscreen"),
                size: wgpu::Extent3d {
                    width: EXTENT[0],
                    height: EXTENT[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            Ok(Self {
                renderer,
                _texture: texture,
                view,
            })
        }
        fn draw(&mut self, p: &PreparedFrame) -> Result<()> {
            {
                let _sync = latency::PictureSpan::new("sync_begin", "sync_end");
                self.renderer
                    .sync_resources(&p.resources)
                    .map_err(|e| e.to_string())?;
            }
            self.renderer
                .render(&p.frame, &self.view)
                .map_err(|e| e.to_string())?;
            if let Some(overlay) = &p.overlay {
                self.renderer
                    .render_overlay(overlay, &self.view)
                    .map_err(|e| e.to_string())?;
            }
            // Offscreen GPU completion is deliberately serialized, as in --headless.
            // There is no swapchain or scanout; do not invent present trace events.
            self.renderer
                .device
                .poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(2)),
                })
                .map_err(|e| e.to_string())?;
            Ok(())
        }
    }

    fn orbit_heading(head: Point, radius: f64) -> f64 {
        let r = head.x.hypot(head.y);
        if r < 1. {
            return 0.;
        }
        let inward = ((r - radius) / 250.).clamp(-2., 8.);
        let x = -head.y / r - inward * head.x / r;
        let y = head.x / r - inward * head.y / r;
        y.atan2(x)
    }
    struct Host {
        game: LocalGame,
        transport: Transport,
        gpu: Offscreen,
        started: Instant,
        reference: Option<(u16, u64)>,
        zoomed: bool,
        zoom: f64,
        middle: bool,
        frames: u64,
        deaths: u64,
        sends: Vec<(u64, u32, u64, Vec<u8>)>,
    }
    impl Host {
        fn now(&self) -> f64 {
            self.started.elapsed().as_secs_f64() * 1000.
        }
        fn effects(&mut self) -> Result<()> {
            for _ in 0..64 {
                let Some(e) = self.game.effect() else {
                    return Ok(());
                };
                if matches!(e.kind, EffectKind::Connect(_)) {
                    self.game.construct(e.generation, e.id, self.now())?;
                }
                self.transport.execute(&e)?;
                self.game.acknowledge(e.generation, e.id, true)?;
                if let EffectKind::Send(bytes) = e.kind {
                    if self.sends.len() >= 65_536 {
                        return Err("control journal overflow".into());
                    }
                    self.sends
                        .push((latency::stamp(), e.generation, e.id, bytes));
                }
            }
            Err("effect drain budget".into())
        }
        fn pictures(&mut self) -> Result<()> {
            for _ in 0..16 {
                let Some(p) = self.game.take_frame() else {
                    return Ok(());
                };
                if let Err(error) = self.gpu.draw(&p) {
                    self.game.presented(false)?;
                    return Err(error);
                }
                self.game.presented(true)?;
                self.frames += 1;
                self.game.bank.recycle_frame(p);
                self.effects()?;
            }
            Err("picture drain budget".into())
        }
        fn service(&mut self) -> Result<()> {
            for event in self.transport.poll() {
                match event {
                    Event::Opened(g) => self.game.opened(g, self.now())?,
                    Event::Binary(g, b) => {
                        let id = self.transport.message_id();
                        self.game.receive_message(g, &b, self.now(), id)?;
                    }
                    Event::Closed(g) => self.game.closed(g),
                    Event::Failed(_, e) => return Err(e),
                }
                self.effects()?;
            }
            self.game.observe_time(self.now())?;
            self.effects()?;
            self.game.service_ingress()?;
            self.effects()
        }
        fn input(&mut self) -> Result<()> {
            let state = self.game.client().received();
            let Some(own) = state.own_snake_id.and_then(|id| state.snakes.get(&id)) else {
                return Ok(());
            };
            let head = own.head;
            let pit = state.setup.as_ref().ok_or("missing setup")?.radius * PIT_RATIO;
            self.middle = head.x.hypot(head.y) < pit * 0.7;
            let own_identity = self
                .game
                .client()
                .snake_objects()
                .find(|(_, own, _, s)| *own && s.metrics().alive)
                .map(|(t, _, _, s)| (s.metrics().id, t.serial()));
            let candidates: Vec<_> = self
                .game
                .client()
                .snake_objects()
                .filter_map(|(t, own, drawn, s)| {
                    let m = s.metrics();
                    let p = state.snakes.get(&m.id)?;
                    let distance = (p.head.x - head.x).hypot(p.head.y - head.y);
                    (!own && drawn && m.alive && distance <= 1200.).then_some((
                        m.id,
                        t.serial(),
                        distance,
                    ))
                })
                .collect();
            if !candidates
                .iter()
                .any(|(id, t, _)| self.reference == Some((*id, *t)))
            {
                self.reference = candidates
                    .iter()
                    .min_by(|a, b| a.2.total_cmp(&b.2).then_with(|| a.0.cmp(&b.0)))
                    .map(|(id, t, _)| (*id, *t));
            }
            latency_rig::select(own_identity, self.reference, self.middle);
            let angle = orbit_heading(head, pit * 0.50);
            {
                let _callback = latency::callback();
                self.game.pointer(
                    480. + 220. * angle.cos(),
                    270. + 220. * angle.sin(),
                    960.,
                    540.,
                    false,
                )?;
            }
            {
                let _callback = latency::callback();
                self.game.key(
                    "Space",
                    self.middle && (4. ..6.).contains(&(self.started.elapsed().as_secs_f64() % 8.)),
                );
            }
            if !self.zoomed && self.middle {
                self.game.wheel_zoom(self.zoom);
                self.zoomed = true;
            }
            Ok(())
        }
        fn redraw(&mut self) -> Result<()> {
            self.service()?;
            if self.game.phase() == Phase::Dead {
                self.deaths += 1;
                self.game.stop();
                self.reference = None;
                latency_rig::select(None, None, false);
            }
            self.effects()?;
            if self.game.phase() == Phase::Playing {
                self.input()?;
            }
            self.game.begin_redraw(self.now(), EXTENT, 1.)?;
            self.effects()?;
            self.pictures()?;
            if let Some(error) = &self.game.failure {
                return Err(error.clone());
            }
            Ok(())
        }
        fn run(&mut self, options: &Options, wakes: &mpsc::Receiver<()>) -> Result<()> {
            let mut next = Instant::now();
            while self.started.elapsed().as_secs_f64() < options.seconds {
                if self.game.can_play() {
                    slither_wgpu_slice::endpoint_login::play(
                        &mut self.game,
                        &options.endpoint,
                        "Latency rig",
                        0,
                    )?;
                }
                self.service()?;
                if Instant::now() >= next {
                    self.redraw()?;
                    next += FRAME;
                    if next < Instant::now() {
                        next = Instant::now() + FRAME;
                    }
                }
                let _ = wakes.recv_timeout(next.saturating_duration_since(Instant::now()));
            }
            Ok(())
        }
    }
    pub fn main() -> Result<()> {
        let options = Options::parse(&std::env::args().skip(1).collect::<Vec<_>>())?;
        std::fs::create_dir(&options.output).map_err(|e| e.to_string())?;
        let trace =
            latency::Run::from_env().ok_or("SLITHER_LATENCY_TRACE and revision are required")?;
        latency_rig::start();
        let started = Instant::now();
        let (tx, rx) = mpsc::sync_channel(1);
        let mut host = Host {
            game: LocalGame::new(0.)?,
            transport: Transport::with_wake(move || {
                let _ = tx.try_send(());
            }),
            gpu: Offscreen::new()?,
            started,
            reference: None,
            zoomed: false,
            zoom: options.zoom,
            middle: false,
            frames: 0,
            deaths: 0,
            sends: Vec::with_capacity(65_536),
        };
        let result = host.run(&options, &rx);
        let drain = host.pictures();
        host.game.stop();
        let cleanup = drain
            .and_then(|()| host.effects())
            .and_then(|()| host.pictures());
        host.transport.close();
        let complete = result.is_ok() && cleanup.is_ok();
        let mut controls = std::io::BufWriter::new(
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(options.output.join("controls.jsonl"))
                .map_err(|e| e.to_string())?,
        );
        for (ns, g, id, bytes) in &host.sends {
            writeln!(
                controls,
                "{{\"ns\":\"{ns}\",\"generation\":{g},\"effect\":\"{id}\",\"bytes\":{bytes:?}}}"
            )
            .map_err(|e| e.to_string())?;
        }
        controls.flush().map_err(|e| e.to_string())?;
        println!(
            "{{\"complete\":{complete},\"frames\":{},\"deaths\":{},\"seconds\":{},\"middleAtEnd\":{}}}",
            host.frames,
            host.deaths,
            started.elapsed().as_secs_f64(),
            host.middle
        );
        drop(host);
        latency_rig::finish(&options.output.join("heads.jsonl"), complete)?;
        trace.finish(complete);
        result.and(cleanup)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::f64::consts::PI;
        #[test]
        fn orbit_points_inward_outside_and_never_commands_exit_at_target() {
            let h = orbit_heading(Point { x: 3000., y: 0. }, 1500.);
            assert!(h.cos() < -0.9);
            assert!((orbit_heading(Point { x: 1500., y: 0. }, 1500.) - PI / 2.).abs() < 1e-12);
            assert!(orbit_heading(Point { x: 1000., y: 0. }, 1500.).cos() > 0.);
        }
        #[test]
        fn measurement_cli_rejects_external_reserved_and_unbounded_runs() {
            for (server, seconds, zoom) in [
                ("192.0.2.1:19090", "60", "0"),
                ("127.0.0.1:8766", "60", "0"),
                ("127.0.0.1:19090", "NaN", "0"),
                ("127.0.0.1:19090", "151", "0"),
                ("127.0.0.1:19090", "60", "-99"),
            ] {
                let args = [
                    "--server",
                    server,
                    "--output",
                    "unused",
                    "--seconds",
                    seconds,
                    "--zoom",
                    zoom,
                ]
                .map(String::from);
                assert!(Options::parse(&args).is_err());
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    if let Err(error) = rig::main() {
        eprintln!("Latency rig failed: {error}");
        std::process::exit(1);
    }
}
