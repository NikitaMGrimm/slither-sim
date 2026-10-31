//! Independent piecewise-linear truth and authored packets, not a smoothing model.
use super::{
    driver::Driver,
    synthetic_tape::{bundle, challenge, setup, u24},
};
use serde_json::{Value, json};
use slither_wgpu_slice::Result;
use std::f64::consts::{PI, TAU};

const DURATION: f64 = 8000.;
const ORIGIN: f64 = 2.;
const STATIONS: [f64; 5] = [0., 42., 84., 168., 252.];
const IDS: [u16; 2] = [7, 8];

fn heading(segment: i32, turning: bool) -> f64 {
    if !turning || segment < 0 {
        return 0.;
    }
    [0., PI / 8., 0., -PI / 8.][(segment as usize / 4) % 4]
}
// Closed-form segment lengths, independent of client update/draw/correction clocks.
fn path(distance: f64, turning: bool, id: u16) -> [f64; 2] {
    let mut p = [10000., 10000. + f64::from(id - 7) * 120.];
    if distance < 0. {
        p[0] += distance;
        return p;
    }
    let n = (distance / 42.).floor() as i32;
    for k in 0..n {
        let a = heading(k, turning);
        p[0] += 42. * a.cos();
        p[1] += 42. * a.sin();
    }
    let a = heading(n, turning);
    let rest = distance - f64::from(n) * 42.;
    p[0] += rest * a.cos();
    p[1] += rest * a.sin();
    p
}
fn angle16(a: f64) -> u16 {
    ((a.rem_euclid(TAU) / TAU * 65536.).round() as u32 % 65536) as u16
}
fn appearance(id: u16, speed: f64) -> Vec<u8> {
    let p = path(0., false, id);
    let mut b = vec![b's'];
    b.extend(id.to_be_bytes());
    b.extend(u24(0));
    b.push(b'0');
    b.extend(u24(0));
    b.extend(((speed * 32.).round() as u16).to_be_bytes());
    b.extend(u24(0));
    b.push(0);
    b.extend(u24((p[0] * 5.).round() as u32));
    b.extend(u24((p[1] * 5.).round() as u32));
    b.extend([0, 0, 255]);
    b.extend(u24(((p[0] - 462.) * 5.).round() as u32));
    b.extend(u24((p[1] * 5.).round() as u32));
    for _ in 0..10 {
        b.extend([211, 127]);
    } // exact 42-unit decoded deltas
    b.extend(0u16.to_be_bytes());
    b
}
fn anchor(id: u16, n: usize, turning: bool, absolute_bias: Option<f64>) -> Vec<u8> {
    let mut b = vec![if absolute_bias.is_some() { b'=' } else { b'g' }];
    b.extend(id.to_be_bytes());
    b.extend(angle16(heading(n as i32 - 1, turning)).to_be_bytes());
    if let Some(bias) = absolute_bias {
        let p = path(n as f64 * 42., turning, id);
        b.extend(((p[0] + bias).round() as u16).to_be_bytes());
        b.extend((p[1].round() as u16).to_be_bytes());
    }
    b
}
fn motion(id: u16, n: usize, turning: bool, bias: f64) -> Vec<u8> {
    let a = ((heading(n as i32, turning) + bias).rem_euclid(TAU) / TAU * 256.).round() as u32 % 256;
    let mut b = vec![b'3'];
    b.extend(id.to_be_bytes());
    b.extend([a as u8, a as u8]);
    b
}
#[derive(Clone)]
struct Packet {
    generated: f64,
    arrival: f64,
    anchor: Option<usize>,
    bytes: Vec<u8>,
}

pub fn run(config: Value) -> Result<Value> {
    probe(config, None)
}

