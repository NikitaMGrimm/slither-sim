#[cfg(test)]
mod tests {
    use crate::{model::*, prey::Prey, simulator::Simulator};

    #[test]
    fn arena_observations_include_every_living_opponent_food_and_prey() {
        let mut rng = Rng(7);
        let mut world = world(&mut rng, 16, 100, 10.);
        world.config.vision = crate::Vision::Arena;
        for (i, snake) in world.snakes.iter_mut().enumerate() {
            snake.head = Point {
                x: if i == 0 { -900. } else { 900. },
                y: 0.,
            };
            snake.body = vec![snake.head];
        }
        world.food.retain(|f| f.x.is_finite() && f.y.is_finite());
        world
            .prey
            .retain(|p| p.position.x.is_finite() && p.position.y.is_finite());
        for radius in [1000., 3000., 1000.] {
            world.radius = radius;
            let all = world.observe_all();
            for (i, slot) in all.iter().enumerate() {
                let one = world.observe(i).unwrap();
                assert_eq!(one.vision_radius, Some(2. * radius));
                assert_eq!(one.food, world.food);
                assert_eq!(one.prey, world.prey);
                assert_eq!(
                    one.opponents,
                    world
                        .snakes
                        .iter()
                        .enumerate()
                        .filter(|(j, s)| *j != i && s.alive)
                        .map(|(_, s)| s.clone())
                        .collect::<Vec<_>>()
                );
                if world.snakes[i].alive {
                    assert_eq!(Some(&one), slot.as_ref());
                }
            }
        }
    }

    struct Rng(u64);
    impl Rng {
        fn unit(&mut self) -> f64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
        fn below(&mut self, n: usize) -> usize {
            (self.unit() * n as f64) as usize
        }
        fn pick(&mut self, values: &[f64]) -> f64 {
            values[self.below(values.len())]
        }
    }

