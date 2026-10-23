use slither_sim::*;

#[test]
fn recorded_boost_food_preserves_debits_motion_and_random_state() {
    let config = Config {
        radius: 50000.0,
        food_count: 0,
        ambient_replenish_per_second: 0.0,
        ..Config::sandbox()
    };
    assert!(config.emit_boost_food);
    let old: Config = serde_json::from_str("{}").unwrap();
    assert!(old.emit_boost_food);
    let mut generated = Simulator::new(17, config).unwrap();
    generated.add("self", 0, 1).unwrap();
    generated.snakes[0].fam = 20.0;
    let mut recorded = generated.clone();
    recorded.config.emit_boost_food = false;
    let actions = [Some(Action {
        heading: 0.0,
        boost: true,
    })];
    for _ in 0..200 {
        generated.step(&actions).unwrap();
        recorded.step(&actions).unwrap();
        assert_eq!(generated.snakes, recorded.snakes);
        assert_eq!(generated.boost_accounting(0), recorded.boost_accounting(0));
        assert!(recorded.food.is_empty());
    }
    assert!(generated.food.len() > 5);
    recorded.config.emit_boost_food = true;
    for _ in 0..30 {
        generated.step(&actions).unwrap();
        recorded.step(&actions).unwrap();
    }
    assert_eq!(generated.food.last(), recorded.food.last());
    assert_eq!(generated.snakes, recorded.snakes);
}
