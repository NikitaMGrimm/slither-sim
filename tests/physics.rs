use serde::Deserialize;
use slither_sim::{physics::*, *};
use std::f64::consts::{FRAC_PI_2, PI};

/// Stock-client reconstruction, retained separately from authoritative geometry.
fn append_client_anchor(body: &mut Vec<Point>, anchor: Point, grow: bool) {
    body.insert(0, anchor);
    if !grow {
        body.pop();
    }
    for i in 3..body.len() {
        let weight = 0.43 * ((i - 2) as f64 / 4.0).min(1.0);
        body[i].x += (body[i - 1].x - body[i].x) * weight;
        body[i].y += (body[i - 1].y - body[i].y) * weight;
    }
}

#[derive(Deserialize)]
struct BodyCase {
    before: Vec<Point>,
    after: Vec<Point>,
    grow: bool,
}
#[test]
fn original_client_body_updates_small_and_large() {
    for fixture in [
        include_str!("fixtures/body-updates.json"),
        include_str!("fixtures/body-updates-larger.json"),
    ] {
        let cases: Vec<BodyCase> = serde_json::from_str(fixture).unwrap();
        assert!(cases.len() > 30);
        for c in cases {
            let mut body = c.before;
            body.reverse();
            let mut after = c.after;
            after.reverse();
            append_client_anchor(&mut body, after[0], c.grow);
            assert_eq!(body.len(), after.len());
            for (a, b) in body.iter().zip(after) {
                assert!(a.distance(b) < 1e-6);
            }
        }
    }
}
#[test]
fn independent_food_nutrition() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/food-growth.json")).unwrap();
    for row in fixture["samples"].as_array().unwrap() {
        let predicted = nutrition(
            row["size"].as_f64().unwrap(),
            row["segments"].as_u64().unwrap() as f64 + row["fam"].as_f64().unwrap(),
        );
        let observed = row["delta"].as_f64().unwrap();
        assert!((predicted - observed).abs() / observed < 0.0001);
    }
}
#[test]
fn scale_score_mass_and_chords() {
    assert_eq!(scale(108), 2.0);
    assert_eq!(scale(1000), 6.0);
    assert_eq!(score(2, 0.0), 10.0);
    assert_eq!(score(13, 0.8218), 194.0);
    let (mut n, mut f) = (4, 0.1);
    assert_eq!(remove_mass(&mut n, &mut f, 0.2), 1);
    assert_eq!(n, 3);
    assert!((f - 0.9).abs() < 1e-12);
    remove_mass(&mut n, &mut f, 10.0);
    assert_eq!((n, f), (2, 0.0));
    assert_eq!(
        crossed_anchor(Point { x: 41.0, y: 0.0 }, Point::default()),
        None
    );
    assert_eq!(
        crossed_anchor(Point { x: 43.0, y: 0.0 }, Point::default()),
        Some(Point { x: 42.0, y: 0.0 })
    );
    assert_eq!(
        swept_distance(
            Point { x: -20.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 0.0, y: -10.0 },
            Point { x: 0.0, y: 10.0 }
        ),
        0.0
    );
}

#[test]
fn anchor_projects_current_head_ray_after_curved_crossing() {
    let last = Point::default();
    let head = Point { x: 43.0, y: 3.0 };
    let anchor = crossed_anchor(head, last).unwrap();
    let angle = head.y.atan2(head.x);
    let quantized =
        (angle / std::f64::consts::TAU * 65536.0).floor() / 65536.0 * std::f64::consts::TAU;
    assert!((anchor.x - 42.0 * quantized.cos()).abs() < 1e-12);
    assert!((anchor.y - 42.0 * quantized.sin()).abs() < 1e-12);
    // Interpolating the prior-head/current-head segment onto the circle would
    // place this anchor around y=1.4, instead of the current head ray's y=2.9.
    assert!(anchor.y > 2.8);
}

