//! Opt-in offline observations. No renderer, update, random request or policy is invoked.
//! The flat numeric ABI is decoded by tools/feel-replay/observation.mjs.

use crate::presentation::{AnimatedClient, EyeLayoutInput, FaceGeometry, FrameStep};
use crate::state::Point;
use std::sync::{Arc, Mutex};

/// An immutable copy of one exact core boundary, including a staged draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Observation {
    /// Core boundary kind.
    pub phase: &'static str,
    /// Retained own identity, without conversion through f64.
    pub token: Option<u64>,
    /// Raw binary64 cells; f32 storage is widened exactly.
    pub values: Vec<f64>,
}

#[derive(Debug, Default)]
struct Journal {
    rows: Vec<Observation>,
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
        self.0
            .lock()
            .expect("trace journal")
            .rows
            .iter()
            .rev()
            .find(|r| r.phase == "draw")
            .cloned()
    }

    pub(crate) fn observe(
        &self,
        client: &AnimatedClient,
        phase: &'static str,
        step: Option<FrameStep>,
        extent: Option<[u32; 2]>,
    ) {
        let mut journal = self.0.lock().expect("trace journal");
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
            c.index() as f64,
            c.pending() as f64,
            f64::from(camera.follows()),
            camera.zoom(),
        ];
        values.extend(c.samples().iter().flatten().copied());
        let token = client.own_token().map(|t| t.serial());
        if let Some(snake) = client.own_snake() {
            let m = snake.metrics();
            let h = snake.head_corrections();
            let a = snake.angle_corrections();
            let l = snake.length();
            let pupil = snake.eyes().pupil();
            values.extend([
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
                h.index() as f64,
                f64::from(h.pending()),
                a.index() as f64,
                f64::from(a.pending()),
                l.index() as f64,
                f64::from(l.pending()),
                0.64285 + 0.514285714 / ((m.logical_segments as f64 + 16.) / 36.).max(1.),
            ]);
            values.extend(h.samples().iter().flatten().map(|v| f64::from(*v)));
            values.extend(a.samples().iter().map(|v| f64::from(*v)));
            values.extend(l.samples().iter().map(|v| f64::from(*v)));
            let half = Point {
                x: f64::from(journal.extent[0]) / 2.,
                y: f64::from(journal.extent[1]) / 2.,
            };
            values.extend([
                half.x + (m.corrected_head.x - position.x) * camera.zoom(),
                half.y + (m.corrected_head.y - position.y) * camera.zoom(),
            ]);
            // This lane currently authors paired-eye fixtures. Reject unsupported
            // faces rather than manufacture their projected pupil observations.
            if let FaceGeometry::Paired(face) = snake.active_face_geometry() {
                if let Ok(layout) = face.eyes.layout(
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
                    for pair in layout.pairs {
                        values.extend([pair.pupil.x, pair.pupil.y]);
                    }
                }
            }
        }
        journal.rows.push(Observation {
            phase,
            token,
            values,
        });
    }

    /// Copy into JS native Numbers before the A1 writer; never plain JSON.
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
                record
            })
            .collect()
    }
}
