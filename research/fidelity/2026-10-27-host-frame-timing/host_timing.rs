use slither_sim::client::server::{Host, HostConfig};
use std::{env, fs, thread, time::Duration};

fn main() {
    let args: Vec<_> = env::args().collect();
    let config: HostConfig = serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let host = Host::bind(config).unwrap().start().unwrap();
    println!("READY {}", host.local_addr());
    thread::sleep(Duration::from_secs(args[2].parse().unwrap()));
    println!(
        "ticks={} deferred={} busy={}",
        host.ticks(),
        host.deferred_ticks(),
        host.simulation_seconds()
    );
    host.stop().unwrap();
}
