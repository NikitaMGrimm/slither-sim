//! Authored round-pen wordmarks. A swelling, displaced chain of circles draws
//! each stroke; the retained reveal clock also damps the ripple to exact rest.
use crate::{Result, materials::RgbaImage};
use slither_client::menu_layout::MenuRect;
use std::collections::btree_map::Entry;

#[path = "wordmark_glyphs.rs"]
mod alphabet;
pub use alphabet::glyph;

pub const DEFAULT_TEXT: &str = "SlitherClient";
/// Top/bottom endpoints, alternating at capitals and spaces. Edit colours here.
pub const COLORS: [[[u8; 3]; 2]; 2] = [
    [[126, 255, 157], [18, 147, 88]],
    [[183, 112, 255], [89, 38, 162]],
];
pub const MAX_CHARACTERS: usize = 128;
// Temporary full canvas; retained nonempty tiles share the 13 MiB Menu cap.
pub const MAX_RASTER_BYTES: usize = 10 * 1024 * 1024;
pub(crate) const RASTER_SCALE_QUOTA: &str = "Wordmark raster scale bounds";
pub(crate) const RASTER_PIXEL_QUOTA: &str = "Wordmark raster byte budget";
const PEN: f64 = 0.86;
const TRACKING: f64 = 1.25;

#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    pub text: String,
    pub x: f64,
    pub width: f64,
    pub color: usize,
    pub fallback: bool,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub rect: MenuRect,
    pub unit: f64,
    pub words: Vec<Word>,
}
impl Layout {
    /// Keep extreme physical sizes within the transient raster bound without
    /// switching supported glyphs to a different font or scaling texture pixels.
    /// Normal DPR/layouts remain byte-for-byte unchanged.
    pub fn fit_raster(&mut self, dpr: f64) -> Result<()> {
        if !dpr.is_finite() || dpr <= 0. {
            return Err(RASTER_SCALE_QUOTA.into());
        }
        if raster_size(self, dpr).is_ok() {
            return Ok(());
        }
        let width = self.rect.width * dpr;
        let height = self.rect.height * dpr;
        let fit = (2047. / height)
            .min((MAX_RASTER_BYTES as f64 / (4. * (width + 1.) * (height + 1.))).sqrt())
            .min(1.)
            * 0.99;
        let center = [
            self.rect.x + self.rect.width / 2.,
            self.rect.y + self.rect.height / 2.,
        ];
        self.unit *= fit;
        self.rect.width *= fit;
        self.rect.height *= fit;
        self.rect.x = center[0] - self.rect.width / 2.;
        self.rect.y = center[1] - self.rect.height / 2.;
        raster_size(self, dpr).map(|_| ())
    }
    /// Snapped raster origin; atlas pieces use integer offsets from this grid.
    pub fn origin_pixels(&self, dpr: f64) -> [f64; 2] {
        [self.rect.x * dpr, self.rect.y * dpr].map(f64::round)
    }
    /// The raster depends on this local offset, even when a resize leaves its
    /// snapped canvas origin unchanged. Whole-pixel translations need no new pixels.
    pub(crate) fn word_offset_pixels(&self, word: &Word, dpr: f64) -> f64 {
        self.word_x_pixels(word, dpr) - self.origin_pixels(dpr)[0]
    }
    /// Snap the line's first word once, then use rounded relative advances.
    pub fn word_x_pixels(&self, word: &Word, dpr: f64) -> f64 {
        let first = self.words.first().map_or(0., |w| w.x);
        let origin = ((self.rect.x + first * self.unit) * dpr).round();
        origin + ((word.x - first) * self.unit * dpr).round()
    }
}

