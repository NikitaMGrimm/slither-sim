//! Food-only raster refinement. Logical source sizes and all draw plans stay fixed.
use crate::{Result, materials::RgbaImage};
use slither_client::authored_sources::{CanvasRecipe, PaintCommand};

pub(crate) fn byte_len(mut width: u32, mut height: u32, minimum: u32) -> usize {
    let mut bytes = 0;
    loop {
        bytes += width as usize * height as usize * 4;
        if width == 1 && height == 1
            || (width / 2).max(1) < minimum
            || (height / 2).max(1) < minimum
        {
            return bytes;
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
}

pub(crate) fn materialize(recipe: &CanvasRecipe, scale: u32) -> Result<RgbaImage> {
    let mut refined = recipe.clone();
    refined.width *= scale;
    refined.height *= scale;
    for command in &mut refined.commands {
        match command {
            PaintCommand::RadialGradient { circles, .. } => {
                for value in circles {
                    *value *= f64::from(scale);
                }
            }
            PaintCommand::FillRect(rect) => {
                for value in rect {
                    *value *= f64::from(scale);
                }
            }
            _ => return Err("food gradient recipe".into()),
        }
    }
    let mut image = crate::materials::materialize(&refined)?;
    // A 2x2/1x1 body mip has equal ink in every texel, turning the circular
    // contour into a square across its unpadded quad. Keep at least four texels
    // for bodies; the already soft, padded glows can filter down to one.
    append_mips(&mut image, if scale > 1 { 4 } else { 1 });
    Ok(image)
}

fn append_mips(image: &mut RgbaImage, minimum: u32) {
    let (mut width, mut height) = (image.width, image.height);
    let mut offset = 0;
    image
        .pixels
        .reserve(byte_len(width, height, minimum) - image.pixels.len());
    while (width > 1 || height > 1)
        && (width / 2).max(1) >= minimum
        && (height / 2).max(1) >= minimum
    {
        let (w, h) = ((width / 2).max(1), (height / 2).max(1));
        let next = image.pixels.len();
        for y in 0..h {
            for x in 0..w {
                // Area weights include the last row/column of odd-sized sources.
                // Average premultiplied encoded components, like the existing sampler.
                let [left, right] = [x, x + 1].map(|v| f64::from(v * width) / f64::from(w));
                let [top, bottom] = [y, y + 1].map(|v| f64::from(v * height) / f64::from(h));
                let mut sum = [0.; 4];
                for sy in top.floor() as u32..bottom.ceil() as u32 {
                    for sx in left.floor() as u32..right.ceil() as u32 {
                        let area = (right.min(f64::from(sx + 1)) - left.max(f64::from(sx)))
                            * (bottom.min(f64::from(sy + 1)) - top.max(f64::from(sy)));
                        let pixel = offset + (sy as usize * width as usize + sx as usize) * 4;
                        for (c, value) in sum.iter_mut().enumerate() {
                            *value += f64::from(image.pixels[pixel + c]) * area;
                        }
                    }
                }
                let area = (right - left) * (bottom - top);
                image.pixels.extend(sum.map(|v| (v / area).round() as u8));
            }
        }
        offset = next;
        (width, height) = (w, h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magnified_small_food_has_a_round_rim_and_complete_mips() {
        let recipe = &slither_client::authored_sources::authored_source_recipes().food[0].bodies[0];
        let image = materialize(recipe, 4).unwrap();
        assert!(image.mip_level_count().unwrap() > 1);
        // Bilinear reconstruction along a circle should not acquire the source
        // grid's square lobes. This stresses the smallest body when magnified.
        let sample = |u: f64, v: f64| {
            let x = u * f64::from(image.width) - 0.5;
            let y = v * f64::from(image.height) - 0.5;
            let mut alpha = 0.;
            for dy in 0..2 {
                for dx in 0..2 {
                    let px = (x.floor() as i32 + dx).clamp(0, image.width as i32 - 1);
                    let py = (y.floor() as i32 + dy).clamp(0, image.height as i32 - 1);
                    let weight = if dx == 0 { 1. - x.fract() } else { x.fract() }
                        * if dy == 0 { 1. - y.fract() } else { y.fract() };
                    alpha += f64::from(
                        image.pixels[((py as u32 * image.width + px as u32) * 4 + 3) as usize],
                    ) * weight;
                }
            }
            alpha
        };
        let (mut min, mut max) = (255_f64, 0_f64);
        for i in 0..360 {
            let angle = f64::from(i) * std::f64::consts::TAU / 360.;
            let alpha = sample(0.5 + 0.45 * angle.cos(), 0.5 + 0.45 * angle.sin());
            min = min.min(alpha);
            max = max.max(alpha);
        }
        assert!(max - min < 16., "rim variation {min}..{max}");
        let mut malformed = image.clone();
        malformed.pixels.pop();
        assert!(malformed.mip_level_count().is_err());
        assert!(
            image
                .pixels
                .as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[..3].iter().all(|c| *c <= p[3]))
        );
    }

    #[test]
    fn odd_mips_include_every_premultiplied_texel() {
        let mut image = RgbaImage {
            width: 5,
            height: 3,
            pixels: vec![0; 5 * 3 * 4],
        };
        image.pixels[(5 * 3 - 1) * 4..].copy_from_slice(&[120, 60, 30, 240]);
        append_mips(&mut image, 1);
        assert_eq!(image.pixels.len(), byte_len(5, 3, 1));
        assert_eq!(&image.pixels[image.pixels.len() - 4..], &[8, 4, 2, 16]);
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
