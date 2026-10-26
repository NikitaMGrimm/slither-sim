use slither_sim::{
    Constraint, CoreStepError, ErrorCode, ErrorKind, SimError, transport::StepError,
};
use std::error::Error;

#[test]
fn rejected_batches_and_headings_preserve_exact_state() {
    use slither_sim::{Action, Config, Simulator, WorldStatus, profiling::StepProfile};
    for profiled in [false, true] {
        let mut world = Simulator::new(7, Config::sandbox()).unwrap();
        world.add("a", 0, 1).unwrap();
        world.pickup_events = Some(Vec::new());
        world.food_spawn_events = Some(Vec::new());
        let before = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
        let events = (world.pickup_events.clone(), world.food_spawn_events.clone());
        let mut profile = StepProfile::default();
        for (actions, kind) in [
            (
                vec![],
                ErrorKind::ActionShape {
                    expected: 1,
                    actual: 0,
                },
            ),
            (
                vec![None],
                ErrorKind::InvalidAction {
                    rule: Constraint::Missing,
                },
            ),
            (
                vec![Some(Action {
                    heading: f64::NAN,
                    boost: false,
                })],
                ErrorKind::InvalidAction {
                    rule: Constraint::NonFinite,
                },
            ),
            (
                vec![Some(Action {
                    heading: f64::INFINITY,
                    boost: false,
                })],
                ErrorKind::InvalidAction {
                    rule: Constraint::NonFinite,
                },
            ),
        ] {
            let result = if profiled {
                world.step_profiled(&actions, &mut profile)
            } else {
                world.step(&actions)
            };
            let CoreStepError::Rejected(cause) = result.unwrap_err() else {
                panic!("expected rejection")
            };
            assert_eq!(cause.kind, kind);
            if matches!(kind, ErrorKind::InvalidAction { .. }) {
                assert_eq!(
                    (cause.slot, cause.field.as_deref()),
                    (Some(0), Some("heading"))
                );
            }
            assert_eq!(
                serde_json::to_value(world.checkpoint().unwrap()).unwrap(),
                before
            );
            assert_eq!(
                (&world.pickup_events, &world.food_spawn_events),
                (&events.0, &events.1)
            );
            assert_eq!(profile.ticks, 0);
            assert_eq!(world.status(), WorldStatus::Running);
        }
    }
}

#[test]
fn food_overflow_completes_one_tick_and_retries_preserve_all_state() {
    use slither_sim::{
        Action, Config, Food, Simulator, WorldStatus, physics, profiling::StepProfile,
    };
    let mut reference = None;
    for profiled in [false, true] {
        let mut world = Simulator::new(
            7,
            Config {
                food_count: 0,
                ambient_replenish_per_second: 0.,
                ..Config::sandbox()
            },
        )
        .unwrap();
        world.add("a", 0, 1).unwrap();
        world.pickup_events = Some(Vec::new());
        world.food_spawn_events = Some(Vec::new());
        let snake = &world.snakes[0];
        let heading = snake.heading;
        let (point, _) = physics::pickup_circle(snake.head, heading, snake.speed, snake.segments);
        world.food = vec![
            Food {
                x: point.x,
                y: point.y,
                value: f64::MAX,
                origin: None,
                size: None
            };
            2
        ];
        let actions = [Some(Action {
            heading,
            boost: false,
        })];
        let mut profile = StepProfile::default();
        let result = if profiled {
            world.step_profiled(&actions, &mut profile)
        } else {
            world.step(&actions)
        };
        let CoreStepError::Stopped(cause) = result.unwrap_err() else {
            panic!("expected a completed stopped tick")
        };
        assert_eq!(cause.kind, ErrorKind::NumericOverflow);
        assert_eq!((cause.slot, cause.field.as_deref()), (Some(0), Some("fam")));
        assert_eq!(world.status(), WorldStatus::Stopped(&cause));
        assert_eq!(world.time, world.config.dt);
        assert_eq!(world.snakes[0].fam, f64::MAX);
        assert!(world.food.is_empty());
        assert_eq!(world.pickup_events.as_ref().unwrap().len(), 2);
        let before = serde_json::to_value(world.checkpoint().unwrap()).unwrap();
        let events = (world.pickup_events.clone(), world.food_spawn_events.clone());
        let ticks = profile.ticks;
        let result = if profiled {
            world.step_profiled(&actions, &mut profile)
        } else {
            world.step(&actions)
        };
        assert!(
            matches!(result, Err(CoreStepError::Rejected(ref e)) if e.kind == ErrorKind::WorldStopped)
        );
        assert_eq!(world.status(), WorldStatus::Stopped(&cause));
        assert_eq!(
            serde_json::to_value(world.checkpoint().unwrap()).unwrap(),
            before
        );
        assert_eq!(
            (world.pickup_events.clone(), world.food_spawn_events.clone()),
            events
        );
        assert_eq!(profile.ticks, ticks);
        if let Some(reference) = &reference {
            assert_eq!(&before, reference);
        } else {
            reference = Some(before);
        }
    }
}

