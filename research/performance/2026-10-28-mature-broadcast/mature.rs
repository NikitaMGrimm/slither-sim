//! Research-only mature core checkpoint, fresh deterministic remote sessions.
use super::{RawSimulator, SimulationConfig, PlayerAppearance};
use crate::{Action, Simulator, RestoreOptions, EventRecording, bot::{Forager, Timing}};
use std::{fs, io::{self, Write}, path::Path};

const SEATS: usize = 500;
const PRELUDE: usize = 120;
const TICKS: usize = 360;
fn config() -> SimulationConfig {
    SimulationConfig {agents: SEATS, world: crate::Config::official(SEATS),
        delivery: super::DeliveryProfile::Official, ..Default::default()}
}
fn profile(slot: usize) -> PlayerAppearance {
    PlayerAppearance {nickname: format!("Local{slot}").into_bytes(), skin: (slot%9) as u8, ..Default::default()}
}
pub fn census(world: &Simulator) -> serde_json::Value {
    let mut n: Vec<_> = world.snakes().iter().filter(|s|s.alive).map(|s|s.segments).collect();
    n.sort_unstable();
    let q=|p:f64| n.get(((n.len().saturating_sub(1)) as f64*p).ceil() as usize).copied().unwrap_or(0);
    serde_json::json!({"living":n.len(),"mean":n.iter().sum::<usize>() as f64/n.len().max(1) as f64,
        "p90":q(0.9),"p99":q(0.99),"max":n.last(),"anchors":world.snakes().iter().filter(|s|s.alive).map(|s|s.body.len()).sum::<usize>(),
        "food":world.food().len(),"prey":world.prey().len(),"deaths":world.deaths().len()})
}
fn canonical(world: &Simulator) -> Vec<u8> {
    let mut value=serde_json::to_value(world.checkpoint().unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("producer"); serde_json::to_vec(&value).unwrap()
}
#[repr(C)] struct Ts {sec:i64,nsec:i64}
unsafe extern "C" {fn clock_gettime(clock:i32, value:*mut Ts)->i32;}
fn cpu() -> f64 {let mut t=Ts{sec:0,nsec:0};assert_eq!(unsafe{clock_gettime(2,&mut t)},0);t.sec as f64+t.nsec as f64*1e-9}
fn respawn(sim: &mut RawSimulator, actions: &mut [Option<Action>]) {
    for slot in 0..SEATS {
        if !sim.backend.world.snakes()[slot].alive {
            if let Ok((body,heading))=sim.backend.spawn_pose(slot) {
                sim.backend.world.respawn_body(slot,body,heading).unwrap();
                actions[slot]=Some(Action{heading,boost:false});
            }
        }
    }
}
pub fn warm(out: &Path) {
    fs::create_dir(out).unwrap();
    let mut sim=RawSimulator::new(7,config(),SEATS).unwrap();
    sim.backend.world.grow_for_seats(SEATS,true);
    for slot in 0..SEATS {assert!(sim.connect_waited(slot,profile(slot),2.).unwrap());}
    sim.backend.world.record_events(EventRecording::default());
    let mut policy=Forager::default(); let timing=Timing::default();
    let mut actions:Vec<_>=sim.backend.world.snakes().iter().map(|s|Some(Action{heading:s.heading,boost:false})).collect();
    let started=cpu(); let mut selected=false; let mut tick=0;
    // Synthetic session churn creates staggered ages using ordinary retirement
    // and admission, never imported or rescaled body geometry.
    while tick<180_000 {
        if tick>0 && tick%50==0 {
            let slot=100+(tick/50-1)%400;
            if sim.backend.world.snakes()[slot].alive {sim.backend.world.retire(slot).unwrap();}
            actions[slot]=None;
        }
        respawn(&mut sim,&mut actions);
        if tick%6==0 {
            actions=sim.backend.world.observe_all().iter().map(|o|o.as_ref().map(|o|policy.decide(o,&timing,&[]))).collect();
        }
        sim.backend.world.step(&actions).unwrap(); tick+=1;
        if tick%1000==0 {
            let mut row=census(&sim.backend.world);
            row["ticks"]=tick.into();row["cpuSeconds"]=(cpu()-started).into();
            println!("{row}");io::stdout().flush().unwrap();
            let mean=row["mean"].as_f64().unwrap();let p90=row["p90"].as_u64().unwrap();let p99=row["p99"].as_u64().unwrap();
            selected=tick>=10_000 && row["living"].as_u64().unwrap()>=490 && (55. ..=90.).contains(&mean)
                && (150..=230).contains(&p90) && (220..=350).contains(&p99);
            if selected || cpu()-started>=600. {break;}
        }
    }
    let bytes=serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap();
    let restored=Simulator::from_checkpoint(&bytes,RestoreOptions::default()).unwrap();
    assert_eq!(canonical(&sim.backend.world),canonical(&restored));
    fs::write(out.join(if selected {"checkpoint.json"} else {"unqualified-checkpoint.json"}),bytes).unwrap();
    let mut row=census(&sim.backend.world);row["qualified"]=selected.into();row["ticks"]=tick.into();
    row["cpuSeconds"]=(cpu()-started).into();row["roundtripExact"]=true.into();
    fs::write(out.join("meta.json"),serde_json::to_vec(&row).unwrap()).unwrap();
    assert!(selected,"bounded mature screen failed; retained checkpoint is unqualified");
}

pub fn run(path: &Path, meta: &Path, legacy: bool, trace: bool) {
    let saved:serde_json::Value=serde_json::from_slice(&fs::read(meta).unwrap()).unwrap();
    assert_eq!(saved["qualified"],true);
    let mut sim=RawSimulator::new(7,config(),SEATS).unwrap();
    sim.backend.world=Simulator::from_checkpoint(&fs::read(path).unwrap(),RestoreOptions {
        events:EventRecording{pickups:true,food_births:true},..Default::default()}).unwrap();
    sim.backend.tick=saved["ticks"].as_u64().unwrap();sim.backend.food_index.stale=true;
    sim.backend.food_presentation=super::FoodPresentation::new(7,SEATS);
    let initial=canonical(&sim.backend.world);
    let headings:Vec<_>=sim.backend.world.snakes().iter().map(|s|
        ((s.heading.rem_euclid(std::f64::consts::TAU)/std::f64::consts::TAU*251.).round() as usize)%251).collect();
    for slot in 0..SEATS {
        sim.backend.profiles[slot]=profile(slot);sim.backend.slots[slot].life=1;
        sim.backend.slots[slot].control=Action{heading:sim.backend.world.snakes()[slot].heading,boost:false};
        sim.backend.connect(slot).unwrap();if legacy {sim.protocol13(slot);}
    }
    sim.backend.food_presentation.sync(sim.backend.world.food()).unwrap();
    sim.backend.food_index.rebuild(sim.backend.world.food(),&sim.backend.food_presentation);
    assert_eq!(initial,canonical(&sim.backend.world));
    let mut output=io::BufWriter::new(io::stdout().lock());
    let mut record=|tag:u64, bytes:&[u8]| {if trace {output.write_all(&tag.to_le_bytes()).unwrap();output.write_all(&(bytes.len() as u64).to_le_bytes()).unwrap();output.write_all(bytes).unwrap();}};
    record(0,&initial);let (mut frames,mut bytes,mut rejoins)=(0usize,0usize,0usize);
    let mut start=serde_json::Value::Null;let mut end_min=SEATS;
    for tick in 0..PRELUDE+TICKS {
        for slot in 0..SEATS {
            if !sim.backend.world.snakes()[slot].alive && tick%72==0 {
                if sim.connect_waited(slot,profile(slot),2.).unwrap() {rejoins+=1;if legacy {sim.protocol13(slot);}}
            }
            if tick%6==0 && sim.backend.slots[slot].connection.open {
                let now=sim.backend.time();let h=((headings[slot]+tick/24)%251) as u8;
                sim.receive_at(slot,&[h],now).unwrap();sim.receive_at(slot,&[254],now).unwrap();
            }
        }
        let measured=tick>=PRELUDE;
        if tick==PRELUDE {start=census(&sim.backend.world);record(3,&canonical(&sim.backend.world));crate::study_counter::clear();}
        let guard=measured.then(||crate::study_counter::enter(2));
        sim.advance_tick_with(0,&mut |_|Ok(())).unwrap();drop(guard);
        end_min=end_min.min(sim.backend.world.living());
        for slot in 0..SEATS {for (at,frame) in sim.take(slot).unwrap() {
            frames+=1;bytes+=frame.len();
            if trace {let mut h=Vec::new();h.extend_from_slice(&(tick as u64).to_le_bytes());h.extend_from_slice(&(slot as u64).to_le_bytes());h.extend_from_slice(&at.to_bits().to_le_bytes());record(1,&h);record(2,&frame);}
        }}
    }
    record(4,&canonical(&sim.backend.world));
    let report=serde_json::json!({"case":if legacy {"mature500-churn-legacy-v1"}else{"mature500-churn-stock-v1"},
        "prelude":PRELUDE,"ticks":TICKS,"frames":frames,"bytes":bytes,"rejoins":rejoins,"minimumLiving":end_min,
        "initial":saved,"measuredStart":start,"end":census(&sim.backend.world)});
    record(5,&serde_json::to_vec(&report).unwrap());drop(record);output.flush().unwrap();
    eprintln!("{report}");
}
