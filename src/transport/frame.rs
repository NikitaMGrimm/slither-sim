//! One world's reply frame. Every snake, food item and prey that any observed slot
//! sees appears once, in columns; each slot's view lists the rows it sees. Replies
//! used to repeat that data per observer, which made them about ten times larger.
//!
//! [`Frame`] collects the columns in the fixed-width types that in-process callers
//! and the C ABI read directly; JSON is written from them only on request.
use crate::{
    model::{DeathEvent, DeathReason, Origin},
    observation::Sighting,
    simulator::Simulator,
};
use serde::{Serialize, Serializer, ser::SerializeStruct};

/// Rounding steps per unit of observed coordinates, angles and speeds; every other
/// number is exact. Protocol-19 clients receive snake and prey positions in steps
/// of 0.2 world units or coarser, food in steps of about 1.17, motion angles in
/// steps of 2π/256 and speeds in steps of 0.03125 units/s or coarser. Measured
/// grids and error bounds: research/fidelity/2026-10-24-observation-precision.
/// The simulation itself keeps full precision.
pub(crate) const POSITION_STEPS: f64 = 100.0;
pub(crate) const ANGLE_STEPS: f64 = 10_000.0;
pub(crate) const SPEED_STEPS: f64 = 100.0;

/// `x` to the nearest multiple of `1 / steps`. The quotient is the double nearest
/// that decimal, so it prints with at most the step's decimals. Values too large
/// to scale stay exact; adding zero turns -0.0 into 0.0.
fn round(x: f64, steps: f64) -> f64 {
    let scaled = (x * steps).round();
    if scaled.is_finite() {
        scaled / steps + 0.0
    } else {
        x
    }
}

/// Names of the `food.origin` codes, in `info.origins` order.
pub const ORIGINS: [&str; 3] = ["ambient", "boost", "death"];
/// `food.origin` code of food without a recorded origin; JSON prints `null`.
pub const NO_ORIGIN: u8 = 255;
/// Names of the `deaths.reason` codes; JSON prints the name.
pub const REASONS: [&str; 2] = ["body", "boundary"];
/// Row or slot index meaning none: the `self` row of a dead slot and the killer
/// of a death without one. JSON prints `null`.
pub const NONE: u32 = u32::MAX;

fn origin_code(origin: Option<Origin>) -> u8 {
    match origin {
        Some(Origin::Ambient) => 0,
        Some(Origin::Boost) => 1,
        Some(Origin::Death) => 2,
        None => NO_ORIGIN,
    }
}

/// A row or slot index: a `u32` other than the [`NONE`] sentinel.
fn row(i: usize, what: &str) -> Result<u32, String> {
    u32::try_from(i)
        .ok()
        .filter(|&i| i != NONE)
        .ok_or_else(|| format!("Frame {what} index {i} must be below u32::MAX, which means none"))
}

/// A count or column offset: any `u32`.
fn count(i: usize, what: &str) -> Result<u32, String> {
    u32::try_from(i).map_err(|_| format!("Frame {what} count {i} exceeds u32::MAX"))
}

/// Read-only columns of one world's current frame, with the values its JSON frame
/// prints. Column slices of a table have equal length, one element per row.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct FrameRef<'a> {
    /// World time in seconds.
    pub time: f64,
    /// Ticks advanced by the request that produced this frame; 0 after reset.
    pub ticks: u32,
    /// Surviving snakes.
    pub living: u32,
    /// Arena radius around the origin.
    pub radius: f64,
    /// Vision radius.
    pub vision: f64,
    pub snakes: SnakeColumns<'a>,
    pub food: FoodColumns<'a>,
    pub prey: PreyColumns<'a>,
    pub views: ViewColumns<'a>,
    pub deaths: DeathColumns<'a>,
}

/// One row per snake that some living slot sees. Positions, headings and speeds
/// are rounded like the JSON frame.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct SnakeColumns<'a> {
    pub slot: &'a [u32],
    pub x: &'a [f64],
    pub y: &'a [f64],
    pub heading: &'a [f64],
    pub speed: &'a [f64],
    pub radius: &'a [f64],
    pub length: &'a [f64],
    pub segments: &'a [u32],
    pub fam: &'a [f64],
    pub score: &'a [f64],
    pub kills: &'a [u32],
    /// One more entry than rows: row `r`'s anchors, head to tail, are
    /// `body_x[body_start[r]..body_start[r + 1]]` and the same range of `body_y`.
    pub body_start: &'a [u32],
    pub body_x: &'a [f64],
    pub body_y: &'a [f64],
}

