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
        BODY_RADIUS, BOOST_SPEED, advance, base_speed, pickup_circle, scale, segment_distance,
        swept_distance, turn_rate,
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
/// Longest boost forecast, in substeps (2 seconds); later points count as
/// unreachable by boosting.
const RAMP_SUBSTEPS: usize = 250;
/// Chords of a boosted arc; acceleration while turning bends it unevenly.
const BOOST_CHORDS: usize = 8;
/// Equal-time samples of an arc against a crossed victim's head.
const TIMED_SAMPLES: usize = 16;

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
    /// Largest angle between our approach and the victim's heading at a cut
    /// point, radians; larger angles are head-on and kill us instead.
    pub kill_cross_angle: f64,
    /// Extra cut-off value per escape the victim lacks: of its hard-left,
    /// straight and hard-right branches over `kill_trap_seconds`, the share
    /// blocked by the rim, nearby bodies or the wall our cut lays. 0 ignores
    /// escapes.
    pub kill_trap_weight: f64,
    pub kill_trap_seconds: f64,
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
    /// Boost only at or above this score; boosting spends mass.
    pub boost_min_score: f64,
    /// Boost to win a cut-off that cruise speed cannot reach in time.
    pub kill_boost: bool,
    /// Boost along a clear arc when every cruise-speed arc is dangerous.
    pub escape_boost: bool,
    /// Boost toward a food cell holding at least this much value (after
    /// `occupied_factor`), such as a fresh kill's drop; 0 never does.
    pub food_boost_value: f64,
    /// Extra clearance per unit of our own width, added to `safe`: a longer
    /// snake turns wider and loses more by dying.
    pub safe_width_factor: f64,
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
            kill_cross_angle: 2.0,
            kill_trap_weight: 0.0,
            kill_trap_seconds: 0.4,
            coil_weight: 0.0,
            coil_mass_ratio: 0.5,
            coil_range: 600.0,
            coil_radius_widths: 1.5,
            coil_lead_angle: 0.6,
            coil_close_angle: 4.7,
            coil_shrink: 0.7,
            boost_min_score: 30.0,
            kill_boost: false,
            escape_boost: false,
            food_boost_value: 0.0,
            safe_width_factor: 0.0,
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
            self.kill_cross_angle,
            self.kill_trap_weight,
            self.kill_trap_seconds,
            self.coil_weight,
            self.coil_mass_ratio,
            self.coil_range,
            self.coil_radius_widths,
            self.coil_lead_angle,
            self.coil_close_angle,
            self.coil_shrink,
            self.boost_min_score,
            self.food_boost_value,
            self.safe_width_factor,
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
            || self.kill_trap_weight < 0.0
            || self.food_boost_value < 0.0
            || self.safe_width_factor < 0.0
            || self.kill_trap_seconds < 0.0
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
/// guarantee collision avoidance. Boost stays off unless a boost option is set.
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
    me.safe = p.safe + p.safe_width_factor * me.width;
    if lead_steps > 0 {
        let motion = replay(&o.me, &o.arena, timing, lead_steps, |m| {
            scratch
                .sweep
                .push(pickup_circle(m.head, m.heading, m.speed.speed, o.me.segments).0)
        });
        me.head = motion.head;
        me.heading = motion.heading;
        me.speed = motion.speed.speed.max(base_speed(o.me.segments));
        me.state = motion.speed;
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
    let can_boost = o.me.score >= p.boost_min_score;
    let boosts = p.kill_boost || p.escape_boost || p.food_boost_value > 0.0;
    let ramp = (can_boost && boosts).then(|| me.boost_ramp());
    let scene = Scene::new(&view, &me, p, intents, ramp.is_some(), &mut scratch.near);
    let mut goal = scene.food(&view, &me, p).unwrap_or_else(|| roam(o, &me, p));
    // The best goal reachable at cruise speed, should boosting turn out unsafe.
    let mut cruise = goal;
    let mut consider = |g: Goal| {
        if g.rate > goal.rate {
            goal = g;
        }
        if !g.boost && g.rate > cruise.rate {
            cruise = g;
        }
    };
    if p.kill_weight > 0.0 {
        if let Some(g) = cut(
            &view,
            &me,
            p,
            intents,
            &scratch.near,
            ramp.as_ref().filter(|_| p.kill_boost),
        ) {
            consider(g);
        }
    }
    if p.coil_weight > 0.0 {
        if let Some(g) = coil(&view, &me, p) {
            consider(g);
        }
    }
    if goal.boost {
        if let Some(action) = ramp
            .as_ref()
            .and_then(|r| boost_toward(o, &me, p, &scratch.near, &goal, r))
        {
            return action;
        }
        // Only the best cut-off was kept; find the best one at cruise speed.
        goal = cruise;
        if let Some(g) = cut(&view, &me, p, intents, &scratch.near, None) {
            if g.rate > goal.rate {
                goal = g;
            }
        }
    }
    steer(
        o,
        &me,
        p,
        &scratch.near,
        goal,
        ramp.as_ref().filter(|_| p.escape_boost),
    )
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

#[derive(Clone, Copy)]
struct Me {
    head: Point,
    heading: f64,
    speed: f64,
    turn: f64,
    width: f64,
    segments: usize,
    /// Speed and boost progress at the forecast moment.
    state: SpeedState,
    /// Clearance below which an arc is dangerous (`Params::safe`, widened by
    /// `safe_width_factor`).
    safe: f64,
}

impl Me {
    fn new(me: &Snake) -> Self {
        Self {
            head: me.head,
            heading: me.heading,
            speed: me.speed.max(base_speed(me.segments)),
            turn: turn_rate(me.segments),
            width: width(me),
            segments: me.segments,
            state: SpeedState::from_observed_speed(me.speed, me.segments),
            safe: 0.0,
        }
    }
    /// Cumulative distance after each substep of boosting from our speed
    /// state: the server accelerates gradually (`SpeedState::step`).
    fn boost_ramp(&self) -> [f64; RAMP_SUBSTEPS] {
        let mut ramp = [0.0; RAMP_SUBSTEPS];
        let mut state = self.state;
        let mut distance = 0.0;
        for r in &mut ramp {
            distance += state.step(true, self.segments, SUBSTEP, SpeedModel::default()) * SUBSTEP;
            *r = distance;
        }
        ramp
    }
    /// Path points and headings boosting toward `heading` for `seconds`:
    /// [`BOOST_CHORDS`] intervals, each the exact constant-speed turn at the
    /// ramp's mean speed over it.
    fn boost_path(
        &self,
        ramp: &[f64; RAMP_SUBSTEPS],
        heading: f64,
        seconds: f64,
    ) -> [(Point, f64); BOOST_CHORDS + 1] {
        let last = RAMP_SUBSTEPS as f64 * SUBSTEP;
        let terminal = (ramp[RAMP_SUBSTEPS - 1] - ramp[RAMP_SUBSTEPS - 2]) / SUBSTEP;
        let at = |t: f64| {
            if t >= last {
                // Past the ramp, at its final speed.
                return ramp[RAMP_SUBSTEPS - 1] + (t - last) * terminal;
            }
            let x = t / SUBSTEP;
            let i = x.floor() as usize;
            let before = if i == 0 { 0.0 } else { ramp[i - 1] };
            before + (ramp[i] - before) * (x - i as f64)
        };
        let dt = seconds / BOOST_CHORDS as f64;
        let mut path = [(self.head, self.heading); BOOST_CHORDS + 1];
        for i in 1..=BOOST_CHORDS {
            let (q, h) = path[i - 1];
            let speed = (at(dt * i as f64) - at(dt * (i - 1) as f64)) / dt;
            let delta = wrap(heading - h);
            path[i] = (
                turn_then_straight(q, h, delta, speed, self.turn, dt),
                h + delta.signum() * delta.abs().min(self.turn * dt),
            );
        }
        path
    }
    /// Like [`Me::reach`] when boosting along `ramp`; None beyond it.
    fn reach_boosted(
        &self,
        ramp: &[f64; RAMP_SUBSTEPS],
        p: Point,
        orbit_factor: f64,
    ) -> Option<f64> {
        let distance = p.distance(self.head);
        let angle = wrap(toward(self.head, p) - self.heading).abs();
        // The orbit test uses the top speed's wider turning circle.
        if distance < orbit_factor * BOOST_SPEED / self.turn * angle.sin() {
            return None;
        }
        let steps = ramp.partition_point(|&d| d < distance);
        (steps < RAMP_SUBSTEPS).then(|| (steps + 1) as f64 * SUBSTEP + angle / self.turn)
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

#[derive(Clone, Copy)]
struct Goal {
    heading: f64,
    rate: f64,
    /// Reaching it needs boost speed.
    boost: bool,
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
    /// A head path's head crosses it from `t0` to `t1` seconds after the
    /// forecast moment, within `bow` of the point moving uniformly along it.
    t0: f64,
    t1: f64,
    bow: f64,
}

impl Obstacle {
    fn new(from: Point, to: Point, width: f64, owner: usize, head: bool) -> Self {
        Self {
            from,
            to,
            width,
            owner,
            head,
            t0: 0.0,
            t1: 0.0,
            bow: 0.0,
            mid: Point {
                x: (from.x + to.x) / 2.0,
                y: (from.y + to.y) / 2.0,
            },
            half: from.distance(to) / 2.0,
        }
    }
    fn during(self, t0: f64, t1: f64) -> Self {
        Self { t0, t1, ..self }
    }
    fn bowed(self, bow: f64) -> Self {
        Self { bow, ..self }
    }
}

/// How far a point that travels `length` along a path turning one way through
/// `turned` radians strays from the point moving uniformly along the chord at
/// the same moment. Velocities differ from the chord's mean by at most
/// `2 sin(turned / 2) <= min(turned, 2)` of the speed, and both start and end
/// together, so the gap is at most half of that over the whole span.
fn bow(length: f64, turned: f64) -> f64 {
    length * turned.abs().min(2.0) / 2.0
}

/// Food pooled in cells around the head and cells that bodies or oncoming
/// heads occupy.
struct Scene {
    cells: [(f64, f64, f64); MAX_CELLS * MAX_CELLS],
    occupied: [bool; MAX_CELLS * MAX_CELLS],
    n: usize,
}

impl Scene {
    fn new(
        view: &View,
        me: &Me,
        p: &Params,
        intents: &[Intent],
        boosting: bool,
        near: &mut Vec<Obstacle>,
    ) -> Self {
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
        // Gather for the fastest arc this decision may check.
        let speed = if boosting {
            me.speed.max(BOOST_SPEED)
        } else {
            me.speed
        };
        // A width-scaled safety margin counts obstacles that much further out.
        let reach = speed * p.lookahead_seconds
            + 2.0 * me.width
            + p.reach_extra
            + p.safe_width_factor * me.width;
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
            // Forecast trail points between the forecast and observed heads
            // when the head turns during the lead, newest first.
            let mut trail = [Point::default(); 2];
            let mut trail_len = 0;
            let head = if turning {
                // Turning toward the reported wanted heading at its turn rate.
                let delta = p.intent_weight * wrap(intent.wanted - enemy.heading);
                let turn = turn_rate(enemy.segments);
                let path = |t: f64| {
                    turn_then_straight(enemy.head, enemy.heading, delta, enemy.speed, turn, t)
                };
                let head = if view.lead > 0.0 {
                    trail = [path(view.lead * 2.0 / 3.0), path(view.lead / 3.0)];
                    trail_len = 2;
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
                    let half = p.lookahead_seconds / 2.0;
                    let bow = bow(enemy.speed * half, (turn * half).min(delta.abs()));
                    near.push(
                        Obstacle::new(head, middle, w, k, true)
                            .during(0.0, half)
                            .bowed(bow),
                    );
                    near.push(
                        Obstacle::new(middle, ahead, w, k, true)
                            .during(half, 2.0 * half)
                            .bowed(bow),
                    );
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
                    near.push(
                        Obstacle::new(head, ahead, width + unsure * enemy.speed, k, true)
                            .during(0.0, p.lookahead_seconds),
                    );
                }
                head
            };
            // During the lead the observed head became body behind the forecast head.
            let mut previous = head;
            let mut was_close = close(previous);
            let observed = (view.lead > 0.0).then_some(enemy.head);
            for point in trail[..trail_len]
                .iter()
                .copied()
                .chain(observed)
                .chain(enemy.body.iter().copied())
            {
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
                    // Rich enough to race for; boosting still has to be safe.
                    boost: p.food_boost_value > 0.0 && value >= p.food_boost_value,
                    crosses: None,
                });
            }
        }
        best
    }
}

