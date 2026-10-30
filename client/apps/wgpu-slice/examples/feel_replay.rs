//! Offline A3 CPU peer. See tools/feel-replay/native-peer.md for the bound runner.
#[path = "feel_replay/codec.rs"]
mod codec;
#[path = "feel_replay/driver.rs"]
mod driver;
#[path = "feel_replay/observation.rs"]
mod observation;
use serde_json::{Value, json};
use slither_wgpu_slice::Result;
use std::io::{self, Read};

fn number(v: &Value) -> Result<f64> {
    v.as_f64().ok_or_else(|| "Expected number".into())
}
fn text(v: &Value) -> Result<&str> {
    v.as_str().ok_or_else(|| "Expected string".into())
}

fn replay(request: Value) -> Result<Value> {
    let geometry = &request["geometry"];
    let w = number(&geometry["css"]["width"])?;
    let h = number(&geometry["css"]["height"])?;
    let mut d = driver::Driver::new(w, h)?;
    if geometry["backing"] != json!({"width":d.extent[0] as f64,"height":d.extent[1] as f64}) {
        return Err("Backing does not match production CSS geometry policy".into());
    }
    d.play()?;
    d.game.pointer(
        number(&request["pointer"]["x"])?,
        number(&request["pointer"]["y"])?,
        w,
        h,
        false,
    )?;
    let mut events = vec![];
    let mut commands = vec![];
    let mut samples = vec![];
    let mut frame = 0;
    let mut draw = 0;
    let mut appeared = false;
    let mut errors = vec![];
    for (seq, step) in request["steps"]
        .as_array()
        .ok_or("Expected steps")?
        .iter()
        .enumerate()
    {
        let mut event = step["event"].clone();
        let now = number(&event["actualMs"])?;
        d.effects.clear();
        d.commands.clear();
        if let Some(bytes) = step["packet"].as_array() {
            let bytes: Vec<u8> = bytes
                .iter()
                .map(|b| {
                    number(b).and_then(|n| {
                        if n.fract() == 0. && (0. ..=255.).contains(&n) {
                            Ok(n as u8)
                        } else {
                            Err("Expected byte".into())
                        }
                    })
                })
                .collect::<Result<_>>()?;
            d.packet(&bytes, now)?;
        } else if step["control"] == "close" {
            d.close(now)?;
        } else if step["control"] == "play" {
            d.play()?;
            d.flush(now)?;
        } else if event["kind"] == "frame" {
            d.frame(now)?;
        } else {
            for action in step["actions"].as_array().ok_or("Expected input actions")? {
                match text(&action["kind"])? {
                    "pointer" => d.game.pointer(
                        number(&action["x"])?,
                        number(&action["y"])?,
                        w,
                        h,
                        action["down"] == true,
                    )?,
                    "release" => d.game.mouse_up(),
                    "key" => {
                        d.game.key(text(&action["code"])?, action["down"] == true);
                    }
                    "blur" => d.game.blur(),
                    "stop" => d.game.stop(),
                    "play" => {
                        if d.game.can_play() {
                            d.play()?;
                        }
                    }
                    "resize" => d.game.viewport(d.extent, 1.)?,
                    _ => return Err("Unknown input action".into()),
                }
                d.drain_effects(now)?;
                d.flush(now)?;
            }
        }
        event["effectIds"] = json!(d.effects);
        for mut command in d.commands.drain(..) {
            event["effectIds"]
                .as_array_mut()
                .unwrap()
                .push(command["effectId"].clone());
            command["seq"] = json!(commands.len());
            command["eventSeq"] = json!(seq);
            command["frameId"] = json!(frame);
            commands.push(command);
        }
        let session = d.game.feel_session();
        for row in d.recorder.take() {
            let o = observation::decode(&row, geometry)?;
            let eligibility = if !o["own"].is_null() {
                appeared = true;
                "own"
            } else if appeared {
                "between-lives"
            } else {
                "pre-appearance"
            };
            let drawing = row.phase == "draw";
            let draw_id = if drawing {
                let id = draw;
                draw += 1;
                json!(id)
            } else {
                Value::Null
            };
            samples.push(json!({"id":format!("{}-{}-{}",text(&event["sourceId"])?,row.phase,samples.len()),
                "eventSeq":seq,"phase":row.phase,"frameId":frame,"drawId":draw_id,"atMs":now,"eligibility":eligibility,
                "delta":o["delta"], "lag":{"active":session.lagging,"multiplier":o["multiplier"],
                    "pingPending":session.ping_pending,"pongAgeMs":(now-session.last_ping_ms).max(0.)},
                "own":o["own"],"camera":o["camera"],"quality":{"requested":"high",
                    "active":if d.game.quality().high_quality {"high"} else {"low"},"successfulDraws":draw},
                "geometry":geometry,"status":{"prepared":drawing,"submitted":drawing,"committed":drawing,
                    "retries":0,"sendFailed":false,"presentFailed":false}}));
        }
        errors.extend(d.recorder.take_errors());
        if event["kind"] == "frame" {
            frame += 1;
        }
        events.push(event);
    }
    let pending = d.game.feel_pending();
    let body = json!({"schema":2,"events":events,"commands":commands,"samples":samples,
        "final":{"atMs":request["durationMs"],"events":events.len(),"commands":commands.len(),"samples":samples.len(),
            "draws":draw,"pendingEffects":pending[0],"pendingFrames":pending[1],
            "lastBoundaryId":samples.last().map(|s|s["id"].clone()).unwrap_or(json!("missing")),
            "completed":d.game.failure.is_none() && !d.recorder.has_failed(),"unexpectedNetwork":false}});
    Ok(json!({"body":body,"random":d.recorder.take_random(),"effects":d.receipts,"errors":errors}))
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    let request = codec::read(&input)?;
    let result = match args.as_slice() {
        [] => replay(request)?,
        // Node hashes the observed entropy and supplies the bound manifest. Rust
        // emits the final trace without decimal serialization of observations.
        [mode] if mode == "--assemble" => {
            let mut body = request["body"].clone();
            body["manifest"] = request["manifest"].clone();
            body
        }
        [mode] if mode == "--codec" => request,
        _ => {
            return Err("Expected no args, --assemble or --codec".into());
        }
    };
    print!("{}", codec::write(result)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Native feel replay failed: {error}");
        std::process::exit(1);
    }
}
