use crate::{Config, EventRecording, Point, RestoreOptions, SimError, Simulator};

/// Synthetic ring fixture matching TestRing, for tests that append a later slot.
#[allow(dead_code)]
pub fn ring_slot(
    world: &mut Simulator,
    id: impl Into<String>,
    index: usize,
    count: usize,
) -> Result<usize, SimError> {
    assert!(count > 0 && index < count);
    let angle = index as f64 / count as f64 * std::f64::consts::TAU;
    let (body, heading) = if let Some(distance) = world.config().spawn_radius {
        world.spawn_body(
            Point {
                x: angle.cos() * distance,
                y: angle.sin() * distance,
            },
            angle + std::f64::consts::FRAC_PI_2,
        )?
    } else {
        let distance = world.radius() * 0.5;
        let head = Point {
            x: angle.cos() * distance,
            y: angle.sin() * distance,
        };
        (
            vec![
                head,
                Point {
                    x: head.x + angle.cos() * 42.,
                    y: head.y + angle.sin() * 42.,
                },
            ],
            angle + std::f64::consts::PI,
        )
    };
    world.spawn(id.into(), body, heading)
}

/// Rebuild an experimental continuation with changed rules through the validated
/// checkpoint boundary, preserving its RNG, speed, boost and kiosk histories.
#[allow(dead_code)]
pub fn reconfigure(world: &mut Simulator, config: Config) {
    restore_with(world, config, |_| {});
}

/// Move the live radius as a measured update applied now would, keeping the
/// initial Config radius. Checkpoints accept no other move of a fixed arena.
#[allow(dead_code)]
pub fn set_live_radius(world: &mut Simulator, radius: f64) {
    let mut config = world.config().clone();
    let time = world.time();
    assert!(
        config
            .radius_updates
            .iter()
            .all(|update| update.time < time)
    );
    config
        .radius_updates
        .push(crate::RadiusUpdate { time, radius });
    let applied = config.radius_updates.len();
    restore_with(world, config, |state| {
        state["radius"] = serde_json::json!(radius);
        state["radiusUpdateIndex"] = serde_json::json!(applied);
    });
}

fn restore_with(world: &mut Simulator, config: Config, edit: impl FnOnce(&mut serde_json::Value)) {
    let mut checkpoint = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
    let mut resolved = serde_json::to_value(&config).unwrap();
    for key in [
        "pit",
        "spawnRadius",
        "spawnRadiusMax",
        "killScoreSeconds",
        "minScore",
    ] {
        resolved
            .as_object_mut()
            .unwrap()
            .entry(key)
            .or_insert(serde_json::Value::Null);
    }
    resolved
        .as_object_mut()
        .unwrap()
        .entry("ambientRefillCountsAllFood")
        .or_insert(serde_json::json!(false));
    checkpoint["config"] = resolved;
    edit(&mut checkpoint["state"]);
    let options = RestoreOptions {
        events: EventRecording {
            pickups: world.pickup_events().is_some(),
            food_births: world.food_spawn_events().is_some(),
        },
        #[cfg(feature = "parallel")]
        parallel: world.parallel(),
    };
    world
        .restore(&serde_json::to_vec(&checkpoint).unwrap(), options)
        .unwrap();
}

/// Correct observed prey pose/speed while retaining its scheduled decision and mode.
#[allow(dead_code)]
pub fn restore_prey(world: &mut Simulator, index: usize, prey: crate::Prey) {
    let mut checkpoint = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
    let observed = serde_json::to_value(prey).unwrap();
    checkpoint["state"]["prey"][index]
        .as_object_mut()
        .unwrap()
        .extend(observed.as_object().unwrap().clone());
    let options = RestoreOptions {
        events: EventRecording {
            pickups: world.pickup_events().is_some(),
            food_births: world.food_spawn_events().is_some(),
        },
        #[cfg(feature = "parallel")]
        parallel: world.parallel(),
    };
    world
        .restore(&serde_json::to_vec(&checkpoint).unwrap(), options)
        .unwrap();
}
