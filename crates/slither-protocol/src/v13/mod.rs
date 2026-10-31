//! Protocol-13 server messages. IDs are always explicit, including the local snake.
//!
//! Observed layouts and interpretation limits are recorded in the
//! [catalog](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-protocol13-catalog/REPORT.md).
//! Coordinate scales, delta origins and turn directions are protocol interpretations,
//! not measured physics. This reader owns no position, ID or food caches.
//!
//! `DecodeAdmission` retains the v19 admission differences for identical layouts
//! (setup, removal, prey, HUD, minimap and control). All admissions require explicit
//! IDs and complete coordinate tuples for v13-specific layouts. In particular,
//! the unobserved one-field `5` form is rejected under every admission.

use crate::{
    DecodeAdmission, DecodeError, ErrorKind, byte, frame::CommandRef, require, triple, v19, word,
};
use core::{iter::FusedIterator, slice};

// These layouts and their checked interpretations are identical in both versions.
pub use crate::v19::{
    Direction, PreyMotion, PreyRef, RemovalRef, SetupRef, hud, minimap, setup_protocol_version,
};

pub const PROTOCOL_VERSION: u8 = 13;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SnakeAppearanceRef<'a> {
    pub command: &'a [u8],
    pub id: u16,
    pub heading: u32,
    pub raw_direction: u8,
    pub wanted_heading: u32,
    pub speed: u16,
    pub fraction: u32,
    pub skin: u8,
    pub head: [u32; 2],
    pub name: &'a [u8],
    pub custom_skin: &'a [u8],
    pub cosmetic: u8,
    pub body: BodyRef<'a>,
}

