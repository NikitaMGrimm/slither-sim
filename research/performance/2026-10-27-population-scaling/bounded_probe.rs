// Appended only to the separately archived diagnostic module. It records
// reached warmup ticks without presenting them as a steady-state sample.
#[repr(C)]
struct CpuTimespec {seconds:std::os::raw::c_long,nanos:std::os::raw::c_long}
unsafe extern "C" {fn clock_gettime(clock:i32,time:*mut CpuTimespec)->i32;}
fn process_cpu()->f64 {
    let mut t=CpuTimespec {seconds:0,nanos:0};
    assert_eq!(unsafe{clock_gettime(2,&mut t)},0);
    t.seconds as f64+t.nanos as f64*1e-9
}
pub fn bounded_run() {
    use std::io::Write;
    assert_eq!(std::thread::available_parallelism().unwrap().get(),1);
    let args:Vec<_>=std::env::args().collect();
    let root=Path::new(&args[1]);fs::create_dir(root).unwrap();
    let limit:f64=args[2].parse().unwrap();assert!((5.0..=3600.0).contains(&limit));
    let start=process_cpu();let n=10000;
    let mut world=Simulator::new(WorldOptions {seed:7,config:Config::official(n),population:InitialPopulation::Dormant {ids:(0..n).map(|s|s.to_string()).collect()}}).unwrap();
    let mut d=Driver {tick:0,random:Rng(7 ^ 0x7370_6177_6e01),wander:(0..n).map(|s|Rng((s as u64+1)*0x9e3779b1|1)).collect(),actions:vec![None;n],retry:vec![0;n]};
    let mut file=std::io::BufWriter::new(fs::File::create(root.join("ticks.jsonl")).unwrap());
    while process_cpu()-start<limit-2. {
        pmu::reset(true);
        let mut row=tick(&mut world,&mut d,true);
        let phases=pmu::result();
        row["denseAccepted"]=json!(phases["instructions"][20].as_u64().unwrap()==1);
        row["eaterRectangleCells"]=phases["instructions"][22].clone();
        row["foodRelativeCellLimit"]=phases["instructions"][23].clone();
        row["phases"]=phases;row["state"]=stats(&world);
        row["processCpuSeconds"]=json!(process_cpu()-start);
        writeln!(file,"{row}").unwrap();file.flush().unwrap();
    }
    pmu::reset(false);
    let summary=json!({"status":"unsettled","reason":"maintaining agent CPU-time budget", "ticks":d.tick,
        "cpuSeconds":process_cpu()-start,"limitSeconds":limit,"end":stats(&world),
        "minimumLengthScreenTicks":30000,"usesOriginalCountedHostAdmission":true});
    fs::write(root.join("summary.json"),serde_json::to_vec_pretty(&summary).unwrap()).unwrap();
    println!("{summary}");
}
