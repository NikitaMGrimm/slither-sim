//! World-thread host bots. `center` and `follow` slots never connect, decode,
//! receive frames or model latency: the world thread respawns and steers them
//! from world state, so they cost only their steering and physics.
//!
//! Follow bots crash into the owner's body, anywhere along it, including
//! right behind the head. Avoidance is steering only: collisions, death food
//! and every other rule stay the same for everyone. Planning targets the
//! owner's steady states: one heading, or one wanted turn rate, held at one
//! speed. It checks a fitted head path until a predicted strike or a reserved
//! body lifetime. Neither a predicted strike nor the growth reserve is an
//! unconditional disappearance bound, so eventual safety remains unproven.
//! Bodies laid before settling, changes of state and arena-rim sliding add
//! further limits; see
//! docs/hosting.md#what-is-and-isnt-guaranteed. Synthetic regressions live in
//! simulation/host_bot_tests.rs.
use crate::{Point, Simulator, model::Snake, physics};
use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    f64::consts::{PI, TAU},
    ops::Range,
};

/// What the host's bot slots do.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BotBehavior {
    /// A4 food seekers: protocol clients of their slots on bot worker threads.
    #[default]
    Forager,
    /// World-thread bots that steer toward the arena center.
    Center,
    /// World-thread bots that crash into the owner's body; `center` while no
    /// owner is alive.
    Follow,
}

// Engineering margins, not fitted mechanics. Lengths are world units.
/// Fixed tube margin beyond the real contact distance. At a bot's tightest
/// turn the chord between its anchors sags about 5 units. The fit tests
/// measure selected prediction errors against the remaining budget.
const MARGIN: f64 = physics::ANCHOR_SPACING / 4.;
/// The owner's circle is fitted to the most recent stretch of history that
/// turned at least this far, so even a gentle turn spans many quantized
/// heading steps...
pub(super) const FIT_TURN: f64 = 1.;
/// ...but at most this much history. This covers the roughly 104-second
/// zero-pending growth reserve of a newborn. Pending mass or larger bots can
/// extend the planning horizon beyond it; this cap is not an error bound.
pub(super) const FIT_SECONDS: f64 = 105.;
/// One wire heading step: quantized steering turns gently in these steps.
const HEADING_STEP: f64 = TAU / 251.;
/// Anchor contraction can move the struck body line a few units, and the
/// owner and a dying bot are checked in the same tick: a bot's body counts
/// as present this many ticks past its predicted impact.
const IMPACT_SLACK_TICKS: usize = 6;
/// A straight run past a partly swept circle is walked in at most this many
/// samples, and counts as unsafe beyond them.
const RAY_SAMPLES: usize = 256;
/// Candidate wanted headings, 15 degrees apart on alternating sides of the aim.
const HEADING_STEPS: usize = 24;
/// Far bots re-aim and dead bots respawn on staggered six-tick decisions.
const DECISION_TICKS: u64 = 6;

#[cfg_attr(test, derive(Clone))]
pub(super) struct InWorldBots {
    slots: Range<usize>,
    behavior: BotBehavior,
    /// Recent owner head positions for the path fit.
    track: Option<OwnerTrack>,
    /// This tick's owner prediction; `None` for `center` or without an owner.
    path: Option<OwnerPath>,
    /// Each bot's heading toward its target.
    aims: Vec<f64>,
    /// The last chosen heading, the aim included: the fallback while the
    /// aim's own plan is unsafe.
    held: Vec<Option<f64>>,
    /// The selected target's identity and the owner's newest anchor at the
    /// last decision. A held heading must keep the oracle it was planned on.
    held_targets: Vec<Option<(Target, Point)>>,
    /// Bots that found no safe heading last tick and are steering out.
    escaping: Vec<bool>,
    /// Bots on a striking plan: the tick by which they must have struck the
    /// owner. The body they laid on the way was checked only until then.
    deadlines: Vec<Option<u64>>,
    /// Rollout speeds and positions of the bot being steered, reused.
    speeds: Vec<f64>,
    trail: Vec<(Point, f64)>,
    /// Off in the sensitivity control: chase the owner's head instead.
    #[cfg(test)]
    pub(super) avoidance: bool,
    /// Bot ticks spent with no safe heading.
    #[cfg(test)]
    pub(super) escape_ticks: u64,
    /// Bot ticks that dropped a striking plan with no other way to keep it.
    #[cfg(test)]
    pub(super) broken: u64,
    /// Count bot ticks whose existing body lies in the owner's path.
    #[cfg(test)]
    pub(super) witness: bool,
    #[cfg(test)]
    pub(super) conflicts: u64,
    /// Original conservative anchor screen, retained separately from contacts.
    #[cfg(test)]
    pub(super) bound_hits: u64,
}

impl InWorldBots {
    pub(super) fn new(slots: Range<usize>, behavior: BotBehavior) -> Self {
        Self {
            aims: vec![0.; slots.len()],
            held: vec![None; slots.len()],
            held_targets: vec![None; slots.len()],
            escaping: vec![false; slots.len()],
            deadlines: vec![None; slots.len()],
            slots,
            behavior,
            track: None,
            path: None,
            speeds: Vec::new(),
            trail: Vec::new(),
            #[cfg(test)]
            avoidance: true,
            #[cfg(test)]
            escape_ticks: 0,
            #[cfg(test)]
            broken: 0,
            #[cfg(test)]
            witness: false,
            #[cfg(test)]
            conflicts: 0,
            #[cfg(test)]
            bound_hits: 0,
        }
    }

    pub(super) fn slots(&self) -> Range<usize> {
        self.slots.clone()
    }

    /// Staggered decision ticks for aiming and respawning.
    pub(super) fn decides(&self, slot: usize, tick: u64) -> bool {
        (tick + (slot - self.slots.start) as u64).is_multiple_of(DECISION_TICKS)
    }

    /// A new owner life must not inherit the last one's path history.
    pub(super) fn reset_owner(&mut self) {
        self.track = None;
    }

    /// Fit the living owner's path for this tick.
    pub(super) fn observe(&mut self, world: &Simulator, owner: Option<usize>) {
        let points = self.path.take().map(|path| path.points).unwrap_or_default();
        let Some(slot) = owner.filter(|_| self.behavior == BotBehavior::Follow) else {
            if self.track.take().is_some() {
                self.deadlines.fill(None);
                self.held_targets.fill(None);
            }
            return;
        };
        let snake = &world.snakes()[slot];
        let dt = world.config().dt;
        let track = match &mut self.track {
            Some(track) if track.slot == slot => track,
            track => {
                // Strikes planned against another owner life are void.
                self.deadlines.fill(None);
                self.held_targets.fill(None);
                track.insert(OwnerTrack {
                    slot,
                    heading: snake.heading,
                    held: 0,
                    history: VecDeque::new(),
                })
            }
        };
        track.push(snake, (FIT_SECONDS / dt).ceil() as usize);
        // A circle inside the arena turns at least speed / radius, so its
        // quantized heading steps at least this often; a heading held longer
        // belongs to a straight line.
        let lane_hold = (HEADING_STEP * world.radius() / (snake.speed * dt)).ceil() as usize + 2;
        self.path = Some(OwnerPath::new(
            slot,
            snake,
            track.fit(lane_hold),
            dt,
            points,
        ));
    }

    pub(super) fn born(&mut self, slot: usize, heading: f64) {
        let index = slot - self.slots.start;
        self.aims[index] = heading;
        self.held[index] = None;
        self.held_targets[index] = None;
        self.escaping[index] = false;
        self.deadlines[index] = None;
    }

    /// Whether a new bot of this shape starts outside the owner's predicted
    /// path for its reserved lifetime, with room for any first plan.
    pub(super) fn spawn_is_safe(&mut self, body: &[Point], heading: f64) -> bool {
        let Some(path) = &mut self.path else {
            return true;
        };
        let segments = body.len();
        let horizon = path.steps(lifetime(segments, 0.)) + path.plan_steps(segments);
        path.extend(horizon);
        let arc = path.arc(0, horizon, path.contact(segments));
        let reach = physics::base_speed(segments) * path.plan_steps(segments) as f64 * path.dt;
        if path.encloses(body[0], horizon) || arc.clearance(body[0]) <= reach {
            return false;
        }
        // Clearance beyond the distance to the previous point bounds every
        // point between them, including the step from the forward head tip.
        let mut previous = physics::head_tip(body[0], heading, segments);
        body.iter().all(|&point| {
            let clear = arc.clearance(point) > point.distance(previous);
            previous = point;
            clear
        })
    }

