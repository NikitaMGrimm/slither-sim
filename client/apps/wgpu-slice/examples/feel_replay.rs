//! Offline A3 CPU peer. See tools/feel-replay/native-peer.md for the bound runner.
#[path = "feel_replay/driver.rs"]
mod driver;
#[path = "feel_replay/latency.rs"]
mod latency;
#[path = "feel_replay/observation.rs"]
mod observation;
#[path = "../tests/support/announcement_tape.rs"]
mod synthetic_tape;
use serde_json::{Value, json};
use slither_wgpu_slice::Result;
use std::collections::BTreeMap;
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
    d.desktop = match request["lane"].as_str() {
        None | Some("equal-schedule") => false,
        Some("desktop-ordering") => true,
        _ => return Err("Unknown native replay lane".into()),
    };
    if number(&geometry["backing"]["width"])? != f64::from(d.extent[0])
        || number(&geometry["backing"]["height"])? != f64::from(d.extent[1])
    {
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
    let mut successful_draws = 0;
    let mut observed = BTreeMap::new();
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
        if step["skippedRedraw"] == true {
            d.lifecycle
                .push(json!({"kind":"blocked-redraw", "atMs":now}));
        } else if step["acknowledgment"] == true {
            d.ack_picture(now)?;
        } else if let Some(bytes) = step["packet"].as_array() {
            if d.desktop {
                d.lifecycle.push(
                    json!({"kind":"transport", "scheduledMs":event["scheduledMs"], "atMs":now}),
                );
            }
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
            if d.desktop {
                d.redraw(now, number(&step["ackAtMs"])?)?;
            } else {
                d.frame(now)?;
            }
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
        if d.desktop {
            for received in d.game.take_feel_ingress() {
                d.lifecycle
                    .push(json!({"kind":"ingest", "receivedMs":received, "atMs":now}));
            }
        }
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
            observed.insert(row.sequence, samples.len());
            samples.push(json!({"id":format!("{}-{}-{}",text(&event["sourceId"])?,row.phase,samples.len()),
                "eventSeq":seq,"phase":row.phase,"frameId":frame,"drawId":draw_id,"atMs":now,"acknowledgedMs":null,"eligibility":eligibility,
                "delta":o["delta"], "lag":{"active":session.lagging,"multiplier":o["multiplier"],
                    "pingPending":session.ping_pending,"pongAgeMs":(now-session.last_ping_ms).max(0.)},
                "own":o["own"],"camera":o["camera"],"quality":{"requested":"high",
                    "active":if d.game.quality().high_quality {"high"} else {"low"},"successfulDraws":0},
                "geometry":geometry,"status":{"prepared":drawing,"submitted":false,"committed":false,
                    "retries":0,"sendFailed":false,"presentFailed":false}}));
        }
        for sequence in std::mem::take(&mut d.acknowledged_pictures) {
            let index = *observed
                .get(&sequence)
                .ok_or("Acknowledged picture has no recorded observation")?;
            let sample = &mut samples[index];
            if sample["phase"] != "draw" || sample["status"]["committed"] == true {
                return Err("Invalid/duplicate picture acknowledgement".into());
            }
            sample["status"]["submitted"] = json!(true);
            sample["status"]["committed"] = json!(true);
            sample["acknowledgedMs"] = json!(now);
            if d.desktop {
                d.lifecycle
                    .push(json!({"kind":"presented", "atMs":now, "eventSeq":seq,
                    "sequence":sequence.to_string(), "drawId":sample["drawId"]}));
            }
        }
        errors.extend(d.recorder.take_errors());
        if event["kind"] == "frame" {
            frame += 1;
        }
        events.push(event);
    }
    // Like browser assembly, receipt identities determine final commitment and
    // cumulative sample counts. Receipt times remain on their actual events.
    for sample in &mut samples {
        if sample["phase"] == "draw" && sample["status"]["committed"] == true {
            successful_draws += 1;
        }
        sample["quality"]["successfulDraws"] = json!(successful_draws);
    }
    let pending = d.game.feel_pending();
    let body = json!({"schema":3,"events":events,"commands":commands,"samples":samples,
        "final":{"atMs":request["durationMs"],"events":events.len(),"commands":commands.len(),"samples":samples.len(),
            "draws":successful_draws,"pendingEffects":pending[0],"pendingFrames":pending[1],
            "lastBoundaryId":samples.last().map(|s|s["id"].clone()).unwrap_or(json!("missing")),
            "completed":d.game.failure.is_none() && !d.recorder.has_failed(),"unexpectedNetwork":false}});
    let mut result =
        json!({"body":body,"random":d.recorder.take_random(),"effects":d.receipts,"errors":errors});
    if d.desktop {
        result["lane"] = json!("desktop-ordering");
        result["lifecycle"] = json!(d.lifecycle);
    }
    Ok(result)
}

