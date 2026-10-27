//! Caches for the step's food paths. Untrusted food lists are checked bit for
//! bit; owned mirrored spans preserve that verification. The eater grid omits
//! only pickup candidates that cannot qualify.
use crate::model::{Food, Origin, Point};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// One axis of an admission cell key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Axis {
    /// `floor(coordinate / cellSize)`.
    Index(i64),
    /// The coordinate's bits. From 2^53 cells on, a cell is no wider than the
    /// spacing of doubles at that coordinate, so distinct coordinates lie in
    /// distinct cells; a saturating `i64` cast would instead merge them, as a
    /// tiny `cellSize` did for every pellet of a quadrant.
    Exact(u64),
}
fn axis(value: f64, size: f64) -> Axis {
    const EXACT_CELLS: f64 = (1u64 << f64::MANTISSA_DIGITS) as f64;
    let cell = (value / size).floor();
    if cell.abs() < EXACT_CELLS {
        Axis::Index(cell as i64)
    } else {
        Axis::Exact(value.to_bits())
    }
}
pub(crate) fn food_cell(x: f64, y: f64, size: f64) -> (Axis, Axis) {
    (axis(x, size), axis(y, size))
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
    fn write_u32(&mut self, n: u32) {
        self.write_u64(n.into());
    }
    /// Derived `Hash` writes the `Axis` discriminant through this.
    fn write_isize(&mut self, n: isize) {
        self.write_u64(n as u64);
    }
    fn finish(&self) -> u64 {
        // The final multiply leaves the low bits that pick a bucket weakest.
        self.0.rotate_left(26)
    }
}

type Counts = HashMap<(Axis, Axis), usize, BuildHasherDefault<CellHasher>>;

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
    fn cell(self, size: f64) -> (Axis, Axis) {
        food_cell(f64::from_bits(self.x), f64::from_bits(self.y), size)
    }
}

/// Per-cell pellet counts and the ambient count, derived from a mirror of the
/// food list. Every count is always a function of `entries`; the mirror is
/// first verified against the current food. Untrusted spans compare again;
/// the simulator can retain verification because public edits reset this cache,
/// internal pushes/removals are mirrored, and retention changes the list length.
#[derive(Clone, Default)]
pub(crate) struct FoodCache {
    entries: Vec<Entry>,
    ambient: usize,
    /// At most three cell sizes: boost, death and ambient admission.
    grids: Vec<(u64, Counts)>,
    /// Lazy, derived from the same bit-verified entries as the admission counts.
    pellets: Option<PelletIndex>,
    /// Inside a step nothing else can edit food, so one comparison per step
    /// suffices while every internal edit is mirrored.
    open: bool,
    verified: bool,
    /// A prior comparison still covers this owner's list after mirrored edits.
    matched: bool,
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
        self.matched = false;
    }
    /// Start an owned span: every non-mirrored edit must reset the cache or
    /// change the food length. Simulator food APIs reset it; radius/pit retention
    /// only removes items. Thus unchanged length preserves the prior comparison.
    /// Debug builds check that invariant before reusing verification.
    pub(crate) fn begin_mirrored(&mut self, food: &[Food]) {
        self.open = true;
        self.verified = self.matched && self.entries.len() == food.len();
        #[cfg(debug_assertions)]
        if self.verified {
            assert!(
                self.entries.iter().copied().eq(food.iter().map(Entry::of)),
                "reused food verification does not match the food list"
            );
        }
        self.matched = self.verified;
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
            self.pellets = None;
        }
        self.verified = self.open;
        self.matched = true;
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
                    *counts.entry(e.cell(size)).or_default() += 1;
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
            *counts.entry(entry.cell(f64::from_bits(*key))).or_default() += 1;
        }
        self.entries.push(entry);
        if self.entries.len() >= NONE as usize {
            self.pellets = None;
        }
        if let Some(index) = &mut self.pellets {
            index.push(entry, self.entries.len() - 1);
        }
    }
    /// Mirror removing original food indices in strictly ascending order.
    pub(crate) fn remove(&mut self, food_len: usize, removed: impl Iterator<Item = usize>) {
        if food_len != self.entries.len() {
            // Already stale; the next comparison rebuilds.
            self.entries.clear();
            self.ambient = 0;
            self.grids.clear();
            self.pellets = None;
            self.verified = false;
            self.matched = false;
            return;
        }
        let (mut read, mut write) = (0, 0);
        let mut first_removed = food_len;
        for i in removed {
            first_removed = first_removed.min(i);
            let e = self.entries[i];
            if let Some(index) = &mut self.pellets {
                index.remove(i);
                index.food_slots.copy_within(read..i, write);
            }
            self.ambient -= usize::from(e.ambient);
            for (key, counts) in &mut self.grids {
                let cell = e.cell(f64::from_bits(*key));
                if let Some(count) = counts.get_mut(&cell) {
                    *count -= 1;
                    if *count == 0 {
                        counts.remove(&cell);
                    }
                }
            }
            // Pickup already supplies ordered indices. Copy survivor ranges rather
            // than testing every cached pellet twice; retain their original order.
            self.entries.copy_within(read..i, write);
            write += i - read;
            read = i + 1;
        }
        self.entries.copy_within(read.., write);
        self.entries.truncate(write + self.entries.len() - read);
        if let Some(index) = &mut self.pellets {
            index.food_slots.copy_within(read.., write);
            index.food_slots.truncate(self.entries.len());
            // Stable compaction leaves the prefix before the first removal in
            // place. Only the remaining handles can have a new food position;
            // no removals means the entire prefix stays unchanged.
            for (i, &slot) in index.food_slots.iter().enumerate().skip(first_removed) {
                index.slots[slot as usize].food = i as u32;
            }
        }
    }

    /// Lazily build the pellet index; cold restores and external edits rebuild
    /// from the verified mirror. Larger arrays retain the original scan paths.
    pub(crate) fn pellets(&mut self, food: &[Food]) -> Option<&PelletIndex> {
        self.sync(food);
        if food.len() >= NONE as usize {
            return None;
        }
        if self.pellets.is_none() {
            let mut index = PelletIndex::default();
            for (i, &entry) in self.entries.iter().enumerate() {
                index.push(entry, i);
            }
            self.pellets = Some(index);
        }
        self.pellets.as_ref()
    }
}

