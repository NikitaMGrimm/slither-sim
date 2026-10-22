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
    /// Display-width estimate. Collision width is configured separately.
    pub radius: f64,
    pub length: f64,
    pub segments: usize,
    pub fam: f64,
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
    pub value: f64,
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
    pub vision_radius: Option<f64>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum DeathReason {
    Body,
    Boundary,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DeathEvent {
    pub time: f64,
    pub victim: String,
    pub killer: Option<String>,
    pub reason: DeathReason,
    pub food: usize,
}