fn scene() -> Simulator {
    let mut s = Simulator::new(
        42,
        Config {
            food_count: 0,
            radius: 10000.0,
            ..Config::sandbox()
        },
    )
    .unwrap();
    s.add("0", 0, 1).unwrap();
    s.snakes[0].head = Point::default();
    s.snakes[0].body = vec![Point::default(), Point { x: -42.0, y: 0.0 }];
    s.snakes[0].heading = 0.0;
    s
}

fn growth_mass(s: &Snake) -> f64 {
    s.segments as f64 - 2.0 + s.fam
}
#[test]
fn growth_waits_for_anchor() {
    let mut s = scene();
    s.snakes[0].fam = 1.25;
    let a = [Some(Action::default())];
    s.step(&a).unwrap();
    assert_eq!(s.snakes[0].segments, 2);
    while s.snakes[0].head.x < 43.0 {
        s.step(&a).unwrap();
    }
    assert_eq!(s.snakes[0].segments, 3);
    assert_eq!(s.snakes[0].fam, 0.25);
    assert_eq!(s.snakes[0].body.len(), 3);
}
#[test]
fn boost_follows_held_request_and_minimum_mass() {
    let mut s = scene();
    let on = [Some(Action {
        heading: 0.0,
        boost: true,
    })];
    s.step(&on).unwrap();
    assert!(!s.is_boosting(0)); // No mass above the two-segment minimum.
    assert_eq!(s.snakes[0].speed, base_speed(2));
    // The held request starts boosting as soon as mass is available and drops a pellet at once.
    s.snakes[0].fam = 0.6;
    s.step(&on).unwrap();
    assert!(s.is_boosting(0));
    assert!(s.snakes[0].fam < 0.6);
    assert!(s.food.iter().any(|f| f.origin == Some(Origin::Boost)));
    for _ in 0..2000 {
        s.step(&on).unwrap();
    }
    assert_eq!(s.snakes[0].segments, 2);
    assert_eq!(s.snakes[0].fam, 0.0);
    assert!(!s.is_boosting(0));
    assert_eq!(s.snakes[0].speed, base_speed(2));
}
#[test]
fn exhausted_boost_waits_for_release_even_if_food_is_eaten_in_the_same_tick() {
    let mut s = scene();
    s.snakes[0].fam = 0.02;
    let on = [Some(Action {
        heading: 0.0,
        boost: true,
    })];
    // Eat after the final boost debit in the same simulation step.
    s.food.push(Food {
        x: 45.0,
        y: 0.0,
        size: Some(8.0),
        value: 0.0,
        origin: Some(Origin::Ambient),
    });
    s.step(&on).unwrap();
    assert!(s.snakes[0].fam > 0.02);
    assert!(!s.is_boosting(0));
    let spent = s.boost_accounting(0).unwrap();
    for _ in 0..100 {
        s.step(&on).unwrap();
    }
    assert!(!s.is_boosting(0));
    assert_eq!(s.boost_accounting(0).unwrap(), spent);
    s.step(&[Some(Action::default())]).unwrap();
    s.step(&on).unwrap();
    assert!(s.is_boosting(0));
    assert!(s.boost_accounting(0).unwrap().boost_mass_spent > spent.boost_mass_spent);
    // A respawn clears the exhausted state along with the rest of the life state.
    s.snakes[0].fam = 0.001;
    for _ in 0..30 {
        s.step(&on).unwrap();
    }
    s.snakes[0].alive = false;
    s.respawn(0, Point::default(), 0.0, 2).unwrap();
    s.snakes[0].fam = 0.6;
    s.step(&on).unwrap();
    assert!(s.is_boosting(0));
}