/// Unsupported scalars select installed-font rendering for their entire word.
/// The callback measures that word at a ten-unit cap height, in alphabet units.
pub fn layout(
    value: &str,
    holder: MenuRect,
    viewport_width: f64,
    fallback_width: impl FnMut(&str) -> Result<f64>,
) -> Result<Layout> {
    layout_impl(
        value,
        holder,
        viewport_width,
        false,
        usize::MAX,
        fallback_width,
    )
}
pub(crate) fn fallback_layout(
    value: &str,
    holder: MenuRect,
    viewport_width: f64,
    max_runs: usize,
    fallback_width: impl FnMut(&str) -> Result<f64>,
) -> Result<Layout> {
    // The legacy plain-font logo fits the holder, unlike the wider stroke line.
    layout_impl(
        value,
        holder,
        viewport_width.min(holder.width + 32.),
        true,
        max_runs,
        fallback_width,
    )
}
struct FontRun {
    text: String,
    color: usize,
    spaces: usize,
}
fn bounded_font_runs(raw: Vec<FontRun>, limit: usize) -> (Vec<FontRun>, bool) {
    let mut runs: Vec<FontRun> = Vec::new();
    for run in raw {
        if let Some(last) = runs.last_mut().filter(|last| last.color == run.color) {
            last.text.push_str(&" ".repeat(run.spaces));
            last.text.push_str(&run.text);
        } else if runs.len() < limit {
            runs.push(run);
        } else {
            if let Some(last) = runs.last_mut() {
                last.text.push_str("...");
            }
            return (runs, true);
        }
    }
    (runs, false)
}
fn layout_impl(
    value: &str,
    holder: MenuRect,
    viewport_width: f64,
    font_only: bool,
    max_font_runs: usize,
    mut fallback_width: impl FnMut(&str) -> Result<f64>,
) -> Result<Layout> {
    if value.chars().count() > MAX_CHARACTERS
        || !viewport_width.is_finite()
        || viewport_width <= 0.
        || [holder.x, holder.y, holder.width, holder.height]
            .into_iter()
            .any(|n| !n.is_finite())
        || holder.height <= 0.
    {
        return Err("Wordmark layout bounds".into());
    }
    let mut chunks: Vec<String> = Vec::new();
    for c in value.chars() {
        if chunks.is_empty()
            || c == ' '
            || c.is_uppercase()
            || chunks.last().is_some_and(|s| s.ends_with(' '))
        {
            chunks.push(String::new());
        }
        chunks.last_mut().unwrap().push(c);
    }
    let mut raw = Vec::new();
    let mut spaces = 0;
    for text in chunks {
        if text == " " {
            spaces += 1;
            continue;
        }
        raw.push(FontRun {
            text,
            color: raw.len() % COLORS.len(),
            spaces,
        });
        spaces = 0;
    }
    if font_only {
        let (runs, truncated) = bounded_font_runs(raw, max_font_runs);
        raw = runs;
        if truncated {
            spaces = 0;
        }
    }
    let mut words = Vec::new();
    let mut advance = 0.;
    for run in raw {
        advance += run.spaces as f64 * 4.;
        let fallback = font_only || !run.text.chars().all(|c| glyph(c).is_some());
        let width = if fallback {
            fallback_width(&run.text)?
        } else {
            run.text
                .chars()
                .map(|c| glyph(c).unwrap().0 + TRACKING)
                .sum::<f64>()
                - TRACKING
        };
        if !width.is_finite() || width < 0. {
            return Err("Wordmark advance bounds".into());
        }
        words.push(Word {
            text: run.text,
            x: advance + 3.,
            width,
            color: run.color,
            fallback,
        });
        advance += width + TRACKING;
    }
    advance += spaces as f64 * 4.;
    let natural_width = (advance - TRACKING).max(0.) + 6.;
    let unit = (holder.height / 16.).min((viewport_width - 32.).max(1.) / natural_width);
    Ok(Layout {
        rect: MenuRect {
            x: holder.x + holder.width / 2. - natural_width * unit / 2.,
            y: holder.y + (holder.height - 16. * unit) / 2.,
            width: natural_width * unit,
            height: 16. * unit,
        },
        unit,
        words,
    })
}

