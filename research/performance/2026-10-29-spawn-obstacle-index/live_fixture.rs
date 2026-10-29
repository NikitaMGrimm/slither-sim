//! Research-only child of client::simulation. Physics checkpoints are not bot inputs.
use super::*;
use std::{fs, path::Path};

fn probe(world: Simulator) -> SimulatorBackend {
    // The admission method reads only world and spawn_random. The excluded
    // joining slot is absent from this frozen 320-snake checkpoint.
    let mut backend = SimulatorBackend::new(7, SimulationConfig {
        agents: 1, world: crate::Config { food_count: 0, ..crate::Config::sandbox() },
        ..Default::default()
    }).unwrap();
    backend.world = world;
    backend
}

fn census(world: &Simulator) -> serde_json::Value {
    let living: Vec<_> = world.snakes().iter().filter(|s| s.alive).collect();
    let mut lengths: Vec<_> = living.iter().map(|s| s.body.len()).collect();
    lengths.sort_unstable();
    let mut radii: Vec<_> = living.iter().map(|s| s.head.distance(Point::default())).collect();
    radii.sort_by(f64::total_cmp);
    serde_json::json!({"living": living.len(), "radius":world.radius(),
        "meanSegments": lengths.iter().sum::<usize>() as f64 / living.len() as f64,
        "p90Segments":lengths[(lengths.len()-1)*90/100],
        "p99Segments":lengths[(lengths.len()-1)*99/100],
        "headRadiusP50":radii[(radii.len()-1)/2],
        "food":world.food().len(),
        "boosting": (0..world.snakes().len()).filter(|&i| world.snakes()[i].alive && world.is_boosting(i)).count()})
}

pub fn warmup(destination: &str) {
    let out=Path::new(destination); fs::create_dir(out).unwrap();
    let mut backend=SimulatorBackend::new(7, SimulationConfig {
        agents:320, world:crate::Config::battledome(), ..Default::default()
    }).unwrap();
    let mut policy=ForagerBot::new();
    let mut cursor=0;
    let mut alive_ticks=0u64; let mut boosted_ticks=0u64;
    let mut history=Vec::new();
    // One queued retry per decision, independent of the policy. The complete
    // search is original; this warmup scheduler is not a live-queue replica.
    for decision in 0..=21_429 {
        for offset in 0..320 {
            let slot=(cursor+offset)%320;
            if matches!(backend.frame_for(slot).unwrap().status,ClientStatus::Dead|ClientStatus::Disconnected) {
                let _=backend.reconnect_slot(slot); cursor=(slot+1)%320; break;
            }
        }
        let mut actions=Vec::with_capacity(320);
        for slot in 0..320 {
            let frame=backend.frame_for(slot).unwrap();
            let action=policy.decide(&frame).map(|mut a| {
                if frame.rules.as_ref().is_some_and(|r|r.free_boost_at_floor) { a.boost=true; }
                a
            });
            actions.push(action);
        }
        backend.step_all(&actions).unwrap();
        for slot in 0..320 {
            if backend.world.snakes()[slot].alive {
                alive_ticks+=1; boosted_ticks+=u64::from(backend.world.is_boosting(slot));
            }
        }
        if decision%143 != 0 { continue; }
        let now=backend.time(); let counts=census(&backend.world);
        eprintln!("CENSUS {now:.3} {counts}");
        history.push(serde_json::json!({"seconds":now,"census":counts}));
        fs::write(out.join("warmup-history.json"),serde_json::to_vec_pretty(&history).unwrap()).unwrap();
        if now<600. || counts["living"].as_u64().unwrap()<310
            || !(50. ..=60.).contains(&counts["meanSegments"].as_f64().unwrap()) { continue; }
        let bytes=serde_json::to_vec(&backend.world.checkpoint().unwrap()).unwrap();
        let frozen=Simulator::from_checkpoint(&bytes,crate::RestoreOptions::default()).unwrap();
        let mut query=probe(frozen); let mut refused=None; let mut success=None;
        for seed in 0..256u64 {
            query.spawn_random=Random::new(seed);
            let value=query.spawn_pose_waited(usize::MAX,0.);
            match value {
                Ok(_) if success.is_none()=>success=Some(seed),
                Err(ref s) if s==spawn::WAITING && refused.is_none()=>refused=Some(seed),
                Ok(_)=>{}, Err(ref s) if s==spawn::WAITING=>{}, Err(s)=>panic!("{s}"),
            }
            if refused.is_some() && success.is_some() { break; }
        }
        if let (Some(refused),Some(success))=(refused,success) {
            fs::write(out.join("world.json"),bytes).unwrap();
            let summary=serde_json::json!({"seconds":now,"census":counts,
                "refusedSeed":refused,"successfulSeed":success,"waitedSeconds":0.,
                "aliveDecisionSamples":alive_ticks,"boostingDecisionSamples":boosted_ticks,
                "boostShare":boosted_ticks as f64/alive_ticks as f64,
                "dataOrigin":"deterministic in-process received-frame A4 warmup; synthetic fixture, not live capture"});
            fs::write(out.join("fixture.json"),serde_json::to_vec_pretty(&summary).unwrap()).unwrap();
            eprintln!("FIXTURE_READY {summary}"); return;
        }
    }
    panic!("No checkpoint met the frozen ecology/outcome eligibility gates");
}

pub fn admission(path: &str, row: &str, count: usize, measured: bool) {
    assert!(["refused","successful"].contains(&row));
    let folder=Path::new(path);
    let fixture:serde_json::Value=serde_json::from_slice(&fs::read(folder.join("fixture.json")).unwrap()).unwrap();
    let world=Simulator::from_checkpoint(&fs::read(folder.join("world.json")).unwrap(),crate::RestoreOptions::default()).unwrap();
    let mut backend=probe(world);
    assert_eq!(fs::read_dir("/proc/self/task").unwrap().count(),1);
    let seed=fixture[if row=="refused" {"refusedSeed"} else {"successfulSeed"}].as_u64().unwrap();
    if std::env::var_os("STUDY_PROFILE").is_some() {
        profile_control("enable");
        for _ in 0..count {
            backend.spawn_random=Random::new(seed);
            let result=backend.spawn_pose_waited(usize::MAX,0.);
            assert_eq!(result.is_err(),row=="refused");
            std::hint::black_box(result);
        }
        profile_control("disable");
        println!("{}",serde_json::json!({"row":row,"searches":count,"seed":seed}));
        return;
    }
    let mut records=Vec::new();
    if measured { crate::study_tick::enable(); }
    for _ in 0..count {
        backend.spawn_random=Random::new(seed);
        let start=crate::study_tick::begin();
        let result=backend.spawn_pose_waited(usize::MAX,0.);
        let sample=crate::study_tick::end(start);
        assert_eq!(result.is_err(),row=="refused");
        records.push(serde_json::json!({"sample":sample,"result":result,
            "random":format!("{:?}",backend.spawn_random)}));
    }
    println!("{}",serde_json::json!({"row":row,"records":records}));
}

fn profile_control(command: &str) {
    use std::io::{BufRead,Write};
    if let Ok(path)=std::env::var("STUDY_PERF_CTL") {
        let mut f=fs::OpenOptions::new().write(true).open(path).unwrap();
        writeln!(f,"{command}").unwrap();f.flush().unwrap();
        let f=fs::File::open(std::env::var("STUDY_PERF_ACK").unwrap()).unwrap();
        let mut line=String::new();std::io::BufReader::new(f).read_line(&mut line).unwrap();
        assert_eq!(line.trim(),"ack");
    }
}