#[test]
fn boost_drops_follow_the_measured_schedule_and_cost() {
    let mut s = scene();
    s.snakes[0].fam = 0.9;
    let on = [Some(Action {
        heading: 0.0,
        boost: true,
    })];
    let mut drops = vec![];
    for tick in 0..400 {
        let before = s.snakes[0].fam;
        s.step(&on).unwrap();
        // The final drop is capped by the remaining growth mass.
        if s.snakes[0].fam < before && before > 0.1 {
            drops.push((tick, before - s.snakes[0].fam));
        }
    }
    assert!(drops.len() >= 10);
    for w in drops.windows(2) {
        let gap = (w[1].0 - w[0].0) as f64 * s.config.dt;
        assert!((gap - BOOST_DROP_SECONDS).abs() <= s.config.dt + 1e-9);
    }
    for (_, cost) in &drops {
        assert!((0.94 * boost_cost(2)..=1.08 * boost_cost(2)).contains(cost));
    }
}
#[test]
fn acceleration_is_size_dependent_and_braking_is_faster() {
    let small = approach_speed(0.0, 1e9, 2, 1.0);
    let large = approach_speed(0.0, 1e9, 320, 1.0);
    assert!((small - 15.39375 / 0.032).abs() < 1e-9);
    assert!(large < small * 0.5);
    assert!((approach_speed(1e9, 0.0, 2, 1.0) - (1e9 - small * 4.0 / 3.0)).abs() < 1e-3);
    for segments in [2, 100, 320, 430] {
        let base = base_speed(segments);
        assert_eq!(
            approach_speed(base, BOOST_SPEED, segments, 10.0),
            BOOST_SPEED
        );
        assert_eq!(approach_speed(BOOST_SPEED, base, segments, 10.0), base);
        let direct = approach_speed(base, BOOST_SPEED, segments, 0.2);
        let half = approach_speed(base, BOOST_SPEED, segments, 0.1);
        assert!((approach_speed(half, BOOST_SPEED, segments, 0.1) - direct).abs() < 1e-10);
    }
}

#[test]
fn independently_recorded_speed_ramps() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/speed-ramp-validation.json")).unwrap();
    let ramps = fixture["ramps"].as_array().unwrap();
    assert_eq!(ramps.len(), 1567);
    let mut errors = std::collections::BTreeMap::<String, (usize, f64, f64)>::new();
    for row in ramps {
        let segments = row["segments"].as_u64().unwrap() as usize;
        let points = row["points"].as_array().unwrap();
        let n = points.len() as f64;
        let mean_t = points
            .iter()
            .map(|p| p[0].as_f64().unwrap() / 1000.0)
            .sum::<f64>()
            / n;
        let mean_speed = points.iter().map(|p| p[1].as_f64().unwrap()).sum::<f64>() / n;
        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for point in points {
            let dt = point[0].as_f64().unwrap() / 1000.0 - mean_t;
            numerator += dt * (point[1].as_f64().unwrap() - mean_speed);
            denominator += dt * dt;
        }
        let observed = numerator / denominator;
        let up = row["up"].as_bool().unwrap();
        // Query the real simulation helper far from clipping at the target.
        let current = 10.0 / 0.032;
        let target = if up {
            BOOST_SPEED
        } else {
            base_speed(segments)
        };
        let rate = (approach_speed(current, target, segments, 0.001) - current) * 0.032 / 0.001;
        let old =
            15.45 / (1.0 + 0.39 * (scale(segments) - 1.0)) * if up { 1.0 } else { -4.0 / 3.0 };
        let entry = errors
            .entry(row["source"].as_str().unwrap().to_owned())
            .or_default();
        entry.0 += 1;
        entry.1 += (rate - observed).powi(2);
        entry.2 += (old - observed).powi(2);
    }
    assert_eq!(errors.len(), 3);
    for (source, (count, sum, old_sum)) in errors {
        let rmse = (sum / count as f64).sqrt();
        assert!(rmse < 0.12, "{source}: speed slope RMSE {rmse}");
        assert!(
            sum < old_sum,
            "{source}: must improve on the previous empirical law"
        );
    }
}
#[test]
fn steering_uses_the_one_byte_heading() {
    let step = std::f64::consts::TAU / 251.0;
    assert_eq!(quantize_heading(0.5 * step), 0.0);
    assert!((quantize_heading(3.7 * step) - 3.0 * step).abs() < 1e-12);
    assert!((quantize_heading(-0.5 * step) - 250.0 * step).abs() < 1e-12);
}