type Point = [f64; 2];
// Shared analytic ellipse, flattened at five-degree intervals before the
// existing distance-spaced round pen. Matching endpoints keep one pen stroke.
fn ellipse_arc(paths: &mut Vec<Vec<Point>>, [x, y, w, h]: [f64; 4], start: f64, sweep: f64) {
    let steps = (sweep.abs() / 5.).ceil() as u32;
    let point = |t: f64| {
        let angle = (start + sweep * t).to_radians();
        [
            x + w / 2. * (1. + angle.cos()),
            y + h / 2. * (1. + angle.sin()),
        ]
    };
    let first = point(0.);
    if !paths
        .last()
        .and_then(|p| p.last())
        .is_some_and(|p| (p[0] - first[0]).hypot(p[1] - first[1]) < 1e-8)
    {
        paths.push(vec![first]);
    }
    let path = paths.last_mut().unwrap();
    for i in 1..=steps {
        path.push(point(f64::from(i) / f64::from(steps)));
    }
}
fn arch(paths: &mut Vec<Vec<Point>>, [x, y, w, h]: [f64; 4]) {
    ellipse_arc(paths, [x, y, w, 2. * h], 180., 180.);
}
fn s_curve(paths: &mut Vec<Vec<Point>>, [x, y, w, h]: [f64; 4]) {
    ellipse_arc(paths, [x, y, w, h / 2.], -30., -240.);
    ellipse_arc(paths, [x, y + h / 2., w, h / 2.], -90., 240.);
}
fn paths(code: &str) -> Vec<Vec<Point>> {
    let mut tokens = code.split_whitespace();
    let mut paths: Vec<Vec<Point>> = Vec::new();
    while let Some(command) = tokens.next() {
        let mut number = || tokens.next().unwrap().parse::<f64>().unwrap();
        match command {
            "M" => paths.push(vec![[number(), number()]]),
            "L" => paths.last_mut().unwrap().push([number(), number()]),
            "A" | "B" | "H" | "S" => {
                let bounds = [number(), number(), number(), number()];
                match command {
                    "A" => ellipse_arc(&mut paths, bounds, number(), number()),
                    "B" => ellipse_arc(&mut paths, bounds, 0., -360.),
                    "H" => arch(&mut paths, bounds),
                    "S" => s_curve(&mut paths, bounds),
                    _ => unreachable!(),
                }
            }
            "Q" => {
                let control = [number(), number()];
                let end = [number(), number()];
                let path = paths.last_mut().unwrap();
                let start = *path.last().unwrap();
                for i in 1..=20 {
                    let t = f64::from(i) / 20.;
                    path.push(std::array::from_fn(|k| {
                        (1. - t).powi(2) * start[k]
                            + 2. * (1. - t) * t * control[k]
                            + t * t * end[k]
                    }));
                }
            }
            _ => unreachable!("authored alphabet command"),
        }
    }
    paths
}

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub center: Point,
    pub radius: f64,
    pub word: usize,
}
/// Public for offline motion/bounds regression and specimen measurements.
pub fn samples(layout: &Layout, progress: f64) -> Result<Vec<Sample>> {
    if !progress.is_finite() {
        return Err("Wordmark reveal bounds".into());
    }
    let progress = progress.clamp(0., 1.);
    let mut strokes = Vec::new();
    for (word_index, word) in layout.words.iter().enumerate().filter(|(_, w)| !w.fallback) {
        let mut x = word.x;
        for c in word.text.chars() {
            let (advance, code) = glyph(c).unwrap();
            for mut path in paths(code) {
                for p in &mut path {
                    p[0] += x;
                    p[1] += 2.;
                }
                strokes.push((path, word_index));
            }
            x += advance + TRACKING;
        }
    }
    let count = strokes.len().max(1) as f64;
    let mut result = Vec::new();
    for (index, (path, word_index)) in strokes.into_iter().enumerate() {
        let lengths: Vec<_> = path
            .windows(2)
            .map(|p| (p[1][0] - p[0][0]).hypot(p[1][1] - p[0][1]))
            .collect();
        let total = lengths.iter().sum::<f64>();
        let mut traveled = 0.;
        for (edge, length) in path.windows(2).zip(lengths) {
            let steps = (length / 0.18).ceil().max(1.) as u32;
            for i in 0..=steps {
                let t = f64::from(i) / f64::from(steps);
                let along = if total > 0. {
                    (traveled + t * length) / total
                } else {
                    0.
                };
                // Authored timing: early strokes swell first, most geometry is
                // present by ~2.5 s, then the traveling ripple settles by ~6.7 s.
                let start = 0.13 * index as f64 / count + 0.085 * along;
                let growth = ((progress - start) / 0.095).clamp(0., 1.);
                if growth == 0. {
                    continue;
                }
                let radius = PEN * growth * growth * (3. - 2. * growth);
                let wave = 1.25
                    * (1. - progress).powi(3)
                    * (0.55 + 0.45 * along)
                    * (progress * 34. - along * 8. + index as f64 * 0.7).sin();
                let normal = if length > 0. {
                    [
                        -(edge[1][1] - edge[0][1]) / length,
                        (edge[1][0] - edge[0][0]) / length,
                    ]
                } else {
                    [1., 0.]
                };
                result.push(Sample {
                    center: std::array::from_fn(|k| {
                        (edge[0][k] + t * (edge[1][k] - edge[0][k]) + wave * normal[k])
                            * layout.unit
                    }),
                    radius: radius * layout.unit,
                    word: word_index,
                });
            }
            traveled += length;
        }
    }
    Ok(result)
}

/// Physical dimensions stay exact: wide rasters are tiled for WebGL2, never
/// downsampled or enlarged by the snapped overlay. Admission precedes allocation.
pub fn raster_size(layout: &Layout, dpr: f64) -> Result<[u32; 2]> {
    if !dpr.is_finite() || dpr <= 0. {
        return Err(RASTER_SCALE_QUOTA.into());
    }
    let size = [
        (layout.rect.width * dpr).ceil().max(1.) as u32,
        (layout.rect.height * dpr).ceil().max(1.) as u32,
    ];
    if size[1] > 2048 || u64::from(size[0]) * u64::from(size[1]) * 4 > MAX_RASTER_BYTES as u64 {
        return Err(RASTER_PIXEL_QUOTA.into());
    }
    Ok(size)
}

