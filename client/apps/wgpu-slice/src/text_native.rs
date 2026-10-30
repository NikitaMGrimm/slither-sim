//! Windows adapter: no bundled fonts, desktop APIs or window creation.
use super::{Raster, Weight};
use crate::{Result, materials::RgbaImage};
use fontdue::{Font, FontSettings};

fn expand_luminance(bucket: u8) -> u8 {
    (bucket << 5) | (bucket << 2) | (bucket >> 1)
}

fn luminance_bucket(color: [u8; 3]) -> usize {
    // SkScalerContext canonicalizes RGB to three bits before reducing A8 masks
    // to luminance, then canonicalizes that grey again (SkComputeLuminance).
    let [r, g, b] = color.map(|c| u32::from(expand_luminance(c >> 5)));
    (((54 * r + 183 * g + 19 * b) >> 8) >> 5) as usize
}

fn linear_srgb(value: f64) -> f64 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn encoded_srgb(value: f64) -> f64 {
    if value <= 0.0031308 {
        value * 12.92
    } else {
        1.055 * value.powf(1. / 2.4) - 0.055
    }
}

fn coverage_table(bucket: u8) -> [u8; 256] {
    // Windows Chromium uses sRGB (gamma=0 sentinel) and contrast=1.0:
    // https://github.com/chromium/chromium/blob/152.0.7977.167/skia/BUILD.gn#L137-L142
    // Skia's model guesses the encoded background as 1 - foreground, boosts
    // coverage by c*(1-c)*contrast*linear(background), then solves for the alpha
    // that reproduces a linear-light blend in an encoded source-over texture.
    // https://skia.org/docs/dev/design/raster_tragedy/
    // https://github.com/google/skia/blob/0789f2ab93b844c5f2f5a96eb80c7c8f1ef6f570/src/core/SkMaskGamma.cpp#L74-L125
    // This is a grey-mask approximation over fontdue, not DirectWrite hinting
    // or LCD rendering. Three-bit luminance avoids the singular middle grey.
    let foreground = f64::from(expand_luminance(bucket)) / 255.;
    let background = 1. - foreground;
    let linear_foreground = linear_srgb(foreground);
    let linear_background = linear_srgb(background);
    const CONTRAST: f64 = 1.;
    std::array::from_fn(|i| {
        if i == 0 || i == 255 {
            return i as u8;
        }
        let coverage = i as f64 / 255.;
        let boosted = coverage + coverage * (1. - coverage) * CONTRAST * linear_background;
        let encoded =
            encoded_srgb(linear_foreground * boosted + linear_background * (1. - boosted));
        (((encoded - background) / (foreground - background)) * 255.)
            .round()
            .clamp(0., 255.) as u8
    })
}

fn coverage_lut(color: [u8; 3]) -> &'static [u8; 256] {
    // Only 2 KiB, shared by all rasterizers. The hot pixel loop is one lookup.
    static TABLES: std::sync::OnceLock<[[u8; 256]; 8]> = std::sync::OnceLock::new();
    &TABLES.get_or_init(|| std::array::from_fn(|i| coverage_table(i as u8)))
        [luminance_bucket(color)]
}

#[derive(Default)]
pub struct Rasterizer {
    regular: Option<Arial>,
    bold: Option<Arial>,
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn preblend_preserves_endpoints_and_is_monotonic_in_every_luminance_bucket() {
        for bucket in 0..8 {
            let table = coverage_table(bucket);
            assert_eq!(table[0], 0);
            assert_eq!(table[255], 255);
            assert!(table.windows(2).all(|p| p[0] <= p[1]));
        }
    }

