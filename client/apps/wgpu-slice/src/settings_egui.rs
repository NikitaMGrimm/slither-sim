//! Native Settings trial. The shared settings owner still validates and persists changes.
use crate::{Result, gpu};
use egui::{Color32, Id, Rect, RichText, Sense, Stroke, Vec2, pos2, vec2};
use slither_client::{
    menu_controls::{Activation, ControlId},
    menu_preferences::MenuQuality,
    menu_theme::MenuTheme,
    settings::{self, SettingsState, WorldResolution},
};

pub struct SettingsUi {
    pub context: egui::Context,
    pub controls: Vec<(ControlId, Rect, bool)>,
    pub panel_rect: Rect,
    input: egui::RawInput,
    output: Option<egui::PlatformOutput>,
    textures: egui::TexturesDelta,
    renderer: Option<egui_wgpu::Renderer>,
    meshes: Vec<egui::ClippedPrimitive>,
    alpha: f64,
    target: bool,
    now: f64,
    initial_focus: bool,
    extent: [u32; 2],
    dpr: f32,
}
impl SettingsUi {
    pub fn new() -> Result<Self> {
        let context = egui::Context::default();
        let theme = MenuTheme::default();
        let mut fonts = egui::FontDefinitions::empty();
        for (name, role) in [
            ("body", slither_client::menu_theme::ControlStyle::HelpText),
            ("bold", slither_client::menu_theme::ControlStyle::Heading),
        ] {
            let bytes = crate::text::TextCache::native_font_bytes(theme.text_style(role))?;
            fonts
                .font_data
                .insert(name.into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .insert(egui::FontFamily::Name(name.into()), vec![name.into()]);
        }
        fonts
            .families
            .insert(egui::FontFamily::Proportional, vec!["body".into()]);
        fonts
            .families
            .insert(egui::FontFamily::Monospace, vec!["body".into()]);
        context.set_fonts(fonts);
        context.options_mut(|options| options.zoom_with_keyboard = false);
        context.set_theme(egui::Theme::Dark);
        context.style_mut_of(egui::Theme::Dark, |style| {
            style.visuals = egui::Visuals::dark();
            style.visuals.override_text_color = Some(Color32::from_rgb(
                theme.text[0],
                theme.text[1],
                theme.text[2],
            ));
            style.spacing.item_spacing = vec2(12., 12.);
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(18.));
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(16.));
            style.animation_time = 0.;
        });
        Ok(Self {
            context,
            controls: Vec::new(),
            panel_rect: Rect::NOTHING,
            input: Default::default(),
            output: None,
            textures: Default::default(),
            renderer: None,
            meshes: Vec::new(),
            alpha: 0.,
            target: false,
            now: 0.,
            initial_focus: false,
            extent: [960, 540],
            dpr: 1.,
        })
    }
    pub fn active(&self) -> bool {
        self.target || self.alpha > 0.
    }
    pub fn alpha(&self) -> f64 {
        self.alpha
    }
    pub fn cancel_pointer(&mut self) {
        self.context.input_mut(|i| i.pointer = Default::default());
        self.input.events.clear();
    }
    pub fn sync(&mut self, state: &SettingsState) {
        self.alpha = state.alpha();
        let visible = state.is_open();
        if visible != self.target {
            self.target = visible;
            self.initial_focus = visible;
            self.cancel_pointer();
            self.context.memory_mut(|m| m.stop_text_input());
        }
    }
    pub fn step(&mut self, now: f64, state: &SettingsState) {
        self.now = now;
        self.sync(state);
    }
    pub fn input(&mut self, input: egui::RawInput) {
        self.input.append(input);
    }
    pub fn platform_output(&mut self) -> Option<egui::PlatformOutput> {
        self.output.take()
    }
    pub fn focus(&mut self, control: ControlId) {
        self.context
            .memory_mut(|m| m.request_focus(widget_id(control)));
        self.initial_focus = false;
    }
    pub fn focused(&self) -> Option<ControlId> {
        [settings::QUALITY, settings::RESOLUTION, settings::DONE]
            .into_iter()
            .find(|id| self.context.memory(|m| m.has_focus(widget_id(*id))))
    }
    pub fn frame(&mut self, state: &mut SettingsState, extent: [u32; 2], dpr: f64) {
        self.extent = extent;
        self.dpr = dpr as f32;
        let area = vec2(extent[0] as f32 / self.dpr, extent[1] as f32 / self.dpr);
        let mut input = self.input.take();
        // egui transfers Tab past the last widget on the following frame. This
        // modal wraps immediately, so there is no frame without a focused control.
        let ids = [settings::QUALITY, settings::RESOLUTION, settings::DONE];
        let mut focus = self.focused();
        let mut tabbed = false;
        input.events.retain(|event| {
            if matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::Tab | egui::Key::Enter | egui::Key::Space | egui::Key::Escape,
                    pressed: true,
                    repeat: true,
                    ..
                }
            ) {
                return false;
            }
            if let egui::Event::Key {
                key: egui::Key::Tab,
                pressed,
                modifiers,
                ..
            } = event
            {
                if *pressed {
                    let index = focus
                        .and_then(|id| ids.iter().position(|item| *item == id))
                        .unwrap_or(0);
                    focus = Some(ids[(index + if modifiers.shift { 2 } else { 1 }) % ids.len()]);
                    tabbed = true;
                }
                false
            } else {
                true
            }
        });
        if tabbed && let Some(id) = focus {
            self.focus(id);
        }
        input.screen_rect = Some(Rect::from_min_size(pos2(0., 0.), area));
        input.time = Some(self.now / 1000.);
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(self.dpr);
        let mut actions = Vec::new();
        let mut controls = Vec::new();
        let initial = std::mem::take(&mut self.initial_focus);
        let mut panel_rect = Rect::NOTHING;
        let alpha = self.alpha as f32;
        let interactive = state.is_open();
        let mut output = self.context.run_pass(input, |ctx| {
            actions.clear();
            controls.clear();
            if !self.active() {
                return;
            }
            if interactive && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                actions.push((settings::DONE, None));
            }
            let theme = MenuTheme::default();
            let ink = |rgb: [u8; 3]| Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
            egui::Area::new(Id::unique("settings-backdrop"))
                .fixed_pos(pos2(0., 0.))
                .order(egui::Order::Middle)
                .movable(false)
                .fade_in(false)
                .show(ctx, |ui| {
                    let rect = Rect::from_min_size(pos2(0., 0.), area);
                    let response = ui.allocate_rect(rect, Sense::click());
                    ui.painter().rect_filled(
                        rect,
                        0.,
                        ink(theme.backdrop).gamma_multiply(0.88 * alpha),
                    );
                    if interactive && response.clicked() {
                        actions.push((settings::DONE, None));
                    }
                });
            let width = 560_f32.min((area.x - 48.).max(1.));
            egui::Area::new(Id::unique("settings-panel"))
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .order(egui::Order::Foreground)
                .movable(false)
                .fade_in(false)
                .show(ctx, |ui| {
                    ui.set_opacity(alpha);
                    if !interactive {
                        ui.disable();
                    }
                    egui::Frame::new()
                        .fill(ink(theme.field))
                        .corner_radius(16)
                        .inner_margin(24)
                        .shadow(egui::epaint::Shadow {
                            offset: [0, 4],
                            blur: 12,
                            spread: 0,
                            color: Color32::from_black_alpha(110),
                        })
                        .show(ui, |ui| {
                            ui.set_width((width - 48.).max(1.));
                            ui.label(
                                RichText::new("Settings")
                                    .size(24.)
                                    .family(egui::FontFamily::Name("bold".into())),
                            );
                            ui.add_space(8.);
                            egui::ScrollArea::vertical()
                                .animated(false)
                                .max_height((area.y - 238.).max(24.))
                                .show(ui, |ui| {
                                    for (id, title, caption, checked, help) in [
                                        (
                                            settings::QUALITY,
                                            "Quality",
                                            "High quality",
                                            state.preferences().quality == MenuQuality::High,
                                            "Off: Low. High adapts to load. Next game.",
                                        ),
                                        (
                                            settings::RESOLUTION,
                                            "World resolution",
                                            "Native/sharp",
                                            state.preferences().resolution
                                                == WorldResolution::Native,
                                            "Sharper world; higher GPU use. Same view area.",
                                        ),
                                    ] {
                                        ui.label(title);
                                        let response = pill(
                                            ui,
                                            id,
                                            caption,
                                            Some(checked),
                                            ui.available_width(),
                                            false,
                                        );
                                        if initial && id == settings::QUALITY {
                                            response.request_focus();
                                        }
                                        if response.has_focus() && (tabbed || initial) {
                                            response.scroll_to_me(Some(egui::Align::Center));
                                        }
                                        controls.push((
                                            id,
                                            response.rect.intersect(ui.clip_rect()),
                                            response.has_focus(),
                                        ));
                                        if response.clicked() {
                                            actions.push((id, Some(!checked)));
                                        }
                                        ui.label(RichText::new(help).size(16.));
                                        ui.add_space(8.);
                                    }
                                    if state.native_unavailable() {
                                        ui.label(
                                            RichText::new("Native unavailable at this size")
                                                .size(16.),
                                        );
                                    }
                                    if state.not_saved() {
                                        ui.label(
                                            RichText::new("Not saved; applies this session")
                                                .size(16.),
                                        );
                                    }
                                });
                            ui.add_space(8.);
                            ui.vertical_centered(|ui| {
                                let response = pill(
                                    ui,
                                    settings::DONE,
                                    "Done",
                                    None,
                                    112_f32.min(ui.available_width()),
                                    true,
                                );
                                controls.push((
                                    settings::DONE,
                                    response.rect,
                                    response.has_focus(),
                                ));
                                if response.clicked() {
                                    actions.push((settings::DONE, None));
                                }
                            });
                        });
                    panel_rect = ui.min_rect();
                });
        });
        self.controls = controls;
        self.panel_rect = panel_rect;
        for (control, checked) in actions {
            state.activate(state.intent(control, checked, Activation::Pointer));
        }
        self.textures
            .append(std::mem::take(&mut output.textures_delta));
        self.meshes = self
            .context
            .tessellate(std::mem::take(&mut output.shapes), output.pixels_per_point);
        self.output = Some(output.platform_output);
    }
    pub fn paint(&mut self, gpu: &gpu::Renderer, target: &wgpu::TextureView) {
        if !self.active() {
            return;
        }
        let renderer = self.renderer.get_or_insert_with(|| {
            egui_wgpu::Renderer::new(
                &gpu.device,
                gpu.format,
                egui_wgpu::RendererOptions {
                    dithering: false,
                    ..Default::default()
                },
            )
        });
        for (id, deltas) in &self.textures.set {
            for delta in deltas {
                renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: self.extent,
            pixels_per_point: self.dpr,
        };
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        let commands =
            renderer.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &self.meshes, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui Settings"),
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
            renderer.render(&mut pass.forget_lifetime(), &self.meshes, &screen);
        }
        gpu.queue
            .submit(commands.into_iter().chain([encoder.finish()]));
        for id in &self.textures.free {
            renderer.free_texture(id);
        }
        self.textures.clear();
    }
}
impl Drop for SettingsUi {
    fn drop(&mut self) {
        // CPU-only tests and shutdown may intentionally skip the final GPU submission.
        self.textures.clear();
    }
}
fn widget_id(id: ControlId) -> Id {
    Id::unique(("settings", id.0))
}
fn pill(
    ui: &mut egui::Ui,
    id: ControlId,
    label: &str,
    checked: Option<bool>,
    width: f32,
    primary: bool,
) -> egui::Response {
    // Reserve the complete focus/shadow footprint inside scroll clipping.
    let (slot, _) = ui.allocate_exact_size(vec2(width, 52.), Sense::hover());
    let rect = Rect::from_center_size(slot.center(), vec2((slot.width() - 12.).max(1.), 40.));
    let response = ui.interact(rect, widget_id(id), Sense::click());
    response.widget_info(|| {
        if let Some(checked) = checked {
            egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), checked, label)
        } else {
            egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label)
        }
    });
    let theme = MenuTheme::default();
    let tokens = theme.surface(
        if primary {
            slither_client::menu_theme::ControlStyle::PrimaryButton
        } else {
            slither_client::menu_theme::ControlStyle::SecondaryButton
        },
        slither_client::menu_theme::ControlState::Idle,
    );
    let shadow = egui::epaint::Shadow {
        offset: [0, 3],
        blur: 8,
        spread: 0,
        color: Color32::from_black_alpha(100),
    };
    ui.painter().add(shadow.as_shape(rect, 20));
    let colors = tokens
        .colors
        .map(|rgb| Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
    capsule(
        ui,
        rect,
        colors,
        if response.is_pointer_button_down_on() {
            0.8
        } else if response.hovered() {
            1.1
        } else {
            1.
        },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.expand(3.),
            23.,
            Stroke::new(2., Color32::from_rgb(224, 224, 255)),
            egui::StrokeKind::Outside,
        );
    }
    let galley = ui.painter().layout_no_wrap(
        label.into(),
        egui::FontId::new(20., egui::FontFamily::Name("bold".into())),
        Color32::WHITE,
    );
    let extra = if checked.is_some() { 34. } else { 0. };
    let left = rect.center().x - (galley.size().x + extra) / 2.;
    if let Some(checked) = checked {
        let box_rect = Rect::from_center_size(pos2(left + 11., rect.center().y), vec2(22., 22.));
        let fill = if checked {
            theme.selection
        } else {
            theme.backdrop
        };
        ui.painter()
            .rect_filled(box_rect, 4., Color32::from_rgb(fill[0], fill[1], fill[2]));
        if checked {
            ui.painter().add(egui::Shape::line(
                vec![
                    box_rect.min + vec2(5., 11.),
                    box_rect.min + vec2(9., 15.),
                    box_rect.min + vec2(17., 6.),
                ],
                Stroke::new(3., Color32::WHITE),
            ));
        }
    }
    ui.painter().galley(
        pos2(left + extra, rect.center().y - galley.size().y / 2.),
        galley,
        Color32::WHITE,
    );
    response
}

