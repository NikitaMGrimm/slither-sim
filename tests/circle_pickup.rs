use serde::Deserialize;
use slither_sim::*;

#[derive(Deserialize)]
struct Drop {
    time: f64,
    food: Food,
}

#[derive(Deserialize)]
struct Circle {
    initial: Snake,
    drops: Vec<Drop>,
}

#[test]
fn recorded_circle_retains_food_until_boost_exhaustion() {
    let circle: Circle =
        serde_json::from_str(include_str!("fixtures/boost-circle-food.json")).unwrap();
    // The official capture has no pickups during these nine seconds of full-speed
    // circling. Reuse its food coordinates, not a simulator-generated expected result.
    for seed in 1..=10 {
        let mut sim = Simulator::new(
            seed,
            Config {
                radius: 50000.0,
                food_count: 0,
                ambient_replenish_per_second: 0.0,
                emit_boost_food: false,
                ..Config::sandbox()
            },
        )
        .unwrap();
        sim.add("human", 0, 1).unwrap();
        sim.snakes[0] = circle.initial.clone();
        sim.pickup_events = Some(vec![]);
        let mut next_drop = 0;
        let mut exhausted_at = None;
        let mut collected_after = 0;
        while sim.time < 12.0 {
            while next_drop < circle.drops.len() && circle.drops[next_drop].time <= sim.time {
                sim.food.push(circle.drops[next_drop].food.clone());
                next_drop += 1;
            }
            sim.step(&[Some(Action {
                heading: sim.snakes[0].heading + std::f64::consts::FRAC_PI_2,
                boost: true,
            })])
            .unwrap();
            // Observed positions replace generated drops before pickup checks, while
            // normal boost costs, timing, and random-number consumption remain active.
            let pickups = sim.pickup_events.as_ref().unwrap();
            if sim.is_boosting(0) {
                assert!(
                    pickups.is_empty(),
                    "seed {seed}: food collected during steady boost at {} seconds",
                    sim.time
                );
            } else {
                exhausted_at.get_or_insert(sim.time);
                collected_after += pickups.len();
            }
        }
        // The recorded transition is at 9.11 s. Hidden initial drop phase and random
        // boost costs prevent exact tick agreement; bound them to two drop intervals.
        assert!((8.8..9.4).contains(&exhausted_at.unwrap()));
        assert!(
            collected_after > 10,
            "food must become collectible on slowdown"
        );
    }
}
