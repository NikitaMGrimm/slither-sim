//! Owner-approved continuous zoom policy; no protocol, camera or clock changes.
//! Rules and resource limits: docs/zoom.md.
use super::{BodyPaintPass, BodyPaintPlan, FoodImageDraw, FoodPaintError, FoodSource};
use crate::state::Point;

/// Blend new downsampling from factor one through one ordinary zoom-out notch.
pub fn continuous_zoom_weight(factor: f64) -> f64 {
    if factor >= 1. {
        0.
    } else if factor <= 0.9 {
        1.
    } else {
        smooth((1. - factor) / 0.1)
    }
}

fn smooth(t: f64) -> f64 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// Continuous size fade, with the stock picture retained exactly at factor one.
pub fn zoom_size_fade(size: f64, low: f64, high: f64, factor: f64) -> f64 {
    let activation = continuous_zoom_weight(factor);
    1. - activation * (1. - smooth((size - low) / (high - low)))
}

/// Floor each image's own extent at one displayed physical pixel. Returned alpha
/// is the exact rectangle-area ratio, not a body-dependent glow eligibility test.
/// The stock overload, including its center/half-size convention, stays exact at 1.
pub fn continuous_food_image(
    source: FoodSource,
    center: Point,
    scale: f64,
    factor: f64,
    physical_pixels_per_unit: f64,
) -> Result<(FoodImageDraw, f64), FoodPaintError> {
    if !physical_pixels_per_unit.is_finite() || physical_pixels_per_unit <= 0. {
        return Err(FoodPaintError::InvalidView);
    }
    let draw = super::food_image_draw(source, center, scale, factor)?;
    if factor >= 1. {
        return Ok((draw, 1.));
    }
    let FoodImageDraw::Scaled {
        source,
        mut destination,
    } = draw
    else {
        unreachable!("non-stock zoom uses the explicit overload")
    };
    let activation = continuous_zoom_weight(factor);
    let floor = 1. / physical_pixels_per_unit;
    let width = destination.width + activation * (floor - destination.width).max(0.);
    let height = destination.height + activation * (floor - destination.height).max(0.);
    let area = (destination.width / width) * (destination.height / height);
    destination.x -= (width - destination.width) / 2.;
    destination.y -= (height - destination.height) / 2.;
    destination.width = width;
    destination.height = height;
    Ok((
        FoodImageDraw::Scaled {
            source,
            destination,
        },
        area,
    ))
}

