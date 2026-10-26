use serde_json::{Value, json};
use slither_sim::{Config, Observation, Simulator, Snake, transport};
use slither_sim::{Constraint, ErrorKind};
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
    Simulator::new(slither_sim::WorldOptions {
        seed,
        config,
        population: slither_sim::InitialPopulation::Placed {
            ids: (0..agents).map(|slot| slot.to_string()).collect(),
            placement: slither_sim::Placement::TestRing,
        },
    })
    .unwrap()
}

fn assert_frame_matches(frame: &Value, world: &Simulator) {
    assert_eq!(frame["time"], json!(world.time()));
    assert_eq!(frame["living"], world.living());
    assert_eq!(frame["radius"], json!(world.radius()));
    assert_eq!(frame["vision"], json!(world.config().vision));
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
        let mut core = world(7, agents, replies[0]["result"]["config"].clone());
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
        json!({"op":"reset","agents":u64::from(u32::MAX) + 1}),
        json!({"op":"reset","config":{"dt":0}}),
        json!({"op":"reset","decisionTicks":0}),
        json!({"op":"reset","config":{"unknownRule":true}}),
        json!({"op":"snapshot","world":2}),
        snapshots[0].clone(),
        snapshots[1].clone(),
    ]);
    for reply in &replies[3..14] {
        assert!(reply["error"]["message"].is_string(), "{reply}");
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
        assert!(reply["error"]["message"].is_string());
    }
    for reply in &lines[6..] {
        let info = &reply["result"];
        assert_eq!(info["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(info["defaultConfig"]["dt"], 0.007);
        assert_eq!(info["maxAgents"], u32::MAX);
        assert_eq!(info["maxWorlds"], usize::MAX);
        assert_eq!(info["maxDecisionTicks"], u32::MAX);
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
    assert_eq!(replies[0]["error"]["message"], "Reset first");
    let snapshot = replies[2]["result"].as_object().unwrap();
    assert_eq!(snapshot.len(), 7);
    for hidden in ["random", "boosts", "speeds", "joins", "kiosks"] {
        assert!(!snapshot.contains_key(hidden));
    }
    let core = world(9, 2, replies[1]["result"]["config"].clone());
    assert_eq!(snapshot["snakes"], json!(core.snakes()));
    assert_eq!(snapshot["food"], json!(core.food()));
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

/// `frame` in the shape of its JSON frame, rebuilt from the typed columns only.
fn columns_value(frame: transport::FrameRef) -> Value {
    let index = |i: u32| (i != transport::NONE).then_some(i);
    let v = frame.views;
    assert_eq!(v.snakes_start.len(), v.me.len() + 1);
    assert_eq!(v.food_start.len(), v.me.len() + 1);
    assert_eq!(v.prey_start.len(), v.me.len() + 1);
    let rows = |start: &[u32], rows: &[u32], slot: usize| {
        rows[start[slot] as usize..start[slot + 1] as usize].to_vec()
    };
    let views: Vec<_> = (0..v.me.len())
        .map(|slot| match index(v.me[slot]) {
            None => Value::Null,
            Some(me) => json!({
                "self": me,
                "snakes": rows(v.snakes_start, v.snakes, slot),
                "food": rows(v.food_start, v.food, slot),
                "prey": rows(v.prey_start, v.prey, slot),
            }),
        })
        .collect();
    let (s, f, p, d) = (frame.snakes, frame.food, frame.prey, frame.deaths);
    json!({
        "time": frame.time, "ticks": frame.ticks, "living": frame.living,
        "radius": frame.radius, "vision": frame.vision, "rules": frame.rules,
        "snakes": {
            "slot": s.slot, "x": s.x, "y": s.y, "heading": s.heading, "speed": s.speed,
            "radius": s.radius, "length": s.length, "segments": s.segments, "fam": s.fam,
            "score": s.score, "kills": s.kills, "killScore": s.kill_score,
            "bodyStart": s.body_start,
            "bodyX": s.body_x, "bodyY": s.body_y,
        },
        "food": {
            "x": f.x, "y": f.y, "value": f.value,
            "size": f.size.iter().map(|&x| (!x.is_nan()).then_some(x)).collect::<Vec<_>>(),
            "origin": f.origin.iter().map(|&o| (o != transport::NO_ORIGIN).then_some(o))
                .collect::<Vec<_>>(),
        },
        "prey": {
            "id": p.id, "x": p.x, "y": p.y, "heading": p.heading,
            "targetHeading": p.target_heading, "speed": p.speed, "size": p.size,
        },
        "views": views,
        "deaths": (0..d.time.len()).map(|i| json!({
            "time": d.time[i], "victim": d.victim[i], "killer": index(d.killer[i]),
            "reason": transport::REASONS[d.reason[i] as usize], "food": d.food[i],
        })).collect::<Vec<_>>(),
    })
}

fn result(reply: &[u8]) -> Value {
    let reply: Value = serde_json::from_slice(reply).unwrap();
    assert!(reply.get("error").is_none(), "{reply}");
    reply["result"].clone()
}

#[test]
fn requests_answer_with_the_serve_lines() {
    let input: &[&[u8]] = &[
        b"not json",
        br#"{"op":"info"}"#,
        br#"{"op":"step","actions":[]}"#,
        br#"{"op":"reset","seed":5,"worlds":2,"agents":3}"#,
        br#"{"op":"step","actions":[{"heading":[0,1,2],"boost":[true,false,false]},{"heading":[2,1,0],"boost":[false,false,true]}]}"#,
        br#"{"op":"snapshot","world":1}"#,
        br#"{"op":"reset","config":{"preset":"sandbox","radius":60,"foodCount":0}}"#,
    ];
    let mut session = transport::Session::new();
    assert_eq!((session.worlds(), session.agents()), (0, 0));
    assert!(session.frame(0).is_none());
    let mut lines = Vec::new();
    for request in input {
        lines.extend(session.request(request));
        lines.push(b'\n');
    }
    let mut served = Vec::new();
    transport::serve(Cursor::new(input.join(&b'\n')), &mut served).unwrap();
    assert_eq!(lines, served);
    assert_eq!((session.worlds(), session.agents()), (2, 3));
    assert!(session.frame(1).is_some() && session.frame(2).is_none());
    let reply: Value = serde_json::from_slice(&session.request(input.last().unwrap())).unwrap();
    assert_eq!(
        reply["error"]["message"],
        "World 0: Initial spawn body extends outside arena"
    );
    assert_eq!(reply["sessionEnded"], false);
}

#[test]
fn typed_steps_match_json_steps() {
    let (mut seen_kill, mut seen_boundary, mut seen_partial) = (false, false, false);
    let (mut seen_pit, mut seen_score) = (false, false);
    // A hard arena where snakes die at the wall, a crowded arena with body
    // kills and a synthetic pit arena with a short kill-score window, each
    // with several worlds.
    for (reset, steps) in [
        (
            json!({"op":"reset","seed":11,"worlds":3,"agents":6,
                "config":{"radius":400,"dynamicRadius":false,"boundaryMode":"hard","foodCount":200}}),
            50,
        ),
        (
            json!({"op":"reset","seed":7,"worlds":2,"agents":24,
                "config":{"radius":1200,"dynamicRadius":false,"foodCount":3000}}),
            30,
        ),
        (
            json!({"op":"reset","seed":5,"worlds":2,"agents":24,
                "config":{"radius":700,"dynamicRadius":false,"foodCount":1000,
                    "pit":{"radius":300},"spawnRadius":500,"killScoreSeconds":0.3}}),
            60,
        ),
    ] {
        let line = serde_json::to_vec(&reset).unwrap();
        let mut by_json = transport::Session::new();
        let mut typed = transport::Session::new();
        let reply = result(&by_json.request(&line));
        assert_eq!(result(&typed.request(&line)), reply);
        let (worlds, agents) = (typed.worlds(), typed.agents());
        for w in 0..worlds {
            assert_eq!(columns_value(typed.frame(w).unwrap()), reply["worlds"][w]);
        }
        for t in 0..steps {
            let (mut headings, mut boosts, mut actions) = (Vec::new(), Vec::new(), Vec::new());
            for w in 0..worlds {
                let frame = typed.frame(w).unwrap();
                let heading: Vec<_> = (0..agents)
                    .map(|s| {
                        (frame.views.me[s] != transport::NONE)
                            .then(|| (s as f64 * 0.7 + w as f64 * 0.3 + t as f64 * 0.05) % 6.0)
                    })
                    .collect();
                let boost: Vec<_> = (0..agents).map(|s| (s + t + w) % 3 == 0).collect();
                headings.extend(heading.iter().map(|h| h.unwrap_or(f64::NAN)));
                boosts.extend(&boost);
                actions.push(json!({"heading": heading, "boost": boost}));
            }
            let request = json!({"op":"step","actions":actions});
            let reply = result(&by_json.request(&serde_json::to_vec(&request).unwrap()));
            typed.step(&headings, &boosts).unwrap();
            for w in 0..worlds {
                let frame = &reply["worlds"][w];
                assert_eq!(columns_value(typed.frame(w).unwrap()), *frame);
                assert_eq!(columns_value(by_json.frame(w).unwrap()), *frame);
                for death in frame["deaths"].as_array().unwrap() {
                    seen_kill |= death["killer"].is_u64();
                    seen_boundary |= death["reason"] == "boundary";
                    seen_pit |= death["reason"] == "pit";
                }
                seen_score |= frame["snakes"]["killScore"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|score| score.as_u64() > Some(0));
                let living = frame["living"].as_u64().unwrap() as usize;
                seen_partial |= 0 < living && living < agents;
            }
        }
        for w in 0..worlds {
            let snapshot = serde_json::to_vec(&json!({"op":"snapshot","world":w})).unwrap();
            assert_eq!(by_json.request(&snapshot), typed.request(&snapshot));
        }
    }
    assert!(seen_kill && seen_boundary && seen_partial && seen_pit && seen_score);
}

#[test]
fn rejected_typed_steps_change_nothing() {
    let mut session = transport::Session::new();
    assert!(
        matches!(session.step(&[], &[]), Err(transport::StepError::Rejected(ref e)) if e.kind == ErrorKind::NotReset)
    );
    result(&session.request(br#"{"op":"reset","seed":3,"agents":2,"worlds":2}"#));
    let state = |session: &mut transport::Session| {
        (0..2)
            .map(|w| {
                let line = format!(r#"{{"op":"snapshot","world":{w}}}"#);
                let frame = columns_value(session.frame(w).unwrap());
                (session.request(line.as_bytes()), frame)
            })
            .collect::<Vec<_>>()
    };
    let before = state(&mut session);
    // The first world's actions are valid; the second world's are not.
    let error = session
        .step(&[0.0, 1.0, 0.0, f64::NAN], &[false; 4])
        .unwrap_err();
    let transport::StepError::Rejected(cause) = &error else {
        panic!("{error}")
    };
    assert_eq!(
        cause.kind,
        ErrorKind::InvalidAction {
            rule: Constraint::Missing
        }
    );
    assert_eq!(
        (cause.world, cause.slot, cause.field.as_deref()),
        (Some(1), Some(1), Some("heading"))
    );
    let reply: Value = serde_json::from_slice(&session.request(
        br#"{"op":"step","actions":[{"heading":[0,1],"boost":[false,false]},{"heading":[0,null],"boost":[false,false]}]}"#,
    ))
    .unwrap();
    assert_eq!(reply["error"]["message"], error.to_string());
    assert_eq!(reply["sessionEnded"], false);
    for (headings, boosts) in [
        (&[0.0, 1.0, 0.0, f64::INFINITY][..], &[false; 4][..]),
        (&[0.0, 1.0, 0.0, f64::NEG_INFINITY], &[false; 4]),
        (&[0.0, 1.0, 0.0], &[false; 3]),
        (&[0.0, 1.0, 0.0, 1.0], &[false; 3]),
        (&[0.0; 6], &[false; 6]),
    ] {
        assert!(session.step(headings, boosts).is_err());
    }
    assert_eq!(state(&mut session), before);
    session.step(&[0.0, 1.0, 2.0, 3.0], &[false; 4]).unwrap();
    assert_ne!(state(&mut session), before);
}

#[test]
fn terminal_steps_clear_all_worlds_and_allow_reset() {
    use slither_sim::transport::StepError;
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/serve-errors-v5.json")).unwrap();
    let reset = serde_json::to_vec(&fixture["reset"]).unwrap();
    let step = serde_json::to_vec(&fixture["step"]).unwrap();
    let mut session = transport::Session::new();
    result(&session.request(&reset));
    assert_eq!(session.worlds(), 2);
    let error = session
        .step(&[std::f64::consts::PI; 2], &[false; 2])
        .unwrap_err();
    let StepError::Ended(cause) = &error else {
        panic!("{error}")
    };
    assert_eq!(cause.kind, ErrorKind::NumericOverflow);
    // Both worlds fail: parallel completion order must not choose the cause.
    assert_eq!(
        (cause.world, cause.slot, cause.field.as_deref()),
        (Some(0), Some(0), Some("fam"))
    );
    assert_eq!((session.worlds(), session.agents()), (0, 0));
    assert!(session.frame(0).is_none() && session.frame(1).is_none());
    assert!(
        matches!(session.step(&[], &[]), Err(StepError::Rejected(ref e)) if e.kind == ErrorKind::NotReset)
    );
    let rejected: Value = serde_json::from_slice(&session.request(&step)).unwrap();
    assert_eq!(rejected, fixture["rejectedReply"]);
    result(&session.request(br#"{"op":"reset","agents":1}"#));
    session.step(&[0.0], &[false]).unwrap();
    assert!(session.frame(0).unwrap().time > 0.0);

    // Exercise the stream and JSON error envelope independently of typed steps.
    let replies = exchange(&[
        fixture["step"].clone(),
        fixture["reset"].clone(),
        fixture["step"].clone(),
        json!({"op":"snapshot"}),
        json!({"op":"reset","agents":1}),
        json!({"op":"step","actions":[{"heading":[0],"boost":[false]}]}),
    ]);
    assert_eq!(replies[0], fixture["rejectedReply"]);
    assert_eq!(replies[2]["error"]["message"], error.to_string());
    let mut terminal = replies[2].clone();
    terminal["error"]["message"] = fixture["terminalReply"]["error"]["message"].clone();
    assert_eq!(terminal, fixture["terminalReply"]);
    assert_eq!(replies[3], fixture["rejectedReply"]);
    assert!(replies[5]["result"]["worlds"][0]["time"].as_f64().unwrap() > 0.0);
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
        assert_eq!(json_line(&in_process, 1)["result"]["warnings"], json!([]));
        let text = String::from_utf8(result.stdout).unwrap();
        assert_eq!(text.lines().count(), requests.len());
        assert!(
            text == in_process,
            "{args:?} replies differ from in-process replies"
        );
    }
}

#[cfg(feature = "networking")]
fn json_line(text: &str, index: usize) -> Value {
    serde_json::from_str(text.lines().nth(index).unwrap()).unwrap()
}

#[test]
#[cfg(feature = "networking")]
fn reset_warnings_reach_the_reply_and_only_stderr() {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let request = json!({"op":"reset","config":{"dt":0.01}});
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
        .write_all(format!("{request}\n").as_bytes())
        .unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success());
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    let warnings = json_line(&stdout, 0)["result"]["warnings"].clone();
    assert_eq!(warnings.as_array().unwrap().len(), 1);
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert_eq!(
        stderr.trim_end(),
        format!("warning: {}", warnings[0].as_str().unwrap())
    );
}

#[test]
fn reset_sizes_are_bounded_by_frame_types_and_memory_not_policy() {
    let small = json!({"radius":300,"dynamicRadius":false,"foodCount":0,"prey":{"count":0}});
    let wide = json!({"radius":5000,"dynamicRadius":false,"foodCount":0,"prey":{"count":0}});
    let replies = exchange(&[
        json!({"op":"reset","worlds":1025,"agents":1,"config":small}),
        json!({"op":"reset","agents":1,"decisionTicks":1001,"config":wide}),
        json!({"op":"step","actions":[{"heading":[0],"boost":[false]}]}),
        json!({"op":"reset","agents":129,"config":small}),
        json!({"op":"reset","agents":u64::from(u32::MAX) + 1,"config":small}),
        json!({"op":"reset","decisionTicks":u64::from(u32::MAX) + 1,"config":small}),
        json!({"op":"reset","worlds":usize::MAX,"agents":2,"config":small}),
        json!({"op":"reset","worlds":usize::MAX,"agents":1,"config":small}),
        json!({"op":"reset","worlds":0}),
        json!({"op":"snapshot"}),
    ]);
    assert_eq!(
        replies[0]["result"]["worlds"].as_array().unwrap().len(),
        1025
    );
    assert_eq!(replies[1]["result"]["decisionTicks"], 1001);
    assert_eq!(replies[2]["result"]["worlds"][0]["ticks"], 1001);
    let errors: Vec<&str> = replies[4..9]
        .iter()
        .map(|reply| reply["error"]["message"].as_str().unwrap())
        .collect();
    assert!(errors[0].contains("u32::MAX means none"), "{}", errors[0]);
    assert!(errors[1].contains("ticks as u32"), "{}", errors[1]);
    assert!(errors[2].contains("worlds * agents"), "{}", errors[2]);
    assert!(errors[3].contains("do not fit in memory"), "{}", errors[3]);
    assert!(errors[4].contains("at least one world"), "{}", errors[4]);
    // Rejected resets keep the 129-agent session.
    assert_eq!(
        replies[9]["result"]["snakes"].as_array().unwrap().len(),
        129
    );
}

#[test]
fn resolved_dynamic_radius_reset_replays_the_initial_frame() {
    let mut session = transport::Session::new();
    let mut reset = json!({"op":"reset", "seed":1, "agents":1,
        "config":{"radius":1000., "dynamicRadius":true, "foodCount":0,
        "ambientReplenishPerSecond":0., "prey":{"count":0}}});
    let first: Value =
        serde_json::from_slice(&session.request(&serde_json::to_vec(&reset).unwrap())).unwrap();
    reset["config"] = first["result"]["config"].clone();
    let replay: Value =
        serde_json::from_slice(&session.request(&serde_json::to_vec(&reset).unwrap())).unwrap();
    assert_eq!(first, replay);
    assert_eq!(first["result"]["worlds"][0]["radius"], 1000.);
}

#[test]
fn initial_config_and_live_radius_survive_reset_replay_and_samples() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/serve-radius-v5.json")).unwrap();
    let mut session = transport::Session::new();
    let first = result(&session.request(&serde_json::to_vec(&fixture["reset"]).unwrap()));
    assert_eq!(first["config"]["radius"], fixture["initialRadius"]);
    assert_eq!(first["worlds"][0]["radius"], fixture["liveRadii"][0]);
    let mut replay = transport::Session::new();
    let mut resolved = fixture["reset"].clone();
    resolved["config"] = first["config"].clone();
    assert_eq!(
        result(&replay.request(&serde_json::to_vec(&resolved).unwrap())),
        first
    );
    for radius in fixture["liveRadii"].as_array().unwrap() {
        let snapshot = result(&session.request(br#"{"op":"snapshot"}"#));
        assert_eq!(snapshot["config"]["radius"], fixture["initialRadius"]);
        assert_eq!(snapshot["radius"], *radius);
        assert_eq!(session.frame(0).unwrap().radius, radius.as_f64().unwrap());
        session.step(&[0.], &[false]).unwrap();
    }
}