    #[test]
    fn preblend_uses_colour_luminance_for_light_and_dark_text() {
        let dark = coverage_lut([0; 3]);
        let grey = coverage_lut([128; 3]);
        let light = coverage_lut([255; 3]);
        assert_eq!([dark[64], dark[128], dark[192]], [58, 119, 185]);
        assert_eq!([grey[64], grey[128], grey[192]], [81, 149, 207]);
        assert_eq!([light[64], light[128], light[192]], [137, 188, 225]);
        for i in [64, 128, 192] {
            assert!(dark[i] <= i as u8);
            assert!(light[i] >= i as u8);
            assert!(dark[i] <= grey[i] && grey[i] <= light[i]);
        }
        // Skia A8 uses encoded BT.709 luminance, not max(R,G,B) or opacity.
        assert_eq!(luminance_bucket([255, 0, 0]), 1);
        assert_eq!(luminance_bucket([0, 255, 0]), 5);
        assert_eq!(luminance_bucket([0, 0, 255]), 0);
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn ascii_space_advances_match_canvas_without_pair_kerning() {
        let mut rasterizer = Rasterizer::new();
        // Authored strings, absolute advances measured in 15px Arial Canvas.
        for (text, expected) in [("A\u{b}V", 24.1772), ("A V A V A V A V A V A V", 165.9009)] {
            let raster = rasterizer
                .rasterize(text, 15., Weight::Normal, [255; 3])
                .unwrap();
            assert!(
                (raster.advance - expected).abs() < 0.05,
                "{text:?}: {} px, Canvas {expected} px",
                raster.advance
            );
        }
    }
    #[test]
    fn nbsp_retains_pair_kerning() {
        let raster = Rasterizer::new()
            .rasterize("A\u{a0}V", 15., Weight::Normal, [255; 3])
            .unwrap();
        assert!(
            (raster.advance - 23.3496).abs() < 0.05,
            "{}",
            raster.advance
        );
    }
    #[test]
    fn soft_hyphen_is_invisible_and_vertical_tab_is_a_space() {
        let mut rasterizer = Rasterizer::new();
        let mut raster = |s: &str| {
            rasterizer
                .rasterize(s, 15., Weight::Normal, [255; 3])
                .unwrap()
        };
        let soft = raster("A\u{ad}V");
        let invisible = raster("A\u{1d}V");
        assert_eq!(soft.advance, invisible.advance);
        assert_eq!(soft.image.pixels, invisible.image.pixels);
        let tab = raster("A\u{b}V");
        let space = raster("A V");
        assert_eq!(tab.advance, space.advance);
        assert_eq!(tab.image.pixels, space.image.pixels);
        let soft = raster("\u{ad}");
        assert_eq!(soft.advance, 0.);
        assert!(soft.image.pixels.iter().all(|p| *p == 0));
    }
    #[test]
    fn c0_and_c1_rasters_use_canvas_missing_glyph_or_whitespace() {
        let mut rasterizer = Rasterizer::new();
        for weight in [Weight::Normal, Weight::Bold] {
            for px in [15., 30.] {
                for code in (0..=31).chain(128..=159) {
                    let c = char::from_u32(code).unwrap().to_string();
                    // Authored oracle: Chromium uses Arial's missing glyph for
                    // these controls, except ASCII whitespace and U+001D.
                    let equivalent = match code {
                        9..=13 => " ",
                        29 => "",
                        _ => "\u{ffff}",
                    };
                    for (actual, expected) in [
                        (c.clone(), equivalent.to_owned()),
                        (format!("A{c}V"), format!("A{equivalent}V")),
                    ] {
                        // U+001D is a shaping boundary, covered above separately.
                        if code == 29 && actual.len() > 1 {
                            continue;
                        }
                        let actual = rasterizer.rasterize(&actual, px, weight, [255; 3]).unwrap();
                        let expected = rasterizer
                            .rasterize(&expected, px, weight, [255; 3])
                            .unwrap();
                        assert_eq!(
                            actual.advance, expected.advance,
                            "U+{code:04X}, {px}, {weight:?}"
                        );
                        assert_eq!(actual.offset, expected.offset);
                        assert_eq!(actual.image.width, expected.image.width);
                        assert_eq!(actual.image.height, expected.image.height);
                        assert_eq!(
                            actual.image.pixels, expected.image.pixels,
                            "U+{code:04X}, {px}, {weight:?}"
                        );
                        if equivalent == "\u{ffff}" {
                            assert!(actual.image.pixels.iter().any(|p| *p != 0));
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn del_raster_is_blank_with_canvas_fallback_advance() {
        let mut rasterizer = Rasterizer::new();
        for (px, regular, bold) in [
            (8., 4., 4.),
            (12., 6., 7.),
            (12.5, 7., 8.),
            (15., 8., 9.),
            (17.9, 9., 10.),
            (18., 9., 9.),
            (30., 15., 15.),
        ] {
            for (weight, advance) in [(Weight::Normal, regular), (Weight::Bold, bold)] {
                let del = rasterizer
                    .rasterize("\u{7f}", px, weight, [255; 3])
                    .unwrap();
                assert_eq!(del.advance, advance);
                assert!(del.image.pixels.iter().all(|p| *p == 0));
                let av = rasterizer.rasterize("AV", px, weight, [255; 3]).unwrap();
                let padded = rasterizer
                    .rasterize("\u{7f}AV\u{7f}", px, weight, [255; 3])
                    .unwrap();
                assert!((padded.advance - av.advance - 2. * advance).abs() < 0.00001);
            }
        }
    }
}
struct Arial {
    font: Font,
    em_ascent: f64,
}
fn canvas_character(c: char) -> Option<char> {
    match c {
        '\u{9}'..='\u{d}' => Some(' '),
        '\u{1d}' | '\u{ad}' => None,
        // fontdue merges legacy cmaps, where these codes can name unrelated
        // accented glyphs or the null glyph. Canvas uses Arial's .notdef box.
        '\u{0}'..='\u{1f}' | '\u{80}'..='\u{9f}' => Some('\u{ffff}'),
        _ => Some(c),
    }
}
fn del_advance(px: f64, weight: Weight) -> f32 {
    // Windows Chromium's blank SimSun fallback has half-em advances. At these
    // small sizes its hinted widths snap to pixels, plus one for synthetic bold.
    if (12. ..18.).contains(&px) {
        ((px.round() / 2.).round() + f64::from(weight == Weight::Bold)) as f32
    } else {
        (px / 2.) as f32
    }
}
impl Rasterizer {
    pub fn new() -> Self {
        Self::default()
    }
    #[cfg(test)]
    pub fn rasterize(
        &mut self,
        text: &str,
        px: f64,
        weight: Weight,
        color: [u8; 3],
    ) -> Result<Raster> {
        self.rasterize_limited(text, px, weight, color, None)
    }
    pub fn rasterize_limited(
        &mut self,
        text: &str,
        px: f64,
        weight: Weight,
        color: [u8; 3],
        max_width: Option<u32>,
    ) -> Result<Raster> {
        let slot = match weight {
            Weight::Normal => &mut self.regular,
            Weight::Bold => &mut self.bold,
        };
        if slot.is_none() {
            let file = match weight {
                Weight::Normal => "arial.ttf",
                Weight::Bold => "arialbd.ttf",
            };
            let bytes = std::fs::read(std::path::Path::new("C:/Windows/Fonts").join(file))
                .map_err(|e| format!("system Arial ({file}): {e}"))?;
            let face = ttf_parser::Face::parse(&bytes, 0)
                .map_err(|e| format!("system Arial metrics: {e}"))?;
            let ascent = f64::from(
                face.typographic_ascender()
                    .ok_or("Arial typographic ascent")?,
            );
            let descent = f64::from(
                face.typographic_descender()
                    .ok_or("Arial typographic descent")?,
            );
            if ascent <= descent {
                return Err("Arial typographic metrics".into());
            }
            let em_ascent = ascent / (ascent - descent);
            *slot = Some(Arial {
                font: Font::from_bytes(bytes, FontSettings::default())
                    .map_err(|e| format!("system Arial: {e}"))?,
                em_ascent,
            });
        }
        let arial = slot.as_ref().unwrap();
        let font = &arial.font;
        // Supersampling retains fractional advances and kerning without ClearType
        // color fringes in the renderer's transparent premultiplied textures.
        const SAMPLE: usize = 4;
        let size = px as f32 * SAMPLE as f32;
        let mut glyphs = Vec::new();
        let mut pen = 0_f32;
        let mut previous = None;
        let mut bounds = [0_i32; 4];
        for c in text.chars() {
            if c == '\u{7f}' {
                pen += del_advance(px, weight) * SAMPLE as f32;
                previous = None;
                continue;
            }
            let Some(c) = canvas_character(c) else {
                // Canvas suppresses these glyphs but keeps a shaping boundary:
                // adjacent letters do not acquire kerning across the control.
                previous = None;
                continue;
            };
            // Canvas breaks kerning at ASCII spaces, including normalized
            // whitespace. NBSP keeps its ordinary glyph-pair behavior.
            if let Some(p) = previous
                && p != ' '
                && c != ' '
            {
                pen += font.horizontal_kern(p, c, size).unwrap_or(0.);
            }
            let m = font.metrics(c, size);
            let x = pen.round() as i32 + m.xmin;
            let y = -m.ymin - m.height as i32;
            bounds[0] = bounds[0].min(x);
            bounds[1] = bounds[1].min(y);
            bounds[2] = bounds[2].max(x + m.width as i32);
            bounds[3] = bounds[3].max(y + m.height as i32);
            glyphs.push((x, y, m.width, m.height, c));
            pen += m.advance_width;
            previous = Some(c);
        }
        let left = (f64::from(bounds[0]) / SAMPLE as f64).floor() as i32 - 1;
        let top = (f64::from(bounds[1]) / SAMPLE as f64).floor() as i32 - 1;
        let width =
            ((f64::from(bounds[2]) / SAMPLE as f64).ceil() as i32 - left + 1).max(1) as usize;
        let width = max_width.map_or(width, |limit| width.min(limit as usize + 2));
        let height =
            ((f64::from(bounds[3]) / SAMPLE as f64).ceil() as i32 - top + 1).max(1) as usize;
        if width > 8192 || height > 512 {
            return Err("text raster extent budget".into());
        }
        let mut mask = vec![0_u8; width * height * SAMPLE * SAMPLE];
        let stride = width * SAMPLE;
        for (x, y, w, h, c) in glyphs {
            let x = (x - left * SAMPLE as i32) as usize;
            let y = (y - top * SAMPLE as i32) as usize;
            if x >= stride {
                continue;
            }
            let (_, coverage) = font.rasterize(c, size);
            for row in 0..h {
                for col in 0..w.min(stride - x) {
                    let dst = &mut mask[(y + row) * stride + x + col];
                    let a = u32::from(coverage[row * w + col]);
                    *dst = (a + u32::from(*dst) * (255 - a) / 255) as u8;
                }
            }
        }
        let mut pixels = vec![0; width * height * 4];
        let coverage = coverage_lut(color);
        for y in 0..height {
            for x in 0..width {
                let mut sum = 0_u32;
                for dy in 0..SAMPLE {
                    for dx in 0..SAMPLE {
                        sum += u32::from(mask[(y * SAMPLE + dy) * stride + x * SAMPLE + dx]);
                    }
                }
                // Preblend the final averaged coverage, before premultiplication.
                let a = coverage[((sum + 8) / 16) as usize];
                let dst = &mut pixels[(y * width + x) * 4..][..4];
                for i in 0..3 {
                    dst[i] = ((u16::from(color[i]) * u16::from(a) + 127) / 255) as u8;
                }
                dst[3] = a;
            }
        }
        Ok(Raster {
            image: RgbaImage {
                width: width as u32,
                height: height as u32,
                pixels,
            },
            offset: [f64::from(left), f64::from(top)],
            advance: f64::from(pen) / SAMPLE as f64,
            // Canvas middle uses the em square, not hhea's taller line box.
            // Normalize OS/2 typography to one em, at the browser's 1/64px grid.
            middle: (px * arial.em_ascent * 64.).round() / 64. - px / 2.,
        })
    }
}
