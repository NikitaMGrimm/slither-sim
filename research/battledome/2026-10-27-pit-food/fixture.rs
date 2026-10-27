// Included inside server::tests in isolated archived copies only.
#[test]
#[ignore]
fn pit_food_supply_study() {
    let seats: usize = std::env::var("STUDY_SEATS").unwrap().parse().unwrap();
    let seed: u32 = std::env::var("STUDY_SEED").unwrap().parse().unwrap();
    let seconds: f64 = std::env::var("STUDY_SECONDS").unwrap().parse().unwrap();
    let config = SimulationConfig {
        agents: seats + 1,
        world: crate::Config::battledome(),
        delivery: super::super::DeliveryProfile::Responsive,
        ..Default::default()
    };
    let (mut sim, gate, _unused) = connected(LatencyConfig::default(), 0);
    sim.world = RawSimulator::new(seed, config, seats + 1).unwrap();
    sim.max_clients = 1;
    sim.generations = vec![0; seats + 1];
    sim.ticks = 0;
    sim.next_spawn_tick = 0;
    sim.last_spawn_tick = None;
    sim.waiting.clear();
    let (socket_route, _socket_frames) = mpsc::channel();
    let (bot_route, frames) = mpsc::channel();
    sim.routes = vec![socket_route, bot_route];
    sim.batches = vec![Vec::new(), Vec::new()];
    let mut clients: Vec<_> = (1..=seats).map(|slot| Seat::new(slot, slot - 1, 0.042)).collect();
    let mut policy = ForagerBot::new();
    let latency = LatencyConfig::default();
    let mut random = Random::new(u64::from(seed) ^ 0x626f_7473_0000);
    for seat in &mut clients { seat.join(0., &gate.commands, &latency, &mut random).unwrap(); }
    let ticks = (seconds / sim.world.dt()).ceil() as u64;
    let mut minimum = sim.world.study_world().food().len();
    let mut maximum = minimum;
    let mut samples = vec![food_supply_sample(sim.world.study_world(), 0)];
    let mut next_sample = 1.;
    for tick in 0..ticks {
        let now = tick as f64 * sim.world.dt();
        for seat in &mut clients {
            seat.run(now, &gate.commands, &latency, &mut random, 0.042, &mut policy).unwrap();
        }
        sim.apply_commands().unwrap();
        sim.advance_tick().unwrap();
        sim.ticks += 1;
        sim.hand_over().unwrap();
        for batch in frames.try_iter() {
            for delivery in batch {
                let (slot, generation) = delivery.session();
                let seat = &mut clients[slot - 1];
                if generation != seat.generation { continue; }
                match delivery {
                    Delivery::Frames { frames, .. } => seat.frames.extend(frames),
                    Delivery::Closed { .. } => seat.close(now),
                }
            }
        }
        let world = sim.world.study_world();
        minimum = minimum.min(world.food().len());
        maximum = maximum.max(world.food().len());
        if world.time() + 1e-9 >= next_sample || tick + 1 == ticks {
            samples.push(food_supply_sample(world, tick + 1));
            next_sample += 1.;
            if (samples.len() - 1).is_multiple_of(30) {
                eprintln!("sample {}", samples.last().unwrap());
            }
        }
    }
    let world = sim.world.study_world();
    assert!(world.living() > 0);
    let result = serde_json::json!({
        "arm": std::env::var("STUDY_ARM").unwrap(), "seed": seed, "seats": seats,
        "requestedSeconds": seconds, "seconds": world.time(), "ticks": ticks,
        "minimumFood": minimum, "maximumFood": maximum, "pending": sim.waiting.len(),
        "config": world.config(), "samples": samples,
    });
    std::fs::write(std::env::var("STUDY_OUTPUT").unwrap(), serde_json::to_vec_pretty(&result).unwrap()).unwrap();
}

fn food_supply_sample(world: &crate::Simulator, tick: u64) -> serde_json::Value {
    let pit = world.pit_radius().unwrap();
    let (mut inside, mut outside, mut inside_nutrition, mut outside_nutrition) = (0, 0, 0., 0.);
    for food in world.food() {
        if food.point().distance(crate::Point::default()) < pit {
            inside += 1; inside_nutrition += food.value;
        } else { outside += 1; outside_nutrition += food.value; }
    }
    let mut scores: Vec<_> = world.snakes().iter().filter(|s| s.alive).map(|s| s.score).collect();
    scores.sort_by(f64::total_cmp);
    let median = if scores.is_empty() { 0. } else if scores.len().is_multiple_of(2) {
        (scores[scores.len()/2-1] + scores[scores.len()/2]) / 2.
    } else { scores[scores.len()/2] };
    let config = world.config();
    let area = if config.dynamic_radius { (world.radius() / config.radius).powi(2) } else { 1. };
    let full_target = (config.food_count as f64 * area) as usize;
    // Generic JSON keeps the same fixture compatible with main's older Config.
    let pit_food = serde_json::to_value(config).unwrap().get("foodOnlyInsidePit").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let target = if pit_food { (full_target as f64 * (pit.min(world.radius()) / world.radius()).powi(2)) as usize } else { full_target };
    serde_json::json!({
        "tick": tick, "time": world.time(), "food": world.food().len(), "target": target,
        "inside": inside, "outside": outside, "insideNutrition": inside_nutrition,
        "outsideNutrition": outside_nutrition, "radius": world.radius(), "pit": pit,
        "living": scores.len(), "meanScore": scores.iter().sum::<f64>() / scores.len().max(1) as f64,
        "medianScore": median, "maxScore": scores.last().copied().unwrap_or(0.),
        "kills": world.deaths().iter().filter(|d| d.killer.is_some()).count(),
        "deaths": world.deaths().len(), "deathsWithFood": world.deaths().iter().filter(|d| d.food > 0).count(),
    })
}
