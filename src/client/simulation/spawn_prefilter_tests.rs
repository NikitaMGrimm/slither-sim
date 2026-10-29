//! Frozen prefilter-free search oracle from 23eddca8. Keep its draws, ties,
//! fallback and error path independent of the production shortcut.
use super::*;

impl SimulatorBackend {
    fn original_spawn_pose_waited(
        &mut self,
        slot: usize,
        waited: f64,
    ) -> Result<(Vec<Point>, f64), String> {
        let circular = self.world.config().spawn_radius.is_some();
        let mut safety = None;
        let mut floor = None;
        let mut open: Option<(f64, Vec<Point>, f64)> = None;
        let attempts = if circular {
            spawn::CIRCULAR_ATTEMPTS
        } else {
            SPAWN_ATTEMPTS
        };
        for attempt in 0..attempts {
            let distance = spawn::spawn_distance(
                self.world.config(),
                self.world.radius(),
                self.spawn_random.unit(),
            );
            let angle = self.spawn_random.unit() * TAU;
            let heading = self.spawn_random.unit() * TAU;
            let head = Point {
                x: angle.cos() * distance,
                y: angle.sin() * distance,
            };
            // Local policy: official joins keep a median 925 units from
            // visible bodies, but three came within 550-600.
            let (body, heading) = self
                .world
                .spawn_body(head, heading)
                .map_err(|error| error.to_string())?;
            let clear = if circular {
                let query = safety.get_or_insert_with(|| {
                    spawn::Safety::new_excluding(
                        &self.world,
                        Some(slot),
                        body.len(),
                        spawn::CIRCULAR_CLEARANCE,
                        spawn::HEAD_PATH_SECONDS,
                    )
                });
                query.head_is_clear(body[0]) && query.clears(&body)
            } else {
                self.world
                    .body_is_clear_excluding(&body, spawn::CLEARANCE, Some(slot))
            };
            if clear && (!circular || safety.as_ref().unwrap().clears_departures(body[0], heading))
            {
                return Ok((body, heading));
            }
            if circular {
                if waited >= 2. && attempt < 256 && self.world.config().min_score.is_some() {
                    let query = floor.get_or_insert_with(|| {
                        spawn::Safety::departure_floor(&self.world, slot, body.len())
                    });
                    if query.head_is_clear(body[0])
                        && query.clears(&body)
                        && query.clears_departures(body[0], heading)
                    {
                        let incumbent = open.as_ref().map_or(0., |(margin, ..)| *margin);
                        let margin = query.body_margin(incumbent);
                        if margin > incumbent {
                            open = Some((margin, body, heading));
                        }
                    }
                }
                continue;
            }
            // A candidate's margin only decreases while scanning neighbors.
            // Stop once it cannot beat zero (admission) or the incumbent;
            // keep strict ties and all random draws unchanged. See:
            // research/performance/2026-10-25-battledome-spawn-cost/REPORT.md.
            let best = open.as_ref().map_or(0., |(best, ..)| best.max(0.));
            if let Some(margin) = self
                .world
                .spawn_margin_bounded_excluding(&body, best, Some(slot))
                && open.as_ref().is_none_or(|(best, ..)| margin > *best)
            {
                open = Some((margin, body, heading));
            }
        }
        if circular {
            if let Some((_, body, heading)) = open {
                return Ok((body, heading));
            }
            return Err(spawn::WAITING.into());
        }
        match open {
            Some((margin, body, heading)) if margin > 0. => Ok((body, heading)),
            Some(_) => Err(
                "Spawn candidate search exhausted: no sampled position is clear of other snakes"
                    .into(),
            ),
            None => Err(
                "Spawn candidate search exhausted: no sampled position fits inside the arena"
                    .into(),
            ),
        }
    }
}

fn scene(seed: u32, gap: Option<f64>, floor: bool) -> SimulatorBackend {
    let mut world = crate::Config::battledome();
    world.dynamic_radius = false;
    world.spawn_radius_max = None;
    world.food_count = 0;
    world.ambient_replenish_per_second = 0.;
    if !floor {
        world.min_score = None;
    }
    let mut sim = RawSimulator::new(
        seed,
        SimulationConfig {
            agents: 2,
            world,
            ..Default::default()
        },
        2,
    )
    .unwrap();
    if let Some(gap) = gap {
        let radius = sim.backend.world.config().spawn_radius.unwrap() + gap;
        let body: Vec<_> = (0..=64)
            .map(|i| {
                let angle = TAU * i as f64 / 64.;
                Point {
                    x: radius * angle.cos(),
                    y: radius * angle.sin(),
                }
            })
            .collect();
        sim.backend
            .world
            .import_life(
                1,
                crate::ReplayLife {
                    head: body[0],
                    speed: crate::InitialSpeed::Infer(crate::physics::base_speed(body.len())),
                    body,
                    heading: std::f64::consts::FRAC_PI_2,
                    fam: 0.,
                    kills: 0,
                    alive: true,
                },
            )
            .unwrap();
    }
    sim.backend
}

#[test]
fn prefilter_matches_original_poses_errors_rng_and_later_searches() {
    let mut exhausted = 0;
    let mut admitted = 0;
    for seed in [1, 7, 31] {
        for gap in [None, Some(150.), Some(350.), Some(800.)] {
            for floor in [false, true] {
                let mut actual = scene(seed, gap, floor);
                let mut original = scene(seed, gap, floor);
                for waited in [0., 2_f64.next_down(), 2., 9.] {
                    let a = actual.spawn_pose_waited(0, waited);
                    let b = original.original_spawn_pose_waited(0, waited);
                    // JSON serializes all finite body/heading values exactly;
                    // compare bits as well, including any signed zero.
                    match (&a, &b) {
                        (Ok((ab, ah)), Ok((bb, bh))) => {
                            admitted += 1;
                            assert_eq!(ah.to_bits(), bh.to_bits());
                            assert_eq!(ab.len(), bb.len());
                            for (a, b) in ab.iter().zip(bb) {
                                assert_eq!(
                                    (a.x.to_bits(), a.y.to_bits()),
                                    (b.x.to_bits(), b.y.to_bits())
                                );
                            }
                        }
                        (Err(a), Err(b)) => {
                            assert_eq!(a, b);
                            exhausted += usize::from(a == spawn::WAITING);
                        }
                        _ => panic!(
                            "seed={seed} gap={gap:?} floor={floor} waited={waited}: {a:?} != {b:?}"
                        ),
                    }
                    assert_eq!(
                        format!("{:?}", actual.spawn_random),
                        format!("{:?}", original.spawn_random)
                    );
                    assert_eq!(
                        serde_json::to_value(actual.world.checkpoint().unwrap()).unwrap(),
                        serde_json::to_value(original.world.checkpoint().unwrap()).unwrap()
                    );
                }
            }
        }
    }
    assert!(exhausted >= 12 && admitted >= 12);
}

#[test]
fn fallback_poses_are_not_discarded_by_the_stricter_head_gate() {
    let mut strict = scene(7, Some(350.), true);
    assert_eq!(strict.spawn_pose_waited(0, 0.), Err(spawn::WAITING.into()));
    let mut fallback = scene(7, Some(350.), true);
    let mut original = scene(7, Some(350.), true);
    assert_eq!(
        fallback.spawn_pose_waited(0, 2.),
        original.original_spawn_pose_waited(0, 2.)
    );
    assert!(fallback.spawn_pose_waited(0, 3.).is_ok());
}
