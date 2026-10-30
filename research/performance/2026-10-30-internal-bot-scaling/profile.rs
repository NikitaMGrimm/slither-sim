//! Study-only direct-observation internal bots. No networking dependency or adapter.
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use slither_sim::{
    Action, Config, InitialPopulation, Observation, Placement, Point, RestoreOptions, Simulator,
    WorldOptions,
    bot::{Forager, Timing},
    profiling::{StepProfile, WorldStats},
};
use std::{f64::consts::TAU, fs, path::Path, time::Instant};

unsafe extern "C" {
    fn clock_gettime(clock: i32, out: *mut Timespec) -> i32;
    fn sched_setaffinity(pid: i32, size: usize, mask: *const u64) -> i32;
}
#[repr(C)]
struct Timespec {
    sec: i64,
    ns: i64,
}
fn cpu(clock: i32) -> f64 {
    let mut time = Timespec { sec: 0, ns: 0 };
    assert_eq!(unsafe { clock_gettime(clock, &mut time) }, 0);
    time.sec as f64 + time.ns as f64 * 1e-9
}
fn pin(core: usize) {
    let mut mask = [0u64; 16];
    mask[core / 64] |= 1 << (core % 64);
    assert_eq!(unsafe { sched_setaffinity(0, 128, mask.as_ptr()) }, 0);
}
fn pool() -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(3)
        .start_handler(|i| pin(i + 1))
        .build()
        .unwrap()
}
fn memory() -> Value {
    let text = fs::read_to_string("/proc/self/status").unwrap();
    let fields: serde_json::Map<String, Value> = text
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            ["VmRSS", "VmHWM", "RssAnon", "RssFile", "RssShmem", "VmSize"]
                .contains(&key)
                .then(|| {
                    (
                        key.to_string(),
                        json!(
                            value
                                .split_whitespace()
                                .next()
                                .unwrap()
                                .parse::<u64>()
                                .unwrap()
                        ),
                    )
                })
        })
        .collect();
    Value::Object(fields)
}
fn hex(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn census(world: &Simulator) -> Value {
    let mut lengths: Vec<_> = world
        .snakes()
        .iter()
        .filter(|s| s.alive)
        .map(|s| s.body.len())
        .collect();
    lengths.sort_unstable();
    let q = |p: f64| {
        lengths
            .get(((lengths.len().saturating_sub(1)) as f64 * p).ceil() as usize)
            .copied()
    };
    json!({"world":WorldStats::from_world(world),"meanLength":lengths.iter().sum::<usize>() as f64/lengths.len().max(1) as f64,
        "p50Length":q(0.5),"p90Length":q(0.9),"p99Length":q(0.99),"maxLength":q(1.),"memoryKiB":memory()})
}
#[derive(Clone, Serialize, Deserialize)]
struct Meta {
    random: u64,
    tick: u64,
    retry: Vec<u64>,
    actions: Vec<Option<Action>>,
}
struct Scene {
    world: Simulator,
    meta: Meta,
    observations: [Vec<Option<Observation>>; 6],
    masks: [Vec<bool>; 6],
    policies: [Forager; 3],
    pool: Option<rayon::ThreadPool>,
    tape: Vec<u8>,
}
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Sample {
    tick: u64,
    world_cpu: f64,
    world_wall: f64,
    admission_cpu: f64,
    admission_wall: f64,
    observation_cpu: f64,
    observation_wall: f64,
    decision_cpu: f64,
    decision_wall: f64,
    policy_loop_cpu: f64,
    decisions: usize,
    observations: usize,
    admitted: usize,
    refused: usize,
    poses: usize,
    total_wall: f64,
    profile: Option<StepProfile>,
    replay_wall: f64,
}
impl Scene {
    fn prime(&mut self) {
        self.world.set_parallel(self.pool.is_some());
        for phase in 0..6 {
            if let Some(pool) = &self.pool {
                pool.install(|| {
                    self.world
                        .observe_selected_into(&self.masks[phase], &mut self.observations[phase])
                })
                .unwrap();
                let chunk = self.meta.actions.len().div_ceil(3);
                pool.install(|| {
                    self.policies
                        .par_iter_mut()
                        .zip(self.observations[phase].par_chunks(chunk))
                        .for_each(|(policy, observations)| {
                            for observation in observations.iter().flatten() {
                                std::hint::black_box(policy.decide(
                                    observation,
                                    &Timing::default(),
                                    &[],
                                ));
                            }
                        })
                });
            } else {
                self.world
                    .observe_selected_into(&self.masks[phase], &mut self.observations[phase])
                    .unwrap();
                for observation in self.observations[phase].iter().flatten() {
                    std::hint::black_box(self.policies[0].decide(
                        observation,
                        &Timing::default(),
                        &[],
                    ));
                }
            }
        }
    }
    fn prepare(world: Simulator, meta: Meta, parallel: bool) -> Self {
        let n = world.snakes().len();
        Self {
            world,
            meta,
            observations: std::array::from_fn(|_| Vec::new()),
            masks: std::array::from_fn(|phase| {
                (0..n).map(|slot| (slot + phase) % 6 == 0).collect()
            }),
            policies: std::array::from_fn(|_| Forager::new()),
            pool: parallel.then(pool),
            tape: Vec::new(),
        }
    }
    fn new(n: usize, parallel: bool) -> Self {
        let world = Simulator::new(WorldOptions {
            seed: 7,
            config: Config::official(n),
            population: InitialPopulation::Placed {
                ids: (0..n).map(|i| i.to_string()).collect(),
                placement: Placement::Admitted {
                    attempts: 64.try_into().unwrap(),
                    preferred_clearance: 600.,
                },
            },
        })
        .unwrap();
        let meta = Meta {
            random: 7 ^ 0x626f7473,
            tick: 0,
            retry: vec![0; n],
            actions: world
                .snakes()
                .iter()
                .map(|s| {
                    Some(Action {
                        heading: s.heading,
                        boost: false,
                    })
                })
                .collect(),
        };
        Self::prepare(world, meta, parallel)
    }
    fn load(root: &Path, parallel: bool) -> Self {
        let world = Simulator::from_checkpoint(
            &fs::read(root.join("world.json")).unwrap(),
            RestoreOptions::default(),
        )
        .unwrap();
        let meta = serde_json::from_slice(&fs::read(root.join("meta.json")).unwrap()).unwrap();
        Self::prepare(world, meta, parallel)
    }
    fn save(&self, root: &Path) {
        fs::create_dir_all(root).unwrap();
        fs::write(
            root.join("world.json"),
            serde_json::to_vec(&self.world.checkpoint().unwrap()).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("meta.json"),
            serde_json::to_vec(&self.meta).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("census.json"),
            serde_json::to_vec_pretty(&census(&self.world)).unwrap(),
        )
        .unwrap();
    }
    fn unit(&mut self) -> f64 {
        self.meta.random ^= self.meta.random << 13;
        self.meta.random ^= self.meta.random >> 7;
        self.meta.random ^= self.meta.random << 17;
        (self.meta.random >> 11) as f64 / (1u64 << 53) as f64
    }
    fn admission(&mut self, sample: &mut Sample) {
        for slot in 0..self.world.snakes().len() {
            if self.world.snakes()[slot].alive {
                continue;
            }
            self.meta.actions[slot] = None;
            if self.meta.tick < self.meta.retry[slot] {
                continue;
            }
            let mut best: Option<(f64, Vec<Point>, f64)> = None;
            for _ in 0..64 {
                sample.poses += 1;
                let unit = self.unit();
                let distance = slither_sim::spawn::spawn_distance(
                    self.world.config(),
                    self.world.radius(),
                    unit,
                );
                let angle = self.unit() * TAU;
                let heading = self.unit() * TAU;
                let head = Point {
                    x: angle.cos() * distance,
                    y: angle.sin() * distance,
                };
                let (body, heading) = self.world.spawn_body(head, heading).unwrap();
                if self.world.body_is_clear(&body, 600.) {
                    best = Some((f64::INFINITY, body, heading));
                    break;
                }
                if let Some(margin) = self.world.spawn_margin(&body)
                    && margin > 0.
                    && best
                        .as_ref()
                        .is_none_or(|(previous, ..)| margin > *previous)
                {
                    best = Some((margin, body, heading));
                }
            }
            if let Some((_, body, heading)) = best {
                self.world.respawn_body(slot, body, heading).unwrap();
                self.meta.actions[slot] = Some(Action {
                    heading,
                    boost: false,
                });
                sample.admitted += 1;
            } else {
                sample.refused += 1;
                self.meta.retry[slot] = self.meta.tick + 143;
            }
        }
    }
    fn tick(
        &mut self,
        profiled: bool,
        hash: &mut Sha256,
        record_actions: bool,
        replay: Option<&[u8]>,
    ) -> Sample {
        let total = Instant::now();
        let mut s = Sample {
            tick: self.meta.tick,
            ..Default::default()
        };
        let start = Instant::now();
        let c = cpu(3);
        self.admission(&mut s);
        s.admission_cpu = cpu(3) - c;
        s.admission_wall = start.elapsed().as_secs_f64();
        if let Some(replay) = replay {
            let start = Instant::now();
            assert_eq!(replay.len(), 10 * self.meta.actions.len());
            for (action, bytes) in self.meta.actions.iter_mut().zip(replay.chunks_exact(10)) {
                *action = match bytes[0] {
                    0 => None,
                    1 => Some(Action {
                        heading: f64::from_bits(u64::from_le_bytes(
                            bytes[1..9].try_into().unwrap(),
                        )),
                        boost: bytes[9] != 0,
                    }),
                    _ => panic!("invalid replay tag"),
                };
            }
            s.replay_wall = start.elapsed().as_secs_f64();
        } else {
            let phase = (self.meta.tick % 6) as usize;
            let start = Instant::now();
            let c = cpu(2);
            self.world.set_parallel(self.pool.is_some());
            if let Some(pool) = &self.pool {
                pool.install(|| {
                    self.world
                        .observe_selected_into(&self.masks[phase], &mut self.observations[phase])
                })
                .unwrap();
            } else {
                self.world
                    .observe_selected_into(&self.masks[phase], &mut self.observations[phase])
                    .unwrap();
            }
            s.observation_cpu = cpu(2) - c;
            s.observation_wall = start.elapsed().as_secs_f64();
            let observations = &self.observations[phase];
            s.observations = observations.iter().filter(|o| o.is_some()).count();
            let start = Instant::now();
            let c = cpu(2);
            let decide = |policy: &mut Forager,
                          actions: &mut [Option<Action>],
                          observations: &[Option<Observation>]| {
                let c = cpu(3);
                let mut count = 0;
                for (action, observation) in actions.iter_mut().zip(observations) {
                    if let Some(observation) = observation {
                        *action = Some(policy.decide(observation, &Timing::default(), &[]));
                        count += 1;
                    }
                }
                (cpu(3) - c, count)
            };
            if let Some(pool) = &self.pool {
                let chunk = self.meta.actions.len().div_ceil(3);
                let counts: Vec<_> = pool.install(|| {
                    self.policies
                        .par_iter_mut()
                        .zip(self.meta.actions.par_chunks_mut(chunk))
                        .zip(observations.par_chunks(chunk))
                        .map(|((p, a), o)| decide(p, a, o))
                        .collect()
                });
                s.policy_loop_cpu = counts.iter().map(|v| v.0).sum();
                s.decisions = counts.iter().map(|v| v.1).sum();
            } else {
                let (c, n) = decide(&mut self.policies[0], &mut self.meta.actions, observations);
                s.policy_loop_cpu = c;
                s.decisions = n;
            }
            s.decision_cpu = cpu(2) - c;
            s.decision_wall = start.elapsed().as_secs_f64();
            assert_eq!(s.decisions, s.observations);
        }
        let start = Instant::now();
        let c = cpu(3);
        if profiled {
            let mut profile = StepProfile::default();
            self.world
                .step_profiled(&self.meta.actions, &mut profile)
                .unwrap();
            s.profile = Some(profile);
        } else {
            self.world.step(&self.meta.actions).unwrap();
        }
        s.world_cpu = cpu(3) - c;
        s.world_wall = start.elapsed().as_secs_f64();
        self.meta.tick += 1;
        s.total_wall = total.elapsed().as_secs_f64();
        // Trace work is outside all measured intervals.
        if record_actions {
            for action in &self.meta.actions {
                let mut bytes = [0u8; 10];
                if let Some(action) = action {
                    bytes[0] = 1;
                    bytes[1..9].copy_from_slice(&action.heading.to_bits().to_le_bytes());
                    bytes[9] = u8::from(action.boost);
                }
                hash.update(bytes);
                self.tape.extend_from_slice(&bytes);
            }
        }
        s
    }
}
fn main() {
    pin(0);
    let args: Vec<_> = std::env::args().collect();
    let mode = &args[1];
    let root = Path::new(&args[2]);
    if mode == "warm" || mode == "smoke" {
        let n = args[3].parse().unwrap();
        let ticks: u64 = args[4].parse().unwrap();
        fs::create_dir_all(root).unwrap();
        let construction = Instant::now();
        let construction_cpu = cpu(2);
        let mut scene = Scene::new(n, true);
        let mut hash = Sha256::new();
        let started = Instant::now();
        eprintln!(
            "{}",
            json!({"kind":"initial","constructionSeconds":construction.elapsed().as_secs_f64(),
            "constructionCpuSeconds":cpu(2)-construction_cpu,"census":census(&scene.world)})
        );
        for _ in 0..ticks {
            let s = scene.tick(false, &mut hash, mode == "smoke", None);
            if scene.meta.tick % 1000 == 0 {
                eprintln!(
                    "{}",
                    json!({"kind":"warm","tick":scene.meta.tick,"last":s,"census":census(&scene.world)})
                );
            }
            if scene.meta.tick % 1000 == 0
                && (root.join("STOP_WARM").exists() || started.elapsed().as_secs() > 7200)
            {
                break;
            }
        }
        scene.save(root);
        eprintln!(
            "{}",
            json!({"kind":"ready","tick":scene.meta.tick,"census":census(&scene.world),"actionSha256":hex(hash.finalize())})
        );
    } else if mode == "measure" {
        let parallel = args[3] == "parallel";
        let ticks: usize = args[4].parse().unwrap();
        let profiled = args[5] == "profile";
        let verify = args[5] == "verify";
        let output = Path::new(&args[6]);
        let mut scene = Scene::load(root, parallel);
        let mut hash = Sha256::new();
        let tape = (args[3] == "simulation").then(|| fs::read(root.join("tape.bin")).unwrap());
        let stride = scene.meta.actions.len() * 10;
        if let Some(tape) = &tape {
            assert_eq!(tape.len(), stride * ticks);
        }
        let world_memory = memory();
        if args[3] != "simulation" {
            scene.prime();
        }
        let initial = census(&scene.world);
        let start = Instant::now();
        let c = cpu(2);
        let main_c = cpu(3);
        let rows: Vec<_> = (0..ticks)
            .map(|i| {
                scene.tick(
                    profiled,
                    &mut hash,
                    verify,
                    tape.as_ref().map(|t| &t[i * stride..(i + 1) * stride]),
                )
            })
            .collect();
        let wall = start.elapsed().as_secs_f64();
        let process = cpu(2) - c;
        let main_cpu = cpu(3) - main_c;
        let final_census = census(&scene.world);
        let action_hash = hex(hash.finalize());
        let final_bytes = serde_json::to_vec(&scene.world.checkpoint().unwrap()).unwrap();
        if verify {
            fs::write(output.with_extension("tape"), &scene.tape).unwrap();
        }
        fs::write(
            output,
            serde_json::to_vec(
                &json!({"mode":args[3],"profiled":profiled,"verify":verify,"ticks":ticks,
            "initial":initial,"worldOnlyMemoryKiB":world_memory,"final":final_census,"samples":rows,"wallSeconds":wall,
            "processCpuSeconds":process,"mainCpuSeconds":main_cpu,"actionSha256":action_hash,
            "finalStateSha256":hex(Sha256::digest(&final_bytes))}),
            )
            .unwrap(),
        )
        .unwrap();
    } else {
        panic!("unknown mode")
    }
}
