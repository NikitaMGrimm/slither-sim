//! Bounded received scene state for a documented subset of protocol 19.
//!
//! This original presentation model reads transmitted anchors, not authoritative
//! physics. Bodies are head-to-tail; coordinates are centered on setup's grid
//! center. It does not extrapolate motion or reproduce body contraction. Unknown
//! packet families (including HUD, minimap and game-over) are counted and
//! ignored without claiming to decode them. Canonical prey packets emit ordered
//! presentation inputs without retaining a received prey map.

use crate::{
    math,
    presentation::{HudIngressError, HudIngressPlan, HudLeaderboardRows, HudLeaderboardRowsBudget},
    protocol::{self, ElapsedHeader, ProtocolError},
};
use std::{collections::BTreeMap, fmt};

use slither_protocol::{
    DecodeAdmission, ErrorKind,
    frame::CommandRef,
    scalar,
    v19::{self, Message},
};

/// Maximum simultaneously retained snakes.
pub const MAX_SNAKES: usize = 256;
/// Maximum anchors per snake, an independent scene retention policy.
pub const MAX_BODY_POINTS: usize = 4096;
/// Maximum total retained anchors across snakes.
pub const MAX_TOTAL_BODY_POINTS: usize = 65_536;
/// Maximum simultaneously retained food positions.
pub const MAX_FOODS: usize = 65_536;
/// Absolute bound on each centered coordinate and advertised radius.
pub const MAX_COORDINATE: f64 = 16_777_215.0;

/// A centered world position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    /// Horizontal position.
    pub x: f64,
    /// Vertical position.
    pub y: f64,
}

/// Typed transmitted world configuration. No physics constants are inferred.
#[derive(Clone, Debug, PartialEq)]
pub struct Setup {
    /// Advertised protocol; this module accepts only 19.
    pub protocol: u8,
    /// Offset subtracted from wire positions on both axes.
    pub grid_center: f64,
    /// Advertised arena radius, or 0.98 times grid center when absent.
    pub radius: f64,
    /// Sector side length in world units.
    pub sector_size: u16,
    /// Advertised sector count along an edge.
    pub sector_count: u16,
    /// Maximum segment score-table index (`mscps`), not a physical body cap.
    pub max_segments: u16,
    /// Distance between directed anchors.
    pub body_spacing: f64,
    /// Advertised contraction coefficient; retained but not simulated.
    pub body_contraction: f64,
    /// Advertised speed-angle divisor.
    pub spangdv: f64,
    /// Advertised base speed coefficient.
    pub nsp1: f64,
    /// Advertised size speed coefficient.
    pub nsp2: f64,
    /// Advertised boost speed coefficient.
    pub nsp3: f64,
    /// Advertised turn coefficient.
    pub mamu: f64,
    /// Advertised secondary turn coefficient.
    pub mamu2: f64,
    /// Optional server number; never used as the own snake identifier.
    pub server_number: Option<u16>,
    /// Optional mode byte.
    pub game_mode: Option<u8>,
    /// Optional team byte when mode is 2.
    pub team: Option<u8>,
}

/// A received snake and its original presentation fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Snake {
    /// Wire identifier.
    pub id: u16,
    /// Wire skin index.
    pub skin: u8,
    /// Name decoded one byte per Unicode scalar (Latin-1).
    pub name: String,
    /// Opaque custom appearance bytes, bounded by their one-byte length.
    pub custom_skin: Vec<u8>,
    /// Cosmetic marker.
    pub cosmetic: u8,
    /// Latest transmitted head or front anchor.
    pub head: Point,
    /// Received anchors, head to tail. No contraction/interpolation is applied.
    pub body: Vec<Point>,
    /// Reported heading in radians.
    pub heading: f64,
    /// Reported desired heading in radians.
    pub wanted_heading: f64,
    /// Reported speed in world units per second (wire time unit is 32 ms).
    pub speed: f64,
    /// Fractional mass report, in the inclusive range 0 through 1.
    pub fam: f64,
    /// Fixed six-unit presentation radius, not a decoded physical hitbox.
    pub radius: f64,
    anchor_direction: Option<u16>,
    segment_length: f64,
}

/// Latest received food at one key, without retained multiplicity or animation.
#[derive(Clone, Debug, PartialEq)]
pub struct Food {
    /// Centered world position.
    pub position: Point,
    /// Transmitted size in world units.
    pub size: f64,
    /// Raw color: `F` supplies it directly; `f/b` omissions reuse their shared cache.
    /// The cache starts at zero; explicit reset and first own appearance restore it.
    pub color: u8,
    /// Owning sector coordinates.
    pub sector: (u8, u8),
}

/// Outcome of one committed frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameReport {
    /// Structurally valid commands, including unsupported ones.
    pub commands: usize,
    /// Unsupported command families, not decoded by this module.
    pub ignored: usize,
}

/// Transmitted turning direction, independent of an omitted direction update.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    /// No active turn.
    None,
    /// Turn toward decreasing angles.
    Decreasing,
    /// Turn toward increasing angles.
    Increasing,
}

/// Preserve compact own targeting even after removal from the received ID map.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SnakeTarget {
    /// The retained own presentation object, which may be dying.
    Own,
    /// An explicitly addressed live snake.
    Id(u16),
}

