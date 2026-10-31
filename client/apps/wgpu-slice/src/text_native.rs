//! Native adapter: no bundled fonts, desktop APIs or window creation.
use super::{Family, Raster, Slant, TextStyle, Weight, display_character};
use crate::{Result, materials::RgbaImage};
use fontdue::{Font, FontSettings};
use slither_client::presentation::TextMeasurement;
use std::collections::BTreeMap;

fn coverage_gamma(color: [u8; 3]) -> f64 {
    let [r, g, b] = color.map(f64::from);
    let luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.;
    // A continuous perceptual ink-weight adjustment, not a browser raster model.
    // Light text needs heavier edges; dark text must not receive that boost.
    1.10 - 0.65 * luminance
}

fn adjusted_coverage(coverage: u8, gamma: f64) -> u8 {
    (255. * (f64::from(coverage) / 255.).powf(gamma)).round() as u8
}

#[derive(Default)]
pub struct Rasterizer {
    faces: BTreeMap<(Family, Weight, Slant), Arial>,
}

#[cfg(test)]
mod coverage_tests {
    use super::*;

    #[test]
    fn curve_preserves_endpoints_and_monotonicity() {
        for grey in 0..=255 {
            let gamma = coverage_gamma([grey; 3]);
            let values: Vec<_> = (0..=255).map(|c| adjusted_coverage(c, gamma)).collect();
            assert_eq!(values[0], 0);
            assert_eq!(values[255], 255);
            assert!(values.windows(2).all(|p| p[0] <= p[1]));
        }
    }

    #[test]
    fn light_text_is_heavier_than_raw_coverage() {
        let dark = coverage_gamma([0; 3]);
        let light = coverage_gamma([255; 3]);
        for c in 1..255 {
            assert!(adjusted_coverage(c, dark) <= c);
            assert!(adjusted_coverage(c, light) >= c);
        }
        assert!(adjusted_coverage(128, light) > 128);
    }

