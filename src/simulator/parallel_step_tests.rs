//! Thread-count invariance of the parallel step: every tick's authoritative
//! checkpoint, deaths and events must match the serial world on any pool.
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
