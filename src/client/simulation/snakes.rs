//! Snake report state, coverage candidates and ordered wire diffs.
use super::{DeliveryProfile, PlayerAppearance};
use crate::{
    client::encode,
    model::{Point, Snake},
    physics,
};
use std::{collections::HashMap, f64::consts::TAU};

/// Front anchors searched for a known snake's previous front. Contraction runs
/// before each insertion from index 3 on, so that front stays unchanged at
/// indices 1, 2 and 3 through three insertions and moves on the fourth: up to
/// `FRESH_ANCHORS - 1` new anchors encode incrementally, and more resend the
/// snake in full.
pub(super) const FRESH_ANCHORS: usize = 4;
/// Release rules from the [scheduler study] (2026-10-25; 1.93 million
/// retained frames): body anchors leave in the tick that makes them.
/// A snake's motion (heading, wanted heading,
/// speed) is reported when it changed, at most every six ~7 ms ticks: busy
/// gaps sit near 42 ms, a straight snake stays silent for seconds and one
/// snake's reports share a phase across recipients. Changed mass follows its
/// own ~105 ms opportunity. The exact comparators and rearm rules are not
/// identified; these minimum gaps are model choices. Clean [near-server data]
/// includes 24 ms motion, 84 ms mass and motion-before-mass counterexamples;
/// decoder regressions accept them without changing these release rules.
/// Own wanted-heading and boost changes wait for this gate too, with rare
/// exceptions ([wanted-gate check]).
///
/// [scheduler study]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-packet-scheduler/REPORT.md
/// [near-server data]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-vps-timing/REPORT.md
/// [wanted-gate check]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-wanted-gate/REPORT.md
pub(super) const MOTION_REPORT_TICKS: u64 = 6;
pub(super) const FAM_REPORT_TICKS: u64 = 15;

/// What one connection has been told about a snake, in wire terms.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct KnownSnake {
    pub(super) life: u64,
    pub(super) front: Point,
    pub(super) len: usize,
    pub(super) heading: u8,
    pub(super) wanted: u8,
    pub(super) speed: u8,
    pub(super) fam: u32,
    pub(super) kills: usize,
}

/// Reuse a delta only within one broadcast, for the same snake and identical
/// recipient history. Own-snake packets are never shared. Keeping just the
/// last history bounds storage even with divergent viewers.
#[derive(Clone, Default)]
pub(super) struct SnakeDeltaCache(Option<SnakeDelta>);

/// Packet caches and candidate storage are private to one broadcast. Recipients
/// finish using the candidate list before the next one reuses its allocation.
pub(super) struct SnakeBroadcast {
    pub(super) deltas: Vec<SnakeDeltaCache>,
    pub(super) candidates: Vec<usize>,
    pub(super) body_bounds: BodyBounds,
}

impl SnakeBroadcast {
    pub(super) fn new(snakes: &[Snake], mut body_bounds: BodyBounds) -> Self {
        body_bounds.update(snakes);
        Self {
            deltas: vec![SnakeDeltaCache::default(); snakes.len()],
            candidates: Vec::new(),
            body_bounds,
        }
    }
}

const BODY_BOUND_EDGES: usize = 8;
const BODY_BOUND_MIN_EDGES: usize = 16;
const BODY_BOUND_LIMIT: f64 = 1e7;

/// Body-only bounds, retained between broadcasts and verified against every
/// point before reuse. Anchor insertion contracts interior points too, so a
/// matching head/tail or length alone cannot validate this cache.
#[derive(Default)]
pub(super) struct BodyBounds {
    snakes: Vec<CachedBodyBounds>,
}

#[derive(Default)]
struct CachedBodyBounds {
    body: Vec<Point>,
    blocks: Vec<physics::Bounds>,
}

fn boundable(point: Point) -> bool {
    point.finite() && point.x.abs() <= BODY_BOUND_LIMIT && point.y.abs() <= BODY_BOUND_LIMIT
}

impl BodyBounds {
    #[cfg(test)]
    fn new(snakes: &[Snake]) -> Self {
        let mut result = Self::default();
        result.update(snakes);
        result
    }

    fn update(&mut self, snakes: &[Snake]) {
        let indexed = |snake: &Snake| snake.alive && snake.body.len() > BODY_BOUND_MIN_EDGES;
        if self.snakes.is_empty() && !snakes.iter().any(indexed) {
            return;
        }
        self.snakes
            .resize_with(snakes.len(), CachedBodyBounds::default);
        for (snake, cached) in snakes.iter().zip(&mut self.snakes) {
            if !indexed(snake) {
                cached.body.clear();
                cached.blocks.clear();
                continue;
            }
            // Every coordinate bit is the reuse witness: even signed-zero or
            // NaN-payload edits rebuild the complete set of blocks.
            let same_body =
                cached.body.len() == snake.body.len()
                    && cached.body.iter().zip(&snake.body).all(|(a, b)| {
                        a.x.to_bits() == b.x.to_bits() && a.y.to_bits() == b.y.to_bits()
                    });
            if same_body {
                continue;
            }
            cached.body.clone_from(&snake.body);
            cached.blocks.clear();
            for first in (0..snake.body.len() - 1).step_by(BODY_BOUND_EDGES) {
                let end = (first + BODY_BOUND_EDGES + 1).min(snake.body.len());
                let points = &snake.body[first..end];
                if !points.iter().copied().all(boundable) {
                    cached.blocks.clear();
                    break;
                }
                let mut bounds = physics::Bounds::between(points[0], points[0]);
                for &point in &points[1..] {
                    bounds.include(point);
                }
                cached.blocks.push(bounds);
            }
        }
    }

    pub(super) fn for_snake(&self, slot: usize) -> &[physics::Bounds] {
        self.snakes.get(slot).map_or(&[], |cached| &cached.blocks)
    }
}