/// Whether the sliding rim (`boundary::constrain`) kills a head here: only a
/// contact point past the rim pushed outward steeply dies, and a head meeting
/// it closer to the tangent slides along it. Escape branches are not
/// constrained, so past the rim this forecasts where sliding would lead.
fn rim_kills(arena: &Arena, head: Point, heading: f64, segments: usize) -> bool {
    let tip = forward(head, heading, boundary::CONTACT_LEAD * scale(segments));
    let (x, y) = (tip.x - arena.center.x, tip.y - arena.center.y);
    let distance = x.hypot(y);
    distance >= arena.radius
        && x * heading.cos() + y * heading.sin() > boundary::LETHAL_OUTWARD * distance
}

/// Whether a branch `at(t)` (head and heading) dies at the rim within
/// `TRAP_CHORDS` samples `dt` apart. Its first contact, located between
/// samples by bisection, is lethal or starts a slide; while sliding, the head
/// stays on the rim, so later samples die once their heading points outward
/// steeply at the bearing of their contact point.
fn branch_hits_rim(
    arena: &Arena,
    segments: usize,
    dt: f64,
    at: impl Fn(f64) -> (Point, f64),
) -> bool {
    let lead = boundary::CONTACT_LEAD * scale(segments);
    let contact = |t: f64| {
        let (q, h) = at(t);
        forward(q, h, lead).distance(arena.center) >= arena.radius
    };
    let Some(j) = (0..=TRAP_CHORDS).find(|&j| contact(dt * j as f64)) else {
        return false;
    };
    let (mut lo, mut hi) = (dt * j.saturating_sub(1) as f64, dt * j as f64);
    if j > 0 {
        for _ in 0..6 {
            let mid = (lo + hi) / 2.0;
            if contact(mid) {
                hi = mid;
            } else {
                lo = mid;
            }
        }
    }
    let (q, h) = at(hi);
    rim_kills(arena, q, h, segments)
        // Sample j itself follows the bisected contact.
        || (j..=TRAP_CHORDS).any(|i| {
            let (q, h) = at(dt * i as f64);
            let tip = forward(q, h, lead);
            let (x, y) = (tip.x - arena.center.x, tip.y - arena.center.y);
            x * h.cos() + y * h.sin() > boundary::LETHAL_OUTWARD * x.hypot(y)
        })
}

