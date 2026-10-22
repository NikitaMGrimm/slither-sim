//! Protocol-19 decoding into the client contract, shared by every backend.
//!
//! Wire layouts follow the stock client parser and `tools/fidelity/slither_protocol.py`.
//! Coordinates are centered by subtracting the setup grid center. Only transmitted
//! state is kept: kill counts come only from `k` reports, private score credits are
//! never reconstructed, and unknown values stay unknown rather than being filled
//! from simulator state. Admin (`i`) and testing-only commands are ignored.
use super::{
    CONTRACT, CategoryKnowledge, ClientFrame, ClientStatus, ClientTiming, ReceivedField,
    ReceivedMetadata, ReceivedTracker, SentCommand, ServerSettings,
    hud::{
        ClientHud, DailyRecord, KillReport, Leaderboard, LeaderboardEntry, MAX_REMOVALS,
        MinimapTeams, SessionIdentifiers, SnakeProfile, SnakeRemoval, TeamRing, TeamScores, latin1,
    },
    minimap::{Minimap, MinimapDecoder},
};
use crate::{
    Action, Arena, Food, Observation, Point, Snake,
    physics::{self, SPEED_UNIT_SECONDS},
    prey::Prey,
};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::f64::consts::TAU;

pub const PROTOCOL_VERSION: u8 = 19;
const MAX_SENT_COMMANDS: usize = 64;
const PING_RANGE_SECONDS: f64 = 5.;
/// The stock client sends a game ping only after more than 250 ms.
const PING_INTERVAL_SECONDS: f64 = 0.25;
const MAX_PING_SECONDS: f64 = 2.;
/// Bound on motion reports retained between anchors for head extrapolation.
const MAX_MOTION_STEPS: usize = 256;
/// The stock `setMscps` extends its score tables by this many repeated entries.
const SCORE_TABLE_EXTENSION: usize = 2048;

fn truncated(p: &[u8]) -> String {
    format!(
        "Truncated '{}' packet of {} bytes",
        char::from(p[0]),
        p.len()
    )
}
fn malformed(p: &[u8]) -> String {
    format!(
        "Malformed '{}' packet of {} bytes",
        char::from(p[0]),
        p.len()
    )
}
fn byte(p: &[u8], i: usize) -> Result<u8, String> {
    p.get(i).copied().ok_or_else(|| truncated(p))
}
fn u16_at(p: &[u8], i: usize) -> Result<u16, String> {
    Ok(u16::from(byte(p, i)?) << 8 | u16::from(byte(p, i + 1)?))
}
fn u24_at(p: &[u8], i: usize) -> Result<u32, String> {
    Ok(u32::from(byte(p, i)?) << 16 | u32::from(byte(p, i + 1)?) << 8 | u32::from(byte(p, i + 2)?))
}
fn angle24(p: &[u8], i: usize) -> Result<f64, String> {
    Ok(f64::from(u24_at(p, i)?) * TAU / 16_777_215.)
}
fn angle8(p: &[u8], i: usize) -> Result<f64, String> {
    Ok(f64::from(byte(p, i)?) * TAU / 256.)
}
fn speed8(p: &[u8], i: usize) -> Result<f64, String> {
    Ok(f64::from(byte(p, i)?) / 18. / SPEED_UNIT_SECONDS)
}
fn fraction24(p: &[u8], i: usize) -> Result<f64, String> {
    Ok(f64::from(u24_at(p, i)?) / 16_777_215.)
}
/// Big-endian 32 bits read as the stock client's signed `<<24` expression.
fn i32_at(p: &[u8], i: usize) -> Result<i32, String> {
    let bytes = p.get(i..i + 4).ok_or_else(|| truncated(p))?;
    Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}
/// A length-prefixed byte string at `i`; returns the bytes and the next offset.
fn short_bytes(p: &[u8], i: usize) -> Result<(&[u8], usize), String> {
    let end = i + 1 + usize::from(byte(p, i)?);
    Ok((p.get(i + 1..end).ok_or_else(|| truncated(p))?, end))
}

/// Split one elapsed-ms framed message into command packets.
pub fn split_frame(bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    if bytes.len() < 3 {
        return Err("Short timing frame".into());
    }
    split_payload(&bytes[2..])
}

/// Both timed and header-free connections use the same packet bundling.
pub(crate) fn split_payload(bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    let Some(&first) = bytes.first() else {
        return Err("Empty packet payload".into());
    };
    if first >= 32 {
        return Ok(vec![bytes]);
    }
    let mut m = 0;
    let mut packets = Vec::new();
    while m < bytes.len() {
        let length = if bytes[m] < 32 {
            if m + 1 >= bytes.len() {
                return Err("Truncated packet length".into());
            }
            let length = usize::from(bytes[m]) << 8 | usize::from(bytes[m + 1]);
            m += 2;
            length
        } else {
            m += 1;
            usize::from(bytes[m - 1] - 32)
        };
        if length == 0 || length > bytes.len() - m {
            return Err("Invalid packet length".into());
        }
        packets.push(&bytes[m..m + length]);
        m += length;
    }
    Ok(packets)
}

/// Encode one decision as the bytes a protocol-19 client sends: a heading byte on
/// the 251-step grid, then `[253]`/`[254]` only when boost differs from the last
/// boost actually sent.
pub fn action_packets(
    action: Action,
    previous_boost: Option<bool>,
) -> Result<Vec<Vec<u8>>, String> {
    if !action.heading.is_finite() {
        return Err("Action heading must be finite".into());
    }
    let heading = (action.heading.rem_euclid(TAU) * 251. / TAU)
        .floor()
        .min(250.) as u8;
    let mut packets = vec![vec![heading]];
    if previous_boost != Some(action.boost) {
        packets.push(vec![if action.boost { 253 } else { 254 }]);
    }
    Ok(packets)
}

#[derive(Clone, Copy, Debug)]
enum Target {
    Own,
    Id(u16),
}

#[derive(Clone, Debug)]
struct Setup {
    settings: ServerSettings,
    radius: f64,
    /// Stock-client `(fpsls, fmlts)` score tables for the advertised `mscps`.
    scores: Vec<(f64, f64)>,
}
impl Setup {
    fn score(&self, segments: usize, fam: f64) -> f64 {
        let (sum, weight) = self.scores[segments.min(self.scores.len() - 1)];
        ((sum + fam / weight - 1.) * 15. - 5.).floor()
    }
    /// Score of a reported mass, `None` beyond the stock client's extended tables.
    fn reported_score(&self, segments: usize, fam: f64) -> Option<f64> {
        let score = self.score(segments, fam);
        (segments < self.scores.len() + SCORE_TABLE_EXTENSION && score.is_finite()).then_some(score)
    }
    fn center(&self, x: f64, y: f64) -> Point {
        let grid = f64::from(self.settings.grid_center);
        Point {
            x: x - grid,
            y: y - grid,
        }
    }
}
fn score_table(mscps: u16) -> Vec<(f64, f64)> {
    let max = usize::from(mscps);
    let mut table: Vec<(f64, f64)> = Vec::with_capacity(max + 1);
    for i in 0..=max {
        let weight = if i >= max {
            table[i - 1].1
        } else {
            (1. - i as f64 / max as f64).powf(2.25)
        };
        let sum = if i == 0 {
            0.
        } else {
            table[i - 1].0 + 1. / table[i - 1].1
        };
        table.push((sum, weight));
    }
    table
}

