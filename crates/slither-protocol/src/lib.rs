//! Allocation-free wire reading. Consumer state, admission phases and clocks stay outside.
#![no_std]
#![forbid(unsafe_code)]

pub mod challenge;
pub mod extensions;
pub mod frame;
pub mod scalar;
pub mod v19;

/// Structural and scalar failures; unknown opcodes are returned as values.
/// Callers must include a wildcard arm for future error kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    Empty,
    TooLarge,
    TooManyCommands,
    Truncated,
    InvalidLength,
    InvalidCommand,
    WrongOpcode,
    UnsupportedVersion,
    InvalidScalar,
    OutputCapacity,
    Syntax,
    Literal,
    Alphabet,
    Nibble,
}

/// A byte location in the original frame when decoded through `decode_command`.
/// Direct family parsers report command-relative locations. `None` means no opcode
/// was available (for example, an incomplete envelope length).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeError {
    pub kind: ErrorKind,
    pub opcode: Option<u8>,
    pub offset: usize,
}
impl DecodeError {
    pub(crate) fn at(bytes: &[u8], offset: usize, kind: ErrorKind) -> Self {
        Self {
            kind,
            opcode: bytes.first().copied(),
            offset,
        }
    }
}

/// Existing consumer admission contracts, independent of package/wire versions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeAdmission {
    Sim,
    ClientReceived,
    ClientPresentation,
}

pub(crate) fn require(
    p: &[u8],
    valid: bool,
    offset: usize,
    kind: ErrorKind,
) -> Result<(), DecodeError> {
    if valid {
        Ok(())
    } else {
        Err(DecodeError::at(p, offset, kind))
    }
}
pub(crate) fn byte(p: &[u8], at: usize) -> Result<u8, DecodeError> {
    p.get(at)
        .copied()
        .ok_or_else(|| DecodeError::at(p, at, ErrorKind::Truncated))
}
pub(crate) fn word(p: &[u8], at: usize) -> Result<u16, DecodeError> {
    Ok(u16::from_be_bytes([byte(p, at)?, byte(p, at + 1)?]))
}
pub(crate) fn triple(p: &[u8], at: usize) -> Result<u32, DecodeError> {
    Ok(u32::from_be_bytes([
        0,
        byte(p, at)?,
        byte(p, at + 1)?,
        byte(p, at + 2)?,
    ]))
}
