//! Platform-rasterized Arial runs, submitted as ordinary ordered image draws.
use crate::{Result, frame::Draw, materials::RgbaImage, scene::Resource};
use slither_client::presentation::{BodyAsset, BodyAssetStatus, BodyBlend, BodyRect};
use std::collections::BTreeMap;

#[cfg(not(target_arch = "wasm32"))]
#[path = "text_native.rs"]
mod platform;
#[cfg(target_arch = "wasm32")]
#[path = "text_web.rs"]
mod platform;

// Display only: protocol owners retain the original bytes and normalization.
// One scalar maps to at most one ordinary glyph; both platform paths use this.
fn display_character(c: char) -> char {
    if c.is_whitespace() && c.is_control() {
        ' '
    } else if c.is_control() || c == '\u{ad}' {
        '\u{fffd}'
    } else {
        c
    }
}
#[cfg(target_arch = "wasm32")]
fn display_text(text: &str) -> String {
    text.chars().map(display_character).collect()
}

pub(crate) const TEXT_RASTER_QUOTA: &str = "text raster extent budget";

pub const MINIMAP_CACHE_BYTES: usize = 1024 * 1024;
pub(crate) const PANEL_TOTALS_QUOTA: &str = "Aggregate panel byte/draw budget";
pub(crate) const MENU_CACHE_QUOTA: &str = "Menu cache slot/pixel budget";
pub(crate) const PANEL_SLOT_QUOTA: &str = "Panel namespace/slot budget";
pub(crate) const PANEL_DRAW_QUOTA: &str = "Panel draw budget";
pub(crate) const HUD_SLOT_QUOTA: &str = "HUD text slot budget";
pub(crate) const TEXT_CACHE_RUN_QUOTA: &str = "text cache run budget";
pub(crate) const TEXT_CACHE_PIXEL_QUOTA: &str = "HUD text pixel budget";
pub(crate) const TEXT_CACHE_QUOTA: &str = "text cache budget";

pub const MINIMAP_SHADOW_CACHE_BYTES: usize = 1024 * 1024;
// Allocation rationale: https://github.com/NikitaMGrimm/slither-sim/blob/main/client/docs/panel-text.md#aggregate-resource-proof;
// regression: https://github.com/NikitaMGrimm/slither-sim/blob/main/client/apps/wgpu-slice/tests/panel_text.rs#L20
pub const PANEL_CACHE_BYTES: usize = 12 * 1024 * 1024;
pub const PANEL_CACHE_RUNS: usize = 64;
// Exact 5x controls, last-life/Play Again and cropped stroke-logo tiles through
// 1.1x outgoing growth: tests/menu.rs::stroke_wordmark_overrides_only_logo_art_with_bounded_tiled_rasters.
// The aggregate 13 MiB admission and 64 MiB whole-picture guard both apply.
pub const MENU_CACHE_BYTES: usize = 13 * 1024 * 1024;
pub const MENU_CACHE_RUNS: usize = 64;
pub const CACHE_BYTES: usize = 4 * 1024 * 1024 + MINIMAP_CACHE_BYTES + MINIMAP_SHADOW_CACHE_BYTES;
pub const CACHE_RUNS: usize = 258;
pub const TOTAL_CACHE_RUNS: usize = CACHE_RUNS + PANEL_CACHE_RUNS + MENU_CACHE_RUNS;
pub const TOTAL_CACHE_BYTES: usize = CACHE_BYTES + PANEL_CACHE_BYTES + MENU_CACHE_BYTES;

pub use slither_client::presentation::{
    PLATFORM_FONT_FAMILY as FONT_FAMILY, TextAlign as Align, TextBaseline as Baseline,
    TextFamily as Family, TextSlant as Slant, TextStyle, TextWeight as Weight,
};
// HUD slots replace their previous content and have a separate allowance. Fast
// score updates cannot evict world labels from the nickname LRU.
pub const HUD_CACHE_BYTES: usize = 2 * 1024 * 1024;
// Backing-space diagnostics retain their bounded high-scale fallback. Native
// physical overlays bypass it and rasterize at the exact window DPR.
pub const MAX_HUD_RASTER_SCALE: f64 = 3.;
// Matches the renderer's WebGL2 downlevel texture bound. Menu/HUD text keeps
// exact DPR; their byte quotas still admit each raster and its tiles.
pub(super) const MAX_OVERLAY_TEXT_EXTENT: u32 = 2048;
const NICKNAME_CACHE_RUNS: usize = 216;
pub const MAX_NICKNAME_RUNS_PER_FRAME: usize = 128;

pub struct TextRun<'a> {
    pub text: &'a str,
    pub style: TextStyle,
    pub position: [f64; 2],
    pub align: Align,
    pub baseline: Baseline,
    pub alpha: f64,
}

/// Stable namespaces; a producer cannot replace another producer's slots.
#[derive(Clone, Copy)]
pub enum PanelGroup {
    Announcement,
    Result,
}
impl PanelGroup {
    fn slots(self) -> (u8, usize) {
        match self {
            Self::Announcement => (0, 48),
            Self::Result => (48, 16),
        }
    }
}
/// All active panel producers are submitted together, once per composition.
pub struct PanelSubmission<'a> {
    pub group: PanelGroup,
    pub plan: &'a slither_client::presentation::PanelTextPlan,
    pub alpha: f64,
}
/// Text and its immutable resource copies in producer order.
pub type PanelDraws = Vec<Vec<(Draw, Resource)>>;

/// Admission is shared by real clipped allocations and the conservative proof.
/// Counts include every horizontal tile (510 content pixels plus neighbours).
pub fn preflight_panel_totals(bytes: usize, text_draws: [usize; 2]) -> Result<()> {
    if bytes > PANEL_CACHE_BYTES
        || text_draws
            .into_iter()
            .any(|n| n > crate::render::PANEL_PRODUCER_DRAWS - crate::render::PANEL_CONTROL_DRAWS)
    {
        return Err(PANEL_TOTALS_QUOTA.into());
    }
    Ok(())
}