/// Appearance coordinates use the shared `scalar::position5` interpretation.
/// Every trailing pair is a half-unit delta (`scalar::delta_half`), including
/// the last pair; protocol 19 instead interprets that pair as a direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BodyRef<'a> {
    pub first: Option<[u32; 2]>,
    tail: &'a [u8],
}
impl<'a> BodyRef<'a> {
    pub fn raw_tail(self) -> &'a [u8] {
        self.tail
    }
    pub fn deltas(self) -> BodyDeltas<'a> {
        BodyDeltas {
            chunks: self.tail.as_chunks::<2>().0.iter(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct BodyDeltas<'a> {
    chunks: slice::Iter<'a, [u8; 2]>,
}
impl Iterator for BodyDeltas<'_> {
    type Item = [u8; 2];
    fn next(&mut self) -> Option<Self::Item> {
        self.chunks.next().copied()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}
impl ExactSizeIterator for BodyDeltas<'_> {}
impl FusedIterator for BodyDeltas<'_> {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Coordinates {
    Absolute([u16; 2]),
    /// Raw bytes; interpreted integer deltas are each byte minus 128.
    Relative([u8; 2]),
}
impl Coordinates {
    pub fn delta(self) -> Option<[i16; 2]> {
        match self {
            Self::Relative(raw) => Some(raw.map(|n| i16::from(n) - 128)),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnchorRef {
    pub id: u16,
    pub coordinates: Coordinates,
    pub grew: bool,
    pub fraction: Option<u32>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionRef<'a> {
    pub command: &'a [u8],
    pub id: u16,
    pub heading: Option<u8>,
    pub wanted_heading: Option<u8>,
    pub speed: Option<u8>,
    /// Checked protocol interpretation of opcode and length, not a raw field.
    pub direction: Option<Direction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoodBatchRef<'a> {
    tuples: &'a [u8],
}
impl<'a> FoodBatchRef<'a> {
    pub fn raw_tuples(self) -> &'a [u8] {
        self.tuples
    }
    pub fn foods(self) -> Foods<'a> {
        Foods {
            chunks: self.tuples.as_chunks::<6>().0.iter(),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoodTuple {
    pub color: u8,
    pub position: [u16; 2],
    pub size: u8,
}
#[derive(Clone, Debug)]
pub struct Foods<'a> {
    chunks: slice::Iter<'a, [u8; 6]>,
}
impl Iterator for Foods<'_> {
    type Item = FoodTuple;
    fn next(&mut self) -> Option<Self::Item> {
        let p = self.chunks.next()?;
        Some(FoodTuple {
            color: p[0],
            position: [
                u16::from_be_bytes([p[1], p[2]]),
                u16::from_be_bytes([p[3], p[4]]),
            ],
            size: p[5],
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}
impl ExactSizeIterator for Foods<'_> {}
impl FusedIterator for Foods<'_> {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoodSpawn {
    pub color: u8,
    pub position: [u16; 2],
    pub size: u8,
    pub rapid: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoodEaten {
    pub position: [u16; 2],
    /// No cached-eater form in v13; zero remains an explicit ID when present.
    pub eater: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Message<'a> {
    Setup(SetupRef<'a>),
    SnakeAppearance(SnakeAppearanceRef<'a>),
    SnakeRemoval(RemovalRef<'a>),
    Anchor(AnchorRef),
    Motion(MotionRef<'a>),
    Mass {
        id: u16,
        fraction: u32,
    },
    Tail {
        id: u16,
        fraction: Option<u32>,
    },
    FoodBatch(FoodBatchRef<'a>),
    FoodSpawn(FoodSpawn),
    FoodEaten(FoodEaten),
    SectorClear([u8; 2]),
    PreyAppearance(PreyRef),
    PreyRemoval {
        id: u16,
        eater: Option<u16>,
    },
    PreyMotion(PreyMotion),
    Minimap(minimap::MinimapPacket<'a>),
    Leaderboard(hud::Leaderboard<'a>),
    Session {
        server_id: u16,
        session_id: u32,
    },
    Kills {
        id: u16,
        count: u32,
    },
    Pong(&'a [u8]),
    LifeControl {
        status: Option<u8>,
        command: &'a [u8],
    },
    Radius(u32),
    Challenge(&'a [u8]),
    // Unobserved in the v13 catalog; supported for the independent local encoder.
    ScoreAdjustment(u8),
    Announcement(hud::Announcement<'a>),
    Rules(crate::extensions::slither_sim::RulesRef<'a>),
    Unknown {
        opcode: u8,
        command: &'a [u8],
    },
}

pub fn decode_command(
    command: CommandRef<'_>,
    admission: DecodeAdmission,
) -> Result<Message<'_>, DecodeError> {
    decode(command.bytes, admission).map_err(|mut e| {
        e.offset = e.offset.saturating_add(command.offset);
        e
    })
}

/// Decode one bare command. No version selection or consumer lane is enabled
/// automatically. Unsupported version-specific opcodes retain their exact bytes.
pub fn decode(p: &[u8], admission: DecodeAdmission) -> Result<Message<'_>, DecodeError> {
    let code = byte(p, 0)?;
    let n = p.len();
    Ok(match code {
        b'a' => Message::Setup(v19::setup_for_version(p, admission, PROTOCOL_VERSION)?),
        b'g' | b'G' | b'n' | b'N' => {
            let relative = matches!(code, b'G' | b'N');
            let grew = matches!(code, b'n' | b'N');
            // Absolute n is encoder-derived; the retained official corpus only
            // observed g/G/N. No angle-only or implicit-own forms are admitted.
            let at = if relative { 5 } else { 7 };
            require(
                p,
                n == at + if grew { 3 } else { 0 },
                n,
                ErrorKind::InvalidLength,
            )?;
            Message::Anchor(AnchorRef {
                id: word(p, 1)?,
                coordinates: if relative {
                    Coordinates::Relative([p[3], p[4]])
                } else {
                    Coordinates::Absolute([word(p, 3)?, word(p, 5)?])
                },
                grew,
                fraction: if grew { Some(triple(p, at)?) } else { None },
            })
        }
        b'F' => {
            require(p, (n - 1).is_multiple_of(6), n, ErrorKind::InvalidLength)?;
            Message::FoodBatch(FoodBatchRef { tuples: &p[1..] })
        }
        b'f' | b'b' => {
            require(p, n == 7, n, ErrorKind::InvalidLength)?;
            Message::FoodSpawn(FoodSpawn {
                color: p[1],
                position: [word(p, 2)?, word(p, 4)?],
                size: p[6],
                rapid: code == b'b',
            })
        }
        b'c' => {
            // The 5-byte no-eater form is encoder-derived, not observed here.
            require(p, matches!(n, 5 | 7), n, ErrorKind::InvalidLength)?;
            Message::FoodEaten(FoodEaten {
                position: [word(p, 1)?, word(p, 3)?],
                eater: if n == 7 { Some(word(p, 5)?) } else { None },
            })
        }
        b'e' | b'E' | b'3' | b'4' | b'5' => {
            require(
                p,
                (4..=6).contains(&n) && !(code == b'5' && n == 4),
                n,
                ErrorKind::InvalidLength,
            )?;
            let v19::Message::Motion(m) = v19::decode(p, admission)? else {
                unreachable!()
            };
            Message::Motion(MotionRef {
                command: p,
                id: word(p, 1)?,
                heading: m.heading,
                wanted_heading: m.wanted_heading,
                speed: m.speed,
                direction: m.direction,
            })
        }
        // Reuse the identical parsers. This whitelist deliberately excludes
        // v19-only own IDs, sector/food caches, angle anchors and team messages.
        b's' | b'h' | b'r' | b'w' | b'y' | b'j' | b'l' | b'k' | b'p' | b'v' | b'S' | b'z'
        | b'6' | b'M' | b'V' | b'U' | b'u' | b'L' | b'R' | b'm' | b'~' => {
            match v19::decode(p, admission)? {
                v19::Message::SnakeAppearance(a) => Message::SnakeAppearance(SnakeAppearanceRef {
                    command: a.command,
                    id: a.id,
                    heading: a.heading,
                    raw_direction: a.raw_direction,
                    wanted_heading: a.wanted_heading,
                    speed: a.speed,
                    fraction: a.fraction,
                    skin: a.skin,
                    head: a.head,
                    name: a.name,
                    custom_skin: a.custom_skin,
                    cosmetic: a.cosmetic,
                    body: BodyRef {
                        first: a.body.first,
                        tail: a.body.raw_tail(),
                    },
                }),
                v19::Message::SnakeRemoval(r) => Message::SnakeRemoval(r),
                v19::Message::Mass { id, fraction } => Message::Mass { id, fraction },
                v19::Message::Tail { id, fraction } => Message::Tail { id, fraction },
                v19::Message::SectorClear(s) => Message::SectorClear(s),
                v19::Message::PreyAppearance(p) => Message::PreyAppearance(p),
                v19::Message::PreyRemoval { id, eater } => Message::PreyRemoval { id, eater },
                v19::Message::PreyMotion(p) => Message::PreyMotion(p),
                v19::Message::Minimap(m) => Message::Minimap(m),
                v19::Message::Leaderboard(l) => Message::Leaderboard(l),
                v19::Message::Session {
                    server_id,
                    session_id,
                } => Message::Session {
                    server_id,
                    session_id,
                },
                v19::Message::Kills { id, count } => Message::Kills { id, count },
                v19::Message::Pong(p) => Message::Pong(p),
                v19::Message::LifeControl { status, command } => {
                    Message::LifeControl { status, command }
                }
                v19::Message::Radius(r) => Message::Radius(r),
                v19::Message::Challenge(c) => Message::Challenge(c),
                v19::Message::ScoreAdjustment(s) => Message::ScoreAdjustment(s),
                v19::Message::Announcement(a) => Message::Announcement(a),
                v19::Message::Rules(r) => Message::Rules(r),
                _ => unreachable!("whitelisted shared layout"),
            }
        }
        _ => Message::Unknown {
            opcode: code,
            command: p,
        },
    })
}
