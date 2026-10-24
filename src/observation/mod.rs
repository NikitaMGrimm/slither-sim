//! Visibility, independent of stepping, rewards, and transport.
use crate::{model::*, physics::Bounds, prey::Prey, simulator::Simulator};
use std::ops::Range;

/// Fewest observers worth a parallel task: one observation takes roughly 10-30
/// microseconds, and waking pool threads costs tens of microseconds.
#[cfg(feature = "parallel")]
const OBSERVERS_PER_TASK: usize = 8;

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
        let mut slot = None;
        Observer::new(self, 1).observe_into(index, &mut Vec::new(), &mut slot)?;
        Ok(slot.expect("observed"))
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
    /// Writes the observation of `index` into `slot`, refilling the vectors of the
    /// observation already there. `marks` is empty or has one zeroed bit per food
    /// item; it is zeroed again on return.
    fn observe_into(
        &self,
        index: usize,
        marks: &mut Vec<u64>,
        slot: &mut Option<Observation>,
    ) -> Result<(), String> {
        let w = self.world;
        let me = w.snakes.get(index).ok_or("Unknown agent slot")?;
        let (previous_me, mut food, mut prey, mut opponents) = slot
            .take()
            .map(|o| (Some(o.me), o.food, o.prey, o.opponents))
            .unwrap_or_default();
        food.clear();
        prey.clear();
        let mut copies = Copies {
            world: w,
            food: &mut food,
            prey: &mut prey,
            opponents: &mut opponents,
            kept: 0,
        };
        self.visit(index, marks, &mut copies)?;
        let kept = copies.kept;
        opponents.truncate(kept);
        let me = match previous_me {
            Some(mut previous) => {
                previous.clone_from(me);
                previous
            }
            None => me.clone(),
        };
        *slot = Some(Observation {
            time: w.time,
            me,
            opponents,
            food,
            prey,
            arena: Arena {
                center: Point::default(),
                radius: w.radius,
            },
            vision_radius: Some(w.vision),
        });
        Ok(())
    }
    /// Reports what `index` sees to `sink`: world indices of visible food, prey and
    /// opponents, each in world order. `marks` is as in `observe_into`.
    pub(crate) fn visit(
        &self,
        index: usize,
        marks: &mut Vec<u64>,
        sink: &mut impl Sink,
    ) -> Result<(), String> {
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
        match query {
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
                sink.reserve_food(count);
                let mut reported = 0;
                for (word, bits) in marks.iter_mut().enumerate().skip(first) {
                    if reported == count {
                        break;
                    }
                    let mut bits = std::mem::take(bits);
                    while bits != 0 {
                        sink.food(word * 64 + bits.trailing_zeros() as usize);
                        reported += 1;
                        bits &= bits - 1;
                    }
                }
            }
            _ => {
                for (i, f) in w.food.iter().enumerate() {
                    if visible(f.point()) {
                        sink.food(i);
                    }
                }
            }
        }
        for (i, p) in w.prey.iter().enumerate() {
            if visible(p.position) {
                sink.prey(i);
            }
        }
        let visible_opponents = w
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
            .map(|(i, _)| i);
        for i in visible_opponents {
            sink.opponent(i);
        }
        Ok(())
    }
}

/// Receives what one slot sees from [`Observer::visit`], as world indices.
pub(crate) trait Sink {
    /// `count` food items follow.
    fn reserve_food(&mut self, _count: usize) {}
    fn food(&mut self, index: usize);
    fn prey(&mut self, index: usize);
    fn opponent(&mut self, index: usize);
}

/// What one slot sees: world indices of food, prey and opponents, in world order.
#[derive(Debug, Default)]
pub(crate) struct Sighting {
    pub food: Vec<usize>,
    pub prey: Vec<usize>,
    pub opponents: Vec<usize>,
}
impl Sink for Sighting {
    fn reserve_food(&mut self, count: usize) {
        self.food.reserve(count);
    }
    fn food(&mut self, index: usize) {
        self.food.push(index);
    }
    fn prey(&mut self, index: usize) {
        self.prey.push(index);
    }
    fn opponent(&mut self, index: usize) {
        self.opponents.push(index);
    }
}

