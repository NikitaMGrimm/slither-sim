//! Client-derived borrowed HUD views. Text and score tables stay in consumers.
use crate::{DecodeError, ErrorKind, require, triple, word};
use core::iter::FusedIterator;
pub type HudPacketError = DecodeError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Leaderboard<'a> {
    command: &'a [u8],
    row_count: usize,
}
impl<'a> Leaderboard<'a> {
    pub fn command(self) -> &'a [u8] {
        self.command
    }
    pub fn local_row(self) -> u8 {
        self.command[1]
    }
    pub fn rank(self) -> u16 {
        word(self.command, 2).unwrap()
    }
    pub fn population(self) -> u16 {
        word(self.command, 4).unwrap()
    }
    pub fn row_count(self) -> usize {
        self.row_count
    }
    pub fn rows(self) -> LeaderboardRows<'a> {
        LeaderboardRows {
            remaining: &self.command[6..],
            ordinal: 1,
            count: self.row_count,
        }
    }
}
#[derive(Clone, Debug)]
pub struct LeaderboardRows<'a> {
    remaining: &'a [u8],
    ordinal: usize,
    count: usize,
}
impl<'a> Iterator for LeaderboardRows<'a> {
    type Item = LeaderboardRow<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        let end = 7 + usize::from(self.remaining[6]);
        let row = LeaderboardRow {
            bytes: &self.remaining[..end],
            ordinal: self.ordinal,
        };
        self.remaining = &self.remaining[end..];
        self.ordinal += 1;
        self.count -= 1;
        Some(row)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }
}
impl ExactSizeIterator for LeaderboardRows<'_> {}
impl FusedIterator for LeaderboardRows<'_> {}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaderboardRow<'a> {
    bytes: &'a [u8],
    ordinal: usize,
}
impl<'a> LeaderboardRow<'a> {
    pub fn bytes(self) -> &'a [u8] {
        self.bytes
    }
    pub fn ordinal(self) -> usize {
        self.ordinal
    }
    pub fn count(self) -> u16 {
        word(self.bytes, 0).unwrap()
    }
    pub fn raw_fraction(self) -> u32 {
        triple(self.bytes, 2).unwrap()
    }
    pub fn fractional_mass(self) -> f64 {
        crate::scalar::fraction24(self.raw_fraction())
    }
    pub fn raw_color(self) -> u8 {
        self.bytes[5]
    }
    pub fn palette_index(self) -> u8 {
        self.raw_color() % 9
    }
    pub fn name(self) -> &'a [u8] {
        &self.bytes[7..]
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Announcement<'a> {
    command: &'a [u8],
}
impl<'a> Announcement<'a> {
    pub fn command(self) -> &'a [u8] {
        self.command
    }
    pub fn count(self) -> u32 {
        triple(self.command, 1).unwrap()
    }
    pub fn raw_fraction(self) -> u32 {
        triple(self.command, 4).unwrap()
    }
    pub fn fractional_mass(self) -> f64 {
        crate::scalar::fraction24(self.raw_fraction())
    }
    pub fn name(self) -> &'a [u8] {
        &self.command[8..8 + usize::from(self.command[7])]
    }
    pub fn message(self) -> &'a [u8] {
        &self.command[8 + usize::from(self.command[7])..]
    }
}
pub fn parse_leaderboard(command: &[u8]) -> Result<Leaderboard<'_>, HudPacketError> {
    parse_leaderboard_with_limit(command, crate::frame::MAX_FRAME_BYTES)
}
pub fn parse_leaderboard_with_limit(
    command: &[u8],
    max_bytes: usize,
) -> Result<Leaderboard<'_>, HudPacketError> {
    require(command, command.len() <= max_bytes, 0, ErrorKind::TooLarge)?;
    opcode(command, b'l')?;
    require(
        command,
        command.len() >= 6,
        command.len(),
        ErrorKind::Truncated,
    )?;
    let mut at = 6;
    let mut row_count = 0;
    while at < command.len() {
        let length = usize::from(crate::byte(command, at + 6)?);
        require(
            command,
            command.len() - at >= 7 + length,
            command.len(),
            ErrorKind::Truncated,
        )?;
        at += 7 + length;
        row_count += 1;
    }
    Ok(Leaderboard { command, row_count })
}
pub fn parse_announcement(command: &[u8]) -> Result<Announcement<'_>, HudPacketError> {
    parse_announcement_with_limit(command, crate::frame::MAX_FRAME_BYTES)
}
pub fn parse_announcement_with_limit(
    command: &[u8],
    max_bytes: usize,
) -> Result<Announcement<'_>, HudPacketError> {
    require(command, command.len() <= max_bytes, 0, ErrorKind::TooLarge)?;
    opcode(command, b'm')?;
    let length = usize::from(crate::byte(command, 7)?);
    require(
        command,
        command.len() >= 8 + length,
        command.len(),
        ErrorKind::Truncated,
    )?;
    Ok(Announcement { command })
}
fn opcode(p: &[u8], expected: u8) -> Result<(), DecodeError> {
    require(p, !p.is_empty(), 0, ErrorKind::Empty)?;
    require(p, p[0] == expected, 0, ErrorKind::WrongOpcode)
}
