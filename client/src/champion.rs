//! Champion composition/policy, admitted only by committed session victory.
use crate::{
    menu_controls::*,
    menu_input::{EditorId, TextRule},
    menu_theme::ControlStyle,
    session_policy::{Phase, View},
};

/// Stable message control.
pub const MESSAGE: ControlId = ControlId(2);
/// Stable Save action.
pub const SAVE: ControlId = ControlId(3);
/// Caller-clocked champion handoff. No rank, socket or independent death timer.
#[derive(Clone, Debug, Default)]
pub struct ChampionState {
    generation: Option<u32>,
    finished: bool,
    alpha: f64,
    previous: Option<f64>,
    ending: bool,
    ordinary_alpha: f64,
}
impl ChampionState {
    /// Current editor is mounted.
    pub fn visible(&self) -> bool {
        self.generation.is_some() && !self.finished
    }
    /// The host may restore ordinary login after the sampled exit.
    pub fn finished(&self) -> bool {
        self.finished
    }
    /// Sampled ordinary-control fade after the editor exits.
    pub fn ordinary_alpha(&self) -> f64 {
        if self.generation.is_none() {
            1.
        } else {
            self.ordinary_alpha
        }
    }
    /// Observe committed session state; return true on a fresh victory handoff.
    pub fn observe(&mut self, generation: u32, session: &View) -> bool {
        if self.generation.is_some_and(|g| g != generation) {
            *self = Self::default();
        }
        // Terminal transport ends the handoff without claiming it was saved.
        // The existing menu/session return still controls login admission.
        self.ending = session.champion_acknowledged
            || matches!(session.phase, Phase::Failed | Phase::Disconnected);
        if self.generation.is_none() && session.outcome == Some(Phase::Victory) {
            self.generation = Some(generation);
            return true;
        }
        false
    }
    /// Sample fades from the same core clock used by menu callbacks.
    pub fn step(&mut self, now: f64, return_alpha: f64) -> Result<(), &'static str> {
        if !now.is_finite() || self.previous.is_some_and(|p| now < p) {
            return Err("Invalid champion clock");
        }
        let delta = self.previous.map_or(0., |p| ((now - p) / 8.).clamp(0., 5.));
        self.previous = Some(now);
        if self.finished {
            self.ordinary_alpha = (self.ordinary_alpha + 0.02 * delta).min(1.);
        }
        if self.visible() {
            if self.ending {
                self.alpha = (self.alpha - 0.02 * delta).max(0.);
                if self.alpha == 0. {
                    self.finished = true;
                }
            } else {
                self.alpha = return_alpha.clamp(0., 1.);
            }
        }
        Ok(())
    }
    /// Build the close-reference composition. The field's content is 420x24
    /// inside a 444px holder; long text scrolls rather than shrinking.
    pub fn panel(&self, session: &View, viewport: [f64; 2]) -> PanelView {
        let scale = (viewport[1] / 560.)
            .min(1.)
            .min((viewport[0] / 500.).min(1.))
            .max(0.1);
        let mut placement = Placement::rect(-240., -45., 480., 240.);
        placement.anchor = [0.5, 0.5];
        placement.origin = [0.5, 0.5];
        placement.scale = scale;
        let enabled = self.visible()
            && !self.ending
            && !session.champion_sent
            && !session.close_requested
            && !matches!(session.phase, Phase::Failed | Phase::Disconnected);
        let control = |id, name: &str, y, w, h, style, alpha, tab_order, kind| Control {
            id,
            placement: Placement::rect((480. - w) / 2., y, w, h),
            clip: None,
            visible: true,
            enabled,
            alpha,
            name: name.into(),
            tab_order,
            style,
            kind,
        };
        PanelView {
            id: 1,
            generation: u64::from(self.generation.unwrap_or(0)),
            placement,
            visible: self.visible(),
            enabled,
            alpha: self.alpha,
            modal: true,
            initial_focus: Some(MESSAGE),
            restore_focus: None,
            controls: vec![
                control(
                    ControlId(0),
                    "Champion",
                    0.,
                    480.,
                    32.,
                    ControlStyle::Heading,
                    1.,
                    None,
                    ControlKind::Label {
                        text: "You're the new champion!".into(),
                    },
                ),
                control(
                    ControlId(1),
                    "Instruction",
                    42.,
                    480.,
                    28.,
                    ControlStyle::HelpText,
                    0.6,
                    None,
                    ControlKind::Label {
                        text: "Enter a message to share with everyone!".into(),
                    },
                ),
                control(
                    MESSAGE,
                    "Victory message",
                    88.,
                    444.,
                    48.,
                    ControlStyle::Field,
                    1.,
                    Some(0),
                    ControlKind::TextField {
                        editor: EditorId::Champion,
                        placeholder: String::new(),
                        rule: TextRule::Champion,
                        inset: [12., 12.],
                    },
                ),
                control(
                    SAVE,
                    if session.champion_acknowledged {
                        "Message saved"
                    } else {
                        "Save Message"
                    },
                    164.,
                    180.,
                    47.,
                    ControlStyle::PrimaryButton,
                    1.,
                    Some(1),
                    ControlKind::Button {
                        label: if session.champion_acknowledged {
                            "Message saved"
                        } else {
                            "Save Message"
                        }
                        .into(),
                    },
                ),
            ],
        }
    }
    /// Validate the copied envelope. Transport rechecks authority separately.
    pub fn save_intent(&self, panel: &PanelView, intent: ControlIntent) -> Option<Activation> {
        (panel.visible
            && panel.enabled
            && intent.panel == panel.id
            && intent.generation == panel.generation
            && (intent.control == SAVE
                || intent.control == MESSAGE && intent.activation == Activation::Enter))
            .then_some(intent.activation)
    }
}
