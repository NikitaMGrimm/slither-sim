//! Caches for the step's food paths. Neither one can change a result: the cell
//! occupancy cache is compared with `Simulator::food` bit for bit before it is
//! trusted, and the eater grid omits only pickup candidates that cannot qualify.
use crate::model::{Food, Origin, Point};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

pub(crate) fn food_cell(x: f64, y: f64, size: f64) -> (i64, i64) {
    ((x / size).floor() as i64, (y / size).floor() as i64)
}

/// Multiply-rotate hash for cell keys; only lookups use it, never iteration order.
#[derive(Default)]
struct CellHasher(u64);
impl Hasher for CellHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b.into());
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    fn write_i64(&mut self, n: i64) {
        self.write_u64(n as u64);
    }
    fn finish(&self) -> u64 {
        // The final multiply leaves the low bits that pick a bucket weakest.
        self.0.rotate_left(26)
    }
}

type Counts = HashMap<(i64, i64), usize, BuildHasherDefault<CellHasher>>;

/// The fields of one pellet that occupancy and the ambient count read.
#[derive(Clone, Copy, PartialEq)]
struct Entry {
    x: u64,
    y: u64,
    ambient: bool,
}
impl Entry {
    fn of(food: &Food) -> Self {
        Self {
            x: food.x.to_bits(),
            y: food.y.to_bits(),
            ambient: food.origin == Some(Origin::Ambient),
        }
    }
}

/// Per-cell pellet counts and the ambient count, derived from a mirror of the
/// food list. Every count is always a function of `entries`; the mirror is
/// trusted only after comparing it with the current food, so external edits
/// between steps force a rebuild.
#[derive(Clone, Default)]
pub(crate) struct FoodCache {
    entries: Vec<Entry>,
    ambient: usize,
    /// At most three cell sizes: boost, death and ambient admission.
    grids: Vec<(u64, Counts)>,
    /// Inside a step nothing else can edit food, so one comparison per step
    /// suffices while every internal edit is mirrored.
    open: bool,
    verified: bool,
}
impl std::fmt::Debug for FoodCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoodCache").finish_non_exhaustive()
    }
}
impl FoodCache {
    /// Start a span in which only mirrored edits change food.
    pub(crate) fn begin(&mut self) {
        self.open = true;
        self.verified = false;
    }
    pub(crate) fn end(&mut self) {
        self.open = false;
        self.verified = false;
    }
    pub(crate) fn is_open(&self) -> bool {
        self.open
    }
    fn sync(&mut self, food: &[Food]) {
        if self.verified {
            return;
        }
        let same = self.entries.len() == food.len()
            && self
                .entries
                .iter()
                .zip(food)
                .all(|(e, f)| *e == Entry::of(f));
        if !same {
            self.entries.clear();
            self.entries.extend(food.iter().map(Entry::of));
            self.ambient = self.entries.iter().filter(|e| e.ambient).count();
            self.grids.clear();
        }
        self.verified = self.open;
    }
    /// Pellets in `food` with ambient origin.
    pub(crate) fn ambient(&mut self, food: &[Food]) -> usize {
        self.sync(food);
        self.ambient
    }
    /// Whether the pellet's cell holds fewer than `max` pellets of `food`.
    pub(crate) fn has_room(&mut self, food: &[Food], pellet: &Food, size: f64, max: usize) -> bool {
        self.sync(food);
        let key = size.to_bits();
        let grid = match self.grids.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                if self.grids.len() == 3 {
                    self.grids.remove(0);
                }
                let mut counts = Counts::default();
                for e in &self.entries {
                    *counts
                        .entry(food_cell(f64::from_bits(e.x), f64::from_bits(e.y), size))
                        .or_default() += 1;
                }
                self.grids.push((key, counts));
                self.grids.len() - 1
            }
        };
        self.grids[grid]
            .1
            .get(&food_cell(pellet.x, pellet.y, size))
            .map_or(0, |&count| count)
            < max
    }
    /// Mirror `food.push(pellet)`.
    pub(crate) fn push(&mut self, pellet: &Food) {
        let entry = Entry::of(pellet);
        self.ambient += usize::from(entry.ambient);
        for (key, counts) in &mut self.grids {
            *counts
                .entry(food_cell(pellet.x, pellet.y, f64::from_bits(*key)))
                .or_default() += 1;
        }
        self.entries.push(entry);
    }
    /// Mirror removing every pellet whose `removed` flag is set.
    pub(crate) fn remove(&mut self, removed: &[bool]) {
        if removed.len() != self.entries.len() {
            // Already stale; the next comparison rebuilds.
            self.entries.clear();
            self.ambient = 0;
            self.grids.clear();
            return;
        }
        for (e, _) in self.entries.iter().zip(removed).filter(|(_, r)| **r) {
            self.ambient -= usize::from(e.ambient);
            for (key, counts) in &mut self.grids {
                let cell = food_cell(
                    f64::from_bits(e.x),
                    f64::from_bits(e.y),
                    f64::from_bits(*key),
                );
                if let Some(count) = counts.get_mut(&cell) {
                    *count -= 1;
                    if *count == 0 {
                        counts.remove(&cell);
                    }
                }
            }
        }
        let mut i = 0;
        self.entries.retain(|_| {
            let keep = !removed[i];
            i += 1;
            keep
        });
    }
}

