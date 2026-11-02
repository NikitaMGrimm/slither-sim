//! Opt-in menu resources. No adapter, input, session or independent animation clock.
use crate::menu_counts::{MenuAttribution, MenuComponent, MenuDrawCounts};
use crate::{
    Result,
    cosmetics::{Cosmetics, MenuArt},
    frame::{self, Draw, Frame},
    materials,
    render::PreparedFrame,
    scene::Resource,
    text::*,
    wordmark,
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
mod focus;

pub const MENU_PRODUCER_DRAWS: usize = 512;
// Windows scaling tops out at 500%. Above that, the picker cannot fit the
// 13 MiB menu allowance at 1:1; capping texel density is visually negligible.
pub const MAX_MENU_RASTER_DENSITY: f64 = 5.;
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
    Focus([u32; 2], u64, u64, [u8; 3], u32),
    Spinner(u32, u64, u64),
    Control([u32; 2], u64, SurfaceTokens, [u32; 2]),
    Solid([u32; 2], [u8; 3]),
}
const PAGE_COLOR: [u8; 3] = [22, 28, 34];
pub const WORDMARK_GREEN: [u8; 3] = wordmark::COLORS[0][1];
pub const WORDMARK_PURPLE: [u8; 3] = wordmark::COLORS[1][1];

