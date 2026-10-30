//! Opt-in offline observations. No renderer, update, random request or policy is invoked.
//! The flat numeric ABI is decoded by tools/feel-replay/observation.mjs.
//! Contract: [A1](../tools/feel-replay/README.md),
//! [browser peers](../tools/feel-replay/browser-peers.md) and the authored
//! [synthetic fixture](../tools/feel-replay/browser-synthetic.json).

use crate::presentation::{AnimatedClient, EyeLayoutInput, FaceGeometry, FrameStep};
use crate::state::Point;
use std::sync::{Arc, Mutex};

/// An immutable copy of one exact core boundary, including a staged draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    /// Recorder-local identity for picture acknowledgements, outside numeric cells.
    pub sequence: u64,
    /// Core boundary kind.
    pub phase: &'static str,
    /// Retained own identity, without conversion through f64.
    pub token: Option<u64>,
    /// Raw binary64 cells; f32 storage is widened exactly.
    pub values: Vec<f64>,
}

#[derive(Debug, Default)]
struct Journal {
    next_sequence: u64,
    rows: Vec<Observation>,
    errors: Vec<String>,
    failed: bool,
    last_draw: Option<Observation>,
    pictures: Vec<u64>,
    random: Vec<(&'static str, f64)>,
    step: Option<FrameStep>,
    extent: [u32; 2],
}

/// Shared across transaction clones so each prepared draw keeps its own copy.
/// Failed candidates remain observable; hosts must record their failed status.
#[derive(Clone, Debug, Default)]
pub struct Recorder(Arc<Mutex<Journal>>);
impl PartialEq for Recorder {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Recorder {
    /// Explicit observation failures; draining them never clears the failure gate.
    pub fn take_errors(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().expect("trace journal").errors)
    }

    /// A missing/unsupported observation makes this run incomplete.
    pub fn has_failed(&self) -> bool {
        self.0.lock().expect("trace journal").failed
    }

    /// Observe an existing random request without supplying or changing entropy.
    pub fn random(&self, tag: &'static str, value: f64) {
        self.0
            .lock()
            .expect("trace journal")
            .random
            .push((tag, value));
    }

    /// Drain ordered request-tagged units, independent of numeric observations.
    pub fn take_random(&self) -> Vec<(&'static str, f64)> {
        std::mem::take(&mut self.0.lock().expect("trace journal").random)
    }

    /// Drain copies in boundary order. Native A3 can consume these same cells.
    pub fn take(&self) -> Vec<Observation> {
        std::mem::take(&mut self.0.lock().expect("trace journal").rows)
    }

    /// Copy the most recently prepared draw into its host-owned picture envelope.
    pub fn last_draw(&self) -> Option<Observation> {
        self.0.lock().expect("trace journal").last_draw.clone()
    }

    /// Identity only, without copying the numeric row into a Canvas plan.
    pub fn last_draw_sequence(&self) -> Option<u64> {
        self.0
            .lock()
            .expect("trace journal")
            .last_draw
            .as_ref()
            .map(|row| row.sequence)
    }

    /// Called by a host only after successful submission and picture commit.
    pub fn acknowledge_picture(&self, sequence: u64) {
        self.0
            .lock()
            .expect("trace journal")
            .pictures
            .push(sequence);
    }

    /// Drain acknowledgements independently of prepared observations.
    pub fn take_pictures(&self) -> Vec<u64> {
        std::mem::take(&mut self.0.lock().expect("trace journal").pictures)
    }

    /// Decimal identities avoid truncating u64 through JavaScript Numbers.
    #[cfg(target_arch = "wasm32")]
    pub fn take_pictures_js(&self) -> Vec<String> {
        self.take_pictures()
            .into_iter()
            .map(|sequence| sequence.to_string())
            .collect()
    }

