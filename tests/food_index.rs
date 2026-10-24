use slither_sim::{Action, Config, Food, Origin, Simulator};

fn world(food_index: bool) -> Simulator {
    let mut world = Simulator::new(
        11,
        Config {
            food_count: 3000,
            food_index,
            ..Config::sandbox()
        },
    )
    .unwrap();
    for i in 0..40 {
        world.add(format!("s{i}"), i, 40).unwrap();
    }
    world.pickup_events = Some(Vec::new());
    world
}

#[test]
fn indexed_pickup_matches_scanning_every_eater() {
    let (mut indexed, mut scanned) = (world(true), world(false));
    // Compact enough for the dense eater grid, which handles every tick here.
    assert!(40 * indexed.food.len() >= 32000);
    let mut pickups = 0;
    for tick in 0..400 {
        let actions: Vec<Option<Action>> = (0..40)
            .map(|i| {
                indexed.snakes[i].alive.then(|| Action {
                    heading: i as f64 * 2.4 + tick as f64 * 0.003 * (i % 5) as f64,
                    boost: (tick / 40 + i) % 4 == 0,
                })
            })
            .collect();
        if tick % 50 == 25 {
            // Public edits between steps: a moved, a relabeled and an injected pellet.
            for world in [&mut indexed, &mut scanned] {
                world.food[7].x += 75.0;
                world.food[9].origin = Some(Origin::Boost);
                world.food.push(Food {
                    x: -0.0,
                    y: 3.0,
                    value: 1.0,
                    size: Some(2.0),
                    origin: Some(Origin::Ambient),
                });
            }
        }
        indexed.step(&actions).unwrap();
        scanned.step(&actions).unwrap();
        assert_eq!(indexed.pickup_events, scanned.pickup_events, "tick {tick}");
        pickups += indexed.pickup_events.as_ref().unwrap().len();
        if tick % 20 == 0 {
            scanned.config.food_index = true;
            assert_eq!(
                serde_json::to_string(&indexed).unwrap(),
                serde_json::to_string(&scanned).unwrap(),
                "tick {tick}"
            );
            scanned.config.food_index = false;
        }
    }
    assert!(pickups > 100, "only {pickups} pickups");
    assert!(!indexed.deaths.is_empty());
}

#[test]
fn hashed_pickup_matches_scanning_every_eater() {
    // Snakes spread over a wide arena, each heading into a line of pellets.
    let world = |food_index| {
        let mut world = Simulator::new(
            5,
            Config {
                radius: 20_000.0,
                food_count: 3000,
                food_index,
                ..Config::sandbox()
            },
        )
        .unwrap();
        for i in 0..40 {
            world.add(format!("s{i}"), i, 40).unwrap();
        }
        world.food = (0..3000)
            .map(|k| {
                let s = &world.snakes[k % 40];
                let d = 20.0 + 8.0 * (k / 40) as f64;
                Food {
                    x: s.head.x + d * s.heading.cos(),
                    y: s.head.y + d * s.heading.sin(),
                    value: 1.0,
                    size: Some(2.0),
                    origin: Some(Origin::Ambient),
                }
            })
            .collect();
        world.pickup_events = Some(Vec::new());
        world
    };
    let (mut indexed, mut scanned) = (world(true), world(false));
    // Covers span far more 128-unit cells than four per pellet, so the dense
    // grid declines, while eaters times pellets reaches the hashed threshold.
    let (xs, ys): (Vec<f64>, Vec<f64>) =
        indexed.snakes.iter().map(|s| (s.head.x, s.head.y)).unzip();
    let span = |v: &[f64]| {
        let (lo, hi) = v
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        (hi - lo) / 128.0
    };
    let cells = span(&xs) * span(&ys);
    assert!(cells > 4.0 * 3000.0, "{cells} cells");
    assert!(40 * indexed.food.len() >= 32000);
    let headings: Vec<f64> = indexed.snakes.iter().map(|s| s.heading).collect();
    let mut pickups = 0;
    for tick in 0..400 {
        let actions: Vec<Option<Action>> = (0..40)
            .map(|i| {
                indexed.snakes[i].alive.then(|| Action {
                    heading: headings[i],
                    boost: false,
                })
            })
            .collect();
        indexed.step(&actions).unwrap();
        scanned.step(&actions).unwrap();
        assert_eq!(indexed.pickup_events, scanned.pickup_events, "tick {tick}");
        pickups += indexed.pickup_events.as_ref().unwrap().len();
    }
    scanned.config.food_index = true;
    assert_eq!(
        serde_json::to_string(&indexed).unwrap(),
        serde_json::to_string(&scanned).unwrap()
    );
    assert!(pickups > 1000, "only {pickups} pickups");
}
