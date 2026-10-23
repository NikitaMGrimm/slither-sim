//! Local process transport. Each connection controls one independent world.
use crate::{Action, Config, DeathEvent, Food, Observation, Prey, Simulator, Snake};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::{self, BufRead, Write};

pub const PROTOCOL: &str = "slither-sim-json-v1";

fn default_seed() -> u32 {
    1
}
fn default_agents() -> usize {
    4
}
fn default_decision_ticks() -> usize {
    6
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command {
    heading: f64,
    boost: bool,
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
        #[serde(default = "default_agents")]
        agents: usize,
        #[serde(default = "default_decision_ticks")]
        decision_ticks: usize,
        #[serde(default)]
        config: Box<Config>,
    },
    Step {
        actions: Vec<Option<Command>>,
    },
    Observe {
        slot: usize,
    },
    Snapshot {},
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ResetReply<'a> {
    seed: u32,
    config: &'a Config,
    decision_ticks: usize,
    observations: Vec<Option<Observation>>,
    time: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StepReply<'a> {
    observations: Vec<Option<Observation>>,
    deaths: &'a [DeathEvent],
    time: f64,
    elapsed_seconds: f64,
    ticks: usize,
    living: usize,
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
    #[derive(Serialize)]
    struct Reply<'a, T> {
        protocol: &'a str,
        result: T,
    }
    crate::json::to_vec(&Reply {
        protocol: PROTOCOL,
        result,
    })
    .expect("transport replies serialize")
}

struct WorldSession {
    world: Simulator,
    decision_ticks: usize,
}

#[derive(Default)]
struct Session {
    current: Option<WorldSession>,
}

impl Session {
    fn handle(&mut self, request: Request) -> Result<Vec<u8>, String> {
        match request {
            Request::Info {} => Ok(reply(&json!({
                "version": env!("CARGO_PKG_VERSION"),
                "defaultConfig": Config::default(),
                "defaultDecisionTicks": default_decision_ticks(),
                "maxAgents": 128,
                "maxDecisionTicks": 1000,
            }))),
            Request::Reset {
                seed,
                agents,
                decision_ticks,
                config,
            } => {
                if !(1..=128).contains(&agents) || !(1..=1000).contains(&decision_ticks) {
                    return Err("Need 1..128 agents and 1..1000 decisionTicks".into());
                }
                // Construct completely before replacing a working session.
                let mut world = Simulator::new(seed, *config)?;
                for slot in 0..agents {
                    world.add(slot.to_string(), slot, agents)?;
                }
                let result = reply(&ResetReply {
                    seed,
                    config: &world.config,
                    decision_ticks,
                    observations: world.observe_all(),
                    time: world.time,
                });
                self.current = Some(WorldSession {
                    world,
                    decision_ticks,
                });
                Ok(result)
            }
            Request::Step { actions } => {
                let session = self.current.as_mut().ok_or("Reset first")?;
                let actions: Vec<_> = actions
                    .into_iter()
                    .map(|command| {
                        command.map(|command| Action {
                            heading: command.heading,
                            boost: command.boost,
                        })
                    })
                    .collect();
                // Reject the whole batch before advancing any physics.
                session.world.validate_actions(&actions)?;
                let start = session.world.time;
                let previous_deaths = session.world.deaths.len();
                let mut ticks = 0;
                for _ in 0..session.decision_ticks {
                    if session.world.living() == 0 {
                        break;
                    }
                    session.world.advance_validated(&actions);
                    ticks += 1;
                }
                Ok(reply(&StepReply {
                    observations: session.world.observe_all(),
                    deaths: &session.world.deaths[previous_deaths..],
                    time: session.world.time,
                    elapsed_seconds: session.world.time - start,
                    ticks,
                    living: session.world.living(),
                }))
            }
            Request::Observe { slot } => {
                let world = &self.current.as_ref().ok_or("Reset first")?.world;
                let snake = world.snakes.get(slot).ok_or("Unknown agent slot")?;
                let observation = if snake.alive {
                    Some(world.observe(slot)?)
                } else {
                    None
                };
                Ok(reply(&json!({ "observation": observation })))
            }
            Request::Snapshot {} => {
                let world = &self.current.as_ref().ok_or("Reset first")?.world;
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

/// Serve a trusted local controller until EOF. Request errors leave the stream usable.
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
            Err(error) => crate::json::to_vec(&json!({"protocol": PROTOCOL, "error": error}))
                .expect("transport errors serialize"),
        };
        output.write_all(&reply)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
}
