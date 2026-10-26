//! Shared broad phase for per-recipient prey visibility.
//! Measured scope: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-host-prey-visibility/REPORT.md
use crate::{Prey, model::Point};

pub(super) struct PreyBounds(Vec<[f64; 4]>);
impl PreyBounds {
    pub(super) fn new(prey: &[Prey], vision: f64) -> Self {
        // Leave rounding slack rather than replacing hypot's inclusive edge
        // with rounded axis arithmetic. Infinite bounds simply reject nothing.
        let reach = (2. * vision).max(1.);
        Self(
            prey.iter()
                .map(|prey| {
                    let Point { x, y } = prey.position;
                    [x - reach, y - reach, x + reach, y + reach]
                })
                .collect(),
        )
    }

    /// A superset of visible prey in world order. Keep the original distance
    /// test after lookup, including its exact rounding and inclusive boundary.
    pub(super) fn near(&self, point: Point) -> impl Iterator<Item = usize> + '_ {
        self.0
            .iter()
            .enumerate()
            .filter_map(move |(index, &[x0, y0, x1, y1])| {
                (point.x >= x0 && point.x <= x1 && point.y >= y0 && point.y <= y1).then_some(index)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn prey(id: u64, x: f64, y: f64) -> Prey {
        Prey {
            id,
            position: Point { x, y },
            heading: 0.,
            target_heading: 0.,
            speed: 1.,
            size: 1.,
        }
    }

    #[test]
    fn candidates_preserve_exact_visibility_world_order_and_duplicate_ids() {
        for vision in [f64::from_bits(1), 0.1, 128., 1500., 1e10, 1e300, f64::MAX] {
            let mut items = Vec::new();
            for x in [
                -1e300, -32768., -3000., -256., -0., 0., 256., 3000., 32768., 1e300,
            ] {
                for y in [-3000., 0., 3000.] {
                    items.push(prey((items.len() % 7) as u64, x, y));
                }
            }
            let grid = PreyBounds::new(&items, vision);
            for anchor in &items {
                for x in [
                    anchor.position.x,
                    anchor.position.x + vision,
                    anchor.position.x - vision,
                ] {
                    for x in [x.next_down(), x, x.next_up()] {
                        if !x.is_finite() {
                            continue;
                        }
                        let head = Point {
                            x,
                            y: anchor.position.y,
                        };
                        let mut expected = BTreeMap::new();
                        let indices: Vec<_> = grid.near(head).collect();
                        for (i, prey) in items.iter().enumerate() {
                            if prey.position.distance(head) <= vision {
                                assert!(indices.binary_search(&i).is_ok(), "omitted prey {i}");
                                expected.entry(prey.id).or_insert(i);
                            }
                        }
                        assert!(indices.windows(2).all(|w| w[0] < w[1]));
                        let mut actual = BTreeMap::new();
                        for &i in &indices {
                            if items[i].position.distance(head) <= vision {
                                actual.entry(items[i].id).or_insert(i);
                            }
                        }
                        assert_eq!(actual, expected, "vision={vision}, head={head:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn rebuilding_tracks_motion_and_rejects_distant_candidates() {
        let mut items = vec![prey(0, 0., 0.); 16];
        let before = PreyBounds::new(&items, 1500.);
        assert_eq!(
            before.near(Point::default()).collect::<Vec<_>>(),
            (0..16).collect::<Vec<_>>()
        );
        for item in &mut items {
            item.position.x = 30_000.;
        }
        let after = PreyBounds::new(&items, 1500.);
        assert!(after.near(Point::default()).next().is_none());
    }
}
