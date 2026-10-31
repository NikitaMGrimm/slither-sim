//! Shared actual local session. Host effects are acknowledged outside every Rust borrow.
use crate::{
    Result,
    bank::Bank,
    render::{self, PreparedFrame},
};
use slither_client::{
    challenge::solve_challenge,
    desktop_input::{DesktopInput, OwnInput},
    gameplay_policy::GameplayPolicy,
    presentation::*,
    protocol::{self, ElapsedHeader},
    session_policy::{Command, Phase, ReportEvent, SessionPolicy},
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Connect(String),
    Send(Vec<u8>),
    Close,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Effect {
    pub id: u64,
    pub generation: u32,
    pub kind: EffectKind,
}
#[derive(Clone, Copy)]
enum After {
    None,
    Socket,
    Opened,
    Challenged,
    Control,
}
struct Pending {
    effect: Effect,
    after: After,
    started: bool,
}
// The prepared candidate and its observation travel together through retries and
// queueing. Native callers keep the existing PreparedFrame API.
// [Contract](../../../tools/feel-replay/README.md),
// [peers](../../../tools/feel-replay/browser-peers.md),
// [synthetic fixture](../../../tools/feel-replay/browser-synthetic.json).
struct Picture<T> {
    value: T,
    #[cfg(feature = "feel-trace")]
    observation: Option<slither_client::feel_trace::Observation>,
}
struct Work {
    now: f64,
    lagging: bool,
    step: FrameStep,
    actions: ClientControlActions,
    phase: u8,
    extent: [u32; 2],
    scale: f64,
    hud: HudHistory,
    reset_rows: bool,
    world: Option<AnimatedClient>,
    prepared: Option<Picture<PreparedFrame>>,
    quality: QualityState,
    statistics_consumed: bool,
}
enum Probe {
    Captured,
    Core(String),
}
impl From<ClientPresentationError> for Probe {
    fn from(e: ClientPresentationError) -> Self {
        Self::Core(e.to_string())
    }
}
#[derive(Debug)]
struct Ingress(String);
impl From<ClientPresentationError> for Ingress {
    fn from(e: ClientPresentationError) -> Self {
        Self(e.to_string())
    }
}
fn sample(v: ClientScoreView) -> Option<HudSample> {
    if let (ScoreStatus::Ready(score), Some(own)) = (v.status, v.own) {
        Some(HudSample {
            score,
            token: own.token,
        })
    } else {
        None
    }
}

pub fn loopback_endpoint(value: &str) -> Result<u16> {
    let port = value
        .strip_prefix("ws://127.0.0.1:")
        .and_then(|s| s.strip_suffix("/slither"))
        .ok_or("Expected ws://127.0.0.1:<port>/slither")?;
    if port.is_empty() || port.starts_with('0') || !port.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Invalid loopback port".into());
    }
    let p = port.parse::<u16>().map_err(|_| "Invalid loopback port")?;
    if p == 0 {
        return Err("Invalid loopback port".into());
    }
    Ok(p)
}

/// Shared endpoint admission; the browser's local entry applies its own loopback gate.
pub fn server_endpoint(value: &str) -> Result<url::Url> {
    if value
        .bytes()
        .any(|b| b.is_ascii_control() || b.is_ascii_whitespace() || b == b'\\')
    {
        return Err("Invalid server URL".into());
    }
    let url = url::Url::parse(value).map_err(|_| "Invalid server URL")?;
    if url.scheme() != "ws"
        || url.host_str().is_none()
        || url.port_or_known_default() == Some(0)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/slither"
    {
        return Err("Expected ws://host:port/slither without credentials or query".into());
    }
    Ok(url)
}

/// In-memory login data; never format or persist this value.
#[derive(Default)]
pub struct LoginProfile {
    pub client_version: u16,
    pub page_echo: [u8; 20],
}
impl Drop for LoginProfile {
    fn drop(&mut self) {
        self.page_echo.fill(0);
    }
}

