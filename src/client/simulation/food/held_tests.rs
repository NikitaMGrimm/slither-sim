// Frozen membership-refresh owner from 47811bc8; compare bytes and handles.
use super::*;

fn frozen_food_updates(
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
    let held = std::mem::take(&mut known.entries);
    let mut subscriptions = subscribed.iter().peekable();
    for (sector, _) in &held {
        while subscriptions.next_if(|next| *next < sector).is_some() {}
        if subscriptions.peek() != Some(&sector) {
            packets.push(encode::sector_clear(*sector));
        }
    }
    let mut held = held.into_iter().peekable();
    known.entries.reserve(subscribed.len());
    for &sector in subscribed {
        while held.next_if(|(old, _)| *old < sector).is_some() {}
        let previous = held
            .next_if(|(old, _)| *old == sector)
            .map(|(_, food)| food);
        let current = sectors[usize::from(u16::from_be_bytes(sector))]
            .as_ref()
            .expect("subscribed sectors are listed");
        let held = match previous {
            None => {
                let foods: Vec<_> = current.0.iter().map(|(k, v)| (*k, v.style)).collect();
                if !foods.is_empty() {
                    packets.extend(encode::food_sector(sector, &foods));
                }
                current.clone()
            }
            Some(previous) => {
                let unchanged = Arc::ptr_eq(&previous, current);
                if !unchanged || changes.eaten.contains(&sector) {
                    changes.append_diff(packets, sector, &previous, current, eaters);
                }
                // A membership edit does not replace unchanged listings. Move
                // the held handle instead of dropping and cloning that Arc.
                if unchanged { previous } else { current.clone() }
            }
        };
        known.entries.push((sector, held));
    }
    known.subscription_revision = Some(interest.revision().clone());
    known.food_revision = Some(changes.current.clone());
}

fn assert_known(actual: &KnownSectors, expected: &KnownSectors) {
    assert_eq!(actual.entries.len(), expected.entries.len());
    for ((a_key, a), (e_key, e)) in actual.entries.iter().zip(&expected.entries) {
        assert_eq!(a_key, e_key);
        assert_eq!(a, e);
        assert!(Arc::ptr_eq(a, e));
    }
    for (a, e) in [
        (
            &actual.subscription_revision,
            &expected.subscription_revision,
        ),
        (&actual.food_revision, &expected.food_revision),
    ] {
        match (a, e) {
            (Some(a), Some(e)) => assert!(Arc::ptr_eq(a, e)),
            (None, None) => {}
            _ => panic!("known revision differs"),
        }
    }
}

#[test]
fn membership_refresh_reuses_storage_through_retirement_insertion_and_empty_views() {
    let keys: Vec<_> = (0..12).map(|n| [108, n]).collect();
    let listing = Arc::new(SectorFood::default());
    let mut sectors = vec![None; 1 << 16];
    for &key in &keys {
        sectors[usize::from(u16::from_be_bytes(key))] = Some(listing.clone());
    }
    let mut known = KnownSectors::default();
    known.entries.reserve(32);
    let pointer = known.entries.as_ptr();
    let capacity = known.entries.capacity();
    for selection in [
        vec![1, 3, 5, 7, 9],
        vec![0, 3, 4, 7, 10],
        vec![],
        vec![2, 3, 6, 9],
    ] {
        let interest =
            SectorInterest::from_subscribed(selection.into_iter().map(|n| keys[n]).collect());
        food_updates(
            &mut Vec::new(),
            &mut known,
            &interest,
            &sectors,
            &FoodChanges::default(),
            &BTreeMap::new(),
        );
        assert_eq!(known.entries.as_ptr(), pointer);
        assert_eq!(known.entries.capacity(), capacity);
        assert_eq!(
            known
                .entries
                .iter()
                .map(|(key, _)| *key)
                .collect::<BTreeSet<_>>(),
            *interest.subscribed()
        );
    }
}

#[test]
fn membership_refresh_matches_frozen_bytes_handles_and_revisions() {
    let keys: Vec<_> = (0..8).map(|n| [108, 108 + n]).collect();
    for seed in [1_u64, 7, 42, 0xfeed_beef] {
        let mut random = seed;
        let mut sectors = vec![None; 1 << 16];
        for &key in &keys {
            sectors[usize::from(u16::from_be_bytes(key))] = Some(Arc::new(SectorFood::default()));
        }
        let mut actual = vec![KnownSectors::default(); 3];
        let mut expected = actual.clone();
        let mut interests = vec![SectorInterest::default(); 3];
        let mut previous = Arc::new(());
        for step in 0..600 {
            let mut changed = BTreeSet::new();
            let mut eaters = BTreeMap::new();
            for (sector_index, &key) in keys.iter().enumerate() {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                let index = usize::from(u16::from_be_bytes(key));
                if step % 3 == sector_index % 3 {
                    let decorative = random & 8 != 0;
                    let run = (0..3)
                        .filter_map(|n| {
                            if random & (1 << n) == 0 {
                                return None;
                            }
                            let offset = [64 + n * 64, 64 + n * 64];
                            let point = Point {
                                x: f64::from(key[0]) * 300. - f64::from(encode::GRID_CENTER)
                                    + f64::from(offset[0]) * 300. / 256.
                                    + if decorative { 0.125 } else { 0. },
                                y: f64::from(key[1]) * 300. - f64::from(encode::GRID_CENTER)
                                    + f64::from(offset[1]) * 300. / 256.,
                            };
                            Some((
                                offset,
                                KnownFood {
                                    point,
                                    decorative,
                                    style: encode::FoodStyle {
                                        color: (random >> 8) as u8 % 9,
                                        size: 16 + (random >> 16) as u8 % 16,
                                        snake_food: random & 16 != 0,
                                    },
                                },
                            ))
                        })
                        .collect();
                    sectors[index] = Some(Arc::new(SectorFood(run)));
                    changed.insert(key);
                }
                if step % 5 == sector_index % 5
                    && let Some((_, food)) = sectors[index].as_ref().unwrap().0.first()
                {
                    eaters.insert(food_identity(food.point), (sector_index + 1) as u16);
                }
            }
            let eaten = eaten_sectors(&eaters);
            changed.extend(&eaten);
            let current = Arc::new(());
            for recipient in 0..3 {
                if step % 43 == recipient {
                    actual[recipient] = KnownSectors::default();
                    expected[recipient] = KnownSectors::default();
                }
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                if step % 4 != 0 {
                    let mask = (random >> 24) as u8;
                    interests[recipient] = SectorInterest::from_subscribed(
                        keys.iter()
                            .enumerate()
                            .filter_map(|(n, &key)| (mask & (1 << n) != 0).then_some(key))
                            .collect(),
                    );
                }
                // Skipped recipients retain older listing/revision identities.
                if step % 7 == recipient {
                    continue;
                }
                let changes = || FoodChanges {
                    changed: changed.iter().copied().collect(),
                    eaten: eaten.clone(),
                    previous: previous.clone(),
                    current: current.clone(),
                    ..Default::default()
                };
                let mut got = vec![vec![0xaa]];
                let mut want = got.clone();
                food_updates(
                    &mut got,
                    &mut actual[recipient],
                    &interests[recipient],
                    &sectors,
                    &changes(),
                    &eaters,
                );
                frozen_food_updates(
                    &mut want,
                    &mut expected[recipient],
                    &interests[recipient],
                    &sectors,
                    &changes(),
                    &eaters,
                );
                assert_eq!(got, want, "seed={seed} step={step} recipient={recipient}");
                assert_known(&actual[recipient], &expected[recipient]);
            }
            previous = current;
        }
    }
}
