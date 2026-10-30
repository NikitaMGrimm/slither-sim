//! Opt-in menu resources. No adapter, input, session or independent animation clock.
use crate::{
    Result,
    cosmetics::{Cosmetics, MenuArt},
    frame::{self, Draw, Frame},
    materials,
    render::PreparedFrame,
    scene::Resource,
    text::*,
};
use slither_client::{
    menu_layout::{MenuElement as E, MenuLayout, MenuRect},
    presentation::*,
    start_page::StartMenuView,
};
use std::collections::BTreeMap;

pub const MENU_PRODUCER_DRAWS: usize = 512;
use crate::bank::MENU_MATERIAL_HANDLE_BASE;
type MaterialKey = (u32, u32, u8, u64);

pub fn check_menu_draws(count: usize) -> Result<()> {
    if count > MENU_PRODUCER_DRAWS {
        return Err("Menu producer draw budget".into());
    }
    Ok(())
}
/// Immutable prepared resources survive subsequent resize, retirement and art replacement.
pub struct MenuFrame {
    pub frame: Frame,
    /// Page colour below the world, in the frame's coordinate space; absent when login is hidden.
    pub backdrop: Option<Draw>,
    pub resources: Vec<Resource>,
}
impl MenuFrame {
    /// Map the menu's logical quads into the world backing before the native
    /// host scales that complete picture to the physical window. Input/IME keep
    /// the original logical layout, including fractional window DPR.
    pub fn into_backing(mut self, extent: [u32; 2]) -> Result<Self> {
        crate::render::dimensions(extent, 1.)?;
        let scale = [
            f64::from(extent[0]) * self.frame.viewport_scale / f64::from(self.frame.extent[0]),
            f64::from(extent[1]) * self.frame.viewport_scale / f64::from(self.frame.extent[1]),
        ];
        for draw in self.frame.draws.iter_mut().chain(&mut self.backdrop) {
            // Menu composition emits axis-aligned images, unlike world bodies.
            if draw.rotation != 0. {
                return Err("Rotated menu host mapping".into());
            }
            draw.destination.x *= scale[0];
            draw.destination.width *= scale[0];
            draw.destination.y *= scale[1];
            draw.destination.height *= scale[1];
            draw.translation[0] *= scale[0];
            draw.translation[1] *= scale[1];
            if let Some(corners) = &mut draw.corners {
                for p in corners {
                    p[0] *= scale[0];
                    p[1] *= scale[1];
                }
            }
        }
        self.frame.extent = extent;
        self.frame.viewport_scale = 1.;
        Ok(self)
    }
    /// Put the page below every world draw and menu controls above them.
    /// A failed whole-picture admission publishes nothing.
    pub fn append_to(self, picture: &mut PreparedFrame) -> Result<()> {
        let count = self.frame.draws.len() + usize::from(self.backdrop.is_some());
        check_menu_draws(count)?;
        if self.frame.extent != picture.frame.extent
            || self.frame.viewport_scale != picture.frame.viewport_scale
        {
            return Err("Menu picture extent/scale mismatch".into());
        }
        // Keyed by handle alone: the renderer keeps one revision per handle.
        let mut resources = BTreeMap::new();
        for r in picture.resources.iter().chain(&self.resources) {
            if let Some(old) = resources.insert(r.asset.handle, r.clone())
                && old.asset != r.asset
            {
                return Err("Menu resource handle collides with the world picture".into());
            }
        }
        if resources.len() > 2048
            || resources
                .values()
                .map(|r| r.image.pixels.len())
                .sum::<usize>()
                > crate::bank::PIXEL_BUDGET
        {
            return Err("Menu whole-picture pixel budget".into());
        }
        frame::reserve_draws(&mut picture.frame.draws, count)?;
        if let Some(backdrop) = self.backdrop {
            picture.frame.draws.insert(0, backdrop);
        }
        picture.frame.draws.extend(self.frame.draws);
        picture.resources = std::mem::take(&mut resources).into_values().collect();
        Ok(())
    }
}
#[derive(Default)]
pub struct Menu {
    pub focus: Option<E>,
    pub skin_hovered: bool,
    pub skin_pressed: bool,
    materials: BTreeMap<MaterialKey, Resource>,
    next_handle: u64,
}
struct Label {
    text: String,
    rect: MenuRect,
    style: TextStyle,
    alpha: f64,
    align: Align,
}
struct Composition<'a> {
    old: &'a BTreeMap<MaterialKey, Resource>,
    materials: BTreeMap<MaterialKey, Resource>,
    next_handle: u64,
    scale: f64,
    draws: Vec<Draw>,
    art: Vec<Resource>,
    labels: Vec<Label>,
}
fn body(r: MenuRect) -> BodyRect {
    BodyRect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}
