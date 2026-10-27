//! Research-only child module of client::simulation, identical on both arms.
use super::{PlayerAppearance, RawSimulator, SimulationConfig};
mod tick_counter;
use std::{hint::black_box, io::{self, Write}};

pub fn run(case: &str, trace: bool) {
    let (count, legacy) = match case {
        "official500" => (500, 0),
        "official500-half" => (500, 250),
        "official500-legacy" => (500, 500),
        _ => panic!("unknown case"),
    };
    let config = SimulationConfig {
        agents: count,
        world: crate::Config::official(count),
        ..Default::default()
    };
    let mut sim = RawSimulator::new(7, config, count).unwrap();
    // Current official-500 configuration; no custom geometry or admission policy.
    sim.backend.world.grow_for_seats(count, true);
    let profile = |slot| PlayerAppearance {
        nickname: format!("Local{slot}").into_bytes(), skin: (slot % 9) as u8,
        ..Default::default()
    };
    for slot in 0..count {
        assert!(sim.connect_waited(slot, profile(slot), 2.).unwrap(), "initial slot {slot}");
        if slot < legacy { sim.protocol13(slot); }
    }
    let mut tick_counter = tick_counter::Counter::from_env();
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
        let study_start=tick_counter.as_ref().map(tick_counter::Counter::read);
        sim.advance_tick_with(0, &mut |_| Ok(())).unwrap();
        let study_end=tick_counter.as_ref().map(tick_counter::Counter::read);
        let population = sim.backend.world.snakes().iter().filter(|s| s.alive).count();
        if let Some(counter)=&mut tick_counter {counter.record(tick,study_start.unwrap(),study_end.unwrap(),population);}
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
        "populationFinal":sim.backend.world.snakes().iter().filter(|s|s.alive).count(),
        "spawnRandom":format!("{:?}",sim.backend.spawn_random),"latencyRandom":format!("{:?}",sim.backend.random)});
    if trace { record(3, &serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap()); }
    record(4, &serde_json::to_vec(&summary).unwrap());
    drop(record); output.flush().unwrap();
    eprintln!("{summary}");
}
