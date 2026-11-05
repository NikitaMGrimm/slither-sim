//! Thread-count invariance of the parallel step: sampled authoritative
//! checkpoints and every tick's deaths and events match the serial world.
use super::*;
use crate::EventRecording;
use std::cell::Cell;
use std::hash::{DefaultHasher, Hash, Hasher};

thread_local! {
    /// Snakes per parallel task on this thread; tests lower it so small
    /// worlds take the parallel path too.
    pub(super) static TASK_SNAKES: Cell<usize> = const { Cell::new(SNAKES_PER_TASK) };
}

const POOLS: [usize; 5] = [1, 2, 3, 4, 7];

fn unit(slot: usize, salt: usize) -> f64 {
    let mut random = Random((slot as u32).wrapping_mul(2_654_435_761) ^ salt as u32);
    random.next();
    random.next()
}

fn world(config: Config, snakes: usize) -> Simulator {
    let mut world = Simulator::new(WorldOptions {
        seed: 7,
        config,
        population: InitialPopulation::Placed {
            ids: (0..snakes).map(|i| format!("s{i}")).collect(),
            placement: Placement::TestRing,
        },
    })
    .unwrap();
    world.record_events(EventRecording {
        pickups: true,
        food_births: true,
    });
    world
}

/// Give a living slot a head-first body, before the first step.
fn lay_body(world: &mut Simulator, slot: usize, body: Vec<Point>, heading: f64) {
    let s = &mut world.snakes[slot];
    s.head = body[0];
    s.heading = heading;
    s.segments = body.len();
    s.fam = 0.0;
    s.body = body;
    Simulator::update_mass(s);
}

/// Lay a living slot out as a straight body behind `head`, before the first step.
fn lay(world: &mut Simulator, slot: usize, head: Point, heading: f64, segments: usize) {
    let body = (0..segments)
        .map(|i| Point {
            x: head.x - heading.cos() * 42.0 * i as f64,
            y: head.y - heading.sin() * 42.0 * i as f64,
        })
        .collect();
    lay_body(world, slot, body, heading);
}

/// Wander toward the centre with per-slot jitter and staggered boosts.
fn steer(world: &Simulator, tick: usize) -> Vec<Option<Action>> {
    world
        .snakes
        .iter()
        .enumerate()
        .map(|(slot, s)| {
            s.alive.then(|| Action {
                heading: (-s.head.y).atan2(-s.head.x) + (unit(slot, tick / 24) - 0.5) * 2.5,
                boost: (tick + slot * 13) % 61 < 9,
            })
        })
        .collect()
}

/// Toward the centre and into the pit, then back out of it.
fn dome_steer(world: &Simulator, tick: usize) -> Vec<Option<Action>> {
    let mut actions = steer(world, tick);
    if tick >= 1100 {
        for action in actions.iter_mut().flatten() {
            action.heading += PI;
        }
    }
    actions
}

/// Hold every heading, never boosting.
fn hold(world: &Simulator, _: usize) -> Vec<Option<Action>> {
    world
        .snakes
        .iter()
        .map(|s| {
            s.alive.then_some(Action {
                heading: s.heading,
                boost: false,
            })
        })
        .collect()
}

/// Respawn dead slots on the rim, alternating which ones try each tick.
fn rim_respawns(world: &mut Simulator, tick: usize) {
    for slot in (tick % 3..world.snakes.len()).step_by(3) {
        if world.snakes[slot].alive {
            continue;
        }
        let angle = TAU * unit(slot, tick + 99);
        let r = world.radius() * 0.8;
        let head = Point {
            x: r * angle.cos(),
            y: r * angle.sin(),
        };
        if world.spawn_is_clear(head, angle + PI, 12, 0.0) {
            world.respawn(slot, head, angle + PI, 12).unwrap();
        }
    }
}

