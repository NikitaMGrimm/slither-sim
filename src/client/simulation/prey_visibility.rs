//! Shared broad phase for per-recipient prey visibility.
//! Measured scope: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-host-prey-visibility/REPORT.md
//! Cell lookup: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-prey-cells/REPORT.md
use crate::{Prey, model::Point};

pub(super) struct PreyBounds {
    bounds: Vec<[f64; 4]>,
    all: Vec<usize>,
    x_cells: Option<XCells>,
}

struct XCells {
    first: i64,
    size: f64,
    cells: Vec<Vec<usize>>,
}

impl XCells {
    fn new(bounds: &[[f64; 4]], size: f64) -> Option<Self> {
        if bounds.len() < 16 || !size.is_finite() {
            return None;
        }
        let mut first = i64::MAX;
        let mut last = i64::MIN;
        for &[x0, _, x1, _] in bounds {
            if !x0.is_finite() || !x1.is_finite() {
                return None;
            }
            first = first.min((x0 / size) as i64);
            last = last.max((x1 / size) as i64);
        }
        // Sparse/extreme worlds keep the exhaustive path rather than allocating
        // a cell for every gap. This threshold only selects the broad phase.
        let span = last.checked_sub(first)?;
        if span >= 256 {
            return None;
        }
        let mut cells = vec![Vec::new(); span as usize + 1];
        for (index, &[x0, _, x1, _]) in bounds.iter().enumerate() {
            for cell in (x0 / size) as i64..=(x1 / size) as i64 {
                cells[(cell - first) as usize].push(index);
            }
        }
        Some(Self { first, size, cells })
    }

    fn near(&self, x: f64) -> &[usize] {
        ((x / self.size) as i64)
            .checked_sub(self.first)
            .and_then(|offset| usize::try_from(offset).ok())
            .and_then(|offset| self.cells.get(offset))
            .map_or(&[], Vec::as_slice)
    }
}

impl PreyBounds {
    pub(super) fn new(prey: &[Prey], vision: f64) -> Self {
        // Leave rounding slack rather than replacing hypot's inclusive edge
        // with rounded axis arithmetic. Infinite bounds simply reject nothing.
        let reach = (2. * vision).max(1.);
        let bounds: Vec<_> = prey
            .iter()
            .map(|prey| {
                let Point { x, y } = prey.position;
                [x - reach, y - reach, x + reach, y + reach]
            })
            .collect();
        // Insert each original inclusive x interval into every mapped cell it
        // touches. Truncating/saturating casts are monotone, so a point inside
        // that interval shares one of its cells, including negative/zero edges.
        // Cells append in world order: duplicate IDs must still choose the first
        // visible prey before broadcast's ID-ordered appearance/removal passes.
        // Keep the original bounds and hypot tests; cells only skip candidates.
        let x_cells = XCells::new(&bounds, reach);
        Self {
            bounds,
            all: (0..prey.len()).collect(),
            x_cells,
        }
    }

