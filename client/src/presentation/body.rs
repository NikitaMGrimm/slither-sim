//! Retained body anchors and their update-driven visual corrections.

use std::f64::consts::{PI, TAU};

use super::{PresentationError, finite};
use crate::{
    math,
    state::{Appearance, MAX_BODY_POINTS, Point, Setup},
};

/// Samples in each body's x, y, edge-length, and spacing correction channel.
pub const BODY_SAMPLES: usize = 53;
/// Physical records, including dying anchors, retained by one body.
/// Logical segment count has the separate received-state limit of 4096.
pub const MAX_RETAINED_BODY_POINTS: usize = MAX_BODY_POINTS * 2;
const SPACING_SAMPLES: usize = 100;
// The count is at most 4096. Reuse its unused high bit so legacy bodies retain
// their record layout and allocation footprint, including replay allocations.
const VISUAL_REDUCED: usize = 1 << (usize::BITS - 1);
const COMPACT_INTERVAL: u16 = 1 << 15;

#[derive(Clone, Debug, PartialEq)]
struct BodyCorrections {
    samples: [[f32; BODY_SAMPLES]; 4],
    offset: [f64; 4],
    index: usize,
    pending: i32,
}

#[cfg(test)]
mod compact_tests {
    use super::*;
    use crate::presentation::*;
    use crate::state::Direction;

    fn fixture(count: usize, direction: f64) -> BodyGeometry {
        let setup = Setup {
            protocol: 19,
            grid_center: 1000.,
            radius: 900.,
            sector_size: 100,
            sector_count: 20,
            max_segments: 4096,
            body_spacing: 6.,
            body_contraction: 0.5,
            spangdv: 4.,
            nsp1: 4.,
            nsp2: 0.5,
            nsp3: 12.,
            mamu: 0.03,
            mamu2: 0.02,
            server_number: None,
            game_mode: None,
            team: None,
        };
        let points: Vec<_> = (0..count)
            .map(|i| Point {
                x: 60030. - direction * i as f64 * 6.,
                y: 4.,
            })
            .collect();
        BodyGeometry::from_appearance(
            &Appearance {
                id: 3,
                skin: 0,
                name: String::new(),
                custom_skin: Vec::new(),
                cosmetic: 255,
                absolute_head: points[0],
                absolute_body: points,
                anchor_direction: Some(0),
                heading: 0.,
                wanted_heading: 0.,
                speed_per_32ms: 4.,
                fam: 0.,
                wire_direction: Direction::None,
            },
            &setup,
        )
        .unwrap()
    }

    fn assert_span(body: &BodyGeometry, received: &[Point]) {
        assert_curve(body, received, 0.);
    }

    fn assert_curve(body: &BodyGeometry, received: &[Point], relative_tolerance: f64) {
        assert_eq!(body.logical_segments(), received.len());
        assert_eq!(
            body.points()
                .iter()
                .map(BodyPoint::represented_intervals)
                .sum::<usize>(),
            received.len()
        );
        let first = received[0];
        let curve = sample_curve_with_heading(
            CurveInput {
                corrected_head: first,
                head_fraction: 0.,
                body: body.points(),
                displayed_length: received.len() as f64,
                segment_length: 6.,
                point_spacing: 6.,
                target_point_spacing: 6.,
                quality_scale: 64.,
                detailed_style: false,
            },
            0.,
        )
        .unwrap();
        let epsilon = 0.02; // Includes Float32 rounding at the 60,000-unit fixture origin.
        let bounds = received
            .iter()
            .fold([first.x, first.x, first.y, first.y], |b, p| {
                [b[0].min(p.x), b[1].max(p.x), b[2].min(p.y), b[3].max(p.y)]
            });
        for p in &curve.samples {
            assert!(f64::from(p.x) >= bounds[0] - epsilon);
            assert!(f64::from(p.x) <= bounds[1] + epsilon);
            assert!(f64::from(p.y) >= bounds[2] - epsilon);
            assert!(f64::from(p.y) <= bounds[3] + epsilon);
        }
        let length: f64 = curve
            .samples
            .windows(2)
            .map(|p| {
                (f64::from(p[1].x) - f64::from(p[0].x)).hypot(f64::from(p[1].y) - f64::from(p[0].y))
            })
            .sum();
        let received_length: f64 = received
            .windows(2)
            .map(|p| (p[1].x - p[0].x).hypot(p[1].y - p[0].y))
            .sum();
        assert!(
            (length - received_length).abs() <= epsilon + relative_tolerance * received_length,
            "visual {length}, received {received_length}"
        );
    }