/// Respawn dead slots in the Battledome band with clear floor-size bodies.
fn band_respawns(world: &mut Simulator, tick: usize) {
    let band = world.config.spawn_radius.unwrap() * world.config.circular_scale(world.radius());
    for slot in 0..world.snakes.len() {
        if world.snakes[slot].alive {
            continue;
        }
        let angle = TAU * unit(slot, tick + 7);
        let head = Point {
            x: band * angle.cos(),
            y: band * angle.sin(),
        };
        let (body, heading) = world.spawn_body(head, angle + FRAC_PI_2).unwrap();
        if world.body_is_clear(&body, 0.0) {
            world.respawn_body(slot, body, heading).unwrap();
        }
    }
}

type Controller = fn(&Simulator, usize) -> Vec<Option<Action>>;
type Lifecycle = fn(&mut Simulator, usize);

/// Per tick: an authoritative checkpoint digest every `every` ticks and on the
/// last, plus the tick's deaths, pickups and food births.
fn trace(
    mut world: Simulator,
    ticks: usize,
    every: usize,
    controller: Controller,
    lifecycle: Lifecycle,
) -> Vec<(Option<u64>, String)> {
    let mut rows = Vec::with_capacity(ticks);
    for tick in 0..ticks {
        let deaths = world.deaths.len();
        let actions = controller(&world, tick);
        world.step(&actions).unwrap();
        let digest = (tick % every == 0 || tick + 1 == ticks).then(|| {
            let mut hasher = DefaultHasher::new();
            serde_json::to_vec(&world.checkpoint().unwrap())
                .unwrap()
                .hash(&mut hasher);
            hasher.finish()
        });
        let events = format!(
            "{:?} {:?} {:?}",
            &world.deaths[deaths..],
            world.pickup_events(),
            world.food_spawn_events()
        );
        rows.push((digest, events));
        lifecycle(&mut world, tick);
    }
    rows
}

/// The serial trace, then the same trace with `set_parallel(true)` on pools of
/// every size in `POOLS`, splitting phases above `per_task` snakes per task.
fn assert_pool_invariant(
    world: &Simulator,
    ticks: usize,
    every: usize,
    per_task: usize,
    controller: Controller,
    lifecycle: Lifecycle,
) -> Vec<(Option<u64>, String)> {
    let serial = trace(world.clone(), ticks, every, controller, lifecycle);
    for threads in POOLS {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        let mut parallel = world.clone();
        parallel.set_parallel(true);
        let rows = pool.install(|| {
            TASK_SNAKES.with(|task| task.set(per_task));
            trace(parallel, ticks, every, controller, lifecycle)
        });
        for (tick, (expected, actual)) in serial.iter().zip(&rows).enumerate() {
            assert_eq!(expected, actual, "{threads} threads, tick {tick}");
        }
        assert_eq!(serial.len(), rows.len());
    }
    serial
}

fn deaths(rows: &[(Option<u64>, String)]) -> usize {
    rows.iter()
        .map(|(_, e)| e.matches("DeathEvent").count())
        .sum()
}

#[test]
fn small_worlds_keep_every_phase_serial() {
    let parts = |threads: usize, snakes: usize| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap()
            .install(|| step_parts(true, snakes))
    };
    for threads in [1, 2, 8] {
        assert_eq!(parts(threads, MIN_STEP_TASKS * SNAKES_PER_TASK - 1), 1);
    }
    assert_eq!(parts(1, 100_000), 1);
    assert_eq!(parts(2, MIN_STEP_TASKS * SNAKES_PER_TASK), 8);
    assert_eq!(parts(8, MIN_STEP_TASKS * SNAKES_PER_TASK), MIN_STEP_TASKS);
    assert_eq!(parts(8, 64 * SNAKES_PER_TASK), 32);
    assert_eq!(step_parts(false, 100_000), 1);
}

