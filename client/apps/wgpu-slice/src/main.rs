#![forbid(unsafe_code)]
#[cfg(target_arch = "wasm32")]
fn main() {}
#[cfg(not(target_arch = "wasm32"))]
mod headless;
#[cfg(not(target_arch = "wasm32"))]
mod scheduling;
#[cfg(not(target_arch = "wasm32"))]
mod desktop {
    use crate::scheduling::Schedule;
    use slither_client::session_policy::Phase;
    use slither_wgpu_slice::{
        Result,
        game::LocalGame,
        gpu::{RenderError, SurfaceHost, instance},
        login_native::NativeLogin,
        menu_input_native::WindowEditHost,
        native::{Event, Transport},
        official::{self, Preparation},
    };
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };
    use winit::{
        application::ApplicationHandler,
        event::{ElementState, MouseButton, WindowEvent},
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy},
        keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
        window::{Window, WindowId},
    };
    #[derive(Debug)]
    pub struct TransportWake;
    #[derive(Default)]
    pub struct Options {
        endpoint: Option<String>,
        nickname: String,
        skin: u8,
        official: bool,
        server: Option<String>,
        headless: Option<crate::headless::Options>,
        cosmetics_dir: Option<std::path::PathBuf>,
        wordmark: bool,
        accessory: Option<u8>,
    }
    pub fn options() -> Result<Option<Options>> {
        parse_options(std::env::args().skip(1))
    }
    fn parse_options(mut args: impl Iterator<Item = String>) -> Result<Option<Options>> {
        let mut o = Options {
            nickname: "Player".into(),
            ..Default::default()
        };
        let mut headless = false;
        let mut duration = 30.0;
        let mut frame_every = 5.0;
        let mut frames = None;
        let mut headless_arguments = false;
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" => {
                    println!(
                        "Offline by default.\nslither-wgpu-desktop [--official [--server host:port] | --endpoint ws://host:port/slither] [--nickname NAME] [--skin 0..65] [--cosmetics-dir DIR] [--wordmark] [--accessory 0..31]\n[--headless --duration SECONDS --frames NEW_EXTERNAL_DIRECTORY --frame-every SECONDS]\nHeadless (--endpoint only; official servers are interactive only): automatic Play, deterministic circles/boost, offscreen PNGs and exit summary. Defaults: duration 30s, frame interval 5s.\nWindow: Enter Play, Escape Stop/cancel. Pointer/arrows/Space: existing Rust input.\nNo automatic reconnect. --endpoint uses a fixed official-format login for custom-server compatibility, without fetching. Diagnostic: slither-wgpu-diagnostic."
                    );
                    return Ok(None);
                }
                "--endpoint" => {
                    let e = args.next().ok_or("Endpoint value required")?;
                    slither_wgpu_slice::game::server_endpoint(&e)?;
                    o.endpoint = Some(e);
                }
                "--wordmark" => o.wordmark = true,
                "--official" => o.official = true,
                "--headless" => headless = true,
                "--duration" | "--frame-every" => {
                    let value = args
                        .next()
                        .ok_or("Time value required")?
                        .parse::<f64>()
                        .map_err(|_| "Invalid time value")?;
                    if arg == "--duration" {
                        duration = value;
                    } else {
                        frame_every = value;
                    }
                    headless_arguments = true;
                }
                "--frames" => {
                    frames = Some(args.next().ok_or("Frame directory required")?.into());
                    headless_arguments = true;
                }
                "--server" => {
                    let server = args.next().ok_or("Server value required")?;
                    official::explicit_endpoint(&server)?;
                    o.server = Some(server);
                }
                "--nickname" => o.nickname = args.next().ok_or("Nickname required")?,
                "--cosmetics-dir" => {
                    o.cosmetics_dir =
                        Some(args.next().ok_or("Cosmetics directory required")?.into())
                }
                "--accessory" => {
                    let id = args
                        .next()
                        .ok_or("Accessory required")?
                        .parse::<u8>()
                        .map_err(|_| "Invalid accessory")?;
                    if id >= 32 {
                        return Err("Accessory must be 0..31".into());
                    }
                    o.accessory = Some(id);
                }
                "--skin" => {
                    o.skin = args
                        .next()
                        .ok_or("Skin required")?
                        .parse()
                        .map_err(|_| "Invalid skin")?
                }
                _ => return Err(format!("Unknown option {arg}")),
            }
        }
        if o.official && o.endpoint.is_some() {
            return Err("Use --official or --endpoint".into());
        }
        if o.server.is_some() && !o.official {
            return Err("--server requires --official".into());
        }
        if headless {
            // Owner decision 2026-10-30: official logins only from a person pressing Play.
            if o.official || o.endpoint.is_none() {
                return Err("Headless requires --endpoint; --official is interactive only".into());
            }
            o.headless = Some(crate::headless::Options::new(
                duration,
                frame_every,
                frames.ok_or("Headless requires --frames NEW_EXTERNAL_DIRECTORY")?,
            )?);
        } else if headless_arguments {
            return Err("--duration, --frames and --frame-every require --headless".into());
        }
        let mut menu = slither_client::menu_preferences::MenuPreferences::new(true);
        menu.select(o.skin, &[])?;
        slither_client::protocol::encode_login(0, &[0; 20], &o.nickname, o.skin, None)
            .map_err(|e| e.to_string())?;
        Ok(Some(o))
    }
    struct Running {
        login: NativeLogin,
        endpoint: Option<String>,
        official: bool,
        server: Option<String>,
        window: Arc<Window>,
        host: SurfaceHost,
        game: LocalGame,
        transport: Transport,
        started: Instant,
        cursor: [f64; 2],
        shift: bool,
        preparation: Option<Preparation>,
        nickname: String,
        skin: u8,
        schedule: Schedule,
        occluded: bool,
        surface_occluded: bool,
        title: String,
        discovery_poll: Option<Instant>,
        handshake_wake: Option<Instant>,
        pacing_log: Option<(Instant, u32, Instant, f64)>,
    }
    impl Running {
        fn edit_effects(&mut self, effects: Vec<slither_client::menu_input::HostEditEffect>) {
            use slither_client::menu_input::HostEditEffect;
            let effects = if self.login.champion.policy.visible() {
                let effects = self
                    .login
                    .champion
                    .input
                    .execute(effects, &mut WindowEditHost(&self.window));
                self.login.champion.commit_effects(effects)
            } else {
                let effects = self
                    .login
                    .input
                    .execute(effects, &mut WindowEditHost(&self.window));
                let admission = self.game.can_play()
                    && self.preparation.is_none()
                    && (self.official || self.endpoint.is_some());
                self.login
                    .input
                    .state
                    .commit_menu(effects, &mut self.login.state, admission)
            };
            for effect in effects {
                match effect {
                    HostEditEffect::Gameplay { key, pressed } => {
                        if !pressed || !self.login.blocks_gameplay() {
                            self.game.key(key.name(), pressed);
                        }
                    }
                    HostEditEffect::ReleasePointerBoost => self.game.mouse_up(),
                    HostEditEffect::ClipboardFailed(error) => eprintln!("{error}"),
                    _ => {}
                }
            }
        }
        fn champion_submission(&mut self) -> Result<()> {
            if let Some((generation, units, activation)) = self.login.champion.take_submission() {
                self.game.save_champion(generation, &units, activation)?;
                self.login.observe_game(&self.game)?;
                self.effects()?;
            }
            let effects = std::mem::take(&mut self.login.champion.effects);
            self.edit_effects(effects);
            Ok(())
        }
        fn effects(&mut self) -> Result<()> {
            for _ in 0..64 {
                let Some(e) = self.game.effect() else {
                    return Ok(());
                };
                if matches!(&e.kind, slither_wgpu_slice::game::EffectKind::Connect(_)) {
                    // SessionPolicy's strict >3333ms construction deadline must
                    // still be observed while the window has stopped drawing.
                    let constructed = Instant::now();
                    self.game.construct(
                        e.generation,
                        e.id,
                        constructed.duration_since(self.started).as_secs_f64() * 1000.,
                    )?;
                    self.handshake_wake = Some(constructed + Duration::from_millis(3334));
                }
                match self.transport.execute(&e) {
                    Ok(()) => self.game.acknowledge(e.generation, e.id, true)?,
                    Err(error) => {
                        self.transport.close();
                        self.game.fail(&error);
                    }
                }
            }
            Err("Effect drain budget".into())
        }
        fn service(&mut self) -> Result<()> {
            if let Some(preparation) = &mut self.preparation
                && let Some(result) = preparation.poll()
            {
                let cancelled = preparation.cancelled();
                self.preparation.take();
                if cancelled {
                    self.game.stop();
                } else {
                    match result {
                        Ok(prepared) => {
                            println!("Selected {}; starting explicit Play", prepared.endpoint);
                            if let Err(error) = self.game.play_with_profile(
                                &prepared.endpoint,
                                &self.nickname,
                                self.skin,
                                &prepared.profile,
                            ) {
                                self.game.fail(&error);
                            }
                        }
                        Err(e) => {
                            eprintln!("Official discovery failed: {e}");
                            self.game.fail(&e);
                        }
                    }
                }
            }
            self.discovery_poll = if self.preparation.is_some() {
                Some(
                    self.discovery_poll
                        .filter(|t| *t > Instant::now())
                        .unwrap_or_else(|| Instant::now() + Duration::from_millis(50)),
                )
            } else {
                None
            };
            for event in self.transport.poll() {
                match event {
                    Event::Opened(g) => {
                        if let Err(e) = self
                            .game
                            .opened(g, self.started.elapsed().as_secs_f64() * 1000.)
                        {
                            self.game.fail(&e)
                        }
                    }
                    Event::Binary(g, b) => {
                        if let Err(e) =
                            self.game
                                .receive(g, &b, self.started.elapsed().as_secs_f64() * 1000.)
                        {
                            self.game.fail(&e)
                        }
                    }
                    Event::Closed(g) => self.game.closed(g),
                    Event::Failed(g, e) => {
                        if g == self.game.generation() {
                            self.game.fail(&e);
                        }
                    }
                }
                self.effects()?;
            }
            self.game
                .observe_time(self.started.elapsed().as_secs_f64() * 1000.)?;
            self.effects()?;
            self.game.service_ingress()?;
            self.effects()?;
            // Do not return while a cancelled acquisition still owns its worker.
            if self.preparation.is_none() {
                self.login.observe_game(&self.game)?;
            }
            self.champion_submission()?;
            let active = !matches!(
                self.game.phase(),
                Phase::Ready | Phase::Failed | Phase::Disconnected
            );
            self.schedule.activity(active || self.login.needs_redraw());
            if !matches!(
                self.game.phase(),
                Phase::Opening | Phase::Challenge | Phase::Setup
            ) {
                self.handshake_wake = None;
            }
            let title = format!(
                "SlitherClient | {}{} | MSAA{}",
                if self.preparation.is_some() {
                    "Acquiring official profile/server | "
                } else {
                    ""
                },
                self.game.hud_text().replace('\n', " | "),
                self.host.renderer.sample_count
            );
            if self.title != title {
                self.window.set_title(&title);
                self.title = title;
            }
            Ok(())
        }
        fn visibility(&mut self) {
            let s = self.window.inner_size();
            self.schedule.visibility(
                !self.occluded
                    && !self.surface_occluded
                    && s.width != 0
                    && s.height != 0
                    && self.window.is_minimized() != Some(true),
            );
        }
        fn frame(&mut self) -> Result<()> {
            let size = self.window.inner_size();
            let physical = [size.width, size.height];
            let dpr = self.window.scale_factor();
            self.host.resize(physical)?;
            let extent = self.game.window_viewport(physical, dpr)?;
            if !self.host.prepare_present()? {
                if self.host.is_occluded() {
                    self.surface_occluded = true;
                    self.visibility();
                } else {
                    self.schedule.retry(Instant::now());
                }
                return Ok(());
            }
            // FIFO may have waited. Ingest everything now available before
            // sampling the next frame; receive() retains its own deferral rules.
            self.service()?;
            let admission = self.game.can_play()
                && self.preparation.is_none()
                && (self.official || self.endpoint.is_some());
            let (intents, effects) = self.login.step(
                self.started.elapsed().as_secs_f64() * 1000.,
                admission,
                self.game.phase() == Phase::Playing,
            )?;
            self.edit_effects(effects);
            self.champion_submission()?;
            for intent in intents {
                if let slither_client::start_page::StartMenuIntent::Play { nickname, quality } =
                    intent
                {
                    self.nickname = nickname;
                    self.game.set_requested_quality(match quality {
                        slither_client::menu_preferences::MenuQuality::High => {
                            slither_client::presentation::QualityMode::High
                        }
                        slither_client::menu_preferences::MenuQuality::Low => {
                            slither_client::presentation::QualityMode::Low
                        }
                    });
                    if self.official {
                        match Preparation::start(self.server.clone()) {
                            Ok(preparation) => self.preparation = Some(preparation),
                            Err(error) => self.game.fail(&error),
                        }
                    } else if let Some(endpoint) = &self.endpoint
                        && let Err(error) = slither_wgpu_slice::endpoint_login::play(
                            &mut self.game,
                            endpoint,
                            &self.nickname,
                            self.skin,
                        )
                    {
                        self.game.fail(&error);
                    }
                }
            }
            self.effects()?;
            self.game
                .begin_redraw(self.started.elapsed().as_secs_f64() * 1000., extent, 1.)?;
            self.effects()?;
            // A fresh official acquisition still refers to the previous session.
            if self.preparation.is_none() {
                self.login.observe_game(&self.game)?;
            }
            let Some(p) = self.game.take_frame() else {
                self.host.discard_acquired();
                self.schedule.retry(Instant::now());
                return Ok(());
            };
            let view = self.login.state.view(admission);
            let composed = (|| -> Result<_> {
                Ok(if view.visible || view.spinner_visible {
                    let picture = slither_wgpu_slice::menu::MenuFrame::compose_optional(
                        self.login.prepare_host(
                            physical,
                            dpr,
                            admission,
                            self.game.menu_cosmetics(),
                        ),
                        &p,
                    )?;
                    let effects = self.login.cursor_area()?;
                    self.edit_effects(effects);
                    picture
                } else {
                    None
                })
            })();
            let composed = match composed {
                Ok(picture) => picture,
                Err(error) => return self.skip_bad_frame(p.frame.identity, error),
            };
            let picture = composed.as_ref().unwrap_or(&p);
            if let Err(e) = self.host.renderer.sync_resources(&picture.resources) {
                return match e {
                    RenderError::Frame(e) => self.skip_bad_frame(p.frame.identity, e),
                    RenderError::Host(e) => {
                        self.game.presented(false)?;
                        Err(e)
                    }
                };
            }
            self.window.pre_present_notify();
            match self
                .host
                .present_scaled_with_overlay(&picture.frame, picture.overlay.as_ref())
            {
                Ok(true) => {
                    self.game.presented(true)?;
                    self.game.bank.recycle_frame(p);
                    if let Some(picture) = composed {
                        self.game.bank.recycle_frame(picture);
                    }
                    let effects = self.login.sync_return_focus(self.game.can_play());
                    self.edit_effects(effects);
                    self.schedule.presented();
                    self.log_present();
                }
                Ok(false) => {
                    self.game.retry_frame(p)?;
                    self.schedule.retry(Instant::now());
                }
                Err(RenderError::Frame(e)) => {
                    return self.skip_bad_frame(p.frame.identity, e);
                }
                Err(RenderError::Host(e)) => {
                    self.game.presented(false)?;
                    return Err(e);
                }
            }
            self.effects()?;
            Ok(())
        }
        fn skip_bad_frame(&mut self, identity: u64, error: String) -> Result<()> {
            self.host.discard_acquired();
            self.game
                .draw_diagnostics
                .admit::<()>(identity, Err(error))?;
            self.game.skip_frame()?;
            self.schedule.retry(Instant::now());
            self.effects()
        }
        fn log_present(&mut self) {
            if let Some((since, count, last, max_gap)) = &mut self.pacing_log {
                let now = Instant::now();
                if *count == 0 {
                    *since = now;
                    *last = now;
                }
                *max_gap = max_gap.max(now.duration_since(*last).as_secs_f64() * 1000.);
                *last = now;
                *count += 1;
                let seconds = now.duration_since(*since).as_secs_f64();
                if seconds >= 2. {
                    eprintln!(
                        "native pacing: {:.1} presents/s, max gap {:.2}ms; monitor {:.1}Hz (submission, not scanout)",
                        f64::from(*count) / seconds,
                        *max_gap,
                        f64::from(
                            self.window
                                .current_monitor()
                                .and_then(|m| m.refresh_rate_millihertz())
                                .unwrap_or(0)
                        ) / 1000.
                    );
                    *since = now;
                    *count = 0;
                    *max_gap = 0.;
                }
            }
        }
        fn pointer(&mut self, down: bool) -> Result<()> {
            let s = self.window.inner_size();
            let d = self.window.scale_factor();
            if s.width == 0 || s.height == 0 {
                return Ok(());
            }
            self.game.pointer(
                self.cursor[0],
                self.cursor[1],
                f64::from(s.width) / d,
                f64::from(s.height) / d,
                down,
            )
        }
    }
    pub struct App {
        options: Options,
        running: Option<Running>,
        failure: Option<String>,
        proxy: EventLoopProxy<TransportWake>,
    }
    impl App {
        fn fail(&mut self, l: &ActiveEventLoop, e: String) {
            eprintln!("GPU host stopped: {e}");
            self.failure = Some(e);
            if let Some(r) = &mut self.running {
                r.game.stop();
                r.transport.close();
            }
            l.exit();
        }
        fn open(
            l: &ActiveEventLoop,
            options: &Options,
            proxy: EventLoopProxy<TransportWake>,
        ) -> Result<Running> {
            let mut game = LocalGame::new(0.)?;
            if let Some(directory) = &options.cosmetics_dir {
                game.configure_cosmetics(slither_wgpu_slice::cosmetics::Cosmetics::directory(
                    directory,
                    options.accessory,
                )?)?;
            }
            let window = Arc::new(
                l.create_window(
                    Window::default_attributes()
                        .with_title("SlitherClient — Enter Play / Escape Stop")
                        .with_inner_size(winit::dpi::LogicalSize::new(960., 540.)),
                )
                .map_err(|e| e.to_string())?,
            );
            let s = window.inner_size();
            let i = instance();
            let surface = i
                .create_surface(window.clone())
                .map_err(|e| e.to_string())?;
            let host = pollster::block_on(SurfaceHost::new(&i, surface, [s.width, s.height]))?;
            println!(
                "Actual adapter {:?}; MSAA{}; no transport until explicit Play",
                host.renderer.adapter_info, host.renderer.sample_count
            );
            let mut login = NativeLogin::new(&options.nickname)?;
            login.set_wordmark(options.wordmark);
            Ok(Running {
                login,
                endpoint: options.endpoint.clone(),
                official: options.official,
                server: options.server.clone(),
                window,
                host,
                game,
                transport: Transport::with_wake(move || {
                    let _ = proxy.send_event(TransportWake);
                }),
                started: Instant::now(),
                cursor: [480., 270.],
                shift: false,
                preparation: None,
                nickname: String::new(),
                skin: options.skin,
                schedule: Schedule::default(),
                occluded: false,
                surface_occluded: false,
                title: String::new(),
                discovery_poll: None,
                handshake_wake: None,
                pacing_log: std::env::var_os("SLITHER_FRAME_PACING")
                    .map(|_| (Instant::now(), 0, Instant::now(), 0.)),
            })
        }
    }
    impl ApplicationHandler<TransportWake> for App {
        fn resumed(&mut self, l: &ActiveEventLoop) {
            if self.running.is_none() {
                match Self::open(l, &self.options, self.proxy.clone()) {
                    Ok(r) => self.running = Some(r),
                    Err(e) => self.fail(l, e),
                }
            }
        }
        fn user_event(&mut self, l: &ActiveEventLoop, _: TransportWake) {
            if let Some(r) = &mut self.running
                && let Err(e) = r.service()
            {
                self.fail(l, e);
            }
        }
        fn window_event(&mut self, l: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
            let Some(r) = &mut self.running else { return };
            if r.window.id() != id {
                return;
            }
            let result = (|| -> Result<()> {
                if let WindowEvent::ModifiersChanged(modifiers) = &event {
                    r.shift = modifiers.state().shift_key();
                    r.login.input.modifiers(modifiers.state());
                    r.login.champion.input.modifiers(modifiers.state());
                }
                let admission = r.game.can_play()
                    && r.preparation.is_none()
                    && (r.official || r.endpoint.is_some());
                if let WindowEvent::KeyboardInput { event, .. } = &event
                    && event.state == ElementState::Pressed
                    && !r.login.champion.policy.visible()
                    && !event.repeat
                    && event.logical_key == Key::Named(NamedKey::Enter)
                    && r.login.state.view(admission).phase
                        == slither_client::start_page::MenuPhase::Returning
                    && !r.login.state.view(admission).play_disabled
                {
                    r.login.activate(admission);
                    r.schedule.invalidate();
                    return Ok(());
                }
                if let WindowEvent::KeyboardInput { event, .. } = &event
                    && event.state == ElementState::Pressed
                    && !r.login.champion.policy.visible()
                    && !r.login.state.view(admission).disabled
                {
                    if event.logical_key == Key::Named(NamedKey::Tab) {
                        let next = (r.login.focus + if r.shift { 2 } else { 1 }) % 3;
                        let effects = r.login.focus(next);
                        r.edit_effects(effects);
                        r.schedule.invalidate();
                        return Ok(());
                    }
                    if r.login.focus != 0
                        && !event.repeat
                        && matches!(
                            event.logical_key,
                            Key::Named(NamedKey::Enter | NamedKey::Space)
                        )
                    {
                        r.login.activate(admission);
                        r.schedule.invalidate();
                        return Ok(());
                    }
                }
                let champion = r.login.champion.policy.visible();
                if let WindowEvent::Focused(focused) = &event {
                    if !focused {
                        r.shift = false;
                    }
                    let inactive = if champion {
                        &mut r.login.input
                    } else {
                        &mut r.login.champion.input
                    };
                    // Retain window ownership across a committed panel handoff.
                    inactive.window_event(&event);
                }
                let mapped = if champion {
                    r.login.champion.event(&event)
                } else {
                    r.login.input.window_event(&event)
                };
                r.edit_effects(mapped.effects);
                r.champion_submission()?;
                if mapped.consumed {
                    r.schedule.invalidate();
                    return Ok(());
                }
                match event {
                    WindowEvent::CloseRequested => {
                        if let Some(p) = &r.preparation {
                            p.cancel();
                        }
                        r.game.stop();
                        r.transport.close();
                        l.exit();
                    }
                    WindowEvent::RedrawRequested => {
                        // An OS exposure can resume a surface that reported
                        // occlusion even where winit has no Occluded event.
                        if r.surface_occluded {
                            r.surface_occluded = false;
                            r.schedule.invalidate();
                        }
                        r.visibility();
                        if r.schedule.begin_redraw(Instant::now()) {
                            r.frame()?;
                        }
                    }
                    WindowEvent::Occluded(occluded) => {
                        r.occluded = occluded;
                        if !occluded {
                            r.surface_occluded = false;
                        }
                        r.visibility();
                    }
                    WindowEvent::Resized(s) => {
                        r.surface_occluded = false;
                        r.schedule.invalidate();
                        r.visibility();
                        r.host.resize([s.width, s.height])?;
                        r.game.viewport(
                            slither_wgpu_slice::render::backing_size([
                                f64::from(s.width) / r.window.scale_factor(),
                                f64::from(s.height) / r.window.scale_factor(),
                            ])?,
                            1.,
                        )?;
                    }
                    WindowEvent::ScaleFactorChanged { .. } => r.schedule.invalidate(),
                    WindowEvent::CursorMoved { position, .. } => {
                        let d = r.window.scale_factor();
                        r.cursor = slither_client::desktop_input::mouse_client_position(
                            [position.x, position.y],
                            d,
                        );
                        if r.login.intercepts_pointer(r.cursor) {
                            let effects = r.login.pointer(r.cursor, None, admission);
                            r.edit_effects(effects);
                        } else {
                            r.pointer(false)?;
                        }
                    }
                    WindowEvent::MouseInput {
                        state,
                        button: MouseButton::Left,
                        ..
                    } => {
                        if r.login.intercepts_pointer(r.cursor) {
                            let effects = r.login.pointer(
                                r.cursor,
                                Some(state == ElementState::Pressed),
                                admission,
                            );
                            r.edit_effects(effects);
                            r.game.mouse_up();
                            r.champion_submission()?;
                        } else if state == ElementState::Pressed {
                            r.pointer(true)?;
                        } else {
                            r.game.mouse_up();
                        }
                    }
                    WindowEvent::Focused(false) => r.game.blur(),
                    WindowEvent::Focused(true) => {
                        r.surface_occluded = false;
                        r.visibility();
                        r.schedule.invalidate();
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        let down = event.state == ElementState::Pressed;
                        match event.physical_key {
                            PhysicalKey::Code(KeyCode::Enter) if down && !event.repeat => {}
                            PhysicalKey::Code(KeyCode::Escape) if down => {
                                r.login.state.cancel_pending_play();
                                if let Some(p) = &r.preparation {
                                    p.cancel();
                                }
                                r.game.stop();
                                r.effects()?;
                                r.schedule.invalidate();
                            }
                            PhysicalKey::Code(code) => {
                                let name = match code {
                                    KeyCode::ArrowLeft => "ArrowLeft",
                                    KeyCode::ArrowRight => "ArrowRight",
                                    KeyCode::ArrowUp => "ArrowUp",
                                    KeyCode::Space => "Space",
                                    _ => "",
                                };
                                r.game.key(name, down);
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
                Ok(())
            })();
            if let Err(e) = result {
                self.fail(l, e);
            }
        }
        fn about_to_wait(&mut self, l: &ActiveEventLoop) {
            l.set_control_flow(ControlFlow::Wait);
            if let Some(r) = &mut self.running {
                if let Err(e) = r.service() {
                    self.fail(l, e);
                    return;
                }
                r.visibility();
                if r.schedule.request_redraw(Instant::now()) {
                    r.window.request_redraw();
                }
                if let Some(deadline) = [r.schedule.deadline(), r.discovery_poll, r.handshake_wake]
                    .into_iter()
                    .flatten()
                    .min()
                {
                    l.set_control_flow(ControlFlow::WaitUntil(deadline));
                }
            }
        }
    }
    pub fn run(mut options: Options) -> Result<()> {
        if let Some(headless) = options.headless.take() {
            let cosmetics = options
                .cosmetics_dir
                .as_deref()
                .map(|directory| {
                    slither_wgpu_slice::cosmetics::Cosmetics::directory(
                        directory,
                        options.accessory,
                    )
                })
                .transpose()?;
            return crate::headless::run(
                headless,
                options
                    .endpoint
                    .as_deref()
                    .ok_or("Headless requires --endpoint")?,
                &options.nickname,
                options.skin,
                cosmetics,
            );
        }
        if !options.official && options.endpoint.is_none() {
            eprintln!(
                "No connection mode configured; Play stays disabled. Use --official or --endpoint."
            );
        }
        let event_loop = EventLoop::<TransportWake>::with_user_event()
            .build()
            .map_err(|e| e.to_string())?;
        let mut app = App {
            options,
            running: None,
            failure: None,
            proxy: event_loop.create_proxy(),
        };
        event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
        app.running.take();
        app.failure.map_or(Ok(()), Err)
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        fn parse(args: &[&str]) -> Result<Option<Options>> {
            parse_options(args.iter().map(|s| s.to_string()))
        }
        #[test]
        fn cli_modes_are_explicit_and_validate_before_window_or_network() {
            let official = parse(&[
                "--official",
                "--server",
                "game.example:445",
                "--nickname",
                "Player",
                "--skin",
                "65",
            ])
            .unwrap()
            .unwrap();
            assert!(official.official);
            assert_eq!(official.server.as_deref(), Some("game.example:445"));
            assert_eq!(official.skin, 65);
            assert!(parse(&["--endpoint", "ws://localhost:19491/slither"]).is_ok());
            assert!(parse(&["--help"]).unwrap().is_none());
            for args in [
                vec!["--official", "--endpoint", "ws://127.0.0.1:19491/slither"],
                vec!["--server", "x:445"],
                vec!["--official", "--skin", "66"],
                vec!["--official", "--server"],
                vec![
                    "--official",
                    "--nickname",
                    "this nickname is too long to encode",
                ],
            ] {
                assert!(parse(&args).is_err());
            }
        }
        #[test]
        fn wordmark_overrides_local_logo_only_when_requested() {
            assert!(!parse(&[]).unwrap().unwrap().wordmark);
            let options = parse(&["--wordmark", "--cosmetics-dir", "synthetic-artwork"])
                .unwrap()
                .unwrap();
            assert!(options.wordmark);
            assert!(options.cosmetics_dir.is_some());
        }
        #[test]
        fn headless_requires_an_endpoint_and_bounded_explicit_output() {
            let endpoint = ["--endpoint", "ws://127.0.0.1:19491/slither"];
            let options = parse(
                &[
                    &endpoint[..],
                    &[
                        "--headless",
                        "--duration",
                        "10",
                        "--frames",
                        "external-frames",
                        "--frame-every",
                        "2",
                        "--cosmetics-dir",
                        "synthetic-artwork",
                        "--accessory",
                        "30",
                    ],
                ]
                .concat(),
            )
            .unwrap()
            .unwrap();
            assert!(options.headless.is_some());
            assert_eq!(options.accessory, Some(30));
            assert_eq!(
                options.cosmetics_dir.as_deref(),
                Some(std::path::Path::new("synthetic-artwork"))
            );
            for args in [
                vec!["--headless"],
                vec!["--headless", "--frames", "unused"],
                [&endpoint[..], &["--headless"]].concat(),
                [&endpoint[..], &["--frames", "unused"]].concat(),
                [
                    &endpoint[..],
                    &["--headless", "--frames", "unused", "--duration", "NaN"],
                ]
                .concat(),
                [
                    &endpoint[..],
                    &["--headless", "--frames", "unused", "--frame-every", "inf"],
                ]
                .concat(),
            ] {
                assert!(parse(&args).is_err());
            }
        }
        #[test]
        fn headless_never_uses_the_official_mode() {
            // Otherwise complete headless runs; only the official mode differs.
            for mode in [
                vec!["--official"],
                vec!["--official", "--server", "game.example:445"],
            ] {
                let args = [mode, vec!["--headless", "--frames", "unused"]].concat();
                let error = parse(&args).err().expect("official headless run");
                assert!(error.contains("--official is interactive only"), "{error}");
            }
        }
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let result = desktop::options().and_then(|o| o.map_or(Ok(()), desktop::run));
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