    #[test]
    fn compact_straight_motion_keeps_the_pattern_at_a_fixed_head_distance() {
        // Authored straight pose: one world unit per stock paint sample.
        // Advancing between anchors must not stretch the pattern over the tail.
        let mut body = fixture(128, 1.);
        body.compact_visual();
        let mut indices = Vec::new();
        for phase in [0., 1., 2., 3., 4., 5., 6.] {
            let head = Point {
                x: 60030. + phase,
                y: 4.,
            };
            let curve = sample_curve_with_heading(
                CurveInput {
                    corrected_head: head,
                    head_fraction: phase / 6.,
                    body: body.points(),
                    displayed_length: 128.,
                    segment_length: 6.,
                    point_spacing: 6.,
                    target_point_spacing: 6.,
                    quality_scale: 1.,
                    detailed_style: false,
                },
                0.,
            )
            .unwrap();
            let index = curve
                .samples
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (head.x - f64::from(a.x) - 384.)
                        .abs()
                        .total_cmp(&(head.x - f64::from(b.x) - 384.).abs())
                })
                .unwrap()
                .0;
            indices.push(index);
        }
        assert_eq!(indices, vec![384; 7], "head-anchored pattern samples");
    }

    fn painted_pattern(curve: &CurveOutput, skin_id: u8, head: Point, limit: usize) -> usize {
        let skin = resolve_authored_local_skin(skin_id, &[]).unwrap();
        let samples: Vec<_> = curve
            .samples
            .iter()
            .enumerate()
            .map(|(i, p)| BodyPaintSample {
                ordinal: curve.sample_ordinal(i),
                x: p.x,
                y: p.y,
                tangent: p.angle,
                flag: 2,
            })
            .collect();
        // Unique authored numeric handles let this test observe both palette and
        // shade without installing or rendering any image assets.
        let assets: Vec<_> = (0..42 * 60)
            .map(|handle| BodyAsset {
                handle,
                revision: 1,
                width: 48,
                height: 48,
                status: BodyAssetStatus::Ready,
            })
            .collect();
        let variants: Vec<Vec<_>> = (0..42)
            .map(|palette| {
                (0..60)
                    .map(|variant| BodyAssetUse {
                        asset: palette * 60 + variant,
                        source_rect: None,
                    })
                    .collect()
            })
            .collect();
        let palettes: Vec<_> = variants
            .iter()
            .map(|variants| BodyPalette {
                variants,
                boost_glow: Some(variants[0]),
            })
            .collect();
        let effect = Some(BodyAssetUse {
            asset: 0,
            source_rect: None,
        });
        let plan = plan_reduced_body(BodyPaintInput {
            geometry: BodyPaintGeometry {
                samples: &samples,
                active_count: samples.len(),
                distance_cursor_seed: curve.distance_cursor_seed().unwrap(),
                eligible: true,
            },
            recipe: BodyPaintRecipe {
                base_palette: skin.base_palette,
                palettes: &palettes,
                runs: &skin.runs,
                variant_mode: skin.variant_mode,
                variant_period: skin.variant_period,
                custom: skin.custom,
                style: Some(skin.body_style),
                outline: effect,
                shadow: effect,
                death: effect,
            },
            assets: &assets,
            view: BodyPaintView {
                camera: head,
                zoom: 1.,
                half_width: 1000.,
                half_height: 1000.,
            },
            quality: BodyPaintQuality {
                high_quality: true,
                blend: 1.,
            },
            visual: BodyPaintVisual {
                scale: 1.,
                spawn: 1.,
                death: 0.,
                dead: false,
                smoothed_speed: 4.,
                boost_threshold: 5.,
                cruise_speed: 4.,
                maximum_speed: 10.,
                boost_phase: 0.,
            },
            entry_alpha: 1.,
            command_limit: limit,
        })
        .unwrap();
        plan.commands
            .iter()
            .find(|c| c.ordinal == 384 && c.destination.width == c.destination.height)
            .expect("fixed body sample survives nested quotas")
            .asset_index
    }

    #[test]
    fn compact_shades_and_stripes_survive_motion_tail_retirement_and_camera_quotas() {
        for skin in [0, 10, 60] {
            let mut body = fixture(128, 1.);
            body.compact_visual();
            let mut expected = None;
            for anchor in 0..16 {
                if anchor > 0 {
                    let head = Point {
                        x: 60030. + f64::from(anchor) * 6.,
                        y: 4.,
                    };
                    body.apply_anchor(Some(head), None, false, head, false)
                        .unwrap();
                    body.advance(1000);
                }
                for phase in [0., 1., 3., 5.] {
                    let head = Point {
                        x: 60030. + f64::from(anchor) * 6. + phase,
                        y: 4.,
                    };
                    let curve = sample_curve_with_heading(
                        CurveInput {
                            corrected_head: head,
                            head_fraction: phase / 6.,
                            body: body.points(),
                            displayed_length: 128.,
                            segment_length: 6.,
                            point_spacing: 6.,
                            target_point_spacing: 6.,
                            quality_scale: 1.,
                            detailed_style: false,
                        },
                        0.,
                    )
                    .unwrap();
                    for quota in [97, 193, 385, 1000] {
                        let reduced = super::super::curve::sample_curve_with_limit(
                            CurveInput {
                                corrected_head: head,
                                head_fraction: phase / 6.,
                                body: body.points(),
                                displayed_length: 128.,
                                segment_length: 6.,
                                point_spacing: 6.,
                                target_point_spacing: 6.,
                                quality_scale: 1.,
                                detailed_style: false,
                            },
                            0.,
                            quota,
                        )
                        .unwrap();
                        assert!(reduced.samples.len() <= quota);
                        assert_eq!(reduced.samples.capacity(), reduced.samples.len());
                        assert_eq!(
                            reduced.storage_bytes(),
                            reduced.samples.len()
                                * (size_of::<CurveSample>() + size_of::<CurveVertex>())
                        );
                        for (i, sample) in reduced.samples.iter().enumerate() {
                            assert_eq!(sample, &curve.samples[reduced.sample_ordinal(i)]);
                        }
                        let i = (0..reduced.samples.len())
                            .find(|&i| reduced.sample_ordinal(i) == 384)
                            .unwrap();
                        assert_eq!(head.x - f64::from(reduced.samples[i].x), 384.);
                        let pattern = painted_pattern(&reduced, skin, head, usize::MAX);
                        assert_eq!(
                            pattern,
                            *expected.get_or_insert(pattern),
                            "skin {skin}, anchor {anchor}, phase {phase}, quota {quota}"
                        );
                        assert_eq!(painted_pattern(&reduced, skin, head, quota), pattern);
                    }
                }
            }
        }
    }

    #[test]
    fn compact_turning_and_growing_snakes_keep_pattern_indices_at_fixed_arc_distance() {
        for growing in [false, true] {
            for skin in [0, 10, 60] {
                let position = |i: f64| {
                    if growing {
                        Point {
                            x: 60030. + i * 6.,
                            y: 4.,
                        }
                    } else {
                        Point {
                            x: 60030. + 1000. * (i * 0.006).sin(),
                            y: 1000. * (i * 0.006).cos(),
                        }
                    }
                };
                let mut body = fixture(128, 1.);
                for (i, point) in body.points.iter_mut().enumerate() {
                    point.position = position(-(i as f64));
                }
                body.compact_visual();
                let mut expected_pattern = None;
                for step in 0..16 {
                    if step > 0 {
                        let head = position(f64::from(step));
                        body.apply_anchor(Some(head), None, growing, head, false)
                            .unwrap();
                        body.advance(1000);
                        if step % 5 == 0 {
                            body.compact_visual();
                        }
                    }
                    for phase in [0., 0.25, 0.75] {
                        let head = position(f64::from(step) + phase);
                        // Independently walk the retained polyline to the same
                        // physical distance behind the head, even as it turns
                        // or the tail-to-head extent grows.
                        let mut remaining = 384.;
                        let mut previous = head;
                        let mut expected_position = None;
                        for point in body.points().iter().filter(|p| !p.dying()) {
                            let next = point.position();
                            let length = (next.x - previous.x).hypot(next.y - previous.y);
                            if length >= remaining {
                                expected_position = Some(Point {
                                    x: previous.x + (next.x - previous.x) * remaining / length,
                                    y: previous.y + (next.y - previous.y) * remaining / length,
                                });
                                break;
                            }
                            remaining -= length;
                            previous = next;
                        }
                        let expected_position = expected_position.unwrap();
                        for quota in [97, 193, 385] {
                            let curve = super::super::curve::sample_curve_with_limit(
                                CurveInput {
                                    corrected_head: head,
                                    head_fraction: phase,
                                    body: body.points(),
                                    displayed_length: body.logical_segments() as f64,
                                    segment_length: 6.,
                                    point_spacing: 6.,
                                    target_point_spacing: 6.,
                                    quality_scale: 1.,
                                    detailed_style: false,
                                },
                                0.,
                                quota,
                            )
                            .unwrap();
                            let (i, sample) = curve
                                .samples
                                .iter()
                                .enumerate()
                                .min_by(|(_, a), (_, b)| {
                                    (f64::from(a.x) - expected_position.x)
                                        .hypot(f64::from(a.y) - expected_position.y)
                                        .total_cmp(
                                            &(f64::from(b.x) - expected_position.x)
                                                .hypot(f64::from(b.y) - expected_position.y),
                                        )
                                })
                                .unwrap();
                            assert_eq!(
                                curve.sample_ordinal(i),
                                384,
                                "growing {growing}, step {step}, phase {phase}, quota {quota}"
                            );
                            assert!(
                                (f64::from(sample.x) - expected_position.x)
                                    .hypot(f64::from(sample.y) - expected_position.y)
                                    < 0.01
                            );
                            let pattern = painted_pattern(&curve, skin, head, usize::MAX);
                            assert_eq!(pattern, *expected_pattern.get_or_insert(pattern));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn limited_curves_preserve_full_grid_samples_tangents_vertices_and_eye_target() {
        for compact in [false, true] {
            let mut body = fixture(128, 1.);
            if compact {
                body.compact_visual();
            }
            let input = CurveInput {
                corrected_head: Point { x: 60033., y: 5. },
                head_fraction: 0.5,
                body: body.points(),
                displayed_length: 127.4,
                segment_length: 6.,
                point_spacing: 6.,
                target_point_spacing: 6.,
                quality_scale: 1.,
                detailed_style: false,
            };
            let full = sample_curve_with_heading(input, 0.).unwrap();
            for limit in [2, 8, 97, 385, 1000] {
                let reduced =
                    super::super::curve::sample_curve_with_limit(input, 0., limit).unwrap();
                assert!(reduced.samples.capacity() <= limit);
                assert_eq!(
                    reduced.storage_bytes(),
                    reduced.samples.len() * (size_of::<CurveSample>() + size_of::<CurveVertex>())
                );
                assert_eq!(reduced.target_eye_heading, full.target_eye_heading);
                assert_eq!(reduced.distance_cursor_seed(), full.distance_cursor_seed());
                for (i, sample) in reduced.samples.iter().enumerate() {
                    let ordinal = reduced.sample_ordinal(i);
                    assert_eq!(sample, &full.samples[ordinal]);
                    assert_eq!(reduced.vertices()[i], full.vertices()[ordinal]);
                }
                assert_eq!(reduced.samples.last(), full.samples.last());
            }
        }
    }

    #[test]
    fn compact_terminal_and_sixteen_motion_updates_preserve_received_span() {
        assert_eq!(
            size_of::<BodyPoint>(),
            960,
            "legacy replay allocation layout"
        );
        let mut body = fixture(2048, 1.);
        let mut received: Vec<_> = body.points().iter().map(BodyPoint::position).collect();
        while body.points().len() > 8 {
            body.compact_visual();
        }
        assert_span(&body, &received);
        for _ in 0..16 {
            let head = Point {
                x: received[0].x + 6.,
                y: 4.,
            };
            received.pop();
            received.insert(0, head);
            body.apply_anchor(Some(head), None, false, head, false)
                .unwrap();
            body.advance(1000);
            assert_span(&body, &received);
        }
    }

    #[test]
    fn compact_exhaustion_can_accept_a_new_absolute_growth_anchor() {
        let mut body = fixture(32, 1.);
        body.compact_visual();
        for _ in 0..32 {
            assert!(body.remove_tail().unwrap());
        }
        assert_eq!(body.logical_segments(), 0);
        assert!(body.points().is_empty());
        let head = Point { x: 60036., y: 4. };
        body.apply_anchor(Some(head), None, true, Point { x: 60030., y: 4. }, false)
            .unwrap();
        assert_eq!(body.logical_segments(), 1);
        assert_eq!(body.points()[0].position(), head);
    }

    #[test]
    fn deterministic_compact_growth_motion_and_tail_sequences_preserve_intervals() {
        for seed in [7_u64, 28, 161803] {
            let direction = if seed & 1 == 0 { -1. } else { 1. };
            let mut body = fixture(128, direction);
            let mut received: Vec<_> = body.points().iter().map(BodyPoint::position).collect();
            while body.points().len() > 8 {
                body.compact_visual();
            }
            let mut random = seed;
            for step in 0..if cfg!(debug_assertions) { 96 } else { 768 } {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                let action = (random >> 32) % 3;
                if action == 0 && received.len() > 8 {
                    received.pop();
                    body.remove_tail().unwrap();
                } else {
                    let grew = action == 1;
                    let head = Point {
                        x: received[0].x + direction * 6.,
                        y: 4.,
                    };
                    if !grew {
                        received.pop();
                    }
                    received.insert(0, head);
                    body.apply_anchor(Some(head), None, grew, head, false)
                        .unwrap();
                }
                if step % 7 == 0 && body.points().len() > 8 {
                    body.compact_visual();
                }
                body.advance(1000);
                assert_span(&body, &received);
            }
        }
    }

    #[test]
    fn compact_curved_motion_stays_in_received_bounds_and_tracks_length() {
        for seed in [11_u64, 42] {
            let position = |i: i32| {
                let a = f64::from(i) / 128.;
                Point {
                    x: 60030. + 768. * a.sin(),
                    y: 768. * a.cos(),
                }
            };
            let mut body = fixture(128, 1.);
            for (i, p) in body.points.iter_mut().enumerate() {
                p.position = position(-(i as i32));
            }
            let mut received: Vec<_> = body.points.iter().map(BodyPoint::position).collect();
            while body.points.len() > 8 {
                body.compact_visual();
            }
            let mut random = seed;
            let mut head_index = 0;
            for step in 0..if cfg!(debug_assertions) { 96 } else { 768 } {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                let action = (random >> 32) % 3;
                if action == 0 && received.len() > 8 {
                    received.pop();
                    body.remove_tail().unwrap();
                } else {
                    head_index += 1;
                    let head = position(head_index);
                    let grew = action == 1;
                    if !grew {
                        received.pop();
                    }
                    received.insert(0, head);
                    body.apply_anchor(Some(head), None, grew, head, false)
                        .unwrap();
                }
                if step % 7 == 0 && body.points.len() > 8 {
                    body.compact_visual();
                }
                body.sync_received_tail(*received.last().unwrap());
                body.advance(1000);
                // Smooth curved paths admit chord simplification; the straight
                // reproductions above instead require absolute 0.02-unit parity.
                assert_curve(&body, &received, 0.10);
            }
        }
    }
}

impl Default for BodyCorrections {
    fn default() -> Self {
        Self {
            samples: [[0.; BODY_SAMPLES]; 4],
            offset: [0.; 4],
            index: 0,
            pending: 0,
        }
    }
}

impl BodyCorrections {
    fn inject(
        &mut self,
        changes: [Option<f64>; 4],
        weights: &[f32; BODY_SAMPLES],
    ) -> Result<(), PresentationError> {
        for (channel, change) in changes.into_iter().enumerate() {
            let Some(change) = change else { continue };
            finite(&[change])?;
            for (distance, weight) in weights.iter().enumerate() {
                let slot = (self.index + distance) % BODY_SAMPLES;
                let stored =
                    (f64::from(self.samples[channel][slot]) - change * f64::from(*weight)) as f32;
                if !stored.is_finite() {
                    return Err(PresentationError);
                }
                self.samples[channel][slot] = stored;
            }
            // An omitted channel retains its currently displayed offset as well
            // as its samples. Contraction does not refresh the edge channel.
            self.offset[channel] = f64::from(self.samples[channel][self.index]);
        }
        self.pending = BODY_SAMPLES as i32;
        Ok(())
    }

    fn advance(&mut self, steps: u32) {
        if steps == 0 {
            return;
        }
        if self.pending > 0 {
            let consumed = steps.min(self.pending as u32);
            for _ in 0..consumed {
                for (samples, offset) in self.samples.iter_mut().zip(&mut self.offset) {
                    *offset = f64::from(samples[self.index]);
                    samples[self.index] = 0.;
                }
                self.index = (self.index + 1) % BODY_SAMPLES;
            }
            self.pending -= consumed as i32;
        } else if self.pending == 0 {
            self.pending = -1;
            self.offset = [0.; 4];
        }
    }
}

/// One absolute body anchor, which can remain after its logical tail removal.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyPoint {
    position: Point,
    previous_delta: Point,
    edge_length: f64,
    spacing_multiplier: f64,
    tail_fade: f64,
    dying: bool,
    // Fits the record's existing alignment padding; legacy allocation is exact.
    represented: u16,
    corrections: BodyCorrections,
}

impl BodyPoint {
    pub(super) fn compacted(&self) -> bool {
        self.represented & COMPACT_INTERVAL != 0
    }

    /// Logical incoming intervals represented by this retained visual anchor.
    pub fn represented_intervals(&self) -> usize {
        usize::from(self.represented & !COMPACT_INTERVAL).max(1)
    }
    /// Absolute, contracted anchor position before displayed corrections.
    pub fn position(&self) -> Point {
        self.position
    }

    /// Delta from the preceding tailward anchor at creation, never recomputed.
    pub fn previous_delta(&self) -> Point {
        self.previous_delta
    }

    /// Legacy creation-time edge length divided by segment length; compact
    /// anchors instead carry their represented incoming logical interval count.
    pub fn edge_length(&self) -> f64 {
        self.edge_length
    }

    /// Current base spacing multiplier, promoted from the Float32 spacing table.
    pub fn spacing_multiplier(&self) -> f64 {
        self.spacing_multiplier
    }

    /// Tail fade in 0 through 1, independent of logical membership.
    pub fn tail_fade(&self) -> f64 {
        self.tail_fade
    }

    /// Whether a tail or non-growing anchor update marked this point dying.
    pub fn dying(&self) -> bool {
        self.dying
    }

    /// Displayed x, y, edge-length, and spacing correction offsets.
    pub fn offset(&self) -> [f64; 4] {
        self.corrections.offset
    }

    /// Displayed positional correction, promoted from Float32 storage.
    pub fn position_offset(&self) -> Point {
        Point {
            x: self.corrections.offset[0],
            y: self.corrections.offset[1],
        }
    }

    /// Displayed edge-length correction.
    pub fn edge_length_offset(&self) -> f64 {
        self.corrections.offset[2]
    }

    /// Displayed spacing correction.
    pub fn spacing_offset(&self) -> f64 {
        self.corrections.offset[3]
    }

    /// Next correction sample index.
    pub fn index(&self) -> usize {
        self.corrections.index
    }

    /// Positive sample count, zero awaiting positive-step cleanup, or -1.
    pub fn pending(&self) -> i32 {
        self.corrections.pending
    }

    /// Physical Float32 samples, ordered x, y, edge-length, then spacing.
    pub fn samples(&self) -> &[[f32; BODY_SAMPLES]; 4] {
        &self.corrections.samples
    }
}

/// Head-to-tail absolute body geometry, separate from head, mass, and camera state.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyGeometry {
    points: Vec<BodyPoint>,
    logical_segments: usize,
    anchor_direction: Option<u16>,
    segment_length: f64,
    contraction: f64,
    weights: [f32; BODY_SAMPLES],
    spacing: [f32; SPACING_SAMPLES],
}

fn difference(a: Point, b: Point) -> Result<Point, PresentationError> {
    let result = Point {
        x: a.x - b.x,
        y: a.y - b.y,
    };
    finite(&[a.x, a.y, b.x, b.y, result.x, result.y])?;
    Ok(result)
}

fn distance(delta: Point) -> Result<f64, PresentationError> {
    // Preserve the squared-sum boundary instead of hypot's scaled arithmetic.
    let squared = delta.x * delta.x + delta.y * delta.y;
    finite(&[squared])?;
    Ok(squared.sqrt())
}

impl BodyGeometry {
    pub(crate) fn configure_contraction(
        &mut self,
        contraction: f64,
    ) -> Result<(), PresentationError> {
        if !contraction.is_finite() || !(0. ..=1.).contains(&contraction) {
            return Err(PresentationError);
        }
        self.contraction = contraction;
        self.spacing = std::array::from_fn(|index| {
            let contracted = index.saturating_sub(3).min(4);
            (1. - contraction * contracted as f64 / 4.) as f32
        });
        Ok(())
    }

    /// Initialize received absolute anchors without contracting their geometry.
    /// The tail's previous delta is zero; all initial normalized edge lengths are one.
    pub fn from_appearance(
        appearance: &Appearance,
        setup: &Setup,
    ) -> Result<Self, PresentationError> {
        finite(&[
            setup.body_spacing,
            setup.body_contraction,
            appearance.absolute_head.x,
            appearance.absolute_head.y,
        ])?;
        if !matches!(setup.protocol, 13 | 19)
            || setup.body_spacing <= 0.
            || !(0. ..=1.).contains(&setup.body_contraction)
            || appearance.absolute_body.len() > MAX_BODY_POINTS
        {
            return Err(PresentationError);
        }
        let weights = std::array::from_fn(|index| {
            (0.5 * (1.
                - (PI * (BODY_SAMPLES - 1 - index) as f64 / (BODY_SAMPLES - 1) as f64).cos()))
                as f32
        });
        let spacing = std::array::from_fn(|index| {
            let contracted = index.saturating_sub(3).min(4);
            (1. - setup.body_contraction * contracted as f64 / 4.) as f32
        });
        let mut points = Vec::with_capacity(appearance.absolute_body.len());
        for (index, position) in appearance.absolute_body.iter().copied().enumerate() {
            let tailward = appearance
                .absolute_body
                .get(index + 1)
                .copied()
                .unwrap_or(position);
            points.push(BodyPoint {
                position,
                previous_delta: difference(position, tailward)?,
                edge_length: 1.,
                spacing_multiplier: f64::from(spacing[index.min(SPACING_SAMPLES - 4)]),
                tail_fade: 0.,
                dying: false,
                represented: 0,
                corrections: BodyCorrections::default(),
            });
        }
        Ok(Self {
            logical_segments: points.len(),
            points,
            anchor_direction: appearance.anchor_direction,
            segment_length: setup.body_spacing,
            contraction: setup.body_contraction,
            weights,
            spacing,
        })
    }

    /// Add an absolute or cached-direction anchor and return its absolute position.
    /// Non-growth marks the oldest living point dying; growth increments logical length.
    /// If every point is already dying, non-growth still leaves logical length unchanged.
    /// All arithmetic and retention failures preserve the entire previous body.
    pub fn apply_anchor(
        &mut self,
        absolute_position: Option<Point>,
        direction: Option<u16>,
        grew: bool,
        displayed_head: Point,
        visible: bool,
    ) -> Result<Point, PresentationError> {
        finite(&[displayed_head.x, displayed_head.y])?;
        if self.points.len() >= MAX_RETAINED_BODY_POINTS
            || (grew && self.logical_segments() >= MAX_BODY_POINTS)
        {
            return Err(PresentationError);
        }
        // Reduced rivals do not retain the legacy three dying tail ghosts.
        // After complete exhaustion an absolute growth anchor must still be
        // admissible; use the retained head only as its prior visual reference.
        let exhausted;
        let last = if let Some(last) = self.points.first() {
            last
        } else if self.detail_reduced() {
            exhausted = BodyPoint {
                position: displayed_head,
                previous_delta: Point::default(),
                edge_length: 1.,
                spacing_multiplier: 1.,
                tail_fade: 0.,
                dying: true,
                represented: COMPACT_INTERVAL | 1,
                corrections: BodyCorrections::default(),
            };
            &exhausted
        } else {
            return Err(PresentationError);
        };
        let next_direction = direction.or(self.anchor_direction);
        let position = match absolute_position {
            Some(position) => position,
            None => {
                let angle = f64::from(next_direction.ok_or(PresentationError)?) * (TAU / 65_536.);
                // This anchor reanchors the predicted head used by pointer input.
                Point {
                    x: last.position.x + math::cos(angle) * self.segment_length,
                    y: last.position.y + math::sin(angle) * self.segment_length,
                }
            }
        };
        let previous_delta = difference(position, last.position)?;
        let edge_length = distance(previous_delta)? / self.segment_length;
        finite(&[edge_length])?;
        if visible && edge_length == 0. {
            return Err(PresentationError);
        }
        let mut point = BodyPoint {
            position,
            previous_delta,
            edge_length,
            spacing_multiplier: 1.,
            tail_fade: 0.,
            dying: false,
            represented: if self.detail_reduced() {
                COMPACT_INTERVAL | 1
            } else {
                0
            },
            corrections: BodyCorrections::default(),
        };
        if visible {
            let tailward_display = Point {
                x: last.position.x + last.corrections.offset[0],
                y: last.position.y + last.corrections.offset[1],
            };
            let mut toward_head = difference(displayed_head, tailward_display)?;
            let head_distance = distance(toward_head)?;
            if head_distance > 1. {
                toward_head.x /= head_distance;
                toward_head.y /= head_distance;
            }
            let chord = edge_length * self.segment_length;
            let projected = if head_distance < self.segment_length {
                head_distance
            } else {
                chord
            };
            let prior_position = Point {
                x: tailward_display.x + toward_head.x * projected,
                y: tailward_display.y + toward_head.y * projected,
            };
            let displacement = difference(position, prior_position)?;
            point.corrections.inject(
                [
                    Some(displacement.x),
                    Some(displacement.y),
                    Some(1. - projected / chord),
                    Some(0.),
                ],
                &self.weights,
            )?;
        }
        let mut next = self.clone();
        if next.detail_reduced() {
            point.edge_length = 1.;
        }
        if grew {
            next.logical_segments += 1;
        } else if next.detail_reduced() {
            next.consume_compact_tail();
        } else if let Some(tail) = next.points.iter_mut().rev().find(|point| !point.dying) {
            tail.dying = true;
        }
        next.points.insert(0, point);
        next.anchor_direction = next_direction;
        // The three newest anchors stay fixed. Each moved anchor follows the
        // already moved neighbor, including retained dying records.
        for index in 3..if next.detail_reduced() {
            3
        } else {
            next.points.len()
        } {
            let neighbor = next.points[index - 1].position;
            let point = &mut next.points[index];
            let before = point.position;
            let weight = next.contraction * (index - 2).min(4) as f64 / 4.;
            point.position.x += (neighbor.x - point.position.x) * weight;
            point.position.y += (neighbor.y - point.position.y) * weight;
            let moved = difference(point.position, before)?;
            let spacing = f64::from(next.spacing[index.min(SPACING_SAMPLES - 3)]);
            let spacing_change = spacing - point.spacing_multiplier;
            point.spacing_multiplier = spacing;
            if visible {
                point.corrections.inject(
                    [Some(moved.x), Some(moved.y), None, Some(spacing_change)],
                    &next.weights,
                )?;
            }
        }
        *self = next;
        Ok(position)
    }

    /// Mark the oldest living anchor dying and decrement logical length.
    /// Returns false when no living point remains; retained records are not erased.
    /// A living point with zero logical length would require a negative count;
    /// that unsupported transition returns an error without changing the body.
    pub fn remove_tail(&mut self) -> Result<bool, PresentationError> {
        if self.detail_reduced() {
            let Some(remaining) = self.logical_segments().checked_sub(1) else {
                return Ok(false);
            };
            self.logical_segments = remaining | VISUAL_REDUCED;
            self.consume_compact_tail();
            return Ok(true);
        }
        let Some(tail) = self.points.iter_mut().rev().find(|point| !point.dying) else {
            return Ok(false);
        };
        let remaining = self
            .logical_segments
            .checked_sub(1)
            .ok_or(PresentationError)?;
        tail.dying = true;
        self.logical_segments = remaining;
        Ok(true)
    }

    /// Fade dying anchors and consume correction slots on positive integer steps.
    /// Preserve the three newest dying records even after full fade. A drained
    /// correction ring cleans up only on a subsequent positive call.
    pub fn advance(&mut self, whole_steps: u32) {
        if whole_steps == 0 {
            return;
        }
        let mut dying_count = 0;
        self.points.retain_mut(|point| {
            if point.dying {
                dying_count += 1;
                point.tail_fade = (point.tail_fade + 0.0015 * f64::from(whole_steps)).min(1.);
                if point.tail_fade >= 1. && dying_count >= 4 {
                    return false;
                }
            }
            point.corrections.advance(whole_steps);
            true
        });
    }

    /// Retained anchors in head-to-tail order, including dying points.
    pub fn points(&self) -> &[BodyPoint] {
        &self.points
    }

    /// Collapse only visual interior anchors. Keep the three newest anchors,
    /// both endpoints, surviving correction offsets and the protocol count.
    /// Reductions are nested and never reversed during this appearance, avoiding
    /// camera-driven oscillation. See docs/snake-capacity.md.
    pub(crate) fn compact_visual(&mut self) {
        if self.points.len() <= 8 {
            return;
        }
        // Retired correction/ghost detail is optional for reduced rivals. Own
        // bodies never enter this path. Track incoming logical spans separately
        // from the legacy sampler's outgoing traversal intervals.
        self.points.retain(|point| !point.dying);
        let count = self.points.len();
        let mut compact = Vec::<BodyPoint>::with_capacity(count.div_ceil(2) + 3);
        let mut represented = 0;
        for (index, point) in std::mem::take(&mut self.points).into_iter().enumerate() {
            represented += point.represented_intervals();
            if index < 3 || index % 2 == 0 || index == count - 1 {
                let mut point = point;
                point.represented = COMPACT_INTERVAL | represented as u16;
                point.edge_length = represented as f64;
                represented = 0;
                compact.push(point);
            }
        }
        self.points = compact.into_boxed_slice().into_vec();
        self.logical_segments |= VISUAL_REDUCED;
    }

    fn consume_compact_tail(&mut self) {
        let Some(index) = self.points.len().checked_sub(1) else {
            return;
        };
        let span = self.points[index].represented_intervals();
        if span == 1 || index == 0 {
            self.points.pop();
        } else {
            let near = self.points[index - 1].position;
            let tail = &mut self.points[index];
            tail.position.x += (near.x - tail.position.x) / span as f64;
            tail.position.y += (near.y - tail.position.y) / span as f64;
            tail.represented = COMPACT_INTERVAL | (span - 1) as u16;
            tail.edge_length = (span - 1) as f64;
        }
    }

    /// Fractional traversal is exact for straight folded spans. On a bend its
    /// chord approximates the retired path; pin the remaining endpoint to the
    /// received tail after ingress so repeated retirement cannot drift outward.
    pub(crate) fn sync_received_tail(&mut self, position: Point) {
        if self.detail_reduced()
            && let Some(tail) = self.points.last_mut()
            && tail.position != position
        {
            tail.position = position;
            tail.corrections = BodyCorrections::default();
        }
    }

    /// Whether this appearance has lost visual anchors under scene pressure.
    pub fn detail_reduced(&self) -> bool {
        self.logical_segments & VISUAL_REDUCED != 0
    }

    pub(crate) fn storage_slots(&self) -> usize {
        self.points.capacity()
    }

    pub(crate) fn reclaim_storage(&mut self) {
        self.points = std::mem::take(&mut self.points)
            .into_boxed_slice()
            .into_vec();
    }

    /// Nonnegative protocol count, updated by growth and tail removals.
    /// It is not recomputed from retained or living points: a non-growing anchor
    /// after exhaustion can add a living point while this count remains zero.
    pub fn logical_segments(&self) -> usize {
        self.logical_segments & !VISUAL_REDUCED
    }

    /// Effective cached wire heading for later relative anchors.
    pub fn anchor_direction(&self) -> Option<u16> {
        self.anchor_direction
    }

    /// Configured distance between directed anchors.
    pub fn segment_length(&self) -> f64 {
        self.segment_length
    }

    /// Configured contraction coefficient.
    pub fn contraction(&self) -> f64 {
        self.contraction
    }

    /// Float32 easing weights for all four body channels.
    pub fn weights(&self) -> &[f32; BODY_SAMPLES] {
        &self.weights
    }

    /// Float32 base spacing table, before per-point spacing corrections.
    pub fn spacing_multipliers(&self) -> &[f32; SPACING_SAMPLES] {
        &self.spacing
    }
}
