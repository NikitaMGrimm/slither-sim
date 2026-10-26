//! Local process transport. Each connection controls a session of independent worlds.
//!
//! [`serve`] speaks the JSON-lines protocol over a stream. In-process callers can
//! drive the same [`Session`] directly: [`Session::request`] answers one JSON
//! request, and [`Session::step`] with [`Session::frame`] skip JSON altogether.
use crate::{
    Action, Config, Constraint, DeathEvent, ErrorKind, Food, InitialPopulation, Placement, Prey,
    SimError, Simulator, Snake, WorldOptions,
};
use frame::Frame;
pub use frame::{
    DeathColumns, FoodColumns, FrameRef, NO_ORIGIN, NONE, ORIGINS, PreyColumns, REASONS,
    SnakeColumns, ViewColumns,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, BufRead, Write};
use std::num::{NonZeroU32, NonZeroUsize};

mod frame;
#[cfg(test)]
mod tests;

pub const PROTOCOL: &str = "slither-sim-json-v3";

/// A step rejected before advancement, or a failure after advancement began.
#[derive(Debug, PartialEq, Eq)]
pub enum StepError {
    /// No world advanced; the session is unchanged.
    Rejected(SimError),
    /// All worlds were cleared after advancement failed; reset to continue.
    Ended(SimError),
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(cause) => cause.fmt(f),
            Self::Ended(cause) => write!(f, "{cause}{ENDED}"),
        }
    }
}

impl std::error::Error for StepError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Rejected(cause) | Self::Ended(cause) => cause,
        })
    }
}

impl From<SimError> for StepError {
    fn from(cause: SimError) -> Self {
        Self::Rejected(cause)
    }
}

/// Frame slot indices are `u32` and `u32::MAX` means none.
const MAX_AGENTS: usize = NONE as usize;
/// Worlds are independent; only `worlds * agents` and memory bound them.
const MAX_WORLDS: usize = usize::MAX;
/// Frames report the ticks a step advanced as a `u32`.
const MAX_DECISION_TICKS: usize = u32::MAX as usize;

fn default_seed() -> u32 {
    1
}
fn default_worlds() -> usize {
    1
}
fn default_agents() -> usize {
    4
}
fn default_decision_ticks() -> usize {
    6
}

/// Typed reset options. Placement defaults to the historical synthetic ring.
#[derive(Clone, Debug)]
pub struct ResetOptions {
    pub seed: u32,
    pub worlds: NonZeroUsize,
    pub agents: NonZeroU32,
    pub decision_ticks: NonZeroU32,
    pub config: Config,
    pub placement: Placement,
}

impl Default for ResetOptions {
    fn default() -> Self {
        Self {
            seed: default_seed(),
            worlds: NonZeroUsize::new(default_worlds()).unwrap(),
            agents: NonZeroU32::new(default_agents() as u32).unwrap(),
            decision_ticks: NonZeroU32::new(default_decision_ticks() as u32).unwrap(),
            config: Config::default(),
            placement: Placement::default(),
        }
    }
}

impl ResetOptions {
    /// Shared boundary validation for JSON and language bindings, before any
    /// allocation or world creation. Typed callers may construct options directly.
    pub fn from_counts(
        seed: u32,
        worlds: usize,
        agents: usize,
        decision_ticks: usize,
        config: Config,
        placement: Placement,
    ) -> Result<Self, SimError> {
        if worlds == 0 || agents == 0 || decision_ticks == 0 {
            return Err(SimError::config(
                Constraint::NonPositive,
                if worlds == 0 {
                    "worlds"
                } else if agents == 0 {
                    "agents"
                } else {
                    "decisionTicks"
                },
                "Need at least one world, agent and decision tick",
            ));
        }
        if agents > MAX_AGENTS {
            return Err(SimError::new(ErrorKind::RepresentationLimit,
                format!("agents must be at most {MAX_AGENTS}: frame slot indices are u32 and u32::MAX means none")).at_field("agents"));
        }
        if decision_ticks > MAX_DECISION_TICKS {
            return Err(SimError::new(
                ErrorKind::RepresentationLimit,
                format!(
                    "decisionTicks must be at most {MAX_DECISION_TICKS}: frames report ticks as u32"
                ),
            )
            .at_field("decisionTicks"));
        }
        Ok(Self {
            seed,
            worlds: NonZeroUsize::new(worlds).unwrap(),
            agents: NonZeroU32::new(agents as u32).unwrap(),
            decision_ticks: NonZeroU32::new(decision_ticks as u32).unwrap(),
            config,
            placement,
        })
    }
}

