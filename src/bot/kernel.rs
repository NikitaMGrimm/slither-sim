//! Latency-aware food forager using only one bot's received observation.
//!
//! Ported from the owner's `slither-arena-rs` Forager
//! (`src/bots/scripts.rs`, `forager`) and its Smith predictor
//! (`src/bots/timing.rs`) at commit `14e9fc47bfbadabe3b782c5bf0d5ee5ec2b0b26e`.
//! The private default parameters preserve that historical regression. Named
//! releases use fully resolved immutable parameters; see the parent module.
//! Parameters are policy choices rather than server rules.
//!
//! The kernel keeps no state between decisions. Its scratch storage holds reusable
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
pub(super) struct Params {
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
    /// A coil's value scales by its wrapped share of a full turn raised to
    /// this power: small snakes often slip out of a barely started orbit.
    /// 0 values every orbit as if it will close.
    pub coil_progress_power: f64,
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
    /// Cost per share of our escape branches blocked at a fan arc's end: from
    /// where the arc leaves us, hard left, straight and hard right over
    /// `exit_seconds` at cruise speed, each blocked by the rim or a gathered
    /// body or head path. 0 skips the check (the original search). Hindsight
    /// replays motivated this option; see the committed policy port evidence
    /// note for the scope and limits of historical evaluation.
    pub exit_weight: f64,
    pub exit_seconds: f64,
    /// Value of a visible prey per unit of its food-equivalent nutrition
    /// (`PREY_NUTRITION` times ordinary food at its size), compared with food
    /// value; 0 ignores prey. Prey flee from the nearest head at a speed that
    /// grows with that snake's speed, so a small cruising snake cannot catch
    /// one: a prey goal boosts when that wins after the boost's mass cost.
    pub prey_weight: f64,
    /// ... within this distance of our head ...
    pub prey_range: f64,
    /// ... and catchable within this many seconds.
    pub prey_horizon: f64,
    /// Race rivals for food cells holding at least `race_min_value`: such a
    /// cell's value counts the share we expect to beat the nearest visible
    /// rival head to, a ramp `race_sigma` seconds wide around its arrival
    /// time, and boosting wins the cell when the extra share and time saved
    /// outweigh the boost's mass cost. 0 disables racing.
    pub race_sigma: f64,
    pub race_min_value: f64,
    /// Time rivals for at most this many raced cells per decision, the most
    /// promising first; dense contested scenes otherwise time most cells
    /// (live official play exceeds 24 in about 1 decision in 600).
    pub race_cells: usize,
    /// Boosting costs nothing at or below this score (Battledome's 400-point
    /// floor, a public mode rule); 0 for official worlds.
    pub boost_free_score: f64,
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
            coil_progress_power: 0.0,
            boost_min_score: 30.0,
            kill_boost: false,
            escape_boost: false,
            food_boost_value: 0.0,
            safe_width_factor: 0.0,
            exit_weight: 0.0,
            exit_seconds: 0.4,
            prey_weight: 0.0,
            prey_range: 650.0,
            prey_horizon: 2.0,
            race_sigma: 0.0,
            race_min_value: 1.0,
            race_cells: 24,
            boost_free_score: 0.0,
        }
    }
}

