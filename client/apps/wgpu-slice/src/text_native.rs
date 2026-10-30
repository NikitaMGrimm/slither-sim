//! Native adapter: no bundled fonts, desktop APIs or window creation.
use super::{Family, Raster, Slant, TextStyle, Weight};
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
    font_ascent: f32,
    font_descent: f32,
    font_line_gap: f32,
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
            let face = ttf_parser::Face::parse(&bytes, 0)
                .map_err(|e| format!("font metrics ({file}): {e}"))?;
            let ascent = f64::from(face.typographic_ascender().unwrap_or(face.ascender()));
            let descent = f64::from(face.typographic_descender().unwrap_or(face.descender()));
            if ascent <= descent {
                return Err("font typographic metrics".into());
            }
            // These installed Windows faces use OS/2 usWinAscent/usWinDescent
            // for DirectWrite/Skia's font box, not OS/2's normalized typo box.
            // Source/rounding rationale: client/docs/panel-text.md#font-box-and-baselines.
            let os2 = face.tables().os2.ok_or("font OS/2 metrics")?;
            let upem = f32::from(face.units_per_em());
            let font_ascent = f32::from(os2.windows_ascender()) / upem;
            let font_descent = -f32::from(os2.windows_descender()) / upem;
            let font_line_gap = f32::from(face.line_gap()) / upem;
            slot.insert(Arial {
                font: Font::from_bytes(bytes, FontSettings::default())
                    .map_err(|e| format!("font ({file}): {e}"))?,
                em_ascent: ascent / (ascent - descent),
                font_ascent,
                font_descent,
                font_line_gap,
            });
        }
        Ok(&self.faces[&key])
    }
    pub fn normal_line_height(&mut self, style: TextStyle, scale: f64) -> Result<f64> {
        let device = TextStyle {
            size: f64::from(style.size as f32 * scale as f32),
            ..style
        };
        let m = self.measure("", device)?;
        // CSS normal includes the hhea line gap as well as the rounded font box.
        // Installed-face regression: tests/announcements.rs; docs/announcements.md.
        let gap = (device.size as f32 * self.face(device)?.font_line_gap).round();
        Ok((m.font_ascent.round() + m.font_descent.round() + f64::from(gap)) / scale)
    }
    pub fn measure(&mut self, text: &str, style: TextStyle) -> Result<TextMeasurement> {
        let face = self.face(style)?;
        let font = &face.font;
        let font_ascent = style.size as f32 * face.font_ascent;
        let font_descent = style.size as f32 * face.font_descent;
        // Blink's AscentDescentWithHacks rounds ordinary font boxes; Canvas
        // retains subpixels for tiny fonts (ascent <3 or total height <2).
        let tiny = font_ascent < 3. || font_ascent + font_descent < 2.;
        let size = style.size as f32 * 4.;
        let mut pen = 0_f32;
        let mut previous = None;
        let mut ascent = 0_f64;
        let mut descent = 0_f64;
        for c in text.chars() {
            if c == '\u{7f}' {
                pen += del_advance(style.size, style.weight) * 4.;
                previous = None;
                continue;
            }
            let Some(c) = canvas_character(c) else {
                previous = None;
                continue;
            };
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
            font_ascent: f64::from(if tiny {
                font_ascent
            } else {
                font_ascent.round()
            }),
            font_descent: f64::from(if tiny {
                font_descent
            } else {
                font_descent.round()
            }),
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
        let weight = style.weight;
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
            // Canvas middle uses the em square, not hhea's taller line box.
            // Normalize OS/2 typography to one em, at the browser's 1/64px grid.
            middle: (px * arial.em_ascent * 64.).round() / 64. - px / 2.,
        })
    }
}

/// Keep styled Lucida in the matched family, using its regular installed face.
/// Known non-parity: Chromium/Skia synthesizes bold/oblique; synthesis or an
/// accepted approximation belongs to end-screen backlog item 5, not this PR.
/// https://github.com/chromium/chromium/blob/a7453b805045ac88d29dfee5262913dcdad7508f/third_party/blink/renderer/platform/fonts/win/font_cache_skia_win.cc#L484-L498
/// Missing selected files are explicit errors; no fonts are bundled or sheared.
#[cfg(not(target_os = "linux"))]
pub fn face_name(style: TextStyle) -> &'static str {
    match (style.family, style.weight, style.slant) {
        (Family::Lucida, _, _) => "l_10646.ttf",
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