#[derive(Clone, Debug)]
pub struct ResetSummary {
    pub config: Config,
    pub seed: u32,
    pub placement: Placement,
    pub decision_ticks: NonZeroU32,
    pub warnings: Vec<String>,
}

/// Fully built worlds and frames. Dropping it leaves the incumbent untouched.
pub struct PreparedReset {
    run: Run,
    summary: ResetSummary,
    unprinted: Vec<String>,
}

impl std::fmt::Debug for PreparedReset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedReset")
            .field("summary", &self.summary)
            .finish_non_exhaustive()
    }
}

impl PreparedReset {
    pub fn summary(&self) -> &ResetSummary {
        &self.summary
    }
}

/// One world's actions: a heading and a boost flag per agent slot. Dead slots
/// may give a null heading.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WorldActions {
    heading: Vec<Option<f64>>,
    boost: Vec<bool>,
}

#[derive(Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum Request {
    // Empty struct variants make serde reject unknown fields on these requests too.
    Info {},
    Reset {
        #[serde(default = "default_seed")]
        seed: u32,
        #[serde(default = "default_worlds")]
        worlds: usize,
        #[serde(default = "default_agents")]
        agents: usize,
        #[serde(default = "default_decision_ticks")]
        decision_ticks: usize,
        #[serde(default)]
        config: Box<Config>,
        #[serde(default)]
        placement: Placement,
    },
    Step {
        actions: Vec<WorldActions>,
    },
    Snapshot {
        #[serde(default)]
        world: usize,
    },
}

#[derive(Serialize)]
struct SnapshotReply<'a> {
    config: &'a Config,
    time: f64,
    snakes: &'a [Snake],
    food: &'a [Food],
    prey: &'a [Prey],
    deaths: &'a [DeathEvent],
}

/// Replies keep the sorted-key bytes of the `serde_json::Value` they were once
/// built through; see `crate::json`.
fn reply(result: &impl Serialize) -> Vec<u8> {
    crate::json::reply(PROTOCOL, result)
}

/// `{"protocol":..,"result":{<head>"worlds":[<frames>]}}`, where `head` holds any
/// members that sort before `worlds`, each followed by a comma.
fn worlds_reply(head: &[u8], frames: &[Vec<u8>]) -> Result<Vec<u8>, SimError> {
    let length = frames.iter().try_fold(head.len() + 64, |sum, frame| {
        sum.checked_add(frame.len() + 1)
    });
    let mut out = Vec::new();
    length
        .and_then(|length| out.try_reserve_exact(length).ok())
        .ok_or_else(|| {
            SimError::new(
                ErrorKind::AllocationFailed,
                "The reply does not fit in memory",
            )
        })?;
    out.extend_from_slice(b"{\"protocol\":");
    out.extend_from_slice(&serde_json::to_vec(PROTOCOL).expect("strings serialize"));
    out.extend_from_slice(b",\"result\":{");
    out.extend_from_slice(head);
    out.extend_from_slice(b"\"worlds\":[");
    for (index, frame) in frames.iter().enumerate() {
        if index > 0 {
            out.push(b',');
        }
        out.extend_from_slice(frame);
    }
    out.extend_from_slice(b"]}}");
    Ok(out)
}

struct WorldState {
    world: Simulator,
    frame: Frame,
    actions: Vec<Option<Action>>,
}

impl WorldState {
    fn set_actions(
        &mut self,
        actions: impl IntoIterator<Item = Option<Action>>,
    ) -> Result<(), SimError> {
        self.actions.clear();
        self.actions.extend(actions);
        self.world.validate_actions(&self.actions)
    }

    /// Holds the validated actions for up to `decision_ticks` ticks, stopping
    /// once every snake has died, then collects the new frame.
    fn advance(&mut self, decision_ticks: usize) -> Result<(), SimError> {
        let previous_deaths = self.world.deaths().len();
        let mut ticks = 0;
        for _ in 0..decision_ticks {
            if self.world.living() == 0 {
                break;
            }
            self.world.advance_validated(&self.actions)?;
            ticks += 1;
        }
        self.frame
            .collect(&self.world, ticks, &self.world.deaths()[previous_deaths..])
    }
}

struct Run {
    worlds: Vec<WorldState>,
    agents: usize,
    decision_ticks: usize,
}