/// Transmitted cosmetic fields of a full appearance.
#[derive(Clone, Debug)]
struct Profile {
    name: String,
    skin: u8,
    custom_skin: Vec<u8>,
    cosmetic: u8,
}

#[derive(Clone, Debug)]
struct Appearance {
    id: u16,
    profile: Profile,
    heading: f64,
    wanted: f64,
    speed: f64,
    fam: f64,
    head: (f64, f64),
    first: Option<(f64, f64)>,
    deltas: Vec<(f64, f64)>,
    /// Final point encoded as a 16-bit direction from its predecessor.
    iang: Option<u16>,
}

#[derive(Clone, Debug)]
enum Packet<'a> {
    Setup(Setup),
    Appear(Appearance),
    Remove(u16, Option<bool>),
    Anchor {
        target: Target,
        iang: Option<u16>,
        absolute: Option<(u16, u16)>,
        /// Present exactly for growing anchors.
        fam: Option<f64>,
    },
    Motion {
        target: Target,
        heading: Option<f64>,
        wanted: Option<f64>,
        speed: Option<f64>,
    },
    Fam(u16, f64),
    Tail(u16, Option<f64>),
    Credit(u8),
    Sector(u8, u8, Vec<(u8, u8, f64)>),
    Pellet(Option<(u8, u8)>, u8, u8, f64),
    Eaten(Option<(u8, u8)>, u8, u8),
    ClearSector(u8, u8),
    PreyAppear(u16, (f64, f64), f64, f64, f64, f64),
    PreyRemove(u16),
    PreyMove {
        id: u16,
        position: (f64, f64),
        heading: Option<f64>,
        wanted: Option<f64>,
        speed: Option<f64>,
    },
    Minimap(&'a [u8]),
    Leaderboard {
        own_position: u8,
        rank: u16,
        player_count: u16,
        /// `(segments, fam, color, name)` rows.
        rows: Vec<(u16, f64, u8, String)>,
    },
    Record {
        segments: u32,
        fam: f64,
        name: String,
        message: String,
    },
    Session(u16, u32),
    TeamScore([i32; 2]),
    TeamHistory(Vec<[i32; 2]>),
    Kills(u16, u32),
    Pong,
    GameOver,
    Radius(f64),
    Ignored,
}
impl Packet<'_> {
    /// Packets that depend on setup data, or on entities only setup admits.
    /// Session and team reports have no such dependency; a kill report for an
    /// unknown snake is ignored, as by the stock client.
    fn needs_setup(&self) -> bool {
        !matches!(
            self,
            Packet::Setup(_)
                | Packet::Pong
                | Packet::GameOver
                | Packet::Session(..)
                | Packet::TeamScore(_)
                | Packet::TeamHistory(_)
                | Packet::Kills(..)
                | Packet::Ignored
        )
    }
}