#[derive(Clone)]
struct SnakeDelta {
    before: KnownSnake,
    after: KnownSnake,
    packets: Vec<Vec<u8>>,
}

impl SnakeDeltaCache {
    #[expect(clippy::too_many_arguments, reason = "Same explicit inputs as changes")]
    pub(super) fn changes(
        &mut self,
        packets: &mut Vec<Vec<u8>>,
        own: bool,
        id: u16,
        known: &mut KnownSnake,
        snake: &Snake,
        wanted: impl FnOnce() -> f64,
        profile: &PlayerAppearance,
        reports: &SnakeReports,
    ) -> Result<(), String> {
        // With no fresh anchor or length change, only open motion/mass gates
        // and kills can emit anything. The caller still observes this history
        // for stale reports, even when there are no packets to encode.
        if snake.body.first() == Some(&known.front)
            && known.len == snake.body.len()
            && known.kills == snake.kills
            && (!reports.motion_due || [known.heading, known.wanted, known.speed] == reports.motion)
            && (!reports.fam_due || known.fam == encode::fam_u24(snake.fam))
        {
            // Equality treats signed zeros alike; changes assigns the actual
            // front even in that case, so preserve its exact stored bits.
            known.front = snake.body[0];
            return Ok(());
        }
        if own {
            return changes(packets, own, id, known, snake, wanted(), profile, reports);
        }
        let entry = &mut self.0;
        if let Some(delta) = entry.as_ref().filter(|delta| delta.before == *known) {
            packets.extend_from_slice(&delta.packets);
            known.clone_from(&delta.after);
        } else {
            let before = known.clone();
            let start = packets.len();
            changes(packets, own, id, known, snake, wanted(), profile, reports)?;
            *entry = Some(SnakeDelta {
                before,
                after: known.clone(),
                packets: packets[start..].to_vec(),
            });
        }
        Ok(())
    }
}

/// One snake's report opportunities, shared by every recipient. Recipients
/// differ only in which fields they still lack.
#[derive(Clone, Debug, Default)]
pub(super) struct SnakeReports {
    pub(super) life: u64,
    /// Heading, wanted heading and speed bytes last reported.
    pub(super) motion: [u8; 3],
    pub(super) motion_at: u64,
    pub(super) fam: u32,
    pub(super) fam_at: u64,
    /// Reported in the current tick.
    pub(super) motion_due: bool,
    pub(super) fam_due: bool,
    /// Some recipient holds a value other than the last report, from an
    /// appearance or a growth/tail mass; the next opportunity reports even
    /// if the snake returned to that value.
    pub(super) motion_stale: bool,
    pub(super) fam_stale: bool,
}

impl KnownSnake {
    /// Shown in full, including its complete motion.
    pub(super) fn new(life: u64, snake: &Snake, wanted: f64) -> Result<Self, String> {
        Ok(Self {
            life,
            front: snake.body[0],
            len: snake.body.len(),
            heading: encode::angle_byte(snake.heading),
            wanted: encode::angle_byte(wanted),
            speed: encode::speed_byte(snake.speed)?,
            fam: encode::fam_u24(snake.fam),
            kills: snake.kills,
        })
    }
}

impl SnakeReports {
    /// Open this tick's opportunities for a living snake. A new life starts
    /// from the state its appearance carries.
    pub(super) fn update(
        &mut self,
        life: u64,
        motion: [u8; 3],
        fam: u32,
        tick: u64,
        delivery: DeliveryProfile,
    ) {
        if self.life != life {
            *self = Self {
                life,
                motion,
                motion_at: tick,
                fam,
                fam_at: tick,
                ..Self::default()
            };
            return;
        }
        // Our responsive design reflects wanted-heading/speed changes promptly;
        // the stock client already turns toward a held target between reports.
        // Standalone mass drives its smoothed length and own score display.
        // See the delivery-profiles study (2026-10-25), not an official rule:
        // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-delivery-profiles/REPORT.md
        let responsive = delivery == DeliveryProfile::Responsive;
        let urgent = responsive && motion[1..] != self.motion[1..];
        self.motion_due =
            (motion != self.motion || self.motion_stale) && (urgent || tick >= self.motion_at);
        if self.motion_due {
            self.motion = motion;
            self.motion_at = tick + MOTION_REPORT_TICKS;
            self.motion_stale = false;
        }
        self.fam_due = (fam != self.fam || self.fam_stale) && (responsive || tick >= self.fam_at);
        if self.fam_due {
            self.fam = fam;
            self.fam_at = tick + FAM_REPORT_TICKS;
            self.fam_stale = false;
        }
    }

    /// Note a recipient's view of this snake after its packets this tick.
    pub(super) fn observe(&mut self, known: &KnownSnake) {
        self.motion_stale |= [known.heading, known.wanted, known.speed] != self.motion;
        self.fam_stale |= known.fam != self.fam;
    }
}