/// Complete initial presentation inputs, separate from centered received state.
///
/// Absolute geometry preserves wire arithmetic without a centering round trip.
/// Body points are head to tail. A final directed body segment supplies the
/// effective initial heading; otherwise the header heading is used. Initial
/// active turning is `Direction::None`, irrespective of `wire_direction`.
#[derive(Clone, Debug, PartialEq)]
pub struct Appearance {
    /// Wire identifier, including zero.
    pub id: u16,
    /// Wire skin index.
    pub skin: u8,
    /// Latin-1 nickname.
    pub name: String,
    /// Opaque custom appearance bytes.
    pub custom_skin: Vec<u8>,
    /// Cosmetic marker.
    pub cosmetic: u8,
    /// Absolute transmitted head position.
    pub absolute_head: Point,
    /// Absolute body anchors, decoded before centering.
    pub absolute_body: Vec<Point>,
    /// Final directed body heading in wire units, for later compact anchors.
    pub anchor_direction: Option<u16>,
    /// Effective initial heading in radians.
    pub heading: f64,
    /// Transmitted wanted heading in radians.
    pub wanted_heading: f64,
    /// Exact wire speed scalar before conversion to world units per second.
    pub speed_per_32ms: f64,
    /// Received fractional mass.
    pub fam: f64,
    /// Validated header metadata; this does not start an active turn.
    pub wire_direction: Direction,
}

/// One food creation, independent of the lossy unique-key received map.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FoodSpawnInput {
    /// Packed sector and offset bytes, preserving the complete wire bit pattern.
    pub key: u32,
    /// Original sector coordinates, independent of later animation.
    pub sector: (u8, u8),
    /// Absolute world position, decoded before centering.
    pub absolute_position: Point,
    /// Original size byte, including zero.
    pub size_byte: u8,
    /// Original size byte divided by five in world units.
    pub base_size: f64,
    /// Raw explicit or cached color, before presentation palette selection.
    pub raw_color: u8,
    /// True for `F` and `b`; false for the slower `f` appearance.
    pub rapid: bool,
}

/// One prey appearance, before instance ownership or source selection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreySpawnInput {
    /// Wire identifier, including zero and duplicates.
    pub id: u16,
    /// Absolute wire position, independent of centered received state.
    pub absolute_position: Point,
    /// Original size byte, including zero.
    pub size_byte: u8,
    /// Original size byte divided by five.
    pub base_size: f64,
    /// Raw palette byte, before modulo-nine selection.
    pub raw_color: u8,
    /// Active initial turn, unlike snake appearance direction metadata.
    pub direction: Direction,
    /// Transmitted heading, including its possible full-turn endpoint.
    pub heading: f64,
    /// Transmitted target heading.
    pub wanted_heading: f64,
    /// Exact wire speed scalar in its 32 ms unit.
    pub speed_per_32ms: f64,
}

/// A prey reanchor with independently present motion fields.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreyMotionInput {
    /// Wire identifier of the newest matching retained appearance.
    pub id: u16,
    /// Absolute packet position before presentation correction.
    pub absolute_position: Point,
    /// Omission preserves the active turn.
    pub direction: Option<Direction>,
    /// Omission preserves the heading.
    pub heading: Option<f64>,
    /// Omission preserves the target heading.
    pub wanted_heading: Option<f64>,
    /// A present zero speed is distinct from omission.
    pub speed_per_32ms: Option<f64>,
}

/// A validated presentation change in wire order, without advancing any clock.
#[derive(Clone, Debug, PartialEq)]
pub enum PresentationEvent {
    /// Apply configuration while preserving entities, own identity, caches and camera state.
    Setup(Setup),
    /// Complete authored-admitted minimap command, only in the enabled lane.
    /// V run reachability is checked against retained size during presentation.
    Minimap(Vec<u8>),
    /// Replace retained original-own R; emitted only by opted-in score ingestion.
    OwnScoreAdjustment(u8),
    /// Create or replace a snake with complete initial presentation inputs.
    SnakeAppearance(Appearance),
    /// Apply only the motion fields present in this command.
    SnakeMotion {
        /// Explicit live ID or the retained own presentation object.
        target: SnakeTarget,
        /// Optional heading in radians.
        heading: Option<f64>,
        /// Optional wanted heading in radians.
        wanted_heading: Option<f64>,
        /// Optional turning direction; omission preserves the previous value.
        direction: Option<Direction>,
        /// Optional exact wire speed scalar.
        speed_per_32ms: Option<f64>,
    },
    /// Add a received front anchor, with optional growth and fractional mass.
    SnakeAnchor {
        /// Explicit live ID or the retained own presentation object.
        target: SnakeTarget,
        /// Absolute wire position, omitted by relative anchor commands.
        absolute_position: Option<Point>,
        /// Explicit wire direction; omission reuses presentation's cached value.
        direction: Option<u16>,
        /// Whether this anchor grows the body.
        grew: bool,
        /// Optional fractional mass accompanying growth.
        fam: Option<f64>,
    },
    /// Replace fractional mass independently of geometry.
    SnakeMass {
        /// Known wire identifier.
        id: u16,
        /// Received fractional mass.
        fam: f64,
    },
    /// Remove a tail anchor, optionally replacing fractional mass.
    SnakeTailRemoved {
        /// Known wire identifier.
        id: u16,
        /// Optional received fractional mass.
        fam: Option<f64>,
    },
    /// Request removal by ID, even when absent from the received-state map.
    /// Each request can reach a retained duplicate presentation object.
    SnakeRemoved {
        /// Wire identifier requested for removal.
        id: u16,
        /// True only when the wire removal flag equals one.
        died: bool,
    },
    /// Replace the target arena radius, without advancing displayed easing.
    RadiusTarget(f64),
    /// Append one food instance, including repeated keys and equal creations.
    FoodSpawn(FoodSpawnInput),
    /// Consume the newest matching retained instance, even if the received map is empty.
    FoodConsume {
        /// Packed sector and offset bytes.
        key: u32,
        /// Explicit or cached eater ID, including zero; absent means immediate removal.
        eater_id: Option<u16>,
    },
    /// Clear all retained instances from a sector, including consumed foods.
    FoodSectorClear {
        /// Original sector coordinates.
        sector: (u8, u8),
    },
    /// Append a prey appearance without collapsing duplicate wire identifiers.
    PreySpawn(PreySpawnInput),
    /// Reanchor the newest retained matching prey.
    PreyMotion(PreyMotionInput),
    /// Remove the newest matching prey immediately.
    PreyRemoved {
        /// Wire identifier, even if absent from the presentation world.
        id: u16,
    },
    /// Capture the newest matching prey using event-time eater lookup.
    PreyEaten {
        /// Prey wire identifier.
        id: u16,
        /// Explicit eater wire identifier, including zero.
        eater_id: u16,
    },
}

