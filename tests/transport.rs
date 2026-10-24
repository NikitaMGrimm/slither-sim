use serde_json::{Value, json};
use slither_sim::{Config, Observation, Simulator, Snake, transport};
use std::io::Cursor;

fn exchange(requests: &[Value]) -> Vec<Value> {
    raw_exchange(requests)
        .lines()
        .map(|line| {
            let reply: Value = serde_json::from_str(line).unwrap();
            // Replies keep the canonical bytes of a sorted-key `Value`.
            assert_eq!(serde_json::to_string(&reply).unwrap(), line);
            assert_eq!(reply["protocol"], transport::PROTOCOL);
            assert_ne!(reply.get("result").is_some(), reply.get("error").is_some());
            reply
        })
        .collect()
}

fn raw_exchange(requests: &[Value]) -> String {
    let mut input = String::new();
    for request in requests {
        input.push_str(&serde_json::to_string(request).unwrap());
        input.push('\n');
    }
    let mut output = Vec::new();
    transport::serve(Cursor::new(input), &mut output).unwrap();
    String::from_utf8(output).unwrap()
}

/// The protocol's rounding: the double nearest a multiple of `1 / steps`.
fn round(x: f64, steps: f64) -> Value {
    json!((x * steps).round() / steps + 0.0)
}
fn position(x: f64) -> Value {
    round(x, 100.0)
}

/// What a v1-style observation of `slot` looks like once rounded, rebuilt from the
/// core's own observation.
fn expected(observation: &Observation, slot: usize) -> Value {
    let snake = |s: &Snake| {
        json!({
            "slot": s.id.parse::<usize>().unwrap(),
            "x": position(s.head.x),
            "y": position(s.head.y),
            "body": s.body.iter().map(|p| [position(p.x), position(p.y)]).collect::<Vec<_>>(),
            "heading": round(s.heading, 10_000.0),
            "speed": round(s.speed, 100.0),
            "fam": s.fam, "kills": s.kills, "length": s.length, "radius": s.radius,
            "score": s.score, "segments": s.segments,
        })
    };
    let origin = |f: &slither_sim::Food| {
        f.origin.map(|o| match o {
            slither_sim::Origin::Ambient => 0,
            slither_sim::Origin::Boost => 1,
            slither_sim::Origin::Death => 2,
        })
    };
    assert_eq!(observation.me.id, slot.to_string());
    json!({
        "self": snake(&observation.me),
        "snakes": observation.opponents.iter().map(snake).collect::<Vec<_>>(),
        "food": observation.food.iter().map(|f| json!({
            "x": position(f.x), "y": position(f.y), "value": f.value,
            "size": f.size, "origin": origin(f),
        })).collect::<Vec<_>>(),
        "prey": observation.prey.iter().map(|p| json!({
            "id": p.id, "x": position(p.position.x), "y": position(p.position.y),
            "heading": round(p.heading, 10_000.0), "targetHeading": round(p.target_heading, 10_000.0),
            "speed": round(p.speed, 100.0), "size": p.size,
        })).collect::<Vec<_>>(),
    })
}

/// Slot `slot`'s view of `frame`, in the same shape as `expected`.
fn view(frame: &Value, slot: usize) -> Value {
    let view = &frame["views"][slot];
    let snakes = &frame["snakes"];
    let snake = |row: &Value| {
        let r = row.as_u64().unwrap() as usize;
        let (start, end) = (
            snakes["bodyStart"][r].as_u64().unwrap() as usize,
            snakes["bodyStart"][r + 1].as_u64().unwrap() as usize,
        );
        let mut s = json!({
            "body": (start..end)
                .map(|i| [snakes["bodyX"][i].clone(), snakes["bodyY"][i].clone()])
                .collect::<Vec<_>>(),
        });
        for column in [
            "slot", "x", "y", "heading", "speed", "fam", "kills", "length", "radius", "score",
            "segments",
        ] {
            s[column] = snakes[column][r].clone();
        }
        s
    };
    let rows = |table: &str, columns: &[&str]| {
        view[table]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| {
                let r = row.as_u64().unwrap() as usize;
                let mut item = json!({});
                for &column in columns {
                    item[column] = frame[table][column][r].clone();
                }
                item
            })
            .collect::<Vec<_>>()
    };
    json!({
        "self": snake(&view["self"]),
        "snakes": view["snakes"].as_array().unwrap().iter().map(snake).collect::<Vec<_>>(),
        "food": rows("food", &["x", "y", "value", "size", "origin"]),
        "prey": rows("prey", &["id", "x", "y", "heading", "targetHeading", "speed", "size"]),
    })
}

