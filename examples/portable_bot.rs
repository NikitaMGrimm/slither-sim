//! Runs the shared default Forager policy. `cargo run --release --locked --example portable_bot -- --steps 200`
use slither_sim::client::{
    Backend, BotRunner, ClientStatus, ForagerBot, NetworkBackend, NetworkConfig, SimulationConfig,
    SimulatorBackend,
};
fn run(backend: impl Backend, steps: usize, lives: u64) -> Result<(), String> {
    let mut runner = BotRunner::new(backend);
    let mut bot = ForagerBot::new();
    let mut ended = 0;
    for step in 1..=steps {
        let frame = runner.step(&mut bot)?; // Observe, decide, then send the action.
        if matches!(
            frame.status,
            ClientStatus::Dead | ClientStatus::Disconnected
        ) {
            ended += 1;
            if ended >= lives {
                println!("steps={step} ended={ended} status={:?}", frame.status);
                return Ok(());
            }
            runner.backend_mut().reconnect()?; // Each new life is explicit.
        }
        if step == steps {
            println!("steps={step} ended={ended} status={:?}", frame.status);
        }
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (mut seed, mut steps, mut lives) = (None, 500, 1);
    let mut nickname = None;
    let (mut url, mut config) = (None, NetworkConfig::default());
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let value = args.next().ok_or(format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--seed" => seed = Some(value.parse::<u32>()?),
            "--steps" => steps = value.parse()?,
            "--lives" => lives = value.parse()?,
            "--url" => url = Some(value),
            "--origin" => config.origin = Some(value),
            "--nickname" => nickname = Some(value),
            _ => return Err(format!("Unknown argument: {flag}").into()),
        }
    }
    if steps == 0 || lives == 0 {
        return Err("--steps and --lives must be positive".into());
    }
    if let Some(url) = url {
        if seed.is_some() {
            return Err("--seed applies to the simulator only".into());
        }
        config.url = url;
        config.nickname = nickname.unwrap_or(config.nickname);
        run(NetworkBackend::connect(config)?, steps, lives)?;
    } else {
        if config.origin.is_some() || nickname.is_some() {
            return Err("--origin and --nickname require --url".into());
        }
        let backend = SimulatorBackend::new(seed.unwrap_or(1), SimulationConfig::default())?;
        run(backend, steps, lives)?;
    }
    Ok(())
}
