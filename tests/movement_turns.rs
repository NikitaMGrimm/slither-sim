use serde::Deserialize;
use slither_sim::{
    Point,
    physics::{BOOST_SPEED, advance, base_speed, turn_rate},
};

#[derive(Deserialize)]
struct Fixture {
    method: String,
    sources: Vec<Source>,
    runs: Vec<TurnRun>,
}

#[derive(Deserialize)]
struct Source {
    id: String,
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
struct TurnRun {
    source: String,
    connection: u32,
    snake_id: u32,
    generation: u32,
    first_center_anchor_index: usize,
    last_center_anchor_index: usize,
    first_center_server_ms: u64,
    last_center_server_ms: u64,
    segments: usize,
    mode: String,
    direction: String,
    samples: usize,
    duration_seconds: f64,
    angle_delta_radians: f64,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/movement-turn-runs.json")).unwrap()
}

/// Exercise the same public turn integration used by the simulator, with a
/// continually distant wanted heading so the maximum-rate branch stays active.
fn simulated_angle(run: &TurnRun) -> f64 {
    let direction = if run.direction == "left" { 1.0 } else { -1.0 };
    let speed = if run.mode == "boost" {
        BOOST_SPEED
    } else {
        base_speed(run.segments)
    };
    let mut head = Point::default();
    let mut heading = 0.0;
    let mut remaining = run.duration_seconds;
    while remaining > 0.0 {
        let dt = remaining.min(0.007);
        (head, heading) = advance(
            head,
            heading,
            heading + direction * 1.2,
            speed,
            turn_rate(run.segments),
            dt,
        );
        remaining -= dt;
    }
    heading
}

#[test]
fn fixture_keeps_observation_provenance_and_clean_run_bounds() {
    let f = fixture();
    assert!(f.method.contains("body-anchor"));
    assert_eq!(f.sources.len(), 2);
    assert_eq!(f.runs.len(), 9);
    for source in &f.sources {
        assert!(source.path.starts_with("recordings/headless/"));
        assert_eq!(source.sha256.len(), 64);
        assert!(source.sha256.bytes().all(|c| c.is_ascii_hexdigit()));
    }
    for run in &f.runs {
        assert!(f.sources.iter().any(|source| source.id == run.source));
        assert!(run.connection > 0 && run.snake_id > 0 && run.generation > 0);
        assert_eq!(
            run.last_center_anchor_index - run.first_center_anchor_index + 1,
            run.samples
        );
        assert!(run.last_center_server_ms > run.first_center_server_ms);
        assert!(run.duration_seconds > 0.0);
        assert_eq!(run.direction == "left", run.angle_delta_radians > 0.0);
        assert!(matches!(run.mode.as_str(), "boost" | "cruise"));
    }
}

#[test]
fn original_large_snakes_match_sustained_turn_angle_in_both_directions() {
    let f = fixture();
    let large: Vec<_> = f.runs.iter().filter(|run| run.segments >= 160).collect();
    assert_eq!(large.len(), 6);
    assert!(large.iter().any(|run| run.mode == "boost"));
    assert!(large.iter().any(|run| run.direction == "left"));
    assert!(large.iter().any(|run| run.direction == "right"));

    let mut weighted_squared_error = 0.0;
    let mut duration = 0.0;
    for run in large {
        let error_per_second =
            (simulated_angle(run) - run.angle_delta_radians).abs() / run.duration_seconds;
        assert!(
            error_per_second < 0.045,
            "{} conn={} snake={} size={} {} {}: {error_per_second:.4} rad/s",
            run.source,
            run.connection,
            run.snake_id,
            run.segments,
            run.mode,
            run.direction
        );
        weighted_squared_error += run.duration_seconds * error_per_second.powi(2);
        duration += run.duration_seconds;
    }
    assert!((weighted_squared_error / duration).sqrt() < 0.025);
}

#[test]
fn small_snakes_keep_their_recorded_cruise_and_boost_turns() {
    let f = fixture();
    let small: Vec<_> = f.runs.iter().filter(|run| run.segments <= 9).collect();
    assert_eq!(small.len(), 3);
    for run in small {
        let error_per_second =
            (simulated_angle(run) - run.angle_delta_radians).abs() / run.duration_seconds;
        assert!(
            error_per_second < 0.07,
            "small {}: {error_per_second:.4} rad/s",
            run.mode
        );
    }
}
