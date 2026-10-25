//! Ordered food lifetimes, shared sector listings and per-connection diffs.
use super::SimulatorBackend;
use crate::{
    client::{encode, fast_hash::FastMap, presentation::FoodPresentation},
    model::{Food, Origin, Point},
    physics,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// The physical identity disambiguates food collapsed onto the same wire grid.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct KnownFood {
    pub(super) point: Point,
    pub(super) style: encode::FoodStyle,
    /// Decorations have no pickup lifetime, even at an eaten physical point.
    pub(super) decorative: bool,
}

/// One sector's food by wire offset, in ascending offset order.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct SectorFood(pub(super) Vec<([u8; 2], KnownFood)>);
impl SectorFood {
    fn get(&self, offset: &[u8; 2]) -> Option<&KnownFood> {
        let index = self.0.binary_search_by_key(offset, |(key, _)| *key).ok()?;
        Some(&self.0[index].1)
    }
    #[cfg(test)]
    pub(super) fn contains_key(&self, offset: &[u8; 2]) -> bool {
        self.get(offset).is_some()
    }
}
#[cfg(test)]
impl std::ops::Index<&[u8; 2]> for SectorFood {
    type Output = KnownFood;
    fn index(&self, offset: &[u8; 2]) -> &KnownFood {
        self.get(offset).expect("food at offset")
    }
}

/// Listings a connection holds, in ascending sector order.
#[derive(Clone, Debug, Default)]
pub(super) struct KnownSectors(Vec<([u8; 2], Arc<SectorFood>)>);
#[cfg(test)]
impl KnownSectors {
    pub(super) fn get(&self, sector: &[u8; 2]) -> Option<&Arc<SectorFood>> {
        let index = self.0.binary_search_by_key(sector, |(key, _)| *key).ok()?;
        Some(&self.0[index].1)
    }
    pub(super) fn contains_key(&self, sector: &[u8; 2]) -> bool {
        self.get(sector).is_some()
    }
}
#[cfg(test)]
impl std::ops::Index<&[u8; 2]> for KnownSectors {
    type Output = SectorFood;
    fn index(&self, sector: &[u8; 2]) -> &SectorFood {
        self.get(sector).expect("known sector")
    }
}

/// World food by wire sector, in world order, kept current from birth and
/// pickup events so a broadcast never scans every pellet.
///
/// Ordinary (ambient) births stay off the wire until the next shared food
/// maintenance: official ordinary-food `f` packets come in bursts ~504 ms apart
/// that align across clients, together with sector clears, while boost and
/// death food and pickups go out promptly (the [scheduler study], 2026-10-25).
/// Whether the official server creates ambient food at maintenance or only
/// announces it then is not identified; physics here is unchanged.
///
/// [scheduler study]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-packet-scheduler/REPORT.md
#[derive(Default)]
pub(super) struct FoodIndex {
    sectors: FastMap<u16, Vec<IndexedFood>>,
    pub(super) count: usize,
    /// A pickup did not match the index; rebuild before the next listing.
    pub(super) stale: bool,
    /// Sectors whose shown food changed since the last listing.
    dirty: BTreeSet<u16>,
    /// Sectors holding ambient births not yet shown.
    hidden: BTreeSet<u16>,
}

/// One pellet lifetime, with the color it was born with: a listing may show
/// a newer pellet while an older one at its position is still hidden.
#[derive(Clone)]
struct IndexedFood {
    offset: [u8; 2],
    food: Food,
    shown: bool,
    color: u8,
}
impl FoodIndex {
    pub(super) fn rebuild(&mut self, food: &[Food], presentation: &FoodPresentation) {
        self.dirty.extend(self.sectors.keys());
        self.sectors.clear();
        self.hidden.clear();
        self.count = 0;
        self.stale = false;
        let mut lifetimes = FastMap::default();
        for item in food {
            let lifetime = lifetimes.entry(food_identity(item.point())).or_insert(0);
            self.add(item, true, presentation.color(item, *lifetime));
            *lifetime += 1;
        }
    }

