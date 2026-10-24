//! Maintainer-only, offline portable-policy calibration and decision-cost probe.
//! See research/bots/2026-10-24-reference-policy/REPORT.md for the frozen plan.
use serde_json::json;
use slither_sim::{
    Action, Config, PickupItem,
    client::{
        Bot, CONTRACT, ClientFrame, ClientStatus, ReferenceBot, SimulationConfig, SimulatorBackend,
    },
};
use std::{f64::consts::TAU, hint::black_box, time::Instant};

const AGENTS: usize = 16;
const DECISION_TICKS: usize = 6;
const KINDS: [&str; 5] = ["reference", "greedy", "circle", "idle", "suicide"];

fn control(kind: &str, frame: &ClientFrame) -> Option<Action> {
    if kind == "reference" {
        return ReferenceBot.decide(frame);
    }
    if frame.status != ClientStatus::Playing || kind == "idle" {
        return None;
    }
    let o = frame.observation.as_ref()?;
    let head = o.me.head;
    let toward = |p: slither_sim::Point| (p.y - head.y).atan2(p.x - head.x);
    let heading = match kind {
        "greedy" if head.distance(o.arena.center) > o.arena.radius * 0.8 => toward(o.arena.center),
        "greedy" => o
            .food
            .iter()
            .min_by(|a, b| {
                head.distance(a.point())
                    .total_cmp(&head.distance(b.point()))
            })
            .map_or(o.me.heading, |f| toward(f.point())),
        "circle" => o.me.heading + 0.3,
        "suicide" => toward(o.arena.center) + TAU / 2.0,
        _ => unreachable!(),
    };
    Some(Action {
        heading,
        boost: false,
    })
}

fn backend(seed: u32, agents: usize) -> SimulatorBackend {
    SimulatorBackend::new(
        seed,
        SimulationConfig {
            agents,
            world: Config::official(agents),
            decision_ticks: DECISION_TICKS,
            broadcast_ticks: DECISION_TICKS,
            ..Default::default()
        },
    )
    .unwrap()
}

fn matches(start: u32, count: u32, seconds: f64) {
    let ticks = (seconds / 0.007).round() as usize;
    for seed in start..start + count {
        for rotation in [0, 5] {
            let mut backend = backend(seed, AGENTS);
            let initial_config = backend.config().clone();
            let kinds: Vec<_> = (0..AGENTS)
                .map(|slot| {
                    KINDS[match (slot + rotation) % AGENTS {
                        0..=7 => 0,
                        8..=9 => 1,
                        10..=11 => 2,
                        12..=13 => 3,
                        _ => 4,
                    }]
                })
                .collect();
            let mut alive_ticks = [0usize; AGENTS];
            let mut food_value = [0.0; AGENTS];
            let mut score_auc = [0.0; AGENTS];
            let mut decisions = [0usize; AGENTS];
            let mut checksum = 0u64;
            for tick in 0..ticks {
                if tick % DECISION_TICKS == 0 {
                    for (slot, kind) in kinds.iter().enumerate() {
                        let frame = backend.frame_for(slot).unwrap();
                        assert_eq!(frame.contract, CONTRACT);
                        if let Some(action) = control(kind, &frame) {
                            checksum = checksum.rotate_left(7) ^ action.heading.to_bits();
                            decisions[slot] += 1;
                            // Fixed zero processing: wall-clock benchmark cost must not
                            // change the competence experiment's actions or tick timing.
                            backend.set_slot_processing_seconds(slot, 0.0).unwrap();
                            backend.send_to(slot, action).unwrap();
                        }
                    }
                }
                for (slot, snake) in backend.world().snakes.iter().enumerate() {
                    if snake.alive {
                        alive_ticks[slot] += 1;
                        score_auc[slot] += snake.score * 0.007;
                    }
                }
                // Collect every physics tick: step_all would hide pickups on
                // the first five ticks of a six-tick decision interval.
                backend.advance_ticks(1).unwrap();
                for event in backend.world().pickup_events.as_ref().unwrap() {
                    if let PickupItem::Food(food) = &event.item {
                        food_value[event.eater] += food.value;
                    }
                }
            }
            let rows: Vec<_> = (0..AGENTS).map(|slot| json!({
                "slot":slot, "kind":kinds[slot], "survival":alive_ticks[slot] as f64 / ticks as f64,
                "food_value":food_value[slot], "score_auc":score_auc[slot], "decisions":decisions[slot],
                "kills":backend.world().snakes[slot].kills,
            })).collect();
            println!(
                "{}",
                json!({"seed":seed,"rotation":rotation,"ticks":ticks,"config":initial_config,
                "contract":CONTRACT,"checksum":checksum,"rows":rows})
            );
        }
    }
}

fn cost(agents: usize, iterations: usize) {
    let mut backend = backend(7, agents);
    let mut frames = Vec::new();
    for step in 0..240 {
        let current: Vec<_> = (0..agents)
            .map(|slot| backend.frame_for(slot).unwrap())
            .collect();
        let actions: Vec<_> = current.iter().map(|f| ReferenceBot.decide(f)).collect();
        if step % 15 == 0 {
            frames.extend(
                current
                    .into_iter()
                    .filter(|f| f.status == ClientStatus::Playing),
            );
        }
        backend.step_all(&actions).unwrap();
    }
    assert!(!frames.is_empty());
    let mut bot = ReferenceBot;
    for frame in &frames {
        black_box(bot.decide(black_box(frame)));
    }
    for repeat in 0..3 {
        let started = Instant::now();
        let cpu = cpu_seconds();
        for i in 0..iterations {
            black_box(bot.decide(black_box(&frames[i % frames.len()])));
        }
        let cpu_seconds = cpu_seconds().zip(cpu).map(|(end, start)| end - start);
        println!(
            "{}",
            json!({"agents":agents,"repeat":repeat,"decisions":iterations,"frames":frames.len(),
            "wall_us_per_decision":started.elapsed().as_secs_f64()*1e6/iterations as f64,
            "cpu_us_per_decision":cpu_seconds.map(|s|s*1e6/iterations as f64),
            "mean_food":frames.iter().map(|f|f.observation.as_ref().unwrap().food.len()).sum::<usize>() as f64/frames.len() as f64,
            "policy_state_bytes":std::mem::size_of::<ReferenceBot>(),"contract":CONTRACT})
        );
    }
}

// Match the existing Windows perf tools' user+kernel process accounting.
// Other platforms still report elapsed cost; scored Linux accounting is a later harness step.
#[cfg(windows)]
fn cpu_seconds() -> Option<f64> {
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
        fn GetProcessTimes(
            process: *mut std::ffi::c_void,
            creation: *mut u64,
            exit: *mut u64,
            kernel: *mut u64,
            user: *mut u64,
        ) -> i32;
    }
    let (mut creation, mut exit, mut kernel, mut user) = (0u64, 0u64, 0u64, 0u64);
    // SAFETY: pseudo-handle for this process; four writable FILETIME-sized values.
    let ok = unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )
    };
    (ok != 0).then_some((kernel + user) as f64 * 1e-7)
}

#[cfg(not(windows))]
fn cpu_seconds() -> Option<f64> {
    None
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("matches") if args.len() == 5 => matches(
            args[2].parse().unwrap(),
            args[3].parse().unwrap(),
            args[4].parse().unwrap(),
        ),
        Some("cost") if args.len() == 4 => cost(args[2].parse().unwrap(), args[3].parse().unwrap()),
        _ => panic!("reference_study matches START COUNT SECONDS | cost AGENTS DECISIONS"),
    }
}
