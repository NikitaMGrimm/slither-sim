//! Numeric receipt counters only; never retain packet bodies or change session policy.
use slither_client::protocol::{self, ElapsedHeader};
use slither_protocol::{DecodeAdmission, v13, v19};

#[derive(Default, Debug, PartialEq)]
pub(super) struct WireCounts {
    pub pings: u64,
    pub pongs: u64,
    pub own_moves: u64,
    pub own_growth: u64,
    pub own_motion: u64,
    pub own_mass: u64,
    pub own_tail: u64,
    pub own_removal: u64,
}
impl WireCounts {
    pub fn sent(&mut self, bytes: &[u8]) {
        self.pings += u64::from(bytes == [251]);
    }

    /// Called after successful ingress/effect/picture drains. These count
    /// recognized receipts addressed to the retained own ID, not prediction.
    pub fn received(&mut self, bytes: &[u8], version: Option<u8>, own: Option<u16>) {
        let Ok(frame) = protocol::read_frame(bytes, ElapsedHeader::Absent) else {
            return;
        };
        for command in frame.commands() {
            if !matches!(
                command.bytes[0],
                b'p' | b's'
                    | b'g'
                    | b'G'
                    | b'n'
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
                    | b'h'
                    | b'r'
            ) {
                continue;
            }
            if version == Some(13) {
                match v13::decode_command(command, DecodeAdmission::ClientReceived) {
                    Ok(v13::Message::Pong(_)) => self.pongs += 1,
                    Ok(v13::Message::Anchor(a)) if own == Some(a.id) => self.anchor(a.grew),
                    Ok(v13::Message::Motion(m)) if own == Some(m.id) => self.own_motion += 1,
                    Ok(v13::Message::Mass { id, .. }) if own == Some(id) => self.own_mass += 1,
                    Ok(v13::Message::Tail { id, .. }) if own == Some(id) => self.own_tail += 1,
                    Ok(v13::Message::SnakeRemoval(r)) if own == Some(r.id) => self.own_removal += 1,
                    _ => {}
                }
            } else if version == Some(19) {
                match v19::decode_command(command, DecodeAdmission::ClientReceived) {
                    Ok(v19::Message::Pong(_)) => self.pongs += 1,
                    Ok(v19::Message::Anchor(a)) if is_own(a.target, own) => self.anchor(a.grew),
                    Ok(v19::Message::Motion(m)) if is_own(m.target, own) => self.own_motion += 1,
                    Ok(v19::Message::Mass { id, .. }) if own == Some(id) => self.own_mass += 1,
                    Ok(v19::Message::Tail { id, .. }) if own == Some(id) => self.own_tail += 1,
                    Ok(v19::Message::SnakeRemoval(r)) if own == Some(r.id) => self.own_removal += 1,
                    _ => {}
                }
            }
        }
    }
    fn anchor(&mut self, grew: bool) {
        self.own_moves += 1;
        self.own_growth += u64::from(grew);
    }
}
fn is_own(target: v19::Target, own: Option<u16>) -> bool {
    match target {
        v19::Target::Own => own.is_some(),
        v19::Target::Id(id) => own == Some(id),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counters_use_matching_readers_and_full_ids_including_zero() {
        let mut c = WireCounts::default();
        c.sent(&[251]);
        c.sent(&[1]);
        c.sent(&[251, 0]);
        for p in [
            vec![b'p'],
            vec![b'p', 0],
            vec![b'g', 0, 0, 3, 232, 3, 232],
            vec![b'G', 0, 0, 128, 128],
            vec![b'N', 0, 0, 128, 128, 0, 0, 0],
            vec![b'G', 1, 0, 128, 128], // Peer 256 must not alias own 0.
            vec![b'e', 0, 0, 32],
            vec![b'h', 0, 0, 0, 0, 0],
            vec![b'r', 0, 0],
            vec![b's', 0, 0, 1],
            vec![b'd', 1, 2, 3],
        ] {
            c.received(&p, Some(13), Some(0));
        }
        assert_eq!(
            c,
            WireCounts {
                pings: 1,
                pongs: 1,
                own_moves: 3,
                own_growth: 1,
                own_motion: 1,
                own_mass: 1,
                own_tail: 1,
                own_removal: 1
            }
        );
        let mut modern = WireCounts::default();
        for p in [
            vec![b'G', 0, 0],
            vec![b'N', 0, 0, 0, 0, 0],
            vec![b'd', 1, 2, 3],
            vec![b'p'],
        ] {
            modern.received(&p, Some(19), Some(0));
        }
        assert_eq!(modern.own_moves, 2);
        assert_eq!(modern.own_growth, 1);
        assert_eq!(modern.own_motion, 1);
        assert_eq!(modern.pongs, 1);
    }
}
