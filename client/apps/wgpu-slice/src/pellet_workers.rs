//! A bounded, native-only pool for exact pellet projection.
use super::{Input, Projected, project};
use crate::{
    Result,
    frame::Frame,
    pellet_batch::{Batch, Sources},
};
use slither_client::presentation::BodyPaintView;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread::{self, JoinHandle},
};

// Resource cap including main: leave CPU capacity for other applications.
// available_parallelism limits this further; Wasm does not instantiate a pool.
const MAX_PARTICIPANTS: usize = 4;
// Paced density controls show only tens of microseconds saved at 1,024 visible
// pellets, versus a repeatable reduction with two/four participants at 2,048.
// Below that, neither copy inputs nor start workers.
// Measured cutoff, not a gameplay or visual limit:
// https://github.com/NikitaMGrimm/slither-sim/blob/main/research/client-performance/2026-11-03-pellet-workers/REPORT.md.
const MIN_VISIBLE: usize = 2048;
// Claim bounded independent work, then merge in the original numeric order.
const CHUNK: usize = 128;
// A missed wakeup must not incur a fresh input copy on every busy callback.
const MISS_BACKOFF: usize = 32;

struct Job {
    inputs: Vec<Input>,
    frame: Frame,
    sources: Arc<Sources>,
    view: BodyPaintView,
    user_zoom: f64,
    density: f64,
    attachment: [u32; 2],
    next: AtomicUsize,
    cancelled: AtomicBool,
    results: Vec<OnceLock<Vec<Result<Projected>>>>,
    worker_chunks: AtomicUsize,
    #[cfg(feature = "frame-profile")]
    dispatched: std::time::Instant,
    #[cfg(feature = "frame-profile")]
    wakes: [std::sync::atomic::AtomicU64; 3],
}

impl Job {
    fn range(&self, chunk: usize) -> &[Input] {
        let begin = chunk * CHUNK;
        &self.inputs[begin..(begin + CHUNK).min(self.inputs.len())]
    }

    fn compute(&self, chunk: usize) -> Option<Vec<Result<Projected>>> {
        let inputs = self.range(chunk);
        let mut values = Vec::new();
        values.try_reserve_exact(inputs.len()).ok()?;
        values.extend(inputs.iter().map(|input| self.one(*input)));
        Some(values)
    }

    fn one(&self, input: Input) -> Result<Projected> {
        project(
            &self.frame,
            input,
            &self.sources,
            self.view,
            self.user_zoom,
            self.density,
            self.attachment,
        )
    }

