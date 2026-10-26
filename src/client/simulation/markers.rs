//! Pit decoration on the stock food grid, never authoritative food.
use super::food::KnownFood;
use crate::{client::encode, model::Point};
use std::{collections::BTreeMap, f64::consts::TAU};

#[derive(Default)]
pub(super) struct PitMarkers(BTreeMap<u16, Vec<([u8; 2], KnownFood)>>, f64);

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
        Ok(Self(sectors, pit_radius))
    }

    pub(super) fn radius(&self) -> f64 {
        self.1
    }

    pub(super) fn overlay(
        &self,
        sector: u16,
        food: &[([u8; 2], KnownFood)],
    ) -> Option<Vec<([u8; 2], KnownFood)>> {
        let markers = self.0.get(&sector)?;
        // A stock cell holds one item. Reserve marker keys in browser listings;
        // keep colliding physical pellets in physics and the separate bot view.
        let mut run = markers.clone();
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
    fn pit_markers_are_unique_outside_the_edge_and_use_the_design_style() {
        for radius in [0.01, 1784., 31999.] {
            let markers = PitMarkers::new(radius).unwrap();
            let count: usize = markers.0.values().map(Vec::len).sum();
            if radius == 1784. {
                assert_eq!(count, 749);
            }
            assert!(count > 0);
            let coordinate = |sector, offset| {
                f64::from(sector) * 300. + f64::from(offset) * (300. / 256.) - 32550.
            };
            let mut points = Vec::new();
            for (&sector, entries) in &markers.0 {
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