fn circle(
    mask: &mut [u8],
    owners: &mut [u8],
    word: u8,
    size: [u32; 2],
    center: Point,
    radius: f64,
) {
    let [width, height] = size;
    let [cx, cy] = center;
    let outer = radius + 0.5;
    for y in
        ((cy - outer).floor().max(0.) as u32)..((cy + outer).ceil().min(f64::from(height)) as u32)
    {
        let dy = (f64::from(y) + 0.5 - cy).abs();
        if dy >= outer {
            continue;
        }
        let half = (outer * outer - dy * dy).sqrt();
        for x in
            ((cx - half).floor().max(0.) as u32)..((cx + half).ceil().min(f64::from(width)) as u32)
        {
            let at = (y * width + x) as usize;
            if mask[at] == 255 && owners[at] >= word {
                continue;
            }
            let dx = f64::from(x) + 0.5 - cx;
            let distance2 = dx * dx + dy * dy;
            let a = if distance2 <= (radius - 0.5).max(0.).powi(2) {
                255
            } else {
                ((outer - distance2.sqrt()).clamp(0., 1.) * 255.).round() as u8
            };
            if a >= mask[at] {
                mask[at] = a;
                owners[at] = word;
            }
        }
    }
}

// Reveal-only sprites quantize radius/centre to eighths of a physical pixel.
// The settled frame still uses the original analytic pen at exact coordinates.
struct Disc {
    side: usize,
    pixels: Vec<u8>,
    rows: Vec<[u16; 4]>,
}
impl Disc {
    fn new(radius: f64, phase: [u8; 2]) -> Self {
        let pad = radius.ceil() as usize + 1;
        let side = pad * 2 + 2;
        let center = phase.map(|v| pad as f64 + f64::from(v) / 8.);
        let mut pixels = vec![0; side * side];
        let mut rows = Vec::with_capacity(side);
        for (y, row) in pixels.chunks_exact_mut(side).enumerate() {
            for (x, p) in row.iter_mut().enumerate() {
                let distance = (x as f64 + 0.5 - center[0]).hypot(y as f64 + 0.5 - center[1]);
                *p = ((radius + 0.5 - distance).clamp(0., 1.) * 255.).round() as u8;
            }
            let left = row.iter().position(|v| *v > 0).unwrap_or(side);
            let right = row.iter().rposition(|v| *v > 0).map_or(side, |x| x + 1);
            let solid_left = row.iter().position(|v| *v == 255).unwrap_or(right);
            let solid_right = row.iter().rposition(|v| *v == 255).map_or(right, |x| x + 1);
            rows.push([
                left as u16,
                right as u16,
                solid_left as u16,
                solid_right as u16,
            ]);
        }
        Self { side, pixels, rows }
    }
    fn stamp(
        &self,
        mask: &mut [u8],
        owners: &mut [u8],
        word: u8,
        size: [u32; 2],
        center: [i32; 2],
    ) {
        let pad = (self.side - 2) as i32 / 2;
        let base = center.map(|v| v - pad);
        for (y, bounds) in self.rows.iter().enumerate() {
            let dy = base[1] + y as i32;
            if dy < 0 || dy >= size[1] as i32 {
                continue;
            }
            let [left, right, solid_left, solid_right] = bounds.map(i32::from);
            let left = left.max(-base[0]);
            let right = right.min(size[0] as i32 - base[0]);
            if left >= right {
                continue;
            }
            let solid_left = solid_left.clamp(left, right);
            let solid_right = solid_right.clamp(solid_left, right);
            let at = dy as usize * size[0] as usize;
            let row = &mut mask[at..at + size[0] as usize];
            let owner = &mut owners[at..at + size[0] as usize];
            for (start, end) in [(left, solid_left), (solid_right, right)] {
                for x in start..end {
                    let dx = (base[0] + x) as usize;
                    let a = self.pixels[y * self.side + x as usize];
                    if a >= row[dx] {
                        row[dx] = a;
                        owner[dx] = word;
                    }
                }
            }
            let start = (base[0] + solid_left) as usize;
            let end = (base[0] + solid_right) as usize;
            row[start..end].fill(255);
            owner[start..end].fill(word);
        }
    }
}

