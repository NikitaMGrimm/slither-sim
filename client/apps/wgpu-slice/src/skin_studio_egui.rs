//! Native skin studio widgets. The shared core owns the transaction, layout,
//! scrolled windows and roving focus; egui resources and input belong to the
//! native host. The preview snake stays a renderer layer above egui.
use crate::{
    Result,
    egui_panel::{self as widgets, PressGuard, label, outline, rect},
    egui_theme as theme,
    scene::Resource,
};
use egui::{Align, Color32, Id, Rect, Sense, pos2, vec2};
use slither_client::{
    authored_sources::standard_palette_rgb,
    menu_controls::{Activation, Control, ControlId, ControlIntent, ControlKind, PanelView},
    menu_theme::{ControlState, ControlStyle, MenuTheme},
    presentation::TextStyle,
    skin_studio::{BACKDROP, CANCEL},
};
use std::collections::BTreeMap;

pub fn widget_id(id: ControlId) -> Id {
    Id::unique(("studio", id.0))
}
/// Tab wraps within the modal in the core's tab order: the active section's
/// tab, its roving item(s), its actions, then Cancel and Save.
pub fn focus_order(panel: &PanelView) -> Vec<Id> {
    widgets::focus_order(panel, widget_id)
}

/// One cosmetic thumbnail texture, keyed by its renderer resource identity.
struct Thumbnail {
    handle: u64,
    revision: u64,
    texture: egui::TextureHandle,
}

