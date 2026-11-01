//! Validated login construction and bounded frame splitting without network I/O.

pub use slither_protocol::frame::{CommandRef, Header as ElapsedHeader};
use slither_protocol::{DecodeAdmission, ErrorKind, frame, v19};
use std::fmt;

pub mod hud;
pub mod minimap;

pub use slither_protocol::frame::{MAX_FRAME_BYTES, MAX_FRAME_COMMANDS};

/// Validation failures for the prototype protocol boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProtocolError {
    /// Nicknames contain at most 24 ASCII bytes in 0x20..=0x7f.
    Nickname,
    /// Custom skin data contains at most 255 bytes.
    CustomSkin,
    /// No payload was present.
    EmptyFrame,
    /// The frame exceeded the byte bound.
    FrameTooLarge,
    /// An elapsed header was incomplete.
    TruncatedElapsed,
    /// A two-byte packet length was incomplete.
    TruncatedLength,
    /// A packet length was zero or exceeded the remaining frame.
    InvalidLength,
    /// A bundled command had no printable-or-higher command byte.
    InvalidCommand,
    /// The frame exceeded the command count bound.
    TooManyCommands,
    /// The command was not a setup command.
    NotSetup,
    /// Setup ended before the mandatory fields or within an optional field.
    TruncatedSetup,
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Nickname => "nickname must contain at most 24 ASCII bytes in 0x20..=0x7f",
            Self::CustomSkin => "custom skin must contain at most 255 bytes",
            Self::EmptyFrame => "frame has no command payload",
            Self::FrameTooLarge => "frame exceeds 1048576 bytes",
            Self::TruncatedElapsed => "frame has a truncated two-byte elapsed header",
            Self::TruncatedLength => "frame has a truncated two-byte command length",
            Self::InvalidLength => "command length is zero or exceeds remaining frame bytes",
            Self::InvalidCommand => "command byte must be at least 32",
            Self::TooManyCommands => "frame exceeds 4096 commands",
            Self::NotSetup => "expected setup command 'a'",
            Self::TruncatedSetup => "setup command has incomplete mandatory or optional fields",
        })
    }
}

impl std::error::Error for ProtocolError {}

/// Construct a login with caller-supplied version and page echo bytes.
///
/// The page echo is the stock page literal copied after the version.
/// Empty nicknames are allowed. No official profile values are embedded.
/// Custom skin bytes are appended after the two appearance markers.
pub fn encode_login(
    client_version: u16,
    page_echo: &[u8; 20],
    nickname: &str,
    skin: u8,
    custom_skin: Option<&[u8]>,
) -> Result<Vec<u8>, ProtocolError> {
    if nickname.len() > 24 || !nickname.bytes().all(|b| (b' '..=b'\x7f').contains(&b)) {
        return Err(ProtocolError::Nickname);
    }
    let custom_skin = custom_skin.unwrap_or_default();
    if custom_skin.len() > 255 {
        return Err(ProtocolError::CustomSkin);
    }
    let mut bytes = vec![0; 28 + nickname.len() + custom_skin.len()];
    v19::client::write_login(
        v19::client::ModernLogin {
            client_version,
            page_echo,
            nickname: nickname.as_bytes(),
            skin,
            custom_skin,
        },
        &mut bytes,
    )
    // The local checks prove the encoder preconditions. Keep invariant failures
    // static so this adapter does not retain the shared error's Debug formatter.
    .unwrap_or_else(|_| panic!("validated login and exact output capacity"));
    Ok(bytes)
}

/// Encode the protocol-19 victory message on the existing connection.
/// The server accepts one bounded ASCII payload; see docs/raw-server.md, local records.
/// Substitute per UTF-16 unit before truncation, including isolated surrogates.
pub fn encode_champion(units: &[u16]) -> Vec<u8> {
    let text = crate::menu_input::TextRule::Champion.normalize_units(units);
    let mut bytes = vec![0; 2 + text.len()];
    v19::client::write_champion(text.as_bytes(), &mut bytes)
        .unwrap_or_else(|_| panic!("normalized champion text and exact output capacity"));
    bytes
}

