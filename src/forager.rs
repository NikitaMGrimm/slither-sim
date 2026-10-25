//! Latency-aware food forager using only one bot's received observation.
//!
//! Ported from the owner's `slither-arena-rs` Forager
//! (`src/bots/scripts.rs`, `forager`) and its Smith predictor
//! (`src/bots/timing.rs`) at commit `14e9fc47bfbadabe3b782c5bf0d5ee5ec2b0b26e`.
//! With [`Params::default`] and a zero [`Timing`] lead, decisions match that
//! Forager exactly. Every tunable number is a [`Params`] field; the defaults are
//! the original constants, which are policy choices rather than server rules.
//!
//! The kernel keeps no state between decisions. [`Scratch`] holds reusable
//! buffers so that steady-state decisions allocate no heap memory.

use crate::{
    Action, Arena, BoundaryMode, Observation, Point, Snake, SpeedModel, SpeedState, boundary,
    physics::{
        BODY_RADIUS, advance, base_speed, pickup_circle, scale, segment_distance, swept_distance,
        turn_rate,
    },
};

/// Largest food grid side; [`Params::cells`] may use fewer.
pub const MAX_CELLS: usize = 17;
/// Largest steering fan; [`Params::fan`] may use fewer.
pub const MAX_FAN: usize = 16;
/// Remembered sent commands used to replay the bot's own motion; the
/// portable decoder keeps the same number.
pub const MAX_SENT: usize = 64;
/// Own-motion forecast step, as in the original predictor (one protocol unit).
pub const SUBSTEP: f64 = 0.008;
/// Longest forecast lead, in substeps (2 seconds).
pub const MAX_LEAD_SUBSTEPS: usize = 250;

/// Every tunable number of the policy. Defaults reproduce the original Forager.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Side of a square food-pooling cell, world units.
    pub cell: f64,
    /// Cells per side of the grid centred on the head (at most [`MAX_CELLS`]).
    pub cells: usize,
    /// Seconds added to every goal's travel time; prefers rich over slightly nearer.
    pub settle_seconds: f64,
    /// Value factor for cells that bodies or oncoming heads occupy.
    pub occupied_factor: f64,
    /// A point is unreachable inside `orbit_factor × radius × sin(angle)`.
    pub orbit_factor: f64,
    /// Food this close to the rim (beyond the bot's width) is ignored.
    pub rim_food_margin: f64,
    /// Opponent head paths mark occupied cells over this many seconds ...
    pub head_path_seconds: f64,
    /// ... in this many equal steps (plus the current head).
    pub head_path_steps: usize,
    /// Steering arcs are checked this far ahead, seconds.
    pub lookahead_seconds: f64,
    /// Extra obstacle-gathering radius beyond the arc and two widths.
    pub reach_extra: f64,
    /// Seconds of straight travel past the arc's end checked against the rim ...
    pub rim_ahead_seconds: f64,
    /// ... with this much clearance credit.
    pub rim_ahead_credit: f64,
    /// Clearance below which an arc counts as dangerous.
    pub safe: f64,
    /// Cost per unit of clearance below `safe`, in radians of offset.
    pub danger_weight: f64,
    /// With no food in view, head for the centre beyond this share of the radius.
    pub roam_fraction: f64,
    /// Heading offsets tried in order; ascending magnitudes allow early exit.
    pub fan: [f64; MAX_FAN],
    pub fan_len: usize,
    /// Share of the measured round trip spent on the uplink.
    pub uplink_share: f64,
    /// Multiplies the forecast length; 0 disables prediction. Command arrival
    /// times stay as measured.
    pub lead_scale: f64,
    /// Obstacle widening per second of lead uncertainty and unit of closing speed.
    pub uncertainty_margin: f64,
    /// Share of each opponent's reported turn toward its wanted heading that
    /// its predicted path follows; 0 continues straight (the original).
    pub intent_weight: f64,
    /// Obstacle widening per second of an opponent's heading-receipt age and
    /// unit of its speed.
    pub stale_margin: f64,
    /// Share of our own width counted beside the arc. Only a head's forward
    /// probe kills (`physics::strikes`), so below 1 the arc is checked from the
    /// head's tip, shifted forward by the rest of the width. 1 is the original.
    pub own_width_factor: f64,
    /// Value of a cut-off per unit of victim mass (segments + fam), compared
    /// with food value; 0 disables kill seeking.
    pub kill_weight: f64,
    /// Opponents whose head is this far away are not cut off.
    pub kill_range: f64,
    /// Cut points are sampled along a victim's predicted path up to this many
    /// seconds ahead ...
    pub kill_horizon: f64,
    /// ... at this many equal steps.
    pub kill_samples: usize,
    /// We must cross a cut point at least this many seconds before the victim.
    pub kill_gap: f64,
    /// ... and at most this many, or it has time to turn away.
    pub kill_window: f64,
    /// Value of encircling a smaller snake per unit of its mass, compared with
    /// food value; 0 disables coiling.
    pub coil_weight: f64,
    /// Only snakes with at most this share of our mass are encircled ...
    pub coil_mass_ratio: f64,
    /// ... whose head is within this distance.
    pub coil_range: f64,
    /// Orbit radius: this many summed widths plus the target's turning radius.
    pub coil_radius_widths: f64,
    /// The orbit is steered toward a point this many radians ahead of us.
    pub coil_lead_angle: f64,
    /// Once our body wraps this many radians around the target, the orbit
    /// tightens by `coil_shrink`; body contraction then closes it further.
    pub coil_close_angle: f64,
    pub coil_shrink: f64,
}