    pub(super) fn sector(food: &Food) -> Option<(u16, [u8; 2])> {
        let key = encode::food_key(food.point()).ok()?;
        Some((u16::from_be_bytes([key[0], key[1]]), [key[2], key[3]]))
    }

    pub(super) fn add(&mut self, food: &Food, shown: bool, color: u8) {
        self.count += 1;
        let Some((sector, offset)) = Self::sector(food) else {
            // Off the wire grid; listings never include it.
            return;
        };
        self.sectors.entry(sector).or_default().push(IndexedFood {
            offset,
            food: food.clone(),
            shown,
            color,
        });
        if shown {
            self.dirty.insert(sector);
        } else {
            self.hidden.insert(sector);
        }
    }

    /// Physics visits pickups in reverse food order, so of several pellets at
    /// one position the newest goes first.
    pub(super) fn remove(&mut self, food: &Food) {
        self.count = self.count.wrapping_sub(1);
        let Some((sector, _)) = Self::sector(food) else {
            return;
        };
        let identity = food_identity(food.point());
        let Some(entries) = self.sectors.get_mut(&sector) else {
            self.stale = true;
            return;
        };
        match entries
            .iter()
            .rposition(|entry| food_identity(entry.food.point()) == identity)
        {
            Some(index) => {
                let removed = entries.remove(index);
                if entries.is_empty() {
                    self.sectors.remove(&sector);
                }
                if removed.shown {
                    self.dirty.insert(sector);
                }
            }
            None => self.stale = true,
        }
    }

    /// Show every ambient birth held since the last maintenance.
    pub(super) fn reveal(&mut self) {
        for sector in std::mem::take(&mut self.hidden) {
            if let Some(entries) = self.sectors.get_mut(&sector) {
                for entry in entries {
                    entry.shown = true;
                }
            }
            self.dirty.insert(sector);
        }
    }

    /// The first shown pellet of each wire cell, in offset order.
    pub(super) fn listing(&self, sector: u16) -> Vec<([u8; 2], KnownFood)> {
        let mut run: Vec<_> = self
            .sectors
            .get(&sector)
            .into_iter()
            .flatten()
            .filter(|entry| entry.shown)
            .map(|entry| {
                let food = &entry.food;
                let size = food
                    .size
                    .unwrap_or_else(|| (food.value / physics::nutrition(1., 2.)).max(0.).sqrt());
                (
                    entry.offset,
                    KnownFood {
                        point: food.point(),
                        decorative: false,
                        style: encode::FoodStyle {
                            color: entry.color,
                            size: encode::food_size(size),
                            snake_food: matches!(food.origin, Some(Origin::Boost | Origin::Death)),
                        },
                    },
                )
            })
            .collect();
        // Stable, so deduplication keeps the earliest pellet of each wire cell.
        run.sort_by_key(|(offset, _)| *offset);
        run.dedup_by_key(|(offset, _)| *offset);
        run
    }
}

impl SimulatorBackend {
    /// List the food of every sector a connection subscribes to. Each wire cell
    /// shows its first shown pellet in world order. Only new and changed
    /// listings are rebuilt, from the index.
    pub(super) fn sync_food_sectors(&mut self) -> Result<(), String> {
        // Taken so an early error drops a partly filled table.
        let mut buckets = std::mem::take(&mut self.food_buckets);
        buckets.resize(1 << 16, u32::MAX);
        let mut sectors = Vec::new();
        for slot in self.slots.iter().filter(|slot| slot.connection.setup) {
            for &sector in slot.connection.food_interest.subscribed() {
                let index = u16::from_be_bytes(sector);
                if buckets[usize::from(index)] == u32::MAX {
                    buckets[usize::from(index)] = sectors.len() as u32;
                    sectors.push(index);
                }
            }
        }
        let table = &mut self.food_sectors;
        table.resize(1 << 16, None);
        if self.pit_markers.is_some() {
            self.decorated_food_sectors.resize(1 << 16, None);
        }
        for &index in &self.listed {
            if buckets[usize::from(index)] == u32::MAX {
                table[usize::from(index)] = None;
                if self.pit_markers.is_some() {
                    self.decorated_food_sectors[usize::from(index)] = None;
                }
            }
        }
        for &index in &sectors {
            let listing = &mut table[usize::from(index)];
            if listing.is_none() || self.food_index.dirty.contains(&index) {
                let run = self.food_index.listing(index);
                if listing.as_ref().is_none_or(|old| old.0 != run) {
                    *listing = Some(Arc::new(SectorFood(run)));
                    if let Some(markers) = &self.pit_markers {
                        let physical = listing.as_ref().unwrap();
                        self.decorated_food_sectors[usize::from(index)] = Some(
                            markers
                                .overlay(index, &physical.0)
                                .map_or_else(|| physical.clone(), |run| Arc::new(SectorFood(run))),
                        );
                    }
                }
            }
        }
        // Unlisted sectors are built afresh when a connection subscribes.
        self.food_index.dirty.clear();
        for &index in &sectors {
            buckets[usize::from(index)] = u32::MAX;
        }
        self.food_buckets = buckets;
        self.listed = sectors;
        Ok(())
    }
}

