//! Scalar recorder calibration on the existing production CPU driver. No socket/window/GPU.
use super::{driver, synthetic_tape};
use serde_json::{Value, json};
use slither_wgpu_slice::{Result, latency};
use std::{sync::mpsc, time::Instant};

const FRAMES: usize = 1200;
struct Measurement {
    times: Vec<f64>,
    commands: Vec<Value>,
    pictures: Vec<slither_client::feel_trace::Observation>,
    recorder: latency::Recorder,
}
fn schedule(
    record: bool,
    turns: mpsc::Receiver<()>,
    done: mpsc::Sender<()>,
) -> Result<Measurement> {
    let recorder = latency::Recorder::new(262_144);
    let guard = record.then(|| recorder.attach());
    let mut d = driver::Driver::new(960., 540.)?;
    d.desktop = true;
    d.play()?;
    d.redraw(0., 0.)?;
    d.packet(&synthetic_tape::bundle(&[synthetic_tape::challenge()]), 1.)?;
    d.packet(
        &synthetic_tape::bundle(&[synthetic_tape::setup(), synthetic_tape::snake(0)]),
        2.,
    )?;
    let mut times = Vec::with_capacity(FRAMES);
    let mut commands = Vec::new();
    let mut pictures = Vec::new();
    let mut now = 3.;
    done.send(()).map_err(|e| e.to_string())?;
    for frame in 0..FRAMES + 100 {
        turns.recv().map_err(|e| e.to_string())?;
        // Fixed cadence gates and irregular phases; same operations and logical times on both sides.
        now += [1000. / 60., 1000. / 170., 1000. / 240., 7., 19.][frame % 5];
        let start = Instant::now();
        if frame % 17 == 0 {
            d.game
                .pointer(600. + (frame % 41) as f64, 220., 960., 540., false)?;
        }
        if frame % 29 == 0 {
            d.game.key("ArrowLeft", frame % 58 == 0);
        }
        if frame % 37 == 0 {
            d.game.key("Space", frame % 74 == 0);
        }
        d.redraw(now, now)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        if frame >= 100 {
            times.push(elapsed);
        }
        commands.append(&mut d.commands);
        // Full numeric observations include the actual prepared picture geometry, not a hash of IDs.
        pictures.extend(d.recorder.take());
        done.send(()).map_err(|e| e.to_string())?;
    }
    d.close(now + 1.)?;
    drop(guard);
    Ok(Measurement {
        times,
        commands,
        pictures,
        recorder,
    })
}
fn percentile(v: &[f64], p: f64) -> f64 {
    v[(v.len() as f64 * p).ceil() as usize - 1]
}
pub fn run() -> Result<()> {
    let mut rounds = Vec::new();
    for round in 0..5 {
        // Adjacent frame pairs reduce host-load drift. Channels, waits, construction,
        // observation copying and export remain outside each timed driver operation.
        let (a, b) = std::thread::scope(|scope| -> Result<_> {
            let (off_turn, off_rx) = mpsc::channel();
            let (off_done, off_ready) = mpsc::channel();
            let (on_turn, on_rx) = mpsc::channel();
            let (on_done, on_ready) = mpsc::channel();
            let off = scope.spawn(|| schedule(false, off_rx, off_done));
            let on = scope.spawn(|| schedule(true, on_rx, on_done));
            let result = (|| -> Result<()> {
                off_ready.recv().map_err(|e| e.to_string())?;
                on_ready.recv().map_err(|e| e.to_string())?;
                for frame in 0..FRAMES + 100 {
                    let order = if (round + frame) % 2 == 0 {
                        [(&off_turn, &off_ready), (&on_turn, &on_ready)]
                    } else {
                        [(&on_turn, &on_ready), (&off_turn, &off_ready)]
                    };
                    for (turn, done) in order {
                        turn.send(()).map_err(|e| e.to_string())?;
                        done.recv().map_err(|e| e.to_string())?;
                    }
                }
                Ok(())
            })();
            drop(off_turn);
            drop(on_turn);
            let a = off.join().map_err(|_| "off driver panicked")?;
            let b = on.join().map_err(|_| "on driver panicked")?;
            result?;
            Ok((a?, b?))
        })?;
        if a.commands != b.commands || a.pictures != b.pictures {
            return Err("Trace off/on changed commands or pictures".into());
        }
        let (rows, dropped) = b.recorder.snapshot();
        if dropped != 0 {
            return Err("Latency calibration overflow".into());
        }
        let mut delta: Vec<_> = a.times.iter().zip(&b.times).map(|(a, b)| b - a).collect();
        delta.sort_by(f64::total_cmp);
        rounds.push(json!({"round":round,"frames":FRAMES,"commands":a.commands.len(),"observations":a.pictures.len(),
            "records":rows.len(),"dropped":dropped,"pairedRecorderDeltaP50Ms":percentile(&delta,0.5),
            "pairedRecorderDeltaP95Ms":percentile(&delta,0.95), "offMeanMs":a.times.iter().sum::<f64>() / FRAMES as f64, "onMeanMs":b.times.iter().sum::<f64>() / FRAMES as f64}));
    }
    println!(
        "{}",
        json!({"scope":"release CPU driver; scalar recorder; no transport/GPU/present", "warmupFrames":100,"rounds":rounds})
    );
    Ok(())
}