/// The first failure of `results`, naming its world.
fn each_ok<R>(results: Vec<Result<R, SimError>>) -> Result<Vec<R>, SimError> {
    results
        .into_iter()
        .enumerate()
        .map(|(index, result)| result.map_err(|error| error.at_world(index)))
        .collect()
}

/// Advancement or frame collection failed, so the session cannot continue.
const ENDED: &str = "; the session has ended, reset to continue";

/// `work` on every world, on the rayon pool when there are several. Worlds are
/// independent and results return in world order, so replies never depend on it.
fn each_world<R: Send>(
    worlds: &mut [WorldState],
    work: impl Fn(&mut WorldState) -> R + Sync + Send,
) -> Vec<R> {
    #[cfg(feature = "parallel")]
    if worlds.len() > 1 {
        use rayon::prelude::*;
        return worlds.par_iter_mut().map(work).collect();
    }
    worlds.iter_mut().map(work).collect()
}

/// One controller's worlds, as `serve` keeps them for a connection.
///
/// JSON requests and typed steps act on the same worlds and may be mixed. A
/// session is `Send` but not shared: use it from one thread at a time.
#[derive(Default)]
pub struct Session {
    current: Option<Run>,
    /// Warnings of the latest reset that `serve` has not printed yet.
    unprinted: Vec<String>,
}

impl Session {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn prepare_reset(options: ResetOptions) -> Result<PreparedReset, SimError> {
        let ResetOptions {
            seed,
            worlds,
            agents,
            decision_ticks,
            mut config,
            placement,
        } = options;
        let worlds = worlds.get();
        let agents = usize::try_from(agents.get()).map_err(|_| {
            SimError::new(ErrorKind::RepresentationLimit, "agents must fit in usize")
                .at_field("agents")
        })?;
        worlds.checked_mul(agents).ok_or_else(|| {
            SimError::new(
                ErrorKind::RepresentationLimit,
                "worlds * agents must fit in usize",
            )
            .at_field("worlds")
        })?;
        config.size_for_population(agents);
        config.validate()?;
        let mut states = Vec::new();
        states.try_reserve_exact(worlds).map_err(|_| {
            SimError::new(
                ErrorKind::AllocationFailed,
                format!("{worlds} worlds do not fit in memory"),
            )
            .at_field("worlds")
        })?;
        for index in 0..worlds {
            let mut ids = Vec::new();
            ids.try_reserve_exact(agents).map_err(|_| {
                SimError::new(
                    ErrorKind::AllocationFailed,
                    "Agent slots do not fit in memory",
                )
                .at_field("agents")
                .at_world(index)
            })?;
            ids.extend((0..agents).map(|slot| slot.to_string()));
            let world = Simulator::new(WorldOptions {
                seed: seed.wrapping_add(index as u32),
                config: config.clone(),
                population: InitialPopulation::Placed { ids, placement },
            })
            .map_err(|e| e.at_world(index))?;
            states.push(WorldState {
                world,
                frame: Frame::default(),
                actions: Vec::new(),
            });
        }
        each_ok(each_world(&mut states, |state| {
            state.frame.collect(&state.world, 0, &[])
        }))?;
        let warnings = config.warnings();
        let summary = ResetSummary {
            config: states[0].world.config().clone(),
            seed,
            placement,
            decision_ticks,
            warnings: warnings.clone(),
        };
        Ok(PreparedReset {
            run: Run {
                worlds: states,
                agents,
                decision_ticks: decision_ticks.get() as usize,
            },
            summary,
            unprinted: warnings,
        })
    }

    /// Installs already allocated state; cannot fail or allocate.
    pub fn commit_reset(&mut self, prepared: PreparedReset) -> ResetSummary {
        self.current = Some(prepared.run);
        self.unprinted = prepared.unprinted;
        prepared.summary
    }

    pub fn reset(&mut self, options: ResetOptions) -> Result<ResetSummary, SimError> {
        let prepared = Self::prepare_reset(options)?;
        Ok(self.commit_reset(prepared))
    }

    /// Answers one JSON request line, given without its newline, with exactly
    /// the reply line `serve` writes, also without its newline. Error replies
    /// include `sessionEnded`: false for unchanged-state rejection, true when
    /// advancement failed and cleared all worlds. Reset to continue in that case.
    pub fn request(&mut self, line: &[u8]) -> Vec<u8> {
        let result = serde_json::from_slice(line)
            .map_err(|error| {
                StepError::Rejected(SimError::new(ErrorKind::InvalidRequest, error.to_string()))
            })
            .and_then(|request| self.handle(request));
        match result {
            Ok(reply) => reply,
            Err(error) => crate::json::to_vec(&json!({
                "protocol": PROTOCOL,
                "error": error.to_string(),
                "sessionEnded": matches!(error, StepError::Ended(_)),
            }))
            .expect("error replies serialize"),
        }
    }