/// Keep empty subscriptions internally. Retained receipts introduce food with
/// nonempty `F`, then births/pickups; see docs/server-rules.md#food-sector-delivery.
/// `sectors` lists every subscribed sector.
pub(super) fn food_updates(
    packets: &mut Vec<Vec<u8>>,
    known: &mut KnownSectors,
    subscribed: &BTreeSet<[u8; 2]>,
    sectors: &[Option<Arc<SectorFood>>],
    eaten: &BTreeSet<[u8; 2]>,
    eaters: &BTreeMap<(u64, u64), u16>,
) {
    let held = std::mem::take(&mut known.0);
    let mut subscriptions = subscribed.iter().peekable();
    for (sector, _) in &held {
        while subscriptions.next_if(|next| *next < sector).is_some() {}
        if subscriptions.peek() != Some(&sector) {
            packets.push(encode::sector_clear(*sector));
        }
    }
    let mut held = held.into_iter().peekable();
    known.0.reserve(subscribed.len());
    for &sector in subscribed {
        while held.next_if(|(old, _)| *old < sector).is_some() {}
        let previous = held
            .next_if(|(old, _)| *old == sector)
            .map(|(_, food)| food);
        let current = sectors[usize::from(u16::from_be_bytes(sector))]
            .as_ref()
            .expect("subscribed sectors are listed");
        let key = |offset: [u8; 2]| [sector[0], sector[1], offset[0], offset[1]];
        match previous {
            None => {
                let foods: Vec<_> = current.0.iter().map(|(k, v)| (*k, v.style)).collect();
                if !foods.is_empty() {
                    packets.extend(encode::food_sector(sector, &foods));
                }
            }
            // The same listing without a pickup at its points sends nothing.
            Some(previous) if Arc::ptr_eq(&previous, current) && !eaten.contains(&sector) => {}
            Some(previous) => {
                for (offset, food) in &previous.0 {
                    let eater = (!food.decorative)
                        .then(|| eaters.get(&food_identity(food.point)).copied())
                        .flatten();
                    if current.get(offset) != Some(food) || eater.is_some() {
                        packets.push(encode::food_remove(key(*offset), eater));
                    }
                }
                for (offset, food) in &current.0 {
                    if previous.get(offset) != Some(food)
                        || previous.get(offset).is_some_and(|old| {
                            !old.decorative && eaters.contains_key(&food_identity(old.point))
                        })
                    {
                        packets.push(encode::food_add(key(*offset), food.style));
                    }
                }
            }
        }
        known.0.push((sector, current.clone()));
    }
}

/// Sectors that may hold a pellet at an eaten position.
pub(super) fn eaten_sectors(eaters: &BTreeMap<(u64, u64), u16>) -> BTreeSet<[u8; 2]> {
    eaters
        .keys()
        .filter_map(|&(x, y)| {
            let point = Point {
                x: f64::from_bits(x),
                y: f64::from_bits(y),
            };
            encode::food_key(point).ok().map(|key| [key[0], key[1]])
        })
        .collect()
}

pub(super) fn food_identity(point: Point) -> (u64, u64) {
    let bits = |v: f64| if v == 0. { 0 } else { v.to_bits() };
    (bits(point.x), bits(point.y))
}