    fn drain(&self, stop: &AtomicBool, behavior: &Behavior, worker: bool) {
        while !stop.load(Ordering::Acquire) && !self.cancelled.load(Ordering::Acquire) {
            let chunk = self.next.fetch_add(1, Ordering::Relaxed);
            if chunk >= self.results.len() {
                break;
            }
            if worker {
                behavior.after_claim(stop);
            }
            if stop.load(Ordering::Acquire) || self.cancelled.load(Ordering::Acquire) {
                break;
            }
            if let Some(values) = self.compute(chunk) {
                // Each cell has one claimant. Main never initializes a worker's
                // missing cell, so set() cannot wait on another initializer.
                let _ = self.results[chunk].set(values);
                if worker {
                    self.worker_chunks.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    }

    fn finish(
        &self,
        mirror: &mut [[u8; 96]],
        flags: &mut [u32],
        coverage: &mut [f64; 4],
    ) -> Result<usize> {
        let mut late = 0;
        for (chunk, cell) in self.results.iter().enumerate() {
            if let Some(values) = cell.get() {
                for value in values {
                    value
                        .as_ref()
                        .map_err(Clone::clone)?
                        .apply(mirror, flags, coverage);
                }
            } else {
                late += 1;
                // No rendezvous, lock, or allocation on this fallback. Keep
                // reverse-world/inner/outer order, including the first error.
                for input in self.range(chunk) {
                    self.one(*input)?.apply(mirror, flags, coverage);
                }
            }
        }
        Ok(late)
    }
}

struct Cancel<'a>(&'a Job);
impl Drop for Cancel<'_> {
    fn drop(&mut self) {
        self.0.cancelled.store(true, Ordering::Release);
    }
}

#[derive(Clone, Default)]
pub(super) struct Behavior {
    #[cfg(any(test, feature = "frame-profile"))]
    pub delay: std::time::Duration,
    #[cfg(any(test, feature = "frame-profile"))]
    pub panic: bool,
    #[cfg(test)]
    gate: Option<Arc<Gate>>,
    #[cfg(test)]
    claimed: Option<std::sync::mpsc::Sender<()>>,
}

impl Behavior {
    fn after_claim(&self, _stop: &AtomicBool) {
        #[cfg(any(test, feature = "frame-profile"))]
        {
            #[cfg(test)]
            if let Some(claimed) = &self.claimed {
                let _ = claimed.send(());
            }
            if !self.delay.is_zero() {
                thread::sleep(self.delay);
            }
            #[cfg(test)]
            if let Some(gate) = &self.gate {
                gate.wait(_stop);
            }
            assert!(!self.panic, "authored pellet worker panic");
        }
    }
    fn cancel(&self) {
        #[cfg(test)]
        if let Some(gate) = &self.gate {
            gate.wake();
        }
    }
}

struct Worker {
    send: Option<SyncSender<Arc<Job>>>,
    thread: Option<JoinHandle<()>>,
    ready: Arc<AtomicBool>,
}

struct NotReady(Arc<AtomicBool>);
impl Drop for NotReady {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct Pool {
    workers: Vec<Worker>,
    stop: Arc<AtomicBool>,
    behavior: Behavior,
}

impl Pool {
    fn new(participants: usize, behavior: Behavior) -> Option<Self> {
        let mut pool = Self {
            workers: Vec::new(),
            stop: Arc::default(),
            behavior,
        };
        pool.workers
            .try_reserve_exact(participants.saturating_sub(1))
            .ok()?;
        for i in 1..participants {
            let (send, receive) = sync_channel::<Arc<Job>>(1);
            let stop = pool.stop.clone();
            let behavior = pool.behavior.clone();
            let ready = Arc::new(AtomicBool::new(false));
            let flag = ready.clone();
            let thread = thread::Builder::new()
                .name(format!("pellet-{i}"))
                .spawn(move || {
                    let _not_ready = NotReady(flag.clone());
                    // Unwinding is confined to this worker. A claimed but unpublished
                    // chunk is recomputed by main; a failed worker is not restarted.
                    let _ = catch_unwind(AssertUnwindSafe(|| {
                        while !stop.load(Ordering::Acquire) {
                            flag.store(true, Ordering::Release);
                            let Ok(job) = receive.recv() else {
                                break;
                            };
                            flag.store(false, Ordering::Release);
                            #[cfg(feature = "frame-profile")]
                            job.wakes[i - 1].store(
                                job.dispatched.elapsed().as_nanos() as u64,
                                Ordering::Release,
                            );
                            job.drain(&stop, &behavior, true);
                        }
                    }));
                })
                .ok()?;
            pool.workers.push(Worker {
                send: Some(send),
                thread: Some(thread),
                ready,
            });
        }
        Some(pool)
    }

    fn ready(&self) -> bool {
        self.workers.iter().any(|w| w.ready.load(Ordering::Acquire))
    }

    fn dispatch(&self, job: &Arc<Job>) {
        // Claim the sleeping receiver once. Never queue another picture behind
        // an in-flight job, even if a ready observation raced its wakeup.
        for worker in &self.workers {
            if worker
                .ready
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                let _ = worker
                    .send
                    .as_ref()
                    .expect("live pool")
                    .try_send(job.clone());
            }
        }
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.behavior.cancel();
        // Disconnect every receiver before joining any worker. In-flight tasks
        // own numeric inputs only and stop between bounded 128-food chunks.
        for worker in &mut self.workers {
            worker.send.take();
        }
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

pub(in crate::gpu) struct Workers {
    participants: usize,
    attempted: bool,
    pool: Option<Pool>,
    backoff: usize,
    behavior: Behavior,
}

// Two participants lose to serial in the 4,096/8,192-pellet paced controls.
// Four improve both; leave smaller available CPU budgets serial by default.
// The density table in the linked study records this adoption cutoff.
fn default_participants(available: usize) -> usize {
    if available < MAX_PARTICIPANTS {
        1
    } else {
        MAX_PARTICIPANTS
    }
}

impl Default for Workers {
    fn default() -> Self {
        Self::with_participants(default_participants(
            thread::available_parallelism().map_or(1, usize::from),
        ))
    }
}

impl Workers {
    pub(super) fn with_participants(participants: usize) -> Self {
        Self {
            participants: participants.clamp(1, MAX_PARTICIPANTS),
            attempted: false,
            pool: None,
            backoff: 0,
            behavior: Behavior::default(),
        }
    }

