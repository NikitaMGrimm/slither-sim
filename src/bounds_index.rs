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
    (v / CELL).floor() as i64
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
    use crate::Point;

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