const CELL_BIAS: i64 = 16384;

/// Eater covers stay within +/-8193 cells (REGULAR/CELL plus rounding margin),
/// so biased coordinates fit disjoint 16-bit fields and can never encode zero.
fn pack_cell(x: i64, y: i64) -> u32 {
    ((x + CELL_BIAS) as u32) | (((y + CELL_BIAS) as u32) << 16)
}

fn pellet_cell(entry: Entry) -> u32 {
    let x = f64::from_bits(entry.x) / CELL;
    let y = f64::from_bits(entry.y) / CELL;
    if !(x.abs() <= REGULAR / CELL + 1.0 && y.abs() <= REGULAR / CELL + 1.0) {
        // Such a pellet cannot win any regular eater's exact test, including
        // NaN/inf (see EaterGrid::build). Zero is outside all regular covers.
        return 0;
    }
    pack_cell(floor_small(x), floor_small(y))
}

/// Cell lists contain stable handles, not food-array indices. A handle stores
/// its current array index and its position in its cell for constant-time
/// deletion; freed handles are reused. Neither list order nor hashing selects
/// a winner: pickup tests use the ordered eater list and hits are sorted later.
#[derive(Clone, Default)]
pub(crate) struct PelletIndex {
    cells: HashMap<u32, Vec<u32>, BuildHasherDefault<CellHasher>>,
    slots: Vec<PelletSlot>,
    food_slots: Vec<u32>,
    free: Vec<u32>,
}

#[derive(Clone)]
struct PelletSlot {
    food: u32,
    cell: u32,
    offset: usize,
}

