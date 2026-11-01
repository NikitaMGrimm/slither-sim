#[cfg(feature = "latency-rig")]
use crate as sim;
#[cfg(not(feature = "latency-rig"))]
use slither_sim as sim;

use std::io::{self, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

#[cfg(feature = "latency-rig")]
include!("latency_host_root.rs");

#[cfg(feature = "latency-rig")]
use crate::client::{Host, HostConfig, RunningHost, latency_truth};
#[cfg(not(feature = "latency-rig"))]
use sim::client::{Host, HostConfig, RunningHost};

const HOST_USAGE: &str = "Usage: slither-sim host [--bind IP:PORT] [--config FILE] [--stats SECONDS]\n\n\
    Binary protocol-19 WebSocket server at /slither. Defaults bind loopback.\n\
    FILE is a HostConfig JSON object with players and bots sections; --bind overrides bind.\n\
    --stats prints tick rate, deferred ticks, load, population and admission rejections every SECONDS.\n\
    See docs/raw-server.md for limits and VPS configuration.";

fn command_exit(result: Result<(), impl std::fmt::Display>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

struct HostOptions {
    config: HostConfig,
    stats_interval: Option<Duration>,
    #[cfg(feature = "latency-rig")]
    latency: Option<LatencyOptions>,
}

#[cfg(feature = "latency-rig")]
struct LatencyOptions {
    path: std::path::PathBuf,
    seconds: Duration,
}

fn host_usage() -> String {
    #[cfg(feature = "latency-rig")]
    return format!(
        "{HOST_USAGE}\n    Measurement only: --latency-truth NEWFILE [--latency-seconds SECONDS (default 60, max 180)]."
    );
    #[cfg(not(feature = "latency-rig"))]
    HOST_USAGE.to_owned()
}

fn parse_host_args(args: &[&str]) -> Result<Option<HostOptions>, String> {
    let mut bind = None;
    let mut file = None;
    let mut stats_interval = None;
    #[cfg(feature = "latency-rig")]
    let mut latency_path = None;
    #[cfg(feature = "latency-rig")]
    let mut latency_seconds = None;
    let mut args = args.iter().copied();
    while let Some(arg) = args.next() {
        match arg {
            "--help" | "-h" => return Ok(None),
            "--bind" => {
                if bind.is_some() {
                    return Err("Repeated --bind".into());
                }
                bind = Some(args.next().ok_or("--bind needs an IP:PORT")?);
            }
            "--config" => {
                if file.is_some() {
                    return Err("Repeated --config".into());
                }
                file = Some(args.next().ok_or("--config needs a file")?);
            }
            "--stats" => {
                if stats_interval.is_some() {
                    return Err("Repeated --stats".into());
                }
                let error = "--stats needs positive seconds within the supported duration range";
                let seconds = args
                    .next()
                    .ok_or(error)?
                    .parse::<f64>()
                    .map_err(|_| error)?;
                let interval = Duration::try_from_secs_f64(seconds).map_err(|_| error)?;
                if interval.is_zero() {
                    return Err(error.into());
                }
                stats_interval = Some(interval);
            }
            #[cfg(feature = "latency-rig")]
            "--latency-truth" => {
                if latency_path.is_some() {
                    return Err("Repeated --latency-truth".into());
                }
                latency_path = Some(args.next().ok_or("--latency-truth needs a new file")?);
            }
            #[cfg(feature = "latency-rig")]
            "--latency-seconds" => {
                if latency_seconds.is_some() {
                    return Err("Repeated --latency-seconds".into());
                }
                let seconds = args
                    .next()
                    .ok_or("--latency-seconds needs seconds")?
                    .parse::<f64>()
                    .map_err(|_| "--latency-seconds needs seconds")?;
                latency_seconds = Some(latency_truth::duration(seconds)?);
            }
            _ => return Err(format!("Unknown host argument: {arg}")),
        }
    }
    let mut config: HostConfig = match file {
        Some(path) => {
            let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
            serde_json::from_slice(&bytes).map_err(|e| format!("{path}: {e}"))?
        }
        None => HostConfig::default(),
    };
    if let Some(bind) = bind {
        config.bind = bind.parse().map_err(|_| "--bind needs a literal IP:PORT")?;
    }
    config.validate()?;
    #[cfg(feature = "latency-rig")]
    let latency = match latency_path {
        Some(path) => {
            let seconds =
                latency_seconds.unwrap_or(latency_truth::duration(latency_truth::DEFAULT_SECONDS)?);
            latency_truth::validate(
                std::path::Path::new(path),
                config.bind,
                seconds,
                config.simulation.world.dt,
                config.players.max + config.bots.count,
            )?;
            Some(LatencyOptions {
                path: path.into(),
                seconds,
            })
        }
        None if latency_seconds.is_some() => {
            return Err("--latency-seconds requires --latency-truth".into());
        }
        None => None,
    };
    Ok(Some(HostOptions {
        config,
        stats_interval,
        #[cfg(feature = "latency-rig")]
        latency,
    }))
}

struct HostStats {
    at: Instant,
    ticks: u64,
    deferred: u64,
    lost_seconds: f64,
    simulation_seconds: f64,
}

impl HostStats {
    fn sample(host: &RunningHost) -> Self {
        Self {
            at: Instant::now(),
            ticks: host.ticks(),
            deferred: host.deferred_ticks(),
            lost_seconds: host.lost_seconds(),
            simulation_seconds: host.simulation_seconds(),
        }
    }

    fn write_since(
        &self,
        previous: &Self,
        players: usize,
        bot_slots: usize,
        output: &mut impl Write,
    ) -> io::Result<()> {
        let elapsed = self.at.duration_since(previous.at).as_secs_f64();
        writeln!(
            output,
            "stats: ticks/s={:.2} deferred_ticks={} lost_seconds={:.6} simulation_load={:.1}% players={} bot_slots={}",
            (self.ticks - previous.ticks) as f64 / elapsed,
            self.deferred - previous.deferred,
            self.lost_seconds - previous.lost_seconds,
            100. * (self.simulation_seconds - previous.simulation_seconds) / elapsed,
            players,
            bot_slots,
        )
    }
}

fn report_host_stats(
    host: &RunningHost,
    interval: Duration,
    bot_slots: usize,
    output: &mut impl Write,
) -> io::Result<()> {
    let mut previous = HostStats::sample(host);
    while !host.is_finished() {
        // Bound failure detection latency even when reports are hours apart.
        std::thread::sleep(
            interval
                .saturating_sub(previous.at.elapsed())
                .min(Duration::from_millis(100)),
        );
        if host.is_finished() {
            break;
        }
        if previous.at.elapsed() >= interval {
            let current = HostStats::sample(host);
            current.write_since(&previous, host.players(), bot_slots, output)?;
            for ((phase, reason), count) in host.admission_rejections() {
                writeln!(
                    output,
                    "admission_rejections: phase={phase} reason={reason:?} count={count}"
                )?;
            }
            previous = current;
        }
    }
    Ok(())
}

fn host(args: &[&str]) -> ExitCode {
    let options = match parse_host_args(args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{}", host_usage());
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}\n{}", host_usage());
            return ExitCode::from(2);
        }
    };
    for warning in options.config.warnings() {
        eprintln!("warning: {warning}");
    }
    let bot_slots = options.config.bots.count;
    let result = Host::bind(options.config).and_then(|host| {
        #[cfg(feature = "latency-rig")]
        let host = if let Some(latency) = options.latency {
            host.with_latency_truth(&latency.path, latency.seconds)?
        } else {
            host
        };
        eprintln!("Listening on ws://{}/slither", host.local_addr()?);
        if let Some(interval) = options.stats_interval {
            let running = host.start()?;
            // Stats are diagnostics: if stderr breaks, stop reporting but keep serving.
            let _ = report_host_stats(&running, interval, bot_slots, &mut io::stderr());
            running.wait()
        } else {
            host.run()
        }
    });
    command_exit(result)
}