/// One row per food item that some living slot sees.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct FoodColumns<'a> {
    pub x: &'a [f64],
    pub y: &'a [f64],
    pub value: &'a [f64],
    /// NaN where the item has no size; JSON prints `null`.
    pub size: &'a [f64],
    /// Index into [`ORIGINS`], or [`NO_ORIGIN`].
    pub origin: &'a [u8],
}

/// One row per prey that some living slot sees.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct PreyColumns<'a> {
    pub id: &'a [u64],
    pub x: &'a [f64],
    pub y: &'a [f64],
    pub heading: &'a [f64],
    pub target_heading: &'a [f64],
    pub speed: &'a [f64],
    pub size: &'a [f64],
}

/// Each slot's view in compressed sparse rows. `me` has one entry per slot: its
/// own `snakes` row, or [`NONE`] when dead. Slot `s` sees the `snakes` rows
/// `snakes[snakes_start[s]..snakes_start[s + 1]]`, and likewise food and prey;
/// each `*_start` has one more entry than there are slots, and dead slots have
/// empty ranges.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct ViewColumns<'a> {
    pub me: &'a [u32],
    pub snakes_start: &'a [u32],
    pub snakes: &'a [u32],
    pub food_start: &'a [u32],
    pub food: &'a [u32],
    pub prey_start: &'a [u32],
    pub prey: &'a [u32],
}

/// One row per death during the request that produced this frame.
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct DeathColumns<'a> {
    pub time: &'a [f64],
    pub victim: &'a [u32],
    /// Killer slot, or [`NONE`].
    pub killer: &'a [u32],
    /// Index into [`REASONS`].
    pub reason: &'a [u8],
    /// Food items dropped.
    pub food: &'a [u32],
}

// Fields are declared in key order, so the canonical writer never reorders them.

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnakeTable {
    body_start: Vec<u32>,
    body_x: Vec<f64>,
    body_y: Vec<f64>,
    fam: Vec<f64>,
    heading: Vec<f64>,
    kills: Vec<u32>,
    length: Vec<f64>,
    radius: Vec<f64>,
    score: Vec<f64>,
    segments: Vec<u32>,
    slot: Vec<u32>,
    speed: Vec<f64>,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Default)]
struct FoodTable {
    origin: Vec<u8>,
    size: Vec<f64>,
    value: Vec<f64>,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreyTable {
    heading: Vec<f64>,
    id: Vec<u64>,
    size: Vec<f64>,
    speed: Vec<f64>,
    target_heading: Vec<f64>,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Default)]
struct ViewTable {
    me: Vec<u32>,
    snakes_start: Vec<u32>,
    snakes: Vec<u32>,
    food_start: Vec<u32>,
    food: Vec<u32>,
    prey_start: Vec<u32>,
    prey: Vec<u32>,
}

#[derive(Default)]
struct DeathTable {
    food: Vec<u32>,
    killer: Vec<u32>,
    reason: Vec<u8>,
    time: Vec<f64>,
    victim: Vec<u32>,
}

/// The current frame of one world, keeping its buffers between requests.
#[derive(Default)]
pub(crate) struct Frame {
    sightings: Vec<Option<Sighting>>,
    marks: Vec<u64>,
    food_seen: Vec<u64>,
    /// Row of each seen food item, prey and snake; entries of unseen ones are stale.
    food_row: Vec<u32>,
    prey_row: Vec<u32>,
    snake_row: Vec<u32>,
    time: f64,
    ticks: u32,
    living: u32,
    radius: f64,
    vision: f64,
    snakes: SnakeTable,
    food: FoodTable,
    prey: PreyTable,
    views: ViewTable,
    deaths: DeathTable,
}

const UNSEEN: u32 = NONE;

impl Frame {
    /// Collects the frame of `world` after `ticks` ticks that produced `deaths`.
    /// Fails, leaving the frame partly written, when a column value does not
    /// fit its `u32` type.
    pub(crate) fn collect(
        &mut self,
        world: &Simulator,
        ticks: usize,
        deaths: &[DeathEvent],
    ) -> Result<(), String> {
        world.sight_all_into(&mut self.sightings, &mut self.marks);
        self.time = world.time;
        self.ticks = count(ticks, "tick")?;
        self.living = count(world.living(), "living snake")?;
        self.radius = world.config.radius;
        self.vision = world.config.vision;
        self.collect_snakes(world)?;
        self.collect_food(world)?;
        self.collect_prey(world)?;
        self.collect_views()?;
        let slot_of = |id: &str| world.snakes.iter().position(|s| s.id == id);
        let d = &mut self.deaths;
        d.food.clear();
        d.killer.clear();
        d.reason.clear();
        d.time.clear();
        d.victim.clear();
        for death in deaths {
            d.food.push(count(death.food, "death food")?);
            d.killer
                .push(match death.killer.as_deref().and_then(slot_of) {
                    Some(slot) => row(slot, "killer slot")?,
                    None => NONE,
                });
            d.reason.push(match death.reason {
                DeathReason::Body => 0,
                DeathReason::Boundary => 1,
            });
            d.time.push(death.time);
            d.victim.push(row(
                slot_of(&death.victim).expect("victims are spawned agents"),
                "victim slot",
            )?);
        }
        Ok(())
    }