/// Older heads resolve first, so a precomputed first strike can name a body
/// that fell earlier in the walk. Then the serial search decides: a mutual
/// pair leaves the newer snake alive, and a head crossing two bodies is killed
/// by the older one once the newer has fallen.
#[test]
fn collision_ordering_matches_the_serial_walk_when_strikes_fall_first() {
    const GROUPS: usize = 12;
    let config = Config {
        radius: 20_000.0,
        food_count: 0,
        ambient_replenish_per_second: 0.0,
        ..Config::sandbox()
    };
    let mut world = world(config, GROUPS * 6);
    // Each group is turned so that no tested segment is axis-aligned: the
    // inclusive segment test can miss a zero-width range after rounding.
    const TURN: f64 = 0.3;
    let (sin, cos) = TURN.sin_cos();
    for g in 0..GROUPS {
        let at = |x: f64, y: f64| Point {
            x: x * cos - y * sin - 7500.0 + 2500.0 * (g % 6) as f64,
            y: x * sin + y * cos - 3000.0 + 3000.0 * (g / 6) as f64,
        };
        // Oldest to newest: X strikes nothing, Y heads into W's body, and Z
        // heads north across both X's and Y's bodies.
        lay(&mut world, 6 * g, at(200.0, -2.0), TURN, 10);
        lay(&mut world, 6 * g + 1, at(-200.0, 2.0), TURN + PI, 10);
        lay(&mut world, 6 * g + 2, at(0.0, -25.0), TURN + FRAC_PI_2, 10);
        lay(
            &mut world,
            6 * g + 3,
            at(-225.0, 300.0),
            TURN + FRAC_PI_2,
            10,
        );
        // P heads east into Q's bent body; Q heads south across P's body.
        lay(&mut world, 6 * g + 4, at(0.0, 1000.0), TURN, 10);
        let mut q = vec![at(-50.0, 1020.0), at(-10.0, 1032.0), at(25.0, 1015.0)];
        q.extend((1..8).map(|i| at(25.0, 1015.0 - 42.0 * i as f64)));
        lay_body(&mut world, 6 * g + 5, q, TURN - FRAC_PI_2);
    }
    let rows = assert_pool_invariant(&world, 3, 1, 1, hold, |_, _| {});
    let mut serial = world.clone();
    serial.step(&hold(&serial, 0)).unwrap();
    let killers: Vec<_> = serial
        .deaths
        .iter()
        .map(|d| (d.victim.clone(), d.killer.clone()))
        .collect();
    let mut expected = vec![];
    for g in 0..GROUPS {
        let id = |k: usize| format!("s{}", 6 * g + k);
        expected.push((id(1), Some(id(3))));
        expected.push((id(2), Some(id(0))));
        expected.push((id(4), Some(id(5))));
    }
    expected.sort();
    let mut actual = killers;
    actual.sort();
    assert_eq!(actual, expected);
    assert_eq!(deaths(&rows), 3 * GROUPS);
}

/// An official-style world with food, prey, boosts, deaths and respawns, split
/// into tasks of 8 snakes so every phase runs in parallel.
#[test]
fn official_world_steps_identically_on_any_pool() {
    let world = world(Config::official(300), 300);
    let rows = assert_pool_invariant(&world, 240, 3, 8, steer, rim_respawns);
    assert!(deaths(&rows) > 20, "{}", deaths(&rows));
}

/// Thousands of snakes converge in a dense field: the collision grid, many
/// simultaneous deaths and death food. Half the shipped task size keeps the
/// split as deaths take the field below the shipped minimum.
#[test]
fn dense_world_steps_identically_on_any_pool() {
    const SIDE: usize = 68;
    let mut config = Config::official(SIDE * SIDE);
    config.radius = 8000.0;
    config.dynamic_radius = false;
    config.size_to_population = false;
    config.food_count = 20_000;
    config.ambient_replenish_per_second = 500.0;
    config.prey.count = 20;
    let mut world = world(config, SIDE * SIDE);
    for slot in 0..SIDE * SIDE {
        let head = Point {
            x: (slot % SIDE) as f64 * 90.0 - 3000.0 + 30.0 * unit(slot, 1),
            y: (slot / SIDE) as f64 * 90.0 - 3000.0 + 30.0 * unit(slot, 2),
        };
        let heading = (-head.y).atan2(-head.x) + (unit(slot, 3) - 0.5);
        lay(&mut world, slot, head, heading, 4 + slot % 9);
    }
    let rows = assert_pool_invariant(&world, 24, 8, SNAKES_PER_TASK / 2, steer, |_, _| {});
    assert!(deaths(&rows) > 500, "{}", deaths(&rows));
}

