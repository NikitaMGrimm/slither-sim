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

pub const CACHE_BYTES: usize = 4 * 1024 * 1024;
pub const CACHE_RUNS: usize = 256;

pub use slither_client::presentation::{
    PLATFORM_FONT_FAMILY as FONT_FAMILY, TextAlign as Align, TextBaseline as Baseline, TextStyle,
    TextWeight as Weight,
};
// HUD slots replace their previous content and have a separate allowance. Fast
// score updates cannot evict world labels from the nickname LRU.
pub const HUD_CACHE_BYTES: usize = 2 * 1024 * 1024;
// Preserve ordinary DPR 1–3 text. Denser displays upscale these bounded rasters
// rather than exhausting the HUD allowance or exceeding the font-pixel limit.
pub const MAX_HUD_RASTER_SCALE: f64 = 3.;
const NICKNAME_CACHE_RUNS: usize = CACHE_RUNS - slither_client::presentation::MAX_HUD_TEXT_SPANS;
pub const MAX_NICKNAME_RUNS_PER_FRAME: usize = 128;

pub struct TextRun<'a> {
    pub text: &'a str,
    pub style: TextStyle,
    pub position: [f64; 2],
    pub align: Align,
    pub baseline: Baseline,
    pub alpha: f64,
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
    color: [u8; 3],
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Nickname(Content),
    Hud(u8),
}
impl Key {
    fn is_hud(&self) -> bool {
        matches!(self, Self::Hud(_))
    }
}
struct Entry {
    content: Content,
    tiles: Vec<Tile>,
    bytes: usize,
    offset: [f64; 2],
    advance: f64,
    middle: f64,
    used: u64,
}
struct Tile {
    resource: Resource,
    start: u32,
    source_x: u32,
    width: u32,
}

pub struct TextCache {
    rasterizer: platform::Rasterizer,
    entries: BTreeMap<Key, Entry>,
    bytes: usize,
    clock: u64,
    next_handle: u64,
}
impl Default for TextCache {
    fn default() -> Self {
        Self {
            rasterizer: platform::Rasterizer::new(),
            entries: BTreeMap::new(),
            bytes: 0,
            clock: 0,
            next_handle: crate::bank::TEXT_HANDLE_BASE,
        }
    }
}
impl TextCache {
    /// Rasterize at backing resolution. Position and opacity are deliberately
    /// outside the cache key, so movement/fading never uploads a new texture.
    /// The returned owned resource remains valid after cache eviction.
    /// Invalid or unrasterizable cosmetic labels produce no draws, so missing
    /// fonts, platform errors and raster budgets cannot terminate gameplay.
    pub fn draw(&mut self, run: TextRun<'_>, backing_scale: f64) -> Result<Vec<(Draw, Resource)>> {
        Ok(self
            .try_draw(run, backing_scale, None, None)
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
            return Err("HUD text slot budget".into());
        }
        self.try_draw(run, backing_scale, Some(slot), max_width)
    }
    pub(crate) fn begin_hud_frame(&mut self, spans: usize) {
        // A shorter layout must not pin stale HUD pixels needed by later fields.
        // Prepared/queued frames already own their immutable resource copies.
        self.entries.retain(|key, _| match key {
            Key::Hud(slot) => usize::from(*slot) < spans,
            Key::Nickname(_) => true,
        });
        self.bytes = self.entries.values().map(|entry| entry.bytes).sum();
    }
    fn try_draw(
        &mut self,
        run: TextRun<'_>,
        backing_scale: f64,
        slot: Option<u8>,
        max_width: Option<f64>,
    ) -> Result<(f64, Vec<(Draw, Resource)>)> {
        let px = run.style.size * backing_scale;
        if !run.position.into_iter().all(f64::is_finite)
            || !(0. ..=1.).contains(&run.alpha)
            || !backing_scale.is_finite()
            || backing_scale <= 0.
            || !run.style.size.is_finite()
            || run.style.size <= 0.
            || !(1. ..=128.).contains(&px)
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
            color: run.style.color,
        };
        let key = slot.map_or_else(|| Key::Nickname(content.clone()), Key::Hud);
        self.clock += 1;
        if self.entries.get(&key).is_none_or(|e| e.content != content) {
            let raster = self.rasterizer.rasterize_limited(
                run.text,
                px,
                run.style.weight,
                run.style.color,
                max_width,
            )?;
            let [w, h] = [raster.image.width, raster.image.height];
            if w == 0
                || h == 0
                || w > 8192
                || h > 512
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
                self.next_handle = self
                    .next_handle
                    .checked_add(1)
                    .filter(|handle| {
                        *handle < crate::bank::TEXT_HANDLE_BASE + crate::bank::HANDLE_NAMESPACE_SIZE
                    })
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
                        },
                    },
                    start,
                    source_x: start - left,
                    width: end - start,
                });
            }
            let (byte_limit, run_limit) = if key.is_hud() {
                (
                    HUD_CACHE_BYTES,
                    slither_client::presentation::MAX_HUD_TEXT_SPANS,
                )
            } else {
                (CACHE_BYTES - HUD_CACHE_BYTES, NICKNAME_CACHE_RUNS)
            };
            if bytes > byte_limit {
                return Err("text cache run budget".into());
            }
            if let Some(old) = self.entries.remove(&key) {
                self.bytes -= old.bytes;
            }
            loop {
                let group: Vec<_> = self
                    .entries
                    .iter()
                    .filter(|(k, _)| k.is_hud() == key.is_hud())
                    .collect();
                if group.len() < run_limit
                    && group.iter().map(|(_, e)| e.bytes).sum::<usize>() + bytes <= byte_limit
                {
                    break;
                }
                if key.is_hud() {
                    return Err("HUD text pixel budget".into());
                }
                let oldest = group
                    .into_iter()
                    .min_by_key(|(_, e)| e.used)
                    .map(|(k, _)| k.clone())
                    .ok_or("text cache budget")?;
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
                    (
                        Draw {
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
                        },
                        tile.resource.clone(),
                    )
                })
                .collect(),
        ))
    }
    pub fn cached_runs(&self) -> usize {
        self.entries.len()
    }
    pub fn cached_bytes(&self) -> usize {
        self.bytes
    }
}

#[cfg(all(test, windows))]
mod hud_budget_tests {
    use super::*;
    use crate::{bank::Bank, frame::Frame, hud, render};
    use slither_client::presentation::{HudOpacity, LeaderboardRow};

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
            ([3000, 1500], 5., 37),
            ([8192, 8192], 16., 37),
            ([3000, 1500], 16., 6),
            ([3000, 1500], 5., 37),
        ] {
            let plan = hud::plan(&view, extent, dpr).unwrap();
            let expected: Vec<_> = plan.lines.iter().flat_map(|line| &line.spans).collect();
            assert_eq!(expected.len(), count);
            let mut previous = None;
            for _ in 0..2 {
                let out = render::finish_hud(
                    render::PreparedFrame {
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
                    assert_eq!(
                        f64::from_bits(entry.content.size),
                        span.style.size * dpr.min(MAX_HUD_RASTER_SCALE)
                    );
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