/// A victim's hard-left, straight and hard-right escape branches from its
/// forecast head, as the path of its tip (`physics::strikes` kills through the
/// tip), with whether the rim or a gathered body already blocks each.
/// Bodies beyond our gathering radius are unknown and count as open.
struct Escapes {
    tips: [[Point; TRAP_CHORDS + 1]; 3],
    blocked: [bool; 3],
    seconds: f64,
}

/// Chords of each escape branch; at 50 ms (the default 0.4 s span) a boosting
/// victim moves less than a small snake's width between samples.
const TRAP_CHORDS: usize = 8;

impl Escapes {
    #[allow(clippy::too_many_arguments)]
    fn new(
        view: &View,
        me: &Me,
        p: &Params,
        near: &[Obstacle],
        k: usize,
        enemy: &Snake,
        head: Point,
        heading: f64,
    ) -> Self {
        let o = view.o;
        let radius = width(enemy);
        let turn = turn_rate(enemy.segments);
        let dt = p.kill_trap_seconds / TRAP_CHORDS as f64;
        let mut tips = [[head; TRAP_CHORDS + 1]; 3];
        let mut blocked = [false; 3];
        for (b, delta) in [-std::f64::consts::PI, 0.0, std::f64::consts::PI]
            .into_iter()
            .enumerate()
        {
            let at = |t: f64| {
                let q = turn_then_straight(head, heading, delta, enemy.speed, turn, t);
                (q, heading + delta.signum() * delta.abs().min(turn * t))
            };
            let heads: [(Point, f64); TRAP_CHORDS + 1] = std::array::from_fn(|j| at(dt * j as f64));
            tips[b] = heads.map(|(q, h)| forward(q, h, radius));
            blocked[b] = branch_hits_rim(&o.arena, enemy.segments, dt, at);
        }
        // Other snakes' gathered bodies: a tip crossing their centre line, or
        // within their radius of it, dies.
        for obstacle in near.iter().filter(|ob| !ob.head && ob.owner != k) {
            let reach = width(&o.opponents[obstacle.owner]);
            for (b, tip) in tips.iter().enumerate() {
                blocked[b] = blocked[b]
                    || tip
                        .windows(2)
                        .any(|w| swept_distance(w[0], w[1], obstacle.from, obstacle.to) < reach);
            }
        }
        // Our own body near it.
        let span = enemy.speed * p.kill_trap_seconds + 2.0 * radius + me.width;
        let mut previous = o.me.head;
        for &anchor in &o.me.body {
            if anchor.distance(head) < span || previous.distance(head) < span {
                for (b, tip) in tips.iter().enumerate() {
                    blocked[b] = blocked[b]
                        || tip
                            .windows(2)
                            .any(|w| swept_distance(w[0], w[1], previous, anchor) < me.width);
                }
            }
            previous = anchor;
        }
        Self {
            tips,
            blocked,
            seconds: p.kill_trap_seconds,
        }
    }

