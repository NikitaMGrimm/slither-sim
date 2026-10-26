use slither_sim::{Action, Config, Point, Simulator};
use std::{
    fs::OpenOptions,
    io::{BufWriter, Write},
};

struct Random(u32);
impl Random {
    fn lifetime(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        -60.0 * ((f64::from(self.0) + 0.5) / 4294967296.0).ln()
    }
}

fn center(i: usize) -> Point {
    Point {
        x: ((i % 24) as f64 - 11.5) * 500.0,
        y: ((i / 24) as f64 - 9.5) * 500.0,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args().nth(1).expect("output JSONL required");
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    let mut file = BufWriter::new(file);
    let mut config = Config::official(480);
    // Reproduce the historical opt-in controller after the default changed.
    config.dynamic_radius = true;
    // Controller-isolation scene: retain actual update_radius and 7ms physics,
    // but remove food/prey/kiosk work and prescribe synthetic lifecycle events.
    config.food_count = 0;
    config.ambient_replenish_per_second = 0.0;
    config.prey.count = 0;
    config.kiosk.enabled = false;
    let initial_radius = config.radius;
    let dt = config.dt;
    let mut world = Simulator::new(7, config)?;
    let mut rng = Random(20261026);
    let mut next_event: Vec<f64> = (0..480).map(|_| rng.lifetime()).collect();
    let mut deaths = 0;
    let mut respawns = 0;
    for i in 0..480 {
        world.add(i.to_string(), i, 480)?;
        world.snakes[i].alive = false;
        let c = center(i);
        world.respawn(
            i,
            Point {
                x: c.x + 100.0,
                y: c.y,
            },
            std::f64::consts::FRAC_PI_2,
            2,
        )?;
    }
    let mut actions = vec![None; 480];
    let mut previous_radius = initial_radius;
    let mut total_variation = 0.0;
    let mut changed_ticks = 0usize;
    let mut direction_changes = 0usize;
    let mut previous_direction = 0i32;
    let mut next_sample = 0.0;
    let mut target_variation = 0.0;
    let mut previous_target = initial_radius;
    while world.time < 180.0 {
        for i in 0..480 {
            if world.time >= next_event[i] {
                if world.snakes[i].alive {
                    world.snakes[i].alive = false;
                    next_event[i] = world.time + 2.0;
                    deaths += 1;
                } else {
                    let c = center(i);
                    world.respawn(
                        i,
                        Point {
                            x: c.x + 100.0,
                            y: c.y,
                        },
                        std::f64::consts::FRAC_PI_2,
                        2,
                    )?;
                    next_event[i] = world.time + rng.lifetime();
                    respawns += 1;
                }
            }
            let s = &world.snakes[i];
            let c = center(i);
            let x = s.head.x - c.x;
            let y = s.head.y - c.y;
            let r = x.hypot(y);
            let pull = (r - 100.0) / 50.0;
            actions[i] = s.alive.then_some(Action {
                heading: (x - pull * y).atan2(-y - pull * x),
                boost: false,
            });
        }
        let alive_before = world.living();
        let target = 670.0 * (alive_before as f64).sqrt();
        target_variation += (target - previous_target).abs();
        previous_target = target;
        world.step(&actions)?;
        assert!(
            world.deaths.is_empty(),
            "unexpected physical death invalidates synthetic lifecycle schedule"
        );
        let dr = world.config.radius - previous_radius;
        total_variation += dr.abs();
        if dr.abs() > 1e-10 {
            changed_ticks += 1;
            let direction = if dr > 0.0 { 1 } else { -1 };
            if previous_direction != 0 && previous_direction != direction {
                direction_changes += 1;
            }
            previous_direction = direction;
        }
        previous_radius = world.config.radius;
        // update_radius runs before the rest of the tick. This scene has no
        // physical deaths; its controller must therefore equal this scalar step.
        let expected = initial_radius; // counterfactual fixed/connected-slot target
        if world.time + 1e-9 >= next_sample {
            writeln!(
                file,
                "{}",
                serde_json::json!({"time": world.time, "living": world.living(),
                "radius": world.config.radius, "target": target, "connected_slot_counterfactual": expected,
                "fixed_counterfactual": expected, "deaths": deaths, "respawns": respawns,
                "total_variation": total_variation, "target_variation": target_variation,
                "changed_ticks": changed_ticks, "direction_changes": direction_changes})
            )?;
            next_sample += 0.1;
        }
    }
    writeln!(
        file,
        "{}",
        serde_json::json!({"summary": true, "seconds": world.time,
        "dt": dt, "initial_radius": initial_radius, "living": world.living(), "deaths": deaths,
        "respawns": respawns, "total_variation": total_variation, "target_variation": target_variation,
        "changed_ticks": changed_ticks, "direction_changes": direction_changes})
    )?;
    file.flush()?;
    println!(
        "completed {:.3}s, deaths={}, respawns={}, radius={:.3}, variation={:.3}, direction_changes={}",
        world.time, deaths, respawns, world.config.radius, total_variation, direction_changes
    );
    Ok(())
}
