//! World-owned food colors, separate from physics and its random stream.
//!
//! Ported from the retained local-client presentation helper and anonymous fixture.
//! Stock recipes describe received death-food prefixes, allowing rotations, rather
//! than renderer body colors. Their initial phase, fancy stock/custom boost
//! equivalence, and custom-white mapping remain unresolved; this model starts at
//! the retained recipe's first color and chooses white from 0..2 once per life.
//! Unknown skins explicitly fall back to skin % 9. Ambient weights are a retained
//! pooled presentation model, not an identified official RNG or universal mix.
//! See docs/presentation.md for evidence and limits; these choices do not establish
//! new empirical claims and do not define a prey palette.

use std::collections::{HashMap, VecDeque};
use std::hash::{BuildHasherDefault, Hasher};

use super::latency::Random;
use crate::model::{Food, Origin};
use crate::simulator::FoodSpawnEvent;

const AMBIENT_COLOR_WEIGHTS: [u32; 9] =
    [12536, 38648, 11965, 16363, 13075, 10999, 5350, 21929, 11243];

const STOCK_FOOD_PALETTES: [&[u8]; 66] = [
    &[0],
    &[1],
    &[2],
    &[3],
    &[4],
    &[5],
    &[6],
    &[7],
    &[8],
    &[1, 7],
    &[1, 7],
    &[5, 7],
    &[3, 7],
    &[1, 7],
    &[7],
    &[1, 2, 3, 4, 5, 6, 7, 8, 0],
    &[1, 4],
    &[1],
    &[7],
    &[5],
    &[3, 0],
    &[3, 4, 1],
    &[5, 3],
    &[4, 7, 1],
    &[4, 2],
    &[5, 1],
    &[4],
    &[3],
    &[4, 7, 3],
    &[4],
    &[1],
    &[1],
    &[1],
    &[4],
    &[3, 1, 4, 7],
    &[7, 6],
    &[1],
    &[5],
    &[4],
    &[2],
    &[3],
    &[7],
    &[3, 5, 7, 2],
    &[7, 1],
    &[1],
    &[7],
    &[7],
    &[4, 7, 1],
    &[7],
    &[2],
    &[1, 7, 4],
    &[7, 1],
    &[7, 4],
    &[1],
    &[7],
    &[4],
    &[5],
    &[0],
    &[3],
    &[4],
    &[7, 1],
    &[1, 3, 5, 7, 0],
    &[0, 0, 4, 0],
    &[7],
    &[1],
    &[2],
];

fn custom_food_color(color: u8) -> Option<u8> {
    Some(match color {
        0..=8 => color,
        10 | 11 | 14 | 15 | 16 | 19 | 20 | 29 | 30 | 37 | 41 => 1,
        12 | 18 | 24 | 32 => 4,
        13 | 25 | 26 | 35 => 3,
        17 | 21 | 39 => 0,
        22 | 33 => 5,
        23 => 2,
        27 | 28 | 31 => 7,
        34 => 8,
        _ => return None,
    })
}

/// Fast hashing of exact position bits. Map iteration order is never
/// observed, so the hasher affects only speed.
#[derive(Default)]
struct BitsHasher(u64);
impl Hasher for BitsHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.write_u64(u64::from(byte));
        }
    }
    fn write_u64(&mut self, value: u64) {
        let product = u128::from(self.0 ^ value) * 0x9e37_79b9_7f4a_7c15;
        self.0 = product as u64 ^ (product >> 64) as u64;
    }
    fn finish(&self) -> u64 {
        self.0
    }
}
type BitsMap<V> = HashMap<(u64, u64), V, BuildHasherDefault<BitsHasher>>;

fn identity(food: &Food) -> (u64, u64) {
    let bits = |value: f64| if value == 0. { 0 } else { value.to_bits() };
    (bits(food.x), bits(food.y))
}

struct Source {
    palette: Vec<u8>,
    cursor: usize,
}

