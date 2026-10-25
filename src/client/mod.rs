//! Portable bot inputs reconstructed from the messages delivered to one client.
//!
//! The simulator and network backends share this contract. Authoritative snapshots
//! and configured one-way latency are deliberately outside it.
use crate::{Action, Observation};
use serde::{Deserialize, Serialize};
use std::time::Instant;

pub mod challenge;
mod encode;
mod fast_hash;
pub mod hud;
pub mod latency;
pub mod minimap;
pub mod network;
mod presentation;
pub mod protocol;
pub mod received;
pub mod server;
pub mod simulation;
pub mod transport;
mod visibility;

pub use hud::{
    ClientHud, DailyRecord, KillReport, Leaderboard, LeaderboardEntry, MinimapTeams,
    SessionIdentifiers, SnakeProfile, SnakeRemoval, TeamScores,
};
pub use latency::{Congestion, LatencyConfig};
pub use minimap::{MAX_FRAME_MINIMAP_CELLS, MAX_TEAM_MINIMAP_CELLS, Minimap};
pub use network::{NetworkBackend, NetworkConfig};
pub use protocol::ClientState;
pub use received::{
    CategoryKnowledge, ReceivedField, ReceivedHistory, ReceivedMetadata, ReceivedSnake,
    ReceivedTracker,
};
pub use server::{Host, HostConfig};
pub use simulation::{SimulationConfig, SimulatorBackend};