fn parse(p: &[u8]) -> Result<Packet<'_>, String> {
    let Some(&code) = p.first() else {
        return Err("Empty packet".into());
    };
    let n = p.len();
    Ok(match code {
        b'a' => {
            if n < 25 {
                return Err("Short setup packet".into());
            }
            if p[23] != PROTOCOL_VERSION {
                return Err(format!(
                    "Unsupported protocol version {}; expected {PROTOCOL_VERSION}",
                    p[23]
                ));
            }
            // Bytes 25..27 are the stock client's `real_sid` server number, shown
            // as "server N" on the minimap. They never identify the own snake.
            if matches!(n, 26 | 28 | 29) {
                return Err(truncated(p));
            }
            let grid_center = u24_at(p, 1)?;
            let max_segments = u16_at(p, 4)?;
            let sector_size = u16_at(p, 6)?;
            let body_contraction = f64::from(u16_at(p, 21)?) / 1000.;
            let body_spacing = f64::from(p[24]);
            if !(0. ..=1.).contains(&body_contraction) {
                return Err("Unsupported body contraction; expected 0 through 1".into());
            }
            if grid_center == 0 || max_segments == 0 || sector_size == 0 || p[24] == 0 {
                return Err("Invalid setup values".into());
            }
            let radius = if n >= 30 {
                f64::from(u24_at(p, 27)?)
            } else {
                f64::from(grid_center) * 0.98
            };
            // Stock `game_mode` 2 is team mode; only then is the next byte `team_val`.
            let game_mode = p.get(30).copied();
            let hundredths = |i| -> Result<f64, String> { Ok(f64::from(u16_at(p, i)?) / 100.) };
            let thousandths = |i| -> Result<f64, String> { Ok(f64::from(u16_at(p, i)?) / 1000.) };
            Packet::Setup(Setup {
                settings: ServerSettings {
                    protocol: PROTOCOL_VERSION,
                    grid_center,
                    sector_size,
                    max_segments,
                    body_spacing,
                    body_contraction,
                    sector_count_along_edge: u16_at(p, 8)?,
                    spangdv: f64::from(p[10]) / 10.,
                    nsp1: hundredths(11)?,
                    nsp2: hundredths(13)?,
                    nsp3: hundredths(15)?,
                    mamu: thousandths(17)?,
                    mamu2: thousandths(19)?,
                    server_number: if n >= 27 { Some(u16_at(p, 25)?) } else { None },
                    game_mode,
                    team_val: if game_mode == Some(2) {
                        p.get(31).copied()
                    } else {
                        None
                    },
                },
                radius,
                scores: score_table(max_segments),
            })
        }
        b's' if n - 1 <= 6 => {
            // Short `s` removes a snake; the stock client treats byte 3 == 1 as a kill.
            Packet::Remove(u16_at(p, 1)?, p.get(3).map(|&b| b == 1))
        }
        b's' => {
            let skin = byte(p, 15)?;
            let (name, m) = short_bytes(p, 22)?;
            let (custom_skin, m) = short_bytes(p, m)?;
            let profile = Profile {
                name: latin1(name),
                skin,
                custom_skin: custom_skin.to_vec(),
                cosmetic: byte(p, m)?,
            };
            let mut m = m + 1;
            let mut first = None;
            let mut deltas = Vec::new();
            let mut iang = None;
            if m < n {
                first = Some((
                    f64::from(u24_at(p, m)?) / 5.,
                    f64::from(u24_at(p, m + 3)?) / 5.,
                ));
                m += 6;
                while m < n {
                    if m + 2 == n {
                        iang = Some(u16_at(p, m)?);
                    } else {
                        deltas.push((
                            (f64::from(byte(p, m)?) - 127.) / 2.,
                            (f64::from(byte(p, m + 1)?) - 127.) / 2.,
                        ));
                    }
                    m += 2;
                }
            }
            Packet::Appear(Appearance {
                id: u16_at(p, 1)?,
                profile,
                heading: angle24(p, 3)?,
                wanted: angle24(p, 7)?,
                speed: f64::from(u16_at(p, 10)?) / 1000. / SPEED_UNIT_SECONDS,
                fam: fraction24(p, 12)?,
                head: (
                    f64::from(u24_at(p, 16)?) / 5.,
                    f64::from(u24_at(p, 19)?) / 5.,
                ),
                first,
                deltas,
                iang,
            })
        }
        b'g' | b'n' | b'G' | b'N' | b'+' | b'=' => {
            let own = matches!(code, b'G' | b'N')
                || (code == b'=' && n == 7)
                || (code == b'+' && n == 10);
            let (target, m) = if own {
                (Target::Own, 1)
            } else {
                (Target::Id(u16_at(p, 1)?), 3)
            };
            let payload = n.saturating_sub(m);
            let grow = matches!(code, b'n' | b'N' | b'+');
            let (iang, absolute, fam_at) = match (code, payload) {
                (b'+', 9) | (b'=', 6) => (
                    Some(u16_at(p, m)?),
                    Some((u16_at(p, m + 2)?, u16_at(p, m + 4)?)),
                    m + 6,
                ),
                (b'g' | b'G', 0) | (b'n' | b'N', 3) => (None, None, m),
                (b'g' | b'G', 2) | (b'n' | b'N', 5) => (Some(u16_at(p, m)?), None, m + 2),
                _ => return Err(malformed(p)),
            };
            Packet::Anchor {
                target,
                iang,
                absolute,
                fam: if grow {
                    Some(fraction24(p, fam_at)?)
                } else {
                    None
                },
            }
        }
        b'e' | b'E' | b'3' | b'4' | b'5' | b'd' | b'7' => {
            let own = matches!(code, b'd' | b'7') || n - 1 <= 2;
            let (target, m) = if own {
                (Target::Own, 1)
            } else {
                (Target::Id(u16_at(p, 1)?), 3)
            };
            let all = || -> Result<_, String> {
                Ok((
                    Some(angle8(p, m)?),
                    Some(angle8(p, m + 1)?),
                    Some(speed8(p, m + 2)?),
                ))
            };
            let (heading, wanted, speed) = match (n, code) {
                (6, _) => all()?,
                (5 | 3, b'e') => (Some(angle8(p, m)?), None, Some(speed8(p, m + 1)?)),
                (5 | 3, b'E' | b'4') => (None, Some(angle8(p, m)?), Some(speed8(p, m + 1)?)),
                (5 | 3, b'3' | b'5') => (Some(angle8(p, m)?), Some(angle8(p, m + 1)?), None),
                (4 | 2, b'e') => (Some(angle8(p, m)?), None, None),
                (4 | 2, b'E' | b'4') => (None, Some(angle8(p, m)?), None),
                (4 | 2, b'3') => (None, None, Some(speed8(p, m)?)),
                (4 | 2, b'd' | b'7') => all()?,
                _ => return Err(malformed(p)),
            };
            Packet::Motion {
                target,
                heading,
                wanted,
                speed,
            }
        }
        b'h' if n == 6 => Packet::Fam(u16_at(p, 1)?, fraction24(p, 3)?),
        b'r' if n == 3 => Packet::Tail(u16_at(p, 1)?, None),
        b'r' if n == 6 => Packet::Tail(u16_at(p, 1)?, Some(fraction24(p, 3)?)),
        b'R' if n == 2 => Packet::Credit(p[1]),
        b'F' if n >= 3 && (n - 3) % 4 == 0 => Packet::Sector(
            p[1],
            p[2],
            p[3..]
                .chunks_exact(4)
                .map(|c| (c[1], c[2], f64::from(c[3]) / 5.))
                .collect(),
        ),
        b'b' | b'f' => match n - 1 {
            3 => Packet::Pellet(None, p[1], p[2], f64::from(p[3]) / 5.),
            4 => Packet::Pellet(None, p[1], p[2], f64::from(p[4]) / 5.),
            5 => Packet::Pellet(Some((p[1], p[2])), p[3], p[4], f64::from(p[5]) / 5.),
            6 => Packet::Pellet(Some((p[1], p[2])), p[3], p[4], f64::from(p[6]) / 5.),
            _ => return Err(malformed(p)),
        },
        b'c' | b'C' | b'<' => {
            let eater = usize::from(code == b'<') * 2;
            match (n - 1).checked_sub(eater) {
                Some(2) => Packet::Eaten(None, p[1], p[2]),
                Some(4) => Packet::Eaten(Some((p[1], p[2])), p[3], p[4]),
                _ => return Err(malformed(p)),
            }
        }
        b'w' if n == 3 => Packet::ClearSector(p[1], p[2]),
        b'y' => match n - 1 {
            2 | 4 => Packet::PreyRemove(u16_at(p, 1)?),
            19 => Packet::PreyAppear(
                u16_at(p, 1)?,
                (f64::from(u24_at(p, 4)?) / 5., f64::from(u24_at(p, 7)?) / 5.),
                f64::from(p[10]) / 5.,
                angle24(p, 12)?,
                angle24(p, 15)?,
                f64::from(u16_at(p, 18)?) / 1000. / SPEED_UNIT_SECONDS,
            ),
            _ => return Err(malformed(p)),
        },
        b'j' => {
            let m = 7;
            let speed = |i| -> Result<_, String> {
                Ok(Some(f64::from(u16_at(p, i)?) / 1000. / SPEED_UNIT_SECONDS))
            };
            // Byte `m` is a turn direction where present; it is not decoded.
            let (heading, wanted, speed) = match n - 1 {
                6 => (None, None, None),
                8 => (None, None, speed(m)?),
                9 => (Some(angle24(p, m)?), None, None),
                10 => (None, Some(angle24(p, m + 1)?), None),
                11 => (Some(angle24(p, m)?), None, speed(m + 3)?),
                12 => (None, Some(angle24(p, m + 1)?), speed(m + 4)?),
                13 => (Some(angle24(p, m + 1)?), Some(angle24(p, m + 4)?), None),
                15 => (
                    Some(angle24(p, m + 1)?),
                    Some(angle24(p, m + 4)?),
                    speed(m + 7)?,
                ),
                _ => return Err(malformed(p)),
            };
            Packet::PreyMove {
                id: u16_at(p, 1)?,
                position: (
                    1. + f64::from(u16_at(p, 3)?) * 3.,
                    1. + f64::from(u16_at(p, 5)?) * 3.,
                ),
                heading,
                wanted,
                speed,
            }
        }
        b'M' | b'V' | b'U' | b'u' | b'L' => Packet::Minimap(p),
        b'l' => {
            let mut rows = Vec::new();
            let mut m = 6;
            while m < n {
                let (name, next) = short_bytes(p, m + 6)?;
                rows.push((u16_at(p, m)?, fraction24(p, m + 2)?, p[m + 5], latin1(name)));
                m = next;
            }
            Packet::Leaderboard {
                own_position: byte(p, 1)?,
                rank: u16_at(p, 2)?,
                player_count: u16_at(p, 4)?,
                rows,
            }
        }
        b'm' => {
            let (name, m) = short_bytes(p, 7)?;
            Packet::Record {
                segments: u24_at(p, 1)?,
                fam: fraction24(p, 4)?,
                name: latin1(name),
                message: latin1(&p[m..]),
            }
        }
        b'S' if n == 7 => {
            Packet::Session(u16_at(p, 1)?, u32::from_be_bytes([p[3], p[4], p[5], p[6]]))
        }
        b'o' if n == 9 => Packet::TeamScore([i32_at(p, 1)?, i32_at(p, 5)?]),
        b'o' if n > 1 && (n - 1) % 8 == 0 => Packet::TeamHistory(
            (1..n)
                .step_by(8)
                .map(|i| Ok([i32_at(p, i)?, i32_at(p, i + 4)?]))
                .collect::<Result<_, String>>()?,
        ),
        b'k' if n == 6 => Packet::Kills(u16_at(p, 1)?, u24_at(p, 3)?),
        // The stock sector display list adds no food knowledge beyond F/w.
        b'W' if n == 3 => Packet::Ignored,
        b'p' => Packet::Pong,
        b'v' => Packet::GameOver,
        b'z' if n == 4 => Packet::Radius(f64::from(u24_at(p, 1)?)),
        b'h' | b'r' | b'R' | b'F' | b'w' | b'z' | b'S' | b'o' | b'k' | b'W' => {
            return Err(malformed(p));
        }
        // Challenge, admin `i` and other commands carry no state this contract exposes.
        _ => Packet::Ignored,
    })
}

