//! Seeks food and steers away from the wall and other snakes it can see.
//! `cargo run --release --locked --example cautious_bot -- --steps 200`
use slither_sim::{Action, Food, Observation, Point, physics};
use std::f64::consts::{PI, TAU};

fn toward(from: Point, to: Point) -> f64 {
    (to.y - from.y).atan2(to.x - from.x)
}

fn wrap(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn decide(view: &Observation) -> Action {
    let me = &view.me;
    let turn_rate = physics::turn_rate(me.segments);
    // Penalize food behind us: a nearby pellet can otherwise cause endless circles.
    let cost = |p| {
        me.head.distance(p)
            + 2.0 * me.speed / turn_rate * wrap(toward(me.head, p) - me.heading).abs()
    };
    let target = view
        .food
        .iter()
        .map(Food::point)
        .min_by(|a, b| cost(*a).total_cmp(&cost(*b)))
        .unwrap_or(view.arena.center);
    let mut desired = toward(me.head, target);
    if me.head.distance(view.arena.center) > view.arena.radius - 220.0 {
        desired = toward(me.head, view.arena.center);
    }
    let mut best = (f64::NEG_INFINITY, me.heading);
    for delta in [
        wrap(desired - me.heading),
        0.0,
        -0.5,
        0.5,
        -1.0,
        1.0,
        -2.0,
        2.0,
        PI,
    ] {
        let wanted = me.heading + delta;
        let (mut position, mut heading) = (me.head, me.heading);
        let mut clearance = f64::INFINITY;
        for sample in 1..=4 {
            // Predict the quantized command, including the actual direction of half-turns.
            (position, heading) = physics::advance(
                position,
                heading,
                physics::quantize_heading(wanted),
                me.speed,
                turn_rate,
                0.09,
            );
            clearance = clearance
                .min(view.arena.radius - position.distance(view.arena.center) - me.radius - 20.0);
            for other in &view.opponents {
                let future_head = Point {
                    x: other.head.x + other.heading.cos() * other.speed * sample as f64 * 0.09,
                    y: other.head.y + other.heading.sin() * other.speed * sample as f64 * 0.09,
                };
                let margin = me.radius + other.radius + 25.0;
                clearance = clearance.min(position.distance(future_head) - margin);
                // Include head-to-first-anchor and every body edge, not only anchor points.
                let mut previous = other.head;
                for &anchor in &other.body {
                    clearance = clearance
                        .min(physics::segment_distance(position, previous, anchor) - margin);
                    previous = anchor;
                }
            }
        }
        let danger = if clearance < 0.0 {
            100.0 - clearance * 5.0
        } else {
            0.0
        };
        let score = 2.0 * wrap(wanted - desired).cos() - 0.08 * delta.abs()
            + 0.004 * clearance.clamp(-1000.0, 150.0)
            - danger;
        if score > best.0 {
            best = (score, wanted);
        }
    }
    Action {
        heading: best.1,
        boost: false,
    }
}

#[cfg(feature = "networking")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use slither_sim::client::{
        Backend, Bot, BotRunner, ClientFrame, ClientStatus, SimulationConfig, SimulatorBackend,
    };
    struct CautiousBot;
    impl Bot for CautiousBot {
        fn decide(&mut self, frame: &ClientFrame) -> Option<Action> {
            frame.observation.as_ref().map(decide)
        }
    }
    let (mut seed, mut steps, mut lives) = (1, 500, 1);
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seed" => seed = value.parse()?,
            "--steps" => steps = value.parse()?,
            "--lives" => lives = value.parse()?,
            _ => return Err(format!("Unknown argument: {flag}").into()),
        }
    }
    if steps == 0 || lives == 0 {
        return Err("--steps and --lives must be positive".into());
    }
    let config = SimulationConfig {
        agents: 4,
        ..SimulationConfig::default()
    };
    let mut runner = BotRunner::new(SimulatorBackend::new(seed, config)?);
    let mut ended = 0;
    for step in 1..=steps {
        let frame = runner.step(&mut CautiousBot)?;
        if matches!(
            frame.status,
            ClientStatus::Dead | ClientStatus::Disconnected
        ) {
            ended += 1;
            if ended >= lives {
                println!("steps={step} ended={ended} status={:?}", frame.status);
                break;
            }
            runner.backend_mut().reconnect()?;
        }
        if step == steps {
            println!("steps={step} ended={ended} status={:?}", frame.status);
        }
    }
    Ok(())
}

// The policy also compiles without networking; only this portable runner needs it.
#[cfg(not(feature = "networking"))]
fn main() {
    let _policy: fn(&Observation) -> Action = decide;
    eprintln!("Run with the networking feature (enabled by default).");
}