impl PelletIndex {
    fn push(&mut self, entry: Entry, food: usize) {
        let cell = pellet_cell(entry);
        let slot = self.free.pop().unwrap_or(self.slots.len() as u32);
        let list = self.cells.entry(cell).or_default();
        let value = PelletSlot {
            food: food as u32,
            cell,
            offset: list.len(),
        };
        if slot as usize == self.slots.len() {
            self.slots.push(value);
        } else {
            self.slots[slot as usize] = value;
        }
        list.push(slot);
        self.food_slots.push(slot);
    }
    fn remove(&mut self, food: usize) {
        let slot = self.food_slots[food];
        let value = &self.slots[slot as usize];
        let list = self.cells.get_mut(&value.cell).unwrap();
        let offset = value.offset;
        list.swap_remove(offset);
        if let Some(&moved) = list.get(offset) {
            self.slots[moved as usize].offset = offset;
        }
        if list.is_empty() {
            self.cells.remove(&self.slots[slot as usize].cell);
        }
        self.free.push(slot);
    }
    pub(crate) fn food(&self, cell: u32) -> impl Iterator<Item = usize> + '_ {
        self.cells
            .get(&cell)
            .into_iter()
            .flatten()
            .map(|&slot| self.slots[slot as usize].food as usize)
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
    /// Each occupied cell once, with its original ascending eater list.
    pub(crate) fn occupied(&self) -> impl Iterator<Item = (u32, Candidates<'_>)> {
        self.touched.iter().map(|&cell| {
            let x = self.lo.0 + (cell as usize % self.width) as i64;
            let y = self.lo.1 + (cell as usize / self.width) as i64;
            (
                pack_cell(x, y),
                Candidates {
                    nodes: &self.nodes,
                    node: self.heads[cell as usize],
                },
            )
        })
    }
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

#[derive(Clone)]
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
    fn tiny_cells_keep_distinct_positions_apart() {
        let mut cache = FoodCache::default();
        let mut food = Vec::new();
        let size = 1e-300;
        for i in 0..100 {
            let angle = f64::from(i) * 0.7;
            let f = Food {
                x: 500.0 * angle.cos(),
                y: 500.0 * angle.sin(),
                value: 1.0,
                size: Some(1.0),
                origin: Some(Origin::Ambient),
            };
            // A saturating cast once put every pellet of a quadrant in one cell.
            assert!(cache.has_room(&food, &f, size, 2), "pellet {i}");
            cache.push(&f);
            food.push(f);
        }
        let same = food[0].clone();
        assert!(cache.has_room(&food, &same, size, 2));
        cache.push(&same);
        food.push(same.clone());
        assert!(!cache.has_room(&food, &same, size, 2));
    }

