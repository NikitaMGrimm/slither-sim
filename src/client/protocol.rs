//! Protocol-19 decoding into the client contract, shared by every backend.
//!
//! Wire extraction uses `slither_protocol` with historical simulator admission.
//! The independent Python reader stays in `tools/fidelity/slither_protocol.py`.
//! Coordinates are centered by subtracting the setup grid center. Only transmitted
//! state is kept: kill counts come only from `k` reports, private score credits are
//! never reconstructed, and unknown values stay unknown rather than being filled
//! from simulator state. Admin (`i`) and testing-only commands are ignored.
use super::{
    CONTRACT, CategoryKnowledge, ClientFrame, ClientStatus, ClientTiming, ReceivedField,
    ReceivedMetadata, ReceivedTracker, SentCommand, ServerSettings,
    fast_hash::{FastMap, FastSet},
    hud::{
        ClientHud, DailyRecord, KillReport, Leaderboard, LeaderboardEntry, MAX_REMOVALS,
        MinimapTeams, SessionIdentifiers, SnakeProfile, SnakeRemoval, TeamRing, TeamScores, latin1,
    },
    minimap::{MAX_FRAME_MINIMAP_CELLS, Minimap, MinimapDecoder},
};
use crate::{Action, Arena, Food, Observation, Point, Snake, physics, prey::Prey};
use slither_protocol::{DecodeAdmission, DecodeError, ErrorKind, frame, scalar, v19};
use std::collections::{BTreeMap, VecDeque};
use std::f64::consts::TAU;
use std::sync::OnceLock;

pub const PROTOCOL_VERSION: u8 = 19;
/// Sent commands retained to interpret server replies; a local budget.
const MAX_SENT_COMMANDS: usize = 64;
/// History behind `pingRangeSeconds`; a reporting choice.
const PING_RANGE_SECONDS: f64 = 5.;
/// The stock client sends a game ping only after more than 250 ms.
const PING_INTERVAL_SECONDS: f64 = 0.25;
/// The stock `setMscps` extends its score tables by this many repeated entries.
const SCORE_TABLE_EXTENSION: usize = 2048;
/// Decoder retention budgets: a hostile or broken server cannot grow received
/// state without bound. Local resource policies, not protocol widths; dense
/// simulated worlds can exceed them before any wire field overflows.
const MAX_RECEIVED_SNAKES: usize = 4096;
const MAX_RECEIVED_BODY_POINTS: usize = 65_536;
const MAX_RECEIVED_SNAKE_POINTS: usize = 4096;
const MAX_RECEIVED_FOOD: usize = 131_072;

struct FoodBudget<'a> {
    base: &'a BTreeMap<u32, Food>,
    updates: BTreeMap<u32, bool>,
    cleared_sectors: FastSet<u16>,
    count: usize,
}
impl FoodBudget<'_> {
    fn set(&mut self, id: u32, present: bool) -> Result<(), String> {
        let previous = self.updates.get(&id).copied().unwrap_or_else(|| {
            !self.cleared_sectors.contains(&((id >> 16) as u16)) && self.base.contains_key(&id)
        });
        if present && !previous {
            if self.count == MAX_RECEIVED_FOOD {
                return Err("Received food limit exceeded".into());
            }
            self.count += 1;
        } else if previous && !present {
            self.count -= 1;
        }
        self.updates.insert(id, present);
        Ok(())
    }

    fn clear_sector(&mut self, sx: u8, sy: u8) {
        let first = food_id(sx, sy, 0, 0);
        let last = food_id(sx, sy, 255, 255);
        // Visit original pellets only on the first clear; later clears must not
        // rescan deleted base entries or resurrect them after an insertion.
        if self.cleared_sectors.insert(u16::from_be_bytes([sx, sy])) {
            self.count -= self
                .base
                .range(first..=last)
                .filter(|(id, _)| !self.updates.contains_key(id))
                .count();
        }
        for (_, present) in self.updates.extract_if(first..=last, |_, _| true) {
            self.count -= usize::from(present);
        }
    }
}

