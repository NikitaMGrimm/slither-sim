use std::io;
use std::process::ExitCode;

const HOST_USAGE: &str = "Usage: slither-sim host [--bind IP:PORT] [--config FILE]\n\n\
    Binary protocol-19 WebSocket server at /slither. Defaults bind loopback.\n\
    FILE is a HostConfig JSON object; --bind overrides its bind address.\n\
    See docs/raw-server.md for limits and VPS configuration.";

fn host(args: &[&str]) -> ExitCode {
    use slither_sim::client::{Host, HostConfig};
    let parsed = (|| -> Result<Option<HostConfig>, String> {
        let mut bind = None;
        let mut file = None;
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
        Ok(Some(config))
    })();
    let config = match parsed {
        Ok(Some(config)) => config,
        Ok(None) => {
            println!("{HOST_USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(error) => {
            eprintln!("{error}\n{HOST_USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = Host::bind(config).and_then(|mut host| {
        eprintln!("Listening on ws://{}/slither", host.local_addr()?);
        host.run()
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
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
    match slither_sim::transport::serve(io::stdin().lock(), io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
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
        ["client"] => {
            match slither_sim::client::transport::serve(io::stdin().lock(), io::stdout().lock()) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::FAILURE
                }
            }
        }
        ["host", args @ ..] => host(args),
        _ => {
            eprintln!("Usage: slither-sim serve | client | host | --help | --version");
            ExitCode::from(2)
        }
    }
}
