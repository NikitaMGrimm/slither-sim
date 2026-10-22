use slither_sim::{Action, Config, Simulator};

fn main() -> Result<(), String> {
    let mut world = Simulator::new(7, Config::default())?;
    for slot in 0..4 {
        world.add(slot.to_string(), slot, 4)?;
    }
    for _ in 0..300 {
        let actions: Vec<_> = world
            .observe_all()
            .into_iter()
            .map(|observation| {
                observation.map(|observation| {
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