    /// Steps every world like a JSON `step`, without writing its reply; read
    /// the new frames with [`Session::frame`]. `headings` and `boosts` hold
    /// one entry per agent slot of every world, world-major: slot `s` of world
    /// `w` is entry `w * agents + s`. A NaN heading means no action, which only
    /// dead slots may give. [`StepError::Rejected`] changes nothing. Numeric
    /// overflow during advancement or an unrepresentable frame returns
    /// [`StepError::Ended`] and clears all worlds; reset to continue.
    pub fn step(&mut self, headings: &[f64], boosts: &[bool]) -> Result<(), StepError> {
        let run = self
            .current
            .as_mut()
            .ok_or_else(|| SimError::new(ErrorKind::NotReset, "Reset first"))?;
        let need = run.worlds.len() * run.agents;
        if headings.len() != need || boosts.len() != need {
            return Err(SimError::new(
                ErrorKind::ActionShape {
                    expected: need,
                    actual: if headings.len() != need {
                        headings.len()
                    } else {
                        boosts.len()
                    },
                },
                format!(
                    "One heading and one boost per agent of every world required: \
                 got {} headings and {} boosts, need {need}",
                    headings.len(),
                    boosts.len()
                ),
            )
            .at_field(if headings.len() != need {
                "headings"
            } else {
                "boosts"
            })
            .into());
        }
        // Reject the whole step before advancing any world.
        for (index, (state, (headings, boosts))) in run
            .worlds
            .iter_mut()
            .zip(headings.chunks(run.agents).zip(boosts.chunks(run.agents)))
            .enumerate()
        {
            state
                .set_actions(headings.iter().zip(boosts).map(|(&heading, &boost)| {
                    (!heading.is_nan()).then_some(Action { heading, boost })
                }))
                .map_err(|error| error.at_world(index))?;
        }
        let decision_ticks = run.decision_ticks;
        let result = each_ok(each_world(&mut run.worlds, |state| {
            state.advance(decision_ticks)
        }));
        result.map(|_| ()).map_err(|error| {
            self.current = None;
            StepError::Ended(error)
        })
    }