impl Default for Params {
    fn default() -> Self {
        let mut fan = [0.0; MAX_FAN];
        let original = [0.0, -0.3, 0.3, -0.6, 0.6, -1.0, 1.0, -1.5, 1.5, -2.2, 2.2];
        fan[..original.len()].copy_from_slice(&original);
        Self {
            cell: 150.0,
            cells: 13,
            settle_seconds: 0.5,
            occupied_factor: 0.25,
            orbit_factor: 1.8,
            rim_food_margin: 40.0,
            head_path_seconds: 1.0,
            head_path_steps: 4,
            lookahead_seconds: 0.45,
            reach_extra: 60.0,
            rim_ahead_seconds: 1.0,
            rim_ahead_credit: 60.0,
            safe: 30.0,
            danger_weight: 0.3,
            roam_fraction: 0.5,
            fan,
            fan_len: original.len(),
            uplink_share: 0.5,
            lead_scale: 1.0,
            uncertainty_margin: 1.0,
            intent_weight: 0.0,
            stale_margin: 0.0,
            own_width_factor: 1.0,
            kill_weight: 0.0,
            kill_range: 450.0,
            kill_horizon: 0.8,
            kill_samples: 4,
            kill_gap: 0.15,
            kill_window: 0.5,
            coil_weight: 0.0,
            coil_mass_ratio: 0.5,
            coil_range: 600.0,
            coil_radius_widths: 1.5,
            coil_lead_angle: 0.6,
            coil_close_angle: 4.7,
            coil_shrink: 0.7,
        }
    }
}

impl Params {
    pub fn validate(&self) -> Result<(), String> {
        let finite = [
            self.cell,
            self.settle_seconds,
            self.occupied_factor,
            self.orbit_factor,
            self.rim_food_margin,
            self.head_path_seconds,
            self.lookahead_seconds,
            self.reach_extra,
            self.rim_ahead_seconds,
            self.rim_ahead_credit,
            self.safe,
            self.danger_weight,
            self.roam_fraction,
            self.uplink_share,
            self.lead_scale,
            self.uncertainty_margin,
            self.intent_weight,
            self.stale_margin,
            self.own_width_factor,
            self.kill_weight,
            self.kill_range,
            self.kill_horizon,
            self.kill_gap,
            self.kill_window,
            self.coil_weight,
            self.coil_mass_ratio,
            self.coil_range,
            self.coil_radius_widths,
            self.coil_lead_angle,
            self.coil_close_angle,
            self.coil_shrink,
        ];
        if finite.iter().any(|v| !v.is_finite())
            || self.fan[..self.fan_len.min(MAX_FAN)]
                .iter()
                .any(|v| !v.is_finite())
        {
            return Err("Forager parameters must be finite".into());
        }
        if !(1..=MAX_CELLS).contains(&self.cells)
            || !(1..=MAX_FAN).contains(&self.fan_len)
            || self.cell <= 0.0
            || self.lookahead_seconds <= 0.0
            || !(0.0..=1.0).contains(&self.uplink_share)
            || self.lead_scale < 0.0
            || self.uncertainty_margin < 0.0
            || !(0.0..=1.0).contains(&self.intent_weight)
            || self.stale_margin < 0.0
            || !(0.0..=1.0).contains(&self.own_width_factor)
            || self.kill_weight < 0.0
            || self.coil_weight < 0.0
            || !(0.0..=1.0).contains(&self.coil_shrink)
            || self.kill_samples > 16
        {
            return Err(format!(
                "Forager needs 1..={MAX_CELLS} cells, 1..={MAX_FAN} fan offsets, a positive cell \
                 and lookahead, an uplink share, intent weight and own-width factor in 0..=1 and nonnegative \
                 lead, uncertainty and stale margins"
            ));
        }
        Ok(())
    }
}