#[cfg(test)]
mod desktop_tests {
    use super::*;

    #[test]
    fn equal_setup_pair_has_per_row_acknowledgment_counts() {
        use synthetic_tape::{bundle, challenge, setup};
        let backing = slither_wgpu_slice::render::backing_size([960., 540.]).unwrap();
        let steps = vec![
            json!({"event":{"sourceId":"startup","kind":"frame","actualMs":0.}}),
            json!({"event":{"sourceId":"challenge","kind":"packet","actualMs":1.},"packet":challenge()}),
            json!({"event":{"sourceId":"setups","kind":"packet","actualMs":2.},"packet":bundle(&[setup(),setup()])}),
        ];
        let result = replay(json!({"geometry":{"css":{"width":960.,"height":540.},
            "backing":{"width":backing[0],"height":backing[1]}},
            "pointer":{"x":480.,"y":270.},"durationMs":2.,"steps":steps}))
        .unwrap();
        let draws: Vec<_> = result["body"]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["phase"] == "draw")
            .collect();
        assert_eq!(draws.len(), 3);
        for (i, row) in draws.iter().enumerate() {
            assert_eq!(
                row["quality"]["successfulDraws"].as_u64(),
                Some(i as u64 + 1)
            );
            assert_eq!(row["status"]["committed"], true);
        }
    }

    #[test]
    fn desktop_trace_matches_receipts_across_later_ack_events() {
        let step = |id: &str, kind: &str, now: f64| json!({"event":{"sourceId":id,"kind":kind,"scheduledMs":now,"actualMs":now}});
        let mut startup = step("startup", "frame", 0.);
        startup["ackAtMs"] = json!(40.);
        let mut first_ack = step("startup-ack", "input", 40.);
        first_ack["acknowledgment"] = json!(true);
        let mut next = step("next", "frame", 50.);
        next["ackAtMs"] = json!(90.);
        let backing = slither_wgpu_slice::render::backing_size([960., 540.]).unwrap();
        let mut request = json!({"lane":"desktop-ordering", "durationMs":50.,
            "geometry":{"css":{"width":960.,"height":540.},
                "backing":{"width":f64::from(backing[0]),"height":f64::from(backing[1])}},
            "pointer":{"x":480.,"y":270.}, "steps":[startup,first_ack,next]});
        let waiting = replay(request.clone()).unwrap();
        assert_eq!(waiting["body"]["final"]["draws"].as_u64(), Some(1));
        let draws: Vec<_> = waiting["body"]["samples"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s["phase"] == "draw")
            .collect();
        assert_eq!(draws.len(), 2);
        for (id, sample) in draws.iter().enumerate() {
            assert_eq!(sample["drawId"].as_u64(), Some(id as u64));
            assert_eq!(sample["quality"]["successfulDraws"].as_u64(), Some(1));
        }
        let mut last_ack = step("next-ack", "input", 90.);
        last_ack["acknowledgment"] = json!(true);
        request["steps"].as_array_mut().unwrap().push(last_ack);
        request["durationMs"] = json!(90.);
        let done = replay(request).unwrap();
        assert_eq!(done["body"]["final"]["draws"].as_u64(), Some(2));
        let ack_times: Vec<_> = done["lifecycle"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "presented")
            .map(|e| e["atMs"].as_f64().unwrap())
            .collect();
        assert_eq!(ack_times, [40., 90.]);
        let receipts = done["lifecycle"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "presented");
        for receipt in receipts {
            let event = &done["body"]["events"][receipt["eventSeq"].as_u64().unwrap() as usize];
            assert_eq!(event["actualMs"], receipt["atMs"]);
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--latency-overhead"] {
        return latency::run();
    }
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    if !args.is_empty() {
        return Err("Expected no arguments".into());
    }
    let request = serde_json::from_str(&input).map_err(|e| e.to_string())?;
    let trace = slither_wgpu_slice::latency::Run::from_env()?;
    let result = replay(request)?;
    if let Some(trace) = trace {
        trace.finish(result["body"]["final"]["completed"] == true)?;
    }
    println!(
        "{}",
        serde_json::to_string(&result).map_err(|e| e.to_string())?
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Native feel replay failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synthetic_tape::{bundle, challenge, setup, snake};

    #[test]
    fn ordinary_json_roundtrips_cadence_edges_and_unicode_keys() {
        let values = [
            1000. / 60.,
            32.99999999999999,
            33.,
            33.00000000000001,
            0.9999999999999999,
        ];
        let value = json!({"0": values, "é😀": "ordinary JSON"});
        let text = serde_json::to_string(&value).unwrap();
        let decoded: Value = serde_json::from_str(&text).unwrap();
        for (i, expected) in values.iter().enumerate() {
            assert_eq!(decoded["0"][i].as_f64().unwrap(), *expected);
        }
        assert!(serde_json::from_str::<Value>("{\"time\":1e999}").is_err());
    }

    #[test]
    fn overflow_observation_is_retained_without_claiming_a_picture_acknowledgement() {
        let mut steps = Vec::new();
        let mut step = |kind: &str, now: f64, packet: Option<Vec<u8>>| {
            let mut value = json!({"event":{"seq":steps.len(),"sourceId":format!("synthetic-{}",steps.len()),
                "kind":kind,"scheduledMs":now,"actualMs":now,"target":"host","type":kind,
                "coordinates":null,"code":null,"trusted":false,"repeat":false,"buttons":0,
                "focused":true,"visibility":"visible","effectIds":[]},"actions":[]});
            if let Some(packet) = packet {
                value["packet"] = json!(packet);
            }
            steps.push(value);
        };
        step("frame", 0., None);
        step("packet", 1., Some(challenge()));
        step("packet", 3., Some(bundle(&[setup(), snake(44)])));
        step("frame", 20., None);
        let foods = (0..60_000)
            .map(|i| vec![b'F', 11, 10, 41, (i % 245) as u8, (i / 245) as u8, 20])
            .collect::<Vec<_>>();
        for (i, commands) in foods.chunks(4000).enumerate() {
            step("packet", 30. + i as f64, Some(bundle(commands)));
        }
        step("frame", 60., None);
        let backing = slither_wgpu_slice::render::backing_size([960., 540.]).unwrap();
        let geometry = json!({"css":{"width":960,"height":540},"backing":{"width":backing[0] as f64,"height":backing[1] as f64},
            "surface":{"width":960,"height":540},"dpr":1});
        let result = replay(
            json!({"geometry":geometry,"pointer":{"x":0,"y":0},"steps":steps,"durationMs":60}),
        )
        .unwrap();
        let samples = result["body"]["samples"].as_array().unwrap();
        let draw_rows = samples
            .iter()
            .filter(|s| s["phase"] == "draw")
            .collect::<Vec<_>>();
        let acknowledged = draw_rows
            .iter()
            .filter(|s| s["status"]["committed"] == true)
            .count();
        let skipped = draw_rows.last().unwrap();
        assert_eq!(
            skipped["status"],
            json!({"prepared":true,"submitted":false,"committed":false,
            "retries":0,"sendFailed":false,"presentFailed":false})
        );
        assert_eq!(draw_rows.len(), acknowledged + 1);
        assert_eq!(skipped["quality"]["successfulDraws"], acknowledged);
        assert_eq!(result["body"]["final"]["draws"], acknowledged);
        assert_eq!(result["body"]["final"]["pendingFrames"], 0);
        assert_eq!(result["body"]["final"]["completed"], true);
        assert!(result["errors"].as_array().unwrap().is_empty());
    }
}
