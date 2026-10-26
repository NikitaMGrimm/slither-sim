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
        "policy={:?} parameter_manifest_sha256={}",
        slither_sim::bot::PolicyId::default(),
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