fn probe(config: Value, malformed_at: Option<usize>) -> Result<Value> {
    let speed = match config["speed"].as_str() {
        Some("stationary") if config["anchors"] == false => 0.,
        Some("normal") => 180.,
        Some("boost") => 437.5,
        _ => return Err("Expected normal/boost, or stationary no-anchor control".into()),
    };
    let turning = match config["motion"].as_str() {
        Some("straight") => false,
        Some("turn") => true,
        _ => return Err("Expected straight/turn".into()),
    };
    let jitter = config["jitterMs"].as_f64().ok_or("Expected jitterMs")?;
    if ![0., 4., 12.].contains(&jitter) {
        return Err("Unsupported frozen jitter tape".into());
    }
    let impulse = config["impulse"] == true;
    let delay = config["delayMs"].as_f64().unwrap_or(20.);
    if !(0. ..=100.).contains(&delay) {
        return Err("Invalid delay".into());
    }
    let rate = config["rate"].as_str().ok_or("Expected rate")?;
    let period = match rate {
        "60" => 1000. / 60.,
        "170" => 1000. / 170.,
        "240" => 1000. / 240.,
        "irregular" => 0.,
        _ => return Err("Unsupported rate".into()),
    };
    let mut packets = Vec::new();
    let mut previous = ORIGIN;
    let mut position_on = None;
    let mut heading_on = None;
    let mut old_bias = 0.;
    for n in 1.. {
        if config["anchors"] == false {
            break;
        }
        let generated = ORIGIN + n as f64 * 42000. / speed;
        if generated > ORIGIN + DURATION {
            break;
        }
        let bias = if impulse && (3000. ..4300.).contains(&(generated - ORIGIN)) {
            24.
        } else {
            0.
        };
        let angle_bias = if impulse && (5300. ..6600.).contains(&(generated - ORIGIN)) {
            PI / 8.
        } else {
            0.
        };
        let offset = [0., 1., -1., 0., -1., 1., 1., -1.][(n - 1) % 8] * jitter;
        let arrival = (generated + delay + offset).max(previous);
        previous = arrival;
        if bias != 0. && old_bias == 0. {
            position_on = Some(arrival);
        }
        if angle_bias != 0. && heading_on.is_none() {
            heading_on = Some(arrival);
        }
        let mut commands = Vec::new();
        for id in IDS {
            commands.push(anchor(id, n, turning, (bias != old_bias).then_some(bias)));
            commands.push(motion(id, n, turning, angle_bias));
        }
        old_bias = bias;
        packets.push(Packet {
            generated,
            arrival,
            anchor: Some(n),
            bytes: bundle(&commands),
        });
    }
    // Authored pongs prevent unrelated lag damping. They never update pose ages.
    for n in 1..=250 {
        let t = ORIGIN + n as f64 * 32.;
        packets.push(Packet {
            generated: t,
            arrival: t,
            anchor: None,
            bytes: bundle(&[vec![112]]),
        });
    }
    packets.sort_by(|a, b| {
        a.arrival
            .total_cmp(&b.arrival)
            .then_with(|| b.anchor.cmp(&a.anchor))
    });
    let mut d = Driver::new(960., 540.)?;
    d.desktop = true;
    d.play()?;
    d.redraw(0., 0.)?;
    d.packet(&bundle(&[challenge()]), 1.)?;
    let mut world_setup = setup();
    world_setup[1..4].copy_from_slice(&u24(10000));
    d.packet(&bundle(&[world_setup]), ORIGIN)?;
    d.redraw(ORIGIN, ORIGIN)?; // Date the empty world before appearances, at zero elapsed.
    d.packet(
        &bundle(&[appearance(7, speed), appearance(8, speed)]),
        ORIGIN,
    )?;
    d.recorder.record_geometry(IDS);
    d.game.key("Space", speed > 400.);
    let mut rows = Vec::new();
    let mut failures = Vec::new();
    let mut next = 0;
    let mut now = ORIGIN;
    let mut pose_time = ORIGIN;
    let mut pose_id = 0;
    let mut frame = 0;
    let mut lifecycle = Vec::new();
    loop {
        // Frozen tie order: packet generation order at equal delivery, then redraw.
        let step = (|| -> Result<()> {
            while next < packets.len() && packets[next].arrival <= now {
                let p = &packets[next];
                d.packet(&p.bytes, p.arrival)?;
                if let Some(id) = p.anchor {
                    pose_time = p.generated;
                    pose_id = id;
                }
                next += 1;
            }
            if malformed_at == Some(frame) {
                d.packet(&bundle(&[vec![b'g']]), now)?;
            }
            d.redraw(now, now)
        })();
        let acknowledged = std::mem::take(&mut d.acknowledged_pictures);
        for g in d.recorder.take_geometry() {
            if g.snakes.len() != 2 {
                failures.push(format!("frame {frame}: missing selected snake geometry"));
            }
            let ack = acknowledged.contains(&g.sequence);
            let mut snakes = Vec::new();
            for s in &g.snakes {
                let m = s.metrics;
                let age = (now - ORIGIN) / 1000.;
                let distance = speed * age;
                let truth = path(distance, turning, m.id);
                let stations: Vec<_> = STATIONS
                    .iter()
                    .map(|station| path(distance - station, turning, m.id))
                    .collect();
                let station_headings: Vec<_> = STATIONS
                    .iter()
                    .map(|station| heading(((distance - station) / 42.).floor() as i32, turning))
                    .collect();
                let station_segments: Vec<_> = STATIONS
                    .iter()
                    .map(|station| {
                        let start = ((distance - station) / 42.).floor() * 42.;
                        [path(start, turning, m.id), path(start + 42., turning, m.id)]
                    })
                    .collect();
                let a = heading((distance / 42.).floor() as i32, turning);
                snakes.push(json!({"id":m.id,"token":s.token.to_string(),"own":s.own,
                    "drawn":[m.corrected_head.x,m.corrected_head.y],"raw":[m.raw_head.x,m.raw_head.y],
                    "heading":m.corrected_heading,"rawHeading":m.heading,"truthHeading":a,"speed":speed,"scale":m.scale,
                    "truth":truth,"truthBody":stations,"truthBodyHeadings":station_headings,"truthBodySegments":station_segments,"body":s.body,"packetHead":[s.packet_head.x,s.packet_head.y],
                    "anchorId":pose_id,"poseGeneratedMs":pose_time,"headingGeneratedMs":pose_time,"speedGeneratedMs":ORIGIN}));
            }
            let mut row = json!({"frame":frame,"sequence":g.sequence.to_string(),"atMs":now,"measured":now>=ORIGIN+2000.,
                "acknowledged":ack,"camera":[g.camera.x,g.camera.y],"zoom":g.zoom,"snakes":snakes});
            // Zero is implicit so the frozen live-only captures keep their ABI.
            if g.excluded_retained != 0 {
                row["excludedRetained"] = json!(g.excluded_retained);
            }
            rows.push(row);
        }
        if d.recorder.has_failed() {
            failures.extend(d.recorder.take_errors());
        }
        lifecycle.append(&mut d.lifecycle);
        d.recorder.take();
        d.recorder.take_random();
        // A failed production boundary must still export every prior/partial row.
        // Returning `?` here would discard the measuring run's diagnostic prefix.
        if let Err(error) = step {
            failures.push(format!("frame {frame} at {now} ms: {error}"));
            break;
        }
        frame += 1;
        let next_time = if period > 0. {
            ORIGIN + frame as f64 * period
        } else {
            now + [4., 7., 19., 5., 11., 23., 6., 9.][(frame - 1) % 8]
        };
        if next_time > ORIGIN + DURATION {
            break;
        }
        now = next_time;
    }
    d.game.stop();
    if let Err(error) = d.drain_effects(now) {
        failures.push(format!("shutdown: {error}"));
    }
    if d.game.feel_pending() != [0, 0] {
        failures.push("Pending work at completion".into());
    }
    if d.game.failure.is_some() {
        failures.push(format!("{:?}", d.game.failure));
    }
    Ok(
        json!({"format":"smoothing-lag-v1","config":config,"durationMs":DURATION,"warmupMs":2000.,"originMs":ORIGIN,
        "stations":STATIONS,"positionImpulseMs":position_on,"headingImpulseMs":heading_on,"positionAmplitude":24.,"headingAmplitude":PI/8.,
        "rows":rows,"lifecycle":lifecycle,"commands":d.commands,"failures":failures,"completed":failures.is_empty(),
        "packets":packets.iter().map(|p|json!({"generatedMs":p.generated,"arrivalMs":p.arrival,"anchorId":p.anchor,"bytes":p.bytes})).collect::<Vec<_>>()}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_truth_has_exact_42_unit_edges_and_continuous_corners() {
        for turning in [false, true] {
            for n in 0..100 {
                let a = path(n as f64 * 42., turning, 7);
                let b = path((n + 1) as f64 * 42., turning, 7);
                assert!(((b[0] - a[0]).hypot(b[1] - a[1]) - 42.).abs() < 1e-9);
                let left = path((n + 1) as f64 * 42. - 1e-7, turning, 7);
                assert!((left[0] - b[0]).hypot(left[1] - b[1]) < 2e-7);
            }
        }
        assert_eq!(path(180., false, 7), [10180., 10000.]);
        assert_eq!(path(-42., true, 8), [9958., 10120.]);
    }
    #[test]
    fn production_failure_exports_the_observed_prefix_and_rejects_completion() {
        let result=probe(json!({"rate":"170","speed":"normal","motion":"straight","jitterMs":0,"delayMs":20,"impulse":false}),Some(5)).unwrap();
        assert_eq!(result["completed"], false);
        assert_eq!(result["rows"].as_array().unwrap().len(), 5);
        assert!(
            result["failures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e.as_str().unwrap().contains("frame 5"))
        );
        assert!(
            result["rows"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["acknowledged"] == true)
        );
    }
}