    const ODD: [f64; 9] = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1e20,
        -1e300,
        (1u64 << 40) as f64,
        (1u64 << 40) as f64 + 4096.0,
        -0.0,
        f64::MAX,
    ];

    /// Mostly ordinary points near the arena, sometimes outside it or degenerate.
    fn point(rng: &mut Rng, spread: f64, near: Point, vision: f64) -> Point {
        match rng.below(12) {
            0 => Point {
                x: rng.pick(&ODD),
                y: (rng.unit() - 0.5) * spread,
            },
            1 => Point {
                x: (rng.unit() - 0.5) * spread,
                y: rng.pick(&ODD),
            },
            // On or a few ulps around the vision circle and box edges.
            2 | 3 => {
                let a = rng.unit() * std::f64::consts::TAU;
                let r = f64::from_bits(
                    vision
                        .abs()
                        .to_bits()
                        .wrapping_add(rng.below(5) as u64)
                        .wrapping_sub(2),
                );
                let (s, c) = if rng.below(2) == 0 {
                    a.sin_cos()
                } else {
                    (0.0, 1.0)
                };
                Point {
                    x: near.x + c * r,
                    y: near.y + s * r,
                }
            }
            4 => near,
            _ => Point {
                x: (rng.unit() - 0.5) * spread,
                y: (rng.unit() - 0.5) * spread,
            },
        }
    }

    fn world(rng: &mut Rng, snakes: usize, food: usize, vision: f64) -> Simulator {
        let mut world = Simulator::new(crate::WorldOptions {
            seed: 7,
            config: crate::Config::sandbox(),
            ..Default::default()
        })
        .unwrap();
        world.config.vision = crate::Vision::Radius(vision);
        let spread = rng.pick(&[10.0, 3000.0, 20_000.0]);
        let center = Point::default();
        let heads: Vec<_> = (0..snakes)
            .map(|_| point(rng, spread, center, vision))
            .collect();
        // Bodies cross the vision edges of other heads, not only their own.
        world.snakes = (0..snakes)
            .map(|i| {
                let anchors = rng.below(40);
                Snake {
                    id: i.to_string(),
                    head: heads[i],
                    body: (0..anchors)
                        .map(|_| {
                            let near = heads[rng.below(snakes)];
                            point(rng, spread, near, vision)
                        })
                        .collect(),
                    heading: rng.unit(),
                    speed: 185.0,
                    radius: 10.0,
                    length: anchors as f64,
                    segments: anchors,
                    fam: 0.0,
                    score: 10.0,
                    kills: 0,
                    alive: rng.below(5) != 0,
                }
            })
            .collect();
        world.food = (0..food)
            .map(|_| {
                let near = match world.snakes.len() {
                    0 => center,
                    n => world.snakes[rng.below(n)].head,
                };
                let p = point(rng, spread, near, vision);
                Food {
                    x: p.x,
                    y: p.y,
                    value: rng.unit(),
                    size: (rng.below(2) == 0).then_some(rng.unit()),
                    origin: (rng.below(2) == 0).then_some(Origin::Death),
                }
            })
            .collect();
        world.prey = (0..rng.below(6))
            .map(|id| Prey {
                id: id as u64,
                position: point(rng, spread, center, vision),
                heading: 0.0,
                target_heading: 0.0,
                speed: 1.0,
                size: 1.0,
            })
            .collect();
        world
    }

    /// Debug output keeps NaN, signed zeros and exact float digits distinct.
    fn key(observations: &[Option<Observation>]) -> Vec<String> {
        observations.iter().map(|o| format!("{o:?}")).collect()
    }

    #[test]
    fn batch_observations_match_single_slot_observations() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let visions = [
            1500.0,
            1500.0,
            300.0,
            0.0,
            -0.0,
            1e-9,
            0.5,
            1e5,
            (1u64 << 40) as f64,
            1e13,
            f64::INFINITY,
            f64::NAN,
            -1.0,
        ];
        // Reused across worlds of every shape, so refills meet longer and shorter
        // vectors, other slot counts and other snakes.
        let mut buffer = Vec::new();
        for case in 0..600 {
            let vision = visions[case % visions.len()];
            let snakes = [0, 1, 2, 3, 4, 5, 9, 30][rng.below(8)];
            let food = [0, 1, 255, 256, 700, 3000][rng.below(6)];
            let world = world(&mut rng, snakes, food, vision);
            let single: Vec<_> = (0..snakes)
                .map(|i| world.snakes[i].alive.then(|| world.observe(i).unwrap()))
                .collect();
            assert_eq!(key(&world.observe_all()), key(&single), "case {case}");
            world.observe_all_into(&mut buffer);
            assert_eq!(key(&buffer), key(&single), "case {case}");
            let mask: Vec<bool> = (0..snakes).map(|_| rng.below(3) != 0).collect();
            let selected: Vec<_> = single
                .iter()
                .zip(&mask)
                .map(|(o, &m)| o.clone().filter(|_| m))
                .collect();
            assert_eq!(
                key(&world.observe_selected(&mask).unwrap()),
                key(&selected),
                "case {case}"
            );
            let before = key(&buffer);
            assert!(
                world
                    .observe_selected_into(&[true; 31], &mut buffer)
                    .is_err()
            );
            assert_eq!(key(&buffer), before, "case {case}");
            world.observe_selected_into(&mask, &mut buffer).unwrap();
            assert_eq!(key(&buffer), key(&selected), "case {case}");
        }
    }

    /// Rows of the food grid are trimmed to a chord of the vision circle, which is
    /// narrowest near the top and bottom of the circle.
    #[test]
    fn batches_keep_food_on_the_vision_edge_of_trimmed_rows() {
        let mut rng = Rng(0x853c_49e6_748f_ea9b);
        let ulps = |v: f64, n: i64| f64::from_bits((v.to_bits() as i64 + n) as u64);
        for case in 0..60 {
            let vision = [1500.0, 300.0, 1.0, 0.5, 1e5, (1u64 << 40) as f64][case % 6];
            let offset = [0.0, 3e4, (1u64 << 39) as f64][rng.below(3)];
            let mut world = world(&mut rng, 16, 0, vision);
            for (i, snake) in world.snakes.iter_mut().enumerate() {
                snake.alive = true;
                snake.head = Point {
                    x: offset + (i % 4) as f64 * vision * 0.7 + rng.unit(),
                    y: -offset + (i / 4) as f64 * vision * 0.7 + rng.unit(),
                };
            }
            let heads: Vec<Point> = world.snakes.iter().map(|s| s.head).collect();
            world.food = (0..4096)
                .map(|_| {
                    let h = heads[rng.below(heads.len())];
                    // Near the poles of the circle, and exactly on the vertical axis.
                    let a = std::f64::consts::FRAC_PI_2 * [1.0, -1.0][rng.below(2)]
                        + (rng.unit() - 0.5) * [0.0, 1e-6, 1e-3, 0.2][rng.below(4)];
                    let r = ulps(vision, rng.below(9) as i64 - 4);
                    let (sin, cos) = a.sin_cos();
                    let x = ulps(h.x + cos * r, rng.below(5) as i64 - 2);
                    let y = ulps(h.y + sin * r, rng.below(5) as i64 - 2);
                    Food {
                        x,
                        y,
                        value: rng.unit(),
                        size: None,
                        origin: None,
                    }
                })
                .collect();
            let single: Vec<_> = (0..16).map(|i| Some(world.observe(i).unwrap())).collect();
            let visible: usize = single.iter().flatten().map(|o| o.food.len()).sum();
            assert!(visible > 0, "case {case}");
            assert_eq!(key(&world.observe_all()), key(&single), "case {case}");
        }
    }

    #[cfg(feature = "parallel")]
    #[test]
    fn parallel_batches_match_sequential_batches() {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let mut rng = Rng(0x2545_f491_4f6c_dd1d);
        let mut buffer = Vec::new();
        for case in 0..40 {
            let vision = [1500.0, 300.0, f64::NAN, 1e13][case % 4];
            let snakes = [16, 17, 40, 70][rng.below(4)];
            let food = [0, 700, 3000][rng.below(3)];
            let mut world = world(&mut rng, snakes, food, vision);
            let all = key(&world.observe_all());
            let mask: Vec<bool> = (0..snakes).map(|_| rng.below(3) != 0).collect();
            let selected = key(&world.observe_selected(&mask).unwrap());
            world.set_parallel(true);
            pool.install(|| {
                assert_eq!(key(&world.observe_all()), all, "case {case}");
                assert_eq!(
                    key(&world.observe_selected(&mask).unwrap()),
                    selected,
                    "case {case}"
                );
                world.observe_all_into(&mut buffer);
                assert_eq!(key(&buffer), all, "case {case}");
                world.observe_selected_into(&mask, &mut buffer).unwrap();
                assert_eq!(key(&buffer), selected, "case {case}");
            });
        }
    }
}
