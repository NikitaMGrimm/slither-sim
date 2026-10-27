//! Research-only child module of client::simulation, identical on both arms.
use super::{PlayerAppearance, RawSimulator, SimulationConfig};
use std::{hint::black_box, io::{self, Write}};

pub fn run(case: &str, trace: bool) {
    let (count, legacy) = match case {
        "official500" => (500, 0),
        "official500-half" => (500, 250),
        "dome64" => (64, 0),
        "dome80" => (80, 0),
        "dome80-legacy" => (80, 80),
        _ => panic!("unknown case"),
    };
    let dome = count < 500;
    let mut config = SimulationConfig {
        agents: count,
        world: if dome { crate::Config::battledome() } else { crate::Config::official(count) },
        ..Default::default()
    };
    if dome {
        // Retain the original study's wide initial arena via a public replay
        // update. The current controller then shrinks it normally; this is a
        // synthetic admission fixture, separate from the default live census.
        config.world.radius_updates.push(crate::RadiusUpdate {
            time: 0., radius: 2. * config.world.seat_radius(count),
        });
    }
    let mut sim = RawSimulator::new(7, config, count).unwrap();
    // Controlled dense fixture, not a capacity/admission qualification. Use
    // current seating and the existing two-second guarded admission path.
    sim.backend.world.grow_for_seats(count, true);
    let profile = |slot| PlayerAppearance {
        nickname: format!("Local{slot}").into_bytes(), skin: (slot % 9) as u8,
        ..Default::default()
    };
    for slot in 0..count {
        assert!(sim.connect_waited(slot, profile(slot), 2.).unwrap(), "initial slot {slot}");
        if slot < legacy { sim.protocol13(slot); }
    }
    let mut output = io::BufWriter::new(io::stdout().lock());
    let mut record = |tag: u64, bytes: &[u8]| {
        if trace {
            output.write_all(&tag.to_le_bytes()).unwrap();
            output.write_all(&(bytes.len() as u64).to_le_bytes()).unwrap();
            output.write_all(bytes).unwrap();
        }
    };
    if trace { record(0, &serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap()); }
    let (mut frames, mut bytes, mut maps, mut reconnects, mut refused) = (0u64,0u64,0u64,0u64,0u64);
    let (mut population_sum, mut population_min) = (0usize,count);
    let mut forced = false;
    for tick in 0..1080usize {
        if tick == 720 { sim.disconnect(0).unwrap(); }
        if tick.is_multiple_of(72) {
            for slot in 0..count {
                if !sim.backend.world.snakes()[slot].alive {
                    if sim.connect_waited(slot, profile(slot), 2.).unwrap() {
                        reconnects += 1;
                        if slot < legacy { sim.protocol13(slot); }
                        if tick == 720 && slot == 0 { forced = true; }
                    } else { refused += 1; }
                }
            }
        }
        if tick.is_multiple_of(6) {
            for slot in 0..count {
                if !sim.backend.slots[slot].connection.open { continue; }
                let now = sim.backend.time();
                let heading = ((slot * 37 + tick / 6 * 3) % 251) as u8;
                sim.receive_at(slot, &[heading], now).unwrap();
                sim.receive_at(slot, &[if (tick / 6 + slot) % 5 == 0 {253} else {254}], now).unwrap();
            }
        }
        sim.advance_tick_with(0, &mut |_| Ok(())).unwrap();
        let population = sim.backend.world.snakes().iter().filter(|s| s.alive).count();
        population_sum += population;
        population_min = population_min.min(population);
        for slot in 0..count {
            for (at, frame) in sim.take(slot).unwrap() {
                frames += 1; bytes += frame.len() as u64;
                for p in crate::client::protocol::split_frame(&frame).unwrap() {
                    if b"MVU".contains(&p[0]) { maps += 1; }
                }
                if trace {
                    let mut header = Vec::with_capacity(24);
                    header.extend_from_slice(&(tick as u64).to_le_bytes());
                    header.extend_from_slice(&(slot as u64).to_le_bytes());
                    header.extend_from_slice(&at.to_bits().to_le_bytes());
                    record(1, &header); record(2, &frame);
                }
                black_box(frame);
            }
        }
    }
    assert!(forced && reconnects > 0 && maps >= count as u64 && frames > 0);
    let summary = serde_json::json!({"case":case,"ticks":1080,"seats":count,"legacy":legacy,
        "frames":frames,"bytes":bytes,"maps":maps,"reconnects":reconnects,"refused":refused,
        "forcedReconnect":forced,"populationSum":population_sum,"populationMin":population_min,
        "populationFinal":sim.backend.world.snakes().iter().filter(|s|s.alive).count()});
    if trace { record(3, &serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap()); }
    record(4, &serde_json::to_vec(&summary).unwrap());
    drop(record); output.flush().unwrap();
    eprintln!("{summary}");
}
