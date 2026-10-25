use slither_sim::{Config, Origin, Point, Simulator};
use std::{
    f64::consts::TAU,
    io::{self, Write},
};
#[allow(dead_code)]
mod cautious;

// Synthetic respawns on the preset ring, using a separate seeded LCG.
// Accept the first contact-clear pose out of 64 draws; defer one second on
// failure. Full best-margin host admission is too costly for this study budget.
fn respawn(world: &mut Simulator, slot: usize, random: &mut u64) -> Result<bool, String> {
    let mut unit = || {
        *random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*random >> 11) as f64 / (1u64 << 53) as f64
    };
    for _ in 0..64 {
        let angle = unit() * TAU;
        let heading = unit() * TAU;
        let radius = world.config.spawn_radius.unwrap();
        let (body, heading) = world.spawn_body(
            Point {
                x: radius * angle.cos(),
                y: radius * angle.sin(),
            },
            heading,
        )?;
        if world.body_is_clear(&body, 0.0) {
            world.respawn_body(slot, body, heading)?;
            return Ok(true);
        }
    }
    Ok(false)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    let seed: u32 = args[1].parse()?;
    let agents: usize = args[2].parse()?;
    let seconds: f64 = args[3].parse()?;
    let mut world = Simulator::new(seed, Config::battledome())?;
    for slot in 0..agents {
        world.add(format!("bot{slot}"), slot, agents)?;
    }
    let mut random = u64::from(seed) ^ 0x7265737061776e;
    let mut tick = 0usize;
    let mut retry = vec![0.0; agents];
    let config = serde_json::to_string(&world.config)?;
    eprintln!("config={config}");
    let mut output = io::BufWriter::new(io::stdout().lock());
    writeln!(
        output,
        "seed,agents,time,living,food,ambient,death,boost,food_value,ambient_value,death_value,boost_value,snake_mass,mean_score,median_score,max_score,deaths,lives,grown_lives,late_lives,late_grown_lives"
    )?;
    let mut next_sample = 0.0;
    let mut seen_deaths = 0;
    let mut births = vec![0.0; agents];
    let mut grew = vec![false; agents];
    let (mut lives, mut grown, mut late_lives, mut late_grown) = (agents, 0, 0, 0);
    loop {
        for death in &world.deaths[seen_deaths..] {
            let slot = world
                .snakes
                .iter()
                .position(|s| s.id == death.victim)
                .unwrap();
            births[slot] = -1.0;
        }
        seen_deaths = world.deaths.len();
        for (slot, snake) in world.snakes.iter().enumerate() {
            if !snake.alive {
                continue;
            }
            if births[slot] < 0.0 {
                births[slot] = tick as f64 * world.config.dt;
                grew[slot] = false;
                lives += 1;
                if births[slot] >= 300.0 {
                    late_lives += 1;
                }
            }
            if !grew[slot] && snake.score > 400.0 {
                grew[slot] = true;
                grown += 1;
                if births[slot] >= 300.0 {
                    late_grown += 1;
                }
            }
        }
        if (tick as f64 * world.config.dt) >= next_sample
            || (tick as f64 * world.config.dt) >= seconds
        {
            let mut count = [0; 3];
            let mut value = [0.0; 3];
            for food in &world.food {
                let i = match food.origin.unwrap() {
                    Origin::Ambient => 0,
                    Origin::Death => 1,
                    Origin::Boost => 2,
                };
                count[i] += 1;
                value[i] += food.value;
            }
            let mut scores: Vec<_> = world
                .snakes
                .iter()
                .filter(|s| s.alive)
                .map(|s| s.score)
                .collect();
            scores.sort_by(f64::total_cmp);
            let n = scores.len();
            let median = if n == 0 {
                0.0
            } else {
                (scores[(n - 1) / 2] + scores[n / 2]) / 2.0
            };
            let mass: f64 = world
                .snakes
                .iter()
                .filter(|s| s.alive)
                .map(|s| s.segments as f64 + s.fam)
                .sum();
            writeln!(
                output,
                "{seed},{agents},{:.3},{n},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{mass:.6},{:.3},{median:.3},{:.3},{seen_deaths},{lives},{grown},{late_lives},{late_grown}",
                (tick as f64 * world.config.dt),
                world.food.len(),
                count[0],
                count[1],
                count[2],
                value.iter().sum::<f64>(),
                value[0],
                value[1],
                value[2],
                scores.iter().sum::<f64>() / n.max(1) as f64,
                scores.last().unwrap_or(&0.0)
            )?;
            output.flush()?;
            next_sample += 10.0;
        }
        if (tick as f64 * world.config.dt) >= seconds {
            break;
        }
        for slot in 0..agents {
            if !world.snakes[slot].alive && tick as f64 * world.config.dt >= retry[slot] {
                respawn(&mut world, slot, &mut random)?;
                retry[slot] = tick as f64 * world.config.dt + 1.0;
            }
        }
        let views = world.observe_all();
        let actions: Vec<_> = views
            .iter()
            .map(|view| view.as_ref().map(cautious::action))
            .collect();
        for _ in 0..6 {
            world.step(&actions)?;
            tick += 1;
        }
    }
    Ok(())
}