    #[test]
    fn gamma_varies_continuously_with_foreground_luminance() {
        for grey in 0..255 {
            let change = coverage_gamma([grey; 3]) - coverage_gamma([grey + 1; 3]);
            assert!(change > 0. && change < 0.003);
        }
        assert!(coverage_gamma([0, 255, 0]) < coverage_gamma([255, 0, 0]));
        assert!(coverage_gamma([255, 0, 0]) < coverage_gamma([0, 0, 255]));
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn controls_use_the_same_bounded_display_policy_for_metrics_and_pixels() {
        let mut rasterizer = Rasterizer::new();
        for family in [Family::Arial, Family::Lucida] {
            for weight in [Weight::Normal, Weight::Bold] {
                let style = TextStyle {
                    family,
                    weight,
                    size: 15.,
                    slant: Slant::Normal,
                    color: [255; 3],
                };
                for code in (0..=31).chain(127..=159).chain([173]) {
                    let c = char::from_u32(code).unwrap();
                    let equivalent = if c.is_whitespace() { ' ' } else { '\u{fffd}' };
                    for (input, expected) in [
                        (c.to_string(), equivalent.to_string()),
                        (format!("A{c}V"), format!("A{equivalent}V")),
                    ] {
                        let m = rasterizer.measure(&input, style).unwrap();
                        let actual = rasterizer
                            .rasterize_styled(&input, 15., style, None)
                            .unwrap();
                        let expected = rasterizer
                            .rasterize_styled(&expected, 15., style, None)
                            .unwrap();
                        assert_eq!(m.advance, actual.advance);
                        assert!(m.advance.is_finite() && m.advance > 0.);
                        assert_eq!(actual.advance, expected.advance);
                        assert_eq!(actual.offset, expected.offset);
                        assert_eq!(actual.image.width, expected.image.width);
                        assert_eq!(actual.image.height, expected.image.height);
                        assert_eq!(actual.image.pixels, expected.image.pixels);
                    }
                }
            }
        }
    }

    #[test]
    fn whitespace_keeps_its_normal_advance_and_pair_boundaries() {
        let mut rasterizer = Rasterizer::new();
        let space = rasterizer
            .rasterize("A V", 15., Weight::Normal, [255; 3])
            .unwrap();
        for text in ["A\tV", "A\nV", "A\rV", "A\u{b}V", "A\u{85}V"] {
            let actual = rasterizer
                .rasterize(text, 15., Weight::Normal, [255; 3])
                .unwrap();
            assert_eq!(actual.advance, space.advance);
            assert_eq!(actual.image.width, space.image.width);
            assert_eq!(actual.image.height, space.image.height);
            assert_eq!(actual.image.pixels, space.image.pixels);
        }
        assert_eq!(display_character('\u{a0}'), '\u{a0}');
    }
}

struct Arial {
    font: Font,
    em_ascent: f64,
}
impl Rasterizer {
    pub fn new() -> Self {
        Self::default()
    }
    fn face(&mut self, style: TextStyle) -> Result<&Arial> {
        let key = (style.family, style.weight, style.slant);
        if let std::collections::btree_map::Entry::Vacant(slot) = self.faces.entry(key) {
            let file = face_name(style);
            let directory = if cfg!(target_os = "linux") {
                "/usr/share/fonts/truetype/dejavu"
            } else {
                "C:/Windows/Fonts"
            };
            let bytes = std::fs::read(std::path::Path::new(directory).join(file))
                .map_err(|e| format!("installed face ({file}): {e}"))?;
            let font = Font::from_bytes(bytes, FontSettings::default())
                .map_err(|e| format!("font ({file}): {e}"))?;
            let metrics = font
                .horizontal_line_metrics(1.)
                .ok_or("font line metrics")?;
            let ascent = f64::from(metrics.ascent);
            let descent = f64::from(metrics.descent);
            if ascent <= descent {
                return Err("font line metrics".into());
            }
            slot.insert(Arial {
                font,
                em_ascent: ascent / (ascent - descent),
            });
        }
        Ok(&self.faces[&key])
    }
    pub fn measure(&mut self, text: &str, style: TextStyle) -> Result<TextMeasurement> {
        let face = self.face(style)?;
        let font = &face.font;
        let metrics = font
            .horizontal_line_metrics(style.size as f32)
            .ok_or("font line metrics")?;
        let size = style.size as f32 * 4.;
        let mut pen = 0_f32;
        let mut previous = None;
        let mut ascent = 0_f64;
        let mut descent = 0_f64;
        for c in text.chars().map(display_character) {
            if let Some(p) = previous
                && p != ' '
                && c != ' '
            {
                pen += font.horizontal_kern(p, c, size).unwrap_or(0.);
            }
            let m = font.metrics(c, size);
            ascent = ascent.max(f64::from(m.ymin + m.height as i32) / 4.);
            descent = descent.max(-f64::from(m.ymin) / 4.);
            pen += m.advance_width;
            previous = Some(c);
        }
        Ok(TextMeasurement {
            advance: f64::from(pen) / 4.,
            ascent,
            descent,
            font_ascent: f64::from(metrics.ascent),
            font_descent: -f64::from(metrics.descent),
        })
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
    #[cfg(test)]
    pub fn rasterize_limited(
        &mut self,
        text: &str,
        px: f64,
        weight: Weight,
        color: [u8; 3],
        max_width: Option<u32>,
    ) -> Result<Raster> {
        self.rasterize_styled(
            text,
            px,
            TextStyle {
                size: px,
                weight,
                family: Family::Arial,
                slant: Slant::Normal,
                color,
            },
            max_width,
        )
    }
    pub fn rasterize_styled(
        &mut self,
        text: &str,
        px: f64,
        style: TextStyle,
        max_width: Option<u32>,
    ) -> Result<Raster> {
        let color = style.color;
        let arial = self.face(style)?;
        let font = &arial.font;
        // Supersampling retains fractional advances and kerning without ClearType
        // color fringes in the renderer's transparent premultiplied textures.
        const SAMPLE: usize = 4;
        let size = px as f32 * SAMPLE as f32;
        let mut glyphs = Vec::new();
        let mut pen = 0_f32;
        let mut previous = None;
        let mut bounds = [0_i32; 4];
        for c in text.chars().map(display_character) {
            // Whitespace separates ordinary glyph pairs; NBSP keeps its glyph.
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
        if width > 8192 || height > super::MAX_OVERLAY_TEXT_EXTENT as usize {
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
        let gamma = coverage_gamma(color);
        let curve: [u8; 256] = std::array::from_fn(|c| adjusted_coverage(c as u8, gamma));
        for y in 0..height {
            for x in 0..width {
                let mut sum = 0_u32;
                for dy in 0..SAMPLE {
                    for dx in 0..SAMPLE {
                        sum += u32::from(mask[(y * SAMPLE + dy) * stride + x * SAMPLE + dx]);
                    }
                }
                // Adjust the final 4x4 average before premultiplication.
                let a = curve[((sum + 8) / 16) as usize];
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
            // Center the em square using the face's logical ascent/descent.
            middle: px * arial.em_ascent - px / 2.,
        })
    }
}

/// Real installed faces; bold Lucida requests deliberately use Arial Bold.
/// Missing selected files remain explicit errors. No font is bundled or synthesized.
#[cfg(not(target_os = "linux"))]
pub fn face_name(style: TextStyle) -> &'static str {
    match (style.family, style.weight, style.slant) {
        (Family::Lucida, Weight::Normal, _) => "l_10646.ttf",
        (_, Weight::Normal, Slant::Normal) => "arial.ttf",
        (_, Weight::Bold, Slant::Normal) => "arialbd.ttf",
        (_, Weight::Normal, Slant::Italic) => "ariali.ttf",
        (_, Weight::Bold, Slant::Italic) => "arialbi.ttf",
    }
}

#[cfg(target_os = "linux")]
pub fn face_name(style: TextStyle) -> &'static str {
    // CPU-only local-host CI still prepares real text resources. Linux uses
    // its installed DejaVu faces; this probe makes no Arial/pixel parity claim.
    match (style.weight, style.slant) {
        (Weight::Normal, Slant::Normal) => "DejaVuSans.ttf",
        (Weight::Bold, Slant::Normal) => "DejaVuSans-Bold.ttf",
        (Weight::Normal, Slant::Italic) => "DejaVuSans-Oblique.ttf",
        (Weight::Bold, Slant::Italic) => "DejaVuSans-BoldOblique.ttf",
    }
}