#[derive(Clone, Copy)]
struct BodyBudget {
    slot: usize,
    known: bool,
    direction: bool,
    segments: usize,
}

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
/// Split one elapsed-ms framed message into command packets.
pub fn split_frame(bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    if bytes.len() < 3 {
        return Err("Short timing frame".into());
    }
    split_payload(&bytes[2..])
}

/// Both timed and header-free connections use the same packet bundling.
pub(crate) fn split_payload(bytes: &[u8]) -> Result<Vec<&[u8]>, String> {
    decode_payload(bytes).map(|frame| frame.commands().map(|command| command.bytes).collect())
}

fn decode_payload(bytes: &[u8]) -> Result<frame::DecodedFrame<'_>, String> {
    frame::decode_frame(
        bytes,
        frame::Header::Absent,
        frame::Limits::UNBOUNDED,
        frame::FrameAdmission::Historical,
    )
    .map_err(|error| {
        match error.kind {
            ErrorKind::Empty => "Empty packet payload",
            ErrorKind::Truncated => "Truncated packet length",
            _ => "Invalid packet length",
        }
        .into()
    })
}

/// Only the supported challenge grammar identifies header-free framing; timing
/// headers and session IDs may consist entirely of alphabetic bytes too.
pub(crate) fn initial_raw_framing(bytes: &[u8]) -> bool {
    bytes.first() == Some(&b'6') && super::challenge::solve_challenge(&bytes[1..]).is_ok()
}

