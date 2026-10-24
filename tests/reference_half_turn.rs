use slither_sim::{Action, Config, Food, Point, Simulator, reference};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

fn scene(rotation: f64, heading_offset: f64) -> Simulator {
    let rotate = |point: Point| Point {
        x: point.x * rotation.cos() - point.y * rotation.sin(),
        y: point.x * rotation.sin() + point.y * rotation.cos(),
    };
    let mut world = Simulator::new(1, Config::sandbox()).unwrap();
    world.add("self", 0, 2).unwrap();
    world.add("other", 1, 2).unwrap();
    for (slot, body, heading) in [
        (
            0,
            vec![Point::default(), Point { x: -42.0, y: 0.0 }],
            rotation + heading_offset,
        ),
        (
            1,
            vec![Point { x: 50.0, y: 40.0 }, Point { x: 50.0, y: 82.0 }],
            rotation - FRAC_PI_2,
        ),
    ] {
        world.snakes[slot].alive = false;
        world
            .respawn_body(slot, body.into_iter().map(rotate).collect(), heading)
            .unwrap();
    }
    world.snakes[1].speed = 0.0;
    world.initialize_speed_state(1, None).unwrap();
    let target = rotate(Point { x: -200.0, y: 70.0 });
    world.food = vec![Food {
        x: target.x,
        y: target.y,
        value: 1.0,
        size: None,
        origin: None,
    }];
    world.prey.clear();
    world
}

fn angles() -> impl Iterator<Item = (f64, f64)> {
    // Straddle heading-bin boundaries near the half-turn, and rotate the same
    // geometry across axes and the 0/2π seam. No libm last-bit tie is assumed.
    [0.0, 0.2, FRAC_PI_2, PI, -FRAC_PI_2, TAU + 0.4]
        .into_iter()
        .flat_map(|rotation| {
            [-0.51, -0.49, 0.0, 0.49, 0.51]
                .into_iter()
                .map(move |offset| (rotation, offset * TAU / 251.0))
        })
}

#[test]
fn half_turn_hazard_is_avoided_by_the_executed_policy() {
    for (rotation, heading_offset) in angles() {
        let mut world = scene(rotation, heading_offset);
        let mut action = Action::default();
        for tick in 0..52 {
            if tick % 6 == 0 && world.snakes[0].alive {
                action = reference::decide(&world.observe(0).unwrap());
            }
            let actions: Vec<_> = world
                .snakes
                .iter()
                .enumerate()
                .map(|(slot, snake)| {
                    snake.alive.then_some(if slot == 0 {
                        action
                    } else {
                        Action {
                            heading: snake.heading,
                            boost: false,
                        }
                    })
                })
                .collect();
            world.step(&actions).unwrap();
        }
        assert!(
            world.snakes[0].alive,
            "rotation={rotation} offset={heading_offset} death={:?}",
            world.death_time(0)
        );
    }
}

#[cfg(feature = "networking")]
#[test]
fn selected_half_turn_escape_matches_the_encoded_command_and_physics() {
    use slither_sim::physics;

    for (rotation, heading_offset) in angles() {
        let mut world = scene(rotation, heading_offset);
        let me = &world.snakes[0];
        let action = reference::decide(&world.observe(0).unwrap());
        let packets = slither_sim::client::protocol::action_packets(action, Some(false)).unwrap();
        assert_eq!(packets.len(), 1);
        let encoded_target = f64::from(packets[0][0]) * TAU / 251.0;
        let (expected_position, expected_heading) = physics::advance(
            me.head,
            me.heading,
            encoded_target,
            me.speed,
            physics::turn_rate(me.segments),
            world.config.dt,
        );
        let heading_before = me.heading;
        world
            .step(&[
                Some(action),
                Some(Action {
                    heading: world.snakes[1].heading,
                    boost: false,
                }),
            ])
            .unwrap();
        assert!(world.snakes[0].head.distance(expected_position) < 1e-9);
        assert!((world.snakes[0].heading - expected_heading).abs() < 1e-9);
        assert!(
            expected_heading < heading_before,
            "executed turn approaches the upper-side hazard: rotation={rotation} offset={heading_offset} action={action:?}"
        );
    }
}