    /// The wanted heading for a living bot this tick, or `None` to keep the
    /// last one.
    pub(super) fn heading(&mut self, world: &Simulator, slot: usize, tick: u64) -> Option<f64> {
        let index = slot - self.slots.start;
        let snake = &world.snakes()[slot];
        let decide = self.decides(slot, tick);
        let Some(path) = &mut self.path else {
            return decide.then(|| toward(snake.head, Point::default()));
        };
        let owner = &world.snakes()[path.owner];
        #[cfg(test)]
        if !self.avoidance {
            return Some(toward(snake.head, owner.head));
        }
        // Reserve growth through the score-table size. This extends the
        // current release estimate but is not a bound on arbitrary pickups.
        let release = path.steps(lifetime(snake.segments, snake.fam));
        let plan = path.plan_steps(snake.segments);
        let contact = path.contact(snake.segments);
        // Room for approaches that end on the owner's body.
        let limit = 3 * plan;
        path.extend(limit + release);
        path.extend_bodies(limit);
        let path: &OwnerPath = path;
        // Coarse head-distance filter for the reserved planning horizon.
        // The detailed planner runs once the bot can reach that path within
        // a half turn. It does not establish a lifetime bound for old body.
        let speed = snake.speed.max(physics::base_speed(snake.segments));
        let reach = speed * plan as f64 * path.dt;
        let near = path.arc(0, release + plan, contact).clearance(snake.head) <= reach
            || path.encloses(snake.head, release + plan);
        if !near {
            #[cfg(test)]
            if self.witness {
                self.bound_hits += u64::from(path.body_conflict(snake, None));
                self.conflicts += u64::from(
                    path.frozen_body_contact(snake, owner.heading, None)
                        .is_some(),
                );
            }
            self.held[index] = None;
            self.held_targets[index] = None;
            self.escaping[index] = false;
            self.deadlines[index] = None;
            if decide {
                let point = path
                    .target(owner, snake.head, limit)
                    .map_or(owner.head, |t| t.point);
                self.aims[index] = toward(snake.head, point);
            }
            return decide.then_some(self.aims[index]);
        }
        // A body too short to target leaves the head to aim for.
        let target = path.target(owner, snake.head, limit);
        let previous_wanted = self.held[index];
        let previous_target =
            self.held_targets[index].and_then(|(target, anchor)| target.after_tick(anchor, owner));
        let aim = toward(snake.head, target.as_ref().map_or(owner.head, |t| t.point));
        self.aims[index] = aim;
        let mut state = physics::SpeedState::from_observed_speed(snake.speed, snake.segments);
        let model = world.config().speed_model;
        self.speeds.clear();
        self.speeds
            .extend((0..limit).map(|_| state.step(false, snake.segments, path.dt, model)));
        let bot = Rollout {
            head: snake.head,
            heading: snake.heading,
            segments: snake.segments,
            owner_segments: owner.segments,
            speeds: &self.speeds,
            release,
            contact,
            target: target.as_ref(),
        };
        // The body laid so far was checked until the bot's deadline if it is
        // striking, and for its reserved lifetime otherwise. A striking
        // bot keeps to plans that strike by its deadline, unless the body it
        // already laid is clear for as long as the new plan keeps it.
        let committed = self.deadlines[index];
        let trail = &mut self.trail;
        let mut plan_for = |wanted: f64| {
            select_target(
                target,
                previous_target.filter(|_| previous_wanted == Some(wanted)),
                |target| {
                    let rollout = Rollout {
                        target: target.as_ref(),
                        ..bot
                    };
                    path.plan(&rollout, wanted, trail).filter(|&impact| {
                        path.keeps_body_clear(snake, committed, tick, impact, contact)
                    })
                },
            )
        };
        let chosen = steer(&mut self.held[index], aim, &mut plan_for);
        if chosen.is_none() && committed.is_some() {
            // A predicted strike can slip even in a steady state. All
            // continuations failed the existing-body guard; the escape below
            // is best effort and cannot replace that check.
            #[cfg(test)]
            {
                self.broken += 1;
            }
        }
        let Some((wanted, selected)) = chosen else {
            // No heading passed the model's checks. The fastest way out
            // changes slowly, so rank escapes when first trapped and on
            // decision ticks, and keep heading out in between.
            #[cfg(test)]
            {
                self.escape_ticks += 1;
                if self.witness {
                    self.bound_hits += u64::from(path.body_conflict(snake, None));
                    self.conflicts += u64::from(
                        path.frozen_body_contact(snake, owner.heading, None)
                            .is_some(),
                    );
                }
            }
            self.held[index] = None;
            self.held_targets[index] = None;
            self.deadlines[index] = None;
            if self.escaping[index] && !decide {
                return None;
            }
            self.escaping[index] = true;
            return Some(path.escape(&bot, aim, plan));
        };
        self.escaping[index] = false;
        self.held_targets[index] = selected.target.map(|target| (target, owner.body[0]));
        let impact = selected.impact;
        self.deadlines[index] = impact.map(|impact| tick + (impact + IMPACT_SLACK_TICKS) as u64);
        #[cfg(test)]
        if self.witness {
            self.bound_hits += u64::from(path.body_conflict(snake, impact));
            self.conflicts += u64::from(
                path.frozen_body_contact(snake, owner.heading, impact)
                    .is_some(),
            );
        }
        Some(wanted)
    }
}

/// The owner's recent head positions, with its heading unwrapped so turns
/// accumulate across the ±π boundary.
#[cfg_attr(test, derive(Clone))]
struct OwnerTrack {
    slot: usize,
    heading: f64,
    /// Ticks since the heading last changed.
    held: usize,
    history: VecDeque<(Point, f64)>,
}

impl OwnerTrack {
    fn push(&mut self, owner: &Snake, capacity: usize) {
        let turn = wrap(owner.heading - self.heading);
        let turned = self.history.back().map_or(0., |&(_, turned)| turned + turn);
        self.held = if turn.abs() < 1e-9 { self.held + 1 } else { 0 };
        self.heading = owner.heading;
        if self.history.len() > capacity {
            self.history.pop_front();
        }
        self.history.push_back((owner.head, turned));
    }

    /// A lane while the heading was never changed or held longer than
    /// `lane_hold` ticks; otherwise the circle fitted by least squares to
    /// the most recent stretch that turned `FIT_TURN`. Quantized steering
    /// turns gently as a polygon of held headings: its corners and edges stay
    /// in a band around one circle, and the tube widens by half that band.
    fn fit(&self, lane_hold: usize) -> Option<Circle> {
        if self.held + 1 >= self.history.len() || self.held > lane_hold {
            return None;
        }
        let &(now, turned) = self.history.back()?;
        let start = self
            .history
            .iter()
            .rposition(|&(_, earlier)| (turned - earlier).abs() >= FIT_TURN)
            .unwrap_or(0);
        let stretch = || self.history.range(start..).map(|&(point, _)| point);
        let count = (self.history.len() - start) as f64;
        let (x, y) = stretch().fold((0., 0.), |(x, y), p| (x + p.x, y + p.y));
        let mean = Point {
            x: x / count,
            y: y / count,
        };
        // Kåsa's algebraic fit, about the centroid for conditioning.
        let (uu, uv, vv, uuu, vvv, uvv, vuu) = stretch().fold(
            (0., 0., 0., 0., 0., 0., 0.),
            |(uu, uv, vv, uuu, vvv, uvv, vuu), p| {
                let (u, v) = (p.x - mean.x, p.y - mean.y);
                (
                    uu + u * u,
                    uv + u * v,
                    vv + v * v,
                    uuu + u * u * u,
                    vvv + v * v * v,
                    uvv + u * v * v,
                    vuu + v * u * u,
                )
            },
        );
        let det = uu * vv - uv * uv;
        let (bu, bv) = ((uuu + uvv) / 2., (vvv + vuu) / 2.);
        let center = Point {
            x: mean.x + (bu * vv - uv * bv) / det,
            y: mean.y + (uu * bv - uv * bu) / det,
        };
        let (low, high) = stretch().fold((f64::INFINITY, 0f64), |(low, high), p| {
            let radius = length(p.x - center.x, p.y - center.y);
            (low.min(radius), high.max(radius))
        });
        let radius = (low + high) / 2.;
        // Beyond this the arc over any horizon used here is a lane to well
        // under a unit.
        if !radius.is_finite() || radius >= 1e9 {
            return None;
        }
        let first = self.history[start].0;
        let (ax, ay) = (first.x - center.x, first.y - center.y);
        let (bx, by) = (now.x - center.x, now.y - center.y);
        // The mean rate over the stretch, steps and holds alike.
        let swept = (ax * by - ay * bx).atan2(ax * bx + ay * by);
        Some(Circle {
            center,
            radius,
            band: (high - low) / 2.,
            sign: swept.signum(),
            turn: swept.abs() / (count - 1.),
        })
    }
}

/// The owner's head path at whole ticks from now, under its fitted lane or
/// circle and current speed. Samples extend on demand within one world tick.
#[cfg_attr(test, derive(Clone))]
struct OwnerPath {
    owner: usize,
    speed: f64,
    segments: usize,
    dt: f64,
    circle: Option<Circle>,
    /// Lane direction, or the per-tick rotation about the circle center.
    step: (f64, f64),
    /// Angle swept per tick around the circle.
    turn: f64,
    points: Vec<Point>,
    /// Predicted owner bodies, each from the tick of an anchor insertion.
    bodies: Vec<(usize, Vec<Point>)>,
    /// Ticks the body prediction covers.
    bodied: usize,
}

