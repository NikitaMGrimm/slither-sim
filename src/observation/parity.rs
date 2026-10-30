//! Independent full scans define visibility and ordering, without batch bounds.
use super::*;

fn scan(w: VisibleState<'_>, index: usize) -> Sighting {
    let me = &w.snakes[index];
    let visible = |p: Point| {
        (p.x - me.head.x).abs() <= w.vision
            && (p.y - me.head.y).abs() <= w.vision
            && p.distance(me.head) <= w.vision
    };
    Sighting {
        food: w
            .food
            .iter()
            .enumerate()
            .filter_map(|(i, f)| visible(f.point()).then_some(i))
            .collect(),
        prey: w
            .prey
            .iter()
            .enumerate()
            .filter_map(|(i, p)| visible(p.position).then_some(i))
            .collect(),
        opponents: w
            .snakes
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                (i != index && s.alive && (visible(s.head) || s.body.iter().any(|&p| visible(p))))
                    .then_some(i)
            })
            .collect(),
    }
}

fn materialize(w: VisibleState<'_>, index: usize, seen: &Sighting) -> Observation {
    Observation {
        time: w.time,
        me: w.snakes[index].clone(),
        food: seen.food.iter().map(|&i| w.food[i]).collect(),
        prey: seen.prey.iter().map(|&i| w.prey[i].clone()).collect(),
        opponents: seen
            .opponents
            .iter()
            .map(|&i| w.snakes[i].clone())
            .collect(),
        arena: Arena {
            center: Point::default(),
            radius: w.radius,
        },
        vision_radius: Some(w.vision),
    }
}

fn equal_observation(a: &Observation, b: &Observation) {
    let bits = |values: &[f64]| values.iter().map(|v| v.to_bits()).collect::<Vec<_>>();
    assert_eq!(
        bits(&[a.time, a.arena.center.x, a.arena.center.y, a.arena.radius]),
        bits(&[b.time, b.arena.center.x, b.arena.center.y, b.arena.radius])
    );
    assert_eq!(
        a.vision_radius.map(f64::to_bits),
        b.vision_radius.map(f64::to_bits)
    );
    assert_eq!(
        (a.opponents.len(), a.food.len(), a.prey.len()),
        (b.opponents.len(), b.food.len(), b.prey.len())
    );
    for (a, b) in std::iter::once(&a.me)
        .chain(&a.opponents)
        .zip(std::iter::once(&b.me).chain(&b.opponents))
    {
        assert_eq!(
            (&a.id, a.segments, a.kills, a.alive, a.body.len()),
            (&b.id, b.segments, b.kills, b.alive, b.body.len())
        );
        assert_eq!(
            bits(&[a.heading, a.speed, a.radius, a.length, a.fam, a.score]),
            bits(&[b.heading, b.speed, b.radius, b.length, b.fam, b.score])
        );
        for (a, b) in std::iter::once(&a.head)
            .chain(&a.body)
            .zip(std::iter::once(&b.head).chain(&b.body))
        {
            assert_eq!(
                (a.x.to_bits(), a.y.to_bits()),
                (b.x.to_bits(), b.y.to_bits())
            );
        }
    }
    for (a, b) in a.food.iter().zip(&b.food) {
        assert_eq!(bits(&[a.x, a.y, a.value]), bits(&[b.x, b.y, b.value]));
        assert_eq!(
            (a.size.map(f64::to_bits), a.origin),
            (b.size.map(f64::to_bits), b.origin)
        );
    }
    for (a, b) in a.prey.iter().zip(&b.prey) {
        assert_eq!(a.id, b.id);
        assert_eq!(
            bits(&[
                a.position.x,
                a.position.y,
                a.heading,
                a.target_heading,
                a.speed,
                a.size
            ]),
            bits(&[
                b.position.x,
                b.position.y,
                b.heading,
                b.target_heading,
                b.speed,
                b.size
            ])
        );
    }
}

fn random(state: &mut u64) -> f64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 11) as f64 / (1u64 << 53) as f64
}

fn snake(slot: usize, head: Point, body: Vec<Point>) -> Snake {
    Snake {
        id: slot.to_string(),
        head,
        segments: body.len(),
        body,
        heading: 0.7,
        speed: 5.,
        radius: 10.,
        length: 12.75,
        fam: 0.75,
        score: 100.,
        kills: 3,
        alive: true,
    }
}