fn blur(mask: &mut [u8], size: [u32; 2], radius: usize) {
    let [width, height] = size.map(|n| n as usize);
    let mut tmp = vec![0; mask.len()];
    let mut columns = vec![0_u32; width];
    let divisor = radius * 2 + 1;
    // The sum is bounded by 255 * the kernel width. Preserve the old integer
    // truncation without performing a variable integer division at every pixel.
    let quotient: Vec<_> = (0..=255 * divisor).map(|v| (v / divisor) as u8).collect();
    for _ in 0..3 {
        for (src, dst) in mask.chunks_exact(width).zip(tmp.chunks_exact_mut(width)) {
            let mut sum: u32 = src[..=radius.min(width - 1)]
                .iter()
                .map(|v| u32::from(*v))
                .sum();
            for x in 0..width {
                dst[x] = quotient[sum as usize];
                if x >= radius {
                    sum -= u32::from(src[x - radius]);
                }
                if x + radius + 1 < width {
                    sum += u32::from(src[x + radius + 1]);
                }
            }
        }
        columns.fill(0);
        for row in tmp.chunks_exact(width).take(radius.min(height - 1) + 1) {
            for (sum, value) in columns.iter_mut().zip(row) {
                *sum += u32::from(*value);
            }
        }
        // Advance all columns together: source, destination and running sums
        // stay contiguous instead of walking a full-height strided column.
        for (y, dst) in mask.chunks_exact_mut(width).enumerate() {
            for (pixel, sum) in dst.iter_mut().zip(&columns) {
                *pixel = quotient[*sum as usize];
            }
            if y >= radius {
                let row = &tmp[(y - radius) * width..][..width];
                for (sum, value) in columns.iter_mut().zip(row) {
                    *sum -= u32::from(*value);
                }
            }
            if y + radius + 1 < height {
                let row = &tmp[(y + radius + 1) * width..][..width];
                for (sum, value) in columns.iter_mut().zip(row) {
                    *sum += u32::from(*value);
                }
            }
        }
    }
}