/// Pixel bounds relative to a left-aligned alphabetic anchor, plus advance and
/// the alphabetic-to-middle baseline shift, all in raster pixels.
pub(super) struct Raster {
    pub image: RgbaImage,
    pub offset: [f64; 2],
    pub advance: f64,
    pub middle: f64,
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Content {
    text: String,
    max_width: Option<u32>,
    size: u64,
    weight: Weight,
    family: Family,
    slant: Slant,
    clip: Option<[u64; 4]>,
    // Exact RGB also identifies the native luminance-dependent coverage curve.
    color: [u8; 3],
    shadow: Option<[u64; 3]>,
    line_height: Option<u64>,
}
impl Content {
    fn menu(run: &TextRun<'_>, scale: f64, clip: Option<BodyRect>) -> Self {
        Self {
            text: run.text.into(),
            max_width: None,
            size: (run.style.size * scale).to_bits(),
            weight: run.style.weight,
            family: run.style.family,
            slant: run.style.slant,
            clip: clip.map(|c| [c.x, c.y, c.width, c.height].map(f64::to_bits)),
            color: run.style.color,
            shadow: None,
            line_height: None,
        }
    }
    fn panel(run: &slither_client::presentation::PanelTextRun, scale: f64) -> Self {
        Self {
            text: run.text.clone(),
            max_width: None,
            size: (run.style.size * scale).to_bits(),
            weight: run.style.weight,
            family: run.style.family,
            slant: run.style.slant,
            color: run.style.color,
            clip: Some(
                [
                    run.clip.x - run.position[0],
                    run.clip.y - run.position[1],
                    run.clip.width,
                    run.clip.height,
                ]
                .map(f64::to_bits),
            ),
            shadow: None,
            line_height: None,
        }
    }
}
pub fn preflight_menu_totals(slots: usize, bytes: usize) -> Result<()> {
    if slots > MENU_CACHE_RUNS || bytes > MENU_CACHE_BYTES {
        return Err(MENU_CACHE_QUOTA.into());
    }
    Ok(())
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Nickname(Content),
    Hud(u8),
    Minimap,
    Panel(u8),
    Menu(u8),
}
impl Key {
    #[cfg(all(test, windows))]
    fn is_hud(&self) -> bool {
        matches!(self, Self::Hud(_))
    }
    fn group(&self) -> u8 {
        match self {
            Self::Nickname(_) => 0,
            Self::Hud(_) => 1,
            Self::Minimap => 2,
            Self::Panel(_) => 3,
            Self::Menu(_) => 4,
        }
    }
}
#[derive(Clone)]
struct Entry {
    content: Content,
    tiles: Vec<Tile>,
    bytes: usize,
    offset: [f64; 2],
    advance: f64,
    middle: f64,
    used: u64,
}
#[derive(Clone)]
struct Tile {
    resource: Resource,
    start: u32,
    source_x: u32,
    width: u32,
}

struct CachedShadow {
    content: Content,
    raster: Raster,
}

pub struct TextCache {
    minimap_shadow: Option<CachedShadow>,
    rasterizer: platform::Rasterizer,
    entries: BTreeMap<Key, Entry>,
    bytes: usize,
    clock: u64,
    next_handle: u64,
}
impl Default for TextCache {
    fn default() -> Self {
        Self {
            minimap_shadow: None,
            rasterizer: platform::Rasterizer::new(),
            entries: BTreeMap::new(),
            bytes: 0,
            clock: 0,
            next_handle: crate::bank::TEXT_HANDLE_BASE,
        }
    }
}
impl TextCache {
    pub fn menu() -> Self {
        Self {
            next_handle: crate::bank::MENU_TEXT_HANDLE_BASE,
            ..Self::default()
        }
    }
    /// Menu text and authored material allocations share a separate quota.
    /// The final frame admission runs inside `finish`, before cache publication.
    /// Only immutable resource copies leave this transaction.
    pub(crate) fn compose_menu<T>(
        &mut self,
        runs: &[TextRun<'_>],
        clips: &[Option<BodyRect>],
        scale: f64,
        materials: (usize, usize),
        finish: impl FnOnce(Vec<Vec<(Draw, Resource)>>) -> Result<T>,
    ) -> Result<T> {
        preflight_menu_totals(materials.0 + runs.len(), materials.1)?;
        let mut staged = Self {
            rasterizer: std::mem::take(&mut self.rasterizer),
            next_handle: self.next_handle,
            clock: self.clock,
            ..Self::default()
        };
        // Retire changed and hidden slots together before admitting replacements.
        // A growing early run cannot compete with an obsolete later run.
        for (i, run) in runs.iter().enumerate() {
            let key = Key::Menu(i as u8);
            if let Some(entry) = self.entries.get(&key)
                && entry.content == Content::menu(run, scale, clips.get(i).copied().flatten())
            {
                staged.entries.insert(key, entry.clone());
                staged.bytes += entry.bytes;
            }
        }
        let result = (|| {
            let mut draws = Vec::new();
            for (i, run) in runs.iter().enumerate() {
                draws.push(
                    staged
                        .try_draw(
                            TextRun { ..*run },
                            scale,
                            Some(Key::Menu(i as u8)),
                            None,
                            None,
                            clips.get(i).copied().flatten(),
                        )?
                        .1,
                );
            }
            preflight_menu_totals(
                materials.0 + staged.entries.len(),
                materials.1 + staged.bytes,
            )?;
            finish(draws)
        })();
        self.rasterizer = std::mem::take(&mut staged.rasterizer);
        if result.is_ok() {
            self.entries.retain(|k, _| !matches!(k, Key::Menu(_)));
            self.entries.extend(staged.entries);
            self.bytes = self.entries.values().map(|e| e.bytes).sum();
            self.next_handle = staged.next_handle;
            self.clock = staged.clock;
        }
        result
    }
    /// Fontdue/Canvas metrics are copied into the shared line breaker.
    pub fn measure_panel(
        &mut self,
        text: &str,
        style: TextStyle,
    ) -> Result<slither_client::presentation::TextMeasurement> {
        self.rasterizer.measure(text, style)
    }
    /// Installed native face selected for this display style.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn native_face(style: TextStyle) -> &'static str {
        platform::face_name(style)
    }