    /// A superset of visible prey in world order. Keep the original distance
    /// test after lookup, including its exact rounding and inclusive boundary.
    pub(super) fn near(&self, point: Point) -> impl Iterator<Item = usize> + '_ {
        let indices = match &self.x_cells {
            Some(cells) if point.x.is_finite() => cells.near(point.x),
            _ => &self.all,
        };
        indices.iter().copied().filter(move |&index| {
            let [x0, y0, x1, y1] = self.bounds[index];
            point.x >= x0 && point.x <= x1 && point.y >= y0 && point.y <= y1
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

    fn oracle(items: &[Prey], vision: f64, heads: &[Point]) {
        let grid = PreyBounds::new(items, vision);
        let reach = (2. * vision).max(1.);
        for &head in heads {
            // The original exhaustive broad phase, independent of cell mapping.
            let expected: Vec<_> = items
                .iter()
                .enumerate()
                .filter_map(|(i, prey)| {
                    let Point { x, y } = prey.position;
                    (head.x >= x - reach
                        && head.x <= x + reach
                        && head.y >= y - reach
                        && head.y <= y + reach)
                        .then_some(i)
                })
                .collect();
            let actual: Vec<_> = grid.near(head).collect();
            assert_eq!(actual, expected, "vision={vision}, head={head:?}");

            let mut expected = BTreeMap::new();
            for (i, prey) in items.iter().enumerate() {
                if prey.position.distance(head) <= vision {
                    expected.entry(prey.id).or_insert(i);
                }
            }
            let mut actual = BTreeMap::new();
            for i in grid.near(head) {
                if items[i].position.distance(head) <= vision {
                    actual.entry(items[i].id).or_insert(i);
                }
            }
            assert_eq!(actual, expected, "first visible duplicate must win");
        }
    }

    #[test]
    fn cells_match_exhaustive_order_and_visibility_on_randomized_and_boundary_geometry() {
        let mut random = 7_u64;
        let mut unit = || {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            (random >> 11) as f64 / (1_u64 << 53) as f64
        };
        for vision in [0_f64, f64::from_bits(1), 0.1, 128., 1500., 1e10, 1e100] {
            let reach = (2. * vision).max(1.);
            for count in [16, 17, 64, 257] {
                let mut items: Vec<_> = (0..count)
                    .map(|i| {
                        prey(
                            (i % 7) as u64,
                            (unit() * 16. - 8.) * reach,
                            (unit() * 16. - 8.) * reach,
                        )
                    })
                    .collect();
                // Put actual inclusive-visibility witnesses on both sides of
                // zero and cell edges, including the upper inserted cell.
                items[0] = prey(0, -vision / 2., 0.);
                items[1] = prey(0, vision / 2., 0.);
                items[2] = prey(1, -reach, 0.);
                items[3] = prey(1, reach, 0.);
                assert!(PreyBounds::new(&items, vision).x_cells.is_some());
                let mut heads = Vec::new();
                for x in [-reach, -vision / 2., -0., 0., vision / 2., reach] {
                    heads.extend([x.next_down(), x, x.next_up()].map(|x| Point { x, y: 0. }));
                }
                for prey in items.iter().take(16) {
                    for x in [
                        prey.position.x - vision,
                        prey.position.x + vision,
                        prey.position.x - reach,
                        prey.position.x + reach,
                    ] {
                        heads.extend([x.next_down(), x, x.next_up()].map(|x| Point {
                            x,
                            y: prey.position.y,
                        }));
                    }
                }
                heads.extend((0..64).map(|_| Point {
                    x: (unit() * 20. - 10.) * reach,
                    y: (unit() * 20. - 10.) * reach,
                }));
                oracle(&items, vision, &heads);
            }
        }
    }

    #[test]
    fn fallback_saturation_and_rebuilding_preserve_the_exhaustive_path() {
        for center in [-1e300_f64, -1_048_576., -0., 0., 1_048_576., 1e300] {
            let mut items: Vec<_> = (0..32)
                .map(|i| prey((i % 3) as u64, center + i as f64, 0.))
                .collect();
            let heads = [
                Point { x: center, y: 0. },
                Point {
                    x: center + 16.,
                    y: 0.,
                },
                Point {
                    x: -f64::MAX,
                    y: 0.,
                },
                Point { x: f64::MAX, y: 0. },
            ];
            assert!(PreyBounds::new(&items, 128.).x_cells.is_some());
            oracle(&items, 128., &heads);
            items.remove(0);
            items.reverse();
            items.push(prey(0, center, 0.));
            items.retain(|prey| prey.id != 2);
            oracle(&items, 128., &heads);
            for item in &mut items {
                item.position.x = -item.position.x;
            }
            oracle(&items, 128., &heads);
        }
        let mut items = vec![prey(0, 0., 0.); 16];
        items[1].position.x = 1e300;
        assert!(PreyBounds::new(&items, 1.).x_cells.is_none());
        oracle(&items, 1., &[Point::default()]);
        assert!(PreyBounds::new(&items[..15], 1500.).x_cells.is_none());
        oracle(&items[..15], 1500., &[Point::default()]);
        for vision in [1e300, f64::MAX] {
            oracle(
                &items,
                vision,
                &[
                    Point::default(),
                    Point {
                        x: f64::INFINITY,
                        y: 0.,
                    },
                ],
            );
        }
        for x in [f64::NEG_INFINITY, f64::INFINITY, f64::NAN] {
            items[0].position.x = x;
            assert!(PreyBounds::new(&items, 1500.).x_cells.is_none());
            oracle(&items, 1500., &[Point::default(), Point { x, y: 0. }]);
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