    /// World `world`'s frame after the latest reset or step, or `None` before
    /// the first reset and for unknown worlds. It holds the values the JSON
    /// frame prints, rounded alike.
    pub fn frame(&self, world: usize) -> Option<FrameRef<'_>> {
        Some(self.current.as_ref()?.worlds.get(world)?.frame.columns())
    }

    /// Worlds of the current session; 0 before the first reset.
    pub fn worlds(&self) -> usize {
        self.current.as_ref().map_or(0, |run| run.worlds.len())
    }

    /// Agent slots per world; 0 before the first reset.
    pub fn agents(&self) -> usize {
        self.current.as_ref().map_or(0, |run| run.agents)
    }

    fn handle(&mut self, request: Request) -> Result<Vec<u8>, StepError> {
        match request {
            Request::Info {} => Ok(reply(&json!({
                "version": env!("CARGO_PKG_VERSION"),
                "defaultConfig": Config::default(),
                "defaultDecisionTicks": default_decision_ticks(),
                "maxAgents": MAX_AGENTS,
                "maxWorlds": MAX_WORLDS,
                "maxDecisionTicks": MAX_DECISION_TICKS,
                "origins": frame::ORIGINS,
                "precision": {
                    "angle": 1.0 / frame::ANGLE_STEPS,
                    "position": 1.0 / frame::POSITION_STEPS,
                    "speed": 1.0 / frame::SPEED_STEPS,
                },
            }))),
            Request::Reset {
                seed,
                worlds,
                agents,
                decision_ticks,
                config,
                placement,
            } => {
                let options = ResetOptions::from_counts(
                    seed,
                    worlds,
                    agents,
                    decision_ticks,
                    *config,
                    placement,
                )?;
                let prepared = Self::prepare_reset(options)?;
                // Encode every frame and the complete reply before installing it.
                let frames: Vec<_> = prepared
                    .run
                    .worlds
                    .iter()
                    .map(|state| state.frame.encode())
                    .collect();
                let mut head = b"\"config\":".to_vec();
                head.extend_from_slice(
                    &crate::json::to_vec(&prepared.summary.config).expect("configs serialize"),
                );
                head.extend_from_slice(
                    format!(",\"decisionTicks\":{decision_ticks},\"seed\":{seed},").as_bytes(),
                );
                head.extend_from_slice(b"\"warnings\":");
                head.extend_from_slice(
                    &crate::json::to_vec(&prepared.summary.warnings).expect("strings serialize"),
                );
                head.push(b',');
                let result = worlds_reply(&head, &frames)?;
                self.commit_reset(prepared);
                Ok(result)
            }
            Request::Step { actions } => {
                let run = self
                    .current
                    .as_mut()
                    .ok_or_else(|| SimError::new(ErrorKind::NotReset, "Reset first"))?;
                if actions.len() != run.worlds.len() {
                    return Err(SimError::new(
                        ErrorKind::ActionShape {
                            expected: run.worlds.len(),
                            actual: actions.len(),
                        },
                        format!(
                            "One action entry per world required: got {}, need {}",
                            actions.len(),
                            run.worlds.len()
                        ),
                    )
                    .at_field("actions")
                    .into());
                }
                // Reject the whole request before advancing any world.
                for (index, (state, world_actions)) in
                    run.worlds.iter_mut().zip(actions).enumerate()
                {
                    if world_actions.heading.len() != run.agents
                        || world_actions.boost.len() != run.agents
                    {
                        return Err(SimError::new(
                            ErrorKind::ActionShape {
                                expected: run.agents,
                                actual: if world_actions.heading.len() != run.agents {
                                    world_actions.heading.len()
                                } else {
                                    world_actions.boost.len()
                                },
                            },
                            "one heading and one boost per agent required",
                        )
                        .at_world(index)
                        .at_field(if world_actions.heading.len() != run.agents {
                            "heading"
                        } else {
                            "boost"
                        })
                        .into());
                    }
                    state
                        .set_actions(world_actions.heading.iter().zip(&world_actions.boost).map(
                            |(&heading, &boost)| heading.map(|heading| Action { heading, boost }),
                        ))
                        .map_err(|error| error.at_world(index))?;
                }
                let decision_ticks = run.decision_ticks;
                let reply = each_ok(each_world(&mut run.worlds, |state| {
                    state.advance(decision_ticks)?;
                    Ok(state.frame.encode())
                }))
                .and_then(|frames| worlds_reply(b"", &frames));
                // Either failure follows advancement, so a retry would step twice.
                reply.map_err(|error| {
                    self.current = None;
                    StepError::Ended(error)
                })
            }
            Request::Snapshot { world } => {
                let run = self
                    .current
                    .as_ref()
                    .ok_or_else(|| SimError::new(ErrorKind::NotReset, "Reset first"))?;
                let world = &run
                    .worlds
                    .get(world)
                    .ok_or_else(|| SimError::new(ErrorKind::UnknownWorld, "Unknown world"))?
                    .world;
                // Deliberately exclude hidden RNG/control state from this debug view.
                Ok(reply(&SnapshotReply {
                    config: world.config(),
                    time: world.time(),
                    snakes: world.snakes(),
                    food: world.food(),
                    prey: world.prey(),
                    deaths: world.deaths(),
                }))
            }
        }
    }
}

/// Serve a trusted local controller until EOF. Request errors leave the stream
/// usable. Sessions with several worlds step them on the current rayon pool.
/// Reset warnings also go to stderr, since stdout carries only replies.
pub fn serve(mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut session = Session::default();
    let mut line = Vec::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        let request = line.strip_suffix(b"\n").unwrap_or(&line);
        let request = request.strip_suffix(b"\r").unwrap_or(request);
        output.write_all(&session.request(request))?;
        output.write_all(b"\n")?;
        output.flush()?;
        for warning in session.unprinted.drain(..) {
            eprintln!("warning: {warning}");
        }
    }
}

#[cfg(test)]
mod reset_tests {
    use super::*;

    #[test]
    fn prepared_reset_sizes_before_validation_and_keeps_official_prey_density() {
        let prepared = Session::prepare_reset(ResetOptions {
            agents: NonZeroU32::new(484).unwrap(),
            config: Config {
                // This ring fits only after population sizing enlarges the default arena.
                spawn_radius: Some(1000.),
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::default()
            },
            ..ResetOptions::default()
        })
        .unwrap();
        assert_eq!(prepared.summary().config.radius, 14_740.);
        assert_eq!(prepared.summary().config.prey.count, 68);
        assert!(prepared.summary().config.size_to_population);
        let mut typed = Session::new();
        typed.commit_reset(prepared);
        let mut json = Session::new();
        let reply: serde_json::Value = serde_json::from_slice(&json.request(
            br#"{"op":"reset","agents":484,"config":{"spawnRadius":1000,"foodCount":0,"ambientReplenishPerSecond":0}}"#,
        ))
        .unwrap();
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(
            typed.current.as_ref().unwrap().worlds[0].frame.encode(),
            json.current.as_ref().unwrap().worlds[0].frame.encode()
        );
    }

