use slither_sim::{physics::*, *};

#[test]
fn death_density_settings_validate_and_round_trip() {
    let mut config = Config::default();
    config.death_food.cell_size = 0.0;
    assert!(Simulator::new(1, config).is_err());

    let mut old = serde_json::to_value(Config::default()).unwrap();
    old.as_object_mut().unwrap().remove("deathFood");
    let restored: Config = serde_json::from_value(old).unwrap();
    assert!(restored.death_food.enabled);
}

/// A vertical snake at `x` whose body spans y = -100..300 and whose head, far from
/// the origin, moves away along +y.
fn lay_across(world: &mut Simulator, index: usize, x: f64) {
    let s = &mut world.snakes[index];
    s.head = Point { x, y: 342. };
    s.heading = std::f64::consts::FRAC_PI_2;
    s.body = (0..11)
        .map(|i| Point {
            x,
            y: 300. - 42. * i as f64,
        })
        .collect();
    s.segments = s.body.len();
    Simulator::update_mass(s);
}

#[test]
fn collision_reports_actual_retained_food_for_default_tuned_and_disabled_limits() {
    for (enabled, capacity, expected) in [(true, 22, 22), (true, 7, 7), (false, 22, 61)] {
        let mut world = Simulator::new(
            7,
            Config {
                food_count: 0,
                ambient_replenish_per_second: 0.,
                death_food: DeathFoodConfig {
                    enabled,
                    max_per_cell: capacity,
                    ..DeathFoodConfig::default()
                },
                ..Config::default()
            },
        )
        .unwrap();
        world.add("victim", 0, 1).unwrap();
        world.add("obstacle", 0, 1).unwrap();
        let center = Point { x: 37.5, y: 37.5 };
        world.snakes[0].head = center;
        world.snakes[0].body = vec![center; 30];
        world.snakes[0].segments = 30;
        Simulator::update_mass(&mut world.snakes[0]);
        // The victim faces -x; a long body crosses just ahead of its tip.
        lay_across(&mut world, 1, center.x - 25.);
        world
            .step(&[Some(Action::default()), Some(Action::default())])
            .unwrap();
        assert!(!world.snakes[0].alive);
        assert!(world.snakes[1].alive);
        assert_eq!(world.deaths.len(), 1);
        assert_eq!(world.deaths[0].food, expected);
        assert_eq!(world.food.len(), expected);
    }
}

#[test]
fn death_scatter_matches_recorded_square_support_and_tail_first_order() {
    let mut corner_count = 0;
    for seed in 0..128 {
        let mut world = Simulator::new(
            seed,
            Config {
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::default()
            },
        )
        .unwrap();
        world.add("victim", 0, 1).unwrap();
        world.add("obstacle", 0, 1).unwrap();
        world.snakes[0].head = Point::default();
        world.snakes[0].heading = 0.;
        world.snakes[0].body = vec![Point::default(), Point { x: -42., y: 0. }];
        // A long body crosses just ahead of the victim's tip.
        lay_across(&mut world, 1, 25.);
        let mut cloned = world.clone();
        world
            .step(&[Some(Action::default()), Some(Action::default())])
            .unwrap();
        cloned
            .step(&[Some(Action::default()), Some(Action::default())])
            .unwrap();
        assert_eq!(world.food, cloned.food);
        assert!(!world.snakes[0].alive);
        assert!(world.snakes[1].alive);
        let victim = &world.snakes[0];
        let mut sites: Vec<_> = death_food_sites(&victim.body, victim.head).collect();
        sites.reverse();
        assert_eq!(world.food.len(), sites.len());
        let sc = scale(victim.segments);
        for (pellet, site) in world.food.iter().zip(sites) {
            let dx = (pellet.x + 300. / 512. - site.x) / sc;
            let dy = (pellet.y + 300. / 512. - site.y) / sc;
            assert!(dx.abs().max(dy.abs()) <= 15. + 300. / 256. / sc);
            corner_count += usize::from(dx.hypot(dy) > 18.);
        }
    }
    // The official packet fixture contains square-corner offsets that a disk
    // cannot generate, even allowing a complete food-grid cell of error.
    assert!(corner_count > 8, "square-corner samples: {corner_count}");
}

