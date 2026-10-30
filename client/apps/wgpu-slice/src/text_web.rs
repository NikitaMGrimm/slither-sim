//! Browser adapter: Canvas glyphs and metrics.
use super::{Raster, Slant, TextStyle, Weight};
use crate::{Result, materials::RgbaImage};
use slither_client::presentation::TextMeasurement;
use wasm_bindgen::JsCast;
use web_sys::{OffscreenCanvas, OffscreenCanvasRenderingContext2d};

#[derive(Default)]
pub struct Rasterizer {
    metrics: Option<(OffscreenCanvas, OffscreenCanvasRenderingContext2d)>,
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
                family: super::Family::Arial,
                slant: Slant::Normal,
                color,
            },
            max_width,
        )
    }
    pub fn measure(&mut self, text: &str, style: TextStyle) -> Result<TextMeasurement> {
        if self.metrics.is_none() {
            let canvas =
                OffscreenCanvas::new(1, 1).map_err(|e| format!("Canvas metrics: {e:?}"))?;
            let context = canvas
                .get_context("2d")
                .map_err(|e| format!("Canvas metrics: {e:?}"))?
                .ok_or("Canvas context")?
                .dyn_into()
                .map_err(|_| "Canvas context type")?;
            // Keep measurement state separate from rasterization, which resizes
            // its canvas and changes the baseline. Failed creation can retry.
            self.metrics = Some((canvas, context));
        }
        let (_, context) = self.metrics.as_ref().unwrap();
        context.set_font(&css_font(style, style.size));
        let m = context
            .measure_text(text)
            .map_err(|e| format!("Canvas metrics: {e:?}"))?;
        Ok(TextMeasurement {
            advance: m.width(),
            ascent: m.actual_bounding_box_ascent().max(0.),
            descent: m.actual_bounding_box_descent().max(0.),
            font_ascent: m.font_bounding_box_ascent(),
            font_descent: m.font_bounding_box_descent(),
        })
    }
    pub fn rasterize_styled(
        &mut self,
        text: &str,
        px: f64,
        style: TextStyle,
        max_width: Option<u32>,
    ) -> Result<Raster> {
        let color = style.color;
        let error = |e| format!("Canvas text: {e:?}");
        let canvas = OffscreenCanvas::new(1, 1).map_err(error)?;
        let context: OffscreenCanvasRenderingContext2d = canvas
            .get_context("2d")
            .map_err(error)?
            .ok_or("Canvas text context unavailable")?
            .dyn_into()
            .map_err(|e: js_sys::Object| error(e.into()))?;
        let font = css_font(style, px);
        context.set_font(&font);
        let metrics = context.measure_text(text).map_err(error)?;
        let advance = metrics.width();
        let left = (-metrics.actual_bounding_box_left()).floor() - 1.;
        let top = (-metrics.actual_bounding_box_ascent()).floor() - 1.;
        let width = (metrics.actual_bounding_box_right().ceil() - left + 1.).max(1.);
        let width = max_width.map_or(width, |limit| width.min(f64::from(limit) + 2.));
        let height = (metrics.actual_bounding_box_descent().ceil() - top + 1.).max(1.);
        context.set_text_baseline("middle");
        let middle = metrics.actual_bounding_box_ascent()
            - context
                .measure_text(text)
                .map_err(error)?
                .actual_bounding_box_ascent();
        if ![width, height, left, top, advance, middle]
            .into_iter()
            .all(f64::is_finite)
            || width > 8192.
            || height > 512.
        {
            return Err("text raster extent budget".into());
        }
        canvas.set_width(width as u32);
        canvas.set_height(height as u32);
        context.set_font(&font);
        context.set_text_baseline("alphabetic");
        let [r, g, b] = color;
        context.set_fill_style_str(&format!("#{r:02x}{g:02x}{b:02x}"));
        context.fill_text(text, -left, -top).map_err(error)?;
        let mut pixels = context
            .get_image_data(0., 0., width, height)
            .map_err(error)?
            .data()
            .0;
        // Canvas readback is straight RGBA; the shared texture convention is
        // encoded-sRGB premultiplied RGBA, including anti-aliased edge coverage.
        for p in pixels.as_chunks_mut::<4>().0 {
            for i in 0..3 {
                p[i] = ((u16::from(p[i]) * u16::from(p[3]) + 127) / 255) as u8;
            }
        }
        Ok(Raster {
            image: RgbaImage {
                width: width as u32,
                height: height as u32,
                pixels,
            },
            offset: [left, top],
            advance,
            middle,
        })
    }
}

fn css_font(style: TextStyle, px: f64) -> String {
    format!(
        "{} {} {px}px {}",
        if style.slant == Slant::Italic {
            "italic"
        } else {
            "normal"
        },
        if style.weight == Weight::Bold {
            "bold"
        } else {
            "normal"
        },
        style.family.css()
    )
}
