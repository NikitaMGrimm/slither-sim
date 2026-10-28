//! Actual background-client path; built only in the diagnostic source archive.
use super::*;
use crate::study_counter as pmu;
use serde_json::json;
use std::{fs, path::Path};
pub fn run() {
    assert_eq!(std::thread::available_parallelism().unwrap().get(),1,"pin study to one CPU");
    let args:Vec<_>=std::env::args().collect(); let root=Path::new(&args[1]);
    fs::create_dir(root).unwrap();
    let config=SimulationConfig {agents:500,world:crate::Config::official(500),delivery:DeliveryProfile::Official,..Default::default()};
    let mut sim=SimulatorBackend::new(7,config).unwrap();
    let mut rows=vec![];
    for decision in 0..1200 {
        if sim.frame().unwrap().status==ClientStatus::Dead { let _=sim.reconnect(); }
        if sim.frame().unwrap().status==ClientStatus::Playing {
            sim.send_action(Action {heading:decision as f64*0.013,boost:false}).unwrap();
        }
        pmu::reset(decision>=1000);
        let a=(decision>=1000).then(pmu::read_counter);
        let frame=sim.next_frame().unwrap(); std::hint::black_box(&frame);
        if let Some(a)=a {let b=pmu::read_counter();
            assert!((b[2]-a[2]) as f64/(b[1]-a[1]) as f64>=0.99);
            rows.push(json!({"decision":decision,"instructions":b[0]-a[0],"enabled":b[1]-a[1],"running":b[2]-a[2],"alive":sim.world.living(),"phases":pmu::result()}));
        }
    }
    pmu::reset(false);
    fs::write(root.join("rows.json"),serde_json::to_vec(&rows).unwrap()).unwrap();
    fs::write(root.join("world.json"),serde_json::to_vec(&sim.world.checkpoint().unwrap()).unwrap()).unwrap();
    println!("{}",json!({"decisions":rows.len(),"alive":sim.world.living(),"deaths":sim.world.deaths().len(),
        "meanSegments":sim.world.snakes().iter().filter(|s|s.alive).map(|s|s.segments).sum::<usize>() as f64/sim.world.living() as f64}));
}
