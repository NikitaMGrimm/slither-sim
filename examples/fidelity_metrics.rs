//! Report fixed-fixture error measurements as one JSON object.
//!
//! cargo run --locked --example fidelity_metrics -- tests/fixtures > metrics.json
//!
//! The directory must contain `movement-turn-runs.json` and
//! `speed-ramp-validation.json`. Data is read at runtime so two simulator
//! builds can be measured against the same pinned fixtures. The formulas match
//! `tests/movement_turns.rs` and `tests/physics.rs::independently_recorded_speed_ramps`;
//! the values are residuals against recorded observations, not an overall
//! accuracy claim, and carry no pass/fail thresholds.

use serde::Deserialize;
use serde_json::json;
use slither_sim::{
    Point,
    physics::{BOOST_SPEED, advance, approach_speed, base_speed, turn_rate},
};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

const TURN_FILE: &str = "movement-turn-runs.json";
const RAMP_FILE: &str = "speed-ramp-validation.json";
/// Converts simulator speed (world units/s) to the protocol speed units used in
/// the ramp fixture, as in the physics test.
const PROTOCOL_SPEED_SCALE: f64 = 0.032;

#[derive(Deserialize)]
struct TurnFixture {
    runs: Vec<TurnRun>,
}

#[derive(Deserialize)]
struct TurnRun {
    segments: usize,
    mode: String,
    direction: String,
    duration_seconds: f64,
    angle_delta_radians: f64,
}

#[derive(Deserialize)]
struct RampFixture {
    sources: Vec<RampSource>,
    ramps: Vec<Ramp>,
}

#[derive(Deserialize)]
struct RampSource {
    name: String,
}

#[derive(Deserialize)]
struct Ramp {
    segments: usize,
    up: bool,
    points: Vec<(f64, f64)>,
    source: String,
}

#[derive(Debug, PartialEq)]
struct Metric {
    id: String,
    label: String,
    value: f64,
    unit: String,
    samples: usize,
}

fn finite(value: f64, what: &str) -> Result<f64, String> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(format!("{what} is not finite: {value}"))
    }
}

/// Integrates the public turn helper toward a continually distant wanted
/// heading, matching `tests/movement_turns.rs::simulated_angle`.
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

fn validate_turn(run: &TurnRun, index: usize) -> Result<(), String> {
    let at = format!("{TURN_FILE} runs[{index}]");
    if !matches!(run.mode.as_str(), "boost" | "cruise") {
        return Err(format!("{at}: unknown mode {:?}", run.mode));
    }
    if !matches!(run.direction.as_str(), "left" | "right") {
        return Err(format!("{at}: unknown direction {:?}", run.direction));
    }
    let duration = finite(run.duration_seconds, &format!("{at} duration_seconds"))?;
    if duration <= 0.0 {
        return Err(format!("{at}: duration_seconds must be positive"));
    }
    let angle = finite(
        run.angle_delta_radians,
        &format!("{at} angle_delta_radians"),
    )?;
    if (run.direction == "left") != (angle > 0.0) {
        return Err(format!("{at}: angle sign disagrees with {}", run.direction));
    }
    Ok(())
}

/// Square root of the duration-weighted mean squared value of `(weight, value)`.
fn weighted_rmse(pairs: &[(f64, f64)]) -> Result<f64, String> {
    let weight: f64 = pairs.iter().map(|(w, _)| w).sum();
    if pairs.is_empty() || weight <= 0.0 {
        return Err("weighted RMSE needs a positive total weight".into());
    }
    let sum: f64 = pairs.iter().map(|(w, v)| w * v * v).sum();
    finite((sum / weight).sqrt(), "weighted RMSE")
}

fn turn_metrics(fixture: &TurnFixture) -> Result<Vec<Metric>, String> {
    let mut large = Vec::new();
    let mut small = Vec::new();
    for (index, run) in fixture.runs.iter().enumerate() {
        validate_turn(run, index)?;
        let cohort = match run.segments {
            s if s >= 160 => &mut large,
            s if s <= 9 => &mut small,
            _ => continue,
        };
        let error_per_second = finite(
            (simulated_angle(run) - run.angle_delta_radians).abs() / run.duration_seconds,
            &format!("{TURN_FILE} runs[{index}] turn error"),
        )?;
        cohort.push((run.duration_seconds, error_per_second));
    }
    let mut metrics = Vec::new();
    for (id, label, cohort) in [
        (
            "turn-large-rmse",
            "Sustained turn error, segments >= 160 (duration-weighted RMSE)",
            &large,
        ),
        (
            "turn-small-rmse",
            "Sustained turn error, segments <= 9 (duration-weighted RMSE)",
            &small,
        ),
    ] {
        let value = weighted_rmse(cohort).map_err(|e| format!("{id}: {e}"))?;
        metrics.push(Metric {
            id: id.into(),
            label: label.into(),
            value,
            unit: "rad/s".into(),
            samples: cohort.len(),
        });
    }
    Ok(metrics)
}

