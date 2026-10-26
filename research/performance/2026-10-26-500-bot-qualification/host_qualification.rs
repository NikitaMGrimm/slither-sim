//! Study extension of examples/host_load.rs: fixed hold, raw receipts and counters.
use serde_json::json;
use slither_sim::{
    Config,
    client::{
        BotConfig, Host, HostConfig, LatencyConfig, PlayerConfig, SimulationConfig,
        network::login_packet, protocol::split_frame,
    },
};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tungstenite::{Message, connect, stream::MaybeTlsStream};

fn unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 5 {
        return Err("host_qualification OUTPUT BOT_THREADS WARMUP HOLD".into());
    }
    let out = PathBuf::from(&args[1]);
    let bot_threads: usize = args[2].parse().map_err(|_| "threads")?;
    let warmup: f64 = args[3].parse().map_err(|_| "warmup")?;
    let hold: f64 = args[4].parse().map_err(|_| "hold")?;
    let config = HostConfig {
        bind: "127.0.0.1:0".parse().unwrap(),
        seed: 7,
        players: PlayerConfig {
            max: 1,
            latency: LatencyConfig::fixed(0.04),
        },
        bots: BotConfig {
            count: 500,
            threads: bot_threads,
            latency: LatencyConfig::fixed(0.04),
        },
        simulation: SimulationConfig {
            world: Config::official(501),
            ..Default::default()
        },
    };
    let host = Host::bind(config.clone())?.start()?;
    let epoch = Instant::now();
    let epoch_unix = unix();
    let start = epoch + Duration::from_secs_f64(warmup);
    let end = start + Duration::from_secs_f64(hold);
    fs::write(
        out.join("ready.json"),
        serde_json::to_vec(&json!({
            "startUnix": epoch_unix + warmup, "endUnix": epoch_unix + warmup + hold,
            "config": config, "policy": format!("{:?}", slither_sim::bot::PolicyId::default()),
            "parameterManifestSha256": slither_sim::bot::PARAMETER_MANIFEST_SHA256,
        }))
        .unwrap(),
    )
    .map_err(|e| e.to_string())?;
    let url = format!("ws://{}/slither", host.local_addr());
    let watcher_out = out.clone();
    let watcher = thread::Builder::new().name("study-watcher".into()).spawn(move || -> Result<(), String> {
        let mut rows = Vec::new();
        let mut populations = Vec::new();
        let mut connections = 0_u64;
        let mut failures = 0_u64;
        let mut timeouts = 0_u64;
        let mut playing_seconds = 0.;
        let mut max_steering_lateness_ms = 0_f64;
        while Instant::now() < end {
            let (mut socket, _) = connect(url.as_str()).map_err(|e| e.to_string())?;
            if let MaybeTlsStream::Plain(tcp) = socket.get_mut() {
                tcp.set_read_timeout(Some(Duration::from_millis(100))).map_err(|e| e.to_string())?;
                tcp.set_nodelay(true).map_err(|e| e.to_string())?;
            }
            socket.send(Message::Binary(login_packet("Watcher", 0)?.into())).map_err(|e| e.to_string())?;
            connections += 1;
            let mut server_ms = 0_u64;
            let mut setup = false;
            let mut playing_since = None;
            let mut steer = Instant::now();
            while Instant::now() < end {
                let message = match socket.read() {
                    Ok(message) => message,
                    Err(tungstenite::Error::Io(e)) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => {
                        timeouts += 1;
                        continue;
                    }
                    Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => break,
                    Err(error) => { failures += 1; eprintln!("watcher read: {error}"); break; }
                };
                let now = Instant::now();
                if matches!(message, Message::Close(_)) { break; }
                if let Message::Binary(bytes) = message {
                    if bytes.len() < 3 { continue; }
                    server_ms += u64::from(u16::from_be_bytes([bytes[0], bytes[1]]));
                    let packets = split_frame(&bytes)?;
                    let is_setup = packets.iter().any(|p| p[0] == b'a');
                    if is_setup { setup = true; server_ms = 0; }
                    // The first full appearance is self; short s packets remove rivals.
                    if packets.iter().any(|p| p[0] == b's' && p.len() > 7) && playing_since.is_none() { playing_since = Some(now.max(start)); }
                    if setup && !is_setup && now >= start && now < end {
                        rows.push((connections, now.duration_since(epoch).as_secs_f64() * 1000., server_ms, bytes[2]));
                        for packet in &packets {
                            if packet[0] == b'l' && packet.len() >= 6 {
                                populations.push(u16::from_be_bytes([packet[4], packet[5]]));
                            }
                        }
                    }
                    if packets.iter().any(|p| p[0] == b'v') { break; }
                }
                if setup && now >= steer {
                    max_steering_lateness_ms = max_steering_lateness_ms.max(now.duration_since(steer).as_secs_f64() * 1000.);
                    let heading = (now.duration_since(epoch).as_secs_f64() * 1.2) % std::f64::consts::TAU;
                    let byte = (heading * 251. / std::f64::consts::TAU) as u8;
                    if socket.send(Message::Binary(vec![byte].into())).is_err() { failures += 1; break; }
                    steer = now + Duration::from_millis(50);
                }
            }
            if let Some(since) = playing_since {
                playing_seconds += Instant::now().min(end).saturating_duration_since(since).as_secs_f64();
            }
            let _ = socket.close(None);
            if Instant::now() < end { thread::sleep(Duration::from_millis(250)); }
        }
        let mut csv = BufWriter::new(File::create(watcher_out.join("frames.csv")).map_err(|e| e.to_string())?);
        writeln!(csv, "connection,receipt_ms,server_ms,type").map_err(|e| e.to_string())?;
        for (connection, receipt, server, kind) in &rows {
            writeln!(csv, "{connection},{receipt:.6},{server},{kind}").map_err(|e| e.to_string())?;
        }
        csv.flush().map_err(|e| e.to_string())?;
        fs::write(watcher_out.join("watcher.json"), serde_json::to_vec_pretty(&json!({
            "frames":rows.len(), "connections":connections, "readOrSendFailures":failures, "readTimeouts":timeouts,
            "playingSeconds":playing_seconds, "maxSteeringLatenessMs":max_steering_lateness_ms,
            "leaderboardPopulationSamples":populations,
        })).unwrap()).map_err(|e| e.to_string())
    }).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    let mut boundary = 0;
    let mut next = epoch;
    loop {
        let now = Instant::now();
        let due = if boundary == 0 {
            next.min(start)
        } else {
            next.min(end)
        };
        if now >= due {
            let label = if now >= end {
                "end"
            } else if boundary == 0 && now >= start {
                "start"
            } else {
                "sample"
            };
            rows.push(json!({"unix": epoch_unix + now.duration_since(epoch).as_secs_f64(),
                "ticks":host.ticks(), "deferred":host.deferred_ticks(), "simulationSeconds":host.simulation_seconds(),
                "players":host.players(), "boundary":label}));
            if now >= end {
                break;
            }
            if now >= start {
                boundary = 1;
            }
            next = now + Duration::from_secs(1);
        }
        thread::sleep(Duration::from_millis(1));
    }
    // Keep all host threads alive until the external /proc end snapshot completes.
    thread::sleep(Duration::from_secs(2));
    watcher.join().map_err(|_| "watcher panicked")??;
    host.stop()?;
    fs::write(
        out.join("host.json"),
        serde_json::to_vec_pretty(&json!({"rows":rows})).unwrap(),
    )
    .map_err(|e| e.to_string())
}
