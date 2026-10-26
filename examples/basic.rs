use slither_sim::{Action, Config, Simulator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut world = Simulator::new(7, Config::default())?;
    for slot in 0..4 {
        world.add(slot.to_string(), slot, 4)?;
    }
    // One buffer for every decision: `observe_all_into` refills its vectors.
    let mut observations = Vec::new();
    for _ in 0..300 {
        world.observe_all_into(&mut observations);
        let actions: Vec<_> = observations
            .iter()
            .map(|observation| {
                observation.as_ref().map(|observation| {
                    let head = observation.me.head;
                    let heading = observation
                        .food
                        .iter()
                        .min_by(|a, b| {
                            head.distance(a.point())
                                .total_cmp(&head.distance(b.point()))
                        })
                        .map_or((-head.y).atan2(-head.x), |food| {
                            (food.y - head.y).atan2(food.x - head.x)
                        });
                    Action {
                        heading,
                        boost: false,
                    }
                })
            })
            .collect();
        world.step(&actions)?;
    }
    println!("time={:.3}s living={}", world.time, world.living());
    Ok(())
}
