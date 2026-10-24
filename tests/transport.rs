use serde_json::{Value, json};
use slither_sim::{Action, Config, Simulator, transport};
use std::io::Cursor;

fn exchange(requests: &[Value]) -> Vec<Value> {
    let mut input = String::new();
    for request in requests {
        input.push_str(&serde_json::to_string(request).unwrap());
        input.push('\n');
    }
    let mut output = Vec::new();
    transport::serve(Cursor::new(input), &mut output, Default::default()).unwrap();
    String::from_utf8(output)
        .unwrap()
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

#[test]
fn grouped_steps_match_the_core_exactly() {
    let reset = json!({"op":"reset","seed":7,"agents":2,"decisionTicks":6});
    let actions = vec![
        Some(Action {
            heading: std::f64::consts::PI,
            boost: false,
        }),
        Some(Action {
            heading: 0.,
            boost: false,
        }),
    ];
    let step = json!({"op":"step","actions":actions});
    let replies = exchange(&[reset, step.clone(), step, json!({"op":"snapshot"})]);
    let mut world = Simulator::new(7, Config::default()).unwrap();
    for slot in 0..2 {
        world.add(slot.to_string(), slot, 2).unwrap();
    }
    assert_eq!(
        replies[0]["result"]["observations"],
        json!(world.observe_all())
    );
    for reply in &replies[1..3] {
        for _ in 0..6 {
            world.step(&actions).unwrap();
        }
        assert_eq!(reply["result"]["observations"], json!(world.observe_all()));
        assert_eq!(reply["result"]["time"], json!(world.time));
        assert_eq!(reply["result"]["ticks"], 6);
    }
    assert_eq!(replies[3]["result"]["snakes"], json!(world.snakes));
    assert_eq!(replies[3]["result"]["food"], json!(world.food));
}

#[test]
fn rejected_batches_and_resets_preserve_the_existing_world() {
    let replies = exchange(&[
        json!({"op":"reset","seed":3,"agents":2}),
        json!({"op":"snapshot"}),
        json!({"op":"step","actions":[{"heading":0,"boost":false}]}),
        json!({"op":"step","actions":[null,null]}),
        json!({"op":"reset","agents":0}),
        json!({"op":"reset","config":{"dt":0}}),
        json!({"op":"reset","decisionTicks":1001}),
        json!({"op":"reset","config":{"unknownRule":true}}),
        json!({"op":"step","actions":[{"heading":0,"boost":false,"unknown":1},null]}),
        json!({"op":"snapshot"}),
    ]);
    for reply in &replies[2..9] {
        assert!(reply["error"].is_string(), "{reply}");
    }
    assert_eq!(replies[1], replies[9]);
}

#[test]
fn malformed_lines_and_unknown_requests_do_not_break_framing() {
    let input =
        b"not json\n\n{\"op\":\"unknown\"}\n{\"op\":\"info\",\"extra\":1}\n\xff\n{\"op\":\"info\"}\r\n{\"op\":\"info\"}";
    let mut output = Vec::new();
    transport::serve(Cursor::new(input), &mut output, Default::default()).unwrap();
    let lines: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(lines.len(), 7);
    for reply in &lines[..5] {
        assert_eq!(reply["protocol"], transport::PROTOCOL);
        assert!(reply["error"].is_string());
    }
    for reply in &lines[5..] {
        assert_eq!(reply["result"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(reply["result"]["defaultConfig"]["dt"], 0.007);
    }
}

#[test]
fn observations_are_read_only_and_snapshots_exclude_hidden_state() {
    let replies = exchange(&[
        json!({"op":"observe","slot":0}),
        json!({"op":"reset","seed":9,"agents":2}),
        json!({"op":"snapshot"}),
        json!({"op":"observe","slot":0}),
        json!({"op":"observe","slot":2}),
        json!({"op":"snapshot"}),
    ]);
    assert_eq!(replies[0]["error"], "Reset first");
    assert_eq!(
        replies[3]["result"]["observation"],
        replies[1]["result"]["observations"][0]
    );
    assert!(replies[4]["error"].is_string());
    assert_eq!(replies[2], replies[5]);
    let snapshot = replies[2]["result"].as_object().unwrap();
    assert_eq!(snapshot.len(), 6);
    for hidden in ["random", "boosts", "speeds", "joins", "kiosks"] {
        assert!(!snapshot.contains_key(hidden));
    }
}

#[test]
fn dead_slots_remain_stable_and_all_dead_worlds_stop() {
    let replies = exchange(&[
        json!({"op":"reset","agents":1,"decisionTicks":1000,
            "config":{"radius":100,"foodCount":0,"boundaryMode":"hard"}}),
        json!({"op":"step","actions":[{"heading":0,"boost":false}]}),
        json!({"op":"observe","slot":0}),
        json!({"op":"step","actions":[null]}),
    ]);
    let death = &replies[1]["result"];
    assert_eq!(death["living"], 0);
    assert_eq!(death["observations"], json!([null]));
    assert_eq!(death["deaths"].as_array().unwrap().len(), 1);
    assert!((1..1000).contains(&death["ticks"].as_u64().unwrap()));
    assert_eq!(replies[2]["result"]["observation"], Value::Null);
    assert_eq!(replies[3]["result"]["ticks"], 0);
    assert_eq!(replies[3]["result"]["time"], death["time"]);
    assert_eq!(replies[3]["result"]["deaths"], json!([]));
}

#[test]
#[cfg(feature = "networking")]
fn executable_emits_only_replies_and_exits_on_eof() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new(env!("CARGO_BIN_EXE_slither-sim"))
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"{\"op\":\"info\"}\n")
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    let text = String::from_utf8(result.stdout).unwrap();
    assert_eq!(text.lines().count(), 1);
    let reply: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(reply["protocol"], transport::PROTOCOL);
}

#[cfg(feature = "parallel")]
#[test]
fn parallel_serving_writes_the_same_bytes() {
    let mut requests = vec![json!({"op":"reset","seed":3,"agents":64})];
    for step in 0..12 {
        let actions: Vec<Value> = (0..64)
            .map(|slot| json!({"heading": f64::from(slot * step) * 0.1, "boost": slot % 5 == 0}))
            .collect();
        requests.push(json!({"op":"step","actions":actions}));
    }
    let input: String = requests.iter().map(|r| format!("{r}\n")).collect();
    let run = |parallel| {
        let mut output = Vec::new();
        let options = transport::ServeOptions { parallel };
        transport::serve(Cursor::new(input.as_bytes()), &mut output, options).unwrap();
        output
    };
    let sequential = run(false);
    let replies = String::from_utf8(sequential.clone()).unwrap();
    assert_eq!(replies.lines().count(), requests.len());
    assert!(replies.lines().all(|line| line.contains("\"result\"")));
    assert_eq!(run(true), sequential);
}
