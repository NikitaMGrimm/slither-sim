//! Serialization only: profile choice, text normalization and send cadence are
//! caller policy. Variable builders validate everything before writing.
use crate::ErrorKind;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EncodeError {
    pub kind: ErrorKind,
    pub opcode: u8,
    pub offset: usize,
}
#[derive(Clone, Copy, Debug)]
pub struct ModernLogin<'a> {
    pub client_version: u16,
    pub page_echo: &'a [u8; 20],
    pub nickname: &'a [u8],
    pub skin: u8,
    pub custom_skin: &'a [u8],
}
pub fn write_login(login: ModernLogin<'_>, out: &mut [u8]) -> Result<usize, EncodeError> {
    let error = |kind, offset| EncodeError {
        kind,
        opcode: b's',
        offset,
    };
    if login.nickname.len() > 24 {
        return Err(error(ErrorKind::InvalidLength, 25));
    }
    if login.custom_skin.len() > 255 {
        return Err(error(ErrorKind::InvalidLength, 28 + login.nickname.len()));
    }
    let len = 28 + login.nickname.len() + login.custom_skin.len();
    if out.len() < len {
        return Err(error(ErrorKind::OutputCapacity, out.len()));
    }
    out[..2].copy_from_slice(&[b's', 30]);
    out[2..4].copy_from_slice(&login.client_version.to_be_bytes());
    out[4..24].copy_from_slice(login.page_echo);
    out[24] = login.skin;
    out[25] = login.nickname.len() as u8;
    let end = 26 + login.nickname.len();
    out[26..end].copy_from_slice(login.nickname);
    out[end..end + 2].copy_from_slice(&[0, 255]);
    out[end + 2..len].copy_from_slice(login.custom_skin);
    Ok(len)
}
/// `text` has already undergone the consumer's UTF-16 normalization/truncation.
pub fn write_champion(text: &[u8], out: &mut [u8]) -> Result<usize, EncodeError> {
    let error = |kind, offset| EncodeError {
        kind,
        opcode: 255,
        offset,
    };
    if text.len() > 140 {
        return Err(error(ErrorKind::InvalidLength, 2));
    }
    let len = 2 + text.len();
    if out.len() < len {
        return Err(error(ErrorKind::OutputCapacity, out.len()));
    }
    out[..2].copy_from_slice(&[255, b'v']);
    out[2..len].copy_from_slice(text);
    Ok(len)
}
pub fn heading_byte(value: u8) -> Result<[u8; 1], EncodeError> {
    if value > 250 {
        return Err(EncodeError {
            kind: ErrorKind::InvalidScalar,
            opcode: value,
            offset: 0,
        });
    }
    Ok([value])
}
/// Simulator action quantization. Client pointer policy retains its distinct
/// normalization and multiplication order and can call `heading_byte` instead.
pub fn sim_heading(heading: f64) -> Result<[u8; 1], EncodeError> {
    if !heading.is_finite() {
        return Err(EncodeError {
            kind: ErrorKind::InvalidScalar,
            opcode: 0,
            offset: 0,
        });
    }
    let tau = core::f64::consts::TAU;
    // Equivalent to rem_euclid for finite heading and a positive TAU. The
    // nonnegative float-to-integer cast is floor, including -0.0 -> 0.
    let remainder = heading % tau;
    let normalized = if remainder < 0. {
        remainder + tau
    } else {
        remainder
    };
    let quantized = (normalized * 251. / tau) as u16;
    Ok([quantized.min(250) as u8])
}
pub fn boost(enabled: bool) -> [u8; 1] {
    [if enabled { 253 } else { 254 }]
}
pub fn turn(increasing: bool, count: u8) -> Result<[u8; 2], EncodeError> {
    if !(1..=127).contains(&count) {
        return Err(EncodeError {
            kind: ErrorKind::InvalidScalar,
            opcode: 252,
            offset: 1,
        });
    }
    Ok([252, if increasing { count + 128 } else { count }])
}
pub const PING: [u8; 1] = [251];
pub const REQUEST_BARE: [u8; 1] = [1];
pub const REQUEST_SEQUENCE: [u8; 1] = [2];
pub const REQUEST_SESSION: [u8; 1] = [4];
pub const STOCK_HELLO: [u8; 2] = [99, 0];
pub const SHORT_HELLO: [u8; 1] = [99];
pub const LEGACY_HELLO: [u8; 2] = [99, 3];
pub const NTL_HELLO: [u8; 2] = [99, 4];
