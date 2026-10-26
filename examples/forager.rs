//! Runs the shared Forager policy on direct core observations, without networking.
//! `cargo run --locked --no-default-features --features bot --example forager`
//!
//! Direct observations have no transport delay or received opponent intentions;
//! `portable_bot` runs the same policy on received frames.
use slither_sim::{
    Config, InitialPopulation, Placement, Simulator, WorldOptions,
    bot::{Forager, Timing},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut world = Simulator::new(WorldOptions {
        seed: 1,
        config: Config::sandbox(),
        population: InitialPopulation::Placed {
            ids: vec!["Forager".into()],
            placement: Placement::default(),
        },
    })?;
    let mut bot = Forager::new();
    let timing = Timing::default(); // Explicit zero observation/command/processing delay.
    for _ in 0..200 {
        if !world.snakes()[0].alive {
            break;
        }
        let action = bot.decide(&world.observe(0)?, &timing, &[]);
        world.step(&[Some(action)])?;
    }
    println!("score={}", world.snakes()[0].score);
    Ok(())
}
