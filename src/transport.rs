//! Local process transport. Each connection controls a session of independent worlds.
use crate::{Action, Config, DeathEvent, Food, Prey, Simulator, Snake};
use frame::FrameBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, BufRead, Write};

mod frame;

pub const PROTOCOL: &str = "slither-sim-json-v2";

const MAX_AGENTS: usize = 128;
const MAX_WORLDS: usize = 1024;
const MAX_DECISION_TICKS: usize = 1000;

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
fn worlds_reply(head: &[u8], frames: &[Vec<u8>]) -> Vec<u8> {
    let length = frames.iter().map(|frame| frame.len() + 1).sum::<usize>() + head.len() + 64;
    let mut out = Vec::with_capacity(length);
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
    out
}

struct WorldState {
    world: Simulator,
    frames: FrameBuilder,
    actions: Vec<Option<Action>>,
}

struct Run {
    worlds: Vec<WorldState>,
    agents: usize,
    decision_ticks: usize,
}

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

#[derive(Default)]
struct Session {
    current: Option<Run>,
}

impl Session {
    fn handle(&mut self, request: Request) -> Result<Vec<u8>, String> {
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
                if !(1..=MAX_WORLDS).contains(&worlds)
                    || !(1..=MAX_AGENTS).contains(&agents)
                    || !(1..=MAX_DECISION_TICKS).contains(&decision_ticks)
                {
                    return Err(format!(
                        "Need 1..{MAX_WORLDS} worlds, 1..{MAX_AGENTS} agents and \
                         1..{MAX_DECISION_TICKS} decisionTicks"
                    ));
                }
                // Construct completely before replacing a working session.
                let mut built = Vec::with_capacity(worlds);
                for index in 0..worlds {
                    let mut world =
                        Simulator::new(seed.wrapping_add(index as u32), (*config).clone())?;
                    for slot in 0..agents {
                        world.add(slot.to_string(), slot, agents)?;
                    }
                    built.push(world);
                }
                // Earlier frame builders keep their buffers for the new worlds.
                let mut builders = self
                    .current
                    .take()
                    .map(|run| run.worlds.into_iter().map(|state| state.frames))
                    .into_iter()
                    .flatten();
                let mut states: Vec<_> = built
                    .into_iter()
                    .map(|world| WorldState {
                        world,
                        frames: builders.next().unwrap_or_default(),
                        actions: Vec::new(),
                    })
                    .collect();
                let frames = each_world(&mut states, |state| {
                    state.frames.encode(&state.world, 0, &[])
                });
                let mut head = b"\"config\":".to_vec();
                head.extend_from_slice(
                    &crate::json::to_vec(&states[0].world.config).expect("configs serialize"),
                );
                head.extend_from_slice(
                    format!(",\"decisionTicks\":{decision_ticks},\"seed\":{seed},").as_bytes(),
                );
                let result = worlds_reply(&head, &frames);
                self.current = Some(Run {
                    worlds: states,
                    agents,
                    decision_ticks,
                });
                Ok(result)
            }
            Request::Step { actions } => {
                let run = self.current.as_mut().ok_or("Reset first")?;
                if actions.len() != run.worlds.len() {
                    return Err(format!(
                        "One action entry per world required: got {}, need {}",
                        actions.len(),
                        run.worlds.len()
                    ));
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
                        ));
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
                let frames = each_world(&mut run.worlds, |state| {
                    let previous_deaths = state.world.deaths.len();
                    let mut ticks = 0;
                    for _ in 0..decision_ticks {
                        if state.world.living() == 0 {
                            break;
                        }
                        state.world.advance_validated(&state.actions);
                        ticks += 1;
                    }
                    state
                        .frames
                        .encode(&state.world, ticks, &state.world.deaths[previous_deaths..])
                });
                Ok(worlds_reply(b"", &frames))
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
        let result = serde_json::from_slice(request)
            .map_err(|error| error.to_string())
            .and_then(|request| session.handle(request));
        let reply = match result {
            Ok(reply) => reply,
            Err(error) => crate::json::error_reply(PROTOCOL, &error),
        };
        output.write_all(&reply)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
}