    pub(crate) fn observe(
        &self,
        client: &AnimatedClient,
        phase: &'static str,
        step: Option<FrameStep>,
        extent: Option<[u32; 2]>,
    ) {
        let mut journal = self.0.lock().expect("trace journal");
        if phase == "draw" {
            journal.last_draw = None;
        }
        if let Some(step) = step {
            journal.step = Some(step);
        }
        if let Some(extent) = extent {
            journal.extent = extent;
        }
        let step = journal.step;
        let camera = client.camera();
        let c = camera.corrections();
        let position = camera.position();
        let offset = c.offset();
        let mut values = vec![
            step.map_or(0., |s| s.unlagged_delta),
            step.map_or(0., |s| s.delta),
            step.map_or(0., |s| f64::from(s.whole_steps)),
            step.map_or(0., |s| f64::from(s.double_steps)),
            step.map_or(1., |s| s.lag_multiplier),
            position.x,
            position.y,
            offset.x,
            offset.y,
            f64::from(camera.follows()),
            camera.zoom(),
        ];
        let token = client.own_token().map(|t| t.serial());
        if let Some(snake) = client.own_snake() {
            let m = snake.metrics();
            // prepare_draw_antenna_resolved_staged excludes expired snakes from
            // both production renderers. Retain their stored channels, but emit
            // no projected draw positions. Unsupported families still fail closed.
            if !matches!(snake.active_face_geometry(), FaceGeometry::Paired(_)) {
                journal.failed = true;
                journal
                    .errors
                    .push(format!("{phase}: unsupported face layout"));
                return;
            }
            let h = snake.head_corrections();
            let a = snake.angle_corrections();
            let pupil = snake.eyes().pupil();
            values.extend([
                f64::from(m.expired),
                m.death_progress,
                m.spawn_progress,
                (m.spawn_progress * (1. - m.death_progress).max(0.)).powi(2),
                m.raw_head.x,
                m.raw_head.y,
                m.corrected_head.x,
                m.corrected_head.y,
                m.heading,
                m.corrected_heading,
                m.wanted_heading,
                snake.eyes().gaze_heading(),
                pupil.x,
                pupil.y,
                h.offset()[0],
                h.offset()[1],
                h.offset()[2],
                a.offset(),
                m.speed_per_32ms,
                m.visual_speed,
                f64::from(m.speed_per_32ms > m.boost_threshold_speed),
                m.boost_phase,
                m.logical_segments as f64,
                m.fam,
                m.displayed_length,
                0.64285 + 0.514285714 / ((m.logical_segments as f64 + 16.) / 36.).max(1.),
            ]);
            // Semantic centerline only; correction storage is private to presentation.
            values.push(snake.body().points().len() as f64);
            for p in snake.body().points() {
                let position = p.position();
                let offset = p.position_offset();
                values.extend([position.x + offset.x, position.y + offset.y]);
            }
            if !m.expired {
                let half = Point {
                    x: f64::from(journal.extent[0]) / 2.,
                    y: f64::from(journal.extent[1]) / 2.,
                };
                values.extend([
                    half.x + (m.corrected_head.x - position.x) * camera.zoom(),
                    half.y + (m.corrected_head.y - position.y) * camera.zoom(),
                ]);
                // Fail closed BEFORE publishing a partial numeric row. A1 requires
                // both projected pupils (contract/fixture links at module entry).
                match projected_pupils(
                    snake.active_face_geometry(),
                    snake.eyes().heading(),
                    pupil,
                    EyeLayoutInput {
                        corrected_head: m.corrected_head,
                        camera: position,
                        viewport_half: half,
                        scale: m.scale,
                        zoom: camera.zoom(),
                        death_progress: m.death_progress,
                    },
                ) {
                    Ok(pupils) => {
                        for p in pupils {
                            values.extend([p.x, p.y]);
                        }
                    }
                    Err(reason) => {
                        journal.failed = true;
                        journal.errors.push(format!("{phase}: {reason}"));
                        return;
                    }
                }
            }
        }
        let Some(next_sequence) = journal.next_sequence.checked_add(1) else {
            journal.failed = true;
            journal.errors.push("Observation identity exhausted".into());
            return;
        };
        let row = Observation {
            sequence: journal.next_sequence,
            phase,
            token,
            values,
        };
        journal.next_sequence = next_sequence;
        if phase == "draw" {
            journal.last_draw = Some(row.clone());
        }
        journal.rows.push(row);
    }