#[test]
fn boost_accounting_counts_active_ticks_and_exact_growth_mass() {
    let mut s = scene();
    s.snakes[0].fam = 0.6;
    let initial_mass = growth_mass(&s.snakes[0]);
    let on = [Some(Action {
        heading: 0.0,
        boost: true,
    })];

    s.step(&on).unwrap();

    let accounting = s.boost_accounting(0).unwrap();
    assert_eq!(accounting.active_boost_seconds, s.config.dt);
    assert!(accounting.boost_mass_spent > 0.0);
    assert!(
        (accounting.boost_mass_spent - (initial_mass - growth_mass(&s.snakes[0]))).abs() < 1e-12
    );
}

#[test]
fn boost_release_stops_further_accounting() {
    let mut s = scene();
    s.snakes[0].fam = 0.6;
    let on = [Some(Action {
        heading: 0.0,
        boost: true,
    })];
    s.step(&on).unwrap();
    let before_release = s.boost_accounting(0).unwrap();

    s.step(&[Some(Action::default())]).unwrap();
    for _ in 0..30 {
        s.step(&[Some(Action::default())]).unwrap();
    }

    assert_eq!(s.boost_accounting(0).unwrap(), before_release);
    assert!(!s.is_boosting(0));
}

#[test]
fn boost_exhaustion_counts_the_final_active_tick_and_respawn_resets_it() {
    let mut s = scene();
    s.snakes[0].fam = 0.02;
    let initial_mass = growth_mass(&s.snakes[0]);
    let on = [Some(Action {
        heading: 0.0,
        boost: true,
    })];

    s.step(&on).unwrap();

    let accounting = s.boost_accounting(0).unwrap();
    assert_eq!(accounting.active_boost_seconds, s.config.dt);
    assert_eq!(accounting.boost_mass_spent, initial_mass);
    assert_eq!(growth_mass(&s.snakes[0]), 0.0);
    assert!(!s.is_boosting(0));

    s.snakes[0].alive = false;
    s.respawn(0, Point::default(), 0.0, 2).unwrap();
    assert_eq!(s.boost_accounting(0).unwrap(), BoostAccounting::default());
}
#[test]
fn invalid_batches_do_not_mutate_and_observations_are_detached() {
    let mut s = scene();
    let before = s.snakes.clone();
    assert!(s.step(&[]).is_err());
    assert!(
        s.step(&[Some(Action {
            heading: f64::NAN,
            boost: false
        })])
        .is_err()
    );
    assert_eq!(s.snakes, before);
    assert_eq!(s.time, 0.0);
    let mut observation = s.observe(0).unwrap();
    observation.me.body[0].x = 12345.0;
    assert_eq!(s.snakes, before);
}
#[test]
fn collision_death_kill_and_food_ties() {
    let mut s = scene();
    s.add("1", 0, 1).unwrap();
    s.snakes[1].head = Point { x: 210.0, y: 20.0 };
    s.snakes[1].heading = 0.0;
    s.snakes[1].segments = 7;
    s.snakes[1].head.y = 35.0;
    s.snakes[1].body = [210.0, 168.0, 126.0, 84.0, 42.0, 0.0, -42.0]
        .map(|x| Point { x, y: 35.0 })
        .to_vec();
    let a = [Some(Action::default()); 2];
    s.step(&a).unwrap();
    assert!(s.snakes[0].alive);
    // Alongside the body, 25 apart center to center: overlapping but harmless.
    s.snakes[0].head.y = 10.0;
    s.step(&a).unwrap();
    assert!(s.snakes[0].alive);
    // Turned into the body, the head's tip strikes it.
    s.snakes[0].heading = FRAC_PI_2;
    let up = Action {
        heading: FRAC_PI_2,
        boost: false,
    };
    s.step(&[Some(up), Some(Action::default())]).unwrap();
    assert!(!s.snakes[0].alive);
    assert_eq!(s.snakes[1].kills, 1);
    assert_eq!(s.deaths[0].killer.as_deref(), Some("1"));
    assert!(s.food.iter().any(|f| f.origin == Some(Origin::Death)));
    let mut s = Simulator::new(
        1,
        Config {
            food_count: 0,
            ..Config::sandbox()
        },
    )
    .unwrap();
    for (id, y) in [("b", -20.0), ("a", 20.0)] {
        let i = s.add(id, 0, 1).unwrap();
        s.snakes[i].head = Point { x: 0.0, y };
        s.snakes[i].body = vec![Point { x: 0.0, y }, Point { x: -42.0, y }];
        s.snakes[i].heading = 0.0;
    }
    s.food.push(Food {
        // Equidistant from both pickup circles, which lead the heads by ~43 units.
        x: base_speed(2) * 0.032 * 7.4,
        y: 0.0,
        size: Some(5.0),
        value: 0.0,
        origin: None,
    });
    s.step(&a).unwrap();
    assert_eq!(s.snakes[0].fam, 0.0);
    assert!(s.snakes[1].fam > 0.0);
}

