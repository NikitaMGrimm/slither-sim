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

pub(crate) fn decode(packet: &[u8]) -> Result<WorldRules, String> {
    let invalid = || "Invalid SlitherSim rules packet".to_owned();
    if packet.len() != LENGTH
        || packet[0] != COMMAND
        || packet[1] != VERSION
        || packet[3] & !15 != 0
    {
        return Err(invalid());
    }
    let boundary_mode = match packet[2] {
        0 => BoundaryMode::Sliding,
        1 => BoundaryMode::Hard,
        _ => return Err(invalid()),
    };
    let flags = packet[3];
    let optional = |bit, value: f64| -> Result<Option<f64>, String> {
        if flags & bit == 0 {
            if value != 0. {
                return Err(invalid());
            }
            Ok(None)
        } else if value.is_finite() && value > 0. {
            Ok(Some(value))
        } else {
            Err(invalid())
        }
    };
    let pit_radius = optional(1, f64::from_be_bytes(packet[4..12].try_into().unwrap()))?;
    let min = u32::from_be_bytes(packet[12..16].try_into().unwrap());
    let min_score = if flags & 2 != 0 {
        if !MassFloor::score_range().contains(&min) {
            return Err(invalid());
        }
        Some(min)
    } else {
        if min != 0 {
            return Err(invalid());
        }
        None
    };
    let kill_score_seconds = optional(4, f64::from_be_bytes(packet[16..24].try_into().unwrap()))?;
    let free_boost_at_floor = flags & 8 != 0;
    if free_boost_at_floor != min_score.is_some() {
        return Err(invalid());
    }
    Ok(WorldRules {
        boundary_mode,
        pit_radius,
        min_score,
        kill_score_seconds,
        free_boost_at_floor,
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
            let mut packet = valid.clone();
            packet[3] = 1;
            packet[4..12].copy_from_slice(&value.to_be_bytes());
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