pub fn raster(layout: &Layout, dpr: f64, progress: f64) -> Result<RgbaImage> {
    let size = raster_size(layout, dpr)?;
    let mut physical = layout.clone();
    for (word, original) in physical.words.iter_mut().zip(&layout.words) {
        // Relative advances are rounded once from the same canvas origin.
        word.x = layout.word_offset_pixels(original, dpr) / (layout.unit * dpr);
    }
    let mut mask = vec![0; size[0] as usize * size[1] as usize];
    // Colour belongs to the circle, including its round end and displaced
    // edge. A vertical colour cut would tint the neighbouring word's pen.
    let mut owners = vec![0; mask.len()];
    let mut discs = std::collections::BTreeMap::new();
    let mut disc_bytes = 0;
    const DISC_BYTES: usize = 2 * 1024 * 1024;
    for sample in samples(&physical, progress)? {
        let center = sample.center.map(|n| n * dpr);
        let radius = sample.radius * dpr;
        if progress < 1. {
            // Growing samples otherwise create hundreds of one-use sprites.
            // Half-pixel radius / quarter-pixel centre buckets bound that
            // construction cost; full-width strokes retain eighth-pixel coverage.
            let growing = sample.radius < PEN * layout.unit;
            let grid = if growing { 4. } else { 8. };
            let multiplier = if growing { 2 } else { 1 };
            let radius_grid = if growing { 2. } else { 8. };
            let radius_multiplier = if growing { 4 } else { 1 };
            let bucket = (radius * radius_grid).round() as u32 * radius_multiplier;
            if bucket == 0 {
                continue;
            }
            let quantized = center.map(|n| (n * grid).round() as i32 * multiplier);
            let phase = quantized.map(|n| n.rem_euclid(8) as u8);
            let key = (bucket, phase);
            let side = (f64::from(bucket) / 8.).ceil() as usize * 2 + 4;
            let disc = match discs.entry(key) {
                Entry::Occupied(entry) => Some(entry.into_mut()),
                Entry::Vacant(entry) if disc_bytes + side * side + side * 8 <= DISC_BYTES => {
                    disc_bytes += side * side + side * 8;
                    Some(entry.insert(Disc::new(f64::from(bucket) / 8., phase)))
                }
                Entry::Vacant(_) => None,
            };
            if let Some(disc) = disc {
                disc.stamp(
                    &mut mask,
                    &mut owners,
                    sample.word as u8,
                    size,
                    quantized.map(|n| n.div_euclid(8)),
                );
                continue;
            }
        }
        circle(
            &mut mask,
            &mut owners,
            sample.word as u8,
            size,
            center,
            radius,
        );
    }
    // At ordinary DPR the moving shadow uses half-resolution coverage. Higher
    // densities keep the full shadow so the existing packed atlas byte proof
    // remains valid (tests/menu.rs outgoing DPR5 case). Rest is always exact.
    let shadow_scale = if progress < 1. && dpr <= 3. { 2 } else { 1 };
    let shadow_size = size.map(|n| n.div_ceil(shadow_scale));
    let mut shadow = if shadow_scale == 1 {
        mask.clone()
    } else {
        let mut small = vec![0; shadow_size[0] as usize * shadow_size[1] as usize];
        for y in 0..shadow_size[1] {
            for x in 0..shadow_size[0] {
                let mut sum = 0_u16;
                for row in y * 2..(y * 2 + 2).min(size[1]) {
                    for col in x * 2..(x * 2 + 2).min(size[0]) {
                        sum += u16::from(mask[(row * size[0] + col) as usize]);
                    }
                }
                small[(y * shadow_size[0] + x) as usize] = ((sum + 2) / 4) as u8;
            }
        }
        small
    };
    let radius = (1.3 * layout.unit / 7. * dpr).ceil() as usize;
    blur(
        &mut shadow,
        shadow_size,
        if shadow_scale == 1 {
            radius
        } else {
            (radius / 2).max(1)
        },
    );
    let mut pixels = vec![0; mask.len() * 4];
    let width = size[0] as usize;
    let shift = (2. * layout.unit / 7. * dpr).round() as usize;
    let shadow_width = shadow_size[0] as usize;
    let empty = vec![0; shadow_width];
    let shadow_only: [u8; 256] =
        std::array::from_fn(|v| ((v as f64 / 255. * 0.45) * 255.).round() as u8);
    for (y, row) in pixels.chunks_exact_mut(width * 4).enumerate() {
        let t = ((y as f64 / dpr / layout.unit - 2.) / 10.).clamp(0., 1.);
        let gradients = COLORS.map(|colors| {
            std::array::from_fn::<_, 3, _>(|k| {
                f64::from(colors[0][k]) * (1. - t) + f64::from(colors[1][k]) * t
            })
        });
        let opaque = gradients.map(|rgb| {
            [
                rgb[0].round() as u8,
                rgb[1].round() as u8,
                rgb[2].round() as u8,
                255,
            ]
        });
        let shade = if y >= shift {
            &shadow[(y - shift) / shadow_scale as usize * shadow_width..][..shadow_width]
        } else {
            &empty
        };
        let coverage = &mask[y * width..][..width];
        let owners = &owners[y * width..][..width];
        for (x, ((pixel, alpha), owner)) in row
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(coverage)
            .zip(owners)
            .enumerate()
        {
            let shade = shade[x / shadow_scale as usize];
            if *alpha == 0 {
                pixel[3] = shadow_only[usize::from(shade)];
                continue;
            }
            let color = layout.words.get(usize::from(*owner)).map_or(0, |w| w.color);
            if *alpha == 255 {
                *pixel = opaque[color];
                continue;
            }
            let alpha = f64::from(*alpha) / 255.;
            for (channel, value) in pixel[..3].iter_mut().zip(gradients[color]) {
                *channel = (value * alpha).round() as u8;
            }
            let shade = f64::from(shade) / 255. * 0.45;
            pixel[3] = ((alpha + shade * (1. - alpha)) * 255.).round() as u8;
        }
    }
    Ok(RgbaImage {
        width: size[0],
        height: size[1],
        pixels,
    })
}

#[cfg(test)]
mod raster_tests {
    use super::*;

