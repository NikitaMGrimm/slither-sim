//! Synthetic post-churn continuation: preserve the core, prescribe a fresh controller.
use super::{RawSimulator, SimulationConfig};
use crate::{Action, Simulator, RestoreOptions, bot::{Forager,Timing}};
use std::{fs,io::{self,Write},path::Path};
#[repr(C)] struct Ts{sec:i64,nsec:i64}
unsafe extern "C" {fn clock_gettime(clock:i32,value:*mut Ts)->i32;}
fn cpu()->f64{let mut t=Ts{sec:0,nsec:0};assert_eq!(unsafe{clock_gettime(2,&mut t)},0);t.sec as f64+t.nsec as f64*1e-9}
fn canonical(w:&Simulator)->Vec<u8>{let mut v=serde_json::to_value(w.checkpoint().unwrap()).unwrap();v.as_object_mut().unwrap().remove("producer");serde_json::to_vec(&v).unwrap()}
pub fn run(input:&Path,out:&Path){
    fs::create_dir(out).unwrap();
    let mut sim=RawSimulator::new(7,SimulationConfig{agents:500,world:crate::Config::official(500),..Default::default()},500).unwrap();
    sim.backend.world=Simulator::from_checkpoint(&fs::read(input).unwrap(),RestoreOptions::default()).unwrap();
    // Fresh seed-7 external admission RNG is explicit; this resumes the exact
    // authoritative core, not a complete host/controller checkpoint.
    let start=cpu();let mut tick=180_000usize;let mut selected=false;
    let mut policy=Forager::default();let timing=Timing::default();
    let mut actions:Vec<_>=sim.backend.world.snakes().iter().map(|s|s.alive.then_some(Action{heading:s.heading,boost:false})).collect();
    while tick<210_000 {
        for slot in 0..500 {
            if !sim.backend.world.snakes()[slot].alive {
                if let Ok((body,heading))=sim.backend.spawn_pose(slot){sim.backend.world.respawn_body(slot,body,heading).unwrap();actions[slot]=Some(Action{heading,boost:false});}
            }
        }
        if tick%6==0 {actions=sim.backend.world.observe_all().iter().map(|o|o.as_ref().map(|o|policy.decide(o,&timing,&[]))).collect();}
        sim.backend.world.step(&actions).unwrap();tick+=1;
        if tick%1000==0 {
            let mut row=super::study_mature::census(&sim.backend.world);row["ticks"]=tick.into();row["cpuSeconds"]=(cpu()-start).into();println!("{row}");io::stdout().flush().unwrap();
            selected=row["living"].as_u64().unwrap()>=490 && (55. ..=90.).contains(&row["mean"].as_f64().unwrap())
                && (150..=230).contains(&row["p90"].as_u64().unwrap()) && (220..=350).contains(&row["p99"].as_u64().unwrap());
            if selected || cpu()-start>=100. {break;}
        }
    }
    let bytes=serde_json::to_vec(&sim.backend.world.checkpoint().unwrap()).unwrap();let restored=Simulator::from_checkpoint(&bytes,RestoreOptions::default()).unwrap();assert_eq!(canonical(&sim.backend.world),canonical(&restored));
    fs::write(out.join(if selected{"checkpoint.json"}else{"unqualified-checkpoint.json"}),bytes).unwrap();
    let mut row=super::study_mature::census(&sim.backend.world);row["qualified"]=selected.into();row["ticks"]=tick.into();row["cpuSeconds"]=(cpu()-start).into();row["roundtripExact"]=true.into();
    row["construction"]="churn180k followed by prescribed no-forced-churn controller; fresh external admission RNG".into();
    fs::write(out.join("meta.json"),serde_json::to_vec(&row).unwrap()).unwrap();assert!(selected,"post-churn screen failed");
}
