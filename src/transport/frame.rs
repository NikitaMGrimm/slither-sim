//! One world's reply frame. Every snake, food item and prey that any observed slot
//! sees appears once, in columns; each slot's view lists the rows it sees. Replies
//! used to repeat that data per observer, which made them about ten times larger.
use crate::{
    model::{DeathEvent, DeathReason, Origin},
    observation::Sighting,
    simulator::Simulator,
};
use serde::Serialize;

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

/// Codes of the `food.origin` column, in `info.origins` order.
pub(crate) const ORIGINS: [&str; 3] = ["ambient", "boost", "death"];
fn origin_code(origin: Origin) -> u8 {
    match origin {
        Origin::Ambient => 0,
        Origin::Boost => 1,
        Origin::Death => 2,
    }
}

// Fields are declared in key order, so the canonical writer never reorders them.

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct SnakeColumns {
    /// Row `r`'s anchors are `body_x[body_start[r]..body_start[r + 1]]`.
    body_start: Vec<usize>,
    body_x: Vec<f64>,
    body_y: Vec<f64>,
    fam: Vec<f64>,
    heading: Vec<f64>,
    kills: Vec<usize>,
    length: Vec<f64>,
    radius: Vec<f64>,
    score: Vec<f64>,
    segments: Vec<usize>,
    slot: Vec<usize>,
    speed: Vec<f64>,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Default, Serialize)]
struct FoodColumns {
    origin: Vec<Option<u8>>,
    size: Vec<Option<f64>>,
    value: Vec<f64>,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreyColumns {
    heading: Vec<f64>,
    id: Vec<u64>,
    size: Vec<f64>,
    speed: Vec<f64>,
    target_heading: Vec<f64>,
    x: Vec<f64>,
    y: Vec<f64>,
}

#[derive(Default, Serialize)]
struct View {
    food: Vec<usize>,
    prey: Vec<usize>,
    #[serde(rename = "self")]
    me: usize,
    snakes: Vec<usize>,
}

#[derive(Serialize)]
struct Death {
    food: usize,
    killer: Option<usize>,
    reason: DeathReason,
    time: f64,
    victim: usize,
}

#[derive(Serialize)]
struct Frame<'a> {
    deaths: &'a [Death],
    food: &'a FoodColumns,
    living: usize,
    prey: &'a PreyColumns,
    radius: f64,
    snakes: &'a SnakeColumns,
    ticks: usize,
    time: f64,
    views: &'a [Option<View>],
    vision: f64,
}

/// Builds the frames of one world, keeping its buffers between replies.
#[derive(Default)]
pub(crate) struct FrameBuilder {
    sightings: Vec<Option<Sighting>>,
    marks: Vec<u64>,
    food_seen: Vec<u64>,
    /// Row of each seen food item, prey and snake; entries of unseen ones are stale.
    food_row: Vec<usize>,
    prey_row: Vec<usize>,
    snake_row: Vec<usize>,
    snakes: SnakeColumns,
    food: FoodColumns,
    prey: PreyColumns,
    views: Vec<Option<View>>,
    deaths: Vec<Death>,
}

const UNSEEN: usize = usize::MAX;

impl FrameBuilder {
    /// The frame of `world` after `ticks` ticks that produced `deaths`.
    pub(crate) fn encode(
        &mut self,
        world: &Simulator,
        ticks: usize,
        deaths: &[DeathEvent],
    ) -> Vec<u8> {
        world.sight_all_into(&mut self.sightings, &mut self.marks);
        self.collect_snakes(world);
        self.collect_food(world);
        self.collect_prey(world);
        self.collect_views();
        let slot_of = |id: &str| world.snakes.iter().position(|s| s.id == id);
        self.deaths.clear();
        self.deaths.extend(deaths.iter().map(|death| Death {
            food: death.food,
            killer: death.killer.as_deref().and_then(slot_of),
            reason: death.reason,
            time: death.time,
            victim: slot_of(&death.victim).expect("victims are spawned agents"),
        }));
        crate::json::to_vec(&Frame {
            deaths: &self.deaths,
            food: &self.food,
            living: world.living(),
            prey: &self.prey,
            radius: world.config.radius,
            snakes: &self.snakes,
            ticks,
            time: world.time,
            views: &self.views,
            vision: world.config.vision,
        })
        .expect("frames serialize")
    }

    fn collect_snakes(&mut self, world: &Simulator) {
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
            self.snake_row[slot] = c.slot.len();
            c.body_start.push(c.body_x.len());
            for p in &s.body {
                c.body_x.push(round(p.x, POSITION_STEPS));
                c.body_y.push(round(p.y, POSITION_STEPS));
            }
            c.fam.push(s.fam);
            c.heading.push(round(s.heading, ANGLE_STEPS));
            c.kills.push(s.kills);
            c.length.push(s.length);
            c.radius.push(s.radius);
            c.score.push(s.score);
            c.segments.push(s.segments);
            c.slot.push(slot);
            c.speed.push(round(s.speed, SPEED_STEPS));
            c.x.push(round(s.head.x, POSITION_STEPS));
            c.y.push(round(s.head.y, POSITION_STEPS));
        }
        c.body_start.push(c.body_x.len());
    }

    fn collect_food(&mut self, world: &Simulator) {
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
                self.food_row[i] = c.x.len();
                c.origin.push(f.origin.map(origin_code));
                c.size.push(f.size);
                c.value.push(f.value);
                c.x.push(round(f.x, POSITION_STEPS));
                c.y.push(round(f.y, POSITION_STEPS));
            }
        }
    }

    fn collect_prey(&mut self, world: &Simulator) {
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
            self.prey_row[i] = c.id.len();
            c.heading.push(round(p.heading, ANGLE_STEPS));
            c.id.push(p.id);
            c.size.push(p.size);
            c.speed.push(round(p.speed, SPEED_STEPS));
            c.target_heading.push(round(p.target_heading, ANGLE_STEPS));
            c.x.push(round(p.position.x, POSITION_STEPS));
            c.y.push(round(p.position.y, POSITION_STEPS));
        }
    }

    fn collect_views(&mut self) {
        self.views.truncate(self.sightings.len());
        self.views.resize_with(self.sightings.len(), || None);
        for (slot, (sighting, view)) in self.sightings.iter().zip(&mut self.views).enumerate() {
            let Some(sighting) = sighting else {
                *view = None;
                continue;
            };
            let view = view.get_or_insert_with(View::default);
            view.me = self.snake_row[slot];
            view.food.clear();
            view.food
                .extend(sighting.food.iter().map(|&i| self.food_row[i]));
            view.prey.clear();
            view.prey
                .extend(sighting.prey.iter().map(|&i| self.prey_row[i]));
            view.snakes.clear();
            view.snakes
                .extend(sighting.opponents.iter().map(|&i| self.snake_row[i]));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::round;

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
