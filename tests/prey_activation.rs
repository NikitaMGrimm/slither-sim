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
    Simulator::new(
        42,
        Config {
            food_count: 0,
            ..Config::default()
        },
    )
    .unwrap()
}

fn add_snake(w: &mut Simulator, head: Point, boost: bool) {
    let index = w.add(format!("threat{}", w.snakes.len()), 0, 1).unwrap();
    let snake = &mut w.snakes[index];
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
        snake.fam = 0.5;
        Simulator::update_mass(snake);
        snake.speed = physics::BOOST_SPEED;
    }
}

#[test]
fn diagonal_prey_activates_inside_the_recorded_square() {
    let mut w = world();
    add_snake(&mut w, Point::default(), false);
    w.prey.push(prey(Point { x: 280., y: 280. }));
    w.step(&[Some(Action {
        heading: std::f64::consts::PI,
        boost: false,
    })])
    .unwrap();
    assert_eq!(w.prey[0].speed, w.config.prey.escape_speed);
}

#[test]
fn nearer_snake_outside_square_does_not_hide_an_eligible_diagonal_threat() {
    let mut w = world();
    add_snake(&mut w, Point { x: -330., y: 0. }, false);
    add_snake(&mut w, Point { x: -290., y: -290. }, true);
    w.prey.push(prey(Point::default()));
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
    assert_eq!(w.prey[0].speed, w.config.prey.sprint_speed);
}

#[test]
fn axial_prey_beyond_square_stays_at_rest() {
    let mut w = world();
    add_snake(&mut w, Point::default(), false);
    w.prey.push(prey(Point { x: 340., y: 0. }));
    w.step(&[Some(Action {
        heading: std::f64::consts::PI,
        boost: false,
    })])
    .unwrap();
    assert_eq!(w.prey[0].speed, w.config.prey.cruise_speed);
}