/// What one connection has been told about each snake slot.
#[derive(Clone, Debug, Default)]
pub(super) struct KnownSnakes {
    /// Ascending occupied slots preserve wire order without an optional record
    /// for every world slot. The [diagnostic] found about seven occupied records
    /// per recipient, versus hundreds of reserved optional slots.
    ///
    /// [diagnostic]: https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-simulation-memory/REPORT.md
    entries: Vec<(usize, KnownSnake)>,
}
impl KnownSnakes {
    fn index(&self, slot: usize) -> Option<usize> {
        // Bound linear work for crowded views without reserving sparse slots.
        if self.entries.len() <= 8 {
            self.entries.iter().position(|(key, _)| *key == slot)
        } else {
            self.entries
                .binary_search_by_key(&slot, |(key, _)| *key)
                .ok()
        }
    }
    pub(super) fn contains_key(&self, slot: usize) -> bool {
        self.index(slot).is_some()
    }
    pub(super) fn get_mut(&mut self, slot: usize) -> Option<&mut KnownSnake> {
        let index = self.index(slot)?;
        self.entries.get_mut(index).map(|(_, known)| known)
    }
    pub(super) fn remove(&mut self, slot: usize) -> Option<KnownSnake> {
        let index = self.index(slot)?;
        Some(self.entries.remove(index).1)
    }
    pub(super) fn insert(&mut self, slot: usize, known: KnownSnake) {
        match self.entries.binary_search_by_key(&slot, |(key, _)| *key) {
            Ok(index) => self.entries[index].1 = known,
            Err(index) => self.entries.insert(index, (slot, known)),
        }
    }
    #[cfg(test)]
    pub(super) fn slots(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries.iter().map(|(key, _)| *key)
    }
    pub(super) fn candidates(&self, nearby: &[usize], out: &mut Vec<usize>) {
        out.clear();
        out.reserve(nearby.len() + self.entries.len());
        let mut known = self.entries.iter().map(|(key, _)| *key).peekable();
        for &slot in nearby {
            while let Some(slot) = known.next_if(|&known| known < slot) {
                out.push(slot);
            }
            if known.peek() == Some(&slot) {
                known.next();
            }
            out.push(slot);
        }
        out.extend(known);
    }
}

/// Living snakes listed by every grid cell their widened bounds overlap.
/// `covered` rejects a head outside a snake's bounds widened by the vision
/// reach, so the cell holding a head lists every snake that can cover it.
/// Testing all snakes for each of ~500 connections every tick cost ~2.3 ms.
#[derive(Default)]
pub(super) struct CoverageGrid {
    cell: f64,
    cells: HashMap<(i64, i64), Vec<usize>>,
    spans: Vec<Option<CoverageSpan>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CoverageSpan {
    first: (i64, i64),
    last: (i64, i64),
}

impl CoverageSpan {
    fn cells(self) -> impl Iterator<Item = (i64, i64)> {
        (self.first.0..=self.last.0)
            .flat_map(move |x| (self.first.1..=self.last.1).map(move |y| (x, y)))
    }

    fn contains(self, (x, y): (i64, i64)) -> bool {
        self.first.0 <= x && x <= self.last.0 && self.first.1 <= y && y <= self.last.1
    }
}
impl CoverageGrid {
    pub(super) fn new(snakes: &[Snake], bounds: &[[f64; 4]], vision: f64) -> Self {
        let mut grid = Self::default();
        grid.update(snakes, bounds, vision);
        grid
    }

    /// Cache only integer cell membership, never the exact coverage decision.
    /// Bounds are recomputed every broadcast, including diagnostic broadcasts
    /// and externally edited poses. Most moves keep the same widened cells.
    pub(super) fn update(&mut self, snakes: &[Snake], bounds: &[[f64; 4]], vision: f64) {
        let reach = vision + 1.;
        let cell = reach.max(256.);
        if self.cell.to_bits() != cell.to_bits() {
            self.cells.clear();
            self.spans.clear();
            self.cell = cell;
        }
        while self.spans.len() > bounds.len() {
            let slot = self.spans.len() - 1;
            let old = self.spans.pop().expect("coverage slot");
            self.change(slot, old, None);
        }
        self.spans.resize(bounds.len(), None);
        for (slot, &[x0, y0, x1, y1]) in bounds.iter().enumerate() {
            let next = snakes[slot].alive.then(|| CoverageSpan {
                first: self.key(x0 - reach, y0 - reach),
                last: self.key(x1 + reach, y1 + reach),
            });
            let old = self.spans[slot];
            if old == next {
                continue;
            }
            self.change(slot, old, next);
            self.spans[slot] = next;
        }
    }

    fn change(&mut self, slot: usize, old: Option<CoverageSpan>, next: Option<CoverageSpan>) {
        if let Some(old) = old {
            for key in old
                .cells()
                .filter(|&key| !next.is_some_and(|span| span.contains(key)))
            {
                let slots = self.cells.get_mut(&key).expect("coverage cell");
                let index = slots.binary_search(&slot).expect("coverage cell slot");
                slots.remove(index);
                // Do not retain cells left behind as snakes roam the world.
                if slots.is_empty() {
                    self.cells.remove(&key);
                }
            }
        }
        if let Some(next) = next {
            for key in next
                .cells()
                .filter(|&key| !old.is_some_and(|span| span.contains(key)))
            {
                let slots = self.cells.entry(key).or_default();
                // Fresh builds append slots in world order. Preserve that order
                // when an existing slot moves into an already occupied cell.
                let index = slots
                    .binary_search(&slot)
                    .expect_err("new coverage cell slot");
                slots.insert(index, slot);
            }
        }
    }

    fn key(&self, x: f64, y: f64) -> (i64, i64) {
        (
            (x / self.cell).floor() as i64,
            (y / self.cell).floor() as i64,
        )
    }