    /// The collected frame as one JSON object.
    pub(crate) fn encode(&self) -> Vec<u8> {
        crate::json::to_vec(&Encoded(self)).expect("frames serialize")
    }

    pub(crate) fn columns(&self) -> FrameRef<'_> {
        let (s, f, p, v, d) = (
            &self.snakes,
            &self.food,
            &self.prey,
            &self.views,
            &self.deaths,
        );
        FrameRef {
            time: self.time,
            ticks: self.ticks,
            living: self.living,
            radius: self.radius,
            vision: self.vision,
            snakes: SnakeColumns {
                slot: &s.slot,
                x: &s.x,
                y: &s.y,
                heading: &s.heading,
                speed: &s.speed,
                radius: &s.radius,
                length: &s.length,
                segments: &s.segments,
                fam: &s.fam,
                score: &s.score,
                kills: &s.kills,
                body_start: &s.body_start,
                body_x: &s.body_x,
                body_y: &s.body_y,
            },
            food: FoodColumns {
                x: &f.x,
                y: &f.y,
                value: &f.value,
                size: &f.size,
                origin: &f.origin,
            },
            prey: PreyColumns {
                id: &p.id,
                x: &p.x,
                y: &p.y,
                heading: &p.heading,
                target_heading: &p.target_heading,
                speed: &p.speed,
                size: &p.size,
            },
            views: ViewColumns {
                me: &v.me,
                snakes_start: &v.snakes_start,
                snakes: &v.snakes,
                food_start: &v.food_start,
                food: &v.food,
                prey_start: &v.prey_start,
                prey: &v.prey,
            },
            deaths: DeathColumns {
                time: &d.time,
                victim: &d.victim,
                killer: &d.killer,
                reason: &d.reason,
                food: &d.food,
            },
        }
    }

    fn collect_snakes(&mut self, world: &Simulator) -> Result<(), String> {
        self.snake_row.clear();
        self.snake_row.resize(world.snakes.len(), UNSEEN);
        for (slot, sighting) in self.sightings.iter().enumerate() {
            if let Some(sighting) = sighting {
                self.snake_row[slot] = 0;
                for &i in &sighting.opponents {
                    self.snake_row[i] = 0;
                }
            }
        }
        let c = &mut self.snakes;
        for column in [
            &mut c.body_x,
            &mut c.body_y,
            &mut c.fam,
            &mut c.heading,
            &mut c.length,
            &mut c.radius,
            &mut c.score,
            &mut c.speed,
            &mut c.x,
            &mut c.y,
        ] {
            column.clear();
        }
        for column in [
            &mut c.body_start,
            &mut c.kills,
            &mut c.segments,
            &mut c.slot,
        ] {
            column.clear();
        }
        for (slot, s) in world.snakes.iter().enumerate() {
            if self.snake_row[slot] == UNSEEN {
                continue;
            }
            self.snake_row[slot] = row(c.slot.len(), "snake row")?;
            c.body_start.push(count(c.body_x.len(), "body anchor")?);
            for p in &s.body {
                c.body_x.push(round(p.x, POSITION_STEPS));
                c.body_y.push(round(p.y, POSITION_STEPS));
            }
            c.fam.push(s.fam);
            c.heading.push(round(s.heading, ANGLE_STEPS));
            c.kills.push(count(s.kills, "kill")?);
            c.length.push(s.length);
            c.radius.push(s.radius);
            c.score.push(s.score);
            c.segments.push(count(s.segments, "segment")?);
            c.slot.push(row(slot, "snake slot")?);
            c.speed.push(round(s.speed, SPEED_STEPS));
            c.x.push(round(s.head.x, POSITION_STEPS));
            c.y.push(round(s.head.y, POSITION_STEPS));
        }
        c.body_start.push(count(c.body_x.len(), "body anchor")?);
        Ok(())
    }

    fn collect_food(&mut self, world: &Simulator) -> Result<(), String> {
        self.food_seen.clear();
        self.food_seen.resize(world.food.len().div_ceil(64), 0);
        for sighting in self.sightings.iter().flatten() {
            for &i in &sighting.food {
                self.food_seen[i / 64] |= 1 << (i % 64);
            }
        }
        self.food_row.resize(world.food.len(), UNSEEN);
        let c = &mut self.food;
        c.origin.clear();
        c.size.clear();
        c.value.clear();
        c.x.clear();
        c.y.clear();
        for (word, &bits) in self.food_seen.iter().enumerate() {
            let mut bits = bits;
            while bits != 0 {
                let i = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let f = &world.food[i];
                self.food_row[i] = row(c.x.len(), "food row")?;
                c.origin.push(origin_code(f.origin));
                c.size.push(f.size.unwrap_or(f64::NAN));
                c.value.push(f.value);
                c.x.push(round(f.x, POSITION_STEPS));
                c.y.push(round(f.y, POSITION_STEPS));
            }
        }
        Ok(())
    }

    fn collect_prey(&mut self, world: &Simulator) -> Result<(), String> {
        self.prey_row.clear();
        self.prey_row.resize(world.prey.len(), UNSEEN);
        for sighting in self.sightings.iter().flatten() {
            for &i in &sighting.prey {
                self.prey_row[i] = 0;
            }
        }
        let c = &mut self.prey;
        for column in [
            &mut c.heading,
            &mut c.size,
            &mut c.speed,
            &mut c.target_heading,
            &mut c.x,
            &mut c.y,
        ] {
            column.clear();
        }
        c.id.clear();
        for (i, p) in world.prey.iter().enumerate() {
            if self.prey_row[i] == UNSEEN {
                continue;
            }
            self.prey_row[i] = row(c.id.len(), "prey row")?;
            c.heading.push(round(p.heading, ANGLE_STEPS));
            c.id.push(p.id);
            c.size.push(p.size);
            c.speed.push(round(p.speed, SPEED_STEPS));
            c.target_heading.push(round(p.target_heading, ANGLE_STEPS));
            c.x.push(round(p.position.x, POSITION_STEPS));
            c.y.push(round(p.position.y, POSITION_STEPS));
        }
        Ok(())
    }

    fn collect_views(&mut self) -> Result<(), String> {
        let v = &mut self.views;
        for column in [
            &mut v.me,
            &mut v.snakes_start,
            &mut v.snakes,
            &mut v.food_start,
            &mut v.food,
            &mut v.prey_start,
            &mut v.prey,
        ] {
            column.clear();
        }
        v.snakes_start.push(0);
        v.food_start.push(0);
        v.prey_start.push(0);
        for (slot, sighting) in self.sightings.iter().enumerate() {
            if let Some(sighting) = sighting {
                v.me.push(self.snake_row[slot]);
                v.snakes
                    .extend(sighting.opponents.iter().map(|&i| self.snake_row[i]));
                v.food
                    .extend(sighting.food.iter().map(|&i| self.food_row[i]));
                v.prey
                    .extend(sighting.prey.iter().map(|&i| self.prey_row[i]));
            } else {
                v.me.push(NONE);
            }
            v.snakes_start.push(count(v.snakes.len(), "viewed snake")?);
            v.food_start.push(count(v.food.len(), "viewed food")?);
            v.prey_start.push(count(v.prey.len(), "viewed prey")?);
        }
        Ok(())
    }
}