/// A completely committed frame and its ordered presentation changes.
#[derive(Clone, Debug, PartialEq)]
pub struct AppliedFrame {
    /// Received-state command counts.
    pub report: FrameReport,
    /// Changes published only after the entire frame succeeds.
    pub events: Vec<PresentationEvent>,
}

/// Private ranges preserve wire positions even when commands emit no events.
pub(crate) struct HudCommandBoundary {
    pub event_end: usize,
    pub marker: HudCommandMarker,
}

pub(crate) enum HudCommandMarker {
    Other,
    Leaderboard {
        local_row: u8,
        rank: u16,
        population: u16,
        death_pending: bool,
        rows: Option<HudLeaderboardRows>,
    },
    Announcement(crate::presentation::AnnouncementPacket),
    FinalCapture,
}

pub(crate) struct HudCommandCollector<'a> {
    pub plan: HudIngressPlan<'a>,
    pub announcements: bool,
    pub boundaries: Vec<HudCommandBoundary>,
}

/// A malformed or out-of-policy frame never changes the scene or counters.
#[derive(Clone, Debug, PartialEq)]
pub enum StateError {
    /// An enabled minimap command has invalid fixed or reachable wire syntax.
    Minimap(protocol::minimap::MinimapDecodeError),
    /// Explicit HUD ingress plan, canonical controls or complete leaderboard failed.
    Hud(HudIngressError),
    /// Existing framing or setup-boundary validation failed.
    Protocol(ProtocolError),
    /// A supported command has an invalid length or scalar value.
    InvalidPacket(u8),
    /// A world command arrived before setup.
    MissingSetup,
    /// The advertised protocol is unsupported.
    UnsupportedProtocol(u8),
    /// A bounded entity, body, or food collection would overflow.
    Limit,
    /// A received/reconstructed position exceeds the coordinate bound.
    Coordinate,
    /// A compact food packet needs a previous sector in its own stream.
    MissingSector,
    /// A compact anchor lacks a previous anchor or direction.
    MissingAnchor(u16),
}

impl fmt::Display for StateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Minimap(error) => error.fmt(f),
            Self::Hud(error) => error.fmt(f),
            Self::Protocol(error) => error.fmt(f),
            Self::InvalidPacket(code) => {
                write!(f, "invalid supported '{}' packet", char::from(*code))
            }
            Self::MissingSetup => f.write_str("world command requires setup"),
            Self::UnsupportedProtocol(version) => write!(f, "unsupported scene protocol {version}"),
            Self::Limit => f.write_str("scene collection limit exceeded"),
            Self::Coordinate => f.write_str("scene coordinate exceeds bound"),
            Self::MissingSector => f.write_str("compact food packet has no previous sector"),
            Self::MissingAnchor(id) => write!(f, "snake {id} has no known anchor direction"),
        }
    }
}
impl std::error::Error for StateError {}
impl From<ProtocolError> for StateError {
    fn from(error: ProtocolError) -> Self {
        Self::Protocol(error)
    }
}

/// Deterministically ordered received scene, independent of browser and network I/O.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClientState {
    /// Current configuration. Setup preserves existing entities and compact caches.
    pub setup: Option<Setup>,
    /// Current snakes, in wire identifier order.
    pub snakes: BTreeMap<u16, Snake>,
    /// Lossy unique-key projection in packed sector/offset order. Creation replaces
    /// the latest value and consumption removes it immediately. This map cannot
    /// represent duplicate instances, insertion order, or consumption animation.
    pub foods: BTreeMap<u32, Food>,
    /// First full appearance in this world; retained through removal and setup.
    pub own_snake_id: Option<u16>,
    /// Successfully committed frames; saturates rather than overflowing.
    pub frames_applied: u64,
    /// Commands in committed frames, including ignored commands; saturates.
    pub commands_applied: u64,
    /// Unsupported commands in committed frames; saturates.
    pub ignored_commands: u64,
    /// Latest committed frame's explicitly selected elapsed header.
    pub last_elapsed_ms: Option<u16>,
    added_sector: Option<(u8, u8)>,
    eaten_sector: Option<(u8, u8)>,
    added_color: u8,
    eater_id: Option<u16>,
}

impl ClientState {
    /// Validate framing, then stage all supported decoding and changes atomically.
    ///
    /// Accepted world families: `s`, `g/n/G/N/=/+`, `e/E/3/4/5/d/7`,
    /// `h/r`, `F/f/b/c/C/<`, `w`, prey `j/y`, and radius `z`; setup is `a`.
    /// Unknown snake updates are validated then ignored. Unsupported families
    /// are opaque and counted even before setup. No clocks or simulation advance.
    /// Prey accepts only canonical lengths and ASCII directions after setup;
    /// these packets are supported even without a received prey map.
    pub fn apply_frame(
        &mut self,
        bytes: &[u8],
        header: ElapsedHeader,
    ) -> Result<FrameReport, StateError> {
        self.apply_transaction(bytes, header, None, false, false, None)
    }