const CELL: f64 = 128.0;
/// Eaters with center coordinates and radius within this bound are indexed.
/// Below 2^20 rounding in the cover bounds stays under 1e-9, so the cover
/// contains every pellet the exact pickup test can accept; see `EaterGrid::build`.
const REGULAR: f64 = (1u64 << 20) as f64;
const MAX_CELLS: usize = 1 << 20;
const NONE: u32 = u32::MAX;

/// Dense grid of pickup circles, with each cell listing eaters in ascending order.
#[derive(Clone, Default)]
pub(crate) struct EaterGrid {
    /// Cell bounds, `[lo, end)`, as exact small integers.
    lo_x: f64,
    lo_y: f64,
    end_x: f64,
    end_y: f64,
    lo: (i64, i64),
    width: usize,
    heads: Vec<u32>,
    touched: Vec<u32>,
    /// (eater, next node) linked lists.
    nodes: Vec<(u32, u32)>,
    covers: Vec<[i64; 4]>,
}
impl std::fmt::Debug for EaterGrid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EaterGrid").finish_non_exhaustive()
    }
}
impl EaterGrid {
    /// Index `(center, radius)` circles. Returns false, leaving the grid unusable,
    /// when a circle is non-finite or too far out or the grid would exceed
    /// `max_cells` or the fixed limit.
    ///
    /// Exactness: a pellet `p` passes the pickup test only if
    /// `hypot(p.x - c.x, p.y - c.y) <= r`, which, for finite values under 2^20, puts
    /// `p.x` within `r + 4e-10` of `c.x` after rounding. The cover bound
    /// `c.x - r - 1e-9` rounds by at most 2.4e-10, so it never excludes such a pellet;
    /// floor division by a positive cell is monotonic. With finite circles a
    /// non-finite pellet fails the distance test or gets a NaN distance, which
    /// never wins. The hashed index uses the same bound, so its result agrees.
    pub(crate) fn build(
        &mut self,
        circles: impl ExactSizeIterator<Item = (Point, f64)>,
        max_cells: usize,
    ) -> bool {
        for &i in &self.touched {
            self.heads[i as usize] = NONE;
        }
        self.touched.clear();
        self.nodes.clear();
        self.covers.clear();
        let (mut lo_x, mut lo_y, mut hi_x, mut hi_y) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
        let mut area = 0usize;
        for (c, r) in circles {
            if !(r >= 0.0 && c.x.abs() + r <= REGULAR && c.y.abs() + r <= REGULAR) {
                return false;
            }
            let cover = [
                ((c.x - r - 1e-9) / CELL).floor() as i64,
                ((c.x + r + 1e-9) / CELL).floor() as i64,
                ((c.y - r - 1e-9) / CELL).floor() as i64,
                ((c.y + r + 1e-9) / CELL).floor() as i64,
            ];
            lo_x = lo_x.min(cover[0]);
            hi_x = hi_x.max(cover[1]);
            lo_y = lo_y.min(cover[2]);
            hi_y = hi_y.max(cover[3]);
            area = area
                .saturating_add(((cover[1] - cover[0] + 1) * (cover[3] - cover[2] + 1)) as usize);
            self.covers.push(cover);
        }
        if self.covers.is_empty() {
            (lo_x, lo_y, hi_x, hi_y) = (0, 0, -1, -1);
        }
        let width = (hi_x - lo_x + 1) as usize;
        let cells = width * (hi_y - lo_y + 1) as usize;
        if cells > MAX_CELLS.min(max_cells)
            || area > MAX_CELLS
            || self.covers.len() >= NONE as usize
        {
            return false;
        }
        if self.heads.len() < cells {
            self.heads.resize(cells, NONE);
        }
        (self.lo_x, self.lo_y) = (lo_x as f64, lo_y as f64);
        (self.end_x, self.end_y) = ((hi_x + 1) as f64, (hi_y + 1) as f64);
        self.lo = (lo_x, lo_y);
        self.width = width;
        // Prepending in descending eater order leaves each list ascending.
        for (e, cover) in self.covers.iter().enumerate().rev() {
            for y in cover[2]..=cover[3] {
                for x in cover[0]..=cover[1] {
                    let cell = (x - lo_x) as usize + (y - lo_y) as usize * width;
                    let head = &mut self.heads[cell];
                    if *head == NONE {
                        self.touched.push(cell as u32);
                    }
                    self.nodes.push((e as u32, *head));
                    *head = (self.nodes.len() - 1) as u32;
                }
            }
        }
        true
    }
    /// Eaters whose cover contains `p`'s cell, ascending.
    pub(crate) fn candidates(&self, p: Point) -> Option<Candidates<'_>> {
        // `floor` is a library call on baseline x86-64. For an integer bound,
        // `floor(v) >= lo` iff `v >= lo` and `floor(v) < end` iff `v < end`.
        let x = p.x / CELL;
        let y = p.y / CELL;
        // NaN pellets fall out here; the exact test never picks them either.
        if !(x >= self.lo_x && x < self.end_x && y >= self.lo_y && y < self.end_y) {
            return None;
        }
        let cell = (floor_small(x) - self.lo.0) as usize
            + (floor_small(y) - self.lo.1) as usize * self.width;
        let node = self.heads[cell];
        (node != NONE).then_some(Candidates {
            nodes: &self.nodes,
            node,
        })
    }
}

