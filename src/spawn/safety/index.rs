//! Conservative candidates for the immutable, fixed-width obstacle predicates.
use super::{Bounds, Obstacle};

const LIMIT: f64 = 1e7;
const CELL: f64 = 1024.;
const MAX_CELLS: usize = 4096;
const MAX_WORDS: usize = 65_536;

pub(super) fn boundable(bounds: Bounds) -> bool {
    [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
        .into_iter()
        .all(|v| v.is_finite() && v.abs() <= LIMIT)
        && bounds.min.x <= bounds.max.x
        && bounds.min.y <= bounds.max.y
}

fn expanded(bounds: Bounds, width: f64) -> Option<Bounds> {
    if !boundable(bounds) || !(0. ..=LIMIT).contains(&width) {
        return None;
    }
    // At this finite scale near() adds at most 0.01. One unit also covers
    // subtraction/union roundoff. Division by the power-of-two cell is exact.
    Some(Bounds {
        min: super::Point {
            x: bounds.min.x - width - 1.,
            y: bounds.min.y - width - 1.,
        },
        max: super::Point {
            x: bounds.max.x + width + 1.,
            y: bounds.max.y + width + 1.,
        },
    })
}

fn envelope(obstacle: &Obstacle) -> Option<Bounds> {
    if !obstacle.valid
        || ![
            obstacle.head_width,
            obstacle.body_width,
            obstacle.path_width,
            obstacle.inward_width,
            obstacle.departure_width,
            obstacle.departure_path_width,
        ]
        .into_iter()
        .all(|w| (0. ..=LIMIT).contains(&w))
    {
        return None;
    }
    let mut bounds = expanded(
        obstacle.bounds,
        obstacle
            .head_width
            .max(obstacle.body_width)
            .max(obstacle.departure_width),
    )?;
    let path = expanded(
        obstacle.path.bounds,
        obstacle.path_width.max(obstacle.departure_path_width),
    )?;
    bounds.include(path.min);
    bounds.include(path.max);
    if let Some(path) = &obstacle.inward {
        let inward = expanded(path.bounds, obstacle.inward_width)?;
        bounds.include(inward.min);
        bounds.include(inward.max);
    }
    Some(bounds)
}

#[derive(Clone, Copy)]
struct Cells {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

impl Cells {
    fn of(bounds: Bounds) -> Self {
        Self {
            min_x: (bounds.min.x / CELL).floor() as i32,
            min_y: (bounds.min.y / CELL).floor() as i32,
            max_x: (bounds.max.x / CELL).floor() as i32,
            max_y: (bounds.max.y / CELL).floor() as i32,
        }
    }
}

pub(super) struct ObstacleIndex {
    bounds: Cells,
    width: usize,
    words: usize,
    global: Vec<u64>,
    cells: Vec<u64>,
}

impl ObstacleIndex {
    pub(super) fn build(obstacles: &[Obstacle]) -> Option<Self> {
        let envelopes: Vec<_> = obstacles
            .iter()
            .map(|o| envelope(o).map(Cells::of))
            .collect();
        let mut bounds = envelopes.iter().flatten().next().copied()?;
        for b in envelopes.iter().flatten() {
            bounds.min_x = bounds.min_x.min(b.min_x);
            bounds.min_y = bounds.min_y.min(b.min_y);
            bounds.max_x = bounds.max_x.max(b.max_x);
            bounds.max_y = bounds.max_y.max(b.max_y);
        }
        let width = (bounds.max_x - bounds.min_x + 1) as usize;
        let height = (bounds.max_y - bounds.min_y + 1) as usize;
        let count = width.checked_mul(height)?;
        let words = obstacles.len().div_ceil(64);
        let storage = count.checked_mul(words)?;
        if count > MAX_CELLS || storage > MAX_WORDS {
            return None;
        }
        let mut index = Self {
            bounds,
            width,
            words,
            global: vec![0; words],
            cells: vec![0; storage],
        };
        for (slot, envelope) in envelopes.into_iter().enumerate() {
            let word = slot / 64;
            let bit = 1u64 << (slot % 64);
            if let Some(b) = envelope {
                for y in b.min_y..=b.max_y {
                    for x in b.min_x..=b.max_x {
                        let offset = index.offset(x, y) + word;
                        index.cells[offset] |= bit;
                    }
                }
            } else {
                // Invalid and unsupported obstacles are visited for every query,
                // at their original position, including outside the grid.
                index.global[word] |= bit;
            }
        }
        Some(index)
    }

    fn offset(&self, x: i32, y: i32) -> usize {
        (((y - self.bounds.min_y) as usize * self.width) + (x - self.bounds.min_x) as usize)
            * self.words
    }

    pub(super) fn candidates(&self, query: Bounds) -> Candidates<'_> {
        debug_assert!(boundable(query));
        let mut bounds = Cells::of(query);
        bounds.min_x = bounds.min_x.max(self.bounds.min_x);
        bounds.min_y = bounds.min_y.max(self.bounds.min_y);
        bounds.max_x = bounds.max_x.min(self.bounds.max_x);
        bounds.max_y = bounds.max_y.min(self.bounds.max_y);
        Candidates {
            index: self,
            bounds,
            word: 0,
            bits: 0,
        }
    }
}

pub(super) struct Candidates<'a> {
    index: &'a ObstacleIndex,
    bounds: Cells,
    word: usize,
    bits: u64,
}

impl Iterator for Candidates<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.bits != 0 {
                let bit = self.bits.trailing_zeros() as usize;
                self.bits &= self.bits - 1;
                return Some((self.word - 1) * 64 + bit);
            }
            if self.word == self.index.words {
                return None;
            }
            self.bits = self.index.global[self.word];
            for y in self.bounds.min_y..=self.bounds.max_y {
                for x in self.bounds.min_x..=self.bounds.max_x {
                    self.bits |= self.index.cells[self.index.offset(x, y) + self.word];
                }
            }
            self.word += 1;
        }
    }
}
