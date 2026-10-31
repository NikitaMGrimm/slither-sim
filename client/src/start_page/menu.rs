//! Opt-in menu policy; adapters must still obtain route/session Play admission.
//! Numeric source rationale is collected in docs/start-menu.md.

use super::{MenuLocale, MenuWording, normalize_nickname_units};
use crate::menu_layout::round5;
use crate::menu_preferences::MenuQuality;
use crate::presentation::HudFinal;
use crate::session_policy::Phase;

/// Persistent login result, independent of the session-reset end-screen policy.
#[derive(Clone, Debug, PartialEq)]
pub struct LastLifeView {
    /// Localized regular-weight prefix; empty before an accepted captured result.
    pub label: &'static str,
    /// Ungrouped bold number, or a nonbreaking space for the initial blank slot.
    pub number: String,
    /// Regular-weight punctuation, present only above 1000.
    pub suffix: &'static str,
    /// Prefix opacity; number and punctuation use full opacity.
    pub label_alpha: f64,
}

/// Copied lifecycle observations; the session remains the only death clock.
#[derive(Clone, Copy, Debug)]
pub struct MenuSession {
    /// Current session phase.
    pub phase: Phase,
    /// First accepted outcome, including after transport close.
    pub outcome: Option<Phase>,
    /// Gameplay remains owned through the death hold and return.
    pub playing: bool,
    /// The existing death hold/return is pending.
    pub death_pending: bool,
    /// The session's return clock is running.
    pub return_active: bool,
    /// Committed world alpha from that clock.
    pub return_fraction: f64,
}
impl From<&crate::session_policy::View> for MenuSession {
    fn from(view: &crate::session_policy::View) -> Self {
        Self {
            phase: view.phase,
            outcome: view.outcome,
            playing: view.playing,
            death_pending: view.death_pending,
            return_active: view.return_active,
            return_fraction: view.return_fraction,
        }
    }
}

/// Menu visibility clock, never a replacement for SessionPolicy's lifecycle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuPhase {
    /// Editable login.
    Login,
    /// A click is locked and its request is pending/preparing.
    Connecting,
    /// Accepted gameplay is fading the login away.
    Outgoing,
    /// Login is hidden.
    World,
    /// Accepted game over is waiting for the strict 1600ms boundary.
    DeathHold,
    /// Login is fading back in.
    Returning,
}

/// Copied input actions; IME ownership is resolved before dispatch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    /// Explicit button activation.
    Play,
    /// Enter only activates after composition is committed by the editor.
    Enter {
        /// An active IME composition owns this keypress.
        composing: bool,
    },
    /// Request input focus.
    FocusNickname,
    /// Activate Change Skin.
    Skin,
    /// Activate Choose Server.
    Server,
    /// Toggle the requested graphics mode.
    Quality,
}

/// Effects to execute outside the core borrow. No DOM, socket or storage handles.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StartMenuIntent {
    /// Focus the nickname field.
    FocusNickname,
    /// A once-only request on the first admitted update after the click/return.
    /// The caller must use its
    /// existing route/profile admission; this data alone grants no connection.
    Play {
        /// Normalized committed name frozen at the click.
        nickname: String,
        /// Requested graphics mode frozen at the click.
        quality: MenuQuality,
    },
    /// Open the later local preview policy.
    OpenSkinChooser,
    /// Open the later server-directory policy; never opens a game session.
    OpenServerPicker,
    /// Best-effort independent graphics-key write. Failure leaves memory intact.
    PersistQuality(&'static str),
}

