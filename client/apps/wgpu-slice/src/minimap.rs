//! CSS-pixel minimap composition. The shared core owns all map and lifecycle state.
use crate::{
    Result,
    frame::{Draw, Frame},
    materials::RgbaImage,
    scene::Resource,
    text::{Align, Baseline, TextCache, TextRun, TextStyle, Weight},
};
use slither_client::{presentation::*, state::Point};

use crate::bank::MINIMAP_HANDLE_BASE as HANDLE;
/// Largest holder tiles, four marker strips, and the centred text/shadow tiles
/// (536 logical pixels at capped 3x, with blur support on either side).
pub const MAX_DRAWS: usize = (slither_client::protocol::minimap::MAX_MINIMAP_DIMENSION as usize
    + 24)
    .div_ceil(510)
    .pow(2)
    + 4
    + 4;

#[derive(Default)]
pub struct Minimap {
    background: Option<(u16, RgbaImage)>,
    pixels: Vec<u8>,
    tiles: Vec<(Draw, Resource)>,
    revision: u64,
}

fn over(dst: &mut [u8], src: &[u8], opacity: f64) {
    let alpha = f64::from(src[3]) / 255. * opacity;
    for i in 0..4 {
        dst[i] = (f64::from(src[i]) * opacity + f64::from(dst[i]) * (1. - alpha)).round() as u8;
    }
}

fn image(width: u32, height: u32) -> RgbaImage {
    RgbaImage {
        width,
        height,
        pixels: vec![0; width as usize * height as usize * 4],
    }
}

fn coverage(x: f64, y: f64, center: Point, radius: f64) -> f64 {
    let mut count = 0;
    for dy in 0..4 {
        for dx in 0..4 {
            let x = x + (f64::from(dx) + 0.5) / 4. - center.x;
            let y = y + (f64::from(dy) + 0.5) / 4. - center.y;
            count += u8::from(x * x + y * y <= radius * radius);
        }
    }
    f64::from(count) / 16.
}

fn background(plan: MinimapBackgroundPlan, side: u32) -> RgbaImage {
    let mut out = image(side, side);
    let n = side as usize;
    let mut mask = vec![0.; n * n];
    for y in 0..n {
        for x in 0..n {
            mask[y * n + x] = coverage(x as f64, y as f64, plan.center, plan.radius);
        }
    }
    // Match the existing material adapter's Canvas-shadow approximation.
    let sigma = plan.shadow_blur / 2.;
    let radius = (3. * sigma).ceil() as i32;
    let mut weights: Vec<_> = (-radius..=radius)
        .map(|k| (-0.5 * (f64::from(k) / sigma).powi(2)).exp())
        .collect();
    let sum: f64 = weights.iter().sum();
    weights.iter_mut().for_each(|v| *v /= sum);
    let mut horizontal = vec![0.; n * n];
    for y in 0..n {
        for x in 0..n {
            for (k, weight) in (-radius..=radius).zip(&weights) {
                let xx = x as i32 + k;
                let yy = y as i32 - plan.shadow_offset.y as i32;
                if (0..n as i32).contains(&xx) && (0..n as i32).contains(&yy) {
                    horizontal[y * n + x] += mask[yy as usize * n + xx as usize] * weight;
                }
            }
        }
    }
    for y in 0..n {
        for x in 0..n {
            let mut shadow = 0.;
            for (k, weight) in (-radius..=radius).zip(&weights) {
                let yy = y as i32 + k;
                if (0..n as i32).contains(&yy) {
                    shadow += horizontal[yy as usize * n + x] * weight;
                }
            }
            let dst = &mut out.pixels[(y * n + x) * 4..][..4];
            dst[3] = (shadow * 255.).round() as u8;
            let mut rgb = [0.; 3];
            let mut alpha: f64 = 0.;
            for dy in 0..4 {
                for dx in 0..4 {
                    let px = x as f64 + (f64::from(dx) + 0.5) / 4.;
                    let py = y as f64 + (f64::from(dy) + 0.5) / 4.;
                    if (px - plan.center.x).hypot(py - plan.center.y) <= plan.radius {
                        let quadrant = (px >= plan.center.x) == (py >= plan.center.y);
                        let stroke = (px - plan.center.x).abs() < plan.stroke_width / 2.
                            || (py - plan.center.y).abs() < plan.stroke_width / 2.;
                        let color = if stroke {
                            plan.stroke
                        } else if quadrant {
                            plan.quadrant_fill
                        } else {
                            plan.fill
                        };
                        for i in 0..3 {
                            rgb[i] += f64::from(color[i]) / 16.;
                        }
                        alpha += 255. / 16.;
                    }
                }
            }
            over(
                dst,
                &[
                    rgb[0].round() as u8,
                    rgb[1].round() as u8,
                    rgb[2].round() as u8,
                    alpha.round() as u8,
                ],
                1.,
            );
        }
    }
    out
}

