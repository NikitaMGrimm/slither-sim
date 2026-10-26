//! Load a local host with background bots and measure how evenly frames arrive.
//!
//! ```sh
//! cargo run --release --locked --features networking --example host_load -- --bots 500 --seconds 60
//! ```
//!
//! Every connection has the same fixed round trip without jitter, so a frame's
//! receipt minus its server time is constant when the host delivers on time.
//! A loopback watcher circles in place and reports that excess delay: its
//! spread is host scheduling error, not modeled network. The host's simulation
//! load and deferred ticks show whether one core keeps up.
use slither_sim::{
    Config,
    client::{
        BotConfig, Host, HostConfig, LatencyConfig, PlayerConfig, SimulationConfig,
        network::login_packet,
    },
};
use std::{
    process::ExitCode,
    time::{Duration, Instant},
};
use tungstenite::{Message, connect};

const USAGE: &str =
    "Usage: host_load [--bots N] [--bot-threads N] [--seconds N] [--round-trip-ms N]";
/// Excess delays are measured against the smallest within this many frames.
const WINDOW: usize = 1200;

struct Options {
    bots: usize,
    bot_threads: usize,
    seconds: f64,
    round_trip: f64,
}

fn options() -> Result<Options, String> {
    let mut options = Options {
        bots: 500,
        bot_threads: 2,
        seconds: 60.,
        round_trip: 0.04,
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    for pair in args.chunks(2) {
        let [flag, value] = pair else {
            return Err(USAGE.into());
        };
        let number = |value: &str| value.parse::<f64>().map_err(|_| USAGE.to_string());
        match flag.as_str() {
            "--bots" => options.bots = number(value)? as usize,
            "--bot-threads" => options.bot_threads = number(value)? as usize,
            "--seconds" => options.seconds = number(value)?,
            "--round-trip-ms" => options.round_trip = number(value)? / 1000.,
            _ => return Err(USAGE.into()),
        }
    }
    Ok(options)
}

fn percentile(sorted: &[f64], share: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * share).round() as usize]
}

fn run(options: Options) -> Result<(), String> {
    println!(
        "policy=A4-mean parameter_manifest_sha256={}",
        slither_sim::bot::PARAMETER_MANIFEST_SHA256,
    );
    let config = HostConfig {
        bind: "127.0.0.1:0".parse().map_err(|_| "bad address")?,
        players: PlayerConfig {
            max: 1,
            latency: LatencyConfig::fixed(options.round_trip),
        },
        bots: BotConfig {
            count: options.bots,
            threads: options.bot_threads,
            latency: LatencyConfig::fixed(options.round_trip),
        },
        simulation: SimulationConfig {
            world: Config::official(options.bots + 1),
            ..Default::default()
        },
        ..Default::default()
    };
    let host = Host::bind(config)?.start()?;
    let url = format!("ws://{}/slither", host.local_addr());
    let (mut socket, _) = connect(url.as_str()).map_err(|e| e.to_string())?;
    socket
        .send(Message::Binary(login_packet("Watcher", 0)?.into()))
        .map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut first: Option<Instant> = None;
    let mut server_ms = 0_u64;
    let mut excess = Vec::new();
    let mut steered = start;
    let mut report = start + Duration::from_secs(5);
    let mut busy = (0., start);
    while start.elapsed().as_secs_f64() < options.seconds {
        let message = socket.read().map_err(|e| e.to_string())?;
        let received = Instant::now();
        let Message::Binary(bytes) = message else {
            continue;
        };
        if bytes.len() < 2 {
            continue;
        }
        server_ms += u64::from(u16::from_be_bytes([bytes[0], bytes[1]]));
        let first = *first.get_or_insert(received);
        excess.push(received.duration_since(first).as_secs_f64() - server_ms as f64 / 1000.);
        if received >= steered {
            // A slow circle keeps the watcher alive and in one place.
            let heading = (start.elapsed().as_secs_f64() * 1.2) % std::f64::consts::TAU;
            let byte = (heading * 251. / std::f64::consts::TAU) as u8;
            socket
                .send(Message::Binary(vec![byte].into()))
                .map_err(|e| e.to_string())?;
            steered = received + Duration::from_millis(50);
        }
        if received >= report {
            let seconds = host.simulation_seconds();
            let load = (seconds - busy.0) / received.duration_since(busy.1).as_secs_f64();
            busy = (seconds, received);
            eprintln!(
                "t={:5.1}s ticks={} deferred={} simulation load={:.0}%",
                start.elapsed().as_secs_f64(),
                host.ticks(),
                host.deferred_ticks(),
                load * 100.
            );
            report += Duration::from_secs(5);
        }
    }
    let ticks = host.ticks();
    let deferred = host.deferred_ticks();
    let load = host.simulation_seconds() / start.elapsed().as_secs_f64();
    host.stop()?;
    let mut relative: Vec<f64> = excess
        .chunks(WINDOW)
        .flat_map(|chunk| {
            let least = chunk.iter().copied().fold(f64::INFINITY, f64::min);
            chunk.iter().map(move |delay| (delay - least) * 1000.)
        })
        .collect();
    relative.sort_by(f64::total_cmp);
    if relative.is_empty() {
        return Err("The watcher received no frames".into());
    }
    println!(
        "bots={} frames={} ticks={ticks} deferred={deferred} simulation load={:.0}%",
        options.bots,
        relative.len(),
        load * 100.
    );
    println!(
        "excess delay ms: p50={:.2} p90={:.2} p99={:.2} max={:.2}",
        percentile(&relative, 0.5),
        percentile(&relative, 0.9),
        percentile(&relative, 0.99),
        relative[relative.len() - 1]
    );
    Ok(())
}