/// A command this client sent, `seconds_ago` before the decision.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sent {
    pub seconds_ago: f64,
    pub action: Action,
}

/// Received knowledge about one opponent, aligned with `Observation::opponents`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intent {
    /// Latest reported wanted heading in radians; NaN when unknown.
    pub wanted: f64,
    /// Seconds since its heading was last received.
    pub heading_age: f64,
}

impl Default for Intent {
    fn default() -> Self {
        Self {
            wanted: f64::NAN,
            heading_age: 0.0,
        }
    }
}

/// What the client knows about its connection for one decision.
///
/// The observed state is `observation_age` server seconds old. A command takes
/// `command_delay` seconds from leaving the client to acting; the command
/// decided now first waits `processing` seconds before it leaves. Commands
/// already sent carry their actual departure ages, so replay delays them by
/// `command_delay` only. `uncertainty` is the spread of the total lead, for
/// example the recent ping range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub observation_age: f64,
    pub command_delay: f64,
    pub processing: f64,
    pub uncertainty: f64,
    /// Most recent first; only the first `sent_len` entries are used.
    pub sent: [Sent; MAX_SENT],
    pub sent_len: usize,
}

impl Default for Timing {
    fn default() -> Self {
        Self {
            observation_age: 0.0,
            command_delay: 0.0,
            processing: 0.0,
            uncertainty: 0.0,
            sent: [Sent::default(); MAX_SENT],
            sent_len: 0,
        }
    }
}

impl Timing {
    /// The original predictor's split of a measured round trip into equal halves.
    pub fn received(ping_seconds: f64) -> Self {
        Self {
            observation_age: ping_seconds / 2.0,
            command_delay: ping_seconds / 2.0,
            ..Self::default()
        }
    }
    /// Append a sent command, most recent first; extras beyond [`MAX_SENT`] are dropped.
    pub fn push_sent(&mut self, seconds_ago: f64, action: Action) {
        if self.sent_len < MAX_SENT {
            self.sent[self.sent_len] = Sent {
                seconds_ago,
                action,
            };
            self.sent_len += 1;
        }
    }
    fn sent(&self) -> &[Sent] {
        &self.sent[..self.sent_len.min(MAX_SENT)]
    }
}

/// Reusable decision buffers. Construct once per bot; contents never carry
/// information between decisions.
#[derive(Clone, Debug)]
pub struct Scratch {
    near: Vec<Obstacle>,
    sweep: Vec<Point>,
}

impl Default for Scratch {
    fn default() -> Self {
        Self {
            near: Vec::with_capacity(4096),
            sweep: Vec::with_capacity(MAX_LEAD_SUBSTEPS + 1),
        }
    }
}

/// Seek the richest food per second of travel along the clearest short arc.
///
/// The observation is first advanced by the timing's lead: the bot's own head
/// replays the commands in flight, opponents continue straight, and pellets its
/// pickup circle sweeps meanwhile are treated as eaten. This heuristic does not
/// guarantee collision avoidance. Boost is always off.
///
/// `intents` holds received knowledge per opponent in observation order; it may
/// be shorter (missing entries are unknown) or empty.
pub fn decide(
    o: &Observation,
    timing: &Timing,
    intents: &[Intent],
    p: &Params,
    scratch: &mut Scratch,
) -> Action {
    let lead = timing.observation_age + timing.command_delay + timing.processing;
    let lead_steps = (lead * p.lead_scale / SUBSTEP)
        .round()
        .clamp(0.0, MAX_LEAD_SUBSTEPS as f64) as usize;
    let lead = lead_steps as f64 * SUBSTEP;
    scratch.sweep.clear();
    let mut me = Me::new(&o.me);
    if lead_steps > 0 {
        let motion = replay(&o.me, &o.arena, timing, lead_steps, |m| {
            scratch
                .sweep
                .push(pickup_circle(m.head, m.heading, m.speed.speed, o.me.segments).0)
        });
        me.head = motion.head;
        me.heading = motion.heading;
        me.speed = motion.speed.speed.max(base_speed(o.me.segments));
    }
    let pickup_radius = pickup_circle(me.head, me.heading, me.speed, o.me.segments).1;
    let view = View {
        o,
        lead,
        sweep: &scratch.sweep,
        sweep_bounds: bounds(&scratch.sweep),
        pickup_radius,
        uncertainty: timing.uncertainty * p.uncertainty_margin,
    };
    let scene = Scene::new(&view, &me, p, intents, &mut scratch.near);
    let mut goal = scene.food(&view, &me, p).unwrap_or_else(|| roam(o, &me, p));
    if p.kill_weight > 0.0 {
        if let Some(cut) = cut(&view, &me, p, intents) {
            if cut.rate > goal.rate {
                goal = cut;
            }
        }
    }
    if p.coil_weight > 0.0 {
        if let Some(coil) = coil(&view, &me, p) {
            if coil.rate > goal.rate {
                goal = coil;
            }
        }
    }
    steer(o, &me, p, &scratch.near, goal)
}