/// The fitted circle: `radius` is the middle of the band the path keeps,
/// `band` its half-width, `sign` the turn direction and `turn` the mean
/// angle per tick.
#[derive(Clone, Copy)]
struct Circle {
    center: Point,
    radius: f64,
    band: f64,
    sign: f64,
    turn: f64,
}

/// The owner body point a bot aims for and the body line around it.
#[derive(Clone, Copy)]
struct Target {
    point: Point,
    /// The body segment from anchor `segment` to the next, as indexed now.
    segment: usize,
}

impl Target {
    /// Each engine tick inserts at most one anchor. Insertion leaves the old
    /// first anchor unchanged at index one (`append_anchor`), so that exact
    /// identity advances the segment without searching a contracting body.
    /// Keep the original point too: replacing it with the new nearest point
    /// can stop a held trace before its previously predicted strike.
    fn after_tick(mut self, anchor: Point, owner: &Snake) -> Option<Self> {
        if owner.body[0] != anchor {
            if owner.body.get(1) != Some(&anchor) {
                return None;
            }
            self.segment += 1;
        }
        (self.segment + 1 < owner.body.len()).then_some(self)
    }
}

struct Planned {
    impact: Option<usize>,
    target: Option<Target>,
}

/// Check each target before choosing it. In particular, apply the existing-
/// body guard inside `plan` so a rejected miss on the nearest target cannot
/// mask a retained strike on the same heading.
fn select_target(
    target: Option<Target>,
    previous: Option<Target>,
    mut plan: impl FnMut(Option<Target>) -> Option<Option<usize>>,
) -> Option<Planned> {
    let mut for_target = |target| plan(target).map(|impact| Planned { impact, target });
    for_target(target).or_else(|| previous.and_then(|target| for_target(Some(target))))
}

/// One bot's state and limits for planning its next ticks.
struct Rollout<'a> {
    head: Point,
    heading: f64,
    segments: usize,
    owner_segments: usize,
    /// Engine speed at each tick of the longest plan.
    speeds: &'a [f64],
    /// Ticks a body point laid now may last.
    release: usize,
    contact: f64,
    target: Option<&'a Target>,
}

impl OwnerPath {
    fn new(
        owner: usize,
        snake: &Snake,
        circle: Option<Circle>,
        dt: f64,
        mut points: Vec<Point>,
    ) -> Self {
        let turn = circle.map_or(0., |circle| circle.turn);
        let step = match circle {
            Some(circle) => ((circle.sign * turn).cos(), (circle.sign * turn).sin()),
            None => (snake.heading.cos(), snake.heading.sin()),
        };
        points.clear();
        points.push(snake.head);
        Self {
            owner,
            speed: snake.speed,
            segments: snake.segments,
            dt,
            circle,
            step,
            turn,
            points,
            bodies: vec![(0, snake.body.clone())],
            bodied: 0,
        }
    }

    fn steps(&self, seconds: f64) -> usize {
        (seconds / self.dt).ceil() as usize
    }

    /// Ticks for a bot to turn by half a circle: any wanted heading is
    /// reached by then, so every plan ends in a straight line.
    fn plan_steps(&self, segments: usize) -> usize {
        self.steps(PI / physics::turn_rate(segments)) + 1
    }

    /// Sample through `until` ticks from now.
    fn extend(&mut self, until: usize) {
        let head = self.points[0];
        for k in self.points.len()..=until {
            let point = match self.circle {
                // Repeated rotation drifts by about k ulps: far below a unit.
                Some(Circle { center, .. }) => {
                    let last = self.points[k - 1];
                    let (x, y) = (last.x - center.x, last.y - center.y);
                    let (cos, sin) = self.step;
                    Point {
                        x: center.x + x * cos - y * sin,
                        y: center.y + x * sin + y * cos,
                    }
                }
                None => Point {
                    x: head.x + k as f64 * self.speed * self.dt * self.step.0,
                    y: head.y + k as f64 * self.speed * self.dt * self.step.1,
                },
            };
            self.points.push(point);
        }
    }

    /// The predicted head `k` ticks from now: sampled, or directly beyond.
    fn point(&self, k: usize) -> Point {
        if let Some(&point) = self.points.get(k) {
            return point;
        }
        let head = self.points[0];
        match self.circle {
            Some(Circle { center, sign, .. }) => {
                let (sin, cos) = (sign * self.turn * k as f64).sin_cos();
                let (x, y) = (head.x - center.x, head.y - center.y);
                Point {
                    x: center.x + x * cos - y * sin,
                    y: center.y + x * sin + y * cos,
                }
            }
            None => Point {
                x: head.x + k as f64 * self.speed * self.dt * self.step.0,
                y: head.y + k as f64 * self.speed * self.dt * self.step.1,
            },
        }
    }

    /// The tube's constant half-width for a bot of this size: the owner's
    /// probe reaches past the bot's centerline by the bot radius, the bot's
    /// tip leads its center by another, the owner moves one tick between
    /// checks, quantized gentle turns keep within the fitted band, plus the
    /// fixed margin. A bot's own tick of motion is checked separately in each
    /// plan.
    fn contact(&self, segments: usize) -> f64 {
        physics::collision_distance(self.segments, segments)
            + physics::BODY_RADIUS * physics::scale(segments)
            + self.speed * self.dt
            + self.circle.map_or(0., |circle| circle.band)
            + MARGIN
    }

    /// Whether `point` is inside a circle the owner sweeps whole within
    /// `ticks`: from there no straight line clears the tube.
    fn encloses(&self, point: Point, ticks: usize) -> bool {
        self.circle.is_some_and(|circle| {
            self.turn * ticks as f64 >= TAU && point.distance(circle.center) < circle.radius
        })
    }

    /// The tube around the owner's predicted head between two ticks.
    fn arc(&self, from: usize, until: usize, contact: f64) -> Arc {
        Arc {
            a: self.point(from),
            b: self.point(until),
            circle: self.circle,
            sweep: (until - from) as f64 * self.turn,
            width: contact,
        }
    }

    /// The nearest point on the owner's body line, from the first anchor
    /// behind the head back to the part that outlasts every plan: the tail
    /// end moves away before a bot arrives. A point on the line, not the
    /// nearest anchor, so the aim turns smoothly as the bot closes in.
    fn target(&self, owner: &Snake, head: Point, ticks: usize) -> Option<Target> {
        let travel = self.speed * ticks as f64 * self.dt;
        let insertions = (travel / physics::ANCHOR_SPACING).ceil() as usize + 1;
        let usable = owner.body.len().saturating_sub(insertions + 3);
        let (segment, point) = (0..usable)
            .map(|i| {
                (
                    i,
                    closest_on_segment(head, owner.body[i], owner.body[i + 1]),
                )
            })
            .min_by(|a, b| a.1.distance(head).total_cmp(&b.1.distance(head)))?;
        Some(Target { point, segment })
    }

    /// Predict the owner's body through `until` ticks along the predicted
    /// head path with the engine's own anchor rules: an anchor every 42
    /// units, each insertion contracting the older anchors. Each snapshot
    /// holds from its insertion tick; growth only lengthens the tail.
    fn extend_bodies(&mut self, until: usize) {
        if until <= self.bodied {
            return;
        }
        self.extend(until);
        let mut body = self.bodies[self.bodies.len() - 1].1.clone();
        for tick in self.bodied + 1..=until {
            if let Some(anchor) = physics::crossed_anchor(self.points[tick], body[0]) {
                physics::append_anchor(&mut body, anchor, false);
                self.bodies.push((tick, body.clone()));
            }
        }
        self.bodied = until;
    }

    /// The predicted owner body at `tick` and the anchors inserted by then.
    fn body_at(&self, tick: usize) -> (usize, &[Point]) {
        let index = self.bodies.partition_point(|&(from, _)| from <= tick) - 1;
        (index, &self.bodies[index].1)
    }

    /// The planned strike's tick, or `None` for a plan that runs straight,
    /// if steering toward `wanted` keeps every body tick the bot lays out of
    /// the owner's path while that body may last; `None` for an unsafe plan.
    /// The plan follows the engine's kinematics until the bot strikes its
    /// target, ending its body, or until it runs straight; that straight line
    /// is checked for unbounded continuation in the frozen rollout model.
    /// Later size changes, fitting error and body contraction need their own
    /// bounds before this implies safety in the world.
    fn plan(
        &self,
        bot: &Rollout,
        wanted: f64,
        trail: &mut Vec<(Point, f64)>,
    ) -> Option<Option<usize>> {
        let (impact, heading) = self.trace(bot, wanted, trail);
        let gone = impact.map_or(usize::MAX, |impact| impact + IMPACT_SLACK_TICKS);
        let clear = (1..).zip(trail.iter()).all(|(step, &(head, travel))| {
            let until = (step + bot.release).min(gone);
            self.arc(step, until, bot.contact).clearance(head) > travel
        });
        (clear && (impact.is_some() || self.straight_is_clear(bot, trail, heading)))
            .then_some(impact)
    }

