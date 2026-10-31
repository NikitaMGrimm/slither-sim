//! Hollow rounded focus paint; three stretched strips share one bounded half-raster.
use super::*;

fn raster(
    cap: u32,
    height: u32,
    radius: f64,
    stroke: f64,
    rgb: [u8; 3],
    row: u32,
) -> materials::RgbaImage {
    let width = cap + 2;
    let rows = 512.min(height - row);
    let full_width = f64::from(2 * cap + 3);
    let mut pixels = Vec::with_capacity((width * rows * 4) as usize);
    for y in row..row + rows {
        for x in 0..width {
            let qx = (f64::from(x) + 0.5 - full_width / 2.).abs() - (full_width / 2. - radius);
            let qy = (f64::from(y) + 0.5 - f64::from(height) / 2.).abs()
                - (f64::from(height) / 2. - radius);
            let distance = qx.max(0.).hypot(qy.max(0.)) + qx.max(qy).min(0.) - radius;
            let coverage = (0.5 - distance).clamp(0., 1.) - (0.5 - distance - stroke).clamp(0., 1.);
            pixels.extend(rgb.map(|c| (f64::from(c) * coverage).round() as u8));
            pixels.push((255. * coverage).round() as u8);
        }
    }
    materials::RgbaImage {
        width,
        height: rows,
        pixels,
    }
}

