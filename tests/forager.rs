use slither_sim::{
    Action, Config, Food, Observation, Point, Simulator,
    forager::{self, Params, Scratch, Timing},
    physics,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
};

struct CountAllocations;
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

fn allocated() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

unsafe impl GlobalAlloc for CountAllocations {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        allocated();
        unsafe { System.realloc(ptr, layout, size) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        allocated();
        unsafe { System.alloc_zeroed(layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountAllocations = CountAllocations;

fn fixture() -> Observation {
    let mut world = Simulator::new(1, Config::sandbox()).unwrap();
    world.add("self", 0, 1).unwrap();
    let mut observation = world.observe(0).unwrap();
    observation.me.head = Point::default();
    observation.me.heading = 0.0;
    observation.me.body = vec![Point { x: -42.0, y: 0.0 }];
    observation.food.clear();
    observation.prey.clear();
    observation
}

fn food(x: f64, y: f64, value: f64) -> Food {
    Food {
        x,
        y,
        value,
        size: None,
        origin: None,
    }
}

fn decide(observation: &Observation, timing: &Timing, params: &Params) -> Action {
    forager::decide(observation, timing, params, &mut Scratch::default())
}

fn still(observation: &Observation) -> Action {
    decide(observation, &Timing::default(), &Params::default())
}

/// Native decisions recorded from the original crate at its pinned commit.
#[test]
fn default_zero_lead_decisions_match_the_original_forager() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/forager-native-parity.json")).unwrap();
    assert_eq!(fixture["dataOrigin"], "local-simulator");
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 12);
    let mut scratch = Scratch::default();
    for case in cases {
        let observation: Observation = serde_json::from_value(case["observation"].clone()).unwrap();
        let expected: u64 = case["headingBits"].as_str().unwrap().parse().unwrap();
        let action = forager::decide(
            &observation,
            &Timing::default(),
            &Params::default(),
            &mut scratch,
        );
        assert_eq!(action.heading.to_bits(), expected);
        assert!(!action.boost);
    }
}

#[test]
fn richer_pooled_food_beats_a_slightly_nearer_crumb() {
    let mut observation = fixture();
    observation.food = vec![food(200.0, 160.0, 1.0)];
    assert!(still(&observation).heading > 0.0);
    observation
        .food
        .extend((0..6).map(|i| food(240.0 + i as f64, -200.0, 1.0)));
    assert!(still(&observation).heading < 0.0);
}

#[test]
fn food_inside_the_turning_circle_is_skipped() {
    let mut observation = fixture();
    // Abeam at 30 units: unreachable without orbiting.
    observation.food = vec![food(0.0, 30.0, 5.0), food(400.0, -50.0, 1.0)];
    assert!(still(&observation).heading < 0.0);
}

#[test]
fn rim_turns_the_bot_inward_and_body_ahead_deflects_it() {
    let mut observation = fixture();
    observation.me.head.x = observation.arena.radius - 60.0;
    let action = still(&observation);
    assert!(action.heading.abs() > 0.2, "{action:?}");

    let mut observation = fixture();
    observation.food = vec![food(400.0, 0.0, 1.0)];
    assert_eq!(still(&observation).heading, 0.0);
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    opponent.head = Point { x: 120.0, y: 300.0 };
    // Anchors at the protocol's spacing: obstacles are gathered by nearby anchors.
    opponent.body = (1..=14)
        .map(|i| Point {
            x: 120.0,
            y: 300.0 - physics::ANCHOR_SPACING * i as f64,
        })
        .collect();
    observation.opponents.push(opponent);
    let action = still(&observation);
    assert!(action.heading.abs() > 0.2, "{action:?}");
}

#[test]
fn lead_replays_in_flight_commands_and_scale_zero_disables_it() {
    let mut observation = fixture();
    observation.food = vec![food(300.0, 0.0, 1.0)];
    let params = Params::default();
    let mut timing = Timing {
        observation_age: 0.1,
        command_delay: 0.1,
        ..Default::default()
    };
    // A sharp left turn sent just before the decision acts for most of the lead.
    timing.push_sent(
        0.01,
        Action {
            heading: 1.5,
            boost: false,
        },
    );
    let led = decide(&observation, &timing, &params);
    // Predicted to be turned left of the pellet, the bot steers back right of it.
    assert!(led.heading < 0.0, "{led:?}");
    let off = Params {
        lead_scale: 0.0,
        ..params
    };
    assert_eq!(decide(&observation, &timing, &off), still(&observation));
}

#[test]
fn opponents_advance_by_the_lead() {
    let mut observation = fixture();
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    // Crossing our path from the right, currently clear of the short arc.
    opponent.head = Point {
        x: 150.0,
        y: -330.0,
    };
    opponent.heading = std::f64::consts::FRAC_PI_2;
    opponent.speed = physics::BOOST_SPEED;
    opponent.body = vec![Point {
        x: 150.0,
        y: -380.0,
    }];
    observation.opponents.push(opponent);
    assert_eq!(still(&observation).heading, 0.0);
    let timing = Timing {
        observation_age: 0.15,
        command_delay: 0.15,
        ..Default::default()
    };
    assert_ne!(
        decide(&observation, &timing, &Params::default()).heading,
        0.0
    );
}

#[test]
fn ping_uncertainty_widens_margins() {
    let mut observation = fixture();
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    opponent.head = Point { x: 200.0, y: 100.0 };
    opponent.heading = 0.0;
    opponent.speed = 0.0;
    opponent.body = vec![Point { x: 100.0, y: 100.0 }];
    observation.opponents.push(opponent);
    assert_eq!(still(&observation).heading, 0.0);
    let timing = Timing {
        uncertainty: 0.4,
        ..Default::default()
    };
    assert_ne!(
        decide(&observation, &timing, &Params::default()).heading,
        0.0
    );
}

#[test]
fn invalid_parameters_are_rejected() {
    let defaults = Params::default();
    assert!(defaults.validate().is_ok());
    for bad in [
        Params {
            cells: 0,
            ..defaults
        },
        Params {
            cells: forager::MAX_CELLS + 1,
            ..defaults
        },
        Params {
            fan_len: 0,
            ..defaults
        },
        Params {
            lookahead_seconds: 0.0,
            ..defaults
        },
        Params {
            uplink_share: 1.5,
            ..defaults
        },
        Params {
            safe: f64::NAN,
            ..defaults
        },
    ] {
        assert!(bad.validate().is_err(), "{bad:?}");
    }
}

#[test]
fn steady_decisions_do_not_allocate_or_mutate_the_observation() {
    let mut observation = fixture();
    observation.food = (0..512)
        .map(|i| {
            food(
                (i % 40) as f64 * 30.0 - 600.0,
                (i / 40) as f64 * 30.0 - 200.0,
                1.0,
            )
        })
        .collect();
    for k in 0..24 {
        let mut opponent = observation.me.clone();
        opponent.id = format!("opponent{k}");
        opponent.head = Point {
            x: 60.0 + 20.0 * k as f64,
            y: -150.0,
        };
        opponent.heading = 1.0;
        opponent.speed = 200.0;
        opponent.segments = 400;
        opponent.body = (0..430)
            .map(|i| Point {
                x: opponent.head.x + (i as f64 * 0.3).sin() * 80.0,
                y: opponent.head.y - 10.0 * i as f64,
            })
            .collect();
        observation.opponents.push(opponent);
    }
    let original = observation.clone();
    let mut timing = Timing {
        observation_age: 0.12,
        command_delay: 0.1,
        uncertainty: 0.03,
        ..Default::default()
    };
    for i in 0..forager::MAX_SENT {
        timing.push_sent(
            0.042 * i as f64,
            Action {
                heading: 0.1 * i as f64,
                boost: i % 3 == 0,
            },
        );
    }
    let params = Params::default();
    let mut scratch = Scratch::default();
    // Warm once: buffers reserve their working capacity.
    forager::decide(&observation, &timing, &params, &mut scratch);
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    for _ in 0..32 {
        std::hint::black_box(forager::decide(
            std::hint::black_box(&observation),
            &timing,
            &params,
            &mut scratch,
        ));
    }
    COUNTING.with(|counting| counting.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(observation, original);
}