fn world(seed: u32, agents: usize, config: Value) -> Simulator {
    let config: Config = serde_json::from_value(config).unwrap();
    let mut world = Simulator::new(seed, config).unwrap();
    for slot in 0..agents {
        world.add(slot.to_string(), slot, agents).unwrap();
    }
    world
}

fn assert_frame_matches(frame: &Value, world: &Simulator) {
    assert_eq!(frame["time"], json!(world.time));
    assert_eq!(frame["living"], world.living());
    assert_eq!(frame["radius"], json!(world.config.radius));
    assert_eq!(frame["vision"], json!(world.config.vision));
    for (slot, observation) in world.observe_all().iter().enumerate() {
        match observation {
            Some(observation) => assert_eq!(view(frame, slot), expected(observation, slot)),
            None => assert_eq!(frame["views"][slot], Value::Null),
        }
    }
    // Every row is seen by some slot, and each item appears once.
    for (table, column) in [("snakes", "slot"), ("food", "x"), ("prey", "id")] {
        let mut seen = vec![false; frame[table][column].as_array().unwrap().len()];
        for view in frame["views"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| !v.is_null())
        {
            let key = if table == "snakes" { "snakes" } else { table };
            for row in view[key].as_array().unwrap() {
                seen[row.as_u64().unwrap() as usize] = true;
            }
            if table == "snakes" {
                seen[view["self"].as_u64().unwrap() as usize] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "unseen {table} rows");
    }
}

#[test]
fn frames_rebuild_the_core_observations() {
    let spread = |agents: usize| (0..agents).map(|i| i as f64 * 0.7).collect::<Vec<_>>();
    let mut shifted_rows = false;
    // A crowded arena with enough food for the batch food index, a sparse one, the
    // default world, and a hard boundary that kills slots 0 and 1 while 2 and 3 live
    // on, so table rows stop matching slots.
    for (agents, config, headings, boost, steps) in [
        (
            24,
            json!({"radius":1200,"dynamicRadius":false,"foodCount":3000}),
            spread(24),
            vec![false; 24],
            5,
        ),
        (
            6,
            json!({"radius":20000,"dynamicRadius":false}),
            spread(6),
            vec![false; 6],
            5,
        ),
        (4, json!({}), spread(4), vec![false; 4], 5),
        (
            4,
            json!({"radius":400,"dynamicRadius":false,"boundaryMode":"hard","foodCount":200}),
            vec![0.0, 4.0, 5.5, 0.9],
            vec![true, false, false, false],
            40,
        ),
    ] {
        let step = json!({"op":"step","actions":[{"heading":headings,"boost":boost}]});
        let mut requests = vec![json!({"op":"reset","seed":7,"agents":agents,"config":config})];
        requests.extend(std::iter::repeat_n(step, steps));
        let replies = exchange(&requests);
        let mut core = world(7, agents, config);
        assert_frame_matches(&replies[0]["result"]["worlds"][0], &core);
        assert_eq!(replies[0]["result"]["worlds"][0]["ticks"], 0);
        let actions: Vec<_> = headings
            .iter()
            .zip(&boost)
            .map(|(&heading, &boost)| Some(slither_sim::Action { heading, boost }))
            .collect();
        for reply in &replies[1..] {
            for _ in 0..6 {
                if core.living() > 0 {
                    core.step(&actions).unwrap();
                }
            }
            let frame = &reply["result"]["worlds"][0];
            assert_frame_matches(frame, &core);
            let views = frame["views"].as_array().unwrap();
            shifted_rows |= views[0].is_null() && views[1..].iter().any(|v| !v.is_null());
        }
    }
    assert!(shifted_rows, "no frame had a dead slot before a living one");
}

#[test]
fn worlds_match_separate_sessions_with_offset_seeds() {
    let config = json!({"radius":1500,"dynamicRadius":false,"foodCount":800});
    let headings = |w: usize| (0..5).map(|i| (i + w) as f64 * 1.1).collect::<Vec<_>>();
    let step = |worlds: &[usize]| {
        json!({"op":"step","actions":worlds.iter().map(|&w| json!({
            "heading": headings(w), "boost": [w % 2 == 0, false, false, true, false],
        })).collect::<Vec<_>>()})
    };
    let together = exchange(&[
        json!({"op":"reset","seed":4294967295u32,"worlds":3,"agents":5,"config":config}),
        step(&[0, 1, 2]),
        step(&[0, 1, 2]),
    ]);
    for w in 0..3 {
        let seed = 4294967295u32.wrapping_add(w as u32);
        let alone = exchange(&[
            json!({"op":"reset","seed":seed,"agents":5,"config":config}),
            step(&[w]),
            step(&[w]),
        ]);
        for (joint, single) in together.iter().zip(&alone) {
            assert_eq!(joint["result"]["worlds"][w], single["result"]["worlds"][0]);
        }
    }
    assert_eq!(together[0]["result"]["seed"], 4294967295u32);
    assert_eq!(together[0]["result"]["worlds"].as_array().unwrap().len(), 3);
}

#[test]
fn rejected_requests_preserve_every_world() {
    let ok = json!({"heading":[0,1],"boost":[false,false]});
    let snapshots = [json!({"op":"snapshot"}), json!({"op":"snapshot","world":1})];
    let replies = exchange(&[
        json!({"op":"reset","seed":3,"agents":2,"worlds":2}),
        snapshots[0].clone(),
        snapshots[1].clone(),
        // The first world's actions are valid; the second world's are not.
        json!({"op":"step","actions":[ok, {"heading":[0,null],"boost":[false,false]}]}),
        json!({"op":"step","actions":[ok, {"heading":[0],"boost":[false]}]}),
        json!({"op":"step","actions":[ok]}),
        json!({"op":"step","actions":[ok, {"heading":[0,1],"boost":[false,false],"x":1}]}),
        json!({"op":"reset","agents":0}),
        json!({"op":"reset","worlds":0}),
        json!({"op":"reset","worlds":1025}),
        json!({"op":"reset","config":{"dt":0}}),
        json!({"op":"reset","decisionTicks":1001}),
        json!({"op":"reset","config":{"unknownRule":true}}),
        json!({"op":"snapshot","world":2}),
        snapshots[0].clone(),
        snapshots[1].clone(),
    ]);
    for reply in &replies[3..14] {
        assert!(reply["error"].is_string(), "{reply}");
    }
    assert_eq!(replies[1], replies[14]);
    assert_eq!(replies[2], replies[15]);
    assert_ne!(replies[1], replies[2]);
}

#[test]
fn malformed_lines_and_unknown_requests_do_not_break_framing() {
    let input = b"not json\n\n{\"op\":\"unknown\"}\n{\"op\":\"info\",\"extra\":1}\n\xff\n{\"op\":\"observe\",\"slot\":0}\n{\"op\":\"info\"}\r\n{\"op\":\"info\"}";
    let mut output = Vec::new();
    transport::serve(Cursor::new(input), &mut output).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 8);
    for reply in &lines[..6] {
        assert_eq!(reply["protocol"], transport::PROTOCOL);
        assert!(reply["error"].is_string());
    }
    for reply in &lines[6..] {
        let info = &reply["result"];
        assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(info["defaultConfig"]["dt"], 0.007);
        assert_eq!(info["maxWorlds"], 1024);
        assert_eq!(info["origins"], json!(["ambient", "boost", "death"]));
        assert_eq!(
            info["precision"],
            json!({"angle":0.0001,"position":0.01,"speed":0.01})
        );
    }
}