fn image(resource: &Resource, r: MenuRect, alpha: f64) -> Draw {
    Draw {
        corners: None,
        repeat_uv: None,
        black_effect: false,
        asset: resource.asset,
        source: None,
        destination: body(r),
        translation: [0., 0.],
        rotation: 0.,
        effective_alpha: alpha,
        blend: BodyBlend::SourceOver,
    }
}
impl Composition<'_> {
    fn solid_draw(&mut self, r: MenuRect, kind: u8, alpha: f64) -> Result<Draw> {
        let key = (1, 1, kind, 0);
        if !self.materials.contains_key(&key) {
            let resource = if let Some(resource) = self.old.get(&key) {
                resource.clone()
            } else {
                self.next_handle = self
                    .next_handle
                    .checked_add(1)
                    .filter(|h| *h < MENU_MATERIAL_HANDLE_BASE + crate::bank::HANDLE_NAMESPACE_SIZE)
                    .ok_or("Menu material handle overflow")?;
                let rgb = match kind {
                    8 => [224, 224, 255],
                    10 => [22, 28, 34],
                    _ => [150, 108, 213],
                };
                Resource {
                    asset: BodyAsset {
                        handle: self.next_handle,
                        revision: 1,
                        width: 1,
                        height: 1,
                        status: BodyAssetStatus::Ready,
                    },
                    image: materials::RgbaImage {
                        width: 1,
                        height: 1,
                        pixels: vec![rgb[0], rgb[1], rgb[2], 255],
                    },
                }
            };
            self.materials.insert(key, resource);
        }
        Ok(image(&self.materials[&key], r, alpha))
    }
    fn solid(&mut self, r: MenuRect, kind: u8, alpha: f64) -> Result<()> {
        let draw = self.solid_draw(r, kind, alpha)?;
        self.push(draw)
    }
    fn push(&mut self, draw: Draw) -> Result<()> {
        check_menu_draws(self.draws.len() + 1)?;
        frame::push_draw(&mut self.draws, draw)
    }
    fn control(&mut self, r: MenuRect, kind: u8, alpha: f64) -> Result<()> {
        // Capsules need few pixels; keep their complete shadow inside the
        // renderer's existing 512px texture limit. Text retains its 3x raster.
        let surface = match kind {
            0 => materials::MenuSurface::Play,
            1..=2 => materials::MenuSurface::Overlay,
            5 => materials::MenuSurface::Nickname,
            _ => materials::MenuSurface::Compact,
        };
        // Include the broader nickname shadow and the menu's outgoing growth;
        // leave room for the three independent ceil operations below.
        let scale = self
            .scale
            .min(1.5)
            .min(509. / (r.width.max(r.height) + 2. * surface.padding()));
        let size = [
            (r.width * scale).ceil() as u32,
            (r.height * scale).ceil() as u32,
        ];
        let key = (size[0], size[1], kind, scale.to_bits());
        if !self.materials.contains_key(&key) {
            let resource = if let Some(r) = self.old.get(&key) {
                r.clone()
            } else {
                let colors = match kind {
                    0 => [[86, 172, 129], [54, 108, 81]],
                    1 => [[117, 240, 202], [72, 171, 132]],
                    2 => [[12, 20, 18], [40, 59, 54]],
                    4 | 5 => [[76, 68, 124], [76, 68, 124]],
                    6 => [[108, 215, 128], [80, 160, 100]],
                    _ => [[150, 108, 213], [100, 70, 150]],
                };
                let image =
                    materials::menu_control(size, f64::from(size[1]) / 2., colors, scale, surface)?;
                self.next_handle = self
                    .next_handle
                    .checked_add(1)
                    .filter(|h| *h < MENU_MATERIAL_HANDLE_BASE + crate::bank::HANDLE_NAMESPACE_SIZE)
                    .ok_or("Menu material handle overflow")?;
                Resource {
                    asset: BodyAsset {
                        handle: self.next_handle,
                        revision: 1,
                        width: image.width,
                        height: image.height,
                        status: BodyAssetStatus::Ready,
                    },
                    image,
                }
            };
            self.materials.insert(key, resource);
        }
        let pad = (surface.padding() * scale).ceil() / scale;
        let resource = &self.materials[&key];
        let r = MenuRect {
            x: r.x - pad,
            y: r.y - pad,
            width: f64::from(resource.image.width) / scale,
            height: f64::from(resource.image.height) / scale,
        };
        self.push(image(resource, r, alpha))
    }
    fn label(&mut self, text: &str, r: MenuRect, size: f64, bold: bool, alpha: f64) {
        self.labels.push(Label {
            text: text.into(),
            rect: r,
            style: TextStyle {
                size,
                weight: if bold { Weight::Bold } else { Weight::Normal },
                family: Family::Arial,
                slant: Slant::Normal,
                color: [224, 224, 255],
            },
            alpha,
            align: Align::Center,
        });
    }
    fn art(&mut self, resource: &Resource, r: MenuRect, alpha: f64) -> Result<()> {
        self.push(image(resource, r, alpha))?;
        self.art.push(resource.clone());
        Ok(())
    }
}
impl Menu {
    /// Raster scale is bounded at 3x. Geometry comes only from the core layout;
    /// a caller supplies the selected logo frame, never a filesystem path.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        text: &mut TextCache,
        view: &StartMenuView,
        layout: &MenuLayout,
        extent: [u32; 2],
        dpr: f64,
        cosmetics: Option<&Cosmetics>,
        logo_frame: u8,
    ) -> Result<MenuFrame> {
        self.prepare_with_input(text, view, layout, extent, dpr, cosmetics, logo_frame, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_input(
        &mut self,
        text: &mut TextCache,
        view: &StartMenuView,
        layout: &MenuLayout,
        extent: [u32; 2],
        dpr: f64,
        cosmetics: Option<&Cosmetics>,
        logo_frame: u8,
        input: Option<&slither_client::menu_input::MenuInputState>,
    ) -> Result<MenuFrame> {
        crate::render::dimensions(extent, dpr)?;
        let mut c = Composition {
            old: &self.materials,
            materials: BTreeMap::new(),
            next_handle: self.next_handle.max(MENU_MATERIAL_HANDLE_BASE),
            scale: dpr.min(3.),
            draws: Vec::new(),
            art: Vec::new(),
            labels: Vec::new(),
        };
        let a = view.login_alpha;
        let backdrop = if view.visible {
            Some(c.solid_draw(
                MenuRect {
                    x: 0.,
                    y: 0.,
                    width: f64::from(extent[0]) / dpr,
                    height: f64::from(extent[1]) / dpr,
                },
                10,
                1.,
            )?)
        } else {
            None
        };
        if view.visible {
            let qr = layout.viewport_rect(E::Quality);
            if self.focus == Some(E::Quality) {
                c.control(qr, 4, a * 0.35)?;
            }
            let role = match view.quality {
                slither_client::menu_preferences::MenuQuality::High => MenuArt::QualityHigh,
                slither_client::menu_preferences::MenuQuality::Low => MenuArt::QualityLow,
            };
            if let Some(art) = cosmetics.and_then(|p| p.menu_art(role)) {
                c.art(art, qr, a)?;
            } else {
                c.label(
                    if role == MenuArt::QualityHigh {
                        "High quality"
                    } else {
                        "Low quality"
                    },
                    qr,
                    14.,
                    false,
                    a,
                );
            }
            let logo = layout.viewport_rect(E::Logo);
            let logo_alpha = a * view.logo_alpha * view.logo_reveal;
            if let Some(art) = cosmetics.and_then(|p| p.menu_art(MenuArt::Logo(logo_frame))) {
                c.art(
                    art,
                    MenuRect {
                        height: logo.width * f64::from(art.asset.height)
                            / f64::from(art.asset.width),
                        ..logo
                    },
                    logo_alpha,
                )?;
            } else {
                c.label("slither", logo, 40. * layout.scale, true, logo_alpha);
                c.labels.last_mut().unwrap().style.color = [130, 214, 91];
            }
            if view.tips_visible {
                c.label(
                    view.tip,
                    layout.viewport_rect(E::Tips),
                    14. * layout.scale,
                    false,
                    a * view.tip_alpha,
                );
                let label = c.labels.last_mut().unwrap();
                label.style.family = Family::Lucida;
                label.style.color = [128, 104, 192];
            }
            if view.ordinary_controls_visible {
                let nick = layout.viewport_rect(E::NicknameHolder);
                c.control(nick, 5, a * view.nickname_alpha)?;
                c.label(
                    if view.nickname.is_empty() {
                        view.wording.nickname
                    } else {
                        &view.nickname
                    },
                    layout.viewport_rect(E::Nickname),
                    15. * layout.scale,
                    false,
                    a * view.nickname_alpha,
                );
                c.labels.last_mut().unwrap().align = Align::Left;
                c.labels.last_mut().unwrap().style.family = Family::Lucida;
                if view.nickname.is_empty() {
                    c.labels.last_mut().unwrap().style.color = [112, 112, 176];
                }
                if let Some(input) = input
                    .filter(|i| i.focus() == Some(slither_client::menu_input::TextField::Nickname))
                {
                    let editor = input.editor(slither_client::menu_input::TextField::Nickname);
                    let (anchor, caret) = editor.selection();
                    let r = layout.viewport_rect(E::Nickname);
                    let style = c.labels.last().unwrap().style;
                    let full = text.measure_panel(&view.nickname, style)?.advance;
                    let fit = if full > r.width { r.width / full } else { 1. };
                    let advance = |prefix: &str, text: &mut TextCache| -> Result<f64> {
                        Ok(text.measure_panel(prefix, style)?.advance * fit)
                    };
                    let left = advance(&editor.text()[..anchor.min(caret)], text)?;
                    let right = advance(&editor.text()[..anchor.max(caret)], text)?;
                    if anchor != caret && editor.preedit().is_none() {
                        c.solid(
                            MenuRect {
                                x: r.x + left,
                                y: r.y,
                                width: (right - left).max(1.),
                                height: r.height,
                            },
                            9,
                            0.45 * a * view.nickname_alpha,
                        )?;
                    }
                    let cursor = if let Some(preedit) = editor.preedit() {
                        let end = preedit.selection.map_or(preedit.text.len(), |(_, end)| end);
                        advance(&editor.text()[..anchor.min(caret)], text)?
                            + advance(&preedit.text[..end], text)?
                    } else {
                        advance(&editor.text()[..caret], text)?
                    };
                    c.solid(
                        MenuRect {
                            x: r.x + cursor.min(r.width - 1.),
                            y: r.y,
                            width: 1.,
                            height: r.height,
                        },
                        8,
                        a * view.nickname_alpha,
                    )?;
                    if let Some(preedit) = editor.preedit() {
                        let start = advance(&editor.text()[..anchor.min(caret)], text)?;
                        let width = advance(&preedit.text, text)?.min((r.width - start).max(0.));
                        if width > 0. {
                            c.solid(
                                MenuRect {
                                    x: r.x + start,
                                    y: r.y + r.height - 2.,
                                    width,
                                    height: 1.,
                                },
                                8,
                                a * view.nickname_alpha,
                            )?;
                        }
                    }
                }
                let play = layout.viewport_rect(E::Play);
                let pa = a
                    * view.play_alpha
                    * view.play_return_alpha
                    * if view.disabled { 0.38 } else { 1. };
                c.control(play, 0, pa)?;
                if !view.disabled {
                    c.control(
                        play,
                        1,
                        pa * view.hover_alpha.max(if self.focus == Some(E::Play) {
                            0.33
                        } else {
                            0.
                        }),
                    )?;
                    c.control(play, 2, pa * view.press_alpha)?;
                }
                c.label(view.play_label, play, 20. * layout.scale, true, pa);
                c.labels.last_mut().unwrap().style.color = [255; 3];
                let r = layout.viewport_rect(E::Skin);
                let skin_alpha = a
                    * view.skin_alpha
                    * if self.skin_pressed && self.skin_hovered {
                        0.5
                    } else if self.skin_hovered {
                        1.
                    } else {
                        0.7
                    };
                if let Some(art) = cosmetics.and_then(|p| p.menu_art(MenuArt::Skin)) {
                    c.art(art, r, skin_alpha)?;
                } else {
                    c.control(
                        MenuRect {
                            x: r.x + 23.,
                            y: r.y + 36.,
                            width: 48.,
                            height: 25.,
                        },
                        0,
                        skin_alpha,
                    )?;
                    c.label(
                        view.wording.change_skin,
                        MenuRect {
                            y: r.y + r.height - 24.,
                            height: 24.,
                            ..r
                        },
                        14.,
                        false,
                        skin_alpha,
                    );
                }
            }
        }
        if view.spinner_visible {
            let r = layout.viewport_rect(E::Spinner);
            let center = [r.x + r.width / 2., r.y + r.height / 2.];
            for n in 0..16 {
                let phase = n as f64 * std::f64::consts::TAU / 16. + view.spinner_time * 0.05;
                let radius = 40. * view.spinner_scale;
                let side = 10. * view.spinner_scale;
                if side > 0. {
                    c.control(
                        MenuRect {
                            x: center[0] + radius * phase.cos() - side / 2.,
                            y: center[1] + radius * phase.sin() - side / 2.,
                            width: side,
                            height: side,
                        },
                        if n < 8 { 6 } else { 7 },
                        view.spinner_alpha * (0.35 + 0.65 * n as f64 / 15.),
                    )?;
                }
            }
        }
        let material_bytes = c
            .materials
            .values()
            .map(|r| r.image.pixels.len())
            .sum::<usize>();
        // Essential controls belong to the Menu allowance. Optional cosmetics
        // must not consume the capacity needed by Play, nickname or fallbacks.
        // Measure replacement faces normally. Fit full labels into their authored
        // boxes, without synthetic bold or browser-specific metric machinery.
        for l in &mut c.labels {
            l.style.size = l.style.size.min(128. / c.scale);
            let width = text.measure_panel(&l.text, l.style)?.advance;
            if width > l.rect.width {
                l.style.size *= l.rect.width / width;
            }
        }
        let runs: Vec<_> = c
            .labels
            .iter()
            .map(|l| TextRun {
                text: &l.text,
                style: l.style,
                position: [
                    l.rect.x
                        + if l.align == Align::Left {
                            0.
                        } else {
                            l.rect.width / 2.
                        },
                    l.rect.y + l.rect.height / 2.,
                ],
                align: l.align,
                baseline: Baseline::Middle,
                alpha: l.alpha,
            })
            .collect();
        let result = text.compose_menu(
            &runs,
            c.scale,
            (c.materials.len(), material_bytes),
            |groups| {
                let mut resources: BTreeMap<_, _> = c
                    .materials
                    .values()
                    .chain(&c.art)
                    .map(|r| (frame::ResourceKey::from(r.asset), r.clone()))
                    .collect();
                for group in groups {
                    for (draw, resource) in group {
                        check_menu_draws(c.draws.len() + 1)?;
                        frame::push_draw(&mut c.draws, draw)?;
                        resources.insert(resource.asset.into(), resource);
                    }
                }
                check_menu_draws(c.draws.len() + usize::from(backdrop.is_some()))?;
                let frame = Frame {
                    identity: 0,
                    extent,
                    viewport_scale: dpr,
                    tick: 0,
                    draws: c.draws,
                    snakes: Vec::new(),
                };
                frame.validate(&resources.iter().map(|(k, r)| (*k, r.asset)).collect())?;
                Ok(MenuFrame {
                    frame,
                    backdrop,
                    resources: resources.into_values().collect(),
                })
            },
        )?;
        self.next_handle = c.next_handle;
        self.materials = c.materials;
        Ok(result)
    }
}