fn main() -> ExitCode {
    match options().and_then(run) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

// The PR harness reuses this host workload with real socket recipients and no
// background bots. The load generator is a child process, outside host CPU.
#[cfg(target_os = "linux")]
pub fn cpu_seconds() -> Option<f64> {
    #[repr(C)]
    struct Timespec {
        seconds: std::os::raw::c_long,
        nanos: std::os::raw::c_long,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, time: *mut Timespec) -> i32;
    }
    let mut time = Timespec {
        seconds: 0,
        nanos: 0,
    };
    // Linux CLOCK_PROCESS_CPUTIME_ID includes all host threads, not the driver.
    if unsafe { clock_gettime(2, &mut time) } != 0 {
        return None;
    }
    Some(time.seconds as f64 + time.nanos as f64 * 1e-9)
}
#[cfg(not(target_os = "linux"))]
pub fn cpu_seconds() -> Option<f64> {
    None
}

pub fn socket_driver() -> Result<(), String> {
    use std::{
        io::{self, Write},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        thread,
    };
    let args: Vec<_> = std::env::args().collect();
    let url = args.get(2).ok_or("missing driver URL")?;
    let count: usize = args
        .get(3)
        .ok_or("missing client count")?
        .parse()
        .map_err(|_| "bad client count")?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    thread::spawn(move || {
        use std::io::Read;
        let _ = io::stdin().read(&mut [0]);
        stopped.store(true, Ordering::Relaxed);
    });
    let mut sockets = Vec::with_capacity(count);
    for i in 0..count {
        let (mut socket, _) = connect(url.as_str()).map_err(|e| e.to_string())?;
        socket
            .send(Message::Binary(
                login_packet(&format!("Load{i}"), 0)?.into(),
            ))
            .map_err(|e| e.to_string())?;
        match socket.get_mut() {
            tungstenite::stream::MaybeTlsStream::Plain(stream) => {
                stream.set_nonblocking(true).map_err(|e| e.to_string())?
            }
            _ => return Err("load driver requires plain loopback".into()),
        }
        sockets.push(socket);
    }
    println!("ready");
    io::stdout().flush().map_err(|e| e.to_string())?;
    let started = Instant::now();
    let mut steer = started;
    let (mut packets, mut bytes, mut joins, mut deaths) = (0u64, 0u64, 0u64, 0u64);
    let mut playing = vec![false; count];
    let mut own_ids = vec![None; count];
    let mut minimum_playing = count;
    while !stop.load(Ordering::Relaxed) {
        let command = Instant::now() >= steer;
        for (i, socket) in sockets.iter_mut().enumerate() {
            for _ in 0..64 {
                match socket.read() {
                    Ok(Message::Binary(data)) => {
                        packets += 1;
                        bytes += data.len() as u64;
                        if data.get(2) == Some(&b's') && data.len() > 10 {
                            let id = u16::from_be_bytes([data[3], data[4]]);
                            if own_ids[i].is_none() {
                                own_ids[i] = Some(id);
                                joins += 1;
                            }
                            if own_ids[i] == Some(id) {
                                playing[i] = true;
                            }
                        }
                        if data.get(2) == Some(&b'v') {
                            deaths += 1;
                            playing[i] = false;
                            own_ids[i] = None;
                            socket
                                .write(Message::Binary(
                                    login_packet(&format!("Load{i}"), 0)?.into(),
                                ))
                                .map_err(|e| e.to_string())?;
                        }
                    }
                    Ok(_) => {}
                    Err(tungstenite::Error::Io(e))
                        if e.kind() == std::io::ErrorKind::WouldBlock =>
                    {
                        break;
                    }
                    Err(e) => return Err(e.to_string()),
                }
            }
            if command {
                let heading = ((started.elapsed().as_secs_f64() * 1.2
                    + i as f64 * 2.399963229728653)
                    % std::f64::consts::TAU
                    * 251.
                    / std::f64::consts::TAU) as u8;
                socket
                    .write(Message::Binary(vec![heading].into()))
                    .map_err(|e| e.to_string())?;
            }
            match socket.flush() {
                Ok(()) => {}
                Err(tungstenite::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => return Err(e.to_string()),
            }
        }
        if started.elapsed().as_secs_f64() >= 1. {
            minimum_playing = minimum_playing.min(playing.iter().filter(|p| **p).count());
        }
        if command {
            steer = Instant::now() + Duration::from_millis(100);
        }
        thread::sleep(Duration::from_millis(1));
    }
    println!(
        "{}",
        serde_json::json!({"packets": packets, "bytes": bytes, "joins": joins, "deaths": deaths,
        "playingAtEnd": playing.iter().filter(|p| **p).count(), "minimumPlaying": minimum_playing})
    );
    Ok(())
}

pub fn benchmark(
    case: &str,
    clients: usize,
    ticks: usize,
    world: Config,
) -> Result<serde_json::Value, String> {
    use std::{
        io::{BufRead, BufReader, Read},
        process::{Command, Stdio},
        thread,
    };
    let host = Host::bind(HostConfig {
        bind: "127.0.0.1:0".parse().map_err(|_| "bad address")?,
        seed: 7,
        players: PlayerConfig {
            max: clients,
            latency: LatencyConfig::fixed(0.),
        },
        bots: BotConfig {
            count: 0,
            ..Default::default()
        },
        simulation: SimulationConfig {
            world,
            ..Default::default()
        },
    })?
    .start()?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut command = if let Ok(core) = std::env::var("SLITHER_LOAD_DRIVER_CPU") {
        let mut command = Command::new("taskset");
        command.args(["-c", &core]).arg(&executable);
        command
    } else {
        Command::new(executable)
    };
    let mut child = command
        .args([
            "--socket-driver",
            &format!("ws://{}/slither", host.local_addr()),
            &clients.to_string(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    // Always stop/reap the driver, even on validation failure.
    let result = (|| {
        let mut output = BufReader::new(child.stdout.take().ok_or("no driver stdout")?);
        let mut ready = String::new();
        output.read_line(&mut ready).map_err(|e| e.to_string())?;
        if ready.trim() != "ready" {
            return Err("socket driver did not become ready".into());
        }
        thread::sleep(Duration::from_secs(1));
        if host.players() != clients {
            return Err(format!(
                "only {}/{} sockets connected",
                host.players(),
                clients
            ));
        }
        let (before, busy, deferred, cpu, start) = (
            host.ticks(),
            host.simulation_seconds(),
            host.deferred_ticks(),
            cpu_seconds(),
            Instant::now(),
        );
        while host.ticks() - before < ticks as u64 {
            if start.elapsed().as_secs_f64() > 20. {
                return Err("socket host failed to complete tick window".into());
            }
            thread::sleep(Duration::from_millis(1));
        }
        let actual = host.ticks() - before;
        let wall = host.simulation_seconds() - busy;
        let cpu = cpu_seconds().zip(cpu).map(|(end, start)| end - start);
        let elapsed = start.elapsed().as_secs_f64();
        let deferred = host.deferred_ticks() - deferred;
        let connected = host.players();
        drop(child.stdin.take());
        let mut diagnostics = String::new();
        output
            .read_to_string(&mut diagnostics)
            .map_err(|e| e.to_string())?;
        if !child.wait().map_err(|e| e.to_string())?.success() {
            return Err("socket driver failed".into());
        }
        let driver: serde_json::Value =
            serde_json::from_str(diagnostics.trim()).map_err(|e| e.to_string())?;
        if connected != clients
            || driver["packets"].as_u64().unwrap_or(0) == 0
            || driver["joins"].as_u64().unwrap_or(0) == 0
        {
            return Err("socket workload delivered no gameplay or lost connections".into());
        }
        let scale = ticks as f64 / actual as f64;
        Ok(
            serde_json::json!({"kind": "report", "format": "slither-perf-baseline-v1", "case": case,
            "mode": "timing", "iterations": ticks, "seed": 7,
            "timing": {"operations": ticks, "wallSeconds": wall * scale, "cpuSeconds": cpu.map(|v| v * scale),
                "scope": "host CPU across simulation/socket threads, excluding child load generator; simulation busy wall separately; normalized to requested ticks"},
            "details": {"clients": clients, "connected": connected, "actualTicks": actual,
                "deferredTicks": deferred, "elapsedSeconds": elapsed, "driver": driver,
                "hostCpu": std::env::var("SLITHER_LOAD_HOST_CPU").ok(), "driverCpu": std::env::var("SLITHER_LOAD_DRIVER_CPU").ok()}}),
        )
    })();
    drop(child.stdin.take());
    if child.try_wait().map_err(|e| e.to_string())?.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    host.stop()?;
    result
}
