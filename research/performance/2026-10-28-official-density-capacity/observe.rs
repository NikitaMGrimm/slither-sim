//! Archived-copy read-only research hooks. No game state is changed.
use std::{cell::RefCell, time::Instant};
thread_local! {
    static TICKS: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}
pub fn tick(start: Instant, tick: u64, deferred: u64, world: &crate::Simulator) {
    let elapsed = start.elapsed().as_nanos() as u64;
    TICKS.with_borrow_mut(|times| {
        times.push(elapsed);
        if tick % 100 != 0 { return; }
        let mut lengths: Vec<usize> = world.snakes().iter().filter(|s|s.alive).map(|s|s.segments).collect();
        lengths.sort_unstable();
        let quantile = |p: f64| lengths.get(((lengths.len().saturating_sub(1)) as f64*p).ceil() as usize).copied().unwrap_or(0);
        eprintln!("CENSUS {}", serde_json::json!({
            "unix":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs_f64(),
            "tick":tick,"deferred":deferred,"living":lengths.len(),"occupied":world.occupied(),
            "meanSegments":lengths.iter().sum::<usize>() as f64/lengths.len().max(1) as f64,
            "p50Segments":quantile(0.5),"p90Segments":quantile(0.9),"p99Segments":quantile(0.99),
            "maxSegments":lengths.last(),"food":world.food().len(),"prey":world.prey().len(),
            "deaths":world.deaths().len(),"radius":world.radius(),"tickNanos":times
        }));
        times.clear();
    });
}
