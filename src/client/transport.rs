//! Trusted local JSON-lines control of one portable client backend.
//!
//! One controller owns one backend. Only `step` advances a simulation or reads a
//! network socket; `observe` repeats the current frame. Reconnection happens only
//! on an explicit `reconnect` request.
use crate::Action;
use crate::client::{
    Backend, CONTRACT, ClientFrame,
    network::{NetworkBackend, NetworkConfig},
    protocol::PROTOCOL_VERSION,
    simulation::{SimulationConfig, SimulatorBackend},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

pub const PROTOCOL: &str = "slither-client-json-v1";

fn default_seed() -> u32 {
    1
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
    Simulation {
        #[serde(default = "default_seed")]
        seed: u32,
        #[serde(default)]
        config: Box<SimulationConfig>,
    },
    Network {
        config: Box<NetworkConfig>,
    },
    Step {
        #[serde(default)]
        action: Option<Command>,
        #[serde(default)]
        processing_seconds: Option<f64>,
    },
    Observe {},
    Reconnect {},
    Wire {},
}

enum Active {
    Simulation(Box<SimulatorBackend>),
    Network(Box<NetworkBackend>),
}

impl Active {
    fn name(&self) -> &'static str {
        match self {
            Self::Simulation(_) => "simulation",
            Self::Network(_) => "network",
        }
    }

    fn backend(&mut self) -> &mut dyn Backend {
        match self {
            Self::Simulation(backend) => backend.as_mut(),
            Self::Network(backend) => backend.as_mut(),
        }
    }

    fn check_send(&self, action: Action) -> Result<(), String> {
        match self {
            Self::Simulation(backend) => {
                backend.check_send(backend.config().controlled_slot, action)
            }
            Self::Network(backend) => backend.check_send(action),
        }
    }

    /// Poll the current frame. This never advances time or reads a socket.
    fn frame(&mut self) -> Result<ClientFrame, String> {
        match self {
            Self::Simulation(backend) => backend.frame(),
            Self::Network(backend) => backend.frame(),
        }
    }

    fn reply(&mut self) -> Result<Value, String> {
        Ok(json!({"backend": self.name(), "frame": self.frame()?}))
    }
}

#[derive(Default)]
struct Session {
    active: Option<Active>,
}

impl Session {
    fn active(&mut self) -> Result<&mut Active, String> {
        self.active
            .as_mut()
            .ok_or_else(|| "Start a simulation or network backend first".into())
    }

    /// Replace the backend only after the new one is fully constructed.
    fn replace(&mut self, mut active: Active) -> Result<Value, String> {
        let result = active.reply()?;
        self.active = Some(active);
        Ok(result)
    }

    fn handle(&mut self, request: Request) -> Result<Value, String> {
        match request {
            Request::Info {} => Ok(json!({
                "version": env!("CARGO_PKG_VERSION"),
                "contract": CONTRACT,
                "wireProtocol": PROTOCOL_VERSION,
                "stability": "experimental",
                "backend": self.active.as_ref().map(Active::name),
                "defaultSeed": default_seed(),
                "defaultSimulationConfig": SimulationConfig::default(),
                "defaultNetworkConfig": NetworkConfig::default(),
            })),
            Request::Simulation { seed, config } => self.replace(Active::Simulation(Box::new(
                SimulatorBackend::new(seed, *config)?,
            ))),
            Request::Network { config } => {
                self.replace(Active::Network(Box::new(NetworkBackend::connect(*config)?)))
            }
            Request::Step {
                action,
                processing_seconds,
            } => {
                let active = self.active()?;
                // Validate the whole request before the backend sees any of it.
                if processing_seconds.is_some_and(|seconds| seconds < 0.) {
                    return Err("processingSeconds must be non-negative".into());
                }
                let action = action.map(|Command { heading, boost }| Action { heading, boost });
                // A rejected action must not record a processing duration or
                // leave a simulated decision pending for the next request.
                if let Some(action) = action {
                    active.check_send(action)?;
                }
                let backend = active.backend();
                if let Some(seconds) = processing_seconds {
                    backend.set_processing_seconds(seconds)?;
                }
                if let Some(action) = action {
                    backend.send_action(action)?;
                }
                let frame = backend.next_frame()?;
                Ok(json!({"backend": active.name(), "frame": frame}))
            }
            Request::Observe {} => self.active()?.reply(),
            Request::Reconnect {} => {
                let active = self.active()?;
                active.backend().reconnect()?;
                active.reply()
            }
            Request::Wire {} => {
                let records = match self.active()? {
                    Active::Simulation(backend) => {
                        backend.drain_records(backend.config().controlled_slot)?
                    }
                    Active::Network(backend) => backend.drain_records(),
                };
                Ok(json!({"records": records}))
            }
        }
    }
}

/// Serve a trusted local controller until EOF. Each input line, including malformed
/// or non-UTF-8 input, receives exactly one reply; only I/O failures stop the loop.
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
            Ok(result) => json!({"protocol": PROTOCOL, "result": result}),
            Err(error) => json!({"protocol": PROTOCOL, "error": error}),
        };
        serde_json::to_writer(&mut output, &reply)?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
}