/// Per-snake bookkeeping beside the receipt tracker, keyed by wire id.
#[derive(Clone, Debug)]
struct Side {
    slot: usize,
    iang: Option<u16>,
    /// False after an anchor whose position could not be reconstructed.
    body_known: bool,
    /// Server time of the last exactly received position.
    anchor_server: f64,
    base: Point,
    base_server: f64,
    /// `(server seconds, heading, speed)` reports since `base_server`.
    steps: Vec<(f64, f64, f64)>,
    profile: Profile,
    appeared_at: f64,
    /// Last `(count, receipt)` kill report for this appearance; the stock client
    /// starts every new appearance from an unreported count.
    kills: Option<(u32, f64)>,
}
impl Side {
    /// Last exact position advanced through the received headings and speeds.
    fn head_at(&self, server: f64) -> Point {
        let mut head = self.base;
        for (i, &(start, heading, speed)) in self.steps.iter().enumerate() {
            let end = self.steps.get(i + 1).map_or(server, |step| step.0);
            let dt = (end.min(server) - start.max(self.base_server)).max(0.);
            head.x += heading.cos() * speed * dt;
            head.y += heading.sin() * speed * dt;
        }
        head
    }
    fn restart(&mut self, point: Point, server: f64, heading: f64, speed: f64) {
        self.anchor_server = server;
        self.base = point;
        self.base_server = server;
        self.steps.clear();
        self.steps.push((server, heading, speed));
    }
    fn report(&mut self, server: f64, heading: f64, speed: f64) {
        self.steps.push((server, heading, speed));
        if self.steps.len() > MAX_MOTION_STEPS {
            // Fold the oldest report into the base; the result is still a prediction.
            let (start, heading, speed) = self.steps.remove(0);
            let next = self.steps[0].0;
            let dt = (next - start.max(self.base_server)).max(0.);
            self.base.x += heading.cos() * speed * dt;
            self.base.y += heading.sin() * speed * dt;
            self.base_server = next;
        }
    }
}

/// State of one connection. Replaced wholesale on reconnect.
#[derive(Clone, Debug, Default)]
struct Session {
    connected: bool,
    frames: u64,
    raw_framing: bool,
    header_ms: Option<u64>,
    setup_receipt: Option<f64>,
    server_time: Option<f64>,
    setup: Option<Setup>,
    sides: HashMap<u16, Side>,
    free_slots: Vec<usize>,
    slot_count: usize,
    self_id: Option<u16>,
    own_dead: bool,
    credit: u8,
    food: BTreeMap<u32, Food>,
    last_food_sector: Option<(u8, u8)>,
    last_eaten_sector: Option<(u8, u8)>,
    prey: BTreeMap<u16, Prey>,
    minimap: MinimapDecoder,
    /// HUD reports stored with `age_seconds` zero beside their receipt time.
    leaderboard: Option<(Leaderboard, f64)>,
    record: Option<(DailyRecord, f64)>,
    session_ids: Option<(SessionIdentifiers, f64)>,
    teams: Option<(TeamRing, f64)>,
    /// `(snake id, killed, receipt)`, newest first.
    removals: VecDeque<(u16, Option<bool>, f64)>,
    bodies_at: Option<f64>,
    food_at: Option<f64>,
    prey_at: Option<f64>,
    pending_ping: Option<f64>,
    last_ping: Option<f64>,
    ping: Option<f64>,
    pongs: VecDeque<(f64, f64)>,
    sent_heading: Option<f64>,
    sent_boost: Option<bool>,
    sent: VecDeque<(f64, Action)>,
}

/// Client knowledge reconstructed only from received and actually sent bytes.
#[derive(Clone, Debug, Default)]
pub struct ClientState {
    tracker: ReceivedTracker,
    session: Session,
    /// Per-id life counters; kept across reconnects so ids never repeat a life.
    incarnations: HashMap<u16, u64>,
    life: u64,
    local: Option<f64>,
    processing: f64,
    disconnected: bool,
}

impl ClientState {
    pub fn new() -> Self {
        Self::default()
    }

