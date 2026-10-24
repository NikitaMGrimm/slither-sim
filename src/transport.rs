//! Local process transport. Each connection controls a session of independent worlds.
//!
//! [`serve`] speaks the JSON-lines protocol over a stream. In-process callers can
//! drive the same [`Session`] directly: [`Session::request`] answers one JSON
//! request, and [`Session::step`] with [`Session::frame`] skip JSON altogether.
use crate::{Action, Config, DeathEvent, Food, Prey, Simulator, Snake};
use frame::Frame;
pub use frame::{
    DeathColumns, FoodColumns, FrameRef, NO_ORIGIN, NONE, ORIGINS, PreyColumns, REASONS,
    SnakeColumns, ViewColumns,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, BufRead, Write};

mod frame;

pub const PROTOCOL: &str = "slither-sim-json-v3";

/// A step rejected before advancement, or a failure after advancement began.
#[derive(Debug, PartialEq, Eq)]
pub enum StepError {
    /// No world advanced; the session is unchanged.
    Rejected(String),
    /// All worlds were cleared after advancement failed; reset to continue.
    Ended(String),
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(message) | Self::Ended(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for StepError {}

impl From<String> for StepError {
    fn from(message: String) -> Self {
        Self::Rejected(message)
    }
}

impl From<&str> for StepError {
    fn from(message: &str) -> Self {
        Self::Rejected(message.into())
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
fn worlds_reply(head: &[u8], frames: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let length = frames.iter().try_fold(head.len() + 64, |sum, frame| {
        sum.checked_add(frame.len() + 1)
    });
    let mut out = Vec::new();
    length
        .and_then(|length| out.try_reserve_exact(length).ok())
        .ok_or("The reply does not fit in memory")?;
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
    /// Holds the validated actions for up to `decision_ticks` ticks, stopping
    /// once every snake has died, then collects the new frame.
    fn advance(&mut self, decision_ticks: usize) -> Result<(), String> {
        let previous_deaths = self.world.deaths.len();
        let mut ticks = 0;
        for _ in 0..decision_ticks {
            if self.world.living() == 0 {
                break;
            }
            self.world.advance_validated(&self.actions)?;
            ticks += 1;
        }
        self.frame
            .collect(&self.world, ticks, &self.world.deaths[previous_deaths..])
    }
}

struct Run {
    worlds: Vec<WorldState>,
    agents: usize,
    decision_ticks: usize,
}

/// The first failure of `results`, naming its world.
fn each_ok<R>(results: Vec<Result<R, String>>) -> Result<Vec<R>, String> {
    results
        .into_iter()
        .enumerate()
        .map(|(index, result)| result.map_err(|error| format!("World {index}: {error}")))
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

    /// Answers one JSON request line, given without its newline, with exactly
    /// the reply line `serve` writes, also without its newline. Error replies
    /// include `sessionEnded`: false for unchanged-state rejection, true when
    /// advancement failed and cleared all worlds. Reset to continue in that case.
    pub fn request(&mut self, line: &[u8]) -> Vec<u8> {
        let result = serde_json::from_slice(line)
            .map_err(|error| StepError::Rejected(error.to_string()))
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
        let run = self.current.as_mut().ok_or("Reset first")?;
        let need = run.worlds.len() * run.agents;
        if headings.len() != need || boosts.len() != need {
            return Err(format!(
                "One heading and one boost per agent of every world required: \
                 got {} headings and {} boosts, need {need}",
                headings.len(),
                boosts.len()
            )
            .into());
        }
        // Reject the whole step before advancing any world.
        for (index, (state, (headings, boosts))) in run
            .worlds
            .iter_mut()
            .zip(headings.chunks(run.agents).zip(boosts.chunks(run.agents)))
            .enumerate()
        {
            state.actions.clear();
            state
                .actions
                .extend(headings.iter().zip(boosts).map(|(&heading, &boost)| {
                    (!heading.is_nan()).then_some(Action { heading, boost })
                }));
            state
                .world
                .validate_actions(&state.actions)
                .map_err(|error| format!("World {index}: {error}"))?;
        }
        let decision_ticks = run.decision_ticks;
        let result = each_ok(each_world(&mut run.worlds, |state| {
            state.advance(decision_ticks)
        }));
        result.map(|_| ()).map_err(|error| {
            self.current = None;
            StepError::Ended(error + ENDED)
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
            } => {
                if worlds == 0 || agents == 0 || decision_ticks == 0 {
                    return Err("Need at least one world, agent and decision tick".into());
                }
                if agents > MAX_AGENTS {
                    return Err(format!(
                        "agents must be at most {MAX_AGENTS}: frame slot indices are u32 \
                         and u32::MAX means none"
                    )
                    .into());
                }
                if decision_ticks > MAX_DECISION_TICKS {
                    return Err(format!(
                        "decisionTicks must be at most {MAX_DECISION_TICKS}: frames report \
                         ticks as u32"
                    )
                    .into());
                }
                // The typed step and the C ABI index slots world-major.
                worlds
                    .checked_mul(agents)
                    .ok_or("worlds * agents must fit in usize")?;
                // Construct completely, with fresh frames, before replacing a
                // working session: it stays intact until the whole reply exists.
                let mut states = Vec::new();
                states
                    .try_reserve_exact(worlds)
                    .map_err(|_| format!("{worlds} worlds do not fit in memory"))?;
                for index in 0..worlds {
                    let mut world =
                        Simulator::new(seed.wrapping_add(index as u32), (*config).clone())?;
                    world.add_slots(agents)?;
                    states.push(WorldState {
                        world,
                        frame: Frame::default(),
                        actions: Vec::new(),
                    });
                }
                let frames = each_ok(each_world(&mut states, |state| {
                    state.frame.collect(&state.world, 0, &[])?;
                    Ok(state.frame.encode())
                }))?;
                let mut head = b"\"config\":".to_vec();
                head.extend_from_slice(
                    &crate::json::to_vec(&states[0].world.config).expect("configs serialize"),
                );
                head.extend_from_slice(
                    format!(",\"decisionTicks\":{decision_ticks},\"seed\":{seed},").as_bytes(),
                );
                let warnings = config.warnings();
                head.extend_from_slice(b"\"warnings\":");
                head.extend_from_slice(&crate::json::to_vec(&warnings).expect("strings serialize"));
                head.push(b',');
                let result = worlds_reply(&head, &frames)?;
                self.current = Some(Run {
                    worlds: states,
                    agents,
                    decision_ticks,
                });
                self.unprinted = warnings;
                Ok(result)
            }
            Request::Step { actions } => {
                let run = self.current.as_mut().ok_or("Reset first")?;
                if actions.len() != run.worlds.len() {
                    return Err(format!(
                        "One action entry per world required: got {}, need {}",
                        actions.len(),
                        run.worlds.len()
                    )
                    .into());
                }
                // Reject the whole request before advancing any world.
                for (index, (state, world_actions)) in
                    run.worlds.iter_mut().zip(actions).enumerate()
                {
                    if world_actions.heading.len() != run.agents
                        || world_actions.boost.len() != run.agents
                    {
                        return Err(format!(
                            "World {index}: one heading and one boost per agent required"
                        )
                        .into());
                    }
                    state.actions.clear();
                    state.actions.extend(
                        world_actions.heading.iter().zip(&world_actions.boost).map(
                            |(&heading, &boost)| heading.map(|heading| Action { heading, boost }),
                        ),
                    );
                    state
                        .world
                        .validate_actions(&state.actions)
                        .map_err(|error| format!("World {index}: {error}"))?;
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
                    StepError::Ended(error + ENDED)
                })
            }
            Request::Snapshot { world } => {
                let run = self.current.as_ref().ok_or("Reset first")?;
                let world = &run.worlds.get(world).ok_or("Unknown world")?.world;
                // Deliberately exclude hidden RNG/control state from this debug view.
                Ok(reply(&SnapshotReply {
                    config: &world.config,
                    time: world.time,
                    snakes: &world.snakes,
                    food: &world.food,
                    prey: &world.prey,
                    deaths: &world.deaths,
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