/// Least-squares slope of speed over time, with times in milliseconds.
fn observed_slope(points: &[(f64, f64)]) -> Result<f64, String> {
    if points.len() < 2 {
        return Err("needs at least two points".into());
    }
    for &(t, speed) in points {
        finite(t, "time")?;
        finite(speed, "speed")?;
    }
    let n = points.len() as f64;
    let mean_t = points.iter().map(|p| p.0 / 1000.0).sum::<f64>() / n;
    let mean_speed = points.iter().map(|p| p.1).sum::<f64>() / n;
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for &(t, speed) in points {
        let dt = t / 1000.0 - mean_t;
        numerator += dt * (speed - mean_speed);
        denominator += dt * dt;
    }
    if denominator <= 0.0 {
        return Err("needs distinct times".into());
    }
    finite(numerator / denominator, "slope")
}

/// Simulated ramp rate in protocol speed units per second, queried far from
/// clipping at the target exactly as the physics test does.
fn simulated_slope(segments: usize, up: bool) -> f64 {
    let current = 10.0 / PROTOCOL_SPEED_SCALE;
    let target = if up {
        BOOST_SPEED
    } else {
        base_speed(segments)
    };
    (approach_speed(current, target, segments, 0.001) - current) * PROTOCOL_SPEED_SCALE / 0.001
}

fn ramp_metrics(fixture: &RampFixture) -> Result<Vec<Metric>, String> {
    let sources: BTreeSet<&str> = fixture.sources.iter().map(|s| s.name.as_str()).collect();
    if sources.len() != fixture.sources.len() {
        return Err(format!("{RAMP_FILE}: duplicate source names"));
    }
    let mut errors = BTreeMap::<&str, (usize, f64)>::new();
    for (index, ramp) in fixture.ramps.iter().enumerate() {
        let at = format!("{RAMP_FILE} ramps[{index}]");
        if !sources.contains(ramp.source.as_str()) {
            return Err(format!("{at}: unlisted source {:?}", ramp.source));
        }
        let observed = observed_slope(&ramp.points).map_err(|e| format!("{at}: {e}"))?;
        let rate = finite(simulated_slope(ramp.segments, ramp.up), &at)?;
        let entry = errors.entry(ramp.source.as_str()).or_default();
        entry.0 += 1;
        entry.1 += (rate - observed).powi(2);
    }
    let mut metrics = Vec::new();
    for source in sources {
        let Some(&(count, sum)) = errors.get(source) else {
            return Err(format!("{RAMP_FILE}: source {source:?} has no ramps"));
        };
        metrics.push(Metric {
            id: format!("speed-ramp-rmse:{source}"),
            label: format!("Speed ramp slope error, {source} (RMSE)"),
            value: finite((sum / count as f64).sqrt(), source)?,
            unit: "protocol speed/s".into(),
            samples: count,
        });
    }
    Ok(metrics)
}

