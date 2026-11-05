//! Validation and the single f64 -> f32 submission boundary; no scene or clock policy.
use crate::Result;
use slither_client::presentation::{
    BodyAsset, BodyAssetStatus, BodyBlend, BodyImageCommand, BodyPaintPass, BodyRect,
    FoodImageCommand, FoodImageDraw,
};
use std::collections::BTreeMap;

/// Per-picture CPU draw records, independent of GPU buffer size. See docs/draw-chunks.md.
pub const DRAW_RECORD_BUDGET: usize = 64 * 1024 * 1024;
pub(crate) const DRAW_ALLOCATION_FAILED: &str = "frame draw allocation failed";

pub const MAX_DRAWS: usize = DRAW_RECORD_BUDGET / std::mem::size_of::<Draw>();
pub const DRAW_MEMORY_LIMIT: &str = "frame draw-record memory budget exceeded; picture skipped";
pub const VERTEX_CHUNK_QUADS: usize = 8192;
pub const QUAD_VERTEX_BYTES: usize = 4 * 7 * 4;
pub const MAX_EXTENT: u32 = slither_client::world_resolution::MAX_EXTENT;

pub fn check_draw_count(count: usize) -> Result<()> {
    if count > MAX_DRAWS {
        Err(DRAW_MEMORY_LIMIT.into())
    } else {
        Ok(())
    }
}

/// Admit before Vec growth, capping geometric spare capacity at the record bound.
pub(crate) fn reserve_draws(draws: &mut Vec<Draw>, additional: usize) -> Result<()> {
    let required = draws
        .len()
        .checked_add(additional)
        .ok_or(DRAW_MEMORY_LIMIT)?;
    check_draw_count(required)?;
    if required > draws.capacity() {
        let capacity = required
            .max(draws.capacity().saturating_mul(2))
            .min(MAX_DRAWS);
        draws
            .try_reserve_exact(capacity - draws.len())
            .map_err(|_| DRAW_ALLOCATION_FAILED)?;
    }
    Ok(())
}

pub(crate) fn push_draw(draws: &mut Vec<Draw>, draw: Draw) -> Result<()> {
    reserve_draws(draws, 1)?;
    draws.push(draw);
    Ok(())
}