    /// Advance until the bot strikes its target or runs straight in the
    /// wanted direction, recording each tick's head and travel.
    fn trace(
        &self,
        bot: &Rollout,
        wanted: f64,
        trail: &mut Vec<(Point, f64)>,
    ) -> (Option<usize>, f64) {
        trail.clear();
        let goal = physics::quantize_heading(wanted);
        let (mut head, mut heading) = (bot.head, bot.heading);
        let mut tip = physics::head_tip(head, heading, bot.segments);
        let mut steering = physics::SteeringCache::default();
        for (step, &speed) in (1..).zip(bot.speeds) {
            (head, heading, _) =
                steering.advance(head, heading, wanted, speed, bot.segments, self.dt);
            trail.push((head, speed * self.dt));
            let last = tip;
            tip = physics::head_tip(head, heading, bot.segments);
            if let Some(target) = bot.target {
                // The target segment and its neighbors on the body predicted
                // for this tick, shifted by the anchors inserted since.
                let (inserted, body) = self.body_at(step);
                let at = target.segment + inserted;
                // A retained target can leave the tail during this rollout.
                // Missing geometry cannot certify a strike.
                if let Some(line) = body.get(at.saturating_sub(1)..(at + 3).min(body.len()))
                    && physics::strikes(
                        last,
                        tip,
                        heading,
                        bot.segments,
                        bot.owner_segments,
                        line.iter().copied(),
                    )
                {
                    return (Some(step), heading);
                }
            }
            let straight = wrap(goal - heading).abs() < 1e-9;
            let closing = bot.target.is_some_and(|target| {
                (target.point.x - head.x) * heading.cos()
                    + (target.point.y - head.y) * heading.sin()
                    > 0.
            });
            if straight && !closing {
                break;
            }
        }
        (None, heading)
    }

    /// Whether the bot's straight run after `trail` keeps all the body it lays
    /// out of the owner's path: in the owner's moving frame, the lane stretch
    /// the owner covers while that body lasts; or the stretch of the circle
    /// it covers, the whole ring once that is a full turn.
    fn straight_is_clear(&self, bot: &Rollout, trail: &[(Point, f64)], heading: f64) -> bool {
        let Some(&(head, travel)) = trail.last() else {
            return false;
        };
        let (cos, sin) = (heading.cos(), heading.sin());
        let room = bot.contact + travel;
        match self.circle {
            Some(circle) => {
                let (x, y) = (head.x - circle.center.x, head.y - circle.center.y);
                let ahead = (-(x * cos + y * sin)).max(0.);
                let closest = length(x + ahead * cos, y + ahead * sin);
                if length(x, y) >= circle.radius && closest - circle.radius > room {
                    return true;
                }
                self.turn * (bot.release as f64) < TAU
                    && self.ray_is_clear(bot, circle, trail.len(), head, (cos, sin), travel)
            }
            None => {
                let owner = self.points[trail.len()];
                let speed = travel / self.dt;
                let from = Point {
                    x: head.x - owner.x,
                    y: head.y - owner.y,
                };
                let velocity = Point {
                    x: speed * cos - self.speed * self.step.0,
                    y: speed * sin - self.speed * self.step.1,
                };
                let lane = self.speed * bot.release as f64 * self.dt;
                let end = Point {
                    x: lane * self.step.0,
                    y: lane * self.step.1,
                };
                ray_segment_distance(from, velocity, end) > room
            }
        }
    }

    /// Walk a straight run from tick `from` past a circle the owner sweeps
    /// only partly while a body point lasts: each point must clear the arc
    /// the owner covers in that time. Ticks between samples are skipped while
    /// the clearance cannot close: per tick the point moves `travel` and the
    /// arc's ends move the owner's travel. Once outside the ring's tube and
    /// leaving it, the run never returns.
    fn ray_is_clear(
        &self,
        bot: &Rollout,
        circle: Circle,
        from: usize,
        mut head: Point,
        (cos, sin): (f64, f64),
        travel: f64,
    ) -> bool {
        let closing = travel + self.speed * self.dt;
        let mut tick = from;
        for _ in 0..RAY_SAMPLES {
            let (x, y) = (head.x - circle.center.x, head.y - circle.center.y);
            if x * cos + y * sin >= 0. && length(x, y) - circle.radius > bot.contact + travel {
                return true;
            }
            let clearance = self
                .arc(tick, tick + bot.release, bot.contact)
                .clearance(head);
            if clearance <= travel {
                return false;
            }
            let skip = ((clearance - travel) / closing).ceil().max(1.);
            tick += skip as usize;
            head.x += cos * travel * skip;
            head.y += sin * travel * skip;
        }
        false
    }

    /// Preserve the previous impact deadline unless the existing body also
    /// clears the replacement plan's disappearance estimate.
    fn keeps_body_clear(
        &self,
        snake: &Snake,
        committed: Option<u64>,
        tick: u64,
        impact: Option<usize>,
        contact: f64,
    ) -> bool {
        let gone = impact.map(|impact| impact + IMPACT_SLACK_TICKS);
        committed.is_none_or(|deadline| {
            gone.is_some_and(|gone| tick + gone as u64 <= deadline)
                || self.body_is_clear(snake, gone, contact)
        })
    }

    /// Check the existing body against the fitted path until `gone`, or
    /// each anchor's reserved lifetime. Clearance beyond the neighboring
    /// point distance also covers the segment between those points.
    fn body_is_clear(&self, snake: &Snake, gone: Option<usize>, contact: f64) -> bool {
        let insertion = insertion_seconds(snake.segments);
        let growth =
            physics::MAX_SEGMENTS.saturating_sub(snake.segments) as f64 + snake.fam.ceil() + 1.;
        let mut previous = snake.head;
        snake.body.iter().enumerate().all(|(index, &anchor)| {
            let left = ((snake.body.len() - index) as f64 + growth) * insertion;
            let until = self.steps(left).min(gone.unwrap_or(usize::MAX));
            let clear = self.arc(0, until, contact).clearance(anchor) > anchor.distance(previous);
            previous = anchor;
            clear
        })
    }

    /// Original conservative screen: frozen anchors within the sum-of-radii
    /// tube before their estimated release or predicted impact. This is not
    /// the engine's forward-probe collision predicate. Keep the raw count.
    #[cfg(test)]
    fn body_conflict(&self, snake: &Snake, impact: Option<usize>) -> bool {
        let insertion = insertion_seconds(snake.segments);
        let gone = impact.map_or(usize::MAX, |impact| impact + IMPACT_SLACK_TICKS);
        let touch = physics::collision_distance(self.segments, snake.segments);
        let pending = snake.fam.ceil() + 1.;
        snake.body.iter().enumerate().any(|(index, &anchor)| {
            let left = ((snake.body.len() - index) as f64 + pending) * insertion;
            let until = self.steps(left).min(gone);
            self.arc(0, until, touch).clearance(anchor) <= 0.
        })
    }

    /// Diagnostic only: the engine's contact predicate against the ENTIRE
    /// current frozen tip/head/body line. Owner centers follow the fitted
    /// lane/circle; later headings follow each displacement chord, starting
    /// from the observed heading. This is not reconstruction of future wire
    /// input, owner growth, bot contraction, or a missed predicted strike.
    /// Keep the old estimated release/impact horizon explicit.
    #[cfg(test)]
    fn frozen_body_contact(
        &self,
        snake: &Snake,
        owner_heading: f64,
        impact: Option<usize>,
    ) -> Option<usize> {
        let until = self
            .steps(release_time(snake.segments, snake.fam))
            .min(impact.map_or(usize::MAX, |impact| impact + IMPACT_SLACK_TICKS));
        let first = physics::head_tip(snake.head, snake.heading, snake.segments);
        let line = || {
            [first, snake.head]
                .into_iter()
                .chain(snake.body.iter().copied())
        };
        // Cover full segments and the head, not only anchors: clearance is
        // 1-Lipschitz, so clearance beyond a segment's length screens it out.
        // The path rotates the observed head, whose radius can differ from
        // the fit's mid-band radius. Include that exact offset in this
        // broad phase only; the contact predicate below is unchanged.
        let radial_offset = self.circle.map_or(0., |circle| {
            (self.point(0).distance(circle.center) - circle.radius).abs()
        });
        let arc = self.arc(
            0,
            until,
            physics::collision_distance(self.segments, snake.segments) + radial_offset,
        );
        let mut previous = first;
        let near = arc.clearance(first) <= 0.
            || line().skip(1).any(|point| {
                let hit = arc.clearance(point) <= point.distance(previous);
                previous = point;
                hit
            });
        if !near {
            return None;
        }
        let mut head = self.point(0);
        let mut tip = physics::head_tip(head, owner_heading, self.segments);
        for tick in 1..=until {
            let next = self.point(tick);
            let heading = toward(head, next);
            let last_tip = tip;
            tip = physics::head_tip(next, heading, self.segments);
            if physics::strikes(
                last_tip,
                tip,
                heading,
                self.segments,
                snake.segments,
                line(),
            ) {
                return Some(tick);
            }
            head = next;
        }
        None
    }

