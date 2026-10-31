//! Thin host calls; JS schedules RAF and copies canvas extent/DPR only.
use crate::{
    frame::Frame,
    gpu::{SurfaceHost, instance},
    scene::{Driver, LAST_TICK, Scene},
};
use wasm_bindgen::prelude::*;

/// Same gameplay owner/effect protocol as the native host. No WebSocket or DOM policy in Rust.
#[wasm_bindgen]
pub struct LocalGameplay {
    host: GameplaySurface,
    game: crate::game::LocalGame,
    #[cfg(feature = "feel-trace")]
    trace: Option<slither_client::feel_trace::Recorder>,
}
// Single-variant in ordinary builds. The offline variant replaces only GPU I/O;
// LocalGame still prepares every resource and acknowledges every actual picture.
// Keep the existing inline production host instead of adding a heap allocation
// solely to shrink the feature-only empty sink variant.
// [Contract](../../../tools/feel-replay/README.md),
// [peer rules](../../../tools/feel-replay/browser-peers.md) and authored
// [fixture](../../../tools/feel-replay/browser-synthetic.json): replace GPU I/O only.
#[cfg_attr(feature = "feel-trace", allow(clippy::large_enum_variant))]
enum GameplaySurface {
    Gpu(SurfaceHost),
    #[cfg(feature = "feel-trace")]
    Cpu,
}
impl GameplaySurface {
    fn resize(&mut self, extent: [u32; 2]) -> crate::Result<()> {
        match self {
            Self::Gpu(host) => host.resize(extent),
            #[cfg(feature = "feel-trace")]
            Self::Cpu => Ok(()),
        }
    }
}
fn js_error(e: String) -> JsValue {
    JsValue::from_str(&e)
}
/// Explicit offline diagnostic only; does not mount or enable a game menu.
#[wasm_bindgen]
pub struct MenuSpecimen {
    host: SurfaceHost,
    canvas: web_sys::HtmlCanvasElement,
    menu: crate::menu::Menu,
    text: crate::text::TextCache,
}
#[wasm_bindgen]
impl MenuSpecimen {
    pub fn render(&mut self, dpr: f64, state: u8) -> std::result::Result<(), JsValue> {
        let extent = [self.canvas.width(), self.canvas.height()];
        let view = crate::menu_specimen::view(state).map_err(js_error)?;
        let layout =
            crate::menu_specimen::layout(&mut self.text, &view, extent, dpr).map_err(js_error)?;
        let mut p = self
            .menu
            .prepare(&mut self.text, &view, &layout, extent, dpr, None, 0)
            .map_err(js_error)?;
        self.host.resize(extent).map_err(js_error)?;
        if let Some(backdrop) = p.backdrop.take() {
            p.frame.draws.insert(0, backdrop);
        }
        self.host
            .renderer
            .sync_resources(&p.resources)
            .map_err(|e| js_error(e.into()))?;
        if !self.host.present(&p.frame).map_err(js_error)? {
            return Err(js_error("Menu specimen not presented".into()));
        }
        Ok(())
    }
}
#[wasm_bindgen]
pub async fn menu_specimen(
    canvas: web_sys::HtmlCanvasElement,
    dpr: f64,
    state: u8,
) -> std::result::Result<MenuSpecimen, JsValue> {
    let extent = [canvas.width(), canvas.height()];
    let i = instance();
    let surface = i
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|e| js_error(e.to_string()))?;
    let host = SurfaceHost::new(&i, surface, extent)
        .await
        .map_err(js_error)?;
    let mut specimen = MenuSpecimen {
        host,
        canvas,
        menu: crate::menu::Menu::default(),
        text: crate::text::TextCache::menu(),
    };
    specimen.render(dpr, state)?;
    Ok(specimen)
}
/// Shared desktop policy; JS supplies CSS content size without DPR multiplication.
#[wasm_bindgen]
pub fn game_backing_size(width: f64, height: f64) -> std::result::Result<Vec<u32>, JsValue> {
    crate::render::backing_size([width, height])
        .map(|v| v.to_vec())
        .map_err(js_error)
}
/// Startup-only local image batch. The game cannot see a partially decoded set.
#[wasm_bindgen]
pub struct CosmeticImages(crate::cosmetics::Images);
#[wasm_bindgen]
impl CosmeticImages {
    pub fn pixel_bytes(&self) -> usize {
        self.0.pixel_bytes()
    }
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self(crate::cosmetics::Images::default())
    }
    pub fn paths() -> Vec<String> {
        crate::cosmetics::paths()
    }
    pub fn add(&mut self, path: &str, bytes: &[u8]) -> std::result::Result<(), JsValue> {
        self.0.add(path, bytes).map_err(js_error)
    }
    pub fn manifest(&mut self, bytes: &[u8]) -> std::result::Result<(), JsValue> {
        self.0.manifest(bytes).map_err(js_error)
    }
    pub fn menu_paths(&self) -> Vec<String> {
        self.0.menu_paths()
    }
}
impl Default for CosmeticImages {
    fn default() -> Self {
        Self::new()
    }
}
impl LocalGameplay {
    fn checked<T>(
        &mut self,
        f: impl FnOnce(&mut crate::game::LocalGame) -> crate::Result<T>,
    ) -> std::result::Result<T, JsValue> {
        f(&mut self.game).map_err(|e| {
            self.game.fail(&e);
            js_error(e)
        })
    }
    fn pictures(&mut self) -> crate::Result<()> {
        for _ in 0..16 {
            let Some(p) = self.game.take_frame() else {
                return Ok(());
            };
            let presented = match &mut self.host {
                GameplaySurface::Gpu(host) => {
                    let session = self.game.login_session();
                    // A close without an accepted outcome returns straight to login.
                    let world_alpha = if session.playing || session.outcome.is_some() {
                        session.return_fraction
                    } else {
                        0.
                    };
                    let composed = if world_alpha < 1. {
                        let mut picture = p.clone();
                        crate::menu::MenuFrame::world_crossfade(
                            p.ui_frame().extent,
                            p.ui_frame().viewport_scale,
                            world_alpha,
                        )?
                        .append_to(&mut picture)?;
                        Some(picture)
                    } else {
                        None
                    };
                    // Keep the issued picture unchanged for a presentation retry.
                    let picture = composed.as_ref().unwrap_or(&p);
                    host.renderer.sync_resources(&picture.resources)?;
                    host.present_scaled_with_overlay(&picture.frame, picture.overlay.as_ref())
                }
                #[cfg(feature = "feel-trace")]
                GameplaySurface::Cpu => Ok(true),
            };
            match presented {
                Ok(true) => {
                    #[cfg(feature = "feel-trace")]
                    let picture = self.game.issued_feel_observation().map(|row| row.sequence);
                    self.game.presented(true)?;
                    #[cfg(feature = "feel-trace")]
                    if let (Some(trace), Some(sequence)) = (&self.trace, picture) {
                        trace.acknowledge_picture(sequence);
                    }
                }
                Ok(false) => {
                    self.game.retry_frame(p)?;
                    return Ok(());
                }
                Err(e) => {
                    self.game.presented(false)?;
                    return Err(e.into());
                }
            }
        }
        if self.game.has_frame() {
            Err("Frame drain budget".into())
        } else {
            Ok(())
        }
    }
}
#[wasm_bindgen]
impl LocalGameplay {
    pub fn phase(&self) -> String {
        self.game.phase().label().into()
    }
    pub fn login_view(&self) -> std::result::Result<JsValue, JsValue> {
        let session = self.game.login_session();
        let object = js_sys::Object::new();
        for (name, value) in [
            ("phase", session.phase.label().into()),
            (
                "outcome",
                session.outcome.map_or(JsValue::NULL, |p| p.label().into()),
            ),
            ("playing", session.playing.into()),
            ("deathPending", session.death_pending.into()),
            ("returnActive", session.return_active.into()),
            ("returnFraction", session.return_fraction.into()),
        ] {
            js_sys::Reflect::set(&object, &name.into(), &value)?;
        }
        if let Some(capture) = self.game.login_final() {
            js_sys::Reflect::set(&object, &"score".into(), &capture.sample.score.into())?;
            js_sys::Reflect::set(
                &object,
                &"ownToken".into(),
                &JsValue::from(capture.sample.token.serial()),
            )?;
        }
        Ok(object.into())
    }
    pub fn playing(&self) -> bool {
        self.game.phase() == slither_client::session_policy::Phase::Playing
    }
    pub fn requested_quality(&mut self, high: bool) {
        self.game.set_requested_quality(if high {
            slither_client::presentation::QualityMode::High
        } else {
            slither_client::presentation::QualityMode::Low
        });
    }
    #[wasm_bindgen(js_name=create)]
    pub async fn create(
        canvas: web_sys::HtmlCanvasElement,
        now_ms: f64,
    ) -> std::result::Result<LocalGameplay, JsValue> {
        let extent = [canvas.width(), canvas.height()];
        let i = instance();
        let surface = i
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self {
            host: GameplaySurface::Gpu(
                SurfaceHost::new(&i, surface, extent)
                    .await
                    .map_err(js_error)?,
            ),
            game: crate::game::LocalGame::new(now_ms).map_err(js_error)?,
            #[cfg(feature = "feel-trace")]
            trace: None,
        })
    }
    /// Offline CPU gate: same gameplay, resources and host protocol; no GPU device.
    #[cfg(feature = "feel-trace")]
    pub fn replay(now_ms: f64) -> std::result::Result<LocalGameplay, JsValue> {
        let trace = slither_client::feel_trace::Recorder::default();
        let mut game = crate::game::LocalGame::new(now_ms).map_err(js_error)?;
        game.record_feel(trace.clone());
        Ok(Self {
            host: GameplaySurface::Cpu,
            game,
            trace: Some(trace),
        })
    }
    /// Copied observations belong to their prepared candidate, never client().
    #[cfg(feature = "feel-trace")]
    pub fn take_feel_trace(&self) -> js_sys::Array {
        self.trace.as_ref().expect("replay owner").take_js()
    }
    /// Actual submitted and committed pictures, matched by recorder identity.
    #[cfg(feature = "feel-trace")]
    pub fn take_feel_pictures(&self) -> Vec<String> {
        self.trace
            .as_ref()
            .expect("replay owner")
            .take_pictures_js()
    }
    /// A1 completeness failures from the exact candidate's recorder.
    #[cfg(feature = "feel-trace")]
    pub fn take_feel_trace_errors(&self) -> Vec<String> {
        self.trace.as_ref().expect("replay owner").take_errors()
    }
    /// Observe the private factory RNG without adding an entropy provider seam.
    #[cfg(feature = "feel-trace")]
    pub fn take_feel_random(&self) -> js_sys::Array {
        self.trace
            .as_ref()
            .expect("replay owner")
            .take_random()
            .into_iter()
            .map(|(tag, value)| {
                let row = js_sys::Array::new();
                row.push(&tag.into());
                row.push(&value.into());
                row
            })
            .collect()
    }
    /// Session/quality metadata after the host has drained synchronous effects.
    #[cfg(feature = "feel-trace")]
    pub fn feel_metadata(&self, now: f64) -> Vec<f64> {
        let session = self.game.feel_session();
        let q = self.game.quality();
        let pending = self.game.feel_pending();
        vec![
            f64::from(session.lagging),
            f64::from(session.ping_pending),
            (now - session.last_ping_ms).max(0.),
            f64::from(q.high_quality),
            pending[1] as f64,
            pending[0] as f64,
        ]
    }
    pub fn adapter(&self) -> String {
        match &self.host {
            GameplaySurface::Gpu(host) => format!(
                "{:?}; MSAA{}",
                host.renderer.adapter_info, host.renderer.sample_count
            ),
            #[cfg(feature = "feel-trace")]
            GameplaySurface::Cpu => "offline CPU presentation sink".into(),
        }
    }
    pub fn cosmetics(
        &mut self,
        images: CosmeticImages,
        accessory: i32,
    ) -> std::result::Result<(), JsValue> {
        let own = if accessory == -1 {
            None
        } else {
            Some(
                u8::try_from(accessory)
                    .map_err(|_| js_error("Accessory must be -1 or 0..31".into()))?,
            )
        };
        let cosmetics =
            crate::cosmetics::Cosmetics::from_images(images.0, own).map_err(js_error)?;
        self.game.configure_cosmetics(cosmetics).map_err(js_error)
    }
    pub fn play(
        &mut self,
        endpoint: &str,
        nickname: &str,
        skin: u8,
    ) -> std::result::Result<(), JsValue> {
        self.checked(|g| {
            crate::game::loopback_endpoint(endpoint)?;
            g.play(endpoint, nickname, skin)
        })
    }
    pub fn stop(&mut self) {
        self.game.stop();
    }
    pub fn fail(&mut self) {
        self.game.fail("Browser host failure");
    }
    pub fn can_play(&self) -> bool {
        self.game.can_play()
    }
    pub fn hud(&self) -> String {
        self.game.hud_text()
    }
    pub fn effect_kind(&self) -> u8 {
        self.game.effect().map_or(0, |e| match e.kind {
            crate::game::EffectKind::Connect(_) => 1,
            crate::game::EffectKind::Send(_) => 2,
            crate::game::EffectKind::Close => 3,
        })
    }
    pub fn effect_id(&self) -> u64 {
        self.game.effect().map_or(0, |e| e.id)
    }
    pub fn effect_generation(&self) -> u32 {
        self.game.effect().map_or(0, |e| e.generation)
    }
    pub fn effect_url(&self) -> String {
        self.game
            .effect()
            .and_then(|e| {
                if let crate::game::EffectKind::Connect(u) = e.kind {
                    Some(u)
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
    pub fn effect_bytes(&self) -> Vec<u8> {
        self.game
            .effect()
            .and_then(|e| {
                if let crate::game::EffectKind::Send(b) = e.kind {
                    Some(b)
                } else {
                    None
                }
            })
            .unwrap_or_default()
    }
    pub fn acknowledge(&mut self, g: u32, id: u64, ok: bool) -> std::result::Result<(), JsValue> {
        self.checked(|s| s.acknowledge(g, id, ok))
    }
    pub fn opened(&mut self, g: u32, now: f64) -> std::result::Result<(), JsValue> {
        self.checked(|s| s.opened(g, now))
    }
    pub fn construct(&mut self, g: u32, id: u64, now: f64) -> std::result::Result<(), JsValue> {
        self.checked(|s| s.construct(g, id, now))
    }
    pub fn receive(&mut self, g: u32, b: &[u8], now: f64) -> std::result::Result<(), JsValue> {
        self.checked(|s| s.receive(g, b, now))
    }
    pub fn closed(&mut self, g: u32) {
        self.game.closed(g);
    }
    pub fn key(&mut self, code: &str, down: bool) -> bool {
        self.game.key(code, down)
    }
    pub fn blur(&mut self) {
        self.game.blur();
    }
    pub fn pointer(
        &mut self,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        down: bool,
    ) -> std::result::Result<(), JsValue> {
        self.checked(|g| g.pointer(x, y, w, h, down))
    }
    pub fn mouse_up(&mut self) {
        self.game.mouse_up();
    }
    /// Receive/Setup flush does not advance the gameplay clock.
    pub fn flush(
        &mut self,
        now: f64,
        width: u32,
        height: u32,
        dpr: f64,
    ) -> std::result::Result<(), JsValue> {
        let result = (|| {
            self.game.observe_time(now)?;
            self.host.resize([width, height])?;
            if matches!(self.host, GameplaySurface::Gpu(_)) {
                // HUD owns its opacity; the login cover must precede this layer.
                self.game.bank.overlay_viewport = Some(([width, height], dpr));
            }
            self.game.viewport([width, height], dpr)?;
            self.pictures()
        })();
        result.map_err(|e| {
            self.game.fail(&e);
            js_error(e)
        })
    }
    pub fn frame(
        &mut self,
        now: f64,
        width: u32,
        height: u32,
        dpr: f64,
    ) -> std::result::Result<(), JsValue> {
        self.flush(now, width, height, dpr)?;
        self.checked(|g| g.begin_frame(now, [width, height], dpr))?;
        self.pictures().map_err(|e| {
            self.game.fail(&e);
            js_error(e)
        })
    }
}

#[wasm_bindgen]
pub struct GameplaySlice {
    host: SurfaceHost,
    scene: Scene,
    driver: Driver,
    frame: Option<Frame>,
}
#[wasm_bindgen]
impl GameplaySlice {
    /// No Canvas/software fallback. Missing WebGPU is an explicit initialization error.
    #[wasm_bindgen(js_name=create)]
    pub async fn create(
        canvas: web_sys::HtmlCanvasElement,
    ) -> std::result::Result<GameplaySlice, JsValue> {
        let extent = [canvas.width(), canvas.height()];
        let i = instance();
        let surface = i
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let mut host = SurfaceHost::new(&i, surface, extent)
            .await
            .map_err(|e| JsValue::from_str(&e))?;
        let scene = Scene::new().map_err(|e| JsValue::from_str(&e))?;
        for r in &scene.resources {
            host.renderer
                .upload(r)
                .map_err(|e| JsValue::from_str(&e.to_string()))?;
        }
        Ok(Self {
            host,
            scene,
            driver: Driver::default(),
            frame: None,
        })
    }
    pub fn adapter(&self) -> String {
        format!("{:?}", self.host.renderer.adapter_info)
    }
    /// Same fixed ticks and packet schedule as the desktop host. No domain math in JS.
    pub fn frame(
        &mut self,
        now_ms: f64,
        width: u32,
        height: u32,
        dpr: f64,
    ) -> std::result::Result<bool, JsValue> {
        let mut run = || -> crate::Result<bool> {
            let extent = [width, height];
            self.host.resize(extent)?;
            if extent.contains(&0) {
                self.driver.pause();
                self.frame = None;
                return Ok(false);
            }
            let due = self.driver.due(now_ms)?;
            for _ in 0..due {
                if self.scene.tick() < LAST_TICK {
                    self.frame = Some(self.scene.next_frame(extent, dpr)?);
                }
            }
            if self
                .frame
                .as_ref()
                .is_none_or(|f| f.extent != extent || f.viewport_scale != dpr)
            {
                self.frame = Some(self.scene.draw_frame(extent, dpr)?);
            }
            if let Some(frame) = &self.frame
                && frame.extent == extent
            {
                return self.host.present(frame);
            }
            Ok(false)
        };
        run().map_err(|e| JsValue::from_str(&e))
    }
    pub fn tick(&self) -> u32 {
        self.scene.tick()
    }
    pub fn pause(&mut self) {
        self.driver.pause();
    }
}