fn wrap(a: f64) -> f64 {
    a.sin().atan2(a.cos())
}
fn toward(from: Point, to: Point) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}
fn forward(p: Point, heading: f64, distance: f64) -> Point {
    Point {
        x: p.x + heading.cos() * distance,
        y: p.y + heading.sin() * distance,
    }
}
/// Head position after `t` seconds of turning by `delta` radians at `turn`
/// radians per second, then continuing straight: the continuous form of
/// repeated `physics::advance` toward a fixed target.
fn turn_then_straight(
    head: Point,
    heading: f64,
    delta: f64,
    speed: f64,
    turn: f64,
    t: f64,
) -> Point {
    if delta == 0.0 || turn <= 0.0 {
        return forward(head, heading, speed * t);
    }
    let side = delta.signum();
    let arc = (delta.abs() / turn).min(t);
    let radius = speed / turn;
    let end = heading + side * turn * arc;
    let p = Point {
        x: head.x + side * radius * (end.sin() - heading.sin()),
        y: head.y - side * radius * (end.cos() - heading.cos()),
    };
    forward(p, end, speed * (t - arc))
}

fn width(s: &Snake) -> f64 {
    BODY_RADIUS * scale(s.segments)
}

#[derive(Clone, Copy)]
struct Motion {
    head: Point,
    heading: f64,
    speed: SpeedState,
}

/// Own motion through the lead, steering by the command in force at each
/// substep (the original predictor's `Timing::replay`). A command sent
/// `seconds_ago` arrives `observation_age - seconds_ago + command_delay` after
/// the observed state; before every remembered command the oldest is the best
/// guess, and with none the observed heading is held.
fn replay(
    me: &Snake,
    arena: &Arena,
    timing: &Timing,
    steps: usize,
    mut each: impl FnMut(&Motion),
) -> Motion {
    let segments = me.segments;
    let mut motion = Motion {
        head: me.head,
        heading: me.heading,
        speed: SpeedState::from_observed_speed(me.speed, segments),
    };
    let hold = Action {
        heading: me.heading,
        boost: false,
    };
    let sent = timing.sent();
    let age = timing.observation_age;
    let delay = timing.command_delay;
    let turn = turn_rate(segments);
    let sc = scale(segments);
    each(&motion);
    for step in 0..steps {
        let at = step as f64 * SUBSTEP;
        let command = sent
            .iter()
            .find(|c| age - c.seconds_ago + delay <= at + 1e-9)
            .or(sent.last())
            .map_or(hold, |c| c.action);
        let speed = motion
            .speed
            .step(command.boost, segments, SUBSTEP, SpeedModel::default());
        (motion.head, motion.heading) = advance(
            motion.head,
            motion.heading,
            command.heading,
            speed,
            turn,
            SUBSTEP,
        );
        let (point, _) = boundary::constrain(
            Point {
                x: motion.head.x - arena.center.x,
                y: motion.head.y - arena.center.y,
            },
            motion.heading,
            sc,
            arena.radius,
            BoundaryMode::Sliding,
        );
        motion.head = Point {
            x: point.x + arena.center.x,
            y: point.y + arena.center.y,
        };
        each(&motion);
    }
    motion
}

fn bounds(points: &[Point]) -> (Point, Point) {
    let Some(&first) = points.first() else {
        return (Point::default(), Point::default());
    };
    points.iter().fold((first, first), |(lo, hi), p| {
        (
            Point {
                x: lo.x.min(p.x),
                y: lo.y.min(p.y),
            },
            Point {
                x: hi.x.max(p.x),
                y: hi.y.max(p.y),
            },
        )
    })
}

/// The observation as seen at the forecast moment, without copying it.
struct View<'a> {
    o: &'a Observation,
    lead: f64,
    /// Own pickup-circle centres through the lead; empty without a lead.
    sweep: &'a [Point],
    sweep_bounds: (Point, Point),
    pickup_radius: f64,
    /// Lead uncertainty already multiplied by its margin parameter, seconds.
    uncertainty: f64,
}