/// Battledome adds the pit, the size floor, the kill score and band respawns.
#[test]
fn battledome_world_steps_identically_on_any_pool() {
    let mut world = world(Config::battledome(), 64);
    for slot in 0..64 {
        world.retire(slot).unwrap();
    }
    band_respawns(&mut world, 0);
    let rows = assert_pool_invariant(&world, 1500, 10, 2, dome_steer, band_respawns);
    assert!(rows.iter().any(|(_, e)| e.contains("Pit")));
    assert!(deaths(&rows) > 10, "{}", deaths(&rows));
}

fn empty_config() -> Config {
    Config {
        radius: 20_000.0,
        food_count: 0,
        ambient_replenish_per_second: 0.0,
        ..Config::sandbox()
    }
}

fn split_once(mut world: Simulator, threads: usize, per_task: usize) -> Simulator {
    world.set_parallel(true);
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
        .install(|| {
            TASK_SNAKES.with(|task| task.set(per_task));
            assert!(step_parts(true, world.snakes.iter().filter(|s| s.alive).count()) > 1);
            world.step(&hold(&world, 0)).unwrap();
        });
    world
}

fn collision_order(world: &Simulator) -> Vec<usize> {
    let mut order = world.buffers.alive.clone();
    order.sort_unstable_by_key(|&i| world.joins[i]);
    order
}

/// Rebuild the grid for query checks, after verifying that the actual step
/// selected it rather than BoundsIndex or the full-scan fallback.
fn retained_grid(world: &Simulator) -> (CollisionGrid, f64) {
    assert!(
        world.buffers.collision_grid_used,
        "step must select the grid"
    );
    let order = collision_order(world);
    assert!(order.len() > 4096);
    let longest = order
        .iter()
        .map(|&i| world.snakes[i].segments)
        .max()
        .unwrap();
    let bounds: Vec<_> = order
        .iter()
        .map(|&i| world.buffers.bodies[i].unwrap())
        .collect();
    let reaches: Vec<_> = order
        .iter()
        .map(|&i| {
            3.666 * collision_distance(longest.min(GRID_HEAD_SEGMENTS), world.snakes[i].segments)
        })
        .collect();
    (
        CollisionGrid::new(&bounds, &reaches).expect("fixture must use the collision grid"),
        3.666 * collision_distance(longest, longest),
    )
}

