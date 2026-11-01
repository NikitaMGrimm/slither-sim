//! Native server picker widgets. The shared core owns layout, list windows,
//! choices and validation; egui resources and input belong to the native host.
use crate::{Result, egui_theme as theme};
use egui::{Align, Color32, Id, Rect, Sense, Stroke, pos2, vec2};
use slither_client::{
    menu_controls::{Activation, Control, ControlId, ControlIntent, ControlKind, PanelView},
    menu_input::TextRule,
    menu_layout::MenuRect,
    menu_theme::{ControlState, ControlStyle, MenuTheme, SurfaceRecipe},
    presentation::TextStyle,
    server_picker::{ADDRESS, BACKDROP, CLOSE, SEARCH},
};

pub fn widget_id(id: ControlId) -> Id {
    Id::unique(("picker", id.0))
}
fn available(panel: &PanelView, control: &Control) -> bool {
    panel.visible
        && panel.enabled
        && control.visible
        && control.enabled
        && !matches!(control.kind, ControlKind::Label { .. })
}
/// Tab wraps within the modal in the core's tab order, skipping disabled controls.
pub fn focus_order(panel: &PanelView) -> Vec<Id> {
    let mut order: Vec<_> = panel
        .controls
        .iter()
        .filter(|c| available(panel, c))
        .filter_map(|c| c.tab_order.map(|n| (n, c.id)))
        .collect();
    order.sort();
    order.into_iter().map(|(_, id)| widget_id(id)).collect()
}
pub fn rule(field: ControlId) -> Option<TextRule> {
    match field {
        ADDRESS => Some(TextRule::ServerAddress),
        SEARCH => Some(TextRule::NumericServer),
        _ => None,
    }
}
fn rect(r: MenuRect) -> Rect {
    Rect::from_min_size(
        pos2(r.x as f32, r.y as f32),
        vec2(r.width as f32, r.height as f32),
    )
}

