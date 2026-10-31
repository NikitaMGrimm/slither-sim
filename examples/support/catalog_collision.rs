//! Synthetic coverage for the dense collision grid and its grown-head queries.
use super::*;

fn fixture(kind: EngineWorld) -> Result<Simulator, String> {
    let mut world = runtime::ring(SEED, kind.config(), 4096)?;
    let mut snakes = world.snakes().to_vec();
    for (slot, snake) in snakes.iter_mut().enumerate() {
        snake.head = Point {
            x: (slot % 64) as f64 * 512. - 16128.,
            y: (slot / 64) as f64 * 512. - 16128.,
        };
        snake.segments = if slot == 0 { 532 } else { 8 };
        snake.body = (0..snake.segments)
            .map(|anchor| Point {
                x: snake.head.x - anchor as f64 * 42.,
                y: snake.head.y,
            })
            .collect();
        snake.heading = 0.;
        snake.speed = 0.;
        snake.fam = 0.;
    }
    runtime::initial_snakes(&mut world, snakes)?;
    runtime::record_events(&mut world);
    Ok(world)
}

pub(super) fn run(args: &Args, kind: EngineWorld, out: &mut impl Write) -> Result<Value, String> {
    let mut world = fixture(kind)?;
    let initial = WorldStats::from_world(&world);
    let actions = vec![
        Some(Action {
            heading: 0.,
            boost: false
        });
        4096
    ];
    let mut meter = Meter::default();
    let mut profile = StepProfile::default();
    if args.trace {
        line(
            out,
            json!({"kind":"initial", "case":args.case, "snapshot":world_events(&world)?}),
        )?;
        line(out, json!({"kind":"warmed", "ticks":0}))?;
    }
    for iteration in 0..args.iterations {
        if iteration > 0 && iteration % kind.epoch_ticks() == 0 {
            world = fixture(kind)?;
            if args.trace {
                line(
                    out,
                    json!({"kind":"epoch", "iteration":iteration, "snapshot":world_events(&world)?}),
                )?;
            }
        }
        meter.operation(args.trace, || {
            if args.profile {
                world.step_profiled(&actions, &mut profile)
            } else {
                world.step(&actions)
            }
            .map_err(|e| e.to_string())
        })?;
        // A death would drop below the grid threshold and measure different work.
        if world.living() != 4096 {
            return Err("collision-grid fixture lost a living snake".into());
        }
        if args.trace {
            line(
                out,
                json!({"kind":"step", "iteration":iteration, "snapshot":world_events(&world)?}),
            )?;
        }
    }
    if args.trace {
        line(
            out,
            json!({"kind":"end", "case":args.case, "operations":args.iterations, "living":world.living()}),
        )?;
    }
    Ok(report(
        args,
        meter,
        "step",
        json!({"initial":initial,
        "final":WorldStats::from_world(&world), "minimumLiving":4096,
        "epochTicks":kind.epoch_ticks(), "synthetic":kind.synthetic(),
        "profile":if args.profile {Some(profile)} else {None}}),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grown_grid_fixture_stays_above_dispatch_threshold() {
        let mut world = fixture(EngineWorld::CollisionGrid).unwrap();
        assert_eq!(world.snakes()[0].segments, 532);
        let actions = vec![
            Some(Action {
                heading: 0.,
                boost: false
            });
            4096
        ];
        for _ in 0..EngineWorld::CollisionGrid.epoch_ticks() {
            world.step(&actions).unwrap();
            assert_eq!(world.living(), 4096);
        }
    }
}