#[test]
fn large_head_research_uses_the_grid_after_its_cached_target_falls() {
    let mut world = world(empty_config(), 4104);
    const TURN: f64 = 0.3;
    let (sin, cos) = TURN.sin_cos();
    let at = |x: f64, y: f64| Point {
        x: x * cos - y * sin,
        y: x * sin + y * cos + 6500.0,
    };
    lay(&mut world, 0, at(200.0, -2.0), TURN, 10);
    lay(&mut world, 1, at(-200.0, 2.0), TURN + PI, 10);
    lay(&mut world, 2, at(0.0, -25.0), TURN + FRAC_PI_2, 40);
    lay(&mut world, 3, at(-225.0, 300.0), TURN + FRAC_PI_2, 10);
    for slot in 4..world.snakes.len() {
        let k = slot - 4;
        lay(
            &mut world,
            slot,
            Point {
                x: -7500.0 + (k % 64) as f64 * 230.0,
                y: -10_000.0 + (k / 64) as f64 * 210.0,
            },
            0.0,
            4,
        );
    }
    assert_pool_invariant(&world, 1, 1, SNAKES_PER_TASK, hold, |_, _| {});
    let parallel = split_once(world, 3, SNAKES_PER_TASK);
    let order = collision_order(&parallel);
    let position = order.iter().position(|&i| i == 2).unwrap();
    assert!(parallel.snakes[2].segments > GRID_HEAD_SEGMENTS);
    assert_eq!(
        parallel.buffers.strikes[position],
        Some(1),
        "cached target must fall first"
    );
    assert_eq!(
        parallel
            .deaths
            .iter()
            .map(|d| (d.victim.as_str(), d.killer.as_deref()))
            .collect::<Vec<_>>(),
        [("s1", Some("s3")), ("s2", Some("s0"))]
    );
    let grid = retained_grid(&parallel);
    let scene = Collisions {
        snakes: &parallel.snakes,
        bodies: &parallel.buffers.bodies,
        heads: &parallel.buffers.heads,
        last_tips: &parallel.buffers.last_tips,
        order: &order,
        grid: Some(&grid),
        index: None,
    };
    let (tip, _, head_scale) = scene.heads[2];
    let mut candidates = Vec::new();
    assert!(head_scale > scale(GRID_HEAD_SEGMENTS));
    assert!(
        grid.0
            .candidates_for_head(
                tip,
                head_scale,
                scale(GRID_HEAD_SEGMENTS),
                grid.1,
                &mut candidates
            )
            .is_some()
    );
    let mut profile = StepProfile::default();
    assert_eq!(
        scene.first_strike::<false>(2, |_| false, &mut candidates, &mut profile),
        Some(1)
    );
    assert_eq!(
        scene.first_strike::<false>(2, |i| i == 1, &mut candidates, &mut profile),
        Some(0)
    );
}

fn spread(world: &mut Simulator, segments: usize) {
    for slot in 0..world.snakes.len() {
        lay(
            world,
            slot,
            Point {
                x: -6000.0 + (slot % 4) as f64 * 4000.0,
                y: -6000.0 + (slot / 4) as f64 * 4000.0,
            },
            0.0,
            segments,
        );
    }
}

fn prime_body_bounds(world: &mut Simulator) {
    world.buffers.body_bounds = world
        .snakes
        .iter()
        .map(|s| snake_body_bounds(&s.body))
        .collect();
}

#[test]
fn kiosk_shrink_and_recovery_invalidate_cached_bodies_in_a_split_step() {
    let mut config = empty_config();
    config.kiosk = KioskConfig {
        floor_score: 10.0,
        grace_seconds: 0.0,
        stationary_radius: 500.0,
        shrink_per_second: 50.0,
        recovery_per_second: 50.0,
        ..Default::default()
    };
    let mut world = world(config, 16);
    spread(&mut world, 80);
    world.kiosks[0].anchor = Some(world.snakes[0].head);
    world.kiosks[0].held_seconds = 1.0;
    world.kiosks[1].anchor = Some(Point {
        x: world.snakes[1].head.x + 2000.0,
        y: world.snakes[1].head.y,
    });
    world.kiosks[1].score_credit = 1000.0;
    let initial_credit = world.kiosks[1].score_credit;
    let mut serial = world.clone();
    prime_body_bounds(&mut serial);
    let old_bounds = serial.buffers.body_bounds[0].unwrap();
    serial.step(&hold(&serial, 0)).unwrap();
    assert!(
        serial.snakes[0].body.len() < 80,
        "penalty must remove anchors"
    );
    assert!(serial.kiosks[0].score_credit > 0.0);
    assert!(serial.kiosks[1].score_credit < initial_credit && serial.kiosks[1].score_credit > 0.0);
    assert!(
        serial.snakes[1].fam > 0.0,
        "recovery must restore physical mass"
    );
    assert!(serial.buffers.body_bounds[0].unwrap().min.x > old_bounds.min.x);
    let expected = serde_json::to_vec(&serial.checkpoint().unwrap()).unwrap();
    for threads in [2, 3, 7] {
        let mut parallel = world.clone();
        prime_body_bounds(&mut parallel);
        let parallel = split_once(parallel, threads, 1);
        assert_eq!(
            serde_json::to_vec(&parallel.checkpoint().unwrap()).unwrap(),
            expected
        );
        assert_eq!(parallel.pickup_events(), serial.pickup_events());
        assert_eq!(parallel.food_spawn_events(), serial.food_spawn_events());
        for slot in [0, 1] {
            let cached = parallel.buffers.body_bounds[slot].unwrap();
            let actual = snake_body_bounds(&parallel.snakes[slot].body).unwrap();
            assert_eq!((cached.min, cached.max), (actual.min, actual.max));
        }
    }
}

