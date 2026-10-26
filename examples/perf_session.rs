//! Untimed typed/JSON session parity trace. Shared by perf_build overlays.
use serde_json::{Value, json};
use slither_sim::{Config, transport::Session};
use std::io::{self, Write};

fn emit(out: &mut impl Write, value: Value) -> Result<(), Box<dyn std::error::Error>> {
    serde_json::to_writer(&mut *out, &value)?;
    writeln!(out)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    for (name, config, agents) in [
        ("sandbox", Config::sandbox(), 4),
        ("official", Config::official(128), 128),
        ("battledome", Config::battledome(), 32),
    ] {
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
        )?;
        for tick in 0..=32 {
            for world in 0..2 {
                let left = format!("{:?}", typed.frame(world).unwrap());
                let right = format!("{:?}", encoded.frame(world).unwrap());
                assert_eq!(left, right, "{name} tick {tick} world {world}");
                let snapshot = serde_json::to_vec(&json!({"op":"snapshot", "world":world}))?;
                let typed_state = typed.request(&snapshot);
                assert_eq!(typed_state, encoded.request(&snapshot));
                emit(
                    &mut out,
                    json!({"kind":"frame", "case":name, "tick":tick,
                    "world":world, "columns":left, "snapshotBytes":typed_state}),
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
            )?;
        }
    }
    emit(&mut out, json!({"kind":"end", "cases":3, "decisions":32}))?;
    out.flush()?;
    Ok(())
}
