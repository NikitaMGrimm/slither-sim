mod support;
use slither_sim::*;

fn prey(position: Point) -> Prey {
    Prey {
        id: 100,
        position,
        heading: 0.,
        target_heading: 0.,
        speed: 3.8 / 0.032,
        size: 6.,
    }
}

fn world() -> Simulator {
    Simulator::new(slither_sim::WorldOptions {
        seed: 42,
        config: Config {
            food_count: 0,
            ..Config::sandbox()
        },
        population: slither_sim::InitialPopulation::Empty,
    })
    .unwrap()
}

fn add_snake(w: &mut Simulator, head: Point, boost: bool) {
    let id = format!("threat{}", w.snakes().len());
    let index = support::ring_slot(w, id, 0, 1).unwrap();
    let mut detached_snake = w.snakes()[index].clone();
    let snake = &mut detached_snake;
    snake.head = head;
    snake.heading = std::f64::consts::PI;
    snake.body = vec![
        head,
        Point {
            x: head.x + 42.,
            y: head.y,
        },
    ];
    if boost {
        snake.segments = 10;
        snake.body.resize(10, *snake.body.last().unwrap());
        snake.fam = 0.5;
        snake.speed = physics::BOOST_SPEED;
    }
    w.import_life(
        index,
        slither_sim::ReplayLife::from_observed(
            snake.clone(),
            slither_sim::InitialSpeed::Infer(snake.speed),
        ),
    )
    .unwrap();
}

#[test]
fn diagonal_prey_activates_inside_the_recorded_square() {
    let mut w = world();
    add_snake(&mut w, Point::default(), false);
    w.insert_prey(slither_sim::ReplayPrey {
        prey: prey(Point { x: 280., y: 280. }),
        mode: slither_sim::PreyMode::Roaming,
    })
    .unwrap();
    w.step(&[Some(Action {
        heading: std::f64::consts::PI,
        boost: false,
    })])
    .unwrap();
    assert_eq!(w.prey()[0].speed, w.config().prey.escape_speed);
}

#[test]
fn nearer_snake_outside_square_does_not_hide_an_eligible_diagonal_threat() {
    let mut w = world();
    add_snake(&mut w, Point { x: -330., y: 0. }, false);
    add_snake(&mut w, Point { x: -290., y: -290. }, true);
    w.insert_prey(slither_sim::ReplayPrey {
        prey: prey(Point::default()),
        mode: slither_sim::PreyMode::Roaming,
    })
    .unwrap();
    w.step(&[
        Some(Action {
            heading: std::f64::consts::PI,
            boost: false,
        }),
        Some(Action {
            heading: std::f64::consts::PI,
            boost: true,
        }),
    ])
    .unwrap();
    assert_eq!(w.prey()[0].speed, w.config().prey.sprint_speed);
}

#[test]
fn axial_prey_beyond_square_stays_at_rest() {
    let mut w = world();
    add_snake(&mut w, Point::default(), false);
    w.insert_prey(slither_sim::ReplayPrey {
        prey: prey(Point { x: 340., y: 0. }),
        mode: slither_sim::PreyMode::Roaming,
    })
    .unwrap();
    w.step(&[Some(Action {
        heading: std::f64::consts::PI,
        boost: false,
    })])
    .unwrap();
    assert_eq!(w.prey()[0].speed, w.config().prey.cruise_speed);
}
