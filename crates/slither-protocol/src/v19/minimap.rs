//! Five client-derived run grammars with simulator dimension admission. No
//! retained bitmap or painting policy is stored here. Unreachable tails remain
//! opaque, including escapes beyond the last cell/layer.
use crate::{DecodeAdmission, DecodeError, ErrorKind, require, word};
use core::iter::FusedIterator;
pub const MAX_MINIMAP_DIMENSION: u16 = 512;
pub type MinimapDecodeError = DecodeError;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MinimapFormat {
    UpperU,
    LayeredL,
    SnapshotM,
    ToggleV,
    LegacyU,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinimapPacket<'a> {
    command: &'a [u8],
    format: MinimapFormat,
    header_len: usize,
}
impl<'a> MinimapPacket<'a> {
    pub fn format(self) -> MinimapFormat {
        self.format
    }
    pub fn command(self) -> &'a [u8] {
        self.command
    }
    pub fn advertised_size(self) -> Option<u16> {
        match self.format {
            MinimapFormat::UpperU | MinimapFormat::SnapshotM => {
                Some(word(self.command, 1).unwrap())
            }
            MinimapFormat::LayeredL => Some(word(self.command, 2).unwrap()),
            _ => None,
        }
    }
    pub fn layer_count(self) -> u8 {
        if self.format == MinimapFormat::LayeredL {
            self.command[1]
        } else {
            1
        }
    }
    pub fn dimension(self) -> Option<u16> {
        if self.format == MinimapFormat::LegacyU {
            Some(80)
        } else {
            self.advertised_size().map(|s| s.min(MAX_MINIMAP_DIMENSION))
        }
    }
    /// Validates reachable runs with the effective (or retained V) dimension
    /// before yielding anything. Exhausted short grids are valid.
    pub fn selected(self, dimension: u16) -> Result<SelectedCells<'a>, MinimapDecodeError> {
        require(
            self.command,
            dimension <= MAX_MINIMAP_DIMENSION,
            1,
            ErrorKind::InvalidScalar,
        )?;
        let walk = Walk {
            packet: self,
            cells: usize::from(dimension).pow(2),
            cursor: 0,
            at: self.header_len,
            layer: 0,
            bit: 0,
            bits: 0,
            visited_layers: 0,
            done: false,
        };
        let mut validation = walk.clone();
        for cell in &mut validation {
            cell?;
        }
        Ok(SelectedCells {
            walk,
            visited_layers: validation.visited_layers,
        })
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedCell {
    pub layer: u8,
    pub index: usize,
}
#[derive(Clone, Debug)]
pub struct SelectedCells<'a> {
    walk: Walk<'a>,
    visited_layers: usize,
}
impl SelectedCells<'_> {
    /// Layers with at least one reachable payload byte, including all-zero runs.
    pub fn visited_layers(&self) -> usize {
        self.visited_layers
    }
}
impl Iterator for SelectedCells<'_> {
    type Item = SelectedCell;
    fn next(&mut self) -> Option<Self::Item> {
        self.walk.next().map(|c| c.expect("validated minimap"))
    }
}
impl FusedIterator for SelectedCells<'_> {}
#[derive(Clone, Debug)]
struct Walk<'a> {
    packet: MinimapPacket<'a>,
    cells: usize,
    cursor: usize,
    at: usize,
    layer: u8,
    bit: u8,
    bits: u8,
    visited_layers: usize,
    done: bool,
}
impl Iterator for Walk<'_> {
    type Item = Result<SelectedCell, MinimapDecodeError>;
    fn next(&mut self) -> Option<Self::Item> {
        while !self.done && self.layer < self.packet.layer_count() {
            if self.cursor >= self.cells {
                self.layer += 1;
                self.cursor = 0;
                self.bit = 0;
                continue;
            }
            if self.bit != 0 {
                let set = self.bits & self.bit != 0;
                self.bit >>= 1;
                let index = if self.packet.format == MinimapFormat::LegacyU {
                    self.cursor
                } else {
                    self.cells - 1 - self.cursor
                };
                self.cursor += 1;
                if set {
                    return Some(Ok(SelectedCell {
                        layer: self.layer,
                        index,
                    }));
                }
                continue;
            }
            let Some(&byte) = self.packet.command.get(self.at) else {
                self.done = true;
                return None;
            };
            self.at += 1;
            self.visited_layers = usize::from(self.layer) + 1;
            if byte < 128 {
                self.bits = byte;
                self.bit = 64;
            } else {
                let extended = !matches!(
                    self.packet.format,
                    MinimapFormat::UpperU | MinimapFormat::LegacyU
                );
                let skip = if byte == 255 && extended {
                    let Some(&operand) = self.packet.command.get(self.at) else {
                        self.done = true;
                        return Some(Err(DecodeError::at(
                            self.packet.command,
                            self.at,
                            ErrorKind::Truncated,
                        )));
                    };
                    self.at += 1;
                    126 * usize::from(operand)
                } else {
                    usize::from(byte - 128)
                };
                self.cursor = (self.cursor + skip).min(self.cells);
            }
        }
        self.done = true;
        None
    }
}
pub fn parse_minimap(
    command: &[u8],
    admission: DecodeAdmission,
) -> Result<MinimapPacket<'_>, MinimapDecodeError> {
    let (format, header_len) = match command.first() {
        Some(b'U') => (MinimapFormat::UpperU, 3),
        Some(b'L') => (MinimapFormat::LayeredL, 4),
        Some(b'M') => (MinimapFormat::SnapshotM, 3),
        Some(b'V') => (MinimapFormat::ToggleV, 1),
        Some(b'u') => (MinimapFormat::LegacyU, 1),
        _ => return Err(DecodeError::at(command, 0, ErrorKind::WrongOpcode)),
    };
    require(
        command,
        command.len() >= header_len,
        command.len(),
        ErrorKind::Truncated,
    )?;
    let packet = MinimapPacket {
        command,
        format,
        header_len,
    };
    if admission == DecodeAdmission::Sim
        && let Some(size) = packet.advertised_size()
    {
        require(
            command,
            size != 0 && (format != MinimapFormat::SnapshotM || size <= 512),
            if format == MinimapFormat::LayeredL {
                2
            } else {
                1
            },
            ErrorKind::InvalidScalar,
        )?;
    }
    if let Some(dimension) = packet.dimension() {
        packet.selected(dimension)?;
    }
    Ok(packet)
}
