//! Caches for the step's food paths. The cell occupancy cache cannot change a
//! result: it is compared with `Simulator::food` bit for bit before it is trusted.
use crate::model::{Food, Origin};
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
