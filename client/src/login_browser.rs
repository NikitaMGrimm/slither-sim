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
        let intents = self.state.step(now, false).map_err(JsValue::from_str)?;
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
        if self.layout.is_none() || self.extent != [width, height] {
            self.layout = Some(
                MenuLayout::new(width, height, view.transition, view.tips_visible, metrics)
                    .map_err(JsValue::from_str)?,
            );
            self.extent = [width, height];
        } else {
            self.layout
                .as_mut()
                .expect("initialized extent")
                .update(view.transition, view.tips_visible, metrics)
                .map_err(JsValue::from_str)?;
        }
        let object = js_sys::Object::new();
        for (key, value) in [
            ("visible", view.visible),
            ("disabled", view.disabled),
            ("spinner_visible", view.spinner_visible),
            ("focus", intents.contains(&StartMenuIntent::FocusNickname)),
        ] {
            set(&object, key, value)?;
        }
        for (key, value) in [
            ("alpha", view.login_alpha),
            ("world_alpha", view.world_alpha),
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
