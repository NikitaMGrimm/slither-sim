//! Research generator jobs: decision loops only enqueue, never execute blocking jobs.
use std::sync::{atomic::{AtomicUsize, Ordering}, mpsc, Arc};
type Job = Box<dyn FnOnce() + Send>;
pub struct Background {
    sender: mpsc::Sender<Job>,
    queued: Arc<AtomicUsize>,
}
impl Background {
    pub fn new(name: &str) -> Self {
        let (sender, receiver) = mpsc::channel::<Job>();
        let queued = Arc::new(AtomicUsize::new(0));
        let count = queued.clone();
        std::thread::Builder::new().name(name.into()).spawn(move || {
            for job in receiver {
                job();
                count.fetch_sub(1, Ordering::Release);
            }
        }).unwrap();
        Self { sender, queued }
    }
    pub fn submit(&self, job: impl FnOnce() + Send + 'static) -> Result<(), String> {
        // Bound memory; a backed-up generator fails rather than delaying decisions.
        let previous = self.queued.fetch_add(1, Ordering::AcqRel);
        if previous >= 128 {
            self.queued.fetch_sub(1, Ordering::Release);
            return Err("generator background queue capacity exceeded".into());
        }
        #[cfg(synchronous_mutation)]
        { job(); self.queued.fetch_sub(1, Ordering::Release); return Ok(()); }
        #[cfg(not(synchronous_mutation))]
        self.sender.send(Box::new(job)).map_err(|_| "generator background worker exited".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};
    #[test]
    fn slow_connect_and_publication_do_not_block_other_seat_decisions() {
        let connect = Background::new("slow-connect");
        let output = Background::new("slow-report");
        let (sender, receiver) = mpsc::channel();
        let started = Instant::now();
        let mut due = started;
        connect.submit(move || {
            std::thread::sleep(Duration::from_millis(500));
            sender.send(()).unwrap();
        }).unwrap();
        output.submit(|| std::thread::sleep(Duration::from_millis(500))).unwrap();
        let mut decisions = 0;
        let mut worst = Duration::ZERO;
        while started.elapsed() < Duration::from_millis(650) {
            let now = Instant::now();
            if now >= due {
                worst = worst.max(now.duration_since(due));
                decisions += 1;
                due += Duration::from_millis(42);
            }
            std::thread::sleep(Duration::from_micros(200));
        }
        assert!(decisions >= 15, "other seat stopped: {decisions} decisions");
        assert!(worst < Duration::from_millis(42), "late decision: {worst:?}");
        receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    }
}
