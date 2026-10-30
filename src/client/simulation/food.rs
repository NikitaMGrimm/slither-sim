//! Ordered food lifetimes, shared sector listings and per-connection diffs.
//! Sparse delivery: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-host-food-sharing/REPORT.md
use super::SimulatorBackend;
use crate::{
    client::{
        encode, fast_hash::FastMap, presentation::FoodPresentation, visibility::SectorInterest,
    },
    model::{Food, Origin, Point},
    physics,
};
use std::{
    cell::RefCell,
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

/// The union of live connections' food subscriptions. Membership identities
/// avoid scanning every unchanged recipient's sectors on every broadcast.
#[derive(Default)]
pub(super) struct FoodSubscriptions {
    held: Vec<HeldSubscription>,
    counts: Vec<u32>,
    sectors: BTreeSet<u16>,
    /// Zero-to-one subscription transitions in the latest sync.
    entered: BTreeSet<u16>,
}

#[derive(Default)]
struct HeldSubscription {
    revision: Option<Arc<()>>,
    sectors: Vec<u16>,
}

impl FoodSubscriptions {
    fn sync<'a>(
        &mut self,
        interests: impl ExactSizeIterator<Item = Option<&'a SectorInterest>>,
    ) -> BTreeSet<u16> {
        self.entered.clear();
        let len = interests.len();
        self.held
            .resize_with(self.held.len().max(len), HeldSubscription::default);
        self.counts.resize(1 << 16, 0);
        let mut retired = BTreeSet::new();
        for (held, interest) in self
            .held
            .iter_mut()
            .zip(interests.chain(std::iter::repeat(None)))
        {
            let revision = interest.map(SectorInterest::revision);
            let unchanged = match (&held.revision, revision) {
                (Some(old), Some(new)) => Arc::ptr_eq(old, new),
                (None, None) => true,
                _ => false,
            };
            if unchanged {
                continue;
            }
            for sector in held.sectors.drain(..) {
                self.counts[usize::from(sector)] -= 1;
                if self.counts[usize::from(sector)] == 0 {
                    self.sectors.remove(&sector);
                    retired.insert(sector);
                }
            }
            // Retain the identity strongly: a closed/replaced connection cannot
            // have its old address reused while this snapshot still names it.
            held.revision = revision.cloned();
            if let Some(interest) = interest {
                for &sector in interest.subscribed() {
                    let sector = u16::from_be_bytes(sector);
                    let count = &mut self.counts[usize::from(sector)];
                    if *count == 0 {
                        self.sectors.insert(sector);
                        self.entered.insert(sector);
                    }
                    *count += 1;
                    held.sectors.push(sector);
                }
            }
        }
        self.held.truncate(len);
        // A sector may change owners in this same broadcast. Keep its listing
        // unless the final union really lost it, regardless of update order.
        retired.retain(|&sector| self.counts[usize::from(sector)] == 0);
        retired
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
pub(super) struct KnownSectors {
    entries: Vec<([u8; 2], Arc<SectorFood>)>,
    subscription_revision: Option<Arc<()>>,
    food_revision: Option<Arc<()>>,
}
#[cfg(feature = "perf-internals")]
impl KnownSectors {
    pub(super) fn perf_len(&self) -> usize {
        self.entries.iter().map(|(_, sector)| sector.0.len()).sum()
    }
}
#[cfg(test)]
impl KnownSectors {
    pub(super) fn get(&self, sector: &[u8; 2]) -> Option<&Arc<SectorFood>> {
        let index = self
            .entries
            .binary_search_by_key(sector, |(key, _)| *key)
            .ok()?;
        Some(&self.entries[index].1)
    }
    pub(super) fn contains_key(&self, sector: &[u8; 2]) -> bool {
        self.get(sector).is_some()
    }
}

/// Shared for one broadcast; a pickup can require a diff even when the listing
/// itself is unchanged (a replacement may occupy the same wire cell).
#[derive(Default)]
pub(super) struct FoodChanges {
    /// Changed listing handles plus every sector touched by a pickup, sorted
    /// and unique so each recipient can skip hints outside its held range.
    pub(super) changed: Vec<[u8; 2]>,
    pub(super) eaten: BTreeSet<[u8; 2]>,
    // Sparse hints cover just this transition. A recipient skipped by an early
    // broadcast error must fall back to the full diff against its older view.
    pub(super) previous: Arc<()>,
    pub(super) current: Arc<()>,
    pub(super) diffs: RefCell<FastMap<(u16, usize), SectorDiff>>,
}

/// One transition per current listing, shared only within this broadcast, when
/// the pickup set is fixed. Keep both identities alive so addresses cannot be
/// reused. Physical and decorated listings can differ in the same sector.
/// https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-food-diffs/REPORT.md
pub(super) struct SectorDiff {
    previous: Arc<SectorFood>,
    current: Arc<SectorFood>,
    packets: Vec<Vec<u8>>,
}

impl FoodChanges {
    fn append_diff(
        &self,
        packets: &mut Vec<Vec<u8>>,
        sector: [u8; 2],
        previous: &Arc<SectorFood>,
        current: &Arc<SectorFood>,
        eaters: &BTreeMap<(u64, u64), u16>,
    ) {
        let mut diffs = self.diffs.borrow_mut();
        let diff = diffs
            .entry((u16::from_be_bytes(sector), Arc::as_ptr(current) as usize))
            .or_insert_with(|| {
                let mut packets = Vec::new();
                sector_changes(&mut packets, sector, previous, current, eaters);
                SectorDiff {
                    previous: previous.clone(),
                    current: current.clone(),
                    packets,
                }
            });
        if Arc::ptr_eq(previous, &diff.previous) && Arc::ptr_eq(current, &diff.current) {
            packets.extend_from_slice(&diff.packets);
        } else {
            // A recipient skipped an earlier broadcast and holds an older view.
            sector_changes(packets, sector, previous, current, eaters);
        }
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
        // A pickup before the first rebuild can temporarily wrap the count;
        // match remove() until the authoritative listing resynchronizes it.
        self.count = self.count.wrapping_add(1);
        let Some((sector, offset)) = Self::sector(food) else {
            // Off the wire grid; listings never include it.
            return;
        };
        self.sectors.entry(sector).or_default().push(IndexedFood {
            offset,
            food: *food,
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

    /// Remove pellets outside a shrunk dynamic wall with the same test physics
    /// uses, keeping survivor order, colors and visibility. Only sectors that
    /// lose a shown pellet need a new listing. Returns the culled food.
    ///
    /// Off-grid food is not indexed; culling it leaves `count` high, so the
    /// next broadcast falls back to a full rebuild.
    pub(super) fn cull(&mut self, radius: f64) -> Vec<Food> {
        let mut culled = Vec::new();
        self.sectors.retain(|&sector, entries| {
            if sector_reach(sector) < radius {
                return true;
            }
            let (mut shown, mut hidden) = (false, false);
            entries.retain(|entry| {
                let inside = entry.food.point().distance_lt(Point::default(), radius);
                if !inside {
                    shown |= entry.shown;
                    hidden |= !entry.shown;
                    culled.push(entry.food);
                    self.count = self.count.wrapping_sub(1);
                }
                inside
            });
            if shown {
                self.dirty.insert(sector);
            }
            if hidden && entries.iter().all(|entry| entry.shown) {
                self.hidden.remove(&sector);
            }
            !entries.is_empty()
        });
        culled
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
    pub(super) fn sync_food_sectors(&mut self) -> Result<FoodChanges, String> {
        let eaten = eaten_sectors(&self.food_eaters);
        let mut changed = eaten.clone();
        let now = self.time();
        let pit_changed = match (&mut self.pit_markers, self.world.pit_radius()) {
            (Some(markers), Some(radius)) => markers.update(radius, now)?,
            _ => false,
        };
        let retired = self.food_subscriptions.sync(self.slots.iter().map(|slot| {
            slot.connection
                .setup
                .then_some(&slot.connection.food_interest)
        }));
        let table = &mut self.food_sectors;
        table.resize(1 << 16, None);
        if self.pit_markers.is_some() {
            self.decorated_food_sectors.resize(1 << 16, None);
        }
        for index in retired {
            table[usize::from(index)] = None;
            if self.pit_markers.is_some() {
                self.decorated_food_sectors[usize::from(index)] = None;
            }
        }
        // Listings are independent and indexed by sector. Recipient packet
        // order comes from food_updates' sorted subscriptions, not this union.
        let mut list_sector = |index: u16| {
            let listing = &mut table[usize::from(index)];
            if listing.is_none() || self.food_index.dirty.contains(&index) {
                let run = self.food_index.listing(index);
                if listing.as_ref().is_none_or(|old| old.0 != run) {
                    *listing = Some(Arc::new(SectorFood(run)));
                    changed.insert(index.to_be_bytes());
                }
            }
            if (pit_changed
                || self
                    .decorated_food_sectors
                    .get(usize::from(index))
                    .is_none_or(Option::is_none)
                || self.food_index.dirty.contains(&index))
                && let Some(markers) = &self.pit_markers
            {
                let physical = listing.as_ref().unwrap();
                self.decorated_food_sectors[usize::from(index)] = Some(
                    markers
                        .overlay(index, &physical.0)
                        .map_or_else(|| physical.clone(), |run| Arc::new(SectorFood(run))),
                );
                // Pit motion or timed retirement can replace only the decorated
                // listing; stable recipients need a hint for either transition.
                changed.insert(index.to_be_bytes());
            }
        };
        if pit_changed {
            for &index in &self.food_subscriptions.sectors {
                list_sector(index);
            }
        } else {
            // A listing changes only on first subscription or shown-food edits.
            // Pit motion/timed retirement above still refreshes the full union.
            // Final counts exclude sectors that entered and retired in one sync;
            // handoffs keep their existing handles. Recipient order is separate.
            // research/performance/2026-10-27-shared-food-work/REPORT.md
            for &index in self
                .food_subscriptions
                .entered
                .union(&self.food_index.dirty)
            {
                if self.food_subscriptions.counts[usize::from(index)] != 0 {
                    list_sector(index);
                }
            }
        }
        // Unlisted sectors are built afresh when a connection subscribes.
        self.food_index.dirty.clear();
        let current = Arc::new(());
        let previous = std::mem::replace(&mut self.food_revision, current.clone());
        Ok(FoodChanges {
            changed: changed.into_iter().collect(),
            eaten,
            previous,
            current,
            ..Default::default()
        })
    }
}

/// Keep empty subscriptions internally. Retained receipts introduce food with
/// nonempty `F`, then births/pickups; see docs/server-rules.md#food-sector-delivery.
/// `sectors` lists every subscribed sector.
pub(super) fn food_updates(
    packets: &mut Vec<Vec<u8>>,
    known: &mut KnownSectors,
    interest: &SectorInterest,
    sectors: &[Option<Arc<SectorFood>>],
    changes: &FoodChanges,
    eaters: &BTreeMap<(u64, u64), u16>,
) {
    let subscribed = interest.subscribed();
    if known
        .subscription_revision
        .as_ref()
        .is_some_and(|revision| Arc::ptr_eq(revision, interest.revision()))
        && known
            .food_revision
            .as_ref()
            .is_some_and(|revision| Arc::ptr_eq(revision, &changes.previous))
    {
        let mut update = |sector, previous: &mut Arc<SectorFood>| {
            let current = sectors[usize::from(u16::from_be_bytes(sector))]
                .as_ref()
                .expect("subscribed sectors are listed");
            if !Arc::ptr_eq(previous, current) || changes.eaten.contains(&sector) {
                changes.append_diff(packets, sector, previous, current, eaters);
                if !Arc::ptr_eq(previous, current) {
                    previous.clone_from(current);
                }
            }
        };
        // Most shared hints belong to other views. Both collections are sorted,
        // so exclude sectors outside this recipient's range before choosing the
        // sparse search or dense walk. Holes within the range still need lookup.
        let changed = match (known.entries.first(), known.entries.last()) {
            (Some((first, _)), Some((last, _))) => {
                let start = changes.changed.partition_point(|sector| sector < first);
                let end = changes.changed.partition_point(|sector| sector <= last);
                &changes.changed[start..end]
            }
            _ => &[],
        };
        if changed.len() < known.entries.len() {
            for sector in changed {
                if let Ok(index) = known.entries.binary_search_by_key(sector, |(key, _)| *key) {
                    update(*sector, &mut known.entries[index].1);
                }
            }
        } else {
            for (sector, previous) in &mut known.entries {
                update(*sector, previous);
            }
        }
        known.food_revision = Some(changes.current.clone());
        return;
    }
    let mut subscriptions = subscribed.iter().peekable();
    // Retire every old sector before encoding additions, retaining the vector's
    // storage and the handles of subscriptions that still exist.
    known.entries.retain(|(sector, _)| {
        while subscriptions.next_if(|next| *next < sector).is_some() {}
        let keep = subscriptions.peek() == Some(&sector);
        if !keep {
            packets.push(encode::sector_clear(*sector));
        }
        keep
    });
    known
        .entries
        .reserve(subscribed.len() - known.entries.len());
    for (index, &sector) in subscribed.iter().enumerate() {
        let current = sectors[usize::from(u16::from_be_bytes(sector))]
            .as_ref()
            .expect("subscribed sectors are listed");
        if let Some((_, previous)) = known
            .entries
            .get_mut(index)
            .filter(|(held, _)| *held == sector)
        {
            if !Arc::ptr_eq(previous, current) || changes.eaten.contains(&sector) {
                changes.append_diff(packets, sector, previous, current, eaters);
                if !Arc::ptr_eq(previous, current) {
                    previous.clone_from(current);
                }
            }
        } else {
            let foods: Vec<_> = current.0.iter().map(|(k, v)| (*k, v.style)).collect();
            if !foods.is_empty() {
                packets.extend(encode::food_sector(sector, &foods));
            }
            // Retained entries are sorted; inserting new subscriptions here
            // preserves both packet order and the next retained handle.
            known.entries.insert(index, (sector, current.clone()));
        }
    }
    known.subscription_revision = Some(interest.revision().clone());
    known.food_revision = Some(changes.current.clone());
}

fn sector_changes(
    packets: &mut Vec<Vec<u8>>,
    sector: [u8; 2],
    previous: &SectorFood,
    current: &SectorFood,
    eaters: &BTreeMap<(u64, u64), u16>,
) {
    let key = |offset: [u8; 2]| [sector[0], sector[1], offset[0], offset[1]];
    // Decorations have no physical lifetime. Sub-cell pit motion is invisible
    // on the wire; real pellets still distinguish exact pickup positions.
    let unchanged = |before: Option<&KnownFood>, after: &KnownFood| {
        before.is_some_and(|old| {
            old == after || (old.decorative && after.decorative && old.style == after.style)
        })
    };
    for (offset, food) in &previous.0 {
        let eater = (!food.decorative)
            .then(|| eaters.get(&food_identity(food.point)).copied())
            .flatten();
        if !unchanged(current.get(offset), food) || eater.is_some() {
            packets.push(encode::food_remove(key(*offset), eater));
        }
    }
    for (offset, food) in &current.0 {
        if !unchanged(previous.get(offset), food)
            || previous.get(offset).is_some_and(|old| {
                !old.decorative && eaters.contains_key(&food_identity(old.point))
            })
        {
            if food.decorative {
                // F uses the stock rapid fade without mislabelling decorations
                // as snake food. Ordinary f births fade too slowly for a live edge.
                packets.extend(encode::food_sector(sector, &[(*offset, food.style)]));
            } else {
                packets.push(encode::food_add(key(*offset), food.style));
            }
        }
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

/// Bound on the center distance of any pellet keyed to `sector`. Wire cells
/// round to the nearest 300/256 units, so a unit of slack covers both edges.
fn sector_reach(sector: u16) -> f64 {
    let size = f64::from(encode::SECTOR_SIZE);
    let far = |cell: u8| {
        let low = f64::from(cell) * size - f64::from(encode::GRID_CENTER);
        low.abs().max((low + size).abs()) + 1.
    };
    let [x, y] = sector.to_be_bytes();
    far(x).hypot(far(y))
}

pub(super) fn food_identity(point: Point) -> (u64, u64) {
    let bits = |v: f64| if v == 0. { 0 } else { v.to_bits() };
    (bits(point.x), bits(point.y))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retiring_membership_keeps_unchanged_pickups_and_fresh_listing_delivery() {
        let point = Point::default();
        let wire = encode::food_key(point).unwrap();
        let sector = [wire[0], wire[1]];
        let retired = [sector[0], sector[1] + 1];
        let offset = [wire[2], wire[3]];
        let style = encode::FoodStyle {
            color: 4,
            size: 20,
            snake_food: false,
        };
        let listing = Arc::new(SectorFood(vec![(
            offset,
            KnownFood {
                point,
                style,
                decorative: false,
            },
        )]));
        let mut sectors = vec![None; 1 << 16];
        sectors[usize::from(u16::from_be_bytes(sector))] = Some(listing.clone());
        sectors[usize::from(u16::from_be_bytes(retired))] = Some(Arc::new(SectorFood::default()));
        let first = FoodChanges::default();
        let mut known = KnownSectors::default();
        let mut packets = Vec::new();
        food_updates(
            &mut packets,
            &mut known,
            &SectorInterest::from_subscribed(BTreeSet::from([sector, retired])),
            &sectors,
            &first,
            &BTreeMap::new(),
        );
        packets.clear();
        let interest = SectorInterest::from_subscribed(BTreeSet::from([sector]));
        let changes = FoodChanges {
            previous: first.current.clone(),
            eaten: BTreeSet::from([sector]),
            ..Default::default()
        };
        let eaters = BTreeMap::from([(food_identity(point), 17)]);
        food_updates(
            &mut packets,
            &mut known,
            &interest,
            &sectors,
            &changes,
            &eaters,
        );
        assert_eq!(
            packets,
            [
                encode::sector_clear(retired),
                encode::food_remove(wire, Some(17)),
                encode::food_add(wire, style)
            ]
        );
        assert!(Arc::ptr_eq(known.get(&sector).unwrap(), &listing));
        assert!(!known.contains_key(&retired));
        // Fresh state, as on reconnect, must still receive the initial listing.
        let mut fresh = KnownSectors::default();
        packets.clear();
        food_updates(
            &mut packets,
            &mut fresh,
            &interest,
            &sectors,
            &FoodChanges::default(),
            &BTreeMap::new(),
        );
        assert_eq!(packets, encode::food_sector(sector, &[(offset, style)]));
        assert!(Arc::ptr_eq(fresh.get(&sector).unwrap(), &listing));
    }

    #[test]
    fn shared_diffs_match_uncached_lifetimes_views_and_older_recipients() {
        let sector = [108, 108];
        let point = Point::default();
        let wire = encode::food_key(point).unwrap();
        let offset = [wire[2], wire[3]];
        let food = KnownFood {
            point,
            decorative: false,
            style: encode::FoodStyle {
                color: 4,
                size: 20,
                snake_food: false,
            },
        };
        let old = Arc::new(SectorFood(vec![(offset, food)]));
        let older = Arc::new(SectorFood::default());
        let physical = Arc::new(SectorFood(vec![
            (offset, food),
            (
                [200, 200],
                KnownFood {
                    point: Point { x: 100., y: 100. },
                    ..food
                },
            ),
        ]));
        let decorated = Arc::new(SectorFood(vec![(
            offset,
            KnownFood {
                decorative: true,
                ..food
            },
        )]));
        let eaters = BTreeMap::from([(food_identity(point), 17)]);
        let changes = FoodChanges::default();
        // One shared physical transition and one decorated transition; an older
        // view falls back without replacing the common transition. Also cover
        // pickups with no listing change and distinct sectors sharing handles.
        for (key, previous, current) in [
            (sector, &old, &physical),
            (sector, &old, &decorated),
            (sector, &older, &physical),
            (sector, &old, &physical),
            (sector, &decorated, &decorated),
            ([107, 108], &old, &physical),
        ] {
            let mut expected = Vec::new();
            sector_changes(&mut expected, key, previous, current, &eaters);
            for _ in 0..3 {
                let mut actual = Vec::new();
                changes.append_diff(&mut actual, key, previous, current, &eaters);
                assert_eq!(actual, expected);
            }
        }
        assert_eq!(changes.diffs.borrow().len(), 3);
        // A new broadcast must not reuse the preceding pickup set, even when
        // both listing identities remain the same.
        let next = FoodChanges::default();
        let mut actual = Vec::new();
        next.append_diff(&mut actual, sector, &old, &old, &BTreeMap::new());
        assert!(actual.is_empty());
        changes.append_diff(&mut actual, sector, &old, &old, &eaters);
        assert_eq!(
            actual,
            [
                encode::food_remove(wire, Some(17)),
                encode::food_add(wire, food.style)
            ]
        );
    }

    // Frozen exhaustive union walk: independent of sparse work hints.
    fn reference_sync_food_sectors(backend: &mut SimulatorBackend) -> Result<FoodChanges, String> {
        let eaten = eaten_sectors(&backend.food_eaters);
        let mut changed = eaten.clone();
        let now = backend.time();
        let pit_changed = match (&mut backend.pit_markers, backend.world.pit_radius()) {
            (Some(markers), Some(radius)) => markers.update(radius, now)?,
            _ => false,
        };
        let retired = backend
            .food_subscriptions
            .sync(backend.slots.iter().map(|slot| {
                slot.connection
                    .setup
                    .then_some(&slot.connection.food_interest)
            }));
        let table = &mut backend.food_sectors;
        table.resize(1 << 16, None);
        if backend.pit_markers.is_some() {
            backend.decorated_food_sectors.resize(1 << 16, None);
        }
        for index in retired {
            table[usize::from(index)] = None;
            if backend.pit_markers.is_some() {
                backend.decorated_food_sectors[usize::from(index)] = None;
            }
        }
        // Listings are independent and indexed by sector. Recipient packet
        // order comes from food_updates' sorted subscriptions, not this union.
        for &index in &backend.food_subscriptions.sectors {
            let listing = &mut table[usize::from(index)];
            if listing.is_none() || backend.food_index.dirty.contains(&index) {
                let run = backend.food_index.listing(index);
                if listing.as_ref().is_none_or(|old| old.0 != run) {
                    *listing = Some(Arc::new(SectorFood(run)));
                    changed.insert(index.to_be_bytes());
                }
            }
            if (pit_changed
                || backend
                    .decorated_food_sectors
                    .get(usize::from(index))
                    .is_none_or(Option::is_none)
                || backend.food_index.dirty.contains(&index))
                && let Some(markers) = &backend.pit_markers
            {
                let physical = listing.as_ref().unwrap();
                backend.decorated_food_sectors[usize::from(index)] = Some(
                    markers
                        .overlay(index, &physical.0)
                        .map_or_else(|| physical.clone(), |run| Arc::new(SectorFood(run))),
                );
                // Pit motion or timed retirement can replace only the decorated
                // listing; stable recipients need a hint for either transition.
                changed.insert(index.to_be_bytes());
            }
        }
        // Unlisted sectors are built afresh when a connection subscribes.
        backend.food_index.dirty.clear();
        let current = Arc::new(());
        let previous = std::mem::replace(&mut backend.food_revision, current.clone());
        Ok(FoodChanges {
            changed: changed.into_iter().collect(),
            eaten,
            previous,
            current,
            ..Default::default()
        })
    }

    #[test]
    fn sparse_listing_work_matches_full_union_through_lifetimes_and_pit_changes() {
        use crate::{Action, Config, PitConfig, RadiusUpdate, client::SimulationConfig};
        for pit in [false, true] {
            let mut world = Config::sandbox();
            world.radius = 8960.;
            world.pit = pit.then_some(PitConfig { radius: 4460. });
            world.food_count = 0;
            world.ambient_replenish_per_second = 0.;
            world.prey.count = 0;
            world.radius_updates = vec![
                RadiusUpdate {
                    time: 0.,
                    radius: 8960.,
                },
                RadiusUpdate {
                    time: 8. * world.dt,
                    radius: 8900.,
                },
                RadiusUpdate {
                    time: 12. * world.dt,
                    radius: 9000.,
                },
            ];
            let config = SimulationConfig {
                world,
                agents: 3,
                ..Default::default()
            };
            let mut actual = SimulatorBackend::new(47, config.clone()).unwrap();
            let mut reference = SimulatorBackend::new(47, config).unwrap();
            let foods: Vec<_> = (0..8)
                .map(|i| Food {
                    x: if i < 4 { 0. } else { 4560. },
                    y: if i % 2 == 0 { 0. } else { 900. },
                    value: i as f64 + 1.,
                    size: Some(i as f64 + 1.),
                    origin: Some(Origin::Ambient),
                })
                .collect();
            let sectors: BTreeSet<_> = foods
                .iter()
                .map(|food| FoodIndex::sector(food).unwrap().0.to_be_bytes())
                .chain([[108, 110], [110, 108]])
                .collect();
            let first = SectorInterest::from_subscribed(sectors.clone());
            let second = SectorInterest::from_subscribed(sectors.iter().copied().skip(1).collect());
            let empty = SectorInterest::from_subscribed(BTreeSet::new());
            let mut active = Vec::new();
            for tick in 0..180 {
                let addition = &foods[tick % foods.len()];
                active.push(*addition);
                let removal = if tick % 3 == 1 {
                    Some(active.remove(0))
                } else {
                    None
                };
                for backend in [&mut actual, &mut reference] {
                    if tick > 0 {
                        let actions: Vec<_> = backend
                            .world
                            .snakes()
                            .iter()
                            .map(|snake| {
                                snake.alive.then_some(Action {
                                    heading: snake.heading,
                                    boost: false,
                                })
                            })
                            .collect();
                        backend.world.step(&actions).unwrap();
                        backend.tick += 1;
                    }
                    for slot in 0..3 {
                        let choice = (tick / 7 + slot) % 5;
                        backend.slots[slot].connection.setup = choice != 0;
                        backend.slots[slot].connection.food_interest = match choice {
                            0 | 1 => first.clone(),
                            2 | 3 => second.clone(),
                            _ => empty.clone(),
                        };
                    }
                    backend
                        .food_index
                        .add(addition, tick % 4 != 0, (tick % 9) as u8);
                    if let Some(food) = &removal {
                        backend.food_index.remove(food);
                    }
                    if tick % 6 == 0 {
                        backend.food_index.reveal();
                    }
                    if tick % 29 == 0 {
                        backend.food_presentation.sync(&active).unwrap();
                        backend
                            .food_index
                            .rebuild(&active, &backend.food_presentation);
                    }
                    backend.food_eaters.clear();
                    if tick % 5 == 0 {
                        backend
                            .food_eaters
                            .insert(food_identity(addition.point()), 1);
                    }
                }
                let got = actual.sync_food_sectors().unwrap();
                let want = reference_sync_food_sectors(&mut reference).unwrap();
                assert_eq!(got.changed, want.changed, "pit={pit} tick={tick}");
                assert_eq!(got.eaten, want.eaten, "pit={pit} tick={tick}");
                for (name, actual, reference) in [
                    ("physical", &actual.food_sectors, &reference.food_sectors),
                    (
                        "decorated",
                        &actual.decorated_food_sectors,
                        &reference.decorated_food_sectors,
                    ),
                ] {
                    assert_eq!(actual.len(), reference.len());
                    for (sector, (actual, reference)) in actual.iter().zip(reference).enumerate() {
                        assert_eq!(
                            actual, reference,
                            "{name} pit={pit} tick={tick} sector={sector}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn subscription_union_matches_full_rebuild_through_handoffs_and_slot_reuse() {
        let interest = |sectors: &[[u8; 2]]| {
            SectorInterest::from_subscribed(sectors.iter().copied().collect())
        };
        let a = interest(&[[0, 0], [1, 2], [255, 255]]);
        let b = interest(&[[1, 2], [2, 3]]);
        let c = interest(&[[3, 4], [4, 5]]);
        let empty = interest(&[]);
        let cases = [
            vec![Some(a.clone()), Some(b.clone())],
            vec![Some(a.clone()), Some(b.clone())],
            // Neither owner nor a fresh identity with equal membership may
            // retire a sector that still belongs to the final union.
            vec![Some(b.clone()), Some(a.clone())],
            vec![
                Some(b.clone()),
                Some(interest(&[[0, 0], [1, 2], [255, 255]])),
            ],
            vec![None, Some(a.clone()), Some(c.clone())],
            vec![Some(c.clone()), Some(empty.clone())],
            vec![],
            vec![Some(empty), None, Some(a.clone()), Some(a)],
            vec![None],
        ];
        let mut cache = FoodSubscriptions::default();
        let mut previous = BTreeSet::new();
        for case in cases {
            let mut counts = vec![0; 1 << 16];
            let expected: BTreeSet<_> = case
                .iter()
                .flatten()
                .flat_map(|i| {
                    i.subscribed()
                        .iter()
                        .map(|&sector| u16::from_be_bytes(sector))
                })
                .collect();
            for interest in case.iter().flatten() {
                for &sector in interest.subscribed() {
                    counts[usize::from(u16::from_be_bytes(sector))] += 1;
                }
            }
            let retired = cache.sync(case.iter().map(Option::as_ref));
            assert_eq!(cache.sectors, expected);
            assert_eq!(cache.counts, counts);
            assert_eq!(cache.held.len(), case.len());
            assert_eq!(retired, previous.difference(&expected).copied().collect());
            previous = expected;
        }
    }

    #[test]
    fn sector_handoff_keeps_both_listing_handles_until_the_last_subscriber_leaves() {
        let mut world = crate::Config::sandbox();
        world.pit = Some(crate::PitConfig { radius: 500. });
        let mut backend = SimulatorBackend::new(
            7,
            crate::client::SimulationConfig {
                world,
                agents: 2,
                ..Default::default()
            },
        )
        .unwrap();
        let sectors = BTreeSet::from([[108, 110], [110, 108]]);
        backend.slots[0].connection.setup = true;
        backend.slots[1].connection.setup = false;
        backend.slots[0].connection.food_interest =
            SectorInterest::from_subscribed(sectors.clone());
        backend.sync_food_sectors().unwrap();
        let index = usize::from(u16::from_be_bytes([110, 108]));
        let physical = backend.food_sectors[index].as_ref().unwrap().clone();
        let decorated = backend.decorated_food_sectors[index]
            .as_ref()
            .unwrap()
            .clone();
        assert!(!Arc::ptr_eq(&physical, &decorated));
        backend.slots[0].connection.setup = false;
        backend.slots[1].connection.setup = true;
        backend.slots[1].connection.food_interest =
            SectorInterest::from_subscribed(sectors.clone());
        backend.sync_food_sectors().unwrap();
        assert!(Arc::ptr_eq(
            &physical,
            backend.food_sectors[index].as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(
            &decorated,
            backend.decorated_food_sectors[index].as_ref().unwrap()
        ));
        backend.slots[1].connection.setup = false;
        backend.sync_food_sectors().unwrap();
        assert!(backend.food_sectors[index].is_none());
        assert!(backend.decorated_food_sectors[index].is_none());
        backend.slots[0].connection.setup = true;
        backend.slots[0].connection.food_interest = SectorInterest::from_subscribed(sectors);
        backend.sync_food_sectors().unwrap();
        assert!(!Arc::ptr_eq(
            &physical,
            backend.food_sectors[index].as_ref().unwrap()
        ));
    }

    /// Full ordered-map walk, independent of membership identity and change hints.
    fn reference_updates(
        packets: &mut Vec<Vec<u8>>,
        known: &mut BTreeMap<[u8; 2], Arc<SectorFood>>,
        subscribed: &BTreeSet<[u8; 2]>,
        sectors: &[Option<Arc<SectorFood>>],
        eaters: &BTreeMap<(u64, u64), u16>,
    ) {
        let gone: Vec<_> = known
            .keys()
            .filter(|key| !subscribed.contains(*key))
            .copied()
            .collect();
        for sector in gone {
            packets.push(encode::sector_clear(sector));
            known.remove(&sector);
        }
        for &sector in subscribed {
            let current = sectors[usize::from(u16::from_be_bytes(sector))]
                .as_ref()
                .unwrap();
            if let Some(previous) = known.get(&sector) {
                sector_changes(packets, sector, previous, current, eaters);
            } else if !current.0.is_empty() {
                let food: Vec<_> = current
                    .0
                    .iter()
                    .map(|(key, food)| (*key, food.style))
                    .collect();
                packets.extend(encode::food_sector(sector, &food));
            }
            known.insert(sector, current.clone());
        }
    }

    #[test]
    fn ranged_hints_match_full_walk_for_every_subscription_and_change_subset() {
        // Include byte boundaries, holes and both extrema. A changed key can
        // lie outside a view, at either endpoint, or in an unsubscribed hole.
        let keys = [
            [0, 0],
            [0, 1],
            [0, 255],
            [1, 0],
            [127, 255],
            [255, 254],
            [255, 255],
        ];
        let old = Arc::new(SectorFood::default());
        let new = Arc::new(SectorFood(vec![(
            [2, 3],
            KnownFood {
                point: Point::default(),
                decorative: false,
                style: encode::FoodStyle {
                    color: 3,
                    size: 20,
                    snake_food: false,
                },
            },
        )]));
        let mut sectors = vec![None; 1 << 16];
        let eaters = BTreeMap::new();
        for changed_mask in 0..1 << keys.len() {
            let changes = FoodChanges {
                changed: keys
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &key)| (changed_mask & (1 << i) != 0).then_some(key))
                    .collect(),
                ..Default::default()
            };
            for (i, key) in keys.iter().enumerate() {
                sectors[usize::from(u16::from_be_bytes(*key))] =
                    Some(if changed_mask & (1 << i) != 0 {
                        new.clone()
                    } else {
                        old.clone()
                    });
            }
            for held_mask in 0..1 << keys.len() {
                let subscribed = keys
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &key)| (held_mask & (1 << i) != 0).then_some(key))
                    .collect();
                let interest = SectorInterest::from_subscribed(subscribed);
                let mut known = KnownSectors {
                    entries: interest
                        .subscribed()
                        .iter()
                        .map(|&key| (key, old.clone()))
                        .collect(),
                    subscription_revision: Some(interest.revision().clone()),
                    food_revision: Some(changes.previous.clone()),
                };
                let mut reference = known.entries.iter().cloned().collect();
                let mut actual = Vec::new();
                let mut expected = Vec::new();
                food_updates(
                    &mut actual,
                    &mut known,
                    &interest,
                    &sectors,
                    &changes,
                    &eaters,
                );
                reference_updates(
                    &mut expected,
                    &mut reference,
                    interest.subscribed(),
                    &sectors,
                    &eaters,
                );
                assert_eq!(actual, expected, "held={held_mask}, changed={changed_mask}");
                assert!(Arc::ptr_eq(
                    known.food_revision.as_ref().unwrap(),
                    &changes.current
                ));
                for (sector, listing) in &known.entries {
                    assert!(Arc::ptr_eq(listing, &reference[sector]));
                }
            }
        }
    }

    #[test]
    fn sparse_and_dense_updates_match_full_walk_for_both_food_streams() {
        let point = |i: u8| Point {
            x: -32_245.,
            y: f64::from(i) * 300. - 32_545.,
        };
        let item = |i, color, decorative| KnownFood {
            point: point(i),
            decorative,
            style: encode::FoodStyle {
                color,
                size: 20,
                snake_food: false,
            },
        };
        let mut physical = vec![None; 1 << 16];
        let mut decorated = physical.clone();
        let mut known = KnownSectors::default();
        let mut shown = KnownSectors::default();
        let mut reference = BTreeMap::new();
        let mut shown_reference = BTreeMap::new();
        let mut interest = SectorInterest::default();
        let mut revision = Arc::default();
        for step in 0..48 {
            let mut changes = FoodChanges {
                previous: revision,
                ..Default::default()
            };
            revision = changes.current.clone();
            let updated: Vec<u8> = if step % 8 == 0 {
                (0..32).collect()
            } else if step % 7 == 0 {
                Vec::new()
            } else {
                vec![step % 32, ((u16::from(step) * 7 + 1) % 32) as u8]
            };
            for i in updated {
                let mut foods = if (step + i) % 5 == 0 {
                    Vec::new()
                } else {
                    vec![([2, 3], item(i, (step + i) % 9, false))]
                };
                let index = usize::from(u16::from_be_bytes([1, i]));
                physical[index] = Some(Arc::new(SectorFood(foods.clone())));
                // A decoration at the pickup's physical point must remain visible.
                foods.push(([7, 8], item(i, 5, true)));
                decorated[index] = Some(Arc::new(SectorFood(foods)));
                changes.changed.push([1, i]);
            }
            if step % 9 == 0 {
                interest = SectorInterest::from_subscribed(
                    (0..12).map(|j| [1, (step + j * 5) % 32]).collect(),
                );
            }
            if step == 23 {
                known = KnownSectors::default();
                shown = KnownSectors::default();
                reference.clear();
                shown_reference.clear();
            }
            let eaters = if step % 7 == 0 {
                BTreeMap::new()
            } else {
                BTreeMap::from([(food_identity(point(((u16::from(step) * 11) % 32) as u8)), 4)])
            };
            changes.eaten = eaten_sectors(&eaters);
            changes.changed.extend(changes.eaten.iter().copied());
            changes.changed.sort_unstable();
            changes.changed.dedup();
            for (stream, (held, oracle, table)) in [
                (&mut known, &mut reference, &physical),
                (&mut shown, &mut shown_reference, &decorated),
            ]
            .into_iter()
            .enumerate()
            {
                if step == 12 && stream == 1 {
                    continue; // This stream must catch up from an older snapshot.
                }
                let mut actual = Vec::new();
                let mut expected = Vec::new();
                food_updates(&mut actual, held, &interest, table, &changes, &eaters);
                reference_updates(&mut expected, oracle, interest.subscribed(), table, &eaters);
                assert_eq!(actual, expected, "step={step}");
                assert_eq!(
                    held.entries.iter().map(|(key, _)| *key).collect::<Vec<_>>(),
                    oracle.keys().copied().collect::<Vec<_>>()
                );
                for (sector, listing) in &held.entries {
                    assert!(Arc::ptr_eq(listing, &oracle[sector]));
                }
            }
        }
    }

    #[test]
    fn stable_pit_recipient_matches_full_walk_for_overlap_and_timed_retirement() {
        use crate::{Config, RadiusUpdate, client::SimulationConfig};
        const SHRINK: usize = 72;
        const RETIRE: usize = 144;
        let mut world = Config::battledome();
        world.food_count = 0;
        world.ambient_replenish_per_second = 0.;
        // Script one inward step then hold the live edge, isolating retirement
        // without another pit move or physical-food event supplying its hint.
        world.radius_updates = (0..=160)
            .map(|tick| RadiusUpdate {
                time: tick as f64 * world.dt,
                radius: if tick < SHRINK { 8960. } else { 8900. },
            })
            .collect();
        let mut backend = SimulatorBackend::new(
            7,
            SimulationConfig {
                world,
                ..Default::default()
            },
        )
        .unwrap();
        let mut interest = SectorInterest::default();
        interest.advance(Point { x: 4560., y: 0. }, 0., 0.);
        backend.slots[0].connection.setup = true;
        backend.slots[0].connection.food_interest = interest.clone();
        let mut known = KnownSectors::default();
        let mut reference = BTreeMap::new();
        let mut physical = Vec::new();
        for tick in 0..=160 {
            if tick > 0 {
                let heading = backend.world.snakes()[0].heading;
                backend
                    .world
                    .step(&[Some(crate::Action {
                        heading,
                        boost: false,
                    })])
                    .unwrap();
                backend.tick += 1;
            }
            let changes = backend.sync_food_sectors().unwrap();
            assert!(backend.world.food().is_empty() && changes.eaten.is_empty());
            if tick == 0 {
                physical = interest
                    .subscribed()
                    .iter()
                    .map(|sector| {
                        let index = usize::from(u16::from_be_bytes(*sector));
                        (index, backend.food_sectors[index].as_ref().unwrap().clone())
                    })
                    .collect();
            } else {
                // Prove the recipient takes the stable-revision path and has
                // no changed physical listing to accidentally cover a lost hint.
                assert!(Arc::ptr_eq(
                    known.subscription_revision.as_ref().unwrap(),
                    interest.revision()
                ));
                assert!(Arc::ptr_eq(
                    known.food_revision.as_ref().unwrap(),
                    &changes.previous
                ));
                for (index, held) in &physical {
                    assert!(Arc::ptr_eq(
                        held,
                        backend.food_sectors[*index].as_ref().unwrap()
                    ));
                }
            }
            let mut actual = Vec::new();
            let mut expected = Vec::new();
            food_updates(
                &mut actual,
                &mut known,
                &interest,
                &backend.decorated_food_sectors,
                &changes,
                &backend.food_eaters,
            );
            reference_updates(
                &mut expected,
                &mut reference,
                interest.subscribed(),
                &backend.decorated_food_sectors,
                &backend.food_eaters,
            );
            assert_eq!(actual, expected, "tick={tick}");
            match tick {
                0 => assert!(actual.iter().any(|p| p[0] == b'F')),
                SHRINK => {
                    assert!(!changes.changed.is_empty());
                    assert!(
                        actual.iter().any(|p| p[0] == b'F'),
                        "new ring needs rapid additions"
                    );
                    assert!(
                        actual.iter().all(|p| p[0] == b'F'),
                        "old ring must remain during overlap"
                    );
                }
                RETIRE => {
                    assert!(!changes.changed.is_empty());
                    assert!(
                        actual.iter().any(|p| p[0] == b'c'),
                        "old ring must retire without pit motion"
                    );
                    assert!(actual.iter().all(|p| p[0] == b'c'));
                }
                _ => assert!(actual.is_empty(), "quiet tick={tick}"),
            }
            for (sector, held) in &known.entries {
                assert!(Arc::ptr_eq(held, &reference[sector]));
            }
        }
        let settled =
            super::super::markers::PitMarkers::new(backend.world.pit_radius().unwrap()).unwrap();
        for (sector, held) in &known.entries {
            assert_eq!(
                held.0,
                settled
                    .overlay(u16::from_be_bytes(*sector), &[])
                    .unwrap_or_default()
            );
        }
    }

    #[test]
    fn shrinking_markers_only_send_changes_to_their_wire_cells() {
        let old = super::super::markers::PitMarkers::new(1784.).unwrap();
        let next_radius = 1784. * ((3584. - 23.4375 * 0.007) / 3584.);
        let next = super::super::markers::PitMarkers::new(next_radius).unwrap();
        let mut unchanged = 0;
        let mut moved = 0;
        for sector in 0..=u16::MAX {
            let previous = SectorFood(old.overlay(sector, &[]).unwrap_or_default());
            let current = SectorFood(next.overlay(sector, &[]).unwrap_or_default());
            if previous.0.is_empty() && current.0.is_empty() {
                continue;
            }
            let sector = sector.to_be_bytes();
            let mut packets = Vec::new();
            sector_changes(&mut packets, sector, &previous, &current, &BTreeMap::new());
            for (offset, food) in &previous.0 {
                let key = [sector[0], sector[1], offset[0], offset[1]];
                if let Some(now) = current.get(offset) {
                    assert_eq!(food.style, now.style);
                    if food.point != now.point {
                        unchanged += 1;
                    }
                    assert!(
                        !packets.iter().any(|p| p[1..5] == key),
                        "unchanged encoded decoration was resent"
                    );
                } else {
                    moved += 1;
                    assert!(packets.contains(&encode::food_remove(key, None)));
                }
            }
            for (offset, food) in &current.0 {
                if previous.get(offset).is_none() {
                    assert!(
                        packets.contains(&encode::food_sector(sector, &[(*offset, food.style)])[0])
                    );
                }
            }
        }
        assert!(unchanged > 0 && moved > 0);
    }

    #[test]
    fn same_cell_physical_food_replacements_keep_their_lifetime_identity() {
        let old = KnownFood {
            point: Point { x: 0., y: 0. },
            style: encode::FoodStyle {
                color: 8,
                size: 31,
                snake_food: false,
            },
            decorative: false,
        };
        let next = KnownFood {
            point: Point { x: 0.1, y: 0. },
            ..old
        };
        let key = encode::food_key(old.point).unwrap();
        assert_eq!(encode::food_key(next.point).unwrap(), key);
        let previous = SectorFood(vec![([key[2], key[3]], old)]);
        let current = SectorFood(vec![([key[2], key[3]], next)]);
        let mut packets = Vec::new();
        sector_changes(
            &mut packets,
            [key[0], key[1]],
            &previous,
            &current,
            &BTreeMap::new(),
        );
        assert_eq!(
            packets,
            [
                encode::food_remove(key, None),
                encode::food_add(key, next.style)
            ]
        );
    }

    #[test]
    fn a_birth_after_an_unindexed_startup_pickup_waits_for_rebuild() {
        let food = Food {
            x: 0.,
            y: 0.,
            value: 1.,
            size: Some(1.),
            origin: None,
        };
        let mut index = FoodIndex::default();
        index.remove(&food);
        assert!(index.stale);
        index.add(&food, false, 0);
        assert_eq!(index.count, 0);
        assert!(index.stale);
        let mut presentation = FoodPresentation::new(7, 1);
        presentation.sync(std::slice::from_ref(&food)).unwrap();
        index.rebuild(std::slice::from_ref(&food), &presentation);
        assert_eq!(index.count, 1);
        assert!(!index.stale);
    }

    #[test]
    fn a_wall_cull_dirties_only_sectors_that_lose_shown_food() {
        let pellet = |x, y| Food {
            x,
            y,
            value: 1.,
            size: Some(2.),
            origin: None,
        };
        let sector = |x, y| FoodIndex::sector(&pellet(x, y)).unwrap().0;
        let mut index = FoodIndex::default();
        // Quiet: entirely inside, with a hidden birth that must stay hidden.
        index.add(&pellet(0., 0.), true, 1);
        index.add(&pellet(10., 0.), true, 2);
        index.add(&pellet(20., 0.), false, 3);
        // Mixed: one survivor, a shown and a hidden pellet outside.
        index.add(&pellet(600., 0.), true, 4);
        index.add(&pellet(700., 0.), true, 5);
        index.add(&pellet(710., 0.), false, 6);
        // Two lifetimes at one point leave together.
        index.add(&pellet(1000., 0.), true, 7);
        index.add(&pellet(1000., 0.), true, 8);
        // Hidden only: no listing changes.
        index.add(&pellet(0., 1000.), false, 0);
        let (quiet, mixed, gone, unseen) = (
            sector(0., 0.),
            sector(600., 0.),
            sector(1000., 0.),
            sector(0., 1000.),
        );
        assert_eq!(sector(700., 0.), mixed);
        let before = index.listing(quiet);
        index.dirty.clear();
        let mut culled: Vec<_> = index.cull(670.).iter().map(|f| (f.x, f.y)).collect();
        culled.sort_by(|a, b| a.partial_cmp(b).unwrap());
        assert_eq!(
            culled,
            [
                (0., 1000.),
                (700., 0.),
                (710., 0.),
                (1000., 0.),
                (1000., 0.)
            ]
        );
        assert_eq!(index.count, 4);
        assert_eq!(index.dirty, BTreeSet::from([mixed, gone]));
        assert_eq!(index.hidden, BTreeSet::from([quiet]));
        assert!(!index.sectors.contains_key(&gone));
        assert!(!index.sectors.contains_key(&unseen));
        assert_eq!(index.listing(quiet), before);
        let mixed = index.listing(mixed);
        assert_eq!(mixed.len(), 1);
        assert_eq!(mixed[0].1.point, Point { x: 600., y: 0. });
        assert_eq!(mixed[0].1.style.color, 4);
        // The quiet sector's hidden birth is still revealed at maintenance.
        index.reveal();
        assert_eq!(index.listing(quiet).len(), 3);
    }

    #[test]
    fn sector_reach_bounds_every_wire_keyed_point() {
        for x in [-32550., -150.6, -150., 149.4, 450., 32985.] {
            for y in [-32550., -0.6, 0., 299.4, 32985.] {
                let point = Point { x, y };
                let key = encode::food_key(point).unwrap();
                let reach = sector_reach(u16::from_be_bytes([key[0], key[1]]));
                assert!(point.distance(Point::default()) < reach, "{point:?}");
            }
        }
    }

    #[test]
    fn retained_subscriptions_keep_storage_and_update_only_changed_handles() {
        let food = KnownFood {
            point: Point::default(),
            decorative: false,
            style: encode::FoodStyle {
                color: 3,
                size: 20,
                snake_food: false,
            },
        };
        let mut sectors = vec![None; 1 << 16];
        sectors[1] = Some(Arc::new(SectorFood(vec![([2, 3], food)])));
        sectors[2] = Some(Arc::new(SectorFood::default()));
        sectors[3] = Some(Arc::new(SectorFood::default()));
        let interest = SectorInterest::from_subscribed(BTreeSet::from([[0, 1], [0, 2]]));
        let mut known = KnownSectors::default();
        let mut packets = Vec::new();
        let eaters = BTreeMap::new();
        let initial = FoodChanges::default();
        food_updates(
            &mut packets,
            &mut known,
            &interest,
            &sectors,
            &initial,
            &eaters,
        );
        let storage = known.entries.as_ptr();
        packets.clear();
        sectors[1] = Some(Arc::new(SectorFood::default()));
        food_updates(
            &mut packets,
            &mut known,
            &interest,
            &sectors,
            &FoodChanges {
                changed: vec![[0, 1]],
                previous: initial.current.clone(),
                ..Default::default()
            },
            &eaters,
        );
        assert_eq!(known.entries.as_ptr(), storage);
        assert_eq!(packets, [encode::food_remove([0, 1, 2, 3], None)]);
        for (key, listing) in &known.entries {
            assert!(Arc::ptr_eq(
                listing,
                sectors[usize::from(u16::from_be_bytes(*key))]
                    .as_ref()
                    .unwrap()
            ));
        }
        // Equal counts do not imply equal subscriptions: the old sector clears.
        packets.clear();
        food_updates(
            &mut packets,
            &mut known,
            &SectorInterest::from_subscribed(BTreeSet::from([[0, 2], [0, 3]])),
            &sectors,
            &FoodChanges::default(),
            &eaters,
        );
        assert_eq!(packets, [encode::sector_clear([0, 1])]);
        assert_eq!(
            known
                .entries
                .iter()
                .map(|(key, _)| *key)
                .collect::<Vec<_>>(),
            [[0, 2], [0, 3]]
        );
    }
}

#[cfg(test)]
#[path = "food/held_tests.rs"]
mod held_tests;