/// `v.floor() as i64` for `|v| < 2^52`.
#[inline]
fn floor_small(v: f64) -> i64 {
    let t = v as i64;
    t - i64::from(t as f64 > v)
}

pub(crate) struct Candidates<'a> {
    nodes: &'a [(u32, u32)],
    node: u32,
}
impl Iterator for Candidates<'_> {
    type Item = usize;
    fn next(&mut self) -> Option<usize> {
        if self.node == NONE {
            return None;
        }
        let (eater, next) = self.nodes[self.node as usize];
        self.node = next;
        Some(eater as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::Bounds;

    struct Rng(u64);
    impl Rng {
        fn unit(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn below(&mut self, n: usize) -> usize {
            (self.unit() * n as f64) as usize
        }
    }

    const SPECIAL: [f64; 8] = [
        0.0,
        -0.0,
        -1e-300,
        74.99999999999999,
        -75.0,
        1e300,
        f64::INFINITY,
        f64::NAN,
    ];

    fn pellet(rng: &mut Rng) -> Food {
        let mut coordinate = || {
            if rng.below(8) == 0 {
                SPECIAL[rng.below(SPECIAL.len())]
            } else {
                (rng.unit() - 0.5) * 600.0
            }
        };
        let (x, y) = (coordinate(), coordinate());
        let origin = [None, Some(Origin::Ambient), Some(Origin::Boost)][rng.below(3)];
        Food {
            x,
            y,
            value: 1.0,
            size: Some(1.0),
            origin,
        }
    }

    #[test]
    fn occupancy_cache_matches_recounting_through_external_edits() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut cache = FoodCache::default();
        let mut food = Vec::new();
        let sizes = [75.0, 40.0, 75.0, 128.0, 1e-300];
        for _ in 0..400 {
            // Edits between spans, which the cache must detect.
            match rng.below(6) {
                0 => food.push(pellet(&mut rng)),
                1 if !food.is_empty() => {
                    let i = rng.below(food.len());
                    food[i].x = pellet(&mut rng).x;
                }
                2 if !food.is_empty() => {
                    let i = rng.below(food.len());
                    food[i].origin = pellet(&mut rng).origin;
                }
                3 => food.truncate(food.len().saturating_sub(2)),
                _ => {}
            }
            cache.begin();
            for _ in 0..20 {
                match rng.below(4) {
                    0 => {
                        let f = pellet(&mut rng);
                        cache.push(&f);
                        food.push(f);
                    }
                    1 if !food.is_empty() => {
                        let removed: Vec<bool> = food.iter().map(|_| rng.below(5) == 0).collect();
                        cache.remove(&removed);
                        let mut i = 0;
                        food.retain(|_| {
                            i += 1;
                            !removed[i - 1]
                        });
                    }
                    2 => {
                        let expected = food
                            .iter()
                            .filter(|f| f.origin == Some(Origin::Ambient))
                            .count();
                        assert_eq!(cache.ambient(&food), expected);
                    }
                    _ => {
                        let size = sizes[rng.below(sizes.len())];
                        let max = rng.below(4);
                        let probe = match food.len() {
                            0 => pellet(&mut rng),
                            n => food[rng.below(n)].clone(),
                        };
                        let cell = food_cell(probe.x, probe.y, size);
                        let count = food
                            .iter()
                            .filter(|f| food_cell(f.x, f.y, size) == cell)
                            .count();
                        assert_eq!(cache.has_room(&food, &probe, size, max), count < max);
                    }
                }
            }
            cache.end();
        }
    }

    fn accepts(p: Point, center: Point, radius: f64) -> bool {
        Bounds::between(center, center).near(Bounds::between(p, p), radius + 1e-9)
            && p.distance(center) <= radius
    }

    #[test]
    fn eater_grid_lists_every_eater_that_can_accept_a_pellet() {
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        let mut grid = EaterGrid::default();
        for round in 0..300 {
            let spread = [300.0, 3000.0, 3000.0][round % 3];
            // Far-out clusters exercise rounding near the indexing bound.
            let offset = [0.0, 0.0, 1e6 - 2000.0][round % 3] * (rng.unit() - 0.5).signum();
            let circles: Vec<(Point, f64)> = (0..1 + rng.below(40))
                .map(|_| {
                    let mut c = Point {
                        x: offset + (rng.unit() - 0.5) * spread,
                        y: -offset + (rng.unit() - 0.5) * spread,
                    };
                    let r = 36.0 + rng.unit() * 100.0;
                    if rng.below(2) == 0 {
                        // Covers starting on a cell edge after rounding.
                        c.x = (c.x / CELL).round() * CELL + r;
                    }
                    (c, r)
                })
                .collect();
            assert!(grid.build(circles.iter().copied(), MAX_CELLS));
            for _ in 0..400 {
                let (c, r) = circles[rng.below(circles.len())];
                let angle = rng.unit() * std::f64::consts::TAU;
                let mut p = Point {
                    x: c.x + angle.cos() * r,
                    y: c.y + angle.sin() * r,
                };
                let ulps = |v: f64, rng: &mut Rng| {
                    f64::from_bits(
                        v.to_bits()
                            .wrapping_add(rng.below(7) as u64)
                            .wrapping_sub(3),
                    )
                };
                match rng.below(5) {
                    0 | 1 => {
                        p = Point {
                            x: ulps(c.x - r, &mut rng),
                            y: c.y,
                        }
                    }
                    2 => p.x = ulps(p.x, &mut rng),
                    3 => p.x = (p.x / CELL).round() * CELL - f64::from(rng.below(2) as u8) * 1e-12,
                    _ if rng.below(4) == 0 => p.y = SPECIAL[rng.below(SPECIAL.len())],
                    _ => {}
                }
                let listed: Vec<usize> = grid.candidates(p).into_iter().flatten().collect();
                assert!(listed.windows(2).all(|w| w[0] < w[1]));
                for (e, &(center, radius)) in circles.iter().enumerate() {
                    if accepts(p, center, radius) && !p.x.is_nan() && !p.y.is_nan() {
                        assert!(listed.contains(&e), "{p:?} {center:?} {radius}");
                    }
                }
            }
        }
    }

    #[test]
    fn eater_grid_declines_circles_it_cannot_bound_exactly() {
        let mut grid = EaterGrid::default();
        let c = Point { x: 0.0, y: 0.0 };
        for (center, radius) in [
            (
                Point {
                    x: f64::NAN,
                    y: 0.0,
                },
                36.0,
            ),
            (
                Point {
                    x: 0.0,
                    y: f64::INFINITY,
                },
                36.0,
            ),
            (c, f64::NAN),
            (c, -1.0),
            (Point { x: REGULAR, y: 0.0 }, 36.0),
        ] {
            assert!(!grid.build([(c, 36.0), (center, radius)].into_iter(), MAX_CELLS));
        }
        assert!(!grid.build(
            [(c, 36.0), (Point { x: 9e5, y: 9e5 }, 36.0)].into_iter(),
            MAX_CELLS
        ));
        // A caller limit below the grid size declines too.
        assert!(!grid.build(
            [(c, 36.0), (Point { x: 9e3, y: 9e3 }, 36.0)].into_iter(),
            4096
        ));
        assert!(grid.build([(c, 36.0)].into_iter(), MAX_CELLS));
        assert!(
            grid.candidates(Point {
                x: 0.0,
                y: f64::NAN
            })
            .is_none()
        );
        assert_eq!(
            grid.candidates(Point {
                x: -1e-300,
                y: 35.0
            })
            .unwrap()
            .collect::<Vec<_>>(),
            [0]
        );
    }
}