#[test]
fn randomized_batches_match_full_scans_including_extreme_geometry() {
    let mut marks = Vec::new();
    let mut slot = None;
    let mut state = 0x7185_ac31_9482;
    for n in [1, 7, 32, 255, 256, 513] {
        for spread in [300., 15_000., 300_000.] {
            for vision in [0., 1500., TAME, TAME.next_up(), f64::INFINITY, f64::NAN] {
                let mut snakes: Vec<_> = (0..n)
                    .map(|i| {
                        let head = Point {
                            x: (random(&mut state) - 0.5) * spread,
                            y: (random(&mut state) - 0.5) * spread,
                        };
                        let body = (0..(2 + (random(&mut state) * 32.) as usize))
                            .map(|k| Point {
                                x: head.x - k as f64 * 42.,
                                y: head.y,
                            })
                            .collect();
                        let mut s = snake(i, head, body);
                        s.alive = i % 11 != 3;
                        s
                    })
                    .collect();
                let mut food: Vec<_> = (0..2049)
                    .map(|i| {
                        // Alternate clustered and diffuse food; order crosses many words.
                        let scale = if i % 3 == 0 { 100. } else { spread };
                        Food {
                            x: (random(&mut state) - 0.5) * scale,
                            y: (random(&mut state) - 0.5) * scale,
                            value: 1.,
                            size: Some(2.),
                            origin: Some(Origin::Ambient),
                        }
                    })
                    .collect();
                for (i, x) in [f64::NAN, f64::INFINITY, -TAME * 2., -0.]
                    .into_iter()
                    .enumerate()
                {
                    food[i].x = x;
                    snakes[i % n].head.x = x;
                }
                if n >= 256 {
                    // A tail spans the whole world; heads alone cannot index it.
                    snakes[10].head = Point {
                        x: spread,
                        y: spread,
                    };
                    snakes[10].body = (0..2048)
                        .map(|i| Point {
                            x: spread - i as f64 * spread / 1024.,
                            y: 0.,
                        })
                        .collect();
                    snakes[11].body.fill(Point {
                        x: f64::NAN,
                        y: f64::NAN,
                    });
                    snakes[11].head = Point {
                        x: f64::NAN,
                        y: f64::NAN,
                    };
                }
                let w = VisibleState {
                    time: 13.,
                    snakes: &snakes,
                    food: &food,
                    prey: &[],
                    radius: spread,
                    vision,
                };
                let batch = Observer::new(w, n);
                let tree = batch.snake_index(n);
                for i in 0..n {
                    let expected = scan(w, i);
                    let mut actual = Sighting::default();
                    batch
                        .visit(i, &mut marks, &mut actual, tree.as_ref())
                        .unwrap();
                    assert_eq!(
                        actual.food, expected.food,
                        "food n={n}, i={i}, vision={vision}"
                    );
                    assert_eq!(actual.prey, expected.prey);
                    assert_eq!(
                        actual.opponents, expected.opponents,
                        "snakes n={n}, i={i}, vision={vision}"
                    );
                    batch
                        .observe_into(i, &mut marks, &mut slot, tree.as_ref())
                        .unwrap();
                    equal_observation(slot.as_ref().unwrap(), &materialize(w, i, &expected));
                    assert!(marks.iter().all(|&word| word == 0));
                }
            }
        }
    }
}

#[test]
fn official_worlds_all_selected_and_sightings_match_with_reused_buffers() {
    let mut observations = Vec::new();
    let mut sightings = Vec::new();
    let mut marks = Vec::new();
    for seed in 1..=4 {
        for n in [8, 32, 128, 256, 1000] {
            let mut world = Simulator::new(crate::WorldOptions {
                seed,
                config: crate::Config::official(n),
                population: crate::InitialPopulation::Placed {
                    ids: (0..n).map(|i| i.to_string()).collect(),
                    placement: crate::Placement::TestRing,
                },
            })
            .unwrap();
            // Include dead slots and a rim observer through supported mutation.
            world.retire(n / 2).unwrap();
            world
                .respawn(
                    n / 2,
                    Point {
                        x: world.radius() - 50.,
                        y: 0.,
                    },
                    0.,
                    2,
                )
                .unwrap();
            world.retire(n - 1).unwrap();
            world.observe_all_into(&mut observations);
            world.sight_all_into(&mut sightings, &mut marks);
            let w = VisibleState::of(&world);
            for i in 0..n {
                if w.snakes[i].alive {
                    let expected = scan(w, i);
                    equal_observation(
                        observations[i].as_ref().unwrap(),
                        &materialize(w, i, &expected),
                    );
                    let seen = sightings[i].as_ref().unwrap();
                    assert_eq!(seen.food, expected.food);
                    assert_eq!(seen.prey, expected.prey);
                    assert_eq!(seen.opponents, expected.opponents);
                } else {
                    assert!(observations[i].is_none() && sightings[i].is_none());
                }
            }
            for phase in 0..6 {
                let mask: Vec<_> = (0..n).map(|i| (i + phase) % 6 == 0).collect();
                world
                    .observe_selected_into(&mask, &mut observations)
                    .unwrap();
                for i in 0..n {
                    if mask[i] && w.snakes[i].alive {
                        equal_observation(
                            observations[i].as_ref().unwrap(),
                            &materialize(w, i, &scan(w, i)),
                        );
                    } else {
                        assert!(observations[i].is_none());
                    }
                }
            }
            world
                .observe_selected_into(&vec![false; n], &mut observations)
                .unwrap();
            assert!(observations.iter().all(Option::is_none));
            #[cfg(feature = "parallel")]
            if n >= 256 {
                world.set_parallel(true);
                let pool = rayon::ThreadPoolBuilder::new()
                    .num_threads(4)
                    .build()
                    .unwrap();
                pool.install(|| world.observe_all_into(&mut observations));
                let mask: Vec<_> = (0..n).map(|i| i % 6 == 0).collect();
                let mut selected = Vec::new();
                pool.install(|| world.observe_selected_into(&mask, &mut selected))
                    .unwrap();
                let w = VisibleState::of(&world);
                for i in 0..n {
                    if w.snakes[i].alive {
                        equal_observation(
                            observations[i].as_ref().unwrap(),
                            &materialize(w, i, &scan(w, i)),
                        );
                        if mask[i] {
                            equal_observation(
                                selected[i].as_ref().unwrap(),
                                observations[i].as_ref().unwrap(),
                            );
                        } else {
                            assert!(selected[i].is_none());
                        }
                    } else {
                        assert!(selected[i].is_none() && observations[i].is_none());
                    }
                }
            }
        }
    }
}