/// Recoverable picture failures, separate from session failure and bot observations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DrawDiagnostics {
    pub skipped_frames: u64,
    pub last_skipped_identity: Option<u64>,
    pub strict_validation: bool,
    #[cfg(not(target_arch = "wasm32"))]
    last_log: Option<std::time::Instant>,
}
impl Default for DrawDiagnostics {
    fn default() -> Self {
        Self {
            skipped_frames: 0,
            last_skipped_identity: None,
            strict_validation: cfg!(any(debug_assertions, test, target_arch = "wasm32")),
            #[cfg(not(target_arch = "wasm32"))]
            last_log: None,
        }
    }
}
impl DrawDiagnostics {
    pub fn admit<T>(&mut self, identity: u64, result: Result<T>) -> Result<Option<T>> {
        match result {
            // Native release skips bad pictures; debug/tests expose validation bugs.
            Err(error)
                if !self.strict_validation
                    || (cfg!(target_arch = "wasm32") && error == DRAW_MEMORY_LIMIT) =>
            {
                self.skipped_frames = self.skipped_frames.saturating_add(1);
                self.last_skipped_identity = Some(identity);
                #[cfg(not(target_arch = "wasm32"))]
                if self.should_log(std::time::Instant::now()) {
                    eprintln!("Frame skipped: {error}");
                }
                Ok(None)
            }
            other => other.map(Some),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn should_log(&mut self, now: std::time::Instant) -> bool {
        if self
            .last_log
            .is_some_and(|last| now.duration_since(last) < std::time::Duration::from_secs(5))
        {
            return false;
        }
        self.last_log = Some(now);
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceKey {
    pub handle: u64,
    pub revision: u64,
}
impl From<BodyAsset> for ResourceKey {
    fn from(asset: BodyAsset) -> Self {
        Self {
            handle: asset.handle,
            revision: asset.revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    /// Optional quad corners for arena tessellation and mirrored menu slices.
    pub corners: Option<[[f64; 2]; 4]>,
    pub asset: BodyAsset,
    pub source: Option<BodyRect>,
    /// Unbounded normalized UV rectangle for a repeating image; crops remain bounded.
    pub repeat_uv: Option<BodyRect>,
    /// Eligible for Canvas-like black-effect rounding; colored sources keep normal blending.
    pub black_effect: bool,
    pub destination: BodyRect,
    /// Translation in logical viewport pixels, followed by local rotation.
    pub translation: [f64; 2],
    pub rotation: f64,
    pub effective_alpha: f64,
    pub blend: BodyBlend,
}
impl Draw {
    /// Overlay tiles are already rasterized in physical pixels. Round the
    /// combined origin (including baseline/centering), never each term, so a
    /// linear sampler reads texel centres without a second reconstruction.
    pub(crate) fn snap_overlay(&mut self, scale: f64) {
        self.destination.x = ((self.destination.x + self.translation[0]) * scale).round() / scale;
        self.destination.y = ((self.destination.y + self.translation[1]) * scale).round() / scale;
        self.translation = [0., 0.];
    }

    /// Diagnostic geometry overlap only; transparent source pixels are not inspected.
    pub(crate) fn image_intersects_viewport(&self, extent: [u32; 2], scale: f64) -> bool {
        let r = self.destination;
        if self.corners.is_some()
            || self.effective_alpha <= 0.0
            || r.width == 0.0
            || r.height == 0.0
        {
            return false;
        }
        let (sin, cos) = self.rotation.sin_cos();
        let points = [
            [r.x, r.y],
            [r.x + r.width, r.y],
            [r.x + r.width, r.y + r.height],
            [r.x, r.y + r.height],
        ]
        .map(|[x, y]| {
            [
                self.translation[0] + cos * x - sin * y,
                self.translation[1] + sin * x + cos * y,
            ]
        });
        let width = f64::from(extent[0]) / scale;
        let height = f64::from(extent[1]) / scale;
        let viewport = [[0.0, 0.0], [width, 0.0], [width, height], [0.0, height]];
        // Separating axes for a rotated image rectangle and the viewport, with positive area.
        [[1.0, 0.0], [0.0, 1.0], [cos, sin], [-sin, cos]]
            .into_iter()
            .all(|[x, y]| {
                let span = |points: [[f64; 2]; 4]| {
                    points
                        .map(|p| p[0] * x + p[1] * y)
                        .into_iter()
                        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                            (lo.min(v), hi.max(v))
                        })
                };
                let (lo, hi) = span(points);
                let (view_lo, view_hi) = span(viewport);
                hi > view_lo && view_hi > lo
            })
    }
}
impl From<&BodyImageCommand> for Draw {
    fn from(c: &BodyImageCommand) -> Self {
        Self {
            corners: None,
            asset: c.asset,
            source: c.source_rect,
            repeat_uv: None,
            black_effect: c.blend == BodyBlend::SourceOver
                && matches!(
                    c.pass,
                    BodyPaintPass::Outline
                        | BodyPaintPass::NearHeadShadow
                        | BodyPaintPass::TailOutline
                        | BodyPaintPass::TailShadow
                        | BodyPaintPass::BodyShadow
                ),
            destination: c.destination,
            translation: [c.center.x + c.translation.x, c.center.y + c.translation.y],
            rotation: c.rotation.unwrap_or(0.),
            effective_alpha: c.alpha.effective,
            blend: c.blend,
        }
    }
}
impl From<&FoodImageCommand> for Draw {
    fn from(c: &FoodImageCommand) -> Self {
        let (source, destination) = match c.draw {
            FoodImageDraw::Whole { x, y } => (
                None,
                BodyRect {
                    x,
                    y,
                    width: f64::from(c.source.asset().width),
                    height: f64::from(c.source.asset().height),
                },
            ),
            FoodImageDraw::Scaled {
                source,
                destination,
            } => (Some(source), destination),
        };
        Self {
            corners: None,
            asset: c.source.asset(),
            source,
            repeat_uv: None,
            black_effect: false,
            destination,
            translation: [0.; 2],
            rotation: 0.,
            effective_alpha: c.alpha.effective,
            blend: c.blend,
        }
    }
}

/// Observations from this frame's existing culling and actual normal-body commands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SnakeDrawObservation {
    pub id: u16,
    pub own: bool,
    pub body_commands: usize,
    pub viewport_body_commands: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub identity: u64,
    pub extent: [u32; 2],
    pub viewport_scale: f64,
    pub tick: u32,
    pub draws: Vec<Draw>,
    pub food_batch: Option<crate::pellet_batch::Batch>,
    /// Diagnostic only: does not participate in GPU submission or scene policy.
    pub snakes: Vec<SnakeDrawObservation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    pub key: ResourceKey,
    pub repeat: bool,
    pub black_effect: bool,
    pub blend: BodyBlend,
    /// Six vertices: clip X/Y, UV X/Y, effective alpha. Order is never regrouped.
    pub vertices: [[f32; 5]; 6],
}

fn rect_finite(r: BodyRect) -> bool {
    [r.x, r.y, r.width, r.height]
        .into_iter()
        .all(f64::is_finite)
}

fn same_rect_bits(a: BodyRect, b: BodyRect) -> bool {
    [a.x, a.y, a.width, a.height].map(f64::to_bits)
        == [b.x, b.y, b.width, b.height].map(f64::to_bits)
}

fn same_projection(a: &Draw, b: &Draw) -> bool {
    let same_optional_rect = |a, b| match (a, b) {
        (Some(a), Some(b)) => same_rect_bits(a, b),
        (None, None) => true,
        _ => false,
    };
    same_rect_bits(a.destination, b.destination)
        && a.asset == b.asset
        && same_optional_rect(a.source, b.source)
        && same_optional_rect(a.repeat_uv, b.repeat_uv)
        && a.rotation.to_bits() == b.rotation.to_bits()
        && a.translation.map(f64::to_bits) == b.translation.map(f64::to_bits)
        && a.corners.map(|p| p.map(|v| v.map(f64::to_bits)))
            == b.corners.map(|p| p.map(|v| v.map(f64::to_bits)))
}

fn checked_source(d: &Draw, resident: &BTreeMap<ResourceKey, BodyAsset>) -> Result<BodyRect> {
    let key = ResourceKey::from(d.asset);
    if d.asset.status != BodyAssetStatus::Ready
        || d.asset.width == 0
        || d.asset.height == 0
        || resident.get(&key) != Some(&d.asset)
    {
        return Err("missing, stale or unready resource".into());
    }
    let source = d.source.unwrap_or(BodyRect {
        x: 0.,
        y: 0.,
        width: f64::from(d.asset.width),
        height: f64::from(d.asset.height),
    });
    if !rect_finite(source)
        || d.repeat_uv.is_some_and(|uv| {
            !rect_finite(uv) || uv.width <= 0. || uv.height <= 0. || d.source.is_some()
        })
        || !rect_finite(d.destination)
        || !d.rotation.is_finite()
        || !d.translation.into_iter().all(f64::is_finite)
        || !(0. ..=1.).contains(&d.effective_alpha)
        || source.width <= 0.
        || source.height <= 0.
        || source.x < 0.
        || source.y < 0.
        || source.x + source.width > f64::from(d.asset.width)
        || source.y + source.height > f64::from(d.asset.height)
        || d.destination.width < 0.
        || d.destination.height < 0.
    {
        return Err("unsupported or invalid image rectangle/alpha/transform".into());
    }
    Ok(source)
}

impl Frame {
    pub(crate) fn project_checked(
        &self,
        draw: &Draw,
        resident: &BTreeMap<ResourceKey, BodyAsset>,
    ) -> Result<Quad> {
        self.project(draw, checked_source(draw, resident)?)
    }
    pub fn draw_count(&self) -> usize {
        self.draws.len() + self.food_batch.as_ref().map_or(0, |b| b.draw_count())
    }
    /// Expand retained inputs into the unchanged ordered image stream for parity.
    pub fn canonical(&self) -> Result<Self> {
        if let Some(batch) = &self.food_batch {
            self.check_limits()?;
            return Ok(Self {
                identity: self.identity,
                extent: self.extent,
                viewport_scale: self.viewport_scale,
                tick: self.tick,
                draws: batch.canonical(&self.draws)?,
                food_batch: None,
                snakes: self.snakes.clone(),
            });
        }
        Ok(self.clone())
    }
    pub fn check_limits(&self) -> Result<()> {
        check_draw_count(self.draw_count())?;
        if self
            .food_batch
            .as_ref()
            .is_some_and(|b| b.early_at > b.late_at || b.late_at > self.draws.len())
        {
            return Err("invalid retained food barriers".into());
        }
        if self.extent.iter().any(|n| *n > MAX_EXTENT)
            || !self.viewport_scale.is_finite()
            || self.viewport_scale <= 0.
        {
            return Err("invalid frame extent/scale/budget".into());
        }
        Ok(())
    }
    /// Diagnostic collecting API. Production converts directly into vertex chunks.
    pub fn quads(&self, resident: &BTreeMap<ResourceKey, BodyAsset>) -> Result<Vec<Quad>> {
        self.quad_iter(resident)?.collect()
    }
    pub fn validate(&self, resident: &BTreeMap<ResourceKey, BodyAsset>) -> Result<()> {
        self.check_limits()?;
        if self.extent.contains(&0) {
            return Ok(());
        }
        if let Some(batch) = &self.food_batch
            && (batch.validate(resident).is_err() || self.viewport_scale > 1e10)
        {
            return self.canonical()?.validate(resident);
        }
        // Validation needs an overflow proof, not discarded GPU vertices. At
        // this conservative bound, rectangle corners are <=2L, transformed
        // positions <=5L, and clip coordinates <=10L*scale+1 < f32::MAX.
        // Cropped UVs are already in [0,1]; repeated UV sums have their own
        // bound. Unusual magnitudes keep the exact projection/error path.
        // Study: research/client-performance/2026-11-02-art-validation/REPORT.md.
        let limit = f64::from(f32::MAX) / 32. / self.viewport_scale.max(1.);
        for d in &self.draws {
            let source = checked_source(d, resident)?;
            let r = d.destination;
            let bounded = [
                r.x,
                r.y,
                r.width,
                r.height,
                d.translation[0],
                d.translation[1],
            ]
            .into_iter()
            .all(|v| v.abs() <= limit)
                && d.corners
                    .is_none_or(|p| p.into_iter().flatten().all(|v| v.abs() <= limit))
                && d.repeat_uv.is_none_or(|uv| {
                    [uv.x, uv.y, uv.width, uv.height]
                        .into_iter()
                        .all(|v| v.abs() <= f64::from(f32::MAX) / 4.)
                });
            if !bounded {
                self.project(d, source)?;
            }
        }
        Ok(())
    }
    // Food supplies a constant axis-aligned transform. Specialize it without
    // changing the checked f64 arithmetic or the f32 submission boundary.
    // See research/client-performance/2026-11-02-retained-pellets/REPORT.md.
    #[inline(always)]
    pub(crate) fn project(&self, d: &Draw, source: BodyRect) -> Result<Quad> {
        let key = ResourceKey::from(d.asset);
        let r = d.destination;
        let (sin, cos) = d.rotation.sin_cos();
        let mut corners = [[0.; 5]; 4];
        for (i, [u, v]) in [[0., 0.], [1., 0.], [1., 1.], [0., 1.]]
            .into_iter()
            .enumerate()
        {
            let [x, y] = d
                .corners
                .map_or([r.x + u * r.width, r.y + v * r.height], |p| p[i]);
            if !x.is_finite() || !y.is_finite() {
                return Err("nonfinite polygon corner".into());
            }
            let local_x = x;
            let x = d.translation[0] + cos * x - sin * y;
            let y = d.translation[1] + sin * local_x + cos * y;
            let [uv_x, uv_y] = d.repeat_uv.map_or_else(
                || {
                    [
                        (source.x + u * source.width) / f64::from(d.asset.width),
                        (source.y + v * source.height) / f64::from(d.asset.height),
                    ]
                },
                |uv| [uv.x + u * uv.width, uv.y + v * uv.height],
            );
            let values = [
                2. * (x * self.viewport_scale) / f64::from(self.extent[0]) - 1.,
                1. - 2. * (y * self.viewport_scale) / f64::from(self.extent[1]),
                uv_x,
                uv_y,
                d.effective_alpha,
            ];
            if values
                .iter()
                .any(|v| !v.is_finite() || v.abs() > f64::from(f32::MAX))
            {
                return Err("GPU conversion overflow".into());
            }
            corners[i] = values.map(|v| v as f32);
        }
        Ok(Quad {
            key,
            repeat: d.repeat_uv.is_some(),
            black_effect: d.black_effect,
            blend: d.blend,
            vertices: [
                corners[0], corners[1], corners[2], corners[0], corners[2], corners[3],
            ],
        })
    }

    /// Validate and convert each draw once, before the renderer submits any commands.
    pub fn quad_iter<'a>(
        &'a self,
        resident: &'a BTreeMap<ResourceKey, BodyAsset>,
    ) -> Result<impl Iterator<Item = Result<Quad>> + 'a> {
        self.check_limits()?;
        let streamed = self
            .food_batch
            .as_ref()
            .filter(|batch| batch.canonical_submission && !self.extent.contains(&0))
            .map(|batch| batch.quads(self, resident))
            .transpose()?;
        // A frame and its resident catalog are immutable for this iterator.
        // Paired pellet draws differ only in alpha; retain the last validated
        // projection rather than repeating resource checks and f64 conversion.
        // Bit comparisons preserve signed-zero distinctions in the input math.
        // Measured scope: research/client-performance/2026-11-02-quad-projection/REPORT.md.
        let draws = if streamed.is_some() {
            std::borrow::Cow::Borrowed(&[] as &[Draw])
        } else if let Some(batch) = &self.food_batch {
            std::borrow::Cow::Owned(batch.canonical(&self.draws)?)
        } else {
            std::borrow::Cow::Borrowed(self.draws.as_slice())
        };
        let mut previous: Option<(usize, Quad)> = None;
        let count = if self.extent.contains(&0) {
            0
        } else {
            draws.len()
        };
        Ok(streamed
            .into_iter()
            .flatten()
            .chain((0..count).map(move |index| {
                let d = &draws[index];
                if let Some((last, projected)) = &previous
                    && same_projection(&draws[*last], d)
                {
                    if !(0. ..=1.).contains(&d.effective_alpha) {
                        return Err("unsupported or invalid image rectangle/alpha/transform".into());
                    }
                    let mut next = projected.clone();
                    for vertex in &mut next.vertices {
                        vertex[4] = d.effective_alpha as f32;
                    }
                    next.black_effect = d.black_effect;
                    next.blend = d.blend;
                    return Ok(next);
                }
                let source = checked_source(d, resident)?;
                let projected = self.project(d, source)?;
                previous = Some((index, projected.clone()));
                Ok(projected)
            })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_projection_matches_fresh_conversion_bit_for_bit() {
        // ORIGINAL SYNTHETIC rectangles; singleton iterators always take the uncached path.
        let asset = BodyAsset {
            handle: 1,
            revision: 1,
            width: 64,
            height: 32,
            status: BodyAssetStatus::Ready,
        };
        let draw = Draw {
            corners: None,
            asset,
            source: None,
            repeat_uv: None,
            black_effect: false,
            destination: BodyRect {
                x: -7.,
                y: 9.,
                width: 40.,
                height: 24.,
            },
            translation: [3., -5.],
            rotation: 0.3,
            effective_alpha: 0.4,
            blend: BodyBlend::SourceOver,
        };
        let resident = BTreeMap::from([(asset.into(), asset)]);
        let mut positive_zero = draw.clone();
        positive_zero.rotation = 0.;
        let mut negative_zero = positive_zero.clone();
        negative_zero.rotation = -0.;
        assert!(!same_projection(&positive_zero, &negative_zero));
        let frame = Frame {
            identity: 1,
            extent: [960, 540],
            viewport_scale: 1.25,
            tick: 0,
            draws: vec![],
            snakes: vec![],
            food_batch: None,
        };
        let check = |draws: Vec<Draw>, extent, viewport_scale, resident: &BTreeMap<_, _>| {
            let f = Frame {
                draws,
                extent,
                viewport_scale,
                ..frame.clone()
            };
            let cached = f.quad_iter(resident).unwrap();
            for (d, actual) in f.draws.iter().zip(cached) {
                let singleton = Frame {
                    draws: vec![d.clone()],
                    ..f.clone()
                };
                let expected = singleton.quad_iter(resident).unwrap().next().unwrap();
                match (actual, expected) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(
                            (a.key, a.repeat, a.black_effect, a.blend),
                            (b.key, b.repeat, b.black_effect, b.blend)
                        );
                        assert_eq!(
                            a.vertices.map(|v| v.map(f32::to_bits)),
                            b.vertices.map(|v| v.map(f32::to_bits))
                        );
                    }
                    (Err(a), Err(b)) => assert_eq!(a, b),
                    other => panic!("cached/fresh mismatch: {other:?}"),
                }
            }
        };
        let mut geometries = vec![draw.clone()];
        let mut cropped = draw.clone();
        cropped.source = Some(BodyRect {
            x: 2.,
            y: 3.,
            width: 12.,
            height: 8.,
        });
        geometries.push(cropped);
        let mut repeated = draw.clone();
        repeated.repeat_uv = Some(BodyRect {
            x: -0.2,
            y: 1.4,
            width: 3.,
            height: 2.,
        });
        geometries.push(repeated);
        let mut polygon = draw.clone();
        polygon.corners = Some([[-0., 1.], [20., 2.], [17., 30.], [-3., 19.]]);
        geometries.push(polygon);
        for geometry in geometries {
            let mut draws = vec![geometry.clone()];
            for alpha in [0., -0., 1., 0.125, f64::NAN, f64::INFINITY, -0.1, 1.1, 0.5] {
                let mut d = geometry.clone();
                d.effective_alpha = alpha;
                d.black_effect = true;
                d.blend = BodyBlend::Lighter;
                draws.push(d);
            }
            check(draws, frame.extent, frame.viewport_scale, &resident);
        }
        let mut packed = Vec::new();
        for i in 0..512 {
            let mut d = draw.clone();
            d.destination.x = f64::from(i % 32) * 0.25;
            d.destination.y = f64::from(i / 32) * 0.25;
            d.blend = BodyBlend::Lighter;
            packed.push(d.clone());
            d.effective_alpha = f64::from(i % 17) / 16.;
            packed.push(d);
        }
        check(packed, frame.extent, frame.viewport_scale, &resident);
        let mutations: &[fn(&mut Draw)] = &[
            |d| d.destination.x = -0.,
            |d| d.destination.y += 1.,
            |d| d.destination.width += 1.,
            |d| d.destination.height = 0.,
            |d| d.translation[0] = -0.,
            |d| d.translation[1] += 1.,
            |d| d.rotation = -0.,
            |d| d.asset.revision += 1,
            |d| d.asset.handle += 1,
            |d| d.asset.width += 1,
            |d| d.asset.height = 0,
            |d| d.asset.status = BodyAssetStatus::Pending,
            |d| {
                d.source = Some(BodyRect {
                    x: 64.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                })
            },
            |d| {
                d.repeat_uv = Some(BodyRect {
                    x: 0.,
                    y: 0.,
                    width: -1.,
                    height: 1.,
                })
            },
            |d| d.destination.width = -1.,
            |d| d.destination.x = f64::NAN,
            |d| d.translation[0] = f64::INFINITY,
            |d| d.rotation = f64::NAN,
            |d| d.destination.x = f64::MAX,
            |d| d.corners = Some([[f64::NAN; 2]; 4]),
        ];
        for mutate in mutations {
            let mut changed = draw.clone();
            mutate(&mut changed);
            check(
                vec![draw.clone(), changed, draw.clone()],
                frame.extent,
                1.25,
                &resident,
            );
        }
        for (extent, scale) in [([320, 240], 2.), ([1920, 1080], 0.5)] {
            check(vec![draw.clone(), draw.clone()], extent, scale, &resident);
        }
        let missing = BTreeMap::new();
        check(
            vec![draw.clone(), draw.clone()],
            frame.extent,
            1.25,
            &missing,
        );
        let empty = Frame {
            extent: [0, 0],
            draws: vec![draw],
            ..frame
        };
        assert_eq!(empty.quad_iter(&resident).unwrap().count(), 0);
    }

    #[test]
    fn validation_matches_projection_at_float_and_resource_boundaries() {
        // ORIGINAL SYNTHETIC inputs. GPU conversion remains the exact fallback oracle.
        let asset = BodyAsset {
            handle: 1,
            revision: 2,
            width: 64,
            height: 32,
            status: BodyAssetStatus::Ready,
        };
        let draw = Draw {
            corners: None,
            asset,
            source: None,
            repeat_uv: None,
            black_effect: false,
            destination: BodyRect {
                x: 1.,
                y: -2.,
                width: 24.,
                height: 16.,
            },
            translation: [3., -4.],
            rotation: 0.7,
            effective_alpha: 0.5,
            blend: BodyBlend::Lighter,
        };
        let resident = BTreeMap::from([(asset.into(), asset)]);
        let frame = Frame {
            identity: 1,
            extent: [960, 540],
            viewport_scale: 1.25,
            tick: 0,
            draws: vec![draw.clone()],
            snakes: vec![],
            food_batch: None,
        };
        let check = |f: &Frame, catalog: &BTreeMap<_, _>| {
            let projected = f
                .quad_iter(catalog)
                .and_then(|mut iter| iter.try_for_each(|q| q.map(|_| ())));
            assert_eq!(f.validate(catalog), projected, "frame={f:?}");
        };
        for scale in [
            f64::from_bits(1),
            f64::MIN_POSITIVE,
            0.5,
            1.,
            4.,
            1e20,
            f64::MAX,
        ] {
            let bound = f64::from(f32::MAX) / 32. / scale.max(1.);
            let mut values = vec![
                0.,
                -0.,
                bound.next_down(),
                bound,
                bound.next_up(),
                f64::from(f32::MAX),
                f64::MAX,
                f64::INFINITY,
                f64::NAN,
            ];
            values.extend((-1074..=1023).step_by(17).map(|e| 2_f64.powi(e)));
            for value in values {
                for sign in [1., -1.] {
                    let v = value * sign;
                    for field in 0..8 {
                        let mut f = frame.clone();
                        f.viewport_scale = scale;
                        let d = &mut f.draws[0];
                        match field {
                            0 => d.destination.x = v,
                            1 => d.destination.width = v,
                            2 => d.translation[1] = v,
                            3 => d.rotation = v,
                            4 => d.corners = Some([[v, v]; 4]),
                            5 => {
                                d.repeat_uv = Some(BodyRect {
                                    x: v,
                                    y: 0.,
                                    width: 1.,
                                    height: 1.,
                                })
                            }
                            6 => {
                                d.repeat_uv = Some(BodyRect {
                                    x: 0.,
                                    y: 0.,
                                    width: v,
                                    height: v,
                                })
                            }
                            _ => d.effective_alpha = v,
                        }
                        check(&f, &resident);
                    }
                }
            }
        }
        for mutate in [
            (|d: &mut Draw| d.asset.revision += 1) as fn(&mut Draw),
            |d| d.asset.status = BodyAssetStatus::Pending,
            |d| d.asset.height = 0,
            |d| {
                d.source = Some(BodyRect {
                    x: 64.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                })
            },
            |d| {
                d.source = Some(BodyRect {
                    x: 0.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                });
                d.repeat_uv = Some(BodyRect {
                    x: 0.,
                    y: 0.,
                    width: 1.,
                    height: 1.,
                });
            },
        ] {
            let mut f = frame.clone();
            mutate(&mut f.draws[0]);
            check(&f, &resident);
            assert!(f.validate(&resident).is_err());
        }
        check(&frame, &BTreeMap::new());
        for (extent, scale) in [
            ([0, 540], 1.),
            ([MAX_EXTENT + 1, 540], 1.),
            ([960, 540], 0.),
            ([960, 540], f64::NAN),
        ] {
            let f = Frame {
                extent,
                viewport_scale: scale,
                ..frame.clone()
            };
            check(&f, &resident);
        }
        let mut packed = frame.clone();
        packed.draws = (0..25_000)
            .map(|i| {
                let mut d = draw.clone();
                d.destination.x = f64::from(i % 200) * 0.25;
                d.destination.y = f64::from(i / 200) * 0.25;
                d.effective_alpha = f64::from(i % 17) / 16.;
                d
            })
            .collect();
        check(&packed, &resident);
        // The first failure is retained even when a later draw has invalid metadata.
        packed.draws[0].corners = Some([[f64::NAN; 2]; 4]);
        packed.draws[1].asset.status = BodyAssetStatus::Pending;
        assert_eq!(
            packed.validate(&resident),
            Err("nonfinite polygon corner".into())
        );
        check(&packed, &resident);
    }
    #[test]
    fn strict_validation_returns_errors_and_release_skips_and_recovers() {
        assert!(DrawDiagnostics::default().strict_validation);
        for strict in [true, false] {
            let mut diagnostics = DrawDiagnostics {
                strict_validation: strict,
                ..Default::default()
            };
            for error in [
                "missing, stale or unready resource",
                "Active GPU resource budget",
                DRAW_MEMORY_LIMIT,
            ] {
                let result = diagnostics.admit::<()>(7, Err(error.into()));
                if strict {
                    assert_eq!(result, Err(error.into()));
                    assert_eq!(diagnostics.skipped_frames, 0);
                } else {
                    assert_eq!(result, Ok(None));
                    assert_eq!(diagnostics.last_skipped_identity, Some(7));
                }
            }
            assert_eq!(diagnostics.admit(8, Ok(42)), Ok(Some(42)));
            assert_eq!(diagnostics.skipped_frames, if strict { 0 } else { 3 });
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn frame_error_logging_is_bounded_even_when_errors_change() {
        let mut diagnostics = DrawDiagnostics::default();
        let now = std::time::Instant::now();
        assert!(diagnostics.should_log(now));
        assert!(!diagnostics.should_log(now + std::time::Duration::from_secs(4)));
        assert!(diagnostics.should_log(now + std::time::Duration::from_secs(5)));
    }
    #[test]
    fn viewport_overlap_uses_rotated_geometry_positive_alpha_and_backing_scale() {
        // ORIGINAL SYNTHETIC image rectangles; no source image or GPU.
        let mut d = Draw {
            corners: None,
            repeat_uv: None,
            black_effect: false,
            asset: BodyAsset {
                handle: 1,
                revision: 1,
                width: 100,
                height: 2,
                status: BodyAssetStatus::Ready,
            },
            source: None,
            destination: BodyRect {
                x: 0.,
                y: 0.,
                width: 100.,
                height: 2.,
            },
            translation: [-20., 45.],
            rotation: -std::f64::consts::FRAC_PI_4,
            effective_alpha: 1.,
            blend: BodyBlend::SourceOver,
        };
        // Its axis-aligned bounds overlap, but the diagonal misses the viewport corner.
        assert!(!d.image_intersects_viewport([10, 10], 1.));
        d.translation[1] = 39.;
        assert!(d.image_intersects_viewport([10, 10], 1.));
        assert!(d.image_intersects_viewport([20, 20], 2.));
        d.effective_alpha = 0.;
        assert!(!d.image_intersects_viewport([10, 10], 1.));
        d.effective_alpha = 1.;
        d.rotation = 0.;
        d.translation = [10., 0.];
        assert!(!d.image_intersects_viewport([10, 10], 1.));
    }
}
