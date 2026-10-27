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
    subscribed: Arc<BTreeSet<[u8; 2]>>,
    // Held food listings share this identity, avoiding a key-by-key comparison
    // on every broadcast. Replace it only when membership actually changes.
    revision: Arc<()>,
    /// Keys are exactly the subscribed sectors. Sectors of the current stencil
    /// were last seen at `touched`; their stored values may be older.
    last_seen: BTreeMap<[u8; 2], f64>,
    cleanup_epoch: u64,
    /// Grid center of the latest stencil.
    center: Option<[f64; 2]>,
    touched: f64,
    /// Radius interval with exactly this arena sector set: inclusive below,
    /// exclusive above. A slow moving wall normally needs no membership work.
    arena_interval: Option<(f64, f64)>,
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
        // An unchanged stencil touches the same sectors, so only `touched` moves.
        if self.center != Some([cx, cy]) {
            if let Some(previous) = self.center {
                for sector in stencil(previous) {
                    self.last_seen.insert(sector, self.touched);
                }
            }
            for sector in stencil([cx, cy]) {
                changed |= Arc::make_mut(&mut self.subscribed).insert(sector);
                self.last_seen.insert(sector, now);
            }
            self.center = Some([cx, cy]);
        }
        self.touched = now;

        // Absolute epochs keep phase at multiples of 0.504 from virtual zero.
        // Touch before cleanup; a large jump needs only one bounded sweep and
        // never advances the clock by repeatedly adding a floating interval.
        let epoch = (now / CLEANUP_SECONDS).floor() as u64;
        if epoch > self.cleanup_epoch {
            self.cleanup_epoch = epoch;
            let subscribed = Arc::make_mut(&mut self.subscribed);
            let [cx, cy] = [cx, cy];
            self.last_seen.retain(|sector, last_seen| {
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
            self.revision = Arc::new(());
        }
    }

    /// Whole-arena food interest is a custom-mode choice. Share the set across
    /// recipients, and replace its identity only at an actual sector crossing.
    pub(super) fn advance_arena(&mut self, radius: f64) {
        if self
            .arena_interval
            .is_some_and(|(lo, hi)| radius >= lo && radius < hi)
        {
            return;
        }
        let size = f64::from(encode::SECTOR_SIZE);
        let center = f64::from(encode::GRID_CENTER);
        // food_key rounds before selecting the sector. Its cell can start half
        // a wire unit before the nominal sector edge; include that strip too.
        let half_cell = size / 512.;
        let lower = (((center - radius) / size).floor() as i32).saturating_sub(1);
        let upper = (((center + radius) / size).floor() as i32).saturating_add(1);
        let mut sectors = BTreeSet::new();
        let (mut lo, mut hi) = (0_f64, f64::INFINITY);
        for x in lower.max(0)..=upper.min(255) {
            for y in lower.max(0)..=upper.min(255) {
                let near = |cell: i32| {
                    // At wire coordinates below 1e5, 1e-8 exceeds the rounding
                    // in origin addition, cell selection and hypot. Their edges
                    // can otherwise disagree on a pellet just inside the wall.
                    let slack = 1e-8;
                    let a = f64::from(cell) * size - center - half_cell - slack;
                    let b = a + size + 2. * slack;
                    a.max(-b).max(0.)
                };
                let edge = near(x).hypot(near(y));
                if edge <= radius {
                    sectors.insert([x as u8, y as u8]);
                    lo = lo.max(edge);
                } else {
                    hi = hi.min(edge);
                }
            }
        }
        self.arena_interval = Some((lo, hi));
        if *self.subscribed != sectors {
            self.subscribed = Arc::new(sectors);
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
        Self {
            subscribed: Arc::new(subscribed),
            last_seen,
            ..Self::default()
        }
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

    #[test]
    fn arena_interest_covers_wire_rounding_just_inside_the_wall() {
        let boundary = 149.4140625_f64;
        let radius = boundary.next_down();
        for point in [
            Point {
                x: radius.next_down(),
                y: 0.,
            },
            Point {
                x: 0.,
                y: radius.next_down(),
            },
        ] {
            assert!(point.distance_lt(Point::default(), radius));
            let [x, y, _, _] = encode::food_key(point).unwrap();
            assert!(x == 109 || y == 109);
            let mut interest = SectorInterest::default();
            interest.advance_arena(radius);
            assert!(interest.subscribed().contains(&[x, y]));
        }
    }

    #[test]
    fn arena_interest_follows_the_live_disc_and_shares_unchanged_membership() {
        let mut interest = SectorInterest::default();
        interest.advance_arena(1000.);
        let initial = interest.clone();
        interest.advance_arena(1000.001);
        assert!(Arc::ptr_eq(initial.revision(), interest.revision()));
        assert!(Arc::ptr_eq(&initial.subscribed, &interest.subscribed));
        interest.advance_arena(3000.);
        assert!(initial.subscribed().is_subset(interest.subscribed()));
        assert!(interest.subscribed().len() > initial.subscribed().len());
        for x in -3000..=3000 {
            for y in [-2999., -150., 0., 150., 2999.] {
                let point = Point { x: f64::from(x), y };
                if point.distance(Point::default()) <= 3000. {
                    let [sx, sy, _, _] = encode::food_key(point).unwrap();
                    assert!(interest.subscribed().contains(&[sx, sy]));
                }
            }
        }
        // No retention delay on shrink; the world culls stranded food first.
        interest.advance_arena(1000.);
        assert_eq!(initial.subscribed(), interest.subscribed());
        assert!(!Arc::ptr_eq(initial.revision(), interest.revision()));
        // Extreme valid core radii cannot overflow sector-key arithmetic.
        interest.advance_arena(1e150);
        assert_eq!(interest.subscribed().len(), 65_536);
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
