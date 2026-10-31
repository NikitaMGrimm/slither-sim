//! Copied login projection shared by both browser adapters. DOM owns editing;
//! the menu owns normalization, clocks, geometry and deferred admission.
use crate::{
    menu_layout::{MenuElement as E, MenuLayout, MenuMetrics},
    menu_preferences::MenuQuality,
    start_page::{MenuAction, MenuLocale, StartMenuIntent, StartMenuState},
};
use wasm_bindgen::prelude::*;

/// Browser-owned instance of the shared login clock and intent policy.
#[wasm_bindgen]
pub struct LoginMenu {
    state: StartMenuState,
    layout: Option<MenuLayout>,
    extent: [f64; 2],
}

fn set(object: &js_sys::Object, name: &str, value: impl Into<JsValue>) -> Result<(), JsValue> {
    if js_sys::Reflect::set(object, &name.into(), &value.into())? {
        Ok(())
    } else {
        Err(JsValue::from_str("Cannot publish login view"))
    }
}

#[wasm_bindgen]
impl LoginMenu {
    /// Browser storage migration uses the same bounded record parser as native.
    /// The dev/reference host keeps its existing controls and legacy key.
    pub fn restore_settings(&mut self, record: Option<String>, legacy: Option<String>) {
        self.state.settings = crate::settings::SettingsState::new(
            crate::settings::SettingsPreferences::restore(record.as_deref(), legacy.as_deref()),
        );
    }
    /// Copied canonical record for an explicit dev/reference quality change.
    pub fn settings_record(&self) -> String {
        self.state.settings.preferences().serialize().into()
    }
    /// Requested world raster policy, independent of next-Play Quality.
    pub fn world_resolution(&self) -> String {
        if self.state.settings.preferences().resolution == crate::settings::WorldResolution::Native
        {
            "native"
        } else {
            "original"
        }
        .into()
    }
    /// Start an animated login with the supported locale and stored quality.
    #[wasm_bindgen(constructor)]
    pub fn new(locale: &str, quality: &str) -> Self {
        Self {
            state: StartMenuState::new(
                match locale.split('-').next().unwrap_or("") {
                    "de" => MenuLocale::German,
                    "fr" => MenuLocale::French,
                    "pt" => MenuLocale::Portuguese,
                    _ => MenuLocale::English,
                },
                true,
                MenuQuality::restore(Some(quality)),
            ),
            layout: None,
            extent: [0.; 2],
        }
    }

    /// Normalize a complete committed DOM edit.
    pub fn edit(&mut self, text: &str) -> String {
        self.state
            .edit_nickname(&text.encode_utf16().collect::<Vec<_>>());
        self.state.view(true).nickname
    }

    /// Returns only a best-effort storage value. Play is emitted by step().
    pub fn action(&mut self, kind: &str, composing: bool, admission: bool) -> Option<String> {
        let action = match kind {
            "play" if !composing => MenuAction::Play,
            "enter" => MenuAction::Enter { composing },
            "quality" => MenuAction::Quality,
            _ => return None,
        };
        match self.state.action(action, admission) {
            Some(StartMenuIntent::PersistQuality(value)) => Some(value.into()),
            _ => None,
        }
    }

    /// Observe pointer layers without granting Play admission.
    pub fn pointer(&mut self, hovered: bool, pressed: bool, admission: bool) {
        self.state.play_pointer(hovered, pressed, admission);
    }
    /// Current label for measurement before this callback's layout.
    pub fn play_label(&self) -> String {
        self.state.view(false).play_label.into()
    }

    /// Cancel a queued explicit click while preserving any old return fade.
    pub fn cancel_pending_play(&mut self) {
        self.state.cancel_pending_play();
    }

    /// Return after an explicitly observed transport failure.
    pub fn disconnected(&mut self) {
        self.state.disconnected();
    }

    /// Observe the current session only; stale generations are filtered by its owner.
    pub fn observe_session(&mut self, phase: &str) -> bool {
        use crate::session_policy::Phase;
        match phase {
            "failed" => self.state.attempt_ended(Phase::Failed),
            "disconnected" => self.state.attempt_ended(Phase::Disconnected),
            _ => false,
        }
    }

    /// Copy only committed session metadata and HudHistory's final capture.
    #[allow(clippy::too_many_arguments)]
    pub fn observe_life(
        &mut self,
        phase: &str,
        outcome: &str,
        playing: bool,
        death_pending: bool,
        return_active: bool,
        return_fraction: f64,
        score: Option<f64>,
        token: Option<u64>,
    ) -> Result<bool, JsValue> {
        use crate::{
            presentation::{HudFinal, HudSample, SnakeObjectToken},
            session_policy::Phase,
            start_page::MenuSession,
        };
        let phase = match phase {
            "ready" => Phase::Ready,
            "opening" => Phase::Opening,
            "challenge" => Phase::Challenge,
            "setup" => Phase::Setup,
            "playing" => Phase::Playing,
            "dead" => Phase::Dead,
            "victory" => Phase::Victory,
            "failed" => Phase::Failed,
            "disconnected" => Phase::Disconnected,
            _ => return Err(JsValue::from_str("Invalid login session")),
        };
        let outcome = match outcome {
            "" => None,
            "dead" => Some(Phase::Dead),
            "victory" => Some(Phase::Victory),
            _ => return Err(JsValue::from_str("Invalid login outcome")),
        };
        if !return_fraction.is_finite() || !(0. ..=1.).contains(&return_fraction) {
            return Err(JsValue::from_str("Invalid login fraction"));
        }
        let capture = match (score, token) {
            (Some(score), Some(token)) => Some(HudFinal {
                sample: HudSample {
                    score,
                    token: SnakeObjectToken::new(token)
                        .map_err(|_| JsValue::from_str("Invalid login capture"))?,
                },
                exclamation: score > 1000.,
            }),
            (None, None) => None,
            _ => return Err(JsValue::from_str("Invalid login capture")),
        };
        self.state
            .observe_session(
                MenuSession {
                    phase,
                    outcome,
                    playing,
                    death_pending,
                    return_active,
                    return_fraction,
                },
                capture,
            )
            .map_err(JsValue::from_str)
    }