pub(super) struct FoodPresentation {
    random: Random,
    sources: Vec<Source>,
    // Physics keeps insertion order and can admit several pellets at one point.
    // The wire exposes the first; later births must not recolor that lifetime.
    colors: BitsMap<VecDeque<u8>>,
    /// Reused pellet counts per position for `sync`.
    live: BitsMap<usize>,
}

impl FoodPresentation {
    pub(super) fn new(seed: u32, slots: usize) -> Self {
        Self {
            random: Random::new(u64::from(seed) ^ 0x0063_6f6c_6f72),
            sources: (0..slots)
                .map(|_| Source {
                    palette: vec![0],
                    cursor: 0,
                })
                .collect(),
            colors: BitsMap::default(),
            live: BitsMap::default(),
        }
    }

    pub(super) fn register(&mut self, slot: usize, skin: u8, custom: &[u8]) {
        let mut palette = Vec::new();
        let mut white = None;
        for run in custom.get(8..).unwrap_or_default().chunks_exact(2) {
            let color = if run[1] == 9 {
                *white.get_or_insert_with(|| (self.random.unit() * 3.) as u8)
            } else if let Some(color) = custom_food_color(run[1]) {
                color
            } else {
                continue;
            };
            palette.extend(std::iter::repeat_n(color, usize::from(run[0])));
        }
        if palette.is_empty() {
            palette = STOCK_FOOD_PALETTES
                .get(usize::from(skin))
                .map_or_else(|| vec![skin % 9], |colors| colors.to_vec());
        }
        self.sources[slot] = Source { palette, cursor: 0 };
    }

    pub(super) fn births(&mut self, events: &[FoodSpawnEvent]) -> Result<(), String> {
        for event in events {
            match event.source {
                Some(slot) if slot >= self.sources.len() => {
                    return Err("Food source has no registered appearance".into());
                }
                None if matches!(event.food.origin, Some(Origin::Boost | Origin::Death)) => {
                    return Err("Snake food is missing birth provenance".into());
                }
                _ => {}
            }
        }
        for event in events {
            let color = if let Some(slot) = event.source {
                let source = &mut self.sources[slot];
                let color = source.palette[source.cursor];
                source.cursor = (source.cursor + 1) % source.palette.len();
                color
            } else {
                Self::ambient_color(&mut self.random)
            };
            self.colors
                .entry(identity(&event.food))
                .or_default()
                .push_back(color);
        }
        Ok(())
    }

    pub(super) fn sync(&mut self, food: &[Food]) -> Result<(), String> {
        let mut live = std::mem::take(&mut self.live);
        live.clear();
        let mut short = false;
        for item in food {
            let count = live.entry(identity(item)).or_default();
            *count += 1;
            if self.colors.get(&identity(item)).map_or(0, VecDeque::len) < *count {
                if matches!(item.origin, Some(Origin::Boost | Origin::Death)) {
                    return Err("Snake food is missing birth provenance".into());
                }
                short = true;
            }
        }
        // Walk food order, not hash iteration order, to keep seeded assignment
        // reproducible. Reconciliation also supports injected ambient fixtures.
        for item in food.iter().filter(|_| short) {
            let key = identity(item);
            let colors = self.colors.entry(key).or_default();
            while colors.len() < live[&key] {
                colors.push_back(Self::ambient_color(&mut self.random));
            }
        }
        self.colors.retain(|key, colors| match live.get(key) {
            Some(&count) => {
                colors.truncate(count);
                true
            }
            None => false,
        });
        self.live = live;
        Ok(())
    }

    /// The simulator visits food in reverse insertion order before ambient
    /// replenishment. Consume those lifetimes before assigning refill births.
    /// Return whether the removed lifetime was the visible, oldest pellet.
    pub(super) fn picked_up(&mut self, food: &Food) -> Result<bool, String> {
        let key = identity(food);
        let colors = self
            .colors
            .get_mut(&key)
            .ok_or("Picked-up food is missing presentation provenance")?;
        colors
            .pop_back()
            .ok_or("Picked-up food has no remaining lifetime")?;
        let visible = colors.is_empty();
        if visible {
            self.colors.remove(&key);
        }
        Ok(visible)
    }

