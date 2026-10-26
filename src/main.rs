use std::io::{self, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use slither_sim::client::{Host, HostConfig, RunningHost};

const HOST_USAGE: &str = "Usage: slither-sim host [--bind IP:PORT] [--config FILE] [--stats SECONDS]\n\n\
    Binary protocol-19 WebSocket server at /slither. Defaults bind loopback.\n\
    FILE is a HostConfig JSON object with players and bots sections; --bind overrides bind.\n\
    --stats prints tick rate, deferred ticks, simulation load and population to stderr every SECONDS.\n\
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
}

fn parse_host_args(args: &[&str]) -> Result<Option<HostOptions>, String> {
    let mut bind = None;
    let mut file = None;
    let mut stats_interval = None;
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
    Ok(Some(HostOptions {
        config,
        stats_interval,
    }))
}

struct HostStats {
    at: Instant,
    ticks: u64,
    deferred: u64,
    simulation_seconds: f64,
}

impl HostStats {
    fn sample(host: &RunningHost) -> Self {
        Self {
            at: Instant::now(),
            ticks: host.ticks(),
            deferred: host.deferred_ticks(),
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
            "stats: ticks/s={:.2} deferred_ticks={} simulation_load={:.1}% players={} bot_slots={}",
            (self.ticks - previous.ticks) as f64 / elapsed,
            self.deferred - previous.deferred,
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
            previous = current;
        }
    }
    Ok(())
}

fn host(args: &[&str]) -> ExitCode {
    let options = match parse_host_args(args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{HOST_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}\n{HOST_USAGE}");
            return ExitCode::from(2);
        }
    };
    for warning in options.config.warnings() {
        eprintln!("warning: {warning}");
    }
    let bot_slots = options.config.bots.count;
    let result = Host::bind(options.config).and_then(|host| {
        eprintln!("Listening on ws://{}/slither", host.local_addr()?);
        if let Some(interval) = options.stats_interval {
            let running = host.start()?;
            report_host_stats(&running, interval, bot_slots, &mut io::stderr())
                .map_err(|error| error.to_string())?;
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
    command_exit(slither_sim::transport::serve(
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
                slither_sim::transport::PROTOCOL,
                slither_sim::client::transport::PROTOCOL
            );
            ExitCode::SUCCESS
        }
        ["--version"] | ["-V"] => {
            println!("slither-sim {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        ["serve", args @ ..] => serve(args),
        ["client"] => command_exit(slither_sim::client::transport::serve(
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
            simulation_seconds: 5.,
        };
        let current = HostStats {
            at: previous.at + Duration::from_secs(2),
            ticks: 300,
            deferred: 10,
            simulation_seconds: 5.824,
        };
        let mut output = Vec::new();
        current.write_since(&previous, 40, 4, &mut output).unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "stats: ticks/s=100.00 deferred_ticks=3 simulation_load=41.2% players=40 bot_slots=4\n"
        );
    }

    #[test]
    fn host_stats_report_a_running_loopback_host_and_propagate_output_errors() {
        let config = HostConfig {
            bind: "127.0.0.1:0".parse().unwrap(),
            bots: slither_sim::client::BotConfig {
                count: 0,
                ..Default::default()
            },
            simulation: slither_sim::client::SimulationConfig {
                world: slither_sim::Config::sandbox(),
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
