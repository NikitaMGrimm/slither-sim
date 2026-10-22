//! Visibility, independent of stepping, rewards, and transport.
use crate::{model::*, physics::Bounds, prey::Prey, simulator::Simulator};

/// World state an observation is cut from: the authoritative world, or a frame a
/// client has received.
#[derive(Clone, Copy)]
pub(crate) struct VisibleState<'a> {
    pub time: f64,
    pub snakes: &'a [Snake],
    pub food: &'a [Food],
    pub prey: &'a [Prey],
    pub radius: f64,
    pub vision: f64,
}
impl<'a> VisibleState<'a> {
    pub(crate) fn of(world: &'a Simulator) -> Self {
        Self {
            time: world.time,
            snakes: &world.snakes,
            food: &world.food,
            prey: &world.prey,
            radius: world.config.radius,
            vision: world.config.vision,
        }
    }
    /// Observation of one slot, without the batch index.
    pub(crate) fn observe(self, index: usize) -> Result<Observation, String> {
        Observer::new(self, false).observe(index)
    }
}

struct Index {
    bodies: Vec<Bounds>,
    food_bins: [Vec<usize>; 64],
}
struct Observer<'a> {
    world: VisibleState<'a>,
    index: Option<Index>,
}
impl<'a> Observer<'a> {
    fn new(world: VisibleState<'a>, batch: bool) -> Self {
        let index = if batch
            && world.snakes.len() >= 24
            && world.food.len() >= 1000
            && world.vision >= 1e-6
        {
            let bodies = world
                .snakes
                .iter()
                .map(|s| {
                    let mut b = Bounds::between(s.head, s.head);
                    for &p in &s.body {
                        b.include(p);
                    }
                    b
                })
                .collect();
            let mut food_bins: [Vec<usize>; 64] = std::array::from_fn(|_| Vec::new());
            for (i, f) in world.food.iter().enumerate() {
                food_bins[((f.x / world.vision).floor() as i64) as usize & 63].push(i);
            }
            Some(Index { bodies, food_bins })
        } else {
            None
        };
        Self { world, index }
    }
    fn observe(&self, index: usize) -> Result<Observation, String> {
        let w = self.world;
        let me = w.snakes.get(index).ok_or("Unknown agent slot")?;
        let vision = w.vision;
        let visible = |p: Point| {
            (p.x - me.head.x).abs() <= vision
                && (p.y - me.head.y).abs() <= vision
                && p.distance(me.head) <= vision
        };
        let food = if let Some(search) = &self.index {
            let cell = |x: f64| (x / vision).floor() as i64;
            let lo = cell(me.head.x - vision - 1e-9);
            let hi = cell(me.head.x + vision + 1e-9);
            let mut candidates = Vec::new();
            if hi.saturating_sub(lo) >= 63 {
                candidates.extend((0..w.food.len()).filter(|&i| visible(w.food[i].point())));
            } else {
                for x in lo..=hi {
                    candidates.extend(
                        search.food_bins[x as usize & 63]
                            .iter()
                            .copied()
                            .filter(|&i| visible(w.food[i].point())),
                    );
                }
            }
            candidates.sort_unstable();
            candidates.into_iter().map(|i| w.food[i].clone()).collect()
        } else {
            w.food
                .iter()
                .filter(|f| visible(f.point()))
                .cloned()
                .collect()
        };
        Ok(Observation {
            time: w.time,
            me: me.clone(),
            food,
            prey: w
                .prey
                .iter()
                .filter(|p| visible(p.position))
                .cloned()
                .collect(),
            opponents: w
                .snakes
                .iter()
                .enumerate()
                .filter(|(i, s)| {
                    *i != index
                        && s.alive
                        && self.index.as_ref().is_none_or(|b| {
                            b.bodies[*i].near(Bounds::between(me.head, me.head), vision + 1e-9)
                        })
                        && (visible(s.head) || s.body.iter().any(|&p| visible(p)))
                })
                .map(|(_, s)| s.clone())
                .collect(),
            arena: Arena {
                center: Point::default(),
                radius: w.radius,
            },
            vision_radius: Some(vision),
        })
    }
}
impl Simulator {
    pub fn observe(&self, index: usize) -> Result<Observation, String> {
        VisibleState::of(self).observe(index)
    }
    /// Detached observations; indexes live only for this immutable borrow of world state.
    pub fn observe_all(&self) -> Vec<Option<Observation>> {
        self.observe_matching(None)
            .expect("observations of existing slots cannot fail")
    }
    fn observe_matching(
        &self,
        selected: Option<&[bool]>,
    ) -> Result<Vec<Option<Observation>>, String> {
        if selected.is_some_and(|flags| flags.len() != self.snakes.len()) {
            return Err("One selection flag per agent slot required".into());
        }
        let observer = Observer::new(VisibleState::of(self), true);
        // Result collection cannot promise an exact lower size hint. Reserve stable slots once.
        let mut observations = Vec::with_capacity(self.snakes.len());
        for (i, s) in self.snakes.iter().enumerate() {
            observations.push(if s.alive && selected.is_none_or(|flags| flags[i]) {
                Some(observer.observe(i)?)
            } else {
                None
            });
        }
        Ok(observations)
    }
}
