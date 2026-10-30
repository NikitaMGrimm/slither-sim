//! Camera offsets advance with draws rather than entity animation steps.

use super::{Point, PresentationError, finite};

/// Number of double-precision camera correction samples.
pub const CAMERA_SAMPLES: usize = 62;

// Chromium Math.cos values, as evaluated by the official browser client. A table
// avoids per-platform libm differences; method: docs/camera-weights.md.
// For i in 0..62: w = 0.5 * (1 - cos(PI * (61 - i) / 61));
//                w += (0.5 * (1 - cos(PI * w)) - w) * 0.5.
const CAMERA_WEIGHTS: [f64; CAMERA_SAMPLES] = [
    f64::from_bits(0x3ff0000000000000),
    f64::from_bits(0x3feffd47b440cf08),
    f64::from_bits(0x3feff51309e321f0),
    f64::from_bits(0x3fefe73ed90503c9),
    f64::from_bits(0x3fefd391230c0f65),
    f64::from_bits(0x3fefb9b9fdbbca44),
    f64::from_bits(0x3fef9954e6542650),
    f64::from_bits(0x3fef71ea856631fc),
    f64::from_bits(0x3fef42f2ecb4e3d8),
    f64::from_bits(0x3fef0bd857dc2940),
    f64::from_bits(0x3feecbfa766fe475),
    f64::from_bits(0x3fee82b242ba34fa),
    f64::from_bits(0x3fee2f5663372498),
    f64::from_bits(0x3fedd1400e8216f8),
    f64::from_bits(0x3fed67d061db9134),
    f64::from_bits(0x3fecf276121739ea),
    f64::from_bits(0x3fec70b3561a96d9),
    f64::from_bits(0x3febe223e1ae2d32),
    f64::from_bits(0x3feb4682bfdf0ec1),
    f64::from_bits(0x3fea9dafd638d8e8),
    f64::from_bits(0x3fe9e7b4d65d087b),
    f64::from_bits(0x3fe924c9627e56ec),
    f64::from_bits(0x3fe855562b7dbe1a),
    f64::from_bits(0x3fe779f6d5133f5e),
    f64::from_bits(0x3fe6937a75943bed),
    f64::from_bits(0x3fe5a2e2934fa79d),
    f64::from_bits(0x3fe4a960909d51be),
    f64::from_bits(0x3fe3a85188e0f8a8),
    f64::from_bits(0x3fe2a138b2eee4b6),
    f64::from_bits(0x3fe195b86f5cd0a0),
    f64::from_bits(0x3fe0878a3a2dd728),
    f64::from_bits(0x3fdef0eb8ba451ae),
    f64::from_bits(0x3fdcd48f21465ec0),
    f64::from_bits(0x3fdabd8e9a223693),
    f64::from_bits(0x3fd8af5cee3e0eac),
    f64::from_bits(0x3fd6ad3edec55c80),
    f64::from_bits(0x3fd4ba3ad960b0c4),
    f64::from_bits(0x3fd2d90b14d78823),
    f64::from_bits(0x3fd10c1255d98141),
    f64::from_bits(0x3fceaaa752090795),
    f64::from_bits(0x3fcb6cda7606a450),
    f64::from_bits(0x3fc8612ca68bde0d),
    f64::from_bits(0x3fc58940a71c9c63),
    f64::from_bits(0x3fc2e5f50083c4f6),
    f64::from_bits(0x3fc0777079474b36),
    f64::from_bits(0x3fbc7a654f2b493a),
    f64::from_bits(0x3fb86c4f6f4630ac),
    f64::from_bits(0x3fb4c17cf1237664),
    f64::from_bits(0x3fb175ff8bef4840),
    f64::from_bits(0x3fad0a99cc8db684),
    f64::from_bits(0x3fa7d4dbd45cb05c),
    f64::from_bits(0x3fa340589901b8b4),
    f64::from_bits(0x3f9e84f5047ad800),
    f64::from_bits(0x3f97a1a269638510),
    f64::from_bits(0x3f91c2af5339c068),
    f64::from_bits(0x3f89aac66af66be0),
    f64::from_bits(0x3f819180910d6ee0),
    f64::from_bits(0x3f76376e79f84d80),
    f64::from_bits(0x3f68c126fafc3680),
    f64::from_bits(0x3f55d9ec39bc2100),
    f64::from_bits(0x3f35c25df987c200),
    f64::from_bits(0x0000000000000000),
];

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
            weights: CAMERA_WEIGHTS,
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
    fn camera_weights_pin_chromium_binary64_bits() {
        let expected: [u64; CAMERA_SAMPLES] = [
            0x3ff0000000000000,
            0x3feffd47b440cf08,
            0x3feff51309e321f0,
            0x3fefe73ed90503c9,
            0x3fefd391230c0f65,
            0x3fefb9b9fdbbca44,
            0x3fef9954e6542650,
            0x3fef71ea856631fc,
            0x3fef42f2ecb4e3d8,
            0x3fef0bd857dc2940,
            0x3feecbfa766fe475,
            0x3fee82b242ba34fa,
            0x3fee2f5663372498,
            0x3fedd1400e8216f8,
            0x3fed67d061db9134,
            0x3fecf276121739ea,
            0x3fec70b3561a96d9,
            0x3febe223e1ae2d32,
            0x3feb4682bfdf0ec1,
            0x3fea9dafd638d8e8,
            0x3fe9e7b4d65d087b,
            0x3fe924c9627e56ec,
            0x3fe855562b7dbe1a,
            0x3fe779f6d5133f5e,
            0x3fe6937a75943bed,
            0x3fe5a2e2934fa79d,
            0x3fe4a960909d51be,
            0x3fe3a85188e0f8a8,
            0x3fe2a138b2eee4b6,
            0x3fe195b86f5cd0a0,
            0x3fe0878a3a2dd728,
            0x3fdef0eb8ba451ae,
            0x3fdcd48f21465ec0,
            0x3fdabd8e9a223693,
            0x3fd8af5cee3e0eac,
            0x3fd6ad3edec55c80,
            0x3fd4ba3ad960b0c4,
            0x3fd2d90b14d78823,
            0x3fd10c1255d98141,
            0x3fceaaa752090795,
            0x3fcb6cda7606a450,
            0x3fc8612ca68bde0d,
            0x3fc58940a71c9c63,
            0x3fc2e5f50083c4f6,
            0x3fc0777079474b36,
            0x3fbc7a654f2b493a,
            0x3fb86c4f6f4630ac,
            0x3fb4c17cf1237664,
            0x3fb175ff8bef4840,
            0x3fad0a99cc8db684,
            0x3fa7d4dbd45cb05c,
            0x3fa340589901b8b4,
            0x3f9e84f5047ad800,
            0x3f97a1a269638510,
            0x3f91c2af5339c068,
            0x3f89aac66af66be0,
            0x3f819180910d6ee0,
            0x3f76376e79f84d80,
            0x3f68c126fafc3680,
            0x3f55d9ec39bc2100,
            0x3f35c25df987c200,
            0x0000000000000000,
        ];
        assert_eq!(
            CameraCorrections::default().weights().map(f64::to_bits),
            expected
        );
    }

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn camera_weights_stay_within_native_formula_spread() {
        use crate::math;
        use std::f64::consts::PI;

        for (index, weight) in CAMERA_WEIGHTS.into_iter().enumerate() {
            let mut native = 0.5
                * (1.
                    - math::cos(
                        PI * (CAMERA_SAMPLES - 1 - index) as f64 / (CAMERA_SAMPLES - 1) as f64,
                    ));
            native += (0.5 * (1. - math::cos(PI * native)) - native) * 0.5;
            // Both are nonnegative, so unsigned bit distance is ULP distance.
            assert!(native >= 0. && weight >= 0.);
            let distance = native.to_bits().abs_diff(weight.to_bits());
            assert!(
                distance <= 16,
                "weight {index}: native formula differs by {distance} ULP"
            );
        }
    }
}