#[test]
fn raw_death_fixture_contains_square_corners_beyond_the_former_disk() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/death-jitter-fresh.json")).unwrap();
    let sc = scale(fixture["segments"].as_u64().unwrap() as usize);
    let grid = fixture["sector_size"].as_f64().unwrap() / 256.;
    let mut corners = 0;
    let mut tail_first_distance = 0.;
    let mut head_first_distance = 0.;
    let pellets = fixture["pellets"].as_array().unwrap();
    let sites = fixture["sites_tail_to_head"].as_array().unwrap();
    assert_eq!(pellets.len(), sites.len());
    for (pellet, site) in fixture["pellets"]
        .as_array()
        .unwrap()
        .iter()
        .zip(fixture["sites_tail_to_head"].as_array().unwrap())
    {
        let dx =
            (pellet["position"][0].as_f64().unwrap() + grid / 2. - site[0].as_f64().unwrap()) / sc;
        let dy =
            (pellet["position"][1].as_f64().unwrap() + grid / 2. - site[1].as_f64().unwrap()) / sc;
        assert!(dx.abs().max(dy.abs()) <= 15. + grid / sc);
        corners += usize::from(dx.hypot(dy) > 17. + grid / sc);
    }
    assert_eq!(corners, 2);
    for (index, pellet) in pellets.iter().enumerate() {
        let x = pellet["position"][0].as_f64().unwrap();
        let y = pellet["position"][1].as_f64().unwrap();
        let distance = |site: &serde_json::Value| {
            (x - site[0].as_f64().unwrap()).hypot(y - site[1].as_f64().unwrap())
        };
        tail_first_distance += distance(&sites[index]);
        head_first_distance += distance(&sites[sites.len() - index - 1]);
    }
    assert!(tail_first_distance < head_first_distance / 4.);
}

#[test]
fn measured_sizes_including_held_out_deaths_match_the_quantized_fit() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/death-food.json")).unwrap();
    let mut held_out = 0;
    for row in f["sizes"].as_array().unwrap() {
        let n = row["segments"].as_u64().unwrap() as usize;
        assert_eq!(death_food_size(n), row["size"].as_f64().unwrap(), "{n}");
        held_out += usize::from(row["held_out"].as_bool().unwrap());
    }
    assert!(held_out >= 5);
}

#[test]
fn full_death_counts_match_own_samples_and_compact_exceptions_remain_visible() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/death-food.json")).unwrap();
    let mut complete = 0;
    let mut compact = 0;
    for row in f["own_deaths"].as_array().unwrap() {
        let body: Vec<Point> = serde_json::from_value(row["body"].clone()).unwrap();
        let head: Point = serde_json::from_value(row["head"].clone()).unwrap();
        let predicted = death_food_sites(&body, head).count();
        let received = row["pellets"].as_array().unwrap().len();
        if row["compact_count_gap"] == true {
            compact += 1;
            assert!(predicted > received); // Known mismatch, not a fitted success.
        } else {
            complete += 1;
            assert_eq!(predicted, received);
        }
    }
    assert_eq!(complete, 5);
    assert_eq!(compact, 2);
}

#[test]
fn death_sites_preserve_body_density_and_size_is_used_for_nutrition() {
    let body = [Point { x: 20., y: 0. }, Point { x: 18., y: 0. }];
    let sites: Vec<_> = death_food_sites(&body, Point { x: 62., y: 0. }).collect();
    assert_eq!(
        sites.iter().map(|p| p.x).collect::<Vec<_>>(),
        vec![62., 41., 20., 19., 18.]
    );
    assert!(nutrition(death_food_size(244), 2.0) > nutrition(death_food_size(32), 2.0));
}

#[test]
fn prospective_deaths_validate_sizes_and_keep_count_gaps_explicit() {
    let f: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/death-food-prospective.json")).unwrap();
    let (mut complete, mut gaps, mut boundary) = (0, 0, 0);
    for row in f["own_deaths"].as_array().unwrap() {
        let n = row["segments"].as_u64().unwrap() as usize;
        let count = row["count"].as_u64().unwrap() as usize;
        if row["boundary_candidate"] == true {
            boundary += 1;
            assert_eq!(count, 0);
            continue;
        }
        let sizes = row["sizes"].as_object().unwrap();
        assert_eq!(sizes.len(), 1);
        let size = sizes.keys().next().unwrap().parse::<f64>().unwrap();
        assert_eq!(death_food_size(n), size);
        if count == 2 * n + 1 {
            complete += 1;
        } else {
            gaps += 1;
            assert!(count < 2 * n + 1); // Unresolved receipt/geometry gaps, not fitted successes.
        }
        if row["score"] == 4273 {
            assert_eq!((n, count, size), (165, 331, 14.2));
        }
    }
    assert_eq!((complete, gaps, boundary), (30, 5, 4));
}