/// Writes the JSON frame from the collected columns: views as nested objects and
/// codes that mean none as `null`, exactly as before the columns existed.
struct Encoded<'a>(&'a Frame);
impl Serialize for Encoded<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let frame = self.0;
        let mut out = serializer.serialize_struct("Frame", 10)?;
        out.serialize_field("deaths", &Deaths(&frame.deaths))?;
        out.serialize_field("food", &Food(&frame.food))?;
        out.serialize_field("living", &frame.living)?;
        out.serialize_field("prey", &frame.prey)?;
        out.serialize_field("radius", &frame.radius)?;
        out.serialize_field("snakes", &frame.snakes)?;
        out.serialize_field("ticks", &frame.ticks)?;
        out.serialize_field("time", &frame.time)?;
        out.serialize_field("views", &Views(&frame.views))?;
        out.serialize_field("vision", &frame.vision)?;
        out.end()
    }
}

fn some_index(i: u32) -> Option<u32> {
    (i != NONE).then_some(i)
}

/// `[x, ..]` with each `x` mapped through `f`.
struct Mapped<'a, T, U>(&'a [T], fn(T) -> U);
impl<T: Copy, U: Serialize> Serialize for Mapped<'_, T, U> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.0.iter().map(|&x| (self.1)(x)))
    }
}