/// Copies what a slot sees into the vectors of an observation.
struct Copies<'a, 'w> {
    world: VisibleState<'w>,
    food: &'a mut Vec<Food>,
    prey: &'a mut Vec<Prey>,
    opponents: &'a mut Vec<Snake>,
    /// Opponents written so far; any beyond are left over from earlier calls.
    kept: usize,
}
impl Sink for Copies<'_, '_> {
    fn reserve_food(&mut self, count: usize) {
        self.food.reserve(count);
    }
    fn food(&mut self, index: usize) {
        self.food.push(self.world.food[index].clone());
    }
    fn prey(&mut self, index: usize) {
        self.prey.push(self.world.prey[index].clone());
    }
    fn opponent(&mut self, index: usize) {
        let snake = &self.world.snakes[index];
        // `clone_from` keeps the id and body allocations of the snakes already there.
        match self.opponents.get_mut(self.kept) {
            Some(previous) => previous.clone_from(snake),
            None => self.opponents.push(snake.clone()),
        }
        self.kept += 1;
    }
}
impl Simulator {
    pub fn observe(&self, index: usize) -> Result<Observation, String> {
        VisibleState::of(self).observe(index)
    }
    /// Detached observations; indexes live only for this immutable borrow of world state.
    pub fn observe_all(&self) -> Vec<Option<Observation>> {
        let mut observations = Vec::new();
        self.observe_all_into(&mut observations);
        observations
    }
    /// [`observe_all`](Self::observe_all) into `observations`, refilling the
    /// vectors of the observations already there. A bot that observes every
    /// decision through one buffer then allocates little per call.
    pub fn observe_all_into(&self, observations: &mut Vec<Option<Observation>>) {
        self.observe_matching(None, observations)
            .expect("observations of existing slots cannot fail")
    }
    /// Materialize selected living slots using one shared observation index.
    /// The mask and result have one entry per snake; dead and unselected slots are None.
    pub fn observe_selected(&self, selected: &[bool]) -> Result<Vec<Option<Observation>>, String> {
        let mut observations = Vec::new();
        self.observe_selected_into(selected, &mut observations)?;
        Ok(observations)
    }
    /// [`observe_selected`](Self::observe_selected) into `observations`, like
    /// [`observe_all_into`](Self::observe_all_into). An invalid mask leaves
    /// `observations` unchanged.
    pub fn observe_selected_into(
        &self,
        selected: &[bool],
        observations: &mut Vec<Option<Observation>>,
    ) -> Result<(), String> {
        self.observe_matching(Some(selected), observations)
    }
    /// One entry per slot: what each living slot sees, by the same rules as
    /// [`observe_all`](Self::observe_all) but as indices into world state.
    /// `marks` is scratch space kept between calls.
    pub(crate) fn sight_all_into(
        &self,
        sightings: &mut Vec<Option<Sighting>>,
        marks: &mut Vec<u64>,
    ) {
        sightings.truncate(self.snakes.len());
        sightings.resize_with(self.snakes.len(), || None);
        let observer = Observer::new(VisibleState::of(self), self.living());
        for (i, slot) in sightings.iter_mut().enumerate() {
            if !self.snakes[i].alive {
                *slot = None;
                continue;
            }
            let sighting = slot.get_or_insert_with(Sighting::default);
            sighting.food.clear();
            sighting.prey.clear();
            sighting.opponents.clear();
            observer
                .visit(i, marks, sighting)
                .expect("existing slots are observable");
        }
    }
    fn observe_matching(
        &self,
        selected: Option<&[bool]>,
        observations: &mut Vec<Option<Observation>>,
    ) -> Result<(), String> {
        if selected.is_some_and(|flags| flags.len() != self.snakes.len()) {
            return Err("One selection flag per agent slot required".into());
        }
        let wanted: Vec<bool> = (0..self.snakes.len())
            .map(|i| self.snakes[i].alive && selected.is_none_or(|flags| flags[i]))
            .collect();
        let observers = wanted.iter().filter(|&&w| w).count();
        // A dead-only or empty selection needs stable result slots, but no index.
        if observers == 0 {
            observations.clear();
            observations.resize_with(self.snakes.len(), || None);
            return Ok(());
        }
        observations.truncate(self.snakes.len());
        observations.resize_with(self.snakes.len(), || None);
        let observer = Observer::new(VisibleState::of(self), observers);
        let observe_range = |range: Range<usize>, slots: &mut [Option<Observation>]| {
            let mut marks = Vec::new();
            for (i, slot) in range.zip(slots) {
                if wanted[i] {
                    observer.observe_into(i, &mut marks, slot)?;
                } else {
                    *slot = None;
                }
            }
            Ok::<_, String>(())
        };
        #[cfg(feature = "parallel")]
        if self.parallel() {
            let parts = crate::parallel::parts(observers / OBSERVERS_PER_TASK);
            if parts > 1 {
                let ranges = crate::parallel::balanced(&wanted, parts);
                return crate::parallel::map_ranges(&ranges, observations, &observe_range)
                    .into_iter()
                    .collect();
            }
        }
        observe_range(0..self.snakes.len(), observations)
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
        // Reused across worlds of every shape, so refills meet longer and shorter
        // vectors, other slot counts and other snakes.
        let mut buffer = Vec::new();
        for case in 0..600 {
            let vision = visions[case % visions.len()];
            let snakes = [0, 1, 2, 3, 4, 5, 9, 30][rng.below(8)];
            let food = [0, 1, 255, 256, 700, 3000][rng.below(6)];
            let world = world(&mut rng, snakes, food, vision);
            let single: Vec<_> = (0..snakes)
                .map(|i| world.snakes[i].alive.then(|| world.observe(i).unwrap()))
                .collect();
            assert_eq!(key(&world.observe_all()), key(&single), "case {case}");
            world.observe_all_into(&mut buffer);
            assert_eq!(key(&buffer), key(&single), "case {case}");
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
            let before = key(&buffer);
            assert!(
                world
                    .observe_selected_into(&[true; 31], &mut buffer)
                    .is_err()
            );
            assert_eq!(key(&buffer), before, "case {case}");
            world.observe_selected_into(&mask, &mut buffer).unwrap();
            assert_eq!(key(&buffer), key(&selected), "case {case}");
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_batches_match_sequential_batches() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        let mut buffer = Vec::new();
        for case in 0..40 {
            let vision = [1500.0, 300.0, f64::NAN, 1e13][case % 4];
            let snakes = [16, 17, 40, 70][rng.below(4)];
            let food = [0, 700, 3000][rng.below(3)];
            let mut world = world(&mut rng, snakes, food, vision);
            let all = key(&world.observe_all());
            let mask: Vec<bool> = (0..snakes).map(|_| rng.below(3) != 0).collect();
            let selected = key(&world.observe_selected(&mask).unwrap());
            world.set_parallel(true);
            pool.install(|| {
                assert_eq!(key(&world.observe_all()), all, "case {case}");
                assert_eq!(
                    key(&world.observe_selected(&mask).unwrap()),
                    selected,
                    "case {case}"
                );
                world.observe_all_into(&mut buffer);
                assert_eq!(key(&buffer), all, "case {case}");
                world.observe_selected_into(&mask, &mut buffer).unwrap();
                assert_eq!(key(&buffer), selected, "case {case}");
            });
        }
    }
}