    /// Call after births and a successful sync of the current world food.
    pub(super) fn color(&self, food: &Food) -> u8 {
        self.colors[&identity(food)][0]
    }

    fn ambient_color(random: &mut Random) -> u8 {
        let total: u32 = AMBIENT_COLOR_WEIGHTS.iter().sum();
        let mut left = random.unit() * f64::from(total);
        for (color, weight) in AMBIENT_COLOR_WEIGHTS.iter().enumerate() {
            if left < f64::from(*weight) {
                return color as u8;
            }
            left -= f64::from(*weight);
        }
        unreachable!("unit draw is less than one and ambient weights are positive")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn food(x: f64, origin: Option<Origin>) -> Food {
        Food {
            x,
            y: 0.,
            value: 1.,
            size: Some(3.),
            origin,
        }
    }

    fn births(presentation: &mut FoodPresentation, source: usize, food: &[Food]) {
        let events: Vec<_> = food
            .iter()
            .map(|food| FoodSpawnEvent {
                source: Some(source),
                food: food.clone(),
            })
            .collect();
        presentation.births(&events).unwrap();
        presentation.sync(food).unwrap();
    }

    fn colors(presentation: &FoodPresentation, food: &[Food]) -> Vec<u8> {
        food.iter().map(|food| presentation.color(food)).collect()
    }

    #[test]
    fn tables_match_retained_fixture_and_verified_prefix_rotations() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/client-presentation.json"
        ))
        .unwrap();
        let builtin = fixture["builtin"].as_object().unwrap();
        assert_eq!(builtin.len(), STOCK_FOOD_PALETTES.len());
        for (skin, row) in builtin {
            let skin: usize = skin.parse().unwrap();
            let recipe = STOCK_FOOD_PALETTES[skin];
            let retained: Vec<u8> = serde_json::from_value(row["cycle"].clone()).unwrap();
            assert_eq!(recipe, retained, "skin {skin}");
            for sample in row["evidence"].as_array().unwrap() {
                let colors: Vec<u8> = serde_json::from_value(sample["colors"].clone()).unwrap();
                let prefix = sample["verified_prefix_length"].as_u64().unwrap() as usize;
                assert!(prefix >= recipe.len() * 2);
                assert!(
                    (0..recipe.len()).any(|phase| {
                        colors[..prefix]
                            .iter()
                            .enumerate()
                            .all(|(i, color)| *color == recipe[(i + phase) % recipe.len()])
                    }),
                    "skin {skin}"
                );
            }
        }
        let remaps = fixture["custom_color_map"].as_object().unwrap();
        for color in 0..=u8::MAX {
            let expected = remaps
                .get(&color.to_string())
                .and_then(|row| row["food_color"].as_u64())
                .map(|value| value as u8);
            assert_eq!(custom_food_color(color), expected, "custom color {color}");
        }
    }

    #[test]
    fn initial_sources_use_skin_zero_and_births_use_exact_source() {
        let mut presentation = FoodPresentation::new(1, 3);
        presentation.register(0, 3, &[]);
        presentation.register(1, 7, &[]);
        let food = vec![
            food(1., Some(Origin::Boost)),
            food(2., Some(Origin::Death)),
            food(3., Some(Origin::Death)),
        ];
        let events: Vec<_> = food
            .iter()
            .enumerate()
            .map(|(source, food)| FoodSpawnEvent {
                source: Some(source),
                food: food.clone(),
            })
            .collect();
        presentation.births(&events).unwrap();
        presentation.sync(&food).unwrap();
        assert_eq!(colors(&presentation, &food), [3, 7, 0]);
    }

    #[test]
    fn custom_rle_preserves_counts_remaps_and_ignores_header_and_invalid_colors() {
        let mut presentation = FoodPresentation::new(1, 1);
        let custom = [255, 8, 255, 8, 255, 8, 255, 8, 3, 23, 2, 27, 9, 36, 255];
        presentation.register(0, 3, &custom);
        let food: Vec<_> = (0..10)
            .map(|x| food(x as f64, Some(Origin::Death)))
            .collect();
        births(&mut presentation, 0, &food);
        assert_eq!(colors(&presentation, &food), [2, 2, 2, 7, 7, 2, 2, 2, 7, 7]);
        presentation.register(0, 25, &[0; 9]);
        assert_eq!(presentation.sources[0].palette, [5, 1]);
        presentation.register(0, 25, &[0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 4, 36]);
        assert_eq!(presentation.sources[0].palette, [5, 1]);
    }

    #[test]
    fn admitted_births_advance_in_order_even_if_not_present_at_sync() {
        let mut presentation = FoodPresentation::new(1, 1);
        presentation.register(0, 15, &[]);
        let emitted: Vec<_> = (0..20)
            .map(|x| food(x as f64, Some(Origin::Death)))
            .collect();
        births(&mut presentation, 0, &emitted);
        assert_eq!(
            colors(&presentation, &emitted),
            [1, 2, 3, 4, 5, 6, 7, 8, 0, 1, 2, 3, 4, 5, 6, 7, 8, 0, 1, 2]
        );
        presentation.sync(&emitted[19..]).unwrap();
        assert_eq!(presentation.colors.len(), 1);
        let gone = food(29., Some(Origin::Boost));
        presentation
            .births(&[FoodSpawnEvent {
                source: Some(0),
                food: gone,
            }])
            .unwrap();
        let fresh = food(30., Some(Origin::Boost));
        births(&mut presentation, 0, std::slice::from_ref(&fresh));
        assert_eq!(presentation.color(&fresh), 4);
        assert_eq!(presentation.colors.len(), 1);
    }

    #[test]
    fn register_resets_cursor_without_recoloring_old_food() {
        let mut presentation = FoodPresentation::new(1, 1);
        presentation.register(0, 25, &[]);
        let mut items = vec![food(1., Some(Origin::Death))];
        births(&mut presentation, 0, &items);
        presentation.register(0, 9, &[]);
        presentation.sync(&items).unwrap();
        assert_eq!(presentation.color(&items[0]), 5);
        items.push(food(2., Some(Origin::Boost)));
        presentation
            .births(&[FoodSpawnEvent {
                source: Some(0),
                food: items[1].clone(),
            }])
            .unwrap();
        presentation.sync(&items).unwrap();
        assert_eq!(colors(&presentation, &items), [5, 1]);
        presentation.register(0, 9, &[]);
        presentation.sync(&items).unwrap();
        assert_eq!(colors(&presentation, &items), [5, 1]);
        items.push(food(3., Some(Origin::Death)));
        presentation
            .births(&[FoodSpawnEvent {
                source: Some(0),
                food: items[2].clone(),
            }])
            .unwrap();
        presentation.sync(&items).unwrap();
        assert_eq!(colors(&presentation, &items), [5, 1, 1]);
    }

    #[test]
    fn ambient_and_unspecified_food_have_persistent_variety() {
        let mut presentation = FoodPresentation::new(12, 0);
        let mut food: Vec<_> = (0..200)
            .map(|x| food(x as f64, (x % 2 == 0).then_some(Origin::Ambient)))
            .collect();
        presentation.sync(&food).unwrap();
        let before = colors(&presentation, &food);
        assert!(before.iter().copied().collect::<HashSet<_>>().len() >= 8);
        food.reverse();
        presentation.sync(&food).unwrap();
        let mut after = colors(&presentation, &food);
        after.reverse();
        assert_eq!(before, after);
        let mut repeated = FoodPresentation::new(12, 0);
        food.reverse();
        repeated.sync(&food).unwrap();
        assert_eq!(before, colors(&repeated, &food));
        presentation.sync(&[]).unwrap();
        assert!(presentation.colors.is_empty());
    }

    #[test]
    fn missing_or_unknown_source_is_rejected() {
        let mut presentation = FoodPresentation::new(1, 1);
        for origin in [Origin::Boost, Origin::Death] {
            let food = food(1., Some(origin));
            assert!(
                presentation
                    .sync(std::slice::from_ref(&food))
                    .unwrap_err()
                    .contains("birth provenance")
            );
            assert!(
                presentation
                    .births(&[FoodSpawnEvent {
                        source: None,
                        food: food.clone(),
                    }])
                    .unwrap_err()
                    .contains("birth provenance")
            );
            assert!(
                presentation
                    .births(&[FoodSpawnEvent {
                        source: Some(1),
                        food,
                    }])
                    .unwrap_err()
                    .contains("registered appearance")
            );
        }
        assert!(presentation.colors.is_empty());
    }

    #[test]
    fn white_choice_is_shared_by_all_runs_and_births_in_each_life() {
        let mut presentation = FoodPresentation::new(1, 1);
        let custom = [0, 0, 0, 0, 0, 0, 0, 0, 2, 9, 3, 9];
        let food: Vec<_> = (0..12)
            .map(|x| food(x as f64, Some(Origin::Death)))
            .collect();
        let mut choices = HashSet::new();
        for _ in 0..64 {
            presentation.sync(&[]).unwrap();
            presentation.register(0, 0, &custom);
            births(&mut presentation, 0, &food);
            let color = presentation.color(&food[0]);
            assert!(color <= 2);
            assert!(
                colors(&presentation, &food)
                    .iter()
                    .all(|value| *value == color)
            );
            choices.insert(color);
        }
        assert_eq!(choices, HashSet::from([0, 1, 2]));
    }

    #[test]
    fn unknown_skin_falls_back_to_modulo_nine() {
        let mut presentation = FoodPresentation::new(1, 1);
        presentation.register(0, 250, &[]);
        let food = food(1., Some(Origin::Death));
        births(&mut presentation, 0, std::slice::from_ref(&food));
        assert_eq!(presentation.color(&food), 7);
    }

    #[test]
    fn exact_coordinate_identity_normalizes_zero_and_new_birth_can_reuse_a_point() {
        let mut presentation = FoodPresentation::new(1, 1);
        presentation.register(0, 9, &[]);
        let first = food(-0., Some(Origin::Death));
        let same = food(0., Some(Origin::Death));
        let nearby = food(f64::from_bits(1), Some(Origin::Death));
        births(&mut presentation, 0, &[first, nearby.clone()]);
        assert_eq!(presentation.color(&same), 1);
        assert_eq!(presentation.color(&nearby), 7);
        assert_eq!(presentation.colors.len(), 2);
        presentation.register(0, 3, &[]);
        presentation.sync(&[same.clone(), nearby.clone()]).unwrap();
        assert_eq!(presentation.color(&same), 1);
        presentation.picked_up(&same).unwrap();
        births(&mut presentation, 0, std::slice::from_ref(&same));
        assert_eq!(presentation.color(&same), 3);
        assert_eq!(presentation.colors.len(), 1);
    }

    #[test]
    fn overlapping_births_keep_the_first_lifetime_and_reverse_pickup_order() {
        let mut presentation = FoodPresentation::new(1, 2);
        presentation.register(0, 3, &[]);
        presentation.register(1, 7, &[]);
        let item = food(10., Some(Origin::Death));
        for source in [0, 1] {
            presentation
                .births(&[FoodSpawnEvent {
                    source: Some(source),
                    food: item.clone(),
                }])
                .unwrap();
        }
        presentation.sync(&[item.clone(), item.clone()]).unwrap();
        assert_eq!(presentation.color(&item), 3);
        assert!(!presentation.picked_up(&item).unwrap());
        assert_eq!(presentation.color(&item), 3);
        assert!(presentation.picked_up(&item).unwrap());
        // Replenishment after pickup starts a new lifetime at the same point.
        births(&mut presentation, 1, std::slice::from_ref(&item));
        assert_eq!(presentation.color(&item), 7);
    }
}