    fn local_clock(&self, now: f64) -> Result<(), String> {
        if !now.is_finite() || now < 0. || self.local.is_some_and(|last| now < last) {
            return Err("Invalid local clock".into());
        }
        Ok(())
    }

    /// Start a fresh connection: all received state, pings and sent history reset.
    pub fn new_connection(&mut self, now: f64) -> Result<(), String> {
        self.local_clock(now)?;
        self.tracker.reconnect();
        self.session = Session {
            connected: true,
            ..Default::default()
        };
        self.disconnected = false;
        self.local = Some(now);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.disconnected = true;
        self.session.pending_ping = None;
    }

    fn require_connection(&self) -> Result<(), String> {
        if !self.session.connected || self.disconnected {
            return Err("No open connection".into());
        }
        Ok(())
    }

    /// One received WebSocket message. Invalid frames leave the state unchanged.
    pub fn receive_frame(&mut self, bytes: &[u8], receipt_seconds: f64) -> Result<(), String> {
        self.require_connection()?;
        self.local_clock(receipt_seconds)?;
        if bytes.is_empty() {
            return Err("Empty frame".into());
        }
        // A bare first challenge identifies a connection without timing headers.
        let raw = if self.session.frames == 0 {
            bytes[0] == b'6'
        } else {
            self.session.raw_framing
        };
        let packets = if raw {
            split_payload(bytes)?
        } else {
            split_frame(bytes)?
        };
        let setup = packets.iter().any(|p| p[0] == b'a');
        let (server, header_ms) = if raw {
            let start = if setup {
                Some(receipt_seconds)
            } else {
                self.session.setup_receipt
            };
            (start.map_or(0., |start| receipt_seconds - start), None)
        } else {
            // The setup header is an initial epoch, not elapsed time: setup is
            // the game-clock origin and later headers are elapsed intervals.
            let header = u64::from(bytes[0]) << 8 | u64::from(bytes[1]);
            let ms = match self.session.header_ms {
                Some(ms) if !setup => ms + header,
                _ => 0,
            };
            (ms as f64 / 1000., Some(ms))
        };
        self.apply(&packets, server, receipt_seconds)?;
        self.session.frames += 1;
        self.session.raw_framing = raw;
        if raw && setup {
            self.session.setup_receipt = Some(receipt_seconds);
        }
        if header_ms.is_some() {
            self.session.header_ms = header_ms;
        }
        Ok(())
    }

    /// One command packet with explicit clocks, bypassing transport framing.
    pub fn receive_packet(
        &mut self,
        packet: &[u8],
        server_seconds: f64,
        receipt_seconds: f64,
    ) -> Result<(), String> {
        self.require_connection()?;
        self.local_clock(receipt_seconds)?;
        self.apply(&[packet], server_seconds, receipt_seconds)
    }

    fn apply(&mut self, packets: &[&[u8]], server: f64, receipt: f64) -> Result<(), String> {
        let parsed = packets
            .iter()
            .map(|packet| parse(packet))
            .collect::<Result<Vec<_>, _>>()?;
        let setup = parsed.iter().any(|p| matches!(p, Packet::Setup(_)));
        if !server.is_finite()
            || server < 0.
            || (!setup && self.session.server_time.is_some_and(|last| server < last))
        {
            return Err("Invalid server clock".into());
        }
        // Validate state-dependent packets before any mutation.
        let mut has_setup = self.session.setup.is_some();
        // Cloned only for frames carrying a minimap packet, then committed whole.
        let mut minimap: Option<MinimapDecoder> = None;
        for (packet, raw) in parsed.iter().zip(packets) {
            match packet {
                Packet::Setup(_) if has_setup => {
                    return Err("Repeated setup on one connection".into());
                }
                Packet::Setup(_) => has_setup = true,
                _ if packet.needs_setup() && !has_setup => {
                    return Err(format!("'{}' packet before setup", char::from(raw[0])));
                }
                Packet::Minimap(payload) => {
                    minimap
                        .get_or_insert_with(|| self.session.minimap.clone())
                        .apply(payload, receipt)?;
                }
                _ => {}
            }
        }
        for packet in parsed {
            self.apply_packet(packet, server, receipt)?;
        }
        if let Some(decoder) = minimap {
            self.session.minimap = decoder;
        }
        self.session.server_time = Some(server);
        self.local = Some(receipt);
        Ok(())
    }

    fn resolve(&self, target: Target) -> Option<u16> {
        match target {
            Target::Id(id) => Some(id),
            Target::Own if self.session.own_dead => None,
            Target::Own => self.session.self_id,
        }
    }

    fn own_score(&mut self, id: u16) -> Result<(), String> {
        let session = &self.session;
        if session.self_id != Some(id) || session.own_dead {
            return Ok(());
        }
        let (Some(side), Some(setup)) = (session.sides.get(&id), &session.setup) else {
            return Ok(());
        };
        let Some(snake) = self.tracker.snake(side.slot) else {
            return Ok(());
        };
        let score = setup.score(snake.segments + usize::from(session.credit), snake.fam);
        if score.is_finite() && score >= 0. {
            self.tracker.own_score(side.slot, score)?;
        }
        Ok(())
    }

    fn mass(&mut self, id: u16, segments: usize, fam: f64, receipt: f64) -> Result<(), String> {
        let slot = self.session.sides[&id].slot;
        self.tracker.snake(slot).ok_or("Missing received snake")?;
        self.tracker.mass_values(
            slot,
            segments,
            fam,
            physics::BODY_RADIUS * physics::scale(segments),
            segments as f64 + fam,
            receipt,
        )?;
        self.own_score(id)
    }

