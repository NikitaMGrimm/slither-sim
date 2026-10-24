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
        Observer::new(self, 1).observe(index, &mut Vec::new())
    }
}

/// Within this coordinate range every rounding error in the queries below is
/// under 2^-9 units, far inside the one-unit query margin.
const TAME: f64 = (1u64 << 40) as f64;
fn tame(p: Point) -> bool {
    p.x.abs() <= TAME && p.y.abs() <= TAME
}

/// Food positions bucketed into square cells, contiguous per cell row.
struct FoodGrid {
    origin: Point,
    inv_cell: f64,
    cols: usize,
    rows: usize,
    /// Cell `c` holds `points[starts[c]..starts[c + 1]]`, cells in row-major order.
    starts: Vec<u32>,
    points: Vec<Point>,
    ids: Vec<u32>,
    /// Food outside the tame range, including NaN, is tested by every observer.
    outliers: Vec<u32>,
}
impl FoodGrid {
    fn new(food: &[Food], vision: f64) -> Self {
        let mut bounds: Option<Bounds> = None;
        let mut outliers = Vec::new();
        for (i, f) in food.iter().enumerate() {
            let p = f.point();
            if tame(p) {
                bounds.get_or_insert(Bounds::between(p, p)).include(p);
            } else {
                outliers.push(i as u32);
            }
        }
        let bounds = bounds.unwrap_or(Bounds::between(Point::default(), Point::default()));
        let cap = (2 * (food.len() - outliers.len()) + 16) as f64;
        let mut cell = (vision * 0.5).max(1.0);
        let (cols, rows) = loop {
            let cols = ((bounds.max.x - bounds.min.x) / cell).floor() + 1.0;
            let rows = ((bounds.max.y - bounds.min.y) / cell).floor() + 1.0;
            if cols * rows <= cap {
                break (cols as usize, rows as usize);
            }
            cell *= 2.0;
        };
        let mut grid = Self {
            origin: bounds.min,
            inv_cell: 1.0 / cell,
            cols,
            rows,
            starts: vec![0; cols * rows + 1],
            points: Vec::new(),
            ids: Vec::new(),
            outliers,
        };
        let cells: Vec<u32> = food
            .iter()
            .map(|f| {
                let p = f.point();
                if tame(p) {
                    let c = grid.row(p.y) * cols + grid.col(p.x);
                    grid.starts[c + 1] += 1;
                    c as u32
                } else {
                    u32::MAX
                }
            })
            .collect();
        for c in 0..cols * rows {
            grid.starts[c + 1] += grid.starts[c];
        }
        let placed = grid.starts[cols * rows] as usize;
        grid.points = vec![Point::default(); placed];
        grid.ids = vec![0; placed];
        let mut next = grid.starts[..cols * rows].to_vec();
        for (i, &c) in cells.iter().enumerate() {
            if c != u32::MAX {
                let slot = &mut next[c as usize];
                grid.points[*slot as usize] = food[i].point();
                grid.ids[*slot as usize] = i as u32;
                *slot += 1;
            }
        }
        grid
    }
    // Rounding is monotone, so a coordinate between two query bounds always lands
    // between their cells; exact cell edges do not matter.
    fn col(&self, x: f64) -> usize {
        (((x - self.origin.x) * self.inv_cell) as usize).min(self.cols - 1)
    }
    fn row(&self, y: f64) -> usize {
        (((y - self.origin.y) * self.inv_cell) as usize).min(self.rows - 1)
    }
}