// A capsule mesh carries the existing theme's two colour stops; its outer ring
// feathers one physical pixel, avoiding a raster atlas or clipped image shadow.
fn capsule(ui: &egui::Ui, rect: Rect, colors: [Color32; 2], brightness: f32) {
    let mut mesh = egui::epaint::Mesh::default();
    let color = |y: f32| {
        let t = ((y - rect.top()) / rect.height()).clamp(0., 1.);
        let a = colors[0].to_array();
        let b = colors[1].to_array();
        Color32::from_rgb(
            ((a[0] as f32 * (1. - t) + b[0] as f32 * t) * brightness).min(255.) as u8,
            ((a[1] as f32 * (1. - t) + b[1] as f32 * t) * brightness).min(255.) as u8,
            ((a[2] as f32 * (1. - t) + b[2] as f32 * t) * brightness).min(255.) as u8,
        )
    };
    let vertex = |pos, color| egui::epaint::Vertex {
        pos,
        uv: egui::epaint::WHITE_UV,
        color,
    };
    mesh.vertices
        .push(vertex(rect.center(), color(rect.center().y)));
    let radius = (rect.height() / 2.).min(rect.width() / 2.);
    let feather = 0.5 / ui.pixels_per_point();
    for (center, angle) in [
        (
            pos2(rect.right() - radius, rect.top() + radius),
            -std::f32::consts::FRAC_PI_2,
        ),
        (pos2(rect.right() - radius, rect.bottom() - radius), 0.),
        (
            pos2(rect.left() + radius, rect.bottom() - radius),
            std::f32::consts::FRAC_PI_2,
        ),
        (
            pos2(rect.left() + radius, rect.top() + radius),
            std::f32::consts::PI,
        ),
    ] {
        for i in 0..=12 {
            let angle = angle + i as f32 / 12. * std::f32::consts::FRAC_PI_2;
            let direction = vec2(angle.cos(), angle.sin());
            let inner = center + direction * (radius - feather);
            mesh.vertices.push(vertex(inner, color(inner.y)));
            mesh.vertices.push(vertex(
                center + direction * (radius + feather),
                Color32::TRANSPARENT,
            ));
        }
    }
    let count = (mesh.vertices.len() as u32 - 1) / 2;
    for i in 0..count {
        let a = 1 + i * 2;
        let b = 1 + ((i + 1) % count) * 2;
        mesh.indices.extend([0, a, b, a, b, b + 1, a, b + 1, a + 1]);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}