#[test]
fn stable_cause_codes() {
    let rule = Constraint::InconsistentState;
    for (kind, code, number, name) in [
        (
            ErrorKind::InvalidRequest,
            ErrorCode::InvalidRequest,
            1,
            "invalid_request",
        ),
        (
            ErrorKind::InvalidConfig { rule },
            ErrorCode::InvalidConfig,
            2,
            "invalid_config",
        ),
        (
            ErrorKind::ActionShape {
                expected: 2,
                actual: 1,
            },
            ErrorCode::ActionShape,
            3,
            "action_shape",
        ),
        (
            ErrorKind::InvalidAction { rule },
            ErrorCode::InvalidAction,
            4,
            "invalid_action",
        ),
        (
            ErrorKind::UnknownSlot,
            ErrorCode::UnknownSlot,
            5,
            "unknown_slot",
        ),
        (
            ErrorKind::UnknownWorld,
            ErrorCode::UnknownWorld,
            6,
            "unknown_world",
        ),
        (
            ErrorKind::UnknownEntity,
            ErrorCode::UnknownEntity,
            7,
            "unknown_entity",
        ),
        (
            ErrorKind::DuplicateId,
            ErrorCode::DuplicateId,
            8,
            "duplicate_id",
        ),
        (
            ErrorKind::InvalidState { rule },
            ErrorCode::InvalidState,
            9,
            "invalid_state",
        ),
        (
            ErrorKind::PlacementExhausted { attempts: 64 },
            ErrorCode::PlacementExhausted,
            10,
            "placement_exhausted",
        ),
        (ErrorKind::NotReset, ErrorCode::NotReset, 11, "not_reset"),
        (
            ErrorKind::WorldStopped,
            ErrorCode::WorldStopped,
            12,
            "world_stopped",
        ),
        (
            ErrorKind::UnsupportedCheckpoint {
                format: "future".into(),
                rules_revision: "future".into(),
            },
            ErrorCode::UnsupportedCheckpoint,
            13,
            "unsupported_checkpoint",
        ),
        (
            ErrorKind::InvalidCheckpoint { rule },
            ErrorCode::InvalidCheckpoint,
            14,
            "invalid_checkpoint",
        ),
        (
            ErrorKind::NumericOverflow,
            ErrorCode::NumericOverflow,
            15,
            "numeric_overflow",
        ),
        (
            ErrorKind::RepresentationLimit,
            ErrorCode::RepresentationLimit,
            16,
            "representation_limit",
        ),
        (
            ErrorKind::AllocationFailed,
            ErrorCode::AllocationFailed,
            17,
            "allocation_failed",
        ),
    ] {
        let cause = SimError::new(kind, "diagnostic");
        assert_eq!(cause.code(), code);
        assert_eq!(code as u32, number);
        assert_eq!(code.as_str(), name);
    }
}

#[test]
fn display_and_error_sources_keep_wire_diagnostics() {
    let cause = SimError::new(
        ErrorKind::InvalidAction {
            rule: Constraint::Missing,
        },
        "Missing or invalid action for 3",
    )
    .at_world(1)
    .at_slot(3)
    .at_field("heading");
    assert_eq!(
        cause.to_string(),
        "World 1: Missing or invalid action for 3"
    );
    let rejected = CoreStepError::Rejected(cause.clone());
    assert_eq!(rejected.to_string(), cause.to_string());
    assert_eq!(
        rejected.source().unwrap().downcast_ref::<SimError>(),
        Some(&cause)
    );
    let stopped = CoreStepError::Stopped(cause.clone());
    assert_eq!(
        stopped.source().unwrap().downcast_ref::<SimError>(),
        Some(&cause)
    );
    let ended = StepError::Ended(cause.clone());
    assert_eq!(
        ended.to_string(),
        "World 1: Missing or invalid action for 3; the session has ended, reset to continue"
    );
    assert_eq!(
        ended.source().unwrap().downcast_ref::<SimError>(),
        Some(&cause)
    );
    assert_eq!(
        StepError::Rejected(cause.clone()).to_string(),
        cause.to_string()
    );
}