    #[test]
    fn disc_sprites_match_analytic_coverage_and_word_ownership_on_their_grid() {
        let size = [37, 29];
        for radius in [0.5, 0.875, 1.25, 3.125, 6.25, 12.375] {
            for phase in [[0, 0], [1, 7], [4, 4], [7, 2]] {
                for integer in [[-8, -5], [0, 0], [12, 14], [36, 28], [43, 35]] {
                    let disc = Disc::new(radius, phase);
                    let mut actual = vec![0; 37 * 29];
                    let mut owners = actual.clone();
                    let mut expected = actual.clone();
                    let mut expected_owners = actual.clone();
                    for word in 0..3 {
                        let integer = [integer[0] + i32::from(word), integer[1]];
                        let center = std::array::from_fn(|i| {
                            f64::from(integer[i]) + f64::from(phase[i]) / 8.
                        });
                        disc.stamp(&mut actual, &mut owners, word, size, integer);
                        circle(
                            &mut expected,
                            &mut expected_owners,
                            word,
                            size,
                            center,
                            radius,
                        );
                    }
                    assert_eq!(actual, expected);
                    for (i, alpha) in actual.iter().enumerate().filter(|(_, a)| **a > 0) {
                        assert_eq!(
                            owners[i], expected_owners[i],
                            "visible coverage {alpha} at {i}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn contiguous_blur_preserves_zero_edges_and_integer_truncation() {
        for size in [[1, 1], [1, 9], [7, 3], [37, 29]] {
            for radius in [0, 1, 2, 7, 32] {
                let width = size[0] as usize;
                let height = size[1] as usize;
                let mut expected: Vec<_> = (0..width * height)
                    .map(|i| ((i * 31 + 17) % 256) as u8)
                    .collect();
                let mut actual = expected.clone();
                // Independent direct convolution of the previous three box passes.
                for _ in 0..3 {
                    let old = expected.clone();
                    for y in 0..height {
                        for x in 0..width {
                            let sum: usize = (x.saturating_sub(radius)
                                ..=(x + radius).min(width - 1))
                                .map(|i| usize::from(old[y * width + i]))
                                .sum();
                            expected[y * width + x] = (sum / (radius * 2 + 1)) as u8;
                        }
                    }
                    let old = expected.clone();
                    for y in 0..height {
                        for x in 0..width {
                            let sum: usize = (y.saturating_sub(radius)
                                ..=(y + radius).min(height - 1))
                                .map(|i| usize::from(old[i * width + x]))
                                .sum();
                            expected[y * width + x] = (sum / (radius * 2 + 1)) as u8;
                        }
                    }
                }
                blur(&mut actual, size, radius);
                assert_eq!(actual, expected);
            }
        }
    }

    #[test]
    fn settled_pixels_equal_the_prior_scalar_raster_at_every_density() {
        let holder = MenuRect {
            x: 735.,
            y: 50.,
            width: 450.,
            height: 115.,
        };
        for value in [DEFAULT_TEXT, "Hello World 123"] {
            let layout = layout(value, holder, 1920., |_| unreachable!()).unwrap();
            for dpr in [1., 2., 5.] {
                let actual = raster(&layout, dpr, 1.).unwrap();
                let expected = scalar_reference(&layout, dpr, 1.).unwrap();
                assert_eq!(actual.pixels, expected.pixels);
            }
        }
    }

    // b71b68ae's scalar colour/shadow composition is retained only as a
    // regression oracle. Pen geometry is shared; blur has a separate direct oracle.
    fn scalar_reference(layout: &Layout, dpr: f64, progress: f64) -> Result<RgbaImage> {
        let size = raster_size(layout, dpr)?;
        let mut physical = layout.clone();
        let origin_x = layout.origin_pixels(dpr)[0];
        for (word, original) in physical.words.iter_mut().zip(&layout.words) {
            // Relative advances are rounded once from the same canvas origin.
            word.x = (layout.word_x_pixels(original, dpr) - origin_x) / (layout.unit * dpr);
        }
        let mut mask = vec![0; size[0] as usize * size[1] as usize];
        // Colour belongs to the circle, including its round end and displaced
        // edge. A vertical colour cut would tint the neighbouring word's pen.
        let mut owners = vec![0; mask.len()];
        for sample in samples(&physical, progress)? {
            circle(
                &mut mask,
                &mut owners,
                sample.word as u8,
                size,
                sample.center.map(|n| n * dpr),
                sample.radius * dpr,
            );
        }
        let mut shadow = mask.clone();
        blur(
            &mut shadow,
            size,
            (1.3 * layout.unit / 7. * dpr).ceil() as usize,
        );
        let mut pixels = vec![0; mask.len() * 4];
        let offset = (2. * layout.unit / 7. * dpr).round() as usize * size[0] as usize;
        for (i, p) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let y = (i / size[0] as usize) as f64 / dpr / layout.unit;
            let word = layout.words.get(usize::from(owners[i]));
            let colors = COLORS[word.map_or(0, |w| w.color)];
            let t = ((y - 2.) / 10.).clamp(0., 1.);
            let alpha = f64::from(mask[i]) / 255.;
            let shade = if i >= offset {
                f64::from(shadow[i - offset]) / 255. * 0.45
            } else {
                0.
            };
            for (k, channel) in p.iter_mut().take(3).enumerate() {
                *channel = ((f64::from(colors[0][k]) * (1. - t) + f64::from(colors[1][k]) * t)
                    * alpha)
                    .round() as u8;
            }
            p[3] = ((alpha + shade * (1. - alpha)) * 255.).round() as u8;
        }
        Ok(RgbaImage {
            width: size[0],
            height: size[1],
            pixels,
        })
    }
}

#[cfg(test)]
mod round_geometry_tests {
    use super::*;

    #[test]
    fn bowls_and_shoulders_follow_ellipses_and_s_arcs_share_a_tangent() {
        for (code, center, radii) in [
            ("B 0 4 6 6", [3., 7.], [3., 3.]),
            ("A 0 0 8 10 -40 -280", [4., 5.], [4., 5.]),
            ("H 0 4 6 3", [3., 7.], [3., 3.]),
            ("H 0 10 6 -3", [3., 7.], [3., 3.]),
        ] {
            let curves = paths(code);
            assert_eq!(curves.len(), 1);
            for p in &curves[0] {
                let ellipse = ((p[0] - center[0]) / radii[0]).powi(2)
                    + ((p[1] - center[1]) / radii[1]).powi(2);
                assert!((ellipse - 1.).abs() < 1e-12);
            }
        }
        // Pen-lift counts retain each family's reveal order/delays.
        for (glyph, strokes) in [
            ('C', 1),
            ('G', 1),
            ('O', 1),
            ('Q', 2),
            ('S', 1),
            ('U', 1),
            ('a', 1),
            ('b', 2),
            ('c', 1),
            ('d', 2),
            ('e', 1),
            ('g', 1),
            ('h', 2),
            ('m', 3),
            ('n', 2),
            ('o', 1),
            ('p', 2),
            ('q', 2),
            ('r', 2),
            ('s', 1),
            ('u', 2),
        ] {
            assert_eq!(
                paths(super::glyph(glyph).unwrap().1).len(),
                strokes,
                "{glyph}"
            );
        }
        let curves = paths("S 0 0 7 10");
        assert_eq!(curves.len(), 1);
        let curve = &curves[0];
        let join = 48;
        assert!((curve[join][0] - 3.5).abs() < 1e-12);
        assert!((curve[join][1] - 5.).abs() < 1e-12);
        let a = [
            curve[join][0] - curve[join - 1][0],
            curve[join][1] - curve[join - 1][1],
        ];
        let b = [
            curve[join + 1][0] - curve[join][0],
            curve[join + 1][1] - curve[join][1],
        ];
        assert!(a[0] > 0. && b[0] > 0.);
        assert!((a[0] * b[1] - a[1] * b[0]).abs() < 1e-12);
    }

    #[test]
    fn wiggle_is_two_thirds_of_the_previous_wave_with_the_same_phase_and_decay() {
        let geometry = layout(
            "l",
            MenuRect {
                x: 0.,
                y: 0.,
                width: 450.,
                height: 115.,
            },
            1920.,
            |_| unreachable!(),
        )
        .unwrap();
        let rest = samples(&geometry, 1.).unwrap()[0];
        for progress in [0.5_f64, 0.75] {
            let sample = samples(&geometry, progress).unwrap()[0];
            let previous =
                1.9 * (1. - progress).powi(3) * 0.55 * (progress * 34.).sin() * geometry.unit;
            let ratio = (rest.center[0] - sample.center[0]) / previous;
            assert!((ratio - 1.25 / 1.9).abs() < 1e-12);
            assert_eq!(sample.center[1], rest.center[1]);
            assert_eq!(sample.radius, rest.radius);
        }
    }
}

#[cfg(test)]
mod font_run_tests {
    use super::*;
    fn run(text: &str, color: usize, spaces: usize) -> FontRun {
        FontRun {
            text: text.into(),
            color,
            spaces,
        }
    }
    #[test]
    fn equal_colour_words_share_a_label_and_overflow_gets_an_ellipsis() {
        let (runs, truncated) = bounded_font_runs(
            vec![run("One", 0, 0), run("Two", 0, 1), run("Three", 0, 2)],
            1,
        );
        assert!(!truncated);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].text, "One Two  Three");
        let (runs, truncated) = bounded_font_runs(
            vec![
                run("Slither", 0, 0),
                run("Client", 1, 0),
                run("Suffix", 0, 0),
            ],
            2,
        );
        assert!(truncated);
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[1].text, "Client...");
        for limit in [0, 1, 2, 60] {
            let layout = fallback_layout(
                &"Ab".repeat(60),
                MenuRect {
                    x: 0.,
                    y: 0.,
                    width: 450.,
                    height: 115.,
                },
                960.,
                limit,
                |s| Ok(s.chars().count() as f64 * 7.),
            )
            .unwrap();
            assert!(layout.words.len() <= limit);
            assert!(layout.words.iter().all(|w| w.fallback));
            assert!(layout.rect.width <= 450.);
            if limit > 0 && limit < 60 {
                assert!(layout.words.last().unwrap().text.ends_with("..."));
            }
        }
    }
}
