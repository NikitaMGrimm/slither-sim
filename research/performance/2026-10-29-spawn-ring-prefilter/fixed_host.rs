//! Research-only child of client::server. Real queue and refusal/retry methods;
//! synthetic fixed barriers guarantee many exhausted searches. No socket timing.
use super::*;
use std::io::Write;
#[cfg(target_os="linux")]
use std::{fs, io::BufRead, path::Path};

const SEATS: usize = 80;

fn record(out: &mut dyn Write, tag: u64, value: &[u8]) {
    out.write_all(&tag.to_le_bytes()).unwrap();
    out.write_all(&(value.len() as u64).to_le_bytes()).unwrap();
    out.write_all(value).unwrap();
}

struct Fixture {
    sim: Simulation,
    deliveries: Receiver<Vec<Delivery>>,
    _commands: Sender<Command>,
    legacy: bool,
    row: String,
}

impl Fixture {
    fn new(legacy: bool, row: &str) -> Self {
        assert!(["spawn","minimap","markers"].contains(&row));
        let mut world = crate::Config::battledome();
        world.dynamic_radius = row!="spawn";
        if row=="markers" {
            world.radius_updates=(300..=800).map(|tick|crate::RadiusUpdate {
                time:tick as f64*world.dt,radius:3584.-(tick-300) as f64,
            }).collect();
        }
        let world = RawSimulator::new(7, SimulationConfig { agents: SEATS, world, ..Default::default() }, SEATS).unwrap();
        let dt = world.dt();
        let shared = Arc::new(Shared {
            #[cfg(windows)]
            socket_wake: wake::Wake::new().unwrap(),
            clock: Clock { epoch: Instant::now(), dt, deferred: AtomicU64::new(0) },
            stop: AtomicBool::new(false), ticks: AtomicU64::new(0), busy: AtomicU64::new(0),
            players: AtomicUsize::new(0), pending: (0..SEATS).map(|_| AtomicUsize::new(0)).collect(),
            failure: Mutex::new(None),
        });
        let (commands, inbox) = mpsc::channel();
        let (route, deliveries) = mpsc::channel();
        let mut sim = Simulation { world, commands: inbox, routes: vec![route], batches: vec![Vec::new()],
            max_clients: SEATS, generations: vec![0; SEATS], waiting: VecDeque::new(), next_spawn_tick: 0,
            last_spawn_tick: None, shared, ticks: 0, deferred: 0 };
        let observer = PendingJoin { slot: 0, generation: 1, appearance: PlayerAppearance::default(),
            session_id: false, physical_food: false, protocol13: legacy, full_minimap: false,
            requested_tick: 0, reset_kills: true };
        assert!(sim.join(&observer).unwrap());
        sim.generations[0] = 1;
        sim.world.study_observer();
        sim.world.study_fences(true);
        for slot in 8..SEATS {
            sim.generations[slot] = 1;
            sim.waiting.push_back(PendingJoin { slot, generation: 1, appearance: PlayerAppearance {
                nickname: format!("Wait{slot}").into_bytes(), ..Default::default() },
                session_id: false, physical_food: false, protocol13: legacy, full_minimap: false,
                requested_tick: 0, reset_kills: true });
        }
        if row!="spawn" {
            sim.waiting.clear();
            sim.generations.fill(1);
            sim.world.study_viewers(legacy,true);
        }
        Self { sim, deliveries, _commands: commands, legacy, row:row.into() }
    }

    fn state(&self) -> serde_json::Value {
        serde_json::json!({"backend": self.sim.world.study_state(), "tick": self.sim.ticks,
            "nextSpawnTick": self.sim.next_spawn_tick, "lastSpawnTick": self.sim.last_spawn_tick,
            "waiting": self.sim.waiting.iter().map(|j|(j.slot,j.generation,j.requested_tick)).collect::<Vec<_>>(),
            "generations": self.sim.generations, "legacy": self.legacy,"row":self.row})
    }

    fn tick(&mut self, blocked: bool, trace: Option<&mut dyn Write>) -> serde_json::Value {
        if self.row=="spawn" {
            self.sim.world.study_fences(blocked);
            self.sim.world.study_observer();
        } else {
            self.sim.world.study_viewers(self.legacy,false);
        }
        let flags = self.sim.world.study_flags();
        let waiting = self.sim.waiting.len();
        let samples = crate::study_tick::begin();
        self.sim.apply_commands().unwrap();
        self.sim.advance_tick().unwrap();
        self.sim.ticks += 1;
        self.sim.hand_over().unwrap();
        let sample = crate::study_tick::end(samples);
        let after = self.sim.world.study_flags();
        let marker_changed=sample["events"][3].as_u64().unwrap()>0;
        let mut trace = trace;
        while let Ok(batch) = self.deliveries.try_recv() {
            for delivery in batch {
                match delivery {
                    Delivery::Frames { slot, generation, frames } => {
                        for (at, bytes) in frames {
                            if let Some(out) = trace.as_deref_mut() {
                                record(out, 1, &serde_json::to_vec(&(self.sim.ticks, slot, generation, at.to_bits())).unwrap());
                                record(out, 2, &bytes);
                            }
                        }
                    }
                    Delivery::Closed { reason, .. } => panic!("fixture connection closed: {reason}"),
                }
            }
        }
        serde_json::json!({"tick": self.sim.ticks, "waitingBefore": waiting, "waitingAfter": self.sim.waiting.len(),
            "foodEpoch": flags[0]!=after[0], "minimap": flags[1]!=after[1], "marker": marker_changed,
            "blocked": blocked, "sample": sample})
    }