impl Params {
    #[cfg(test)]
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
            self.coil_progress_power,
            self.boost_min_score,
            self.food_boost_value,
            self.safe_width_factor,
            self.exit_weight,
            self.exit_seconds,
            self.prey_weight,
            self.prey_range,
            self.prey_horizon,
            self.race_sigma,
            self.race_min_value,
            self.boost_free_score,
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
            || self.race_cells == 0
            || self.reach_extra < 0.0
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
            || self.coil_progress_power < 0.0
            || self.exit_weight < 0.0
            || self.prey_weight < 0.0
            || self.prey_horizon < 0.0
            || self.race_sigma < 0.0
            || !(0.0..=2.0).contains(&self.exit_seconds)
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
pub(super) struct Scratch {
    near: Vec<Obstacle>,
    /// Obstacles an exit branch from some fan arc's end can reach.
    exits: Vec<Obstacle>,
    /// Obstacles that can lower a fan arc's capped clearance.
    arcs: Vec<Obstacle>,
    /// Visible rivals' heads, pickup reach and speed, for racing.
    rivals: Vec<(Point, f64, f64)>,
    /// The same three lists at boost reach, gathered only when a boosted
    /// arc is checked; the cruise ones stay for the fallback.
    boost_near: Vec<Obstacle>,
    boost_arcs: Vec<Obstacle>,
    boost_exits: Vec<Obstacle>,
    sweep: Vec<Point>,
}

impl Default for Scratch {
    fn default() -> Self {
        Self {
            near: Vec::with_capacity(4096),
            exits: Vec::with_capacity(4096),
            arcs: Vec::with_capacity(4096),
            rivals: Vec::with_capacity(256),
            boost_near: Vec::with_capacity(4096),
            boost_arcs: Vec::with_capacity(4096),
            boost_exits: Vec::with_capacity(4096),
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
pub(super) fn decide(
    o: &Observation,
    timing: &Timing,
    intents: &[Intent],
    p: &Params,
    scratch: &mut Scratch,
) -> Action {
    decide_with_pit(o, timing, intents, p, scratch, None)
}

pub(super) fn decide_with_pit(
    o: &Observation,
    timing: &Timing,
    intents: &[Intent],
    p: &Params,
    scratch: &mut Scratch,
    pit: Option<f64>,
) -> Action {
    // The same food/obstacle/steering search handles a one-way pit. Outside,
    // enter instead of foraging the spawn band. Inside, its edge is the rim
    // for every food goal, steering arc and escape branch.
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
    // Replay may carry a received outside head into the pit before this
    // decision acts. A head already inside must also retain the hard edge.
    let entering = pit.is_some_and(|radius| {
        o.me.head.distance(o.arena.center) >= radius && me.head.distance(o.arena.center) >= radius
    });
    me.pit = pit.filter(|_| !entering);
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
    // Boost-reach obstacles are gathered only once a boosted arc is checked
    // only for checked boost branches; cut-off nomination still needs the
    // ramp up front when it may boost.
    let nominate = (can_boost && p.kill_boost).then(|| me.boost_ramp());
    let scene = gather(
        &view,
        &me,
        p,
        intents,
        false,
        scratch_lists(&mut scratch.near, &mut scratch.arcs, &mut scratch.exits),
    );
    scratch.rivals.clear();
    if p.race_sigma > 0.0 {
        scratch
            .rivals
            .extend(o.opponents.iter().filter(|s| s.alive).map(|s| {
                // Where its head is at the moment we decide for, as ours.
                let head = view.head(s);
                let (c, r) = pickup_circle(head, s.heading, s.speed, s.segments);
                (head, c.distance(head) + r, s.speed.max(1.0))
            }));
    }
    let race = (p.race_sigma > 0.0).then(|| Race {
        charge: can_boost.then(|| boost_charge(o, p)),
        sigma: p.race_sigma + view.uncertainty,
        rivals: &scratch.rivals,
    });
    let mut goal = scene
        .food(&view, &me, p, race)
        .unwrap_or_else(|| roam(o, &me, p));
    if entering {
        goal = Goal {
            heading: (o.arena.center.y - me.head.y).atan2(o.arena.center.x - me.head.x),
            rate: f64::INFINITY,
            boost: false,
            crosses: None,
        };
    }
    // A raced food goal may be valued at boost speed: should boosting turn
    // out unsafe, food is valued again at cruise speed.
    let revalue = goal.boost && race.is_some_and(|r| r.charge.is_some());
    // The best goal reachable at cruise speed, should boosting turn out unsafe.
    let mut cruise = (!revalue).then_some(goal);
    let mut consider = |g: Goal| {
        if g.rate > goal.rate {
            goal = g;
        }
        if !g.boost && cruise.is_none_or(|c| g.rate > c.rate) {
            cruise = Some(g);
        }
    };
    if p.prey_weight > 0.0 {
        if let Some(g) = prey(o, &me, p, can_boost) {
            consider(g);
        }
    }
    if p.kill_weight > 0.0 {
        if let Some(g) = cut(&view, &me, p, intents, &scratch.near, nominate.as_ref()) {
            consider(g);
        }
    }
    if p.coil_weight > 0.0 {
        if let Some(g) = coil(&view, &me, p) {
            consider(g);
        }
    }
    let mut boosted = false;
    if goal.boost {
        let ramp = can_boost.then(|| nominate.unwrap_or_else(|| me.boost_ramp()));
        // The cruise obstacles are a subset of the boost-reach ones, and
        // clearance is a minimum over obstacles: when no boosted arc that
        // `boost_toward` may take clears them, none clears the full set, so
        // decline without gathering at boost reach.
        let ramp = ramp.filter(|ramp| {
            p.fan[..p.fan_len.min(MAX_FAN)]
                .iter()
                .filter(|offset| offset.abs() < BOOST_MAX_OFFSET)
                .any(|&offset| {
                    let heading = goal.heading + offset;
                    clearance(
                        o,
                        &me,
                        p,
                        &scratch.arcs,
                        heading,
                        me.safe,
                        goal.crosses,
                        Some(ramp),
                        |clearance| clearance < me.safe,
                    ) >= me.safe
                })
        });
        if let Some(ramp) = ramp {
            gather(
                &view,
                &me,
                p,
                intents,
                true,
                scratch_lists(
                    &mut scratch.boost_near,
                    &mut scratch.boost_arcs,
                    &mut scratch.boost_exits,
                ),
            );
            boosted = true;
            if let Some(action) = boost_toward(
                o,
                &me,
                p,
                &scratch.boost_arcs,
                &scratch.boost_exits,
                &goal,
                &ramp,
            ) {
                return action;
            }
        }
        if revalue {
            let food = scene
                .food(&view, &me, p, race.map(|r| Race { charge: None, ..r }))
                .unwrap_or_else(|| roam(o, &me, p));
            // Food came first, so it keeps ties.
            if cruise.is_none_or(|c| food.rate >= c.rate) {
                cruise = Some(food);
            }
        }
        goal = cruise.expect("food, roaming or another cruise goal");
        // Only the best cut-off was kept; find the best one at cruise speed.
        // Without cut-offs (zero weight, zero value) none can win.
        if let Some(g) = (p.kill_weight > 0.0)
            .then(|| cut(&view, &me, p, intents, &scratch.near, None))
            .flatten()
        {
            if g.rate > goal.rate {
                goal = g;
            }
        }
    }
    let best = search(
        o,
        &me,
        p,
        &scratch.arcs,
        &scratch.exits,
        &goal,
        None,
        f64::INFINITY,
    );
    // With `escape_boost`, boost along a clear arc when no cruise arc is clear.
    if p.escape_boost && can_boost && best.2 < me.safe {
        // The cheapest arc may trade clearance for a smaller turn.
        let fan = &p.fan[..p.fan_len.min(MAX_FAN)];
        let any_clear = fan.iter().any(|&offset| {
            let heading = goal.heading + offset;
            clearance(
                o,
                &me,
                p,
                &scratch.arcs,
                heading,
                me.safe,
                goal.crosses,
                None,
                |clearance| clearance < me.safe,
            ) >= me.safe
        });
        if !any_clear {
            let ramp = nominate.unwrap_or_else(|| me.boost_ramp());
            if !boosted {
                gather(
                    &view,
                    &me,
                    p,
                    intents,
                    true,
                    scratch_lists(
                        &mut scratch.boost_near,
                        &mut scratch.boost_arcs,
                        &mut scratch.boost_exits,
                    ),
                );
            }
            let fast = search(
                o,
                &me,
                p,
                &scratch.boost_arcs,
                &scratch.boost_exits,
                &goal,
                Some(&ramp),
                f64::INFINITY,
            );
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

/// The three obstacle lists [`gather`] fills.
struct Lists<'a> {
    near: &'a mut Vec<Obstacle>,
    arcs: &'a mut Vec<Obstacle>,
    exits: &'a mut Vec<Obstacle>,
}
fn scratch_lists<'a>(
    near: &'a mut Vec<Obstacle>,
    arcs: &'a mut Vec<Obstacle>,
    exits: &'a mut Vec<Obstacle>,
) -> Lists<'a> {
    Lists { near, arcs, exits }
}

/// The scene at cruise or boost reach, with the obstacles fan searches scan.
///
/// Arc points (and their tips) lie within the fastest arc's travel plus the
/// tip offset of the head, and exit branch tips within cruise travel plus
/// the tip offset of an arc end. An obstacle further than that plus its half
/// length, width and the clearance cap cannot lower a capped clearance, so
/// fan searches scan only the obstacles that can.
fn gather(
    view: &View,
    me: &Me,
    p: &Params,
    intents: &[Intent],
    boosting: bool,
    lists: Lists,
) -> Scene {
    let scene = Scene::new(view, me, p, intents, boosting, lists.near);
    let fastest = if boosting {
        me.speed.max(BOOST_SPEED)
    } else {
        me.speed
    };
    let tip = (1.0 - p.own_width_factor) * me.width;
    let head = me.head;
    let within = |ob: &Obstacle, r: f64| {
        let b = r + ob.half + ob.width;
        let (dx, dy) = (ob.mid.x - head.x, ob.mid.y - head.y);
        dx * dx + dy * dy <= b * b * (1.0 + 1e-9) + 1e-6
    };
    let arc_reach = fastest * p.lookahead_seconds + tip + me.safe.max(0.0);
    lists.arcs.clear();
    lists.arcs.extend(
        lists
            .near
            .iter()
            .copied()
            .filter(|ob| ob.head || within(ob, arc_reach)),
    );
    lists.exits.clear();
    if p.exit_weight > 0.0 {
        let exit_reach = fastest * p.lookahead_seconds + me.speed * p.exit_seconds + tip;
        lists.exits.extend(
            lists
                .near
                .iter()
                .copied()
                .filter(|ob| within(ob, exit_reach)),
        );
    }
    scene
}

/// Boost economics for racing food cells.
#[derive(Clone, Copy)]
struct Race<'a> {
    /// One boost charge in food equivalents, if we may boost.
    charge: Option<f64>,
    /// Width of the share ramp around a rival's arrival, seconds.
    sigma: f64,
    /// Visible rivals' heads, pickup reach and speed.
    rivals: &'a [(Point, f64, f64)],
}

/// One boost charge (src/physics.rs boost cost, mean jitter 1.01) in food
/// equivalents at our mass; free at or below `boost_free_score`.
fn boost_charge(o: &Observation, p: &Params) -> f64 {
    if o.me.score <= p.boost_free_score {
        return 0.0;
    }
    let n = o.me.segments as f64;
    let mass = n + o.me.fam;
    let nutrition = (1.0 - (mass - 2.0) / 430.0).max(1.0 / 430.0).powf(2.25);
    1.01 * 0.0812 * (1.0 - n / 430.0).max(0.0).powf(1.66) / nutrition
}

/// Earliest time a visible rival's pickup circle reaches `q` at its current
/// speed heading straight there (no turn): a nominal deadline. Arrivals
/// after `by` are not told apart and give `by`.
fn rival_arrival(rivals: &[(Point, f64, f64)], q: Point, by: f64) -> f64 {
    let mut best = by;
    for &(head, mouth, speed) in rivals {
        // Only a rival within `best` of travel can arrive sooner; the slack
        // keeps rounding from dropping a tie.
        let reach = (best * speed + mouth) * (1.0 + 1e-9);
        let d2 = (head.x - q.x).powi(2) + (head.y - q.y).powi(2);
        if d2 < reach * reach {
            best = best.min((head.distance(q) - mouth).max(0.0) / speed);
        }
    }
    best
}

/// Food equivalents of a prey at its size: `PreyConfig::default`'s
/// nutrition factor (src/prey.rs), the mode setting every official world uses.
const PREY_NUTRITION: f64 = 30.0;

/// Flight target speed of a prey fleeing a snake moving at `speed`
/// (`PreyConfig::default` escape and sprint speeds, src/prey.rs).
fn prey_flight_speed(speed: f64) -> f64 {
    let x = ((speed * 0.032 - 5.79) / (14.0 - 5.79)).clamp(0.0, 1.0);
    192.28125 + (343.65625 - 192.28125) * x
}

/// The best prey to chase, if any can be caught within `prey_horizon`.
/// This model is preserved from the frozen kernel. A chased prey flees
/// straight away from our head at the flight speed our speed sets, so we
/// close at our speed minus that until it is within our pickup circle's
/// reach. Boosting closes at boost speed minus the sprint speed, after a
/// ramp, and spends mass: a fresh start drops one charge at once and one per
/// 0.155 s, valued in food equivalents at our mass. Value per second counts
/// the turn toward it and `settle_seconds`, as food does. Coordinate
/// quantization, flight jitter and rival heads are ignored: a heuristic
/// nomination that steering still checks for safety.
fn prey(o: &Observation, me: &Me, p: &Params, can_boost: bool) -> Option<Goal> {
    let segments = o.me.segments;
    let mouth = |speed: f64| {
        let (c, r) = pickup_circle(me.head, me.heading, speed, segments);
        c.distance(me.head) + r
    };
    let charge = boost_charge(o, p);
    let f = 1.0 + 0.305 * (scale(segments) - 1.0);
    let accel = (BOOST_SPEED - base_speed(segments)).max(1.0) * 1.875 / f;
    let ramp = (BOOST_SPEED - me.speed).max(0.0) / accel;
    let mut best: Option<Goal> = None;
    for prey in &o.prey {
        let d = prey.position.distance(me.head);
        if d > p.prey_range {
            continue;
        }
        let value = p.prey_weight * PREY_NUTRITION * 0.0031535 * prey.size * prey.size;
        let turn = wrap(toward(me.head, prey.position) - me.heading).abs() / me.turn;
        let chase = |speed: f64| {
            let close = speed - prey_flight_speed(speed);
            (close > 1.0).then(|| (d - mouth(speed)).max(0.0) / close)
        };
        let mut pick = |t: f64, cost: f64, boost: bool| {
            if t > p.prey_horizon {
                return;
            }
            let rate = (value - cost) / (t + turn + p.settle_seconds);
            if rate > 0.0 && best.is_none_or(|g| rate > g.rate) {
                // Lead it along its current motion for part of the chase.
                let lead = t.min(0.5);
                let aim = forward(prey.position, prey.heading, prey.speed * lead);
                best = Some(Goal {
                    heading: toward(me.head, aim),
                    rate,
                    boost,
                    crosses: None,
                });
            }
        };
        if let Some(t) = chase(me.speed) {
            pick(t, 0.0, false);
        }
        if can_boost {
            if let Some(t) = chase(BOOST_SPEED) {
                let t = t + ramp / 2.0;
                pick(t, charge * (1.0 + t / 0.155), true);
            }
        }
    }
    best
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
    /// Effective hard boundary while inside a configured one-way pit.
    pit: Option<f64>,
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
            pit: None,
        }
    }
    fn radius(&self, o: &Observation) -> f64 {
        self.pit
            .map_or(o.arena.radius, |pit| pit.min(o.arena.radius))
    }
    fn food_margin(&self, p: &Params) -> f64 {
        if self.pit.is_some() {
            p.rim_food_margin.max(0.)
        } else {
            p.rim_food_margin
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn boundary_blocks(
        &self,
        o: &Observation,
        segments: usize,
        speed: f64,
        dt: f64,
        chords: usize,
        at: impl Fn(f64) -> (Point, f64),
    ) -> bool {
        branch_hits_rim(&o.arena, segments, dt, chords, &at)
            || self
                .pit
                .is_some_and(|pit| branch_hits_pit(o.arena.center, pit, speed, dt, chords, at))
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
    /// Like [`Me::reach`] when boosting from now: speed ramps continuously
    /// from ours to boost speed (`SpeedState`'s retained-progress rate at our
    /// size), turning at the same angular rate.
    fn reach_boosting(&self, p: Point, orbit_factor: f64) -> Option<f64> {
        let distance = p.distance(self.head);
        let angle = wrap(toward(self.head, p) - self.heading).abs();
        if distance < orbit_factor * BOOST_SPEED / self.turn * angle.sin() {
            return None;
        }
        let v0 = self.speed.min(BOOST_SPEED);
        let f = 1.0 + 0.305 * (scale(self.segments) - 1.0);
        let a = (BOOST_SPEED - base_speed(self.segments)).max(1.0) * 1.875 / f;
        let ramp = (BOOST_SPEED - v0) / a;
        let ramped = v0 * ramp + 0.5 * a * ramp * ramp;
        let t = if distance <= ramped {
            2.0 * distance / (v0 + (v0 * v0 + 2.0 * a * distance).sqrt())
        } else {
            ramp + (distance - ramped) / BOOST_SPEED
        };
        Some(t + angle / self.turn)
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
        // Occupancy is read only by food(): multiplying a positive cell value
        // by 1 leaves its bits unchanged. A boost regather discards its scene.
        let mark_occupied = !boosting && p.occupied_factor != 1.0;
        // A boost-reach regather keeps the cruise scene's food.
        for food in o.food.iter().filter(|_| !boosting) {
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
        let mut reach = speed * p.lookahead_seconds
            + 2.0 * me.width
            + p.reach_extra
            + p.safe_width_factor * me.width;
        if p.exit_weight > 0.0 {
            // Exit branches run on from an arc's end; `reach_extra` usually
            // covers them already.
            reach =
                reach.max(speed * p.lookahead_seconds + 2.0 * me.width + me.speed * p.exit_seconds);
        }
        let from_head = |q: Point| (q.x - me.head.x).powi(2) + (q.y - me.head.y).powi(2);
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
            let mut reach = reach + extra;
            if p.exit_weight > 0.0 {
                // Exit branch tips run up to cruise travel past an arc's end,
                // itself our tip offset ahead, and meet this obstacle's
                // whole width, however wide the snake.
                let tip = (1.0 - p.own_width_factor) * me.width;
                reach = reach.max(
                    speed * p.lookahead_seconds
                        + me.speed * p.exit_seconds
                        + tip
                        + width
                        + unsure * enemy.speed,
                );
            }
            let reach2 = reach * reach;
            let close = |q: Point| from_head(q) < reach2;
            // Exit branches are tested against whole segments, so a segment
            // whose ends both miss the gathering disc may still cross it.
            let crosses = |a: Point, b: Point, margin: f64| {
                p.exit_weight > 0.0
                    && crosses_disc(
                        a,
                        b,
                        from_head(a),
                        from_head(b),
                        me.head,
                        (reach + margin).powi(2),
                    )
            };
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
                if mark_occupied {
                    for step in 0..=steps {
                        let t = p.head_path_seconds * step as f64 / steps as f64;
                        if let Some(i) = cell(path(view.lead + t)) {
                            scene.occupied[i] = true;
                        }
                    }
                }
                let middle = path(view.lead + p.lookahead_seconds / 2.0);
                let ahead = path(view.lead + p.lookahead_seconds);
                let half = p.lookahead_seconds / 2.0;
                let bow = bow(enemy.speed * half, (turn * half).min(delta.abs()));
                if close(head)
                    || close(middle)
                    || close(ahead)
                    || crosses(head, middle, bow)
                    || crosses(middle, ahead, bow)
                {
                    let w = width + unsure * enemy.speed;
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
                if mark_occupied {
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
                }
                let ahead = forward(head, enemy.heading, enemy.speed * p.lookahead_seconds);
                if close(head) || close(ahead) || crosses(head, ahead, 0.0) {
                    near.push(
                        Obstacle::new(head, ahead, width + unsure * enemy.speed, k, true)
                            .during(0.0, p.lookahead_seconds),
                    );
                }
                head
            };
            // During the lead the observed head became body behind the forecast head.
            let mut previous = head;
            let mut previous2 = from_head(previous);
            let observed = (view.lead > 0.0).then_some(enemy.head);
            for point in trail[..trail_len]
                .iter()
                .copied()
                .chain(observed)
                .chain(enemy.body.iter().copied())
            {
                let point2 = from_head(point);
                if point2 < reach2
                    || previous2 < reach2
                    || (p.exit_weight > 0.0
                        && crosses_disc(previous, point, previous2, point2, me.head, reach2))
                {
                    near.push(Obstacle::new(previous, point, width, k, false));
                }
                if mark_occupied {
                    if let Some(i) = cell(point) {
                        scene.occupied[i] = true;
                    }
                }
                previous = point;
                previous2 = point2;
            }
        }
        scene
    }

    /// The food cell with the most value per second of travel, the first in
    /// grid order on ties.
    fn food(&self, view: &View, me: &Me, p: &Params, race: Option<Race<'_>>) -> Option<Goal> {
        let o = view.o;
        let cells = self.n * self.n;
        let food = |i: usize| {
            let (value, x, y) = self.cells[i];
            (value > 0.0).then(|| {
                let q = Point {
                    x: x / value,
                    y: y / value,
                };
                let value = if self.occupied[i] {
                    p.occupied_factor * value
                } else {
                    value
                };
                (q, value)
            })
        };
        let beats = |rate: f64, i: usize, best: &Option<(Goal, usize)>| {
            best.as_ref()
                .is_none_or(|(b, j)| rate > b.rate || (rate == b.rate && i < *j))
        };
        // Value per second of cell `i`, and whether boosting earns it; None
        // when unreachable or, racing, when it cannot beat `best`.
        let rate = |i: usize, q: Point, value: f64, best: &Option<(Goal, usize)>| {
            let race = race.filter(|_| value >= p.race_min_value);
            let mut rival = f64::INFINITY;
            if let Some(race) = race {
                // Time rivals first. Ours takes at least the straight
                // distance at our fastest speed and at most that at our
                // slowest plus a half turn; a rival a full ramp later leaves
                // us the whole cell, and our share only falls with time, so
                // a contested cell is skipped before timing our own path.
                let d = ((q.x - me.head.x).powi(2) + (q.y - me.head.y).powi(2)).sqrt();
                let slack = 1.0 + 1e-9;
                let latest =
                    (d / me.speed.min(BOOST_SPEED) + std::f64::consts::PI / me.turn) * slack;
                rival = rival_arrival(race.rivals, q, latest + 2.0 * race.sigma);
                let fastest = if race.charge.is_some() {
                    me.speed.max(BOOST_SPEED)
                } else {
                    me.speed
                };
                let t = d / fastest;
                let share = (0.5 + (rival - t) / (2.0 * race.sigma)).clamp(0.0, 1.0);
                if !beats(value * share / (t + p.settle_seconds) * slack, i, best) {
                    return None;
                }
            }
            if q.distance(o.arena.center) > me.radius(o) - me.width - me.food_margin(p) {
                return None;
            }
            let seconds = me.reach(q, p.orbit_factor)?;
            let mut rate = value / (seconds + p.settle_seconds);
            // Rich enough to race for; boosting still has to be safe.
            let mut boost = p.food_boost_value > 0.0 && value >= p.food_boost_value;
            if let Some(race) = race {
                let fast = race
                    .charge
                    .and_then(|charge| Some((charge, me.reach_boosting(q, p.orbit_factor)?)));
                let share = |t: f64| (0.5 + (rival - t) / (2.0 * race.sigma)).clamp(0.0, 1.0);
                rate = value * share(seconds) / (seconds + p.settle_seconds);
                if let Some((charge, t)) = fast {
                    let paid = value * share(t) - charge * (1.0 + t / 0.155);
                    let fast = paid / (t + p.settle_seconds);
                    if fast > rate {
                        rate = fast;
                        boost = true;
                    }
                }
            }
            Some((rate, boost))
        };
        let mut best: Option<(Goal, usize)> = None;
        let take = |i: usize, q: Point, value: f64, best: &mut Option<(Goal, usize)>| {
            let Some((cell_rate, boost)) = rate(i, q, value, best) else {
                return;
            };
            if beats(cell_rate, i, best) {
                *best = Some((
                    Goal {
                        heading: toward(me.head, q),
                        rate: cell_rate,
                        boost,
                        crosses: None,
                    },
                    i,
                ));
            }
        };
        // Raced cells wait, with a bound on their rate, for a pass from the
        // most promising down: contested cells keep the best rate low, so in
        // grid order few could be skipped.
        let mut raced = [(0.0, 0); MAX_CELLS * MAX_CELLS];
        let mut queued = 0;
        let boostable = race.is_some_and(|r| r.charge.is_some());
        for i in 0..cells {
            let Some((q, value)) = food(i) else {
                continue;
            };
            let d2 = (q.x - me.head.x).powi(2) + (q.y - me.head.y).powi(2);
            if race.is_some() && value >= p.race_min_value {
                // Travel takes at least the straight distance; a raced cell
                // may be reached at boost speed.
                let fastest = if boostable {
                    me.speed.max(BOOST_SPEED)
                } else {
                    me.speed
                };
                let bound = value / (d2.sqrt() / fastest + p.settle_seconds) * (1.0 + 1e-9);
                raced[queued] = (bound, i);
                queued += 1;
                continue;
            }
            // Travel takes at least the straight distance: skip what cannot
            // win.
            if let Some((b, _)) = &best {
                let within = (value / b.rate - p.settle_seconds) * me.speed;
                if within <= 0.0 || within * within <= d2 {
                    continue;
                }
            }
            take(i, q, value, &mut best);
        }
        let raced = &mut raced[..queued];
        raced.sort_unstable_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        for &(bound, i) in raced.iter().take(p.race_cells) {
            // Later cells are bounded no higher, and on a tie come later.
            if !beats(bound, i, &best) {
                break;
            }
            let (q, value) = food(i).expect("queued cells hold food");
            take(i, q, value, &mut best);
        }
        best.map(|(goal, _)| goal)
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

/// Conservative head-center exit forecast for the hard one-way pit.
fn branch_hits_pit(
    center: Point,
    radius: f64,
    speed: f64,
    dt: f64,
    chords: usize,
    at: impl Fn(f64) -> (Point, f64),
) -> bool {
    // The pit tests the head center, at every exit angle. Half a sample's
    // travel bounds an outward excursion between adjacent samples too.
    (0..=chords).any(|j| at(dt * j as f64).0.distance(center) + speed * dt / 2. >= radius)
}

/// Whether a branch `at(t)` (head and heading) dies at the rim within
/// `chords` samples `dt` apart. Its first contact, located between
/// samples by bisection, is lethal or starts a slide; while sliding, the head
/// stays on the rim, so later samples die once their heading points outward
/// steeply at the bearing of their contact point.
fn branch_hits_rim(
    arena: &Arena,
    segments: usize,
    dt: f64,
    chords: usize,
    at: impl Fn(f64) -> (Point, f64),
) -> bool {
    let lead = boundary::CONTACT_LEAD * scale(segments);
    let contact = |t: f64| {
        let (q, h) = at(t);
        forward(q, h, lead).distance(arena.center) >= arena.radius
    };
    let Some(j) = (0..=chords).find(|&j| contact(dt * j as f64)) else {
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
        || (j..=chords).any(|i| {
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
            let enemy_bounds = Me {
                pit: me.pit.filter(|&pit| head.distance(o.arena.center) < pit),
                ..*me
            };
            blocked[b] =
                enemy_bounds.boundary_blocks(o, enemy.segments, enemy.speed, dt, TRAP_CHORDS, at);
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
            if q.distance(o.arena.center) > me.radius(o) - me.width - me.food_margin(p) {
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
    if aim.distance(o.arena.center) > me.radius(o) - me.width - me.food_margin(p) {
        return None;
    }
    let remaining = (std::f64::consts::TAU - wrapped.abs()).max(0.0) * radius / me.speed;
    let progress = (wrapped.abs() / std::f64::consts::TAU)
        .min(1.0)
        .powf(p.coil_progress_power);
    Some(Goal {
        heading: toward(me.head, aim),
        rate: p.coil_weight * progress * (target.segments as f64 + target.fam)
            / (remaining + p.settle_seconds),
        boost: false,
        crosses: None,
    })
}

/// With nothing to eat in view, cross the middle of the arena rather than
/// circling or following the rim.
fn roam(o: &Observation, me: &Me, p: &Params) -> Goal {
    let heading = if me.head.distance(o.arena.center) > p.roam_fraction * me.radius(o) {
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

/// Lower bound on clearance along the full pit steering path. The ordinary
/// two-step steering forecast can hide an early outward bulge during a turn.
fn pit_arc_clearance(
    o: &Observation,
    me: &Me,
    p: &Params,
    heading: f64,
    boost: bool,
    cap: f64,
) -> f64 {
    let Some(pit) = me.pit else {
        return cap;
    };
    let speed_limit = if boost {
        me.speed.max(BOOST_SPEED)
    } else {
        me.speed
    };
    if me.head.distance(o.arena.center) + speed_limit * p.lookahead_seconds < pit - me.width - cap {
        return cap;
    }
    let (mut head, mut angle, mut state) = (me.head, me.heading, me.state);
    let (mut time, mut result) = (0., cap);
    while time < p.lookahead_seconds {
        let dt = SUBSTEP.min(p.lookahead_seconds - time);
        let speed = if boost {
            state.step(true, me.segments, dt, SpeedModel::default())
        } else {
            me.speed
        };
        let delta = wrap(heading - angle);
        let next = turn_then_straight(head, angle, delta, speed, me.turn, dt);
        // A constant-speed turn deviates from its chord by at most v*w*dt²/8;
        // the chord itself stays within the larger endpoint center distance.
        let bow = speed * me.turn * dt * dt / 8.;
        let reach = head
            .distance(o.arena.center)
            .max(next.distance(o.arena.center))
            + bow;
        result = result.min(pit - me.width - reach);
        head = next;
        angle += delta.clamp(-me.turn * dt, me.turn * dt);
        time += dt;
    }
    result
}

/// Clearance of the arc toward `heading` from the rim and nearby bodies,
/// capped at `cap`. Only clearance below `safe` changes a cost, so callers pass
/// `safe` and obstacles that provably cannot lower the running minimum are
/// skipped. `stop` may accept a partial minimum once further decreases cannot
/// change the caller's decision; otherwise the result is the exact capped
/// clearance. With `ramp` the arc accelerates to boost speed.
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
    stop: impl Fn(f64) -> bool,
) -> f64 {
    let (mut path, points, rim_heading) = arc(me, p, heading, ramp);
    let path = &mut path[..points];
    let end = path[points - 1].0;
    // Look further toward the rim so a turn starts in time.
    let ahead = forward(end, rim_heading, me.speed * p.rim_ahead_seconds);
    let rim = |q: Point| me.radius(o) - me.width - q.distance(o.arena.center);
    let mut clearance = rim(end).min(rim(ahead) + p.rim_ahead_credit).min(cap);
    if me.pit.is_some() {
        clearance = clearance.min(pit_arc_clearance(o, me, p, heading, ramp.is_some(), cap));
    }
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
        if stop(clearance) {
            break;
        }
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
#[allow(clippy::too_many_arguments)]
fn search(
    o: &Observation,
    me: &Me,
    p: &Params,
    near: &[Obstacle],
    exits: &[Obstacle],
    goal: &Goal,
    ramp: Option<&[f64; RAMP_SUBSTEPS]>,
    max_offset: f64,
) -> (f64, f64, f64, f64) {
    let mut best = (f64::INFINITY, goal.heading, f64::NEG_INFINITY, 0.0);
    for &offset in &p.fan[..p.fan_len.min(MAX_FAN)] {
        // Offsets grow, so none left can beat a clear arc already found.
        if best.0 <= offset.abs() || offset.abs() >= max_offset {
            break;
        }
        let heading = goal.heading + offset;
        // The minimum can only decrease, so this exact cost expression can
        // only increase. Equal costs keep the earlier arc. A stopped arc
        // therefore cannot replace best (including its exact clearance).
        let loses = |clearance: f64| {
            offset.abs() + (me.safe - clearance).max(0.0) * p.danger_weight >= best.0
        };
        let clearance = clearance(o, me, p, near, heading, me.safe, goal.crosses, ramp, loses);
        let mut cost = offset.abs() + (me.safe - clearance).max(0.0) * p.danger_weight;
        // The exit cost only adds, so skip it where the arc already loses.
        if p.exit_weight > 0.0 && cost < best.0 {
            let (path, points, _) = arc(me, p, heading, ramp);
            let (end, end_heading) = path[points - 1];
            cost = blocked_exits(o, me, p, exits, end, end_heading, cost, best.0);
        }
        if cost < best.0 {
            best = (cost, heading, clearance, offset);
        }
    }
    best
}

/// Heads and tips of our hard-left, straight and hard-right exit branches
/// at `EXIT_CHORDS + 1` samples `dt` apart.
type Branches = [[Point; EXIT_CHORDS + 1]; 3];
fn exit_branches(me: &Me, head: Point, heading: f64, dt: f64, tip: f64) -> (Branches, Branches) {
    // The left branch mirrors the right one across our heading line, so only
    // the right one needs the turn's trigonometry.
    let (ux, uy) = (heading.cos(), heading.sin());
    let mirror = |x: f64, y: f64| {
        let (a, b) = (x * ux + y * uy, y * ux - x * uy);
        (a * ux + b * uy, a * uy - b * ux)
    };
    let mut heads: Branches = [[Point::default(); EXIT_CHORDS + 1]; 3];
    let mut tips: Branches = [[Point::default(); EXIT_CHORDS + 1]; 3];
    for j in 0..=EXIT_CHORDS {
        let t = dt * j as f64;
        let q = turn_then_straight(head, heading, std::f64::consts::PI, me.speed, me.turn, t);
        let h = heading + std::f64::consts::PI.min(me.turn * t);
        let (dx, dy) = (h.cos() * tip, h.sin() * tip);
        heads[2][j] = q;
        tips[2][j] = Point {
            x: q.x + dx,
            y: q.y + dy,
        };
        let (mx, my) = mirror(q.x - head.x, q.y - head.y);
        let (tx, ty) = mirror(dx, dy);
        heads[0][j] = Point {
            x: head.x + mx,
            y: head.y + my,
        };
        tips[0][j] = Point {
            x: heads[0][j].x + tx,
            y: heads[0][j].y + ty,
        };
        let d = me.speed * t;
        heads[1][j] = Point {
            x: head.x + ux * d,
            y: head.y + uy * d,
        };
        tips[1][j] = Point {
            x: heads[1][j].x + ux * tip,
            y: heads[1][j].y + uy * tip,
        };
    }
    (heads, tips)
}

/// Whether segment `a`-`b`, both ends further than `r` (squared `r2`) from
/// `c`, still passes within it. Its nearest point is then interior, so the
/// segment is at least as long as both tangent lengths: a cheap test leaves
/// only long segments near the disc for the exact distance.
/// `a2` and `b2` are the ends' squared distances from `c`.
fn crosses_disc(a: Point, b: Point, a2: f64, b2: f64, c: Point, r2: f64) -> bool {
    (a.x - b.x).powi(2) + (a.y - b.y).powi(2) >= a2 - r2 + b2 - r2
        && segment_distance(c, a, b).powi(2) < r2
}

/// Chords per exit branch: at the default 0.4 s a cruising snake moves
/// about 18 units per chord, less than two minimum widths.
const EXIT_CHORDS: usize = 4;
/// Rim samples along an exit branch are at most this far apart, seconds:
/// sparser samples could miss a turning branch that crosses the rim and
/// returns between them.
const EXIT_RIM_SECONDS: f64 = 0.025;

/// Share of our hard-left, straight and hard-right branches from `head`
/// (heading `heading`) over `exit_seconds` at cruise speed that the rim or a
/// gathered obstacle blocks. Branch tips follow `to_tip`, as arcs do; a
/// segment blocks where a tip chord comes within its width. Head paths count
/// as laid for the whole span (conservative); unseen or ungathered snakes
/// count as open. Returns `cost` plus `exit_weight` per blocked share,
/// stopping once that reaches `beaten` (the arc then loses anyway).
#[allow(clippy::too_many_arguments)]
fn blocked_exits(
    o: &Observation,
    me: &Me,
    p: &Params,
    near: &[Obstacle],
    head: Point,
    heading: f64,
    cost: f64,
    beaten: f64,
) -> f64 {
    let dt = p.exit_seconds / EXIT_CHORDS as f64;
    let tip = (1.0 - p.own_width_factor) * me.width;
    let (heads, tips) = exit_branches(me, head, heading, dt, tip);
    // Branches whose samples all stay further inside than the rim's contact
    // lead plus half a chord cannot touch it; others take the exact check.
    let inside =
        me.radius(o) - boundary::CONTACT_LEAD * scale(o.me.segments) - me.speed * dt / 2.0 - 1e-6;
    let mut blocked = 0;
    for (b, delta) in [-std::f64::consts::PI, 0.0, std::f64::consts::PI]
        .into_iter()
        .enumerate()
    {
        if blocked > 0 && cost + p.exit_weight * blocked as f64 / 3.0 >= beaten {
            break;
        }
        let near_rim = heads[b]
            .iter()
            .any(|q| q.distance(o.arena.center) >= inside);
        let at = |t: f64| {
            let q = turn_then_straight(head, heading, delta, me.speed, me.turn, t);
            (q, heading + delta.signum() * delta.abs().min(me.turn * t))
        };
        if near_rim && {
            let chords = (p.exit_seconds / EXIT_RIM_SECONDS).ceil().max(1.0) as usize;
            me.boundary_blocks(
                o,
                o.me.segments,
                me.speed,
                p.exit_seconds / chords as f64,
                chords,
                at,
            )
        } {
            blocked += 1;
            continue;
        }
        let tips = &tips[b];
        let (lo, hi) = bounds(tips);
        // Chord midpoints and half lengths for the same lower bound as
        // `clearance`: segment distance >= midpoint distance - both halves.
        let chords: [(Point, f64); EXIT_CHORDS] = std::array::from_fn(|j| {
            let (a, b) = (tips[j], tips[j + 1]);
            (
                Point {
                    x: (a.x + b.x) / 2.0,
                    y: (a.y + b.y) / 2.0,
                },
                a.distance(b) / 2.0,
            )
        });
        let hit = near.iter().any(|ob| {
            let w = ob.width;
            if ob.from.x.max(ob.to.x) + w < lo.x
                || ob.from.x.min(ob.to.x) - w > hi.x
                || ob.from.y.max(ob.to.y) + w < lo.y
                || ob.from.y.min(ob.to.y) - w > hi.y
            {
                return false;
            }
            (0..EXIT_CHORDS).any(|j| {
                let (mid, half) = chords[j];
                let bound = w + ob.half + half + 1e-6;
                let (dx, dy) = (mid.x - ob.mid.x, mid.y - ob.mid.y);
                dx * dx + dy * dy <= bound * bound
                    && swept_distance(tips[j], tips[j + 1], ob.from, ob.to) < w
            })
        });
        blocked += usize::from(hit);
    }
    cost + p.exit_weight * blocked as f64 / 3.0
}

/// Boosting toward a goal takes only arcs this close to it, radians.
const BOOST_MAX_OFFSET: f64 = 0.4;

/// Boost toward a goal that needs it, like the original Hunter: only along a
/// clear, nearly straight accelerating arc.
fn boost_toward(
    o: &Observation,
    me: &Me,
    p: &Params,
    near: &[Obstacle],
    exits: &[Obstacle],
    goal: &Goal,
    ramp: &[f64; RAMP_SUBSTEPS],
) -> Option<Action> {
    // Only arcs within `BOOST_MAX_OFFSET` of the goal are taken, and any arc
    // costs at least its offset, so the cheapest arc overall is one of them
    // exactly when the cheapest of them costs less than that.
    let best = search(o, me, p, near, exits, goal, Some(ramp), BOOST_MAX_OFFSET);
    (best.2 >= me.safe && best.0 < BOOST_MAX_OFFSET).then(|| Action {
        heading: wrap(best.1),
        boost: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A two-segment snake at the origin heading east, alone in a sandbox
    /// arena of radius 3000.
    fn lone() -> Observation {
        let world = crate::Simulator::new(crate::WorldOptions {
            seed: 1,
            config: crate::Config::sandbox(),
            population: crate::InitialPopulation::Placed {
                ids: vec!["self".into()],
                placement: crate::Placement::default(),
            },
        })
        .unwrap();
        let mut o = world.observe(0).unwrap();
        o.me.head = Point::default();
        o.me.heading = 0.0;
        o.me.segments = 2;
        o.me.speed = base_speed(2);
        o.me.fam = 0.8;
        o.me.score = crate::physics::score(2, 0.8);
        o.me.body = vec![Point { x: -42.0, y: 0.0 }];
        o.food.clear();
        o.prey.clear();
        o.opponents.clear();
        o.arena.radius = 3000.0;
        o
    }

    /// A4's fractional occupancy penalty decides between two near-equal food
    /// cells: the slightly richer north cell holds a far opponent's body. Bits
    /// come from the kernel before occupancy skipping; factor 1 flips the choice.
    #[test]
    fn a4_food_choice_keeps_its_fractional_occupancy_penalty() {
        let mut o = lone();
        let food = |x, y, value| crate::Food {
            x,
            y,
            value,
            size: None,
            origin: None,
        };
        o.food = vec![food(400.0, 200.0, 1.01), food(400.0, -200.0, 1.0)];
        let mut enemy = o.me.clone();
        enemy.id = "far-head-near-food-body".into();
        enemy.head = Point {
            x: 1800.0,
            y: 1200.0,
        };
        enemy.body = vec![Point { x: 400.0, y: 200.0 }];
        o.opponents.push(enemy);
        let p = crate::bot::params::A4_MEAN;
        assert!(p.occupied_factor < 1.0);
        let decide_with =
            |p: &Params| decide(&o, &Timing::default(), &[], p, &mut Scratch::default());
        let penalized = decide_with(&p);
        let identity = decide_with(&Params {
            occupied_factor: 1.0,
            ..p
        });
        assert_eq!(penalized.heading.to_bits(), 0xbfddac670561bb4f);
        assert_eq!(identity.heading.to_bits(), 0x3fddac670561bb4f);
        assert!(!penalized.boost && !identity.boost);
    }

    fn pellet(x: f64, y: f64, value: f64) -> crate::Food {
        crate::Food {
            x,
            y,
            value,
            size: None,
            origin: None,
        }
    }

    fn still_view(o: &Observation) -> View<'_> {
        View {
            o,
            lead: 0.0,
            sweep: &[],
            sweep_bounds: (Point::default(), Point::default()),
            pickup_radius: 0.0,
            uncertainty: 0.0,
        }
    }

    #[test]
    fn clearance_stops_at_a_cost_tie_before_a_later_obstacle() {
        let o = lone();
        let p = Params::default();
        let mut me = Me::new(&o.me);
        me.safe = p.safe;
        let obstacles = [
            Obstacle::new(me.head, me.head, 10.0, 0, false),
            Obstacle::new(me.head, me.head, 20.0, 1, false),
        ];
        let cost = |c: f64| 0.3 + (me.safe - c).max(0.0) * p.danger_weight;
        let beaten = cost(-10.0);
        let stopped = clearance(&o, &me, &p, &obstacles, 0.0, me.safe, None, None, |c| {
            cost(c) >= beaten
        });
        let complete = clearance(&o, &me, &p, &obstacles, 0.0, me.safe, None, None, |_| false);
        assert_eq!(stopped.to_bits(), (-10.0_f64).to_bits());
        assert_eq!(complete.to_bits(), (-20.0_f64).to_bits());
        assert_eq!(cost(stopped), beaten);
        assert!(cost(complete) > beaten);
    }

    #[test]
    fn pruned_search_matches_exhaustive_fan_costs_bit_for_bit() {
        let mut seed = 0xd8a4_2910_32cf_765b;
        for p in [Params::default(), crate::bot::params::A4_MEAN] {
            for case in 0..128 {
                let mut o = lone();
                o.me.head.x = lcg(&mut seed) * 3000.0;
                o.me.heading = lcg(&mut seed) * std::f64::consts::TAU;
                let mut me = Me::new(&o.me);
                me.safe = p.safe + p.safe_width_factor * me.width;
                let near: Vec<_> = (0..48)
                    .map(|i| {
                        let from = Point {
                            x: me.head.x + (lcg(&mut seed) - 0.5) * 800.0,
                            y: (lcg(&mut seed) - 0.5) * 800.0,
                        };
                        let to = Point {
                            x: from.x + (lcg(&mut seed) - 0.5) * 200.0,
                            y: from.y + (lcg(&mut seed) - 0.5) * 200.0,
                        };
                        Obstacle::new(from, to, lcg(&mut seed) * 80.0, i, i % 3 == 0)
                            .during(0.0, p.lookahead_seconds)
                    })
                    .collect();
                let goal = Goal {
                    heading: lcg(&mut seed) * std::f64::consts::TAU,
                    rate: 1.0,
                    boost: false,
                    crosses: (case % 5 == 0).then_some(0),
                };
                let ramp = (case % 4 == 0).then(|| me.boost_ramp());
                let max_offset = if case % 3 == 0 {
                    BOOST_MAX_OFFSET
                } else {
                    f64::INFINITY
                };
                let actual = search(&o, &me, &p, &near, &near, &goal, ramp.as_ref(), max_offset);
                // Score every eligible arc and every exit in full, without
                // offset/cost pruning or the losing-clearance shortcut.
                let mut expected = (f64::INFINITY, goal.heading, f64::NEG_INFINITY, 0.0);
                for &offset in p.fan[..p.fan_len].iter().filter(|o| o.abs() < max_offset) {
                    let heading = goal.heading + offset;
                    let c = clearance(
                        &o,
                        &me,
                        &p,
                        &near,
                        heading,
                        me.safe,
                        goal.crosses,
                        ramp.as_ref(),
                        |_| false,
                    );
                    let mut cost = offset.abs() + (me.safe - c).max(0.0) * p.danger_weight;
                    if p.exit_weight > 0.0 {
                        let (path, points, _) = arc(&me, &p, heading, ramp.as_ref());
                        let (end, end_heading) = path[points - 1];
                        cost = blocked_exits(
                            &o,
                            &me,
                            &p,
                            &near,
                            end,
                            end_heading,
                            cost,
                            f64::INFINITY,
                        );
                    }
                    if cost < expected.0 {
                        expected = (cost, heading, c, offset);
                    }
                }
                let bits = |(a, b, c, d): (f64, f64, f64, f64)| [a, b, c, d].map(f64::to_bits);
                assert_eq!(bits(actual), bits(expected), "case {case}");
            }
        }
    }

    #[test]
    fn exit_rim_check_sees_a_crossing_between_body_samples() {
        // Review of 745115e: over a 2 s exit both turning branches from an
        // outward heading cross the rim lethally near 200 ms and are back
        // inside by the 500 ms body sample.
        let mut o = lone();
        o.arena.radius = 1000.0;
        let p = Params {
            exit_weight: 3.0,
            exit_seconds: 2.0,
            ..Params::default()
        };
        let me = Me::new(&o.me);
        let head = Point { x: 960.0, y: 0.0 };
        assert_eq!(
            blocked_exits(&o, &me, &p, &[], head, 0.0, 0.0, f64::INFINITY),
            3.0
        );
    }

    #[test]
    fn exit_gathering_reaches_the_whole_width_of_a_large_body() {
        // Review of 745115e: the gathering radius assumed opponents no wider
        // than us, so a 430-segment body across our exits went unseen.
        let mut o = lone();
        let mut enemy = o.me.clone();
        enemy.segments = 430;
        enemy.head = Point { x: 210.0, y: 210.0 };
        enemy.speed = base_speed(430);
        enemy.body = (1..=10)
            .map(|i| Point {
                x: 210.0,
                y: 210.0 - i as f64 * 42.0,
            })
            .collect();
        o.opponents.push(enemy);
        let p = Params {
            exit_weight: 3.0,
            ..Params::default()
        };
        let mut me = Me::new(&o.me);
        me.safe = p.safe;
        let mut s = Scratch::default();
        let lists = scratch_lists(&mut s.near, &mut s.arcs, &mut s.exits);
        gather(&still_view(&o), &me, &p, &[], false, lists);
        let (path, points, _) = arc(&me, &p, 0.0, None);
        let (end, heading) = path[points - 1];
        assert!(blocked_exits(&o, &me, &p, &s.exits, end, heading, 0.0, f64::INFINITY) > 0.0);
    }

    #[test]
    fn exit_gathering_keeps_a_segment_whose_ends_lie_outside_its_reach() {
        // Follow-up review of 859b215: both ends of this short segment lie
        // just beyond the gathering radius, yet the straight exit branch
        // passes within its width.
        let mut o = lone();
        let mut enemy = o.me.clone();
        enemy.head = Point {
            x: 181.796875,
            y: 21.0,
        };
        enemy.heading = std::f64::consts::FRAC_PI_2;
        enemy.body = vec![Point {
            x: 181.796875,
            y: -21.0,
        }];
        o.opponents.push(enemy);
        let p = Params {
            exit_weight: 3.0,
            ..Params::default()
        };
        let mut me = Me::new(&o.me);
        me.safe = p.safe;
        let mut s = Scratch::default();
        let lists = scratch_lists(&mut s.near, &mut s.arcs, &mut s.exits);
        gather(&still_view(&o), &me, &p, &[], false, lists);
        let (path, points, _) = arc(&me, &p, 0.0, None);
        let (end, heading) = path[points - 1];
        assert!(blocked_exits(&o, &me, &p, &s.exits, end, heading, 0.0, f64::INFINITY) > 0.0);
    }

    #[test]
    fn a_declined_racing_boost_values_food_again_at_cruise_speed() {
        // Review of b350be3: a rich cell is ours only by boosting, a post
        // blocks the boost, and cruising there loses it to the rival.
        let mut o = lone();
        o.food = vec![pellet(400.0, 0.0, 20.0), pellet(180.0, 300.0, 1.0)];
        let mut rival = o.me.clone();
        rival.head = Point { x: 700.0, y: 0.0 };
        rival.heading = std::f64::consts::PI;
        rival.body.clear();
        o.opponents.push(rival);
        let mut post = o.me.clone();
        post.head = Point { x: 150.0, y: 0.0 };
        post.speed = 0.0;
        post.body.clear();
        o.opponents.push(post);
        let p = Params {
            race_sigma: 0.15,
            boost_min_score: 0.0,
            ..Params::default()
        };
        let action = decide(&o, &Timing::default(), &[], &p, &mut Scratch::default());
        assert!(!action.boost);
        assert!(action.heading > 0.5, "{action:?}");
    }

    #[test]
    fn racing_times_rivals_from_where_they_will_be() {
        // Review of b350be3: our head is forecast through the latency, so a
        // rival's must be too; from its stale head it seemed slower to the
        // rich cell and we boosted straight at a cell it reaches first.
        let mut o = lone();
        o.food = vec![pellet(400.0, 0.0, 32.0), pellet(180.0, 300.0, 1.0)];
        let mut rival = o.me.clone();
        rival.head = Point { x: 650.0, y: 0.0 };
        rival.heading = std::f64::consts::PI;
        rival.body.clear();
        o.opponents.push(rival);
        let p = Params {
            race_sigma: 0.15,
            boost_min_score: 0.0,
            ..Params::default()
        };
        let timing = Timing {
            observation_age: 0.08,
            ..Timing::default()
        };
        let action = decide(&o, &timing, &[], &p, &mut Scratch::default());
        assert!(!action.boost, "{action:?}");
        assert!(action.heading > 0.5, "{action:?}");
    }

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
                branch_hits_rim(&arena, 2, 0.05, TRAP_CHORDS, |t| {
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
            branch_hits_rim(arena, segments, 0.05, TRAP_CHORDS, |t| {
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
    fn mirrored_exit_branches_match_the_direct_turns() {
        let mut seed = 7;
        for _ in 0..500 {
            let segments = 2 + (lcg(&mut seed) * 600.0) as usize;
            let heading = (lcg(&mut seed) - 0.5) * 20.0;
            let head = Point {
                x: (lcg(&mut seed) - 0.5) * 4e4,
                y: (lcg(&mut seed) - 0.5) * 4e4,
            };
            let me = Me {
                head,
                heading,
                speed: base_speed(segments),
                turn: turn_rate(segments),
                width: 0.0,
                segments,
                state: SpeedState::from_observed_speed(base_speed(segments), segments),
                safe: 0.0,
                pit: None,
            };
            let (dt, tip) = (0.1 + lcg(&mut seed) * 0.4, lcg(&mut seed) * 30.0);
            let (heads, tips) = exit_branches(&me, head, heading, dt, tip);
            for (b, delta) in [-std::f64::consts::PI, 0.0, std::f64::consts::PI]
                .into_iter()
                .enumerate()
            {
                for j in 0..=EXIT_CHORDS {
                    let t = dt * j as f64;
                    let q = turn_then_straight(head, heading, delta, me.speed, me.turn, t);
                    let h = heading + delta.signum() * delta.abs().min(me.turn * t);
                    let tipped = forward(q, h, tip);
                    assert!(heads[b][j].distance(q) < 1e-6, "{b} {j}");
                    assert!(tips[b][j].distance(tipped) < 1e-6, "{b} {j}");
                }
            }
        }
    }

    #[test]
    fn pit_clearance_detects_an_early_exit_hidden_by_the_coarse_endpoints() {
        let mut o = lone();
        o.arena.radius = 3584.;
        o.me.head = Point { x: 1783.9, y: 0. };
        o.me.heading = 80_f64.to_radians();
        o.me.segments = 26;
        o.me.speed = base_speed(26);
        let mut me = Me::new(&o.me);
        me.pit = Some(1784.);
        let p = crate::bot::params::A4_MEAN;
        let target = std::f64::consts::PI;
        let (path, n, rim_heading) = arc(&me, &p, target, None);
        let end = path[n - 1].0;
        let ahead = forward(end, rim_heading, me.speed * p.rim_ahead_seconds);
        let old = (1784. - me.width - end.distance(o.arena.center))
            .min(1784. - me.width - ahead.distance(o.arena.center) + p.rim_ahead_credit);
        assert!(
            old >= p.safe,
            "endpoint-only checks report this safe: {old}"
        );
        let (first_step, _) = advance(me.head, me.heading, target, me.speed, me.turn, 0.007);
        assert!(first_step.distance(o.arena.center) > 1784.);
        let checked = clearance(&o, &me, &p, &[], target, p.safe, None, None, |_| false);
        assert!(checked < 0., "the full pit path must be unsafe: {checked}");
        // A pit-less decision retains the original outer-rim arithmetic.
        me.pit = None;
        let expected = (o.arena.radius - me.width - end.distance(o.arena.center))
            .min(o.arena.radius - me.width - ahead.distance(o.arena.center) + p.rim_ahead_credit)
            .min(p.safe);
        assert_eq!(
            clearance(&o, &me, &p, &[], target, p.safe, None, None, |_| false).to_bits(),
            expected.to_bits()
        );
        me.pit = Some(1784.);
        me.speed = BOOST_SPEED;
        me.state = SpeedState::from_observed_speed(BOOST_SPEED, me.segments);
        let ramp = me.boost_ramp();
        assert!(
            clearance(&o, &me, &p, &[], target, p.safe, None, Some(&ramp), |_| {
                false
            }) < 0.
        );
    }

    #[test]
    fn forecast_entry_uses_the_pit_before_the_next_received_position() {
        let mut o = lone();
        o.arena.radius = 3584.;
        o.me.head = Point { x: 1790., y: 0. };
        o.me.heading = std::f64::consts::PI;
        o.me.segments = 26;
        o.me.speed = base_speed(26);
        let timing = Timing {
            observation_age: 0.8,
            ..Default::default()
        };
        let p = crate::bot::params::A4_MEAN;
        let steps = (0.8 * p.lead_scale / SUBSTEP).round() as usize;
        let motion = replay(&o.me, &o.arena, &timing, steps, |_| {});
        assert!(motion.head.distance(o.arena.center) < 1784.);
        let mut projected = o.clone();
        projected.me.head = motion.head;
        projected.me.heading = motion.heading;
        projected.me.speed = motion.speed.speed;
        let mut distinguished = false;
        for target in [Point { x: 1300., y: 300. }, Point { x: 1000., y: -500. }] {
            let food = crate::Food {
                x: target.x,
                y: target.y,
                value: 10.,
                size: None,
                origin: None,
            };
            o.food = vec![food];
            projected.food = vec![food];
            let run = |obs: &Observation, t: &Timing, pit| {
                decide_with_pit(obs, t, &[], &p, &mut Scratch::default(), pit)
            };
            let expected = run(&projected, &Timing::default(), Some(1784.));
            let entry = run(&projected, &Timing::default(), Some(1.));
            if expected != entry {
                assert_eq!(run(&o, &timing, Some(1784.)), expected);
                distinguished = true;
            }
        }
        assert!(
            distinguished,
            "regression must distinguish foraging inside from entering"
        );
    }

    #[test]
    fn pit_escape_forecasts_also_keep_a_smaller_outer_wall() {
        let mut o = lone();
        o.arena.radius = 1000.;
        let mut me = Me::new(&o.me);
        me.pit = Some(1784.);
        let at = |t| {
            (
                Point {
                    x: 900. + 190. * t,
                    y: 0.,
                },
                0.,
            )
        };
        assert!(!branch_hits_pit(o.arena.center, 1784., 190., 0.05, 30, at));
        assert!(me.boundary_blocks(&o, 26, 190., 0.05, 30, at));
    }

    #[test]
    fn a_tangent_pit_exit_is_lethal_even_when_the_outer_wall_would_slide() {
        let arena = Arena {
            center: Point::default(),
            radius: 1784.,
        };
        let at = |t| {
            (
                Point {
                    x: 1744.,
                    y: BOOST_SPEED * t,
                },
                std::f64::consts::FRAC_PI_2,
            )
        };
        assert!(!branch_hits_rim(&arena, 26, 0.05, 30, at));
        assert!(branch_hits_pit(
            arena.center,
            arena.radius,
            BOOST_SPEED,
            0.05,
            30,
            at
        ));
        assert!(!branch_hits_pit(
            arena.center,
            arena.radius,
            BOOST_SPEED,
            0.05,
            30,
            |t| (
                Point {
                    x: 1200.,
                    y: BOOST_SPEED * t
                },
                std::f64::consts::FRAC_PI_2
            )
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
            pit: None,
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