    fn apply_packet(
        &mut self,
        packet: Packet<'_>,
        server: f64,
        receipt: f64,
    ) -> Result<(), String> {
        let session = &mut self.session;
        match packet {
            Packet::Setup(setup) => {
                session.setup = Some(setup);
            }
            Packet::Appear(appearance) => self.appear(appearance, server, receipt)?,
            Packet::Remove(id, killed) => {
                session.bodies_at = Some(receipt);
                if let Some(side) = session.sides.remove(&id) {
                    self.tracker.removal(side.slot, receipt)?;
                    session.free_slots.push(side.slot);
                    session.removals.push_front((id, killed, receipt));
                    session.removals.truncate(MAX_REMOVALS);
                }
                if session.self_id == Some(id) {
                    session.own_dead = true;
                }
            }
            Packet::Anchor {
                target,
                iang,
                absolute,
                fam,
            } => {
                let Some(id) = self.resolve(target) else {
                    return Ok(());
                };
                self.anchor(id, iang, absolute, fam, server, receipt)?;
            }
            Packet::Motion {
                target,
                heading,
                wanted,
                speed,
            } => {
                let Some(id) = self.resolve(target) else {
                    return Ok(());
                };
                let Some(side) = self.session.sides.get_mut(&id) else {
                    return Ok(());
                };
                self.tracker
                    .motion(side.slot, heading, wanted, speed, receipt)?;
                let snake = self
                    .tracker
                    .snake(side.slot)
                    .ok_or("Missing received snake")?;
                side.report(server, snake.heading, snake.speed);
            }
            Packet::Fam(id, fam) => {
                let Some(side) = session.sides.get(&id) else {
                    return Ok(());
                };
                let segments = self.tracker.snake(side.slot).map_or(0, |s| s.segments);
                self.mass(id, segments, fam, receipt)?;
            }
            Packet::Tail(id, fam) => {
                let Some(side) = session.sides.get(&id) else {
                    return Ok(());
                };
                session.bodies_at = Some(receipt);
                let snake = self
                    .tracker
                    .snake(side.slot)
                    .ok_or("Missing received snake")?;
                let (segments, old_fam, length) = (snake.segments, snake.fam, snake.body.len());
                if side.body_known {
                    self.tracker
                        .tail(side.slot, length.saturating_sub(1), receipt)?;
                }
                self.mass(
                    id,
                    segments.saturating_sub(1),
                    fam.unwrap_or(old_fam),
                    receipt,
                )?;
            }
            Packet::Credit(credit) => {
                session.credit = credit;
                if let Some(id) = session.self_id {
                    self.own_score(id)?;
                }
            }
            Packet::Sector(sx, sy, pellets) => {
                session.food_at = Some(receipt);
                for (rx, ry, size) in pellets {
                    self.add_food((sx, sy), rx, ry, size);
                }
            }
            Packet::Pellet(sector, rx, ry, size) => {
                session.food_at = Some(receipt);
                if sector.is_some() {
                    session.last_food_sector = sector;
                }
                // A short form before any sector-bearing form has no known sector.
                if let Some(sector) = session.last_food_sector {
                    self.add_food(sector, rx, ry, size);
                }
            }
            Packet::Eaten(sector, rx, ry) => {
                session.food_at = Some(receipt);
                if sector.is_some() {
                    session.last_eaten_sector = sector;
                }
                if let Some((sx, sy)) = session.last_eaten_sector {
                    session.food.remove(&food_id(sx, sy, rx, ry));
                }
            }
            Packet::ClearSector(sx, sy) => {
                session.food_at = Some(receipt);
                session
                    .food
                    .retain(|&id, _| id >> 16 != u32::from(sx) << 8 | u32::from(sy));
            }
            Packet::PreyAppear(id, (x, y), size, wanted, heading, speed) => {
                session.prey_at = Some(receipt);
                let setup = session.setup.as_ref().ok_or("Missing setup")?;
                session.prey.insert(
                    id,
                    Prey {
                        id: u64::from(id),
                        position: setup.center(x, y),
                        heading,
                        target_heading: wanted,
                        speed,
                        size,
                    },
                );
            }
            Packet::PreyRemove(id) => {
                session.prey_at = Some(receipt);
                session.prey.remove(&id);
            }
            Packet::PreyMove {
                id,
                position: (x, y),
                heading,
                wanted,
                speed,
            } => {
                session.prey_at = Some(receipt);
                let setup = session.setup.as_ref().ok_or("Missing setup")?;
                let position = setup.center(x, y);
                if let Some(prey) = session.prey.get_mut(&id) {
                    prey.position = position;
                    prey.heading = heading.unwrap_or(prey.heading);
                    prey.target_heading = wanted.unwrap_or(prey.target_heading);
                    prey.speed = speed.unwrap_or(prey.speed);
                }
            }
            // Decoded transactionally in `apply`.
            Packet::Minimap(_) => {}
            Packet::Leaderboard {
                own_position,
                rank,
                player_count,
                rows,
            } => {
                let setup = session.setup.as_ref().ok_or("Missing setup")?;
                let entries = rows
                    .into_iter()
                    .enumerate()
                    .map(|(i, (segments, fam, color, name))| LeaderboardEntry {
                        segments,
                        fam,
                        score: setup.reported_score(usize::from(segments), fam),
                        color,
                        name,
                        own: i + 1 == usize::from(own_position),
                    })
                    .collect();
                let board = Leaderboard {
                    own_position: (own_position != 0).then_some(own_position),
                    rank,
                    player_count,
                    entries,
                    age_seconds: 0.,
                };
                session.leaderboard = Some((board, receipt));
            }
            Packet::Record {
                segments,
                fam,
                name,
                message,
            } => {
                let setup = session.setup.as_ref().ok_or("Missing setup")?;
                let record = DailyRecord {
                    segments,
                    fam,
                    score: setup.reported_score(segments as usize, fam),
                    name,
                    message,
                    age_seconds: 0.,
                };
                session.record = Some((record, receipt));
            }
            Packet::Session(id, session_id) => {
                let ids = SessionIdentifiers {
                    snake_id: id.to_string(),
                    session_id,
                    age_seconds: 0.,
                };
                session.session_ids = Some((ids, receipt));
            }
            Packet::TeamScore(pair) => {
                let (ring, at) = session.teams.get_or_insert_with(Default::default);
                ring.update(pair);
                *at = receipt;
            }
            Packet::TeamHistory(pairs) => {
                let (ring, at) = session.teams.get_or_insert_with(Default::default);
                ring.bulk(&pairs);
                *at = receipt;
            }
            Packet::Kills(id, count) => {
                if let Some(side) = session.sides.get_mut(&id) {
                    side.kills = Some((count, receipt));
                }
            }
            Packet::Pong => {
                if let Some(sent) = session.pending_ping.take() {
                    let rtt = receipt - sent;
                    if (0. ..=MAX_PING_SECONDS).contains(&rtt) {
                        session.ping = Some(rtt);
                        session.pongs.push_back((receipt, rtt));
                    }
                }
                while session
                    .pongs
                    .front()
                    .is_some_and(|&(at, _)| receipt - at > PING_RANGE_SECONDS)
                {
                    session.pongs.pop_front();
                }
            }
            Packet::GameOver => session.own_dead = true,
            Packet::Radius(radius) => {
                session.setup.as_mut().ok_or("Missing setup")?.radius = radius;
            }
            Packet::Ignored => {}
        }
        Ok(())
    }

    fn add_food(&mut self, (sx, sy): (u8, u8), rx: u8, ry: u8, size: f64) {
        let Some(setup) = &self.session.setup else {
            return;
        };
        let sector = f64::from(setup.settings.sector_size);
        let point = setup.center(
            f64::from(sx) * sector + f64::from(rx) * (sector / 256.),
            f64::from(sy) * sector + f64::from(ry) * (sector / 256.),
        );
        self.session.food.insert(
            food_id(sx, sy, rx, ry),
            Food {
                x: point.x,
                y: point.y,
                value: physics::nutrition(size, 2.),
                size: Some(size),
                origin: None,
            },
        );
    }