impl View<'_> {
    /// Whether our pickup circle swept this pellet during the lead.
    fn eaten(&self, p: Point) -> bool {
        if self.sweep.len() < 2 {
            return false;
        }
        let (lo, hi) = self.sweep_bounds;
        let r = self.pickup_radius;
        if p.x < lo.x - r || p.x > hi.x + r || p.y < lo.y - r || p.y > hi.y + r {
            return false;
        }
        self.sweep
            .windows(2)
            .any(|w| segment_distance(p, w[0], w[1]) <= r)
    }
    /// An opponent's head at the forecast moment, continuing straight.
    fn head(&self, s: &Snake) -> Point {
        if self.lead > 0.0 {
            forward(s.head, s.heading, s.speed * self.lead)
        } else {
            s.head
        }
    }
}

struct Me {
    head: Point,
    heading: f64,
    speed: f64,
    turn: f64,
    width: f64,
}

impl Me {
    fn new(me: &Snake) -> Self {
        Self {
            head: me.head,
            heading: me.heading,
            speed: me.speed.max(base_speed(me.segments)),
            turn: turn_rate(me.segments),
            width: width(me),
        }
    }
    /// Seconds to reach `p`, or None inside the tightest turning circle on its
    /// side: chasing such a point makes a snake orbit it.
    fn reach(&self, p: Point, orbit_factor: f64) -> Option<f64> {
        let distance = p.distance(self.head);
        let angle = wrap(toward(self.head, p) - self.heading).abs();
        let radius = self.speed / self.turn;
        (distance >= orbit_factor * radius * angle.sin())
            .then(|| distance / self.speed + angle / self.turn)
    }
}

struct Goal {
    heading: f64,
    rate: f64,
    /// Opponent whose head path this goal deliberately crosses ahead of.
    crosses: Option<usize>,
}

/// A visible segment near enough to matter for the next arc, with its midpoint
/// and half length for a cheap distance lower bound.
#[derive(Clone, Copy, Debug)]
struct Obstacle {
    from: Point,
    to: Point,
    width: f64,
    mid: Point,
    half: f64,
    /// Opponent index, and whether this is its predicted head path.
    owner: usize,
    head: bool,
}

impl Obstacle {
    fn new(from: Point, to: Point, width: f64, owner: usize, head: bool) -> Self {
        Self {
            from,
            to,
            width,
            owner,
            head,
            mid: Point {
                x: (from.x + to.x) / 2.0,
                y: (from.y + to.y) / 2.0,
            },
            half: from.distance(to) / 2.0,
        }
    }
}

/// Food pooled in cells around the head and cells that bodies or oncoming
/// heads occupy.
struct Scene {
    cells: [(f64, f64, f64); MAX_CELLS * MAX_CELLS],
    occupied: [bool; MAX_CELLS * MAX_CELLS],
    n: usize,
}

