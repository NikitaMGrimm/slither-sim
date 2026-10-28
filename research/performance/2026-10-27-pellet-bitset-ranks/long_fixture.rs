//! Research-only child module of client::simulation, identical on both arms.
use super::{PlayerAppearance, RawSimulator, SimulationConfig};

use std::{hint::black_box, io::{self, Write}};

pub fn run(case: &str, trace: bool) {
    let (count, legacy, dome, shrinking, orbit) = match case {
        "official500" => (500, 0, false, false, false),
        "official500-orbit" => (500, 0, false, false, true),
        "battledome80" => (80, 0, true, false, true),
        "official500-shrink" => (500, 0, false, true, false),
        _ => panic!("unknown case"),
    };
    let config = SimulationConfig {
        agents: count,
        world: if dome {crate::Config::battledome()} else {crate::Config::official(count)},
        delivery: if dome {super::DeliveryProfile::Responsive} else {super::DeliveryProfile::Official},
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
        let _ = sim.connect_waited(slot, profile(slot), 2.).unwrap();
        if slot < legacy { sim.protocol13(slot); }
    }
    let dt = sim.backend.world.config().dt;
    let ticks: usize = std::env::var("LONG_TICKS").ok().map(|s| s.parse().unwrap()).unwrap_or(60000);
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
    for tick in 0..ticks {
        // Fixed synthetic occupied-seat loss, after a long 500-seat period.
        let seats = if shrinking && tick>=30000 {500usize.saturating_sub((tick-30000)/12*4).max(100)} else {count};
        if shrinking && tick>=30000 && (tick-30000)%12==0 {
            for slot in seats..count {
                if sim.backend.slots[slot].connection.open {sim.disconnect(slot).unwrap();}
            }
        }
        if tick == 720 { sim.disconnect(0).unwrap(); }
        if tick.is_multiple_of(72) {
            for slot in 0..seats {
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
                let heading = if orbit {
                    let head=sim.backend.world.snakes()[slot].head;
                    let target=sim.backend.world.food_radius()*0.65;
                    let correction=((head.x.hypot(head.y)-target)/target).clamp(-1.0,1.0);
                    let wanted=head.y.atan2(head.x)+std::f64::consts::FRAC_PI_2+correction*0.85;
                    (wanted.rem_euclid(std::f64::consts::TAU)/std::f64::consts::TAU*250.0).round() as u8
                } else {((slot * 37 + tick / 6 * 3) % 251) as u8};
                sim.receive_at(slot, &[heading], now).unwrap();
                sim.receive_at(slot, &[if (tick / 6 + slot) % 5 == 0 {253} else {254}], now).unwrap();
            }
        }
        let study_start=crate::study_rebuild_probe::begin(tick);
        sim.advance_tick_with(0, &mut |_| Ok(())).unwrap();
        let study_delta=crate::study_rebuild_probe::end(study_start);
        let population = sim.backend.world.snakes().iter().filter(|s| s.alive).count();
        let births=sim.backend.world.food_spawn_events().unwrap().len();
        let food=sim.backend.world.food().len();
        let retained=sim.backend.world.study_retained();
        let radius=sim.backend.world.radius();
        let mut tick_maps=0;
        population_sum += population;
        population_min = population_min.min(population);
        for slot in 0..count {
            for (at, frame) in sim.take(slot).unwrap() {
                frames += 1; bytes += frame.len() as u64;
                for p in crate::client::protocol::split_frame(&frame).unwrap() {
                    if b"MVU".contains(&p[0]) { maps += 1; tick_maps+=1; }
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
        crate::study_rebuild_probe::row(tick,study_delta,population,births,food,retained,radius,tick_maps);
    }
    crate::study_rebuild_probe::finish();
    assert!(maps > 0 && frames > 0);
    let endpoint=serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap();
    let endpoint_checksum=endpoint.iter().fold(0xcbf2_9ce4_8422_2325u64,|h,&b|(h^u64::from(b)).wrapping_mul(0x100_0000_01b3));
    let summary = serde_json::json!({"case":case,"endpointChecksum":endpoint_checksum,"ticks":ticks,"dt":dt,"seats":count,"legacy":legacy,
        "frames":frames,"bytes":bytes,"maps":maps,"reconnects":reconnects,"refused":refused,
        "forcedReconnect":forced,"populationSum":population_sum,"populationMin":population_min,
        "populationFinal":sim.backend.world.snakes().iter().filter(|s|s.alive).count(),
        "spawnRandom":format!("{:?}",sim.backend.spawn_random),"latencyRandom":format!("{:?}",sim.backend.random)});
    if trace { record(3, &serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap()); }
    record(4, &serde_json::to_vec(&summary).unwrap());
    drop(record); output.flush().unwrap();
    eprintln!("{summary}");
}
