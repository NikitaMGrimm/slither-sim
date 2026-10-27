//! Conservative broad phase over axis-aligned bounds.
//!
//! Callers keep their exact narrow tests; this index only skips items whose
//! bounds cannot be within a given reach of a query box, so results are
//! unchanged. Items are hashed into coarse cells by their bounds; a hash
//! collision adds candidates and never removes one.
use crate::physics::Bounds;

const CELL: f64 = 512.0;
/// Bounds spanning more cells than this per axis stay in an always-tested list.
const MAX_SPAN: i64 = 16;

pub(crate) struct BoundsIndex {
    mask: usize,
    starts: Vec<u32>,
    entries: Vec<u32>,
    /// Positions tested for every probe: non-finite or very large bounds.
    always: Vec<u32>,
}

fn cell(v: f64) -> i64 {
    // Cell numbering only needs to be monotone: overlapping coordinate ranges
    // must share a cell. Truncation (including saturating float-to-int casts)
    // preserves that property and avoids baseline x86-64's floor library call.
    // The wider cell around zero adds candidates; callers keep exact tests.
    (v / CELL) as i64
}

fn bucket(x: i64, y: i64, mask: usize) -> usize {
    ((x as u64).wrapping_mul(73856093) ^ (y as u64).wrapping_mul(19349663)) as usize & mask
}

type Span = Option<(i64, i64, i64, i64)>;

fn span(b: &Bounds) -> Span {
    let finite = b.min.finite() && b.max.finite();
    let (lx, hx, ly, hy) = (cell(b.min.x), cell(b.max.x), cell(b.min.y), cell(b.max.y));
    (finite && hx.saturating_sub(lx) < MAX_SPAN && hy.saturating_sub(ly) < MAX_SPAN)
        .then_some((lx, hx, ly, hy))
}

impl BoundsIndex {
    /// `bounds[k]` belongs to item `k`.
    pub(crate) fn new(bounds: &[Bounds]) -> Self {
        let spans: Vec<Span> = bounds.iter().map(span).collect();
        let cells: usize = spans
            .iter()
            .flatten()
            .map(|&(lx, hx, ly, hy)| ((hx - lx + 1) * (hy - ly + 1)) as usize)
            .sum();
        let mask = (cells * 2).next_power_of_two().max(64) - 1;
        let mut starts = vec![0u32; mask + 2];
        let mut always = vec![];
        for (k, s) in spans.iter().enumerate() {
            match *s {
                Some((lx, hx, ly, hy)) => {
                    for x in lx..=hx {
                        for y in ly..=hy {
                            starts[bucket(x, y, mask) + 1] += 1;
                        }
                    }
                }
                None => always.push(k as u32),
            }
        }
        for b in 1..starts.len() {
            starts[b] += starts[b - 1];
        }
        let mut fill = starts.clone();
        let mut entries = vec![0u32; cells];
        for (k, s) in spans.iter().enumerate() {
            if let Some((lx, hx, ly, hy)) = *s {
                for x in lx..=hx {
                    for y in ly..=hy {
                        let b = bucket(x, y, mask);
                        entries[fill[b] as usize] = k as u32;
                        fill[b] += 1;
                    }
                }
            }
        }
        Self {
            mask,
            starts,
            entries,
            always,
        }
    }

