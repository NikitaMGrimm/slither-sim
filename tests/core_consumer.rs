mod support;
use slither_sim::{Action, AmbientFoodConfig, Config, KioskConfig, Point, Simulator, physics};

fn snapshot(world: &Simulator) -> serde_json::Value {
    serde_json::to_value(world.checkpoint().unwrap()).unwrap()
}

fn empty_world() -> Simulator {
    Simulator::new(slither_sim::WorldOptions {
        seed: 7,
        config: Config {
            radius: 50_000.,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            ..Default::default()
        },
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap()
}

#[test]
fn large_spawn_headings_preserve_orientation_and_can_turn() {
    for operation in ["spawn", "respawn", "respawn_body"] {
        let mut world = empty_world();
        if operation != "spawn" {
            support::ring_slot(&mut world, "self", 0, 1).unwrap();
            world.retire(0).unwrap();
        }
        let heading: f64 = 1e20;
        let body = vec![Point::default(), Point { x: -42., y: 0. }];
        match operation {
            "spawn" => {
                world.spawn("self".into(), body, heading).unwrap();
            }
            "respawn_body" => world.respawn_body(0, body, heading).unwrap(),
            "respawn" => world.respawn(0, Point::default(), heading, 2).unwrap(),
            _ => unreachable!(),
        }
        let before = world.snakes()[0].heading;
        assert!((before.cos() - heading.cos()).abs() < 1e-12);
        assert!((before.sin() - heading.sin()).abs() < 1e-12);
        world.step(&[Some(Action::default())]).unwrap();
        assert!(world.snakes()[0].heading.abs() < before.abs());
    }
}

#[test]
fn selected_observations_match_individual_and_indexed_batches_without_mutation() {
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 7,
        config: Config {
            radius: 3000.,
            food_count: 1500,
            ambient_food: AmbientFoodConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        },
        population: slither_sim::InitialPopulation::Placed {
            ids: (0..32).map(|i| i.to_string()).collect(),
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    world.retire(3).unwrap();
    let before = snapshot(&world);
    let selected: Vec<_> = (0..32).map(|i| i % 3 == 0).collect();
    let all = world.observe_all();
    let observations = world.observe_selected(&selected).unwrap();
    assert_eq!(observations.len(), 32);
    for i in 0..32 {
        if selected[i] && world.snakes()[i].alive {
            assert_eq!(
                serde_json::to_value(&observations[i]).unwrap(),
                serde_json::to_value(&all[i]).unwrap()
            );
            assert_eq!(
                serde_json::to_value(observations[i].as_ref().unwrap()).unwrap(),
                serde_json::to_value(world.observe(i).unwrap()).unwrap()
            );
        } else {
            assert!(observations[i].is_none());
        }
    }
    assert!(
        world
            .observe_selected(&[false; 32])
            .unwrap()
            .iter()
            .all(Option::is_none)
    );
    for length in [0, 31, 33] {
        assert!(world.observe_selected(&vec![true; length]).is_err());
    }
    assert_eq!(snapshot(&world), before);
    assert!(empty_world().observe_selected(&[]).unwrap().is_empty());
}

#[test]
fn observations_follow_dead_only_selection_and_later_public_mutation() {
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 7,
        config: Config {
            radius: 3000.,
            food_count: 1500,
            ambient_food: AmbientFoodConfig {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        },
        population: slither_sim::InitialPopulation::Placed {
            ids: (0..32).map(|slot| slot.to_string()).collect(),
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    for slot in 0..32 {
        world.retire(slot).unwrap();
    }
    let before = snapshot(&world);
    for flags in [[false; 32], [true; 32]] {
        let observations = world.observe_selected(&flags).unwrap();
        assert_eq!(observations.len(), 32);
        assert!(observations.iter().all(Option::is_none));
    }
    let observations = world.observe_all();
    assert_eq!(observations.len(), 32);
    assert!(observations.iter().all(Option::is_none));
    assert!(world.observe_selected(&[false; 31]).is_err());
    assert_eq!(snapshot(&world), before);

    // Reuse the same slots, food length and virtual time after an empty batch.
    {
        let mut snake = world.snakes()[31].clone();
        snake.alive = true;
        world
            .import_life(
                31,
                slither_sim::ReplayLife::from_observed(
                    snake.clone(),
                    slither_sim::InitialSpeed::Infer(snake.speed),
                ),
            )
            .unwrap();
    }
    {
        let mut food = world.food().to_vec();
        food[0].x = world.snakes()[31].head.x;
        world.replace_food(food).unwrap();
    }
    {
        let mut food = world.food().to_vec();
        food[0].y = world.snakes()[31].head.y;
        world.replace_food(food).unwrap();
    }
    let mut selected = [false; 32];
    selected[31] = true;
    let batch = world.observe_selected(&selected).unwrap();
    assert!(batch[..31].iter().all(Option::is_none));
    assert_eq!(
        serde_json::to_value(batch[31].as_ref().unwrap()).unwrap(),
        serde_json::to_value(world.observe(31).unwrap()).unwrap()
    );
    let retained = serde_json::to_value(&batch).unwrap();
    {
        let mut food = world.food().to_vec();
        food[0].x += 10_000.;
        world.replace_food(food).unwrap();
    }
    let changed = world.observe_selected(&selected).unwrap();
    assert_ne!(retained, serde_json::to_value(&changed).unwrap());
    assert_eq!(retained, serde_json::to_value(&batch).unwrap());
}

#[test]
fn supplied_body_clearance_checks_intermediate_edges_heads_and_bounds() {
    let mut world = empty_world();
    {
        let mut config = world.config().clone();
        config.radius = 3000.;
        support::reconfigure(&mut world, config);
    }
    support::ring_slot(&mut world, "other", 0, 1).unwrap();
    {
        let mut snake = world.snakes()[0].clone();
        snake.head = Point { x: 1000., y: 1000. };
        world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    {
        let mut snake = world.snakes()[0].clone();
        snake.body = vec![
            snake.head,
            Point {
                x: -1000.,
                y: 1000.,
            },
            Point {
                x: -1000.,
                y: -1000.,
            },
        ];
        world
            .correct_mass(0, snake.body.clone(), snake.fam)
            .unwrap();
        world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    let crossing = [
        Point::default(),
        Point { x: 0., y: 500. },
        Point { x: -1500., y: 500. },
        Point { x: -1500., y: 0. },
    ];
    assert!(!world.body_is_clear(&crossing, 0.));
    let clear = [Point::default(), Point { x: 500., y: -500. }];
    assert!(world.body_is_clear(&clear, 100.));
    world
        .correct_mass(0, vec![Point { x: 250., y: -250. }; 2], 0.)
        .unwrap();
    {
        let mut snake = world.snakes()[0].clone();
        snake.head = Point { x: 250., y: -250. };
        world
            .correct_pose(0, snake.head, snake.body.clone(), snake.heading)
            .unwrap();
    }
    assert!(!world.body_is_clear(&clear, 0.));
    world.retire(0).unwrap();
    assert!(world.body_is_clear(&clear, 0.));
    assert!(!world.body_is_clear(&[Point::default(), Point { x: 2940., y: 0. }], 0.));
    for body in [
        vec![],
        vec![Point::default()],
        vec![Point::default(); 431],
        vec![Point::default(), Point { x: f64::NAN, y: 0. }],
        vec![
            Point::default(),
            Point {
                x: 0.,
                y: f64::INFINITY,
            },
        ],
    ] {
        assert!(!world.body_is_clear(&body, 0.));
    }
    for clearance in [-1., f64::NAN, f64::INFINITY] {
        assert!(!world.body_is_clear(&clear, clearance));
    }
}

#[test]
fn supplied_body_respawn_rejects_invalid_input_without_mutation() {
    let mut world = empty_world();
    support::ring_slot(&mut world, "player", 0, 1).unwrap();
    let body = vec![Point::default(), Point { x: -42., y: 0. }];
    let before = snapshot(&world);
    assert!(world.respawn_body(0, body.clone(), 0.).is_err());
    assert_eq!(snapshot(&world), before);
    world.retire(0).unwrap();
    let before = snapshot(&world);
    assert!(world.respawn_body(1, body.clone(), 0.).is_err());
    for heading in [f64::NAN, f64::INFINITY] {
        assert!(world.respawn_body(0, body.clone(), heading).is_err());
        assert_eq!(snapshot(&world), before);
    }
    for body in [
        vec![],
        vec![Point::default()],
        vec![Point::default(); 431],
        vec![Point::default(), Point { x: f64::NAN, y: 0. }],
        vec![
            Point::default(),
            Point {
                x: 0.,
                y: f64::INFINITY,
            },
        ],
        vec![
            Point::default(),
            Point {
                x: world.radius(),
                y: 0.,
            },
        ],
    ] {
        assert!(world.respawn_body(0, body, 0.).is_err());
        assert_eq!(snapshot(&world), before);
    }
}

#[test]
fn supplied_body_respawn_resets_motion_penalty_death_and_mass_state() {
    let config = Config {
        kiosk: KioskConfig {
            grace_seconds: 0.014,
            shrink_per_second: 2.,
            ..Default::default()
        },
        ..empty_world().config().clone()
    };
    let mut world = Simulator::new(slither_sim::WorldOptions {
        seed: 7,
        config: config.clone(),
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["player".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    world.retire(0).unwrap();
    world.respawn(0, Point::default(), 0., 380).unwrap();
    {
        let mut snake = world.snakes()[0].clone();
        snake.kills = 13;
        world
            .import_life(
                0,
                slither_sim::ReplayLife::from_observed(
                    snake.clone(),
                    slither_sim::InitialSpeed::Infer(snake.speed),
                ),
            )
            .unwrap();
    }
    let action = [Some(Action {
        heading: 0.,
        boost: true,
    })];
    for _ in 0..8 {
        world.step(&action).unwrap();
    }
    assert!(world.boost_accounting(0).unwrap().active_boost_seconds > 0.);

    {
        let mut snake = world.snakes()[0].clone();
        snake.head.x = world.radius() * 2.;
        world
            .correct_pose(0, snake.head, snake.body, snake.heading)
            .unwrap();
    }
    world.step(&action).unwrap();
    assert!(world.death_time(0).is_some());
    let deaths = world.deaths().len();
    let body = vec![
        Point::default(),
        Point { x: -42., y: 0. },
        Point { x: -42., y: -42. },
        Point { x: -84., y: -42. },
    ];
    world.respawn_body(0, body.clone(), 0.).unwrap();
    assert_eq!(world.snakes()[0].body, body);
    assert_eq!(world.snakes()[0].head, body[0]);
    assert_eq!(world.snakes()[0].segments, 4);
    assert_eq!(world.snakes()[0].fam, 0.);
    assert_eq!(world.snakes()[0].kills, 0);
    assert_eq!(world.snakes()[0].speed, physics::base_speed(4));
    assert_eq!(world.snakes()[0].score, physics::score(4, 0.));
    assert_eq!(world.death_time(0), None);
    assert_eq!(world.deaths().len(), deaths);
    assert!(!world.is_boosting(0));
    let accounting = world.boost_accounting(0).unwrap();
    assert_eq!(accounting.active_boost_seconds, 0.);
    assert_eq!(accounting.boost_mass_spent, 0.);

    let mut fresh = Simulator::new(slither_sim::WorldOptions {
        seed: 7,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: vec!["player".into()],
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap();
    fresh.retire(0).unwrap();
    fresh.respawn_body(0, body, 0.).unwrap();
    for _ in 0..10 {
        let action = [Some(Action::default())];
        world.step(&action).unwrap();
        fresh.step(&action).unwrap();
        assert_eq!(world.snakes(), fresh.snakes());
    }
}

#[test]
fn supplied_body_respawn_assigns_new_join_order_and_matches_straight_respawn() {
    let mut world = empty_world();
    for i in 0..2 {
        support::ring_slot(&mut world, i.to_string(), i, 2).unwrap();
        world.retire(i).unwrap();
    }
    let body = vec![Point::default(), Point { x: -42., y: 0. }];
    world.respawn_body(0, body.clone(), 0.).unwrap();
    world
        .respawn_body(
            1,
            vec![Point { x: 60., y: 1. }, Point { x: 102., y: 1. }],
            std::f64::consts::PI,
        )
        .unwrap();
    world.retire(0).unwrap();
    let mut straight = world.clone();
    straight.respawn(0, Point::default(), 0., 2).unwrap();
    world.respawn_body(0, body, 0.).unwrap();
    assert_eq!(snapshot(&world), snapshot(&straight));
    let actions = [
        Some(Action::default()),
        Some(Action {
            heading: std::f64::consts::PI,
            boost: false,
        }),
    ];
    for _ in 0..30 {
        world.step(&actions).unwrap();
        if !world.deaths().is_empty() {
            break;
        }
    }
    assert!(world.snakes()[0].alive);
    assert!(!world.snakes()[1].alive);
}

fn refill_world(rate: f64) -> Simulator {
    let mut world = empty_world();
    {
        let mut config = world.config().clone();
        config.dt = 0.01;

        config.food_count = 100;

        config.ambient_food.enabled = false;
        support::reconfigure(&mut world, config);
    }
    world.set_ambient_rate(rate).unwrap();
    world
}

fn tick(world: &mut Simulator, count: usize) {
    for _ in 0..count {
        world.step(&[]).unwrap();
    }
}

#[test]
fn changing_refill_rate_preserves_fractional_credit_in_both_directions() {
    let mut faster = refill_world(10.);
    tick(&mut faster, 5);
    faster.set_ambient_rate(20.).unwrap();
    tick(&mut faster, 2);
    assert!(faster.food().is_empty());
    tick(&mut faster, 1);
    assert_eq!(faster.food().len(), 1);

    let mut slower = refill_world(20.);
    tick(&mut slower, 3);
    slower.set_ambient_rate(5.).unwrap();
    tick(&mut slower, 7);
    assert!(slower.food().is_empty());
    tick(&mut slower, 1);
    assert_eq!(slower.food().len(), 1);
}

#[test]
fn refill_setter_rejections_same_rate_and_zero_have_defined_state_effects() {
    let mut world = refill_world(10.);
    tick(&mut world, 5);
    let before = snapshot(&world);
    // 1e300 attempts/s would subtract intervals too small to change the clock.
    for rate in [-1., f64::NAN, f64::INFINITY, 1e300, f64::from_bits(1)] {
        assert!(world.set_ambient_rate(rate).is_err());
        assert_eq!(snapshot(&world), before);
    }
    let mut fast = world.clone();
    fast.set_ambient_rate(1e6).unwrap();
    tick(&mut fast, 1);
    world.set_ambient_rate(10.).unwrap();
    assert_eq!(snapshot(&world), before);
    world.set_ambient_rate(0.).unwrap();
    tick(&mut world, 100);
    assert!(world.food().is_empty());
    world.set_ambient_rate(20.).unwrap();
    tick(&mut world, 4);
    assert!(world.food().is_empty());
    tick(&mut world, 1);
    assert_eq!(world.food().len(), 1);

    let mut untouched = refill_world(7.);
    let mut repeated = untouched.clone();
    for _ in 0..100 {
        repeated.set_ambient_rate(7.).unwrap();
        tick(&mut untouched, 1);
        tick(&mut repeated, 1);
    }
    assert_eq!(snapshot(&repeated), snapshot(&untouched));
}
