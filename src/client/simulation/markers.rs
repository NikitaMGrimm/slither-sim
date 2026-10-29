//! Pit decoration on the stock food grid, never authoritative food.
use super::food::KnownFood;
use crate::{client::encode, model::Point};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::TAU,
};

type Sectors = BTreeMap<u16, Vec<([u8; 2], KnownFood)>>;

// The stock client advances rapid food by 3 * elapsed_ms / (8 * 150),
// reaching full size after 400 ms without lag. Hold both generations for 500 ms.
// See research/battledome/2026-10-26-stock-presentation/REPORT.md for source pin.
const FADE_SECONDS: f64 = 0.5;

#[derive(Default)]
#[cfg_attr(test, derive(Clone))]
pub(super) struct PitMarkers {
    sectors: Sectors,
    radius: f64,
    updated_at: f64,
    previous: Option<(Sectors, f64)>,
    affected: BTreeSet<u16>,
}

impl PitMarkers {
    pub(super) fn new(radius: f64) -> Result<Self, String> {
        let pit_radius = radius;
        // Local design choices, not measurements: roughly 15 units between
        // centers, 3 units outside the lethal edge, palette 8, size byte 31
        // (6.2 decoded). The offset exceeds food-grid rounding error (~0.83).
        // See docs/configuration.md#battledome and the pit_markers regressions.
        let radius = radius + 3.;
        let count = (TAU * radius / 15.).ceil() as usize;
        let mut keys = BTreeMap::new();
        for i in 0..count {
            let angle = TAU * i as f64 / count as f64;
            let point = Point {
                x: radius * angle.cos(),
                y: radius * angle.sin(),
            };
            keys.insert(encode::food_key(point)?, point);
        }
        let mut sectors = BTreeMap::<_, Vec<_>>::new();
        for ([sx, sy, rx, ry], point) in keys {
            sectors
                .entry(u16::from_be_bytes([sx, sy]))
                .or_default()
                .push((
                    [rx, ry],
                    KnownFood {
                        point,
                        style: encode::FoodStyle {
                            color: 8,
                            size: 31,
                            snake_food: false,
                        },
                        decorative: true,
                    },
                ));
        }
        Ok(Self {
            sectors,
            radius: pit_radius,
            updated_at: 0.,
            previous: None,
            affected: BTreeSet::new(),
        })
    }

    pub(super) fn radius(&self) -> f64 {
        self.radius
    }

    /// Keep a mature generation visible while a shrinking replacement fades in.
    /// Growth replaces immediately: old points may be behind the lethal edge.
    pub(super) fn update(&mut self, radius: f64, now: f64) -> Result<bool, String> {
        let cell = f64::from(encode::SECTOR_SIZE) / 256.;
        let previous_radius = self.radius();
        let growth = radius > previous_radius;
        let shrink = radius < previous_radius
            && now - self.updated_at >= FADE_SECONDS
            && (radius / cell).floor() != (previous_radius / cell).floor();
        let mut changed = false;
        let expired = self
            .previous
            .as_ref()
            .is_some_and(|(_, expires)| now >= *expires);
        self.affected.clear();
        if expired || growth || shrink {
            // Include disappearing generations as well as the replacement:
            // expiry or growth can remove a sector absent from the new ring.
            self.affected.extend(self.sectors.keys().copied());
            if let Some((previous, _)) = &self.previous {
                self.affected.extend(previous.keys().copied());
            }
        }
        if expired {
            self.previous = None;
            changed = true;
        }
        if growth || shrink {
            let mut next = Self::new(radius)?;
            next.updated_at = now;
            next.affected = std::mem::take(&mut self.affected);
            if shrink {
                next.previous = Some((std::mem::take(&mut self.sectors), now + FADE_SECONDS));
            }
            *self = next;
            changed = true;
        }
        if changed {
            self.affected.extend(self.sectors.keys().copied());
            if let Some((previous, _)) = &self.previous {
                self.affected.extend(previous.keys().copied());
            }
        }
        Ok(changed)
    }

    pub(super) fn affects_sector(&self, sector: u16) -> bool {
        self.affected.contains(&sector)
    }

    pub(super) fn overlay(
        &self,
        sector: u16,
        food: &[([u8; 2], KnownFood)],
    ) -> Option<Vec<([u8; 2], KnownFood)>> {
        let current = self.sectors.get(&sector);
        let previous = self
            .previous
            .as_ref()
            .and_then(|(sectors, _)| sectors.get(&sector));
        if current.is_none() && previous.is_none() {
            return None;
        }
        // A stock cell holds one item. Reserve marker keys in browser listings;
        // keep colliding physical pellets in physics and the separate bot view.
        let mut run = current.cloned().unwrap_or_default();
        if let Some(previous) = previous {
            run.extend_from_slice(previous);
        }
        run.extend_from_slice(food);
        run.sort_by_key(|(key, _)| *key);
        run.dedup_by_key(|(key, _)| *key);
        Some(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affected_sectors_cover_growth_shrink_overlap_and_exact_expiry() {
        let mut markers = PitMarkers::new(1784.).unwrap();
        for (radius, now) in [
            (1784., 0.),
            (1500., 0.5),
            (1400., 0.8),
            (1200., 1.),
            (1784., 1.1),
            (1800., 1.11),
            (900., 1.7),
            (900., 2.2_f64.next_down()),
            (900., 2.2),
            (900., 2.3),
        ] {
            let previous = markers.clone();
            let changed = markers.update(radius, now).unwrap();
            let mut sectors = BTreeSet::from([0]);
            for generation in [&previous, &markers] {
                sectors.extend(generation.sectors.keys().copied());
                if let Some((previous, _)) = &generation.previous {
                    sectors.extend(previous.keys().copied());
                }
            }
            for sector in sectors {
                if previous.overlay(sector, &[]) != markers.overlay(sector, &[]) {
                    assert!(
                        changed && markers.affects_sector(sector),
                        "lost overlay hint at {now}: {sector}"
                    );
                }
                if !changed {
                    assert!(!markers.affects_sector(sector));
                }
            }
            assert!(!markers.affects_sector(0));
        }
    }

    #[test]
    fn pit_markers_are_unique_outside_the_edge_and_use_the_design_style() {
        for radius in [0.01, 1784., 31999.] {
            let markers = PitMarkers::new(radius).unwrap();
            let count: usize = markers.sectors.values().map(Vec::len).sum();
            if radius == 1784. {
                assert_eq!(count, 749);
            }
            assert!(count > 0);
            let coordinate = |sector, offset| {
                f64::from(sector) * 300. + f64::from(offset) * (300. / 256.) - 32550.
            };
            let mut points = Vec::new();
            for (&sector, entries) in &markers.sectors {
                let [sx, sy] = sector.to_be_bytes();
                for &([rx, ry], food) in entries {
                    assert_eq!(encode::food_key(food.point).unwrap(), [sx, sy, rx, ry]);
                    assert_eq!(
                        food.style,
                        encode::FoodStyle {
                            color: 8,
                            size: 31,
                            snake_food: false
                        }
                    );
                    let point = Point {
                        x: coordinate(sx, rx),
                        y: coordinate(sy, ry),
                    };
                    assert!(
                        (radius + 2.1..radius + 3.9).contains(&point.distance(Point::default()))
                    );
                    points.push(point);
                }
            }
            points.sort_by(|a, b| a.y.atan2(a.x).total_cmp(&b.y.atan2(b.x)));
            for i in 0..points.len() {
                assert!(points[i].distance(points[(i + 1) % points.len()]) < 16.7);
            }
        }
    }
}
