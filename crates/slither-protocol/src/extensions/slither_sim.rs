//! Decoder-only local extension. The independent server encoder and the
//! physics-owned supported score range remain in slither-sim.
use crate::{DecodeError, ErrorKind, require};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RulesRef<'a> {
    pub command: &'a [u8],
    pub version: u8,
    pub boundary: u8,
    pub flags: u8,
    pub pit_radius_bits: u64,
    pub raw_min_score: u32,
    pub kill_score_seconds_bits: u64,
}
pub fn decode(p: &[u8]) -> Result<RulesRef<'_>, DecodeError> {
    require(p, p.len() == 24, p.len(), ErrorKind::InvalidLength)?;
    require(p, p[0] == b'~', 0, ErrorKind::WrongOpcode)?;
    require(p, p[1] == 1, 1, ErrorKind::UnsupportedVersion)?;
    require(p, p[2] <= 1, 2, ErrorKind::InvalidScalar)?;
    require(p, p[3] & !15 == 0, 3, ErrorKind::InvalidScalar)?;
    let r = RulesRef {
        command: p,
        version: p[1],
        boundary: p[2],
        flags: p[3],
        pit_radius_bits: u64::from_be_bytes(p[4..12].try_into().unwrap()),
        raw_min_score: u32::from_be_bytes(p[12..16].try_into().unwrap()),
        kill_score_seconds_bits: u64::from_be_bytes(p[16..24].try_into().unwrap()),
    };
    for (bit, value, at) in [
        (1, f64::from_bits(r.pit_radius_bits), 4),
        (4, f64::from_bits(r.kill_score_seconds_bits), 16),
    ] {
        require(
            p,
            if r.flags & bit == 0 {
                value == 0.
            } else {
                value.is_finite() && value > 0.
            },
            at,
            ErrorKind::InvalidScalar,
        )?;
    }
    require(
        p,
        r.flags & 2 != 0 || r.raw_min_score == 0,
        12,
        ErrorKind::InvalidScalar,
    )?;
    require(
        p,
        (r.flags & 8 != 0) == (r.flags & 2 != 0),
        3,
        ErrorKind::InvalidScalar,
    )?;
    Ok(r)
}
