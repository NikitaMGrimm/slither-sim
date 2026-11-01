use super::{hud, minimap};
use crate::{
    DecodeAdmission, DecodeError, ErrorKind, byte, frame::CommandRef, require, triple, word,
};
use core::{iter::FusedIterator, slice};
pub const PROTOCOL_VERSION: u8 = 19;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Target {
    Own,
    Id(u16),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Direction {
    None,
    Decreasing,
    Increasing,
}
impl Direction {
    /// Checked interpretation without discarding the view's raw direction byte.
    pub fn from_wire(raw: u8) -> Option<Self> {
        match raw {
            b'0' => Some(Self::None),
            b'1' => Some(Self::Decreasing),
            b'2' => Some(Self::Increasing),
            _ => None,
        }
    }
}

/// Raw fields; default radius calculation, score tables and mode interpretation
/// stay with consumers. `command` retains all opaque extensions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SetupRef<'a> {
    pub command: &'a [u8],
    pub protocol: u8,
    pub grid_center: u32,
    pub max_segments: u16,
    pub sector_size: u16,
    pub sector_count: u16,
    pub spangdv: u8,
    pub nsp1: u16,
    pub nsp2: u16,
    pub nsp3: u16,
    pub mamu: u16,
    pub mamu2: u16,
    pub contraction: u16,
    pub body_spacing: u8,
    pub server_number: Option<u16>,
    pub radius: Option<u32>,
    pub game_mode: Option<u8>,
    pub team: Option<u8>,
    pub extensions: &'a [u8],
}
/// Boundary-only accessor; version admission and scalar validity are separate.
pub fn setup_protocol_version(p: &[u8], admission: DecodeAdmission) -> Result<u8, DecodeError> {
    require(p, p.first() == Some(&b'a'), 0, ErrorKind::WrongOpcode)?;
    require(
        p,
        p.len() >= 25 && !matches!(p.len(), 26 | 28 | 29),
        p.len(),
        ErrorKind::Truncated,
    )?;
    if admission != DecodeAdmission::Sim {
        require(p, p.len() != 31 || p[30] != 2, 31, ErrorKind::Truncated)?;
    }
    Ok(p[23])
}
fn setup(p: &[u8], admission: DecodeAdmission) -> Result<SetupRef<'_>, DecodeError> {
    setup_for_version(p, admission, PROTOCOL_VERSION)
}
pub(crate) fn setup_for_version(
    p: &[u8],
    admission: DecodeAdmission,
    version: u8,
) -> Result<SetupRef<'_>, DecodeError> {
    let protocol = setup_protocol_version(p, admission)?;
    require(p, protocol == version, 23, ErrorKind::UnsupportedVersion)?;
    let s = SetupRef {
        command: p,
        protocol,
        grid_center: triple(p, 1)?,
        max_segments: word(p, 4)?,
        sector_size: word(p, 6)?,
        sector_count: word(p, 8)?,
        spangdv: p[10],
        nsp1: word(p, 11)?,
        nsp2: word(p, 13)?,
        nsp3: word(p, 15)?,
        mamu: word(p, 17)?,
        mamu2: word(p, 19)?,
        contraction: word(p, 21)?,
        body_spacing: p[24],
        server_number: if p.len() >= 27 {
            Some(word(p, 25)?)
        } else {
            None
        },
        radius: if p.len() >= 30 {
            Some(triple(p, 27)?)
        } else {
            None
        },
        game_mode: p.get(30).copied(),
        team: if p.get(30) == Some(&2) {
            p.get(31).copied()
        } else {
            None
        },
        extensions: &p[if p.get(30) == Some(&2) {
            p.len().min(32)
        } else {
            p.len().min(31)
        }..],
    };
    for (valid, offset) in [
        (s.grid_center != 0, 1),
        (s.max_segments != 0, 4),
        (s.sector_size != 0, 6),
        (s.body_spacing != 0, 24),
        (s.contraction <= 1000, 21),
    ] {
        require(p, valid, offset, ErrorKind::InvalidScalar)?;
    }
    if admission != DecodeAdmission::Sim {
        require(p, s.sector_count != 0, 8, ErrorKind::InvalidScalar)?;
        require(p, s.radius != Some(0), 27, ErrorKind::InvalidScalar)?;
    }
    Ok(s)
}

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
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BodyDelta {
    HalfUnits([u8; 2]),
    FinalDirection(u16),
}
#[derive(Clone, Debug)]
pub struct BodyDeltas<'a> {
    chunks: slice::Iter<'a, [u8; 2]>,
}
impl Iterator for BodyDeltas<'_> {
    type Item = BodyDelta;
    fn next(&mut self) -> Option<Self::Item> {
        let pair = self.chunks.next()?;
        Some(if self.chunks.len() == 0 {
            BodyDelta::FinalDirection(u16::from_be_bytes([pair[0], pair[1]]))
        } else {
            BodyDelta::HalfUnits([pair[0], pair[1]])
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}
impl ExactSizeIterator for BodyDeltas<'_> {}
impl FusedIterator for BodyDeltas<'_> {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemovalRef<'a> {
    pub command: &'a [u8],
    pub id: u16,
    pub raw_status: Option<u8>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnchorRef {
    pub target: Target,
    pub direction: Option<u16>,
    pub absolute: Option<[u16; 2]>,
    pub grew: bool,
    pub fraction: Option<u32>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionRef<'a> {
    pub command: &'a [u8],
    pub target: Target,
    pub heading: Option<u8>,
    pub wanted_heading: Option<u8>,
    pub speed: Option<u8>,
    pub direction: Option<Direction>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoodSectorRef<'a> {
    pub sector: [u8; 2],
    tuples: &'a [u8],
}
impl<'a> FoodSectorRef<'a> {
    pub fn foods(self) -> Foods<'a> {
        Foods {
            chunks: self.tuples.as_chunks::<4>().0.iter(),
        }
    }
    pub fn raw_tuples(self) -> &'a [u8] {
        self.tuples
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoodTuple {
    pub color: u8,
    pub offset: [u8; 2],
    pub size: u8,
}
#[derive(Clone, Debug)]
pub struct Foods<'a> {
    chunks: slice::Iter<'a, [u8; 4]>,
}
impl Iterator for Foods<'_> {
    type Item = FoodTuple;
    fn next(&mut self) -> Option<Self::Item> {
        let p = self.chunks.next()?;
        Some(FoodTuple {
            color: p[0],
            offset: [p[1], p[2]],
            size: p[3],
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
    pub sector: Option<[u8; 2]>,
    pub offset: [u8; 2],
    pub color: Option<u8>,
    pub size: u8,
    pub rapid: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Eater {
    Absent,
    Cached,
    Id(u16),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoodEaten {
    pub sector: Option<[u8; 2]>,
    pub offset: [u8; 2],
    pub eater: Eater,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreyRef {
    pub id: u16,
    pub color: u8,
    pub position: [u32; 2],
    pub size: u8,
    pub raw_direction: u8,
    pub wanted_heading: u32,
    pub heading: u32,
    pub speed: u16,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreyMotion {
    pub id: u16,
    pub position: [u16; 2],
    pub raw_direction: Option<u8>,
    pub heading: Option<u32>,
    pub wanted_heading: Option<u32>,
    pub speed: Option<u16>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TeamScoresRef<'a> {
    bytes: &'a [u8],
}
impl<'a> TeamScoresRef<'a> {
    pub fn is_history(self) -> bool {
        self.bytes.len() > 8
    }
    pub fn scores(self) -> TeamScores<'a> {
        TeamScores {
            chunks: self.bytes.as_chunks::<8>().0.iter(),
        }
    }
    pub fn raw_scores(self) -> &'a [u8] {
        self.bytes
    }
}
#[derive(Clone, Debug)]
pub struct TeamScores<'a> {
    chunks: slice::Iter<'a, [u8; 8]>,
}
impl Iterator for TeamScores<'_> {
    type Item = [i32; 2];
    fn next(&mut self) -> Option<Self::Item> {
        let p = self.chunks.next()?;
        Some([
            i32::from_be_bytes(p[..4].try_into().unwrap()),
            i32::from_be_bytes(p[4..].try_into().unwrap()),
        ])
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}
impl ExactSizeIterator for TeamScores<'_> {}
impl FusedIterator for TeamScores<'_> {}

/// An extensible output family. Match selected variants and handle future
/// variants with a wildcard; `Unknown` only represents unknown wire opcodes.
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
    ScoreAdjustment(u8),
    FoodSector(FoodSectorRef<'a>),
    FoodSpawn(FoodSpawn),
    FoodEaten(FoodEaten),
    SectorClear([u8; 2]),
    SectorVisible([u8; 2]),
    PreyAppearance(PreyRef),
    PreyRemoval {
        id: u16,
        eater: Option<u16>,
    },
    PreyMotion(PreyMotion),
    Minimap(minimap::MinimapPacket<'a>),
    Leaderboard(hud::Leaderboard<'a>),
    Announcement(hud::Announcement<'a>),
    Session {
        server_id: u16,
        session_id: u32,
    },
    TeamScores(TeamScoresRef<'a>),
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
    Rules(crate::extensions::slither_sim::RulesRef<'a>),
    Challenge(&'a [u8]),
    Unknown {
        opcode: u8,
        command: &'a [u8],
    },
}

/// Errors are translated to the original message offset, including its prefix.
pub fn decode_command(
    command: CommandRef<'_>,
    admission: DecodeAdmission,
) -> Result<Message<'_>, DecodeError> {
    decode(command.bytes, admission).map_err(|mut e| {
        e.offset = e.offset.saturating_add(command.offset);
        e
    })
}
/// Decode one bare command. Selecting a lane does not enable that lane in a
/// consumer; callers must retain their existing family selection and lifecycle.
pub fn decode(p: &[u8], admission: DecodeAdmission) -> Result<Message<'_>, DecodeError> {
    let code = byte(p, 0)?;
    let n = p.len();
    let invalid = || DecodeError::at(p, n, ErrorKind::InvalidLength);
    Ok(match code {
        b'a' => Message::Setup(setup(p, admission)?),
        b's' if n <= 7 => {
            require(
                p,
                if admission == DecodeAdmission::Sim {
                    n >= 3
                } else {
                    matches!(n, 3 | 4)
                },
                n,
                ErrorKind::InvalidLength,
            )?;
            Message::SnakeRemoval(RemovalRef {
                command: p,
                id: word(p, 1)?,
                raw_status: p.get(3).copied(),
            })
        }
        b's' => {
            let name_end = 23 + usize::from(byte(p, 22)?);
            let name = p.get(23..name_end).ok_or_else(invalid)?;
            let custom_end = name_end + 1 + usize::from(byte(p, name_end)?);
            let custom_skin = p.get(name_end + 1..custom_end).ok_or_else(invalid)?;
            let cosmetic = byte(p, custom_end)?;
            let at = custom_end + 1;
            let body = if at == n {
                BodyRef {
                    first: None,
                    tail: &[],
                }
            } else {
                require(
                    p,
                    n >= at + 6 && (n - at - 6).is_multiple_of(2),
                    n,
                    ErrorKind::InvalidLength,
                )?;
                BodyRef {
                    first: Some([triple(p, at)?, triple(p, at + 3)?]),
                    tail: &p[at + 6..],
                }
            };
            if admission == DecodeAdmission::ClientPresentation {
                require(
                    p,
                    Direction::from_wire(p[6]).is_some(),
                    6,
                    ErrorKind::InvalidScalar,
                )?;
            }
            Message::SnakeAppearance(SnakeAppearanceRef {
                command: p,
                id: word(p, 1)?,
                heading: triple(p, 3)?,
                raw_direction: p[6],
                wanted_heading: triple(p, 7)?,
                speed: word(p, 10)?,
                fraction: triple(p, 12)?,
                skin: p[15],
                head: [triple(p, 16)?, triple(p, 19)?],
                name,
                custom_skin,
                cosmetic,
                body,
            })
        }
        b'g' | b'n' | b'G' | b'N' | b'=' | b'+' => {
            let own =
                matches!(code, b'G' | b'N') || code == b'=' && n == 7 || code == b'+' && n == 10;
            let at = if own { 1 } else { 3 };
            let target = if own {
                Target::Own
            } else {
                Target::Id(word(p, 1)?)
            };
            let grew = matches!(code, b'n' | b'N' | b'+');
            let mass = if grew { 3 } else { 0 };
            let absolute = matches!(code, b'=' | b'+');
            let count = n.checked_sub(at).ok_or_else(invalid)?;
            require(
                p,
                if absolute {
                    count == 6 + mass
                } else {
                    count == mass || count == 2 + mass
                },
                n,
                ErrorKind::InvalidLength,
            )?;
            Message::Anchor(AnchorRef {
                target,
                grew,
                direction: if absolute || count == 2 + mass {
                    Some(word(p, at)?)
                } else {
                    None
                },
                absolute: if absolute {
                    Some([word(p, at + 2)?, word(p, at + 4)?])
                } else {
                    None
                },
                fraction: if grew { Some(triple(p, n - 3)?) } else { None },
            })
        }
        b'e' | b'E' | b'3' | b'4' | b'5' | b'd' | b'7' => {
            let own = matches!(code, b'd' | b'7') || n <= 3;
            let at = if own { 1 } else { 3 };
            let target = if own {
                Target::Own
            } else {
                Target::Id(word(p, 1)?)
            };
            let count = n.checked_sub(at).ok_or_else(invalid)?;
            let all = || -> Result<_, DecodeError> {
                Ok((
                    Some(byte(p, at)?),
                    Some(byte(p, at + 1)?),
                    Some(byte(p, at + 2)?),
                ))
            };
            let (heading, wanted_heading, speed) = if admission == DecodeAdmission::Sim && n == 6 {
                all()?
            } else {
                match (code, count) {
                    (b'd' | b'7', 3) => all()?,
                    (b'e' | b'E' | b'3' | b'4' | b'5', 3) if !own => all()?,
                    (b'e', 2) => (Some(p[at]), None, Some(p[at + 1])),
                    (b'E' | b'4', 2) => (None, Some(p[at]), Some(p[at + 1])),
                    (b'3' | b'5', 2) => (Some(p[at]), Some(p[at + 1]), None),
                    (b'e', 1) => (Some(p[at]), None, None),
                    (b'E' | b'4', 1) => (None, Some(p[at]), None),
                    (b'3', 1) => (None, None, Some(p[at])),
                    (b'5', 1) if admission != DecodeAdmission::Sim => (None, None, None),
                    _ => return Err(invalid()),
                }
            };
            let direction = match (code, count) {
                (b'e' | b'd', 3) | (b'E' | b'3', 2) | (b'E', 1) => Some(Direction::Decreasing),
                (b'E' | b'3' | b'4' | b'5' | b'7', 3) | (b'4' | b'5', 2) | (b'4', 1) => {
                    Some(Direction::Increasing)
                }
                _ => None,
            };
            Message::Motion(MotionRef {
                command: p,
                target,
                heading,
                wanted_heading,
                speed,
                direction,
            })
        }
        b'h' if n == 6 => Message::Mass {
            id: word(p, 1)?,
            fraction: triple(p, 3)?,
        },
        b'r' if matches!(n, 3 | 6) => Message::Tail {
            id: word(p, 1)?,
            fraction: if n == 6 { Some(triple(p, 3)?) } else { None },
        },
        b'R' if n == 2 => Message::ScoreAdjustment(p[1]),
        b'F' if n >= 3 && (n - 3).is_multiple_of(4) => Message::FoodSector(FoodSectorRef {
            sector: [p[1], p[2]],
            tuples: &p[3..],
        }),
        b'f' | b'b' if (4..=7).contains(&n) => {
            let explicit = n >= 6;
            let at = if explicit { 3 } else { 1 };
            Message::FoodSpawn(FoodSpawn {
                sector: if explicit { Some([p[1], p[2]]) } else { None },
                offset: [p[at], p[at + 1]],
                color: if n == at + 4 { Some(p[at + 2]) } else { None },
                size: p[n - 1],
                rapid: code == b'b',
            })
        }
        b'c' | b'C' | b'<' => {
            let extra = if code == b'<' { 2 } else { 0 };
            require(
                p,
                n == 3 + extra || n == 5 + extra,
                n,
                ErrorKind::InvalidLength,
            )?;
            let explicit = n == 5 + extra;
            let at = if explicit { 3 } else { 1 };
            Message::FoodEaten(FoodEaten {
                sector: if explicit { Some([p[1], p[2]]) } else { None },
                offset: [p[at], p[at + 1]],
                eater: match code {
                    b'<' => Eater::Id(word(p, at + 2)?),
                    b'C' => Eater::Cached,
                    _ => Eater::Absent,
                },
            })
        }
        b'w' if n == 3 => Message::SectorClear([p[1], p[2]]),
        b'W' if n == 3 => Message::SectorVisible([p[1], p[2]]),
        b'y' if matches!(n, 3 | 5) => Message::PreyRemoval {
            id: word(p, 1)?,
            eater: if n == 5 { Some(word(p, 3)?) } else { None },
        },
        b'y' if n == 20 => {
            if admission != DecodeAdmission::Sim {
                require(
                    p,
                    Direction::from_wire(p[11]).is_some(),
                    11,
                    ErrorKind::InvalidScalar,
                )?;
            }
            Message::PreyAppearance(PreyRef {
                id: word(p, 1)?,
                color: p[3],
                position: [triple(p, 4)?, triple(p, 7)?],
                size: p[10],
                raw_direction: p[11],
                wanted_heading: triple(p, 12)?,
                heading: triple(p, 15)?,
                speed: word(p, 18)?,
            })
        }
        b'j' => {
            let (d, h, w, s) = match n {
                7 => (None, None, None, None),
                9 => (None, None, None, Some(7)),
                10 => (None, Some(7), None, None),
                11 => (Some(7), None, Some(8), None),
                12 => (None, Some(7), None, Some(10)),
                13 => (Some(7), None, Some(8), Some(11)),
                14 => (Some(7), Some(8), Some(11), None),
                16 => (Some(7), Some(8), Some(11), Some(14)),
                _ => return Err(invalid()),
            };
            let raw_direction = d.map(|at| byte(p, at)).transpose()?;
            if admission != DecodeAdmission::Sim
                && let Some(raw) = raw_direction
            {
                require(
                    p,
                    Direction::from_wire(raw).is_some(),
                    d.unwrap(),
                    ErrorKind::InvalidScalar,
                )?;
            }
            Message::PreyMotion(PreyMotion {
                id: word(p, 1)?,
                position: [word(p, 3)?, word(p, 5)?],
                raw_direction,
                heading: h.map(|at| triple(p, at)).transpose()?,
                wanted_heading: w.map(|at| triple(p, at)).transpose()?,
                speed: s.map(|at| word(p, at)).transpose()?,
            })
        }
        b'U' | b'L' | b'M' | b'V' | b'u' => Message::Minimap(minimap::parse_minimap(p, admission)?),
        b'l' => Message::Leaderboard(hud::parse_leaderboard_with_limit(
            p,
            if admission == DecodeAdmission::Sim {
                usize::MAX
            } else {
                crate::frame::MAX_FRAME_BYTES
            },
        )?),
        b'm' => Message::Announcement(hud::parse_announcement_with_limit(
            p,
            if admission == DecodeAdmission::Sim {
                usize::MAX
            } else {
                crate::frame::MAX_FRAME_BYTES
            },
        )?),
        b'S' if n == 7 => Message::Session {
            server_id: word(p, 1)?,
            session_id: u32::from_be_bytes(p[3..7].try_into().unwrap()),
        },
        b'o' if n > 1 && (n - 1).is_multiple_of(8) => {
            Message::TeamScores(TeamScoresRef { bytes: &p[1..] })
        }
        b'k' if n == 6 => Message::Kills {
            id: word(p, 1)?,
            count: triple(p, 3)?,
        },
        b'p' => {
            require(
                p,
                admission == DecodeAdmission::Sim || n == 1,
                n,
                ErrorKind::InvalidLength,
            )?;
            Message::Pong(p)
        }
        b'v' => {
            require(
                p,
                admission == DecodeAdmission::Sim || n == 2,
                n,
                ErrorKind::InvalidLength,
            )?;
            Message::LifeControl {
                status: p.get(1).copied(),
                command: p,
            }
        }
        b'z' if n == 4 => {
            let radius = triple(p, 1)?;
            require(
                p,
                admission == DecodeAdmission::Sim || radius > 0,
                1,
                ErrorKind::InvalidScalar,
            )?;
            Message::Radius(radius)
        }
        b'~' => Message::Rules(crate::extensions::slither_sim::decode(p)?),
        b'6' => Message::Challenge(&p[1..]),
        b'h' | b'r' | b'R' | b'F' | b'f' | b'b' | b'w' | b'W' | b'y' | b'z' | b'S' | b'o'
        | b'k' => return Err(invalid()),
        _ => Message::Unknown {
            opcode: code,
            command: p,
        },
    })
}