impl Scene {
    fn new(view: &View, me: &Me, p: &Params, intents: &[Intent], near: &mut Vec<Obstacle>) -> Self {
        let o = view.o;
        let n = p.cells.min(MAX_CELLS);
        let half = n as f64 * p.cell / 2.0;
        let cell = |q: Point| {
            let x = q.x - me.head.x + half;
            let y = q.y - me.head.y + half;
            ((0.0..2.0 * half).contains(&x) && (0.0..2.0 * half).contains(&y)).then(|| {
                ((y / p.cell) as usize).min(n - 1) * n + ((x / p.cell) as usize).min(n - 1)
            })
        };
        let mut scene = Self {
            cells: [(0.0, 0.0, 0.0); MAX_CELLS * MAX_CELLS],
            occupied: [false; MAX_CELLS * MAX_CELLS],
            n,
        };
        for food in &o.food {
            if let Some(i) = cell(food.point()) {
                if view.eaten(food.point()) {
                    continue;
                }
                let c = &mut scene.cells[i];
                c.0 += food.value;
                c.1 += food.value * food.x;
                c.2 += food.value * food.y;
            }
        }
        near.clear();
        let reach = me.speed * p.lookahead_seconds + 2.0 * me.width + p.reach_extra;
        let near_within =
            |q: Point, reach2: f64| (q.x - me.head.x).powi(2) + (q.y - me.head.y).powi(2) < reach2;
        let steps = p.head_path_steps.max(1);
        for (k, enemy) in o.opponents.iter().enumerate() {
            if !enemy.alive {
                continue;
            }
            let intent = intents.get(k).copied().unwrap_or_default();
            let turning = p.intent_weight > 0.0 && intent.wanted.is_finite();
            let unsure = view.uncertainty;
            let stale = p.stale_margin * intent.heading_age * enemy.speed;
            let width = p.own_width_factor * me.width + width(enemy) + unsure * me.speed + stale;
            // The gathering radius assumes two of our widths; widened envelopes
            // (uncertainty, stale data) extend it so no reachable segment is dropped.
            let extra = (width + unsure * enemy.speed - me.width - self::width(enemy)).max(0.0);
            let reach2 = (reach + extra) * (reach + extra);
            let close = |q: Point| near_within(q, reach2);
            let head = if turning {
                // Turning toward the reported wanted heading at its turn rate.
                let delta = p.intent_weight * wrap(intent.wanted - enemy.heading);
                let turn = turn_rate(enemy.segments);
                let path = |t: f64| {
                    turn_then_straight(enemy.head, enemy.heading, delta, enemy.speed, turn, t)
                };
                let head = if view.lead > 0.0 {
                    path(view.lead)
                } else {
                    enemy.head
                };
                for step in 0..=steps {
                    let t = p.head_path_seconds * step as f64 / steps as f64;
                    if let Some(i) = cell(path(view.lead + t)) {
                        scene.occupied[i] = true;
                    }
                }
                let middle = path(view.lead + p.lookahead_seconds / 2.0);
                let ahead = path(view.lead + p.lookahead_seconds);
                if close(head) || close(middle) || close(ahead) {
                    let w = width + unsure * enemy.speed;
                    near.push(Obstacle::new(head, middle, w, k, true));
                    near.push(Obstacle::new(middle, ahead, w, k, true));
                }
                head
            } else {
                // The original straight-line forecast.
                let head = view.head(enemy);
                for step in 0..=steps {
                    let q = forward(
                        head,
                        enemy.heading,
                        enemy.speed * p.head_path_seconds * step as f64 / steps as f64,
                    );
                    if let Some(i) = cell(q) {
                        scene.occupied[i] = true;
                    }
                }
                let ahead = forward(head, enemy.heading, enemy.speed * p.lookahead_seconds);
                if close(head) || close(ahead) {
                    near.push(Obstacle::new(
                        head,
                        ahead,
                        width + unsure * enemy.speed,
                        k,
                        true,
                    ));
                }
                head
            };
            // During the lead the observed head became body behind the forecast head.
            let mut previous = head;
            let mut was_close = close(previous);
            let observed = (view.lead > 0.0).then_some(enemy.head);
            for point in observed.into_iter().chain(enemy.body.iter().copied()) {
                let is_close = close(point);
                if is_close || was_close {
                    near.push(Obstacle::new(previous, point, width, k, false));
                }
                if let Some(i) = cell(point) {
                    scene.occupied[i] = true;
                }
                previous = point;
                was_close = is_close;
            }
        }
        scene
    }

    /// The food cell with the most value per second of travel.
    fn food(&self, view: &View, me: &Me, p: &Params) -> Option<Goal> {
        let o = view.o;
        let mut best: Option<Goal> = None;
        let cells = self.n * self.n;
        for (&(value, x, y), &occupied) in self.cells[..cells].iter().zip(&self.occupied[..cells]) {
            if value <= 0.0 {
                continue;
            }
            let q = Point {
                x: x / value,
                y: y / value,
            };
            let value = if occupied {
                p.occupied_factor * value
            } else {
                value
            };
            // Travel takes at least the straight distance: skip what cannot win.
            if let Some(b) = &best {
                let within = (value / b.rate - p.settle_seconds) * me.speed;
                if within <= 0.0
                    || within * within <= (q.x - me.head.x).powi(2) + (q.y - me.head.y).powi(2)
                {
                    continue;
                }
            }
            if q.distance(o.arena.center) > o.arena.radius - me.width - p.rim_food_margin {
                continue;
            }
            let Some(seconds) = me.reach(q, p.orbit_factor) else {
                continue;
            };
            let rate = value / (seconds + p.settle_seconds);
            if best.as_ref().is_none_or(|b| rate > b.rate) {
                best = Some(Goal {
                    heading: toward(me.head, q),
                    rate,
                    crosses: None,
                });
            }
        }
        best
    }
}

