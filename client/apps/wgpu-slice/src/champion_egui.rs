//! Native champion editor. The shared core owns the handoff policy, layout and
//! message rule; egui resources and input belong to the native host, and the
//! transport seam stays in `LocalGame::save_champion`.
use crate::{
    Result,
    egui_panel::{self as widgets, PressGuard, rect},
    egui_theme as theme,
};
use egui::{Id, Rect, Sense};
use slither_client::{
    champion::{ChampionState, MESSAGE, SAVE},
    menu_controls::{Activation, Control, ControlId, ControlIntent, ControlKind, PanelView},
    menu_input::TextRule,
    menu_theme::MenuTheme,
    presentation::TextStyle,
    session_policy::View,
};

pub fn widget_id(id: ControlId) -> Id {
    Id::unique(("champion", id.0))
}

#[derive(Default)]
pub struct ChampionUi {
    pub message: String,
    pub controls: Vec<(ControlId, Rect)>,
    pub keyboard_focus: bool,
    pub composing: bool,
    composition: Option<theme::Composition>,
    request_focus: Option<ControlId>,
    generation: Option<u64>,
    press: PressGuard,
    /// Save's sampled hover and press overlays, stepped on the core's clock.
    pub fade: [f64; 2],
    save_state: [bool; 2],
}
impl ChampionUi {
    pub fn begin_composition(&mut self, ctx: &egui::Context) {
        if self.composition.is_none() {
            self.composition = Some(theme::Composition::begin(
                ctx,
                widget_id(MESSAGE),
                &self.message,
            ));
        }
        self.composing = true;
    }
    pub fn cancel_composition(&mut self, ctx: &egui::Context) {
        if let Some(snapshot) = self.composition.take() {
            self.message = snapshot.cancel(ctx, widget_id(MESSAGE));
        }
        self.composing = false;
    }
    /// Linear per-callback overlay damping.
    fn step(&mut self, tokens: MenuTheme) {
        for (i, (active, rate)) in [
            (self.save_state[0], tokens.hover_rate),
            (self.save_state[0] && self.save_state[1], tokens.press_rate),
        ]
        .into_iter()
        .enumerate()
        {
            self.fade[i] = if active {
                (self.fade[i] + rate).min(1.)
            } else {
                (self.fade[i] - tokens.release_rate).max(0.)
            };
        }
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        panel: &PanelView,
        viewport: [f64; 2],
    ) -> Result<Vec<ControlIntent>> {
        self.controls.clear();
        let mut intents = Vec::new();
        if !panel.visible {
            self.generation = None;
            return Ok(intents);
        }
        let resolved = panel.resolve(viewport).map_err(str::to_owned)?;
        let tokens = MenuTheme::default();
        let intent = |control, activation| ControlIntent {
            checked: None,
            panel: panel.id,
            generation: panel.generation,
            control,
            activation,
        };
        ctx.input(|i| {
            if i.pointer.primary_pressed() {
                self.keyboard_focus = false;
            }
        });
        // A fresh handoff starts empty on the message field, with no undo
        // history from an earlier victory.
        if self.generation != Some(panel.generation) {
            self.generation = Some(panel.generation);
            self.request_focus = panel.initial_focus;
            self.press.invalidate();
            egui::text_edit::TextEditState::default().store(ctx, widget_id(MESSAGE));
        }
        let suppress = self.press.suppress(ctx);
        let focused = ctx.memory(|m| m.focused()).and_then(|id| {
            panel
                .controls
                .iter()
                .map(|c| c.id)
                .find(|c| widget_id(*c) == id)
        });
        if panel.enabled && focused == Some(MESSAGE) {
            let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
            // Composition owns Enter even when the same pass also contains its commit.
            let ime = self.composing
                || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
            if enter || ime {
                // A single-line TextEdit surrenders focus on Enter; the field keeps it.
                ctx.input_mut(|i| {
                    i.events.retain(|e| {
                        !matches!(
                            e,
                            egui::Event::Key {
                                key: egui::Key::Enter,
                                pressed: true,
                                ..
                            }
                        )
                    })
                });
            }
            if enter && !ime {
                intents.push(intent(MESSAGE, Activation::Enter));
            }
        }
        // egui-winit reports Space and Enter alike as a click, but an empty
        // message is sent by Space and refused by Enter.
        let space = ctx.input(|i| i.key_pressed(egui::Key::Space));
        let pressed = ctx.input(|i| i.pointer.primary_pressed());
        let area = ctx.input(|i| i.viewport_rect());
        let bounds = resolved
            .iter()
            .filter(|r| r.control.visible)
            .fold(Rect::NOTHING, |all, r| all.union(rect(r.rect)))
            .intersect(area);
        self.save_state = [false; 2];
        egui::Area::new(Id::unique("champion"))
            .fixed_pos(bounds.min)
            .order(egui::Order::Foreground)
            .movable(false)
            .fade_in(false)
            .sense(Sense::hover())
            .show(ctx, |ui| {
                ui.set_min_size(bounds.size());
                ui.set_clip_rect(area);
                ui.set_opacity(panel.alpha as f32);
                for r in resolved.iter().filter(|r| r.control.visible) {
                    let control = &r.control;
                    let enabled = panel.enabled && control.enabled;
                    let bounds = rect(r.rect);
                    let scale = r.scale as f32;
                    let mut style = tokens.text_style(control.style);
                    style.size *= r.scale;
                    let response = ui
                        .scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
                            ui.visuals_mut().disabled_alpha = 1.;
                            ui.multiply_opacity(control.alpha as f32);
                            if !enabled && !matches!(control.kind, ControlKind::Label { .. }) {
                                ui.multiply_opacity(tokens.disabled_opacity(control.style) as f32);
                            }
                            if !enabled {
                                ui.disable();
                            }
                            self.control(ui, control, bounds, scale, style, tokens)
                        })
                        .inner;
                    let Some(response) = response else {
                        continue;
                    };
                    if enabled
                        && (self.request_focus == Some(control.id)
                            || (pressed && response.is_pointer_button_down_on()))
                    {
                        response.request_focus();
                    }
                    self.controls.push((control.id, bounds));
                    if control.id != SAVE {
                        continue;
                    }
                    self.save_state = [response.hovered(), response.is_pointer_button_down_on()];
                    let pointer = response.clicked_by(egui::PointerButton::Primary);
                    if response.clicked() && !(pointer && suppress) {
                        intents.push(intent(
                            SAVE,
                            if pointer {
                                Activation::Pointer
                            } else if space {
                                Activation::Space
                            } else {
                                Activation::Enter
                            },
                        ));
                    }
                }
            });
        self.request_focus = None;
        // Only a press on a control or Tab moves focus: egui's surrender on a
        // click elsewhere must not leave the field, or typing goes nowhere.
        if let Some(id) = focused.filter(|id| widgets::focusable(panel, *id))
            && ctx.memory(|m| m.focused()).is_none()
        {
            ctx.memory_mut(|m| m.request_focus(widget_id(id)));
        }
        // The field lost a composition's focus: restore its snapshot before
        // this pass's actions run, so Save never reads provisional text.
        if self.composition.is_some() && ctx.memory(|m| m.focused()) != Some(widget_id(MESSAGE)) {
            self.cancel_composition(ctx);
        }
        self.press.settle(ctx);
        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                        self.composing = !text.is_empty() && self.composition.is_some()
                    }
                    egui::Event::Ime(egui::ImeEvent::Commit(_))
                    | egui::Event::WindowFocused(false) => self.composing = false,
                    _ => {}
                }
            }
        });
        if !self.composing {
            self.message = TextRule::Champion
                .normalize_units(&self.message.encode_utf16().collect::<Vec<_>>());
            theme::settle_text(
                ctx,
                widget_id(MESSAGE),
                &self.message,
                self.composition.take(),
            );
        }
        Ok(intents)
    }
    fn control(
        &mut self,
        ui: &mut egui::Ui,
        control: &Control,
        r: Rect,
        scale: f32,
        style: TextStyle,
        tokens: MenuTheme,
    ) -> Option<egui::Response> {
        let id = widget_id(control.id);
        match &control.kind {
            ControlKind::Label { text } => {
                widgets::em_middle_label(ui.painter(), text, r, style);
                None
            }
            ControlKind::TextField {
                placeholder, inset, ..
            } => Some(widgets::text_field(
                ui,
                control,
                &mut self.message,
                placeholder,
                *inset,
                r,
                scale,
                style,
                id,
                widgets::DEEP_FIELD,
                self.keyboard_focus,
                tokens,
            )),
            ControlKind::Button { label: text } => Some(widgets::button(
                ui,
                control,
                text,
                r,
                scale,
                style,
                id,
                self.keyboard_focus,
                Some(self.fade.map(|v| v as f32)),
                tokens,
            )),
            _ => None,
        }
    }
}