/// Fade only the unresolved snake decoration; body/boost/death geometry and the
/// painter's already computed scratch cursor remain intact. Drop exact-zero
/// draws only after all visits, preserving the antenna/death insertion barrier.
pub fn fade_body_zoom_details(plan: &mut BodyPaintPlan, diameter: f64, factor: f64) {
    if factor >= 1. {
        return;
    }
    let detail = zoom_size_fade(diameter, 1., 3., factor);
    let shadow = zoom_size_fade(diameter, 2., 6., factor);
    let old_barrier = plan.antenna_barrier;
    let mut barrier = 0;
    let mut ordinal = 0;
    plan.commands.retain_mut(|c| {
        let weight = match c.pass {
            BodyPaintPass::Outline
            | BodyPaintPass::TailOutline
            | BodyPaintPass::NearHeadShadow
            | BodyPaintPass::TailShadow
            | BodyPaintPass::InnerBoost => detail,
            BodyPaintPass::BodyShadow => shadow,
            _ => 1.,
        };
        if weight == 0. {
            return false;
        }
        if c.sequence < old_barrier {
            barrier += 1;
        }
        c.sequence = ordinal;
        ordinal += 1;
        if weight != 1. {
            c.alpha.requested *= weight;
            c.alpha.effective *= weight;
        }
        true
    });
    plan.antenna_barrier = barrier;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::{BodyAsset, BodyAssetStatus};
    fn source(width: u32) -> FoodSource {
        FoodSource::new(
            BodyAsset {
                handle: 1,
                revision: 1,
                width,
                height: width,
                status: BodyAssetStatus::Ready,
            },
            width as f64,
            width as f64,
            width as f64 / 2.,
            width as f64 / 2.,
        )
        .unwrap()
    }
    #[test]
    fn own_extent_floor_preserves_area_and_stock_overload() {
        let c = Point { x: 31., y: 17. };
        for width in [1, 7, 32, 180] {
            let s = source(width);
            assert_eq!(
                continuous_food_image(s, c, 1., 1., 2.56).unwrap(),
                (super::super::food_image_draw(s, c, 1., 1.).unwrap(), 1.)
            );
            for factor in [0.999999, 0.9, 0.065, 0.001] {
                for density in [1., 2.56, 4.] {
                    let (draw, alpha) = continuous_food_image(s, c, 0.75, factor, density).unwrap();
                    let FoodImageDraw::Scaled { destination: d, .. } = draw else {
                        panic!()
                    };
                    let true_side = width as f64 * 0.75 * factor;
                    assert!((d.width * d.height * alpha - true_side * true_side).abs() < 1e-10);
                    if factor <= 0.9 {
                        assert!(d.width * density >= 1. - 1e-12);
                    }
                }
            }
        }
        // A tiny body never controls a broader glow's rectangle or its opacity.
        let (_, body) = continuous_food_image(source(7), c, 1., 0.065, 1.).unwrap();
        let (_, glow) = continuous_food_image(source(180), c, 1., 0.065, 1.).unwrap();
        assert!(body < 1.);
        assert_eq!(glow, 1.);
    }
    #[test]
    fn body_fade_keeps_sample_identity_cursor_and_antenna_barrier() {
        use super::super::{BodyAlpha, BodyBlend, BodyImageCommand, BodyRect};
        let asset = source(16).asset();
        let commands = [
            BodyPaintPass::Outline,
            BodyPaintPass::Body,
            BodyPaintPass::BodyShadow,
            BodyPaintPass::Death,
        ]
        .into_iter()
        .enumerate()
        .map(|(sequence, pass)| BodyImageCommand {
            sequence,
            ordinal: 100 + sequence,
            pass,
            asset_index: 0,
            asset,
            source_rect: None,
            center: Point { x: 0., y: 0. },
            translation: Point { x: 0., y: 0. },
            rotation: None,
            destination: BodyRect {
                x: 0.,
                y: 0.,
                width: 16.,
                height: 16.,
            },
            alpha: BodyAlpha {
                requested: 0.7,
                effective: 0.7,
                accepted: true,
            },
            blend: BodyBlend::SourceOver,
        })
        .collect();
        let original = BodyPaintPlan {
            commands,
            antenna_barrier: 3,
            final_distance_cursor: Point { x: 9., y: 4. },
        };
        let mut stock = original.clone();
        fade_body_zoom_details(&mut stock, 0.1, 1.);
        assert_eq!(stock, original);
        let mut small = original.clone();
        fade_body_zoom_details(&mut small, 0.5, 0.065);
        assert_eq!(small.antenna_barrier, 1);
        assert_eq!(small.final_distance_cursor, original.final_distance_cursor);
        assert_eq!(
            small.commands.iter().map(|c| c.ordinal).collect::<Vec<_>>(),
            vec![101, 103]
        );
        assert_eq!(
            small
                .commands
                .iter()
                .map(|c| c.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(small.commands[0].pass, BodyPaintPass::Body);
        assert_eq!(small.commands[0].alpha.effective, 0.7);
    }
    #[test]
    fn zoom_fades_are_continuous_and_exact_at_stock() {
        for size in [0., 0.5, 1., 2., 3., 4., 6., 20.] {
            assert_eq!(zoom_size_fade(size, 1., 3., 1.).to_bits(), 1_f64.to_bits());
            let mut previous = zoom_size_fade(size, 1., 3., 1.);
            for i in 1..=2000 {
                let current = zoom_size_fade(size, 1., 3., 1. - f64::from(i) * 0.00001);
                assert!((current - previous).abs() < 0.001);
                previous = current;
            }
        }
        for center in [1., 3., 2., 6.] {
            let a = zoom_size_fade(center - 1e-8, 1., 3., 0.065);
            let b = zoom_size_fade(center + 1e-8, 1., 3., 0.065);
            assert!((a - b).abs() < 1e-7);
        }
    }
}