/// The most valuable cut-off: a point on a nearby opponent's predicted path
/// that our head reaches `kill_gap` to `kill_window` seconds before it does,
/// so our body lies across its path when it arrives. Valued at
/// `kill_weight` × its mass per second until it arrives.
fn cut(view: &View, me: &Me, p: &Params, intents: &[Intent]) -> Option<Goal> {
    let o = view.o;
    let samples = p.kill_samples.max(1);
    let mut best: Option<Goal> = None;
    for (k, enemy) in o.opponents.iter().enumerate() {
        if !enemy.alive || enemy.head.distance(me.head) > p.kill_range {
            continue;
        }
        let intent = intents.get(k).copied().unwrap_or_default();
        let delta = if p.intent_weight > 0.0 && intent.wanted.is_finite() {
            p.intent_weight * wrap(intent.wanted - enemy.heading)
        } else {
            0.0
        };
        let turn = turn_rate(enemy.segments);
        let mass = enemy.segments as f64 + enemy.fam;
        for step in 1..=samples {
            let tau = p.kill_horizon * step as f64 / samples as f64;
            let q = turn_then_straight(
                enemy.head,
                enemy.heading,
                delta,
                enemy.speed,
                turn,
                view.lead + tau,
            );
            if q.distance(o.arena.center) > o.arena.radius - me.width - p.rim_food_margin {
                continue;
            }
            let Some(ours) = me.reach(q, p.orbit_factor) else {
                continue;
            };
            let slack = tau - ours;
            if slack < p.kill_gap || slack > p.kill_window {
                continue;
            }
            let rate = p.kill_weight * mass / (tau + p.settle_seconds);
            if best.as_ref().is_none_or(|b| rate > b.rate) {
                best = Some(Goal {
                    heading: toward(me.head, q),
                    rate,
                    crosses: Some(k),
                });
            }
        }
    }
    best
}

/// Encircle the nearest much smaller snake. Our body is laid on an orbit
/// around its forecast head, turning the way we already circle it; once the
/// body wraps `coil_close_angle` around it the orbit tightens, and the body's
/// natural contraction (`physics::contract_body`) closes the ring. Valued at
/// `coil_weight` × its mass per second of the estimated time to close.
fn coil(view: &View, me: &Me, p: &Params) -> Option<Goal> {
    let o = view.o;
    let mass = o.me.segments as f64 + o.me.fam;
    let (_, target) = o
        .opponents
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            t.alive
                && (t.segments as f64 + t.fam) <= p.coil_mass_ratio * mass
                && t.head.distance(me.head) <= p.coil_range
        })
        .min_by(|a, b| {
            a.1.head
                .distance(me.head)
                .total_cmp(&b.1.head.distance(me.head))
        })?;
    let centre = view.head(target);
    let radius = p.coil_radius_widths * (me.width + width(target))
        + target.speed.max(base_speed(target.segments)) / turn_rate(target.segments);
    // Our body must be able to go round the orbit (`Snake::length` counts
    // segments, so measure the received polyline).
    let mut length = 0.0;
    let mut previous = o.me.head;
    for &anchor in &o.me.body {
        length += previous.distance(anchor);
        previous = anchor;
    }
    if length < std::f64::consts::TAU * radius {
        return None;
    }
    // Wrapped angle: signed small-angle sum along our body near the target,
    // starting at the head. Anchors 42 units apart on a radius above ~100 keep
    // the sine approximation within a few percent.
    let mut wrapped = 0.0;
    let rel = |q: Point| (q.x - centre.x, q.y - centre.y);
    let (mut ax, mut ay) = rel(me.head);
    let mut ra = (ax * ax + ay * ay).sqrt();
    for &anchor in &o.me.body {
        let (bx, by) = rel(anchor);
        let rb = (bx * bx + by * by).sqrt();
        if rb > 2.5 * radius || ra == 0.0 || rb == 0.0 {
            break;
        }
        wrapped += (ax * by - ay * bx) / (ra * rb);
        (ax, ay, ra) = (bx, by, rb);
    }
    // Head first, the body trails: we circle opposite to the body's sweep.
    let (hx, hy) = rel(me.head);
    let side = if wrapped != 0.0 {
        -wrapped.signum()
    } else {
        let cross = hx * me.heading.sin() - hy * me.heading.cos();
        if cross >= 0.0 { 1.0 } else { -1.0 }
    };
    let closing = wrapped.abs() >= p.coil_close_angle;
    let orbit = if closing {
        radius * p.coil_shrink
    } else {
        radius
    };
    let phi = hy.atan2(hx) + side * p.coil_lead_angle;
    let aim = Point {
        x: centre.x + orbit * phi.cos(),
        y: centre.y + orbit * phi.sin(),
    };
    if aim.distance(o.arena.center) > o.arena.radius - me.width - p.rim_food_margin {
        return None;
    }
    let remaining = (std::f64::consts::TAU - wrapped.abs()).max(0.0) * radius / me.speed;
    Some(Goal {
        heading: toward(me.head, aim),
        rate: p.coil_weight * (target.segments as f64 + target.fam)
            / (remaining + p.settle_seconds),
        crosses: None,
    })
}