struct Index {
    bodies: Vec<Bounds>,
    food: Option<FoodGrid>,
}
struct Observer<'a> {
    world: VisibleState<'a>,
    index: Option<Index>,
}
impl<'a> Observer<'a> {
    fn new(world: VisibleState<'a>, observers: usize) -> Self {
        // Exact visibility tests still decide every candidate; the index only
        // skips food and snakes that cannot pass them.
        let food = world.food.len();
        let index = (observers >= 2 && (0.0..=TAME).contains(&world.vision)).then(|| Index {
            bodies: world
                .snakes
                .iter()
                .map(|s| {
                    let mut b = Bounds::between(s.head, s.head);
                    for &p in &s.body {
                        b.include(p);
                    }
                    b
                })
                .collect(),
            // Building the grid costs a few passes over the food list, so small
            // batches scan it directly instead.
            food: (observers >= 8
                && observers.saturating_mul(food) >= 1 << 15
                && food < u32::MAX as usize)
                .then(|| FoodGrid::new(world.food, world.vision)),
        });
        Self { world, index }
    }
    /// `marks` is empty or has one zeroed bit per food item; it is zeroed again on return.
    fn observe(&self, index: usize, marks: &mut Vec<u64>) -> Result<Observation, String> {
        let w = self.world;
        let me = w.snakes.get(index).ok_or("Unknown agent slot")?;
        let vision = w.vision;
        let visible = |p: Point| {
            (p.x - me.head.x).abs() <= vision
                && (p.y - me.head.y).abs() <= vision
                && p.distance_le(me.head, vision)
        };
        // A point that passes `visible` lies within `reach` of a tame head on each
        // axis, so it is inside this box and inside the box's cell range.
        let reach = vision + 1.0;
        let query = self.index.as_ref().filter(|_| tame(me.head)).map(|i| {
            let b = Bounds {
                min: Point {
                    x: me.head.x - reach,
                    y: me.head.y - reach,
                },
                max: Point {
                    x: me.head.x + reach,
                    y: me.head.y + reach,
                },
            };
            (i, b)
        });
        let food = match query {
            Some((
                Index {
                    food: Some(grid), ..
                },
                b,
            )) => {
                marks.resize(w.food.len().div_ceil(64), 0);
                let mut count = 0;
                let mut first = usize::MAX;
                // Branch-free: about half the candidates are visible, unpredictably.
                let mut mark = |i: usize, seen: bool| {
                    marks[i / 64] |= u64::from(seen) << (i % 64);
                    count += usize::from(seen);
                    first = first.min(if seen { i / 64 } else { usize::MAX });
                };
                let (lo, hi) = (grid.col(b.min.x), grid.col(b.max.x));
                for row in grid.row(b.min.y)..=grid.row(b.max.y) {
                    let cells = row * grid.cols;
                    let span =
                        grid.starts[cells + lo] as usize..grid.starts[cells + hi + 1] as usize;
                    for (&p, &i) in grid.points[span.clone()].iter().zip(&grid.ids[span]) {
                        // The same pure conjunction as `visible`, without short-circuits.
                        let seen = ((p.x - me.head.x).abs() <= vision)
                            & ((p.y - me.head.y).abs() <= vision)
                            & p.distance_le(me.head, vision);
                        mark(i as usize, seen);
                    }
                }
                for &i in &grid.outliers {
                    mark(i as usize, visible(w.food[i as usize].point()));
                }
                // Ascending bit order restores the original food order.
                let mut food = Vec::with_capacity(count);
                for (word, bits) in marks.iter_mut().enumerate().skip(first) {
                    if food.len() == count {
                        break;
                    }
                    let mut bits = std::mem::take(bits);
                    while bits != 0 {
                        food.push(w.food[word * 64 + bits.trailing_zeros() as usize].clone());
                        bits &= bits - 1;
                    }
                }
                food
            }
            _ => w
                .food
                .iter()
                .filter(|f| visible(f.point()))
                .cloned()
                .collect(),
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
                        && query.is_none_or(|(index, q)| {
                            let s = index.bodies[*i];
                            !(s.max.x < q.min.x
                                || s.min.x > q.max.x
                                || s.max.y < q.min.y
                                || s.min.y > q.max.y)
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
    /// Materialize selected living slots using one shared observation index.
    /// The mask and result have one entry per snake; dead and unselected slots are None.
    pub fn observe_selected(&self, selected: &[bool]) -> Result<Vec<Option<Observation>>, String> {
        self.observe_matching(Some(selected))
    }
    fn observe_matching(
        &self,
        selected: Option<&[bool]>,
    ) -> Result<Vec<Option<Observation>>, String> {
        if selected.is_some_and(|flags| flags.len() != self.snakes.len()) {
            return Err("One selection flag per agent slot required".into());
        }
        let wanted = |i: usize| self.snakes[i].alive && selected.is_none_or(|flags| flags[i]);
        let observers = (0..self.snakes.len()).filter(|&i| wanted(i)).count();
        // A dead-only or empty selection needs stable result slots, but no index.
        if observers == 0 {
            return Ok((0..self.snakes.len()).map(|_| None).collect());
        }
        let observer = Observer::new(VisibleState::of(self), observers);
        let mut marks = Vec::new();
        // Result collection cannot promise an exact lower size hint. Reserve stable slots once.
        let mut observations = Vec::with_capacity(self.snakes.len());
        for i in 0..self.snakes.len() {
            observations.push(if wanted(i) {
                Some(observer.observe(i, &mut marks)?)
            } else {
                None
            });
        }
        Ok(observations)
    }
}

#[cfg(test)]
mod tests {
    use crate::{model::*, prey::Prey, simulator::Simulator};

    struct Rng(u64);
    impl Rng {
        fn unit(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn below(&mut self, n: usize) -> usize {
            (self.unit() * n as f64) as usize
        }
        fn pick(&mut self, values: &[f64]) -> f64 {
            values[self.below(values.len())]
        }
    }

    const ODD: [f64; 9] = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e20,
        -1e300,
        (1u64 << 40) as f64,
        (1u64 << 40) as f64 + 4096.0,
        -0.0,
        f64::MAX,
    ];

    /// Mostly ordinary points near the arena, sometimes outside it or degenerate.
    fn point(rng: &mut Rng, spread: f64, near: Point, vision: f64) -> Point {
        match rng.below(12) {
            0 => Point {
                x: rng.pick(&ODD),
                y: (rng.unit() - 0.5) * spread,
            },
            1 => Point {
                x: (rng.unit() - 0.5) * spread,
                y: rng.pick(&ODD),
            },
            // On or a few ulps around the vision circle and box edges.
            2 | 3 => {
                let a = rng.unit() * std::f64::consts::TAU;
                let r = f64::from_bits(
                    vision
                        .abs()
                        .to_bits()
                        .wrapping_add(rng.below(5) as u64)
                        .wrapping_sub(2),
                );
                let (s, c) = if rng.below(2) == 0 {
                    a.sin_cos()
                } else {
                    (0.0, 1.0)
                };
                Point {
                    x: near.x + c * r,
                    y: near.y + s * r,
                }
            }
            4 => near,
            _ => Point {
                x: (rng.unit() - 0.5) * spread,
                y: (rng.unit() - 0.5) * spread,
            },
        }
    }

    fn world(rng: &mut Rng, snakes: usize, food: usize, vision: f64) -> Simulator {
        let mut world = Simulator::new(7, crate::simulator::Config::sandbox()).unwrap();
        world.config.vision = vision;
        let spread = rng.pick(&[10.0, 3000.0, 20_000.0]);
        let center = Point::default();
        let heads: Vec<_> = (0..snakes)
            .map(|_| point(rng, spread, center, vision))
            .collect();
        // Bodies cross the vision edges of other heads, not only their own.
        world.snakes = (0..snakes)
            .map(|i| {
                let anchors = rng.below(40);
                Snake {
                    id: i.to_string(),
                    head: heads[i],
                    body: (0..anchors)
                        .map(|_| {
                            let near = heads[rng.below(snakes)];
                            point(rng, spread, near, vision)
                        })
                        .collect(),
                    heading: rng.unit(),
                    speed: 185.0,
                    radius: 10.0,
                    length: anchors as f64,
                    segments: anchors,
                    fam: 0.0,
                    score: 10.0,
                    kills: 0,
                    alive: rng.below(5) != 0,
                }
            })
            .collect();
        world.food = (0..food)
            .map(|_| {
                let near = match world.snakes.len() {
                    0 => center,
                    n => world.snakes[rng.below(n)].head,
                };
                let p = point(rng, spread, near, vision);
                Food {
                    x: p.x,
                    y: p.y,
                    value: rng.unit(),
                    size: (rng.below(2) == 0).then_some(rng.unit()),
                    origin: (rng.below(2) == 0).then_some(Origin::Death),
                }
            })
            .collect();
        world.prey = (0..rng.below(6))
            .map(|id| Prey {
                id: id as u64,
                position: point(rng, spread, center, vision),
                heading: 0.0,
                target_heading: 0.0,
                speed: 1.0,
                size: 1.0,
            })
            .collect();
        world
    }

    /// Debug output keeps NaN, signed zeros and exact float digits distinct.
    fn key(observations: &[Option<Observation>]) -> Vec<String> {
        observations.iter().map(|o| format!("{o:?}")).collect()
    }

    #[test]
    fn batch_observations_match_single_slot_observations() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let visions = [
            1500.0,
            1500.0,
            300.0,
            0.0,
            -0.0,
            1e-9,
            0.5,
            1e5,
            (1u64 << 40) as f64,
            1e13,
            f64::INFINITY,
            f64::NAN,
            -1.0,
        ];
        for case in 0..600 {
            let vision = visions[case % visions.len()];
            let snakes = [0, 1, 2, 3, 4, 5, 9, 30][rng.below(8)];
            let food = [0, 1, 255, 256, 700, 3000][rng.below(6)];
            let world = world(&mut rng, snakes, food, vision);
            let single: Vec<_> = (0..snakes)
                .map(|i| world.snakes[i].alive.then(|| world.observe(i).unwrap()))
                .collect();
            assert_eq!(key(&world.observe_all()), key(&single), "case {case}");
            let mask: Vec<bool> = (0..snakes).map(|_| rng.below(3) != 0).collect();
            let selected: Vec<_> = single
                .iter()
                .zip(&mask)
                .map(|(o, &m)| o.clone().filter(|_| m))
                .collect();
            assert_eq!(
                key(&world.observe_selected(&mask).unwrap()),
                key(&selected),
                "case {case}"
            );
        }
    }
}
