use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}
impl Point {
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
    pub fn finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
    /// `distance(other) <= r`, with identical results including NaN and overflow.
    #[inline]
    pub(crate) fn distance_le(self, other: Self, r: f64) -> bool {
        match self.distance_order(other, r) {
            Some(order) => order.is_lt(),
            None => self.distance(other) <= r,
        }
    }
    /// `distance(other) < r`, with identical results including NaN and overflow.
    #[inline]
    pub(crate) fn distance_lt(self, other: Self, r: f64) -> bool {
        match self.distance_order(other, r) {
            Some(order) => order.is_lt(),
            None => self.distance(other) < r,
        }
    }
    /// Decides clear cases from the squared distance and leaves the rest to `hypot`.
    /// The squared sum and `hypot` each err by a few ulps, far inside the 1e-9
    /// relative band, so a decided case always agrees with `distance()`. The range
    /// guards keep overflow, underflow and NaN on the `hypot` path.
    #[inline]
    fn distance_order(self, other: Self, r: f64) -> Option<std::cmp::Ordering> {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let d2 = dx * dx + dy * dy;
        if !(r > 1e-50 && r < 1e50 && d2 < 1e100) {
            return None;
        }
        let r2 = r * r;
        if d2 < r2 * (1.0 - 1e-9) {
            Some(std::cmp::Ordering::Less)
        } else if d2 > r2 * (1.0 + 1e-9) {
            Some(std::cmp::Ordering::Greater)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Action {
    pub heading: f64,
    pub boost: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Snake {
    pub id: String,
    pub head: Point,
    /// Active anchors ordered head to tail; separate from head position.
    pub body: Vec<Point>,
    pub heading: f64,
    pub speed: f64,
    /// Display radius in world units; collision geometry derives from segment counts.
    pub radius: f64,
    /// Body length in segments, including fractional growth: `segments + fam`.
    /// This is mass, not geometric length.
    pub length: f64,
    /// Whole body segment count.
    pub segments: usize,
    /// Unconverted growth in segment units; may exceed one before anchors grow.
    pub fam: f64,
    /// Displayed size points from the segment/fam score table.
    pub score: f64,
    // Received observations use zero until a count is reported; the portable
    // client's HUD distinguishes an unknown count from a reported zero.
    #[serde(default)]
    pub kills: usize,
    pub alive: bool,
}
impl Clone for Snake {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            body: self.body.clone(),
            ..*self
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.id.clone_from(&source.id);
        self.body.clone_from(&source.body);
        *self = Self {
            id: std::mem::take(&mut self.id),
            body: std::mem::take(&mut self.body),
            ..*source
        };
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Ambient,
    Boost,
    Death,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Food {
    pub x: f64,
    pub y: f64,
    /// Nominal nutrition in segment units at minimum mass (2); with `size`,
    /// actual gain depends on the eater's mass. Without it, gain is `value`.
    pub value: f64,
    /// Optional protocol pellet size, used for display and mass-dependent nutrition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Origin>,
}
impl Food {
    pub fn point(&self) -> Point {
        Point {
            x: self.x,
            y: self.y,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Arena {
    pub center: Point,
    pub radius: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    pub time: f64,
    #[serde(rename = "self")]
    pub me: Snake,
    pub opponents: Vec<Snake>,
    pub food: Vec<Food>,
    /// Client-known moving entities.
    #[serde(default)]
    pub prey: Vec<crate::prey::Prey>,
    pub arena: Arena,
    /// Effective world-unit radius: the live diameter for arena vision.
    /// Wire clients leave it unknown (`None`).
    pub vision_radius: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeathReason {
    Body,
    Boundary,
    /// Left a configured `pit` after its head center had been inside.
    Pit,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DeathEvent {
    pub time: f64,
    pub victim: String,
    pub killer: Option<String>,
    pub reason: DeathReason,
    pub food: usize,
}

#[cfg(test)]
mod tests {
    use super::Point;

    #[test]
    fn fast_distance_comparisons_match_hypot() {
        let mut state = 0x2545_f491_u64;
        let mut unit = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f64 / (1u64 << 53) as f64
        };
        let special = [
            0.0,
            -0.0,
            1e-60,
            1e60,
            -3.0,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::INFINITY,
            f64::NAN,
        ];
        for i in 0..200_000 {
            let a = Point {
                x: (unit() - 0.5) * 4e4,
                y: (unit() - 0.5) * 4e4,
            };
            let mut b = Point {
                x: a.x + (unit() - 0.5) * 3e3,
                y: a.y + (unit() - 0.5) * 3e3,
            };
            let mut r = a.distance(b);
            match i % 4 {
                0 => {}
                1 => r = f64::from_bits(r.to_bits().wrapping_add(i as u64 % 5) - 2),
                2 => r = unit() * 3e3,
                _ => {
                    r = special[i / 4 % special.len()];
                    if i % 8 == 3 {
                        b.x = special[i / 8 % special.len()];
                    }
                }
            }
            assert_eq!(a.distance_le(b, r), a.distance(b) <= r, "{a:?} {b:?} {r}");
            assert_eq!(a.distance_lt(b, r), a.distance(b) < r, "{a:?} {b:?} {r}");
        }
    }
}