fn marker(plan: MinimapMarkerPlan) -> RgbaImage {
    let mut out = image(plan.side as u32, plan.side as u32);
    for y in 0..out.height {
        for x in 0..out.width {
            let outer = coverage(
                f64::from(x),
                f64::from(y),
                plan.center,
                plan.radius + plan.stroke_width / 2.,
            );
            let inner = coverage(f64::from(x), f64::from(y), plan.center, plan.radius);
            let dst = &mut out.pixels[((y * out.width + x) * 4) as usize..][..4];
            dst[3] = (outer * 255.).round() as u8;
            over(dst, &[255; 4], inner);
        }
    }
    out
}

fn draw(asset: BodyAsset, destination: BodyRect, source: Option<BodyRect>, alpha: f64) -> Draw {
    Draw {
        asset,
        destination,
        source,
        repeat_uv: None,
        black_effect: false,
        translation: [0.; 2],
        rotation: 0.,
        effective_alpha: alpha,
        blend: BodyBlend::SourceOver,
        corners: None,
    }
}

fn resource(handle: u64, revision: u64, image: RgbaImage) -> Resource {
    Resource {
        asset: BodyAsset {
            handle,
            revision,
            width: image.width,
            height: image.height,
            status: BodyAssetStatus::Ready,
        },
        image,
    }
}

fn marker_strip(
    asset: BodyAsset,
    bounds: BodyRect,
    origin: [f64; 2],
    [left, top, right, bottom]: [f64; 4],
    alpha: f64,
) -> Option<Draw> {
    let x = (left - bounds.x).clamp(0., f64::from(asset.width));
    let y = (top - bounds.y).clamp(0., f64::from(asset.height));
    // Subtracting fractional CSS coordinates can overshoot the 14px image by
    // one ULP. Keep the source crop inside its texture and the scale at 1:1.
    let width = (right - left).clamp(0., f64::from(asset.width) - x);
    let height = (bottom - top).clamp(0., f64::from(asset.height) - y);
    (width > 0. && height > 0.).then(|| {
        draw(
            asset,
            BodyRect {
                x: left + origin[0],
                y: top + origin[1],
                width,
                height,
            },
            Some(BodyRect {
                x,
                y,
                width,
                height,
            }),
            alpha,
        )
    })
}

// Canvas puts a 14-pixel marker image at a fractional CSS position. Sample that
// image rather than rerasterizing the circle at its destination coordinates.
fn sample(image: &RgbaImage, x: f64, y: f64) -> [u8; 4] {
    let (ix, iy) = (x.floor() as i32, y.floor() as i32);
    let (fx, fy) = (x - x.floor(), y - y.floor());
    let mut out = [0.; 4];
    for (dx, wx) in [(0, 1. - fx), (1, fx)] {
        for (dy, wy) in [(0, 1. - fy), (1, fy)] {
            let (px, py) = (ix + dx, iy + dy);
            if (0..image.width as i32).contains(&px) && (0..image.height as i32).contains(&py) {
                let i = ((py as u32 * image.width + px as u32) * 4) as usize;
                for (c, v) in out.iter_mut().enumerate() {
                    *v += f64::from(image.pixels[i + c]) * wx * wy;
                }
            }
        }
    }
    out.map(|v| v.round() as u8)
}