    fn appear(&mut self, a: Appearance, server: f64, receipt: f64) -> Result<(), String> {
        let session = &mut self.session;
        let setup = session.setup.as_ref().ok_or("Missing setup")?;
        let spacing = setup.settings.body_spacing;
        let mut points = Vec::with_capacity(a.deltas.len() + 2);
        if let Some((mut x, mut y)) = a.first {
            points.push((x, y));
            for (dx, dy) in &a.deltas {
                x += dx;
                y += dy;
                points.push((x, y));
            }
            if let Some(iang) = a.iang {
                let angle = f64::from(iang) * TAU / 65536.;
                x += angle.cos() * spacing;
                y += angle.sin() * spacing;
                points.push((x, y));
            }
        }
        // The wire lists anchors tail to head; the contract stores head to tail.
        let body: Vec<Point> = points
            .iter()
            .rev()
            .map(|&(x, y)| setup.center(x, y))
            .collect();
        let head = setup.center(a.head.0, a.head.1);
        let segments = body.len();
        let snake = Snake {
            id: a.id.to_string(),
            head,
            body,
            heading: a.heading,
            speed: a.speed,
            radius: physics::BODY_RADIUS * physics::scale(segments),
            length: segments as f64 + a.fam,
            segments,
            fam: a.fam,
            score: 0.,
            kills: 0,
            alive: true,
        };
        let slot = match session.sides.get(&a.id) {
            Some(side) => side.slot,
            None => session.free_slots.pop().unwrap_or_else(|| {
                session.slot_count += 1;
                session.slot_count - 1
            }),
        };
        self.tracker.ensure_slots(session.slot_count);
        self.tracker.appearance(slot, &snake, a.wanted, receipt)?;
        *self.incarnations.entry(a.id).or_default() += 1;
        let mut side = Side {
            slot,
            iang: a.iang,
            body_known: true,
            anchor_server: server,
            base: head,
            base_server: server,
            steps: Vec::new(),
            profile: a.profile,
            appeared_at: receipt,
            kills: None,
        };
        side.restart(head, server, a.heading, a.speed);
        session.sides.insert(a.id, side);
        session.bodies_at = Some(receipt);
        // The stock client adopts the first full appearance after setup as its own.
        if session.self_id.is_none() {
            session.self_id = Some(a.id);
            self.life += 1;
        }
        self.own_score(a.id)
    }

    fn anchor(
        &mut self,
        id: u16,
        iang: Option<u16>,
        absolute: Option<(u16, u16)>,
        fam: Option<f64>,
        server: f64,
        receipt: f64,
    ) -> Result<(), String> {
        let session = &mut self.session;
        let Some(side) = session.sides.get_mut(&id) else {
            return Ok(());
        };
        let setup = session.setup.as_ref().ok_or("Missing setup")?;
        session.bodies_at = Some(receipt);
        if iang.is_some() {
            side.iang = iang;
        }
        let snake = self
            .tracker
            .snake(side.slot)
            .ok_or("Missing received snake")?;
        let (heading, speed, segments) = (snake.heading, snake.speed, snake.segments);
        let point = match (absolute, side.body_known, snake.body.first(), side.iang) {
            (Some((x, y)), ..) => Some(setup.center(f64::from(x), f64::from(y))),
            (None, true, Some(last), Some(iang)) => {
                let angle = f64::from(iang) * TAU / 65536.;
                let spacing = setup.settings.body_spacing;
                Some(Point {
                    x: last.x + angle.cos() * spacing,
                    y: last.y + angle.sin() * spacing,
                })
            }
            _ => None,
        };
        let contraction = setup.settings.body_contraction;
        match point {
            Some(point) => {
                if side.body_known {
                    self.tracker.anchor_with(
                        side.slot,
                        point,
                        fam.is_some(),
                        contraction,
                        receipt,
                    )?;
                }
                self.tracker.position(side.slot, point, receipt)?;
                side.restart(point, server, heading, speed);
            }
            // Without a previous anchor or direction the new anchor is unknown;
            // the body stays unknown until the snake appears again.
            None if side.body_known => {
                side.body_known = false;
                self.tracker.tail(side.slot, 0, receipt)?;
            }
            None => {}
        }
        match fam {
            Some(fam) => self.mass(id, segments + 1, fam, receipt),
            None => Ok(()),
        }
    }

    /// Record bytes actually sent. Only game commands are kept; handshake ignored.
    pub fn record_sent(&mut self, packet: &[u8], at: f64) -> Result<(), String> {
        self.require_connection()?;
        self.local_clock(at)?;
        self.local = Some(at);
        let session = &mut self.session;
        // Before setup, single bytes are pre-login negotiation such as [1], [2]
        // or the session request [4], not steering, boost or ping commands.
        let (&[code], Some(_)) = (packet, &session.setup) else {
            return Ok(());
        };
        match code {
            0..=250 => session.sent_heading = Some(f64::from(code) * TAU / 251.),
            251 => {
                session.pending_ping = Some(at);
                session.last_ping = Some(at);
                return Ok(());
            }
            253 | 254 => session.sent_boost = Some(code == 253),
            _ => return Ok(()),
        }
        // Until both components have been sent, the server's command is unknown.
        if let (Some(heading), Some(boost)) = (session.sent_heading, session.sent_boost) {
            let action = Action { heading, boost };
            // Bytes sent together form one command.
            if session.sent.front().is_some_and(|&(time, _)| time == at) {
                session.sent[0].1 = action;
            } else {
                session.sent.push_front((at, action));
                session.sent.truncate(MAX_SENT_COMMANDS);
            }
        }
        Ok(())
    }

    pub fn ping_due(&self, now: f64) -> Result<bool, String> {
        self.local_clock(now)?;
        let session = &self.session;
        Ok(session.connected
            && !self.disconnected
            && session.setup.is_some()
            && session.pending_ping.is_none()
            && session
                .last_ping
                .is_none_or(|at| now - at > PING_INTERVAL_SECONDS))
    }

    pub fn set_processing_seconds(&mut self, seconds: f64) -> Result<(), String> {
        if !seconds.is_finite() || seconds < 0. {
            return Err("Invalid processing duration".into());
        }
        self.processing = seconds;
        Ok(())
    }

    /// Report received lifecycle state without cloning observations or advancing receipts.
    pub fn status(&self) -> ClientStatus {
        let session = &self.session;
        if !session.connected || self.disconnected {
            ClientStatus::Disconnected
        } else if session.own_dead {
            ClientStatus::Dead
        } else if session
            .self_id
            .is_some_and(|id| session.sides.contains_key(&id))
        {
            ClientStatus::Playing
        } else {
            ClientStatus::Waiting
        }
    }