    /// Share of branches blocked by a wall our head lays from `from` to `to`
    /// at a steady pace, arriving after `arrival` seconds. A branch chord
    /// meets only the part laid by the time the chord starts, so a wall still
    /// growing during that chord counts as open.
    fn blocked_with_wall(&self, from: Point, to: Point, width: f64, arrival: f64) -> f64 {
        let chord = self.seconds / TRAP_CHORDS as f64;
        let laid = |t: f64| {
            let f = if arrival > 0.0 {
                (t / arrival).min(1.0)
            } else {
                1.0
            };
            Point {
                x: from.x + (to.x - from.x) * f,
                y: from.y + (to.y - from.y) * f,
            }
        };
        let mut count = 0;
        for (tip, &blocked) in self.tips.iter().zip(&self.blocked) {
            if blocked
                || tip
                    .windows(2)
                    .enumerate()
                    .any(|(j, w)| swept_distance(w[0], w[1], from, laid(chord * j as f64)) < width)
            {
                count += 1;
            }
        }
        count as f64 / 3.0
    }
}

/// The most valuable cut-off: a point on a nearby opponent's predicted path
/// that our head reaches `kill_gap` to `kill_window` seconds before it does,
/// so our body lies across its path when it arrives. Valued at
/// `kill_weight` × its mass per second until it arrives, raised by
/// `kill_trap_weight` for each escape it lacks.
fn cut(
    view: &View,
    me: &Me,
    p: &Params,
    intents: &[Intent],
    near: &[Obstacle],
    ramp: Option<&[f64; RAMP_SUBSTEPS]>,
) -> Option<Goal> {
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
        let path = |t: f64| {
            turn_then_straight(
                enemy.head,
                enemy.heading,
                delta,
                enemy.speed,
                turn,
                view.lead + t,
            )
        };
        let heading_at =
            |t: f64| enemy.heading + delta.signum() * delta.abs().min(turn * (view.lead + t));
        let radius = width(enemy);
        // Computed at the victim's first feasible cut point, if any.
        let mut escapes: Option<Escapes> = None;
        for step in 1..=samples {
            let tau = p.kill_horizon * step as f64 / samples as f64;
            let q = path(tau);
            if q.distance(o.arena.center) > o.arena.radius - me.width - p.rim_food_margin {
                continue;
            }
            // Cross its path rather than meet its head: approach within
            // `kill_cross_angle` of its heading there ...
            let approach = toward(me.head, q);
            if wrap(approach - heading_at(tau)).abs() > p.kill_cross_angle {
                continue;
            }
            // ... and arrive with our forward probe (`physics::strikes`: our
            // tip extended by its radius) clear of its tip and head.
            let tip = forward(q, approach, me.width);
            let probe = forward(tip, approach, radius);
            let fits = |ours: Option<f64>| {
                ours.filter(|&t| {
                    let head = path(t);
                    (p.kill_gap..=p.kill_window).contains(&(tau - t))
                        && swept_distance(tip, probe, head, forward(head, heading_at(t), radius))
                            > radius
                })
            };
            let (boost, arrival) = if let Some(t) = fits(me.reach(q, p.orbit_factor)) {
                (false, t)
            } else if let Some(t) = ramp.and_then(|r| fits(me.reach_boosted(r, q, p.orbit_factor)))
            {
                (true, t)
            } else {
                continue;
            };
            // Our actual, possibly turning, arc toward the cut point must stay
            // clear of the victim's head and the body it lays meanwhile.
            let (mut ours, points, _) = arc(me, p, approach, ramp.filter(|_| boost));
            to_tip(&mut ours[..points], me, p);
            let w = p.own_width_factor * me.width + radius + view.uncertainty * enemy.speed;
            let half = p.lookahead_seconds / 2.0;
            let bow = bow(enemy.speed * half, (turn * half).min(delta.abs()));
            let early = Obstacle::new(path(0.0), path(half), w, k, true)
                .during(0.0, half)
                .bowed(bow);
            let late = Obstacle::new(path(half), path(2.0 * half), w, k, true)
                .during(half, 2.0 * half)
                .bowed(bow);
            let ours = &ours[..points];
            let speed = if boost {
                me.speed.max(BOOST_SPEED)
            } else {
                me.speed
            };
            let tip = (1.0 - p.own_width_factor) * me.width;
            let timed = |ob: &Obstacle| timed_clearance(ours, p.lookahead_seconds, ob, speed, tip);
            if timed(&early).min(timed(&late)) < 0.0 {
                continue;
            }
            let mut rate = p.kill_weight * mass / (tau + p.settle_seconds);
            if p.kill_trap_weight > 0.0 {
                let escapes = escapes.get_or_insert_with(|| {
                    Escapes::new(view, me, p, near, k, enemy, path(0.0), heading_at(0.0))
                });
                rate *= 1.0
                    + p.kill_trap_weight * escapes.blocked_with_wall(me.head, q, me.width, arrival);
            }
            if best.as_ref().is_none_or(|b| rate > b.rate) {
                best = Some(Goal {
                    heading: toward(me.head, q),
                    rate,
                    boost,
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
    // We cannot follow an orbit tighter than our own turning circle.
    let ours = me.speed / me.turn;
    let radius = (p.coil_radius_widths * (me.width + width(target))
        + target.speed.max(base_speed(target.segments)) / turn_rate(target.segments))
    .max(ours);
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
        (radius * p.coil_shrink).max(ours)
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
        boost: false,
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
        boost: false,
        crosses: None,
    }
}

/// Head positions and headings along the arc toward `heading` at equal
/// times over `lookahead_seconds`, how many, and the heading the rim check
/// continues along. Cruise arcs are the original two steps; with `ramp` the
/// arc accelerates to boost speed.
fn arc(
    me: &Me,
    p: &Params,
    heading: f64,
    ramp: Option<&[f64; RAMP_SUBSTEPS]>,
) -> ([(Point, f64); BOOST_CHORDS + 1], usize, f64) {
    let mut path = [(me.head, me.heading); BOOST_CHORDS + 1];
    if let Some(ramp) = ramp {
        path = me.boost_path(ramp, heading, p.lookahead_seconds);
        let last = path[BOOST_CHORDS].1;
        (path, BOOST_CHORDS + 1, last)
    } else {
        let step = p.lookahead_seconds / 2.0;
        let (middle, turned) = advance(me.head, me.heading, heading, me.speed, me.turn, step);
        let (end, last) = advance(middle, turned, heading, me.speed, me.turn, step);
        path[1] = (middle, turned);
        path[2] = (end, last);
        // The original continues along the middle heading.
        (path, 3, turned)
    }
}

/// Obstacle widths hold `own_width_factor` of our width; the rest moves the
/// checked path forward to the head's tip.
fn to_tip(path: &mut [(Point, f64)], me: &Me, p: &Params) {
    let tip = (1.0 - p.own_width_factor) * me.width;
    if tip > 0.0 {
        for (q, h) in path.iter_mut() {
            *q = forward(*q, *h, tip);
        }
    }
}

/// Clearance of the arc toward `heading` from the rim and nearby bodies,
/// capped at `cap`. Only clearance below `safe` changes a cost, so callers pass
/// `safe` and obstacles that provably cannot lower the running minimum are
/// skipped; the result equals `min(cap, exact clearance)`. With `ramp` the
/// arc accelerates to boost speed.
#[allow(clippy::too_many_arguments)]
fn clearance(
    o: &Observation,
    me: &Me,
    p: &Params,
    near: &[Obstacle],
    heading: f64,
    cap: f64,
    crosses: Option<usize>,
    ramp: Option<&[f64; RAMP_SUBSTEPS]>,
) -> f64 {
    let (mut path, points, rim_heading) = arc(me, p, heading, ramp);
    let path = &mut path[..points];
    let end = path[points - 1].0;
    // Look further toward the rim so a turn starts in time.
    let ahead = forward(end, rim_heading, me.speed * p.rim_ahead_seconds);
    let rim = |q: Point| o.arena.radius - me.width - q.distance(o.arena.center);
    let mut clearance = rim(end).min(rim(ahead) + p.rim_ahead_credit).min(cap);
    to_tip(path, me, p);
    let speed = if ramp.is_some() {
        me.speed.max(BOOST_SPEED)
    } else {
        me.speed
    };
    let tip = (1.0 - p.own_width_factor) * me.width;
    let mut arcs = [(Point::default(), Point::default(), Point::default(), 0.0); BOOST_CHORDS];
    for (arc, w) in arcs.iter_mut().zip(path.windows(2)) {
        let (a, b) = (w[0].0, w[1].0);
        let mid = Point {
            x: (a.x + b.x) / 2.0,
            y: (a.y + b.y) / 2.0,
        };
        *arc = (a, b, mid, a.distance(b) / 2.0);
    }
    let arcs = &arcs[..points - 1];
    for obstacle in near {
        // A cut-off crosses ahead of its victim's head by design: check the
        // arc against where the head has been by each moment, not its whole
        // path.
        if obstacle.head && crosses == Some(obstacle.owner) {
            clearance = clearance.min(timed_clearance(
                path,
                p.lookahead_seconds,
                obstacle,
                speed,
                tip,
            ));
            continue;
        }
        for &(a, b, mid, half) in arcs {
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

/// Clearance of our arc `path` (tip points and headings at equal times over
/// `seconds`) from the part of a crossed victim's head path occupied at each
/// moment: its head and the body laid behind it. Between samples either may
/// move half a sample's travel closer, which the margin covers. Both paths
/// are checked as chords travelled uniformly; the actual curves stray from
/// them by at most their bows (the obstacle's, and ours from our head speed
/// bound `speed`, tip offset `tip` and each chord's turn).
fn timed_clearance(
    path: &[(Point, f64)],
    seconds: f64,
    ob: &Obstacle,
    speed: f64,
    tip: f64,
) -> f64 {
    let chords = (path.len() - 1) as f64;
    let ours = |t: f64| {
        let x = (t / seconds).clamp(0.0, 1.0) * chords;
        let i = (x.floor() as usize).min(path.len() - 2);
        let (a, b) = (path[i].0, path[i + 1].0);
        let f = x - i as f64;
        Point {
            x: a.x + (b.x - a.x) * f,
            y: a.y + (b.y - a.y) * f,
        }
    };
    let span = (ob.t1 - ob.t0).max(1e-9);
    let fastest = path
        .windows(2)
        .map(|w| w[0].0.distance(w[1].0))
        .fold(0.0, f64::max)
        * chords
        / seconds;
    let ours_bow = path
        .windows(2)
        .map(|w| bow(speed * seconds / chords + 2.0 * tip, wrap(w[1].1 - w[0].1)))
        .fold(0.0, f64::max);
    let dt = seconds / TIMED_SAMPLES as f64;
    let margin = (fastest + 2.0 * ob.half / span) * dt / 2.0 + ob.bow + ours_bow;
    let mut clearance = f64::INFINITY;
    for i in 0..=TIMED_SAMPLES {
        let t = dt * i as f64;
        if t < ob.t0 {
            continue;
        }
        let f = ((t - ob.t0) / span).min(1.0);
        let reached = Point {
            x: ob.from.x + (ob.to.x - ob.from.x) * f,
            y: ob.from.y + (ob.to.y - ob.from.y) * f,
        };
        clearance = clearance.min(segment_distance(ours(t), ob.from, reached) - ob.width - margin);
    }
    clearance
}

/// (cost, heading, clearance, offset) of the cheapest fan arc toward `goal`,
/// at cruise speed or, with `ramp`, accelerating.
fn search(
    o: &Observation,
    me: &Me,
    p: &Params,
    near: &[Obstacle],
    goal: &Goal,
    ramp: Option<&[f64; RAMP_SUBSTEPS]>,
) -> (f64, f64, f64, f64) {
    let mut best = (f64::INFINITY, goal.heading, f64::NEG_INFINITY, 0.0);
    for &offset in &p.fan[..p.fan_len.min(MAX_FAN)] {
        // Offsets grow, so none left can beat a clear arc already found.
        if best.0 <= offset.abs() {
            break;
        }
        let heading = goal.heading + offset;
        let clearance = clearance(o, me, p, near, heading, me.safe, goal.crosses, ramp);
        let cost = offset.abs() + (me.safe - clearance).max(0.0) * p.danger_weight;
        if cost < best.0 {
            best = (cost, heading, clearance, offset);
        }
    }
    best
}

/// Boost toward a goal that needs it, like the original Hunter: only along a
/// clear, nearly straight accelerating arc.
fn boost_toward(
    o: &Observation,
    me: &Me,
    p: &Params,
    near: &[Obstacle],
    goal: &Goal,
    ramp: &[f64; RAMP_SUBSTEPS],
) -> Option<Action> {
    let best = search(o, me, p, near, goal, Some(ramp));
    (best.2 >= me.safe && best.3.abs() < 0.4).then(|| Action {
        heading: wrap(best.1),
        boost: true,
    })
}

/// With `escape`, boost along a clear arc when no cruise arc is clear.
fn steer(
    o: &Observation,
    me: &Me,
    p: &Params,
    near: &[Obstacle],
    goal: Goal,
    escape: Option<&[f64; RAMP_SUBSTEPS]>,
) -> Action {
    let best = search(o, me, p, near, &goal, None);
    if let Some(ramp) = escape.filter(|_| best.2 < me.safe) {
        // The cheapest arc may trade clearance for a smaller turn.
        let fan = &p.fan[..p.fan_len.min(MAX_FAN)];
        let any_clear = fan.iter().any(|&offset| {
            let heading = goal.heading + offset;
            clearance(o, me, p, near, heading, me.safe, goal.crosses, None) >= me.safe
        });
        if !any_clear {
            let fast = search(o, me, p, near, &goal, Some(ramp));
            if fast.2 >= me.safe {
                return Action {
                    heading: wrap(fast.1),
                    boost: true,
                };
            }
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

    fn lcg(seed: &mut u64) -> f64 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (*seed >> 11) as f64 / (1u64 << 53) as f64
    }

    #[test]
    fn timed_clearance_covers_the_curves_between_its_chords() {
        // Review round 6: both chords of a turning, boosted victim's head path
        // cleared a straight arc that the actual curves strike. Whenever the
        // timed check clears, the exact curves must stay a width apart.
        let (seconds, width) = (0.45, 14.5);
        let half = seconds / 2.0;
        let mut seed = 6;
        let mut checked = 0;
        for _ in 0..4_000 {
            let speed = base_speed(2) + lcg(&mut seed) * (BOOST_SPEED - base_speed(2));
            let turn = turn_rate(2 + (lcg(&mut seed) * 300.0) as usize);
            let delta = (2.0 * lcg(&mut seed) - 1.0) * std::f64::consts::PI;
            let victim = |t: f64| turn_then_straight(Point::default(), 0.0, delta, speed, turn, t);
            let bow = bow(speed * half, (turn * half).min(delta.abs()));
            let early = Obstacle::new(victim(0.0), victim(half), width, 0, true)
                .during(0.0, half)
                .bowed(bow);
            let late = Obstacle::new(victim(half), victim(seconds), width, 0, true)
                .during(half, seconds)
                .bowed(bow);
            let start = Point {
                x: (2.0 * lcg(&mut seed) - 1.0) * 120.0 + 60.0,
                y: (2.0 * lcg(&mut seed) - 1.0) * 120.0,
            };
            let heading = lcg(&mut seed) * std::f64::consts::TAU;
            let target = lcg(&mut seed) * std::f64::consts::TAU;
            let ours_speed = base_speed(2) + lcg(&mut seed) * 100.0;
            let ours_turn = turn_rate(2 + (lcg(&mut seed) * 300.0) as usize);
            let ours = |t: f64| advance(start, heading, target, ours_speed, ours_turn, t);
            let path = [ours(0.0), ours(half), ours(seconds)];
            let timed = |ob: &Obstacle| timed_clearance(&path, seconds, ob, ours_speed, 0.0);
            if timed(&early).min(timed(&late)) < 0.0 {
                continue;
            }
            checked += 1;
            for i in 0..=450 {
                let t = i as f64 / 1000.0;
                let q = ours(t).0;
                for j in 0..=i {
                    let distance = q.distance(victim(j as f64 / 1000.0));
                    assert!(distance >= width, "t={t} gap={distance}");
                }
            }
        }
        assert!(checked > 200, "{checked}");
    }

    #[test]
    fn trap_walls_block_only_once_laid() {
        // Review round 6: a wall we lay later must not count against an
        // escape branch that has already passed its place.
        let (radius, speed) = (BODY_RADIUS * scale(2), base_speed(2));
        let tip = |t: f64| Point {
            x: radius + speed * t,
            y: 0.0,
        };
        let mut tips = [[Point { x: 0.0, y: 500.0 }; TRAP_CHORDS + 1]; 3];
        tips[1] = std::array::from_fn(|j| tip(0.05 * j as f64));
        let escapes = Escapes {
            tips,
            blocked: [false; 3],
            seconds: 0.4,
        };
        let (from, to) = (Point { x: 50.0, y: -100.0 }, Point { x: 50.0, y: 100.0 });
        assert_eq!(escapes.blocked_with_wall(from, to, 10.0, 1.0), 0.0);
        assert_eq!(escapes.blocked_with_wall(from, to, 10.0, 0.1), 1.0 / 3.0);

        // Review round 7: the branch crosses at 50 ms, the wall reaches it
        // at 150 ms.
        let mut tips = [[Point { x: 0.0, y: 500.0 }; TRAP_CHORDS + 1]; 3];
        tips[1] = std::array::from_fn(|j| Point {
            x: 40.953125 + 180.9375 * 0.05 * j as f64,
            y: 0.0,
        });
        let escapes = Escapes { tips, ..escapes };
        let (from, to) = (
            Point {
                x: 50.0,
                y: -65.625,
            },
            Point { x: 50.0, y: 65.625 },
        );
        assert_eq!(escapes.blocked_with_wall(from, to, 14.5, 0.3), 0.0);
    }

    #[test]
    fn escape_branches_meet_the_rim_at_first_contact() {
        let arena = Arena {
            center: Point::default(),
            radius: 1000.0,
        };
        let (speed, turn) = (base_speed(2), turn_rate(2));
        let branches = |head: Point, heading: f64| {
            [-std::f64::consts::PI, 0.0, std::f64::consts::PI].map(|delta| {
                branch_hits_rim(&arena, 2, 0.05, |t| {
                    (
                        turn_then_straight(head, heading, delta, speed, turn, t),
                        heading + delta.signum() * delta.abs().min(turn * t),
                    )
                })
            })
        };
        // Review round 7: every branch dies at 84-98 ms (`boundary::constrain`
        // at 7 ms steps), between the old 0.2 s samples.
        assert_eq!(branches(Point { x: 970.0, y: 0.0 }, 0.1), [true; 3]);
        // A tangent slides and turning inward escapes; turning outward while
        // sliding dies.
        let tangent = branches(Point { x: 990.0, y: 0.0 }, std::f64::consts::FRAC_PI_2);
        assert_eq!(tangent, [true, false, false]);
        // Review round 8: a long snake's slide dies by its contact point's
        // bearing (simulator death at 392 ms).
        let hard_turn = |arena: &Arena, segments: usize, head: Point, heading: f64| {
            let (speed, turn) = (base_speed(segments), turn_rate(segments));
            branch_hits_rim(arena, segments, 0.05, |t| {
                (
                    turn_then_straight(head, heading, std::f64::consts::PI, speed, turn, t),
                    heading + std::f64::consts::PI.min(turn * t),
                )
            })
        };
        assert!(hard_turn(&arena, 326, Point { x: 980.0, y: 0.0 }, -1.58));
        // Review round 9: contact in the last interval still checks its end
        // (simulator death at 399 ms).
        let official = Arena {
            center: Point::default(),
            radius: 21600.0,
        };
        assert!(hard_turn(
            &official,
            314,
            Point { x: 21534.0, y: 0.0 },
            -1.488
        ));
    }

    #[test]
    fn rim_kills_like_the_sliding_rim_and_lets_tangents_slide() {
        let arena = Arena {
            center: Point { x: 30.0, y: -20.0 },
            radius: 1000.0,
        };
        // Review round 6: a small snake touching the rim on a tangent slides.
        let at = Point {
            x: 1025.0,
            y: -20.0,
        };
        assert!(!rim_kills(&arena, at, std::f64::consts::FRAC_PI_2, 2));
        assert!(rim_kills(&arena, at, 0.3, 2));
        let mut seed = 9;
        for _ in 0..20_000 {
            let segments = 2 + (lcg(&mut seed) * 400.0) as usize;
            let angle = lcg(&mut seed) * std::f64::consts::TAU;
            let r = arena.radius - lcg(&mut seed) * 60.0;
            let head = Point {
                x: r * angle.cos(),
                y: r * angle.sin(),
            };
            let heading = lcg(&mut seed) * std::f64::consts::TAU;
            let (_, dies) = boundary::constrain(
                head,
                heading,
                scale(segments),
                arena.radius,
                BoundaryMode::Sliding,
            );
            let moved = Point {
                x: head.x + arena.center.x,
                y: head.y + arena.center.y,
            };
            assert_eq!(rim_kills(&arena, moved, heading, segments), dies);
        }
    }

    #[test]
    fn boosted_paths_continue_past_the_ramp_at_its_final_speed() {
        let segments = 2;
        let speed = base_speed(segments);
        let me = Me {
            head: Point::default(),
            heading: 0.0,
            speed,
            turn: turn_rate(segments),
            width: BODY_RADIUS * scale(segments),
            segments,
            state: SpeedState::from_observed_speed(speed, segments),
            safe: 30.0,
        };
        let ramp = me.boost_ramp();
        let two = RAMP_SUBSTEPS as f64 * SUBSTEP;
        let terminal = (ramp[RAMP_SUBSTEPS - 1] - ramp[RAMP_SUBSTEPS - 2]) / SUBSTEP;
        let path = me.boost_path(&ramp, 0.0, two + 1.0);
        let end = path[BOOST_CHORDS].0.x;
        let expected = ramp[RAMP_SUBSTEPS - 1] + terminal;
        assert!((end - expected).abs() < 1e-6, "{end} {expected}");
        assert!(end > ramp[RAMP_SUBSTEPS - 1] + 400.0);
    }

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
