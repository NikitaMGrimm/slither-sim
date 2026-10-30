use super::*;

fn linear(head: Point, body: &[Point]) -> Bounds {
    let mut bounds = Bounds::between(head, head);
    for &point in body {
        bounds.include(point);
    }
    bounds
}

#[test]
fn anchor_extrema_preserve_linear_bounds_verdicts() {
    let special = [
        -f64::MAX,
        -512.,
        -0.,
        0.,
        f64::MIN_POSITIVE,
        512.,
        f64::MAX,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NAN,
    ];
    let mut random = Random(7);
    for case in 0..10_000 {
        let mut point = || Point {
            x: (random.next() - 0.5) * 1e6,
            y: (random.next() - 0.5) * 1e6,
        };
        let mut head = point();
        let mut body: Vec<_> = (0..case % 65).map(|_| point()).collect();
        if case % 3 == 0 {
            head.x = special[case / 3 % special.len()];
            for (i, p) in body.iter_mut().enumerate() {
                p.y = special[(case + i) % special.len()];
            }
        }
        let expected = linear(head, &body);
        let mut actual = Bounds::between(head, head);
        if let Some(cached) = snake_body_bounds(&body) {
            actual.include(cached.min);
            actual.include(cached.max);
        }
        for (a, b) in [
            (actual.min.x, expected.min.x),
            (actual.min.y, expected.min.y),
            (actual.max.x, expected.max.x),
            (actual.max.y, expected.max.y),
        ] {
            // Zero signs and NaN payloads do not affect bounds comparisons or
            // cell numbering; every ordered value must match exactly.
            assert!(a == b || (a.is_nan() && b.is_nan()));
        }
        for query in [
            Bounds::between(head, head),
            Bounds::between(Point::default(), Point { x: 42., y: -42. }),
            expected,
        ] {
            for reach in [
                0.,
                -0.,
                29.,
                29_f64.next_up(),
                1e300,
                f64::INFINITY,
                f64::NAN,
            ] {
                assert_eq!(actual.near(query, reach), expected.near(query, reach));
                assert_eq!(query.near(actual, reach), query.near(expected, reach));
            }
        }
    }
}

fn world(seats: usize) -> Simulator {
    let mut world = Simulator::new(WorldOptions {
        seed: 7,
        config: Config {
            radius: 1e6,
            food_count: 0,
            ambient_replenish_per_second: 0.,
            prey: PreyConfig {
                count: 0,
                ..PreyConfig::default()
            },
            ..Config::sandbox()
        },
        population: InitialPopulation::Empty,
    })
    .unwrap();
    for i in 0..seats {
        let x = i as f64 * 2000.;
        let body = (0..4)
            .map(|k| Point {
                x: x - k as f64 * 42.,
                y: 0.,
            })
            .collect();
        world.spawn(i.to_string(), body, 0.).unwrap();
    }
    world.record_events(checkpoint::EventRecording {
        pickups: true,
        food_births: true,
    });
    world
}

fn actions(world: &Simulator, boost: bool) -> Vec<Option<Action>> {
    world
        .snakes
        .iter()
        .map(|s| s.alive.then_some(Action { heading: 0., boost }))
        .collect()
}

fn equal(warm: &Simulator, cold: &Simulator) {
    assert_eq!(
        serde_json::to_vec(&warm.checkpoint().unwrap()).unwrap(),
        serde_json::to_vec(&cold.checkpoint().unwrap()).unwrap()
    );
    assert_eq!(warm.pickup_events(), cold.pickup_events());
    assert_eq!(warm.food_spawn_events(), cold.food_spawn_events());
    for (snake, bounds) in warm.snakes.iter().zip(&warm.buffers.bodies) {
        if snake.alive {
            let expected = linear(snake.head, &snake.body);
            let actual = bounds.unwrap();
            assert_eq!(actual.min, expected.min);
            assert_eq!(actual.max, expected.max);
        }
    }
}

#[test]
fn public_geometry_edits_invalidate_warm_collision_bounds() {
    for seats in [2, 40] {
        for edit in 0..4 {
            let mut warm = world(seats);
            warm.step(&actions(&warm, false)).unwrap();
            assert!(warm.buffers.body_bounds[1].is_some());
            let mut cold = warm.clone();
            assert!(cold.buffers.body_bounds.is_empty());
            for world in [&mut warm, &mut cold] {
                let head = world.snakes[1].head;
                // The first anchor stays close to the head, so movement cannot
                // hide a missed invalidation by inserting a new anchor.
                let mut body = vec![head, Point { x: 25., y: 100. }, Point { x: 25., y: -100. }];
                match edit {
                    0 => {
                        body.insert(1, head);
                        world.correct_pose(1, head, body, 0.).unwrap();
                    }
                    1 => world.correct_mass(1, body, 0.).unwrap(),
                    2 => world
                        .import_life(
                            1,
                            ReplayLife {
                                head,
                                body,
                                heading: 0.,
                                fam: 0.,
                                kills: 0,
                                alive: true,
                                speed: InitialSpeed::Known(SpeedState::idle(base_speed(3))),
                            },
                        )
                        .unwrap(),
                    _ => {
                        world.retire(1).unwrap();
                        world.respawn_body(1, body, 0.).unwrap();
                    }
                }
            }
            let input = actions(&warm, false);
            warm.step(&input).unwrap();
            cold.step(&input).unwrap();
            equal(&warm, &cold);
            assert_eq!(warm.deaths.last().unwrap().victim, "0");
            assert_eq!(warm.deaths.last().unwrap().killer.as_deref(), Some("1"));
        }
    }
}

#[test]
fn movement_truncation_and_new_slots_match_cold_rebuilds() {
    let mut warm = world(40);
    for tick in 0..240 {
        if tick == 60 {
            warm.append_dormant("new".into()).unwrap();
            warm.respawn(40, Point { x: 90000., y: 0. }, 0., 4).unwrap();
        }
        let mut cold = warm.clone();
        let input = actions(&warm, tick % 6 != 0);
        warm.step(&input).unwrap();
        cold.step(&input).unwrap();
        equal(&warm, &cold);
    }
}

#[test]
fn kiosk_tail_removal_invalidates_cached_anchor_extrema() {
    let mut warm = world(2);
    warm.config.kiosk.enabled = true;
    warm.config.kiosk.floor_score = 1.;
    warm.config.kiosk.grace_seconds = 0.02;
    warm.config.kiosk.shrink_per_second = 100.;
    let mut shrank = false;
    for _ in 0..12 {
        let mut cold = warm.clone();
        let input = actions(&warm, false);
        let old_len = warm.snakes[0].body.len();
        warm.step(&input).unwrap();
        cold.step(&input).unwrap();
        shrank |= warm.snakes[0].body.len() < old_len;
        equal(&warm, &cold);
    }
    assert!(shrank, "exercise the kiosk's body truncation");
}
