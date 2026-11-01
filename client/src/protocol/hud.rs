//! Borrowed projections of canonical bare leaderboard and announcement commands.
//!
//! These explicit decoders do not change generic framing or state admission.
//! Names and messages remain raw Latin-1 bytes; scoring, text policy and HUD
//! lifecycle belong to callers. No parser or iterator allocates or mutates input.
//! Commands are bounded by [`super::MAX_FRAME_BYTES`] (1 MiB). Leaderboard
//! validation is linear with constant auxiliary storage; each row consumes at
//! least seven bytes. Announcement validation and field access take constant
//! work. Views borrow immutable input and retain no owned packet queue.
//!
//! The v13 reader re-exports these identical HUD layouts. Their legacy field
//! meanings are protocol interpretations, as labelled in the catalog:
//! https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-protocol13-catalog/REPORT.md

use std::fmt;

/// Structural failures when decoding one bare HUD command.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HudPacketError {
    /// No opcode was supplied.
    EmptyCommand,
    /// The command exceeds the existing frame byte bound.
    CommandTooLarge,
    /// The opcode does not match the selected decoder.
    WrongOpcode,
    /// A required header, row prefix or declared name is incomplete.
    TruncatedPacket,
}

impl fmt::Display for HudPacketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyCommand => "HUD command has no opcode",
            Self::CommandTooLarge => "HUD command exceeds the frame byte bound",
            Self::WrongOpcode => "HUD command has the wrong opcode for this decoder",
            Self::TruncatedPacket => "HUD command has an incomplete header, row or name",
        })
    }
}

impl std::error::Error for HudPacketError {}

pub use slither_protocol::v19::hud::{Announcement, Leaderboard, LeaderboardRow, LeaderboardRows};

fn map_error(error: slither_protocol::DecodeError) -> HudPacketError {
    use slither_protocol::ErrorKind;
    match error.kind {
        ErrorKind::Empty => HudPacketError::EmptyCommand,
        ErrorKind::TooLarge => HudPacketError::CommandTooLarge,
        ErrorKind::WrongOpcode => HudPacketError::WrongOpcode,
        _ => HudPacketError::TruncatedPacket,
    }
}

/// Validate the complete bare leaderboard through the shared reader.
pub fn parse_leaderboard(command: &[u8]) -> Result<Leaderboard<'_>, HudPacketError> {
    slither_protocol::v19::hud::parse_leaderboard(command).map_err(map_error)
}

/// Validate a bare announcement through the shared reader.
pub fn parse_announcement(command: &[u8]) -> Result<Announcement<'_>, HudPacketError> {
    slither_protocol::v19::hud::parse_announcement(command).map_err(map_error)
}
