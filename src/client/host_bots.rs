//! World-thread host bots. `center` and `follow` slots never connect, decode,
//! receive frames or model latency: the world thread respawns and steers them
//! from world state, so they cost only their steering and physics.
//!
//! Follow bots crash into the owner's body, anywhere along it, including
//! right behind the head. Avoidance is steering only: collisions, death food
//! and every other rule stay the same for everyone. The guarantee covers the
//! owner's steady states only: holding one heading forever, or one turn rate
//! forever. Once the owner has held such a state for longer than the fit
//! window plus the longest bot body lifetime, no follow bot's body lies on the
//! owner's predicted head path while that body exists. A body exists until it
//! is released or its bot dies on the owner. Turns between states and other
//! maneuvers carry no guarantee; see docs/hosting.md#bots-that-follow-the-owner.
//! Synthetic regressions live in simulation/host_bot_tests.rs.
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
/// turn the chord between its anchors sags about 5 units; quantized owner
/// steering and the circle fit stay within the rest (see the fit test).
const MARGIN: f64 = physics::ANCHOR_SPACING / 4.;
/// The owner's path is fitted over the most recent stretch of history that
/// turned at least this far, so a quantized gentle turn still spans several
/// heading steps and gives a stable circle...
const FIT_TURN: f64 = 0.5;
/// ...but over at most this much history.
pub(super) const FIT_SECONDS: f64 = 3.;
/// Anchor contraction can move the struck body line a few units, and the
/// owner and a dying bot are checked in the same tick: a bot's body counts
/// as present this many ticks past its predicted impact.
const IMPACT_SLACK_TICKS: usize = 6;
/// Candidate wanted headings, 15 degrees apart on alternating sides of the aim.
const HEADING_STEPS: usize = 24;
/// Far bots re-aim and dead bots respawn on staggered six-tick decisions.
const DECISION_TICKS: u64 = 6;

pub(super) struct InWorldBots {
    slots: Range<usize>,
    behavior: BotBehavior,
    /// Recent owner head positions for the path fit.
    track: Option<OwnerTrack>,
    /// This tick's owner prediction; `None` for `center` or without an owner.
    path: Option<OwnerPath>,
    /// Each bot's heading toward its target.
    aims: Vec<f64>,
    /// A safe heading other than the aim, kept until the aim is safe again.
    held: Vec<Option<f64>>,
    /// Bots that found no safe heading last tick and are steering out.
    escaping: Vec<bool>,
    /// Rollout speeds and positions of the bot being steered, reused.
    speeds: Vec<f64>,
    trail: Vec<(Point, f64)>,
    /// Off in the sensitivity control: chase the owner's head instead.
    #[cfg(test)]
    pub(super) avoidance: bool,
    /// Bot ticks spent with no safe heading.
    #[cfg(test)]
    pub(super) escape_ticks: u64,
}

impl InWorldBots {
    pub(super) fn new(slots: Range<usize>, behavior: BotBehavior) -> Self {
        Self {
            aims: vec![0.; slots.len()],
            held: vec![None; slots.len()],
            escaping: vec![false; slots.len()],
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
            self.track = None;
            return;
        };
        let snake = &world.snakes()[slot];
        let dt = world.config().dt;
        let track = match &mut self.track {
            Some(track) if track.slot == slot => track,
            track => track.insert(OwnerTrack {
                slot,
                heading: snake.heading,
                history: VecDeque::new(),
            }),
        };
        track.push(snake, (FIT_SECONDS / dt).ceil() as usize);
        self.path = Some(OwnerPath::new(slot, snake, track.fit(), dt, points));
    }

    pub(super) fn born(&mut self, slot: usize, heading: f64) {
        let index = slot - self.slots.start;
        self.aims[index] = heading;
        self.held[index] = None;
        self.escaping[index] = false;
    }