    /// Collect, in descending item order without duplicates, every item whose
    /// bounds could satisfy `bounds.near(query, r)` for any `r <= reach`.
    /// Returns false when the query region is too large or not finite; the
    /// caller must then test every item.
    pub(crate) fn candidates(&self, query: Bounds, reach: f64, out: &mut Vec<u32>) -> bool {
        out.clear();
        // `near` compares against `query.min - r` and `query.max + r`; these
        // cells contain that range because rounding is monotone and `r <= reach`.
        let (lx, hx) = (cell(query.min.x - reach), cell(query.max.x + reach));
        let (ly, hy) = (cell(query.min.y - reach), cell(query.max.y + reach));
        if !(query.min.finite() && query.max.finite() && reach.is_finite())
            || hx.saturating_sub(lx) >= MAX_SPAN
            || hy.saturating_sub(ly) >= MAX_SPAN
        {
            return false;
        }
        for x in lx..=hx {
            for y in ly..=hy {
                let b = bucket(x, y, self.mask);
                out.extend_from_slice(
                    &self.entries[self.starts[b] as usize..self.starts[b + 1] as usize],
                );
            }
        }
        out.extend_from_slice(&self.always);
        out.sort_unstable_by(|a, b| b.cmp(a));
        out.dedup();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::{collision_distance, head_tip, strikes};
    use crate::Point;

    #[test]
    fn filtered_candidates_match_exhaustive_on_numeric_boundaries() {
        let limit = (i64::MAX as f64) * CELL;
        let coordinates = [
            -f64::MAX,
            -limit,
            (-limit).next_up(),
            -1024.0,
            (-512.0f64).next_down(),
            -512.0,
            (-512.0f64).next_up(),
            -f64::MIN_POSITIVE,
            -0.0,
            0.0,
            f64::MIN_POSITIVE,
            512.0f64.next_down(),
            512.0,
            512.0f64.next_up(),
            1024.0,
            limit.next_down(),
            limit,
            f64::MAX,
        ];
        for n in [0, 31, 32, 63, 64, 65, 127, 128, 500, 511, 512, 513] {
            let mut bounds: Vec<_> = (0..n)
                .map(|i| {
                    let x = coordinates[i % coordinates.len()];
                    let y = (i % 5) as f64 * CELL - CELL;
                    Bounds::between(
                        Point { x, y },
                        Point {
                            x: x.next_up(),
                            y: y + 7.0,
                        },
                    )
                })
                .collect();
            if n > 4 {
                bounds[1].min.x = f64::NAN;
                bounds[2].max.y = f64::INFINITY;
                bounds[3] = Bounds::between(Point { x: -1e6, y: -1e6 }, Point { x: 1e6, y: 1e6 });
            }
            let index = BoundsIndex::new(&bounds);
            let mut out = Vec::new();
            for x in coordinates.into_iter().chain([f64::NAN, f64::INFINITY]) {
                for reach in [
                    0.0,
                    28.275,
                    512.0f64.next_down(),
                    512.0,
                    512.0f64.next_up(),
                    1e308,
                ] {
                    let query = Bounds::between(Point { x, y: -512.0 }, Point { x, y: 512.0 });
                    if !index.candidates(query, reach, &mut out) {
                        out.extend((0..n as u32).rev());
                    }
                    assert!(out.windows(2).all(|w| w[0] > w[1]));
                    let actual: Vec<_> = out
                        .iter()
                        .copied()
                        .filter(|&k| bounds[k as usize].near(query, reach))
                        .collect();
                    let exhaustive: Vec<_> = (0..n as u32)
                        .rev()
                        .filter(|&k| bounds[k as usize].near(query, reach))
                        .collect();
                    assert_eq!(actual, exhaustive, "n={n}, x={x:?}, reach={reach:?}");
                }
            }
        }
    }

    struct SnakeLine {
        head: Point,
        heading: f64,
        segments: usize,
        last_tip: Point,
        body: Vec<Point>,
    }

    fn resolve(
        snakes: &[SnakeLine],
        order: &[usize],
        boundary: &[bool],
        indexed: bool,
    ) -> (Vec<(usize, Option<usize>)>, Vec<usize>) {
        let tips: Vec<_> = snakes
            .iter()
            .map(|s| head_tip(s.head, s.heading, s.segments))
            .collect();
        let boxes: Vec<_> = snakes
            .iter()
            .map(|s| {
                let mut b = Bounds::between(s.head, s.head);
                for &p in &s.body {
                    b.include(p);
                }
                b
            })
            .collect();
        let index = BoundsIndex::new(&order.iter().map(|&j| boxes[j]).collect::<Vec<_>>());
        let longest = order.iter().map(|&j| snakes[j].segments).max().unwrap_or(0);
        let mut candidates = Vec::new();
        let mut fallen = vec![false; snakes.len()];
        let mut deaths = Vec::new();
        let mut kills = vec![0; snakes.len()];
        // Same oldest-first/newer-target-first resolution as Simulator::advance.
        // The exhaustive arm calls the existing exact strikes test without an
        // index or bounds rejection, independently checking both broad phases.
        for &i in order {
            if boundary[i] {
                fallen[i] = true;
                deaths.push((i, None));
                continue;
            }
            let s = &snakes[i];
            let probe = Bounds::between(tips[i], tips[i]);
            if !(indexed
                && index.candidates(
                    probe,
                    3.666 * collision_distance(s.segments, longest),
                    &mut candidates,
                ))
            {
                candidates.clear();
                candidates.extend((0..order.len() as u32).rev());
            }
            let killer = candidates.iter().map(|&k| order[k as usize]).find(|&j| {
                if i == j || fallen[j] {
                    return false;
                }
                let other = &snakes[j];
                (!indexed
                    || boxes[j].near(
                        probe,
                        3.666 * collision_distance(s.segments, other.segments),
                    ))
                    && strikes(
                        s.last_tip,
                        tips[i],
                        s.heading,
                        s.segments,
                        other.segments,
                        [tips[j], other.head]
                            .into_iter()
                            .chain(other.body.iter().copied()),
                    )
            });
            if let Some(j) = killer {
                fallen[i] = true;
                deaths.push((i, Some(j)));
                kills[j] += 1;
            }
        }
        (deaths, kills)
    }

    #[test]
    fn indexed_deaths_match_exhaustive_randomized_and_boundary_geometry() {
        let mut state = 0x8a71_f602_35ce_b49du64;
        let mut unit = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        for round in 0..100 {
            let mut snakes = Vec::new();
            let mut boundary = Vec::new();
            for i in 0..48 {
                let extent = if i % 3 == 0 { 80.0 } else { 20000.0 };
                let head = Point {
                    x: (unit() - 0.5) * extent,
                    y: (unit() - 0.5) * extent,
                };
                let heading = (unit() - 0.5) * std::f64::consts::TAU;
                let segments = [2, 3, 10, 100, 430, 2048][(unit() * 6.0) as usize];
                let tip = head_tip(head, heading, segments);
                let body = (0..1 + (unit() * 10.0) as usize)
                    .map(|_| Point {
                        x: head.x + (unit() - 0.5) * if i % 11 == 0 { 20000.0 } else { 500.0 },
                        y: head.y + (unit() - 0.5) * 500.0,
                    })
                    .collect();
                snakes.push(SnakeLine {
                    head,
                    heading,
                    segments,
                    last_tip: Point {
                        x: tip.x + (unit() - 0.5) * 20.0,
                        y: tip.y + (unit() - 0.5) * 20.0,
                    },
                    body,
                });
                boundary.push(unit() < 0.05);
            }
            let mut order: Vec<_> = (0..snakes.len()).filter(|i| i % 7 != round % 7).collect();
            for i in (1..order.len()).rev() {
                order.swap(i, (unit() * (i + 1) as f64) as usize);
            }
            assert_eq!(
                resolve(&snakes, &order, &boundary, true),
                resolve(&snakes, &order, &boundary, false),
                "round {round}"
            );
        }
        for origin in [-1024.0, -512.0, -0.0, 0.0, 512.0, 1024.0, 1e20, -1e20] {
            let reach = 3.666 * collision_distance(2, 2);
            for edge in [reach.next_down(), reach, reach.next_up(), 0.975 * 14.5] {
                let mut snakes = vec![SnakeLine {
                    head: Point {
                        x: origin - 14.5,
                        y: 0.0,
                    },
                    heading: 0.0,
                    segments: 2,
                    last_tip: Point {
                        x: origin - 1.0,
                        y: 0.0,
                    },
                    body: vec![Point {
                        x: origin - 30.0,
                        y: 0.0,
                    }],
                }];
                for i in 1..64 {
                    let head = if i < 3 {
                        Point {
                            x: origin - reach,
                            y: -10.0,
                        }
                    } else {
                        Point {
                            x: origin + i as f64 * 2000.0,
                            y: 2000.0,
                        }
                    };
                    snakes.push(SnakeLine {
                        head,
                        heading: 0.0,
                        segments: 2,
                        last_tip: head_tip(head, 0.0, 2),
                        body: vec![Point {
                            x: if i < 3 { origin + edge } else { head.x - 42.0 },
                            y: 10.0,
                        }],
                    });
                }
                let order: Vec<_> = (0..64).collect();
                let mut boundary = vec![false; 64];
                for late_boundary in [false, true] {
                    boundary[2] = late_boundary;
                    assert_eq!(
                        resolve(&snakes, &order, &boundary, true),
                        resolve(&snakes, &order, &boundary, false),
                        "origin={origin}, edge={edge}, boundary={late_boundary}"
                    );
                }
            }
        }
    }

    #[test]
    fn newer_killer_and_later_boundary_death_keep_attribution() {
        let snakes: Vec<_> = (0..64)
            .map(|i| {
                let x = if i < 3 { 0.0 } else { i as f64 * 2000.0 };
                SnakeLine {
                    head: Point { x, y: 0.0 },
                    heading: 0.0,
                    segments: 2,
                    last_tip: Point {
                        x: x + 13.0,
                        y: 0.0,
                    },
                    body: vec![Point {
                        x: x + 28.0,
                        y: 0.0,
                    }],
                }
            })
            .collect();
        let order: Vec<_> = (0..64).collect();
        let mut boundary = vec![false; 64];
        let (deaths, kills) = resolve(&snakes, &order, &boundary, true);
        assert_eq!(deaths, vec![(0, Some(2)), (1, Some(2))]);
        assert_eq!(kills[2], 2);
        boundary[2] = true;
        let expected = resolve(&snakes, &order, &boundary, false);
        assert_eq!(expected.0, vec![(0, Some(2)), (1, Some(2)), (2, None)]);
        assert_eq!(expected.1[2], 2);
        assert_eq!(resolve(&snakes, &order, &boundary, true), expected);
    }

    #[test]
    fn candidates_include_every_bounds_match() {
        let mut state = 0x9e37_79b9_u64;
        let mut unit = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let mut out = vec![];
        for round in 0..200 {
            let mut bounds: Vec<Bounds> = (0..80)
                .map(|_| {
                    let a = Point {
                        x: (unit() - 0.5) * 2e4,
                        y: (unit() - 0.5) * 2e4,
                    };
                    let size = if unit() < 0.1 { 2e4 } else { 2e3 };
                    Bounds::between(
                        a,
                        Point {
                            x: a.x + unit() * size,
                            y: a.y + unit() * size,
                        },
                    )
                })
                .collect();
            if round % 10 == 0 {
                bounds[3].max.x = f64::NAN;
                bounds[5].min.y = f64::NEG_INFINITY;
            }
            let index = BoundsIndex::new(&bounds);
            for _ in 0..200 {
                let point = Point {
                    x: (unit() - 0.5) * 2.4e4,
                    y: (unit() - 0.5) * 2.4e4,
                };
                let reach = unit() * 400.0;
                let extent = if unit() < 0.5 { 0.0 } else { unit() * 900.0 };
                let query = Bounds::between(
                    point,
                    Point {
                        x: point.x + (unit() - 0.5) * extent,
                        y: point.y + (unit() - 0.5) * extent,
                    },
                );
                assert!(index.candidates(query, reach, &mut out));
                assert!(out.windows(2).all(|w| w[0] > w[1]));
                for (k, b) in bounds.iter().enumerate() {
                    if b.near(query, reach) {
                        assert!(out.contains(&(k as u32)), "{k} {b:?} {query:?} {reach}");
                    }
                }
            }
            let nan = Point {
                x: f64::NAN,
                y: 0.0,
            };
            assert!(!index.candidates(Bounds::between(nan, nan), 10.0, &mut out));
        }
    }
}
