//! Measurement-only packet classification through the production shared reader.
use super::CommandRef;
use slither_protocol::{
    DecodeAdmission,
    v19::{self, Message, Target},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
/// Distinct wire changes, retained separately when measuring report cadence.
pub enum PoseKind {
    /// A new or replaced appearance, which starts a new comparison lifetime.
    Appearance,
    /// Visibility removal or death, which ends that lifetime.
    Removal,
    /// A position/body anchor update; this refreshes the received head cache.
    Anchor,
    /// Heading, target or speed fields; these do not refresh the head cache.
    Motion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Scalar packet identity without raw payload, reconstruction or policy work.
pub struct PosePacket {
    /// None is the protocol's implicit own target, resolved by the observing host.
    pub id: Option<u16>,
    /// The changed channel, independent of other commands in the same frame.
    pub kind: PoseKind,
}

/// Observe command identity only; never reconstruct another position or decoder.
/// Invalid commands do not supply telemetry; production ingress still owns errors.
pub fn pose_packet(command: CommandRef<'_>) -> Option<PosePacket> {
    if !matches!(
        *command.bytes.first()?,
        b's' | b'g'
            | b'n'
            | b'G'
            | b'N'
            | b'='
            | b'+'
            | b'e'
            | b'E'
            | b'3'
            | b'4'
            | b'5'
            | b'd'
            | b'7'
    ) {
        return None;
    }
    let target = |t| match t {
        Target::Own => Some(None),
        Target::Id(id) => Some(Some(id)),
        _ => None,
    };
    let (id, kind) = match v19::decode_command(command, DecodeAdmission::ClientPresentation).ok()? {
        Message::SnakeAppearance(s) => (Some(s.id), PoseKind::Appearance),
        Message::SnakeRemoval(s) => (Some(s.id), PoseKind::Removal),
        Message::Anchor(s) => (target(s.target)?, PoseKind::Anchor),
        Message::Motion(s) => (target(s.target)?, PoseKind::Motion),
        _ => return None,
    };
    Some(PosePacket { id, kind })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn classify(bytes: &[u8]) -> Option<PosePacket> {
        let frame =
            crate::protocol::read_frame(bytes, crate::protocol::ElapsedHeader::Absent).unwrap();
        pose_packet(frame.commands().next().unwrap())
    }
    #[test]
    fn position_anchors_motion_and_removal_preserve_implicit_own_targets() {
        assert_eq!(
            classify(&[b'g', 0, 9]),
            Some(PosePacket {
                id: Some(9),
                kind: PoseKind::Anchor
            })
        );
        assert_eq!(
            classify(&[b'G', 0, 0]),
            Some(PosePacket {
                id: None,
                kind: PoseKind::Anchor
            })
        );
        assert_eq!(classify(&[b'=', 0, 0, 0, 50, 0, 50]).unwrap().id, None);
        assert_eq!(
            classify(&[b'=', 0, 9, 0, 0, 0, 50, 0, 50]).unwrap().id,
            Some(9)
        );
        assert_eq!(
            classify(&[b'e', 0, 9, 32]),
            Some(PosePacket {
                id: Some(9),
                kind: PoseKind::Motion
            })
        );
        assert_eq!(classify(&[b'e', 32]).unwrap().id, None);
        assert_eq!(classify(&[b's', 0, 9]).unwrap().kind, PoseKind::Removal);
        assert_eq!(classify(b"p"), None);
        assert_eq!(classify(b"g"), None);
        assert_eq!(
            pose_packet(CommandRef {
                index: 0,
                offset: 0,
                bytes: &[]
            }),
            None
        );
    }
}