/// Copied login presentation. Font/layout/art execution belongs to later adapters.
#[derive(Clone, Debug, PartialEq)]
pub struct StartMenuView {
    /// Current menu clock phase.
    pub phase: MenuPhase,
    /// Whether the login is mounted, even at zero opacity during return.
    pub visible: bool,
    /// Nickname and other ordinary controls remain disabled through return.
    pub disabled: bool,
    /// Play can accept one queued click during the visible ordinary return.
    pub play_disabled: bool,
    /// Champion is deliberately incomplete until the editor lane is integrated.
    pub champion_pending: bool,
    /// Ordinary nickname/Play/skin controls are suppressed for a champion.
    pub ordinary_controls_visible: bool,
    /// Committed normalized nickname.
    pub nickname: String,
    /// All source locale labels and tips.
    pub wording: MenuWording,
    /// Initial or replay label (measure again when it changes).
    pub play_label: &'static str,
    /// Persistent result, including the initial blank slot.
    pub last_life: LastLifeView,
    /// Login/world crossfade fraction; feed unchanged to MenuLayout.
    pub transition: f64,
    /// Login and fixed corner-link opacity.
    pub login_alpha: f64,
    /// World opacity during the login crossfade.
    pub world_alpha: f64,
    /// Extra Play opacity on return, before multiplying by login/reveal opacity.
    pub play_return_alpha: f64,
    /// Logo opacity; no curve or image data is embedded here.
    pub logo_alpha: f64,
    /// Logo reveal progress for a separately authored animation asset.
    pub logo_reveal: f64,
    /// Nickname holder's reveal opacity.
    pub nickname_alpha: f64,
    /// Skin-menu reveal opacity.
    pub skin_alpha: f64,
    /// Play holder's reveal opacity.
    pub play_alpha: f64,
    /// Current tip; initially a nonbreaking space until the first phase wrap.
    pub tip: &'static str,
    /// False permanently after the first outgoing fade completes; adapters must
    /// remove the row and pass this value to MenuLayout, not just set opacity.
    pub tips_visible: bool,
    /// Rounded tip opacity.
    pub tip_alpha: f64,
    /// Hover overlay, suppressed when disabled.
    pub hover_alpha: f64,
    /// Press overlay, suppressed when disabled.
    pub press_alpha: f64,
    /// The centered 128px spinner is mounted while this is true.
    pub spinner_visible: bool,
    /// Unlagged spinner animation time for the later authored drawing.
    pub spinner_time: f64,
    /// Spinner opacity.
    pub spinner_alpha: f64,
    /// Rounded spinner scale, around its center.
    pub spinner_scale: f64,
    /// Requested graphics mode, independent of adaptive QualityState.
    pub quality: MenuQuality,
}

/// Opt-in state. Existing Canvas start-page functions and callers remain unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct StartMenuState {
    locale: MenuLocale,
    phase: MenuPhase,
    nickname: String,
    quality: MenuQuality,
    last_capture: Option<HudFinal>,
    replay: bool,
    champion: bool,
    last_ms: f64,
    transition: f64,
    outgoing_active: bool,
    lag: f64,
    logo_alpha: f64,
    logo_reveal: f64,
    nickname_progress: f64,
    tip_phase: f64,
    tip_alpha: f64,
    tip_index: Option<usize>,
    tips_active: bool,
    hovered: bool,
    pressed: bool,
    hover_alpha: f64,
    press_alpha: f64,
    spinner_visible: bool,
    spinner_time: f64,
    spinner_alpha: f64,
    spinner_progress: f64,
    pending_play: Option<(String, MenuQuality)>,
    focus_pending: bool,
    return_play: bool,
    return_enter_enabled: bool,
}

impl StartMenuState {
    /// Start at time zero. Desktop animates the logo; mobile/Safari supply false.
    /// Pass the first clock relative to creation, not an unrelated epoch.
    pub fn new(locale: MenuLocale, animate_logo: bool, quality: MenuQuality) -> Self {
        let reveal = if animate_logo { 0. } else { 1. };
        Self {
            locale,
            phase: MenuPhase::Login,
            nickname: String::new(),
            quality,
            last_capture: None,
            replay: false,
            champion: false,
            last_ms: 0.,
            transition: 0.,
            outgoing_active: false,
            lag: 1.,
            logo_alpha: reveal,
            logo_reveal: reveal,
            nickname_progress: reveal,
            tip_phase: 1.9,
            tip_alpha: 0.,
            tip_index: None,
            tips_active: true,
            hovered: false,
            pressed: false,
            hover_alpha: 0.,
            press_alpha: 0.,
            spinner_visible: false,
            spinner_time: 0.,
            spinner_alpha: 0.,
            spinner_progress: 0.,
            pending_play: None,
            focus_pending: true,
            return_play: false,
            return_enter_enabled: false,
        }
    }