impl Minimap {
    pub fn append(
        &mut self,
        client: &AnimatedClient,
        text: &mut TextCache,
        frame: &mut Frame,
        resources: &mut Vec<Resource>,
    ) -> Result<()> {
        if !client.minimap_tracking_enabled() {
            return Ok(());
        }
        let plan = client
            .minimap_render_plan(MinimapLayout::default())
            .map_err(|e| e.to_string())?;
        let Some(widget) = plan.widget else {
            return Ok(());
        };
        let alpha = widget.opacity.clamp(0., 1.);
        if alpha == 0. {
            return Ok(());
        }
        let side = widget.side as u32;
        let dimension = widget.images[0].dimension;
        if self
            .background
            .as_ref()
            .is_none_or(|(d, _)| *d != dimension)
        {
            self.background = Some((dimension, background(widget.background, side)));
        }
        let mut composed = self.background.as_ref().unwrap().1.clone();
        for pixel in composed.pixels.as_chunks_mut::<4>().0 {
            for c in pixel {
                *c = (f64::from(*c) * widget.background.opacity).round() as u8;
            }
        }
        for layer in widget.images {
            let n = u32::from(layer.dimension);
            for y in 0..n {
                for x in 0..n {
                    let src = &layer.rgba[((y * n + x) * 4) as usize..][..4];
                    let i = (((y + layer.top_left.y as u32) * side + x + layer.top_left.x as u32)
                        * 4) as usize;
                    over(&mut composed.pixels[i..i + 4], src, layer.opacity);
                }
            }
        }
        let m = widget.marker;
        let marker = marker(m);
        for y in (m.top_left.y.floor().max(0.) as u32)
            ..((m.top_left.y + m.side).ceil().clamp(0., f64::from(side)) as u32)
        {
            for x in (m.top_left.x.floor().max(0.) as u32)
                ..((m.top_left.x + m.side).ceil().clamp(0., f64::from(side)) as u32)
            {
                let src = sample(
                    &marker,
                    f64::from(x) - m.top_left.x,
                    f64::from(y) - m.top_left.y,
                );
                let i = ((y * side + x) * 4) as usize;
                over(&mut composed.pixels[i..i + 4], &src, m.opacity);
            }
        }
        if self.pixels != composed.pixels {
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or("minimap revision exhausted")?;
            self.pixels = composed.pixels;
            self.tiles.clear();
            // One-pixel gutters preserve linear sampling across the 512px source limit.
            for y in (0..side).step_by(510) {
                for x in (0..side).step_by(510) {
                    let (left, top) = (x.saturating_sub(1), y.saturating_sub(1));
                    let (right, bottom) = ((x + 511).min(side), (y + 511).min(side));
                    let mut tile = image(right - left, bottom - top);
                    for row in top..bottom {
                        let src = &self.pixels[((row * side + left) * 4) as usize
                            ..((row * side + right) * 4) as usize];
                        let dst = ((row - top) * tile.width * 4) as usize;
                        tile.pixels[dst..dst + src.len()].copy_from_slice(src);
                    }
                    let r = resource(HANDLE + self.tiles.len() as u64, self.revision, tile);
                    let destination = BodyRect {
                        x: f64::from(x),
                        y: f64::from(y),
                        width: f64::from((side - x).min(510)),
                        height: f64::from((side - y).min(510)),
                    };
                    let source = BodyRect {
                        x: f64::from(x - left),
                        y: f64::from(y - top),
                        ..destination
                    };
                    self.tiles
                        .push((draw(r.asset, destination, Some(source), 1.), r));
                }
            }
        }
        let origin = [
            f64::from(frame.extent[0]) / frame.viewport_scale - widget.right_bottom.x - widget.side,
            f64::from(frame.extent[1]) / frame.viewport_scale - widget.right_bottom.y - widget.side,
        ];
        for (d, r) in &self.tiles {
            let mut d = d.clone();
            d.translation = origin;
            d.effective_alpha = alpha;
            frame.draws.push(d);
            resources.push(r.clone());
        }
        // The DOM holder has visible overflow. Submit only the marker's outside
        // strips separately; its overlap was already flattened with the map.
        let marker = resource(HANDLE + 0x100, 1, marker);
        let bounds = BodyRect {
            x: m.top_left.x,
            y: m.top_left.y,
            width: m.side,
            height: m.side,
        };
        let end_x = bounds.x + bounds.width;
        let end_y = bounds.y + bounds.height;
        let s = f64::from(side);
        let strips = [
            (bounds.x, bounds.y, end_x, end_y.min(0.)),
            (bounds.x, bounds.y.max(s), end_x, end_y),
            (bounds.x, bounds.y.max(0.), end_x.min(0.), end_y.min(s)),
            (bounds.x.max(s), bounds.y.max(0.), end_x, end_y.min(s)),
        ];
        let mut outside = false;
        for (left, top, right, bottom) in strips {
            if let Some(command) = marker_strip(
                marker.asset,
                bounds,
                origin,
                [left, top, right, bottom],
                alpha * m.opacity,
            ) {
                frame.draws.push(command);
                outside = true;
            }
        }
        if outside {
            resources.push(marker);
        }
        self.append_server_label(&widget.label, origin, alpha, text, frame, resources)?;
        Ok(())
    }