    /// Aggregate preflight, replacement and retirement form one transaction.
    /// No producer starts its own cleanup. An empty composition retires panels.
    /// Old prepared frames keep their owned resource copies after replacement.
    pub fn draw_panel_composition(
        &mut self,
        panels: &[PanelSubmission<'_>],
        scale: f64,
    ) -> Result<PanelDraws> {
        self.draw_panel_composition_with_run_alphas(panels, scale, &[])
    }
    /// Optional per-run opacity metadata, separate from immutable raster keys.
    pub fn draw_panel_composition_with_run_alphas(
        &mut self,
        panels: &[PanelSubmission<'_>],
        scale: f64,
        alphas: &[&[f64]],
    ) -> Result<PanelDraws> {
        if !scale.is_finite() || scale <= 0. {
            return Err("Invalid panel scale".into());
        }
        self.draw_overlay_panels_with_run_alphas(panels, scale.min(MAX_HUD_RASTER_SCALE), alphas)
    }
    pub(crate) fn draw_overlay_panels_with_run_alphas(
        &mut self,
        panels: &[PanelSubmission<'_>],
        scale: f64,
        alphas: &[&[f64]],
    ) -> Result<PanelDraws> {
        if !alphas.is_empty()
            && (alphas.len() != panels.len()
                || panels.iter().zip(alphas).any(|(p, a)| {
                    a.len() != p.plan.runs.len() || a.iter().any(|v| !(0. ..=1.).contains(v))
                }))
        {
            return Err("Panel run opacity metadata".into());
        }
        if !scale.is_finite() || scale <= 0. {
            return Err("Invalid panel scale".into());
        }
        let mut active = [false; 2];
        for p in panels {
            let (base, limit) = p.group.slots();
            let i = usize::from(base != 0);
            if active[i] || !(0. ..=1.).contains(&p.alpha) {
                return Err("Invalid panel namespace/opacity".into());
            }
            if p.plan.runs.len() > limit {
                return Err(PANEL_SLOT_QUOTA.into());
            }
            active[i] = true;
            for run in &p.plan.runs {
                let c = run.clip;
                if ![
                    c.x,
                    c.y,
                    c.width,
                    c.height,
                    run.position[0],
                    run.position[1],
                    run.style.size,
                ]
                .into_iter()
                .all(f64::is_finite)
                    || c.width < 0.
                    || c.width > if base == 0 { 300. } else { 600. }
                    || c.height < 0.
                    || c.height > 32.
                    || run.style.size <= 0.
                    || run.style.size > 24.
                {
                    return Err("Panel clipped allocation bounds".into());
                }
            }
        }
        let mut staged = Self {
            rasterizer: std::mem::take(&mut self.rasterizer),
            entries: self
                .entries
                .iter()
                .filter(|(k, _)| matches!(k, Key::Panel(_)))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            next_handle: self.next_handle,
            clock: self.clock,
            ..Self::default()
        };
        // Retire every obsolete entry before allocating any replacement: a
        // growing early slot must not compete with a shrinking later slot.
        // Only the staged copy changes until aggregate admission succeeds.
        staged.entries.retain(|k, entry| {
            panels.iter().any(|p| {
                let (base, _) = p.group.slots();
                matches!(k, Key::Panel(s) if *s >= base && p.plan.runs.get(usize::from(*s-base)).is_some_and(|r| !r.text.is_empty() && r.clip.width > 0. && r.clip.height > 0. && entry.content == Content::panel(r, scale)))
            })
        });
        staged.bytes = staged.entries.values().map(|e| e.bytes).sum();
        let result = (|| {
            let mut output = Vec::new();
            for (producer, p) in panels.iter().enumerate() {
                let (base, _) = p.group.slots();
                let mut draws = Vec::new();
                for (i, r) in p.plan.runs.iter().enumerate() {
                    if r.clip.width == 0. || r.clip.height == 0. {
                        continue;
                    }
                    let clip = BodyRect {
                        x: r.clip.x - r.position[0],
                        y: r.clip.y - r.position[1],
                        ..r.clip
                    };
                    for (mut draw, resource) in staged
                        .try_draw(
                            TextRun {
                                text: &r.text,
                                style: r.style,
                                position: r.position,
                                align: Align::Left,
                                baseline: Baseline::Alphabetic,
                                alpha: p.alpha * alphas.get(producer).map_or(1., |a| a[i]),
                            },
                            scale,
                            Some(Key::Panel(base + i as u8)),
                            None,
                            None,
                            Some(clip),
                        )?
                        .1
                    {
                        if crate::render::clip_overlay_text(&mut draw, r.clip, scale) {
                            // Preserve the producer quota before growing its staged output.
                            if draws.len()
                                == crate::render::PANEL_PRODUCER_DRAWS
                                    - crate::render::PANEL_CONTROL_DRAWS
                            {
                                return Err(PANEL_DRAW_QUOTA.into());
                            }
                            draws.push((draw, resource));
                        }
                    }
                }
                output.push(draws);
            }
            let mut counts = [0; 2];
            for (key, entry) in &staged.entries {
                if let Key::Panel(slot) = key {
                    counts[usize::from(*slot >= 48)] += entry.tiles.len();
                }
            }
            preflight_panel_totals(staged.bytes, counts)?;
            Ok(output)
        })();
        self.rasterizer = std::mem::take(&mut staged.rasterizer);
        if result.is_ok() {
            self.entries.retain(|k, _| !matches!(k, Key::Panel(_)));
            self.entries.extend(staged.entries);
            self.bytes = self.entries.values().map(|e| e.bytes).sum();
            self.clock = staged.clock;
            self.next_handle = staged.next_handle;
        }
        result
    }
    /// Rasterize at backing resolution. Position and opacity are deliberately
    /// outside the cache key, so movement/fading never uploads a new texture.
    /// The returned owned resource remains valid after cache eviction.
    /// Invalid or unrasterizable cosmetic labels produce no draws, so missing
    /// fonts, platform errors and raster budgets cannot terminate gameplay.
    pub fn draw(&mut self, run: TextRun<'_>, backing_scale: f64) -> Result<Vec<(Draw, Resource)>> {
        Ok(self
            .try_draw(run, backing_scale, None, None, None, None)
            .unwrap_or_default()
            .1)
    }
    /// A bounded semantic HUD slot; changed scores replace old entries, while
    /// already prepared frames retain their own immutable resource copies.
    /// Errors are retained so frame completion cannot silently omit HUD fields.
    pub fn draw_hud(
        &mut self,
        slot: u8,
        run: TextRun<'_>,
        backing_scale: f64,
        max_width: Option<f64>,
    ) -> Result<(f64, Vec<(Draw, Resource)>)> {
        if usize::from(slot) >= slither_client::presentation::MAX_HUD_TEXT_SPANS {
            return Err(HUD_SLOT_QUOTA.into());
        }
        self.try_draw(
            run,
            backing_scale,
            Some(Key::Hud(slot)),
            max_width,
            None,
            None,
        )
    }
    /// One replacing slot with its own pixel allowance; neither HUD retirement
    /// nor changing servers can evict nickname entries or consume HUD pixels.
    pub fn draw_minimap(
        &mut self,
        run: TextRun<'_>,
        backing_scale: f64,
        shadow_offset: [f64; 2],
        shadow_blur: f64,
        width: f64,
    ) -> Result<Vec<(Draw, Resource)>> {
        self.try_draw(
            run,
            backing_scale,
            Some(Key::Minimap),
            Some(width),
            Some([shadow_offset[0], shadow_offset[1], shadow_blur]),
            None,
        )
        .map(|(_, tiles)| tiles)
    }
    pub(crate) fn begin_hud_frame(&mut self, spans: usize) {
        // A shorter layout must not pin stale HUD pixels needed by later fields.
        // Prepared/queued frames already own their immutable resource copies.
        self.entries.retain(|key, _| match key {
            Key::Hud(slot) => usize::from(*slot) < spans,
            Key::Nickname(_) | Key::Minimap | Key::Panel(_) | Key::Menu(_) => true,
        });
        self.bytes = self.entries.values().map(|entry| entry.bytes).sum();
    }
    fn try_draw(
        &mut self,
        run: TextRun<'_>,
        backing_scale: f64,
        slot: Option<Key>,
        max_width: Option<f64>,
        shadow: Option<[f64; 3]>,
        clip: Option<BodyRect>,
    ) -> Result<(f64, Vec<(Draw, Resource)>)> {
        let px = run.style.size * backing_scale;
        let overlay_text = matches!(slot, Some(Key::Menu(_) | Key::Hud(_)));
        let font_limit = if overlay_text {
            f64::from(MAX_OVERLAY_TEXT_EXTENT)
        } else {
            128.
        };
        if !run.position.into_iter().all(f64::is_finite)
            || !(0. ..=1.).contains(&run.alpha)
            || !backing_scale.is_finite()
            || backing_scale <= 0.
            || !run.style.size.is_finite()
            || run.style.size <= 0.
            || !(1. ..=font_limit).contains(&px)
            || run.text.len() > 1024
        {
            return Err("invalid text run/style/budget".into());
        }
        if run.text.is_empty() {
            return Ok((0., Vec::new()));
        }
        let max_width = max_width.map(|w| (w * backing_scale).ceil().clamp(1., 8192.) as u32);
        let content = Content {
            max_width,
            text: run.text.into(),
            size: px.to_bits(),
            weight: run.style.weight,
            family: run.style.family,
            slant: run.style.slant,
            clip: clip.map(|c| [c.x, c.y, c.width, c.height].map(f64::to_bits)),
            color: run.style.color,
            shadow: shadow.map(|s| s.map(|v| (v * backing_scale).to_bits())),
            line_height: shadow.map(|_| (16. * backing_scale).to_bits()),
        };
        let key = slot.unwrap_or_else(|| Key::Nickname(content.clone()));
        self.clock += 1;
        if self.entries.get(&key).is_none_or(|e| e.content != content) {
            let mut raster = if matches!(key, Key::Minimap) {
                let glyphs = self.rasterize_minimap(&content)?;
                // A shadow uses its own colour's coverage transfer, never the
                // fill's already-preblended alpha. Its key keeps black and all
                // raster/layout parameters independent of the label's fill.
                let shadow_content = Content {
                    color: [0; 3],
                    ..content.clone()
                };
                if self
                    .minimap_shadow
                    .as_ref()
                    .is_none_or(|s| s.content != shadow_content)
                {
                    let raster = self.rasterize_minimap(&shadow_content)?;
                    let raster =
                        shadow_raster(raster, shadow_content.shadow.unwrap().map(f64::from_bits))?;
                    self.minimap_shadow = Some(CachedShadow {
                        content: shadow_content,
                        raster,
                    });
                }
                compose_shadow(glyphs, &self.minimap_shadow.as_ref().unwrap().raster)?
            } else if matches!(key, Key::Menu(_))
                && let Some(c) = clip
            {
                self.rasterizer.rasterize_window(
                    run.text,
                    px,
                    run.style,
                    max_width,
                    Some([c.x * backing_scale, (c.x + c.width) * backing_scale]),
                )?
            } else {
                self.rasterizer
                    .rasterize_styled(run.text, px, run.style, max_width)?
            };
            if let Some(clip) = clip
                && !matches!(key, Key::Menu(_))
            {
                raster = crop_panel(raster, clip, backing_scale)?;
            }
            let [w, h] = [raster.image.width, raster.image.height];
            if w == 0
                || h == 0
                || w > 8192
                || h > if overlay_text {
                    MAX_OVERLAY_TEXT_EXTENT
                } else {
                    512
                }
                || raster.image.pixels.len() != w as usize * h as usize * 4
                || !raster
                    .offset
                    .into_iter()
                    .chain([raster.advance, raster.middle])
                    .all(f64::is_finite)
            {
                return Err("text raster bounds".into());
            }
            let mut tiles = Vec::new();
            let mut bytes = 0;
            for start in (0..w).step_by(510) {
                let end = (start + 510).min(w);
                let left = start.saturating_sub(1);
                let right = (end + 1).min(w);
                let width = right - left;
                let mut pixels = Vec::with_capacity(width as usize * h as usize * 4);
                for row in raster.image.pixels.chunks_exact(w as usize * 4) {
                    pixels.extend_from_slice(&row[left as usize * 4..right as usize * 4]);
                }
                bytes += pixels.len();
                let namespace = self.next_handle / crate::bank::HANDLE_NAMESPACE_SIZE;
                self.next_handle = self
                    .next_handle
                    .checked_add(1)
                    .filter(|handle| handle / crate::bank::HANDLE_NAMESPACE_SIZE == namespace)
                    .ok_or("text handle overflow")?;
                tiles.push(Tile {
                    resource: Resource {
                        asset: BodyAsset {
                            handle: self.next_handle,
                            revision: 1,
                            width,
                            height: h,
                            status: BodyAssetStatus::Ready,
                        },
                        image: RgbaImage {
                            width,
                            height: h,
                            pixels,
                        }
                        .into(),
                    },
                    start,
                    source_x: start - left,
                    width: end - start,
                });
            }
            let (byte_limit, run_limit) = match &key {
                Key::Hud(_) => (
                    HUD_CACHE_BYTES,
                    slither_client::presentation::MAX_HUD_TEXT_SPANS,
                ),
                Key::Minimap => (MINIMAP_CACHE_BYTES, 1),
                Key::Panel(_) => (PANEL_CACHE_BYTES, PANEL_CACHE_RUNS),
                Key::Menu(_) => (MENU_CACHE_BYTES, MENU_CACHE_RUNS),
                Key::Nickname(_) => (
                    CACHE_BYTES
                        - HUD_CACHE_BYTES
                        - MINIMAP_CACHE_BYTES
                        - MINIMAP_SHADOW_CACHE_BYTES,
                    NICKNAME_CACHE_RUNS,
                ),
            };
            if bytes > byte_limit {
                return Err(TEXT_CACHE_RUN_QUOTA.into());
            }
            if let Some(old) = self.entries.remove(&key) {
                self.bytes -= old.bytes;
            }
            loop {
                let group: Vec<_> = self
                    .entries
                    .iter()
                    .filter(|(k, _)| k.group() == key.group())
                    .collect();
                if group.len() < run_limit
                    && group.iter().map(|(_, e)| e.bytes).sum::<usize>() + bytes <= byte_limit
                {
                    break;
                }
                if key.group() != 0 {
                    return Err(TEXT_CACHE_PIXEL_QUOTA.into());
                }
                let oldest = group
                    .into_iter()
                    .min_by_key(|(_, e)| e.used)
                    .map(|(k, _)| k.clone())
                    .ok_or(TEXT_CACHE_QUOTA)?;
                self.bytes -= self.entries.remove(&oldest).unwrap().bytes;
            }
            self.bytes += bytes;
            self.entries.insert(
                key.clone(),
                Entry {
                    content,
                    tiles,
                    bytes,
                    offset: raster.offset,
                    advance: raster.advance,
                    middle: raster.middle,
                    used: self.clock,
                },
            );
        }
        let entry = self.entries.get_mut(&key).unwrap();
        entry.used = self.clock;
        let x = match run.align {
            Align::Left => 0.,
            Align::Center => entry.advance / 2.,
            Align::Right => entry.advance,
        };
        let y = match run.baseline {
            Baseline::Alphabetic => 0.,
            Baseline::Middle => entry.middle,
        };
        Ok((
            entry.advance / backing_scale,
            entry
                .tiles
                .iter()
                .map(|tile| {
                    let mut draw = Draw {
                        corners: None,
                        repeat_uv: None,
                        black_effect: false,
                        asset: tile.resource.asset,
                        // Neighbor pixels outside the crop preserve linear sampling
                        // across tile seams at fractional screen positions.
                        source: Some(BodyRect {
                            x: f64::from(tile.source_x),
                            y: 0.,
                            width: f64::from(tile.width),
                            height: f64::from(tile.resource.asset.height),
                        }),
                        destination: BodyRect {
                            x: (entry.offset[0] + f64::from(tile.start) - x) / backing_scale,
                            y: (entry.offset[1] + y) / backing_scale,
                            width: f64::from(tile.width) / backing_scale,
                            height: f64::from(tile.resource.asset.height) / backing_scale,
                        },
                        translation: run.position,
                        rotation: 0.,
                        effective_alpha: run.alpha,
                        blend: BodyBlend::SourceOver,
                    };
                    if !matches!(key, Key::Nickname(_)) {
                        draw.snap_overlay(backing_scale);
                    }
                    (draw, tile.resource.clone())
                })
                .collect(),
        ))
    }
    fn rasterize_minimap(&mut self, content: &Content) -> Result<Raster> {
        let px = f64::from_bits(content.size);
        let width = f64::from(content.max_width.unwrap());
        let mut raster = self.rasterizer.rasterize_styled(
            &content.text,
            px,
            TextStyle {
                size: px,
                weight: content.weight,
                family: content.family,
                slant: content.slant,
                color: content.color,
            },
            None,
        )?;
        if raster.advance > width {
            // CSS centres fitting lines but starts overwide words at the left.
            let mut lines = Vec::new();
            let mut line = String::new();
            for word in content.text.split_whitespace() {
                let candidate = if line.is_empty() {
                    word.into()
                } else {
                    format!("{line} {word}")
                };
                let measured = self.rasterizer.rasterize_styled(
                    &candidate,
                    px,
                    TextStyle {
                        size: px,
                        weight: content.weight,
                        family: content.family,
                        slant: content.slant,
                        color: content.color,
                    },
                    None,
                )?;
                if !line.is_empty() && measured.advance > width {
                    lines.push(self.rasterizer.rasterize_styled(
                        &line,
                        px,
                        TextStyle {
                            size: px,
                            weight: content.weight,
                            family: content.family,
                            slant: content.slant,
                            color: content.color,
                        },
                        None,
                    )?);
                    line = word.into();
                } else {
                    line = candidate;
                }
            }
            if !line.is_empty() {
                lines.push(self.rasterizer.rasterize_styled(
                    &line,
                    px,
                    TextStyle {
                        size: px,
                        weight: content.weight,
                        family: content.family,
                        slant: content.slant,
                        color: content.color,
                    },
                    None,
                )?);
            }
            if !lines.is_empty() {
                raster = label_lines(lines, width, f64::from_bits(content.line_height.unwrap()))?;
            }
        }
        let [dx, dy, blur] = content.shadow.unwrap().map(f64::from_bits);
        let pad = (1.5 * blur + dx.abs().max(dy.abs())).ceil();
        Ok(crop_center(raster, width + 2. * pad))
    }
    pub fn cached_runs(&self) -> usize {
        self.entries.len() + usize::from(self.minimap_shadow.is_some())
    }
    pub fn cached_bytes(&self) -> usize {
        self.bytes
            + self
                .minimap_shadow
                .as_ref()
                .map_or(0, |s| s.raster.image.pixels.len())
    }
}

impl slither_client::presentation::TextMetrics for TextCache {
    fn measure(
        &mut self,
        text: &str,
        style: TextStyle,
    ) -> std::result::Result<slither_client::presentation::TextMeasurement, &'static str> {
        self.measure_panel(text, style)
            .map_err(|_| "Platform panel metrics unavailable")
    }
}

fn crop_panel(mut raster: Raster, c: BodyRect, scale: f64) -> Result<Raster> {
    let [x, y] = raster.offset;
    let left = (c.x * scale - x)
        .floor()
        .max(0.)
        .min(f64::from(raster.image.width)) as u32;
    let top = (c.y * scale - y)
        .floor()
        .max(0.)
        .min(f64::from(raster.image.height)) as u32;
    let right = ((c.x + c.width) * scale - x)
        .ceil()
        .max(f64::from(left))
        .min(f64::from(raster.image.width)) as u32;
    let bottom = ((c.y + c.height) * scale - y)
        .ceil()
        .max(f64::from(top))
        .min(f64::from(raster.image.height)) as u32;
    if right == left || bottom == top {
        // A blank glyph still owns one transparent pixel, never an invalid asset.
        raster.image = RgbaImage {
            width: 1,
            height: 1,
            pixels: vec![0; 4],
        };
    } else {
        let mut pixels = Vec::new();
        for row in top..bottom {
            let start = ((row * raster.image.width + left) * 4) as usize;
            pixels.extend_from_slice(
                &raster.image.pixels[start..start + ((right - left) * 4) as usize],
            );
        }
        raster.image = RgbaImage {
            width: right - left,
            height: bottom - top,
            pixels,
        };
    }
    raster.offset = [x + f64::from(left), y + f64::from(top)];
    Ok(raster)
}

// The input mask has already gone through the shadow colour's own transfer.
// As with minimap backgrounds/materials, approximate the blur with sigma=blur/2.
// Compose before applying run alpha so the holder/local fades affect the group.
fn shadow_raster(mut raster: Raster, [dx, dy, blur]: [f64; 3]) -> Result<Raster> {
    if ![dx, dy, blur].into_iter().all(f64::is_finite)
        || !(0. ..=96.).contains(&blur)
        || dx.abs() > 96.
        || dy.abs() > 96.
    {
        return Err("text shadow budget".into());
    }
    let pad = (1.5 * blur + dx.abs().max(dy.abs())).ceil() as u32;
    let w = raster.image.width + 2 * pad;
    let h = raster.image.height + 2 * pad;
    if w > 8192 || h > 512 || w as usize * h as usize * 4 > MINIMAP_SHADOW_CACHE_BYTES {
        return Err("minimap text pixel budget".into());
    }
    let mut mask = image::GrayImage::new(w, h);
    for y in 0..raster.image.height {
        for x in 0..raster.image.width {
            mask.put_pixel(
                (f64::from(x + pad) + dx).round() as u32,
                (f64::from(y + pad) + dy).round() as u32,
                image::Luma([raster.image.pixels[((y * raster.image.width + x) * 4 + 3) as usize]]),
            );
        }
    }
    let mask = if blur > 0. {
        image::imageops::blur(&mask, (blur / 2.) as f32)
    } else {
        mask
    };
    let mut pixels = vec![0; w as usize * h as usize * 4];
    for (p, a) in pixels
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(mask.into_raw())
    {
        p[3] = a;
    }
    raster.image = RgbaImage {
        width: w,
        height: h,
        pixels,
    };
    raster.offset = raster.offset.map(|v| v - f64::from(pad));
    Ok(raster)
}

fn compose_shadow(glyphs: Raster, shadow: &Raster) -> Result<Raster> {
    let mut image = shadow.image.clone();
    let x = (glyphs.offset[0] - shadow.offset[0]) as u32;
    let y = (glyphs.offset[1] - shadow.offset[1]) as u32;
    if x + glyphs.image.width > image.width || y + glyphs.image.height > image.height {
        return Err("minimap glyph/shadow geometry mismatch".into());
    }
    for row in 0..glyphs.image.height {
        for col in 0..glyphs.image.width {
            let src = &glyphs.image.pixels[((row * glyphs.image.width + col) * 4) as usize..][..4];
            let dst = &mut image.pixels[(((row + y) * image.width + col + x) * 4) as usize..][..4];
            dst[..3].copy_from_slice(&src[..3]);
            dst[3] = (u16::from(src[3])
                + (u16::from(dst[3]) * (255 - u16::from(src[3])) + 127) / 255)
                as u8;
        }
    }
    Ok(Raster {
        image,
        offset: shadow.offset,
        advance: glyphs.advance,
        middle: glyphs.middle,
    })
}

fn label_lines(lines: Vec<Raster>, width: f64, line_height: f64) -> Result<Raster> {
    // Only three 16px lines can contribute glyphs/shadow to the 37px label.
    let lines: Vec<_> = lines
        .into_iter()
        .take(3)
        .enumerate()
        .map(|(i, mut line)| {
            line.offset[0] = (line.offset[0] + ((width - line.advance) / 2.).max(0.)).round();
            line.offset[1] = (line.offset[1] + i as f64 * line_height).round();
            line.advance = width;
            crop_center(line, width + 192.)
        })
        .collect();
    let left = lines
        .iter()
        .map(|l| l.offset[0])
        .fold(f64::INFINITY, f64::min);
    let top = lines
        .iter()
        .map(|l| l.offset[1])
        .fold(f64::INFINITY, f64::min);
    let right = lines
        .iter()
        .map(|l| l.offset[0] + f64::from(l.image.width))
        .fold(0., f64::max);
    let bottom = lines
        .iter()
        .map(|l| l.offset[1] + f64::from(l.image.height))
        .fold(0., f64::max);
    let w = (right - left) as u32;
    let h = (bottom - top) as u32;
    if w == 0 || h == 0 || w > 8192 || h > 512 || w as usize * h as usize * 4 > MINIMAP_CACHE_BYTES
    {
        return Err("minimap text line budget".into());
    }
    let mut pixels = vec![0; w as usize * h as usize * 4];
    for line in lines {
        for (y, row) in line
            .image
            .pixels
            .chunks_exact(line.image.width as usize * 4)
            .enumerate()
        {
            let x = (line.offset[0] - left) as usize;
            let y = (line.offset[1] - top) as usize + y;
            let start = (y * w as usize + x) * 4;
            pixels[start..start + row.len()].copy_from_slice(row);
        }
    }
    Ok(Raster {
        image: RgbaImage {
            width: w,
            height: h,
            pixels,
        },
        offset: [left, top],
        advance: width,
        middle: 0.,
    })
}

fn crop_center(mut raster: Raster, width: f64) -> Raster {
    let left = (raster.advance / 2. - width / 2. - raster.offset[0])
        .floor()
        .clamp(0., f64::from(raster.image.width)) as u32;
    let right = (raster.advance / 2. + width / 2. - raster.offset[0])
        .ceil()
        .clamp(f64::from(left), f64::from(raster.image.width)) as u32;
    let mut pixels = Vec::new();
    for row in raster
        .image
        .pixels
        .chunks_exact(raster.image.width as usize * 4)
    {
        pixels.extend_from_slice(&row[left as usize * 4..right as usize * 4]);
    }
    raster.image.width = right - left;
    raster.image.pixels = pixels;
    raster.offset[0] += f64::from(left);
    raster
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod menu_budget_tests {
    use super::*;
    fn run(text: &str, size: f64) -> TextRun<'_> {
        TextRun {
            text,
            style: TextStyle {
                size,
                weight: Weight::Bold,
                family: Family::Arial,
                slant: Slant::Normal,
                color: [255; 3],
            },
            position: [10., 30.],
            align: Align::Left,
            baseline: Baseline::Alphabetic,
            alpha: 1.,
        }
    }
    #[test]
    fn visible_window_preserves_full_raster_pixels_and_metrics() {
        let mut rasterizer = platform::Rasterizer::new();
        let value = "AV Wfi ".repeat(12);
        let style = run(&value, 20.).style;
        for px in [20., 30., 100.] {
            let full = rasterizer
                .rasterize_styled(&value, px, style, None)
                .unwrap();
            for start in [0.25, full.advance * 0.7] {
                let part = rasterizer
                    .rasterize_window(&value, px, style, None, Some([start, start + 100.]))
                    .unwrap();
                assert!(part.image.width <= 104);
                assert_eq!(
                    (part.advance, part.middle, part.offset[1]),
                    (full.advance, full.middle, full.offset[1])
                );
                let x = (part.offset[0] - full.offset[0]) as usize * 4;
                for (a, b) in part
                    .image
                    .pixels
                    .chunks_exact(part.image.width as usize * 4)
                    .zip(
                        full.image
                            .pixels
                            .chunks_exact(full.image.width as usize * 4),
                    )
                {
                    assert_eq!(a, &b[x..x + a.len()]);
                }
            }
        }
    }
    #[test]
    fn menu_composition_admits_exact_byte_boundary_and_rolls_back_one_more() {
        let mut cache = TextCache::default();
        let runs = [run("Play", 20.)];
        let first = cache.compose_menu(&runs, &[], 1., (0, 0), Ok).unwrap();
        let text_bytes = cache.cached_bytes();
        let material_bytes = MENU_CACHE_BYTES - text_bytes;
        cache
            .compose_menu(&runs, &[], 1., (0, material_bytes), Ok)
            .unwrap();
        let next_handle = cache.next_handle;
        let mut published = false;
        assert!(
            cache
                .compose_menu(&runs, &[], 1., (0, material_bytes + 1), |_| {
                    published = true;
                    Ok(())
                })
                .is_err()
        );
        assert!(!published);
        assert_eq!(cache.next_handle, next_handle);
        assert_eq!(cache.cached_bytes(), text_bytes);
        let retained = cache.compose_menu(&runs, &[], 1., (0, 0), Ok).unwrap();
        assert_eq!(first[0][0].0.asset, retained[0][0].0.asset);
    }
    #[test]
    fn menu_slots_bytes_and_failed_finish_are_transactional_and_isolated() {
        assert!(preflight_menu_totals(64, MENU_CACHE_BYTES).is_ok());
        assert!(preflight_menu_totals(65, 0).is_err());
        assert!(preflight_menu_totals(0, MENU_CACHE_BYTES + 1).is_err());
        let mut cache = TextCache::default();
        let nickname = cache.draw(run("world", 15.), 1.).unwrap()[0].0.asset;
        let hud = cache.draw_hud(0, run("score", 15.), 1., None).unwrap().1[0]
            .0
            .asset;
        let runs: Vec<_> = (0..64).map(|_| run("Play", 20.)).collect();
        let old = cache.compose_menu(&runs, &[], 1., (0, 0), Ok).unwrap();
        let bytes = cache.cached_bytes();
        let handles: Vec<_> = old.iter().flatten().map(|(d, _)| d.asset).collect();
        assert_eq!(
            cache
                .entries
                .keys()
                .filter(|k| matches!(k, Key::Menu(_)))
                .count(),
            64
        );
        assert!(cache.compose_menu(&runs, &[], 1., (1, 0), Ok).is_err());
        assert!(
            cache
                .compose_menu(
                    &[run("replacement", 20.)],
                    &[],
                    1.,
                    (0, 0),
                    |_| -> Result<()> { Err("frame rejected".into()) }
                )
                .is_err()
        );
        let large = "M".repeat(100);
        let big: Vec<_> = (0..16).map(|_| run(&large, 64.)).collect();
        assert!(cache.compose_menu(&big, &[], 1., (0, 0), Ok).is_err());
        assert_eq!(cache.cached_bytes(), bytes);
        let after = cache.compose_menu(&runs, &[], 1., (0, 0), Ok).unwrap();
        assert_eq!(
            handles,
            after
                .iter()
                .flatten()
                .map(|(d, _)| d.asset)
                .collect::<Vec<_>>()
        );
        cache.compose_menu(&[], &[], 1., (0, 0), Ok).unwrap();
        let assets: Vec<_> = cache
            .entries
            .values()
            .flat_map(|e| e.tiles.iter().map(|t| t.resource.asset))
            .collect();
        assert_eq!(assets.len(), 2);
        assert!(assets.contains(&nickname));
        assert!(assets.contains(&hud));
        assert!(!old[0][0].1.image.pixels.is_empty());
    }
}

#[cfg(all(test, windows))]
mod hud_budget_tests {
    use super::*;
    use crate::{bank::Bank, frame::Frame, hud, render};
    use slither_client::presentation::{HudOpacity, LeaderboardRow};

    fn label_run(color: [u8; 3]) -> TextRun<'static> {
        TextRun {
            text: "server 42",
            style: TextStyle {
                family: slither_client::presentation::TextFamily::Arial,
                slant: slither_client::presentation::TextSlant::Normal,
                size: 14.,
                weight: Weight::Normal,
                color,
            },
            position: [80., 13.],
            align: Align::Center,
            baseline: Baseline::Alphabetic,
            alpha: 0.5,
        }
    }

    #[test]
    fn minimap_shadow_uses_black_coverage_independently_of_the_label_fill() {
        let mut cache = TextCache::default();
        let first = cache
            .draw_minimap(label_run([255; 3]), 1., [0., 1.], 8., 160.)
            .unwrap();
        let glyph_content = cache.entries[&Key::Minimap].content.clone();
        let shadow_content = cache.minimap_shadow.as_ref().unwrap().content.clone();
        assert_eq!(glyph_content.color, [255; 3]);
        assert_eq!(shadow_content.color, [0; 3]);
        let glyphs = cache.rasterize_minimap(&glyph_content).unwrap();
        let black = cache.rasterize_minimap(&shadow_content).unwrap();
        assert_ne!(
            glyphs
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect::<Vec<_>>(),
            black
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect::<Vec<_>>()
        );
        let black_shadow = shadow_raster(black, [0., 1., 8.]).unwrap();
        assert_eq!(
            cache.minimap_shadow.as_ref().unwrap().raster.image.pixels,
            black_shadow.image.pixels
        );
        let expected = compose_shadow(glyphs, &black_shadow).unwrap();
        assert_eq!(first[0].1.image.pixels, expected.image.pixels);
        let shadow_pixels = cache
            .minimap_shadow
            .as_ref()
            .unwrap()
            .raster
            .image
            .pixels
            .as_ptr();
        let second = cache
            .draw_minimap(label_run([32; 3]), 1., [0., 1.], 8., 160.)
            .unwrap();
        assert_ne!(first[0].0.asset, second[0].0.asset);
        assert_eq!(
            cache
                .minimap_shadow
                .as_ref()
                .unwrap()
                .raster
                .image
                .pixels
                .as_ptr(),
            shadow_pixels
        );
        assert!(cache.minimap_shadow.as_ref().unwrap().content == shadow_content);
        assert_eq!(cache.cached_runs(), 2);
        assert!(cache.cached_bytes() <= MINIMAP_CACHE_BYTES + MINIMAP_SHADOW_CACHE_BYTES);
    }

    #[test]
    fn minimap_shadow_key_tracks_layout_font_scale_and_blur_but_not_placement_or_fade() {
        let mut cache = TextCache::default();
        cache
            .draw_minimap(label_run([255; 3]), 1., [0., 1.], 8., 32.)
            .unwrap();
        let first = cache.minimap_shadow.as_ref().unwrap().content.clone();
        let pixels = cache
            .minimap_shadow
            .as_ref()
            .unwrap()
            .raster
            .image
            .pixels
            .as_ptr();
        let moved = TextRun {
            position: [123., 456.],
            alpha: 0.125,
            ..label_run([255; 3])
        };
        cache.draw_minimap(moved, 1., [0., 1.], 8., 32.).unwrap();
        assert_eq!(
            cache
                .minimap_shadow
                .as_ref()
                .unwrap()
                .raster
                .image
                .pixels
                .as_ptr(),
            pixels
        );
        for (text, size, weight, scale, offset, blur, width) in [
            ("server 43", 14., Weight::Normal, 1., [0., 1.], 8., 32.),
            ("server 42", 15., Weight::Normal, 1., [0., 1.], 8., 32.),
            ("server 42", 14., Weight::Bold, 1., [0., 1.], 8., 32.),
            ("server 42", 14., Weight::Normal, 2., [0., 1.], 8., 32.),
            ("server 42", 14., Weight::Normal, 1., [1., 2.], 8., 32.),
            ("server 42", 14., Weight::Normal, 1., [0., 1.], 4., 32.),
            ("server 42", 14., Weight::Normal, 1., [0., 1.], 8., 160.),
            // Same backing font, width and shadow, different wrapped line height.
            ("server 42", 28., Weight::Normal, 0.5, [0., 2.], 16., 64.),
        ] {
            let run = TextRun {
                text,
                style: TextStyle {
                    family: slither_client::presentation::TextFamily::Arial,
                    slant: slither_client::presentation::TextSlant::Normal,
                    size,
                    weight,
                    color: [255; 3],
                },
                ..label_run([255; 3])
            };
            cache.draw_minimap(run, scale, offset, blur, width).unwrap();
            assert!(cache.minimap_shadow.as_ref().unwrap().content != first);
            assert_eq!(cache.cached_runs(), 2);
        }
    }

    #[test]
    fn narrow_label_wraps_at_the_space_and_centres_the_fitting_second_line() {
        let mut cache = TextCache::default();
        cache
            .draw_minimap(
                TextRun {
                    text: "server 42",
                    style: TextStyle {
                        family: slither_client::presentation::TextFamily::Arial,
                        slant: slither_client::presentation::TextSlant::Normal,
                        size: 14.,
                        weight: Weight::Normal,
                        color: [255; 3],
                    },
                    position: [16., 13.],
                    align: Align::Center,
                    baseline: Baseline::Alphabetic,
                    alpha: 0.5,
                },
                1.,
                [0., 1.],
                8.,
                32.,
            )
            .unwrap();
        let entry = &cache.entries[&Key::Minimap];
        let mut first = Vec::new();
        let mut second = Vec::new();
        for tile in &entry.tiles {
            for (i, pixel) in tile
                .resource
                .image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .enumerate()
            {
                if pixel[0] == 0 {
                    continue;
                }
                let x = (i % tile.resource.image.width as usize) as f64
                    + entry.offset[0]
                    + f64::from(tile.start);
                let y = (i / tile.resource.image.width as usize) as f64 + entry.offset[1];
                if y <= 0. {
                    first.push(x);
                } else {
                    second.push(x);
                }
            }
        }
        assert!(!first.is_empty() && !second.is_empty());
        // CSS starts an overwide word at the left; the shorter number centres.
        assert!(first.iter().copied().fold(f64::INFINITY, f64::min).abs() <= 1.);
        let left = second.iter().copied().fold(f64::INFINITY, f64::min);
        let right = second.iter().copied().fold(f64::NEG_INFINITY, f64::max) + 1.;
        assert!(((left + right) / 2. - 16.).abs() <= 1.);
    }

    #[test]
    fn minimap_replaces_one_slot_and_keeps_the_full_nickname_and_hud_allowances() {
        let mut cache = TextCache::default();
        fn run(text: &str) -> TextRun<'_> {
            TextRun {
                text,
                style: TextStyle {
                    family: slither_client::presentation::TextFamily::Arial,
                    slant: slither_client::presentation::TextSlant::Normal,
                    size: 14.,
                    weight: Weight::Normal,
                    color: [255; 3],
                },
                position: [80., 166.],
                align: Align::Center,
                baseline: Baseline::Alphabetic,
                alpha: 0.5,
            }
        }
        for i in 0..NICKNAME_CACHE_RUNS {
            cache.draw(run(&format!("World {i}")), 1.).unwrap();
        }
        let nicknames: Vec<_> = cache
            .entries
            .iter()
            .filter(|(k, _)| k.group() == 0)
            .map(|(k, e)| (k.clone(), e.tiles[0].resource.asset))
            .collect();
        let mut first = None;
        for i in 0..300 {
            let dpr: f64 = [1., 2., 3.][i % 3];
            let tiles = cache
                .draw_minimap(run(&format!("server {}", i + 1)), dpr, [0., 1.], 8., 160.)
                .unwrap();
            if first.is_none() {
                first = Some(tiles.clone());
            }
            let entry = &cache.entries[&Key::Minimap];
            assert_eq!(entry.content.text, format!("server {}", i + 1));
            assert_eq!(entry.content.color, [255; 3]);
            assert_eq!(entry.content.weight, Weight::Normal);
            assert_eq!(f64::from_bits(entry.content.size), 14. * dpr.min(3.));
            assert!(entry.bytes <= MINIMAP_CACHE_BYTES);
            assert_eq!(cache.cached_runs(), NICKNAME_CACHE_RUNS + 2);
            cache.begin_hud_frame(0);
            assert!(cache.entries.contains_key(&Key::Minimap));
        }
        for (key, asset) in nicknames {
            assert_eq!(cache.entries[&key].tiles[0].resource.asset, asset);
        }
        assert_ne!(
            first.unwrap()[0].0.asset,
            cache.entries[&Key::Minimap].tiles[0].resource.asset
        );
        for slot in 0..slither_client::presentation::MAX_HUD_TEXT_SPANS {
            cache.draw_hud(slot as u8, run("HUD"), 1., None).unwrap();
        }
        assert_eq!(cache.cached_runs(), CACHE_RUNS);
        assert!(cache.cached_bytes() <= CACHE_BYTES);
    }