    /// Ascending slots of the snakes that may cover `point`.
    pub(super) fn near(&self, point: Point) -> &[usize] {
        self.cells
            .get(&self.key(point.x, point.y))
            .map_or(&[], Vec::as_slice)
    }
}

/// Coverage: any centerline part within `vision` of `head`.
pub(super) fn covered(
    head: Point,
    snake: &Snake,
    [x0, y0, x1, y1]: [f64; 4],
    vision: f64,
    body_bounds: &[physics::Bounds],
) -> bool {
    let reach = vision + 1.;
    if head.x < x0 - reach || head.x > x1 + reach || head.y < y0 - reach || head.y > y1 + reach {
        return false;
    }
    // Reuse the guarded comparison: clear cases avoid hypot, while boundaries
    // and extreme inputs retain the original result. Projection is unchanged:
    // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-broadcast-distance/REPORT.md
    if snake.head.distance_le(head, vision)
        || snake
            .body
            .first()
            .is_some_and(|&point| physics::segment_distance_le(head, snake.head, point, vision))
    {
        return true;
    }
    if body_bounds.is_empty() || !boundable(head) || !(0. ..=BODY_BOUND_LIMIT).contains(&vision) {
        return snake
            .body
            .windows(2)
            .any(|pair| physics::segment_distance_le(head, pair[0], pair[1], vision));
    }
    // As in the admission bounds, one unit safely exceeds projection/hypot
    // roundoff at this finite scale. This only skips impossible blocks; the
    // original predicate still decides visibility, in original edge order.
    // https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-mature-covered-bounds/REPORT.md
    let query = physics::Bounds::between(head, head);
    for (block, &bounds) in body_bounds.iter().enumerate() {
        if !bounds.near(query, reach) {
            continue;
        }
        // Within the existing finite safe domain, the same one-unit margin
        // conservatively encloses the exact projected distance at box corners.
        let dx = (bounds.min.x - head.x).max(head.x - bounds.max.x).max(0.);
        let dy = (bounds.min.y - head.y).max(head.y - bounds.max.y).max(0.);
        if dx * dx + dy * dy > reach * reach {
            continue;
        }
        let first = block * BODY_BOUND_EDGES;
        let end = (first + BODY_BOUND_EDGES + 1).min(snake.body.len());
        if snake.body[first..end]
            .windows(2)
            .any(|pair| physics::segment_distance_le(head, pair[0], pair[1], vision))
        {
            return true;
        }
    }
    false
}

/// Full snake, then its complete motion with the turn direction: the stock
/// appearance handler leaves the heading at the last body anchor's direction.
/// A nonzero kill count follows, since appearances do not carry it.
pub(super) fn full(
    packets: &mut Vec<Vec<u8>>,
    own: bool,
    id: u16,
    snake: &Snake,
    wanted: f64,
    profile: &PlayerAppearance,
) -> Result<(), String> {
    packets.push(encode::appearance(id, snake, wanted, profile)?);
    packets.extend(encode::motion(
        own,
        id,
        Some(encode::angle_byte(snake.heading)),
        Some(encode::angle_byte(wanted)),
        Some(encode::speed_byte(snake.speed)?),
        encode::direction(snake.heading, wanted),
    ));
    if snake.kills > 0 {
        packets.push(encode::kills(id, snake.kills));
    }
    Ok(())
}

/// Incremental body, mass and motion reports for a known snake. Body changes
/// go out in the tick that makes them; motion and standalone mass wait for the
/// snake's shared opportunities in `reports`. We adopt the common observed
/// order: tail/standalone mass, motion, then anchors. The scheduler study linked
/// from `MOTION_REPORT_TICKS` retains 27 reverse mass/motion pairs; this is not a
/// universal official sort.
#[expect(
    clippy::too_many_arguments,
    reason = "Keep wire inputs explicit without a per-snake context"
)]
pub(super) fn changes(
    packets: &mut Vec<Vec<u8>>,
    own: bool,
    id: u16,
    known: &mut KnownSnake,
    snake: &Snake,
    wanted: f64,
    profile: &PlayerAppearance,
    reports: &SnakeReports,
) -> Result<(), String> {
    let body = &snake.body;
    let fam = encode::fam_u24(snake.fam);
    let mut fam_sent = false;
    let change = body.len() as isize - known.len as isize;
    match body
        .iter()
        .take(FRESH_ANCHORS)
        .position(|&point| point == known.front)
        .filter(|&fresh| change <= fresh as isize)
    {
        Some(fresh) => {
            // Growth and tail removal commute geometrically: contraction never
            // reads the tail, so report removals first, then the new anchors.
            let grown = change.clamp(0, fresh as isize);
            for _ in 0..grown - change {
                packets.push(encode::tail(id, fam));
                fam_sent = true;
            }
            // Growth anchors carry the mass themselves.
            if !fam_sent && grown == 0 && reports.fam_due && known.fam != fam {
                packets.push(encode::fam(id, fam));
                fam_sent = true;
            }
            if reports.motion_due {
                let [heading, target, speed] = reports.motion;
                packets.extend(encode::motion(
                    own,
                    id,
                    (heading != known.heading).then_some(heading),
                    (target != known.wanted).then_some(target),
                    (speed != known.speed).then_some(speed),
                    encode::direction(snake.heading, wanted),
                ));
                known.heading = heading;
                known.wanted = target;
                known.speed = speed;
            }
            for j in (0..fresh).rev() {
                let grows = (j as isize) < grown;
                let (point, previous) = (body[j], body[j + 1]);
                let angle = encode::angle_u16((point.y - previous.y).atan2(point.x - previous.x));
                let radians = f64::from(angle) * TAU / 65536.;
                let step = Point {
                    x: previous.x + radians.cos() * physics::ANCHOR_SPACING,
                    y: previous.y + radians.sin() * physics::ANCHOR_SPACING,
                };
                let carried = grows.then_some(fam);
                fam_sent |= grows;
                packets.push(if step.distance(point) < 1e-6 {
                    encode::anchor(own, id, angle, carried)
                } else {
                    encode::anchor_at(own, id, angle, point, carried)?
                });
            }
        }
        None => {
            // Unrecognizable continuity (e.g. an externally edited body): resend.
            full(packets, own, id, snake, wanted, profile)?;
            *known = KnownSnake::new(known.life, snake, wanted)?;
            return Ok(());
        }
    }
    known.front = body[0];
    known.len = body.len();
    if fam_sent {
        known.fam = fam;
    }
    if known.kills != snake.kills {
        known.kills = snake.kills;
        packets.push(encode::kills(id, snake.kills));
    }
    Ok(())
}

