use slither_sim::{
    Action, Config, Food, Observation, Point, Simulator,
    forager::{self, Intent, Params, Scratch, Timing},
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
    forager::decide(observation, timing, &[], params, &mut Scratch::default())
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
            &[],
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
fn reported_wanted_heading_bends_the_predicted_head_path() {
    let mut observation = fixture();
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    // Running parallel below our path, but wanting to turn up across its end.
    opponent.head = Point { x: 40.0, y: -80.0 };
    opponent.heading = 0.0;
    opponent.speed = 185.0;
    opponent.body = vec![Point { x: -2.0, y: -80.0 }];
    observation.opponents.push(opponent);
    let intents = [Intent {
        wanted: std::f64::consts::FRAC_PI_2,
        heading_age: 0.0,
    }];
    let defaults = Params::default();
    let mut scratch = Scratch::default();
    let straight = forager::decide(
        &observation,
        &Timing::default(),
        &intents,
        &defaults,
        &mut scratch,
    );
    // Defaults ignore intents: the original straight forecast.
    assert_eq!(straight, still(&observation));
    let params = Params {
        intent_weight: 1.0,
        ..defaults
    };
    let turned = forager::decide(
        &observation,
        &Timing::default(),
        &intents,
        &params,
        &mut scratch,
    );
    assert_ne!(turned.heading, straight.heading);
}

#[test]
fn tip_based_danger_lets_the_bot_pass_beside_a_body() {
    let mut observation = fixture();
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    // A body ahead, parallel to our path and just beside its end.
    opponent.head = Point { x: 300.0, y: 20.0 };
    opponent.heading = 0.0;
    opponent.speed = 0.0;
    opponent.body = (1..=5)
        .map(|i| Point {
            x: 300.0 - physics::ANCHOR_SPACING * i as f64,
            y: 20.0,
        })
        .collect();
    observation.opponents.push(opponent);
    // Same safety threshold; only the own-width treatment differs.
    let centre = Params {
        safe: 5.0,
        ..Params::default()
    };
    assert_ne!(
        decide(&observation, &Timing::default(), &centre).heading,
        0.0
    );
    let params = Params {
        own_width_factor: 0.0,
        ..centre
    };
    assert_eq!(
        decide(&observation, &Timing::default(), &params).heading,
        0.0
    );
}

#[test]
fn processing_delays_only_the_command_decided_now() {
    let mut observation = fixture();
    observation.food = vec![food(300.0, 80.0, 1.0)];
    let turn = Action {
        heading: 1.2,
        boost: false,
    };
    let mut timing = Timing {
        observation_age: 0.05,
        command_delay: 0.05,
        processing: 0.04,
        ..Default::default()
    };
    timing.push_sent(0.08, turn);
    // The same arrivals and lead, with processing folded into the ages.
    let mut folded = Timing {
        observation_age: 0.09,
        command_delay: 0.05,
        ..Default::default()
    };
    folded.push_sent(0.12, turn);
    let params = Params::default();
    assert_eq!(
        decide(&observation, &timing, &params),
        decide(&observation, &folded, &params)
    );
}

#[test]
fn the_whole_decoder_command_history_is_kept() {
    let mut timing = Timing::default();
    for i in 0..forager::MAX_SENT + 3 {
        timing.push_sent(i as f64 * 0.01, Action::default());
    }
    assert_eq!(forager::MAX_SENT, 64);
    assert_eq!(timing.sent_len, forager::MAX_SENT);
}

#[test]
fn widened_obstacles_are_gathered_beyond_the_plain_reach() {
    // Reviewer reproduction: a stationary head outside the plain gathering
    // radius whose uncertainty-widened envelope reaches the arc.
    let mut observation = fixture();
    observation.me.speed = 200.0;
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    opponent.head = Point { x: 250.0, y: 0.0 };
    opponent.speed = 0.0;
    opponent.body.clear();
    observation.opponents.push(opponent);
    let timing = Timing {
        uncertainty: 0.8,
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
        Params {
            intent_weight: 1.5,
            ..defaults
        },
        Params {
            own_width_factor: -0.1,
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
    // Every optional feature on, so its paths are covered too.
    let params = Params {
        intent_weight: 1.0,
        stale_margin: 0.5,
        own_width_factor: 0.5,
        kill_weight: 0.4,
        coil_weight: 1.0,
        coil_mass_ratio: 2.0,
        kill_boost: true,
        escape_boost: true,
        boost_min_score: 0.0,
        ..Params::default()
    };
    let intents: Vec<_> = (0..observation.opponents.len())
        .map(|k| Intent {
            wanted: k as f64 * 0.3,
            heading_age: 0.05,
        })
        .collect();
    let mut scratch = Scratch::default();
    // Warm once: buffers reserve their working capacity.
    forager::decide(&observation, &timing, &intents, &params, &mut scratch);
    ALLOCATIONS.with(|count| count.set(0));
    COUNTING.with(|counting| counting.set(true));
    for _ in 0..32 {
        std::hint::black_box(forager::decide(
            std::hint::black_box(&observation),
            &timing,
            &intents,
            &params,
            &mut scratch,
        ));
    }
    COUNTING.with(|counting| counting.set(false));
    assert_eq!(ALLOCATIONS.with(Cell::get), 0);
    assert_eq!(observation, original);
}

#[test]
fn kill_seeking_crosses_ahead_of_a_nearby_head() {
    let mut observation = fixture();
    // Food straight ahead; an opponent to the left heading down across our
    // path, far enough that we can cross ahead of it.
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    opponent.segments = 60;
    // We cross x = 60 well before it does, transversely and with its head
    // clear of our forward probe.
    opponent.head = Point { x: 60.0, y: 170.0 };
    opponent.heading = -std::f64::consts::FRAC_PI_2;
    opponent.speed = 185.0;
    opponent.body = (1..=8)
        .map(|i| Point {
            x: 60.0,
            y: 170.0 + physics::ANCHOR_SPACING * i as f64,
        })
        .collect();
    observation.opponents.push(opponent);
    let off = still(&observation);
    let params = Params {
        kill_weight: 0.4,
        ..Params::default()
    };
    let on = decide(&observation, &Timing::default(), &params);
    assert_ne!(on.heading, off.heading);
    // Toward the victim's side, to lay our body across its path.
    assert!(on.heading > off.heading, "{on:?} {off:?}");
}

#[test]
fn coiling_orbits_a_much_smaller_snake_the_way_we_already_turn_around_it() {
    let mut observation = fixture();
    observation.me.segments = 200;
    observation.me.speed = physics::base_speed(200);
    observation.me.body = (1..=100)
        .map(|i| Point {
            x: -physics::ANCHOR_SPACING * i as f64,
            y: 0.0,
        })
        .collect();
    let mut target = observation.me.clone();
    target.id = "small".into();
    target.segments = 5;
    target.speed = physics::base_speed(5);
    target.head = Point { x: 0.0, y: 150.0 };
    target.body = vec![Point { x: -42.0, y: 150.0 }];
    observation.opponents.push(target);
    let off = still(&observation);
    assert_eq!(off.heading, 0.0);
    let params = Params {
        coil_weight: 1.0,
        ..Params::default()
    };
    let on = decide(&observation, &Timing::default(), &params);
    // Counterclockwise around the target: aim ahead and to the left.
    assert!(on.heading > 0.2 && on.heading < 1.2, "{on:?}");
    // Equal masses are not encircled.
    observation.opponents[0].segments = 200;
    assert_eq!(
        decide(&observation, &Timing::default(), &params).heading,
        0.0
    );
}

#[test]
fn a_cut_off_out_of_cruise_reach_boosts_when_allowed() {
    let mut observation = fixture();
    observation.me.score = 100.0;
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let mut opponent = observation.me.clone();
    opponent.id = "opponent".into();
    opponent.segments = 60;
    // Cruise speed cannot cross its path early enough; accelerating to
    // boost speed can.
    opponent.head = Point { x: 100.0, y: 180.0 };
    opponent.heading = -std::f64::consts::FRAC_PI_2;
    opponent.speed = 185.0;
    opponent.body = (1..=8)
        .map(|i| Point {
            x: 100.0,
            y: 180.0 + physics::ANCHOR_SPACING * i as f64,
        })
        .collect();
    observation.opponents.push(opponent);
    let cruise = Params {
        kill_weight: 0.4,
        ..Params::default()
    };
    assert!(!decide(&observation, &Timing::default(), &cruise).boost);
    let boost = Params {
        kill_boost: true,
        ..cruise
    };
    assert!(decide(&observation, &Timing::default(), &boost).boost);
    // Too small to spend mass.
    observation.me.score = 10.0;
    assert!(!decide(&observation, &Timing::default(), &boost).boost);
}

/// A snake at `head` moving along `heading` at cruise speed, its body behind.
fn snake(
    observation: &Observation,
    id: &str,
    head: Point,
    heading: f64,
    segments: usize,
) -> slither_sim::Snake {
    let mut s = observation.me.clone();
    s.id = id.into();
    s.segments = segments;
    s.speed = physics::base_speed(segments);
    s.head = head;
    s.heading = heading;
    s.body = (1..=8)
        .map(|i| Point {
            x: head.x - heading.cos() * physics::ANCHOR_SPACING * i as f64,
            y: head.y - heading.sin() * physics::ANCHOR_SPACING * i as f64,
        })
        .collect();
    s
}

/// A motionless body: a one-anchor wall at `at`.
fn post(observation: &Observation, at: Point) -> slither_sim::Snake {
    let mut s = snake(observation, "post", at, 0.0, 2);
    s.speed = 0.0;
    s.body = vec![Point {
        x: at.x + 10.0,
        y: at.y + 10.0,
    }];
    s
}

fn lcg(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 11) as f64 / (1u64 << 53) as f64
}

#[test]
fn kill_seeking_never_aims_at_an_oncoming_head() {
    // Review repro: head-on, 200 units apart. Arriving first on its path
    // would meet its head, not cross ahead of it.
    let mut observation = fixture();
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let victim = snake(
        &observation,
        "victim",
        Point { x: 200.0, y: 0.0 },
        std::f64::consts::PI,
        2,
    );
    observation.opponents.push(victim);
    let params = Params {
        kill_weight: 1.0,
        ..Params::default()
    };
    assert_eq!(
        decide(&observation, &Timing::default(), &params),
        still(&observation)
    );
}

#[test]
fn boost_cut_offs_follow_the_acceleration_ramp() {
    // Review repro: crossing (190, 0) 0.6 s ahead needs instant top speed;
    // from cruise speed the ramp covers only about 130 units by then.
    let mut observation = fixture();
    observation.me.score = 100.0;
    observation.food = vec![food(400.0, 0.0, 1.0)];
    let mut victim = snake(
        &observation,
        "victim",
        Point { x: 190.0, y: 111.0 },
        -std::f64::consts::FRAC_PI_2,
        60,
    );
    victim.speed = 185.0;
    observation.opponents.push(victim);
    let params = Params {
        kill_weight: 1.0,
        kill_boost: true,
        ..Params::default()
    };
    assert!(!decide(&observation, &Timing::default(), &params).boost);
}

#[test]
fn boost_decisions_gather_every_obstacle_they_check_and_fall_back_to_cruise() {
    // Gathering everything must not change a decision, and a refused boost
    // must leave exactly the cruise-only decision.
    let mut state = 7;
    let mut boosted = 0;
    for case in 0..3000 {
        let mut observation = fixture();
        observation.me.score = 100.0;
        observation.food = vec![food(400.0, 0.0, 1.0)];
        let mut victim = snake(
            &observation,
            "victim",
            Point {
                x: 40.0 + lcg(&mut state) * 200.0,
                y: 60.0 + lcg(&mut state) * 200.0,
            },
            -std::f64::consts::FRAC_PI_2 + (lcg(&mut state) - 0.5),
            60,
        );
        victim.speed = 185.0;
        observation.opponents.push(victim);
        let angle = (lcg(&mut state) - 0.5) * 1.5;
        let distance = 120.0 + lcg(&mut state) * 160.0;
        let at = Point {
            x: angle.cos() * distance,
            y: angle.sin() * distance,
        };
        observation.opponents.push(post(&observation, at));
        for escape_boost in [false, true] {
            let params = Params {
                kill_weight: 1.0,
                kill_boost: true,
                escape_boost,
                ..Params::default()
            };
            let action = decide(&observation, &Timing::default(), &params);
            let everything = Params {
                reach_extra: 1e4,
                ..params
            };
            assert_eq!(
                action,
                decide(&observation, &Timing::default(), &everything),
                "case {case}"
            );
            boosted += usize::from(action.boost);
            if !action.boost && !escape_boost {
                let cruise = Params {
                    kill_boost: false,
                    ..params
                };
                assert_eq!(
                    action,
                    decide(&observation, &Timing::default(), &cruise),
                    "case {case}"
                );
            }
        }
    }
    assert!(boosted > 100, "{boosted}");
}

#[test]
fn coil_orbits_are_no_tighter_than_our_turning_circle() {
    // Review repro: a 430-segment snake turns on a 233-unit radius, wider
    // than the 176-unit orbit the target's size alone asks for. A body
    // that cannot wrap the wider orbit does not coil.
    let mut observation = fixture();
    observation.me.segments = 430;
    observation.me.speed = physics::base_speed(430);
    let ours = observation.me.speed / physics::turn_rate(430);
    let anchors = |n: usize| {
        (1..=n)
            .map(|i| Point {
                x: -physics::ANCHOR_SPACING * i as f64,
                y: 0.0,
            })
            .collect::<Vec<_>>()
    };
    let target = snake(&observation, "small", Point { x: 0.0, y: 150.0 }, 0.0, 5);
    observation.opponents.push(target);
    let params = Params {
        coil_weight: 1.0,
        ..Params::default()
    };
    let wraps = |n: usize| n as f64 * physics::ANCHOR_SPACING;
    let short = 30;
    assert!(wraps(short) > std::f64::consts::TAU * 176.3);
    assert!(wraps(short) < std::f64::consts::TAU * ours);
    observation.me.body = anchors(short);
    assert_eq!(
        decide(&observation, &Timing::default(), &params),
        still(&observation)
    );
    observation.me.body = anchors(40);
    assert_ne!(
        decide(&observation, &Timing::default(), &params),
        still(&observation)
    );
}

#[test]
fn escape_boost_needs_every_cruise_arc_dangerous() {
    // Review repro: the cheapest arc is dangerous, but other cruise arcs
    // are clear, so there is nothing to escape.
    let mut observation = fixture();
    observation.me.score = 100.0;
    let goal: f64 = 1.68204;
    observation.food = vec![food(400.0 * goal.cos(), 400.0 * goal.sin(), 1.0)];
    observation.opponents.push(post(
        &observation,
        Point {
            x: 70.328,
            y: 74.985,
        },
    ));
    let params = Params {
        escape_boost: true,
        ..Params::default()
    };
    assert!(!decide(&observation, &Timing::default(), &params).boost);
}

#[test]
fn steering_off_a_cut_off_heading_respects_the_victim_head_path() {
    // Review repro: the cut heading 0.3558 passes the crossing checks, but a
    // post makes steering pick 0.9558, which approaches the victim's head
    // path at 2.53 rad. Only the validated arc may ignore that path.
    let mut observation = fixture();
    let mut victim = snake(
        &observation,
        "victim",
        Point {
            x: 67.2909,
            y: 173.0063,
        },
        -std::f64::consts::FRAC_PI_2,
        60,
    );
    victim.speed = 185.0;
    observation.opponents.push(victim);
    observation.opponents.push(post(
        &observation,
        Point {
            x: 46.5779,
            y: -21.0958,
        },
    ));
    let params = Params {
        kill_weight: 1.0,
        ..Params::default()
    };
    let action = decide(&observation, &Timing::default(), &params);
    assert!((action.heading - 0.9558).abs() > 1e-3, "{action:?}");
}

#[test]
fn boosted_arcs_bend_with_the_acceleration() {
    // Review repro: boosting toward 1.904 from cruise speed passes a post
    // that one mean speed misses; the accelerating turn hits it.
    let mut observation = fixture();
    observation.me.score = 100.0;
    let h: f64 = 1.904;
    let q = Point {
        x: 300.0 * h.cos(),
        y: 300.0 * h.sin(),
    };
    let vh = h - 1.0;
    let mut victim = snake(
        &observation,
        "victim",
        Point {
            x: q.x - 185.0 * 1.8 * vh.cos(),
            y: q.y - 185.0 * 1.8 * vh.sin(),
        },
        vh,
        60,
    );
    victim.speed = 185.0;
    observation.opponents.push(victim);
    let at = Point {
        x: 47.0438,
        y: 20.1563,
    };
    let mut wall = post(&observation, at);
    wall.body = vec![at];
    observation.opponents.push(wall);
    let params = Params {
        kill_weight: 1.0,
        kill_boost: true,
        kill_samples: 1,
        kill_horizon: 1.8,
        own_width_factor: 0.0,
        safe: 5.0,
        boost_min_score: 0.0,
        ..Params::default()
    };
    let action = decide(&observation, &Timing::default(), &params);
    assert!(
        !(action.boost && (action.heading - h).abs() < 1e-3),
        "{action:?}"
    );
}

#[test]
fn trap_scoring_prefers_the_victim_with_fewer_escapes() {
    // Mirror-image victims are worth the same; bodies beside the second
    // one's turning escapes leave our cut wall closing its last way out.
    let mut observation = fixture();
    let mut open = snake(
        &observation,
        "open",
        Point { x: 60.0, y: 170.0 },
        -std::f64::consts::FRAC_PI_2,
        60,
    );
    open.speed = 185.0;
    let mut trapped = open.clone();
    trapped.id = "trapped".into();
    trapped.head = Point { x: 60.0, y: -170.0 };
    trapped.heading = std::f64::consts::FRAC_PI_2;
    trapped.body = (1..=8)
        .map(|i| Point {
            x: 60.0,
            y: -170.0 - physics::ANCHOR_SPACING * i as f64,
        })
        .collect();
    observation.opponents.push(open);
    observation.opponents.push(trapped);
    for x in [25.0, 95.0] {
        let mut wall = post(&observation, Point { x, y: -100.0 });
        wall.segments = 20;
        wall.body = (1..=5)
            .map(|i| Point {
                x,
                y: -100.0 - physics::ANCHOR_SPACING * i as f64,
            })
            .collect();
        observation.opponents.push(wall);
    }
    let params = Params {
        kill_weight: 1.0,
        ..Params::default()
    };
    let plain = decide(&observation, &Timing::default(), &params);
    assert!(plain.heading > 0.0, "{plain:?}");
    let trap = Params {
        kill_trap_weight: 1.0,
        ..params
    };
    let trapping = decide(&observation, &Timing::default(), &trap);
    assert!(trapping.heading < 0.0, "{trapping:?}");
}
