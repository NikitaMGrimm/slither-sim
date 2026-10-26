//! Visibility, independent of stepping, rewards, and transport.
use crate::{Constraint, ErrorKind, SimError};
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
            time: world.time(),
            snakes: world.snakes(),
            food: world.food(),
            prey: world.prey(),
            radius: world.radius(),
            vision: world.config().vision,
        }
    }
    /// Observation of one slot, without the batch index.
    pub(crate) fn observe(self, index: usize) -> Result<Observation, SimError> {
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
    cell: f64,
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
    fn new(food: &[Food], vision: f64, observers: usize) -> Self {
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
        let placed = food.len() - outliers.len();
        let (width, height) = (bounds.max.x - bounds.min.x, bounds.max.y - bounds.min.y);
        let cap = (2 * placed + 16) as f64;
        let shape = |mut cell: f64| loop {
            let cols = (width / cell).floor() + 1.0;
            let rows = (height / cell).floor() + 1.0;
            if cols * rows <= cap {
                break (cell, cols, rows);
            }
            cell *= 2.0;
        };
        let coarse = shape((vision / 2.0).max(1.0));
        let fine = shape((vision / 8.0).max(1.0));
        // Fine cells let each queried row follow the vision circle: about 1.15
        // candidates per visible pellet in 96- and 128-snake worlds, against 1.7
        // to 1.95 with half-radius cells and the full query box. Each extra cell
        // costs about 1.2 ns to build, and each skipped candidate saves about 2 ns.
        // Per vision^2 of mean food density, an observer skipped 3.1 candidates in
        // official-128, 1.4 in dome-proxy-96 and 2.0 in observe-sparse; the rule
        // conservatively assumes 1.4, the lowest of those measurements. Refine
        // only when the savings, at 2 (from 2.3) cells per candidate, cover the
        // extra cells. Mean density misjudges food that is dense only away from
        // the observers: observe-clustered refines and is about 12% slower. Food
        // narrower than the vision radius spans too few fine cells to trim. See
        // research/performance/2026-10-24-observation-food-index.
        let extra = fine.1 * fine.2 - coarse.1 * coarse.2;
        let skipped = observers as f64 * placed as f64 / (width * height) * vision * vision;
        let refine = width.min(height) >= vision && 2.0 * skipped > extra;
        let (cell, cols, rows) = if refine { fine } else { coarse };
        let (cols, rows) = (cols as usize, rows as usize);
        let mut grid = Self {
            origin: bounds.min,
            cell,
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
                .then(|| FoodGrid::new(world.food, world.vision, observers)),
        });
        Self { world, index }
    }
    /// Refill `slot` for `index`, reusing its vectors. `marks` is empty or has one
    /// zeroed bit per food item; it is zeroed again on return.
    fn observe_into(
        &self,
        index: usize,
        marks: &mut Vec<u64>,
        slot: &mut Option<Observation>,
    ) -> Result<(), SimError> {
        let w = self.world;
        let me = w.snakes.get(index).ok_or_else(|| {
            SimError::new(ErrorKind::UnknownSlot, "Unknown agent slot").at_slot(index)
        })?;
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
    ) -> Result<(), SimError> {
        let w = self.world;
        let me = w.snakes.get(index).ok_or_else(|| {
            SimError::new(ErrorKind::UnknownSlot, "Unknown agent slot").at_slot(index)
        })?;
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
                for row in grid.row(b.min.y)..=grid.row(b.max.y) {
                    // Only a chord of the circle can be visible in this row. Moving
                    // the row a unit closer and using `reach` as the radius leave the
                    // squared chord a margin above 2 * vision, far beyond its rounding
                    // error for tame values, and the added unit covers the rest.
                    let top = grid.origin.y + row as f64 * grid.cell;
                    let dy = (top - me.head.y)
                        .max(me.head.y - (top + grid.cell))
                        .max(1.0)
                        - 1.0;
                    let half = (reach * reach - dy * dy).max(0.0).sqrt() + 1.0;
                    let (lo, hi) = (grid.col(me.head.x - half), grid.col(me.head.x + half));
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
    pub fn observe(&self, index: usize) -> Result<Observation, SimError> {
        VisibleState::of(self).observe(index)
    }
    /// Detached observations; indexes live only for this immutable borrow of world state.
    pub fn observe_all(&self) -> Vec<Option<Observation>> {
        let mut observations = Vec::new();
        self.observe_all_into(&mut observations);
        observations
    }
    /// [`observe_all`](Self::observe_all) into `observations`, reusing its vectors
    /// to keep allocation low when observing every decision.
    pub fn observe_all_into(&self, observations: &mut Vec<Option<Observation>>) {
        self.observe_matching(None, observations)
            .expect("observations of existing slots cannot fail")
    }
    /// Materialize selected living slots using one shared observation index.
    /// The mask and result have one entry per snake; dead and unselected slots are None.
    pub fn observe_selected(
        &self,
        selected: &[bool],
    ) -> Result<Vec<Option<Observation>>, SimError> {
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
    ) -> Result<(), SimError> {
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
        sightings.truncate(self.snakes().len());
        sightings.resize_with(self.snakes().len(), || None);
        let observer = Observer::new(VisibleState::of(self), self.living());
        for (i, slot) in sightings.iter_mut().enumerate() {
            if !self.snakes()[i].alive {
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
    ) -> Result<(), SimError> {
        if selected.is_some_and(|flags| flags.len() != self.snakes().len()) {
            return Err(SimError::state(
                Constraint::LengthMismatch,
                "selected",
                "One selection flag per agent slot required",
            ));
        }
        let wanted: Vec<bool> = (0..self.snakes().len())
            .map(|i| self.snakes()[i].alive && selected.is_none_or(|flags| flags[i]))
            .collect();
        let observers = wanted.iter().filter(|&&w| w).count();
        // A dead-only or empty selection needs stable result slots, but no index.
        if observers == 0 {
            observations.clear();
            observations.resize_with(self.snakes().len(), || None);
            return Ok(());
        }
        observations.truncate(self.snakes().len());
        observations.resize_with(self.snakes().len(), || None);
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
            Ok::<_, SimError>(())
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
        observe_range(0..self.snakes().len(), observations)
    }
}

#[cfg(test)]
mod tests {
    use crate::Food;
    /// The `observe-sparse` lattice: refining it adds about 460,000 cells, which
    /// eight observers cannot repay but thousands can.
    #[test]
    fn food_grid_refines_only_when_observers_repay_the_cells() {
        let step = 131_250.0 / 511.0;
        let food: Vec<Food> = (0..512 * 512)
            .map(|i| Food {
                x: -65_625.0 + step * (i % 512) as f64,
                y: -65_625.0 + step * (i / 512) as f64,
                value: 1.0,
                size: None,
                origin: None,
            })
            .collect();
        assert_eq!(super::FoodGrid::new(&food, 1500.0, 8).cell, 750.0);
        assert_eq!(super::FoodGrid::new(&food, 1500.0, 8192).cell, 187.5);
    }
}
