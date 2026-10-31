//! Camera offsets advance with draws rather than entity animation steps.

use super::{Point, PresentationError, finite};

/// Number of double-precision camera correction samples.
pub const CAMERA_SAMPLES: usize = 62;

/// Session-only wheel preference. Stock camera arithmetic stays untouched until
/// the first nonzero wheel input. These are authored UX limits, not server rules;
/// see docs/zoom.md. The arena endpoint follows resize/radius/camera changes.
#[derive(Clone, Debug, PartialEq)]
pub struct UserZoom {
    target: f64,
    current: f64,
    minimum: f64,
    whole_map: bool,
    active: bool,
}

impl Default for UserZoom {
    fn default() -> Self {
        Self {
            target: 1.,
            current: 1.,
            // No viewport has been prepared yet. Do not turn the first small
            // wheel-down into the whole-map endpoint before its bounds exist.
            minimum: 0.,
            whole_map: false,
            active: false,
        }
    }
}

impl UserZoom {
    /// Exactly 1.25 times the stock, snake-size-dependent scale.
    pub const MAX_FACTOR: f64 = 1.25;

    /// Positive notches zoom in; fractional notches preserve touchpad precision.
    pub fn wheel(&mut self, notches: f64) {
        if !notches.is_finite() || notches == 0. {
            return;
        }
        self.active = true;
        let base = if self.whole_map {
            self.minimum
        } else {
            self.target
        };
        self.target = (base * (notches * 1.12_f64.ln()).clamp(-700., 700.).exp())
            .clamp(self.minimum, Self::MAX_FACTOR);
        self.whole_map = self.target == self.minimum;
        // Reciprocal fractional wheel steps can leave a one-ULP residue.
        // Restore exact stock paint policy instead of entering zoom-out
        // paint policy for an imperceptible factor just below one.
        if !self.whole_map && (self.target - 1.).abs() < 1e-12 {
            self.target = 1.;
        }
    }

    /// Project once per draw. Clamp the absolute scale after multiplying stock
    /// zoom, so a growing snake cannot push the view through either endpoint.
    /// Include the camera offset to fit the circle while still following the snake.
    pub fn prepare(
        &mut self,
        stock: f64,
        viewport: [u32; 2],
        radius: f64,
        center: Point,
        camera: Point,
    ) -> f64 {
        // Inactive means no retained-state transition either: ordered Setup
        // draws and the stock explicit-camera API compare the whole camera.
        if !self.active {
            return stock;
        }
        let fit = (f64::from(viewport[0]) / (2. * (radius + (camera.x - center.x).abs())))
            .min(f64::from(viewport[1]) / (2. * (radius + (camera.y - center.y).abs())));
        let minimum = (fit / stock).min(1.);
        // A settled endpoint tracks a changing arena immediately, rather than
        // temporarily clipping it while interpolating from the old endpoint.
        if self.whole_map && self.current == self.minimum {
            self.current = minimum;
        }
        self.minimum = minimum;
        if self.target <= minimum {
            self.whole_map = true;
        }
        self.target = if self.whole_map {
            minimum
        } else {
            self.target.clamp(minimum, Self::MAX_FACTOR)
        };
        let delta = self.target.ln() - self.current.ln();
        self.current = if delta.abs() < 1e-7 {
            self.target
        } else {
            (self.current.ln() + delta * 0.18).exp()
        };
        self.current = self.current.clamp(minimum, Self::MAX_FACTOR);
        (stock * self.current).clamp(stock * minimum, stock * Self::MAX_FACTOR)
    }

    /// Whether projection should use the displayed transform, including offsets.
    pub fn active(&self) -> bool {
        self.active
    }
}

/// Additive camera offsets with one sample consumed per eligible draw.
///
/// The caller controls draw eligibility and following. Injection leaves the
/// current offset intact. Unlike entity rings, exhaustion has no cleanup state.
#[derive(Clone, Debug, PartialEq)]
pub struct CameraCorrections {
    weights: [f64; CAMERA_SAMPLES],
    samples: [[f64; CAMERA_SAMPLES]; 2],
    index: usize,
    pending: usize,
    offset: Point,
}

impl Default for CameraCorrections {
    fn default() -> Self {
        Self {
            weights: std::array::from_fn(|index| {
                let mut weight = 0.5
                    * (1.
                        - (std::f64::consts::PI * (CAMERA_SAMPLES - 1 - index) as f64
                            / (CAMERA_SAMPLES - 1) as f64)
                            .cos());
                weight += (0.5 * (1. - (std::f64::consts::PI * weight).cos()) - weight) * 0.5;
                weight
            }),
            samples: [[0.; CAMERA_SAMPLES]; 2],
            index: 0,
            pending: 0,
            offset: Point::default(),
        }
    }
}

impl CameraCorrections {
    /// Discard world offsets without moving the draw ring's retained cursor.
    pub(crate) fn clear(&mut self) {
        self.samples = [[0.; CAMERA_SAMPLES]; 2];
        self.pending = 0;
        self.offset = Point::default();
    }

