//! Native start widgets; shared core retains wording, layout and admission.
use crate::{
    Result,
    cosmetics::{Cosmetics, MenuArt},
    egui_theme as theme, materials, wordmark,
};
use egui::{Id, Rect, Sense, pos2, vec2};
use slither_client::{
    login_controls::*,
    menu_controls::ControlId,
    menu_layout::{MenuElement as E, MenuLayout, MenuRect},
    menu_theme::MenuTheme,
    start_page::StartMenuView,
};

pub fn widget_id(id: ControlId) -> Id {
    Id::unique(("login", id.0))
}
pub fn focus_order(view: &StartMenuView) -> Vec<Id> {
    [NICKNAME, PLAY, CHANGE_SKIN, SERVER, SETTINGS]
        .into_iter()
        .filter(|id| {
            view.ordinary_controls_visible
                && match *id {
                    NICKNAME => !view.disabled,
                    PLAY => !view.play_disabled,
                    CHANGE_SKIN => {
                        view.phase == slither_client::start_page::MenuPhase::Login
                            && !view.champion_pending
                    }
                    SERVER => slither_client::server_picker::available(view),
                    SETTINGS => slither_client::settings::available(view),
                    _ => false,
                }
        })
        .map(widget_id)
        .collect()
}
fn rect(r: MenuRect) -> Rect {
    Rect::from_min_size(
        pos2(r.x as f32, r.y as f32),
        vec2(r.width as f32, r.height as f32),
    )
}

struct Logo {
    key: [u64; 5],
    tiles: Vec<egui::TextureHandle>,
}

