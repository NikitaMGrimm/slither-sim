//! Runs the shared default Forager with received frames, or zero timing without networking.
//! `cargo run --release --locked --example cautious_bot -- --steps 200`
#[cfg(feature = "networking")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use slither_sim::client::{
        Backend, BotRunner, ClientStatus, ForagerBot, SimulationConfig, SimulatorBackend,
    };
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
    let mut bot = ForagerBot::new(slither_sim::bot::PolicyId::default());
    let mut ended = 0;
    for step in 1..=steps {
        let frame = runner.step(&mut bot)?;
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

// Direct observations have no transport delay or received opponent intentions.
#[cfg(not(feature = "networking"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use slither_sim::{
        Config, Simulator,
        bot::{Forager, PolicyId, Timing},
    };
    let mut world = Simulator::new(1, Config::sandbox())?;
    world.add("Forager", 0, 1)?;
    let mut bot = Forager::new(PolicyId::default());
    let timing = Timing::default(); // Explicit zero observation/command/processing delay.
    for _ in 0..200 {
        if !world.snakes[0].alive {
            break;
        }
        let action = bot.decide(&world.observe(0)?, &timing, &[]);
        world.step(&[Some(action)])?;
    }
    println!("score={}", world.snakes[0].score);
    Ok(())
}