pub fn check_menu_draws(count: usize) -> Result<()> {
    if count > MENU_PRODUCER_DRAWS {
        return Err(MENU_DRAW_QUOTA.into());
    }
    Ok(())
}
/// Immutable prepared resources survive subsequent resize, retirement and art replacement.
pub struct MenuFrame {
    pub attribution: MenuAttribution,
    pub frame: Frame,
    /// Page colour below the world, in the frame's coordinate space; absent when login is hidden.
    pub backdrop: Option<Draw>,
    /// Crossfade cover after the world blit, before the independently fading HUD.
    pub world_cover: Option<Draw>,
    pub resources: Vec<Resource>,
}
impl MenuFrame {
    pub fn draw_counts(&self) -> MenuDrawCounts {
        MenuDrawCounts {
            components: self.attribution,
            page: usize::from(self.backdrop.is_some()),
            cover: usize::from(self.world_cover.is_some()),
        }
    }
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
            attribution: Default::default(),
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
    /// Change only the string to use another wordmark; None uses DEFAULT_TEXT.
    pub wordmark_text: Option<String>,
    pub focus: Option<E>,
    pub skin_hovered: bool,
    pub skin_pressed: bool,
    materials: BTreeMap<MaterialKey, Resource>,
    art: BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
    next_handle: u64,
    spinner_revision: u64,
    stroke: Option<StrokeCache>,
}
#[derive(Clone)]
struct StrokeCache {
    key: (String, u64, u64, u64),
    sampled_at: f64,
    resources: Vec<Resource>,
    pieces: Vec<StrokePiece>,
}
#[derive(Clone)]
struct StrokePiece {
    atlas: usize,
    offset: [u32; 2],
    source: [u32; 4],
}
struct Label {
    component: MenuComponent,
    /// Material boundary for content below a later modal backdrop.
    before: Option<usize>,
    text: String,
    rect: MenuRect,
    style: TextStyle,
    alpha: f64,
    align: Align,
    line_box: bool,
    clip: Option<MenuRect>,
    fit: bool,
}
struct WordmarkRequest {
    value: String,
    holder: MenuRect,
    viewport_width: f64,
    alpha: f64,
    reveal: f64,
    draw_index: usize,
    label_index: usize,
}
struct Composition<'a> {
    component: MenuComponent,
    attribution: MenuAttribution,
    old: &'a BTreeMap<MaterialKey, Resource>,
    old_art: &'a BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
    materials: BTreeMap<MaterialKey, Resource>,
    next_handle: u64,
    spinner_revision: u64,
    scale: f64,
    viewport_scale: f64,
    draws: Vec<Draw>,
    art: BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
    labels: Vec<Label>,
    wordmark: Option<WordmarkRequest>,
    old_stroke: Option<&'a StrokeCache>,
    stroke: Option<StrokeCache>,
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
// The core advances logo_reveal by 0.00375 per 25 ms (start_page/menu.rs).
const REVEAL_TICK: f64 = 0.00375;
fn reveal_due(progress: f64, previous: Option<f64>) -> bool {
    if progress >= 1. {
        return previous.is_none_or(|p| p < 1.);
    }
    if progress < REVEAL_TICK {
        return false;
    }
    // Leave a complete tick before the exact final raster. The core clock
    // clamps at one, so deferring that final frame could otherwise never finish.
    if progress > 1. - REVEAL_TICK {
        return false;
    }
    previous.is_none_or(|p| progress < p || progress - p >= REVEAL_TICK - 1e-12)
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
            if panel
                .controls
                .iter()
                .any(|c| c.visible && matches!(c.kind, ControlKind::Backdrop))
            {
                for label in &mut self.labels {
                    label.before.get_or_insert(self.draws.len());
                }
            }
            for r in resolved.iter().filter(|r| r.control.visible) {
                let control = &r.control;
                self.component = if panel.id == 0 {
                    MenuComponent::LoginControls
                } else {
                    MenuComponent::Other
                };
                let enabled = panel.enabled && control.enabled;
                let alpha = panel.alpha
                    * control.alpha
                    * if enabled
                        || control.style == ControlStyle::MenuLink
                        || matches!(
                            control.kind,
                            ControlKind::Label { .. }
                                | ControlKind::Backdrop
                                | ControlKind::Surface
                                | ControlKind::ListRow { .. }
                        )
                    {
                        1.
                    } else {
                        theme.disabled_opacity(control.style)
                    };
                let mut style = theme.text_style(control.style);
                style.size *= r.scale;
                // Scrolled controls crop both body and decoration to their window.
                let surface_clip = r.control.clip.map(|_| r.clip);
                let add_label = |c: &mut Composition<'_>,
                                 value: String,
                                 rect: MenuRect,
                                 style: TextStyle,
                                 align: Align,
                                 clip: MenuRect| {
                    c.labels.push(Label {
                        component: if panel.id == 0 {
                            MenuComponent::LoginText
                        } else {
                            c.component
                        },
                        before: None,
                        text: value,
                        rect,
                        style,
                        alpha,
                        align,
                        line_box: false,
                        clip: Some(clip),
                        fit: false,
                    });
                };
                match &control.kind {
                    // The skin studio paints its own controls with egui.
                    ControlKind::CosmeticCell { .. }
                    | ControlKind::Swatch { .. }
                    | ControlKind::SectionTab { .. } => {
                        return Err("Skin studio controls are painted by egui".into());
                    }
                    ControlKind::IconLink { label } | ControlKind::SettingsLink { label } => {
                        let [hover, press] = input.progress(control.id);
                        let link_alpha = if press > 0. {
                            0.5
                        } else if hover > 0. || input.visible_focus() == Some(control.id) {
                            1.
                        } else {
                            0.7
                        };
                        let icon = MenuRect {
                            x: r.rect.x + (r.rect.width - 48.) / 2.,
                            y: r.rect.y + 37.,
                            width: 48.,
                            height: 22.,
                        };
                        if matches!(control.kind, ControlKind::SettingsLink { .. }) {
                            // Authored sliders glyph, using the link's existing palette.
                            for (row, knob) in [(0, 12.), (1, 32.), (2, 22.)] {
                                self.ink(
                                    MenuRect {
                                        y: icon.y + f64::from(row) * 8.,
                                        height: 2.,
                                        ..icon
                                    },
                                    theme.field,
                                    alpha * link_alpha,
                                    r.clip,
                                )?;
                                self.ink(
                                    MenuRect {
                                        x: icon.x + knob,
                                        y: icon.y + f64::from(row) * 8. - 2.,
                                        width: 5.,
                                        height: 6.,
                                    },
                                    theme.selection,
                                    alpha * link_alpha,
                                    r.clip,
                                )?;
                            }
                        } else {
                            for row in 0..3 {
                                let bar = MenuRect {
                                    y: icon.y + f64::from(row) * 8.,
                                    height: 6.,
                                    ..icon
                                };
                                self.ink(bar, theme.field, alpha * link_alpha, r.clip)?;
                                self.ink(
                                    MenuRect {
                                        x: bar.x + 4.,
                                        y: bar.y + 2.,
                                        width: 2.,
                                        height: 2.,
                                    },
                                    theme.selection,
                                    alpha * link_alpha,
                                    r.clip,
                                )?;
                            }
                        }
                        if input.visible_focus() == Some(control.id) && enabled {
                            self.outline(
                                r,
                                MenuRect {
                                    x: icon.x - 3.,
                                    y: icon.y - 3.,
                                    width: icon.width + 6.,
                                    height: icon.height + 6.,
                                },
                                4. * r.scale,
                                alpha,
                                theme,
                            )?;
                        }
                        add_label(
                            self,
                            label.clone(),
                            MenuRect {
                                y: r.rect.y + r.rect.height - 24.,
                                height: 24.,
                                ..r.rect
                            },
                            style,
                            Align::Center,
                            r.clip,
                        );
                        self.labels.last_mut().unwrap().alpha *= link_alpha;
                    }
                    ControlKind::Backdrop => self.fill(r.rect, theme.backdrop, alpha)?,
                    ControlKind::Surface => self.sliced(
                        r.rect,
                        theme.surface(control.style, ControlState::Idle),
                        alpha,
                        surface_clip,
                        r.scale,
                    )?,
                    ControlKind::ListRow {
                        label,
                        detail,
                        selected,
                        ping,
                    } => {
                        // Rows share tiny solid materials; only this window owns text rasters.
                        let focused = input.visible_focus() == Some(control.id);
                        let color = if *selected {
                            theme.selection
                        } else if focused || input.progress(control.id)[0] > 0. {
                            theme.field
                        } else {
                            theme
                                .surface(ControlStyle::ListRow, ControlState::Idle)
                                .colors[0]
                        };
                        self.fill(r.rect, color, alpha)?;
                        let inset = 14. * r.scale;
                        if *selected {
                            self.fill(
                                MenuRect {
                                    width: 3. * r.scale,
                                    ..r.rect
                                },
                                theme.text,
                                alpha,
                            )?;
                        }
                        if focused && enabled {
                            self.outline_control(r, alpha, theme)?;
                        }
                        let left = MenuRect {
                            x: r.rect.x + inset,
                            width: r.rect.width * 0.45,
                            ..r.rect
                        };
                        let right = MenuRect {
                            x: r.rect.x + r.rect.width * 0.48,
                            width: r.rect.width * 0.32,
                            ..r.rect
                        };
                        add_label(
                            self,
                            label.clone(),
                            left,
                            style,
                            Align::Left,
                            intersection(left, r.clip),
                        );
                        add_label(
                            self,
                            detail.clone(),
                            right,
                            style,
                            Align::Right,
                            intersection(right, r.clip),
                        );
                        let bars = ping.map_or(0, |p| {
                            if p < 80 {
                                4
                            } else if p < 160 {
                                3
                            } else if p < 300 {
                                2
                            } else {
                                1
                            }
                        });
                        for index in 0..4 {
                            let h = (5. + index as f64 * 4.) * r.scale;
                            self.fill(
                                MenuRect {
                                    x: r.rect.x + r.rect.width
                                        - inset
                                        - (4 - index) as f64 * 7. * r.scale,
                                    y: r.rect.y + (r.rect.height - h) / 2.,
                                    width: 4. * r.scale,
                                    height: h,
                                },
                                if index < bars {
                                    if *selected {
                                        theme.text
                                    } else {
                                        theme.selection
                                    }
                                } else {
                                    theme.placeholder
                                },
                                alpha,
                            )?;
                        }
                    }
                    ControlKind::Label { text: value } => {
                        add_label(self, value.clone(), r.rect, style, Align::Center, r.clip)
                    }
                    ControlKind::Button { label } => {
                        if control.style == ControlStyle::ChangeSkin {
                            if input.visible_focus() == Some(control.id) {
                                self.sliced(
                                    r.rect,
                                    theme.surface(ControlStyle::QualityButton, ControlState::Idle),
                                    alpha * 0.35,
                                    surface_clip,
                                    r.scale,
                                )?;
                            }
                            continue;
                        }
                        let quality = control.style == ControlStyle::QualityButton;
                        let compact = matches!(
                            control.style,
                            ControlStyle::StudioAction
                                | ControlStyle::StudioSave
                                | ControlStyle::StudioCancel
                        );
                        let focused = input.visible_focus() == Some(control.id);
                        let [hover, press] = input.progress(control.id);
                        if !quality || focused {
                            self.sliced(
                                r.rect,
                                theme.surface(control.style, ControlState::Idle),
                                alpha
                                    * if quality {
                                        0.35
                                    } else if compact {
                                        1. - 0.25 * press
                                    } else {
                                        1.
                                    },
                                surface_clip,
                                r.scale,
                            )?;
                        }
                        if enabled && !quality && !compact {
                            self.sliced(
                                r.rect,
                                theme.surface(control.style, ControlState::Hovered),
                                alpha
                                    * hover.max(if input.visible_focus() == Some(control.id) {
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
                        {
                            add_label(self, label.clone(), r.rect, style, Align::Center, r.clip);
                            let l = self.labels.last_mut().unwrap();
                            l.line_box = control.style == ControlStyle::LoginPlay;
                            l.fit = control.style != ControlStyle::StudioAction;
                        }
                        if compact && focused && enabled {
                            self.outline_control(r, alpha, theme)?;
                        }
                        if focused && enabled && !compact {
                            self.outline_control(r, alpha, theme)?;
                        }
                    }
                    ControlKind::TextField {
                        editor,
                        placeholder,
                        inset,
                        fit,
                        ..
                    } => {
                        self.sliced(
                            r.rect,
                            theme.surface(control.style, ControlState::Idle),
                            alpha,
                            surface_clip,
                            r.scale,
                        )?;
                        if input.visible_focus() == Some(control.id) && enabled {
                            self.outline_control(r, alpha, theme)?;
                        }
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
                            *fit,
                        )?;
                        style = g.style;
                        let clip = intersection(r.clip, g.rect);
                        scroll.insert(*editor, g.scroll);
                        if input.focus() == Some(control.id) && enabled {
                            self.ink(g.selection_rect(), theme.selection, alpha * 0.45, clip)?;
                            if edit.caret_visible() {
                                self.ink(
                                    g.snapped_cursor_rect(self.viewport_scale),
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
                                x: g.rect.x - g.scroll,
                                ..g.rect
                            },
                            style,
                            Align::Left,
                            clip,
                        );
                        self.labels.last_mut().unwrap().line_box = true;
                    }
                }
            }
        }
        Ok(())
    }

    fn fill(&mut self, rect: MenuRect, color: [u8; 3], alpha: f64) -> Result<()> {
        if alpha == 0. {
            return Ok(());
        }
        let resource = self.cached_material(MaterialKey::Solid([1, 1], color), || {
            Ok(materials::RgbaImage {
                width: 1,
                height: 1,
                pixels: vec![color[0], color[1], color[2], 255],
            })
        })?;
        self.push(image(&resource, rect, alpha))
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
    // This uniform-colour panel can stretch its centre and straight edges. Keep
    // corners/shadow at native raster density, retaining only the symmetric half.
    fn settings_panel(
        &mut self,
        r: MenuRect,
        tokens: SurfaceTokens,
        alpha: f64,
        clip: Option<MenuRect>,
        raster_scale: f64,
    ) -> Result<()> {
        let scale = self.scale;
        let pad = (12. * raster_scale).ceil() as u32;
        let radius = (f64::from(tokens.radius) * raster_scale).ceil() as u32;
        let middle = (8. * raster_scale).ceil() as u32;
        let atlas = [radius * 2 + middle; 2];
        let padded = atlas.map(|n| n + 2 * pad);
        let half = padded[0].div_ceil(2);
        let edge = f64::from(radius + pad);
        // All slices share one pixel grid. Independent origin snapping overlaps
        // fractional middles and makes their shared edges brighter during a fade.
        let origin = [r.x, r.y].map(|v| (v * scale).round() / scale);
        let width = (r.width * scale).round() + f64::from(2 * pad);
        let height = (r.height * scale).round() + f64::from(2 * pad);
        let split = (width / 2.).round();
        // Compact's shadow is shifted down 3px. Keep that transition in the
        // fixed top corner rather than stretching it along the straight edge.
        let top_edge = edge + (3. * raster_scale).ceil();
        // Tiny surfaces cannot hold the fixed corners. Omit their chrome before
        // allocating or emitting invalid geometry; zero-sized middles are valid.
        if width / 2. - edge < 0. || height - top_edge - edge < 0. {
            return Ok(());
        }
        let resource = self.cached_material(
            MaterialKey::Control(atlas, raster_scale.to_bits(), tokens, [0, 0]),
            || {
                let raster = materials::menu_control(
                    atlas,
                    f64::from(radius),
                    tokens.colors,
                    raster_scale,
                    materials::MenuSurface::Compact,
                )?;
                let mut pixels = Vec::with_capacity((half * padded[1] * 4) as usize);
                for row in raster.pixels.chunks_exact(padded[0] as usize * 4) {
                    pixels.extend_from_slice(&row[..half as usize * 4]);
                }
                Ok(materials::RgbaImage {
                    width: half,
                    height: padded[1],
                    pixels,
                })
            },
        )?;
        let ys = [
            (0., top_edge, 0., top_edge),
            (
                top_edge,
                height - top_edge - edge,
                top_edge,
                f64::from(padded[1]) - top_edge - edge,
            ),
            (height - edge, edge, f64::from(padded[1]) - edge, edge),
        ];
        for mirrored in [false, true] {
            let half_width = if mirrored { width - split } else { split };
            let xs = [
                (0., edge, 0., edge),
                (edge, half_width - edge, edge, f64::from(half) - edge),
            ];
            for (dx, dw, sx, sw) in xs {
                for (dy, dh, sy, sh) in ys {
                    let mut draw = image(
                        &resource,
                        MenuRect {
                            x: origin[0]
                                + (if mirrored { width - dx - dw } else { dx } - f64::from(pad))
                                    / scale,
                            y: origin[1] + (dy - f64::from(pad)) / scale,
                            width: dw / scale,
                            height: dh / scale,
                        },
                        alpha,
                    );
                    draw.source = Some(BodyRect {
                        x: sx,
                        y: sy,
                        width: sw,
                        height: sh,
                    });
                    let source = draw.source.unwrap();
                    let destination = draw.destination;
                    let visible = clip.is_none_or(|clip| {
                        let clipped = intersection(
                            MenuRect {
                                x: destination.x,
                                y: destination.y,
                                width: destination.width,
                                height: destination.height,
                            },
                            clip,
                        );
                        if clipped.width <= 0. || clipped.height <= 0. {
                            return false;
                        }
                        draw.source = Some(BodyRect {
                            x: source.x
                                + (clipped.x - destination.x) / destination.width * source.width,
                            y: source.y
                                + (clipped.y - destination.y) / destination.height * source.height,
                            width: clipped.width / destination.width * source.width,
                            height: clipped.height / destination.height * source.height,
                        });
                        draw.destination = BodyRect {
                            x: clipped.x,
                            y: clipped.y,
                            width: clipped.width,
                            height: clipped.height,
                        };
                        true
                    });
                    if visible {
                        if mirrored {
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
        Ok(())
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
        if tokens.recipe == SurfaceRecipe::StudioButton {
            return self.studio_capsule(r, tokens, alpha, clip, control_scale);
        }
        if tokens.recipe == SurfaceRecipe::SettingsPanel {
            return self.settings_panel(r, tokens, alpha, clip, raster_scale);
        }
        let surface = match tokens.recipe {
            SurfaceRecipe::Button | SurfaceRecipe::StudioButton => materials::MenuSurface::Play,
            SurfaceRecipe::Overlay => materials::MenuSurface::Overlay,
            SurfaceRecipe::Field | SurfaceRecipe::Panel => materials::MenuSurface::Nickname,
            SurfaceRecipe::Compact | SurfaceRecipe::StudioPanel | SurfaceRecipe::SettingsPanel => {
                materials::MenuSurface::Compact
            }
        };
        let pad = (surface.padding() * raster_scale).ceil() as u32;
        let radius = (f64::from(tokens.radius) * raster_scale).ceil() as u32;
        let size = [
            (r.width * scale).round().max(1.) as u32,
            (r.height * scale).round().max(1.) as u32,
        ];
        // Uniform centre strips use logical pixels: DPR must not multiply
        // the number of quads, including the fields/buttons above a modal.
        let logical_middle = match tokens.recipe {
            SurfaceRecipe::Panel => 64.,
            SurfaceRecipe::Compact => 128.,
            _ => 32.,
        };
        let middle = (logical_middle * raster_scale).ceil().max(1.) as u32;
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
        let xs = slice_axis(size[0], atlas[0], pad, middle)?;
        let ys = slice_axis(size[1], atlas[1], pad, middle)?;
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
    /// Small capsules (the old toolkit's studio-button recipe) share a compact
    /// theme shadow and stretch one uniform centre column. Retaining a wide
    /// centre would cost a raster per width; repeating a one-pixel column would
    /// exceed draws.
    fn studio_capsule(
        &mut self,
        r: MenuRect,
        tokens: SurfaceTokens,
        alpha: f64,
        clip: Option<MenuRect>,
        control_scale: f64,
    ) -> Result<()> {
        let density = self.scale * control_scale;
        let radius = (f64::from(tokens.radius) * density).ceil() as u32;
        // Tabs and actions share the same capsule; only its destination height differs.
        let height = (28. * density).round().max(1.) as u32;
        if height > 512 {
            return Err(MENU_RASTER_AXIS_QUOTA.into());
        }
        let pad = (12. * density).ceil() as u32;
        let atlas = [2 * radius + 1, height];
        let cap = radius + pad;
        let half = cap + 1;
        let full_height = height + 2 * pad;
        if half > 512 || full_height > 512 {
            return Err(MENU_RASTER_AXIS_QUOTA.into());
        }
        let resource = self.cached_material(
            MaterialKey::Control(atlas, density.to_bits(), tokens, [0, 0]),
            || {
                let raster = materials::menu_control(
                    atlas,
                    f64::from(radius),
                    tokens.colors,
                    density,
                    materials::MenuSurface::StudioCapsule,
                )?;
                let mut pixels = Vec::with_capacity((half * full_height * 4) as usize);
                for row in 0..full_height {
                    let start = (row * raster.width * 4) as usize;
                    pixels.extend_from_slice(&raster.pixels[start..start + (half * 4) as usize]);
                }
                Ok(materials::RgbaImage {
                    width: half,
                    height: full_height,
                    pixels,
                })
            },
        )?;
        let scale = self.scale;
        let vertical = r.height / f64::from(height);
        let inner = (f64::from(radius) * vertical).min(r.width / 2.);
        // Snap shared edges, not independent origins with unrounded widths:
        // fractional overlaps otherwise darken the shadow along each join.
        let edges = [
            r.x - f64::from(pad) * vertical,
            r.x + inner,
            r.x + r.width - inner,
            r.x + r.width + f64::from(pad) * vertical,
        ]
        .map(|x| (x * scale).round() / scale);
        for (x, width, source_x, source_width, mirrored) in [
            (edges[0], edges[1] - edges[0], 0., f64::from(cap), false),
            (edges[1], edges[2] - edges[1], f64::from(cap), 1., false),
            (edges[2], edges[3] - edges[2], 0., f64::from(cap), true),
        ] {
            if width == 0. {
                continue;
            }
            let mut draw = image(
                &resource,
                MenuRect {
                    x,
                    y: r.y - f64::from(pad) * vertical,
                    width,
                    height: f64::from(full_height) * vertical,
                },
                alpha,
            );
            draw.source = Some(BodyRect {
                x: source_x,
                y: 0.,
                width: source_width,
                height: f64::from(full_height),
            });
            draw.snap_overlay(scale);
            if clip.is_none_or(|clip| clip_scaled_draw(&mut draw, clip, scale)) {
                if mirrored {
                    let source = draw.source.as_mut().expect("capsule crop");
                    // Fractional clipping can leave the reflected origin a few ulps below zero.
                    source.x = (2. * source_x + source_width - source.x - source.width).max(0.);
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
        Ok(())
    }
    fn ink(&mut self, r: MenuRect, rgb: [u8; 3], alpha: f64, clip: MenuRect) -> Result<()> {
        // Avoid changing a real-DPR caret's exact size through subtracting its
        // large absolute coordinates when clipping is a no-op.
        let r = if self.viewport_scale > self.scale
            && r.x >= clip.x
            && r.y >= clip.y
            && r.x + r.width <= clip.x + clip.width
            && r.y + r.height <= clip.y + clip.height
        {
            r
        } else {
            intersection(r, clip)
        };
        if alpha == 0. || r.width <= 0. || r.height <= 0. {
            return Ok(());
        }
        if self.viewport_scale > self.scale {
            // Solid editor ink needs no high-density texture. Preserve its real
            // DPR caret/selection geometry so painting and IME bounds still agree.
            let resource = self.cached_material(MaterialKey::Solid([1, 1], rgb), || {
                Ok(materials::RgbaImage {
                    width: 1,
                    height: 1,
                    pixels: vec![rgb[0], rgb[1], rgb[2], 255],
                })
            })?;
            let mut draw = image(&resource, r, alpha);
            draw.snap_overlay(self.viewport_scale);
            check_menu_draws(self.draws.len() + 1)?;
            frame::push_draw(&mut self.draws, draw)?;
            self.attribution.add(self.component, 1);
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
        let mut draw = self.solid_draw(
            MenuRect {
                width: f64::from(size[0]) / scale,
                height: f64::from(size[1]) / scale,
                ..r
            },
            rgb,
            alpha,
            size,
        )?;
        if clip_draw(&mut draw, clip, scale) {
            self.push(draw)?;
        }
        Ok(())
    }
    fn solid_draw(
        &mut self,
        r: MenuRect,
        rgb: [u8; 3],
        alpha: f64,
        size: [u32; 2],
    ) -> Result<Draw> {
        if size.into_iter().any(|n| n > 2048) {
            return Err(MENU_SOLID_TEXTURE_QUOTA.into());
        }
        let resource = self.cached_material(MaterialKey::Solid(size, rgb), || {
            Ok(materials::RgbaImage {
                width: size[0],
                height: size[1],
                pixels: [rgb[0], rgb[1], rgb[2], 255].repeat(size[0] as usize * size[1] as usize),
            })
        })?;
        let mut draw = image(&resource, r, alpha);
        draw.snap_overlay(self.scale);
        Ok(draw)
    }
    #[allow(clippy::too_many_arguments)]
    fn stroke_wordmark(
        &mut self,
        value: &str,
        holder: MenuRect,
        viewport_width: f64,
        alpha: f64,
        reveal: f64,
        text: &mut TextCache,
        font_slots: usize,
        font_only: bool,
        reserved_draws: usize,
    ) -> Result<()> {
        let unit = holder.height / 16.;
        let mut style = TextStyle {
            size: holder.height * 0.8,
            weight: Weight::Bold,
            family: Family::Arial,
            slant: Slant::Normal,
            color: WORDMARK_GREEN,
        };
        let ascent = text.measure_panel("H", style)?.ascent;
        style.size *= 10. * unit / ascent;
        let mut layout = if font_only {
            wordmark::fallback_layout(value, holder, viewport_width, font_slots, |s| {
                Ok(text.measure_panel(s, style)?.advance / unit)
            })?
        } else {
            wordmark::layout(value, holder, viewport_width, |s| {
                Ok(text.measure_panel(s, style)?.advance / unit)
            })?
        };
        if !reveal.is_finite() {
            return Err("Wordmark reveal bounds".into());
        }
        let reveal = reveal.clamp(0., 1.);
        layout.fit_raster(self.scale)?;
        if layout.words.iter().filter(|w| w.fallback).count()
            + usize::from(layout.words.iter().any(|w| !w.fallback))
            > font_slots
        {
            layout = wordmark::fallback_layout(value, holder, viewport_width, font_slots, |s| {
                Ok(text.measure_panel(s, style)?.advance / unit)
            })?;
        }
        let stroke_start = self.draws.len();
        let stroke_counts = self.attribution;
        if reveal > 0. && layout.words.iter().any(|w| !w.fallback) {
            let key = (
                value.to_owned(),
                layout.unit.to_bits(),
                self.scale.to_bits(),
                layout
                    .word_offset_pixels(&layout.words[0], self.scale)
                    .to_bits(),
            );
            let old = self.old_stroke.filter(|old| old.key == key);
            let due = reveal_due(reveal, old.map(|c| c.sampled_at));
            if old.is_some() || due {
                let cache = if !due {
                    old.unwrap().clone()
                } else {
                    let raster = wordmark::raster(&layout, self.scale, reveal)?;
                    // Retain only actual painted pixels (including the soft
                    // shadow). Empty padding must not displace essential controls
                    // at 5x DPR; cropping never changes the 1:1 physical scale.
                    let mut bounds = [raster.width, raster.height, 0, 0];
                    for (i, p) in raster.pixels.as_chunks::<4>().0.iter().enumerate() {
                        if p[3] > 0 {
                            let x = i as u32 % raster.width;
                            let y = i as u32 / raster.width;
                            bounds[0] = bounds[0].min(x);
                            bounds[1] = bounds[1].min(y);
                            bounds[2] = bounds[2].max(x + 1);
                            bounds[3] = bounds[3].max(y + 1);
                        }
                    }
                    if bounds[2] == 0 {
                        bounds = [0, 0, 1, 1];
                    }
                    // At high DPR, pack nonempty horizontal bands into one
                    // atlas. This preserves stroke/shadow pixels while excluding
                    // the large holes between ascenders and lowercase bodies.
                    // Ordinary DPR 1/2 retains one image and one draw.
                    let mut regions = Vec::new();
                    if self.scale > 3. {
                        let band = (3. * self.scale).round() as u32;
                        for y in (bounds[1]..bounds[3]).step_by(band as usize) {
                            let height = (bounds[3] - y).min(band);
                            let mut spans: Vec<[u32; 2]> = Vec::new();
                            let mut left = None;
                            for x in bounds[0]..=bounds[2] {
                                let painted = x < bounds[2]
                                    && (y..y + height).any(|row| {
                                        raster.pixels[((row * raster.width + x) * 4 + 3) as usize]
                                            > 0
                                    });
                                if painted && left.is_none() {
                                    left = Some(x);
                                }
                                if !painted && let Some(start) = left.take() {
                                    spans.push([start, x]);
                                }
                            }
                            // Drop large transparent gaps without turning every
                            // letter/counter into a draw. The shared producer
                            // ceiling still admits controls, text and spinner.
                            while spans.len() > 4 {
                                let i = (0..spans.len() - 1)
                                    .min_by_key(|i| spans[i + 1][0] - spans[*i][1])
                                    .unwrap();
                                let end = spans.remove(i + 1)[1];
                                spans[i][1] = end;
                            }
                            for [left, right] in spans {
                                for sx in (left..right).step_by(2048) {
                                    regions.push([sx, y, (right - sx).min(2048), height]);
                                }
                            }
                        }
                    } else {
                        for x in (bounds[0]..bounds[2]).step_by(2048) {
                            regions.push([
                                x,
                                bounds[1],
                                (bounds[2] - x).min(2048),
                                bounds[3] - bounds[1],
                            ]);
                        }
                    }
                    check_menu_draws(
                        self.draws.len()
                            + regions.len()
                            + reserved_draws
                            + layout.words.iter().filter(|w| w.fallback).count(),
                    )?;
                    let mut pieces = Vec::new();
                    let mut sizes: Vec<[u32; 2]> = vec![[0, 0]];
                    regions.sort_by_key(|r| std::cmp::Reverse(r[2]));
                    let row_height = regions.iter().map(|r| r[3]).max().unwrap_or(1);
                    let rows_per_atlas = 2048 / row_height;
                    let mut used: Vec<u32> = Vec::new();
                    for [x, y, width, height] in regions {
                        let row = used
                            .iter()
                            .position(|n| *n + width <= 2048)
                            .unwrap_or(used.len());
                        if row == used.len() {
                            used.push(0);
                        }
                        let atlas = row / rows_per_atlas as usize;
                        if atlas == sizes.len() {
                            sizes.push([0, 0]);
                        }
                        let ax = used[row];
                        let ay = row as u32 % rows_per_atlas * row_height;
                        sizes[atlas][0] = sizes[atlas][0].max(ax + width);
                        sizes[atlas][1] = sizes[atlas][1].max(ay + height);
                        pieces.push(StrokePiece {
                            atlas,
                            offset: [x, y],
                            source: [ax, ay, width, height],
                        });
                        used[row] += width;
                    }
                    sizes.retain(|s| s[0] > 0);
                    let bytes = sizes
                        .iter()
                        .map(|s| s[0] as usize * s[1] as usize * 4)
                        .sum::<usize>();
                    preflight_menu_totals(
                        self.materials.len()
                            + sizes.len()
                            + self.labels.len()
                            + layout.words.iter().filter(|w| w.fallback).count(),
                        self.materials
                            .values()
                            .map(|r| r.image.pixels.len())
                            .sum::<usize>()
                            + bytes,
                    )?;
                    let mut images: Vec<_> = sizes
                        .into_iter()
                        .map(|[width, height]| materials::RgbaImage {
                            width,
                            height,
                            pixels: vec![0; width as usize * height as usize * 4],
                        })
                        .collect();
                    for piece in &pieces {
                        let [sx, sy, width, height] = piece.source;
                        let dst = &mut images[piece.atlas];
                        for row in 0..height {
                            let src = ((piece.offset[1] + row) * raster.width + piece.offset[0])
                                as usize
                                * 4;
                            let at = ((sy + row) * dst.width + sx) as usize * 4;
                            dst.pixels[at..at + width as usize * 4]
                                .copy_from_slice(&raster.pixels[src..src + width as usize * 4]);
                        }
                    }
                    let mut resources = Vec::new();
                    for image in images {
                        self.next_handle = self
                            .next_handle
                            .checked_add(1)
                            .filter(|h| {
                                *h < MENU_MATERIAL_HANDLE_BASE + crate::bank::HANDLE_NAMESPACE_SIZE
                            })
                            .ok_or("Menu material handle overflow")?;
                        resources.push(Resource {
                            asset: BodyAsset {
                                handle: self.next_handle,
                                revision: 1,
                                width: image.width,
                                height: image.height,
                                status: BodyAssetStatus::Ready,
                            },
                            image: image.into(),
                        });
                    }
                    StrokeCache {
                        key,
                        sampled_at: reveal,
                        resources,
                        pieces,
                    }
                };
                check_menu_draws(
                    self.draws.len()
                        + cache.pieces.len()
                        + reserved_draws
                        + layout.words.iter().filter(|w| w.fallback).count(),
                )?;
                let origin = layout.origin_pixels(self.scale);
                for piece in &cache.pieces {
                    let resource = &cache.resources[piece.atlas];
                    let [sx, sy, width, height] = piece.source;
                    let mut draw = image(
                        resource,
                        MenuRect {
                            x: (origin[0] + f64::from(piece.offset[0])) / self.scale,
                            y: (origin[1] + f64::from(piece.offset[1])) / self.scale,
                            width: f64::from(width) / self.scale,
                            height: f64::from(height) / self.scale,
                        },
                        alpha,
                    );
                    draw.source = Some(BodyRect {
                        x: f64::from(sx),
                        y: f64::from(sy),
                        width: f64::from(width),
                        height: f64::from(height),
                    });
                    self.push(draw)?;
                }
                self.stroke = Some(cache);
            }
        }
        // A mixed stroke/font line can retain more than one atlas. If that
        // consumes its reserved slots, use the bounded font line as a whole.
        if layout.words.iter().filter(|w| w.fallback).count()
            + self.stroke.as_ref().map_or(0, |s| s.resources.len())
            > font_slots
        {
            self.draws.truncate(stroke_start);
            self.attribution = stroke_counts;
            self.stroke = None;
            layout = wordmark::fallback_layout(value, holder, viewport_width, font_slots, |s| {
                Ok(text.measure_panel(s, style)?.advance / unit)
            })?;
        }
        for word in layout.words.iter().filter(|w| w.fallback) {
            // One font run for the whole unsupported word, never a partial
            // stroke word. Shape reveal is deliberately absent from this path.
            let r = MenuRect {
                x: layout.word_x_pixels(word, self.scale) / self.scale,
                y: layout.rect.y + 2. * layout.unit,
                width: word.width * layout.unit,
                height: 11. * layout.unit,
            };
            self.label(&word.text, r, style.size * layout.unit / unit, true, alpha);
            let label = self.labels.last_mut().unwrap();
            label.style.color = if word.color == 0 {
                WORDMARK_GREEN
            } else {
                WORDMARK_PURPLE
            };
            label.align = Align::Left;
            label.line_box = true;
        }
        Ok(())
    }
    fn push(&mut self, mut draw: Draw) -> Result<()> {
        draw.snap_overlay(self.scale);
        check_menu_draws(self.draws.len() + 1)?;
        frame::push_draw(&mut self.draws, draw)?;
        self.attribution.add(self.component, 1);
        Ok(())
    }
    fn control(&mut self, r: MenuRect, mut tokens: SurfaceTokens, alpha: f64) -> Result<()> {
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
            component: self.component,
            before: None,
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
            line_box: false,
            clip: None,
            fit: true,
        });
    }
    fn art(&mut self, resource: &Resource, r: MenuRect, alpha: f64) -> Result<()> {
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
        self.push(image(&self.art[&key], r, alpha))?;
        Ok(())
    }
    fn spinner(&mut self, r: MenuRect, view: &StartMenuView) -> Result<()> {
        if view.spinner_scale <= 0. {
            return Ok(());
        }
        let scale = self.scale * view.spinner_scale;
        let side = (128. * scale).ceil().max(1.) as u32;
        let key = MaterialKey::Spinner(side, scale.to_bits(), view.spinner_time.to_bits());
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
    /// Rasterize at the physical overlay scale. Geometry comes from the core layout;
    /// optional local icons never replace the authored stroke logo.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        text: &mut TextCache,
        view: &StartMenuView,
        layout: &MenuLayout,
        extent: [u32; 2],
        dpr: f64,
        cosmetics: Option<&Cosmetics>,
    ) -> Result<MenuFrame> {
        self.prepare_with_input(text, view, layout, extent, dpr, cosmetics, None)
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
        input: Option<&MenuInputState>,
    ) -> Result<MenuFrame> {
        self.prepare_with_panel(text, view, layout, extent, dpr, cosmetics, input, None)
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
        input: Option<&MenuInputState>,
        panel: Option<PanelPaint<'_>>,
    ) -> Result<MenuFrame> {
        crate::render::dimensions(extent, dpr)?;
        let mut c = Composition {
            component: Default::default(),
            attribution: Default::default(),
            old: &self.materials,
            old_art: &self.art,
            materials: BTreeMap::new(),
            next_handle: self.next_handle.max(PAGE_HANDLE),
            spinner_revision: self.spinner_revision,
            scale: dpr.min(MAX_MENU_RASTER_DENSITY),
            viewport_scale: dpr,
            draws: Vec::new(),
            art: BTreeMap::new(),
            labels: Vec::new(),
            wordmark: None,
            old_stroke: self.stroke.as_ref(),
            stroke: None,
        };
        c.component = MenuComponent::LoginText;
        let a = view.login_alpha;
        let backdrop = if view.visible {
            Some(c.solid_draw(
                MenuRect {
                    x: 0.,
                    y: 0.,
                    width: f64::from(extent[0]) / dpr,
                    height: f64::from(extent[1]) / dpr,
                },
                PAGE_COLOR,
                1.,
                [1, 1],
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
        if view.visible {
            let logo = layout.viewport_rect(E::Logo);
            let logo_alpha = a * view.logo_alpha;
            c.wordmark = Some(WordmarkRequest {
                value: self
                    .wordmark_text
                    .as_deref()
                    .unwrap_or(wordmark::DEFAULT_TEXT)
                    .to_owned(),
                holder: logo,
                viewport_width: f64::from(extent[0]) / dpr,
                alpha: logo_alpha,
                reveal: view.logo_reveal,
                draw_index: c.draws.len(),
                label_index: c.labels.len(),
            });
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
                c.component = MenuComponent::LoginControls;
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
                        MenuTheme::default()
                            .surface(ControlStyle::PrimaryButton, ControlState::Idle),
                        skin_alpha,
                    )?;
                    c.component = MenuComponent::LoginText;
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
        if !panel.as_ref().is_some_and(|p| p.panel.id == 0) {
            let viewport = extent.map(|n| f64::from(n) / dpr);
            let description = slither_client::login_controls::panel(view, layout, viewport);
            let mut controls = PanelInputState::default();
            if let Some(id) = match self.focus {
                Some(E::Play) => Some(slither_client::login_controls::PLAY),
                Some(E::Settings) => Some(slither_client::login_controls::SETTINGS),
                _ => input
                    .filter(|i| i.focus() == Some(EditorId::Nickname))
                    .map(|_| slither_client::login_controls::NICKNAME),
            } {
                controls.set_focus(id);
                if self.focus.is_some() {
                    controls.keyboard_navigation();
                }
            }
            controls.set_progress(
                slither_client::login_controls::PLAY,
                [view.hover_alpha, view.press_alpha],
            );
            let mut fallback = MenuInputState::default();
            fallback.reset_editor(EditorId::Nickname, &view.nickname);
            let resolved = description.resolve(viewport).map_err(str::to_owned)?;
            c.panel(
                text,
                &description,
                &controls,
                input.unwrap_or(&fallback),
                &mut BTreeMap::new(),
                MenuTheme::default(),
                &resolved,
            )?;
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
        c.component = MenuComponent::Other;
        if view.spinner_visible {
            c.spinner(layout.viewport_rect(E::Spinner), view)?;
        }
        let spinner_draws = c.draws.split_off(spinner_start);
        let (result, next, spinner, materials, art, stroke) =
            c.finish(text, extent, dpr, backdrop, world_cover, spinner_draws)?;
        if let (Some(p), Some(scroll)) = (panel, staged_scroll) {
            *p.scroll = scroll;
        }

        self.next_handle = next;
        self.spinner_revision = spinner;
        self.materials = materials;
        self.art = art;
        self.stroke = stroke;
        Ok(result)
    }
}

type Published = (
    MenuFrame,
    u64,
    u64,
    BTreeMap<MaterialKey, Resource>,
    BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
    Option<StrokeCache>,
);
// (destination, source, length) in physical pixels, including the atlas shadow.
fn slice_axis(size: u32, atlas: u32, pad: u32, centre: u32) -> Result<Vec<(u32, u32, u32)>> {
    if size == atlas {
        return Ok(vec![(0, 0, size + 2 * pad)]);
    }
    let edge = atlas.saturating_sub(centre) / 2;
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
        let request = self.wordmark.take();
        let reserved_draws = spinner_draws.len()
            + usize::from(backdrop.is_some())
            + usize::from(world_cover.is_some());
        let checkpoint = (self.draws.len(), self.labels.len(), self.next_handle);
        let checkpoint_counts = self.attribution;
        let mut added = (0, 0);
        if let Some(request) = &request {
            match self.place_wordmark(text, request, false, reserved_draws) {
                Ok(counts) => added = counts,
                Err(error) if matches!(error.as_str(), MENU_CACHE_QUOTA | MENU_DRAW_QUOTA) => {
                    self.draws.truncate(checkpoint.0);
                    self.attribution = checkpoint_counts;
                    self.labels.truncate(checkpoint.1);
                    self.stroke = None;
                    self.next_handle = checkpoint.2;
                    added = self.place_wordmark(text, request, true, reserved_draws)?;
                }
                Err(error) => return Err(error),
            }
        }
        let mut result = self.compose_labels(
            text,
            extent,
            dpr,
            backdrop.clone(),
            world_cover.clone(),
            &spinner_draws,
        );
        if result
            .as_ref()
            .err()
            .is_some_and(|error| matches!(error.as_str(), MENU_CACHE_QUOTA | MENU_DRAW_QUOTA))
            && self.stroke.is_some()
            && let Some(request) = &request
        {
            // TextCache's failed transaction has not published. Retry the logo
            // only, using actual text bytes/draws rather than a guessed
            // allowance. compose_labels restores staged draws before this retry;
            // the fallback must pass the same aggregate checks.
            self.draws
                .drain(request.draw_index..request.draw_index + added.0);
            self.attribution
                .remove(MenuComponent::LoginWordmark, added.0);
            self.labels
                .drain(request.label_index..request.label_index + added.1);
            for label in &mut self.labels {
                if let Some(index) = &mut label.before
                    && *index >= request.draw_index + added.0
                {
                    *index -= added.0;
                }
            }
            self.stroke = None;
            self.next_handle = checkpoint.2;
            self.place_wordmark(text, request, true, reserved_draws)?;
            result = self.compose_labels(text, extent, dpr, backdrop, world_cover, &spinner_draws);
        }
        Ok((
            result?,
            self.next_handle,
            self.spinner_revision,
            self.materials,
            self.art,
            self.stroke,
        ))
    }
    fn place_wordmark(
        &mut self,
        text: &mut TextCache,
        request: &WordmarkRequest,
        font_only: bool,
        reserved_draws: usize,
    ) -> Result<(usize, usize)> {
        let font_slots = MENU_CACHE_RUNS.saturating_sub(self.materials.len() + self.labels.len());
        let draws = self.draws.len();
        let labels = self.labels.len();
        self.component = MenuComponent::LoginWordmark;
        self.stroke_wordmark(
            &request.value,
            request.holder,
            request.viewport_width,
            request.alpha,
            request.reveal,
            text,
            font_slots,
            font_only,
            reserved_draws + self.labels.len(),
        )?;
        let added = (self.draws.len() - draws, self.labels.len() - labels);
        // Deferred logo text belongs below the first modal, just like the
        // labels that existed when that modal was composed.
        let before = self.labels[..labels].iter().filter_map(|l| l.before).min();
        for label in &mut self.labels[labels..] {
            label.before = before;
        }
        for label in &mut self.labels {
            if let Some(index) = &mut label.before
                && *index >= request.draw_index
            {
                *index += added.0;
            }
        }
        self.draws[request.draw_index..].rotate_right(added.0);
        self.labels[request.label_index..].rotate_right(added.1);
        Ok(added)
    }
    #[allow(clippy::too_many_arguments)]
    fn compose_labels(
        &mut self,
        text: &mut TextCache,
        extent: [u32; 2],
        dpr: f64,
        backdrop: Option<Draw>,
        world_cover: Option<Draw>,
        spinner_draws: &[Draw],
    ) -> Result<MenuFrame> {
        let material_bytes = self
            .materials
            .values()
            .map(|r| r.image.pixels.len())
            .sum::<usize>()
            + self.stroke.as_ref().map_or(0, |s| {
                s.resources
                    .iter()
                    .map(|r| r.image.pixels.len())
                    .sum::<usize>()
            });
        let material_slots =
            self.materials.len() + self.stroke.as_ref().map_or(0, |s| s.resources.len());
        // Essential controls belong to the Menu allowance. Optional cosmetics
        // must not consume the capacity needed by Play, nickname or fallbacks.
        // Measure replacement faces normally. Fit full labels into their authored
        // boxes, without synthetic bold or browser-specific metric machinery.
        for l in &mut self.labels {
            let width = text.measure_panel(&l.text, l.style)?.advance;
            let available = l.rect.width;
            if l.fit && width > available {
                l.style.size *= available / width;
            }
        }
        let labels = std::mem::take(&mut self.labels);
        let base_attribution = self.attribution;
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
                    [0.; 2],
                    l.style.size,
                    m.font_ascent,
                    m.font_descent,
                );
                let alphabetic = l.line_box;
                TextRun {
                    text: &l.text,
                    style: l.style,
                    position: [
                        l.rect.x
                            + match l.align {
                                Align::Left => 0.,
                                Align::Center => l.rect.width / 2.,
                                Align::Right => l.rect.width,
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
        let draw_start = self.draws.len();
        let result = text.compose_menu(
            &runs,
            &clips,
            self.scale,
            (material_slots, material_bytes),
            |groups| {
                let mut attribution = base_attribution;
                let mut underlays = BTreeMap::<usize, Vec<Draw>>::new();
                let mut underlay_count = 0;
                let mut foreground = Vec::new();
                let reserved = spinner_draws.len()
                    + usize::from(backdrop.is_some())
                    + usize::from(world_cover.is_some());
                let mut resources: BTreeMap<_, _> = self
                    .materials
                    .values()
                    .chain(self.art.values())
                    .chain(self.stroke.iter().flat_map(|s| &s.resources))
                    .map(|r| (frame::ResourceKey::from(r.asset), r.clone()))
                    .collect();
                for (group, label) in groups.into_iter().zip(&labels) {
                    for (mut draw, resource) in group {
                        if let Some(clip) = label.clip
                            && !clip_draw(&mut draw, clip, self.scale)
                        {
                            continue;
                        }
                        check_menu_draws(
                            self.draws.len() + underlay_count + foreground.len() + 1 + reserved,
                        )?;
                        draw.snap_overlay(self.scale);
                        if let Some(index) = label.before {
                            underlays.entry(index).or_default().push(draw);
                            underlay_count += 1;
                        } else {
                            foreground.push(draw);
                        }
                        resources.insert(resource.asset.into(), resource);
                        attribution.add(label.component, 1);
                    }
                }
                // Admit all text/modal/spinner draws before changing the material
                // sequence, so a failed attempt leaves its retry baseline intact.
                check_menu_draws(self.draws.len() + underlay_count + foreground.len() + reserved)?;
                frame::reserve_draws(
                    &mut self.draws,
                    underlay_count + foreground.len() + spinner_draws.len(),
                )?;
                for (index, draws) in underlays.into_iter().rev() {
                    self.draws.splice(index..index, draws);
                }
                self.draws.extend(foreground);
                // The spinner is above the login's text, including the fading tips.
                check_menu_draws(
                    self.draws.len()
                        + spinner_draws.len()
                        + usize::from(backdrop.is_some())
                        + usize::from(world_cover.is_some()),
                )?;
                frame::reserve_draws(&mut self.draws, spinner_draws.len())?;
                self.draws.extend_from_slice(spinner_draws);
                let frame = Frame {
                    identity: 0,
                    extent,
                    viewport_scale: dpr,
                    tick: 0,
                    draws: std::mem::take(&mut self.draws),
                    snakes: Vec::new(),
                };
                if let Err(error) =
                    frame.validate(&resources.iter().map(|(k, r)| (*k, r.asset)).collect())
                {
                    self.draws = frame.draws;
                    return Err(error);
                }
                Ok(MenuFrame {
                    attribution,
                    frame,
                    backdrop,
                    world_cover,
                    resources: resources.into_values().collect(),
                })
            },
        );
        self.labels = labels;
        if result.is_err() {
            // TextCache rolls back its staged resources. Match that transaction
            // for text/spinner draws before retrying with a smaller logo.
            self.draws.truncate(draw_start);
        }
        result
    }
}

// Scaled swatches and compact capsule centres do not have 1:1 source/destination pixels.
fn clip_scaled_draw(draw: &mut Draw, clip: MenuRect, scale: f64) -> bool {
    draw.snap_overlay(scale);
    let original = draw.destination;
    let clipped = intersection(
        MenuRect {
            x: original.x,
            y: original.y,
            width: original.width,
            height: original.height,
        },
        clip,
    );
    if clipped.width <= 0. || clipped.height <= 0. {
        return false;
    }
    let source = draw.source.expect("scaled menu crop");
    let x = source.x + (clipped.x - original.x) / original.width * source.width;
    let y = source.y + (clipped.y - original.y) / original.height * source.height;
    draw.source = Some(BodyRect {
        x,
        y,
        width: (clipped.width / original.width * source.width).min(f64::from(draw.asset.width) - x),
        height: (clipped.height / original.height * source.height)
            .min(f64::from(draw.asset.height) - y),
    });
    draw.destination = body(clipped);
    true
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

    #[test]
    fn animated_settings_slices_share_pixel_edges_without_intersection() {
        for scale in [1_f64, 1.25, 1.5, 2., 3., 5.] {
            for factor in [0.9, 0.93, 0.98, 1.] {
                for (width, height) in [(560.3, 400.7), (328.75, 512.25)] {
                    let mut c = Composition {
                        component: Default::default(),
                        attribution: Default::default(),
                        old: &BTreeMap::new(),
                        old_art: &BTreeMap::new(),
                        materials: BTreeMap::new(),
                        next_handle: PAGE_HANDLE,
                        spinner_revision: 0,
                        scale,
                        viewport_scale: scale,
                        draws: Vec::new(),
                        art: BTreeMap::new(),
                        labels: Vec::new(),
                        wordmark: None,
                        old_stroke: None,
                        stroke: None,
                    };
                    c.sliced(
                        MenuRect {
                            x: 17.3,
                            y: 21.7,
                            width: width * factor,
                            height: height * factor,
                        },
                        MenuTheme::default()
                            .surface(ControlStyle::SettingsSurface, ControlState::Idle),
                        0.6,
                        None,
                        factor,
                    )
                    .unwrap();
                    assert!(!c.draws.is_empty());
                    let bounds: Vec<_> = c
                        .draws
                        .iter()
                        .map(|d| {
                            let corners = d.corners.unwrap_or([
                                [d.destination.x, d.destination.y],
                                [d.destination.x + d.destination.width, d.destination.y],
                                [
                                    d.destination.x + d.destination.width,
                                    d.destination.y + d.destination.height,
                                ],
                                [d.destination.x, d.destination.y + d.destination.height],
                            ]);
                            assert!(
                                corners
                                    .iter()
                                    .flatten()
                                    .all(|v| (v * scale - (v * scale).round()).abs() < 1e-7)
                            );
                            let x = corners.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min);
                            let y = corners.iter().map(|v| v[1]).fold(f64::INFINITY, f64::min);
                            let right = corners
                                .iter()
                                .map(|v| v[0])
                                .fold(f64::NEG_INFINITY, f64::max);
                            let bottom = corners
                                .iter()
                                .map(|v| v[1])
                                .fold(f64::NEG_INFINITY, f64::max);
                            [x, y, right, bottom]
                        })
                        .collect();
                    for (i, a) in bounds.iter().enumerate() {
                        for b in &bounds[i + 1..] {
                            assert!(
                                a[2] <= b[0] + 1e-7
                                    || b[2] <= a[0] + 1e-7
                                    || a[3] <= b[1] + 1e-7
                                    || b[3] <= a[1] + 1e-7
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn settings_panel_skips_negative_middle_strips_but_accepts_zero() {
        for scale in [1., 1.5, 5.] {
            for [width, height] in [
                [1., 100.],
                [100., 1.],
                [1., 1.],
                [32., 36.],
                [32., 100.],
                [100., 35.],
            ] {
                let mut c = Composition {
                    component: Default::default(),
                    attribution: Default::default(),
                    old: &BTreeMap::new(),
                    old_art: &BTreeMap::new(),
                    materials: BTreeMap::new(),
                    next_handle: PAGE_HANDLE,
                    spinner_revision: 0,
                    scale,
                    viewport_scale: scale,
                    draws: Vec::new(),
                    art: BTreeMap::new(),
                    labels: Vec::new(),
                    wordmark: None,
                    old_stroke: None,
                    stroke: None,
                };
                c.sliced(
                    MenuRect {
                        x: 100.,
                        y: 100.,
                        width,
                        height,
                    },
                    MenuTheme::default().surface(ControlStyle::SettingsSurface, ControlState::Idle),
                    1.,
                    None,
                    1.,
                )
                .unwrap();
                let base = MenuFrame::world_crossfade([960, 540], scale, 0.).unwrap();
                let menu = MenuFrame {
                    attribution: c.attribution,
                    frame: Frame {
                        draws: c.draws,
                        ..base.frame.clone()
                    },
                    backdrop: None,
                    world_cover: None,
                    resources: c.materials.values().cloned().collect(),
                };
                let resident = menu
                    .resources
                    .iter()
                    .map(|r| (r.asset.into(), r.asset))
                    .collect();
                menu.frame.validate(&resident).unwrap();
                if width == 1. || height == 1. {
                    assert!(menu.frame.draws.is_empty());
                    assert!(menu.resources.is_empty());
                } else if scale == 1. {
                    assert!(!menu.frame.draws.is_empty()); // exact zero-width/height middles are valid
                }
                let picture = PreparedFrame {
                    frame: base.frame,
                    overlay: None,
                    resources: base.resources,
                };
                assert!(
                    MenuFrame::compose_optional(Ok(menu), &picture)
                        .unwrap()
                        .is_some()
                );
            }
        }
    }

    #[test]
    fn failed_text_draw_admission_preserves_material_order_and_cache() {
        let mut c = Composition {
            component: Default::default(),
            attribution: Default::default(),
            old: &BTreeMap::new(),
            old_art: &BTreeMap::new(),
            materials: BTreeMap::new(),
            next_handle: PAGE_HANDLE,
            spinner_revision: 0,
            scale: 1.,
            viewport_scale: 1.,
            draws: Vec::new(),
            art: BTreeMap::new(),
            labels: Vec::new(),
            wordmark: None,
            old_stroke: None,
            stroke: None,
        };
        let rect = MenuRect {
            x: 0.,
            y: 0.,
            width: 100.,
            height: 30.,
        };
        let draw = c.solid_draw(rect, [80; 3], 1., [1, 1]).unwrap();
        c.draws = vec![draw; 510];
        c.attribution.add(MenuComponent::Other, 510);
        for value in ["one", "two", "three"] {
            c.label(value, rect, 15., false, 1.);
        }
        c.labels[0].before = Some(0);
        let before = c.draws.clone();
        let mut text = TextCache::menu();
        let result = c.compose_labels(&mut text, [960, 720], 1., None, None, &[]);
        assert_eq!(result.err().as_deref(), Some(MENU_DRAW_QUOTA));
        assert_eq!(c.draws, before);
        assert_eq!(text.cached_runs(), 0);
        c.draws.truncate(509);
        c.attribution.remove(MenuComponent::Other, 1);
        let retried = c
            .compose_labels(&mut text, [960, 720], 1., None, None, &[])
            .unwrap();
        assert_eq!(retried.attribution.total(), retried.frame.draws.len());
        assert_eq!(retried.draw_counts().admitted_menu(), 512);
    }

    #[test]
    fn solid_texture_axis_exhaustion_still_degrades() {
        let mut c = Composition {
            component: Default::default(),
            attribution: Default::default(),
            old: &BTreeMap::new(),
            old_art: &BTreeMap::new(),
            materials: BTreeMap::new(),
            next_handle: PAGE_HANDLE,
            spinner_revision: 0,
            scale: 1.,
            viewport_scale: 1.,
            draws: Vec::new(),
            art: BTreeMap::new(),
            labels: Vec::new(),
            wordmark: None,
            old_stroke: None,
            stroke: None,
        };
        let prepared = c
            .solid_draw(
                MenuRect {
                    x: 0.,
                    y: 0.,
                    width: 2049.,
                    height: 1.,
                },
                [255; 3],
                1.,
                [2049, 1],
            )
            .map(|_| unreachable!("axis quota precedes allocation"));
        assert_eq!(
            prepared.as_ref().err().map(String::as_str),
            Some(MENU_SOLID_TEXTURE_QUOTA)
        );
        let base = MenuFrame::world_crossfade([960, 540], 1., 0.).unwrap();
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

#[cfg(test)]
mod reveal_clock_tests {
    use super::*;
    #[test]
    fn reveal_updates_are_25ms_apart_and_finish_on_the_clamped_clock() {
        let mut previous = None;
        let mut times = Vec::new();
        for ms in 0..=6700 {
            let progress = (f64::from(ms) * REVEAL_TICK / 25.).min(1.);
            if reveal_due(progress, previous) {
                previous = Some(progress);
                times.push(ms);
            }
        }
        assert_eq!(times[0], 25);
        assert!(times.windows(2).all(|t| t[1] - t[0] >= 25));
        assert_eq!(previous, Some(1.));
        assert!(times.last().copied().unwrap() <= 6667);
        assert!(!reveal_due(1., previous));
        assert!(reveal_due(0.1, previous)); // new reveal timeline
    }
}
