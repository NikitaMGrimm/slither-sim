//! Bounded geometry copies at the existing preparation seam, never a second draw.
use super::MAX_BODY_TRACE_POINTS;
use crate::{
    presentation::{AnimatedClient, PreparedAccessorySnake, SnakeMetrics},
    state::Point,
};

/// Separate from the feel-trace v3 numeric ABI; coordinates are absolute world units.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometryObservation {
    /// Identity of the matching prepared feel observation, including superseded draws.
    pub sequence: u64,
    /// Actual draw camera, before screen projection.
    pub camera: Point,
    /// Actual world-to-backing scale.
    pub zoom: f64,
    /// Selected visible snakes in painter order.
    pub snakes: Vec<GeometrySnake>,
}
/// Original prepared head/body and admitted packet state for one appearance.
#[derive(Clone, Debug, PartialEq)]
pub struct GeometrySnake {
    /// Stable appearance identity, not a wire-ID-only association.
    pub token: u64,
    /// Whether this is the original own appearance.
    pub own: bool,
    /// Prepared metrics, including raw/corrected head, heading and scale.
    pub metrics: SnakeMetrics,
    /// Actual painter centers, in head-to-tail order, widened from f32.
    pub body: Vec<[f64; 2]>,
    /// Latest admitted packet head, translated back to absolute coordinates.
    pub packet_head: Point,
}
#[derive(Debug)]
pub(super) struct Journal {
    ids: [u16; 2],
    count: usize,
    pub rows: Vec<GeometryObservation>,
}
impl Journal {
    pub fn new(ids: [u16; 2]) -> Self {
        Self {
            ids,
            count: 0,
            rows: Vec::new(),
        }
    }
    pub fn observe(
        &mut self,
        sequence: Option<u64>,
        client: &AnimatedClient,
        prepared: &[PreparedAccessorySnake],
    ) -> bool {
        // The finite probe needs fewer than 2,000 draws. Never thin a larger run.
        if self.count == 4096 || sequence.is_none() {
            return false;
        }
        let mut snakes = Vec::new();
        for p in prepared
            .iter()
            .map(|p| &p.snake)
            .filter(|s| self.ids.contains(&s.metrics.id))
        {
            let body = p
                .body_geometry
                .as_ref()
                .map(|b| b.samples.as_slice())
                .unwrap_or_default();
            if body.len() > MAX_BODY_TRACE_POINTS || snakes.len() == 2 {
                return false;
            }
            let Some(packet) = client.received().snakes.get(&p.metrics.id) else {
                return false;
            };
            let center = client
                .received()
                .setup
                .as_ref()
                .map_or(0., |s| s.grid_center);
            snakes.push(GeometrySnake {
                token: p.token.serial(),
                own: p.own,
                metrics: p.metrics,
                body: body
                    .iter()
                    .map(|p| [f64::from(p.x), f64::from(p.y)])
                    .collect(),
                packet_head: Point {
                    x: packet.head.x + center,
                    y: packet.head.y + center,
                },
            });
        }
        self.rows.push(GeometryObservation {
            sequence: sequence.expect("checked"),
            camera: client.camera().position(),
            zoom: client.camera().zoom(),
            snakes,
        });
        self.count += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_geometry_never_subsamples_or_invents_a_picture_id() {
        let client = AnimatedClient::new(0.).unwrap();
        let mut journal = Journal::new([7, 8]);
        assert!(!journal.observe(None, &client, &[]));
        for sequence in 0..4096 {
            assert!(journal.observe(Some(sequence), &client, &[]));
        }
        assert!(!journal.observe(Some(4096), &client, &[]));
        assert_eq!(journal.rows.len(), 4096);
        assert_eq!(journal.rows.last().unwrap().sequence, 4095);
    }
}