struct Food<'a>(&'a FoodTable);
impl Serialize for Food<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let c = self.0;
        let mut out = serializer.serialize_struct("FoodColumns", 5)?;
        out.serialize_field(
            "origin",
            &Mapped(&c.origin, |code| (code != NO_ORIGIN).then_some(code)),
        )?;
        out.serialize_field(
            "size",
            &Mapped(&c.size, |size| (!size.is_nan()).then_some(size)),
        )?;
        out.serialize_field("value", &c.value)?;
        out.serialize_field("x", &c.x)?;
        out.serialize_field("y", &c.y)?;
        out.end()
    }
}

#[derive(Serialize)]
struct View<'a> {
    food: &'a [u32],
    prey: &'a [u32],
    #[serde(rename = "self")]
    me: u32,
    snakes: &'a [u32],
}

struct Views<'a>(&'a ViewTable);
impl Serialize for Views<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let v = self.0;
        let range = |start: &[u32], slot: usize| start[slot] as usize..start[slot + 1] as usize;
        serializer.collect_seq(v.me.iter().enumerate().map(|(slot, &me)| {
            some_index(me).map(|me| View {
                food: &v.food[range(&v.food_start, slot)],
                prey: &v.prey[range(&v.prey_start, slot)],
                me,
                snakes: &v.snakes[range(&v.snakes_start, slot)],
            })
        }))
    }
}

#[derive(Serialize)]
struct Death {
    food: u32,
    killer: Option<u32>,
    reason: &'static str,
    time: f64,
    victim: u32,
}

struct Deaths<'a>(&'a DeathTable);
impl Serialize for Deaths<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let d = self.0;
        serializer.collect_seq((0..d.time.len()).map(|i| Death {
            food: d.food[i],
            killer: some_index(d.killer[i]),
            reason: REASONS[d.reason[i] as usize],
            time: d.time[i],
            victim: d.victim[i],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unrepresentable_columns_are_errors_not_panics() {
        assert_eq!(row(NONE as usize - 1, "snake row"), Ok(NONE - 1));
        // u32::MAX is the none sentinel, so it is not a row.
        assert!(row(NONE as usize, "snake row").is_err());
        assert_eq!(count(u32::MAX as usize, "body anchor"), Ok(u32::MAX));
        #[cfg(target_pointer_width = "64")]
        {
            let error = count(u32::MAX as usize + 1, "body anchor").unwrap_err();
            assert!(error.contains("body anchor"), "{error}");
            assert!(row(usize::MAX, "food row").is_err());
        }
    }

    #[test]
    fn codes_meaning_none_print_null() {
        let food = FoodTable {
            origin: vec![NO_ORIGIN, 2],
            size: vec![f64::NAN, 1.5],
            value: vec![1.0, 2.0],
            x: vec![0.0, 1.0],
            y: vec![0.0, -1.0],
        };
        assert_eq!(
            crate::json::to_vec(&Food(&food)).unwrap(),
            br#"{"origin":[null,2],"size":[null,1.5],"value":[1.0,2.0],"x":[0.0,1.0],"y":[0.0,-1.0]}"#
        );
        let deaths = DeathTable {
            food: vec![3, 0],
            killer: vec![NONE, 4],
            reason: vec![1, 0],
            time: vec![0.5, 0.75],
            victim: vec![0, 2],
        };
        assert_eq!(
            crate::json::to_vec(&Deaths(&deaths)).unwrap(),
            br#"[{"food":3,"killer":null,"reason":"boundary","time":0.5,"victim":0},{"food":0,"killer":4,"reason":"body","time":0.75,"victim":2}]"#
        );
    }

    #[test]
    fn rounding_prints_at_most_the_step_decimals() {
        for (x, steps, text) in [
            (-352.05080564444495, 100.0, "-352.05"),
            (43.359375, 100.0, "43.36"),
            (3.579663342337374, 10_000.0, "3.5797"),
            (-0.001, 100.0, "0.0"),
            (1e300, 100.0, "1e+300"),
        ] {
            assert_eq!(serde_json::to_string(&round(x, steps)).unwrap(), text);
        }
        assert!(round(f64::NAN, 100.0).is_nan());
        assert_eq!(round(f64::INFINITY, 100.0), f64::INFINITY);
    }
}