    /// Whether a new bot of this shape starts outside the owner's predicted
    /// path for its whole body lifetime, with room for any first plan.
    pub(super) fn spawn_is_safe(&mut self, body: &[Point], heading: f64) -> bool {
        let Some(path) = &mut self.path else {
            return true;
        };
        let segments = body.len();
        let horizon = path.steps(release_time(segments, 0.)) + path.plan_steps(segments);
        path.extend(horizon);
        let arc = path.arc(0, horizon, path.contact(segments));
        let reach = physics::base_speed(segments) * path.plan_steps(segments) as f64 * path.dt;
        if path.encloses(body[0]) || arc.clearance(body[0]) <= reach {
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
        let bot = &world.snakes()[slot];
        let decide = self.decides(slot, tick);
        let Some(path) = &mut self.path else {
            return decide.then(|| toward(bot.head, Point::default()));
        };
        let owner = &world.snakes()[path.owner];
        #[cfg(test)]
        if !self.avoidance {
            return Some(toward(bot.head, owner.head));
        }
        let release = path.steps(release_time(bot.segments, bot.fam));
        let plan = path.plan_steps(bot.segments);
        let contact = path.contact(bot.segments);
        // Room for approaches that end on the owner's body.
        let limit = 3 * plan;
        path.extend(limit + release);
        path.extend_bodies(limit);
        // In-world bots never boost, so no bot outruns this bound. Beyond it
        // from all the owner covers before this plan's bodies are released,
        // every move of the next plan is safe, and at the next tick the bot
        // still has the half turn it needs to find one.
        let speed = bot.speed.max(physics::base_speed(bot.segments));
        let reach = speed * plan as f64 * path.dt;
        let near = path.arc(0, release + plan, contact).clearance(bot.head) <= reach
            || path.encloses(bot.head);
        if !near {
            self.held[index] = None;
            self.escaping[index] = false;
            if decide {
                let point = path
                    .target(owner, bot.head, limit)
                    .map_or(owner.head, |t| t.point);
                self.aims[index] = toward(bot.head, point);
            }
            return decide.then_some(self.aims[index]);
        }
        // A body too short to target leaves the head to aim for.
        let target = path.target(owner, bot.head, limit);
        let aim = toward(bot.head, target.as_ref().map_or(owner.head, |t| t.point));
        self.aims[index] = aim;
        let mut state = physics::SpeedState::from_observed_speed(bot.speed, bot.segments);
        let model = world.config().speed_model;
        self.speeds.clear();
        self.speeds
            .extend((0..limit).map(|_| state.step(false, bot.segments, path.dt, model)));
        let bot = Rollout {
            head: bot.head,
            heading: bot.heading,
            segments: bot.segments,
            owner_segments: owner.segments,
            speeds: &self.speeds,
            release,
            contact,
            target: target.as_ref(),
        };
        let trail = &mut self.trail;
        // Hysteresis: go straight for the target whenever that is safe,
        // otherwise keep the last safe detour while it stays safe, and only
        // then search the other headings closest-first.
        let chosen = if path.safe(&bot, aim, trail) {
            Some(None)
        } else if let Some(held) = self.held[index].filter(|&held| path.safe(&bot, held, trail)) {
            Some(Some(held))
        } else {
            candidates(aim)
                .skip(1)
                .find(|&wanted| path.safe(&bot, wanted, trail))
                .map(Some)
        };
        if let Some(detour) = chosen {
            self.held[index] = detour;
            self.escaping[index] = false;
            return Some(detour.unwrap_or(aim));
        }
        // Only outside the steady states: no heading is safe. The fastest
        // way out changes slowly, so rank escapes when first trapped and on
        // decision ticks, and keep heading out in between.
        #[cfg(test)]
        {
            self.escape_ticks += 1;
        }
        self.held[index] = None;
        if self.escaping[index] && !decide {
            return None;
        }
        self.escaping[index] = true;
        Some(path.escape(&bot, aim, plan))
    }
}

/// The owner's recent head positions, with its heading unwrapped so turns
/// accumulate across the ±π boundary.
struct OwnerTrack {
    slot: usize,
    heading: f64,
    history: VecDeque<(Point, f64)>,
}

impl OwnerTrack {
    fn push(&mut self, owner: &Snake, capacity: usize) {
        let turned = self.history.back().map_or(0., |&(_, turned)| {
            turned + wrap(owner.heading - self.heading)
        });
        self.heading = owner.heading;
        if self.history.len() > capacity {
            self.history.pop_front();
        }
        self.history.push_back((owner.head, turned));
    }

