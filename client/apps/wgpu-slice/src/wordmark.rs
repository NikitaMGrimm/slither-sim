//! Authored round-pen wordmarks. A swelling, displaced chain of circles draws
//! each stroke; the retained reveal clock also damps the ripple to exact rest.
use crate::{Result, materials::RgbaImage};
use slither_client::menu_layout::MenuRect;

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
    /// Snapped raster origin; atlas pieces use integer offsets from this grid.
    pub fn origin_pixels(&self, dpr: f64) -> [f64; 2] {
        [self.rect.x * dpr, self.rect.y * dpr].map(f64::round)
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
    let mut words = Vec::new();
    let mut advance = 0.;
    for chunk in chunks {
        if chunk == " " {
            advance += 4.;
            continue;
        }
        let fallback = !chunk.chars().all(|c| glyph(c).is_some());
        let width = if fallback {
            fallback_width(&chunk)?
        } else {
            chunk
                .chars()
                .map(|c| glyph(c).unwrap().0 + TRACKING)
                .sum::<f64>()
                - TRACKING
        };
        if !width.is_finite() || width < 0. {
            return Err("Wordmark advance bounds".into());
        }
        words.push(Word {
            text: chunk,
            x: advance + 3.,
            width,
            color: words.len() % 2,
            fallback,
        });
        advance += width + TRACKING;
    }
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
fn paths(code: &str) -> Vec<Vec<Point>> {
    let mut tokens = code.split_whitespace();
    let mut paths: Vec<Vec<Point>> = Vec::new();
    while let Some(command) = tokens.next() {
        let mut number = || tokens.next().unwrap().parse::<f64>().unwrap();
        match command {
            "M" => paths.push(vec![[number(), number()]]),
            "L" => paths.last_mut().unwrap().push([number(), number()]),
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
                let wave = 1.9
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
    if !dpr.is_finite() || dpr <= 0. || dpr > 5.5 {
        return Err("Wordmark raster scale bounds".into());
    }
    let size = [
        (layout.rect.width * dpr).ceil().max(1.) as u32,
        (layout.rect.height * dpr).ceil().max(1.) as u32,
    ];
    if size[1] > 2048 || u64::from(size[0]) * u64::from(size[1]) * 4 > MAX_RASTER_BYTES as u64 {
        return Err("Wordmark raster byte budget".into());
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

fn blur(mask: &mut Vec<u8>, size: [u32; 2], radius: usize) {
    let [width, height] = size.map(|n| n as usize);
    let mut tmp = vec![0; mask.len()];
    for _ in 0..3 {
        for (w, h, horizontal) in [(width, height, true), (height, width, false)] {
            for row in 0..h {
                let at = |x: usize| {
                    if horizontal {
                        row * width + x
                    } else {
                        x * width + row
                    }
                };
                let mut sum: u32 = (0..=radius.min(w - 1))
                    .map(|x| u32::from(mask[at(x)]))
                    .sum();
                for x in 0..w {
                    tmp[at(x)] = (sum / (radius as u32 * 2 + 1)) as u8;
                    if x >= radius {
                        sum -= u32::from(mask[at(x - radius)]);
                    }
                    if x + radius + 1 < w {
                        sum += u32::from(mask[at(x + radius + 1)]);
                    }
                }
            }
            std::mem::swap(mask, &mut tmp);
        }
    }
}

pub fn raster(layout: &Layout, dpr: f64, progress: f64) -> Result<RgbaImage> {
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
            *channel = ((f64::from(colors[0][k]) * (1. - t) + f64::from(colors[1][k]) * t) * alpha)
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
