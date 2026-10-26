//! Study wrapper around the unmodified public Host API. Config and counters only.
use serde_json::json;
use slither_sim::client::{Host, HostConfig};
use std::{
    fs, thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn unix() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        return Err("host_probe CONFIG WARMUP HOLD OUTPUT EPOCH_UNIX_S".into());
    }
    let config: HostConfig =
        serde_json::from_slice(&fs::read(&args[1]).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let warmup: f64 = args[2].parse().map_err(|_| "warmup")?;
    let hold: f64 = args[3].parse().map_err(|_| "hold")?;
    let epoch: f64 = args[5].parse().map_err(|_| "epoch")?;
    let start = epoch + warmup;
    let end = start + hold;
    let host = Host::bind(config.clone())?.start()?;
    let bound = unix();
    let monotonic = Instant::now();
    let mut rows = Vec::new();
    let mut next = bound;
    let mut boundary = 0;
    loop {
        let now = bound + monotonic.elapsed().as_secs_f64();
        let due = if boundary == 0 {
            next.min(start)
        } else {
            next.min(end)
        };
        if now >= due {
            rows.push(json!({"unix":now,"ticks":host.ticks(),"deferred":host.deferred_ticks(),"simulationSeconds":host.simulation_seconds(),"players":host.players(),"boundary": if boundary == 0 && now >= start { "start" } else if now >= end { "end" } else { "sample" }}));
            if now >= end {
                break;
            }
            if now >= start {
                boundary = 1;
            }
            next = now + 1.;
        }
        thread::sleep(Duration::from_millis(1));
    }
    // Let the external /proc sampler capture its end boundary before teardown.
    thread::sleep(Duration::from_secs(2));
    host.stop()?;
    let result = json!({"config":config,"boundUnix":bound,"holdStartUnix":start,"holdEndUnix":end,"rows":rows});
    fs::write(&args[4], serde_json::to_vec_pretty(&result).unwrap()).map_err(|e| e.to_string())
}