    #[test]
    fn occupancy_and_pellet_indexes_match_recounting_through_external_edits() {
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
            cache.pellets(&food).unwrap();
            for _ in 0..20 {
                match rng.below(4) {
                    0 => {
                        let f = pellet(&mut rng);
                        cache.push(&f);
                        food.push(f);
                    }
                    1 if !food.is_empty() => {
                        let removed: Vec<bool> = food.iter().map(|_| rng.below(5) == 0).collect();
                        cache.remove(
                            food.len(),
                            removed
                                .iter()
                                .enumerate()
                                .filter_map(|(i, &r)| r.then_some(i)),
                        );
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
                assert_pellet_index(&cache, &food);
            }
            cache.end();
            assert_pellet_index(&cache.clone(), &food);
        }
    }

    fn assert_pellet_index(cache: &FoodCache, food: &[Food]) {
        assert!(cache.entries == food.iter().map(Entry::of).collect::<Vec<_>>());
        let index = cache.pellets.as_ref().unwrap();
        assert_eq!(index.food_slots.len(), food.len());
        assert_eq!(index.slots.len(), food.len() + index.free.len());
        let mut seen = Vec::new();
        let mut live = std::collections::HashSet::new();
        for (&cell, slots) in &index.cells {
            assert!(!slots.is_empty());
            for (offset, &slot) in slots.iter().enumerate() {
                assert!(live.insert(slot));
                let value = &index.slots[slot as usize];
                assert_eq!(value.offset, offset);
                assert_eq!(value.cell, cell);
                assert_eq!(index.food_slots[value.food as usize], slot);
                assert_eq!(pellet_cell(Entry::of(&food[value.food as usize])), cell);
                seen.push(value.food as usize);
            }
        }
        seen.sort_unstable();
        assert_eq!(seen, (0..food.len()).collect::<Vec<_>>());
        let mut free = std::collections::HashSet::new();
        for &slot in &index.free {
            assert!(free.insert(slot));
            assert!(!live.contains(&slot));
        }
    }

    #[test]
    fn removal_boundaries_keep_positions_correct_through_handle_reuse() {
        let pellet = |i| Food {
            x: i as f64,
            y: 0.0,
            value: i as f64 + 1.0,
            size: None,
            origin: Some(Origin::Ambient),
        };
        let mut food: Vec<_> = (0..40).map(pellet).collect();
        let mut cache = FoodCache::default();
        cache.begin();
        cache.pellets(&food).unwrap();
        fn remove(cache: &mut FoodCache, food: &mut Vec<Food>, removed: &[usize]) {
            let first = removed.first().copied().unwrap_or(food.len());
            let prefix = cache.pellets.as_ref().unwrap().food_slots[..first].to_vec();
            cache.remove(food.len(), removed.iter().copied());
            let mut i = 0;
            food.retain(|_| {
                let keep = removed.binary_search(&i).is_err();
                i += 1;
                keep
            });
            assert_eq!(cache.pellets.as_ref().unwrap().food_slots[..first], prefix);
            assert_pellet_index(cache, food);
            assert_pellet_index(&cache.clone(), food);
        }
        // Shared cells also exercise repair of a swapped cell-list entry.
        remove(&mut cache, &mut food, &[29, 32, 39]);
        assert_eq!(cache.pellets.as_ref().unwrap().free.len(), 3);
        for i in 40..50 {
            let f = pellet(i);
            cache.push(&f);
            food.push(f);
        }
        assert_pellet_index(&cache, &food);
        remove(&mut cache, &mut food, &[0, 10, 22, 46]);
        let last = food.len() - 1;
        remove(&mut cache, &mut food, &[last]);
        remove(&mut cache, &mut food, &[]);
        let all: Vec<_> = (0..food.len()).collect();
        remove(&mut cache, &mut food, &all);
        for i in 0..40 {
            let f = pellet(i);
            cache.push(&f);
            food.push(f);
        }
        let middle: Vec<_> = (15..35).collect();
        remove(&mut cache, &mut food, &middle);
        cache.end();
    }

    #[test]
    fn stale_removal_drops_the_pellet_index_before_rebuilding() {
        let mut rng = Rng(47);
        let mut food: Vec<_> = (0..20).map(|_| pellet(&mut rng)).collect();
        let mut cache = FoodCache::default();
        cache.begin();
        cache.pellets(&food).unwrap();
        food.pop();
        cache.remove(food.len(), std::iter::empty());
        assert!(cache.pellets.is_none());
        cache.pellets(&food).unwrap();
        assert_pellet_index(&cache, &food);
    }

    #[test]
    fn owned_spans_reuse_verification_and_detect_unmirrored_retention() {
        let mut rng = Rng(29);
        let mut food: Vec<_> = (0..64).map(|_| pellet(&mut rng)).collect();
        let mut cache = FoodCache::default();
        cache.begin_mirrored(&food);
        assert!(!cache.verified);
        cache.pellets(&food).unwrap();
        for size in [40.0, 75.0, 128.0] {
            cache.has_room(&food, &food[0], size, usize::MAX);
        }
        for _ in 0..8 {
            cache.end();
            cache.begin_mirrored(&food);
            assert!(cache.verified);
            let f = pellet(&mut rng);
            cache.push(&f);
            food.push(f);
            let removed = [0, food.len() / 2, food.len() - 1];
            cache.remove(food.len(), removed.into_iter());
            let mut i = 0;
            food.retain(|_| {
                let keep = !removed.contains(&i);
                i += 1;
                keep
            });
            assert!(cache.verified && cache.matched);
            assert_pellet_index(&cache, &food);
            assert_eq!(
                cache.ambient(&food),
                food.iter()
                    .filter(|f| f.origin == Some(Origin::Ambient))
                    .count()
            );
            cache.end();
            // Stable retention outside the span changes every later food index.
            food.remove(0);
            cache.begin_mirrored(&food);
            assert!(!cache.verified && !cache.matched);
            cache.pellets(&food).unwrap();
            assert_pellet_index(&cache, &food);
        }
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "reused food verification does not match the food list")]
    fn owned_span_rejects_unmirrored_same_length_edits() {
        let mut food = vec![Food {
            x: 0.0,
            y: 0.0,
            value: 1.0,
            size: Some(1.0),
            origin: Some(Origin::Ambient),
        }];
        let mut cache = FoodCache::default();
        cache.begin_mirrored(&food);
        cache.pellets(&food).unwrap();
        cache.end();
        food[0].x += 1.0;
        cache.begin_mirrored(&food);
    }

