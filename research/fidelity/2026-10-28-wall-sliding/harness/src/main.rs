use slither_sim::{Point, boundary};
use std::io::{self, BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = io::BufWriter::new(io::stdout().lock());
    for line in io::stdin().lock().lines() {
        let value: serde_json::Value = serde_json::from_str(&line?)?;
        if value.is_object() {
            writeln!(output, "{}", replay(&value)?)?;
        } else {
            let v: Vec<f64> = serde_json::from_value(value)?;
            if v.len() != 5 || v.iter().any(|n| !n.is_finite()) {
                return Err("expected finite x,y,heading,scale,radius".into());
            }
            let (p, dead) = boundary::constrain(
                Point { x: v[0], y: v[1] },
                v[2],
                v[3],
                v[4],
                boundary::BoundaryMode::Sliding,
            );
            writeln!(output, "{}", serde_json::json!([p.x, p.y, dead]))?;
        }
        output.flush()?;
    }
    Ok(())
}

fn replay(v: &serde_json::Value) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let start: [f64; 2] = serde_json::from_value(v["start"].clone())?;
    let mut state: [f64; 4] = serde_json::from_value(v["initial"].clone())?;
    let changes: Vec<[f64; 5]> = serde_json::from_value(v["changes"].clone())?;
    let dt = v["dt"].as_f64().ok_or("missing dt")?;
    if !dt.is_finite() || dt <= 0.0 || dt > 0.014 {
        return Err("invalid step".into());
    }
    let mut p = Point {
        x: start[0],
        y: start[1],
    };
    let mut free = p;
    let mut time = 0.0;
    let mut dead = false;
    for change in changes {
        let target = change[0];
        if !target.is_finite() || target < time - 1e-10 || target > 1.0 {
            return Err("invalid interval change".into());
        }
        while time < target - 1e-12 {
            let seconds = dt.min(target - time);
            let (sin, cos) = state[0].sin_cos();
            let dx = state[1] / 0.032 * seconds * cos;
            let dy = state[1] / 0.032 * seconds * sin;
            free.x += dx;
            free.y += dy;
            if !dead {
                (p, dead) = boundary::constrain(
                    Point {
                        x: p.x + dx,
                        y: p.y + dy,
                    },
                    state[0],
                    slither_sim::physics::scale(state[2] as usize),
                    state[3],
                    boundary::BoundaryMode::Sliding,
                );
            }
            time += seconds;
        }
        state.copy_from_slice(&change[1..]);
    }
    Ok(serde_json::json!({"xy":[p.x,p.y],"dead":dead,"free":[free.x,free.y]}))
}
