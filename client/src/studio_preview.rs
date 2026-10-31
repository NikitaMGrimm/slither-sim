//! Authored offline pose, independent of packets, worlds and session policy.
use crate::{menu_layout::MenuRect, presentation::*, state::Point};

/// Fixed logical sampling; DPR never increases geometry or command counts.
pub const PREVIEW_SAMPLES: usize = 79;
/// A prepared candidate. Publish its state only after whole-picture admission.
pub struct PreviewPose {
    /// Bounded head-to-tail paint samples from 27 authored anchors.
    pub samples: Vec<BodyPaintSample>,
    /// Synthetic corrected head, independent of game objects.
    pub head: Point,
    /// Head-facing tangent, in radians.
    pub heading: f64,
    /// Gentle gaze in world axes.
    pub pupil: Point,
    /// Fitted logical projection.
    pub view: BodyPaintView,
    /// Complete antenna candidate, never truncated to meet a quota.
    pub antenna: Option<PreparedAntenna>,
    /// State to adopt after the prepared picture is accepted.
    pub next: AppearancePreview,
}
/// Synthetic 27-anchor wave and retained antenna, with injected deterministic tape.
#[derive(Clone, Default)]
pub struct AppearancePreview {
    time: f64,
    last: Option<f64>,
    antenna: Option<(u8, AntennaState)>,
    random_index: usize,
}
impl AppearancePreview {
    /// Minimized/hidden callers clear the time anchor rather than accumulating catch-up.
    pub fn pause(&mut self) {
        self.last = None;
    }
    /// Advance visible local time from monotonic caller samples.
    pub fn tick(&mut self, now: f64) {
        if let Some(last) = self.last {
            self.time += (now - last).max(0.);
        }
        self.last = Some(now);
    }
    /// Visible elapsed milliseconds; hidden time is excluded.
    pub fn time(&self) -> f64 {
        self.time
    }
    /// The caller supplies readiness-resolved skin and entropy; no art can grant entry.
    pub fn prepare(
        &self,
        skin: &SkinDescriptor,
        bounds: MenuRect,
        tape: &[[f64; 2]],
    ) -> Result<PreviewPose, String> {
        if tape.is_empty() || bounds.width <= 0. || bounds.height <= 0. {
            return Err("Invalid preview bounds/tape".into());
        }
        let phase = self.time * std::f64::consts::TAU / 1000.;
        let anchors: Vec<_> = (0..27)
            .map(|i| {
                let t = i as f64 / 26.;
                Point {
                    x: 210. - 420. * t,
                    y: 15. * (t * std::f64::consts::TAU - phase).sin(),
                }
            })
            .collect();
        let samples: Vec<_> = (0..PREVIEW_SAMPLES)
            .map(|ordinal| {
                let index = ordinal as f64 / 3.;
                let i = (index.floor() as usize).min(25);
                let t = index - i as f64;
                let a = anchors[i];
                let b = anchors[i + 1];
                BodyPaintSample {
                    ordinal,
                    x: (a.x + (b.x - a.x) * t) as f32,
                    y: (a.y + (b.y - a.y) * t) as f32,
                    tangent: (b.y - a.y).atan2(b.x - a.x) as f32,
                    flag: 2,
                }
            })
            .collect();
        // Reserve complete bulb/chain extents as well as the tail and body shadows.
        let zoom = (bounds.width / 650.).min(bounds.height / 240.).min(1.3);
        let view = BodyPaintView {
            camera: Point { x: 0., y: 0. },
            zoom,
            half_width: bounds.x + bounds.width / 2.,
            half_height: bounds.y + bounds.height / 2.,
        };
        let head = anchors[0];
        let heading = (anchors[0].y - anchors[1].y).atan2(anchors[0].x - anchors[1].x);
        let mut next = self.clone();
        let antenna = if let Some(a) = skin.antenna {
            let state = match &self.antenna {
                Some((id, state))
                    if *id == skin.skin && state.geometry().same_bits(&a.geometry) =>
                {
                    state.clone()
                }
                _ => AntennaState::from_appearance(a.geometry, head).map_err(|e| e.to_string())?,
            };
            let request = state
                .prepare_draw(AntennaDrawInput {
                    raw_heading: heading,
                    corrected_head: head,
                    scale: 1.,
                    spawn: 1.,
                    death: 0.,
                    active_curve_count: PREVIEW_SAMPLES,
                    whole_snake_visible: true,
                    view,
                    bounds: Some(AntennaCullBounds::from_view(view).map_err(|e| e.to_string())?),
                })
                .map_err(|e| e.to_string())?;
            let count = request.random_pair_count();
            let pairs: Vec<_> = (0..count)
                .map(|i| tape[(self.random_index + i) % tape.len()])
                .collect();
            let (state, plan) = request.finish(&pairs).map_err(|e| e.to_string())?;
            next.random_index += count;
            next.antenna = Some((skin.skin, state));
            Some(plan)
        } else {
            next.antenna = None;
            None
        };
        Ok(PreviewPose {
            samples,
            head,
            heading,
            pupil: Point {
                x: 1.5 * (phase * 0.3).cos(),
                y: 1.5 * (phase * 0.3).sin(),
            },
            view,
            antenna,
            next,
        })
    }
}