#[test]
fn large_food_marks_keep_order_and_clear_summaries_across_resizes() {
    let snakes: Vec<_> = (0..8)
        .map(|i| {
            let head = Point {
                x: i as f64 * 4000.,
                y: 0.,
            };
            snake(i, head, vec![head])
        })
        .collect();
    let mut marks = Vec::new();
    let mut slot = None;
    let mut random_state = 0x0012_8fca_7803;
    for count in [262_143, 262_144, 262_145, 524_289, 1, 0] {
        let food: Vec<_> = (0..count)
            .map(|i| Food {
                x: if i % 37 == 0 {
                    0.
                } else {
                    random(&mut random_state) * 50_000.
                },
                y: if i % 4096 == 0 {
                    0.
                } else {
                    (random(&mut random_state) - 0.5) * 10_000.
                },
                value: i as f64,
                size: None,
                origin: None,
            })
            .collect();
        for vision in [0., 1500.] {
            let w = VisibleState {
                time: 0.,
                snakes: &snakes,
                food: &food,
                prey: &[],
                radius: 100_000.,
                vision,
            };
            let observer = Observer::new(w, 8);
            for i in 0..8 {
                let expected = scan(w, i);
                let mut seen = Sighting::default();
                observer.visit(i, &mut marks, &mut seen, None).unwrap();
                assert_eq!(seen.food, expected.food);
                observer
                    .observe_into(i, &mut marks, &mut slot, None)
                    .unwrap();
                equal_observation(slot.as_ref().unwrap(), &materialize(w, i, &expected));
                assert!(marks.iter().all(|&word| word == 0));
            }
        }
    }
}

#[test]
fn legacy_cell_id_aliases_keep_original_prefix_offsets() {
    let food = [
        Food {
            x: 1.,
            y: 0.,
            value: 1.,
            size: None,
            origin: None,
        },
        Food {
            x: 3.,
            y: 0.,
            value: 2.,
            size: None,
            origin: None,
        },
    ];
    // Model a narrowed high cell alias and the reserved ID, without allocating
    // a table with billions of cells. The original builder retains its prefix
    // table even when encoded cell IDs do not match the histogram.
    for cells in [[0, 0], [0, u32::MAX]] {
        let grid = FoodGrid {
            origin: Point { x: 1., y: 0. },
            cell: 1.,
            inv_cell: 1.,
            cols: 3,
            rows: 1,
            starts: vec![0, 1, 0, 1],
            points: Vec::new(),
            ids: Vec::new(),
            outliers: Vec::new(),
        }
        .populate_legacy(&food, &cells);
        assert_eq!(grid.starts, [0, 1, 1, 2]);
        assert_eq!(grid.points[0], food[0].point());
        assert_eq!(grid.ids[0], 0);
        if cells[1] == u32::MAX {
            assert_eq!(grid.points[1], Point::default());
            assert_eq!(grid.ids[1], 0);
        } else {
            assert_eq!(grid.points[1], food[1].point());
            assert_eq!(grid.ids[1], 1);
        }
    }
}
