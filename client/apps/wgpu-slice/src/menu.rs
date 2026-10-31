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
    menu_controls::{ControlKind, PanelInputState, PanelView, ResolvedControl, intersection},
    menu_input::{EditorId, MenuInputState},
    menu_layout::{MenuElement as E, MenuLayout, MenuRect, single_line_field_placement},
    menu_theme::{ControlState, ControlStyle, MenuTheme, SurfaceRecipe, SurfaceTokens},
    presentation::*,
    start_page::StartMenuView,
};
use std::collections::BTreeMap;

pub const MENU_PRODUCER_DRAWS: usize = 512;
pub(crate) const MENU_DRAW_QUOTA: &str = "Menu producer draw budget";
pub(crate) const MENU_PICTURE_QUOTA: &str = "Menu whole-picture pixel budget";
pub(crate) const MENU_RASTER_AXIS_QUOTA: &str = "Sliced menu raster axis budget";
pub(crate) const MENU_SOLID_TEXTURE_QUOTA: &str = "Menu solid texture budget";
pub(crate) const MENU_ART_AXIS_QUOTA: &str = "Menu art destination budget";
pub(crate) const MENU_ART_PIXEL_QUOTA: &str = "Menu art pixel budget";

use crate::bank::MENU_MATERIAL_HANDLE_BASE;
// Reserved menu-material handles; the generic allocator starts above them.
const SPINNER_HANDLE: u64 = MENU_MATERIAL_HANDLE_BASE;
const PAGE_HANDLE: u64 = MENU_MATERIAL_HANDLE_BASE + 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum MaterialKey {
    Legacy(u32, u32, u8, u64, u64),
    Control([u32; 2], u64, SurfaceTokens, [u32; 2]),
    Solid([u32; 2], [u8; 3]),
}
const PAGE_COLOR: [u8; 3] = [22, 28, 34];
pub const WORDMARK_GREEN: [u8; 3] = [86, 172, 129];
pub const WORDMARK_PURPLE: [u8; 3] = [150, 76, 224];
// Content padding inside the already border-free native input rect.
pub const FIELD_CONTENT_INSET: [f64; 2] = [2., 0.];

pub fn check_menu_draws(count: usize) -> Result<()> {
    if count > MENU_PRODUCER_DRAWS {
        return Err(MENU_DRAW_QUOTA.into());
    }
    Ok(())
}
/// Immutable prepared resources survive subsequent resize, retirement and art replacement.
pub struct MenuFrame {
    pub frame: Frame,
    /// Page colour below the world, in the frame's coordinate space; absent when login is hidden.
    pub backdrop: Option<Draw>,
    /// Crossfade cover after the world blit, before the independently fading HUD.
    pub world_cover: Option<Draw>,
    pub resources: Vec<Resource>,
}
impl MenuFrame {
    /// A menu quota failure skips this overlay for the frame, preserving the
    /// world picture and editor/session state. Invalid geometry still surfaces.
    pub fn append_optional(menu: Result<Self>, picture: &mut PreparedFrame) -> Result<bool> {
        Self::admit_quota(menu.and_then(|menu| menu.append_to(picture))).map(|v| v.is_some())
    }
    /// Preserve the retry picture and #359's single-allocation composition.
    pub fn compose_optional(
        menu: Result<Self>,
        picture: &PreparedFrame,
    ) -> Result<Option<PreparedFrame>> {
        Self::admit_quota(menu.and_then(|menu| menu.compose_over(picture)))
    }
    fn admit_quota<T>(result: Result<T>) -> Result<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            Err(error)
                if matches!(
                    error.as_str(),
                    MENU_DRAW_QUOTA
                        | MENU_PICTURE_QUOTA
                        | MENU_RASTER_AXIS_QUOTA
                        | MENU_SOLID_TEXTURE_QUOTA
                        | MENU_ART_AXIS_QUOTA
                        | MENU_ART_PIXEL_QUOTA
                        | MENU_CACHE_QUOTA
                        | PANEL_TOTALS_QUOTA
                        | PANEL_SLOT_QUOTA
                        | PANEL_DRAW_QUOTA
                        | HUD_SLOT_QUOTA
                        | TEXT_CACHE_RUN_QUOTA
                        | TEXT_CACHE_PIXEL_QUOTA
                        | TEXT_CACHE_QUOTA
                        | TEXT_RASTER_QUOTA
                        | materials::MENU_TEXTURE_QUOTA
                        | frame::DRAW_MEMORY_LIMIT
                        | frame::DRAW_ALLOCATION_FAILED
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// DOM login hosts need the same page/world composition without native controls.
    pub fn world_crossfade(extent: [u32; 2], scale: f64, world_alpha: f64) -> Result<Self> {
        crate::render::dimensions(extent, scale)?;
        if !world_alpha.is_finite() || !(0. ..=1.).contains(&world_alpha) {
            return Err("Invalid world opacity".into());
        }
        let resource = Resource {
            asset: BodyAsset {
                handle: PAGE_HANDLE,
                revision: 1,
                width: 1,
                height: 1,
                status: BodyAssetStatus::Ready,
            },
            image: materials::RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![PAGE_COLOR[0], PAGE_COLOR[1], PAGE_COLOR[2], 255],
            }
            .into(),
        };
        let backdrop = image(
            &resource,
            MenuRect {
                x: 0.,
                y: 0.,
                width: f64::from(extent[0]) / scale,
                height: f64::from(extent[1]) / scale,
            },
            1.,
        );
        let mut cover = backdrop.clone();
        cover.effective_alpha = 1. - world_alpha;
        Ok(Self {
            frame: Frame {
                identity: 0,
                tick: 0,
                extent,
                viewport_scale: scale,
                draws: Vec::new(),
                snakes: Vec::new(),
            },
            backdrop: Some(backdrop),
            world_cover: Some(cover),
            resources: vec![resource],
        })
    }