#[derive(Default)]
pub struct PickerUi {
    pub address: String,
    pub search: String,
    pub controls: Vec<(ControlId, Rect, bool)>,
    pub keyboard_focus: bool,
    pub composing: bool,
    composition: Option<(ControlId, theme::Composition)>,
    request_focus: Option<ControlId>,
    generation: Option<u64>,
}
impl PickerUi {
    pub fn focused(&self) -> Option<ControlId> {
        self.controls.iter().find(|c| c.2).map(|c| c.0)
    }
    pub fn request_focus(&mut self, id: ControlId) {
        self.request_focus = Some(id);
    }
    pub fn text(&self, field: ControlId) -> Option<&str> {
        match field {
            ADDRESS => Some(&self.address),
            SEARCH => Some(&self.search),
            _ => None,
        }
    }
    fn text_mut(&mut self, field: ControlId) -> Option<&mut String> {
        match field {
            ADDRESS => Some(&mut self.address),
            SEARCH => Some(&mut self.search),
            _ => None,
        }
    }
    pub fn begin_composition(&mut self, ctx: &egui::Context, field: ControlId) {
        if self.composition.is_none()
            && let Some(text) = self.text(field)
        {
            let snapshot = theme::Composition::begin(ctx, widget_id(field), text);
            self.composition = Some((field, snapshot));
        }
        self.composing = self.composition.is_some();
    }
    pub fn cancel_composition(&mut self, ctx: &egui::Context) {
        if let Some((field, snapshot)) = self.composition.take() {
            let text = snapshot.cancel(ctx, widget_id(field));
            if let Some(value) = self.text_mut(field) {
                *value = text;
            }
        }
        self.composing = false;
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        panel: &PanelView,
        viewport: [f64; 2],
        interactive: bool,
    ) -> Result<Vec<ControlIntent>> {
        self.controls.clear();
        let mut intents = Vec::new();
        if !panel.visible {
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
        // A new generation (open, custom edit, directory replacement) resets
        // focus to the panel's initial control, as the shared panel input did.
        if self.generation != Some(panel.generation) {
            self.generation = Some(panel.generation);
            self.request_focus = panel.initial_focus;
        }
        let focused = ctx.memory(|m| m.focused()).and_then(|id| {
            panel
                .controls
                .iter()
                .map(|c| c.id)
                .find(|c| widget_id(*c) == id)
        });
        if interactive && matches!(focused, Some(ADDRESS | SEARCH)) {
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
            if enter && !ime && focused == Some(ADDRESS) {
                intents.push(intent(ADDRESS, Activation::Enter));
            }
        }
        if interactive && !self.composing && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            intents.push(intent(CLOSE, Activation::Enter));
        }
        if interactive
            && let Some(search) = resolved
                .iter()
                .find(|r| r.control.id == SEARCH && available(panel, &r.control))
        {
            let hit = rect(search.rect).intersect(rect(search.clip));
            if ctx.input(|i| {
                i.pointer.primary_pressed()
                    && i.pointer.press_origin().is_some_and(|p| hit.contains(p))
            }) {
                // The reference deliberately clears numeric search on every click.
                if self.composition.as_ref().is_some_and(|c| c.0 == SEARCH) {
                    self.cancel_composition(ctx);
                }
                self.search.clear();
            }
        }
        let area = ctx.input(|i| i.viewport_rect());
        let alpha = panel.alpha as f32;
        if let Some(backdrop) = resolved.iter().find(|r| r.control.id == BACKDROP) {
            egui::Area::new(Id::unique("picker-backdrop"))
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
                    if interactive && response.clicked() {
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
        egui::Area::new(Id::unique("picker"))
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
                    let clip = rect(r.clip).intersect(area);
                    let scale = r.scale as f32;
                    let mut style = tokens.text_style(control.style);
                    style.size *= r.scale;
                    let response = ui
                        .scope_builder(egui::UiBuilder::new().max_rect(bounds), |ui| {
                            ui.set_clip_rect(clip);
                            ui.visuals_mut().disabled_alpha = 1.;
                            if !enabled
                                && !matches!(
                                    control.kind,
                                    ControlKind::Label { .. }
                                        | ControlKind::Surface
                                        | ControlKind::ListRow { .. }
                                )
                            {
                                ui.multiply_opacity(tokens.disabled_opacity(control.style) as f32);
                            }
                            if !interactive || !enabled {
                                ui.disable();
                            }
                            self.control(ui, control, bounds, scale, style, tokens)
                        })
                        .inner;
                    let Some(response) = response else {
                        continue;
                    };
                    if interactive && self.request_focus == Some(control.id) {
                        response.request_focus();
                    }
                    self.controls
                        .push((control.id, bounds.intersect(clip), response.has_focus()));
                    if interactive && response.clicked() && rule(control.id).is_none() {
                        intents.push(intent(
                            control.id,
                            if response.clicked_by(egui::PointerButton::Primary) {
                                Activation::Pointer
                            } else {
                                Activation::Enter
                            },
                        ));
                    }
                }
            });
        self.request_focus = None;
        if interactive {
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
                let mut composition = self.composition.take();
                for field in [ADDRESS, SEARCH] {
                    let rule = rule(field).expect("picker field rule");
                    let value = self.text_mut(field).expect("picker field");
                    *value = rule.normalize_units(&value.encode_utf16().collect::<Vec<_>>());
                    let snapshot = composition.take_if(|c| c.0 == field).map(|c| c.1);
                    theme::settle_text(ctx, widget_id(field), value, snapshot);
                }
            }
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
            ControlKind::Surface => {
                let surface = tokens.surface(control.style, ControlState::Idle);
                let radius = f32::from(surface.radius) * scale;
                compact_shadow(ui, r, radius, scale);
                ui.painter()
                    .rect_filled(r, radius, theme::ink(surface.colors[0]));
                // Blank panel space shields the backdrop without taking focus.
                ui.interact(r, id, Sense::CLICK);
                None
            }
            ControlKind::Label { text: value } => {
                label(ui.painter(), value, r, style, Align::Center, false);
                None
            }
            ControlKind::TextField {
                placeholder, inset, ..
            } => {
                let radius = (f32::from(tokens.surface(control.style, ControlState::Idle).radius)
                    * scale)
                    .min(r.height() / 2.);
                ui.painter().add(
                    egui::epaint::Shadow {
                        offset: [0, (4. * scale).round() as i8],
                        blur: (16. * scale).round() as u8,
                        spread: 0,
                        color: Color32::from_black_alpha(180),
                    }
                    .as_shape(r, radius as u8),
                );
                ui.painter()
                    .rect_filled(r, radius, theme::ink(tokens.field));
                ui.painter().rect_stroke(
                    r,
                    radius,
                    Stroke::new(2. * scale, Color32::BLACK),
                    egui::StrokeKind::Inside,
                );
                // egui floors a TextEdit at interact height; the core field size wins.
                ui.spacing_mut().interact_size.y = 0.;
                let visuals = ui.visuals_mut();
                visuals.extreme_bg_color = theme::ink(tokens.field);
                visuals.text_cursor.stroke = Stroke::new(1., theme::ink(tokens.text));
                visuals.selection.bg_fill = theme::ink(tokens.selection).gamma_multiply(0.45);
                visuals.selection.stroke.color = theme::ink(tokens.text);
                let font = egui::FontId::new(style.size as f32, theme::family(style));
                let text = self.text_mut(control.id)?;
                let response = ui.put(
                    r,
                    egui::TextEdit::singleline(text)
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                            left: (inset[0] as f32 * scale).round() as i8,
                            right: (inset[2] as f32 * scale).round() as i8,
                            top: 0,
                            bottom: 0,
                        }))
                        .id(id)
                        .hint_text(
                            // The placeholder shares the field font; egui would
                            // otherwise size an empty field by its default body style.
                            egui::RichText::new(placeholder.as_str())
                                .font(font.clone())
                                .color(theme::ink(tokens.placeholder)),
                        )
                        .font(font)
                        .text_color(theme::ink(style.color))
                        .align(egui::Align2::LEFT_CENTER)
                        .min_size(r.size())
                        .clip_text(true),
                );
                if response.has_focus() && self.keyboard_focus && ui.is_enabled() {
                    outline(ui, r, radius, tokens);
                }
                Some(response)
            }
            ControlKind::Button { label: text } => {
                let response = ui.interact(r, id, Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), text)
                });
                let surface = tokens.surface(control.style, ControlState::Idle);
                let radius = if surface.recipe == SurfaceRecipe::StudioButton {
                    r.height() / 2.
                } else {
                    f32::from(surface.radius) * scale
                };
                compact_shadow(ui, r, radius, scale);
                let colors = surface.colors.map(|rgb| {
                    let color = theme::ink(rgb);
                    // Compact actions dim their surface, not their label, while pressed.
                    if response.is_pointer_button_down_on() {
                        color.gamma_multiply(0.75)
                    } else {
                        color
                    }
                });
                theme::capsule(ui, r, colors, 1.);
                label(
                    ui.painter(),
                    text,
                    r.shrink2(vec2(
                        slither_client::menu_theme::STUDIO_ACTION_PADDING_X as f32 * scale,
                        0.,
                    )),
                    style,
                    Align::Center,
                    control.style != ControlStyle::StudioAction,
                );
                if response.has_focus() && self.keyboard_focus && ui.is_enabled() {
                    outline(ui, r, radius, tokens);
                }
                Some(response)
            }
            ControlKind::ListRow {
                label: name,
                detail,
                selected,
                ping,
            } => {
                let response = ui.interact(r, id, Sense::click());
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::Role::Button,
                        ui.is_enabled(),
                        *selected,
                        format!("{name} {detail}"),
                    )
                });
                // Rows share solid fills; only the visible window has widgets.
                let focused = response.has_focus() && self.keyboard_focus;
                let fill = if *selected {
                    tokens.selection
                } else if focused || response.hovered() {
                    tokens.field
                } else {
                    tokens
                        .surface(ControlStyle::ListRow, ControlState::Idle)
                        .colors[0]
                };
                ui.painter().rect_filled(r, 0., theme::ink(fill));
                if *selected {
                    ui.painter().rect_filled(
                        Rect::from_min_size(r.min, vec2(3. * scale, r.height())),
                        0.,
                        theme::ink(tokens.text),
                    );
                }
                if focused && ui.is_enabled() {
                    let radius = tokens.surface(control.style, ControlState::Idle).radius;
                    outline(ui, r, f32::from(radius) * scale, tokens);
                }
                let inset = 14. * scale;
                column(
                    ui,
                    name,
                    Rect::from_min_size(
                        pos2(r.left() + inset, r.top()),
                        vec2(r.width() * 0.45, r.height()),
                    ),
                    style,
                    Align::Min,
                );
                column(
                    ui,
                    detail,
                    Rect::from_min_size(
                        pos2(r.left() + r.width() * 0.48, r.top()),
                        vec2(r.width() * 0.32, r.height()),
                    ),
                    style,
                    Align::Max,
                );
                let bars = ping.map_or(0, |p| match p {
                    0..80 => 4,
                    80..160 => 3,
                    160..300 => 2,
                    _ => 1,
                });
                for index in 0..4 {
                    let height = (5. + index as f32 * 4.) * scale;
                    let color = if index < bars {
                        if *selected {
                            tokens.text
                        } else {
                            tokens.selection
                        }
                    } else {
                        tokens.placeholder
                    };
                    ui.painter().rect_filled(
                        Rect::from_min_size(
                            pos2(
                                r.right() - inset - (4 - index) as f32 * 7. * scale,
                                r.top() + (r.height() - height) / 2.,
                            ),
                            vec2(4. * scale, height),
                        ),
                        0.,
                        theme::ink(color),
                    );
                }
                Some(response)
            }
            _ => None,
        }
    }
}