fn load<T: for<'de> Deserialize<'de>>(dir: &Path, name: &str) -> Result<T, String> {
    let path = dir.join(name);
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn metrics(dir: &Path) -> Result<Vec<Metric>, String> {
    let mut metrics = turn_metrics(&load(dir, TURN_FILE)?)?;
    metrics.extend(ramp_metrics(&load(dir, RAMP_FILE)?)?);
    Ok(metrics)
}

fn report(metrics: &[Metric]) -> serde_json::Value {
    json!({
        "schema": 1,
        "metrics": metrics.iter().map(|m| json!({
            "id": m.id,
            "label": m.label,
            "value": m.value,
            "unit": m.unit,
            "samples": m.samples,
        })).collect::<Vec<_>>(),
    })
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let [dir] = args.as_slice() else {
        return Err("usage: fidelity_metrics <fixture-directory>".into());
    };
    println!("{}", report(&metrics(Path::new(dir))?));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fidelity_metrics: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(segments: usize, mode: &str, direction: &str, duration: f64, angle: f64) -> TurnRun {
        TurnRun {
            segments,
            mode: mode.into(),
            direction: direction.into(),
            duration_seconds: duration,
            angle_delta_radians: angle,
        }
    }

    #[test]
    fn weighted_rmse_weights_by_duration() {
        // sqrt((1 * 1^2 + 3 * 3^2) / 4) = sqrt(7)
        let value = weighted_rmse(&[(1.0, 1.0), (3.0, -3.0)]).unwrap();
        assert!((value - 7f64.sqrt()).abs() < 1e-12);
        assert!(weighted_rmse(&[]).is_err());
        assert!(weighted_rmse(&[(0.0, 1.0)]).is_err());
    }

    #[test]
    fn observed_slope_is_least_squares_per_second() {
        // Exact line of 2 units per second with millisecond times.
        let line = [(1000.0, 5.0), (1500.0, 6.0), (2250.0, 7.5)];
        assert!((observed_slope(&line).unwrap() - 2.0).abs() < 1e-12);
        // A symmetric peak has no linear trend.
        let noisy = [(0.0, 0.0), (1000.0, 1.0), (2000.0, 0.0)];
        assert!(observed_slope(&noisy).unwrap().abs() < 1e-12);
        assert!(observed_slope(&[(0.0, 1.0)]).is_err());
        assert!(observed_slope(&[(5.0, 1.0), (5.0, 2.0)]).is_err());
    }

    #[test]
    fn turn_error_is_zero_for_a_run_recorded_at_the_simulated_angle() {
        let mut run = turn(200, "cruise", "left", 1.5, 1.0);
        run.angle_delta_radians = simulated_angle(&run);
        let fixture = TurnFixture {
            runs: vec![run, turn(5, "boost", "right", 2.0, -0.1)],
        };
        let metrics = turn_metrics(&fixture).unwrap();
        assert_eq!(metrics[0].id, "turn-large-rmse");
        assert_eq!(metrics[0].samples, 1);
        assert!(metrics[0].value.abs() < 1e-12);
        let small = &fixture.runs[1];
        let expected = (simulated_angle(small) + 0.1).abs() / 2.0;
        assert!((metrics[1].value - expected).abs() < 1e-12);
    }

    #[test]
    fn turn_data_is_rejected_when_invalid_or_a_cohort_is_empty() {
        let ok = || turn(200, "cruise", "left", 1.0, 1.0);
        let small = || turn(9, "boost", "right", 1.0, -1.0);
        for bad in [
            turn(200, "sprint", "left", 1.0, 1.0),
            turn(200, "cruise", "up", 1.0, 1.0),
            turn(200, "cruise", "left", 0.0, 1.0),
            turn(200, "cruise", "left", f64::NAN, 1.0),
            turn(200, "cruise", "left", 1.0, -1.0),
        ] {
            let fixture = TurnFixture {
                runs: vec![bad, small()],
            };
            assert!(turn_metrics(&fixture).is_err());
        }
        // A mid-size run alone leaves both reported cohorts empty.
        let fixture = TurnFixture {
            runs: vec![ok(), turn(50, "cruise", "left", 1.0, 1.0)],
        };
        assert!(
            turn_metrics(&fixture)
                .unwrap_err()
                .contains("turn-small-rmse")
        );
    }

    #[test]
    fn ramp_rmse_uses_simulated_protocol_rate_per_source() {
        let rate = simulated_slope(20, true);
        // Observed slopes rate + 1 and rate - 3 give RMSE sqrt((1 + 9) / 2) = sqrt(5).
        let ramp = |offset: f64| Ramp {
            segments: 20,
            up: true,
            points: vec![(0.0, 0.0), (1000.0, rate + offset)],
            source: "a".into(),
        };
        let fixture = RampFixture {
            sources: vec![RampSource { name: "a".into() }],
            ramps: vec![ramp(1.0), ramp(-3.0)],
        };
        let metrics = ramp_metrics(&fixture).unwrap();
        assert_eq!(metrics.len(), 1);
        assert_eq!(metrics[0].id, "speed-ramp-rmse:a");
        assert_eq!(metrics[0].samples, 2);
        assert!((metrics[0].value - 5f64.sqrt()).abs() < 1e-9);
    }

    #[test]
    fn ramp_data_is_rejected_for_unknown_or_empty_sources() {
        let ramp = |source: &str| Ramp {
            segments: 20,
            up: false,
            points: vec![(0.0, 10.0), (100.0, 9.0)],
            source: source.into(),
        };
        let sources = |names: &[&str]| {
            names
                .iter()
                .map(|name| RampSource {
                    name: (*name).into(),
                })
                .collect()
        };
        for fixture in [
            RampFixture {
                sources: sources(&["a"]),
                ramps: vec![ramp("b")],
            },
            RampFixture {
                sources: sources(&["a", "b"]),
                ramps: vec![ramp("a")],
            },
            RampFixture {
                sources: sources(&["a", "a"]),
                ramps: vec![ramp("a")],
            },
        ] {
            assert!(ramp_metrics(&fixture).is_err());
        }
    }

    #[test]
    fn missing_fields_and_files_are_errors() {
        assert!(serde_json::from_str::<TurnFixture>(r#"{"runs":[{"segments":5}]}"#).is_err());
        assert!(load::<TurnFixture>(Path::new("does-not-exist"), TURN_FILE).is_err());
    }

    #[test]
    fn repository_fixtures_produce_the_pinned_cohorts() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let metrics = metrics(&dir).unwrap();
        let summary: Vec<_> = metrics.iter().map(|m| (m.id.as_str(), m.samples)).collect();
        assert_eq!(
            summary,
            [
                ("turn-large-rmse", 6),
                ("turn-small-rmse", 3),
                ("speed-ramp-rmse:run3-large-radius-20260927", 343),
                ("speed-ramp-rmse:run3-single-20260927", 203),
                ("speed-ramp-rmse:visibility-covariates-20260928", 1021),
            ]
        );
        assert!(
            metrics
                .iter()
                .all(|m| m.value.is_finite() && m.value >= 0.0)
        );
        let report = report(&metrics);
        assert_eq!(report["schema"], 1);
        assert_eq!(report["metrics"].as_array().unwrap().len(), 5);
    }
}