    /// Normalize committed UTF-16 editor data only while nickname is editable.
    pub fn edit_nickname(&mut self, units: &[u16]) {
        if self.phase == MenuPhase::Login && !self.champion {
            self.nickname = normalize_nickname_units(units);
        }
    }

    /// Observe pointer state using the current admission availability. Disabled
    /// controls reject activation but still accept leave/release cleanup.
    /// Overlays advance per callback, not per elapsed unit.
    pub fn play_pointer(&mut self, hovered: bool, pressed: bool, admission_available: bool) {
        let enabled = self.play_enabled(admission_available);
        if !hovered || enabled {
            self.hovered = hovered;
        }
        if !pressed || enabled {
            self.pressed = pressed;
        }
    }

    fn play_enabled(&self, admission_available: bool) -> bool {
        !self.champion
            && (self.phase == MenuPhase::Returning
                || (self.phase == MenuPhase::Login && admission_available))
    }

    /// Route an action. `admission_available` must come from the existing admission
    /// owner. A visible return can queue Play before that admission is ready;
    /// the request remains locked until return completion and an admitted step.
    pub fn action(
        &mut self,
        action: MenuAction,
        admission_available: bool,
    ) -> Option<StartMenuIntent> {
        let returning = self.phase == MenuPhase::Returning;
        let activation = action == MenuAction::Play
            || (action == MenuAction::Enter { composing: false }
                && (!returning || self.return_enter_enabled));
        if activation && self.play_enabled(admission_available) {
            self.return_play = returning;
            self.phase = MenuPhase::Connecting;
            self.focus_pending = false;
            self.pending_play = Some((self.nickname.clone(), self.quality));
            self.spinner_visible = true;
            return None;
        }
        if self.phase != MenuPhase::Login || self.champion {
            return None;
        }
        match action {
            MenuAction::FocusNickname => Some(StartMenuIntent::FocusNickname),
            MenuAction::Skin => Some(StartMenuIntent::OpenSkinChooser),
            MenuAction::Server => Some(StartMenuIntent::OpenServerPicker),
            MenuAction::Quality => Some(StartMenuIntent::PersistQuality(self.quality.toggle())),
            _ => None,
        }
    }

    /// Begin the outgoing login clock only after accepted gameplay, not a click.
    pub fn begin_outgoing(&mut self) -> bool {
        if self.phase != MenuPhase::Connecting || self.pending_play.is_some() {
            return false;
        }
        self.pending_play = None;
        self.phase = MenuPhase::Outgoing;
        self.outgoing_active = true;
        true
    }

