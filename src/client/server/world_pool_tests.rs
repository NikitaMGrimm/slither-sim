use super::*;
use std::sync::Condvar;

/// Gate the workers after Rayon's run loop exits. Signalling termination alone
/// cannot complete shutdown while these owned OS threads are still alive.
fn assert_workers_joined(fail_startup: bool) {
    let exited = Arc::new(AtomicUsize::new(0));
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let (exiting, at_exit) = mpsc::channel();
    let (finished, done) = mpsc::channel();
    let observed = exited.clone();
    let gate = release.clone();
    let owner = thread::spawn(move || {
        let mut started = 0;
        let pool = HostPool::spawn_with(3, "slither-world", |worker| {
            if fail_startup && started == 2 {
                return Err(io::Error::other("injected world worker startup failure"));
            }
            started += 1;
            let exiting = exiting.clone();
            let exited = observed.clone();
            let release = gate.clone();
            Ok(thread::spawn(move || {
                worker.run();
                exiting.send(()).unwrap();
                let (lock, wake) = &*release;
                let _guard = wake
                    .wait_while(lock.lock().unwrap(), |open| !*open)
                    .unwrap();
                exited.fetch_add(1, Ordering::SeqCst);
            }))
        });
        if fail_startup {
            assert!(
                pool.err()
                    .unwrap()
                    .contains("injected world worker startup failure")
            );
        } else {
            let pool = pool.unwrap();
            assert_eq!(pool.workers.len(), 3);
            // Start every worker, then leave them idle before dropping the pool.
            assert_eq!(pool.pool.as_ref().unwrap().broadcast(|_| ()).len(), 3);
            drop(pool);
        }
        finished.send(()).unwrap();
    });
    let count = if fail_startup { 2 } else { 3 };
    for _ in 0..count {
        at_exit.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    let early = done.recv_timeout(Duration::from_millis(25));
    // Always release the workers before asserting, so a failed regression
    // cannot strand a blocked worker in the test process.
    let (lock, wake) = &*release;
    *lock.lock().unwrap() = true;
    wake.notify_all();
    assert!(matches!(early, Err(mpsc::RecvTimeoutError::Timeout)));
    done.recv_timeout(Duration::from_secs(5)).unwrap();
    owner.join().unwrap();
    assert_eq!(exited.load(Ordering::SeqCst), count);
}

#[test]
fn world_pool_drop_joins_idle_workers() {
    assert_workers_joined(false);
}

#[test]
fn world_pool_startup_error_joins_already_started_workers() {
    assert_workers_joined(true);
}

#[test]
fn host_world_loop_uses_the_requested_pool_and_parallel_flag() {
    for threads in [1, 3] {
        for explicit_stop in [false, true] {
            let config = HostConfig {
                bind: "127.0.0.1:0".parse().unwrap(),
                world_threads: threads,
                simulation: SimulationConfig {
                    world: crate::Config {
                        food_count: 0,
                        ambient_replenish_per_second: 0.0,
                        ..crate::Config::sandbox()
                    },
                    ..Default::default()
                },
                ..Default::default()
            };
            let (probe, context) = mpsc::channel();
            let mut host = Host::bind(config).unwrap();
            host.world_context = Some(probe);
            let host = host.start().unwrap();
            let WorldContext {
                threads: actual_threads,
                parallel,
                name,
            } = context.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(parallel, threads > 1);
            if threads > 1 {
                assert_eq!(actual_threads, threads);
                assert!(name.starts_with("slither-world-"), "{name}");
            } else {
                assert_eq!(name, "slither-simulation");
            }
            if explicit_stop {
                host.stop().unwrap();
            } else {
                drop(host);
            }
        }
    }
}
