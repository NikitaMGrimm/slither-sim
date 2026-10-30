//! Linux/POSIX research accounting: include idle and in-flight helper thread CPU.
use std::{os::unix::thread::JoinHandleExt, thread::JoinHandle};

unsafe extern "C" {
    fn pthread_self() -> usize;
    fn pthread_getcpuclockid(thread: usize, clock: *mut i32) -> i32;
    fn clock_gettime(clock: i32, time: *mut Time) -> i32;
}
#[repr(C)]
struct Time { seconds: i64, nanos: i64 }
pub struct Clock(i32);
impl Clock {
    fn for_thread(thread: usize) -> Self {
        let mut clock = 0;
        assert_eq!(unsafe { pthread_getcpuclockid(thread, &mut clock) }, 0);
        Self(clock)
    }
    pub fn current() -> Self { Self::for_thread(unsafe { pthread_self() }) }
    pub fn for_handle(handle: &JoinHandle<()>) -> Self {
        Self::for_thread(handle.as_pthread_t() as usize)
    }
    fn nanos(&self) -> u64 {
        let mut time = Time { seconds: 0, nanos: 0 };
        // An exited helper is a harness failure, not a zero-CPU sample.
        assert_eq!(unsafe { clock_gettime(self.0, &mut time) }, 0);
        time.seconds as u64 * 1_000_000_000 + time.nanos as u64
    }
}
#[derive(Clone, Copy)]
pub struct Snapshot { pub decision: u64, pub connector: u64, pub publisher: u64 }
impl Snapshot {
    pub fn total(&self) -> u64 {
        #[cfg(omit_background_cpu_mutation)]
        { self.decision }
        #[cfg(not(omit_background_cpu_mutation))]
        { self.decision + self.connector + self.publisher }
    }
}
pub struct WorkerCpu { decision: Clock, connector: Clock, publisher: Clock }
impl WorkerCpu {
    pub fn new(decision: Clock, connector: Clock, publisher: Clock) -> Self {
        Self { decision, connector, publisher }
    }
    pub fn snapshot(&self) -> Snapshot {
        Snapshot { decision: self.decision.nanos(), connector: self.connector.nanos(), publisher: self.publisher.nanos() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::{atomic::{AtomicBool, Ordering}, mpsc, Arc}, time::Duration};
    #[test]
    fn sum_includes_connector_and_publisher() {
        let value = Snapshot { decision: 100, connector: 350, publisher: 400 };
        assert_eq!(value.total(), 850); // 10% decision alone conceals 85% total.
    }
    #[test]
    fn clocks_include_helpers_before_their_jobs_finish() {
        let stop = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = mpsc::channel();
        let mut handles = Vec::new();
        for _ in 0..2 {
            let stop = stop.clone(); let ready = ready_tx.clone();
            handles.push(std::thread::spawn(move || {
                ready.send(()).unwrap();
                let mut n = 1u64;
                while !stop.load(Ordering::Relaxed) {
                    n = std::hint::black_box(n.wrapping_mul(6364136223846793005).wrapping_add(1));
                }
                std::hint::black_box(n);
            }));
        }
        ready_rx.recv().unwrap(); ready_rx.recv().unwrap();
        let cpu = WorkerCpu::new(Clock::current(), Clock::for_handle(&handles[0]), Clock::for_handle(&handles[1]));
        let before = cpu.snapshot();
        std::thread::sleep(Duration::from_millis(250));
        let after = cpu.snapshot();
        stop.store(true, Ordering::Relaxed);
        for handle in handles { handle.join().unwrap(); }
        assert!(after.connector > before.connector);
        assert!(after.publisher > before.publisher);
        assert_eq!(after.total() - before.total(), (after.decision-before.decision)+(after.connector-before.connector)+(after.publisher-before.publisher));
        assert!(after.total()-before.total() > after.decision-before.decision);
    }
}