    /// Add new-minus-old camera displacement without changing the current offset.
    /// Nonfinite inputs/results leave the entire ring unchanged.
    pub fn inject(&mut self, displacement: Point) -> Result<(), PresentationError> {
        finite(&[displacement.x, displacement.y])?;
        let mut samples = self.samples;
        for (channel, delta) in [displacement.x, displacement.y].into_iter().enumerate() {
            for (sample, weight) in self.weights.iter().enumerate() {
                let slot = (self.index + sample) % CAMERA_SAMPLES;
                samples[channel][slot] -= delta * weight;
                finite(&[samples[channel][slot]])?;
            }
        }
        self.samples = samples;
        self.pending = CAMERA_SAMPLES;
        Ok(())
    }

    /// Consume exactly one pending sample for a draw with a retained own snake.
    /// No elapsed-time or integer-step argument belongs to this clock.
    pub fn advance_draw(&mut self) {
        if self.pending == 0 {
            return;
        }
        self.pending -= 1;
        self.offset = Point {
            x: self.samples[0][self.index],
            y: self.samples[1][self.index],
        };
        self.samples[0][self.index] = 0.;
        self.samples[1][self.index] = 0.;
        self.index = (self.index + 1) % CAMERA_SAMPLES;
    }

    /// Current draw-sampled x/y offset in world coordinates.
    pub fn offset(&self) -> Point {
        self.offset
    }

    /// Next sample index, wrapping at 62.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Number of draws with pending samples; zero stays zero after exhaustion.
    pub fn pending(&self) -> usize {
        self.pending
    }

    /// Physical ring storage, x followed by y; samples remain double precision.
    pub fn samples(&self) -> &[[f64; CAMERA_SAMPLES]; 2] {
        &self.samples
    }

    /// Double-precision camera weights, including the additional curve shaping.
    pub fn weights(&self) -> &[f64; CAMERA_SAMPLES] {
        &self.weights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_zoom_properties_across_stock_scales_viewports_and_radius_changes() {
        for stock in [0.64285, 0.8, 1., super::super::INITIAL_ZOOM] {
            for viewport in [[960, 540], [540, 960], [300, 300], [3840, 2160]] {
                for camera in [
                    Point::default(),
                    Point {
                        x: 8000.,
                        y: -4000.,
                    },
                ] {
                    let mut z = UserZoom::default();
                    let prepare = |z: &mut UserZoom, radius| {
                        z.prepare(stock, viewport, radius, Point::default(), camera)
                    };
                    assert_eq!(prepare(&mut z, 16000.).to_bits(), stock.to_bits());
                    z.wheel(1000.);
                    let mut last = stock;
                    for _ in 0..160 {
                        let scale = prepare(&mut z, 16000.);
                        assert!(scale >= last && scale <= stock * UserZoom::MAX_FACTOR);
                        last = scale;
                    }
                    assert_eq!(last, stock * UserZoom::MAX_FACTOR);
                    z.wheel(-1000.);
                    for _ in 0..160 {
                        let scale = prepare(&mut z, 16000.);
                        assert!(scale <= last);
                        last = scale;
                    }
                    let fit = (f64::from(viewport[0]) / (2. * (16000. + camera.x.abs())))
                        .min(f64::from(viewport[1]) / (2. * (16000. + camera.y.abs())));
                    assert!((last - fit.min(stock)).abs() < 1e-12);
                    // A settled full-map endpoint must track growth, shrinkage
                    // and subsequent stock-size changes without another wheel event.
                    for radius in [16000., 24000., 500., 12000.] {
                        let scale = prepare(&mut z, radius);
                        assert!(
                            2. * (radius + camera.x.abs()) * scale <= f64::from(viewport[0]) + 1e-9
                        );
                        assert!(
                            2. * (radius + camera.y.abs()) * scale <= f64::from(viewport[1]) + 1e-9
                        );
                    }
                    for resized_stock in [0.64285, 1.15, 0.9] {
                        let scale =
                            z.prepare(resized_stock, [640, 1200], 24000., Point::default(), camera);
                        assert!(2. * (24000. + camera.x.abs()) * scale <= 640. + 1e-9);
                        assert!(2. * (24000. + camera.y.abs()) * scale <= 1200. + 1e-9);
                    }
                }
            }
        }
    }

    #[test]
    fn fractional_wheel_is_multiplicative_and_invalid_input_is_inert() {
        let mut one = UserZoom::default();
        one.prepare(1., [960, 540], 16000., Point::default(), Point::default());
        let mut quarters = one.clone();
        one.wheel(-1.);
        for _ in 0..4 {
            quarters.wheel(-0.25);
        }
        assert!((one.target - quarters.target).abs() < 1e-14);
        let before = one.clone();
        for invalid in [0., f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            one.wheel(invalid);
        }
        assert_eq!(one, before);
        let mut early = UserZoom::default();
        early.wheel(-1.);
        early.prepare(1., [960, 540], 16000., Point::default(), Point::default());
        assert!(!early.whole_map);
        assert!(early.target > 0.8);
    }

    #[test]
    fn camera_weights_preserve_endpoints_sum_and_monotonicity() {
        let ring = CameraCorrections::default();
        let weights = ring.weights();
        assert_eq!(weights.len(), 62);
        assert_eq!(weights[0], 1.);
        assert_eq!(weights[CAMERA_SAMPLES - 1], 0.);
        assert!(
            weights
                .iter()
                .all(|w| w.is_finite() && (0. ..=1.).contains(w))
        );
        assert!(weights.windows(2).all(|pair| pair[0] >= pair[1]));
        assert!((weights.iter().sum::<f64>() - 31.).abs() < 1e-12);
    }
}