#[derive(Default)]
pub struct LoginUi {
    pub controls: Vec<(ControlId, Rect, bool)>,
    pub nickname: String,
    pub composing: bool,
    pub keyboard_focus: bool,
    request_focus: Option<ControlId>,
    composition: Option<(String, egui::text_edit::TextEditState)>,
    logo: Option<Logo>,
    spinner: Option<egui::TextureHandle>,
    skin: Option<(u64, u64, egui::TextureHandle)>,
}
impl LoginUi {
    pub fn request_focus(&mut self, id: ControlId) {
        self.request_focus = Some(id);
    }
    pub fn begin_composition(&mut self, ctx: &egui::Context) {
        if self.composition.is_none() {
            self.composition = Some((
                self.nickname.clone(),
                egui::TextEdit::load_state(ctx, widget_id(NICKNAME)).unwrap_or_default(),
            ));
        }
        self.composing = true;
    }
    pub fn cancel_composition(&mut self, ctx: &egui::Context) {
        if let Some((text, state)) = self.composition.take() {
            self.nickname = text;
            state.store(ctx, widget_id(NICKNAME));
        }
        self.composing = false;
    }
    pub fn new(nickname: String) -> Self {
        Self {
            nickname,
            ..Default::default()
        }
    }
    pub fn sync_art(&mut self, ctx: &egui::Context, cosmetics: Option<&Cosmetics>) {
        let Some(resource) = cosmetics.and_then(|c| c.menu_art(MenuArt::Skin)) else {
            self.skin = None;
            return;
        };
        let asset = resource.asset;
        if self
            .skin
            .as_ref()
            .is_some_and(|(h, v, _)| *h == asset.handle && *v == asset.revision)
        {
            return;
        }
        let image = &resource.image;
        self.skin = Some((
            asset.handle,
            asset.revision,
            ctx.load_texture(
                "login-skin",
                egui::ColorImage::from_rgba_unmultiplied(
                    [image.width as usize, image.height as usize],
                    &image.pixels,
                ),
                Default::default(),
            ),
        ));
    }
    pub fn focused(&self) -> Option<ControlId> {
        self.controls.iter().find(|c| c.2).map(|c| c.0)
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        view: &StartMenuView,
        layout: &MenuLayout,
        interactive: bool,
        status: Option<&str>,
    ) -> Result<Option<(ControlId, bool)>> {
        self.controls.clear();
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
        let mut action = None;
        let area = ctx.input(|i| i.viewport_rect());
        let scale = layout.scale as f32;
        let alpha = view.login_alpha as f32;
        let focused = ctx.memory(|m| m.focused());
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        // Composition owns Enter even when the same pass also contains its commit.
        let ime_enter = self.composing
            || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
        if ime_enter {
            // The host already recorded the held key. Do not let TextEdit's
            // single-line Enter surrender focus before the IME commit arrives.
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
        egui::Area::new(Id::unique("login"))
            .fixed_pos(pos2(0., 0.))
            .movable(false)
            .fade_in(false)
            .order(egui::Order::Background)
            .show(ctx, |ui| -> Result<()> {
                ui.set_min_size(area.size());
                ui.set_clip_rect(area);
                ui.set_opacity(alpha);
                let holder = layout.viewport_rect(E::Logo);
                let mut logo =
                    wordmark::layout(wordmark::DEFAULT_TEXT, holder, area.width() as f64, |_| {
                        unreachable!("authored wordmark glyphs")
                    })?;
                let dpr = (ctx.pixels_per_point() as f64).min(crate::menu::MAX_MENU_RASTER_DENSITY);
                logo.fit_raster(dpr)?;
                let key = [
                    logo.unit.to_bits(),
                    dpr.to_bits(),
                    logo.rect.width.to_bits(),
                    (view.logo_reveal * 280.).floor().to_bits(),
                    logo.word_offset_pixels(&logo.words[0], dpr).to_bits(),
                ];
                if self.logo.as_ref().is_none_or(|old| old.key != key) {
                    let image = wordmark::raster(&logo, dpr, view.logo_reveal)?;
                    let cache = self.logo.get_or_insert_with(|| Logo {
                        key,
                        tiles: Vec::new(),
                    });
                    let count = image.width.div_ceil(2048) as usize;
                    cache.tiles.truncate(count);
                    for (index, x) in (0..image.width).step_by(2048).enumerate() {
                        let width = (image.width - x).min(2048) as usize;
                        let mut pixels = Vec::with_capacity(width * image.height as usize * 4);
                        for row in image.pixels.chunks_exact(image.width as usize * 4) {
                            pixels
                                .extend_from_slice(&row[x as usize * 4..(x as usize + width) * 4]);
                        }
                        let tile = egui::ColorImage::from_rgba_unmultiplied(
                            [width, image.height as usize],
                            &pixels,
                        );
                        if let Some(texture) = cache.tiles.get_mut(index) {
                            texture.set(tile, Default::default());
                        } else {
                            cache.tiles.push(ctx.load_texture(
                                "login-wordmark",
                                tile,
                                Default::default(),
                            ));
                        }
                    }
                    cache.key = key;
                }
                if let Some(cache) = &self.logo {
                    let origin = logo.origin_pixels(dpr).map(|n| (n / dpr) as f32);
                    for (index, texture) in cache.tiles.iter().enumerate() {
                        let size = texture.size_vec2() / dpr as f32;
                        let min = pos2(origin[0] + index as f32 * 2048. / dpr as f32, origin[1]);
                        ui.painter().image(
                            texture.id(),
                            Rect::from_min_size(min, size),
                            Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                            egui::Color32::WHITE.gamma_multiply(view.logo_alpha as f32),
                        );
                    }
                }
                let last = &view.last_life;
                let r = rect(layout.viewport_rect(E::LastLength));
                let mut job = egui::text::LayoutJob::default();
                for (s, bold, a) in [
                    (
                        format!(
                            "{}{}",
                            last.label,
                            if last.label.is_empty() { "" } else { " " }
                        ),
                        false,
                        last.label_alpha,
                    ),
                    (last.number.clone(), true, 1.),
                    (last.suffix.into(), false, 1.),
                ] {
                    job.append(
                        &s,
                        0.,
                        egui::TextFormat {
                            font_id: egui::FontId::new(
                                15. * scale,
                                egui::FontFamily::Name(
                                    if bold { "login-bold" } else { "login-body" }.into(),
                                ),
                            ),
                            color: egui::Color32::WHITE.gamma_multiply(a as f32),
                            ..Default::default()
                        },
                    );
                }
                let galley = ui.fonts_mut(|f| f.layout_job(job));
                ui.painter().galley(
                    r.center() - galley.size() / 2.,
                    galley,
                    egui::Color32::WHITE,
                );
                if view.tips_visible {
                    let r = rect(layout.viewport_rect(E::Tips));
                    ui.painter().text(
                        r.center_top(),
                        egui::Align2::CENTER_TOP,
                        view.tip,
                        egui::FontId::new(14. * scale, egui::FontFamily::Name("login-body".into())),
                        egui::Color32::from_rgb(128, 104, 192)
                            .gamma_multiply(view.tip_alpha as f32),
                    );
                }
                let description = slither_client::login_controls::panel(
                    view,
                    layout,
                    [area.width() as f64, area.height() as f64],
                );
                for control in description.controls.iter().filter(|c| c.visible) {
                    let element = match control.id {
                        NICKNAME => E::NicknameHolder,
                        PLAY => E::Play,
                        CHANGE_SKIN => E::Skin,
                        SERVER => E::Server,
                        _ => E::Settings,
                    };
                    let r = rect(layout.viewport_rect(element)).intersect(area.shrink(4.));
                    let response = ui
                        .scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
                            ui.set_opacity(alpha * control.alpha as f32);
                            ui.visuals_mut().disabled_alpha = 1.;
                            if !interactive
                                || !control.enabled
                                || (self.composing && control.id != NICKNAME)
                            {
                                ui.disable();
                            }
                            if control.id == NICKNAME {
                                let tokens = MenuTheme::default();
                                let size = MenuTheme::default()
                                    .text_style(slither_client::menu_theme::ControlStyle::Nickname)
                                    .size as f32
                                    * scale;
                                let width = ui.fonts_mut(|f| {
                                    f.layout_no_wrap(
                                        self.nickname.clone(),
                                        egui::FontId::new(
                                            size,
                                            egui::FontFamily::Name("login-body".into()),
                                        ),
                                        egui::Color32::WHITE,
                                    )
                                    .size()
                                    .x
                                });
                                let font_size =
                                    size * ((r.width() - 32. * scale) / width.max(1.)).min(1.);
                                ui.visuals_mut().extreme_bg_color = theme::ink(tokens.field);
                                ui.visuals_mut().text_cursor.stroke.color = theme::ink(tokens.text);
                                let visuals = ui.visuals_mut();
                                for widget in [
                                    &mut visuals.widgets.inactive,
                                    &mut visuals.widgets.hovered,
                                    &mut visuals.widgets.active,
                                ] {
                                    widget.corner_radius =
                                        egui::CornerRadius::same((23. * scale) as u8);
                                }
                                ui.visuals_mut().selection.stroke.color = theme::ink(tokens.text);
                                ui.painter().add(
                                    egui::epaint::Shadow {
                                        offset: [0, 4],
                                        blur: 16,
                                        spread: 0,
                                        color: egui::Color32::from_black_alpha(180),
                                    }
                                    .as_shape(r, (23. * scale) as u8),
                                );
                                ui.painter()
                                    .rect_filled(r, 23. * scale, theme::ink(tokens.field));
                                ui.painter().rect_stroke(
                                    r,
                                    23. * scale,
                                    egui::Stroke::new(2. * scale, egui::Color32::BLACK),
                                    egui::StrokeKind::Inside,
                                );
                                ui.put(
                                    r,
                                    egui::TextEdit::singleline(&mut self.nickname)
                                        .frame(egui::Frame::NONE.inner_margin(egui::Margin {
                                            left: (16. * scale).round() as i8,
                                            right: (16. * scale).round() as i8,
                                            top: (10. * scale).round() as i8,
                                            bottom: (10. * scale).round() as i8,
                                        }))
                                        .id(widget_id(NICKNAME))
                                        .hint_text(
                                            egui::RichText::new(view.wording.nickname)
                                                .color(theme::ink(tokens.placeholder)),
                                        )
                                        .font(egui::FontId::new(
                                            font_size,
                                            egui::FontFamily::Name("login-body".into()),
                                        ))
                                        .text_color(theme::ink(tokens.text))
                                        .align(egui::Align2::LEFT_CENTER)
                                        .min_size(r.size())
                                        .clip_text(true),
                                )
                            } else if control.id == PLAY {
                                theme::pill_at(
                                    ui,
                                    widget_id(PLAY),
                                    view.play_label,
                                    r,
                                    true,
                                    20. * scale,
                                    self.keyboard_focus,
                                )
                            } else {
                                let response =
                                    ui.interact(r, widget_id(control.id), Sense::click());
                                response.widget_info(|| {
                                    egui::WidgetInfo::labeled(
                                        egui::Role::Button,
                                        ui.is_enabled(),
                                        &control.name,
                                    )
                                });
                                ui.multiply_opacity(if response.is_pointer_button_down_on() {
                                    0.5
                                } else if response.hovered()
                                    || (response.has_focus() && self.keyboard_focus)
                                {
                                    1.
                                } else {
                                    0.7
                                });
                                if let Some((_, _, texture)) =
                                    self.skin.as_ref().filter(|_| control.id == CHANGE_SKIN)
                                {
                                    ui.painter().image(
                                        texture.id(),
                                        r,
                                        Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                                        egui::Color32::WHITE,
                                    );
                                } else {
                                    theme::link_icon(ui, r, control.id);
                                    ui.painter().text(
                                        r.center_bottom(),
                                        egui::Align2::CENTER_BOTTOM,
                                        if control.id == SERVER {
                                            view.wording.choose_server
                                        } else {
                                            &control.name
                                        },
                                        egui::FontId::new(
                                            14.,
                                            egui::FontFamily::Name("link".into()),
                                        ),
                                        theme::ink(MenuTheme::default().text),
                                    );
                                }
                                if response.has_focus() && self.keyboard_focus {
                                    ui.painter().rect_stroke(
                                        r,
                                        4.,
                                        egui::Stroke::new(
                                            2.,
                                            theme::ink(MenuTheme::default().text),
                                        ),
                                        egui::StrokeKind::Inside,
                                    );
                                }
                                response
                            }
                        })
                        .inner;
                    if interactive
                        && self.request_focus == Some(control.id)
                        && (focused.is_none() || focused == Some(widget_id(control.id)))
                    {
                        response.request_focus();
                    }
                    self.controls.push((control.id, r, response.has_focus()));
                    if response.clicked() {
                        action = Some((
                            control.id,
                            !response.clicked_by(egui::PointerButton::Primary),
                        ));
                    }
                }
                if let Some(status) = status {
                    ui.painter().text(
                        pos2(area.center().x, area.bottom() - 18.),
                        egui::Align2::CENTER_CENTER,
                        status,
                        egui::FontId::new(18., egui::FontFamily::Name("body".into())),
                        theme::ink(MenuTheme::default().text),
                    );
                }
                if view.spinner_visible && view.spinner_scale > 0. {
                    let density = dpr * view.spinner_scale;
                    let image = materials::menu_spinner(view.spinner_time, density)?;
                    let size = vec2(image.width as f32, image.height as f32) / dpr as f32;
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [image.width as usize, image.height as usize],
                        &image.pixels,
                    );
                    if let Some(texture) = &mut self.spinner {
                        texture.set(image, Default::default());
                    } else {
                        self.spinner =
                            Some(ctx.load_texture("login-spinner", image, Default::default()));
                    }
                    let mut painter = ui.painter().clone();
                    painter.set_opacity(view.spinner_alpha as f32);
                    painter.image(
                        self.spinner.as_ref().unwrap().id(),
                        Rect::from_center_size(area.center(), size),
                        Rect::from_min_max(pos2(0., 0.), pos2(1., 1.)),
                        egui::Color32::WHITE,
                    );
                }
                Ok(())
            })
            .inner?;
        self.request_focus = None;
        if interactive {
            ctx.input(|i| {
                for event in &i.events {
                    match event {
                        egui::Event::Ime(egui::ImeEvent::Preedit { text, .. }) => {
                            self.composing = !text.is_empty()
                        }
                        egui::Event::Ime(egui::ImeEvent::Commit(_))
                        | egui::Event::WindowFocused(false) => self.composing = false,
                        _ => {}
                    }
                }
            });
            if !self.composing {
                self.composition = None;
                self.nickname = slither_client::start_page::normalize_nickname(&self.nickname);
                if let Some(mut state) = egui::TextEdit::load_state(ctx, widget_id(NICKNAME))
                    && let Some(range) = state.cursor.char_range()
                {
                    let mut range = range;
                    range.primary.index.0 = range.primary.index.0.min(self.nickname.len());
                    range.secondary.index.0 = range.secondary.index.0.min(self.nickname.len());
                    state.cursor.set_char_range(Some(range));
                    state.store(ctx, widget_id(NICKNAME));
                }
            }
        }
        if interactive
            && enter
            && !ime_enter
            && (focused.is_none()
                || focused == Some(widget_id(NICKNAME))
                || view.phase == slither_client::start_page::MenuPhase::Returning)
            && !view.play_disabled
        {
            action = Some((PLAY, true));
            ctx.memory_mut(|m| m.request_focus(widget_id(NICKNAME)));
        }
        Ok(action)
    }
}
