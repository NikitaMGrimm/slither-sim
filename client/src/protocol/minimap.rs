//! Borrowed decoding of the five minimap command families.
//!
//! Frame envelopes and aggregate byte limits belong to the existing ingress.
//! Short grids and bytes beyond the last visited grid are valid.

use std::fmt;

// The unchanged decoder tests also compile this façade as a private module.
#[allow(unused_imports)]
pub use slither_protocol::v19::minimap::MAX_MINIMAP_DIMENSION;

pub use slither_protocol::v19::minimap::MinimapFormat;

/// Invalid fixed syntax or a reachable extended run without its operand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MinimapDecodeError {
    /// The first byte is absent or is not a minimap opcode.
    WrongOpcode,
    /// A required size or layer-count header is incomplete.
    TruncatedHeader,
    /// A visited extended-run escape has no operand.
    TruncatedRunOperand,
}

impl fmt::Display for MinimapDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WrongOpcode => "expected a minimap opcode",
            Self::TruncatedHeader => "incomplete minimap header",
            Self::TruncatedRunOperand => "incomplete minimap run operand",
        })
    }
}

impl std::error::Error for MinimapDecodeError {}

/// Borrowed shared view with the client's stable error boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinimapPacket<'a>(slither_protocol::v19::minimap::MinimapPacket<'a>);
impl<'a> MinimapPacket<'a> {
    /// Wire family.
    pub fn format(self) -> MinimapFormat {
        self.0.format()
    }
    /// Uncapped wire size, absent for V and legacy u.
    #[cfg_attr(test, allow(dead_code))] // Some unchanged tests include this file privately.
    pub fn advertised_size(self) -> Option<u16> {
        self.0.advertised_size()
    }
    /// Layer count for L, otherwise one.
    pub fn layer_count(self) -> u8 {
        self.0.layer_count()
    }
    /// Complete original command, including ignored tail.
    pub fn command(self) -> &'a [u8] {
        self.0.command()
    }
    /// Effective size; V requires the consumer's retained dimension.
    pub fn dimension(self) -> Option<u16> {
        self.0.dimension()
    }
    pub(crate) fn visit_selected(
        self,
        dimension: u16,
        mut selected: impl FnMut(u8, usize),
    ) -> Result<usize, MinimapDecodeError> {
        let cells = self
            .0
            .selected(dimension)
            .map_err(|_| MinimapDecodeError::TruncatedRunOperand)?;
        let layers = cells.visited_layers();
        for cell in cells {
            selected(cell.layer, cell.index);
        }
        Ok(layers)
    }
}

/// Parse one bare command with the client's dimension and run admission.
pub fn parse_minimap(command: &[u8]) -> Result<MinimapPacket<'_>, MinimapDecodeError> {
    use slither_protocol::{DecodeAdmission, ErrorKind, v19::minimap};
    minimap::parse_minimap(command, DecodeAdmission::ClientReceived)
        .map(MinimapPacket)
        .map_err(|error| match error.kind {
            ErrorKind::WrongOpcode => MinimapDecodeError::WrongOpcode,
            _ if error.offset
                >= match command.first() {
                    Some(b'U' | b'M') => 3,
                    Some(b'L') => 4,
                    _ => 1,
                } =>
            {
                MinimapDecodeError::TruncatedRunOperand
            }
            _ => MinimapDecodeError::TruncatedHeader,
        })
}