/// A structurally validated frame borrowing its complete command slices.
#[derive(Debug, Eq, PartialEq)]
pub struct DecodedFrame<'a> {
    /// Elapsed header, only in the explicitly selected timed mode.
    pub elapsed_ms: Option<u16>,
    /// Complete command bytes, including each command byte.
    pub commands: CommandSlices<'a>,
}
impl<'a> std::ops::Deref for DecodedFrame<'a> {
    type Target = frame::DecodedFrame<'a>;
    fn deref(&self) -> &Self::Target {
        &self.commands.0
    }
}

/// Repeatable borrowed command slices for the public framing façade.
#[derive(Clone, Copy, Debug)]
pub struct CommandSlices<'a>(frame::DecodedFrame<'a>);
impl<'a> CommandSlices<'a> {
    /// Number of validated commands.
    pub fn len(self) -> usize {
        self.0.command_count()
    }
    /// Whether this view contains no commands.
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    /// Traverse complete command slices without allocating a command list.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &'a [u8]> + Clone {
        self.0.commands().map(|c| c.bytes)
    }
}
impl PartialEq for CommandSlices<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
impl Eq for CommandSlices<'_> {}
impl<T: AsRef<[u8]>, const N: usize> PartialEq<[T; N]> for CommandSlices<'_> {
    fn eq(&self, other: &[T; N]) -> bool {
        self.iter().eq(other.iter().map(AsRef::as_ref))
    }
}
impl std::ops::Index<usize> for CommandSlices<'_> {
    type Output = [u8];
    fn index(&self, index: usize) -> &Self::Output {
        self.0
            .commands()
            .nth(index)
            .expect("command index out of bounds")
            .bytes
    }
}

/// Allocation-free reader used by production ingress. The complete envelope is
/// validated before any repeatable, message-relative command view is returned.
pub fn read_frame(
    bytes: &[u8],
    header: ElapsedHeader,
) -> Result<frame::DecodedFrame<'_>, ProtocolError> {
    frame::decode_frame(
        bytes,
        header,
        frame::Limits::CLIENT,
        frame::FrameAdmission::Bounded,
    )
    .map_err(|error| match error.kind {
        ErrorKind::TooLarge => ProtocolError::FrameTooLarge,
        ErrorKind::TooManyCommands => ProtocolError::TooManyCommands,
        ErrorKind::Empty => ProtocolError::EmptyFrame,
        ErrorKind::InvalidCommand => ProtocolError::InvalidCommand,
        _ if error.opcode == Some(b'a') => ProtocolError::TruncatedSetup,
        ErrorKind::Truncated if header == ElapsedHeader::TwoBytes && bytes.len() < 2 => {
            ProtocolError::TruncatedElapsed
        }
        ErrorKind::Truncated => ProtocolError::TruncatedLength,
        _ => ProtocolError::InvalidLength,
    })
}

/// Borrowed slice façade for callers that do not need command offsets/indices.
pub fn decode_frame(
    bytes: &[u8],
    header: ElapsedHeader,
) -> Result<DecodedFrame<'_>, ProtocolError> {
    let frame = read_frame(bytes, header)?;
    Ok(DecodedFrame {
        elapsed_ms: frame.prefix(),
        commands: CommandSlices(frame),
    })
}

/// Return the advertised setup protocol after validating field boundaries.
pub fn setup_protocol_version(command: &[u8]) -> Result<u8, ProtocolError> {
    v19::setup_protocol_version(command, DecodeAdmission::ClientReceived).map_err(|error| {
        if error.kind == ErrorKind::WrongOpcode {
            ProtocolError::NotSetup
        } else {
            ProtocolError::TruncatedSetup
        }
    })
}