/// With nothing to eat in view, cross the middle of the arena rather than
/// circling or following the rim.
fn roam(o: &Observation, me: &Me, p: &Params) -> Goal {
    let heading = if me.head.distance(o.arena.center) > p.roam_fraction * o.arena.radius {
        toward(me.head, o.arena.center)
    } else {
        me.heading
    };
    Goal {
        heading,
        rate: 0.0,
        crosses: None,
    }
}

/// Clearance of the arc toward `heading` from the rim and nearby bodies,
/// capped at `cap`. Only clearance below `safe` changes a cost, so callers pass
/// `safe` and obstacles that provably cannot lower the running minimum are
/// skipped; the result equals `min(cap, exact clearance)`.
fn clearance(
    o: &Observation,
    me: &Me,
    p: &Params,
    near: &[Obstacle],
    heading: f64,
    cap: f64,
    crosses: Option<usize>,
) -> f64 {
    let step = p.lookahead_seconds / 2.0;
    let (middle, turned) = advance(me.head, me.heading, heading, me.speed, me.turn, step);
    let (end, last) = advance(middle, turned, heading, me.speed, me.turn, step);
    // Look further toward the rim so a turn starts in time.
    let ahead = forward(end, turned, me.speed * p.rim_ahead_seconds);
    let rim = |q: Point| o.arena.radius - me.width - q.distance(o.arena.center);
    let mut clearance = rim(end).min(rim(ahead) + p.rim_ahead_credit).min(cap);
    // Obstacle widths hold `own_width_factor` of our width; the rest moves the
    // checked path forward to the head's tip.
    let tip = (1.0 - p.own_width_factor) * me.width;
    let (start, middle, end) = if tip > 0.0 {
        (
            forward(me.head, me.heading, tip),
            forward(middle, turned, tip),
            forward(end, last, tip),
        )
    } else {
        (me.head, middle, end)
    };
    let arcs = [(start, middle), (middle, end)].map(|(a, b)| {
        let mid = Point {
            x: (a.x + b.x) / 2.0,
            y: (a.y + b.y) / 2.0,
        };
        (a, b, mid, a.distance(b) / 2.0)
    });
    for obstacle in near {
        // A cut-off crosses ahead of its victim's head by design; its body
        // still counts.
        if obstacle.head && crosses == Some(obstacle.owner) {
            continue;
        }
        for &(a, b, mid, half) in &arcs {
            // Segment distance >= midpoint distance - both half lengths. The
            // slack, relative to coordinate magnitude, keeps rounding from
            // skipping a segment that would lower it.
            let slack = 1e-6 + 1e-12 * (mid.x.abs() + mid.y.abs() + obstacle.half + half);
            let bound = clearance + obstacle.width + obstacle.half + half + slack;
            let dx = mid.x - obstacle.mid.x;
            let dy = mid.y - obstacle.mid.y;
            if bound > 0.0 && dx * dx + dy * dy > bound * bound {
                continue;
            }
            clearance =
                clearance.min(swept_distance(a, b, obstacle.from, obstacle.to) - obstacle.width);
        }
    }
    clearance
}

fn steer(o: &Observation, me: &Me, p: &Params, near: &[Obstacle], goal: Goal) -> Action {
    let mut best = (f64::INFINITY, goal.heading);
    for &offset in &p.fan[..p.fan_len.min(MAX_FAN)] {
        // Offsets grow, so none left can beat a clear arc already found.
        if best.0 <= offset.abs() {
            break;
        }
        let heading = goal.heading + offset;
        let clearance = clearance(o, me, p, near, heading, p.safe, goal.crosses);
        let cost = offset.abs() + (p.safe - clearance).max(0.0) * p.danger_weight;
        if cost < best.0 {
            best = (cost, heading);
        }
    }
    Action {
        heading: wrap(best.1),
        boost: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_form_turn_matches_fine_physics_steps() {
        let head = Point { x: 10.0, y: -20.0 };
        for (heading, delta) in [(0.3, 1.2), (2.0, -2.5), (-1.0, 0.4), (1.0, 0.0)] {
            let (speed, turn) = (190.0, 3.7);
            let (mut q, mut h) = (head, heading);
            let dt = 1e-5;
            for step in 1..=100_000 {
                (q, h) = advance(q, h, heading + delta, speed, turn, dt);
                if step % 25_000 == 0 {
                    let t = step as f64 * dt;
                    let exact = turn_then_straight(head, heading, delta, speed, turn, t);
                    assert!(
                        exact.distance(q) < 0.05,
                        "{heading} {delta} {t}: {exact:?} {q:?}"
                    );
                }
            }
        }
    }
}