#[test]
fn equal_distance_food_tie_uses_lexical_id_during_split_pickup() {
    let mut world = world(empty_config(), 16);
    // Keep padding inside the dense eater grid's 4,096-cell budget.
    for slot in 0..world.snakes.len() {
        lay(
            &mut world,
            slot,
            Point {
                x: -1500.0 + (slot % 4) as f64 * 1000.0,
                y: -1500.0 + (slot / 4) as f64 * 1000.0,
            },
            0.0,
            4,
        );
    }
    lay(&mut world, 2, Point { x: 0.0, y: -25.0 }, 0.0, 4);
    lay(&mut world, 10, Point { x: 0.0, y: 25.0 }, 0.0, 4);
    let pellet = Food {
        x: 50.0,
        y: 0.0,
        value: 0.125,
        size: None,
        origin: None,
    };
    world.replace_food(vec![pellet]).unwrap();
    assert_pool_invariant(&world, 1, 1, 1, hold, |_, _| {});
    let parallel = split_once(world, 3, 1);
    assert!(parallel.snakes.iter().all(|s| s.alive));
    assert_eq!(
        pellet.point().distance(parallel.snakes[2].head).to_bits(),
        pellet.point().distance(parallel.snakes[10].head).to_bits()
    );
    assert!(
        parallel.eater_grid.occupied_len() > 1,
        "must split occupied eater cells"
    );
    assert_eq!(
        parallel.pickup_events().unwrap(),
        &[PickupEvent {
            eater: 10,
            item: PickupItem::Food(pellet)
        }]
    );
    assert_eq!(parallel.buffers.hits, [(0, 10)]);
}

#[test]
fn boundary_death_precedes_a_body_strike_in_the_same_split_step() {
    let mut world = world(empty_config(), 16);
    spread(&mut world, 4);
    lay(
        &mut world,
        0,
        Point {
            x: 19_985.0,
            y: 0.0,
        },
        0.0,
        10,
    );
    lay_body(
        &mut world,
        1,
        vec![
            Point {
                x: 19_980.0,
                y: 250.0,
            },
            Point {
                x: 19_990.0,
                y: 100.0,
            },
            Point {
                x: 19_995.0,
                y: 0.0,
            },
            Point {
                x: 19_990.0,
                y: -100.0,
            },
        ],
        FRAC_PI_2,
    );
    const TURN: f64 = 0.3;
    let (sin, cos) = TURN.sin_cos();
    let at = |x: f64, y: f64| Point {
        x: x * cos - y * sin,
        y: x * sin + y * cos,
    };
    lay(&mut world, 2, at(-200.0, 2.0), TURN + PI, 10);
    lay(&mut world, 3, at(-225.0, 300.0), TURN + FRAC_PI_2, 10);
    assert_pool_invariant(&world, 1, 1, 1, hold, |_, _| {});
    let parallel = split_once(world, 3, 1);
    let position = collision_order(&parallel)
        .iter()
        .position(|&i| i == 0)
        .unwrap();
    assert_eq!(
        parallel.buffers.strikes[position],
        Some(1),
        "boundary victim must also have a cached body strike"
    );
    assert_eq!(
        parallel.buffers.boundary_deaths,
        [(0, DeathReason::Boundary)]
    );
    assert_eq!(
        parallel
            .deaths
            .iter()
            .map(|d| (d.victim.as_str(), d.killer.as_deref(), d.reason))
            .collect::<Vec<_>>(),
        [
            ("s0", None, DeathReason::Boundary),
            ("s2", Some("s3"), DeathReason::Body)
        ]
    );
}
