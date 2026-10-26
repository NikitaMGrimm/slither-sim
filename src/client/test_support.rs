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
    if config.radius != world.config().radius {
        checkpoint["state"]["radius"] = serde_json::json!(config.radius);
    }
    checkpoint["config"] = resolved;
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