    #[cfg(feature = "frame-profile")]
    pub(in crate::gpu) fn configure(&mut self, participants: usize, delay: std::time::Duration) {
        self.pool.take();
        *self = Self::with_participants(participants);
        self.behavior = Behavior {
            delay,
            ..Default::default()
        };
    }

    pub(in crate::gpu) fn prepare(
        &mut self,
        frame: &Frame,
        b: &Batch,
        attachment: [u32; 2],
        mirror: &mut [[u8; 96]],
        flags: &mut [u32],
        coverage: &mut [f64; 4],
    ) -> Result<bool> {
        self.prepare_with_cutoff(frame, b, attachment, mirror, flags, coverage, MIN_VISIBLE)
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_with_cutoff(
        &mut self,
        frame: &Frame,
        b: &Batch,
        attachment: [u32; 2],
        mirror: &mut [[u8; 96]],
        flags: &mut [u32],
        coverage: &mut [f64; 4],
        cutoff: usize,
    ) -> Result<bool> {
        if self.participants < 2 || b.visible_count < cutoff {
            return Ok(false);
        }
        if self.backoff > 0 {
            self.backoff -= 1;
            return Ok(false);
        }
        if !self.attempted {
            self.attempted = true;
            self.pool = Pool::new(self.participants, self.behavior.clone());
        }
        let Some(pool) = &self.pool else {
            return Ok(false);
        };
        if !pool.ready() {
            return Ok(false);
        }
        let mut inputs = Vec::new();
        if inputs.try_reserve_exact(b.visible_count).is_err() {
            return Ok(false);
        }
        for (i, food) in b.world.foods().iter().enumerate().rev() {
            let slot = b.sources.slots[i];
            if b.visible[slot] {
                inputs.push(Input {
                    slot,
                    animation: food.paint_state(),
                });
            }
        }
        let mut results = Vec::new();
        let chunks = inputs.len().div_ceil(CHUNK);
        if results.try_reserve_exact(chunks).is_err() {
            return Ok(false);
        }
        results.resize_with(chunks, OnceLock::new);
        let job = Arc::new(Job {
            inputs,
            frame: Frame {
                identity: frame.identity,
                extent: frame.extent,
                viewport_scale: frame.viewport_scale,
                tick: frame.tick,
                draws: Vec::new(),
                food_batch: None,
                snakes: Vec::new(),
            },
            sources: b.sources.clone(),
            view: b.view,
            user_zoom: b.user_zoom,
            density: b.physical_pixels_per_unit,
            attachment,
            next: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
            results,
            worker_chunks: AtomicUsize::new(0),
            #[cfg(feature = "frame-profile")]
            dispatched: std::time::Instant::now(),
            #[cfg(feature = "frame-profile")]
            wakes: std::array::from_fn(|_| std::sync::atomic::AtomicU64::new(0)),
        });
        let _cancel = Cancel(&job);
        pool.dispatch(&job);
        job.drain(&pool.stop, &Behavior::default(), false);
        let late = job.finish(mirror, flags, coverage)?;
        if job.worker_chunks.load(Ordering::Relaxed) == 0 {
            self.backoff = MISS_BACKOFF;
        }
        #[cfg(feature = "frame-profile")]
        {
            crate::render::observe_preparation("pellet_late_chunks", late as u64);
            crate::render::observe_preparation(
                "pellet_worker_chunks",
                job.worker_chunks.load(Ordering::Relaxed) as u64,
            );
            for wake in &job.wakes[..self.participants - 1] {
                // Zero means unobserved at this nonblocking read, not instant wakeup.
                crate::render::observe_preparation(
                    "pellet_worker_wakeup_ns",
                    wake.load(Ordering::Acquire),
                );
            }
        }
        #[cfg(not(feature = "frame-profile"))]
        let _ = late;
        Ok(true)
    }
}

#[cfg(test)]
struct Gate {
    entered: std::sync::mpsc::Sender<()>,
    released: std::sync::Mutex<bool>,
    wakeup: std::sync::Condvar,
}
#[cfg(test)]
impl Gate {
    fn wait(&self, stop: &AtomicBool) {
        let _ = self.entered.send(());
        let mut released = self.released.lock().unwrap();
        while !*released && !stop.load(Ordering::Acquire) {
            released = self.wakeup.wait(released).unwrap();
        }
    }
    fn wake(&self) {
        *self.released.lock().unwrap() = true;
        self.wakeup.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bank::Bank,
        pellet_batch::tests::{batch, rectangle, view},
    };
    use std::time::{Duration, Instant};

