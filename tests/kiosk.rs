use slither_sim::{physics::*, *};

fn world() -> Simulator {
    let mut world = Simulator::new(
        7,
        Config {
            radius: 1_000_000.0,
            food_count: 0,
            ambient_replenish_per_second: 0.0,
            dt: 0.032,
            kiosk: KioskConfig {
                grace_seconds: 0.064,
                stationary_radius: 100_000.0,
                shrink_per_second: 2.0,
                score_decay_per_second: 0.5,
                recovery_per_second: 2.0,
                ..KioskConfig::default()
            },
            ..Config::sandbox()
        },
    )
    .unwrap();
    world.add("player", 0, 1).unwrap();
    let snake = &mut world.snakes[0];
    snake.head = Point::default();
    snake.body = (0..380)
        .map(|i| Point {
            x: -42.0 * i as f64,
            y: 0.0,
        })
        .collect();
    snake.heading = 0.0;
    snake.speed = base_speed(380);
    snake.segments = 380;
    snake.fam = 0.0;
    Simulator::update_mass(snake);
    world
}

fn step(world: &mut Simulator, count: usize) {
    for _ in 0..count {
        world.step(&[Some(Action::default())]).unwrap();
        assert!(world.snakes[0].alive);
    }
}

#[test]
fn kiosk_grace_shrink_score_decay_and_recovery_are_distinct() {
    let mut world = world();
    let original_score = world.snakes[0].score;
    step(&mut world, 1);
    assert_eq!(world.snakes[0].score, original_score);
    assert_eq!(world.snakes[0].segments, 380);

    step(&mut world, 12);
    assert!(world.snakes[0].segments < 380);
    assert_eq!(world.snakes[0].body.len(), world.snakes[0].segments);
    assert_eq!(world.snakes[0].score, original_score);

    step(&mut world, 45);
    let physical_score = score(world.snakes[0].segments, world.snakes[0].fam);
    assert!((physical_score - 8_500.0).abs() < 5.0);
    assert!(world.snakes[0].score < original_score);
    assert!(world.snakes[0].score > physical_score);
    let remaining_score = world.snakes[0].score;
    let short_segments = world.snakes[0].segments;

    world.config.kiosk.stationary_radius = 0.01;
    step(&mut world, 1_500);
    assert!(world.snakes[0].segments > short_segments);
    assert_eq!(world.snakes[0].body.len(), world.snakes[0].segments);
    assert_eq!(world.snakes[0].score, remaining_score);
    assert!(score(world.snakes[0].segments, world.snakes[0].fam) <= remaining_score);
}

#[test]
fn ordinary_boost_still_costs_score_and_emits_only_its_own_pellets() {
    let mut world = world();
    step(&mut world, 13);
    let before = world.snakes[0].score;
    let food_before = world.food.len();
    world
        .step(&[Some(Action {
            heading: 0.0,
            boost: true,
        })])
        .unwrap();
    assert!(world.snakes[0].score < before);
    assert_eq!(world.food.len(), food_before + 1);
    assert!(
        world
            .food
            .iter()
            .all(|f| matches!(f.origin, Some(Origin::Boost)))
    );
}

#[test]
fn disabled_penalty_keeps_long_stationary_score_and_size_equal() {
    let mut world = world();
    world.config.kiosk.enabled = false;
    step(&mut world, 60);
    assert_eq!(world.snakes[0].segments, 380);
    assert_eq!(world.snakes[0].score, score(380, 0.0));
}

#[test]
fn disabling_active_penalty_restores_physical_size_without_refunding_score() {
    let mut world = world();
    step(&mut world, 13);
    let size_before = world.snakes[0].segments;
    let score_before = world.snakes[0].score;
    world.config.kiosk.enabled = false;
    step(&mut world, 1_500);
    assert!(world.snakes[0].segments > size_before);
    assert_eq!(world.snakes[0].score, score_before);
}

#[test]
fn kiosk_physical_decay_is_stable_across_step_sizes() {
    let mut fine = world();
    let mut coarse = world();
    fine.config.dt = 0.008;
    coarse.config.dt = 0.016;
    fine.config.kiosk.grace_seconds = 0.0;
    coarse.config.kiosk.grace_seconds = 0.0;
    fine.config.kiosk.shrink_per_second = 0.7;
    coarse.config.kiosk.shrink_per_second = 0.7;
    step(&mut fine, 125);
    step(&mut coarse, 63);
    assert!(
        (score(fine.snakes[0].segments, fine.snakes[0].fam)
            - score(coarse.snakes[0].segments, coarse.snakes[0].fam))
        .abs()
            < 200.0
    );
    assert_eq!(fine.snakes[0].score, coarse.snakes[0].score);
}

#[test]
fn respawn_discards_kiosk_credit_and_old_config_remains_readable() {
    let mut world = world();
    step(&mut world, 13);
    assert!(world.snakes[0].score > score(world.snakes[0].segments, world.snakes[0].fam));
    world.snakes[0].alive = false;
    world.respawn(0, Point::default(), 0.0, 2).unwrap();
    assert_eq!(world.snakes[0].score, score(2, 0.0));
    world.config.kiosk.enabled = false;
    step(&mut world, 3);
    assert_eq!(world.snakes[0].score, score(2, 0.0));

    let mut old: serde_json::Value = serde_json::to_value(Config::sandbox()).unwrap();
    old.as_object_mut().unwrap().remove("kiosk");
    let restored: Config = serde_json::from_value(old).unwrap();
    assert!(restored.kiosk.enabled);
}

#[test]
fn death_food_uses_penalized_physical_body() {
    let mut world = world();
    step(&mut world, 13);
    let short_body = world.snakes[0].body.len();
    assert!(short_body < 380);
    assert!(world.snakes[0].score > score(world.snakes[0].segments, world.snakes[0].fam));

    world.add("obstacle", 0, 2).unwrap();
    // A long body crosses just ahead of the player's tip; its head moves away.
    let player = &world.snakes[0];
    let x = player.head.x + BODY_RADIUS * scale(player.segments) + 10.0;
    let y = player.head.y;
    let obstacle = &mut world.snakes[1];
    obstacle.head = Point { x, y: y + 342.0 };
    obstacle.heading = std::f64::consts::FRAC_PI_2;
    obstacle.body = (0..11)
        .map(|i| Point {
            x,
            y: y + 300.0 - 42.0 * i as f64,
        })
        .collect();
    obstacle.segments = obstacle.body.len();
    Simulator::update_mass(obstacle);
    world
        .step(&[Some(Action::default()), Some(Action::default())])
        .unwrap();
    let event = world.deaths.iter().find(|d| d.victim == "player").unwrap();
    assert_eq!(event.food, 2 * world.snakes[0].body.len() + 1);
    assert!(event.food <= 2 * short_body + 1);
}
