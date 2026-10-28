//! Compiled only in an archived research source tree, under client::simulation.
use super::*;
use crate::{Config, InitialPopulation, WorldOptions, RestoreOptions};
use crate::study_counter as pmu;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use std::{fs, path::Path, time::Instant};

#[derive(Clone, Serialize, Deserialize)]
struct Rng(u64);
impl Rng {
    fn unit(&mut self) -> f64 {
        self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}
#[derive(Serialize, Deserialize)]
struct Driver { tick: usize, random: Rng, wander: Vec<Rng>, actions: Vec<Option<Action>>, retry: Vec<usize> }
fn stats(world: &Simulator) -> Value {
    let mut lengths: Vec<_> = world.snakes().iter().filter(|s| s.alive).map(|s| s.segments).collect();
    lengths.sort_unstable();
    let n=lengths.len(); assert!(n>0);
    json!({"alive": n, "meanSegments": lengths.iter().sum::<usize>() as f64/n as f64,
        "p50": lengths[n/2], "p90": lengths[(n*9/10).min(n-1)],
        "p99": lengths[(n*99/100).min(n-1)], "max": lengths[n-1],
        "food": world.food().len(), "prey": world.prey().len(), "radius": world.radius()})
}
fn tick(world: &mut Simulator, d: &mut Driver, count: bool) -> Value {
    let mut attempts=0; let mut refusals=0; let mut births=0;
    let mut bounds=((!count || std::env::var_os("SCALING_FAST_ADMISSION").is_some()) && !std::env::var_os("SCALING_ORIGINAL_WARM").is_some() && world.snakes().iter().any(|s|!s.alive)).then(||world.snakes().iter().map(|s|s.alive.then(||bounds_of(std::iter::once(s.head).chain(s.body.iter().copied())))).collect::<Vec<_>>());
    let a = count.then(pmu::read_counter);
    for slot in 0..d.actions.len() {
        if !world.snakes()[slot].alive && d.tick>=d.retry[slot] {
            attempts+=1;
            let pose=if let Some(bounds)=bounds.as_ref() {host_pose_fast(world, &mut d.random, slot, 0., bounds)} else {host_pose(world, &mut d.random, slot, 0.)};
            match pose {
                Ok((body,heading)) => { world.respawn_body(slot,body,heading).unwrap();
                    if let Some(bounds)=bounds.as_mut() {let s=&world.snakes()[slot];bounds[slot]=Some(bounds_of(std::iter::once(s.head).chain(s.body.iter().copied())));}
                    d.actions[slot]=Some(Action { heading, boost:false }); births+=1; }
                Err(_) => { refusals+=1; d.retry[slot]=d.tick+2; }
            }
        }
    }
    let spawn = a.map(|a| { let b=pmu::read_counter(); delta(a,b) });
    for (slot,snake) in world.snakes().iter().enumerate() {
        if !snake.alive { d.actions[slot]=None; continue; }
        let action=d.actions[slot].get_or_insert(Action {heading:snake.heading,boost:false});
        if d.tick.is_multiple_of(6) { action.heading=(action.heading+(d.wander[slot].unit()-0.5)*0.36).rem_euclid(TAU); }
        action.boost=((d.tick/300+slot)%8)==0;
    }
    let deaths=world.deaths().len();
    let a=count.then(pmu::read_counter);
    world.step(&d.actions).unwrap();
    let step=a.map(|a|delta(a,pmu::read_counter()));
    d.tick+=1;
    json!({"tick":d.tick,"step":step,"spawn":spawn,"attempts":attempts,"refusals":refusals,
        "births":births,"deaths":world.deaths().len()-deaths,"alive":world.living()})
}
fn delta(a:[u64;3],b:[u64;3])->[u64;3] {
    let d=[b[0]-a[0],b[1]-a[1],b[2]-a[2]];
    assert!(d[1]>0 && d[2] as f64/d[1] as f64>=0.99,"PMU scheduling below 99%"); d
}
fn capped_radius_updates() -> Vec<crate::RadiusUpdate> {
    // Put each event halfway before its tick. Repeated dt additions can drift
    // beyond the replay API's 1 ns tolerance during a long warmup.
    (0..=260000).map(|t|crate::RadiusUpdate {
        time:if t==0 {0.} else {(t as f64-0.5)*0.007},
        radius:crate::client::encode::MAX_RADIUS,
    }).collect()
}
fn save(root:&Path,world:&Simulator,d:&Driver) {
    fs::write(root.join("world.json"),serde_json::to_vec(&world.checkpoint().unwrap()).unwrap()).unwrap();
    fs::write(root.join("driver.json"),serde_json::to_vec(d).unwrap()).unwrap();
}
pub fn run() {
    assert_eq!(std::thread::available_parallelism().unwrap().get(),1,"pin study to one CPU");
    let args:Vec<_>=std::env::args().collect();
    let mode=&args[1]; let root=Path::new(&args[2]);
    if mode=="radius-plan" {
        fs::write(root,serde_json::to_vec(&capped_radius_updates()).unwrap()).unwrap();
        return;
    }
    if mode=="warm" {
        fs::create_dir(root).unwrap();
        let n:usize=args[3].parse().unwrap(); let capped=&args[4]=="capped";
        let mut config=Config::official(n);
        if capped && config.radius>crate::client::encode::MAX_RADIUS {
            let r=crate::client::encode::MAX_RADIUS;
            let (food,rate,prey)=config.ecology_at_radius(r);
            config.radius=r; config.food_count=food; config.ambient_replenish_per_second=rate; config.prey.count=prey;
            config.size_to_population=false;
            config.radius_updates=capped_radius_updates();
        }
        let mut world=Simulator::new(WorldOptions {seed:7,config,population:InitialPopulation::Dormant {ids:(0..n).map(|s|s.to_string()).collect()}}).unwrap();
        let mut d=Driver {tick:0,random:Rng(7 ^ 0x7370_6177_6e01),wander:(0..n).map(|s|Rng((s as u64+1)*0x9e3779b1|1)).collect(),actions:vec![None;n],retry:vec![0;n]};
        let mut blocks=vec![]; let mut settled=false;
        let max_blocks=args.get(5).map_or(24,|v|v.parse().unwrap());
        for block in 0..max_blocks {
            let mut means=vec![]; let mut p90=vec![]; let mut deaths=0; let mut refusals=0;
            for k in 0..10000 { let row=tick(&mut world,&mut d,false);
                deaths+=row["deaths"].as_u64().unwrap(); refusals+=row["refusals"].as_u64().unwrap();
                if k%100==99 {let s=stats(&world); means.push(s["meanSegments"].as_f64().unwrap());p90.push(s["p90"].as_f64().unwrap());}
            }
            let row=json!({"block":block,"ticks":d.tick,"meanSegments":means.iter().sum::<f64>()/means.len() as f64,
                "p90Mean":p90.iter().sum::<f64>()/p90.len() as f64,"deaths":deaths,"refusals":refusals,"end":stats(&world)});
            println!("{row}"); blocks.push(row);
            fs::write(root.join("warmup.json"),serde_json::to_vec_pretty(&blocks).unwrap()).unwrap();
            if blocks.len()>=3 { settled=["meanSegments","p90Mean"].iter().all(|key| {
                let v:Vec<f64>=blocks.iter().rev().take(3).map(|r|r[key].as_f64().unwrap()).collect();
                v.iter().copied().fold(0.,f64::max)/v.iter().copied().fold(f64::INFINITY,f64::min)<=1.05
            }); if settled {break;} }
        }
        save(root,&world,&d);
        println!("{}",json!({"settled":settled,"ticks":d.tick,"end":stats(&world)}));
        fs::write(root.join("summary.json"),serde_json::to_vec(&json!({"settled":settled,"ticks":d.tick,"end":stats(&world)})).unwrap()).unwrap();
    } else if mode=="verify" {
        let out=Path::new(&args[3]); fs::create_dir(out).unwrap();
        let world=Simulator::from_checkpoint(&fs::read(root.join("world.json")).unwrap(),RestoreOptions::default()).unwrap();
        let bounds:Vec<_>=world.snakes().iter().map(|s|s.alive.then(||bounds_of(std::iter::once(s.head).chain(s.body.iter().copied())))).collect();
        let mut original=Rng(0x123456789abcdef1); let mut fast=original.clone();
        for slot in 0..50 {
            let a=host_pose(&world,&mut original,slot,0.);
            let b=host_pose_fast(&world,&mut fast,slot,0.,&bounds);
            assert_eq!(serde_json::to_vec(&a).unwrap(),serde_json::to_vec(&b).unwrap(),"admission oracle slot {slot}");
            assert_eq!(original.0,fast.0,"admission RNG");
        }
        fs::write(out.join("result.json"),serde_json::to_vec(&json!({"poses":50,"exact":true,"rng":original.0})).unwrap()).unwrap();
        println!("{}",json!({"poses":50,"exact":true}));
    } else {
        let out=Path::new(&args[3]); fs::create_dir(out).unwrap();
        let mut world=Simulator::from_checkpoint(&fs::read(root.join("world.json")).unwrap(),RestoreOptions::default()).unwrap();
        let mut d:Driver=serde_json::from_slice(&fs::read(root.join("driver.json")).unwrap()).unwrap();
        for _ in 0..100 {tick(&mut world,&mut d,false);}
        let before=stats(&world); pmu::reset(true); let start=Instant::now();
        let mut rows=vec![]; let mut lengths=vec![];
        for _ in 0..1000 { rows.push(tick(&mut world,&mut d,true)); if d.tick%100==0 {lengths.push(stats(&world));} }
        let wall=start.elapsed().as_secs_f64(); let profile=pmu::result(); pmu::reset(false);
        // The driver outer PMU rows remain independent of diagnostic compilation.
        save(out,&world,&d);
        fs::write(out.join("rows.json"),serde_json::to_vec(&rows).unwrap()).unwrap();
        println!("{}",json!({"before":before,"end":stats(&world),"lengths":lengths,"wallSeconds":wall,"profile":profile,
            "affinity":fs::read_to_string("/proc/self/status").unwrap().lines().find(|line|line.starts_with("Cpus_allowed_list:")).unwrap()}));
    }
}

// host_pose is appended verbatim from spawn_pose_waited by reproduce.py, with
// only self.world/self.spawn_random changed to the explicit arguments.
