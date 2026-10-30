// Frozen drain/re-add owner from 47811bc8, including transient transitions.
use super::*;

fn frozen_sync<'a>(
    cache: &mut FoodSubscriptions,
    interests: impl ExactSizeIterator<Item = Option<&'a SectorInterest>>,
) -> BTreeSet<u16> {
    cache.entered.clear();
    let len = interests.len();
    cache
        .held
        .resize_with(cache.held.len().max(len), HeldSubscription::default);
    cache.counts.resize(1 << 16, 0);
    let mut retired = BTreeSet::new();
    for (held, interest) in cache
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
            cache.counts[usize::from(sector)] -= 1;
            if cache.counts[usize::from(sector)] == 0 {
                cache.sectors.remove(&sector);
                retired.insert(sector);
            }
        }
        // Retain the identity strongly: a closed/replaced connection cannot
        // have its old address reused while this snapshot still names it.
        held.revision = revision.cloned();
        if let Some(interest) = interest {
            for &sector in interest.subscribed() {
                let sector = u16::from_be_bytes(sector);
                let count = &mut cache.counts[usize::from(sector)];
                if *count == 0 {
                    cache.sectors.insert(sector);
                    cache.entered.insert(sector);
                }
                *count += 1;
                held.sectors.push(sector);
            }
        }
    }
    cache.held.truncate(len);
    // A sector may change owners in this same broadcast. Keep its listing
    // unless the final union really lost it, regardless of update order.
    retired.retain(|&sector| cache.counts[usize::from(sector)] == 0);
    retired
}

fn assert_union_state(actual: &FoodSubscriptions, reference: &FoodSubscriptions) {
    assert_eq!(actual.counts, reference.counts);
    assert_eq!(actual.sectors, reference.sectors);
    assert_eq!(actual.entered, reference.entered);
    assert_eq!(actual.held.len(), reference.held.len());
    for (a, b) in actual.held.iter().zip(&reference.held) {
        assert_eq!(a.sectors, b.sectors);
        match (&a.revision, &b.revision) {
            (Some(a), Some(b)) => assert!(Arc::ptr_eq(a, b)),
            (None, None) => {}
            _ => panic!("held revision differs"),
        }
    }
}

#[test]
fn union_preserves_transient_sole_owner_reentry() {
    let first = SectorInterest::from_subscribed(BTreeSet::from([[1, 2], [2, 3]]));
    let second = SectorInterest::from_subscribed(first.subscribed().clone());
    let mut cache = FoodSubscriptions::default();
    cache.sync([Some(&first)].into_iter());
    assert!(cache.sync([Some(&first)].into_iter()).is_empty());
    assert!(cache.entered.is_empty());
    assert!(cache.sync([Some(&second)].into_iter()).is_empty());
    assert_eq!(
        cache.entered,
        BTreeSet::from([u16::from_be_bytes([1, 2]), u16::from_be_bytes([2, 3])])
    );
    let old_identity = cache.held[0].revision.clone().unwrap();
    assert!(Arc::ptr_eq(&old_identity, second.revision()));
    cache.sync([None].into_iter());
    assert!(cache.held[0].revision.is_none());
    assert_eq!(Arc::strong_count(&old_identity), 2);
}

#[test]
fn union_matches_drain_readd_through_random_handoffs_replacements_and_truncation() {
    let keys = [
        [0, 0],
        [0, 255],
        [1, 2],
        [2, 3],
        [107, 108],
        [108, 108],
        [254, 255],
        [255, 255],
    ];
    for seed in [1_u64, 7, 42, 0xfeed_beef] {
        let mut random = seed;
        let mut actual = FoodSubscriptions::default();
        let mut reference = FoodSubscriptions::default();
        let mut recipients: Vec<Option<SectorInterest>> = Vec::new();
        for step in 0..600 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            if step % 13 == 0 {
                recipients.resize_with(((random >> 32) % 13) as usize, || None);
            }
            for (slot, recipient) in recipients.iter_mut().enumerate() {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                match (random >> 48) % 7 {
                    0 => *recipient = None,
                    1 => {}
                    2 => {
                        if let Some(old) = recipient {
                            *old = SectorInterest::from_subscribed(old.subscribed().clone());
                        }
                    }
                    _ => {
                        let mask = (random >> (slot % 16)) as u8;
                        *recipient = Some(SectorInterest::from_subscribed(
                            keys.iter()
                                .enumerate()
                                .filter_map(|(i, &key)| (mask & (1 << i) != 0).then_some(key))
                                .collect(),
                        ));
                    }
                }
            }
            let got = actual.sync(recipients.iter().map(Option::as_ref));
            let want = frozen_sync(&mut reference, recipients.iter().map(Option::as_ref));
            assert_eq!(got, want, "seed={seed} step={step}");
            assert_union_state(&actual, &reference);
        }
    }
}