impl Composition<'_> {
    pub(super) fn outline_control(
        &mut self,
        r: &ResolvedControl,
        alpha: f64,
        theme: MenuTheme,
    ) -> Result<()> {
        let tokens = theme.surface(r.control.style, ControlState::Idle);
        let radius = if matches!(
            r.control.kind,
            ControlKind::Swatch { .. } | ControlKind::CosmeticCell { .. }
        ) {
            // Builder cells share action text tokens, but their painted tiles use cell corners.
            f64::from(
                theme
                    .surface(ControlStyle::SelectedCell, ControlState::Idle)
                    .radius,
            ) * r.scale
        } else if tokens.recipe == SurfaceRecipe::StudioButton {
            r.rect.height / 2.
        } else {
            f64::from(tokens.radius) * r.scale
        };
        self.outline(r, r.rect, radius, alpha, theme)
    }
    pub(super) fn outline(
        &mut self,
        r: &ResolvedControl,
        rect: MenuRect,
        radius: f64,
        alpha: f64,
        theme: MenuTheme,
    ) -> Result<()> {
        if alpha == 0. || rect.width <= 0. || rect.height <= 0. {
            return Ok(());
        }
        let width = (rect.width * self.scale).round().max(1.) as u32;
        let height = (rect.height * self.scale).round().max(1.) as u32;
        let radius = (radius * self.scale)
            .ceil()
            .min(f64::from(width.min(height)) / 2.);
        let cap = radius.ceil() as u32 + 1;
        if cap + 2 > 512 || height > 2048 {
            return Err(MENU_RASTER_AXIS_QUOTA.into());
        }
        let stroke = self.scale;
        let cut = f64::from(cap).min(f64::from(width) / 2.) / self.scale;
        for row in (0..height).step_by(512) {
            let resource = self.cached_material(
                MaterialKey::Focus(
                    [cap, height],
                    radius.to_bits(),
                    stroke.to_bits(),
                    theme.text,
                    row,
                ),
                || Ok(raster(cap, height, radius, stroke, theme.text, row)),
            )?;
            for (x, width, source_x, source_width, mirrored) in [
                (rect.x, cut, 0., f64::from(cap), false),
                (
                    rect.x + cut,
                    (rect.width - 2. * cut).max(0.),
                    f64::from(cap + 1),
                    1.,
                    false,
                ),
                (rect.x + rect.width - cut, cut, 0., f64::from(cap), true),
            ] {
                if width <= 0. {
                    continue;
                }
                let mut draw = image(
                    &resource,
                    MenuRect {
                        x,
                        y: rect.y + f64::from(row) / self.scale,
                        width,
                        height: f64::from(resource.asset.height) / self.scale,
                    },
                    alpha,
                );
                draw.source = Some(BodyRect {
                    x: source_x,
                    y: 0.,
                    width: source_width,
                    height: f64::from(resource.asset.height),
                });
                if !clip_scaled_draw(&mut draw, r.clip, self.scale) {
                    continue;
                }
                if mirrored {
                    let source = draw.source.as_mut().expect("outline crop");
                    source.x = (source_width - source.x - source.width).max(0.);
                    let d = draw.destination;
                    draw.corners = Some([
                        [d.x + d.width, d.y],
                        [d.x, d.y],
                        [d.x, d.y + d.height],
                        [d.x + d.width, d.y + d.height],
                    ]);
                }
                self.push(draw)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn composition<'a>(
        old: &'a BTreeMap<MaterialKey, Resource>,
        old_art: &'a BTreeMap<(frame::ResourceKey, [u32; 2]), Resource>,
        scale: f64,
    ) -> Composition<'a> {
        Composition {
            component: Default::default(),
            attribution: Default::default(),
            old,
            old_art,
            materials: BTreeMap::new(),
            next_handle: PAGE_HANDLE + 100,
            spinner_revision: 0,
            scale,
            viewport_scale: scale,
            draws: Vec::new(),
            art: BTreeMap::new(),
            labels: Vec::new(),
            wordmark: None,
            old_stroke: None,
            stroke: None,
        }
    }
    fn control() -> ResolvedControl {
        use slither_client::menu_controls::{Control, ControlId, Placement};
        let rect = MenuRect {
            x: 17.3,
            y: 21.7,
            width: 108.,
            height: 40.,
        };
        ResolvedControl {
            control: Control {
                id: ControlId(1),
                placement: Placement::rect(0., 0., 108., 40.),
                clip: None,
                visible: true,
                enabled: true,
                alpha: 1.,
                name: "Focus".into(),
                tab_order: Some(0),
                style: ControlStyle::StudioAction,
                kind: ControlKind::Button {
                    label: "Focus".into(),
                },
            },
            rect,
            clip: MenuRect {
                x: 20.,
                y: 24.,
                width: 90.,
                height: 32.,
            },
            scale: 1.,
        }
    }
    #[test]
    fn outlines_clip_mirrors_preserve_alpha_and_reuse_immutable_rasters_at_every_density() {
        for dpr in [1_f64, 1.25, 1.5, 1.75, 2., 3., 4., 5., 6., 7., 8.] {
            let empty = BTreeMap::new();
            let art = BTreeMap::new();
            let mut c = composition(&empty, &art, dpr.min(MAX_MENU_RASTER_DENSITY));
            let r = control();
            c.outline_control(&r, 0.37, MenuTheme::default()).unwrap();
            assert!(c.draws.len() <= 3);
            assert_eq!(c.materials.len(), 1);
            for draw in &c.draws {
                assert_eq!(draw.effective_alpha, 0.37);
                let source = draw.source.unwrap();
                assert!(source.x >= 0. && source.x + source.width <= draw.asset.width as f64);
                assert!(source.y >= 0. && source.y + source.height <= draw.asset.height as f64);
            }
            let resident = c
                .materials
                .values()
                .map(|r| (r.asset.into(), r.asset))
                .collect();
            Frame {
                identity: 1,
                extent: [1920, 1080],
                viewport_scale: dpr,
                tick: 0,
                draws: c.draws.clone(),
                snakes: vec![],
            }
            .validate(&resident)
            .unwrap();
            let old = c.materials.values().next().unwrap().image.clone();
            let mut next = composition(&c.materials, &art, c.scale);
            next.outline_control(&r, 0.8, MenuTheme::default()).unwrap();
            assert!(std::sync::Arc::ptr_eq(
                &old,
                &next.materials.values().next().unwrap().image
            ));
        }
    }
    #[test]
    fn oversized_outline_rejects_before_material_or_draw_publication() {
        let empty = BTreeMap::new();
        let art = BTreeMap::new();
        let mut c = composition(&empty, &art, 5.);
        let mut r = control();
        r.rect.height = 1000.;
        assert_eq!(
            c.outline_control(&r, 1., MenuTheme::default()).unwrap_err(),
            MENU_RASTER_AXIS_QUOTA
        );
        assert!(c.materials.is_empty() && c.draws.is_empty());
    }
    #[test]
    fn builder_swatch_uses_painted_cell_corners_despite_action_text_tokens() {
        let empty = BTreeMap::new();
        let art = BTreeMap::new();
        for density in [1_f64, 1.25, 1.5, 2., 5.] {
            let mut r = control();
            r.control.kind = ControlKind::Swatch {
                palette: 1,
                count: None,
                selected: true,
            };
            let mut c = composition(&empty, &art, density);
            c.outline_control(&r, 1., MenuTheme::default()).unwrap();
            let Some(MaterialKey::Focus(_, radius, ..)) = c.materials.keys().next() else {
                panic!("missing outline")
            };
            assert_eq!(f64::from_bits(*radius), (4. * density).ceil());
        }
    }
    #[test]
    fn focus_rasters_follow_the_radius_keep_centres_clear_and_stay_premultiplied() {
        for dpr in [1_f64, 1.25, 1.5, 1.75, 2., 3., 4., 5., 6., 7., 8.] {
            let density = dpr.min(MAX_MENU_RASTER_DENSITY);
            for logical_radius in [4_f64, 14., 24., 44.] {
                let radius = (logical_radius * density).ceil();
                let cap = radius as u32 + 1;
                let height = (2. * radius + 8. * density).round() as u32;
                let image = raster(cap, height, radius, density, [224, 224, 255], 0);
                let alpha = |x: u32, y: u32| image.pixels[((y * image.width + x) * 4 + 3) as usize];
                assert_eq!(alpha(0, 0), 0);
                assert_eq!(alpha(0, height - 1), 0);
                assert_eq!(alpha(cap + 1, height / 2), 0);
                assert!(alpha(0, height / 2) > 250);
                assert!(alpha(cap + 1, 0) > 250);
                assert!(
                    image
                        .pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|p| p[..3].iter().all(|c| *c <= p[3]))
                );
            }
        }
    }
}
