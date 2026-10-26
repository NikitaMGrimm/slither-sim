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
mod forager;
pub use forager::ForagerBot;
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
pub use latency::{LatencyConfig, Pauses, Variation};
pub use minimap::{MAX_FRAME_MINIMAP_CELLS, MAX_TEAM_MINIMAP_CELLS, Minimap};
pub use network::{NetworkBackend, NetworkConfig};
pub use protocol::ClientState;
pub use received::{
    CategoryKnowledge, ReceivedField, ReceivedHistory, ReceivedMetadata, ReceivedSnake,
    ReceivedTracker,
};
pub use server::{Host, HostConfig, RunningHost};
pub use simulation::{DeliveryProfile, SimulationConfig, SimulatorBackend};

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
