//! Broadcast-local packets; retained immutable snapshots identify XOR histories.
use crate::client::{Minimap, encode};
use std::{collections::BTreeMap, sync::Arc};

pub(super) struct MinimapBroadcast {
    pub(super) map: Arc<Minimap>,
    // Keep wire-family packet caches separate from the shared raster. A future
    // protocol with a different snapshot command must have its own encoding.
    protocol19: Packets,
}

struct Packets {
    full: Vec<u8>,
    // Keep a strong owner beside each identity: replacing a recipient's base
    // cannot recycle that address while this broadcast still caches its delta.
    deltas: BTreeMap<usize, (Arc<Minimap>, Option<Vec<u8>>)>,
}

impl MinimapBroadcast {
    pub(super) fn new(map: Minimap) -> Self {
        Self {
            protocol19: Packets {
                full: encode::minimap(&map),
                deltas: BTreeMap::new(),
            },
            map: Arc::new(map),
        }
    }

    pub(super) fn packet19(&mut self, previous: Option<&Arc<Minimap>>) -> Result<Vec<u8>, String> {
        let packets = &mut self.protocol19;
        let Some(previous) = previous.filter(|previous| previous.side == self.map.side) else {
            return Ok(packets.full.clone());
        };
        let key = Arc::as_ptr(previous) as usize;
        let delta = match packets.deltas.entry(key) {
            std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::btree_map::Entry::Vacant(entry) => {
                // The checked encoder validates each immutable baseline/current
                // pair once, on its first eligible recipient. Skipped maps and
                // reconnects retain their own bases and cannot use another delta.
                let delta = encode::minimap_delta(previous, &self.map)?;
                // Preserve the original strict comparison (ties choose M).
                // Shortest encoding is local policy; see docs/server-rules.md.
                let shorter = (delta.len() < packets.full.len()).then_some(delta);
                entry.insert((previous.clone(), shorter))
            }
        };
        Ok(delta.1.as_ref().unwrap_or(&packets.full).clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(side: usize, bits: usize) -> Minimap {
        Minimap {
            side,
            cells: (0..side * side).map(|i| ((bits >> i) & 1) as u8).collect(),
            age_seconds: 0.,
        }
    }

    fn original(previous: Option<&Minimap>, map: &Minimap) -> Result<Vec<u8>, String> {
        let full = encode::minimap(map);
        if let Some(previous) = previous.filter(|previous| previous.side == map.side) {
            let delta = encode::minimap_delta(previous, map)?;
            if delta.len() < full.len() {
                return Ok(delta);
            }
        }
        Ok(full)
    }

    #[test]
    fn alternating_histories_match_checked_encoding() {
        // Every pair of 2x2 binary maps, with A/B/A ordering, new viewers,
        // identical pixels in a distinct snapshot, and a resized baseline.
        for bits in 0..16 {
            let current = map(2, bits);
            for old in 0..16 {
                let a = Arc::new(map(2, old));
                let b = Arc::new(map(2, old ^ 15));
                let same_pixels = Arc::new((*a).clone());
                let resized = Arc::new(map(1, 1));
                let mut cache = MinimapBroadcast::new(current.clone());
                for previous in [
                    None,
                    Some(&a),
                    Some(&b),
                    Some(&a),
                    Some(&same_pixels),
                    Some(&resized),
                ] {
                    assert_eq!(
                        cache.packet19(previous),
                        original(previous.map(Arc::as_ref), &current)
                    );
                }
            }
        }
    }

    #[test]
    fn equally_short_full_and_delta_packets_keep_the_full_snapshot() {
        let current = map(3, 0);
        let previous = Arc::new(map(3, (1 << 8) | 1));
        let full = encode::minimap(&current);
        assert_eq!(
            encode::minimap_delta(&previous, &current).unwrap().len(),
            full.len()
        );
        let mut cache = MinimapBroadcast::new(current);
        assert_eq!(cache.packet19(Some(&previous)).unwrap(), full);
        assert_eq!(cache.packet19(Some(&previous)).unwrap(), full);
    }

    #[test]
    fn invalid_baselines_still_fail_only_when_their_delta_is_requested() {
        let current = map(2, 1);
        let mut cache = MinimapBroadcast::new(current.clone());
        let mut invalid = map(2, 0);
        invalid.cells[0] = 2;
        let invalid = Arc::new(invalid);
        assert_eq!(cache.packet19(None), original(None, &current));
        assert_eq!(
            cache.packet19(Some(&invalid)),
            original(Some(&invalid), &current)
        );
        // A failed baseline does not poison later viewers or bypass validation.
        let valid = Arc::new(map(2, 0));
        assert_eq!(
            cache.packet19(Some(&valid)),
            original(Some(&valid), &current)
        );
        assert!(cache.packet19(Some(&invalid)).is_err());
        let different_size = Arc::new(Minimap {
            side: 1,
            cells: vec![2],
            age_seconds: f64::NAN,
        });
        assert_eq!(
            cache.packet19(Some(&different_size)),
            original(Some(&different_size), &current)
        );
    }
}