fn serve(args: &[&str]) -> ExitCode {
    match args {
        [] => {}
        #[cfg(feature = "parallel")]
        ["--threads", threads] => {
            let pool = match threads.parse::<usize>() {
                Ok(threads) if threads > 0 => rayon::ThreadPoolBuilder::new()
                    .num_threads(threads)
                    .build_global()
                    .map_err(|error| error.to_string()),
                _ => Err("--threads needs a positive number".into()),
            };
            if let Err(error) = pool {
                eprintln!("{error}");
                return ExitCode::from(2);
            }
        }
        _ => {
            eprintln!("Usage: slither-sim serve [--threads N]");
            return ExitCode::from(2);
        }
    }
    command_exit(sim::transport::serve(
        io::stdin().lock(),
        io::stdout().lock(),
    ))
}

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["--help"] | ["-h"] | ["help"] => {
            println!(
                "slither-sim {}\n\nUsage: slither-sim serve | client | host\n\n\
                 serve      Geometric simulation control over JSON-lines; --threads N caps worlds' cores.\n\
                 client     Portable received-state bot control over JSON-lines.\n\
                 host       Shared protocol-19 WebSocket simulator at /slither.\n\
                 --version  Print the package version.\n\n\
                 Protocols: {}, {}\nSee docs/bot-api.md and docs/client-api.md.",
                env!("CARGO_PKG_VERSION"),
                sim::transport::PROTOCOL,
                sim::client::transport::PROTOCOL
            );
            ExitCode::SUCCESS
        }
        ["--version"] | ["-V"] => {
            println!("slither-sim {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        ["serve", args @ ..] => serve(args),
        ["client"] => command_exit(sim::client::transport::serve(
            io::stdin().lock(),
            io::stdout().lock(),
        )),
        ["host", args @ ..] => host(args),
        _ => {
            eprintln!("Usage: slither-sim serve | client | host | --help | --version");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "latency-rig")]
    fn truth_path() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "slither-truth-args-{}-{}.jsonl",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[cfg(feature = "latency-rig")]
    #[test]
    fn latency_args_are_opt_in_bounded_and_loopback_only() {
        assert!(parse_host_args(&[]).unwrap().unwrap().latency.is_none());
        let path = truth_path();
        let file = path.to_str().unwrap();
        let options = parse_host_args(&["--latency-truth", file])
            .unwrap()
            .unwrap();
        assert_eq!(options.latency.unwrap().seconds, Duration::from_secs(60));
        let options = parse_host_args(&[
            "--latency-truth",
            file,
            "--latency-seconds",
            "180",
            "--bind",
            "[::1]:0",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(options.latency.unwrap().seconds, Duration::from_secs(180));
        assert!(
            !path.exists(),
            "argument validation must not create an artifact"
        );
        for bind in ["0.0.0.0:0", "[::]:0", "192.0.2.1:0"] {
            assert!(parse_host_args(&["--latency-truth", file, "--bind", bind]).is_err());
        }
        for value in ["0", "-1", "NaN", "inf", "180.1", "1e-100", "bad"] {
            assert!(
                parse_host_args(&["--latency-truth", file, "--latency-seconds", value]).is_err()
            );
        }
        for args in [
            vec!["--latency-truth"],
            vec!["--latency-seconds"],
            vec!["--latency-seconds", "1"],
            vec!["--latency-truth", file, "--latency-truth", file],
            vec![
                "--latency-truth",
                file,
                "--latency-seconds",
                "1",
                "--latency-seconds",
                "2",
            ],
        ] {
            assert!(parse_host_args(&args).is_err());
        }
        std::fs::write(&path, b"preserve me").unwrap();
        assert!(parse_host_args(&["--latency-truth", file]).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"preserve me");
        std::fs::remove_file(path).unwrap();
    }

    #[cfg(not(feature = "latency-rig"))]
    #[test]
    fn default_binary_has_no_latency_arguments() {
        assert!(parse_host_args(&["--latency-truth", "unused.jsonl"]).is_err());
        assert!(parse_host_args(&["--latency-seconds", "1"]).is_err());
        assert!(!host_usage().contains("latency"));
    }

    #[cfg(feature = "latency-rig")]
    #[test]
    fn finite_latency_host_stops_joins_and_exports_without_connections() {
        for max_tick in [None, Some(0.014)] {
            let path = truth_path();
            let config = HostConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                bots: sim::client::BotConfig {
                    count: 0,
                    ..Default::default()
                },
                simulation: sim::client::SimulationConfig {
                    world: sim::Config::sandbox(),
                    ..Default::default()
                },
                max_tick,
                ..Default::default()
            };
            let running = Host::bind(config)
                .unwrap()
                .with_latency_truth(&path, Duration::from_millis(40))
                .unwrap()
                .start()
                .unwrap();
            running.wait().unwrap();
            let text = std::fs::read_to_string(&path).unwrap();
            let rows: Vec<serde_json::Value> = text
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect();
            assert_eq!(rows[0]["schema"], "host-latency-truth-v1");
            assert_eq!(rows.last().unwrap()["complete"], true);
            assert!(rows.last().unwrap()["ticks"].as_u64().unwrap() > 0);
            assert_eq!(rows.last().unwrap()["ticks"], rows.len() - 2);
            for row in &rows[1..rows.len() - 1] {
                assert_eq!(row["heads"].as_array().unwrap().len(), 4);
                assert_eq!(row["heads"][0][0], 0);
                assert_eq!(row["heads"][0][1], 0);
                assert_eq!(row["heads"][0][2], false);
            }
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn host_stats_are_opt_in_and_accept_fractional_seconds() {
        let defaults = parse_host_args(&[]).unwrap().unwrap();
        assert!(defaults.stats_interval.is_none());
        let options = parse_host_args(&["--stats", "0.25", "--bind", "127.0.0.1:0"])
            .unwrap()
            .unwrap();
        assert_eq!(options.stats_interval, Some(Duration::from_millis(250)));
        assert_eq!(options.config.bind, "127.0.0.1:0".parse().unwrap());
        assert!(parse_host_args(&["--help"]).unwrap().is_none());
    }

    #[test]
    fn host_stats_reject_missing_invalid_and_repeated_intervals() {
        for value in ["0", "-1", "NaN", "inf", "-inf", "1e100", "1e-100", "oops"] {
            let error = parse_host_args(&["--stats", value]).err().unwrap();
            assert!(
                error.starts_with("--stats needs positive seconds"),
                "{value}: {error}"
            );
        }
        assert!(parse_host_args(&["--stats"]).is_err());
        assert_eq!(
            parse_host_args(&["--stats", "1", "--stats", "2"])
                .err()
                .unwrap(),
            "Repeated --stats"
        );
    }

    #[test]
    fn host_stats_use_interval_deltas_and_elapsed_wall_time() {
        let previous = HostStats {
            at: Instant::now(),
            ticks: 100,
            deferred: 7,
            lost_seconds: 0.1,
            simulation_seconds: 5.,
        };
        let current = HostStats {
            at: previous.at + Duration::from_secs(2),
            ticks: 300,
            deferred: 10,
            lost_seconds: 0.125,
            simulation_seconds: 5.824,
        };
        let mut output = Vec::new();
        current.write_since(&previous, 40, 4, &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "stats: ticks/s=100.00 deferred_ticks=3 lost_seconds=0.025000 simulation_load=41.2% players=40 bot_slots=4\n"
        );
    }

    #[test]
    fn host_stats_report_a_running_loopback_host_and_propagate_output_errors() {
        let config = HostConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            bots: sim::client::BotConfig {
                count: 0,
                ..Default::default()
            },
            simulation: sim::client::SimulationConfig {
                world: sim::Config::sandbox(),
                ..Default::default()
            },
            ..Default::default()
        };
        let host = Host::bind(config).unwrap().start().unwrap();
        // A bounded writer ends reporting after a few lines without a test-only
        // shutdown path in the serving loop.
        let mut output = [0; 512];
        let error = report_host_stats(
            &host,
            Duration::from_millis(20),
            0,
            &mut output.as_mut_slice(),
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::WriteZero);
        assert!(host.ticks() > 0);
        host.stop().unwrap();
        let text = std::str::from_utf8(&output).unwrap();
        let lines: Vec<_> = text.lines().collect();
        assert!(lines.len() >= 2, "{text}");
        for line in &lines[..2] {
            assert!(line.starts_with("stats: ticks/s="), "{line}");
            assert!(line.ends_with("players=0 bot_slots=0"), "{line}");
        }
    }
}