    /// Copy numeric observations and decimal identities into JS.
    #[cfg(target_arch = "wasm32")]
    pub fn take_js(&self) -> js_sys::Array {
        self.take()
            .into_iter()
            .map(|row| {
                let record = js_sys::Array::new();
                record.push(&row.phase.into());
                record.push(
                    &row.token
                        .map(|t| wasm_bindgen::JsValue::from_str(&t.to_string()))
                        .unwrap_or(wasm_bindgen::JsValue::NULL),
                );
                record.push(&js_sys::Float64Array::from(row.values.as_slice()));
                record.push(&row.sequence.to_string().into());
                record
            })
            .collect()
    }
}

fn projected_pupils(
    face: &FaceGeometry,
    heading: f64,
    pupil: Point,
    view: EyeLayoutInput,
) -> Result<[Point; 2], &'static str> {
    let FaceGeometry::Paired(face) = face else {
        return Err("unsupported face layout");
    };
    let layout = face
        .eyes
        .layout(heading, pupil, view)
        .map_err(|_| "paired pupil layout rejected")?;
    Ok(layout.pairs.map(|pair| pair.pupil))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation::SingleEyeGeometry;

    #[test]
    fn observation_identity_distinguishes_equal_values_and_survives_drains() {
        let recorder = Recorder::default();
        let client = AnimatedClient::new(0.).unwrap();
        recorder.observe(&client, "draw", None, None);
        recorder.observe(&client, "draw", None, None);
        let rows = recorder.take();
        assert_eq!(rows[0].values, rows[1].values);
        assert_ne!(rows[0].sequence, rows[1].sequence);
        assert_eq!(recorder.last_draw(), Some(rows[1].clone()));
        recorder.observe(&client, "draw", None, None);
        assert!(recorder.take()[0].sequence > rows[1].sequence);
    }

    #[test]
    fn pictures_acknowledge_only_the_submitted_identity_across_row_drains() {
        let recorder = Recorder::default();
        let client = AnimatedClient::new(0.).unwrap();
        recorder.observe(&client, "draw", None, None);
        let skipped = recorder.last_draw_sequence().unwrap();
        recorder.observe(&client, "draw", None, None);
        let submitted = recorder.last_draw_sequence().unwrap();
        assert_eq!(recorder.take().len(), 2);
        assert!(recorder.take_pictures().is_empty());
        recorder.acknowledge_picture(submitted);
        assert_ne!(skipped, submitted);
        assert_eq!(recorder.take_pictures(), [submitted]);
        assert!(recorder.take_pictures().is_empty());
    }

    #[test]
    fn unsupported_face_and_failed_layout_return_explicit_errors() {
        let view = EyeLayoutInput {
            corrected_head: Point::default(),
            camera: Point::default(),
            viewport_half: Point { x: 750., y: 422.5 },
            scale: 1.,
            zoom: 1.,
            death_progress: 0.,
        };
        let image = FaceGeometry::SingleImage(SingleEyeGeometry {
            base_size: 24.,
            pupil_size: 8.,
            pupil_travel: 4.,
        });
        assert_eq!(
            projected_pupils(&image, 0., Point::default(), view),
            Err("unsupported face layout")
        );
        assert_eq!(
            projected_pupils(
                &FaceGeometry::default(),
                0.,
                Point::default(),
                EyeLayoutInput {
                    death_progress: 1.,
                    ..view
                }
            ),
            Err("paired pupil layout rejected")
        );
    }
}