#[derive(Default)]
pub struct StudioUi {
    pub controls: Vec<(ControlId, Rect, bool)>,
    pub keyboard_focus: bool,
    request_focus: Option<ControlId>,
    generation: Option<u64>,
    press: PressGuard,
    thumbnails: BTreeMap<u8, Thumbnail>,
}
impl StudioUi {
    pub fn focused(&self) -> Option<ControlId> {
        self.controls.iter().find(|c| c.2).map(|c| c.0)
    }
    pub fn request_focus(&mut self, id: ControlId) {
        self.request_focus = Some(id);
    }
    /// A wheel scroll moves the pressed control like a window replacement.
    pub fn invalidate_press(&mut self) {
        self.press.invalidate();
    }
    /// Only the visible cosmetic window keeps textures, like the renderer's thumbnail cache.
    fn sync_thumbnails(&mut self, ctx: &egui::Context, thumbnails: &BTreeMap<u8, Resource>) {
        self.thumbnails.retain(|id, t| {
            thumbnails
                .get(id)
                .is_some_and(|r| r.asset.handle == t.handle && r.asset.revision == t.revision)
        });
        for (id, resource) in thumbnails {
            self.thumbnails.entry(*id).or_insert_with(|| {
                let image = &resource.image;
                Thumbnail {
                    handle: resource.asset.handle,
                    revision: resource.asset.revision,
                    texture: ctx.load_texture(
                        format!("studio-cosmetic-{id}"),
                        // Decoded art keeps premultiplied pixels.
                        egui::ColorImage::from_rgba_premultiplied(
                            [image.width as usize, image.height as usize],
                            &image.pixels,
                        ),
                        Default::default(),
                    ),
                }
            });
        }
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        panel: &PanelView,
        viewport: [f64; 2],
        interactive: bool,
        thumbnails: &BTreeMap<u8, Resource>,
    ) -> Result<Vec<ControlIntent>> {
        let focused = self.focused();
        self.controls.clear();
        let mut intents = Vec::new();
        if !panel.visible {
            self.generation = None;
            self.thumbnails.clear();
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
        if interactive {
            ctx.input(|i| {
                for event in &i.events {
                    match event {
                        egui::Event::Key { pressed: true, .. } => self.keyboard_focus = true,
                        egui::Event::PointerButton { pressed: true, .. } => {
                            self.keyboard_focus = false
                        }
                        _ => {}
                    }
                }
            });
        }
        // A new generation (open, section switch, cosmetic window) resets
        // focus to the section tab and cancels a held press, as the shared
        // panel input did.
        if self.generation != Some(panel.generation) {
            self.generation = Some(panel.generation);
            self.request_focus = panel.initial_focus;
            self.press.invalidate();
        }
        let suppress = self.press.suppress(ctx);
        if suppress && self.request_focus.is_none() {
            // egui surrenders focus on a click elsewhere; a stale release
            // must not move focus any more than it activates.
            self.request_focus = focused;
        }
        let pressed = ctx.input(|i| i.pointer.primary_pressed());
        if interactive && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            intents.push(intent(CANCEL, Activation::Enter));
        }
        self.sync_thumbnails(ctx, thumbnails);
        let area = ctx.input(|i| i.viewport_rect());
        let alpha = panel.alpha as f32;
        if let Some(backdrop) = resolved.iter().find(|r| r.control.id == BACKDROP) {
            egui::Area::new(Id::unique("studio-backdrop"))
                .fixed_pos(pos2(0., 0.))
                .order(egui::Order::Middle)
                .movable(false)
                .fade_in(false)
                .show(ctx, |ui| {
                    let r = rect(backdrop.rect);
                    ui.set_min_size(r.size());
                    let response = ui.interact(r, widget_id(BACKDROP), Sense::CLICK);
                    ui.painter().rect_filled(
                        r,
                        0.,
                        theme::ink(tokens.backdrop)
                            .gamma_multiply(backdrop.control.alpha as f32 * alpha),
                    );
                    if interactive && response.clicked() && !suppress {
                        intents.push(intent(BACKDROP, Activation::Pointer));
                    }
                });
        }
        // egui hit-tests only the top-most layer under the pointer, so the
        // panel layer must not extend beyond the surface over the backdrop.
        let surface = resolved
            .iter()
            .find(|r| r.control.visible && matches!(r.control.kind, ControlKind::Surface))
            .map_or(Rect::NOTHING, |r| rect(r.rect).intersect(area));
        egui::Area::new(Id::unique("studio"))
            .fixed_pos(surface.min)
            .order(egui::Order::Foreground)
            .movable(false)
            .fade_in(false)
            .sense(Sense::hover())
            .show(ctx, |ui| {
                ui.set_min_size(surface.size());
                ui.set_clip_rect(area);
                ui.set_opacity(alpha);
                for r in resolved.iter().filter(|r| r.control.visible) {
                    let control = &r.control;
                    if control.id == BACKDROP {
                        continue;
                    }
                    let enabled = panel.enabled && control.enabled;
                    let bounds = rect(r.rect);
                    let window = rect(r.clip).intersect(area);
                    // A control wholly inside its scrolled window keeps its
                    // shadow outside it; a partly scrolled one crops both.
                    let clip = if window.contains_rect(bounds) {
                        area
                    } else {
                        window
                    };
                    let scale = r.scale as f32;
                    let mut style = tokens.text_style(control.style);
                    style.size *= r.scale;
                    let response = ui
                        .scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
                            ui.set_clip_rect(clip);
                            ui.visuals_mut().disabled_alpha = 1.;
                            // Section controls fade in on a section switch.
                            ui.multiply_opacity(control.alpha as f32);
                            if !enabled
                                && !matches!(
                                    control.kind,
                                    ControlKind::Label { .. } | ControlKind::Surface
                                )
                            {
                                ui.multiply_opacity(tokens.disabled_opacity(control.style) as f32);
                            }
                            if !interactive || !enabled {
                                ui.disable();
                            }
                            self.control(ui, control, bounds, window, scale, style, tokens)
                        })
                        .inner;
                    let Some(response) = response else {
                        continue;
                    };
                    // A fresh press focuses its control, as the shared panel
                    // input did, so arrows continue from a clicked swatch or
                    // cell. A press held across a new generation does not.
                    if interactive
                        && (self.request_focus == Some(control.id)
                            || (enabled && pressed && response.is_pointer_button_down_on()))
                    {
                        response.request_focus();
                    }
                    self.controls.push((
                        control.id,
                        bounds.intersect(window),
                        response.has_focus(),
                    ));
                    let pointer = response.clicked_by(egui::PointerButton::Primary);
                    if interactive && response.clicked() && !(pointer && suppress) {
                        intents.push(intent(
                            control.id,
                            if pointer {
                                Activation::Pointer
                            } else {
                                Activation::Enter
                            },
                        ));
                    }
                }
            });
        self.request_focus = None;
        self.press.settle(ctx);
        Ok(intents)
    }
    #[allow(clippy::too_many_arguments)]
    fn control(
        &self,
        ui: &mut egui::Ui,
        control: &Control,
        r: Rect,
        window: Rect,
        scale: f32,
        style: TextStyle,
        tokens: MenuTheme,
    ) -> Option<egui::Response> {
        let id = widget_id(control.id);
        let focus_ring = |ui: &egui::Ui, response: &egui::Response, radius: f32| {
            if response.has_focus() && self.keyboard_focus {
                outline(ui, r, radius, tokens);
            }
        };
        match &control.kind {
            ControlKind::Surface => {
                widgets::surface(ui, control, r, scale, id, tokens);
                None
            }
            ControlKind::Label { text } => {
                label(ui.painter(), text, r, style, Align::Center, false);
                None
            }
            ControlKind::Button { label: text } => Some(widgets::button(
                ui,
                control,
                text,
                r,
                scale,
                style,
                id,
                self.keyboard_focus,
                tokens,
            )),
            ControlKind::SectionTab {
                label: text,
                selected,
            } => {
                let response = widgets::button(
                    ui,
                    control,
                    text,
                    r,
                    scale,
                    style,
                    id,
                    self.keyboard_focus,
                    tokens,
                );
                if *selected {
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            pos2(r.left() + 16., r.bottom() - 3.),
                            vec2(r.width() - 32., 2.),
                        ),
                        0.,
                        theme::ink(tokens.text),
                    );
                }
                Some(response)
            }
            ControlKind::Swatch {
                palette,
                count,
                selected,
            } => {
                let response = ui.interact(r, id, Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::Button,
                        ui.is_enabled(),
                        *selected,
                        &control.name,
                    )
                });
                let rgb = standard_palette_rgb(*palette)?;
                let width = if count.is_some() { r.width() - 4. } else { 22. };
                // The retained tile: a 20px rounded gradient inside a 22px cell.
                let tile = Rect::from_min_size(
                    pos2(r.left() + (r.width() - width) / 2., r.top() + 3.),
                    vec2(width, 22.),
                )
                .shrink(1.);
                theme::rounded(
                    ui,
                    tile,
                    4.,
                    [rgb, rgb.map(|c| (u16::from(c) * 4 / 5) as u8)].map(theme::ink),
                    1.,
                );
                if *selected {
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            pos2(r.center().x - 2. * scale, r.bottom() - 5. * scale),
                            vec2(4. * scale, 2. * scale),
                        ),
                        0.,
                        theme::ink(tokens.text),
                    );
                }
                focus_ring(ui, &response, cell_radius(tokens, scale));
                if let Some(count) = count {
                    let mut style = style;
                    style.size = 12. * f64::from(scale);
                    style.color = if rgb.iter().map(|c| u32::from(*c)).sum::<u32>() > 480 {
                        tokens.backdrop
                    } else {
                        [255; 3]
                    };
                    label(
                        ui.painter(),
                        &count.to_string(),
                        r,
                        style,
                        Align::Center,
                        true,
                    );
                }
                Some(response)
            }
            ControlKind::CosmeticCell { cosmetic, selected } => {
                let response = ui.interact(r, id, Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::Button,
                        ui.is_enabled(),
                        *selected,
                        &control.name,
                    )
                });
                let radius = cell_radius(tokens, scale);
                widgets::shadow(ui, r, radius, scale, widgets::PANEL);
                ui.painter().rect_filled(
                    r,
                    radius,
                    theme::ink(
                        tokens
                            .surface(ControlStyle::SelectedCell, ControlState::Idle)
                            .colors[0],
                    ),
                );
                let inner = r.shrink(4.);
                if let Some(thumbnail) = cosmetic.and_then(|id| self.thumbnails.get(&id)) {
                    let [w, h] = thumbnail.texture.size().map(|n| n as f32);
                    let factor = (inner.width() / w).min(inner.height() / h);
                    let fitted = Rect::from_center_size(inner.center(), vec2(w, h) * factor);
                    ui.painter()
                        .with_clip_rect(window.intersect(ui.clip_rect()))
                        .image(
                            thumbnail.texture.id(),
                            fitted,
                            Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                            Color32::WHITE,
                        );
                } else if cosmetic.is_none() {
                    label(ui.painter(), "None", r, style, Align::Center, false);
                } else {
                    // Missing art: a small plus in muted text ink.
                    let ink = theme::ink(tokens.text).gamma_multiply(0.6);
                    for size in [vec2(12., 2.), vec2(2., 12.)] {
                        ui.painter()
                            .rect_filled(Rect::from_center_size(r.center(), size), 0., ink);
                    }
                }
                if *selected {
                    ui.painter().rect_filled(
                        Rect::from_min_size(inner.min, vec2(4. * scale, 4. * scale)),
                        0.,
                        theme::ink(tokens.text),
                    );
                }
                focus_ring(ui, &response, radius);
                Some(response)
            }
            _ => None,
        }
    }
}

/// Builder and cosmetic cells share the selected-cell corner.
fn cell_radius(tokens: MenuTheme, scale: f32) -> f32 {
    f32::from(
        tokens
            .surface(ControlStyle::SelectedCell, ControlState::Idle)
            .radius,
    ) * scale
}