    /// For a bot with no safe heading: the candidate whose plan leaves the
    /// path soonest and stays out, then the one leaving the most room.
    fn escape(&self, bot: &Rollout, aim: f64, plan: usize) -> f64 {
        let mut best = (usize::MAX, f64::NEG_INFINITY, aim);
        for wanted in candidates(aim) {
            let (mut head, mut heading) = (bot.head, bot.heading);
            let mut steering = physics::SteeringCache::default();
            // First tick from which every later tick is clear.
            let mut exit = None;
            let mut last = f64::NEG_INFINITY;
            for (step, &speed) in (1..).zip(&bot.speeds[..plan]) {
                (head, heading, _) =
                    steering.advance(head, heading, wanted, speed, bot.segments, self.dt);
                last = self
                    .arc(step, step + bot.release, bot.contact)
                    .clearance(head)
                    - speed * self.dt;
                if last > 0. {
                    exit.get_or_insert(step);
                    continue;
                }
                exit = None;
                // Still inside at or after the best exit: this candidate can
                // only leave later, so it cannot win.
                if step >= best.0 {
                    break;
                }
            }
            let exit = exit.unwrap_or(usize::MAX);
            if exit < best.0 || exit == best.0 && last > best.1 {
                best = (exit, last, wanted);
            }
        }
        best.2
    }
}

/// Hysteresis: the aim whenever its plan is acceptable, otherwise the last
/// chosen heading while it stays acceptable, and only then the other headings
/// closest-first. The chosen heading, the aim included, becomes the next
/// fallback, so a plan that stays safe stays available as the aim moves.
fn steer<T>(
    held: &mut Option<f64>,
    aim: f64,
    mut plan: impl FnMut(f64) -> Option<T>,
) -> Option<(f64, T)> {
    let mut chosen = plan(aim).map(|t| (aim, t));
    if chosen.is_none()
        && let Some(previous) = *held
    {
        chosen = plan(previous).map(|t| (previous, t));
    }
    if chosen.is_none() {
        chosen = candidates(aim)
            .skip(1)
            .find_map(|wanted| plan(wanted).map(|t| (wanted, t)));
    }
    if let Some((wanted, _)) = chosen {
        *held = Some(wanted);
    }
    chosen
}

/// Candidate wanted headings by their deviation from `aim`, alternating sides.
fn candidates(aim: f64) -> impl Iterator<Item = f64> {
    (0..HEADING_STEPS).map(move |candidate| {
        let turn = candidate.div_ceil(2) as f64 * TAU / HEADING_STEPS as f64;
        if candidate.is_multiple_of(2) {
            aim + turn
        } else {
            aim - turn
        }
    })
}

/// The tube around one stretch of the owner's predicted head path.
#[derive(Clone, Copy)]
struct Arc {
    a: Point,
    b: Point,
    circle: Option<Circle>,
    sweep: f64,
    width: f64,
}

impl Arc {
    /// Exact distance to the straight segment or directed circular arc, less
    /// the tube width. Wedge tests avoid trigonometry.
    fn clearance(&self, point: Point) -> f64 {
        let distance = match self.circle {
            None => segment_distance(point, self.a, self.b),
            Some(Circle {
                center,
                radius,
                sign,
                ..
            }) => {
                let p = (point.x - center.x, point.y - center.y);
                let a = (self.a.x - center.x, self.a.y - center.y);
                let b = (self.b.x - center.x, self.b.y - center.y);
                let after = (a.0 * p.1 - a.1 * p.0) * sign >= 0.;
                let before = (p.0 * b.1 - p.1 * b.0) * sign >= 0.;
                let on_arc = self.sweep >= TAU
                    || if self.sweep <= PI {
                        after && before
                    } else {
                        after || before
                    };
                if on_arc {
                    (length(p.0, p.1) - radius).abs()
                } else {
                    length(point.x - self.a.x, point.y - self.a.y)
                        .min(length(point.x - self.b.x, point.y - self.b.y))
                }
            }
        };
        distance - self.width
    }
}

fn toward(from: Point, to: Point) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}

fn wrap(angle: f64) -> f64 {
    angle.sin().atan2(angle.cos())
}

fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let closest = closest_on_segment(p, a, b);
    length(p.x - closest.x, p.y - closest.y)
}

fn closest_on_segment(p: Point, a: Point, b: Point) -> Point {
    let (x, y) = (b.x - a.x, b.y - a.y);
    let squared = x * x + y * y;
    let t = if squared == 0. {
        0.
    } else {
        (((p.x - a.x) * x + (p.y - a.y) * y) / squared).clamp(0., 1.)
    };
    Point {
        x: a.x + t * x,
        y: a.y + t * y,
    }
}

/// Distance between the ray `from + t * direction` (t ≥ 0) and the segment
/// from the origin to `end`: zero if they cross, otherwise attained at the
/// ray's origin or one of the segment's ends.
fn ray_segment_distance(from: Point, direction: Point, end: Point) -> f64 {
    let cross = |a: Point, b: Point| a.x * b.y - a.y * b.x;
    let denominator = cross(direction, end);
    if denominator != 0. {
        // Solve from + t * direction = s * end.
        let t = cross(end, from) / denominator;
        let s = cross(direction, from) / denominator;
        if t >= 0. && (0. ..=1.).contains(&s) {
            return 0.;
        }
    }
    let to_ray = |p: Point| {
        let squared = direction.x * direction.x + direction.y * direction.y;
        let t = if squared == 0. {
            0.
        } else {
            (((p.x - from.x) * direction.x + (p.y - from.y) * direction.y) / squared).max(0.)
        };
        length(
            p.x - from.x - t * direction.x,
            p.y - from.y - t * direction.y,
        )
    };
    segment_distance(from, Point::default(), end)
        .min(to_ray(Point::default()))
        .min(to_ray(end))
}