#[test]
fn snapshots_exclude_hidden_state() {
    let replies = exchange(&[
        json!({"op":"snapshot"}),
        json!({"op":"reset","seed":9,"agents":2}),
        json!({"op":"snapshot"}),
    ]);
    assert_eq!(replies[0]["error"], "Reset first");
    let snapshot = replies[2]["result"].as_object().unwrap();
    assert_eq!(snapshot.len(), 6);
    for hidden in ["random", "boosts", "speeds", "joins", "kiosks"] {
        assert!(!snapshot.contains_key(hidden));
    }
    let core = world(9, 2, json!({}));
    assert_eq!(snapshot["snakes"], json!(core.snakes));
    assert_eq!(snapshot["food"], json!(core.food));
}

#[test]
fn dead_slots_remain_stable_and_all_dead_worlds_stop() {
    let replies = exchange(&[
        json!({"op":"reset","agents":1,"decisionTicks":1000,
            "config":{"radius":100,"foodCount":0,"boundaryMode":"hard"}}),
        json!({"op":"step","actions":[{"heading":[0],"boost":[false]}]}),
        json!({"op":"step","actions":[{"heading":[null],"boost":[false]}]}),
    ]);
    let death = &replies[1]["result"]["worlds"][0];
    assert_eq!(death["living"], 0);
    assert_eq!(death["views"], json!([null]));
    assert_eq!(death["snakes"]["slot"], json!([]));
    assert_eq!(death["snakes"]["bodyStart"], json!([0]));
    assert_eq!(death["deaths"].as_array().unwrap().len(), 1);
    assert_eq!(death["deaths"][0]["victim"], 0);
    assert!((1..1000).contains(&death["ticks"].as_u64().unwrap()));
    let after = &replies[2]["result"]["worlds"][0];
    assert_eq!(after["ticks"], 0);
    assert_eq!(after["time"], death["time"]);
    assert_eq!(after["deaths"], json!([]));
}

