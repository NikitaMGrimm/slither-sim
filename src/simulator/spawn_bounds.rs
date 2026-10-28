//! Admission broad phase; exact predicates and their traversal order stay in simulator.rs.
use super::*;

/// Outside this range use the original geometry, including its floating-point
/// behavior. Within it, one world unit exceeds the projection/cross-product
/// roundoff of the exact distance routines by many orders of magnitude.
pub(super) fn bounds(points: impl Iterator<Item = Point>) -> Option<Bounds> {
    let mut result = None;
    for p in points {
        if !p.finite() || p.x.abs() > 1e7 || p.y.abs() > 1e7 {
            return None;
        }
        result
            .get_or_insert_with(|| Bounds::between(p, p))
            .include(p);
    }
    result
}

pub(super) fn snake_bounds(s: &Snake) -> Option<Bounds> {
    bounds(std::iter::once(s.head).chain(s.body.iter().copied()))
}

pub(super) fn gap(a: Bounds, b: Bounds) -> f64 {
    (a.min.x - b.max.x)
        .max(b.min.x - a.max.x)
        .max(a.min.y - b.max.y)
        .max(b.min.y - a.max.y)
        .max(0.)
}

impl Simulator {
    pub(super) fn spawn_bounds(&self) -> &[Option<Bounds>] {
        // One body walk per queried world state, not per candidate. Retired
        // slots are filtered by alive at query time; replacements update only
        // their slot, including multiple admissions between physics ticks.
        self.spawn_bounds
            .get_or_init(|| self.snakes.iter().map(snake_bounds).collect())
    }

    pub(super) fn refresh_spawn_bounds(&mut self, slot: usize) {
        if let Some(bounds) = self.spawn_bounds.get_mut() {
            bounds[slot] = snake_bounds(&self.snakes[slot]);
        }
    }
}
