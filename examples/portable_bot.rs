//! Run one food-seeking bot against the simulator or an explicit WebSocket server.
//!
//! ```sh
//! cargo run --release --locked --example portable_bot -- --steps 500
//! cargo run --release --locked --example portable_bot -- --url wss://HOST/PATH --origin ORIGIN
//! ```
//!
//! The same `Bot` implementation drives either backend through `BotRunner`.
//! Lost lives are reconnected only explicitly, up to `--lives` lives in total.
use slither_sim::client::{
    Backend, Bot, BotRunner, ClientFrame, ClientStatus,
    network::{NetworkBackend, NetworkConfig},
    simulation::{SimulationConfig, SimulatorBackend},
};
use slither_sim::{Action, Food, Point};
use std::process::ExitCode;

const USAGE: &str = "Usage: portable_bot [--seed N] [--steps N] [--lives N]\n       \
     portable_bot --url ws(s)://HOST/PATH [--origin ORIGIN] [--nickname NAME] [--steps N] [--lives N]";

/// Chase one visible pellet until it disappears; turn back near the boundary.
#[derive(Default)]
struct FoodBot {
    target: Option<Point>,
}

impl Bot for FoodBot {
    fn decide(&mut self, frame: &ClientFrame) -> Option<Action> {
        let observation = frame.observation.as_ref()?;
        let head = observation.me.head;
        let center = observation.arena.center;
        let toward = |point: Point| (point.y - head.y).atan2(point.x - head.x);
        if observation.food.is_empty() || head.distance(center) >= observation.arena.radius * 0.8 {
            self.target = None;
            return Some(Action {
                heading: toward(center),
                boost: false,
            });
        }
        let kept = self
            .target
            .filter(|target| observation.food.iter().any(|food| food.point() == *target));
        let target = match kept {
            Some(target) => target,
            None => observation
                .food
                .iter()
                .map(Food::point)
                .min_by(|a, b| head.distance(*a).total_cmp(&head.distance(*b)))?,
        };
        self.target = Some(target);
        Some(Action {
            heading: toward(target),
            boost: false,
        })
    }

    fn reset(&mut self) {
        self.target = None;
    }
}

struct Args {
    url: Option<String>,
    origin: Option<String>,
    nickname: Option<String>,
    seed: Option<u32>,
    steps: usize,
    lives: u64,
}

fn parse(mut raw: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut args = Args {
        url: None,
        origin: None,
        nickname: None,
        seed: None,
        steps: 500,
        lives: 1,
    };
    fn number<T: std::str::FromStr>(flag: &str, text: String) -> Result<T, String> {
        text.parse().map_err(|_| format!("Invalid {flag}: {text}"))
    }
    while let Some(flag) = raw.next() {
        let mut value = || raw.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--url" => args.url = Some(value()?),
            "--origin" => args.origin = Some(value()?),
            "--nickname" => args.nickname = Some(value()?),
            "--seed" => args.seed = Some(number(&flag, value()?)?),
            "--steps" => args.steps = number(&flag, value()?)?,
            "--lives" => args.lives = number(&flag, value()?)?,
            _ => return Err(format!("Unknown argument: {flag}")),
        }
    }
    if args.steps == 0 || args.lives == 0 {
        return Err("--steps and --lives must be positive".into());
    }
    match &args.url {
        Some(url) if !(url.starts_with("ws://") || url.starts_with("wss://")) => {
            Err("--url must be an explicit ws:// or wss:// URL".into())
        }
        Some(_) if args.seed.is_some() => Err("--seed applies to the simulator only".into()),
        None if args.origin.is_some() || args.nickname.is_some() => {
            Err("--origin and --nickname require --url".into())
        }
        _ => Ok(args),
    }
}

struct Summary {
    frames: usize,
    lives: u64,
    status: ClientStatus,
    time: f64,
}

fn run(backend: impl Backend, args: &Args) -> Result<Summary, String> {
    let mut runner = BotRunner::new(backend);
    let mut bot = FoodBot::default();
    let mut summary = Summary {
        frames: 0,
        lives: 0,
        status: ClientStatus::Waiting,
        time: 0.,
    };
    let mut last_ended = None;
    for _ in 0..args.steps {
        let frame = runner.step(&mut bot)?;
        summary.frames += 1;
        summary.status = frame.status;
        summary.time = frame.time;
        if matches!(
            frame.status,
            ClientStatus::Dead | ClientStatus::Disconnected
        ) {
            let identity = (frame.received.connection_generation, frame.life);
            if last_ended != Some(identity) {
                summary.lives += 1;
                last_ended = Some(identity);
            }
            if summary.lives >= args.lives {
                break;
            }
            eprintln!("life ended ({:?}); reconnecting", frame.status);
            runner.backend_mut().reconnect()?;
        }
    }
    Ok(summary)
}

fn main() -> ExitCode {
    let args = match parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = match &args.url {
        Some(url) => {
            let defaults = NetworkConfig::default();
            let config = NetworkConfig {
                url: url.clone(),
                origin: args.origin.clone(),
                nickname: args.nickname.clone().unwrap_or(defaults.nickname),
                ..defaults
            };
            NetworkBackend::connect(config).and_then(|backend| run(backend, &args))
        }
        None => SimulatorBackend::new(args.seed.unwrap_or(1), SimulationConfig::default())
            .and_then(|backend| run(backend, &args)),
    };
    match result {
        Ok(summary) => {
            println!(
                "frames={} lives={} status={:?} time={:.3}s",
                summary.frames, summary.lives, summary.status, summary.time
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