    /// Atomically apply a frame and return every supported presentation change.
    ///
    /// Events retain wire order, including replacements and mid-frame configuration.
    /// Removal requests always emit, including unknown or already removed IDs,
    /// so retained presentation duplicates remain reachable. Every food creation,
    /// consume and sector clear emits directly in command/record order, including
    /// unknown keys. Every canonical prey command emits, including unknown IDs and
    /// duplicates; other unknown explicit-ID updates emit no events. Compact own
    /// commands retain `Own` targeting after received-state removal and setup. Elapsed
    /// headers never advance presentation here.
    /// Appearance direction metadata must be ASCII `0`, `1` or `2`; received-only
    /// `apply_frame` keeps ignoring this field for compatibility.
    pub fn apply_frame_with_events(
        &mut self,
        bytes: &[u8],
        header: ElapsedHeader,
    ) -> Result<AppliedFrame, StateError> {
        let mut events = Vec::new();
        let report =
            self.apply_transaction(bytes, header, Some(&mut events), false, false, None)?;
        Ok(AppliedFrame { report, events })
    }

    /// Canonical score admission is private to the explicitly enabled native client.
    pub(crate) fn apply_frame_with_score_events(
        &mut self,
        bytes: &[u8],
        header: ElapsedHeader,
    ) -> Result<AppliedFrame, StateError> {
        let mut events = Vec::new();
        let report = self.apply_transaction(bytes, header, Some(&mut events), true, false, None)?;
        Ok(AppliedFrame { report, events })
    }

    pub(crate) fn apply_frame_with_hud_events(
        &mut self,
        bytes: &[u8],
        header: ElapsedHeader,
        collector: &mut HudCommandCollector<'_>,
    ) -> Result<AppliedFrame, StateError> {
        let mut events = Vec::new();
        let report = self.apply_transaction(
            bytes,
            header,
            Some(&mut events),
            true,
            false,
            Some(collector),
        )?;
        Ok(AppliedFrame { report, events })
    }

    /// Minimap admission shares ordinary/score/HUD staging and event positions.
    /// Selecting minimaps never implicitly admits score-adjustment commands.
    pub(crate) fn apply_frame_with_minimap_events(
        &mut self,
        bytes: &[u8],
        header: ElapsedHeader,
        score_enabled: bool,
        collector: Option<&mut HudCommandCollector<'_>>,
    ) -> Result<AppliedFrame, StateError> {
        let mut events = Vec::new();
        let report = self.apply_transaction(
            bytes,
            header,
            Some(&mut events),
            score_enabled,
            true,
            collector,
        )?;
        Ok(AppliedFrame { report, events })
    }

    fn apply_transaction(
        &mut self,
        bytes: &[u8],
        header: ElapsedHeader,
        events: Option<&mut Vec<PresentationEvent>>,
        score_enabled: bool,
        minimap_enabled: bool,
        hud: Option<&mut HudCommandCollector<'_>>,
    ) -> Result<FrameReport, StateError> {
        self.apply_transaction_inner(
            bytes,
            header,
            events,
            score_enabled,
            minimap_enabled,
            hud,
            #[cfg(feature = "feel-trace")]
            None,
        )
    }

