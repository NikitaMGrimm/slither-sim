//! Cold diagnostic only: verify the dense-grid limit, not a steady-state row.
use slither_sim::{Action, Config, InitialPopulation, Placement, Simulator, WorldOptions};
use slither_sim::study_counter as pmu;
use serde_json::json;
use std::{fs, time::Instant};
fn delta(a:[u64;3],b:[u64;3])->[u64;3] {
    let d=[b[0]-a[0],b[1]-a[1],b[2]-a[2]];
    assert!(d[1]>0 && d[2] as f64/d[1] as f64>=0.99);d
}
fn main() {
    assert_eq!(std::thread::available_parallelism().unwrap().get(),1);
    let args:Vec<_>=std::env::args().collect();
    let n:usize=args[1].parse().unwrap();
    let samples:usize=args.get(3).map_or(8,|v|v.parse().unwrap());assert!((1..=64).contains(&samples));
    let options=WorldOptions {seed:7,config:Config::official(n),population:InitialPopulation::Placed {
        ids:(0..n).map(|s|s.to_string()).collect(),placement:Placement::Admitted {attempts:64.try_into().unwrap(),preferred_clearance:600.}
    }};
    println!("{}",json!({"stage":"constructing","n":n}));
    let a=pmu::read_counter();let t=Instant::now();
    let mut world=Simulator::new(options).unwrap();
    let setup_wall=t.elapsed().as_secs_f64();let setup=delta(a,pmu::read_counter());
    println!("{}",json!({"stage":"constructed","n":n,"alive":world.living(),"food":world.food().len(),"setup":setup,"setupWallSeconds":setup_wall}));
    let mut actions=vec![None;n];
    for _ in 0..2 {
        for (a,s) in actions.iter_mut().zip(world.snakes()) {*a=s.alive.then_some(Action {heading:s.heading,boost:false});}
        world.step(&actions).unwrap();
    }
    let (mut lx,mut ly,mut hx,mut hy)=(i64::MAX,i64::MAX,i64::MIN,i64::MIN);
    for s in world.snakes().iter().filter(|s|s.alive) {
        let (c,r)=slither_sim::physics::pickup_circle(s.head,s.heading,s.speed,s.segments);
        lx=lx.min(((c.x-r-1e-9)/128.).floor() as i64);hx=hx.max(((c.x+r+1e-9)/128.).floor() as i64);
        ly=ly.min(((c.y-r-1e-9)/128.).floor() as i64);hy=hy.max(((c.y+r+1e-9)/128.).floor() as i64);
    }
    pmu::reset(true);let mut rows=vec![];
    for _ in 0..samples {
        for (a,s) in actions.iter_mut().zip(world.snakes()) {*a=s.alive.then_some(Action {heading:s.heading,boost:false});}
        let a=pmu::read_counter();world.step(&actions).unwrap();let d=delta(a,pmu::read_counter());rows.push(d);
    }
    let profile=pmu::result();pmu::reset(false);
    let result=json!({"kind":"cold-grid-cliff-diagnostic","n":n,"radius":world.radius(),"ticks":samples,
        "setup":setup,"setupWallSeconds":setup_wall,"steps":rows,"profile":profile,
        "eaterRectangleCells":(hx-lx+1)*(hy-ly+1),"denseGridMaxCells":1<<20,
        "meanSegments":world.snakes().iter().filter(|s|s.alive).map(|s|s.segments).sum::<usize>() as f64/world.living() as f64,
        "alive":world.living(),"food":world.food().len(),"prey":world.prey().len(),
        "note":"indices 0..6 are instruction brackets; 20 counts successful dense ticks; 21 sums food counts"});
    fs::write(&args[2],serde_json::to_vec_pretty(&result).unwrap()).unwrap();println!("{result}");
}