#[cfg(test)]
mod known_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, Simulator};

    fn original_segment_distance(p: Point, a: Point, b: Point) -> f64 {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        let denom = dx * dx + dy * dy;
        let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / if denom == 0.0 { 1.0 } else { denom })
            .clamp(0.0, 1.0);
        p.distance(Point {
            x: a.x + t * dx,
            y: a.y + t * dy,
        })
    }

    fn original_covered(head: Point, snake: &Snake, b: [f64; 4], vision: f64) -> bool {
        let [x0, y0, x1, y1] = b;
        let reach = vision + 1.;
        if head.x < x0 - reach || head.x > x1 + reach || head.y < y0 - reach || head.y > y1 + reach
        {
            return false;
        }
        snake.head.distance(head) <= vision
            || snake
                .body
                .first()
                .is_some_and(|&p| original_segment_distance(head, snake.head, p) <= vision)
            || snake
                .body
                .windows(2)
                .any(|p| original_segment_distance(head, p[0], p[1]) <= vision)
    }

    fn coverage_bounds(snake: &Snake) -> [f64; 4] {
        snake.body.iter().fold(
            [snake.head.x, snake.head.y, snake.head.x, snake.head.y],
            |[x0, y0, x1, y1], p| [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)],
        )
    }

    #[test]
    fn body_bounds_match_original_coverage() {
        let world = Simulator::new(crate::WorldOptions {
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["template".into()],
                placement: crate::Placement::TestRing,
            },
            ..Default::default()
        })
        .unwrap();
        let mut snake = world.snakes()[0].clone();
        // The middle and final blocks are visible without the head/front edge.
        // Adjacent radii keep the exact inclusive boundary independently of
        // the index, including a long edge crossing far from both endpoints.
        snake.head = Point {
            x: -10000.,
            y: 10001.,
        };
        snake.body = (0..34)
            .map(|i| {
                if i < 16 {
                    Point {
                        x: -10000. + i as f64,
                        y: 10000.,
                    }
                } else {
                    Point {
                        x: 0.,
                        y: (i as f64 - 24.) * 100.,
                    }
                }
            })
            .collect();
        let check = |snake: &Snake, p: Point, radii: &[f64]| {
            let index = BodyBounds::new(std::slice::from_ref(snake));
            let bounds = coverage_bounds(snake);
            for &r in radii {
                assert_eq!(
                    covered(p, snake, bounds, r, index.for_snake(0)),
                    original_covered(p, snake, bounds, r),
                    "p={p:?}, radius={r:?}, snake={snake:?}"
                );
            }
        };
        for y in [-800., -750., 0., 850., 900.] {
            check(
                &snake,
                Point { x: 500., y },
                &[500f64.next_down(), 500., 500f64.next_up()],
            );
        }
        // Box corners: the nearest endpoint is exactly a 3-4-5 distance
        // away, including the last partial block. Keep adjacent radii exact.
        for head in [Point { x: 300., y: -1200. }, Point { x: 300., y: 1300. }] {
            check(&snake, head, &[500f64.next_down(), 500., 500f64.next_up()]);
        }
        let mut state = 0x4d15_fa32_1c07_u64;
        let mut coordinate = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            ((state >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 40000.
        };
        for scene in 0..2048 {
            let len = [0, 1, 2, 8, 9, 16, 17, 18, 31, 33, 65, 257][scene % 12];
            snake.head = Point {
                x: coordinate(),
                y: coordinate(),
            };
            snake.body = (0..len)
                .map(|_| Point {
                    x: coordinate(),
                    y: coordinate(),
                })
                .collect();
            for _ in 0..16 {
                let p = Point {
                    x: coordinate(),
                    y: coordinate(),
                };
                let r = snake
                    .body
                    .windows(2)
                    .next()
                    .map_or(0., |edge| original_segment_distance(p, edge[0], edge[1]));
                check(&snake, p, &[0., 500., r.next_down(), r, r.next_up()]);
            }
        }
        let special = [
            0.,
            -0.,
            f64::MIN_POSITIVE,
            1e-200,
            1e-9,
            BODY_BOUND_LIMIT.next_down(),
            BODY_BOUND_LIMIT,
            BODY_BOUND_LIMIT.next_up(),
            1e150,
            1e200,
            f64::MAX,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];
        for x in special {
            for y in special {
                snake.head = Point { x: 0., y: 0. };
                snake.body = vec![Point { x, y }; 34];
                snake.body[17] = Point { x: 42., y: -42. };
                for p in [Point { x, y }, Point { x: 42., y: -42. }, Point::default()] {
                    check(
                        &snake,
                        p,
                        &[
                            -1.,
                            -0.,
                            0.,
                            1e-200,
                            42.,
                            500.,
                            1e150,
                            f64::INFINITY,
                            f64::NAN,
                        ],
                    );
                }
            }
        }
        // Separate broadcasts can replace, shrink, retire and reuse slots.
        let mut snakes = vec![snake.clone(); 3];
        for count in [3, 1, 0, 4] {
            snakes.resize(count, snake.clone());
            for (slot, s) in snakes.iter_mut().enumerate() {
                s.alive = slot % 2 == 0;
                s.head = Point {
                    x: 10000.,
                    y: 10000.,
                };
                s.body = vec![Point::default(); 17 + slot];
                check(s, Point { x: 500., y: 0. }, &[500.]);
            }
            let index = BodyBounds::new(&snakes);
            for (slot, s) in snakes.iter().enumerate() {
                assert_eq!(
                    covered(
                        Point::default(),
                        s,
                        coverage_bounds(s),
                        0.,
                        index.for_snake(slot)
                    ),
                    original_covered(Point::default(), s, coverage_bounds(s), 0.)
                );
            }
        }
    }

    #[test]
    fn retained_body_bounds_follow_every_body_edit() {
        let world = Simulator::new(crate::WorldOptions {
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["template".into()],
                placement: crate::Placement::TestRing,
            },
            ..Default::default()
        })
        .unwrap();
        let mut snakes = vec![world.snakes()[0].clone(); 3];
        let mut cache = BodyBounds::default();
        for scene in 0..256 {
            let count = [3, 1, 0, 4][scene % 4];
            snakes.resize(count, world.snakes()[0].clone());
            for (slot, snake) in snakes.iter_mut().enumerate() {
                snake.alive = (scene + slot) % 7 != 0;
                snake.head = Point {
                    x: 10000.,
                    y: -10000.,
                };
                snake.body = (0..[0, 2, 16, 17, 33, 65][scene % 6])
                    .map(|i| Point {
                        x: (i as f64 - 16.) * 40.,
                        y: slot as f64 * 500.,
                    })
                    .collect();
            }
            // Reuse, then edit a single interior point without changing length
            // or the front/tail. A head/tail-only key would miss this change.
            for edit in 0..4 {
                if edit == 2 {
                    for snake in &mut snakes {
                        if snake.body.len() > 16 {
                            snake.body[9] = Point { x: 500., y: 500. };
                        }
                    }
                } else if edit == 3 {
                    for snake in &mut snakes {
                        if let Some(point) = snake.body.get_mut(9) {
                            *point = Point {
                                x: [0., -0., f64::NAN, 1e200][scene % 4],
                                y: 0.,
                            };
                        }
                    }
                }
                cache.update(&snakes);
                for (slot, snake) in snakes.iter().enumerate() {
                    let bounds = coverage_bounds(snake);
                    for head in [Point::default(), Point { x: 500., y: 500. }, snake.head] {
                        for radius in [0., 1., 500f64.next_down(), 500., 500f64.next_up(), 1e200] {
                            assert_eq!(
                                covered(head, snake, bounds, radius, cache.for_snake(slot)),
                                original_covered(head, snake, bounds, radius),
                                "scene={scene}, slot={slot}, edit={edit}, head={head:?}, radius={radius}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn retained_body_bounds_compare_each_coordinate() {
        let world = Simulator::new(crate::WorldOptions {
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["template".into()],
                placement: crate::Placement::TestRing,
            },
            ..Default::default()
        })
        .unwrap();
        let mut snake = world.snakes()[0].clone();
        snake.head = Point {
            x: -10000.,
            y: -10000.,
        };
        let mut cache = BodyBounds::default();
        for axis in 0..2 {
            for index in 0..65 {
                let far = if axis == 0 {
                    Point { x: 1000., y: 0. }
                } else {
                    Point { x: 0., y: 1000. }
                };
                snake.body = vec![far; 65];
                for changed in [false, true, true, false] {
                    snake.body[index] = if changed { Point::default() } else { far };
                    cache.update(std::slice::from_ref(&snake));
                    let bounds = coverage_bounds(&snake);
                    for radius in [0., 1., 500f64.next_down(), 500., 500f64.next_up()] {
                        assert_eq!(
                            covered(Point::default(), &snake, bounds, radius, cache.for_snake(0)),
                            original_covered(Point::default(), &snake, bounds, radius),
                            "axis={axis}, index={index}, changed={changed}, radius={radius}"
                        );
                    }
                    // The stored witness must include every edit, even when
                    // distinct bit patterns describe the same geometric point.
                    assert_eq!(
                        cache.snakes[0].body[index].x.to_bits(),
                        snake.body[index].x.to_bits()
                    );
                    assert_eq!(
                        cache.snakes[0].body[index].y.to_bits(),
                        snake.body[index].y.to_bits()
                    );
                }
            }
        }
        snake.body = vec![Point::default(); 65];
        for bits in [
            0u64,
            1u64 << 63,
            0x7ff8_0000_0000_0001,
            0x7ff8_0000_0000_0002,
            0,
        ] {
            snake.body[9].x = f64::from_bits(bits);
            cache.update(std::slice::from_ref(&snake));
            assert_eq!(cache.snakes[0].body[9].x.to_bits(), bits);
            let bounds = coverage_bounds(&snake);
            assert_eq!(
                covered(Point::default(), &snake, bounds, 1., cache.for_snake(0)),
                original_covered(Point::default(), &snake, bounds, 1.)
            );
        }
    }

    fn fresh_coverage_cells(
        snakes: &[Snake],
        bounds: &[[f64; 4]],
        vision: f64,
    ) -> HashMap<(i64, i64), Vec<usize>> {
        let reach = vision + 1.;
        let cell = reach.max(256.);
        let key = |x: f64, y: f64| ((x / cell).floor() as i64, (y / cell).floor() as i64);
        let mut cells: HashMap<_, Vec<_>> = HashMap::new();
        for (slot, &[x0, y0, x1, y1]) in bounds.iter().enumerate() {
            if snakes[slot].alive {
                let (left, top) = key(x0 - reach, y0 - reach);
                let (right, bottom) = key(x1 + reach, y1 + reach);
                for x in left..=right {
                    for y in top..=bottom {
                        cells.entry((x, y)).or_default().push(slot);
                    }
                }
            }
        }
        cells
    }

    #[test]
    fn incremental_coverage_matches_fresh_world_order_through_moves_and_lifetimes() {
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["template".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let template = world.snakes()[0].clone();
        let mut grid = CoverageGrid::default();
        for step in 0..512 {
            let count = [0, 1, 12, 7, 3, 9, 2, 0][step / 32 % 8];
            let mut snakes = vec![template.clone(); count];
            let vision = [0., 42., 255., 255.5, 1500., f64::INFINITY, f64::NAN][step / 16 % 7];
            let bounds: Vec<_> = (0..count)
                .map(|slot| {
                    snakes[slot].alive = (step / 4 + slot) % 5 != 0;
                    let x = ((slot * 73 + step / 4 * 17) % 129) as f64 * 128. - 8192.;
                    let y = ((slot * 31 + step / 4 * 11) % 101) as f64 * 128. - 6400.;
                    let width = ((slot + step / 4) % 4) as f64 * 256.;
                    let height = ((slot * 3 + step / 4) % 3) as f64 * 256.;
                    [x, y, x + width, y + height]
                })
                .collect();
            grid.update(&snakes, &bounds, vision);
            let expected = fresh_coverage_cells(&snakes, &bounds, vision);
            assert_eq!(grid.cells, expected, "step {step}");
            assert_eq!(grid.spans.len(), count);
            let cell = (vision + 1.).max(256.);
            for point in [
                Point { x: 0., y: -0. },
                Point { x: -256., y: 256. },
                Point {
                    x: 4096.,
                    y: -4096.,
                },
                Point {
                    x: f64::INFINITY,
                    y: f64::NAN,
                },
            ] {
                let key = (
                    (point.x / cell).floor() as i64,
                    (point.y / cell).floor() as i64,
                );
                assert_eq!(
                    grid.near(point),
                    expected.get(&key).map_or(&[][..], Vec::as_slice)
                );
            }
        }
    }

    #[test]
    fn incremental_coverage_preserves_float_cell_boundaries_and_discards_empty_cells() {
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["template".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut snakes = vec![world.snakes()[0].clone(); 3];
        let mut grid = CoverageGrid::default();
        for x in [
            (-512_f64).next_up(),
            -512.,
            (-512_f64).next_down(),
            -0.,
            0.,
            256_f64.next_down(),
            256.,
            256_f64.next_up(),
            f64::MAX,
            f64::MIN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            let bounds = [[x, 0., x, 0.]; 3];
            for active in 0..8 {
                for (slot, snake) in snakes.iter_mut().enumerate() {
                    snake.alive = active & (1 << slot) != 0;
                }
                grid.update(&snakes, &bounds, 0.);
                assert_eq!(grid.cells, fresh_coverage_cells(&snakes, &bounds, 0.));
                assert!(grid.cells.values().all(|slots| !slots.is_empty()));
            }
        }
        grid.update(&[], &[], 1500.);
        assert!(grid.cells.is_empty());
        assert!(grid.near(Point::default()).is_empty());
    }

    #[test]
    fn candidate_merge_matches_sorted_union_and_reuses_storage() {
        let slots = [0, 1, 2, 7, 31, 63, 64, 500];
        let snake = KnownSnake {
            life: 1,
            front: Point::default(),
            len: 1,
            heading: 0,
            wanted: 0,
            speed: 0,
            fam: 0,
            kills: 0,
        };
        let mut out = Vec::with_capacity(2 * slots.len());
        let allocation = out.as_ptr();
        for shown in 0..1usize << slots.len() {
            let mut known = KnownSnakes::default();
            for (bit, &slot) in slots.iter().enumerate().rev() {
                if shown & (1 << bit) != 0 {
                    known.insert(slot, snake.clone());
                }
            }
            for nearby in 0..1usize << slots.len() {
                let nearby: Vec<_> = slots
                    .iter()
                    .enumerate()
                    .filter_map(|(bit, &slot)| (nearby & (1 << bit) != 0).then_some(slot))
                    .collect();
                let mut expected = nearby.clone();
                expected.extend(known.slots());
                expected.sort_unstable();
                expected.dedup();
                known.candidates(&nearby, &mut out);
                assert_eq!(out, expected);
                assert_eq!(out.as_ptr(), allocation);
            }
        }
    }

    #[test]
    fn known_slots_match_ordered_map_through_replacement_removal_and_reuse() {
        let mut known = KnownSnakes::default();
        let mut reference = std::collections::BTreeMap::new();
        let snake = |life| KnownSnake {
            life,
            front: Point::default(),
            len: 1,
            heading: 0,
            wanted: 0,
            speed: 0,
            fam: 0,
            kills: 0,
        };
        assert!(known.remove(500).is_none());
        // Insert out of order, replace, remove twice, then reuse a high slot.
        for (slot, insert) in [
            (500, true),
            (2, true),
            (27, true),
            (2, true),
            (27, false),
            (27, false),
            (500, false),
            (500, true),
            (0, true),
            (2, false),
        ] {
            if insert {
                let value = snake(slot as u64 + reference.len() as u64);
                known.insert(slot, value.clone());
                reference.insert(slot, value);
            } else {
                assert_eq!(known.remove(slot), reference.remove(&slot));
            }
            assert_eq!(
                known.slots().collect::<Vec<_>>(),
                reference.keys().copied().collect::<Vec<_>>()
            );
            for slot in 0..=501 {
                assert_eq!(known.contains_key(slot), reference.contains_key(&slot));
                assert_eq!(known.get_mut(slot).as_deref(), reference.get(&slot));
            }
        }
        let mut cloned = known.clone();
        for slot in known.slots() {
            cloned.get_mut(slot).unwrap().kills += 1;
            assert_eq!(
                cloned.remove(slot).unwrap().kills,
                reference[&slot].kills + 1
            );
        }
        assert!(cloned.slots().next().is_none());
        assert_eq!(known.slots().collect::<Vec<_>>(), [0, 500]);
    }

    #[test]
    fn shared_deltas_preserve_distinct_histories_and_own_packet_types() {
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["snake".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut detached_snake = world.snakes()[0].clone();
        let snake = &mut detached_snake;
        let before = KnownSnake::new(2, snake, 0.).unwrap();
        let next = Point {
            x: snake.body[0].x + 42.,
            y: snake.body[0].y,
        };
        physics::append_anchor(&mut snake.body, next, false);
        snake.head = next;
        snake.fam = 0.5;
        snake.kills = 3;
        let reports = SnakeReports {
            motion: [7, 9, 13],
            motion_due: true,
            fam_due: true,
            ..Default::default()
        };
        let mut cache = SnakeDeltaCache::default();
        let mut histories = vec![before.clone()];
        let mut stale = before.clone();
        stale.heading = 7;
        stale.fam = encode::fam_u24(0.5);
        histories.push(stale);
        let mut missing_front = before;
        missing_front.front = Point { x: 1e6, y: 1e6 };
        histories.push(missing_front);
        for history in histories {
            // Repeat peer encodings to exercise hits, with an own update between.
            for own in [false, false, true, false] {
                let (mut expected, mut actual) = (vec![vec![99]], vec![vec![99]]);
                let (mut reference, mut shared) = (history.clone(), history.clone());
                changes(
                    &mut expected,
                    own,
                    4,
                    &mut reference,
                    snake,
                    0.7,
                    &PlayerAppearance::default(),
                    &reports,
                )
                .unwrap();
                cache
                    .changes(
                        &mut actual,
                        own,
                        4,
                        &mut shared,
                        snake,
                        || 0.7,
                        &PlayerAppearance::default(),
                        &reports,
                    )
                    .unwrap();
                assert_eq!(actual, expected);
                assert_eq!(shared, reference);
            }
        }
    }

    #[test]
    fn unchanged_updates_match_uncached_packets_and_history() {
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["snake".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut snake = world.snakes()[0].clone();
        snake.fam = 0.5;
        let initial = KnownSnake::new(2, &snake, 0.7).unwrap();
        let profile = PlayerAppearance::default();
        for gates in 0..4 {
            let reports = SnakeReports {
                motion: [7, 9, 13],
                motion_due: gates & 1 != 0,
                fam_due: gates & 2 != 0,
                ..Default::default()
            };
            let mut cache = SnakeDeltaCache::default();
            for front in [snake.body[0], snake.body[1], Point { x: 1e6, y: 1e6 }] {
                for offset in [-1, 0, 1] {
                    for mask in 0..8u8 {
                        for fam in [encode::fam_u24(snake.fam), encode::fam_u24(snake.fam) ^ 1] {
                            for kills in [snake.kills, snake.kills + 1] {
                                let history = KnownSnake {
                                    front,
                                    len: snake.body.len().saturating_add_signed(offset),
                                    heading: reports.motion[0] ^ (mask & 1),
                                    wanted: reports.motion[1] ^ ((mask >> 1) & 1),
                                    speed: reports.motion[2] ^ ((mask >> 2) & 1),
                                    fam,
                                    kills,
                                    ..initial.clone()
                                };
                                for own in [false, false, true, false] {
                                    let (mut expected, mut actual) =
                                        (vec![vec![99]], vec![vec![99]]);
                                    let (mut reference, mut shared) =
                                        (history.clone(), history.clone());
                                    let result = changes(
                                        &mut expected,
                                        own,
                                        4,
                                        &mut reference,
                                        &snake,
                                        0.7,
                                        &profile,
                                        &reports,
                                    );
                                    assert_eq!(
                                        cache.changes(
                                            &mut actual,
                                            own,
                                            4,
                                            &mut shared,
                                            &snake,
                                            || 0.7,
                                            &profile,
                                            &reports
                                        ),
                                        result
                                    );
                                    assert_eq!(actual, expected);
                                    assert_eq!(shared, reference);
                                    assert_eq!(
                                        shared.front.x.to_bits(),
                                        reference.front.x.to_bits()
                                    );
                                    assert_eq!(
                                        shared.front.y.to_bits(),
                                        reference.front.y.to_bits()
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn unchanged_updates_preserve_signed_zero_and_defer_wanted() {
        use std::cell::Cell;
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["snake".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut snake = world.snakes()[0].clone();
        snake.body[0] = Point { x: -0., y: 0. };
        let mut known = KnownSnake::new(2, &snake, 0.7).unwrap();
        known.front = Point { x: 0., y: -0. };
        let profile = PlayerAppearance::default();
        let reports = SnakeReports::default();
        let mut cache = SnakeDeltaCache::default();
        for own in [false, true] {
            let mut packets = vec![vec![99]];
            cache
                .changes(
                    &mut packets,
                    own,
                    4,
                    &mut known,
                    &snake,
                    || panic!("no encoding needs wanted"),
                    &profile,
                    &reports,
                )
                .unwrap();
            assert_eq!(packets, [vec![99]]);
            assert_eq!(known.front.x.to_bits(), snake.body[0].x.to_bits());
            assert_eq!(known.front.y.to_bits(), snake.body[0].y.to_bits());
        }
        let before = known;
        let next = Point { x: 42., y: 0. };
        physics::append_anchor(&mut snake.body, next, false);
        let calls = Cell::new(0);
        for (own, expected_calls) in [(false, 1), (false, 1), (true, 2), (false, 2)] {
            let mut known = before.clone();
            cache
                .changes(
                    &mut Vec::new(),
                    own,
                    4,
                    &mut known,
                    &snake,
                    || {
                        calls.set(calls.get() + 1);
                        0.7
                    },
                    &profile,
                    &reports,
                )
                .unwrap();
            assert_eq!(calls.get(), expected_calls);
        }
    }

    #[test]
    fn skipped_encoding_retains_existing_error_and_partial_state_behavior() {
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["snake".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let original = &world.snakes()[0];
        let history = KnownSnake::new(2, original, 0.7).unwrap();
        for length in [0, 1] {
            let mut snake = original.clone();
            snake.body.truncate(length);
            for own in [false, true] {
                let mut cache = SnakeDeltaCache::default();
                for _ in 0..2 {
                    let (mut reference, mut shared) = (history.clone(), history.clone());
                    let (mut expected, mut actual) = (vec![vec![99]], vec![vec![99]]);
                    let result = changes(
                        &mut expected,
                        own,
                        4,
                        &mut reference,
                        &snake,
                        0.7,
                        &PlayerAppearance::default(),
                        &SnakeReports::default(),
                    );
                    assert_eq!(
                        cache.changes(
                            &mut actual,
                            own,
                            4,
                            &mut shared,
                            &snake,
                            || 0.7,
                            &PlayerAppearance::default(),
                            &SnakeReports::default()
                        ),
                        result
                    );
                    assert_eq!(actual, expected);
                    assert_eq!(shared, reference);
                }
            }
        }
    }
}
