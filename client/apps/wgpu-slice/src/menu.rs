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
const MATERIAL_HANDLE_BASE: u64 = 0x7000_0000_0000_0000;
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
    pub resources: Vec<Resource>,
}
impl MenuFrame {
    /// Append in paint order. A failed whole-picture admission publishes nothing.
    pub fn append_to(self, picture: &mut PreparedFrame) -> Result<()> {
        check_menu_draws(self.frame.draws.len())?;
        if self.frame.extent != picture.frame.extent
            || self.frame.viewport_scale != picture.frame.viewport_scale
        {
            return Err("Menu picture extent/scale mismatch".into());
        }
        let mut resources: BTreeMap<_, _> = picture
            .resources
            .iter()
            .chain(&self.resources)
            .map(|r| (frame::ResourceKey::from(r.asset), r.clone()))
            .collect();
        if resources.len() > 2048
            || resources
                .values()
                .map(|r| r.image.pixels.len())
                .sum::<usize>()
                > crate::bank::PIXEL_BUDGET
        {
            return Err("Menu whole-picture pixel budget".into());
        }
        frame::reserve_draws(&mut picture.frame.draws, self.frame.draws.len())?;
        picture.frame.draws.extend(self.frame.draws);
        picture.resources = std::mem::take(&mut resources).into_values().collect();
        Ok(())
    }
}
#[derive(Default)]
pub struct Menu {
    pub focus: Option<E>,
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
    fn push(&mut self, draw: Draw) -> Result<()> {
        check_menu_draws(self.draws.len() + 1)?;
        frame::push_draw(&mut self.draws, draw)
    }
    fn control(&mut self, r: MenuRect, kind: u8, alpha: f64) -> Result<()> {
        // Capsules need few pixels; keep their complete shadow inside the
        // renderer's existing 512px texture limit. Text retains its 3x raster.
        let scale = self.scale.min(1.5);
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
                    3 => [[75, 91, 84], [50, 65, 58]],
                    4 => [[76, 68, 124], [76, 68, 124]],
                    5 => [[0, 0, 0], [0, 0, 0]],
                    6 => [[108, 215, 128], [80, 160, 100]],
                    _ => [[150, 108, 213], [100, 70, 150]],
                };
                let image = materials::menu_control(size, f64::from(size[1]) / 2., colors, scale)?;
                self.next_handle = self
                    .next_handle
                    .checked_add(1)
                    .filter(|h| *h < MATERIAL_HANDLE_BASE + crate::bank::HANDLE_NAMESPACE_SIZE)
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
        let pad = (12. * scale).ceil() / scale;
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
            next_handle: self.next_handle.max(MATERIAL_HANDLE_BASE),
            scale: dpr.min(3.),
            draws: Vec::new(),
            art: Vec::new(),
            labels: Vec::new(),
        };
        let a = view.login_alpha;
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
                c.art(art, logo, logo_alpha)?;
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
            }
            if view.ordinary_controls_visible {
                let nick = layout.viewport_rect(E::NicknameHolder);
                c.control(nick, 5, a * view.nickname_alpha)?;
                c.control(
                    MenuRect {
                        x: nick.x + 2. * layout.scale,
                        y: nick.y + 2. * layout.scale,
                        width: nick.width - 4. * layout.scale,
                        height: nick.height - 4. * layout.scale,
                    },
                    4,
                    a * view.nickname_alpha,
                )?;
                c.label(
                    if view.nickname.is_empty() {
                        view.wording.nickname
                    } else {
                        &view.nickname
                    },
                    layout.viewport_rect(E::Nickname),
                    15. * layout.scale,
                    false,
                    a * view.nickname_alpha * if view.disabled { 0.5 } else { 1. },
                );
                c.labels.last_mut().unwrap().align = Align::Left;
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
                        c.control(
                            MenuRect {
                                x: r.x + left,
                                y: r.y,
                                width: (right - left).max(1.),
                                height: r.height,
                            },
                            7,
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
                    c.control(
                        MenuRect {
                            x: r.x + cursor.min(r.width - 1.),
                            y: r.y,
                            width: 1.,
                            height: r.height,
                        },
                        6,
                        a * view.nickname_alpha,
                    )?;
                    if let Some(preedit) = editor.preedit() {
                        let start = advance(&editor.text()[..anchor.min(caret)], text)?;
                        let width = advance(&preedit.text, text)?.min((r.width - start).max(0.));
                        if width > 0. {
                            c.control(
                                MenuRect {
                                    x: r.x + start,
                                    y: r.y + r.height - 2.,
                                    width,
                                    height: 1.,
                                },
                                6,
                                a * view.nickname_alpha,
                            )?;
                        }
                    }
                }
                let play = layout.viewport_rect(E::Play);
                let pa = a * view.play_alpha * view.play_return_alpha;
                c.control(play, if view.disabled { 3 } else { 0 }, pa)?;
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
                c.label(
                    view.play_label,
                    play,
                    20. * layout.scale,
                    true,
                    pa * if view.disabled { 0.45 } else { 1. },
                );
                c.labels.last_mut().unwrap().style.color = [255; 3];
                let r = layout.viewport_rect(E::Skin);
                if let Some(art) = cosmetics.and_then(|p| p.menu_art(MenuArt::Skin)) {
                    c.art(art, r, a * view.skin_alpha)?;
                } else {
                    c.control(
                        MenuRect {
                            x: r.x + 23.,
                            y: r.y + 10.,
                            width: 48.,
                            height: 25.,
                        },
                        0,
                        a * view.skin_alpha,
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
                        a * view.skin_alpha,
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
                    resources: resources.into_values().collect(),
                })
            },
        )?;
        self.next_handle = c.next_handle;
        self.materials = c.materials;
        Ok(result)
    }
}