    /// Diagnostic backing composition for retained comparisons. The native
    /// host submits the physical menu directly; input/IME stay in CSS pixels.
    pub fn into_backing(mut self, extent: [u32; 2]) -> Result<Self> {
        crate::render::dimensions(extent, 1.)?;
        let scale = [
            f64::from(extent[0]) * self.frame.viewport_scale / f64::from(self.frame.extent[0]),
            f64::from(extent[1]) * self.frame.viewport_scale / f64::from(self.frame.extent[1]),
        ];
        for draw in self
            .frame
            .draws
            .iter_mut()
            .chain(&mut self.backdrop)
            .chain(&mut self.world_cover)
        {
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
    /// Keep the retry picture intact and allocate its composed draw storage once.
    pub fn compose_over(self, picture: &PreparedFrame) -> Result<PreparedFrame> {
        let world_extra = usize::from(self.backdrop.is_some());
        let ui_extra = self.frame.draws.len() + usize::from(self.world_cover.is_some());
        check_menu_draws(world_extra + ui_extra)?;
        frame::check_draw_count(picture.draw_count() + world_extra + ui_extra)?;
        let copy = |frame: &Frame, extra: usize| -> Result<Frame> {
            let mut draws = Vec::new();
            draws
                .try_reserve_exact(frame.draws.len() + extra)
                .map_err(|_| frame::DRAW_ALLOCATION_FAILED)?;
            draws.extend_from_slice(&frame.draws);
            Ok(Frame {
                identity: frame.identity,
                extent: frame.extent,
                viewport_scale: frame.viewport_scale,
                tick: frame.tick,
                draws,
                snakes: frame.snakes.clone(),
            })
        };
        let mut composed = PreparedFrame {
            frame: copy(
                &picture.frame,
                world_extra
                    + if picture.overlay.is_none() {
                        ui_extra
                    } else {
                        0
                    },
            )?,
            overlay: picture
                .overlay
                .as_ref()
                .map(|f| copy(f, ui_extra))
                .transpose()?,
            resources: picture.resources.clone(),
        };
        self.append_to(&mut composed)?;
        Ok(composed)
    }
    /// Put the page below every world draw and menu controls above them.
    /// A failed whole-picture admission publishes nothing.
    pub fn append_to(self, picture: &mut PreparedFrame) -> Result<()> {
        let count = self.frame.draws.len()
            + usize::from(self.backdrop.is_some())
            + usize::from(self.world_cover.is_some());
        check_menu_draws(count)?;
        if self.frame.extent != picture.ui_frame().extent
            || self.frame.viewport_scale != picture.ui_frame().viewport_scale
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
            return Err(MENU_PICTURE_QUOTA.into());
        }
        frame::check_draw_count(picture.draw_count() + count)?;
        if let Some(overlay) = &mut picture.overlay {
            frame::reserve_draws(
                &mut overlay.draws,
                self.frame.draws.len() + usize::from(self.world_cover.is_some()),
            )?;
            frame::reserve_draws(
                &mut picture.frame.draws,
                usize::from(self.backdrop.is_some()),
            )?;
        } else {
            frame::reserve_draws(&mut picture.frame.draws, count)?;
        }
        if let Some(mut backdrop) = self.backdrop {
            // The page fills the world backing; controls keep their physical UI coordinates.
            backdrop.destination = BodyRect {
                x: 0.,
                y: 0.,
                width: f64::from(picture.frame.extent[0]) / picture.frame.viewport_scale,
                height: f64::from(picture.frame.extent[1]) / picture.frame.viewport_scale,
            };
            picture.frame.draws.insert(0, backdrop);
        }
        if let Some(cover) = self.world_cover {
            if let Some(overlay) = &mut picture.overlay {
                overlay.draws.insert(0, cover);
            } else {
                picture.frame.draws.push(cover);
            }
        }
        picture.ui_frame_mut().draws.extend(self.frame.draws);
        picture.resources = std::mem::take(&mut resources).into_values().collect();
        Ok(())
    }
}
#[derive(Default)]
pub struct Menu {
    pub wordmark: bool,
    pub focus: Option<E>,
    pub skin_hovered: bool,
    pub skin_pressed: bool,
    materials: BTreeMap<MaterialKey, Resource>,
    art: BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
    next_handle: u64,
    spinner_revision: u64,
}
struct Label {
    text: String,
    rect: MenuRect,
    style: TextStyle,
    alpha: f64,
    align: Align,
    reveal: f64,
    reveal_span: Option<[f64; 2]>,
    line_box: bool,
    field_inset: Option<[f64; 2]>,
    clip: Option<MenuRect>,
    fit: bool,
}
/// Two bold runs with one fitted size and a shared advance boundary.
pub fn wordmark_runs(
    text: &mut TextCache,
    logo: MenuRect,
    dpr: f64,
) -> Result<[(MenuRect, TextStyle); 2]> {
    let mut style = TextStyle {
        size: logo.height,
        weight: Weight::Bold,
        family: Family::Arial,
        slant: Slant::Normal,
        color: WORDMARK_GREEN,
    };
    let cap = text.measure_panel("H", style)?.ascent;
    style.size *= logo.height * 0.8 / cap;
    let width = text.measure_panel("Slither", style)?.advance
        + text.measure_panel("Client", style)?.advance;
    style.size *= (logo.width / width).min(1.);
    let mut physical = style;
    physical.size *= dpr;
    let left_px = text.measure_panel("Slither", physical)?.advance;
    let right_px = text.measure_panel("Client", physical)?.advance;
    // Snap the common origin and advance once. Rounding each run's origin
    // independently can move the join one physical pixel in either direction.
    let origin_px = ((logo.x + logo.width / 2.) * dpr - (left_px + right_px) / 2.).round();
    let boundary_px = origin_px + left_px.round();
    Ok([
        (
            MenuRect {
                x: origin_px / dpr,
                width: left_px / dpr,
                ..logo
            },
            style,
        ),
        (
            MenuRect {
                x: boundary_px / dpr,
                width: right_px / dpr,
                ..logo
            },
            TextStyle {
                color: WORDMARK_PURPLE,
                ..style
            },
        ),
    ])
}
// Authored wipe: one opaque crop plus at most 32 one-pixel feather strips.
// Source crops and destinations remain 1:1 on the snapped overlay path.
fn reveal_draw(mut draw: Draw, reveal: f64, scale: f64, span: Option<[f64; 2]>) -> Vec<Draw> {
    draw.snap_overlay(scale);
    if reveal >= 1. {
        return vec![draw];
    }
    if reveal <= 0. {
        return Vec::new();
    }
    let source = draw.source.unwrap_or(BodyRect {
        x: 0.,
        y: 0.,
        width: f64::from(draw.asset.width),
        height: f64::from(draw.asset.height),
    });
    let feather = (12. * scale).round().clamp(1., 32.);
    let span = span.unwrap_or([draw.destination.x, source.width / scale]);
    let end = ((span[1] * scale + feather) * reveal).round()
        - ((draw.destination.x - span[0]) * scale).round();
    let solid = (end - feather).max(0.).min(source.width);
    let mut result = Vec::new();
    let mut crop = |x: f64, width: f64, alpha: f64| {
        let mut part = draw.clone();
        part.source = Some(BodyRect {
            x: source.x + x,
            width,
            ..source
        });
        part.destination.x += x / scale;
        part.destination.width = width / scale;
        part.effective_alpha *= alpha;
        result.push(part);
    };
    if solid > 0. {
        crop(0., solid, 1.);
    }
    for x in solid as u32..(end.min(source.width) as u32) {
        crop(
            f64::from(x),
            1.,
            ((end - f64::from(x) - 0.5) / feather).clamp(0., 1.),
        );
    }
    result
}

struct Composition<'a> {
    old: &'a BTreeMap<MaterialKey, Resource>,
    old_art: &'a BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
    materials: BTreeMap<MaterialKey, Resource>,
    next_handle: u64,
    spinner_revision: u64,
    scale: f64,
    draws: Vec<Draw>,
    art: BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
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
pub struct PanelPaint<'a> {
    pub panel: &'a PanelView,
    pub input: &'a PanelInputState,
    pub edit: &'a MenuInputState,
    pub scroll: &'a mut BTreeMap<EditorId, f64>,
    pub theme: MenuTheme,
}
impl Composition<'_> {
    #[allow(clippy::too_many_arguments)]
    fn panel(
        &mut self,
        text: &mut TextCache,
        panel: &PanelView,
        input: &PanelInputState,
        edit: &MenuInputState,
        scroll: &mut BTreeMap<EditorId, f64>,
        theme: MenuTheme,
        resolved: &[ResolvedControl],
    ) -> Result<()> {
        if panel.visible {
            for r in resolved.iter().filter(|r| r.control.visible) {
                let control = &r.control;
                let enabled = panel.enabled && control.enabled;
                let alpha = panel.alpha
                    * control.alpha
                    * if enabled || matches!(control.kind, ControlKind::Label { .. }) {
                        1.
                    } else {
                        theme.disabled_alpha
                    };
                let mut style = theme.text_style(control.style);
                style.size *= r.scale;
                // The panel's layout box still bounds text/hits. Decoration may
                // use its complete padding unless the description explicitly clips it.
                let surface_clip = control.clip.map(|_| r.clip);
                let add_label = |c: &mut Composition<'_>,
                                 value: String,
                                 rect: MenuRect,
                                 style: TextStyle,
                                 align: Align,
                                 clip: MenuRect| {
                    c.labels.push(Label {
                        text: value,
                        rect,
                        style,
                        alpha,
                        align,
                        reveal: 1.,
                        reveal_span: None,
                        line_box: false,
                        field_inset: None,
                        clip: Some(clip),
                        fit: false,
                    });
                };
                match &control.kind {
                    ControlKind::Label { text: value } => {
                        add_label(self, value.clone(), r.rect, style, Align::Center, r.clip)
                    }
                    ControlKind::Button { label } => {
                        self.sliced(
                            r.rect,
                            theme.surface(control.style, ControlState::Idle),
                            alpha,
                            surface_clip,
                            r.scale,
                        )?;
                        let [hover, press] = input.progress(control.id);
                        if enabled {
                            self.sliced(
                                r.rect,
                                theme.surface(control.style, ControlState::Hovered),
                                alpha
                                    * hover.max(if input.focus() == Some(control.id) {
                                        0.33
                                    } else {
                                        0.
                                    }),
                                surface_clip,
                                r.scale,
                            )?;
                            self.sliced(
                                r.rect,
                                theme.surface(control.style, ControlState::Pressed),
                                alpha * press,
                                surface_clip,
                                r.scale,
                            )?;
                        }
                        add_label(self, label.clone(), r.rect, style, Align::Center, r.clip);
                    }
                    ControlKind::TextField {
                        editor,
                        placeholder,
                        inset,
                        ..
                    } => {
                        self.sliced(
                            r.rect,
                            theme.surface(control.style, ControlState::Idle),
                            alpha,
                            surface_clip,
                            r.scale,
                        )?;
                        let inset = inset.map(|n| n * r.scale);
                        let editor_state = edit
                            .registered_editor(*editor)
                            .ok_or("Unregistered panel editor")?;
                        let g = crate::editor_geometry::EditorGeometry::new(
                            text,
                            editor_state,
                            r.rect,
                            inset,
                            style,
                            scroll.get(editor).copied().unwrap_or(0.),
                        )?;
                        let clip = intersection(r.clip, g.rect);
                        scroll.insert(*editor, g.scroll);
                        if input.focus() == Some(control.id) && enabled {
                            self.ink(g.selection_rect(), theme.selection, alpha * 0.45, clip)?;
                            if edit.caret_visible() {
                                self.ink(
                                    g.snapped_cursor_rect(self.scale),
                                    theme.text,
                                    alpha,
                                    clip,
                                )?;
                            }
                            if let Some(underline) = g.preedit_rect() {
                                self.ink(underline, theme.text, alpha, clip)?;
                            }
                        }
                        let value = if g.display.is_empty() {
                            style.color = theme.placeholder;
                            placeholder.clone()
                        } else {
                            g.display
                        };
                        add_label(
                            self,
                            value,
                            MenuRect {
                                x: r.rect.x - g.scroll,
                                ..r.rect
                            },
                            style,
                            Align::Left,
                            clip,
                        );
                        self.labels.last_mut().unwrap().field_inset = Some(inset);
                    }
                }
            }
        }
        Ok(())
    }

    fn cached_material(
        &mut self,
        key: MaterialKey,
        make: impl FnOnce() -> Result<materials::RgbaImage>,
    ) -> Result<Resource> {
        if let Some(r) = self.materials.get(&key) {
            return Ok(r.clone());
        }
        let resource = if let Some(r) = self.old.get(&key) {
            r.clone()
        } else {
            let image = make()?;
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
                image: image.into(),
            }
        };
        preflight_menu_totals(
            self.materials.len() + 1,
            self.materials
                .values()
                .map(|r| r.image.pixels.len())
                .sum::<usize>()
                + resource.image.pixels.len(),
        )?;
        self.materials.insert(key, resource.clone());
        Ok(resource)
    }
    // Tile uniform middle strips in physical pixels. Only the bounded corner
    // atlas is rasterized; no wide field or tall surface becomes one texture.
    fn sliced(
        &mut self,
        r: MenuRect,
        tokens: SurfaceTokens,
        alpha: f64,
        clip: Option<MenuRect>,
        control_scale: f64,
    ) -> Result<()> {
        if alpha == 0. {
            return Ok(());
        }
        let scale = self.scale;
        let raster_scale = scale * control_scale;
        if !raster_scale.is_finite() || !(0. ..=16.).contains(&raster_scale) || raster_scale == 0. {
            return Err("Sliced menu scale budget".into());
        }
        let surface = match tokens.recipe {
            SurfaceRecipe::Button => materials::MenuSurface::Play,
            SurfaceRecipe::Overlay => materials::MenuSurface::Overlay,
            SurfaceRecipe::Field => materials::MenuSurface::Nickname,
            SurfaceRecipe::Compact => materials::MenuSurface::Compact,
        };
        let pad = (surface.padding() * raster_scale).ceil() as u32;
        let radius = (f64::from(tokens.radius) * raster_scale).ceil() as u32;
        let size = [
            (r.width * scale).round().max(1.) as u32,
            (r.height * scale).round().max(1.) as u32,
        ];
        let middle = 32;
        let atlas = size.map(|n| n.min(2 * radius + middle));
        let padded = atlas.map(|n| n + 2 * pad);
        if padded.iter().any(|n| *n > 2048) {
            return Err(MENU_RASTER_AXIS_QUOTA.into());
        }
        // Every current surface is symmetric in X. Retain one half of the
        // padded atlas and mirror its quads; high-DPR shadows keep full padding
        // without retaining the same pixels twice.
        let half = padded[0].div_ceil(2);
        let mut raster = None;
        let mut tiles = Vec::new();
        for y in (0..padded[1]).step_by(512) {
            for x in (0..half).step_by(512) {
                let w = 512.min(half - x);
                let h = 512.min(padded[1] - y);
                let resource = self.cached_material(
                    MaterialKey::Control(atlas, raster_scale.to_bits(), tokens, [x, y]),
                    || {
                        if raster.is_none() {
                            raster = Some(materials::menu_control(
                                atlas,
                                f64::from(radius),
                                tokens.colors,
                                raster_scale,
                                surface,
                            )?);
                        }
                        let raster = raster.as_ref().expect("rasterized atlas");
                        let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
                        for row in y..y + h {
                            let start = ((row * padded[0] + x) * 4) as usize;
                            pixels.extend_from_slice(&raster.pixels[start..start + w as usize * 4]);
                        }
                        Ok(materials::RgbaImage {
                            width: w,
                            height: h,
                            pixels,
                        })
                    },
                )?;
                tiles.push((x, y, resource));
            }
        }
        let xs = slice_axis(size[0], atlas[0], pad)?;
        let ys = slice_axis(size[1], atlas[1], pad)?;
        check_menu_draws(xs.len() * ys.len())?;
        for &(dy, sy, h) in &ys {
            for &(dx, sx, w) in &xs {
                for (tx, ty, resource) in &tiles {
                    for mirrored in [false, true] {
                        let tile_left = if mirrored {
                            padded[0] - tx - resource.asset.width
                        } else {
                            *tx
                        };
                        let tile_right = tile_left + resource.asset.width;
                        let left = sx.max(tile_left).max(if mirrored { half } else { 0 });
                        let top = sy.max(*ty);
                        let right =
                            (sx + w)
                                .min(tile_right)
                                .min(if mirrored { padded[0] } else { half });
                        let bottom = (sy + h).min(ty + resource.asset.height);
                        if right <= left || bottom <= top {
                            continue;
                        }
                        let mut draw = image(
                            resource,
                            MenuRect {
                                x: r.x + (f64::from(dx + left - sx) - f64::from(pad)) / scale,
                                y: r.y + (f64::from(dy + top - sy) - f64::from(pad)) / scale,
                                width: f64::from(right - left) / scale,
                                height: f64::from(bottom - top) / scale,
                            },
                            alpha,
                        );
                        draw.source = Some(BodyRect {
                            x: f64::from(if mirrored {
                                tile_right - right
                            } else {
                                left - tx
                            }),
                            y: f64::from(top - ty),
                            width: f64::from(right - left),
                            height: f64::from(bottom - top),
                        });
                        draw.snap_overlay(scale);
                        let source = draw.source.unwrap();
                        if clip.is_none_or(|clip| clip_draw(&mut draw, clip, scale)) {
                            if mirrored {
                                // Clip in destination order, then reflect the source crop.
                                let crop = draw.source.as_mut().unwrap();
                                crop.x = 2. * source.x + source.width - crop.x - crop.width;
                                let d = draw.destination;
                                draw.corners = Some([
                                    [d.x + d.width, d.y],
                                    [d.x, d.y],
                                    [d.x, d.y + d.height],
                                    [d.x + d.width, d.y + d.height],
                                ]);
                            }
                            self.push(draw)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn ink(&mut self, r: MenuRect, rgb: [u8; 3], alpha: f64, clip: MenuRect) -> Result<()> {
        let r = intersection(r, clip);
        if alpha == 0. || r.width <= 0. || r.height <= 0. {
            return Ok(());
        }
        let scale = self.scale;
        let size = [
            (r.width * scale).round().max(1.) as u32,
            (r.height * scale).round().max(1.) as u32,
        ];
        if size.iter().any(|n| *n > 512) {
            // Reuse small uniform tiles instead of retaining a full-width
            // selection. The remainder shares the same per-resource limits.
            const TILE: u32 = 128;
            check_menu_draws(
                self.draws.len() + (size[0].div_ceil(TILE) * size[1].div_ceil(TILE)) as usize,
            )?;
            for y in (0..size[1]).step_by(TILE as usize) {
                for x in (0..size[0]).step_by(TILE as usize) {
                    self.ink(
                        MenuRect {
                            x: r.x + f64::from(x) / scale,
                            y: r.y + f64::from(y) / scale,
                            width: f64::from(TILE.min(size[0] - x)) / scale,
                            height: f64::from(TILE.min(size[1] - y)) / scale,
                        },
                        rgb,
                        alpha,
                        clip,
                    )?;
                }
            }
            return Ok(());
        }
        let resource = self.cached_material(MaterialKey::Solid(size, rgb), || {
            Ok(materials::RgbaImage {
                width: size[0],
                height: size[1],
                pixels: [rgb[0], rgb[1], rgb[2], 255].repeat(size[0] as usize * size[1] as usize),
            })
        })?;
        let mut draw = image(
            &resource,
            MenuRect {
                width: f64::from(size[0]) / scale,
                height: f64::from(size[1]) / scale,
                ..r
            },
            alpha,
        );
        draw.snap_overlay(scale);
        if clip_draw(&mut draw, clip, scale) {
            self.push(draw)?;
        }
        Ok(())
    }
    fn solid_draw(&mut self, r: MenuRect, kind: u8, alpha: f64) -> Result<Draw> {
        let size = if kind == 10 {
            [1, 1]
        } else {
            [
                (r.width * self.scale).round().max(1.) as u32,
                (r.height * self.scale).round().max(1.) as u32,
            ]
        };
        if size.into_iter().any(|n| n > 2048) {
            return Err(MENU_SOLID_TEXTURE_QUOTA.into());
        }
        let key = MaterialKey::Legacy(size[0], size[1], kind, 0, 0);
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
                    10 => PAGE_COLOR,
                    _ => [150, 108, 213],
                };
                Resource {
                    asset: BodyAsset {
                        handle: self.next_handle,
                        revision: 1,
                        width: size[0],
                        height: size[1],
                        status: BodyAssetStatus::Ready,
                    },
                    image: materials::RgbaImage {
                        width: size[0],
                        height: size[1],
                        pixels: [rgb[0], rgb[1], rgb[2], 255]
                            .repeat(size[0] as usize * size[1] as usize),
                    }
                    .into(),
                }
            };
            self.materials.insert(key, resource);
        }
        let mut draw = image(&self.materials[&key], r, alpha);
        if kind != 10 {
            draw.destination.width = f64::from(size[0]) / self.scale;
            draw.destination.height = f64::from(size[1]) / self.scale;
            draw.snap_overlay(self.scale);
        }
        Ok(draw)
    }
    fn solid(&mut self, r: MenuRect, kind: u8, alpha: f64) -> Result<()> {
        let draw = self.solid_draw(r, kind, alpha)?;
        self.push(draw)
    }
    fn push(&mut self, mut draw: Draw) -> Result<()> {
        draw.snap_overlay(self.scale);
        check_menu_draws(self.draws.len() + 1)?;
        frame::push_draw(&mut self.draws, draw)
    }
    // Login keeps its policy/input path; its existing recipes share the bounded
    // material implementation so exact high-DPI rasters need no larger quota.
    fn control(&mut self, r: MenuRect, kind: u8, alpha: f64) -> Result<()> {
        let theme = MenuTheme::default();
        let mut tokens = match kind {
            0 => theme.surface(ControlStyle::PrimaryButton, ControlState::Idle),
            1 => theme.surface(ControlStyle::PrimaryButton, ControlState::Hovered),
            2 => theme.surface(ControlStyle::PrimaryButton, ControlState::Pressed),
            4 => theme.surface(ControlStyle::Surface, ControlState::Idle),
            5 => theme.surface(ControlStyle::Field, ControlState::Idle),
            _ => return Err("Unknown login surface".into()),
        };
        if kind == 4 {
            tokens.recipe = SurfaceRecipe::Compact;
        }
        tokens.radius = (r.height / 2.).ceil() as u16;
        let r = MenuRect {
            width: (r.width * self.scale).ceil() / self.scale,
            height: (r.height * self.scale).ceil() / self.scale,
            ..r
        };
        self.sliced(r, tokens, alpha, None, 1.)
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
            reveal: 1.,
            reveal_span: None,
            line_box: false,
            field_inset: None,
            clip: None,
            fit: true,
        });
    }
    fn art(&mut self, resource: &Resource, r: MenuRect, alpha: f64) -> Result<()> {
        self.art_revealed(resource, r, alpha, 1.)
    }
    fn art_revealed(
        &mut self,
        resource: &Resource,
        r: MenuRect,
        alpha: f64,
        reveal: f64,
    ) -> Result<()> {
        let size = [
            (r.width * self.scale).round().max(1.) as u32,
            (r.height * self.scale).round().max(1.) as u32,
        ];
        if size.into_iter().any(|n| n > 2048) {
            return Err(MENU_ART_AXIS_QUOTA.into());
        }
        let key = (resource.asset.into(), size);
        if !self.art.contains_key(&key) {
            let resized = if let Some(old) = self.old_art.get(&key) {
                old.clone()
            } else if [resource.image.width, resource.image.height] == size {
                resource.clone()
            } else {
                // Filter premultiplied pixels once per source revision/physical
                // size. GPU minification would reconstruct this edge every frame.
                let source = image::RgbaImage::from_raw(
                    resource.image.width,
                    resource.image.height,
                    resource.image.pixels.clone(),
                )
                .ok_or("Menu art pixels")?;
                let mut pixels = image::imageops::resize(
                    &source,
                    size[0],
                    size[1],
                    image::imageops::FilterType::CatmullRom,
                )
                .into_raw();
                for p in pixels.as_chunks_mut::<4>().0 {
                    for i in 0..3 {
                        p[i] = p[i].min(p[3]);
                    }
                }
                self.next_handle = self
                    .next_handle
                    .checked_add(1)
                    .filter(|h| *h < MENU_MATERIAL_HANDLE_BASE + crate::bank::HANDLE_NAMESPACE_SIZE)
                    .ok_or("Menu material handle overflow")?;
                Resource {
                    asset: BodyAsset {
                        handle: self.next_handle,
                        revision: 1,
                        width: size[0],
                        height: size[1],
                        status: BodyAssetStatus::Ready,
                    },
                    image: materials::RgbaImage {
                        width: size[0],
                        height: size[1],
                        pixels,
                    }
                    .into(),
                }
            };
            let bytes: usize = self.art.values().map(|r| r.image.pixels.len()).sum();
            if bytes + resized.image.pixels.len() > crate::cosmetics::COSMETIC_PIXEL_BUDGET {
                return Err(MENU_ART_PIXEL_QUOTA.into());
            }
            self.art.insert(key, resized);
        }
        let r = MenuRect {
            width: f64::from(size[0]) / self.scale,
            height: f64::from(size[1]) / self.scale,
            ..r
        };
        for draw in reveal_draw(image(&self.art[&key], r, alpha), reveal, self.scale, None) {
            self.push(draw)?;
        }
        Ok(())
    }
    fn spinner(&mut self, r: MenuRect, view: &StartMenuView) -> Result<()> {
        if view.spinner_scale <= 0. {
            return Ok(());
        }
        let scale = self.scale * view.spinner_scale;
        let side = (128. * scale).ceil().max(1.) as u32;
        let key = MaterialKey::Legacy(side, side, 6, scale.to_bits(), view.spinner_time.to_bits());
        let resource = if let Some(old) = self.old.get(&key) {
            old.clone()
        } else {
            let pixels = materials::menu_spinner(view.spinner_time, scale)?;
            self.spinner_revision = self
                .spinner_revision
                .checked_add(1)
                .ok_or("Spinner revision overflow")?;
            Resource {
                asset: BodyAsset {
                    handle: SPINNER_HANDLE,
                    revision: self.spinner_revision,
                    width: side,
                    height: side,
                    status: BodyAssetStatus::Ready,
                },
                image: pixels.into(),
            }
        };
        let width = f64::from(side) / self.scale;
        self.push(image(
            &resource,
            MenuRect {
                x: r.x + (r.width - width) / 2.,
                y: r.y + (r.height - width) / 2.,
                width,
                height: width,
            },
            view.spinner_alpha,
        ))?;
        self.materials.insert(key, resource);
        Ok(())
    }
}
impl Menu {
    /// The shared control painter. Its resources and text publish together only
    /// after all quotas and the immutable frame validate.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_panel(
        &mut self,
        text: &mut TextCache,
        panel: &PanelView,
        input: &PanelInputState,
        edit: &MenuInputState,
        scroll: &mut BTreeMap<EditorId, f64>,
        theme: MenuTheme,
        extent: [u32; 2],
        dpr: f64,
    ) -> Result<MenuFrame> {
        crate::render::dimensions(extent, dpr)?;
        let viewport = extent.map(|n| f64::from(n) / dpr);
        let resolved = panel.resolve(viewport).map_err(str::to_owned)?;
        let mut staged_scroll = scroll.clone();
        let mut c = Composition {
            old: &self.materials,
            old_art: &self.art,
            materials: BTreeMap::new(),
            next_handle: self.next_handle.max(PAGE_HANDLE),
            spinner_revision: self.spinner_revision,
            scale: dpr,
            draws: Vec::new(),
            art: BTreeMap::new(),
            labels: Vec::new(),
        };
        c.panel(
            text,
            panel,
            input,
            edit,
            &mut staged_scroll,
            theme,
            &resolved,
        )?;
        let (result, next, spinner, materials, art) =
            c.finish(text, extent, dpr, None, None, Vec::new())?;
        self.next_handle = next;
        self.spinner_revision = spinner;
        self.materials = materials;
        self.art = art;
        *scroll = staged_scroll;
        Ok(result)
    }
    /// Rasterize at the physical overlay scale. Geometry comes from the core layout;
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
        input: Option<&MenuInputState>,
    ) -> Result<MenuFrame> {
        self.prepare_with_panel(
            text, view, layout, extent, dpr, cosmetics, logo_frame, input, None,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_panel(
        &mut self,
        text: &mut TextCache,
        view: &StartMenuView,
        layout: &MenuLayout,
        extent: [u32; 2],
        dpr: f64,
        cosmetics: Option<&Cosmetics>,
        logo_frame: u8,
        input: Option<&MenuInputState>,
        panel: Option<PanelPaint<'_>>,
    ) -> Result<MenuFrame> {
        crate::render::dimensions(extent, dpr)?;
        let mut c = Composition {
            old: &self.materials,
            old_art: &self.art,
            materials: BTreeMap::new(),
            next_handle: self.next_handle.max(PAGE_HANDLE),
            spinner_revision: self.spinner_revision,
            scale: dpr,
            draws: Vec::new(),
            art: BTreeMap::new(),
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
        // Fade the completed canvas, not overlapping world draws. The HUD
        // already owns its return opacity, so this cover goes before that layer.
        let world_cover = backdrop.clone().map(|mut draw| {
            draw.effective_alpha = a;
            draw
        });
        let busy = view.phase == slither_client::start_page::MenuPhase::Connecting;
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
            let logo_alpha = a * view.logo_alpha;
            // Authored wipe completes in the first quarter of the retained
            // reveal clock (~1.67s), with bright partial art during the intro.
            let logo_reveal = (4. * view.logo_reveal).clamp(0., 1.);
            if let Some(art) = cosmetics
                .filter(|_| !self.wordmark)
                .and_then(|p| p.menu_art(MenuArt::Logo(logo_frame)))
            {
                // Local art may be taller than the 450x135 original; it must
                // not paint over the rows below the holder.
                let natural = logo.width * f64::from(art.asset.height) / f64::from(art.asset.width);
                let room = layout.viewport_rect(E::LastLength).y - logo.y;
                let fit = (room / natural).min(1.);
                c.art_revealed(
                    art,
                    MenuRect {
                        x: logo.x + logo.width * (1. - fit) / 2.,
                        width: logo.width * fit,
                        height: natural * fit,
                        ..logo
                    },
                    logo_alpha,
                    logo_reveal,
                )?;
            } else {
                let runs = wordmark_runs(text, logo, dpr)?;
                let span = [runs[0].0.x, runs[0].0.width + runs[1].0.width];
                for ((r, style), value) in runs.into_iter().zip(["Slither", "Client"]) {
                    c.label(value, r, style.size, true, logo_alpha);
                    let label = c.labels.last_mut().unwrap();
                    label.style = style;
                    label.line_box = true;
                    label.align = Align::Left;
                    label.reveal = logo_reveal;
                    label.reveal_span = Some(span);
                }
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
                label.line_box = true;
                label.style.color = [128, 104, 192];
                // Tips reserve 30px of flow but use a normal font line at the
                // top of that row, rather than centering in the spare space.
                let m = text.measure_panel(view.tip, label.style)?;
                label.rect.height = m.font_ascent.round() + m.font_descent.round();
            }
            // Center the whole line; its three runs share a box/baseline and
            // preserve the regular prefix/punctuation and real bold number.
            let last = &view.last_life;
            let runs = [
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
                (last.suffix.to_owned(), false, 1.),
            ];
            let rect = layout.viewport_rect(E::LastLength);
            let size = 15. * layout.scale;
            let mut widths = Vec::new();
            for (value, bold, _) in &runs {
                let style = TextStyle {
                    size,
                    weight: if *bold { Weight::Bold } else { Weight::Normal },
                    family: Family::Lucida,
                    slant: Slant::Normal,
                    color: [255; 3],
                };
                widths.push(text.measure_panel(value, style)?.advance);
            }
            let total: f64 = widths.iter().sum();
            let fit = if total > rect.width {
                rect.width / total
            } else {
                1.
            };
            let mut x = rect.x + (rect.width - total * fit) / 2.;
            for ((value, bold, alpha), width) in runs.iter().zip(widths) {
                if !value.is_empty() {
                    c.label(
                        value,
                        MenuRect {
                            x,
                            width: width * fit,
                            ..rect
                        },
                        size * fit,
                        *bold,
                        a * alpha,
                    );
                    let label = c.labels.last_mut().unwrap();
                    label.align = Align::Left;
                    label.style.color = [255; 3];
                    label.style.family = Family::Lucida;
                }
                x += width * fit;
            }
            if view.champion_pending {
                c.label(
                    view.wording.champion_pending,
                    layout.viewport_rect(E::NicknameHolder),
                    15. * layout.scale,
                    false,
                    a,
                );
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
                c.labels.last_mut().unwrap().field_inset =
                    Some(FIELD_CONTENT_INSET.map(|v| v * layout.scale));
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
                    let inset = FIELD_CONTENT_INSET.map(|v| v * layout.scale);
                    let content_width = r.width - 2. * inset[0];
                    let style = c.labels.last().unwrap().style;
                    let full = text.measure_panel(&view.nickname, style)?.advance;
                    let fit = if full > content_width {
                        content_width / full
                    } else {
                        1.
                    };
                    let fitted = TextStyle {
                        size: style.size * fit,
                        ..style
                    };
                    let m = text.measure_panel("", fitted)?;
                    let placement = single_line_field_placement(
                        r,
                        inset,
                        fitted.size,
                        m.font_ascent,
                        m.font_descent,
                    );
                    let advance = |prefix: &str, text: &mut TextCache| -> Result<f64> {
                        Ok(text.measure_panel(prefix, style)?.advance * fit)
                    };
                    let left = advance(&editor.text()[..anchor.min(caret)], text)?;
                    let right = advance(&editor.text()[..anchor.max(caret)], text)?;
                    if anchor != caret && editor.preedit().is_none() {
                        c.solid(
                            MenuRect {
                                x: placement.origin[0] + left,
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
                    if input.caret_visible() {
                        c.solid(
                            MenuRect {
                                x: placement.origin[0] + cursor.min(content_width - 1.),
                                y: placement.caret_top,
                                width: 1.,
                                height: placement.caret_height,
                            },
                            8,
                            a * view.nickname_alpha,
                        )?;
                    }
                    if let Some(preedit) = editor.preedit() {
                        let start = advance(&editor.text()[..anchor.min(caret)], text)?;
                        let width =
                            advance(&preedit.text, text)?.min((content_width - start).max(0.));
                        if width > 0. {
                            c.solid(
                                MenuRect {
                                    x: placement.origin[0] + start,
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
                let pa =
                    a * view.play_alpha * view.play_return_alpha * if busy { 0.38 } else { 1. };
                c.control(play, 0, pa)?;
                if !view.play_disabled {
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
                    &format!("\u{a0}{}\u{a0}", view.play_label),
                    play,
                    20. * layout.scale,
                    true,
                    pa,
                );
                c.labels.last_mut().unwrap().line_box = true;
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
        let mut staged_scroll = panel.as_ref().map(|p| p.scroll.clone());
        if let Some(p) = &panel {
            let resolved = p
                .panel
                .resolve(extent.map(|n| f64::from(n) / dpr))
                .map_err(str::to_owned)?;
            c.panel(
                text,
                p.panel,
                p.input,
                p.edit,
                staged_scroll.as_mut().expect("panel scroll"),
                p.theme,
                &resolved,
            )?;
        }
        let spinner_start = c.draws.len();
        if view.spinner_visible {
            c.spinner(layout.viewport_rect(E::Spinner), view)?;
        }
        let spinner_draws = c.draws.split_off(spinner_start);
        let (result, next, spinner, materials, art) =
            c.finish(text, extent, dpr, backdrop, world_cover, spinner_draws)?;
        if let (Some(p), Some(scroll)) = (panel, staged_scroll) {
            *p.scroll = scroll;
        }

        self.next_handle = next;
        self.spinner_revision = spinner;
        self.materials = materials;
        self.art = art;
        Ok(result)
    }
}

type Published = (
    MenuFrame,
    u64,
    u64,
    BTreeMap<MaterialKey, Resource>,
    BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
);
// (destination, source, length) in physical pixels, including the atlas shadow.
fn slice_axis(size: u32, atlas: u32, pad: u32) -> Result<Vec<(u32, u32, u32)>> {
    if size == atlas {
        return Ok(vec![(0, 0, size + 2 * pad)]);
    }
    let edge = atlas.saturating_sub(32) / 2;
    let middle = atlas - 2 * edge;
    let mut out = vec![(0, 0, edge + pad)];
    let mut offset = edge + pad;
    let end = size
        .checked_sub(edge)
        .and_then(|n| n.checked_add(pad))
        .ok_or("Menu slice extent budget")?;
    while offset < end {
        check_menu_draws(out.len() + 2)?;
        let length = middle.min(end - offset);
        out.push((offset, edge + pad, length));
        offset += length;
    }
    out.push((end, atlas - edge + pad, edge + pad));
    Ok(out)
}
impl Composition<'_> {
    fn finish(
        mut self,
        text: &mut TextCache,
        extent: [u32; 2],
        dpr: f64,
        backdrop: Option<Draw>,
        world_cover: Option<Draw>,
        spinner_draws: Vec<Draw>,
    ) -> Result<Published> {
        let material_bytes = self
            .materials
            .values()
            .map(|r| r.image.pixels.len())
            .sum::<usize>();
        // Essential controls belong to the Menu allowance. Optional cosmetics
        // must not consume the capacity needed by Play, nickname or fallbacks.
        // Measure replacement faces normally. Fit full labels into their authored
        // boxes, without synthetic bold or browser-specific metric machinery.
        for l in &mut self.labels {
            let width = text.measure_panel(&l.text, l.style)?.advance;
            let available = l.rect.width - 2. * l.field_inset.map_or(0., |i| i[0]);
            if l.fit && width > available && l.reveal_span.is_none() {
                l.style.size *= available / width;
            }
        }
        let labels = std::mem::take(&mut self.labels);
        let measurements = labels
            .iter()
            .map(|l| text.measure_panel(&l.text, l.style))
            .collect::<Result<Vec<_>>>()?;
        let runs: Vec<_> = labels
            .iter()
            .zip(measurements)
            .map(|(l, m)| {
                let placement = single_line_field_placement(
                    l.rect,
                    l.field_inset.unwrap_or([0.; 2]),
                    l.style.size,
                    m.font_ascent,
                    m.font_descent,
                );
                let alphabetic = l.line_box || l.field_inset.is_some();
                TextRun {
                    text: &l.text,
                    style: l.style,
                    position: [
                        if l.field_inset.is_some() {
                            placement.origin[0]
                        } else {
                            l.rect.x
                                + if l.align == Align::Left {
                                    0.
                                } else {
                                    l.rect.width / 2.
                                }
                        },
                        if alphabetic {
                            placement.origin[1]
                        } else {
                            l.rect.y + l.rect.height / 2.
                        },
                    ],
                    align: l.align,
                    baseline: if alphabetic {
                        Baseline::Alphabetic
                    } else {
                        Baseline::Middle
                    },
                    alpha: l.alpha,
                }
            })
            .collect();
        // Only fields use a scrolled raster window. Position/baseline policy
        // stays in the existing host geometry; the backend bounds allocation.
        let clips: Vec<_> = labels
            .iter()
            .zip(&runs)
            .map(|(l, r)| {
                l.clip
                    .filter(|_| l.align == Align::Left && !l.fit)
                    .map(|c| BodyRect {
                        x: c.x - r.position[0],
                        y: c.y - r.position[1],
                        width: c.width,
                        height: c.height,
                    })
            })
            .collect();
        let result = text.compose_menu(
            &runs,
            &clips,
            self.scale,
            (self.materials.len(), material_bytes),
            |groups| {
                let mut resources: BTreeMap<_, _> = self
                    .materials
                    .values()
                    .chain(self.art.values())
                    .map(|r| (frame::ResourceKey::from(r.asset), r.clone()))
                    .collect();
                for (group, label) in groups.into_iter().zip(&labels) {
                    for (mut draw, resource) in group {
                        if let Some(clip) = label.clip
                            && !clip_draw(&mut draw, clip, self.scale)
                        {
                            continue;
                        }
                        for part in reveal_draw(draw, label.reveal, self.scale, label.reveal_span) {
                            self.push(part)?;
                        }
                        resources.insert(resource.asset.into(), resource);
                    }
                }
                // The spinner is above the login's text, including the fading tips.
                check_menu_draws(
                    self.draws.len()
                        + spinner_draws.len()
                        + usize::from(backdrop.is_some())
                        + usize::from(world_cover.is_some()),
                )?;
                frame::reserve_draws(&mut self.draws, spinner_draws.len())?;
                self.draws.extend(spinner_draws);
                let frame = Frame {
                    identity: 0,
                    extent,
                    viewport_scale: dpr,
                    tick: 0,
                    draws: std::mem::take(&mut self.draws),
                    snakes: Vec::new(),
                };
                frame.validate(&resources.iter().map(|(k, r)| (*k, r.asset)).collect())?;
                Ok(MenuFrame {
                    frame,
                    backdrop,
                    world_cover,
                    resources: resources.into_values().collect(),
                })
            },
        )?;
        Ok((
            result,
            self.next_handle,
            self.spinner_revision,
            self.materials,
            self.art,
        ))
    }
}

#[cfg(test)]
mod reveal_tests {
    use super::*;
    #[test]
    fn reveal_is_bounded_monotone_and_separate_from_opacity() {
        let resource = Resource {
            asset: BodyAsset {
                handle: 42,
                revision: 1,
                width: 900,
                height: 270,
                status: BodyAssetStatus::Ready,
            },
            image: materials::RgbaImage {
                width: 900,
                height: 270,
                pixels: vec![],
            }
            .into(),
        };
        let draw = image(
            &resource,
            MenuRect {
                x: 20.,
                y: 50.,
                width: 450.,
                height: 135.,
            },
            0.8,
        );
        assert!(reveal_draw(draw.clone(), 0., 2., None).is_empty());
        assert_eq!(
            reveal_draw(draw.clone(), 1., 2., None)[0].effective_alpha,
            0.8
        );
        let mut right = draw.clone();
        right.destination.x += 450.;
        let span = Some([20., 900.]);
        assert!(reveal_draw(right.clone(), 0.25, 2., span).is_empty());
        let left = reveal_draw(draw.clone(), 0.75, 2., span);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].effective_alpha, 0.8);
        let right = reveal_draw(right, 0.75, 2., span);
        assert!(right.len() <= 33);
        assert_eq!(right[0].effective_alpha, 0.8);
        let mut previous = 0.;
        for fraction in [0.01, 0.1, 0.25, 0.5, 0.99] {
            let parts = reveal_draw(draw.clone(), fraction, 2., None);
            assert!(parts.len() <= 33);
            let mut end = 0.;
            for p in &parts {
                let s = p.source.unwrap();
                assert_eq!(s.x, end);
                assert!(s.x + s.width <= 900.);
                assert_eq!(p.destination.width * 2., s.width);
                assert_eq!(p.destination.x, 20. + s.x / 2.);
                assert!(p.effective_alpha > 0. && p.effective_alpha <= 0.8);
                end += s.width;
            }
            if fraction >= 0.1 {
                assert_eq!(parts[0].effective_alpha, 0.8);
            }
            assert!(end >= previous);
            previous = end;
        }
    }
}
fn clip_draw(draw: &mut Draw, clip: MenuRect, scale: f64) -> bool {
    let r = draw.destination;
    let x = (r.x + draw.translation[0]).max((clip.x * scale).ceil() / scale);
    let y = (r.y + draw.translation[1]).max((clip.y * scale).ceil() / scale);
    let right =
        (r.x + r.width + draw.translation[0]).min(((clip.x + clip.width) * scale).floor() / scale);
    let bottom = (r.y + r.height + draw.translation[1])
        .min(((clip.y + clip.height) * scale).floor() / scale);
    if right <= x || bottom <= y {
        return false;
    }
    let src = draw.source.unwrap_or(BodyRect {
        x: 0.,
        y: 0.,
        width: f64::from(draw.asset.width),
        height: f64::from(draw.asset.height),
    });
    // Menu sources/destinations are physical-pixel tiles. Subtract rounded
    // physical endpoints, avoiding a 444.0000000000001 source at fractional DPR.
    let left_px = ((x - r.x - draw.translation[0]) * scale).round();
    let top_px = ((y - r.y - draw.translation[1]) * scale).round();
    let width_px = (right * scale).round() - (x * scale).round();
    let height_px = (bottom * scale).round() - (y * scale).round();
    draw.source = Some(BodyRect {
        x: src.x + left_px,
        y: src.y + top_px,
        width: width_px,
        height: height_px,
    });
    draw.destination = BodyRect {
        x: x - draw.translation[0],
        y: y - draw.translation[1],
        width: width_px / scale,
        height: height_px / scale,
    };
    true
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod quota_tests {
    use super::*;
    use slither_client::menu_input::EditIntent;

    #[test]
    fn solid_texture_axis_exhaustion_still_degrades() {
        let mut text = TextCache::menu();
        let mut view = crate::menu_specimen::view(0).unwrap();
        view.nickname = "W".repeat(24);
        // Isolate selection ink from the decorative field allocation.
        view.nickname_alpha = 0.;
        let extent = [7200, 6000];
        let layout = crate::menu_specimen::layout(&mut text, &view, extent, 12.).unwrap();
        let mut edit = MenuInputState::default();
        edit.reset_editor(EditorId::Nickname, &view.nickname);
        edit.apply(EditIntent::Focus(Some(EditorId::Nickname)));
        edit.apply(EditIntent::SelectAll);
        let mut menu = Menu {
            focus: Some(E::Nickname),
            ..Menu::default()
        };
        let prepared =
            menu.prepare_with_input(&mut text, &view, &layout, extent, 12., None, 0, Some(&edit));
        assert_eq!(
            prepared.as_ref().err().map(String::as_str),
            Some(MENU_SOLID_TEXTURE_QUOTA)
        );
        let base = MenuFrame::world_crossfade(extent, 12., 0.).unwrap();
        let mut picture = PreparedFrame {
            frame: base.frame.clone(),
            overlay: None,
            resources: vec![],
        };
        assert!(!MenuFrame::append_optional(prepared, &mut picture).unwrap());
        assert_eq!(picture.frame, base.frame);
        assert!(picture.resources.is_empty());
    }
}