/// World coordinates are far from overflow, so the plain square root
/// replaces `hypot`, which was a tenth of a follow host's tick.
fn length(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

/// Continuous-arc estimate for one 42-unit chord, using this size's maximum
/// turn and the smallest cruise speed. This omits discrete insertion timing;
/// it is not a proved upper bound for every future world trajectory.
fn insertion_seconds(segments: usize) -> f64 {
    let speed = physics::base_speed(2);
    let turn = physics::turn_rate(segments);
    2. * (physics::ANCHOR_SPACING * turn / (2. * speed)).asin() / turn
}

/// Release estimate without further pickups: count current and pending
/// insertions, plus one partial anchor, using `insertion_seconds`.
#[cfg(test)]
pub(super) fn release_time(segments: usize, fam: f64) -> f64 {
    (segments as f64 + fam.ceil() + 1.) * insertion_seconds(segments)
}

/// Planning reserve through `max(segments, MAX_SEGMENTS)` plus pending mass.
/// A future whole segment suppresses a tail removal. This reserve covers at
/// most `MAX_SEGMENTS.saturating_sub(segments)` such future insertions; it is
/// not a growth cap. `multiplier` has a positive floor and the engine permits
/// larger snakes, so arbitrary future pickups can exceed this estimate.
/// Discrete insertion timing and later size changes also need separate
/// bounds before this can support a universal body-disappearance claim.
pub(super) fn lifetime(segments: usize, fam: f64) -> f64 {
    (segments.max(physics::MAX_SEGMENTS) as f64 + fam.ceil() + 1.) * insertion_seconds(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snake(head: Point, heading: f64) -> Snake {
        let world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: crate::Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["snake".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let mut snake = world.snakes()[0].clone();
        snake.segments = 80;
        snake.speed = physics::base_speed(80);
        snake.head = head;
        snake.heading = heading;
        snake
    }

    /// An owner steering with the engine's quantized headings: at the
    /// maximum rate (`None`), or with its wanted heading turning at `rate`.
    fn drive(rate: Option<f64>, ticks: usize) -> Vec<(Point, f64)> {
        let dt = physics::LOOP_SECONDS;
        let (mut head, mut heading, mut wanted) = (Point { x: 300., y: -200. }, 0.4, 0.4);
        (0..ticks)
            .map(|_| {
                wanted = rate.map_or(heading + PI / 2., |rate| wanted + rate * dt);
                (head, heading) = physics::advance(
                    head,
                    heading,
                    physics::quantize_heading(wanted),
                    physics::base_speed(80),
                    physics::turn_rate(80),
                    dt,
                );
                (head, heading)
            })
            .collect()
    }

    /// The straight-line threshold in the largest wire-compatible arena.
    fn lane_hold() -> usize {
        (HEADING_STEP * 32000. / (physics::base_speed(80) * physics::LOOP_SECONDS)).ceil() as usize
            + 2
    }

    fn new_track(heading: f64) -> OwnerTrack {
        OwnerTrack {
            slot: 0,
            heading,
            held: 0,
            history: VecDeque::new(),
        }
    }

    fn track(path: &[(Point, f64)]) -> OwnerTrack {
        let mut track = new_track(path[0].1);
        for &(head, heading) in path {
            let capacity = (FIT_SECONDS / physics::LOOP_SECONDS).ceil() as usize;
            track.push(&snake(head, heading), capacity);
        }
        track
    }

    /// Fit quality against the owner's real future: the worst distance off
    /// the fitted band, and the worst distance from the predicted head
    /// while the prediction covers less than a full turn, where the tube's
    /// ends matter.
    fn fit_errors(rate: Option<f64>, settle: usize, horizon: usize, every: usize) -> (f64, f64) {
        let dt = physics::LOOP_SECONDS;
        let path = drive(rate, settle + horizon + 3 * every);
        let capacity = (FIT_SECONDS / dt).ceil() as usize;
        let mut track = new_track(path[0].1);
        let (mut lateral, mut along) = (0f64, 0f64);
        for (tick, &(head, heading)) in path.iter().enumerate() {
            track.push(&snake(head, heading), capacity);
            if tick < settle
                || tick + horizon >= path.len()
                || !(tick - settle).is_multiple_of(every)
            {
                continue;
            }
            let fit = track.fit(lane_hold());
            assert_eq!(fit.is_none(), rate == Some(0.), "rate={rate:?}");
            let predicted = OwnerPath::new(0, &snake(head, heading), fit, dt, Vec::new());
            for k in 0..=horizon {
                let actual = path[tick + k].0;
                let off = match fit {
                    Some(circle) => {
                        (actual.distance(circle.center) - circle.radius).abs() - circle.band
                    }
                    None => segment_distance(actual, predicted.point(0), predicted.point(horizon)),
                };
                lateral = lateral.max(off);
                if predicted.turn * (k as f64) < TAU {
                    along = along.max(actual.distance(predicted.point(k)));
                }
            }
        }
        (lateral, along)
    }

    #[test]
    fn host_bots_steady_paths_fit_a_lane_or_one_circle() {
        let dt = physics::LOOP_SECONDS;
        let horizon = (4. / dt) as usize;
        let max = physics::turn_rate(80);
        for rate in [Some(0.), None, Some(max / 2.), Some(0.3), Some(0.1)] {
            let settle = rate
                .map_or(FIT_TURN / max, |rate| FIT_TURN / rate.max(0.1))
                .max(lane_hold() as f64 * dt);
            let (lateral, along) = fit_errors(rate, (settle / dt) as usize + 1, horizon, 25);
            // Off the band the error stays well inside the fixed margin; along
            // the path the tube's rounded ends absorb the rest.
            assert!(lateral < MARGIN / 4., "rate={rate:?} lateral={lateral}");
            assert!(along < MARGIN / 2., "rate={rate:?} along={along}");
            if rate == Some(0.) || rate.is_none() {
                assert!(
                    lateral < 1e-6 && along < 1e-6,
                    "rate={rate:?} {lateral} {along}"
                );
            }
        }
    }

    /// Review repro: at a very gentle constant turn the quantized heading
    /// steps seconds apart. The path must still be predicted as the circle it
    /// is for as long as a body can last. The gentlest rate is a circle about
    /// as large as the largest arena.
    #[test]
    fn host_bots_very_gentle_turns_stay_predicted_while_long_bodies_last() {
        let dt = physics::LOOP_SECONDS;
        let horizon = (lifetime(2, 0.) / dt) as usize;
        for rate in [0.006, 0.007, 0.01, 0.03] {
            let settle = ((FIT_TURN / rate).min(FIT_SECONDS) / dt) as usize + 1;
            let (lateral, along) = fit_errors(Some(rate), settle, horizon, 1500);
            assert!(lateral < MARGIN / 4., "rate={rate} lateral={lateral}");
            assert!(along < MARGIN / 2., "rate={rate} along={along}");
        }
    }

    /// Review repro: a direct aim that was safe stays the fallback when the
    /// next aim is not.
    #[test]
    fn host_bots_a_safe_aim_stays_the_fallback() {
        let mut held = None;
        let all = |_: f64| Some(());
        assert_eq!(steer(&mut held, 0.4, all).map(|c| c.0), Some(0.4));
        let only_last = |wanted: f64| (wanted == 0.4).then_some(());
        assert_eq!(steer(&mut held, 0.5, only_last).map(|c| c.0), Some(0.4));
    }

    /// Selected synthetic geometry from the independent reduced replay.
    /// The original fitted history is absent: fit this diagnostic from two
    /// seconds of the explicitly held future owner input, then also compare
    /// against the authoritative engine's actual samples. Neither is a
    /// reconstruction of the full host's later planner/food/other actors.
    #[test]
    fn host_bots_frozen_contact_rejects_the_captured_bound_false_positive() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/follow-frozen-geometry.json"
        ))
        .unwrap();
        let input = &fixture["owner"];
        let mut owner = snake(
            serde_json::from_value(input["head"].clone()).unwrap(),
            input["heading"].as_f64().unwrap(),
        );
        owner.segments = input["segments"].as_u64().unwrap() as usize;
        owner.speed = input["speed"].as_f64().unwrap();
        owner.fam = input["fam"].as_f64().unwrap();
        owner.body = (0..owner.segments)
            .map(|index| Point {
                x: owner.head.x - index as f64 * physics::ANCHOR_SPACING * owner.heading.cos(),
                y: owner.head.y - index as f64 * physics::ANCHOR_SPACING * owner.heading.sin(),
            })
            .collect();
        let bot: Snake = serde_json::from_value(fixture["bot"].clone()).unwrap();
        let mut config = crate::Config::official(257);
        config.radius = 32000.;
        config.dynamic_radius = false;
        config.size_to_population = false;
        config.food_count = 0;
        config.ambient_replenish_per_second = 0.;
        config.prey.count = 0;
        let dt = config.dt;
        assert_eq!(dt, fixture["dt"].as_f64().unwrap());
        let mut world = Simulator::from_scenario(
            73,
            config,
            crate::Scenario {
                snakes: vec![(
                    "owner".into(),
                    crate::ReplayLife::from_observed(
                        owner.clone(),
                        crate::InitialSpeed::Known(physics::SpeedState::idle(owner.speed)),
                    ),
                )],
                ..Default::default()
            },
        )
        .unwrap();
        let mut wanted = 0.;
        let rate = fixture["wantedRate"].as_f64().unwrap();
        for _ in 0..fixture["wantedSubtractions"].as_u64().unwrap() {
            wanted += rate * dt;
        }
        assert_eq!(
            wanted.to_bits(),
            0xc053_f611_3404_edb9,
            "retained first-action bits"
        );
        let line: Vec<_> = [
            physics::head_tip(bot.head, bot.heading, bot.segments),
            bot.head,
        ]
        .into_iter()
        .chain(bot.body.iter().copied())
        .collect();
        let mut tip = physics::head_tip(owner.head, owner.heading, owner.segments);
        let mut motion = vec![(owner.head, owner.heading)];
        let mut contacts = 0;
        for _ in 0..fixture["expected"]["replayTicks"].as_u64().unwrap() {
            world
                .step(&[Some(crate::Action {
                    heading: wanted,
                    boost: false,
                })])
                .unwrap();
            let actual = &world.snakes()[0];
            let next_tip = physics::head_tip(actual.head, actual.heading, actual.segments);
            contacts += usize::from(physics::strikes(
                tip,
                next_tip,
                actual.heading,
                actual.segments,
                bot.segments,
                line.iter().copied(),
            ));
            tip = next_tip;
            motion.push((actual.head, actual.heading));
            wanted += rate * dt;
        }
        assert_eq!(
            contacts,
            fixture["expected"]["authoritativeFrozenContacts"]
                .as_u64()
                .unwrap() as usize
        );
        let fit = track(&motion[..=(2. / dt).ceil() as usize]).fit(lane_hold());
        assert!(fit.is_some());
        let mut diagnostic = OwnerPath::new(0, &owner, fit, dt, Vec::new());
        assert!(
            diagnostic.body_conflict(&bot, None),
            "retain the raw bound hit"
        );
        assert_eq!(
            diagnostic.frozen_body_contact(&bot, owner.heading, None),
            None
        );
        // Independent reference samples also exercise the same contact
        // predicate without relying on fitted center positions.
        diagnostic.points = motion.iter().map(|&(head, _)| head).collect();
        assert_eq!(
            diagnostic.frozen_body_contact(&bot, owner.heading, None),
            None
        );
    }

    #[test]
    fn host_bots_frozen_contact_detects_engine_lethal_segments_and_head_tips() {
        let mut owner = snake(Point::default(), 0.);
        owner.segments = 2;
        owner.speed = physics::base_speed(2);
        owner.body = vec![owner.head, Point { x: -42., y: 0. }];
        let mut bot = snake(
            Point { x: 120., y: 500. },
            physics::quantize_heading(PI / 2.),
        );
        bot.segments = 30;
        // This diagnostic freezes the line. Use a synthetic stationary
        // obstacle in the independent world too: a moving snake contracts
        // its anchors and is a different timing oracle. This is not a host
        // bot speed or physics-rule change.
        bot.speed = 0.;
        bot.body = (0..30)
            .map(|index| Point {
                x: 120.,
                y: 500. - index as f64 * 42.,
            })
            .collect();
        let dt = physics::LOOP_SECONDS;
        let path = OwnerPath::new(0, &owner, None, dt, Vec::new());
        let predicted = path.frozen_body_contact(&bot, owner.heading, None).unwrap();
        let mut config = crate::Config::sandbox();
        config.food_count = 0;
        config.ambient_replenish_per_second = 0.;
        config.prey.count = 0;
        let mut world = Simulator::from_scenario(
            1,
            config,
            crate::Scenario {
                snakes: [owner.clone(), bot.clone()]
                    .into_iter()
                    .enumerate()
                    .map(|(slot, snake)| {
                        let speed = snake.speed;
                        (
                            slot.to_string(),
                            crate::ReplayLife::from_observed(
                                snake,
                                crate::InitialSpeed::Known(physics::SpeedState::idle(speed)),
                            ),
                        )
                    })
                    .collect(),
                ..Default::default()
            },
        )
        .unwrap();
        let death_tick = (1..=200).find(|_| {
            world
                .step(&[
                    Some(crate::Action {
                        heading: 0.,
                        boost: false,
                    }),
                    Some(crate::Action {
                        heading: PI / 2.,
                        boost: false,
                    }),
                ])
                .unwrap();
            assert_eq!(world.snakes()[1].head, bot.head);
            assert_eq!(world.snakes()[1].heading, bot.heading);
            assert_eq!(world.snakes()[1].body, bot.body);
            !world.snakes()[0].alive
        });
        println!("frozen positive contact_tick={predicted} engine_death_tick={death_tick:?}");
        assert_eq!(death_tick, Some(predicted));

        // Neither distant endpoint overlaps the old anchor tube. The
        // intervening segment still crosses the lethal forward probe.
        bot.segments = 2;
        bot.body = vec![Point { x: 120., y: 70. }, Point { x: 120., y: -70. }];
        assert!(!path.body_conflict(&bot, None));
        assert!(
            path.frozen_body_contact(&bot, owner.heading, None)
                .is_some()
        );
        // Only the tip-to-head segment crosses y=0; all body anchors and the
        // head center are above the endpoint-contact radius.
        // Tilt the crossing so its computed x lies inside a nonzero range.
        // An exactly vertical segment's singleton x comparison can reject
        // this synthetic crossing after the engine's division rounds by an ulp.
        bot.head = Point { x: 120., y: 14.3 };
        bot.heading = -1.5;
        bot.body = vec![Point { x: 120., y: 56.3 }, Point { x: 120., y: 98.3 }];
        assert!(!path.body_conflict(&bot, None));
        let contact = path.frozen_body_contact(&bot, owner.heading, None).unwrap();
        let last = physics::head_tip(path.point(contact - 1), 0., owner.segments);
        let tip = physics::head_tip(path.point(contact), 0., owner.segments);
        assert!(
            !physics::strikes(
                last,
                tip,
                0.,
                owner.segments,
                bot.segments,
                [bot.head].into_iter().chain(bot.body.iter().copied())
            ),
            "omitting the bot tip must lose this contact"
        );
    }

    /// Synthetic reduction of Straight seed 73, tick 632, slot 43. Changing
    /// the held plan's target loses its tick-41 strike. Retaining its point
    /// and advancing the segment identity preserves the original oracle.
    #[test]
    fn host_bots_held_strike_keeps_its_target_when_the_nearest_point_changes() {
        let dt = physics::LOOP_SECONDS;
        let mut owner = snake(
            Point {
                x: 1.159952830116,
                y: 0.,
            },
            0.,
        );
        owner.body = (0..80)
            .map(|i| Point {
                x: -(i as f64) * 42.,
                y: 0.,
            })
            .collect();
        let mut path = OwnerPath::new(0, &owner, None, dt, Vec::new());
        let head = Point {
            x: -18.980620183922,
            y: -64.53070807289916,
        };
        let target = path.target(&owner, head, 330).unwrap();
        assert_eq!(target.segment, 0);
        let speeds = vec![physics::base_speed(2); 330];
        path.extend_bodies(speeds.len());
        let rollout = Rollout {
            head,
            heading: 2.353065413844148,
            segments: 2,
            owner_segments: 80,
            speeds: &speeds,
            release: path.steps(lifetime(2, 0.)),
            contact: path.contact(2),
            target: Some(&target),
        };
        let mut trail = Vec::new();
        let (impact, _) = path.trace(&rollout, 2.357804348586197, &mut trail);
        assert_eq!(impact, None);
        assert_eq!(trail.len(), 37);
        let held = Target {
            point: Point { x: -42., y: 0. },
            segment: 0,
        }
        .after_tick(Point { x: -42., y: 0. }, &owner)
        .unwrap();
        assert_eq!(held.segment, 1);
        assert_eq!(held.point, Point { x: -42., y: 0. });
        let retained = Rollout {
            target: Some(&held),
            ..rollout
        };
        assert_eq!(
            path.plan(&retained, 2.357804348586197, &mut trail),
            Some(Some(41))
        );
        let mut existing = snake(head, rollout.heading);
        existing.segments = 2;
        // The old approach body crosses the owner's lane before the new
        // strike. New-head clearance cannot make that existing body safe.
        existing.body = vec![Point { x: 20., y: 0. }, Point { x: -22., y: 0. }];
        let mut choose = |committed| {
            select_target(Some(target), Some(held), |target| {
                path.plan(
                    &Rollout {
                        target: target.as_ref(),
                        ..rollout
                    },
                    2.357804348586197,
                    &mut trail,
                )
                .filter(|&impact| {
                    path.keeps_body_clear(&existing, committed, 0, impact, rollout.contact)
                })
            })
        };
        // A safe miss remains the normal first choice. When it violates the
        // old commitment, try the retained target with that guard still on.
        assert_eq!(choose(None).unwrap().impact, None);
        let recovered = choose(Some(47)).unwrap();
        assert_eq!(recovered.impact, Some(41));
        assert_eq!(recovered.target.unwrap().segment, 1);
        assert!(
            choose(Some(10)).is_none(),
            "target recovery must not bypass an existing-body rejection"
        );
        let tail = Target {
            point: Point { x: 20000., y: 0. },
            segment: owner.body.len() - 2,
        };
        // The original target can leave the tail before the rollout ends;
        // an absent window neither panics nor manufactures an impact.
        assert_eq!(
            path.trace(
                &Rollout {
                    target: Some(&tail),
                    ..rollout
                },
                0.,
                &mut trail
            )
            .0,
            None
        );
    }

    #[test]
    fn host_bots_predicted_owner_body_matches_the_engine_while_circling() {
        let mut config = crate::Config::sandbox();
        config.food_count = 0;
        config.ambient_replenish_per_second = 0.;
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config,
            population: crate::InitialPopulation::Placed {
                ids: vec!["owner".into()],
                placement: crate::Placement::TestRing,
            },
        })
        .unwrap();
        let dt = world.config().dt;
        let mut owners = Vec::new();
        for _ in 0..1000 {
            let heading = world.snakes()[0].heading + PI / 2.;
            let action = crate::Action {
                heading,
                boost: false,
            };
            world.step(&[Some(action)]).unwrap();
            owners.push(world.snakes()[0].clone());
        }
        let now = 600;
        let history: Vec<_> = owners[..=now].iter().map(|s| (s.head, s.heading)).collect();
        let fit = track(&history).fit(lane_hold());
        let mut predicted = OwnerPath::new(0, &owners[now], fit, dt, Vec::new());
        let ahead = 300;
        predicted.extend_bodies(ahead);
        assert!(predicted.bodies.len() > 5, "several anchor insertions");
        for k in 0..=ahead {
            let actual = &owners[now + k];
            assert!(actual.head.distance(predicted.points[k]) < 1e-6);
            let (_, body) = predicted.body_at(k);
            assert_eq!(body.len(), actual.body.len());
            for (predicted, actual) in body.iter().zip(&actual.body) {
                assert!(predicted.distance(*actual) < 0.01, "tick {k}");
            }
        }
    }

    #[test]
    fn host_bots_arc_distance_matches_dense_samples_of_the_prediction() {
        let dt = physics::LOOP_SECONDS;
        for (index, rate) in [None, Some(0.3), Some(0.)].into_iter().enumerate() {
            let path = drive(rate, 600);
            let &(head, heading) = path.last().unwrap();
            let fit = track(&path).fit(lane_hold());
            let band = fit.map_or(0., |circle| circle.band);
            let mut predicted = OwnerPath::new(0, &snake(head, heading), fit, dt, Vec::new());
            let contact = predicted.contact(2);
            for (from, until) in [(0, 70), (40, 290), (140, 1700), (0, 14_000)] {
                predicted.extend(until / 2);
                let arc = predicted.arc(from, until, contact);
                // Independent oracle: rotate or translate the head per tick.
                let at = |k: usize| match predicted.circle {
                    Some(circle) => {
                        let angle = circle.sign * predicted.turn * k as f64;
                        let (x, y) = (head.x - circle.center.x, head.y - circle.center.y);
                        Point {
                            x: circle.center.x + x * angle.cos() - y * angle.sin(),
                            y: circle.center.y + x * angle.sin() + y * angle.cos(),
                        }
                    }
                    None => Point {
                        x: head.x + k as f64 * predicted.speed * dt * heading.cos(),
                        y: head.y + k as f64 * predicted.speed * dt * heading.sin(),
                    },
                };
                for sample in 0..64 {
                    let point = Point {
                        x: head.x + (sample as f64 * 1.7 + index as f64).sin() * 900.,
                        y: head.y + (sample as f64 * 2.3).cos() * 900.,
                    };
                    let sampled = (from..=until)
                        .map(|k| point.distance(at(k)))
                        .fold(f64::INFINITY, f64::min)
                        - contact;
                    let exact = arc.clearance(point);
                    // Never above the sampled ticks; below them by at most a
                    // tick's travel. The arc runs mid-band, the head anywhere
                    // in the band.
                    assert!(
                        exact <= sampled + band + 1e-6
                            && sampled - exact <= predicted.speed * dt + band,
                        "rate={rate:?} exact={exact} sampled={sampled}"
                    );
                }
            }
        }
    }

    /// The walk past a partly swept circle never clears a run that dense
    /// per-tick checks reject, and clears most runs they accept.
    #[test]
    fn host_bots_straight_runs_past_a_partial_circle_match_dense_samples() {
        let dt = physics::LOOP_SECONDS;
        let rate = 0.0095;
        let path = drive(Some(rate), (FIT_SECONDS / dt) as usize + 1);
        let &(head, heading) = path.last().unwrap();
        let fit = track(&path).fit(lane_hold());
        let circle = fit.unwrap();
        let owner = snake(head, heading);
        let mut predicted = OwnerPath::new(0, &owner, fit, dt, Vec::new());
        predicted.extend(60_000);
        let speeds = [physics::base_speed(10); 1];
        let rollout = Rollout {
            head,
            heading,
            segments: 10,
            owner_segments: 80,
            speeds: &speeds,
            release: predicted.steps(lifetime(10, 0.)),
            contact: predicted.contact(10),
            target: None,
        };
        assert!(predicted.turn * rollout.release as f64 * 2. < TAU);
        let travel = speeds[0] * dt;
        let (mut accepted, mut agreed) = (0, 0);
        for index in 0..400 {
            let angle = index as f64 * 0.37;
            let offset = (index as f64 * 1.3).sin() * 3000.;
            let start = Point {
                x: circle.center.x + (circle.radius + offset) * angle.cos(),
                y: circle.center.y + (circle.radius + offset) * angle.sin(),
            };
            let direction = index as f64 * 2.9;
            let unit = (direction.cos(), direction.sin());
            let walked = predicted.ray_is_clear(&rollout, circle, 1, start, unit, travel);
            let mut dense = true;
            let mut point = start;
            for tick in 1..40_000 {
                let (x, y) = (point.x - circle.center.x, point.y - circle.center.y);
                if x * unit.0 + y * unit.1 >= 0.
                    && length(x, y) - circle.radius > rollout.contact + travel
                {
                    break;
                }
                if predicted
                    .arc(tick, tick + rollout.release, rollout.contact)
                    .clearance(point)
                    <= travel
                {
                    dense = false;
                    break;
                }
                point.x += unit.0 * travel;
                point.y += unit.1 * travel;
            }
            assert!(!walked || dense, "ray {index} cleared by the walk only");
            accepted += usize::from(dense);
            agreed += usize::from(dense && walked);
        }
        assert!(
            accepted > 100 && agreed * 10 >= accepted * 9,
            "{agreed}/{accepted}"
        );
    }

    #[test]
    fn host_bots_ray_segment_distance_matches_dense_samples() {
        let end = Point { x: 400., y: 100. };
        for index in 0..200 {
            let from = Point {
                x: (index as f64 * 1.3).sin() * 600.,
                y: (index as f64 * 0.7).cos() * 600.,
            };
            let direction = Point {
                x: (index as f64 * 2.9).cos() * 50.,
                y: (index as f64 * 3.1).sin() * 50.,
            };
            let scale = 0.25 / length(direction.x, direction.y);
            let sampled = (0..=16_000)
                .map(|t| {
                    let t = t as f64 * scale;
                    let p = Point {
                        x: from.x + t * direction.x,
                        y: from.y + t * direction.y,
                    };
                    segment_distance(p, Point::default(), end)
                })
                .fold(f64::INFINITY, f64::min);
            let exact = ray_segment_distance(from, direction, end);
            assert!(
                exact <= sampled + 1e-9 && sampled - exact <= 0.25,
                "exact={exact} sampled={sampled}"
            );
        }
    }

    #[test]
    fn host_bots_release_bound_covers_body_length_and_pending_growth() {
        for segments in [2, 15, 80, 430] {
            let bound = release_time(segments, 0.);
            // Straight cruise at the slowest speed is a lower bound.
            let straight = (segments + 1) as f64 * physics::ANCHOR_SPACING / physics::base_speed(2);
            assert!(bound >= straight);
            assert!(release_time(segments, 3.5) > bound);
            // The planning reserve reaches the score-table size. That size
            // is not an engine cap; see the regression below.
            let grown = lifetime(segments, 0.);
            assert!(grown >= bound && grown >= release_time(430, 0.));
            assert!(grown <= FIT_SECONDS);
        }
    }

    #[test]
    fn host_bots_growth_reserve_is_not_an_engine_size_cap() {
        assert!(physics::nutrition(5., physics::MAX_SEGMENTS as f64 + 100.) > 0.);
        assert!(lifetime(physics::MAX_SEGMENTS + 1, 0.) > lifetime(physics::MAX_SEGMENTS, 0.));
        assert!(lifetime(2, 10.) > FIT_SECONDS);
    }

    #[test]
    fn host_bots_plans_follow_engine_speed_and_quantized_steering() {
        // A recently grown, nonboosting bot keeps its old idle speed under
        // the default model, but accelerates under direct speed.
        let mut bot = snake(Point::default(), 0.3);
        bot.speed = physics::base_speed(2);
        let far = snake(Point { x: -1e4, y: -1e4 }, 0.);
        let mut path = OwnerPath::new(0, &far, None, physics::LOOP_SECONDS, Vec::new());
        path.extend(600);
        for model in [
            physics::SpeedModel::RetainedProgress,
            physics::SpeedModel::DirectSpeed,
        ] {
            let mut state = physics::SpeedState::from_observed_speed(bot.speed, bot.segments);
            let speeds: Vec<_> = (0..300)
                .map(|_| state.step(false, bot.segments, physics::LOOP_SECONDS, model))
                .collect();
            let retained = model == physics::SpeedModel::RetainedProgress;
            assert_eq!(speeds[0] == speeds[299], retained);
            let rollout = Rollout {
                head: bot.head,
                heading: bot.heading,
                segments: bot.segments,
                owner_segments: 80,
                speeds: &speeds,
                release: 140,
                contact: path.contact(80),
                target: None,
            };
            let aim = 0.65;
            let mut trail = Vec::new();
            assert!(path.plan(&rollout, aim, &mut trail).is_some());
            let (mut head, mut heading) = (bot.head, bot.heading);
            for (&(traced, travel), &speed) in trail.iter().zip(&speeds) {
                (head, heading) = physics::advance(
                    head,
                    heading,
                    physics::quantize_heading(aim),
                    speed,
                    physics::turn_rate(bot.segments),
                    physics::LOOP_SECONDS,
                );
                assert_eq!((traced, travel), (head, speed * physics::LOOP_SECONDS));
            }
            // The plan ends once the bot runs straight on the wanted heading.
            assert!((heading - physics::quantize_heading(aim)).abs() < 1e-9);
        }
    }
}
