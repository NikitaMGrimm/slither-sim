//! One lazy native menu host. Closed menus do not translate or drain egui input.
use crate::{Result, egui_theme, gpu};
use egui::{Id, Key};
use winit::{
    event::{ElementState, WindowEvent},
    keyboard::{Key as NativeKey, ModifiersState, NamedKey},
    window::Window,
};

const ACTION_KEYS: [Key; 4] = [Key::Tab, Key::Enter, Key::Space, Key::Escape];

/// Window bookkeeping precedes input ownership, including while another menu owns input.
pub struct WindowInput {
    physical_cursor: Option<[f64; 2]>,
    dpr: f64,
    inside: bool,
    focused: bool,
    modifiers: ModifiersState,
    held: [bool; 4],
}
impl Default for WindowInput {
    fn default() -> Self {
        Self {
            physical_cursor: None,
            dpr: 1.,
            inside: false,
            focused: true,
            modifiers: ModifiersState::empty(),
            held: [false; 4],
        }
    }
}
impl WindowInput {
    pub fn physical_cursor(&self) -> Option<[f64; 2]> {
        self.physical_cursor
    }

    pub fn cursor(&self) -> [f64; 2] {
        // Legacy menus/gameplay retain the core's f32/floor mapping. The egui
        // adapter is seeded separately with the untouched physical coordinates.
        self.physical_cursor.map_or([480., 270.], |p| {
            slither_client::desktop_input::mouse_client_position(p, self.dpr)
        })
    }
    pub fn observe(&mut self, event: &WindowEvent, dpr: f64) {
        self.dpr = dpr;
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.physical_cursor = Some([position.x, position.y]);
                self.inside = true;
            }
            WindowEvent::CursorLeft { .. } => self.inside = false,
            WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
            WindowEvent::Focused(focused) => {
                self.focused = *focused;
                if !focused {
                    self.held = [false; 4];
                    self.modifiers = ModifiersState::empty();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => self.key(&event.logical_key, event.state),
            _ => {}
        }
    }
    pub fn key(&mut self, key: &NativeKey, state: ElementState) {
        let index = match key {
            NativeKey::Named(NamedKey::Tab) => Some(0),
            NativeKey::Named(NamedKey::Enter) => Some(1),
            NativeKey::Named(NamedKey::Space) => Some(2),
            NativeKey::Named(NamedKey::Escape) => Some(3),
            _ => None,
        };
        if let Some(i) = index {
            self.held[i] = state == ElementState::Pressed;
        }
    }
}

struct Session {
    context: egui::Context,
    adapter: Option<egui_winit::State>,
    input: egui::RawInput,
    output: Option<egui::PlatformOutput>,
    textures: egui::TexturesDelta,
    meshes: Vec<egui::ClippedPrimitive>,
    renderer: Option<egui_wgpu::Renderer>,
    held: [bool; 4],
    reveal_focus: bool,
    extent: [u32; 2],
    dpr: f32,
}
impl Drop for Session {
    fn drop(&mut self) {
        // CPU-only preparation and shutdown may skip the final GPU submission.
        self.textures.clear();
    }
}