    fn scene() -> (Frame, Batch) {
        let mut bank = Bank::new().unwrap();
        let world = rectangle(&bank);
        let b = batch(
            &mut crate::pellet_batch::Cache::default(),
            &mut bank,
            &world,
            view(),
        );
        (
            Frame {
                identity: 1,
                extent: [1500, 844],
                viewport_scale: 1.,
                tick: 0,
                draws: Vec::new(),
                food_batch: None,
                snakes: Vec::new(),
            },
            b,
        )
    }
    fn job(frame: &Frame, b: &Batch) -> Arc<Job> {
        let inputs = b
            .world
            .foods()
            .iter()
            .enumerate()
            .rev()
            .filter_map(|(i, f)| {
                let slot = b.sources.slots[i];
                b.visible[slot].then_some(Input {
                    slot,
                    animation: f.paint_state(),
                })
            })
            .collect::<Vec<_>>();
        let results = (0..inputs.len().div_ceil(CHUNK))
            .map(|_| OnceLock::new())
            .collect();
        Arc::new(Job {
            inputs,
            frame: frame.clone(),
            sources: b.sources.clone(),
            view: b.view,
            user_zoom: b.user_zoom,
            density: b.physical_pixels_per_unit,
            attachment: frame.extent,
            next: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
            results,
            worker_chunks: AtomicUsize::new(0),
            #[cfg(feature = "frame-profile")]
            dispatched: std::time::Instant::now(),
            #[cfg(feature = "frame-profile")]
            wakes: std::array::from_fn(|_| std::sync::atomic::AtomicU64::new(0)),
        })
    }
    fn ready(pool: &Pool) {
        let until = Instant::now() + Duration::from_secs(2);
        while !pool.ready() {
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn expected(j: &Job) -> (Vec<[u8; 96]>, Vec<u32>, [f64; 4]) {
        let mut mirror = vec![[0; 96]; j.inputs.len()];
        let mut flags = vec![0; j.inputs.len()];
        let mut cover = [0.; 4];
        for input in &j.inputs {
            j.one(*input)
                .unwrap()
                .apply(&mut mirror, &mut flags, &mut cover);
        }
        (mirror, flags, cover)
    }
    fn check(j: &Job, expected: &(Vec<[u8; 96]>, Vec<u32>, [f64; 4])) {
        let mut mirror = vec![[0; 96]; expected.0.len()];
        let mut flags = vec![0; expected.1.len()];
        let mut cover = [0.; 4];
        j.finish(&mut mirror, &mut flags, &mut cover).unwrap();
        assert_eq!(mirror, expected.0);
        assert_eq!(flags, expected.1);
        assert_eq!(cover.map(f64::to_bits), expected.2.map(f64::to_bits));
    }
    #[test]
    fn all_participant_counts_preserve_ordered_bytes_flags_and_coverage() {
        let (frame, b) = scene();
        let expected = expected(&job(&frame, &b));
        for n in [2, 3, 4] {
            let pool = Pool::new(n, Behavior::default()).unwrap();
            ready(&pool);
            for _ in 0..5 {
                let j = job(&frame, &b);
                let _cancel = Cancel(&j);
                pool.dispatch(&j);
                j.drain(&pool.stop, &Behavior::default(), false);
                check(&j, &expected);
            }
        }
        assert_eq!(Workers::with_participants(8).participants, 4);
        assert_eq!(Workers::with_participants(0).participants, 1);
    }
    #[test]
    fn claimed_late_chunk_is_recomputed_without_waiting_and_shutdown_joins() {
        let (frame, b) = scene();
        let j = job(&frame, &b);
        let expected = expected(&j);
        let (entered, wait) = std::sync::mpsc::channel();
        let gate = Arc::new(Gate {
            entered,
            released: std::sync::Mutex::new(false),
            wakeup: std::sync::Condvar::new(),
        });
        let pool = Pool::new(
            2,
            Behavior {
                gate: Some(gate.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        ready(&pool);
        pool.dispatch(&j);
        wait.recv_timeout(Duration::from_secs(2)).unwrap();
        // Worker holds an unpublished chunk and cannot leave until after this
        // check. Any rendezvous in finish would deadlock this regression.
        j.drain(&pool.stop, &Behavior::default(), false);
        check(&j, &expected);
        assert!(j.results[0].get().is_none());
        j.cancelled.store(true, Ordering::Release);
        drop(pool);
        assert!(*gate.released.lock().unwrap());
    }
    #[test]
    fn twenty_ms_worker_and_panicking_worker_never_publish_stale_results() {
        let (frame, b) = scene();
        for panic in [false, true] {
            let j = job(&frame, &b);
            let expected = expected(&j);
            let (claimed, wait) = std::sync::mpsc::channel();
            let pool = Pool::new(
                2,
                Behavior {
                    delay: Duration::from_millis(20),
                    panic,
                    claimed: Some(claimed),
                    ..Default::default()
                },
            )
            .unwrap();
            ready(&pool);
            pool.dispatch(&j);
            wait.recv_timeout(Duration::from_secs(2)).unwrap();
            j.drain(&pool.stop, &Behavior::default(), false);
            check(&j, &expected);
            j.cancelled.store(true, Ordering::Release);
            drop(pool);
            check(&j, &expected);
        }
    }
    // Diagnostic only: run the compiled test binary directly, not under cargo,
    // so builds cannot overlap this density/cutoff measurement.
    #[test]
    #[ignore]
    fn measure_pool_density() {
        let mut bank = Bank::new().unwrap();
        println!(
            "available_parallelism={:?}",
            thread::available_parallelism()
        );
        let paced = std::env::var_os("PELLET_DENSITY_PACED").is_some();
        println!("density,settled,participants,repeat,p50_ns,p95_ns");
        for settled in [false, true] {
            for count in [1024, 2048, 4096, 8192, 16384, 25000] {
                if paced && count > 8192 {
                    continue;
                }
                let mut world = crate::pellet_batch::tests::rectangle_count(&bank, count);
                if settled {
                    for _ in 0..12 {
                        world
                            .advance(crate::pellet_batch::tests::step(5.), |_| unreachable!())
                            .unwrap();
                    }
                }
                let b = batch(
                    &mut crate::pellet_batch::Cache::default(),
                    &mut bank,
                    &world,
                    view(),
                );
                let frame = Frame {
                    identity: 1,
                    extent: [1500, 844],
                    viewport_scale: 1.,
                    tick: 0,
                    draws: Vec::new(),
                    food_batch: None,
                    snakes: Vec::new(),
                };
                let mut mirror = vec![[0; 96]; count];
                let mut flags = vec![0; count];
                let mut pools = [Workers::with_participants(2), Workers::with_participants(4)];
                for pool in &mut pools {
                    pool.attempted = true;
                    pool.pool = Pool::new(pool.participants, Behavior::default());
                    ready(pool.pool.as_ref().unwrap());
                }
                for repeat in 0..5 {
                    for n in if repeat % 2 == 0 {
                        [1, 2, 4]
                    } else {
                        [4, 2, 1]
                    } {
                        let mut values = Vec::new();
                        for _ in 0..105 {
                            if paced {
                                thread::sleep(Duration::from_millis(4));
                            }
                            let mut cover = [0.; 4];
                            let begin = Instant::now();
                            if n == 1 {
                                for (i, f) in b.world.foods().iter().enumerate().rev() {
                                    let slot = b.sources.slots[i];
                                    project(
                                        &frame,
                                        Input {
                                            slot,
                                            animation: f.paint_state(),
                                        },
                                        &b.sources,
                                        b.view,
                                        b.user_zoom,
                                        b.physical_pixels_per_unit,
                                        frame.extent,
                                    )
                                    .unwrap()
                                    .apply(
                                        &mut mirror,
                                        &mut flags,
                                        &mut cover,
                                    );
                                }
                            } else {
                                let pool = &mut pools[usize::from(n == 4)];
                                if !pool
                                    .prepare_with_cutoff(
                                        &frame,
                                        &b,
                                        frame.extent,
                                        &mut mirror,
                                        &mut flags,
                                        &mut cover,
                                        0,
                                    )
                                    .unwrap()
                                {
                                    for (i, f) in b.world.foods().iter().enumerate().rev() {
                                        let slot = b.sources.slots[i];
                                        project(
                                            &frame,
                                            Input {
                                                slot,
                                                animation: f.paint_state(),
                                            },
                                            &b.sources,
                                            b.view,
                                            b.user_zoom,
                                            b.physical_pixels_per_unit,
                                            frame.extent,
                                        )
                                        .unwrap()
                                        .apply(
                                            &mut mirror,
                                            &mut flags,
                                            &mut cover,
                                        );
                                    }
                                }
                            }
                            values.push(begin.elapsed().as_nanos());
                            std::hint::black_box(&mirror);
                        }
                        let mut values = values.split_off(5);
                        values.sort_unstable();
                        println!(
                            "{count},{settled},{n},{repeat},{},{}",
                            values[50], values[94]
                        );
                    }
                }
            }
        }
    }

    #[cfg(all(windows, feature = "frame-profile"))]
    #[test]
    #[ignore = "explicit authored offscreen pool/serial lifecycle pixels"]
    fn pool_pixels_match_serial_across_quality_overlay_prefix_resize_and_reconnect() {
        use crate::{
            gpu::{Renderer, instance},
            menu::MenuFrame,
            world_presenter::WorldPresenter,
        };
        use slither_client::{presentation::*, settings::WorldResolution, state::Point};
        let integrated = std::env::var_os("PELLET_TEST_IGPU").is_some();
        let backend = if integrated {
            wgpu::Backends::DX12
        } else {
            wgpu::Backends::VULKAN
        };
        let make = || {
            let adapter = pollster::block_on(instance().enumerate_adapters(backend))
                .into_iter()
                .find(|a| {
                    (a.get_info().device_type == wgpu::DeviceType::IntegratedGpu) == integrated
                })
                .expect("explicit offscreen adapter");
            pollster::block_on(Renderer::from_adapter_with_max_samples(
                adapter,
                wgpu::TextureFormat::Rgba8Unorm,
                4,
            ))
            .unwrap()
        };
        let mut serial = make();
        let mut parallel = make();
        serial.configure_pellet_workers(1, Duration::ZERO);
        parallel.configure_pellet_workers(4, Duration::ZERO);
        let mut sp = WorldPresenter::new(&serial);
        let mut pp = WorldPresenter::new(&parallel);
        let mut bank = Bank::new().unwrap();
        let mut world = rectangle(&bank);
        let mut cache = crate::pellet_batch::Cache::default();
        for tick in 0..12 {
            if tick == 6 {
                world = rectangle(&bank);
                cache = Default::default();
            }
            if tick == 4 {
                world.consume(
                    17,
                    FoodConsume::Eaten(Some(SnakeObjectToken::new(1).unwrap())),
                );
            }
            world
                .advance(crate::pellet_batch::tests::step(5.), |_| {
                    Ok(FoodEaterPose {
                        head: Point {
                            x: 16190.,
                            y: 16130.,
                        },
                        head_offset: Point::default(),
                        heading: 0.3,
                        angle_offset: 0.1,
                    })
                })
                .unwrap();
            let physical = if tick % 3 == 0 {
                [1920, 1080]
            } else {
                [3840, 2160]
            };
            let quality = if tick % 2 == 0 {
                WorldResolution::Original
            } else {
                WorldResolution::Native
            };
            let mut frame = Frame {
                identity: tick + 1,
                extent: [1500, 844],
                viewport_scale: 1.,
                tick: tick as u32,
                draws: Vec::new(),
                food_batch: Some(batch(&mut cache, &mut bank, &world, view())),
                snakes: Vec::new(),
            };
            let page = MenuFrame::world_crossfade(frame.extent, 1., 0.4).unwrap();
            let overlay = (tick % 3 == 1).then(|| Frame {
                identity: tick + 10,
                extent: physical,
                viewport_scale: 1.,
                tick: tick as u32,
                draws: vec![page.world_cover.clone().unwrap()],
                food_batch: None,
                snakes: Vec::new(),
            });
            let mut resources = bank.resources(&frame).unwrap();
            if tick % 3 == 2 {
                frame.draws.push(page.backdrop.clone().unwrap());
                let b = frame.food_batch.as_mut().unwrap();
                b.early_at += 1;
                b.late_at += 1;
            }
            resources.extend(page.resources);
            serial.sync_resources(&resources).unwrap();
            parallel.sync_resources(&resources).unwrap();
            let actual = crate::pellet_batch::offscreen::target(&parallel, physical);
            let expected = crate::pellet_batch::offscreen::target(&serial, physical);
            sp.render(
                &mut serial,
                &frame,
                overlay.as_ref(),
                &expected.create_view(&Default::default()),
                physical,
                quality,
            )
            .unwrap();
            pp.render(
                &mut parallel,
                &frame,
                overlay.as_ref(),
                &actual.create_view(&Default::default()),
                physical,
                quality,
            )
            .unwrap();
            assert_eq!(
                crate::pellet_batch::offscreen::pixels(&parallel, &actual).unwrap(),
                crate::pellet_batch::offscreen::pixels(&serial, &expected).unwrap(),
                "tick={tick} quality={quality:?}"
            );
        }
        // CPU workers hold no device handles. Exercise dropping a used renderer
        // after explicit device teardown as well as normal owner teardown.
        parallel.device.destroy();
        drop(parallel);
        drop(serial);
    }

    #[test]
    fn sparse_and_serial_admission_spawn_nothing() {
        for available in 0..4 {
            assert_eq!(default_participants(available), 1);
        }
        for available in [4, 8, 16] {
            assert_eq!(default_participants(available), 4);
        }
        let (frame, mut b) = scene();
        let mut mirror = vec![[0; 96]; b.visible.len()];
        let mut flags = vec![0; b.visible.len()];
        let mut cover = [0.; 4];
        let mut serial = Workers::with_participants(1);
        assert!(
            !serial
                .prepare(
                    &frame,
                    &b,
                    frame.extent,
                    &mut mirror,
                    &mut flags,
                    &mut cover
                )
                .unwrap()
        );
        assert!(!serial.attempted);
        b.visible_count = 0;
        b.visible.fill(false);
        let mut sparse = Workers::with_participants(4);
        for _ in 0..50 {
            assert!(
                !sparse
                    .prepare(
                        &frame,
                        &b,
                        frame.extent,
                        &mut mirror,
                        &mut flags,
                        &mut cover
                    )
                    .unwrap()
            );
        }
        assert!(!sparse.attempted);
        assert!(sparse.pool.is_none());
    }
    #[test]
    fn missing_and_published_chunks_return_the_same_first_error() {
        let (frame, b) = scene();
        let mut j = job(&frame, &b);
        let j = Arc::get_mut(&mut j).unwrap();
        j.inputs[1].animation.render_position.x = f64::INFINITY;
        j.inputs[CHUNK + 3].animation.render_position.y = f64::NAN;
        let expected = j.one(j.inputs[1]).err().unwrap();
        for computed in [false, true] {
            if computed {
                j.drain(&AtomicBool::new(false), &Behavior::default(), false);
            }
            let mut mirror = vec![[0; 96]; b.visible.len()];
            let mut flags = vec![0; b.visible.len()];
            let mut cover = [0.; 4];
            assert_eq!(
                j.finish(&mut mirror, &mut flags, &mut cover).unwrap_err(),
                expected
            );
        }
    }
}