/// The retained toolkit's compact shadow: shifted 3px down with a short falloff.
fn compact_shadow(ui: &egui::Ui, r: Rect, radius: f32, scale: f32) {
    ui.painter().add(
        egui::epaint::Shadow {
            offset: [0, (3. * scale).round() as i8],
            blur: (8. * scale).round() as u8,
            spread: 0,
            color: Color32::from_black_alpha(150),
        }
        .as_shape(r, radius.min(255.) as u8),
    );
}

/// One logical-pixel keyboard-focus ring, kept inside the hit box.
fn outline(ui: &egui::Ui, r: Rect, radius: f32, tokens: MenuTheme) {
    ui.painter().rect_stroke(
        r.shrink(1.),
        (radius - 1.).max(0.),
        Stroke::new(1., theme::ink(tokens.text)),
        egui::StrokeKind::Inside,
    );
}

/// Row columns crop to their own box; other labels crop only to the panel.
fn column(ui: &egui::Ui, value: &str, r: Rect, style: TextStyle, align: Align) {
    let painter = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    label(&painter, value, r, style, align, false);
}

fn label(painter: &egui::Painter, value: &str, r: Rect, style: TextStyle, align: Align, fit: bool) {
    if value.is_empty() {
        return;
    }
    let family = theme::family(style);
    let color = theme::ink(style.color);
    let mut galley = painter.layout_no_wrap(
        value.into(),
        egui::FontId::new(style.size as f32, family.clone()),
        color,
    );
    if fit && galley.size().x > r.width() && r.width() > 0. {
        let size = style.size as f32 * r.width() / galley.size().x;
        galley = painter.layout_no_wrap(value.into(), egui::FontId::new(size, family), color);
    }
    let x = match align {
        Align::Min => r.left(),
        Align::Center => r.center().x - galley.size().x / 2.,
        Align::Max => r.right() - galley.size().x,
    };
    painter.galley(pos2(x, r.center().y - galley.size().y / 2.), galley, color);
}