    /// Accept an already committed game-over outcome and its command-time capture.
    /// Closed/disconnected phases cannot write a result. Repeated outcomes cannot
    /// rewrite the capture or restart the hold. A missing ready capture stays blank.
    pub fn accepted_game_over(
        &mut self,
        phase: Phase,
        capture: Option<HudFinal>,
    ) -> Result<bool, &'static str> {
        if !matches!(phase, Phase::Dead | Phase::Victory)
            || !matches!(self.phase, MenuPhase::World | MenuPhase::Outgoing)
        {
            return Ok(false);
        }
        if let Some(value) = capture {
            value.score_text()?;
        }
        self.last_capture = capture;
        self.replay = true;
        self.champion = phase == Phase::Victory;
        self.phase = MenuPhase::DeathHold;
        self.return_enter_enabled = false;
        self.lag = 1.;
        Ok(true)
    }

    /// Project the committed session clock. This policy never times a death,
    /// closes a socket or resets a world. Outcome survives a transport close;
    /// only the session's completed return enables ordinary editing and focus.
    pub fn observe_session(
        &mut self,
        session: MenuSession,
        capture: Option<HudFinal>,
    ) -> Result<bool, &'static str> {
        let life = crate::session_policy::Lifecycle {
            death_pending: session.death_pending,
            return_active: session.return_active,
            return_completed: !session.death_pending && session.return_fraction == 0.,
            return_fraction: session.return_fraction,
        };
        // A click locks immediately, but the old session still owns its return
        // clock. Never treat its playing/outcome flags as a fresh attempt.
        if self.return_play {
            self.observe_return(life);
            // Browser admission owners still need the old return-completion
            // notification, even though no nickname focus is requested.
            return Ok(!self.return_play);
        }
        // The previous generation can still be terminal while Play is queued.
        if self.pending_play.is_some() {
            return Ok(false);
        }
        // Failed replay acquisition may still expose the previous session.
        // Its original own token cannot authorize another game over.
        if self.phase == MenuPhase::Connecting
            && matches!(session.phase, Phase::Failed | Phase::Disconnected)
            && session.outcome.is_some()
            && capture.is_some_and(|value| {
                self.last_capture
                    .is_some_and(|last| last.sample.token == value.sample.token)
            })
        {
            return Ok(self.attempt_ended(session.phase));
        }
        if session.playing || session.outcome.is_some() {
            self.begin_outgoing();
        }
        if let Some(outcome) = session.outcome {
            self.accepted_game_over(outcome, capture)?;
        }
        self.return_enter_enabled = matches!(
            session.phase,
            Phase::Ready | Phase::Disconnected | Phase::Failed
        );
        let mut returned = false;
        if matches!(
            self.phase,
            MenuPhase::Outgoing | MenuPhase::World | MenuPhase::DeathHold | MenuPhase::Returning
        ) {
            self.outgoing_active = false;
            self.transition = session.return_fraction;
            if self.transition == 1. {
                self.tips_active = false;
                self.logo_alpha = 1.;
                self.logo_reveal = 1.;
                self.nickname_progress = 1.;
                if self.phase == MenuPhase::Outgoing {
                    self.phase = MenuPhase::World;
                }
            }
            if matches!(self.phase, MenuPhase::DeathHold | MenuPhase::Returning) {
                returned = !session.death_pending && self.transition == 0. && !self.champion;
                self.observe_return(life);
            } else if matches!(session.phase, Phase::Failed | Phase::Disconnected) {
                self.disconnected();
                return Ok(true);
            }
        }
        Ok(returned || self.attempt_ended(session.phase))
    }

    /// Same-step lifecycle projection, also useful to offline callers. No clock
    /// or delta is recomputed here; SessionPolicy owns the strict hold and fade.
    pub fn observe_return(&mut self, life: crate::session_policy::Lifecycle) {
        if self.return_play {
            self.transition = life.return_fraction;
            if life.return_completed {
                self.return_play = false;
            }
            return;
        }
        if !matches!(self.phase, MenuPhase::DeathHold | MenuPhase::Returning) {
            return;
        }
        if life.return_active || life.return_completed {
            self.transition = life.return_fraction;
            self.phase = MenuPhase::Returning;
        }
        if life.return_completed {
            self.phase = MenuPhase::Login;
            self.focus_pending = !self.champion;
        }
    }

    /// The champion owner calls this after acknowledgement or terminal transport
    /// has ended the editor and its sampled exit fade has completed.
    pub fn champion_finished(&mut self) {
        if self.champion {
            self.champion = false;
            if self.phase == MenuPhase::Login {
                self.focus_pending = true;
            }
        }
    }
    /// Consume focus at the host's post-session seam, including return completion.
    pub fn take_focus_intent(&mut self) -> Option<StartMenuIntent> {
        std::mem::take(&mut self.focus_pending).then_some(StartMenuIntent::FocusNickname)
    }

    /// A prepared return may still await picture/transport acknowledgement.
    /// Retain focus and a frozen Play until the existing admission owner is ready.
    pub fn step_with_admission(
        &mut self,
        now_ms: f64,
        lagging: bool,
        admission: bool,
    ) -> Result<Vec<StartMenuIntent>, &'static str> {
        let mut intents = self.advance(now_ms, lagging, admission)?;
        if (self.phase != MenuPhase::Login || self.champion || !admission)
            && intents.contains(&StartMenuIntent::FocusNickname)
        {
            self.focus_pending = true;
            intents.retain(|intent| *intent != StartMenuIntent::FocusNickname);
        }
        Ok(intents)
    }

    /// Return only the dispatched connecting attempt. A queued explicit retry
    /// must not be cancelled by the previous session's terminal observation.
    pub fn attempt_ended(&mut self, phase: Phase) -> bool {
        if self.phase != MenuPhase::Connecting
            || self.pending_play.is_some()
            || !matches!(phase, Phase::Failed | Phase::Disconnected)
        {
            return false;
        }
        self.disconnected();
        true
    }

    /// Cancel a queued click without restarting the old session's return.
    pub fn cancel_pending_play(&mut self) {
        if self.pending_play.is_none() {
            return;
        }
        let returning = self.return_play.then_some(self.transition);
        self.disconnected();
        if let Some(fraction) = returning {
            self.phase = MenuPhase::Returning;
            self.transition = fraction;
            self.focus_pending = false;
        }
    }

    /// Explicit transport-failure return; retains prior last-life content and never
    /// invents an outcome. The champion owner completes its editor exit separately.
    pub fn disconnected(&mut self) {
        if matches!(self.phase, MenuPhase::DeathHold | MenuPhase::Returning) {
            return;
        }
        self.phase = MenuPhase::Login;
        self.transition = 0.;
        self.outgoing_active = false;
        self.pending_play = None;
        self.return_play = false;
        self.focus_pending = !self.champion;
        self.spinner_visible = false;
        self.spinner_alpha = 0.;
        self.spinner_progress = 0.;
        self.spinner_time = 0.;
        self.hovered = false;
        self.pressed = false;
    }

    /// Advance one animation callback at a finite monotonic clock. Frame delta is
    /// elapsed/8 clamped before lag; logo/outgoing use their separate /25 clock.
    /// Death/return uses observe_session, never this animation clock.
    /// Hosts must use `step_with_admission`; this convenience assumes admission.
    pub fn step(
        &mut self,
        now_ms: f64,
        lagging: bool,
    ) -> Result<Vec<StartMenuIntent>, &'static str> {
        self.step_with_admission(now_ms, lagging, true)
    }

    fn advance(
        &mut self,
        now_ms: f64,
        lagging: bool,
        admission: bool,
    ) -> Result<Vec<StartMenuIntent>, &'static str> {
        if !now_ms.is_finite() || now_ms < 0. {
            return Err("Invalid menu clock");
        }
        let elapsed = (now_ms - self.last_ms).max(0.);
        self.last_ms = self.last_ms.max(now_ms);
        let unlagged = (elapsed / 8.).clamp(0., 5.);
        self.lag = if lagging {
            (self.lag * 0.85).max(0.2)
        } else {
            (self.lag + 0.05).min(1.)
        };
        let delta = unlagged * self.lag;
        let logo_delta = elapsed / 25.;
        self.logo_alpha = (self.logo_alpha + 0.05 * logo_delta).min(1.);
        self.logo_reveal = (self.logo_reveal + 0.00375 * logo_delta).min(1.);
        self.nickname_progress = (self.nickname_progress + 0.006 * logo_delta).min(1.);
        self.hover_alpha = overlay(self.hover_alpha, self.hovered);
        self.press_alpha = overlay(self.press_alpha, self.pressed);

        let mut intents = Vec::new();
        if let Some(focus) = self.take_focus_intent() {
            intents.push(focus);
        }
        if admission
            && !self.return_play
            && let Some((nickname, quality)) = self.pending_play.take()
        {
            intents.push(StartMenuIntent::Play { nickname, quality });
        }
        if self.spinner_visible {
            self.spinner_time += unlagged;
            if self.phase == MenuPhase::Connecting {
                self.spinner_alpha = (self.spinner_alpha + unlagged / 86.).min(1.);
                self.spinner_progress = (self.spinner_progress + unlagged / 93.).min(1.);
            } else {
                self.spinner_alpha = (self.spinner_alpha - unlagged / 86.).max(0.);
                if self.spinner_alpha == 0. {
                    self.spinner_visible = false;
                    self.spinner_progress = 0.;
                }
            }
        }
        if self.outgoing_active {
            self.transition = (self.transition + logo_delta * 0.05).min(1.);
            if self.transition == 1. {
                self.outgoing_active = false;
                if self.phase == MenuPhase::Outgoing {
                    self.phase = MenuPhase::World;
                }
                self.tips_active = false;
                self.logo_alpha = 1.;
                self.logo_reveal = 1.;
                self.nickname_progress = 1.;
            }
        }
        if self.transition != 1. && self.tips_active {
            self.tip_phase += delta * 0.017;
            if self.tip_phase >= std::f64::consts::TAU {
                self.tip_phase -= std::f64::consts::TAU;
                self.tip_index = Some(self.tip_index.map_or(0, |i| (i + 1) % 3));
            }
            self.tip_alpha = round5((0.5 - 0.5 * self.tip_phase.cos()).sqrt());
        }
        Ok(intents)
    }

    /// Produce copied semantic data. Admission availability never follows from a
    /// completed animation; pass the existing route owner's current availability.
    pub fn view(&self, admission_available: bool) -> StartMenuView {
        let wording = self.locale.wording();
        let last_life = self.last_capture.map_or_else(
            || LastLifeView {
                label: "",
                number: "\u{a0}".to_owned(),
                suffix: "",
                label_alpha: 0.45,
            },
            |capture| LastLifeView {
                label: wording.final_length,
                number: capture.score_text().expect("validated at acceptance"),
                suffix: if capture.sample.score > 1000. {
                    "!"
                } else {
                    ""
                },
                label_alpha: 0.45,
            },
        );
        let disabled = self.phase != MenuPhase::Login || self.champion || !admission_available;
        StartMenuView {
            phase: self.phase,
            visible: self.phase != MenuPhase::World
                && (self.phase != MenuPhase::DeathHold || self.transition < 1.),
            disabled,
            play_disabled: !self.play_enabled(admission_available),
            champion_pending: self.champion,
            ordinary_controls_visible: !self.champion,
            nickname: self.nickname.clone(),
            wording,
            play_label: if self.replay {
                wording.play_again
            } else {
                wording.play
            },
            last_life,
            transition: self.transition,
            login_alpha: 1. - self.transition,
            world_alpha: self.transition,
            play_return_alpha: if self.return_play
                || matches!(self.phase, MenuPhase::Returning | MenuPhase::DeathHold)
            {
                1. - 0.5 * (6. * self.transition).clamp(0., 1.)
            } else {
                1.
            },
            logo_alpha: self.logo_alpha,
            logo_reveal: self.logo_reveal,
            nickname_alpha: (6. * self.nickname_progress).min(1.),
            skin_alpha: (5. * (self.nickname_progress - 0.05)).clamp(0., 1.),
            play_alpha: (5. * (self.nickname_progress - 0.01)).clamp(0., 1.),
            tip: self.tip_index.map_or("\u{a0}", |i| wording.tips[i]),
            tips_visible: self.tips_active,
            tip_alpha: if self.tips_active { self.tip_alpha } else { 0. },
            hover_alpha: if self.play_enabled(admission_available) {
                self.hover_alpha
            } else {
                0.
            },
            press_alpha: if self.play_enabled(admission_available) {
                self.press_alpha
            } else {
                0.
            },
            spinner_visible: self.spinner_visible,
            spinner_time: self.spinner_time,
            spinner_alpha: self.spinner_alpha,
            // The source adds 0.1 INSIDE the rounding, after scaling only the
            // variable term by 1e5. Consequently the completed scale is 0.9.
            spinner_scale: (0.1
                + 0.9
                    * self.spinner_progress
                    * (1. + 2. * (1. - self.spinner_alpha).powi(2))
                    * 100_000.)
                .round()
                / 100_000.,
            quality: self.quality,
        }
    }
}

fn overlay(value: f64, active: bool) -> f64 {
    if active {
        (value + 0.33).min(1.)
    } else {
        (value - 0.2).max(0.)
    }
}
