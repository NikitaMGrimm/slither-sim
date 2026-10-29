//! Provisional small-snake food interest from the retained sector study.
//!
//! The 2.6-second threshold was a temporal classifier split, not an exact
//! official expiry timer. Combining it with nominal 504 ms cleanup is an
//! approximation; neither the official scheduler nor size scaling is identified.

use super::encode;
use crate::model::Point;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const HEADING_LEAD: f64 = 100.;
const RETENTION_SECONDS: f64 = 2.6;
pub(super) const CLEANUP_SECONDS: f64 = 0.504;

/// Recipient state, reset with its connection rather than shared with the world.
#[derive(Clone, Debug, Default)]
pub(super) struct SectorInterest {
    subscribed: BTreeSet<[u8; 2]>,
    // Held food listings share this identity, avoiding a key-by-key comparison
    // on every broadcast. Replace it only when membership actually changes.
    revision: Arc<()>,
    /// Keys are exactly the subscribed sectors. Sectors of the current stencil
    /// were last seen at `touched`; their stored values may be older.
    last_seen: BTreeMap<[u8; 2], f64>,
    /// Minimum stored time outside the current stencil, with a sector witness.
    /// Current sectors use `touched`, so their older stored times do not count.
    oldest: Option<([u8; 2], f64)>,
    cleanup_epoch: u64,
    /// Grid center of the latest stencil.
    center: Option<[f64; 2]>,
    touched: f64,
}

impl SectorInterest {
    /// Advance on every physics tick using finite, monotonic virtual time.
    /// The caller keeps the final pose for dead clients and resets on reconnect.
    pub(super) fn advance(&mut self, head: Point, heading: f64, now: f64) {
        let mut changed = false;
        let size = f64::from(encode::SECTOR_SIZE);
        let center = f64::from(encode::GRID_CENTER);
        // The wire origin is half a sector from the centered world origin, so
        // add it before flooring, including for negative world coordinates.
        let cx = ((head.x + center + HEADING_LEAD * heading.cos()) / size).floor();
        let cy = ((head.y + center + HEADING_LEAD * heading.sin()) / size).floor();
        let current = |sector: [u8; 2]| {
            let dx = (f64::from(sector[0]) - cx).abs();
            let dy = (f64::from(sector[1]) - cy).abs();
            dx <= 3. && dy <= 3. && !(dx == 3. && dy == 3.)
        };
        // An unchanged stencil touches the same sectors, so only `touched` moves.
        if self.center != Some([cx, cy]) {
            let refresh_oldest = self.oldest.is_some_and(|(sector, _)| current(sector));
            if let Some(previous) = self.center {
                for sector in stencil(previous) {
                    self.last_seen.insert(sector, self.touched);
                    // Existing non-current times cannot exceed the previous
                    // touch, so only an empty minimum needs a new witness.
                    if self.oldest.is_none() && !current(sector) {
                        note_oldest(&mut self.oldest, sector, self.touched);
                    }
                }
            }
            for sector in stencil([cx, cy]) {
                changed |= self.subscribed.insert(sector);
                self.last_seen.insert(sector, now);
            }
            self.center = Some([cx, cy]);
            // Only a re-entering minimum can invalidate the witness. The old
            // stencil above becomes non-current at its previous touch time.
            if refresh_oldest {
                self.oldest = None;
                for (&sector, &seen) in &self.last_seen {
                    if !current(sector) {
                        note_oldest(&mut self.oldest, sector, seen);
                    }
                }
            }
        }
        self.touched = now;

        // Absolute epochs keep phase at multiples of 0.504 from virtual zero.
        // Touch before cleanup; a large jump needs only one bounded sweep and
        // never advances the clock by repeatedly adding a floating interval.
        let epoch = (now / CLEANUP_SECONDS).floor() as u64;
        if epoch > self.cleanup_epoch {
            self.cleanup_epoch = epoch;
            // Subtraction is monotone in the stored time for fixed finite now.
            // Keep the original subtraction: adding a future deadline can round
            // differently at the strict expiry boundary.
            if self
                .oldest
                .is_some_and(|(_, oldest)| now - oldest > RETENTION_SECONDS)
            {
                let subscribed = &mut self.subscribed;
                let oldest = &mut self.oldest;
                *oldest = None;
                self.last_seen.retain(|sector, last_seen| {
                    if current(*sector) {
                        true
                    } else if now - *last_seen <= RETENTION_SECONDS {
                        note_oldest(oldest, *sector, *last_seen);
                        true
                    } else {
                        changed |= subscribed.remove(sector);
                        false
                    }
                });
            }
        }
        if changed {
            self.revision = Arc::new(());
        }
    }