    /// A circle through the oldest, middle and newest positions of the most
    /// recent stretch that turned at least `FIT_TURN`, or `None` for a lane.
    /// Positions, not headings: quantized steering moves the heading in
    /// steps, but the path between them stays within a fraction of a unit of
    /// one circle, so three well-spread points pin it down.
    fn fit(&self) -> Option<Circle> {
        let &(now, turned) = self.history.back()?;
        let start = self
            .history
            .iter()
            .rposition(|&(_, earlier)| (turned - earlier).abs() >= FIT_TURN)
            .unwrap_or(0);
        // A heading held exactly is an exact lane.
        if self.history[start].1 == turned {
            return None;
        }
        let (a, b) = (
            self.history[start].0,
            self.history[(start + self.history.len() - 1) / 2].0,
        );
        circumcircle(a, b, now)
    }
}

fn circumcircle(a: Point, b: Point, c: Point) -> Option<Circle> {
    let (bx, by) = (b.x - a.x, b.y - a.y);
    let (cx, cy) = (c.x - a.x, c.y - a.y);
    let cross = bx * cy - by * cx;
    let (b2, c2) = (bx * bx + by * by, cx * cx + cy * cy);
    let x = (cy * b2 - by * c2) / (2. * cross);
    let y = (bx * c2 - cx * b2) / (2. * cross);
    let radius = length(x, y);
    // Beyond this the arc over any horizon used here is a lane to well
    // under a unit.
    (cross != 0. && radius < 1e9).then(|| Circle {
        center: Point {
            x: a.x + x,
            y: a.y + y,
        },
        radius,
        sign: cross.signum(),
    })
}

/// The owner's head path at whole ticks from now, under its fitted lane or
/// circle and current speed. Samples extend on demand within one world tick.
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

/// Center and radius of the predicted circle; `sign` is the turn direction.
#[derive(Clone, Copy)]
struct Circle {
    center: Point,
    radius: f64,
    sign: f64,
}

/// The owner body point a bot aims for and the body line around it.
struct Target {
    point: Point,
    /// The body segment from anchor `segment` to the next, as indexed now.
    segment: usize,
}

/// One bot's state and limits for planning its next ticks.
struct Rollout<'a> {
    head: Point,
    heading: f64,
    segments: usize,
    owner_segments: usize,
    /// Engine speed at each tick of the longest plan.
    speeds: &'a [f64],
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
        let speed = snake.speed;
        // The engine moves the head by one chord per tick.
        let turn = circle.map_or(0., |circle| {
            2. * (speed * dt / (2. * circle.radius)).min(1.).asin()
        });
        let step = match circle {
            Some(circle) => ((circle.sign * turn).cos(), (circle.sign * turn).sin()),
            None => (snake.heading.cos(), snake.heading.sin()),
        };
        points.clear();
        points.push(snake.head);
        Self {
            owner,
            speed,
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

    /// The tube's constant half-width for a bot of this size: the owner's
    /// probe reaches past the bot's centerline by the bot radius, the bot's
    /// tip leads its center by another, the owner moves one tick between
    /// checks, plus the fixed margin. A bot's own tick of motion is checked
    /// separately in each plan.
    fn contact(&self, segments: usize) -> f64 {
        physics::collision_distance(self.segments, segments)
            + physics::BODY_RADIUS * physics::scale(segments)
            + self.speed * self.dt
            + MARGIN
    }

    /// Inside a predicted circle no straight line stays clear forever.
    fn encloses(&self, point: Point) -> bool {
        self.circle
            .is_some_and(|circle| point.distance(circle.center) < circle.radius)
    }

    /// The tube around the owner's predicted head between two sampled ticks.
    fn arc(&self, from: usize, until: usize, contact: f64) -> Arc {
        Arc {
            a: self.points[from],
            b: self.points[until],
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

    /// Whether steering toward `wanted` keeps every body tick the bot lays
    /// out of the owner's path while that body exists. The plan follows the
    /// engine's kinematics until the bot strikes its target, ending its body,
    /// or until it runs straight; that straight line must then stay clear
    /// forever, so the same plan is still safe at the next tick.
    fn safe(&self, bot: &Rollout, wanted: f64, trail: &mut Vec<(Point, f64)>) -> bool {
        let (impact, heading) = self.trace(bot, wanted, trail);
        let clear = (1..).zip(trail.iter()).all(|(step, &(head, travel))| {
            let gone = impact.map_or(usize::MAX, |impact| impact + IMPACT_SLACK_TICKS);
            let until = (step + bot.release).min(gone);
            self.arc(step, until, bot.contact).clearance(head) > travel
        });
        clear && (impact.is_some() || self.straight_is_clear(bot, trail, heading))
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
                let line = body[at.saturating_sub(1)..(at + 3).min(body.len())]
                    .iter()
                    .copied();
                if physics::strikes(last, tip, heading, bot.segments, bot.owner_segments, line) {
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
    /// out of the owner's path: the whole circle, or in the owner's moving
    /// frame, the lane stretch the owner covers while that body lasts.
    fn straight_is_clear(&self, bot: &Rollout, trail: &[(Point, f64)], heading: f64) -> bool {
        let Some(&(head, travel)) = trail.last() else {
            return false;
        };
        let (cos, sin) = (heading.cos(), heading.sin());
        let room = bot.contact + travel;
        match self.circle {
            Some(Circle { center, radius, .. }) => {
                let (x, y) = (head.x - center.x, head.y - center.y);
                let ahead = (-(x * cos + y * sin)).max(0.);
                let closest = length(x + ahead * cos, y + ahead * sin);
                length(x, y) >= radius && closest - radius > room
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

/// Seconds until the newest anchor leaves the body. An anchor is released
/// after that many *insertions*, including pending growth and one partial
/// anchor; contraction can shorten the drawn body but not delay insertions.
/// Each insertion follows a 42-unit chord, bounded as an arc at the tightest
/// turn and the slowest speed any snake moves: the smallest cruise speed.
pub(super) fn release_time(segments: usize, fam: f64) -> f64 {
    let speed = physics::base_speed(2);
    let turn = physics::turn_rate(segments);
    let per_anchor = 2. * (physics::ANCHOR_SPACING * turn / (2. * speed)).asin() / turn;
    (segments as f64 + fam.ceil() + 1.) * per_anchor
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

    fn track(path: &[(Point, f64)]) -> OwnerTrack {
        let mut track = OwnerTrack {
            slot: 0,
            heading: path[0].1,
            history: VecDeque::new(),
        };
        for &(head, heading) in path {
            let capacity = (FIT_SECONDS / physics::LOOP_SECONDS).ceil() as usize;
            track.push(&snake(head, heading), capacity);
        }
        track
    }

    #[test]
    fn host_bots_steady_paths_fit_a_lane_or_one_circle() {
        let dt = physics::LOOP_SECONDS;
        let horizon = (4. / dt) as usize;
        let settle = (FIT_SECONDS / dt) as usize;
        let max = physics::turn_rate(80);
        for rate in [Some(0.), None, Some(max / 2.), Some(0.3), Some(0.1)] {
            let path = drive(rate, settle + horizon + 600);
            let (mut lateral, mut along) = (0f64, 0f64);
            // Judge steady fits only, against the owner's real future.
            for tick in (settle..path.len() - horizon).step_by(25) {
                let fit = track(&path[..=tick]).fit();
                assert_eq!(fit.is_none(), rate == Some(0.), "rate={rate:?}");
                let (head, heading) = path[tick];
                let mut predicted = OwnerPath::new(0, &snake(head, heading), fit, dt, Vec::new());
                predicted.extend(horizon);
                for k in 0..=horizon {
                    let actual = path[tick + k].0;
                    let off = match fit {
                        Some(circle) => (actual.distance(circle.center) - circle.radius).abs(),
                        None => {
                            segment_distance(actual, predicted.points[0], predicted.points[horizon])
                        }
                    };
                    lateral = lateral.max(off);
                    along = along.max(actual.distance(predicted.points[k]));
                }
            }
            // Sideways error stays inside the fixed margin; along the path
            // the tube's rounded ends (the contact distance) absorb the rest.
            assert!(lateral < MARGIN / 2., "rate={rate:?} lateral={lateral}");
            assert!(along < 40., "rate={rate:?} along={along}");
            if rate == Some(0.) || rate.is_none() {
                assert!(
                    lateral < 1e-6 && along < 1e-6,
                    "rate={rate:?} {lateral} {along}"
                );
            }
        }
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
        let mut predicted = OwnerPath::new(0, &owners[now], track(&history).fit(), dt, Vec::new());
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
            let fit = track(&path).fit();
            let mut predicted = OwnerPath::new(0, &snake(head, heading), fit, dt, Vec::new());
            let contact = predicted.contact(2);
            for (from, until) in [(0, 70), (40, 290), (140, 1700), (0, 14_000)] {
                predicted.extend(until);
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
                    // tick's travel.
                    assert!(
                        exact <= sampled + 1e-6 && sampled - exact <= predicted.speed * dt,
                        "rate={rate:?} exact={exact} sampled={sampled}"
                    );
                }
            }
        }
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
        }
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
            assert!(path.safe(&rollout, aim, &mut trail));
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