    #[cfg(feature = "feel-trace")]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_frame_with_setup_packet_heads(
        &mut self,
        bytes: &[u8],
        header: ElapsedHeader,
        score_enabled: bool,
        minimap_enabled: bool,
        hud: Option<&mut HudCommandCollector<'_>>,
        heads: &mut ([u16; 2], Vec<crate::feel_trace::PacketHeads>),
    ) -> Result<AppliedFrame, StateError> {
        let mut events = Vec::new();
        let report = self.apply_transaction_inner(
            bytes,
            header,
            Some(&mut events),
            score_enabled,
            minimap_enabled,
            hud,
            Some(heads),
        )?;
        Ok(AppliedFrame { report, events })
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_transaction_inner(
        &mut self,
        bytes: &[u8],
        header: ElapsedHeader,
        mut events: Option<&mut Vec<PresentationEvent>>,
        score_enabled: bool,
        minimap_enabled: bool,
        mut hud: Option<&mut HudCommandCollector<'_>>,
        #[cfg(feature = "feel-trace")] mut setup_heads: Option<&mut (
            [u16; 2],
            Vec<crate::feel_trace::PacketHeads>,
        )>,
    ) -> Result<FrameReport, StateError> {
        let decoded = protocol::decode_frame(bytes, header)?;
        if let Some(collector) = hud.as_ref() {
            collector
                .plan
                .validate(decoded.commands())
                .map_err(StateError::Hud)?;
        }
        if let Some(collector) = hud.as_deref_mut() {
            collector
                .boundaries
                .try_reserve_exact(decoded.command_count())
                .map_err(|_| StateError::Hud(HudIngressError::LeaderboardRowsResource))?;
        }
        let mut staged = self.clone();
        let mut ignored = 0;
        let mut captures = 0;
        let mut row_budget = HudLeaderboardRowsBudget::default();
        for command_ref in decoded.commands() {
            let index = command_ref.index;
            let command = command_ref.bytes;
            let mut marker = HudCommandMarker::Other;
            let hud_leaderboard = hud.is_some() && command[0] == b'l';
            if let Some(collector) = hud.as_ref() {
                if hud_leaderboard {
                    // Opt-in admission deliberately validates ignored-playing tails too.
                    let header = protocol::hud::parse_leaderboard(command)
                        .map_err(|error| StateError::Hud(HudIngressError::Leaderboard(error)))?;
                    let guard = collector.plan.guards[index];
                    if guard.playing {
                        marker = HudCommandMarker::Leaderboard {
                            local_row: header.local_row(),
                            rank: header.rank(),
                            population: header.population(),
                            death_pending: guard.death_pending,
                            rows: if collector.plan.schema == 2 {
                                Some(
                                    row_budget
                                        .copy_board(index, header)
                                        .map_err(StateError::Hud)?,
                                )
                            } else {
                                None
                            },
                        };
                    }
                } else if collector.announcements && command[0] == b'm' {
                    marker = HudCommandMarker::Announcement(
                        crate::presentation::AnnouncementPacket::copy(command).map_err(
                            |error| StateError::Hud(HudIngressError::Announcement(error)),
                        )?,
                    );
                } else if collector.plan.final_capture_indices.get(captures) == Some(&index) {
                    marker = HudCommandMarker::FinalCapture;
                    captures += 1;
                }
            }
            if minimap_enabled && matches!(command[0], b'U' | b'L' | b'M' | b'V' | b'u') {
                if staged.setup.is_none() {
                    return Err(StateError::MissingSetup);
                }
                protocol::minimap::parse_minimap(command).map_err(StateError::Minimap)?;
                if let Some(events) = events.as_deref_mut() {
                    events.push(PresentationEvent::Minimap(command.to_vec()));
                }
            } else if !hud_leaderboard
                && !staged.apply_command(command_ref, events.as_deref_mut(), score_enabled)?
            {
                ignored += 1;
            }
            if let Some(collector) = hud.as_deref_mut() {
                collector.boundaries.push(HudCommandBoundary {
                    event_end: events.as_ref().map_or(0, |events| events.len()),
                    marker,
                });
            }
            #[cfg(feature = "feel-trace")]
            if command[0] == b'a'
                && let Some(heads) = setup_heads.as_deref_mut()
            {
                heads
                    .1
                    .push(crate::feel_trace::PacketHeads::capture(&staged, heads.0));
            }
        }
        staged.frames_applied = staged.frames_applied.saturating_add(1);
        staged.commands_applied = staged
            .commands_applied
            .saturating_add(decoded.command_count() as u64);
        staged.ignored_commands = staged.ignored_commands.saturating_add(ignored as u64);
        staged.last_elapsed_ms = decoded.prefix();
        let report = FrameReport {
            commands: decoded.command_count(),
            ignored,
        };
        *self = staged;
        Ok(report)
    }

    fn apply_command(
        &mut self,
        command: CommandRef<'_>,
        events: Option<&mut Vec<PresentationEvent>>,
        score_enabled: bool,
    ) -> Result<bool, StateError> {
        let p = command.bytes;
        let code = p[0];
        // Disabled lanes remain opaque, including malformed bodies.
        if !(score_enabled && code == b'R')
            && !matches!(
                code,
                b'a' | b's'
                    | b'g'
                    | b'n'
                    | b'G'
                    | b'N'
                    | b'='
                    | b'+'
                    | b'e'
                    | b'E'
                    | b'3'
                    | b'4'
                    | b'5'
                    | b'd'
                    | b'7'
                    | b'h'
                    | b'r'
                    | b'F'
                    | b'f'
                    | b'b'
                    | b'c'
                    | b'C'
                    | b'<'
                    | b'w'
                    | b'j'
                    | b'y'
                    | b'z'
            )
        {
            return Ok(false);
        }
        if code != b'a' && code != b'R' && self.setup.is_none() {
            return Err(StateError::MissingSetup);
        }
        let admission = if events.is_some() {
            DecodeAdmission::ClientPresentation
        } else {
            DecodeAdmission::ClientReceived
        };
        let message = v19::decode_command(command, admission).map_err(|error| {
            if error.kind == ErrorKind::UnsupportedVersion {
                StateError::UnsupportedProtocol(
                    v19::setup_protocol_version(p, admission)
                        .expect("decoder validated setup boundaries"),
                )
            } else {
                StateError::InvalidPacket(code)
            }
        })?;
        if let Message::ScoreAdjustment(value) = message {
            if let Some(events) = events {
                events.push(PresentationEvent::OwnScoreAdjustment(value));
            }
            return Ok(true);
        }
        if let Message::Setup(raw) = message {
            let setup = setup_from_wire(raw);
            if let Some(previous) = &self.setup
                && previous.grid_center != setup.grid_center
            {
                // The received preview is centered; retained presentation stays absolute.
                let old_center = previous.grid_center;
                for snake in self.snakes.values_mut() {
                    snake.head =
                        centered(&setup, snake.head.x + old_center, snake.head.y + old_center)?;
                    for point in &mut snake.body {
                        *point = centered(&setup, point.x + old_center, point.y + old_center)?;
                    }
                }
                for food in self.foods.values_mut() {
                    food.position = centered(
                        &setup,
                        food.position.x + old_center,
                        food.position.y + old_center,
                    )?;
                }
            }
            if let Some(events) = events {
                events.push(PresentationEvent::Setup(setup.clone()));
            }
            self.setup = Some(setup);
            return Ok(true);
        }
        let setup = self.setup.as_ref().ok_or(StateError::MissingSetup)?.clone();
        match message {
            Message::SnakeAppearance(raw) => self.appearance(raw, &setup, events)?,
            Message::SnakeRemoval(raw) => {
                self.snakes.remove(&raw.id);
                if let Some(events) = events {
                    events.push(PresentationEvent::SnakeRemoved {
                        id: raw.id,
                        died: raw.raw_status == Some(1),
                    });
                }
            }
            Message::Anchor(raw) => self.anchor(raw, &setup, events)?,
            Message::Motion(raw) => self.motion(raw, events)?,
            Message::PreyAppearance(raw) => {
                if let Some(events) = events {
                    events.push(PresentationEvent::PreySpawn(prey_from_wire(raw)));
                }
            }
            Message::PreyRemoval { id, eater } => {
                if let Some(events) = events {
                    events.push(match eater {
                        Some(eater_id) => PresentationEvent::PreyEaten { id, eater_id },
                        None => PresentationEvent::PreyRemoved { id },
                    });
                }
            }
            Message::PreyMotion(raw) => {
                if let Some(events) = events {
                    events.push(PresentationEvent::PreyMotion(prey_motion_from_wire(raw)));
                }
            }
            Message::Mass { id, fraction }
            | Message::Tail {
                id,
                fraction: Some(fraction),
            } => {
                let fam = scalar::fraction24(fraction);
                if let Some(snake) = self.snakes.get_mut(&id) {
                    if code == b'r' {
                        snake.body.pop();
                    }
                    snake.fam = fam;
                    if let Some(events) = events {
                        events.push(if code == b'r' {
                            PresentationEvent::SnakeTailRemoved { id, fam: Some(fam) }
                        } else {
                            PresentationEvent::SnakeMass { id, fam }
                        });
                    }
                }
            }
            Message::Tail { id, fraction: None } => {
                if let Some(snake) = self.snakes.get_mut(&id) {
                    snake.body.pop();
                    if let Some(events) = events {
                        events.push(PresentationEvent::SnakeTailRemoved { id, fam: None });
                    }
                }
            }
            Message::FoodSector(raw) => {
                // F is a possibly split listing, not a sector replacement.
                let mut events = events;
                for food in raw.foods() {
                    let input = self.insert_food(
                        &setup,
                        (raw.sector[0], raw.sector[1]),
                        (food.offset[0], food.offset[1]),
                        food.size,
                        food.color,
                        true,
                    )?;
                    if let Some(events) = &mut events {
                        events.push(PresentationEvent::FoodSpawn(input));
                    }
                }
            }
            Message::FoodSpawn(raw) => {
                let sector = raw
                    .sector
                    .map(|s| (s[0], s[1]))
                    .or(self.added_sector)
                    .ok_or(StateError::MissingSector)?;
                if let Some(color) = raw.color {
                    self.added_color = color;
                }
                let input = self.insert_food(
                    &setup,
                    sector,
                    (raw.offset[0], raw.offset[1]),
                    raw.size,
                    self.added_color,
                    raw.rapid,
                )?;
                self.added_sector = Some(sector);
                if let Some(events) = events {
                    events.push(PresentationEvent::FoodSpawn(input));
                }
            }
            Message::FoodEaten(raw) => {
                let sector = raw
                    .sector
                    .map(|s| (s[0], s[1]))
                    .or(self.eaten_sector)
                    .ok_or(StateError::MissingSector)?;
                let eater_id = match raw.eater {
                    v19::Eater::Id(id) => {
                        self.eater_id = Some(id);
                        self.eater_id
                    }
                    v19::Eater::Cached => self.eater_id,
                    _ => None,
                };
                let key = food_key(sector, raw.offset[0], raw.offset[1]);
                self.foods.remove(&key);
                self.eaten_sector = Some(sector);
                if let Some(events) = events {
                    events.push(PresentationEvent::FoodConsume { key, eater_id });
                }
            }
            Message::SectorClear([x, y]) => {
                self.foods.retain(|_, food| food.sector != (x, y));
                if let Some(events) = events {
                    events.push(PresentationEvent::FoodSectorClear { sector: (x, y) });
                }
            }
            Message::Radius(raw) => {
                let radius = f64::from(raw);
                self.setup.as_mut().ok_or(StateError::MissingSetup)?.radius = radius;
                if let Some(events) = events {
                    events.push(PresentationEvent::RadiusTarget(radius));
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn appearance(
        &mut self,
        raw: v19::SnakeAppearanceRef<'_>,
        setup: &Setup,
        events: Option<&mut Vec<PresentationEvent>>,
    ) -> Result<(), StateError> {
        let id = raw.id;
        let direction = if events.is_some() {
            direction_from_wire(raw.raw_direction)
        } else {
            Direction::None
        };
        let speed_per_32ms = scalar::full_speed_per_32ms(raw.speed);
        let name = raw.name;
        let custom_skin = raw.custom_skin;
        let cosmetic = raw.cosmetic;
        let mut body = Vec::new();
        let mut absolute_body = events.as_ref().map(|_| Vec::new());
        let mut anchor_direction = None;
        let mut effective_heading = scalar::angle24(raw.heading);
        if let Some([x, y]) = raw.body.first {
            let mut absolute_point = Point {
                x: scalar::position5(x),
                y: scalar::position5(y),
            };
            let mut point = centered(setup, absolute_point.x, absolute_point.y)?;
            body.push(point);
            if let Some(body) = &mut absolute_body {
                body.push(absolute_point);
            }
            for delta in raw.body.deltas() {
                match delta {
                    v19::BodyDelta::FinalDirection(direction) => {
                        point = advance(point, direction, setup.body_spacing)?;
                        anchor_direction = Some(direction);
                        effective_heading = scalar::body_angle16(direction);
                        if absolute_body.is_some() {
                            absolute_point.x += math::cos(effective_heading) * setup.body_spacing;
                            absolute_point.y += math::sin(effective_heading) * setup.body_spacing;
                        }
                    }
                    v19::BodyDelta::HalfUnits([x, y]) => {
                        let dx = scalar::delta_half(x);
                        let dy = scalar::delta_half(y);
                        point = checked_point(point.x + dx, point.y + dy)?;
                        if absolute_body.is_some() {
                            absolute_point.x += dx;
                            absolute_point.y += dy;
                        }
                    }
                    _ => return Err(StateError::InvalidPacket(b's')),
                }
                body.push(point);
                if let Some(body) = &mut absolute_body {
                    body.push(absolute_point);
                }
                if body.len() > MAX_BODY_POINTS {
                    return Err(StateError::Limit);
                }
            }
            body.reverse();
            if let Some(body) = &mut absolute_body {
                body.reverse();
            }
        }
        if body.len() > MAX_BODY_POINTS
            || (!self.snakes.contains_key(&id) && self.snakes.len() >= MAX_SNAKES)
        {
            return Err(StateError::Limit);
        }
        let absolute_head = Point {
            x: scalar::position5(raw.head[0]),
            y: scalar::position5(raw.head[1]),
        };
        let snake = Snake {
            id,
            skin: raw.skin,
            name: name.iter().map(|&b| char::from(b)).collect(),
            custom_skin: custom_skin.to_vec(),
            cosmetic,
            head: centered(setup, absolute_head.x, absolute_head.y)?,
            body,
            heading: scalar::angle24(raw.heading),
            wanted_heading: scalar::angle24(raw.wanted_heading),
            speed: speed_per_32ms / 0.032,
            fam: scalar::fraction24(raw.fraction),
            radius: 6.,
            anchor_direction,
            segment_length: setup.body_spacing,
        };
        self.snakes.insert(id, snake);
        self.check_total_body()?;
        if self.own_snake_id.is_none() {
            self.own_snake_id = Some(id);
            self.added_sector = None;
            self.eaten_sector = None;
            self.added_color = 0;
            self.eater_id = None;
        }
        if let Some((events, absolute_body)) = events.zip(absolute_body) {
            let snake = &self.snakes[&id];
            events.push(PresentationEvent::SnakeAppearance(Appearance {
                id,
                skin: snake.skin,
                name: snake.name.clone(),
                custom_skin: snake.custom_skin.clone(),
                cosmetic,
                absolute_head,
                absolute_body,
                anchor_direction,
                heading: effective_heading,
                wanted_heading: snake.wanted_heading,
                speed_per_32ms,
                fam: snake.fam,
                wire_direction: direction,
            }));
        }
        Ok(())
    }

    fn anchor(
        &mut self,
        raw: v19::AnchorRef,
        setup: &Setup,
        events: Option<&mut Vec<PresentationEvent>>,
    ) -> Result<(), StateError> {
        let own = raw.target == v19::Target::Own;
        let grow = raw.grew;
        let direction = raw.direction;
        let absolute_position = raw.absolute.map(|[x, y]| Point {
            x: f64::from(x),
            y: f64::from(y),
        });
        let position = absolute_position
            .map(|point| centered(setup, point.x, point.y))
            .transpose()?;
        let fam = raw.fraction.map(scalar::fraction24);
        let id = match raw.target {
            v19::Target::Own => self.own_snake_id,
            v19::Target::Id(id) => Some(id),
            _ => return Err(StateError::InvalidPacket(b'g')),
        };
        if let Some(id) = id
            && (own || self.snakes.contains_key(&id))
            && let Some(events) = events
        {
            events.push(PresentationEvent::SnakeAnchor {
                target: if own {
                    SnakeTarget::Own
                } else {
                    SnakeTarget::Id(id)
                },
                absolute_position,
                direction,
                grew: grow,
                fam,
            });
        }
        let Some(snake) = id.and_then(|id| self.snakes.get_mut(&id)) else {
            return Ok(());
        };
        let next_direction = direction.or(snake.anchor_direction);
        let point = if let Some(point) = position {
            point
        } else {
            advance(
                *snake
                    .body
                    .first()
                    .ok_or(StateError::MissingAnchor(snake.id))?,
                next_direction.ok_or(StateError::MissingAnchor(snake.id))?,
                snake.segment_length,
            )?
        };
        let next_len = snake.body.len() + usize::from(grow || snake.body.is_empty());
        if next_len > MAX_BODY_POINTS {
            return Err(StateError::Limit);
        }
        if !grow && !snake.body.is_empty() {
            snake.body.pop();
        }
        snake.body.insert(0, point);
        snake.head = point;
        snake.anchor_direction = next_direction;
        if let Some(fam) = fam {
            snake.fam = fam;
        }
        self.check_total_body()
    }

    fn motion(
        &mut self,
        raw: v19::MotionRef<'_>,
        events: Option<&mut Vec<PresentationEvent>>,
    ) -> Result<(), StateError> {
        // The admitted single-field 5 command changes no fields or events.
        if raw.heading.is_none()
            && raw.wanted_heading.is_none()
            && raw.speed.is_none()
            && raw.direction.is_none()
        {
            return Ok(());
        }
        let own = raw.target == v19::Target::Own;
        let heading = raw.heading.map(scalar::angle8);
        let wanted_heading = raw.wanted_heading.map(scalar::angle8);
        let speed_per_32ms = raw.speed.map(scalar::compact_speed_per_32ms);
        let direction = raw.direction.map(direction_from_shared);
        let id = match raw.target {
            v19::Target::Own => self.own_snake_id,
            v19::Target::Id(id) => Some(id),
            _ => return Err(StateError::InvalidPacket(raw.command[0])),
        };
        if let Some(id) = id
            && (own || self.snakes.contains_key(&id))
            && let Some(events) = events
        {
            events.push(PresentationEvent::SnakeMotion {
                target: if own {
                    SnakeTarget::Own
                } else {
                    SnakeTarget::Id(id)
                },
                heading,
                wanted_heading,
                direction,
                speed_per_32ms,
            });
        }
        if let Some(snake) = id.and_then(|id| self.snakes.get_mut(&id)) {
            if let Some(value) = heading {
                snake.heading = value;
            }
            if let Some(value) = wanted_heading {
                snake.wanted_heading = value;
            }
            if let Some(value) = speed_per_32ms {
                snake.speed = value / 0.032;
            }
        }
        Ok(())
    }

    fn insert_food(
        &mut self,
        setup: &Setup,
        sector: (u8, u8),
        offset: (u8, u8),
        size: u8,
        color: u8,
        rapid: bool,
    ) -> Result<FoodSpawnInput, StateError> {
        let (rx, ry) = offset;
        let key = food_key(sector, rx, ry);
        if !self.foods.contains_key(&key) && self.foods.len() >= MAX_FOODS {
            return Err(StateError::Limit);
        }
        let unit = f64::from(setup.sector_size);
        let position = centered(
            setup,
            (f64::from(sector.0) + f64::from(rx) / 256.) * unit,
            (f64::from(sector.1) + f64::from(ry) / 256.) * unit,
        )?;
        self.foods.insert(
            key,
            Food {
                position,
                size: f64::from(size) / 5.,
                color,
                sector,
            },
        );
        Ok(FoodSpawnInput {
            key,
            sector,
            absolute_position: Point {
                x: f64::from(rx) * (unit / 256.) + f64::from(sector.0) * unit,
                y: f64::from(ry) * (unit / 256.) + f64::from(sector.1) * unit,
            },
            size_byte: size,
            base_size: f64::from(size) / 5.,
            raw_color: color,
            rapid,
        })
    }

    fn check_total_body(&self) -> Result<(), StateError> {
        if self
            .snakes
            .values()
            .map(|snake| snake.body.len())
            .sum::<usize>()
            > MAX_TOTAL_BODY_POINTS
        {
            Err(StateError::Limit)
        } else {
            Ok(())
        }
    }
}

fn direction_from_shared(direction: v19::Direction) -> Direction {
    match direction {
        v19::Direction::Decreasing => Direction::Decreasing,
        v19::Direction::Increasing => Direction::Increasing,
        _ => Direction::None,
    }
}
fn direction_from_wire(raw: u8) -> Direction {
    direction_from_shared(
        v19::Direction::from_wire(raw).expect("client admission checked direction"),
    )
}
fn prey_from_wire(raw: v19::PreyRef) -> PreySpawnInput {
    PreySpawnInput {
        id: raw.id,
        absolute_position: Point {
            x: scalar::position5(raw.position[0]),
            y: scalar::position5(raw.position[1]),
        },
        size_byte: raw.size,
        base_size: scalar::position5(u32::from(raw.size)),
        raw_color: raw.color,
        direction: direction_from_wire(raw.raw_direction),
        heading: scalar::prey_angle24(raw.heading),
        wanted_heading: scalar::prey_angle24(raw.wanted_heading),
        speed_per_32ms: scalar::full_speed_per_32ms(raw.speed),
    }
}
fn prey_motion_from_wire(raw: v19::PreyMotion) -> PreyMotionInput {
    PreyMotionInput {
        id: raw.id,
        absolute_position: Point {
            x: scalar::prey_position(raw.position[0]),
            y: scalar::prey_position(raw.position[1]),
        },
        direction: raw.raw_direction.map(direction_from_wire),
        heading: raw.heading.map(scalar::prey_angle24),
        wanted_heading: raw.wanted_heading.map(scalar::prey_angle24),
        speed_per_32ms: raw.speed.map(scalar::full_speed_per_32ms),
    }
}
fn setup_from_wire(raw: v19::SetupRef<'_>) -> Setup {
    let grid_center = f64::from(raw.grid_center);
    Setup {
        protocol: raw.protocol,
        grid_center,
        radius: raw.radius.map(f64::from).unwrap_or(grid_center * 0.98),
        sector_size: raw.sector_size,
        sector_count: raw.sector_count,
        max_segments: raw.max_segments,
        body_spacing: f64::from(raw.body_spacing),
        body_contraction: f64::from(raw.contraction) / 1000.,
        spangdv: f64::from(raw.spangdv) / 10.,
        nsp1: f64::from(raw.nsp1) / 100.,
        nsp2: f64::from(raw.nsp2) / 100.,
        nsp3: f64::from(raw.nsp3) / 100.,
        mamu: f64::from(raw.mamu) / 1000.,
        mamu2: f64::from(raw.mamu2) / 1000.,
        server_number: raw.server_number,
        game_mode: raw.game_mode,
        team: raw.team,
    }
}
fn checked_point(x: f64, y: f64) -> Result<Point, StateError> {
    if x.is_finite() && y.is_finite() && x.abs() <= MAX_COORDINATE && y.abs() <= MAX_COORDINATE {
        Ok(Point { x, y })
    } else {
        Err(StateError::Coordinate)
    }
}
fn centered(setup: &Setup, x: f64, y: f64) -> Result<Point, StateError> {
    checked_point(x - setup.grid_center, y - setup.grid_center)
}
fn advance(point: Point, direction: u16, spacing: f64) -> Result<Point, StateError> {
    let angle = scalar::anchor_angle16(direction);
    checked_point(
        point.x + math::cos(angle) * spacing,
        point.y + math::sin(angle) * spacing,
    )
}
fn food_key(sector: (u8, u8), rx: u8, ry: u8) -> u32 {
    u32::from_be_bytes([sector.0, sector.1, rx, ry])
}