    /// Advance one host callback and copy geometry, clocks and deferred effects.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        now: f64,
        width: f64,
        height: f64,
        label_width: f64,
        admission: bool,
        playing: bool,
    ) -> Result<JsValue, JsValue> {
        let intents = self
            .state
            .step_with_admission(now, false, admission)
            .map_err(JsValue::from_str)?;
        // The first accepted gameplay update, never the click or socket open,
        // starts the fade and lets the spinner retire on the shared clock.
        if playing {
            self.state.begin_outgoing();
        }
        let view = self.state.view(admission);
        let metrics = MenuMetrics {
            last_length_line_height: 18.,
            nickname_baseline_space: 4.,
            padded_play_label_width: label_width + 20.,
        };
        match self.layout.as_mut() {
            Some(layout) if self.extent == [width, height] => {
                layout
                    .update(view.transition, view.tips_visible, metrics)
                    .map_err(JsValue::from_str)?;
            }
            _ => {
                self.layout = Some(
                    MenuLayout::new(width, height, view.transition, view.tips_visible, metrics)
                        .map_err(JsValue::from_str)?,
                );
                self.extent = [width, height];
            }
        }
        let object = js_sys::Object::new();
        for (key, value) in [
            ("visible", view.visible),
            ("disabled", view.disabled),
            ("play_disabled", view.play_disabled),
            (
                "busy",
                view.phase == crate::start_page::MenuPhase::Connecting,
            ),
            ("ordinary_controls_visible", view.ordinary_controls_visible),
            ("champion_pending", view.champion_pending),
            ("spinner_visible", view.spinner_visible),
            ("focus", intents.contains(&StartMenuIntent::FocusNickname)),
        ] {
            set(&object, key, value)?;
        }
        for (key, value) in [
            ("alpha", view.login_alpha),
            ("world_alpha", view.world_alpha),
            ("last_label_alpha", view.last_life.label_alpha),
            (
                "logo_alpha",
                view.login_alpha * view.logo_alpha * view.logo_reveal,
            ),
            ("nickname_alpha", view.login_alpha * view.nickname_alpha),
            (
                "play_alpha",
                view.login_alpha * view.play_alpha * view.play_return_alpha,
            ),
            ("skin_alpha", view.login_alpha * view.skin_alpha),
            ("tip_alpha", view.login_alpha * view.tip_alpha),
            ("hover", view.hover_alpha),
            ("press", view.press_alpha),
            ("spinner_time", view.spinner_time),
            ("spinner_alpha", view.spinner_alpha),
            ("spinner_scale", view.spinner_scale),
            ("scale", self.layout.as_ref().unwrap().scale),
        ] {
            set(&object, key, value)?;
        }
        for (key, value) in [
            ("nickname", view.nickname.as_str()),
            ("last_label", view.last_life.label),
            ("last_number", view.last_life.number.as_str()),
            ("last_suffix", view.last_life.suffix),
            ("champion_label", view.wording.champion_pending),
            ("placeholder", view.wording.nickname),
            ("play_label", view.play_label),
            ("tip", view.tip),
            ("skin_label", view.wording.change_skin),
            ("quality_label", view.wording.graphics_quality),
            (
                "quality_value",
                match view.quality {
                    MenuQuality::High => view.wording.quality_high,
                    MenuQuality::Low => view.wording.quality_low,
                },
            ),
            ("connecting_label", view.wording.connecting),
            (
                "quality_caption",
                match view.quality {
                    MenuQuality::High => "High quality",
                    MenuQuality::Low => "Low quality",
                },
            ),
            (
                "quality",
                match view.quality {
                    MenuQuality::High => "high",
                    MenuQuality::Low => "low",
                },
            ),
        ] {
            set(&object, key, value)?;
        }
        for (key, element) in [
            ("logo", E::Logo),
            ("last", E::LastLength),
            ("tips", E::Tips),
            ("holder", E::NicknameHolder),
            ("input", E::Nickname),
            ("play", E::Play),
            ("skin", E::Skin),
            ("quality_rect", E::Quality),
            ("spinner", E::Spinner),
        ] {
            let r = self.layout.as_ref().unwrap().viewport_rect(element);
            let rect = js_sys::Array::new();
            for value in [r.x, r.y, r.width, r.height] {
                rect.push(&value.into());
            }
            set(&object, key, rect)?;
        }
        for intent in intents {
            if let StartMenuIntent::Play { nickname, quality } = intent {
                set(&object, "request", nickname)?;
                set(
                    &object,
                    "requested_quality",
                    match quality {
                        MenuQuality::High => "high",
                        MenuQuality::Low => "low",
                    },
                )?;
            }
        }
        Ok(object.into())
    }
}
