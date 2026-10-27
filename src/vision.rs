//! Recipient coverage policy. Arena coverage is a custom-mode design choice,
//! not an inferred official visibility rule.
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A head-centered radius, or every living snake and prey in the live arena.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Vision {
    Radius(f64),
    Arena,
}

/// Food delivery is independent of snake/prey coverage. Local keeps the stock
/// food-sector stencil; arena subscribes every sector touched by the live wall.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FoodView {
    #[default]
    Local,
    Arena,
}

impl FoodView {
    pub(crate) fn is_local(&self) -> bool {
        *self == Self::Local
    }
}

impl Vision {
    /// Finite observation radius in world units. Arena coverage reports the
    /// live diameter, enough to see from one wall to the opposite wall.
    pub fn radius(self, arena_radius: f64) -> f64 {
        match self {
            Self::Radius(radius) => radius,
            Self::Arena => 2. * arena_radius,
        }
    }
}

impl Serialize for Vision {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Radius(radius) => serializer.serialize_f64(*radius),
            Self::Arena => serializer.serialize_str("arena"),
        }
    }
}

impl<'de> Deserialize<'de> for Vision {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl de::Visitor<'_> for Visitor {
            type Value = Vision;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a positive finite radius or \"arena\"")
            }
            fn visit_f64<E: de::Error>(self, radius: f64) -> Result<Vision, E> {
                Ok(Vision::Radius(radius))
            }
            fn visit_u64<E: de::Error>(self, radius: u64) -> Result<Vision, E> {
                self.visit_f64(radius as f64)
            }
            fn visit_i64<E: de::Error>(self, radius: i64) -> Result<Vision, E> {
                self.visit_f64(radius as f64)
            }
            fn visit_str<E: de::Error>(self, value: &str) -> Result<Vision, E> {
                match value {
                    "arena" => Ok(Vision::Arena),
                    _ => Err(E::unknown_variant(value, &["arena"])),
                }
            }
        }
        deserializer.deserialize_any(Visitor)
    }
}