pub struct LocalGame {
    #[cfg(feature = "feel-trace")]
    trace: Option<slither_client::feel_trace::Recorder>,
    #[cfg(feature = "feel-trace")]
    issued_observation: Option<slither_client::feel_trace::Observation>,
    #[cfg(feature = "feel-trace")]
    ingress_observations: Vec<f64>,
    client: AnimatedClient,
    pub bank: Bank,
    session: SessionPolicy,
    input: DesktopInput,
    policy: GameplayPolicy,
    hud: HudHistory,
    rows: LeaderboardHistory,
    projector: LeaderboardScoreProjector,
    nickname: String,
    login: Vec<u8>,
    endpoint: String,
    effects: VecDeque<Pending>,
    next_effect: u64,
    generation: u32,
    has_socket: bool,
    work: Option<Work>,
    packet_frames: VecDeque<Picture<PreparedFrame>>,
    inflight: u8,
    deferred: VecDeque<(u32, Vec<u8>, f64)>,
    deferred_bytes: usize,
    own: Option<SnakeObjectToken>,
    extent: [u32; 2],
    scale: f64,
    identity: u64,
    rng: u64,
    pub failure: Option<String>,
    pub draw_diagnostics: crate::frame::DrawDiagnostics,
    suspended: bool,
    quality: QualityState,
    draws_since_statistics: u32,
}
impl LocalGame {
    pub fn new(now: f64) -> Result<Self> {
        let mut client = AnimatedClient::new(now).map_err(|e| e.to_string())?;
        client.enable_score_tracking().map_err(|e| e.to_string())?;
        client
            .enable_minimap_tracking()
            .map_err(|e| e.to_string())?;
        client
            .enable_announcement_tracking()
            .map_err(|e| e.to_string())?;
        client.enable_nicknames().map_err(|e| e.to_string())?;
        let mut session = SessionPolicy::new();
        session.subscribe(2).map_err(|e| e.0)?;
        Ok(Self {
            #[cfg(feature = "feel-trace")]
            trace: None,
            #[cfg(feature = "feel-trace")]
            issued_observation: None,
            #[cfg(feature = "feel-trace")]
            ingress_observations: Vec::new(),
            client,
            bank: Bank::new()?,
            session,
            input: DesktopInput::new(19.).map_err(|e| e.to_string())?,
            policy: GameplayPolicy::new(true, true, true, true, true, false),
            hud: HudHistory::default(),
            rows: LeaderboardHistory::default(),
            projector: LeaderboardScoreProjector::new(),
            nickname: String::new(),
            login: Vec::new(),
            endpoint: String::new(),
            effects: VecDeque::new(),
            next_effect: 1,
            generation: 0,
            has_socket: false,
            work: None,
            packet_frames: VecDeque::new(),
            inflight: 0,
            deferred: VecDeque::new(),
            deferred_bytes: 0,
            own: None,
            extent: [960, 540],
            scale: 1.,
            identity: 0,
            rng: 17,
            failure: None,
            draw_diagnostics: crate::frame::DrawDiagnostics::default(),
            suspended: false,
            quality: QualityState::new(QualityMode::High, now).map_err(|e| e.to_string())?,
            draws_since_statistics: 0,
        })
    }
    pub fn generation(&self) -> u32 {
        self.generation
    }
    /// A3 may attach the same CPU recorder before the first Play.
    #[cfg(feature = "feel-trace")]
    pub fn record_feel(&mut self, recorder: slither_client::feel_trace::Recorder) {
        self.client.record_feel(recorder.clone());
        self.trace = Some(recorder);
    }
    /// Observation carried by the issued picture, including before world commit.
    #[cfg(feature = "feel-trace")]
    pub fn issued_feel_observation(&self) -> Option<&slither_client::feel_trace::Observation> {
        self.issued_observation.as_ref()
    }
    /// Original receive times of successfully applied envelopes; no packet data.
    #[cfg(feature = "feel-trace")]
    pub fn take_feel_ingress(&mut self) -> Vec<f64> {
        std::mem::take(&mut self.ingress_observations)
    }
    pub fn client(&self) -> &AnimatedClient {
        &self.client
    }
    pub fn quality(&self) -> QualitySnapshot {
        self.quality.snapshot()
    }
    /// Read-only session metadata for the offline peer.
    #[cfg(feature = "feel-trace")]
    pub fn feel_session(&self) -> slither_client::session_policy::View {
        self.session.view()
    }
    /// Outstanding model work, independent of the host's perpetual RAF request.
    #[cfg(feature = "feel-trace")]
    pub fn feel_pending(&self) -> [usize; 2] {
        [
            self.effects.len() + self.deferred.len(),
            self.packet_frames.len()
                + usize::from(self.work.is_some())
                + usize::from(self.inflight != 0),
        ]
    }
    /// Store the preference; the next explicit Play applies it, matching Canvas.
    pub fn set_requested_quality(&mut self, mode: QualityMode) {
        self.quality.set_requested_mode(mode);
        if let Some(w) = &mut self.work {
            w.quality.set_requested_mode(mode);
        }
    }
    pub fn configure_cosmetics(&mut self, cosmetics: crate::cosmetics::Cosmetics) -> Result<()> {
        if !self.can_play() || self.client.snake_objects().next().is_some() {
            return Err("Configure cosmetics before the first Play".into());
        }
        self.bank.cosmetics = Some(cosmetics);
        Ok(())
    }
    pub fn menu_cosmetics(&self) -> Option<&crate::cosmetics::Cosmetics> {
        self.bank.cosmetics.as_ref()
    }
    pub fn phase(&self) -> Phase {
        self.session.view().phase
    }
    /// Login reads committed command-time history and the existing session clock.
    pub fn login_session(&self) -> slither_client::start_page::MenuSession {
        (&self.session.view()).into()
    }
    /// Copied committed authority/status for the native victory editor.
    pub fn champion_session(&self) -> slither_client::session_policy::View {
        self.session.view()
    }
    /// One small, auditable send seam: current generation, committed victory,
    /// existing transport. Enqueue before locking; failure never claims a send.
    pub fn save_champion(
        &mut self,
        generation: u32,
        units: &[u16],
        activation: slither_client::menu_controls::Activation,
    ) -> Result<bool> {
        if generation != self.generation || !self.has_socket || !self.session.can_send_champion() {
            return Ok(false);
        }
        let bytes = protocol::encode_champion(units);
        if activation == slither_client::menu_controls::Activation::Enter && bytes.len() == 2 {
            return Ok(false);
        }
        self.enqueue(EffectKind::Send(bytes), After::None)?;
        self.session.champion_enqueued();
        Ok(true)
    }
    pub fn login_final(&self) -> Option<slither_client::presentation::HudFinal> {
        self.hud.final_capture
    }
    pub fn can_play(&self) -> bool {
        matches!(
            self.phase(),
            Phase::Ready | Phase::Disconnected | Phase::Failed
        ) && self.effects.is_empty()
            && !self.has_socket
            && self.work.is_none()
            && self.inflight == 0
    }
    pub fn play(&mut self, endpoint: &str, nickname: &str, skin: u8) -> Result<()> {
        self.play_with_profile(endpoint, nickname, skin, &LoginProfile::default())
    }
    pub fn play_with_profile(
        &mut self,
        endpoint: &str,
        nickname: &str,
        skin: u8,
        profile: &LoginProfile,
    ) -> Result<()> {
        self.play_with_appearance(
            endpoint,
            nickname,
            &slither_client::skin_studio::AppearanceSelection {
                skin,
                custom_bytes: vec![],
            },
            profile,
        )
    }
    /// Explicit Play owns its appearance before any deferred acquisition or socket work.
    pub fn play_with_appearance(
        &mut self,
        endpoint: &str,
        nickname: &str,
        appearance: &slither_client::skin_studio::AppearanceSelection,
        profile: &LoginProfile,
    ) -> Result<()> {
        if !self.can_play() {
            return Err("Explicit Play unavailable".into());
        }
        server_endpoint(endpoint)?;
        appearance.validate_wire()?;
        let login = protocol::encode_login(
            profile.client_version,
            &profile.page_echo,
            nickname,
            appearance.skin,
            (!appearance.custom_bytes.is_empty()).then_some(appearance.custom_bytes.as_slice()),
        )
        .map_err(|e| e.to_string())?;
        if matches!(self.phase(), Phase::Disconnected | Phase::Failed) {
            self.session = SessionPolicy::new();
            self.session.subscribe(2).map_err(|e| e.0)?;
        }
        self.session.check_play(true).map_err(|e| e.0)?;
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or("Generation exhausted")?;
        self.login = login;
        self.nickname = nickname.into();
        self.endpoint = endpoint.into();
        self.session.begin_play();
        self.quality.begin_connection();
        self.failure = None;
        Ok(())
    }
    fn enqueue(&mut self, kind: EffectKind, after: After) -> Result<()> {
        if self.effects.len() >= 16 {
            return Err("Effect budget".into());
        }
        let id = self.next_effect;
        self.next_effect = self
            .next_effect
            .checked_add(1)
            .ok_or("Effect identity exhausted")?;
        self.effects.push_back(Pending {
            effect: Effect {
                id,
                generation: self.generation,
                kind,
            },
            after,
            started: false,
        });
        Ok(())
    }
    pub fn effect(&self) -> Option<Effect> {
        self.effects.front().map(|p| p.effect.clone())
    }
    /// Validate and date the one Connect immediately before the host constructor effect.
    /// Retained frame timestamps belong to model work, never to the handshake deadline.
    pub fn construct(&mut self, generation: u32, id: u64, now: f64) -> Result<()> {
        let p = self.effects.front().ok_or("No Connect effect")?;
        if p.effect.generation != generation
            || p.effect.id != id
            || p.started
            || !matches!(p.effect.kind, EffectKind::Connect(_))
        {
            return Err("Stale/repeated Connect construction".into());
        }
        self.session.begin_construct(now).map_err(|e| e.0)?;
        self.effects.front_mut().expect("validated").started = true;
        Ok(())
    }
    /// Wall-clock expiry is independent of model updates, backing extent and retained pictures.
    pub fn observe_time(&mut self, now: f64) -> Result<()> {
        if !now.is_finite() {
            return Err("Invalid host timestamp".into());
        }
        if self.session.handshake_timed_out(now) {
            self.fail("Handshake timed out");
        }
        Ok(())
    }
    pub fn acknowledge(&mut self, generation: u32, id: u64, ok: bool) -> Result<()> {
        if self
            .effects
            .front()
            .is_none_or(|p| p.effect.id != id || p.effect.generation != generation)
        {
            return Err("Stale effect acknowledgement".into());
        }
        if ok
            && matches!(
                self.effects.front().expect("checked").effect.kind,
                EffectKind::Connect(_)
            )
            && !self.effects.front().expect("checked").started
        {
            return Err("Connect must begin before acknowledgement".into());
        }
        let p = self.effects.pop_front().expect("checked");
        if !ok {
            self.fail("Transport effect failed");
            return Ok(());
        }
        match p.after {
            After::None => {
                if p.effect.kind == EffectKind::Close {
                    self.has_socket = false;
                }
            }
            After::Socket => self.has_socket = true,
            After::Opened => self.session.opened(),
            After::Challenged => {
                self.login.fill(0);
                self.login.clear();
                self.session.challenged();
            }
            After::Control => self.control()?,
        }
        Ok(())
    }
    fn active(&self, generation: u32) -> bool {
        generation == self.generation && !self.session.terminal()
    }
    pub fn opened(&mut self, generation: u32, now: f64) -> Result<()> {
        if !self.active(generation) {
            return Ok(());
        }
        self.observe_time(now)?;
        if !self.active(generation) {
            return Ok(());
        }
        self.session.check_open().map_err(|e| e.0)?;
        self.enqueue(
            EffectKind::Send(SessionPolicy::negotiation_packet(0).map_err(|e| e.0)?),
            After::None,
        )?;
        self.enqueue(
            EffectKind::Send(SessionPolicy::negotiation_packet(1).map_err(|e| e.0)?),
            After::Opened,
        )
    }
    pub fn stop(&mut self) {
        self.stop_as(false, "Stopped");
    }
    pub fn fail(&mut self, message: &str) {
        if self.failure.is_none() {
            self.failure = Some(message.into());
        }
        self.stop_as(true, message);
    }
    fn stop_as(&mut self, failed: bool, reason: &str) {
        if !self
            .session
            .begin_stop(failed, reason.encode_utf16().collect())
        {
            return;
        }
        // A terminal send/presentation failure cancels the outstanding control transaction.
        // Consume only its session revision; never commit the rejected world candidate.
        self.session.commit_control(self.has_socket);
        self.login.fill(0);
        self.login.clear();
        self.effects.clear();
        self.work = None;
        self.input.leave_tick();
        self.packet_frames.clear();
        self.inflight = 0;
        self.deferred.clear();
        self.deferred_bytes = 0;
        self.session.stop_actions();
        if self.has_socket {
            let _ = self.enqueue(EffectKind::Close, After::None);
        }
    }
    pub fn closed(&mut self, generation: u32) {
        if self.active(generation) {
            self.has_socket = false;
            self.stop_as(false, "Socket closed");
        }
    }
    pub fn receive(&mut self, generation: u32, bytes: &[u8], now: f64) -> Result<()> {
        if !self.active(generation) {
            return Ok(());
        }
        self.observe_time(now)?;
        if !self.active(generation) {
            return Ok(());
        }
        if !now.is_finite() || bytes.len() > protocol::MAX_FRAME_BYTES {
            return Err("Invalid packet/time".into());
        }
        if self.deferred.len() >= 64
            || self.deferred_bytes + bytes.len() > protocol::MAX_FRAME_BYTES
        {
            self.release_stalled_presentation()?;
        }
        if self.work.is_some()
            || self.inflight != 0
            || !self.packet_frames.is_empty()
            || !self.effects.is_empty()
            || !self.deferred.is_empty()
        {
            if self.deferred.len() >= 64
                || self.deferred_bytes + bytes.len() > protocol::MAX_FRAME_BYTES
            {
                return Err("Deferred receive budget".into());
            }
            self.deferred.push_back((generation, bytes.to_vec(), now));
            self.deferred_bytes += bytes.len();
            return Ok(());
        }
        self.ingest(bytes, now)
    }
    fn release_stalled_presentation(&mut self) -> Result<()> {
        // Only a fully prepared connected frame may be superseded. Never bypass
        // the initial reset/Connect barrier, a host-owned picture or pending I/O.
        if !self.session.view().connected || self.inflight != 0 || !self.effects.is_empty() {
            return Ok(());
        }
        if let Some(work) = &self.work {
            if work.world.is_none() || work.prepared.is_none() || work.reset_rows {
                return Ok(());
            }
            // Input effects and the model update already completed. Keep that
            // update exactly once, without claiming a successful GPU present.
            self.commit_work()?;
        }
        self.supersede_setup_frames()?;
        while let Some((generation, bytes, now)) = self.deferred.pop_front() {
            self.deferred_bytes -= bytes.len();
            if self.active(generation) {
                self.ingest(&bytes, now)?;
            }
            // Setup pictures are superseded, not their admitted world/HUD state.
            self.supersede_setup_frames()?;
            if !self.effects.is_empty() {
                break;
            }
        }
        Ok(())
    }
    /// Service deferred ingress after the host's transport/effect drain. Setup
    /// pictures are immutable snapshots of already committed packet state, so
    /// newer ingress may supersede them. Work awaiting presentation and issued
    /// pictures still fence writes to the committed world.
    pub fn service_ingress(&mut self) -> Result<()> {
        if self.work.is_some() || self.inflight != 0 || !self.effects.is_empty() {
            return Ok(());
        }
        while let Some((generation, bytes, now)) = self.deferred.pop_front() {
            self.deferred_bytes -= bytes.len();
            if self.active(generation) {
                self.supersede_setup_frames()?;
                self.ingest(&bytes, now)?;
            }
            if !self.effects.is_empty() {
                break;
            }
        }
        Ok(())
    }
    /// Desktop redraws sample fresh world state instead of spending a display
    /// interval on each queued Setup snapshot. Their core draw/camera samples
    /// have already happened at ingress; never prepare them again. See the
    /// desktop-ordering lane in tools/feel-replay/native-peer.md.
    pub fn begin_redraw(&mut self, now: f64, extent: [u32; 2], scale: f64) -> Result<()> {
        self.observe_time(now)?;
        self.viewport(extent, scale)?;
        self.service_ingress()?;
        if self.work.is_none() && self.inflight == 0 && self.effects.is_empty() {
            self.supersede_setup_frames()?;
        }
        self.begin_frame(now, extent, scale)
    }
    fn supersede_setup_frames(&mut self) -> Result<()> {
        // Canvas counts each completed Setup draw even when a later draw replaces
        // it before display. Keep quality's raw draw count as well as its already
        // consumed camera sample; these removed pictures cannot be counted by ack.
        let draws = u32::try_from(self.packet_frames.len())
            .map_err(|_| "Quality draw counter exhausted")?;
        self.draws_since_statistics = self
            .draws_since_statistics
            .checked_add(draws)
            .ok_or("Quality draw counter exhausted")?;
        self.packet_frames.clear();
        Ok(())
    }
    fn ingest(&mut self, bytes: &[u8], now: f64) -> Result<()> {
        let mut session = self.session.clone();
        session.receive_bytes(bytes.len()).map_err(|e| e.0)?;
        let decoded =
            protocol::decode_frame(bytes, ElapsedHeader::Absent).map_err(|e| e.to_string())?;
        if session
            .receive_header(decoded.commands.len(), decoded.commands[0][0])
            .map_err(|e| e.0)?
        {
            let reply = solve_challenge(&decoded.commands[0][1..]).map_err(|e| e.to_string())?;
            self.session = session;
            self.enqueue(EffectKind::Send(reply), After::None)?;
            self.enqueue(EffectKind::Send(self.login.clone()), After::Challenged)?;
            #[cfg(feature = "feel-trace")]
            if self.trace.is_some() {
                self.ingress_observations.push(now);
            }
            return Ok(());
        }
        for c in &decoded.commands {
            if session
                .command_requires_version(c[0], c.len())
                .map_err(|e| e.0)?
            {
                session
                    .admit_protocol(f64::from(
                        protocol::setup_protocol_version(c).map_err(|e| e.to_string())?,
                    ))
                    .map_err(|e| e.0)?;
            }
        }
        let commands: Vec<_> = decoded
            .commands
            .iter()
            .map(|c| Command {
                opcode: c[0],
                argument: *c.get(1).unwrap_or(&0),
            })
            .collect();
        let plan = session.prepare_controls(&commands, now).map_err(|e| e.0)?;
        let guards: Vec<_> = plan
            .guards
            .iter()
            .map(|g| HudCommandGuard {
                playing: g.playing,
                close_requested: g.close_requested,
                death_pending: g.death_pending,
            })
            .collect();
        let mut client = self.client.clone();
        let bank = RefCell::new(&mut self.bank);
        let rng = Cell::new(self.rng);
        let serial = Cell::new(self.identity);
        let frames = RefCell::new(Vec::new());
        let captured_pixels = Cell::new(0_usize);
        let captured_draws = Cell::new(0_usize);
        let diagnostics = RefCell::new(self.draw_diagnostics.clone());
        let extent = self.extent;
        let scale = self.scale;
        let quality = self.quality.snapshot();
        let unit = || {
            let state = rng.get().wrapping_mul(1664525).wrapping_add(1013904223) & 0xffff_ffff;
            rng.set(state);
            #[cfg(feature = "feel-trace")]
            if let Some(trace) = &self.trace {
                trace.random("factory-unit", state as f64 / 4294967296.);
            }
            state as f64 / 4294967296.
        };
        let report=client.apply_frame_with_tinted_presentation_and_all_factories_and_setup_and_hud::<Ingress,_,_,_,_>(bytes,ElapsedHeader::Absent,
            |a,context|{let skin=resolve_authored_local_skin(a.skin,&a.custom_skin).map_err(|e|Ingress(e.to_string()))?;let presentation=if let Some(c)=&bank.borrow().cosmetics{c.appearance(a,context.own).map_err(Ingress)?}else{FacePresentation{face:skin.face,antenna:None}.into()};Ok(TintedPresentation{presentation,nickname_tint:Some(NicknameTint{base_rgb:bank.borrow().palette_rgb[skin.base_palette],random_units:[unit(),unit(),unit()]})})},
            |s,_|Ok(bank.borrow().food(s.raw_color,[unit(),unit()])),|s,_|Ok(bank.borrow().prey(s.raw_color,[unit(),unit()])),
            |c,_|{
                serial.set(serial.get()+1);
                if frames.borrow().len()>=16 {
                    diagnostics.borrow_mut().admit::<()>(serial.get(),Err("Setup draw budget".into())).map_err(Ingress)?;
                    return Ok(());
                }
                let prepared=render::prepare_gameplay(c,&mut bank.borrow_mut(),extent,scale,serial.get(),quality);
                let Some(frame)=diagnostics.borrow_mut().admit(serial.get(),prepared).map_err(Ingress)? else {
                    frames.borrow_mut().push(None);
                    return Ok(());
                };
                // Bound all retained Setup draw records together, including late HUD capacity.
                let draws=captured_draws.get()+frame.draw_count()+crate::hud::MAX_DRAWS;
                if diagnostics.borrow_mut().admit(serial.get(),crate::frame::check_draw_count(draws)).map_err(Ingress)?.is_none() {
                    frames.borrow_mut().push(None);
                    return Ok(());
                }
                // HUD projection follows the complete ingress report. Reserve its
                // bounded atlas before retaining each immutable scene snapshot.
                let bytes=captured_pixels.get()+frame.pixel_bytes()+crate::hud::MAX_PIXEL_BYTES;
                if bytes>crate::bank::PIXEL_BUDGET {
                    diagnostics.borrow_mut().admit::<()>(serial.get(),Err("Setup capture pixels budget".into())).map_err(Ingress)?;
                    frames.borrow_mut().push(None);
                    return Ok(());
                }
                captured_draws.set(draws);
                captured_pixels.set(bytes);
                frames.borrow_mut().push(Some(Picture { value: frame,
                    #[cfg(feature = "feel-trace")]
                    observation: self.trace.as_ref().and_then(|t| t.last_draw()),
                }));
                Ok(())
            },
            HudIngressPlan{schema:2,command_count:commands.len(),guards:&guards,final_capture_indices:&plan.final_capture_indices}).map_err(|e|e.0)?;
        let mut events = Vec::new();
        let mut row_events = Vec::new();
        let mut meta = Vec::new();
        for event in report.events() {
            let (kind, h) = match event.kind {
                HudIngressEventKind::Setup { table_max } => {
                    row_events.push(LeaderboardEvent {
                        command_index: event.command_index,
                        kind: LeaderboardEventKind::Setup(table_max),
                    });
                    (0, HudHistoryEvent::Setup)
                }
                HudIngressEventKind::SetupDraw { view } => {
                    (1, HudHistoryEvent::SetupDraw(sample(view)))
                }
                HudIngressEventKind::OwnLengthScheduled { token } => {
                    (4, HudHistoryEvent::OwnLengthScheduled(token))
                }
                HudIngressEventKind::Leaderboard {
                    local_row,
                    rank,
                    population,
                    death_pending,
                } => {
                    let _ = local_row;
                    (
                        2,
                        HudHistoryEvent::Leaderboard {
                            rank,
                            population,
                            death_pending,
                        },
                    )
                }
                HudIngressEventKind::FinalScore { view } => (
                    3,
                    HudHistoryEvent::FinalScore(sample(view).ok_or("Missing final score")?),
                ),
            };
            let death_pending = matches!(
                h,
                HudHistoryEvent::Leaderboard {
                    death_pending: true,
                    ..
                }
            );
            meta.push(ReportEvent {
                command_index: event.command_index,
                kind,
                death_pending,
            });
            events.push(h);
        }
        // Raw row history is admitted independently of the playing-gated HUD journal.
        for rows in report.leaderboard_rows() {
            row_events.push(LeaderboardEvent {
                command_index: rows.command_index(),
                kind: LeaderboardEventKind::Board {
                    local_row: commands[rows.command_index()].argument,
                    rows: rows
                        .rows()
                        .iter()
                        .map(|r| LeaderboardRawRow {
                            count: r.count(),
                            fractional_mass: r.fractional_mass(),
                            raw_color: r.raw_color(),
                            name: r.name().to_vec(),
                        })
                        .collect(),
                },
            });
        }
        row_events.sort_by_key(|e| e.command_index);
        session
            .match_report(&meta, report.setup_draws())
            .map_err(|e| e.0)?;
        session.commit_controls().map_err(|e| e.0)?;
        let hud = self.hud.packet(&events)?;
        let mut rows_plan = self.rows.prepare(&row_events)?;
        rows_plan.begin()?;
        let mut scores = Vec::new();
        for request in rows_plan.requests() {
            let input: Vec<_> = request
                .counts
                .iter()
                .zip(&request.fractional_masses)
                .map(|(&count, &fractional_mass)| LeaderboardScoreInput {
                    count,
                    fractional_mass,
                })
                .collect();
            scores.push(
                self.projector
                    .project(request.table_max, &input)
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .map(|s| match s {
                        LeaderboardRowScore::Value(v) => v,
                        LeaderboardRowScore::OutOfTable => f64::NAN,
                    })
                    .collect(),
            );
        }
        let rows = self
            .rows
            .stage_complete(&mut rows_plan, &scores, &self.nickname)?;
        let mut pictures = VecDeque::new();
        if !self.suspended {
            let mut bytes = 0;
            let view =
                Self::format_hud_parts(&hud, &rows, &session, self.failure.as_deref(), false);
            for frame in frames.into_inner().into_iter().flatten() {
                let identity = frame.value.frame.identity;
                let Some(p) = diagnostics.borrow_mut().admit(
                    identity,
                    render::finish_hud(frame.value, &mut self.bank, &view),
                )?
                else {
                    continue;
                };
                let next_bytes = bytes
                    + p.resources
                        .iter()
                        .map(|r| r.image.pixels.len())
                        .sum::<usize>();
                if next_bytes > crate::bank::PIXEL_BUDGET {
                    diagnostics
                        .borrow_mut()
                        .admit::<()>(identity, Err("Queued Setup pixels budget".into()))?;
                    continue;
                }
                bytes = next_bytes;
                pictures.push_back(Picture {
                    value: p,
                    #[cfg(feature = "feel-trace")]
                    observation: frame.observation,
                });
            }
        }
        // All packet outputs and their aggregate charge are valid before any world/history publish.
        rows.commit()?;
        self.client = client;
        self.session = session;
        self.hud = hud;
        self.rows = rows;
        self.identity = serial.get();
        self.rng = rng.get();
        self.packet_frames.extend(pictures);
        self.draw_diagnostics = diagnostics.into_inner();
        #[cfg(feature = "feel-trace")]
        if self.trace.is_some() {
            self.ingress_observations.push(now);
        }
        Ok(())
    }
    /// Probe only the existing transactional clock callback; intentionally abort before entity update.
    /// The same owner commits it exactly once after all staged input send acknowledgements.
    pub fn begin_frame(&mut self, now: f64, extent: [u32; 2], scale: f64) -> Result<()> {
        self.observe_time(now)?;
        self.viewport(extent, scale)?;
        if self.suspended {
            return Ok(());
        }
        if !now.is_finite() {
            return Err("Invalid frame timestamp".into());
        }
        if self.work.is_some()
            || self.inflight != 0
            || !self.effects.is_empty()
            || !self.packet_frames.is_empty()
        {
            return Ok(());
        }
        while let Some((g, b, t)) = self.deferred.pop_front() {
            self.deferred_bytes -= b.len();
            if self.active(g) {
                self.ingest(&b, t)?;
            }
            if !self.packet_frames.is_empty() || !self.effects.is_empty() {
                return Ok(());
            }
        }
        self.session.observe_lag(now);
        let minimap_ui = self.session.view().ui_timer_needed;
        self.session.ui_tick(now).map_err(|e| e.0)?;
        if minimap_ui {
            self.client
                .observe_minimap_ui_fraction(self.session.view().return_fraction)
                .map_err(|e| e.to_string())?;
        }
        let lagging = self.session.view().lagging;
        let mut captured = None;
        let probe = self
            .client
            .advance_with_control::<Probe, _>(now, lagging, |step, control| {
                captured = Some((step, control));
                Err(Probe::Captured)
            });
        if let Err(Probe::Core(e)) = probe {
            return Err(e);
        }
        let (step, view) = captured.ok_or("Missing core step")?;
        self.policy.begin_frame();
        self.policy.begin_control();
        self.input.set_connected(self.session.view().connected);
        if let Some(own) = view.own {
            self.input
                .set_own(
                    OwnInput {
                        wanted_heading: own.wanted_heading,
                        gaze_heading: own.gaze_heading,
                        dead: own.dead,
                        turn_rate: own.turn_rate,
                        size_turn_factor: own.size_turn_factor,
                        speed_turn_factor: own.speed_turn_factor,
                        requested_boost: false,
                        sent_boost: false,
                    },
                    self.own == Some(own.token),
                )
                .map_err(|e| e.to_string())?;
            self.own = Some(own.token);
        } else {
            self.input.clear_own();
            self.own = None;
        }
        self.input
            .accumulate(f64::from(step.whole_steps))
            .map_err(|e| e.to_string())?;
        self.policy.debt_done();
        self.session.check_step(true).map_err(|e| e.0)?;
        let (life, close) = self
            .session
            .advance_step(now, step.delta, f64::from(step.whole_steps))
            .map_err(|e| e.0)?;
        if close {
            self.stop_as(false, "Life returned");
        }
        self.session.finish_step(self.has_socket);
        let (_, pending) = self.session.actions();
        self.policy.lifecycle_done(pending.reset_world);
        let mut hud = self.hud.step(
            step.delta,
            HudLifecycle {
                death_pending: life.death_pending,
                return_active: life.return_active,
                return_completed: life.return_completed,
                return_fraction: life.return_fraction,
            },
        )?;
        if pending.reset_world {
            hud = hud.reset_world();
        }
        let actions = ClientControlActions {
            reset_world: pending.reset_world,
            reset_lag: pending.reset_lag,
            announcement_lifecycle: self.policy.include_announcements().then_some(
                AnnouncementLifecycle {
                    death_pending: life.death_pending,
                    return_active: life.return_active,
                    return_completed: life.return_completed,
                    return_fraction: life.return_fraction,
                },
            ),
            minimap_ui_fraction: self
                .policy
                .include_minimap(life.return_active)
                .then_some(life.return_fraction),
            connected: if pending.disconnected {
                Some(false)
            } else {
                Some(self.session.view().connected)
            },
            ..Default::default()
        };
        let phase = if pending.reset_world || !self.session.view().connected {
            self.input.reset();
            4
        } else {
            self.input.enter_tick().map_err(|e| e.to_string())?;
            0
        };
        self.work = Some(Work {
            now,
            lagging,
            step,
            actions,
            phase,
            extent,
            scale,
            hud,
            reset_rows: pending.reset_world,
            world: None,
            prepared: None,
            quality: self.quality.clone(),
            statistics_consumed: false,
        });
        self.control()
    }
    fn control(&mut self) -> Result<()> {
        loop {
            let Some(w) = &mut self.work else {
                return Ok(());
            };
            let now = w.now;
            let phase = w.phase;
            w.phase += 1;
            let packet = match phase {
                0 => self
                    .input
                    .keyboard_phase(now, 0.)
                    .map_err(|e| e.to_string())?,
                1 => {
                    if self.session.ping_available() && self.session.ping_due(now) {
                        self.session.commit_ping(now)
                    } else {
                        Vec::new()
                    }
                }
                2 => {
                    if self.input.view().own.is_some() {
                        self.input.boost_phase(now).map_err(|e| e.to_string())?
                    } else {
                        Vec::new()
                    }
                }
                3 => {
                    if self.input.view().own.is_some() {
                        self.input.heading_phase(now).map_err(|e| e.to_string())?
                    } else {
                        Vec::new()
                    }
                }
                _ => {
                    self.input.leave_tick();
                    if self.input.view().own.is_some() && self.session.view().connected {
                        w.actions.gaze_heading =
                            Some(self.input.required_gaze().map_err(|e| e.to_string())?);
                    }
                    break;
                }
            };
            if !packet.is_empty() {
                if !SessionPolicy::valid_input(&packet) {
                    return Err("Invalid compact input intent".into());
                }
                self.enqueue(EffectKind::Send(packet), After::Control)?;
                return Ok(());
            }
        }
        let w = self.work.as_mut().expect("work");
        self.policy.control_done();
        let expected = w.step;
        let actions = w.actions;
        let mut world = self.client.clone();
        world
            .advance_with_control::<Probe, _>(w.now, w.lagging, |step, _| {
                if step != expected {
                    return Err(Probe::Core("Step changed before commit".into()));
                }
                Ok(actions)
            })
            .map_err(|e| match e {
                Probe::Core(e) => e,
                Probe::Captured => "Unexpected probe".into(),
            })?;
        self.identity += 1;
        let playing = self.session.view().playing;
        w.quality
            .advance(w.step, playing)
            .map_err(|e| e.to_string())?;
        w.statistics_consumed = w
            .quality
            .update_statistics(w.now, self.draws_since_statistics, playing)
            .map_err(|e| e.to_string())?;
        let frame = render::prepare_gameplay(
            &mut world,
            &mut self.bank,
            w.extent,
            w.scale,
            self.identity,
            w.quality.snapshot(),
        );
        self.policy.controller_done();
        let score = if w.hud.readout_required(self.session.view().playing) {
            sample(world.score_view())
        } else {
            None
        };
        w.hud = w.hud.draw(self.session.view().playing, score)?;
        self.policy.score_done();
        w.world = Some(world);
        let w = self.work.as_ref().expect("work");
        let view = self.format_hud(&w.hud, w.reset_rows);
        let prepared = frame.and_then(|frame| render::finish_hud(frame, &mut self.bank, &view));
        let Some(prepared) = self.draw_diagnostics.admit(self.identity, prepared)? else {
            return self.commit_work();
        };
        self.work.as_mut().expect("work").prepared = Some(Picture {
            value: prepared,
            #[cfg(feature = "feel-trace")]
            observation: self.trace.as_ref().and_then(|t| t.last_draw()),
        });
        Ok(())
    }
    pub fn take_frame(&mut self) -> Option<PreparedFrame> {
        if self.suspended || self.inflight != 0 {
            return None;
        }
        if let Some(frame) = self.packet_frames.pop_front() {
            self.inflight = 1;
            #[cfg(feature = "feel-trace")]
            {
                self.issued_observation = frame.observation;
            }
            return Some(frame.value);
        }
        let prepared = self.work.as_mut()?.prepared.take()?;
        self.inflight = 2;
        #[cfg(feature = "feel-trace")]
        {
            self.issued_observation = prepared.observation;
        }
        Some(prepared.value)
    }
    /// Non-consuming drain-boundary query: never issue an extra unacknowledged picture.
    pub fn has_frame(&self) -> bool {
        !self.suspended
            && self.inflight == 0
            && (!self.packet_frames.is_empty()
                || self.work.as_ref().is_some_and(|w| w.prepared.is_some()))
    }
    pub fn presented(&mut self, success: bool) -> Result<()> {
        let kind = std::mem::replace(&mut self.inflight, 0);
        if kind == 0 {
            return Err("No issued frame".into());
        }
        if !success {
            self.fail("GPU presentation failed");
            return Ok(());
        }
        if kind != 1 {
            self.commit_work()?;
        }
        self.draws_since_statistics = self
            .draws_since_statistics
            .checked_add(1)
            .ok_or("Quality draw counter exhausted")?;
        Ok(())
    }
    /// Retire rejected pixels without losing the issued model/control transaction.
    pub fn skip_frame(&mut self) -> Result<()> {
        let kind = std::mem::replace(&mut self.inflight, 0);
        if kind == 0 {
            return Err("No issued frame".into());
        }
        if kind != 1 {
            self.commit_work()?;
        }
        #[cfg(feature = "feel-trace")]
        {
            self.issued_observation = None;
        }
        Ok(())
    }
    fn commit_work(&mut self) -> Result<()> {
        let w = self.work.take().ok_or("Missing frame work")?;
        self.client = w.world.ok_or("Missing world candidate")?;
        self.hud = w.hud;
        self.quality = w.quality;
        if w.statistics_consumed {
            self.draws_since_statistics = 0;
        }
        if w.reset_rows {
            let rows = self.rows.stage_reset()?;
            rows.commit()?;
            self.rows = rows;
        }
        if self.session.commit_control(self.has_socket) {
            self.enqueue(EffectKind::Connect(self.endpoint.clone()), After::Socket)?;
        }
        self.policy.frame_commit_done(self.session.view().connected);
        self.policy.finish_frame();
        Ok(())
    }
    /// Native DOM siblings use CSS coordinates and physical rasters independently
    /// of the capped game canvas, including resizes that keep the same backing.
    pub fn window_viewport(&mut self, physical: [u32; 2], dpr: f64) -> Result<[u32; 2]> {
        if physical.contains(&0) {
            self.viewport([0, 0], 1.)?;
            return Ok([0, 0]);
        }
        render::dimensions(physical, dpr)?;
        let overlay = Some((physical, dpr));
        if self.bank.overlay_viewport != overlay {
            self.bank.overlay_viewport = overlay;
            self.suspended = true;
        }
        let backing = render::backing_size(physical.map(|n| f64::from(n) / dpr))?;
        self.viewport(backing, 1.)?;
        Ok(backing)
    }
    /// Resize supersedes queued intermediate Setup pictures, never their admitted packet state.
    pub fn viewport(&mut self, extent: [u32; 2], scale: f64) -> Result<()> {
        if extent.contains(&0) {
            self.suspended = true;
            self.supersede_setup_frames()?;
            return Ok(());
        }
        render::dimensions(extent, scale)?;
        let changed = self.suspended || self.extent != extent || self.scale != scale;
        self.suspended = false;
        if changed {
            let mut skipped = false;
            self.extent = extent;
            self.scale = scale;
            self.supersede_setup_frames()?;
            let view = self.work.as_ref().map_or_else(
                || self.hud_view(),
                |w| self.format_hud(&w.hud, w.reset_rows),
            );
            if let Some(w) = &mut self.work {
                w.extent = extent;
                w.scale = scale;
                if let Some(world) = &mut w.world {
                    self.identity += 1;
                    let frame = render::prepare_gameplay(
                        world,
                        &mut self.bank,
                        extent,
                        scale,
                        self.identity,
                        w.quality.snapshot(),
                    )
                    .and_then(|frame| render::finish_hud(frame, &mut self.bank, &view));
                    w.prepared = self
                        .draw_diagnostics
                        .admit(self.identity, frame)?
                        .map(|value| Picture {
                            value,
                            #[cfg(feature = "feel-trace")]
                            observation: self.trace.as_ref().and_then(|t| t.last_draw()),
                        });
                    skipped = w.prepared.is_none();
                }
            }
            if skipped {
                self.commit_work()?;
            }
        }
        Ok(())
    }
    pub fn retry_frame(&mut self, frame: PreparedFrame) -> Result<()> {
        let frame = Picture {
            value: frame,
            #[cfg(feature = "feel-trace")]
            observation: self.issued_observation.take(),
        };
        match std::mem::replace(&mut self.inflight, 0) {
            1 => self.packet_frames.push_front(frame),
            2 => self.work.as_mut().ok_or("Missing frame work")?.prepared = Some(frame),
            _ => return Err("No issued frame".into()),
        }
        Ok(())
    }
    pub fn key(&mut self, code: &str, down: bool) -> bool {
        self.input.key(code, down)
    }
    pub fn blur(&mut self) {
        self.input.blur();
    }
    pub fn pointer(&mut self, x: f64, y: f64, width: f64, height: f64, down: bool) -> Result<()> {
        let v = self.client.control_view();
        let head = v.own.map(|o| o.raw_head).unwrap_or_default();
        let args = [
            x,
            y,
            width,
            height,
            f64::from(v.follows),
            f64::from(v.own.is_some()),
            v.camera.x,
            v.camera.y,
            head.x,
            head.y,
            v.zoom,
        ];
        if down {
            self.input.mouse_down(&args).map_err(|e| e.to_string())?;
        } else {
            self.input.pointer(&args).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn mouse_up(&mut self) {
        self.input.mouse_up();
    }
    pub fn hud_text(&self) -> String {
        self.hud_view().compact_text()
    }
    /// Same typed in-game view for both platform hosts; no session or model mutation.
    pub fn hud_view(&self) -> crate::hud::View {
        self.format_hud(&self.hud, false)
    }
    fn format_hud(&self, hud: &HudHistory, reset_rows: bool) -> crate::hud::View {
        Self::format_hud_parts(
            hud,
            &self.rows,
            &self.session,
            self.failure.as_deref(),
            reset_rows,
        )
    }
    fn format_hud_parts(
        hud: &HudHistory,
        rows: &LeaderboardHistory,
        session: &SessionPolicy,
        failure: Option<&str>,
        reset_rows: bool,
    ) -> crate::hud::View {
        crate::hud::View::project(hud, rows, session, failure, reset_rows)
    }
}
#[cfg(test)]
mod hud_projection_tests {
    use super::*;
    #[test]
    fn full_hud_copies_the_existing_top_ten_projection_without_sorting_or_rescoring() {
        let mut g = LocalGame::new(0.).unwrap();
        g.nickname = "Player".into();
        let events = [
            LeaderboardEvent {
                command_index: 0,
                kind: LeaderboardEventKind::Setup(430),
            },
            LeaderboardEvent {
                command_index: 1,
                kind: LeaderboardEventKind::Board {
                    local_row: 3,
                    rows: (0..12)
                        .map(|i| LeaderboardRawRow {
                            count: 12 + i,
                            fractional_mass: 0.5,
                            raw_color: i as u8,
                            name: format!("Row {i}").into_bytes(),
                        })
                        .collect(),
                },
            },
        ];
        let mut plan = g.rows.prepare(&events).unwrap();
        plan.begin().unwrap();
        let input: Vec<_> = (0..12)
            .map(|i| LeaderboardScoreInput {
                count: 12 + i,
                fractional_mass: 0.5,
            })
            .collect();
        let scores = g
            .projector
            .project(430, &input)
            .unwrap()
            .into_iter()
            .map(|v| match v {
                LeaderboardRowScore::Value(n) => n,
                LeaderboardRowScore::OutOfTable => f64::NAN,
            })
            .collect();
        let rows = g
            .rows
            .stage_complete(&mut plan, &[scores], &g.nickname)
            .unwrap();
        rows.commit().unwrap();
        g.rows = rows;
        g.hud = g
            .hud
            .packet(&[HudHistoryEvent::Leaderboard {
                rank: 3,
                population: 50,
                death_pending: false,
            }])
            .unwrap();
        let view = g.hud_view();
        assert_eq!(view.rows, g.rows.visible_rows());
        assert_eq!(view.rows.len(), 10);
        assert_eq!(view.rows[2].name_text, "Player");
        assert!(view.rows[2].own);
        assert_eq!(view.rank, "--");
        assert_eq!(view.population, "--");
        assert_eq!(
            view.rows.iter().map(|r| r.ordinal).collect::<Vec<_>>(),
            (1..=10).collect::<Vec<_>>()
        );
        assert_eq!(g.hud_text(), view.compact_text());
        assert!(g.effect().is_none());
    }
}
