//! Client-derived envelope grammar, with explicit historical admission.
use crate::{DecodeAdmission, DecodeError, ErrorKind, require, v19::setup_protocol_version};
use core::iter::FusedIterator;

pub type FrameError = DecodeError;
pub const MAX_FRAME_BYTES: usize = 1 << 20;
pub const MAX_FRAME_COMMANDS: usize = 4096;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Header {
    Absent,
    TwoBytes,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_bytes: usize,
    pub max_commands: usize,
}
impl Limits {
    pub const CLIENT: Self = Self {
        max_bytes: MAX_FRAME_BYTES,
        max_commands: MAX_FRAME_COMMANDS,
    };
    pub const UNBOUNDED: Self = Self {
        max_bytes: usize::MAX,
        max_commands: usize::MAX,
    };
}
/// `Historical` only checks lengths; `Bounded` additionally checks command IDs
/// >=32 and client setup boundaries. Both honor the supplied limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameAdmission {
    Historical,
    Bounded,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CommandRef<'a> {
    pub index: usize,
    pub offset: usize,
    pub bytes: &'a [u8],
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedFrame<'a> {
    bytes: &'a [u8],
    start: usize,
    bare: bool,
    prefix: Option<u16>,
    count: usize,
}
impl<'a> DecodedFrame<'a> {
    /// Raw prefix; its elapsed/sequence meaning belongs to the connection.
    pub fn prefix(self) -> Option<u16> {
        self.prefix
    }
    pub fn command_count(self) -> usize {
        self.count
    }
    pub fn commands(self) -> Commands<'a> {
        Commands {
            bytes: self.bytes,
            cursor: self.start,
            index: 0,
            remaining: self.count,
            bare: self.bare,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Commands<'a> {
    bytes: &'a [u8],
    cursor: usize,
    index: usize,
    remaining: usize,
    bare: bool,
}
impl<'a> Iterator for Commands<'a> {
    type Item = CommandRef<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }
        // Construction is private and follows the full immutable prepass.
        let (at, end) = if self.bare {
            (self.cursor, self.bytes.len())
        } else {
            boundary(self.bytes, self.cursor).expect("validated envelope")
        };
        let command = CommandRef {
            index: self.index,
            offset: at,
            bytes: &self.bytes[at..end],
        };
        self.cursor = end;
        self.index += 1;
        self.remaining -= 1;
        Some(command)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}
impl ExactSizeIterator for Commands<'_> {}
impl FusedIterator for Commands<'_> {}

fn boundary(bytes: &[u8], cursor: usize) -> Result<(usize, usize), FrameError> {
    let prefix = bytes[cursor];
    let (at, length) = if prefix < 32 {
        let low = bytes.get(cursor + 1).ok_or(FrameError {
            kind: ErrorKind::Truncated,
            opcode: None,
            offset: cursor + 1,
        })?;
        (cursor + 2, usize::from(u16::from_be_bytes([prefix, *low])))
    } else {
        (cursor + 1, usize::from(prefix - 32))
    };
    if length == 0 || length > bytes.len() - at {
        return Err(FrameError {
            kind: ErrorKind::InvalidLength,
            opcode: None,
            offset: cursor,
        });
    }
    Ok((at, at + length))
}
pub fn validate_command(command: &[u8], admission: FrameAdmission) -> Result<(), FrameError> {
    require(command, !command.is_empty(), 0, ErrorKind::Empty)?;
    if admission == FrameAdmission::Bounded {
        require(command, command[0] >= 32, 0, ErrorKind::InvalidCommand)?;
        if command[0] == b'a' {
            setup_protocol_version(command, DecodeAdmission::ClientReceived)?;
        }
    }
    Ok(())
}
/// Validate the entire message before exposing an infallible repeatable iterator.
pub fn decode_frame(
    bytes: &[u8],
    header: Header,
    limits: Limits,
    admission: FrameAdmission,
) -> Result<DecodedFrame<'_>, FrameError> {
    if bytes.len() > limits.max_bytes {
        return Err(FrameError {
            kind: ErrorKind::TooLarge,
            opcode: None,
            offset: 0,
        });
    }
    let (start, prefix) = match header {
        Header::Absent => (0, None),
        Header::TwoBytes => {
            let p = bytes.get(..2).ok_or(FrameError {
                kind: ErrorKind::Truncated,
                opcode: None,
                offset: bytes.len(),
            })?;
            (2, Some(u16::from_be_bytes([p[0], p[1]])))
        }
    };
    let first = *bytes.get(start).ok_or(FrameError {
        kind: ErrorKind::Empty,
        opcode: None,
        offset: start,
    })?;
    let bare = first >= 32;
    let mut count = 0;
    let mut cursor = start;
    while cursor < bytes.len() {
        if count == limits.max_commands {
            return Err(FrameError {
                kind: ErrorKind::TooManyCommands,
                opcode: None,
                offset: cursor,
            });
        }
        let (at, end) = if bare {
            (cursor, bytes.len())
        } else {
            boundary(bytes, cursor)?
        };
        validate_command(&bytes[at..end], admission).map_err(|mut error| {
            error.offset += at;
            error
        })?;
        cursor = end;
        count += 1;
    }
    Ok(DecodedFrame {
        bytes,
        start,
        bare,
        prefix,
        count,
    })
}
