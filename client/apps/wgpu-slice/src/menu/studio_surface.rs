//! Preserve the existing edge/shadow samples; stretch only the opaque flat interior.
use super::*;

fn interior(
    draw: &Draw,
    materials: &BTreeMap<MaterialKey, Resource>,
    tokens: SurfaceTokens,
    density: f64,
) -> bool {
    let Some(source) = draw.source else {
        return false;
    };
    let Some((MaterialKey::Control(atlas, scale, key_tokens, tile), _)) =
        materials.iter().find(|(_, r)| r.asset == draw.asset)
    else {
        return false;
    };
    if *scale != density.to_bits() || *key_tokens != tokens {
        return false;
    }
    let edge = (f64::from(tokens.radius) * density).ceil() + (12. * density).ceil();
    let pad = (12. * density).ceil();
    // These source rectangles (and their sampler neighbours) are well inside
    // the opaque fill. A mirrored half has the same flat colour. In particular,
    // do not stretch shadow strips: their repeated corner samples are observable.
    source.x + f64::from(tile[0]) >= edge
        && source.y + f64::from(tile[1]) >= edge
        && source.x + source.width + f64::from(tile[0]) <= f64::from(atlas[0]) + 2. * pad - edge
        && source.y + source.height + f64::from(tile[1]) <= f64::from(atlas[1]) + 2. * pad - edge
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_interior_reuses_resources_and_preserves_every_edge_record() {
        let old = BTreeMap::new();
        let art = BTreeMap::new();
        for density in [1., 1.25, 1.5, 2., 3., 4., 5.] {
            let make = || Composition {
                component: MenuComponent::StudioSurface,
                attribution: Default::default(),
                old: &old,
                old_art: &art,
                materials: BTreeMap::new(),
                next_handle: PAGE_HANDLE,
                spinner_revision: 0,
                scale: density,
                viewport_scale: density,
                draws: Vec::new(),
                art: BTreeMap::new(),
                labels: Vec::new(),
                wordmark: None,
                old_stroke: None,
                stroke: None,
            };
            for (w, h) in [(896., 476.), (328., 476.), (896., 328.), (28., 28.)] {
                for clipped in [false, true] {
                    let rect = MenuRect {
                        x: 16.,
                        y: 16.,
                        width: w,
                        height: h,
                    };
                    let clip = clipped.then_some(MenuRect {
                        x: 20.25,
                        y: 21.5,
                        width: w - 9.75,
                        height: h - 13.25,
                    });
                    let tokens = MenuTheme::default()
                        .surface(ControlStyle::StudioSurface, ControlState::Idle);
                    let mut before = make();
                    let mut after = make();
                    before.sliced(rect, tokens, 0.5, clip, 1.).unwrap();
                    after.studio_panel(rect, tokens, 0.5, clip, 1.).unwrap();
                    assert!(after.draws.len() <= before.draws.len());
                    if density >= 2. && w == 896. && h == 476. {
                        assert!(after.draws.len() < before.draws.len());
                    }
                    assert_eq!(after.attribution.total(), after.draws.len());
                    assert_eq!(
                        before.materials.keys().collect::<Vec<_>>(),
                        after.materials.keys().collect::<Vec<_>>()
                    );
                    for (key, resource) in &before.materials {
                        assert_eq!(resource.image.pixels, after.materials[key].image.pixels);
                    }
                    let edges = before
                        .draws
                        .iter()
                        .filter(|d| !interior(d, &before.materials, tokens, density))
                        .collect::<Vec<_>>();
                    let retained = after
                        .draws
                        .iter()
                        .filter(|d| !interior(d, &after.materials, tokens, density))
                        .collect::<Vec<_>>();
                    assert_eq!(edges, retained);
                }
            }
            let mut before = make();
            let mut after = make();
            let mut gradient =
                MenuTheme::default().surface(ControlStyle::StudioSurface, ControlState::Idle);
            gradient.colors[1] = [0; 3];
            let rect = MenuRect {
                x: 32.,
                y: 32.,
                width: 896.,
                height: 476.,
            };
            before.sliced(rect, gradient, 0.75, None, 1.).unwrap();
            after.studio_panel(rect, gradient, 0.75, None, 1.).unwrap();
            assert_eq!(
                before.draws, after.draws,
                "non-flat themes retain the original path"
            );
        }
    }
}

impl Composition<'_> {
    pub(super) fn studio_panel(
        &mut self,
        rect: MenuRect,
        tokens: SurfaceTokens,
        alpha: f64,
        clip: Option<MenuRect>,
        control_scale: f64,
    ) -> Result<()> {
        let start = self.draws.len();
        self.sliced(rect, tokens, alpha, clip, control_scale)?;
        if tokens.recipe != SurfaceRecipe::StudioPanel || tokens.colors[0] != tokens.colors[1] {
            return Ok(());
        }
        let density = self.scale * control_scale;
        let mut bounds = [i64::MAX, i64::MAX, i64::MIN, i64::MIN];
        let mut area = 0;
        let mut count = 0;
        let mut first = start;
        for (index, draw) in self.draws.iter().enumerate().skip(start) {
            if !interior(draw, &self.materials, tokens, density) {
                continue;
            }
            if count == 0 {
                first = index;
            }
            count += 1;
            let r = draw.destination;
            let b = [
                (r.x * self.scale).round() as i64,
                (r.y * self.scale).round() as i64,
                ((r.x + r.width) * self.scale).round() as i64,
                ((r.y + r.height) * self.scale).round() as i64,
            ];
            bounds = [
                bounds[0].min(b[0]),
                bounds[1].min(b[1]),
                bounds[2].max(b[2]),
                bounds[3].max(b[3]),
            ];
            area += (b[2] - b[0]) * (b[3] - b[1]);
        }
        // Keep the old path for small/unrepeated surfaces and any nonrectangular
        // clipped union. Never increase records or cover a hole in that union.
        if count < 2 || area != (bounds[2] - bounds[0]) * (bounds[3] - bounds[1]) {
            return Ok(());
        }
        let mut flat = self.draws[first].clone();
        let source = flat.source.as_mut().expect("interior source");
        *source = BodyRect {
            x: (source.x + source.width / 2.).floor(),
            y: (source.y + source.height / 2.).floor(),
            width: 1.,
            height: 1.,
        };
        flat.corners = None;
        flat.destination = BodyRect {
            x: bounds[0] as f64 / self.scale,
            y: bounds[1] as f64 / self.scale,
            width: (bounds[2] - bounds[0]) as f64 / self.scale,
            height: (bounds[3] - bounds[1]) as f64 / self.scale,
        };
        let materials = &self.materials;
        let mut index = 0;
        self.draws.retain_mut(|draw| {
            let keep = if index == first {
                *draw = flat.clone();
                true
            } else {
                index < start || !interior(draw, materials, tokens, density)
            };
            index += 1;
            keep
        });
        self.attribution.remove(self.component, count - 1);
        Ok(())
    }
}
