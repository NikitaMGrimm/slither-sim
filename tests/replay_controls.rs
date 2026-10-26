mod support;
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
    let mut generated = Simulator::new(slither_sim::WorldOptions {
        seed: 17,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["self".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    {
        let mut snake = generated.snakes()[0].clone();
        snake.fam = 20.0;
        generated
            .correct_mass(0, snake.body.clone(), snake.fam)
            .unwrap();
    }
    let mut recorded = generated.clone();
    {
        let mut config = recorded.config().clone();
        config.emit_boost_food = false;
        support::reconfigure(&mut recorded, config);
    }
    let actions = [Some(Action {
        heading: 0.0,
        boost: true,
    })];
    for _ in 0..200 {
        generated.step(&actions).unwrap();
        recorded.step(&actions).unwrap();
        assert_eq!(generated.snakes(), recorded.snakes());
        assert_eq!(generated.boost_accounting(0), recorded.boost_accounting(0));
        assert!(recorded.food().is_empty());
    }
    assert!(generated.food().len() > 5);
    {
        let mut config = recorded.config().clone();
        config.emit_boost_food = true;
        support::reconfigure(&mut recorded, config);
    }
    for _ in 0..30 {
        generated.step(&actions).unwrap();
        recorded.step(&actions).unwrap();
    }
    assert_eq!(generated.food().last(), recorded.food().last());
    assert_eq!(generated.snakes(), recorded.snakes());
}