    fn window(&mut self, ticks: usize, out: &mut dyn Write, release: bool) -> serde_json::Value {
        record(out, 0, &serde_json::to_vec(&self.state()).unwrap());
        let mut rows = Vec::new();
        for tick in 0..ticks {
            rows.push(self.tick(!release || tick < ticks / 2, Some(out)));
        }
        record(out, 3, &serde_json::to_vec(&self.state()).unwrap());
        // Timing/counter samples are excluded from the parity stream.
        let sum = |index:usize, blocked:bool| rows.iter().filter(|r|r["blocked"]==blocked)
            .map(|r|r["sample"]["events"][index].as_u64().unwrap()).sum::<u64>();
        let searches=sum(0,true); let attempts=sum(1,true); let exhausted=sum(2,true);
        if self.row=="spawn" {
            assert!(searches>=ticks as u64/5 && exhausted==searches && attempts==4096*searches,
                "blocked-band stress did not exhaust its exact candidate budget");
        } else {
            let state=self.sim.world.study_state();
            assert_eq!(state["living"],SEATS);
            assert_eq!(state["decoratedViewers"],SEATS);
        }
        let summary = serde_json::json!({"ticks": ticks, "legacy": self.legacy, "waiting": self.sim.waiting.len(),
            "row":self.row,
            "blockedSearches":searches,"blockedAttempts":attempts,"blockedExhausted":exhausted,
            "releaseSearches":sum(0,false),"releaseExhausted":sum(2,false)});
        record(out, 4, &serde_json::to_vec(&summary).unwrap());
        serde_json::json!({"summary":summary,"ticks":rows})
    }
}

pub fn run(legacy: bool, ticks: usize, measured: bool, row: &str) {
    let mut fixture = Fixture::new(legacy,row);
    // Populate actual observer/food histories and age every pending request
    // beyond the unchanged two-second fallback threshold before freezing.
    for _ in 0..300 { fixture.tick(true, None); }
    assert_eq!(fixture.sim.waiting.len(),if row=="spawn" { SEATS-8 } else { 0 });
    if !measured {
        let result = fixture.window(ticks, &mut io::BufWriter::new(io::stdout().lock()), true);
        eprintln!("{}", result);
        return;
    }
    #[cfg(not(target_os = "linux"))]
    panic!("Instruction measurements require the released Linux bench");
    #[cfg(target_os = "linux")]
    {
        unsafe extern "C" { fn fork() -> i32; fn waitpid(pid:i32,status:*mut i32,options:i32)->i32; }
        assert_eq!(fs::read_dir("/proc/self/task").unwrap().count(), 1);
        eprintln!("READY {}", fixture.state());
        for line in io::stdin().lock().lines() {
            let line = line.unwrap();
            if line == "quit" { break; }
            let words:Vec<_>=line.split_whitespace().collect();
            assert!(words.len()==2 && ["child","parent"].contains(&words[0]));
            let path = Path::new(words[1]); assert!(!path.exists());
            if words[0]=="parent" {
                crate::study_tick::enable();
                let mut output=io::BufWriter::new(fs::File::create(path.with_extension("trace")).unwrap());
                let result=fixture.window(ticks,&mut output,false);output.flush().unwrap();
                fs::write(path,serde_json::to_vec(&result).unwrap()).unwrap();
                eprintln!("DONE {}",path.display());break;
            }
            let pid = unsafe { fork() }; assert!(pid >= 0);
            if pid == 0 {
                crate::study_tick::enable();
                let trace = path.with_extension("trace");
                let mut output = io::BufWriter::new(fs::File::create(trace).unwrap());
                let result = fixture.window(ticks, &mut output, false);
                output.flush().unwrap();
                fs::write(path, serde_json::to_vec(&result).unwrap()).unwrap();
                std::process::exit(0);
            }
            let mut status = 0; assert_eq!(unsafe { waitpid(pid,&mut status,0) },pid); assert_eq!(status,0);
            eprintln!("DONE {}",path.display());
        }
    }
}
