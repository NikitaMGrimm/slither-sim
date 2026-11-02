//! Portable work estimates for optional full-screen food passes.

#[derive(Clone, Copy, Default)]
pub(super) struct LayerHistory {
    pub extent: [u32; 2],
    pub samples: u32,
    pub outline: bool,
    pub glow: bool,
    pub glow_bins: [bool; 4],
}

/// Coverage is in attachment screen areas, including repeated overlapping quads.
/// Count one unit per texture lookup or color-sample blend. This is a conservative
/// selection policy, not a calibrated GPU timer. A 25% entry margin prevents
/// oscillation while the previously selected path still pays its fixed work.
pub(super) fn pays(saved: f64, fixed: f64, active: bool) -> bool {
    saved > fixed * if active { 1. } else { 1.25 }
}

pub(super) fn select_glows(coverage: [f64; 4], samples: u32, previous: LayerHistory) -> [bool; 4] {
    let samples = f64::from(samples);
    let mut saved = 0.;
    // Retain the pre-hoisting admission reserve. Lowering it would change which
    // pictures use reconstructed glows; this optimization preserves that choice.
    let mut fixed = 2. * samples;
    let mut bins = [false; 4];
    for (bin, area) in coverage.into_iter().enumerate().skip(1) {
        if area == 0. {
            continue;
        }
        let reduction = 1. / (1_u32 << (2 * bin)) as f64;
        let saving = area * (1. + samples - 2. * reduction);
        // One/five reconstruction lookups, blending at the world sample count,
        // and clearing/storing the reduced attachment.
        let cost = if bin == 3 { 5. } else { 1. } + samples + 2. * reduction;
        if pays(saving, cost, previous.glow_bins[bin]) {
            bins[bin] = true;
            saved += saving;
            fixed += cost;
        }
    }
    if pays(saved, fixed, previous.glow) {
        bins
    } else {
        [false; 4]
    }
}

/// Actual clipped triangle area, normalized to the [-1,1] attachment square.
/// Food normally takes the rectangle fast path; rotated/cropped public inputs
/// use bounded stack polygon clipping rather than an overestimating bounding box.
pub(super) fn covered_area(vertices: &[[f32; 5]; 6]) -> f64 {
    let [a, b, c, _, _, d] = *vertices;
    if a[0] == d[0] && b[0] == c[0] && a[1] == b[1] && c[1] == d[1] {
        let width =
            (f64::from(a[0].max(b[0])).min(1.) - f64::from(a[0].min(b[0])).max(-1.)).max(0.);
        let height =
            (f64::from(a[1].max(d[1])).min(1.) - f64::from(a[1].min(d[1])).max(-1.)).max(0.);
        return width * height / 4.;
    }
    let mut area = 0.;
    for triangle in vertices.as_chunks::<3>().0 {
        let mut polygon = [[0.; 2]; 8];
        for (p, v) in polygon.iter_mut().zip(triangle) {
            *p = [f64::from(v[0]), f64::from(v[1])];
        }
        let mut count = 3;
        for (axis, sign) in [(0, 1.), (0, -1.), (1, 1.), (1, -1.)] {
            if count == 0 {
                break;
            }
            let old = polygon;
            let mut a = old[count - 1];
            let previous_count = count;
            count = 0;
            for b in old.into_iter().take(previous_count) {
                let da = 1. - sign * a[axis];
                let db = 1. - sign * b[axis];
                if (da >= 0.) != (db >= 0.) {
                    let t = da / (da - db);
                    polygon[count] = [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
                    count += 1;
                }
                if db >= 0. {
                    polygon[count] = b;
                    count += 1;
                }
                a = b;
            }
        }
        if count >= 3 {
            let mut sum = 0.;
            for i in 0..count {
                let a = polygon[i];
                let b = polygon[(i + 1) % count];
                sum += a[0] * b[1] - b[0] * a[1];
            }
            area += sum.abs() / 8.;
        }
    }
    area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipping_counts_covered_work_and_hysteresis_retains_only_profitable_paths() {
        let quad = |points: [[f32; 2]; 4]| {
            [0, 1, 2, 0, 2, 3].map(|i| [points[i][0], points[i][1], 0., 0., 1.])
        };
        assert_eq!(
            covered_area(&quad([[-2., 2.], [2., 2.], [2., -2.], [-2., -2.]])),
            1.
        );
        assert_eq!(
            covered_area(&quad([[2., 1.], [3., 1.], [3., -1.], [2., -1.]])),
            0.
        );
        assert_eq!(
            covered_area(&quad([[-2., 0.], [0., 2.], [2., 0.], [0., -2.]])),
            1.
        );
        assert_eq!(
            covered_area(&quad([[-1., 0.], [0., 1.], [1., 0.], [0., -1.]])),
            0.5
        );
        assert!(!pays(1.1, 1., false));
        assert!(pays(1.1, 1., true));
        assert!(pays(1.26, 1., false));
        assert!(!pays(0.99, 1., true));
        assert_eq!(
            select_glows([0., 0., 0., 0.01], 1, LayerHistory::default()),
            [false; 4]
        );
        assert_eq!(
            select_glows([0., 0.01, 0., 20.], 4, LayerHistory::default()),
            [false, false, false, true]
        );
    }
}