/// Encode one decision as the bytes a protocol-19 client sends: a heading byte on
/// the 251-step grid, then `[253]`/`[254]` only when boost differs from the last
/// boost actually sent.
pub fn action_packets(
    action: Action,
    previous_boost: Option<bool>,
) -> Result<Vec<Vec<u8>>, String> {
    let heading = v19::client::sim_heading(action.heading)
        .map_err(|_| "Action heading must be finite".to_owned())?;
    let mut packets = vec![heading.to_vec()];
    if previous_boost != Some(action.boost) {
        packets.push(v19::client::boost(action.boost).to_vec());
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
    Sector(v19::FoodSectorRef<'a>),
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
    Rules(crate::WorldRules),
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

// Preserve the public simulator diagnostics, including its validation priority.
// The shared reader owns acceptance and field extraction; these are error labels.
fn decode_error(p: &[u8], error: DecodeError) -> String {
    match p.first() {
        None => "Empty packet".into(),
        Some(b'a') if p.len() < 25 => "Short setup packet".into(),
        Some(b'a') if p[23] != PROTOCOL_VERSION => format!(
            "Unsupported protocol version {}; expected {PROTOCOL_VERSION}",
            p[23]
        ),
        Some(b'a') if matches!(p.len(), 26 | 28 | 29) => truncated(p),
        Some(b'a') if u16::from_be_bytes([p[21], p[22]]) > 1000 => {
            "Unsupported body contraction; expected 0 through 1".into()
        }
        Some(b'a') => "Invalid setup values".into(),
        Some(b'~') => "Invalid SlitherSim rules packet".into(),
        Some(b's' | b'l' | b'm') => truncated(p),
        Some(b'd' | b'7') if p.len() == 2 => truncated(p),
        _ if error.kind == ErrorKind::Truncated => truncated(p),
        _ => malformed(p),
    }
}

fn target(value: v19::Target) -> Target {
    match value {
        v19::Target::Own => Target::Own,
        v19::Target::Id(id) => Target::Id(id),
        _ => unreachable!("new shared target requires a simulator projection"),
    }
}

#[inline]
fn parse(p: &[u8]) -> Result<Packet<'_>, String> {
    // Minimap syntax is deliberately deferred to validation after setup/budgets,
    // as before extraction. Eager decoding would change error precedence.
    if matches!(p.first(), Some(b'M' | b'V' | b'U' | b'u' | b'L')) {
        return Ok(Packet::Minimap(p));
    }
    Ok(
        match v19::decode(p, DecodeAdmission::Sim).map_err(|error| decode_error(p, error))? {
            v19::Message::Setup(s) => Packet::Setup(Setup {
                settings: ServerSettings {
                    protocol: PROTOCOL_VERSION,
                    grid_center: s.grid_center,
                    sector_size: s.sector_size,
                    max_segments: s.max_segments,
                    body_spacing: f64::from(s.body_spacing),
                    body_contraction: f64::from(s.contraction) / 1000.,
                    sector_count_along_edge: s.sector_count,
                    spangdv: f64::from(s.spangdv) / 10.,
                    nsp1: f64::from(s.nsp1) / 100.,
                    nsp2: f64::from(s.nsp2) / 100.,
                    nsp3: f64::from(s.nsp3) / 100.,
                    mamu: f64::from(s.mamu) / 1000.,
                    mamu2: f64::from(s.mamu2) / 1000.,
                    server_number: s.server_number,
                    game_mode: s.game_mode,
                    team_val: s.team,
                },
                radius: s
                    .radius
                    .map(f64::from)
                    .unwrap_or_else(|| f64::from(s.grid_center) * 0.98),
                scores: score_table(s.max_segments),
            }),
            v19::Message::SnakeAppearance(s) => {
                let mut deltas = Vec::new();
                let mut iang = None;
                for delta in s.body.deltas() {
                    match delta {
                        v19::BodyDelta::HalfUnits([x, y]) => {
                            deltas.push((scalar::delta_half(x), scalar::delta_half(y)))
                        }
                        v19::BodyDelta::FinalDirection(direction) => iang = Some(direction),
                        _ => unreachable!("new shared body delta requires a simulator projection"),
                    }
                }
                Packet::Appear(Appearance {
                    id: s.id,
                    profile: Profile {
                        name: latin1(s.name),
                        skin: s.skin,
                        custom_skin: s.custom_skin.to_vec(),
                        cosmetic: s.cosmetic,
                    },
                    heading: scalar::angle24(s.heading),
                    wanted: scalar::angle24(s.wanted_heading),
                    speed: scalar::full_speed(s.speed),
                    fam: scalar::fraction24(s.fraction),
                    head: (scalar::position5(s.head[0]), scalar::position5(s.head[1])),
                    first: s
                        .body
                        .first
                        .map(|[x, y]| (scalar::position5(x), scalar::position5(y))),
                    deltas,
                    iang,
                })
            }
            v19::Message::SnakeRemoval(s) => Packet::Remove(s.id, s.raw_status.map(|b| b == 1)),
            v19::Message::Anchor(a) => Packet::Anchor {
                target: target(a.target),
                iang: a.direction,
                absolute: a.absolute.map(|[x, y]| (x, y)),
                fam: a.fraction.map(scalar::fraction24),
            },
            v19::Message::Motion(m) => Packet::Motion {
                target: target(m.target),
                heading: m.heading.map(scalar::angle8),
                wanted: m.wanted_heading.map(scalar::angle8),
                speed: m.speed.map(scalar::compact_speed),
            },
            v19::Message::Mass { id, fraction } => Packet::Fam(id, scalar::fraction24(fraction)),
            v19::Message::Tail { id, fraction } => {
                Packet::Tail(id, fraction.map(scalar::fraction24))
            }
            v19::Message::ScoreAdjustment(credit) => Packet::Credit(credit),
            v19::Message::FoodSector(s) => Packet::Sector(s),
            v19::Message::FoodSpawn(f) => Packet::Pellet(
                f.sector.map(|[x, y]| (x, y)),
                f.offset[0],
                f.offset[1],
                f64::from(f.size) / 5.,
            ),
            v19::Message::FoodEaten(f) => {
                Packet::Eaten(f.sector.map(|[x, y]| (x, y)), f.offset[0], f.offset[1])
            }
            v19::Message::SectorClear([x, y]) => Packet::ClearSector(x, y),
            // The simulator historically projects the first prey angle as heading,
            // unlike the presentation lane. Keep that order and the TAU expression.
            v19::Message::PreyAppearance(p) => Packet::PreyAppear(
                p.id,
                (
                    scalar::position5(p.position[0]),
                    scalar::position5(p.position[1]),
                ),
                f64::from(p.size) / 5.,
                scalar::angle24(p.wanted_heading),
                scalar::angle24(p.heading),
                scalar::full_speed(p.speed),
            ),
            v19::Message::PreyRemoval { id, .. } => Packet::PreyRemove(id),
            v19::Message::PreyMotion(p) => Packet::PreyMove {
                id: p.id,
                position: (
                    scalar::prey_position(p.position[0]),
                    scalar::prey_position(p.position[1]),
                ),
                heading: p.heading.map(scalar::angle24),
                wanted: p.wanted_heading.map(scalar::angle24),
                speed: p.speed.map(scalar::full_speed),
            },
            v19::Message::Leaderboard(l) => Packet::Leaderboard {
                own_position: l.local_row(),
                rank: l.rank(),
                player_count: l.population(),
                rows: l
                    .rows()
                    .map(|r| {
                        (
                            r.count(),
                            scalar::fraction24(r.raw_fraction()),
                            r.raw_color(),
                            latin1(r.name()),
                        )
                    })
                    .collect(),
            },
            v19::Message::Announcement(a) => Packet::Record {
                segments: a.count(),
                fam: scalar::fraction24(a.raw_fraction()),
                name: latin1(a.name()),
                message: latin1(a.message()),
            },
            v19::Message::Session {
                server_id,
                session_id,
            } => Packet::Session(server_id, session_id),
            v19::Message::TeamScores(s) if !s.is_history() => {
                Packet::TeamScore(s.scores().next().unwrap())
            }
            v19::Message::TeamScores(s) => Packet::TeamHistory(s.scores().collect()),
            v19::Message::Kills { id, count } => Packet::Kills(id, count),
            v19::Message::Pong(_) => Packet::Pong,
            v19::Message::LifeControl { .. } => Packet::GameOver,
            v19::Message::Radius(radius) => Packet::Radius(f64::from(radius)),
            v19::Message::Rules(rules) => Packet::Rules(super::rules_packet::from_wire(rules)?),
            _ => Packet::Ignored,
        },
    )
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
    /// Head after completed motion intervals, in their original addition order.
    base: Point,
    base_server: f64,
    heading: f64,
    speed: f64,
    profile: Profile,
    appeared_at: f64,
    /// Last `(count, receipt)` kill report for this appearance; the stock client
    /// starts every new appearance from an unreported count.
    kills: Option<(u32, f64)>,
}
impl Side {
    /// Completed reports plus the current report's unfinished interval.
    fn head_at(&self, server: f64) -> Point {
        let mut head = self.base;
        let dt = (server - self.base_server).max(0.);
        head.x += self.heading.cos() * self.speed * dt;
        head.y += self.heading.sin() * self.speed * dt;
        head
    }
    fn restart(&mut self, point: Point, server: f64, heading: f64, speed: f64) {
        self.anchor_server = server;
        self.base = point;
        self.base_server = server;
        self.heading = heading;
        self.speed = speed;
    }
    fn report(&mut self, server: f64, heading: f64, speed: f64) {
        // apply rejects backwards server clocks. Earlier intervals are complete,
        // so fold each once, preserving the replay's arithmetic and addition
        // order (including equal-time reports). Polling then integrates only the
        // last interval. Anchors still reset the prediction and receipt clock.
        // research/performance/2026-10-28-client-frame-materialization/REPORT.md.
        self.base = self.head_at(server);
        self.base_server = server;
        self.heading = heading;
        self.speed = speed;
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
    rules: Option<crate::WorldRules>,
    sides: FastMap<u16, Side>,
    free_slots: Vec<usize>,
    slot_count: usize,
    self_id: Option<u16>,
    own_dead: bool,
    credit: u8,
    food: BTreeMap<u32, Food>,
    /// Ordered materialization, invalidated by every food edit. Frames own copies.
    food_frame: OnceLock<Vec<Food>>,
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
    incarnations: FastMap<u16, u64>,
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
            initial_raw_framing(bytes)
        } else {
            self.session.raw_framing
        };
        let frame = if raw {
            decode_payload(bytes)?
        } else {
            if bytes.len() < 3 {
                return Err("Short timing frame".into());
            }
            decode_payload(&bytes[2..])?
        };
        let setup = frame.commands().any(|command| command.bytes[0] == b'a');
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
        self.apply(
            frame.commands().map(|command| command.bytes),
            server,
            receipt_seconds,
        )?;
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
        self.apply(std::iter::once(packet), server_seconds, receipt_seconds)
    }

    fn apply<'a>(
        &mut self,
        packets: impl Iterator<Item = &'a [u8]> + Clone,
        server: f64,
        receipt: f64,
    ) -> Result<(), String> {
        let parsed = packets.clone().map(parse).collect::<Result<Vec<_>, _>>()?;
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
        let mut minimap_cells = MAX_FRAME_MINIMAP_CELLS;
        for (index, packet) in parsed.iter().enumerate() {
            match packet {
                Packet::Setup(_) if has_setup => {
                    return Err("Repeated setup on one connection".into());
                }
                Packet::Setup(_) => has_setup = true,
                _ if packet.needs_setup() && !has_setup => {
                    let raw = packets.clone().nth(index).expect("parsed command index");
                    return Err(format!("'{}' packet before setup", char::from(raw[0])));
                }
                Packet::Minimap(payload) => {
                    minimap
                        .get_or_insert_with(|| self.session.minimap.clone())
                        .apply(payload, receipt, &mut minimap_cells)?;
                }
                _ => {}
            }
        }
        self.validate_retention(&parsed)?;
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

    /// Simulate retained sizes before committing any packet in a frame. The
    /// food overlay touches only updated IDs, rather than copying all pellets.
    fn validate_retention(&self, packets: &[Packet<'_>]) -> Result<(), String> {
        if !packets.iter().any(|p| {
            matches!(
                p,
                Packet::Appear(_)
                    | Packet::Anchor { fam: Some(_), .. }
                    | Packet::Sector(..)
                    | Packet::Pellet(..)
            )
        }) {
            return Ok(());
        }
        let mut lengths: Vec<_> = (0..self.session.slot_count)
            .map(|slot| self.tracker.snake(slot).map_or(0, |snake| snake.body.len()))
            .collect();
        let mut total: usize = lengths.iter().sum();
        let mut bodies: FastMap<_, _> = self
            .session
            .sides
            .iter()
            .map(|(&id, side)| {
                (
                    id,
                    BodyBudget {
                        slot: side.slot,
                        known: side.body_known,
                        direction: side.iang.is_some(),
                        segments: self.tracker.snake(side.slot).map_or(0, |s| s.segments),
                    },
                )
            })
            .collect();
        let mut free = self.session.free_slots.clone();
        let mut own = self.session.self_id;
        let mut own_dead = self.session.own_dead;
        let mut last_food = self.session.last_food_sector;
        let mut last_eaten = self.session.last_eaten_sector;
        let mut food = FoodBudget {
            base: &self.session.food,
            updates: BTreeMap::new(),
            cleared_sectors: FastSet::default(),
            count: self.session.food.len(),
        };
        for packet in packets {
            match packet {
                Packet::Appear(a) => {
                    let length = if a.first.is_some() {
                        a.deltas.len() + 1 + usize::from(a.iang.is_some())
                    } else {
                        0
                    };
                    let slot = if let Some(body) = bodies.get(&a.id) {
                        body.slot
                    } else if let Some(slot) = free.pop() {
                        slot
                    } else {
                        if lengths.len() == MAX_RECEIVED_SNAKES {
                            return Err("Received snake limit exceeded".into());
                        }
                        lengths.push(0);
                        lengths.len() - 1
                    };
                    total = total - lengths[slot] + length;
                    lengths[slot] = length;
                    bodies.insert(
                        a.id,
                        BodyBudget {
                            slot,
                            known: true,
                            direction: a.iang.is_some(),
                            segments: length,
                        },
                    );
                    own.get_or_insert(a.id);
                    if length > MAX_RECEIVED_SNAKE_POINTS {
                        return Err("Received snake body limit exceeded".into());
                    }
                }
                Packet::Remove(id, _) => {
                    if let Some(body) = bodies.remove(id) {
                        free.push(body.slot);
                    }
                    if own == Some(*id) {
                        own_dead = true;
                    }
                }
                Packet::GameOver => own_dead = true,
                Packet::Anchor {
                    target,
                    iang,
                    absolute,
                    fam,
                } => {
                    let id = match target {
                        Target::Id(id) => Some(*id),
                        Target::Own => own.filter(|_| !own_dead),
                    };
                    if let Some(body) = id.and_then(|id| bodies.get_mut(&id)) {
                        body.direction |= iang.is_some();
                        let length = &mut lengths[body.slot];
                        let previous = *length;
                        if body.known {
                            if absolute.is_some() || (*length > 0 && body.direction) {
                                *length += usize::from(fam.is_some());
                            } else {
                                *length = 0;
                                body.known = false;
                            }
                        }
                        body.segments += usize::from(fam.is_some());
                        total = total - previous + *length;
                        if *length > MAX_RECEIVED_SNAKE_POINTS
                            || body.segments > MAX_RECEIVED_SNAKE_POINTS
                        {
                            return Err("Received snake body limit exceeded".into());
                        }
                    }
                }
                Packet::Tail(id, _) => {
                    if let Some(body) = bodies.get_mut(id) {
                        if body.known && lengths[body.slot] > 0 {
                            lengths[body.slot] -= 1;
                            total -= 1;
                        }
                        body.segments = body.segments.saturating_sub(1);
                    }
                }
                Packet::Sector(sector) => {
                    for pellet in sector.foods() {
                        food.set(
                            food_id(
                                sector.sector[0],
                                sector.sector[1],
                                pellet.offset[0],
                                pellet.offset[1],
                            ),
                            true,
                        )?;
                    }
                }
                Packet::Pellet(sector, rx, ry, _) => {
                    if sector.is_some() {
                        last_food = *sector;
                    }
                    if let Some((sx, sy)) = last_food {
                        food.set(food_id(sx, sy, *rx, *ry), true)?;
                    }
                }
                Packet::Eaten(sector, rx, ry) => {
                    if sector.is_some() {
                        last_eaten = *sector;
                    }
                    if let Some((sx, sy)) = last_eaten {
                        food.set(food_id(sx, sy, *rx, *ry), false)?;
                    }
                }
                Packet::ClearSector(sx, sy) => food.clear_sector(*sx, *sy),
                _ => {}
            }
            if total > MAX_RECEIVED_BODY_POINTS {
                return Err("Received total body limit exceeded".into());
            }
        }
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
            Packet::Sector(sector) => {
                session.food_at = Some(receipt);
                for pellet in sector.foods() {
                    self.add_food(
                        (sector.sector[0], sector.sector[1]),
                        pellet.offset[0],
                        pellet.offset[1],
                        f64::from(pellet.size) / 5.,
                    );
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
                if let Some((sx, sy)) = session.last_eaten_sector
                    && session.food.remove(&food_id(sx, sy, rx, ry)).is_some()
                {
                    session.food_frame.take();
                }
            }
            Packet::ClearSector(sx, sy) => {
                session.food_at = Some(receipt);
                // Sector bytes form a contiguous key range. Repeated clears of
                // empty sectors must not scan unrelated retained food.
                let ids: Vec<_> = session
                    .food
                    .range(food_id(sx, sy, 0, 0)..=food_id(sx, sy, 255, 255))
                    .map(|(&id, _)| id)
                    .collect();
                if !ids.is_empty() {
                    session.food_frame.take();
                }
                for id in ids {
                    session.food.remove(&id);
                }
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
                    // Any delay is a measurement: a slow link or congestion can
                    // exceed any fixed window, and the pong carries no RTT.
                    let rtt = receipt - sent;
                    if rtt >= 0. {
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
            Packet::Rules(rules) => session.rules = Some(rules),
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
        self.session.food_frame.take();
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
            heading: a.heading,
            speed: a.speed,
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

    /// Validate a departure before the native backend commits any send state.
    pub(super) fn check_sent(&self, at: f64) -> Result<(), String> {
        self.require_connection()?;
        self.local_clock(at)
    }

    /// Record bytes actually sent. Only game commands are kept; handshake ignored.
    pub fn record_sent(&mut self, packet: &[u8], at: f64) -> Result<(), String> {
        self.check_sent(at)?;
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
                    // Repeated frames copy a contiguous snapshot rather than
                    // walking the same ordered tree. Mutations invalidate it;
                    // visibility, values and order still come only from bytes.
                    // research/performance/2026-10-28-client-frame-materialization/REPORT.md.
                    food: session
                        .food_frame
                        .get_or_init(|| session.food.values().cloned().collect())
                        .clone(),
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
            rules: session.rules,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_motion_matches_bounded_replay_bits() {
        // Independent original replay, including its 256-report oldest fold.
        let replay = |base: Point, base_server: f64, steps: &[(f64, f64, f64)], server: f64| {
            let mut head = base;
            for (i, &(start, heading, speed)) in steps.iter().enumerate() {
                let end = steps.get(i + 1).map_or(server, |step| step.0);
                let dt = (end.min(server) - start.max(base_server)).max(0.);
                head.x += heading.cos() * speed * dt;
                head.y += heading.sin() * speed * dt;
            }
            head
        };
        for (origin, epoch) in [
            (Point { x: -0., y: -0. }, 0.),
            (
                Point {
                    x: 13.25,
                    y: -7.125,
                },
                0.007,
            ),
            (Point { x: 1e12, y: -1e12 }, 1e12),
        ] {
            let mut side = Side {
                slot: 0,
                iang: None,
                body_known: true,
                anchor_server: epoch,
                base: origin,
                base_server: epoch,
                heading: -0.,
                speed: 0.,
                profile: Profile {
                    name: String::new(),
                    skin: 0,
                    custom_skin: Vec::new(),
                    cosmetic: 0,
                },
                appeared_at: 0.,
                kills: None,
            };
            let mut base = origin;
            let mut base_server = epoch;
            let mut steps = vec![(epoch, -0., 0.)];
            let mut server = epoch;
            for index in 0..1100 {
                server += [0., 0.007, 0., 0.042, 0.001][index % 5];
                let heading = index as f64 * 0.0731 - 7.;
                let speed = [0., 120., 203.125, 17.][index % 4];
                side.report(server, heading, speed);
                steps.push((server, heading, speed));
                if steps.len() > 256 {
                    let (start, heading, speed) = steps.remove(0);
                    let next = steps[0].0;
                    let dt = (next - start.max(base_server)).max(0.);
                    base.x += heading.cos() * speed * dt;
                    base.y += heading.sin() * speed * dt;
                    base_server = next;
                }
                for at in [server, server + 0.007, server + 0.5] {
                    let expected = replay(base, base_server, &steps, at);
                    let actual = side.head_at(at);
                    assert_eq!(
                        (actual.x.to_bits(), actual.y.to_bits()),
                        (expected.x.to_bits(), expected.y.to_bits()),
                        "report {index}, server {at}"
                    );
                    let cloned = side.clone().head_at(at);
                    assert_eq!(
                        (actual.x.to_bits(), actual.y.to_bits()),
                        (cloned.x.to_bits(), cloned.y.to_bits())
                    );
                }
                // An anchor resets prediction, including after the old cap.
                if index % 317 == 316 {
                    side.restart(origin, server, heading, speed);
                    base = origin;
                    base_server = server;
                    steps = vec![(server, heading, speed)];
                }
            }
        }
    }
}