    #[test]
    fn dense_high_dpr_hud_retains_every_requested_span_and_reuses_its_pixels() {
        // Authored synthetic worst-name board; no server or GPU.
        let view = hud::View {
            playing: true,
            opacity: HudOpacity {
                readout: 1.,
                header: 0.85,
                rows: 1.,
                announcement: 1.,
            },
            status: "CONNECTED".into(),
            score: "10000".into(),
            final_score: "--".into(),
            rank: "5".into(),
            population: "100".into(),
            message: None,
            rows: (1..=10)
                .map(|ordinal| LeaderboardRow {
                    ordinal,
                    number_text: format!("#{ordinal}"),
                    score: (10000 + ordinal) as f64,
                    score_text: (10000 + ordinal).to_string(),
                    name_text: if ordinal == 5 {
                        "Player".into()
                    } else {
                        "W".repeat(255)
                    },
                    css_color: "#80d0d0",
                    own: ordinal == 5,
                    opacity_request: 1.,
                })
                .collect(),
        };
        let mut bank = Bank::new().unwrap();
        let nickname = TextRun {
            text: "World nickname",
            style: TextStyle {
                family: slither_client::presentation::TextFamily::Arial,
                slant: slither_client::presentation::TextSlant::Normal,
                size: 15.,
                weight: Weight::Normal,
                color: [255; 3],
            },
            position: [20., 20.],
            align: Align::Left,
            baseline: Baseline::Alphabetic,
            alpha: 1.,
        };
        let nickname_asset = bank.text.draw(nickname, 1.).unwrap()[0].0.asset;
        for (extent, dpr, count) in [
            ([960, 540], 1., 37),
            ([1920, 1080], 2., 37),
            ([2880, 1620], 3., 37),
            ([3840, 2160], 1.25, 37),
            ([3840, 2160], 1.5, 37),
            ([3808, 2156], 1.75, 37),
            ([3840, 2160], 2., 37),
        ] {
            let plan = hud::plan(&view, extent, dpr).unwrap();
            let expected: Vec<_> = plan.lines.iter().flat_map(|line| &line.spans).collect();
            assert_eq!(expected.len(), count);
            let mut previous = None;
            for _ in 0..2 {
                let out = render::finish_hud(
                    render::PreparedFrame {
                        overlay: None,
                        frame: Frame {
                            identity: 1,
                            tick: 0,
                            extent,
                            viewport_scale: dpr,
                            draws: vec![],
                            snakes: vec![],
                        },
                        resources: vec![],
                    },
                    &mut bank,
                    &view,
                )
                .unwrap();
                assert!(!out.frame.draws.is_empty());
                for (slot, span) in expected.iter().enumerate() {
                    let entry = bank
                        .text
                        .entries
                        .get(&Key::Hud(slot as u8))
                        .expect("every requested HUD span must be cached");
                    assert_eq!(entry.content.text, span.text);
                    assert_eq!(entry.content.weight, span.style.weight);
                    assert_eq!(entry.content.color, span.style.color);
                    assert_eq!(f64::from_bits(entry.content.size), span.style.size * dpr);
                    assert!(f64::from_bits(entry.content.size) <= 128.);
                    assert!(
                        entry.tiles.iter().any(|tile| out
                            .frame
                            .draws
                            .iter()
                            .any(|d| d.asset == tile.resource.asset)),
                        "requested span was cached but not submitted: {}",
                        span.text
                    );
                }
                assert_eq!(
                    bank.text.entries.keys().filter(|key| key.is_hud()).count(),
                    count
                );
                assert!(
                    bank.text
                        .entries
                        .iter()
                        .filter(|(key, _)| key.is_hud())
                        .map(|(_, entry)| entry.bytes)
                        .sum::<usize>()
                        <= HUD_CACHE_BYTES
                );
                assert!(
                    bank.text
                        .entries
                        .values()
                        .flat_map(|entry| &entry.tiles)
                        .any(|tile| tile.resource.asset == nickname_asset)
                );
                let assets: Vec<_> = out.frame.draws.iter().map(|d| d.asset).collect();
                if let Some(previous) = previous {
                    assert_eq!(assets, previous);
                }
                previous = Some(assets);
            }
        }
    }
}