    #[test]
    fn untrusted_spans_and_outside_reads_still_check_same_length_edits() {
        let mut rng = Rng(47);
        let mut food: Vec<_> = (0..16).map(|_| pellet(&mut rng)).collect();
        let mut cache = FoodCache::default();
        cache.begin_mirrored(&food);
        cache.pellets(&food).unwrap();
        cache.end();
        food[0].x = 1234.0;
        cache.begin();
        assert!(!cache.verified && !cache.matched);
        cache.pellets(&food).unwrap();
        assert_pellet_index(&cache, &food);
        cache.end();
        food[1].y = -4321.0;
        cache.pellets(&food).unwrap();
        assert_pellet_index(&cache, &food);
    }

    #[test]
    fn ordered_removal_preserves_survivors_and_all_occupancy_counts() {
        for removed in [
            vec![],
            vec![0],
            vec![7],
            vec![0, 1, 3, 4, 7],
            (0..8).collect(),
        ] {
            let food: Vec<_> = (0..8)
                .map(|i| Food {
                    x: (i / 2) as f64 * 75.0,
                    y: 0.0,
                    value: i as f64,
                    size: Some(1.0),
                    origin: Some(if i % 2 == 0 {
                        Origin::Ambient
                    } else {
                        Origin::Boost
                    }),
                })
                .collect();
            let mut cache = FoodCache::default();
            for size in [40.0, 75.0, 128.0] {
                cache.has_room(&food, &food[0], size, usize::MAX);
            }
            cache.remove(food.len(), removed.iter().copied());
            let survivors: Vec<_> = food
                .iter()
                .enumerate()
                .filter(|(i, _)| !removed.contains(i))
                .map(|(_, f)| Entry::of(f))
                .collect();
            assert!(cache.entries == survivors, "removed {removed:?}");
            assert_eq!(
                cache.ambient,
                survivors.iter().filter(|e| e.ambient).count()
            );
            for (key, actual) in &cache.grids {
                let mut expected = Counts::default();
                for &e in &survivors {
                    *expected.entry(e.cell(f64::from_bits(*key))).or_default() += 1;
                }
                assert_eq!(*actual, expected, "removed {removed:?}");
            }
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
            let occupied: HashMap<_, Vec<_>> = grid
                .occupied()
                .map(|(cell, candidates)| (cell, candidates.collect()))
                .collect();
            assert_eq!(occupied.len(), grid.touched.len());
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
                let cell = pellet_cell(Entry {
                    x: p.x.to_bits(),
                    y: p.y.to_bits(),
                    ambient: false,
                });
                assert_eq!(occupied.get(&cell).cloned().unwrap_or_default(), listed);
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
    fn packed_pellet_cells_preserve_regular_grid_boundary_candidates() {
        let mut grid = EaterGrid::default();
        for center in [
            Point::default(),
            Point {
                x: REGULAR,
                y: REGULAR,
            },
            Point {
                x: -REGULAR,
                y: -REGULAR,
            },
            Point {
                x: REGULAR - 36.0,
                y: -REGULAR + 36.0,
            },
        ] {
            let radius = if center.x == REGULAR - 36.0 {
                36.0
            } else {
                0.0
            };
            assert!(grid.build([(center, radius)].into_iter(), MAX_CELLS));
            let occupied: HashMap<_, Vec<_>> = grid
                .occupied()
                .map(|(cell, c)| (cell, c.collect()))
                .collect();
            let coordinates = [
                center.x,
                center.x.next_up(),
                center.x.next_down(),
                center.x - radius,
                center.x + radius,
                -128.0,
                -128.0f64.next_up(),
                -128.0f64.next_down(),
                -0.0,
                -f64::from_bits(1),
                f64::NAN,
                f64::INFINITY,
                -1e300,
                1e300,
            ];
            for x in coordinates {
                for y in [
                    center.y,
                    center.y.next_up(),
                    center.y.next_down(),
                    center.y - radius,
                    center.y + radius,
                ] {
                    let p = Point { x, y };
                    let cell = pellet_cell(Entry {
                        x: x.to_bits(),
                        y: y.to_bits(),
                        ambient: false,
                    });
                    let expected: Vec<_> = grid.candidates(p).into_iter().flatten().collect();
                    assert_eq!(
                        occupied.get(&cell).cloned().unwrap_or_default(),
                        expected,
                        "{p:?} {center:?}"
                    );
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
