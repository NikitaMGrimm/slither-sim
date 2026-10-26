//! Untimed typed/JSON session parity trace. Shared by perf_build overlays.
#[path = "support/runtime.rs"]
mod runtime;
use serde_json::{Value, json};
use slither_sim::{Config, transport::Session};
use std::io::{self, Write};

fn emit(
    out: &mut impl Write,
    mut value: Value,
    expected_rules: &Value,
) -> Result<(), Box<dyn std::error::Error>> {
    for key in ["replyBytes", "snapshotBytes"] {
        if let Some(bytes) = value.get_mut(key) {
            let raw: Vec<u8> = serde_json::from_value(bytes.clone())?;
            *bytes = serde_json::to_value(runtime::geometric_reply(&raw, expected_rules)?)?;
        }
    }
    serde_json::to_writer(&mut *out, &value)?;
    writeln!(out)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).is_some_and(|arg| arg == "--bench") {
        return bench(
            args.get(2).map(String::as_str).unwrap_or("typed"),
            args.get(3).map(|s| s.parse()).transpose()?.unwrap_or(1000),
        );
    }
    let mut out = io::BufWriter::new(io::stdout().lock());
    for (name, config, agents) in [
        ("sandbox", Config::sandbox(), 4),
        ("official", Config::official(128), 128),
        ("battledome", Config::battledome(), 32),
    ] {
        let expected_rules = runtime::rules(&config);
        let mut typed = Session::new();
        let mut encoded = Session::new();
        let request = serde_json::to_vec(&json!({"op":"reset", "seed":7, "worlds":2,
            "agents":agents, "decisionTicks":6, "config":config}))?;
        let reset = encoded.request(&request);
        let reset_value: Value = serde_json::from_slice(&reset)?;
        assert!(reset_value.get("error").is_none(), "{reset_value}");
        #[cfg(slither_private_state)]
        typed.reset(slither_sim::transport::ResetOptions {
            seed: 7,
            worlds: std::num::NonZeroUsize::new(2).unwrap(),
            agents: std::num::NonZeroU32::new(agents).unwrap(),
            config,
            ..Default::default()
        })?;
        #[cfg(not(slither_private_state))]
        assert_eq!(typed.request(&request), reset);
        emit(
            &mut out,
            json!({"kind":"reset", "case":name, "replyBytes":reset}),
            &expected_rules,
        )?;
        if name == "sandbox" {
            let request =
                br#"{"op":"reset","config":{"preset":"sandbox","radius":60,"foodCount":0}}"#;
            let rejection = encoded.request(request);
            assert_eq!(typed.request(request), rejection);
            let value: Value = serde_json::from_slice(&rejection)?;
            let message = value["error"].get("message").unwrap_or(&value["error"]);
            assert!(
                message
                    .as_str()
                    .unwrap()
                    .ends_with("Initial spawn body extends outside arena")
            );
            emit(
                &mut out,
                json!({"kind":"rejectedReset", "case":name, "replyBytes":rejection}),
                &expected_rules,
            )?;
        }
        for tick in 0..=32 {
            for world in 0..2 {
                let left = runtime::frame_debug(typed.frame(world).unwrap(), &expected_rules);
                let right = runtime::frame_debug(encoded.frame(world).unwrap(), &expected_rules);
                assert_eq!(left, right, "{name} tick {tick} world {world}");
                let snapshot = serde_json::to_vec(&json!({"op":"snapshot", "world":world}))?;
                let typed_state = typed.request(&snapshot);
                assert_eq!(typed_state, encoded.request(&snapshot));
                emit(
                    &mut out,
                    json!({"kind":"frame", "case":name, "tick":tick,
                    "world":world, "columns":left, "snapshotBytes":typed_state}),
                    &expected_rules,
                )?;
            }
            if tick == 32 {
                break;
            }
            let headings: Vec<_> = (0..2 * agents)
                .map(|i| (i * 37 + tick * 3) as f64)
                .collect();
            let boosts: Vec<_> = (0..2 * agents).map(|i| (i + tick) % 9 < 3).collect();
            typed.step(&headings, &boosts)?;
            let n = agents as usize;
            let actions: Vec<_> = (0..2)
                .map(|w| {
                    json!({"heading":headings[w*n..(w+1)*n],
                "boost":boosts[w*n..(w+1)*n]})
                })
                .collect();
            let reply = encoded.request(&serde_json::to_vec(
                &json!({"op":"step", "actions":actions}),
            )?);
            emit(
                &mut out,
                json!({"kind":"step", "case":name, "replyBytes":reply}),
                &expected_rules,
            )?;
        }
    }
    emit(
        &mut out,
        json!({"kind":"end", "cases":3, "decisions":32}),
        &Value::Null,
    )?;
    out.flush()?;
    Ok(())
}

fn bench(mode: &str, iterations: usize) -> Result<(), Box<dyn std::error::Error>> {
    if !matches!(mode, "typed" | "json") || !(1..=100_000).contains(&iterations) {
        return Err("--bench typed|json 1..100000".into());
    }
    let config = Config {
        radius: 20_000.,
        food_count: 350,
        ..Config::sandbox()
    };
    let reset = serde_json::to_vec(
        &json!({"op":"reset","seed":7,"agents":128,"decisionTicks":6,"config":config}),
    )?;
    let headings: Vec<_> = (0..128)
        .map(|s| s as f64 / 128. * std::f64::consts::TAU + std::f64::consts::PI)
        .collect();
    let boosts = vec![false; 128];
    let step =
        serde_json::to_vec(&json!({"op":"step","actions":[{"heading":headings,"boost":boosts}]}))?;
    let mut session = Session::new();
    let reset_session = |session: &mut Session| -> Result<(), Box<dyn std::error::Error>> {
        if mode == "typed" {
            session.reset(slither_sim::transport::ResetOptions {
                seed: 7,
                agents: std::num::NonZeroU32::new(128).unwrap(),
                config: config.clone(),
                ..Default::default()
            })?;
        } else {
            std::hint::black_box(session.request(&reset));
        }
        Ok(())
    };
    reset_session(&mut session)?;
    for _ in 0..20 {
        session.step(&headings, &boosts)?;
    }
    reset_session(&mut session)?;
    let start = std::time::Instant::now();
    for i in 0..iterations {
        if i > 0 && i % 100 == 0 {
            reset_session(&mut session)?;
        }
        if mode == "typed" {
            session.step(&headings, &boosts)?;
            std::hint::black_box(session.frame(0).unwrap());
        } else {
            std::hint::black_box(session.request(&step));
        }
        assert_eq!(session.frame(0).unwrap().living, 128);
    }
    let wall = start.elapsed().as_secs_f64();
    println!(
        "{}",
        json!({"kind":"report","case":format!("session-{mode}-128"),"iterations":iterations,"wallSeconds":wall,"physicsTicks":iterations*6,"living":128})
    );
    Ok(())
}