    /// Submit the shared plain-text label after the map/marker, with the same
    /// group opacity and CSS hidden-overflow rectangle as the reference DOM.
    fn append_server_label(
        &mut self,
        label: &MinimapLabelPlan,
        origin: [f64; 2],
        holder_opacity: f64,
        text: &mut TextCache,
        frame: &mut Frame,
        resources: &mut Vec<Resource>,
    ) -> Result<()> {
        if label.text.is_empty() || holder_opacity <= 0. || label.opacity <= 0. {
            return Ok(());
        }
        let scale = frame.viewport_scale;
        let raster_scale = scale.min(crate::text::MAX_HUD_RASTER_SCALE);
        let anchor = label.text_anchor();
        let clip = BodyRect {
            x: origin[0] + label.top_left.x,
            y: origin[1] + label.top_left.y,
            width: label.size.x,
            height: label.size.y,
        };
        let viewport = BodyRect {
            x: 0.,
            y: 0.,
            width: f64::from(frame.extent[0]) / scale,
            height: f64::from(frame.extent[1]) / scale,
        };
        for (mut draw, resource) in text.draw_minimap(
            TextRun {
                text: &label.text,
                style: TextStyle {
                    family: slither_client::presentation::TextFamily::Arial,
                    slant: slither_client::presentation::TextSlant::Normal,
                    size: label.font_size,
                    weight: Weight::Normal,
                    color: label.fill,
                },
                position: [origin[0] + anchor.x, origin[1] + anchor.y],
                align: Align::Center,
                baseline: Baseline::Alphabetic,
                alpha: holder_opacity.clamp(0., 1.) * label.opacity.clamp(0., 1.),
            },
            raster_scale,
            [label.shadow_offset.x, label.shadow_offset.y],
            label.shadow_blur,
            label.size.x,
        )? {
            if frame.draws.len() >= crate::frame::MAX_DRAWS {
                break;
            }
            if (!label.overflow_hidden || crate::render::clip_text(&mut draw, clip))
                && crate::render::clip_text(&mut draw, viewport)
            {
                frame.draws.push(draw);
                resources.push(resource);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_marker_strip_does_not_exceed_its_texture() {
        let asset = resource(HANDLE + 0x100, 1, image(14, 14)).asset;
        for (bounds, strip) in [
            (
                BodyRect {
                    x: 2.1,
                    y: -1.3,
                    width: 14.,
                    height: 14.,
                },
                [2.1, -1.3, 16.1, 0.],
            ),
            (
                BodyRect {
                    x: -1.3,
                    y: 2.1,
                    width: 14.,
                    height: 14.,
                },
                [-1.3, 2.1, 0., 16.1],
            ),
            (
                BodyRect {
                    x: 2.1,
                    y: 31.3,
                    width: 14.,
                    height: 14.,
                },
                [2.1, 32., 16.1, 45.3],
            ),
            (
                BodyRect {
                    x: 31.3,
                    y: 2.1,
                    width: 14.,
                    height: 14.,
                },
                [32., 2.1, 45.3, 16.1],
            ),
        ] {
            let command = marker_strip(asset, bounds, [0.; 2], strip, 1.).unwrap();
            let frame = Frame {
                identity: 1,
                extent: [64, 64],
                viewport_scale: 1.,
                tick: 0,
                draws: vec![command],
                snakes: Vec::new(),
            };
            assert_eq!(
                frame.quads(&[(asset.into(), asset)].into()).unwrap().len(),
                1
            );
        }
    }

    #[cfg(windows)]
    fn label(text: &str) -> MinimapLabelPlan {
        MinimapLabelPlan {
            text: text.into(),
            top_left: Point { x: 0., y: 153. },
            size: Point { x: 160., y: 37. },
            font_size: 14.,
            font_family: crate::text::FONT_FAMILY,
            text_align: "center",
            overflow_hidden: true,
            fill: [255; 3],
            opacity: 0.5,
            shadow_offset: Point { x: 0., y: 1. },
            shadow_blur: 8.,
        }
    }

    #[cfg(windows)]
    fn frame(dpr: f64) -> Frame {
        Frame {
            identity: 1,
            tick: 0,
            extent: [(480. * dpr) as u32, (320. * dpr) as u32],
            viewport_scale: dpr,
            draws: vec![],
            snakes: vec![],
        }
    }

    #[cfg(windows)]
    #[test]
    fn label_matches_centred_platform_text_and_multiplies_holder_fades() {
        let mut map = Minimap::default();
        let mut cache = TextCache::default();
        let mut resources = vec![];
        let l = label("server 42");
        let mut previous = None;
        for opacity in [1., 0.025, 0.4, 0.] {
            let mut f = frame(1.);
            map.append_server_label(
                &l,
                [304., 144.],
                opacity,
                &mut cache,
                &mut f,
                &mut resources,
            )
            .unwrap();
            if opacity == 0. {
                assert!(f.draws.is_empty());
                continue;
            }
            assert!(!f.draws.is_empty());
            for d in &f.draws {
                assert_eq!(d.effective_alpha, 0.5 * opacity);
            }
            let assets: Vec<_> = f.draws.iter().map(|d| d.asset).collect();
            if let Some(old) = previous {
                assert_eq!(assets, old);
            }
            previous = Some(assets);
            let expected = cache
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
                        position: [384., 310.],
                        align: Align::Center,
                        baseline: Baseline::Alphabetic,
                        alpha: opacity * 0.5,
                    },
                    1.,
                    [0., 1.],
                    8.,
                    160.,
                )
                .unwrap();
            let mut draws = vec![];
            for (mut d, _) in expected {
                if crate::render::clip_text(
                    &mut d,
                    BodyRect {
                        x: 304.,
                        y: 297.,
                        width: 160.,
                        height: 37.,
                    },
                ) && crate::render::clip_text(
                    &mut d,
                    BodyRect {
                        x: 0.,
                        y: 0.,
                        width: 480.,
                        height: 320.,
                    },
                ) {
                    draws.push(d);
                }
            }
            assert_eq!(f.draws, draws);
        }
        let mut f = frame(1.);
        map.append_server_label(
            &label(""),
            [304., 144.],
            1.,
            &mut cache,
            &mut f,
            &mut resources,
        )
        .unwrap();
        assert!(f.draws.is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn overlong_label_clips_css_overflow_at_every_dpr() {
        let mut map = Minimap::default();
        let mut cache = TextCache::default();
        for dpr in [1., 2., 16., 1.25, 2.75] {
            let mut f = frame(dpr);
            let mut resources = vec![];
            let mut l = label(&"W".repeat(100));
            l.size.x = 32.;
            map.append_server_label(&l, [50.25, 20.5], 0.4, &mut cache, &mut f, &mut resources)
                .unwrap();
            assert!(!f.draws.is_empty());
            for d in &f.draws {
                assert!(d.destination.x >= 50.25);
                assert!(d.destination.x + d.destination.width <= 82.25);
                assert!(d.destination.y >= 173.5);
                assert!(d.destination.y + d.destination.height <= 210.5);
                assert_eq!(d.effective_alpha, 0.2);
            }
            assert!(
                resources.iter().any(|r| r
                    .image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[0] > 0)),
                "visible glyphs must survive the label crop"
            );
            f.quads(
                &resources
                    .iter()
                    .map(|r| (r.asset.into(), r.asset))
                    .collect(),
            )
            .unwrap();
        }
    }

    #[cfg(windows)]
    #[test]
    fn high_dpr_reuses_the_three_times_raster_without_changing_logical_placement() {
        let mut map = Minimap::default();
        let mut cache = TextCache::default();
        let mut previous = None;
        for dpr in [3., 16.] {
            let mut f = frame(dpr);
            let mut resources = vec![];
            map.append_server_label(
                &label("server 42"),
                [304., 144.],
                1.,
                &mut cache,
                &mut f,
                &mut resources,
            )
            .unwrap();
            let pixels: Vec<_> = resources
                .iter()
                .map(|r| (r.asset, r.image.pixels.clone()))
                .collect();
            if let Some((draws, old_pixels)) = previous {
                assert_eq!(f.draws, draws);
                assert_eq!(pixels, old_pixels);
            }
            previous = Some((f.draws, pixels));
        }
    }

    #[cfg(windows)]
    #[test]
    fn label_reserve_and_full_frame_degradation_keep_the_global_limit() {
        let mut map = Minimap::default();
        let mut cache = TextCache::default();
        let mut bank = crate::bank::Bank::new().unwrap();
        let filler = draw(
            bank.solid([255; 3]),
            BodyRect {
                x: 1.,
                y: 1.,
                width: 1.,
                height: 1.,
            },
            None,
            1.,
        );
        for count in [
            crate::frame::MAX_DRAWS - MAX_DRAWS - crate::hud::MAX_DRAWS,
            crate::frame::MAX_DRAWS - 1,
            crate::frame::MAX_DRAWS,
        ] {
            let mut f = frame(16.);
            f.draws = vec![filler.clone(); count];
            let mut resources = vec![];
            let mut l = label(&"W".repeat(100));
            l.size.x = 536.;
            map.append_server_label(&l, [0., 0.], 1., &mut cache, &mut f, &mut resources)
                .unwrap();
            assert!(f.draws.len() <= crate::frame::MAX_DRAWS);
            assert!(f.draws.len() - count <= 4);
            assert_eq!(&f.draws[..count], vec![filler.clone(); count]);
            if count < crate::frame::MAX_DRAWS {
                assert!(f.draws.len() > count);
            }
        }
    }
}