    fn snake(&self, id: u16, side: &Side, server: f64) -> Option<Snake> {
        let setup = self.session.setup.as_ref()?;
        let mut snake = self.tracker.snake(side.slot)?.clone();
        snake.head = side.head_at(server);
        let credit = if self.session.self_id == Some(id) {
            usize::from(self.session.credit)
        } else {
            0
        };
        snake.score = setup.score(snake.segments + credit, snake.fam);
        // Zero until reported; `hud.snakes[].kills` distinguishes unknown from zero.
        snake.kills = side.kills.map_or(0, |(count, _)| count as usize);
        if !side.body_known {
            snake.body.clear();
        }
        Some(snake)
    }

    fn metadata(&self, now: f64, server: f64) -> ReceivedMetadata {
        let session = &self.session;
        let mut metadata = self.tracker.metadata(now);
        for row in &mut metadata.snakes {
            let Some((id, side)) = row
                .snake_id
                .parse::<u16>()
                .ok()
                .and_then(|id| Some((id, session.sides.get(&id)?)))
            else {
                continue;
            };
            row.incarnation = self.incarnations.get(&id).copied().unwrap_or(1);
            row.position.predicted = row.position.valid && server > side.anchor_server;
            if !side.body_known {
                row.body = ReceivedField::default();
            }
        }
        let category = |at: Option<f64>| CategoryKnowledge {
            valid: at.is_some(),
            complete: false,
            age_seconds: at.map_or(0., |at| (now - at).max(0.)),
        };
        metadata.bodies = category(session.bodies_at);
        metadata.food = category(session.food_at);
        metadata.prey = category(session.prey_at);
        metadata.minimap =
            session
                .minimap
                .shown
                .as_ref()
                .map_or_else(ReceivedField::default, |(_, at)| ReceivedField {
                    age_seconds: (now - at).max(0.),
                    valid: true,
                    predicted: false,
                });
        metadata
    }

    fn hud(&self, now: f64) -> ClientHud {
        let session = &self.session;
        let age = |at: f64| (now - at).max(0.);
        let mut ids: Vec<u16> = session.sides.keys().copied().collect();
        ids.sort_unstable();
        ClientHud {
            leaderboard: session.leaderboard.as_ref().map(|(board, at)| Leaderboard {
                age_seconds: age(*at),
                ..board.clone()
            }),
            record: session.record.as_ref().map(|(record, at)| DailyRecord {
                age_seconds: age(*at),
                ..record.clone()
            }),
            session: session
                .session_ids
                .as_ref()
                .map(|(ids, at)| SessionIdentifiers {
                    age_seconds: age(*at),
                    ..ids.clone()
                }),
            team_scores: session.teams.as_ref().map(|(ring, at)| {
                let history = ring.newest_first();
                TeamScores {
                    team1: history[0][0],
                    team2: history[0][1],
                    history,
                    age_seconds: age(*at),
                }
            }),
            snakes: ids
                .into_iter()
                .map(|id| {
                    let side = &session.sides[&id];
                    SnakeProfile {
                        snake_id: id.to_string(),
                        name: side.profile.name.clone(),
                        skin: side.profile.skin,
                        custom_skin: side.profile.custom_skin.clone(),
                        cosmetic: side.profile.cosmetic,
                        kills: side.kills.map(|(count, at)| KillReport {
                            count,
                            age_seconds: age(at),
                        }),
                        age_seconds: age(side.appeared_at),
                    }
                })
                .collect(),
            removals: session
                .removals
                .iter()
                .map(|&(id, killed, at)| SnakeRemoval {
                    snake_id: id.to_string(),
                    killed,
                    age_seconds: age(at),
                })
                .collect(),
            minimap_command: session
                .minimap
                .command
                .map(|command| char::from(command).to_string()),
            minimap_teams: session
                .minimap
                .teams
                .as_ref()
                .map(|(side, layers, at)| MinimapTeams {
                    side: *side,
                    layers: layers.clone(),
                    age_seconds: age(*at),
                }),
        }
    }

    /// Poll at a local time. Reads only; receipt ages advance with `now`.
    pub fn frame(&self, now: f64) -> Result<ClientFrame, String> {
        self.local_clock(now)?;
        let session = &self.session;
        let status = self.status();
        let server = session.server_time.unwrap_or(0.);
        let observation = match (status, session.self_id, &session.setup) {
            (ClientStatus::Playing, Some(id), Some(setup)) => {
                let me = self.snake(id, &session.sides[&id], server);
                let opponents = self
                    .tracker
                    .alive_snakes()
                    .filter_map(|(_, snake)| snake.id.parse::<u16>().ok())
                    .filter(|&other| other != id)
                    .filter_map(|other| self.snake(other, session.sides.get(&other)?, server))
                    .collect();
                me.map(|me| Observation {
                    time: server,
                    me,
                    opponents,
                    food: session.food.values().cloned().collect(),
                    prey: session.prey.values().cloned().collect(),
                    arena: Arena {
                        center: Point::default(),
                        radius: setup.radius,
                    },
                    vision_radius: None,
                })
            }
            _ => None,
        };
        let recent: Vec<f64> = session
            .pongs
            .iter()
            .filter(|&&(at, _)| now - at <= PING_RANGE_SECONDS)
            .map(|&(_, rtt)| rtt)
            .collect();
        Ok(ClientFrame {
            contract: CONTRACT.into(),
            time: now,
            status,
            life: self.life,
            settings: session.setup.as_ref().map(|setup| setup.settings.clone()),
            observation,
            received: self.metadata(now, server),
            minimap: session.minimap.shown.as_ref().map(|(map, at)| Minimap {
                age_seconds: (now - at).max(0.),
                ..map.clone()
            }),
            hud: self.hud(now),
            timing: ClientTiming {
                ping_seconds: session.ping,
                ping_range_seconds: (!recent.is_empty()).then(|| {
                    [
                        recent.iter().copied().fold(f64::INFINITY, f64::min),
                        recent.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    ]
                }),
                ping_wait_seconds: session.pending_ping.map(|at| (now - at).max(0.)),
                processing_seconds: self.processing,
                sent_commands: session
                    .sent
                    .iter()
                    .map(|&(at, action)| SentCommand {
                        seconds_ago: (now - at).max(0.),
                        action,
                    })
                    .collect(),
            },
        })
    }
}

fn food_id(sx: u8, sy: u8, rx: u8, ry: u8) -> u32 {
    u32::from_be_bytes([sx, sy, rx, ry])
}