#[derive(Deserialize)]
struct CollisionCases {
    head_segments: usize,
    body_segments: usize,
    last_tip: [f64; 2],
    tip: [f64; 2],
    heading: f64,
    cases: Vec<CollisionCase>,
}
#[derive(Deserialize)]
struct CollisionCase {
    name: String,
    line: Vec<[f64; 2]>,
    strikes: bool,
}

#[test]
fn official_collision_geometry_spares_side_contact() {
    let fixture: CollisionCases =
        serde_json::from_str(include_str!("fixtures/official-collision-cases.json")).unwrap();
    let point = |[x, y]: [f64; 2]| Point { x, y };
    for case in fixture.cases {
        let hit = strikes(
            point(fixture.last_tip),
            point(fixture.tip),
            fixture.heading,
            fixture.head_segments,
            fixture.body_segments,
            case.line.into_iter().map(point),
        );
        assert_eq!(hit, case.strikes, "{}", case.name);
    }
}

/// Two small snakes meet head on; returns the victims and killers.
fn head_on(respawn_first: bool) -> Vec<(String, Option<String>)> {
    let mut s = scene();
    s.add("1", 0, 1).unwrap();
    if respawn_first {
        s.snakes[0].alive = false;
        s.respawn(0, Point::default(), 0.0, 2).unwrap();
    }
    s.snakes[1].head = Point { x: 60.0, y: 1.0 };
    s.snakes[1].body = vec![Point { x: 60.0, y: 1.0 }, Point { x: 102.0, y: 1.0 }];
    s.snakes[1].heading = PI;
    let a = [
        Some(Action::default()),
        Some(Action {
            heading: PI,
            boost: false,
        }),
    ];
    while s.deaths.is_empty() {
        s.step(&a).unwrap();
    }
    s.deaths
        .iter()
        .map(|d| (d.victim.clone(), d.killer.clone()))
        .collect()
}

#[test]
fn head_on_contact_kills_only_the_earlier_joined_snake() {
    assert_eq!(head_on(false), [("0".into(), Some("1".into()))]);
    // A respawned life joins after every current one.
    assert_eq!(head_on(true), [("1".into(), Some("0".into()))]);
}

#[test]
fn collision_bounds_follow_sparse_slots_and_respawn_join_order() {
    for respawn_first in [false, true] {
        let mut world = scene();
        world.add("unused", 0, 1).unwrap();
        world.add("2", 0, 1).unwrap();
        world.snakes[1].alive = false;
        if respawn_first {
            world.snakes[0].alive = false;
            world.respawn(0, Point::default(), 0.0, 2).unwrap();
        }
        world.snakes[2].head = Point { x: 60.0, y: 1.0 };
        world.snakes[2].body = vec![Point { x: 60.0, y: 1.0 }, Point { x: 102.0, y: 1.0 }];
        world.snakes[2].heading = PI;
        let actions = [
            Some(Action::default()),
            None,
            Some(Action {
                heading: PI,
                boost: false,
            }),
        ];
        for _ in 0..100 {
            world.step(&actions).unwrap();
            if !world.deaths.is_empty() {
                break;
            }
        }
        let expected = if respawn_first {
            ("2", "0")
        } else {
            ("0", "2")
        };
        assert_eq!(world.deaths.len(), 1);
        assert_eq!(world.deaths[0].victim, expected.0);
        assert_eq!(world.deaths[0].killer.as_deref(), Some(expected.1));
    }
}
