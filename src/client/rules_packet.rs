//! SlitherSim's opt-in rules extension, never sent to stock browser sockets.
use crate::{BoundaryMode, WorldRules, physics::MassFloor};

/// Outside the pinned stock client's command handlers; see docs/raw-server.md.
pub(crate) const COMMAND: u8 = b'~';
const VERSION: u8 = 1;
const LENGTH: usize = 24;

pub(crate) fn encode(rules: WorldRules) -> Vec<u8> {
    let flags = u8::from(rules.pit_radius.is_some())
        | (u8::from(rules.min_score.is_some()) << 1)
        | (u8::from(rules.kill_score_seconds.is_some()) << 2)
        | (u8::from(rules.free_boost_at_floor) << 3);
    let boundary = match rules.boundary_mode {
        BoundaryMode::Sliding => 0,
        BoundaryMode::Hard => 1,
    };
    let mut packet = Vec::with_capacity(LENGTH);
    packet.extend([COMMAND, VERSION, boundary, flags]);
    packet.extend(rules.pit_radius.unwrap_or(0.).to_be_bytes());
    packet.extend(rules.min_score.unwrap_or(0).to_be_bytes());
    packet.extend(rules.kill_score_seconds.unwrap_or(0.).to_be_bytes());
    packet
}

#[cfg(test)]
pub(crate) fn decode(packet: &[u8]) -> Result<WorldRules, String> {
    let wire = slither_protocol::extensions::slither_sim::decode(packet)
        .map_err(|_| "Invalid SlitherSim rules packet".to_owned())?;
    from_wire(wire)
}

pub(crate) fn from_wire(
    wire: slither_protocol::extensions::slither_sim::RulesRef<'_>,
) -> Result<WorldRules, String> {
    let min_score = (wire.flags & 2 != 0).then_some(wire.raw_min_score);
    if min_score.is_some_and(|min| !MassFloor::score_range().contains(&min)) {
        return Err("Invalid SlitherSim rules packet".into());
    }
    Ok(WorldRules {
        boundary_mode: if wire.boundary == 0 {
            BoundaryMode::Sliding
        } else {
            BoundaryMode::Hard
        },
        pit_radius: (wire.flags & 1 != 0).then(|| f64::from_bits(wire.pit_radius_bits)),
        min_score,
        kill_score_seconds: (wire.flags & 4 != 0)
            .then(|| f64::from_bits(wire.kill_score_seconds_bits)),
        free_boost_at_floor: wire.flags & 8 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Config, Simulator, WorldOptions,
        client::{ClientState, encode as wire},
    };

    #[test]
    fn rules_round_trip_exactly_through_the_received_frame() {
        for config in [
            Config::sandbox(),
            Config::battledome(),
            Config {
                boundary_mode: BoundaryMode::Hard,
                ..Config::battledome()
            },
        ] {
            let world = Simulator::new(WorldOptions {
                config,
                ..Default::default()
            })
            .unwrap();
            let mut rules = world.rules();
            rules.pit_radius = rules.pit_radius.map(|p| p + 0.123456789);
            let packet = encode(rules);
            assert_eq!(packet.len(), LENGTH);
            assert_eq!(decode(&packet).unwrap(), rules);
            let mut client = ClientState::new();
            client.new_connection(0.).unwrap();
            assert!(
                client.receive_packet(&packet, 0., 0.).is_err(),
                "setup is required"
            );
            client
                .receive_frame(
                    &wire::frame(0, &[wire::setup(3584., 4.).unwrap(), packet]).unwrap(),
                    0.,
                )
                .unwrap();
            assert_eq!(client.frame(0.).unwrap().rules, Some(rules));
            client.new_connection(1.).unwrap();
            assert_eq!(client.frame(1.).unwrap().rules, None);
        }
    }

    #[test]
    fn malformed_rules_reject_the_entire_frame_before_mutation() {
        let valid = encode(WorldRules::default());
        let mut cases = Vec::new();
        for len in 0..LENGTH {
            cases.push(valid[..len].to_vec());
        }
        let mut extra = valid.clone();
        extra.push(0);
        cases.push(extra);
        for (offset, value) in [(1, 2), (2, 2), (3, 16), (3, 8)] {
            let mut packet = valid.clone();
            packet[offset] = value;
            cases.push(packet);
        }
        for value in [0., -1., f64::NAN, f64::INFINITY] {
            for (flag, offset) in [(1, 4), (4, 16)] {
                let mut packet = valid.clone();
                packet[3] = flag;
                packet[offset..offset + 8].copy_from_slice(&value.to_be_bytes());
                cases.push(packet);
            }
        }
        for min in [0_u32, u32::MAX] {
            let mut packet = valid.clone();
            packet[3] = 2 | 8;
            packet[12..16].copy_from_slice(&min.to_be_bytes());
            cases.push(packet);
        }
        // Absent fields must not hide nonzero values.
        for offset in [4, 12, 16] {
            let mut packet = valid.clone();
            packet[offset] = 1;
            cases.push(packet);
        }
        let mut client = ClientState::new();
        client.new_connection(0.).unwrap();
        client
            .receive_packet(&wire::setup(3584., 4.).unwrap(), 0., 0.)
            .unwrap();
        let before = client.frame(0.).unwrap();
        for packet in cases {
            assert!(decode(&packet).is_err());
            if packet.first() == Some(&COMMAND) {
                let frame = wire::frame(0, &[wire::radius(4000.).unwrap(), packet]).unwrap();
                assert!(client.receive_frame(&frame, 0.).is_err());
                assert_eq!(client.frame(0.).unwrap(), before);
            }
        }
    }
}