    pub(super) fn subscribed(&self) -> &BTreeSet<[u8; 2]> {
        &self.subscribed
    }

    pub(super) fn revision(&self) -> &Arc<()> {
        &self.revision
    }

    /// Synthetic retained membership for wire-diff regressions.
    #[cfg(test)]
    pub(super) fn from_subscribed(subscribed: BTreeSet<[u8; 2]>) -> Self {
        let last_seen = subscribed.iter().map(|&sector| (sector, 0.)).collect();
        let oldest = subscribed.first().map(|&sector| (sector, 0.));
        Self {
            subscribed,
            last_seen,
            oldest,
            ..Self::default()
        }
    }
}

fn note_oldest(oldest: &mut Option<([u8; 2], f64)>, sector: [u8; 2], seen: f64) {
    if oldest.is_none_or(|(_, previous)| seen < previous) {
        *oldest = Some((sector, seen));
    }
}

/// Sectors around a grid center, without the four corners.
fn stencil([cx, cy]: [f64; 2]) -> impl Iterator<Item = [u8; 2]> {
    (-3_i32..=3).flat_map(move |dx| {
        (-3_i32..=3).filter_map(move |dy| {
            if dx.abs() == 3 && dy.abs() == 3 {
                return None;
            }
            let sx = cx + f64::from(dx);
            let sy = cy + f64::from(dy);
            // Clip the stencil, rather than wrapping or clamping offsets
            // onto an edge. These keys bound both collections to 65,536.
            ((0. ..=255.).contains(&sx) && (0. ..=255.).contains(&sy))
                .then_some([sx as u8, sy as u8])
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    const FAR: Point = Point { x: 10_000., y: 0. };

    fn assert_full_state(actual: &SectorInterest, reference: &SectorInterest) {
        assert_eq!(actual.subscribed, reference.subscribed);
        assert_eq!(actual.last_seen, reference.last_seen);
        assert_eq!(actual.center, reference.center);
        assert_eq!(actual.touched.to_bits(), reference.touched.to_bits());
        assert_eq!(actual.cleanup_epoch, reference.cleanup_epoch);
        let expected = reference
            .last_seen
            .iter()
            .filter(|(sector, _)| {
                reference.center.is_none_or(|[cx, cy]| {
                    let dx = (f64::from(sector[0]) - cx).abs();
                    let dy = (f64::from(sector[1]) - cy).abs();
                    !(dx <= 3. && dy <= 3. && !(dx == 3. && dy == 3.))
                })
            })
            .map(|(_, &seen)| seen)
            .min_by(f64::total_cmp);
        assert_eq!(
            actual.oldest.map(|(_, t)| t.to_bits()),
            expected.map(f64::to_bits)
        );
        if let Some((sector, seen)) = actual.oldest {
            assert_eq!(actual.last_seen[&sector].to_bits(), seen.to_bits());
        }
    }

    fn compare_advance(
        actual: &mut SectorInterest,
        reference: &mut SectorInterest,
        head: Point,
        heading: f64,
        now: f64,
    ) {
        let old = actual.revision().clone();
        let reference_old = reference.revision().clone();
        actual.advance(head, heading, now);
        full_sweep(reference, head, heading, now);
        assert_full_state(actual, reference);
        assert_eq!(
            Arc::ptr_eq(&old, actual.revision()),
            Arc::ptr_eq(&reference_old, reference.revision())
        );
    }

    #[test]
    fn oldest_matches_full_sweep_through_reentries_resets_and_rounding() {
        let mut actual = SectorInterest::default();
        let mut reference = actual.clone();
        for (head, now) in [
            (Point::default(), 0.),
            (FAR, 0.007),
            (Point::default(), 1.),
            (FAR, 2.52),
            (FAR, 3.024),
            (Point::default(), 3.025),
            (FAR, 1e12),
        ] {
            compare_advance(&mut actual, &mut reference, head, 0., now);
        }
        actual = SectorInterest::default();
        reference = SectorInterest::default();
        // An added deadline rounds up to now, despite strictly expired age.
        let seen = 0.119_f64;
        let now = seen + RETENTION_SECONDS;
        assert!(now - seen > RETENTION_SECONDS);
        assert!(now <= seen + RETENTION_SECONDS);
        compare_advance(&mut actual, &mut reference, Point::default(), 0., seen);
        compare_advance(&mut actual, &mut reference, FAR, 0., now);
        assert_eq!(actual.subscribed.len(), 45);
    }

    #[test]
    fn random_walk_skip_matches_full_sweep_and_exact_oldest() {
        for seed in [1_u64, 7, 42, 0xfeed_beef] {
            let mut random = seed;
            let mut actual = SectorInterest::default();
            let mut reference = actual.clone();
            let mut head = Point::default();
            let mut now = 0.;
            for step in 0..12_000 {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                if step % 367 == 0 {
                    actual = SectorInterest::default();
                    reference = SectorInterest::default();
                }
                if step % 257 == 0 {
                    head = at_wire_sector((random >> 16) as u8 as u16, (random >> 24) as u8 as u16);
                } else if step % 11 != 0 {
                    head.x += ((random >> 32) as i8) as f64;
                    head.y += ((random >> 40) as i8) as f64;
                }
                now += if step % 997 == 0 {
                    1e12
                } else if step % 113 == 0 {
                    3.024
                } else {
                    0.007
                };
                let heading = (random >> 48) as f64 / 65536. * std::f64::consts::TAU;
                compare_advance(&mut actual, &mut reference, head, heading, now);
                if step % 509 == 0 {
                    actual = actual.clone();
                    reference = reference.clone();
                }
            }
        }
    }

    // Frozen full-sweep owner from 23e987f3, independent of the minimum hint.
    fn full_sweep(interest: &mut SectorInterest, head: Point, heading: f64, now: f64) {
        let mut changed = false;
        let size = f64::from(encode::SECTOR_SIZE);
        let center = f64::from(encode::GRID_CENTER);
        // The wire origin is half a sector from the centered world origin, so
        // add it before flooring, including for negative world coordinates.
        let cx = ((head.x + center + HEADING_LEAD * heading.cos()) / size).floor();
        let cy = ((head.y + center + HEADING_LEAD * heading.sin()) / size).floor();
        // An unchanged stencil touches the same sectors, so only `touched` moves.
        if interest.center != Some([cx, cy]) {
            if let Some(previous) = interest.center {
                for sector in stencil(previous) {
                    interest.last_seen.insert(sector, interest.touched);
                }
            }
            for sector in stencil([cx, cy]) {
                changed |= interest.subscribed.insert(sector);
                interest.last_seen.insert(sector, now);
            }
            interest.center = Some([cx, cy]);
        }
        interest.touched = now;

        // Absolute epochs keep phase at multiples of 0.504 from virtual zero.
        // Touch before cleanup; a large jump needs only one bounded sweep and
        // never advances the clock by repeatedly adding a floating interval.
        let epoch = (now / CLEANUP_SECONDS).floor() as u64;
        if epoch > interest.cleanup_epoch {
            interest.cleanup_epoch = epoch;
            let subscribed = &mut interest.subscribed;
            let [cx, cy] = [cx, cy];
            interest.last_seen.retain(|sector, last_seen| {
                let dx = (f64::from(sector[0]) - cx).abs();
                let dy = (f64::from(sector[1]) - cy).abs();
                let current = dx <= 3. && dy <= 3. && !(dx == 3. && dy == 3.);
                if current || now - *last_seen <= RETENTION_SECONDS {
                    true
                } else {
                    changed |= subscribed.remove(sector);
                    false
                }
            });
        }
        if changed {
            interest.revision = Arc::new(());
        }
    }

    #[test]
    fn revision_tracks_membership_across_cloning_revisits_and_cleanup() {
        let mut interest = SectorInterest::default();
        let empty = interest.revision().clone();
        interest.advance(Point::default(), 0., 0.);
        assert!(!Arc::ptr_eq(&empty, interest.revision()));
        let original = interest.clone();
        interest.advance(Point { x: 1., y: 1. }, 0., 0.504);
        assert!(Arc::ptr_eq(original.revision(), interest.revision()));

        interest.advance(FAR, 0., 0.505);
        let expanded = interest.revision().clone();
        assert!(!Arc::ptr_eq(original.revision(), &expanded));
        assert_ne!(original.subscribed(), interest.subscribed());
        // Returning to the original center retains the new sectors as well.
        interest.advance(Point::default(), 0., 0.506);
        assert!(Arc::ptr_eq(&expanded, interest.revision()));
        assert_ne!(original.subscribed(), interest.subscribed());

        interest.advance(Point::default(), 0., 3.528);
        assert!(!Arc::ptr_eq(&expanded, interest.revision()));
        assert_eq!(original.subscribed(), interest.subscribed());
        let cleaned = interest.revision().clone();
        interest.advance(Point::default(), 0., 4.032);
        assert!(Arc::ptr_eq(&cleaned, interest.revision()));
        assert!(!Arc::ptr_eq(&cleaned, SectorInterest::default().revision()));
    }

    #[test]
    fn fresh_received_sector_entries_are_admitted_beyond_the_old_circle() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/client-food-interest.json"
        ))
        .unwrap();
        for entry in fixture["entries"].as_array().unwrap() {
            let center = f64::from(encode::GRID_CENTER);
            let head = Point {
                x: entry["head"][0].as_f64().unwrap() - center,
                y: entry["head"][1].as_f64().unwrap() - center,
            };
            let sector = [
                entry["sector"][0].as_u64().unwrap() as u8,
                entry["sector"][1].as_u64().unwrap() as u8,
            ];
            let mut interest = SectorInterest::default();
            interest.advance(head, entry["ang"].as_f64().unwrap(), 0.);
            assert!(interest.subscribed().contains(&sector), "{entry}");
            let nearest = |axis: usize| {
                let p = entry["head"][axis].as_f64().unwrap();
                let lower = f64::from(sector[axis]) * 300.;
                (lower - p).max(p - lower - 300.).max(0.)
            };
            assert!(nearest(0).hypot(nearest(1)) > 500.);
        }
    }

    #[test]
    fn initial_stencil_has_45_sectors_without_four_corners() {
        let mut interest = SectorInterest::default();
        interest.advance(Point::default(), 0., 0.);
        let sectors = interest.subscribed();
        assert_eq!(sectors.len(), 45);
        assert!(sectors.contains(&[108, 108]));
        assert!(sectors.contains(&[105, 106]));
        assert!(sectors.contains(&[106, 105]));
        assert!(sectors.contains(&[110, 111]));
        assert!(sectors.contains(&[111, 110]));
        for corner in [[105, 105], [105, 111], [111, 105], [111, 111]] {
            assert!(!sectors.contains(&corner));
        }
        assert!(
            sectors
                .iter()
                .all(|&[x, y]| (105..=111).contains(&x) && (105..=111).contains(&y))
        );
    }

    #[test]
    fn heading_lead_changes_the_grid_center() {
        let head = Point { x: 60., y: 60. };
        let mut east = SectorInterest::default();
        let mut north = SectorInterest::default();
        east.advance(head, 0., 0.);
        north.advance(head, FRAC_PI_2, 0.);
        assert!(east.subscribed().contains(&[112, 108]));
        assert!(!north.subscribed().contains(&[112, 108]));
        assert!(north.subscribed().contains(&[108, 112]));
        assert!(!east.subscribed().contains(&[108, 112]));
    }

    #[test]
    fn negative_world_positions_floor_after_the_wire_offset_and_lead() {
        let mut interest = SectorInterest::default();
        interest.advance(Point { x: -249., y: -149. }, 0., 0.);
        assert!(interest.subscribed().contains(&[111, 108]));
        assert!(interest.subscribed().contains(&[108, 111]));
        assert!(!interest.subscribed().contains(&[104, 108]));
        assert!(!interest.subscribed().contains(&[108, 104]));

        let mut below_boundary = SectorInterest::default();
        below_boundary.advance(
            Point {
                x: -250.01,
                y: -150.01,
            },
            0.,
            0.,
        );
        assert!(below_boundary.subscribed().contains(&[104, 107]));
        assert!(below_boundary.subscribed().contains(&[107, 104]));
        assert!(!below_boundary.subscribed().contains(&[111, 108]));
        assert!(!below_boundary.subscribed().contains(&[108, 111]));
    }

    #[test]
    fn expired_sectors_wait_for_the_fixed_cleanup_phase() {
        let mut interest = SectorInterest::default();
        interest.advance(Point::default(), 0., 0.);
        interest.advance(FAR, 0., 0.007);
        interest.advance(FAR, 0., 2.52);
        interest.advance(FAR, 0., 3.023);
        assert!(interest.subscribed().contains(&[108, 108]));
        interest.advance(FAR, 0., 3.024);
        assert!(!interest.subscribed().contains(&[108, 108]));
        assert_eq!(interest.subscribed().len(), 45);
    }

    #[test]
    fn revisiting_refreshes_retention_before_cleanup() {
        let mut interest = SectorInterest::default();
        interest.advance(Point::default(), 0., 0.);
        interest.advance(FAR, 0., 0.007);
        interest.advance(Point::default(), 0., 2.4);
        interest.advance(FAR, 0., 2.5);
        interest.advance(FAR, 0., 3.024);
        assert!(interest.subscribed().contains(&[108, 108]));
        interest.advance(FAR, 0., 5.04);
        assert!(!interest.subscribed().contains(&[108, 108]));
    }

    #[test]
    fn a_final_stationary_pose_is_touched_before_cleanup() {
        let mut interest = SectorInterest::default();
        interest.advance(Point::default(), 0., 0.);
        let original = interest.subscribed().clone();
        interest.advance(Point::default(), 0., 1_000_000_000_000.);
        assert_eq!(interest.subscribed(), &original);
        assert_eq!(interest.last_seen.len(), 45);
    }

    #[test]
    fn default_reset_discards_the_old_connections_history_and_clock() {
        let mut interest = SectorInterest::default();
        interest.advance(Point::default(), 0., 2.);
        let clone = interest.clone();
        assert_eq!(clone.subscribed(), interest.subscribed());
        interest = SectorInterest::default();
        assert!(interest.subscribed().is_empty());
        assert!(interest.last_seen.is_empty());
        assert_eq!(interest.cleanup_epoch, 0);
        interest.advance(FAR, 0., 2.1);
        assert!(!interest.subscribed().contains(&[108, 108]));
        assert_eq!(interest.subscribed().len(), 45);
    }

    fn at_wire_sector(x: u16, y: u16) -> Point {
        Point {
            x: f64::from(x) * 300. - 32550. - 100.,
            y: f64::from(y) * 300. - 32550.,
        }
    }

    #[test]
    fn stencil_clips_at_both_wire_edges_without_wrapping() {
        let mut low = SectorInterest::default();
        low.advance(at_wire_sector(0, 0), 0., 0.);
        assert_eq!(low.subscribed().len(), 15);
        assert!(low.subscribed().contains(&[0, 0]));
        assert!(!low.subscribed().contains(&[3, 3]));
        assert!(low.subscribed().iter().all(|&[x, y]| x <= 3 && y <= 3));

        let mut high = SectorInterest::default();
        high.advance(at_wire_sector(255, 255), 0., 0.);
        assert_eq!(high.subscribed().len(), 15);
        assert!(high.subscribed().contains(&[255, 255]));
        assert!(!high.subscribed().contains(&[252, 252]));
        assert!(high.subscribed().iter().all(|&[x, y]| x >= 252 && y >= 252));

        let mut outside = SectorInterest::default();
        outside.advance(
            Point {
                x: -34_000.,
                y: -34_000.,
            },
            0.,
            0.,
        );
        assert!(outside.subscribed().is_empty());
    }

    #[test]
    fn all_legal_ids_bound_state_and_a_large_jump_cleans_in_one_sweep() {
        let mut interest = SectorInterest::default();
        // Centers five cells apart cover every legal ID without cleanup.
        for x in (0..=255).step_by(5) {
            for y in (0..=255).step_by(5) {
                interest.advance(at_wire_sector(x, y), 0., 0.);
            }
        }
        assert_eq!(interest.subscribed().len(), 65_536);
        assert_eq!(interest.last_seen.len(), 65_536);
        interest.advance(Point::default(), 0., 1_000_000_000_000.);
        assert_eq!(interest.subscribed().len(), 45);
        assert_eq!(interest.last_seen.len(), 45);
    }
}