/// Policy and the pending submission the host forwards to the game.
pub struct NativeChampion {
    pub ui: ChampionUi,
    pub policy: ChampionState,
    pub panel: PanelView,
    pub viewport: [f64; 2],
    submission: Option<(u32, Vec<u16>, Activation)>,
}
impl NativeChampion {
    pub fn new(session: &View) -> Self {
        let policy = ChampionState::default();
        Self {
            ui: ChampionUi::default(),
            panel: policy.panel(session, [960., 540.]),
            policy,
            viewport: [960., 540.],
            submission: None,
        }
    }
    pub fn observe(&mut self, game: &crate::game::LocalGame) {
        let session = game.champion_session();
        if self.policy.observe(game.generation(), &session) {
            self.ui = ChampionUi::default();
            self.submission = None;
        }
        self.panel = self.policy.panel(&session, self.viewport);
    }
    pub fn step(&mut self, now: f64, alpha: f64, session: &View) -> Result<()> {
        self.policy.step(now, alpha).map_err(str::to_owned)?;
        self.panel = self.policy.panel(session, self.viewport);
        self.ui.step(MenuTheme::default());
        Ok(())
    }
    /// Apply one pass's activations against the panel that produced them.
    pub fn apply(&mut self, intents: Vec<ControlIntent>) {
        for intent in intents {
            if let Some(activation) = self.policy.save_intent(&self.panel, intent) {
                self.submission = Some((
                    self.panel.generation as u32,
                    self.ui.message.encode_utf16().collect(),
                    activation,
                ));
            }
        }
    }
    pub fn take_submission(&mut self) -> Option<(u32, Vec<u16>, Activation)> {
        self.submission.take()
    }
}