pub const CONTRACT: &str = "slither-client-v2";

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ClientStatus {
    #[default]
    Waiting,
    Playing,
    Dead,
    Disconnected,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SentCommand {
    pub seconds_ago: f64,
    pub action: Action,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientTiming {
    pub ping_seconds: Option<f64>,
    pub ping_range_seconds: Option<[f64; 2]>,
    pub ping_wait_seconds: Option<f64>,
    pub processing_seconds: f64,
    pub sent_commands: Vec<SentCommand>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ServerSettings {
    pub protocol: u8,
    pub grid_center: u32,
    pub sector_size: u16,
    pub max_segments: u16,
    pub body_spacing: f64,
    pub body_contraction: f64,
    /// Setup client constants under their stock names; not simulator physics.
    pub sector_count_along_edge: u16,
    pub spangdv: f64,
    pub nsp1: f64,
    pub nsp2: f64,
    pub nsp3: f64,
    pub mamu: f64,
    pub mamu2: f64,
    /// Stock `real_sid`, shown as "server N"; never a snake identity.
    pub server_number: Option<u16>,
    pub game_mode: Option<u8>,
    /// Stock `team_val`, transmitted only when `game_mode` is 2 (team mode).
    pub team_val: Option<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientFrame {
    pub contract: String,
    /// Local monotonic seconds; independent of the server's elapsed-ms clock.
    pub time: f64,
    pub status: ClientStatus,
    pub life: u64,
    pub settings: Option<ServerSettings>,
    pub observation: Option<Observation>,
    pub received: ReceivedMetadata,
    pub minimap: Option<Minimap>,
    pub timing: ClientTiming,
    #[serde(default)]
    pub hud: ClientHud,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WireEvent {
    Connect,
    Receive,
    Send,
    Disconnect,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct WireRecord {
    pub time: f64,
    pub event: WireEvent,
    pub bytes: Vec<u8>,
}

/// Apply the same ordered input used by either backend, without a network socket.
pub fn replay_record(state: &mut ClientState, record: &WireRecord) -> Result<(), String> {
    match record.event {
        WireEvent::Connect => state.new_connection(record.time),
        WireEvent::Receive => state.receive_frame(&record.bytes, record.time),
        WireEvent::Send => state.record_sent(&record.bytes, record.time),
        WireEvent::Disconnect => {
            state.disconnect();
            Ok(())
        }
    }
}

/// Real-time and virtual-time backends implement this interface.
pub trait Backend {
    fn next_frame(&mut self) -> Result<ClientFrame, String>;
    fn send_action(&mut self, action: Action) -> Result<(), String>;
    fn reconnect(&mut self) -> Result<(), String>;
    fn set_processing_seconds(&mut self, seconds: f64) -> Result<(), String>;
}

pub trait Bot {
    fn decide(&mut self, frame: &ClientFrame) -> Option<Action>;
    fn reset(&mut self) {}
}

/// Stateless portable adapter for [`crate::reference::decide`].
#[derive(Clone, Copy, Debug, Default)]
pub struct ReferenceBot;

impl Bot for ReferenceBot {
    fn decide(&mut self, frame: &ClientFrame) -> Option<Action> {
        if frame.contract != CONTRACT || frame.status != ClientStatus::Playing {
            return None;
        }
        frame.observation.as_ref().map(crate::reference::decide)
    }
}

/// Portable adapter for [`crate::forager::decide`].
///
/// It derives the forecast lead from the frame's own timing. The observation
/// is as old as the latest received message (the smallest valid receipt age of
/// any snake position/heading, body or food report) plus the downlink share of
/// the measured round trip. A command sent now acts after the uplink share plus
/// the previous decision's processing time. The recent ping range is the
/// lead's uncertainty. Each opponent's latest reported wanted heading and
/// heading-receipt age come from its `received.snakes` row. Only received
/// fields are read; positions between reports are already extrapolated by the
/// decoder. The bot keeps reusable buffers but no state between decisions, so
/// `reset` has nothing to clear.
#[derive(Clone, Debug)]
pub struct ForagerBot {
    pub params: crate::forager::Params,
    scratch: crate::forager::Scratch,
    intents: Vec<crate::forager::Intent>,
}

impl Default for ForagerBot {
    fn default() -> Self {
        Self {
            params: Default::default(),
            scratch: Default::default(),
            intents: Vec::with_capacity(4096),
        }
    }
}

impl ForagerBot {
    pub fn new(params: crate::forager::Params) -> Result<Self, String> {
        params.validate()?;
        Ok(Self {
            params,
            ..Default::default()
        })
    }
    /// The kernel timing this adapter derives from a frame.
    pub fn timing(&self, frame: &ClientFrame) -> crate::forager::Timing {
        let timing = &frame.timing;
        let ping = timing.ping_seconds.unwrap_or(0.0);
        let uplink = self.params.uplink_share;
        let received = &frame.received;
        let age = |field: ReceivedField| field.valid.then_some(field.age_seconds);
        let receipt = received
            .snakes
            .iter()
            .flat_map(|row| [age(row.position), age(row.heading)])
            .chain([
                received.bodies.valid.then_some(received.bodies.age_seconds),
                received.food.valid.then_some(received.food.age_seconds),
            ])
            .flatten()
            .fold(f64::INFINITY, f64::min);
        let receipt = if receipt.is_finite() { receipt } else { 0.0 };
        let mut result = crate::forager::Timing {
            observation_age: ping * (1.0 - uplink) + receipt,
            command_delay: ping * uplink,
            processing: timing.processing_seconds,
            uncertainty: timing
                .ping_range_seconds
                .map_or(0.0, |[low, high]| high - low),
            ..Default::default()
        };
        for command in &timing.sent_commands {
            result.push_sent(command.seconds_ago, command.action);
        }
        result
    }
}

impl Bot for ForagerBot {
    fn decide(&mut self, frame: &ClientFrame) -> Option<Action> {
        if frame.contract != CONTRACT || frame.status != ClientStatus::Playing {
            return None;
        }
        let observation = frame.observation.as_ref()?;
        let timing = self.timing(frame);
        fill_intents(&mut self.intents, observation, &frame.received);
        Some(crate::forager::decide(
            observation,
            &timing,
            &self.intents,
            &self.params,
            &mut self.scratch,
        ))
    }
}

/// Per-opponent received knowledge in observation order. Opponents and
/// receipt rows both follow the decoder's slot order, so the scan resumes after
/// the previous match and usually finds the next row at once.
fn fill_intents(
    intents: &mut Vec<crate::forager::Intent>,
    observation: &crate::Observation,
    received: &ReceivedMetadata,
) {
    intents.clear();
    let rows = &received.snakes;
    let mut next = 0;
    for opponent in &observation.opponents {
        let mut intent = crate::forager::Intent::default();
        let found = (0..rows.len())
            .map(|i| (next + i) % rows.len())
            .find(|&i| rows[i].snake_id == opponent.id);
        if let Some(i) = found {
            next = i + 1;
            let row = &rows[i];
            if let Some(wanted) = row.wanted_history.first() {
                // Motion reports round the target to 256 steps; the server
                // steers to the client's 251-step command, so take the nearest.
                let step = std::f64::consts::TAU / 251.0;
                intent.wanted =
                    (wanted.value.rem_euclid(std::f64::consts::TAU) / step).round() % 251.0 * step;
            }
            if row.heading.valid {
                intent.heading_age = row.heading.age_seconds;
            }
        }
        intents.push(intent);
    }
}

pub struct BotRunner<B> {
    backend: B,
    identity: Option<(u64, u64)>,
    status: ClientStatus,
}

impl<B: Backend> BotRunner<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            identity: None,
            status: ClientStatus::Waiting,
        }
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn step(&mut self, bot: &mut impl Bot) -> Result<ClientFrame, String> {
        let frame = self.backend.next_frame()?;
        if frame.contract != CONTRACT {
            return Err("Incompatible bot observation contract".into());
        }
        let identity = (frame.received.connection_generation, frame.life);
        if self.identity != Some(identity)
            || (self.status == ClientStatus::Playing && frame.status != ClientStatus::Playing)
        {
            bot.reset();
        }
        self.identity = Some(identity);
        self.status = frame.status;
        if frame.status == ClientStatus::Playing {
            let started = Instant::now();
            let action = bot.decide(&frame);
            if action.is_some_and(|action| !action.heading.is_finite()) {
                return Err("Action heading must be finite".into());
            }
            self.backend
                .set_processing_seconds(started.elapsed().as_secs_f64())?;
            if let Some(action) = action {
                self.backend.send_action(action)?;
            }
        }
        Ok(frame)
    }
}