#[derive(Default)]
pub struct NativeEguiHost {
    pub window: WindowInput,
    session: Option<Session>,
    active: bool,
    now: f64,
    error: Option<String>,
    restore_cursor: bool,
}
impl NativeEguiHost {
    pub fn active(&self) -> bool {
        self.active
    }
    pub fn initialized(&self) -> bool {
        self.session.is_some()
    }
    pub fn context(&self) -> Option<&egui::Context> {
        self.session.as_ref().map(|s| &s.context)
    }
    pub fn step(&mut self, now: f64) {
        self.now = now;
    }
    pub fn set_active(&mut self, active: bool) {
        if self.active == active {
            return;
        }
        self.active = active;
        self.restore_cursor = !active;
        if let Some(s) = &mut self.session {
            s.adapter = None;
            s.input.events.clear();
            s.held = self.window.held;
            s.context.input_mut(|i| {
                i.pointer = Default::default();
                i.keys_down.clear();
            });
            s.context.memory_mut(|m| m.stop_text_input());
            if !active {
                s.meshes.clear();
                s.output = None;
            }
        }
        if active {
            let _ = self.ensure();
        }
    }
    fn ensure(&mut self) -> Option<&mut Session> {
        if !self.active || self.error.is_some() {
            return None;
        }
        if self.session.is_none() {
            let context = egui::Context::default();
            if let Err(error) = egui_theme::install(&context) {
                self.error = Some(error);
                return None;
            }
            self.session = Some(Session {
                context,
                adapter: None,
                input: Default::default(),
                output: None,
                textures: Default::default(),
                meshes: Vec::new(),
                renderer: None,
                held: self.window.held,
                reveal_focus: true,
                extent: [960, 540],
                dpr: 1.,
            });
        }
        self.session.as_mut()
    }
    pub fn check_error(&self) -> Result<()> {
        self.error.as_ref().map_or(Ok(()), |e| Err(e.clone()))
    }
    pub fn cancel_pointer(&mut self) {
        if !self.active {
            return;
        }
        if let Some(s) = &mut self.session {
            s.context.input_mut(|i| i.pointer = Default::default());
            s.input.events.clear();
        }
    }
    pub fn focus(&mut self, id: Id) {
        if let Some(s) = self.ensure() {
            s.context.memory_mut(|m| m.request_focus(id));
            s.reveal_focus = true;
        }
    }
    pub fn input(&mut self, input: egui::RawInput) {
        if let Some(s) = self.ensure() {
            s.input.append(input);
        }
    }
    fn adapter(&mut self, window: &Window) -> Option<&mut egui_winit::State> {
        let modifiers = self.window.modifiers;
        let cursor = self.window.physical_cursor;
        let inside = self.window.inside;
        let focused = self.window.focused;
        let s = self.ensure()?;
        if s.adapter.is_none() {
            let mut adapter = egui_winit::State::new(
                s.context.clone(),
                egui::ViewportId::ROOT,
                window,
                Some(window.scale_factor() as f32),
                None,
                None,
            );
            let _ = adapter
                .on_window_event(window, &WindowEvent::Focused(focused && window.has_focus()));
            let _ =
                adapter.on_window_event(window, &WindowEvent::ModifiersChanged(modifiers.into()));
            if inside && let Some(cursor) = cursor {
                let _ = adapter.on_window_event(
                    window,
                    &WindowEvent::CursorMoved {
                        device_id: winit::event::DeviceId::dummy(),
                        position: winit::dpi::PhysicalPosition::new(cursor[0], cursor[1]),
                    },
                );
            }
            s.adapter = Some(adapter);
        }
        s.adapter.as_mut()
    }
    /// The window closure is not evaluated while closed. This is also the headless
    /// seam for verifying that inactive callbacks never enter egui-winit.
    pub fn route_window_event<R>(
        &mut self,
        event: &WindowEvent,
        dpr: f64,
        translate: impl FnOnce(&mut Self) -> R,
    ) -> Option<R> {
        self.window.observe(event, dpr);
        if self.active {
            Some(translate(self))
        } else {
            None
        }
    }
    pub fn window_event<'a>(
        &mut self,
        event: &WindowEvent,
        dpr: f64,
        window: impl FnOnce() -> &'a Window,
    ) -> egui_winit::EventResponse {
        self.route_window_event(event, dpr, |host| {
            let window = window();
            if matches!(
                event,
                WindowEvent::Focused(false)
                    | WindowEvent::Resized(_)
                    | WindowEvent::ScaleFactorChanged { .. }
            ) {
                host.cancel_pointer();
            }
            host.adapter(window).map_or(
                egui_winit::EventResponse {
                    consumed: true,
                    repaint: true,
                },
                |adapter| adapter.on_window_event(window, event),
            )
        })
        .unwrap_or(egui_winit::EventResponse {
            consumed: false,
            repaint: false,
        })
    }
    pub fn take_window_input<'a>(
        &mut self,
        window: impl FnOnce() -> &'a Window,
    ) -> Option<egui::RawInput> {
        if !self.active {
            return None;
        }
        let window = window();
        Some(self.adapter(window)?.take_egui_input(window))
    }
    pub fn platform_output(&mut self, window: &Window) {
        if !self.active {
            if std::mem::take(&mut self.restore_cursor) {
                window.set_cursor(winit::window::CursorIcon::Default);
            }
            return;
        }
        let Some(s) = &mut self.session else {
            return;
        };
        let Some(output) = s.output.take() else {
            return;
        };
        if let Some(adapter) = &mut s.adapter {
            adapter.handle_platform_output(window, output);
        }
    }
    pub fn frame(
        &mut self,
        extent: [u32; 2],
        dpr: f64,
        order: &[Id],
        mut draw: impl FnMut(&egui::Context, bool),
    ) {
        let now = self.now;
        let Some(s) = self.ensure() else {
            return;
        };
        s.extent = extent;
        s.dpr = dpr as f32;
        let mut input = s.input.take();
        let mut reveal = std::mem::take(&mut s.reveal_focus);
        let mut focus = s.context.memory(|m| m.focused());
        // egui-winit deliberately emits repeat:false for repeated down events.
        // Detect held keys before custom traversal and before egui infers repeats.
        input.events.retain(|event| {
            if let egui::Event::WindowFocused(false) = event {
                s.held = [false; 4];
            }
            if let egui::Event::Key {
                key,
                pressed,
                repeat,
                modifiers,
                ..
            } = event
            {
                if let Some(i) = ACTION_KEYS.iter().position(|k| k == key) {
                    let held = s.held[i];
                    s.held[i] = *pressed;
                    if *pressed && (held || *repeat) {
                        return false;
                    }
                }
                if *key == Key::Tab && !order.is_empty() {
                    if *pressed {
                        let index = focus.and_then(|id| order.iter().position(|v| *v == id));
                        let next = match index {
                            Some(i) => {
                                (i + if modifiers.shift { order.len() - 1 } else { 1 })
                                    % order.len()
                            }
                            None => {
                                if modifiers.shift {
                                    order.len() - 1
                                } else {
                                    0
                                }
                            }
                        };
                        focus = Some(order[next]);
                        reveal = true;
                    }
                    return false;
                }
            }
            true
        });
        if input
            .events
            .iter()
            .any(|e| matches!(e, egui::Event::WindowFocused(false)))
        {
            s.context.input_mut(|i| {
                i.pointer = Default::default();
                i.keys_down.clear();
            });
        }
        if reveal && let Some(id) = focus {
            s.context.memory_mut(|m| m.request_focus(id));
        }
        input.screen_rect = Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(extent[0] as f32 / s.dpr, extent[1] as f32 / s.dpr),
        ));
        input.time = Some(now / 1000.);
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(s.dpr);
        let mut output = s.context.run_pass(input, |ctx| draw(ctx, reveal));
        s.textures
            .append(std::mem::take(&mut output.textures_delta));
        s.meshes = s
            .context
            .tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        s.output = Some(output.platform_output);
    }
    pub fn paint(&mut self, gpu: &gpu::Renderer, target: &wgpu::TextureView) {
        if !self.active {
            return;
        }
        let Some(s) = &mut self.session else {
            return;
        };
        let renderer = s.renderer.get_or_insert_with(|| {
            egui_wgpu::Renderer::new(
                &gpu.device,
                gpu.format,
                egui_wgpu::RendererOptions {
                    dithering: false,
                    ..Default::default()
                },
            )
        });
        for (id, deltas) in &s.textures.set {
            for delta in deltas {
                renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: s.extent,
            pixels_per_point: s.dpr,
        };
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let commands =
            renderer.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &s.meshes, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("native egui menus"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            renderer.render(&mut pass.forget_lifetime(), &s.meshes, &screen);
        }
        gpu.queue
            .submit(commands.into_iter().chain([encoder.finish()]));
        for id in &s.textures.free {
            renderer.free_texture(id);
        }
        s.textures.clear();
    }
}