#[test]
#[cfg(feature = "networking")]
fn executable_emits_only_replies_and_exits_on_eof() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let headings = |w: usize| (0..6).map(|i| (i + 2 * w) as f64 * 0.9).collect::<Vec<_>>();
    let step = json!({"op":"step","actions":(0..4).map(|w| json!({
        "heading": headings(w), "boost": [w % 2 == 0, false, true, false, false, false],
    })).collect::<Vec<_>>()});
    let requests = [
        json!({"op":"info"}),
        json!({"op":"reset","seed":11,"worlds":4,"agents":6,
            "config":{"radius":900,"dynamicRadius":false,"foodCount":600}}),
        step.clone(),
        step.clone(),
        step,
    ];
    let in_process = raw_exchange(&requests);
    let input: String = requests
        .iter()
        .map(|request| format!("{request}\n"))
        .collect();
    let mut runs = vec![vec!["serve"]];
    // Thread counts only size the pool; replies must not depend on them.
    if cfg!(feature = "parallel") {
        runs.extend([
            vec!["serve", "--threads", "1"],
            vec!["serve", "--threads", "4"],
        ]);
    }
    for args in runs {
        let mut child = Command::new(env!("CARGO_BIN_EXE_slither-sim"))
            .args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        let result = child.wait_with_output().unwrap();
        assert!(result.status.success(), "{args:?}");
        assert!(result.stderr.is_empty());
        let text = String::from_utf8(result.stdout).unwrap();
        assert_eq!(text.lines().count(), requests.len());
        assert!(
            text == in_process,
            "{args:?} replies differ from in-process replies"
        );
    }
}
