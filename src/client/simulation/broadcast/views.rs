//! Recipient visibility prepared ahead of the serial recipient loop.
//!
//! Workers read only the tick's immutable [`ViewQuery`] and each recipient's
//! own snake history, which nothing but that recipient's serial updates
//! changes. The serial loop consumes the results in ascending slot order, so
//! histories, delta caches, food diffs and stale-report observations keep
//! their single owner and frames stay byte-identical for any pool size.
#[cfg(feature = "parallel")]
use super::ViewQuery;
#[cfg(feature = "parallel")]
use crate::client::{
    server::HostPool,
    simulation::{
        connection::Slot,
        snakes::{KnownSnakes, covered},
    },
};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// Recipients per worker task.
#[cfg(feature = "parallel")]
const CHUNK: usize = 64;
/// Tasks per worker in one wave. Waves bound retained results while leaving
/// idle workers other tasks to take over uneven views.
#[cfg(feature = "parallel")]
const WAVE_CHUNKS_PER_THREAD: usize = 4;
/// Recipient ranges shorter than this keep the serial queries. On a 16-core
/// c8a.4xlarge, forced splits made 500-bot hosts 8-15% slower, were
/// inconclusive at 1,000 bots and 8% faster at 2,000; see docs/performance.md.
#[cfg(feature = "parallel")]
const MIN_RANGE: usize = 1024;

/// One recipient's coverage and prey candidates, as the serial query would
/// compute them at the start of its updates.
#[derive(Default)]
#[cfg_attr(not(feature = "parallel"), allow(dead_code))]
pub(super) struct Visibility {
    ready: bool,
    /// Coverage candidates other than the recipient, ascending, each with
    /// `alive && covered`.
    pub(super) snakes: Vec<(usize, bool)>,
    /// Indices of prey within vision in `PreyBounds::near` order, so a
    /// duplicate ID still resolves to the first visible prey.
    pub(super) prey: Vec<usize>,
}

#[cfg(feature = "parallel")]
impl Visibility {
    fn prepare(
        &mut self,
        query: &ViewQuery<'_>,
        slot: usize,
        known: &KnownSnakes,
        scratch: &mut Vec<usize>,
    ) {
        let head = query.snakes[slot].head;
        known.candidates(query.grid.near(head), scratch);
        self.snakes.clear();
        self.snakes
            .extend(scratch.iter().filter(|&&i| i != slot).map(|&i| {
                let snake = &query.snakes[i];
                let eligible = snake.alive
                    && covered(
                        head,
                        snake,
                        query.bounds[i],
                        query.vision,
                        query.body_bounds.for_snake(i),
                    );
                (i, eligible)
            }));
        self.prey.clear();
        self.prey.extend(
            query
                .prey_bounds
                .near(head)
                .filter(|&i| query.prey[i].position.distance_le(head, query.vision)),
        );
        self.ready = true;
    }
}

/// A host-owned pool for recipient visibility, with retained wave storage.
#[cfg(feature = "parallel")]
pub(in crate::client::simulation) struct ViewWorkers {
    pool: HostPool,
    chunk: usize,
    min_range: usize,
    wave: Vec<Visibility>,
}

#[cfg(feature = "parallel")]
impl ViewWorkers {
    pub(in crate::client::simulation) fn new(pool: HostPool) -> Self {
        Self {
            pool,
            chunk: CHUNK,
            min_range: MIN_RANGE,
            wave: Vec::new(),
        }
    }

    /// Diagnostic split sizes, so small fixtures exercise every boundary.
    #[cfg(any(test, feature = "perf-internals"))]
    pub(in crate::client::simulation) fn with_split(
        pool: HostPool,
        chunk: Option<usize>,
        min_range: Option<usize>,
    ) -> Self {
        Self {
            chunk: chunk.unwrap_or(CHUNK).max(1),
            min_range: min_range.unwrap_or(MIN_RANGE),
            ..Self::new(pool)
        }
    }

    pub(super) fn waves(&mut self) -> Waves {
        Waves {
            wave: std::mem::take(&mut self.wave),
            start: 0,
            ready: 0,
            end: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn retained(&self) -> usize {
        self.wave.len()
    }

    /// Keep the wave allocations for the next broadcast.
    pub(super) fn retain(&mut self, waves: Waves) {
        self.wave = waves.wave;
    }
}

/// Prepared results for `start..start + ready` within the current range part
/// `start..end`; slots past `ready` use the serial query.
#[cfg(feature = "parallel")]
pub(super) struct Waves {
    wave: Vec<Visibility>,
    start: usize,
    ready: usize,
    end: usize,
}

#[cfg(feature = "parallel")]
impl Waves {
    /// Prepare the wave holding `slot` unless it is already decided. The
    /// socket prefix `..early` and the bot range are separate, so the early
    /// socket handoff never waits for bot views.
    pub(super) fn ensure(
        &mut self,
        views: &ViewWorkers,
        query: &ViewQuery<'_>,
        slots: &[Slot],
        slot: usize,
        early: usize,
    ) {
        if (self.start..self.end).contains(&slot) {
            return;
        }
        let early = early.min(slots.len());
        let (first, last) = if slot < early {
            (0, early)
        } else {
            (early, slots.len())
        };
        self.start = slot;
        self.ready = 0;
        if last - first < views.min_range {
            self.end = last;
            return;
        }
        let pool = views.pool.pool();
        let chunk = views.chunk;
        let end = last.min(slot + chunk * pool.current_num_threads() * WAVE_CHUNKS_PER_THREAD);
        self.end = end;
        let len = end - slot;
        if self.wave.len() < len {
            self.wave.resize_with(len, Visibility::default);
        }
        // Recipients not set up now are skipped by the loop; mark them unready
        // so a later serial query can never read a stale result.
        let known: Vec<_> = slots[slot..end]
            .iter()
            .map(|s| s.connection.setup.then_some(&s.connection.snakes))
            .collect();
        let wave = &mut self.wave[..len];
        pool.install(|| {
            wave.par_chunks_mut(chunk)
                .zip(known.par_chunks(chunk))
                .enumerate()
                .for_each_init(Vec::new, |scratch, (part, (out, known))| {
                    for (k, (out, known)) in out.iter_mut().zip(known).enumerate() {
                        match known {
                            Some(known) => {
                                out.prepare(query, slot + part * chunk + k, known, scratch)
                            }
                            None => out.ready = false,
                        }
                    }
                });
        });
        self.ready = len;
    }

    pub(super) fn get(&self, slot: usize) -> Option<&Visibility> {
        let index = slot.checked_sub(self.start)?;
        (index < self.ready)
            .then(|| &self.wave[index])
            .filter(|v| v.ready)
    }
}
