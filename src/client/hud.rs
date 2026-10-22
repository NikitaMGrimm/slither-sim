//! Received HUD reports: leaderboard, longest record, session identifiers, team
//! scores, snake profiles and kill counters.
//!
//! Layouts follow the stock client's `l`, `m`, `S`, `o`, `k` and full `s`
//! handlers. Every value is a transmitted server report; none is reconstructed
//! from simulator state. Ages are local receipt ages and advance when polled.
use serde::{Deserialize, Serialize};

/// Most recent `s` removals retained, newest first.
pub const MAX_REMOVALS: usize = 64;
/// Bound on retained team-score history entries.
pub const MAX_TEAM_HISTORY: usize = 4096;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientHud {
    pub leaderboard: Option<Leaderboard>,
    pub record: Option<DailyRecord>,
    pub session: Option<SessionIdentifiers>,
    pub team_scores: Option<TeamScores>,
    /// Profiles of currently known snakes, in wire-id order.
    pub snakes: Vec<SnakeProfile>,
    /// Recent removals of known snakes, newest first.
    pub removals: Vec<SnakeRemoval>,
    /// Command byte of the last applied minimap packet: `M`, `V`, `U`, `u` or `L`.
    pub minimap_command: Option<String>,
    /// Per-team layers of the last `L` snapshot. The latest minimap starts from
    /// their union; later V packets can update it without identifying a team.
    pub minimap_teams: Option<MinimapTeams>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Leaderboard {
    /// One-based row of the own snake; `None` when the server sent 0.
    pub own_position: Option<u8>,
    pub rank: u16,
    pub player_count: u16,
    pub entries: Vec<LeaderboardEntry>,
    pub age_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LeaderboardEntry {
    pub segments: u16,
    pub fam: f64,
    /// Stock display score from the setup tables; `None` outside those tables.
    pub score: Option<f64>,
    /// Raw color byte; the stock client uses it modulo 9.
    pub color: u8,
    /// Wire bytes decoded one byte per character (Latin-1), as the stock client does.
    pub name: String,
    pub own: bool,
}

/// The `m` record message, labelled "today's longest" by the stock browser.
/// The local host scopes its record to one world run.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DailyRecord {
    pub segments: u32,
    pub fam: f64,
    pub score: Option<f64>,
    pub name: String,
    pub message: String,
    pub age_seconds: f64,
}

/// The optional `S` response. An opaque identifier, not authentication or resume state.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionIdentifiers {
    pub snake_id: String,
    pub session_id: u32,
    pub age_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TeamScores {
    pub team1: i32,
    pub team2: i32,
    /// Stock ring-buffer contents as `[team1, team2]`, newest first.
    pub history: Vec<[i32; 2]>,
    pub age_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnakeProfile {
    pub snake_id: String,
    pub name: String,
    pub skin: u8,
    pub custom_skin: Vec<u8>,
    pub cosmetic: u8,
    /// Last `k` report for this appearance; `None` means unknown, not zero.
    pub kills: Option<KillReport>,
    /// Age of the appearance that carried this profile.
    pub age_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KillReport {
    pub count: u32,
    pub age_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnakeRemoval {
    pub snake_id: String,
    /// The stock client treats byte 3 equal to 1 as a kill; `None` when absent.
    pub killed: Option<bool>,
    pub age_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MinimapTeams {
    pub side: usize,
    /// One binary layer per team, in wire order, each `side * side` cells.
    pub layers: Vec<Vec<u8>>,
    pub age_seconds: f64,
}

/// Stock `o` handling: a 9-byte packet writes one pair into the ring after the
/// current position; a longer one prepends pairs and makes the last element current.
/// Stored in the stock array order so later updates match its ring exactly.
#[derive(Clone, Debug, Default)]
pub(crate) struct TeamRing {
    pairs: Vec<[i32; 2]>,
    pos: usize,
}
impl TeamRing {
    pub(crate) fn update(&mut self, pair: [i32; 2]) {
        self.pos += 1;
        if self.pos >= self.pairs.len() {
            self.pos = 0;
        }
        if self.pairs.is_empty() {
            self.pairs.push(pair);
        } else {
            self.pairs[self.pos] = pair;
        }
    }
    pub(crate) fn bulk(&mut self, pairs: &[[i32; 2]]) {
        let mut next: Vec<[i32; 2]> = pairs.iter().rev().copied().collect();
        next.append(&mut self.pairs);
        self.pairs = next;
        self.pos = self.pairs.len() - 1;
        if self.pairs.len() > MAX_TEAM_HISTORY {
            // Keep the newest entries; the order beyond the bound is not retained.
            let mut newest = self.newest_first();
            newest.truncate(MAX_TEAM_HISTORY);
            newest.reverse();
            self.pairs = newest;
            self.pos = self.pairs.len() - 1;
        }
    }
    pub(crate) fn newest_first(&self) -> Vec<[i32; 2]> {
        let n = self.pairs.len();
        (0..n).map(|i| self.pairs[(self.pos + n - i) % n]).collect()
    }
}

/// One byte per character, matching `String.fromCharCode` over wire bytes.
pub(crate) fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}