    #[test]
    fn maximum_config_radius_resets_and_checkpoints() {
        let mut session = Session::new();
        let reply: serde_json::Value = serde_json::from_slice(&session.request(
            br#"{"op":"reset","agents":1,"config":{"preset":"sandbox","radius":1e150,"foodCount":0,"radiusUpdates":[{"time":0.007,"radius":1e150}]}}"#,
        ))
        .unwrap();
        assert!(reply.get("error").is_none(), "{reply}");
        assert_eq!(reply["result"]["config"]["radius"], json!(1e150));
        let world = &session.current.as_ref().unwrap().worlds[0].world;
        let saved = serde_json::to_vec(&world.checkpoint().unwrap()).unwrap();
        let restored =
            Simulator::from_checkpoint(&saved, crate::RestoreOptions::default()).unwrap();
        assert_eq!(
            serde_json::to_vec(&restored.checkpoint().unwrap()).unwrap(),
            saved
        );
    }

    #[test]
    fn typed_preparation_is_atomic_and_matches_json_defaults() {
        let mut session = Session::new();
        let summary = session.reset(ResetOptions::default()).unwrap();
        assert_eq!(summary.placement, Placement::TestRing);
        let before = session.request(br#"{"op":"snapshot"}"#);
        let mut options = ResetOptions::default();
        options.config.radius = -1.;
        assert!(session.reset(options).is_err());
        assert_eq!(session.request(br#"{"op":"snapshot"}"#), before);
        let options = ResetOptions {
            seed: 8,
            ..Default::default()
        };
        let prepared = Session::prepare_reset(options).unwrap();
        assert_eq!(prepared.summary().seed, 8);
        assert_eq!(session.request(br#"{"op":"snapshot"}"#), before);
        drop(prepared);
        assert_eq!(session.request(br#"{"op":"snapshot"}"#), before);
        let prepared = Session::prepare_reset(ResetOptions {
            seed: 8,
            ..Default::default()
        })
        .unwrap();
        session.commit_reset(prepared);
        let mut encoded = Session::new();
        encoded.request(br#"{"op":"reset","seed":8}"#);
        assert_eq!(
            session.request(br#"{"op":"snapshot"}"#),
            encoded.request(br#"{"op":"snapshot"}"#)
        );
        assert_eq!(
            session.current.as_ref().unwrap().worlds[0].frame.encode(),
            encoded.current.as_ref().unwrap().worlds[0].frame.encode()
        );
    }

    #[test]
    fn representation_and_later_world_placement_failure_keep_old_session() {
        let mut session = Session::new();
        session.reset(ResetOptions::default()).unwrap();
        let before = session.request(br#"{"op":"snapshot"}"#);
        let too_many = ResetOptions {
            worlds: NonZeroUsize::new(usize::MAX).unwrap(),
            agents: NonZeroU32::new(2).unwrap(),
            ..Default::default()
        };
        assert_eq!(
            session.reset(too_many).unwrap_err().kind,
            ErrorKind::RepresentationLimit
        );
        let mut found = false;
        for seed in 0..1000 {
            let options = ResetOptions {
                seed,
                worlds: NonZeroUsize::new(2).unwrap(),
                agents: NonZeroU32::new(2).unwrap(),
                config: Config {
                    radius: 400.,
                    food_count: 0,
                    ..Config::sandbox()
                },
                placement: Placement::Admitted {
                    attempts: NonZeroU32::new(1).unwrap(),
                    preferred_clearance: 0.,
                },
                ..Default::default()
            };
            if let Err(error) = Session::prepare_reset(options.clone())
                && error.world == Some(1)
                && matches!(error.kind, ErrorKind::PlacementExhausted { .. })
            {
                assert!(session.reset(options).is_err());
                found = true;
                break;
            }
        }
        assert!(
            found,
            "fixture must exercise a later-world admission failure"
        );
        assert_eq!(session.request(br#"{"op":"snapshot"}"#), before);
    }
}
