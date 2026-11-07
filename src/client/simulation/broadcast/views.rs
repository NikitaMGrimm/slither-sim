//! Recipient updates assembled by view workers ahead of the serial loop.
//!
//! A job reads the tick's immutable [`ViewQuery`], compact life/control facts
//! and its own recipient's view, which nothing but that recipient's commit
//! changes during the broadcast. It assembles into a copy of that view with
//! task-local delta caches and food diffs, and records stale-report
//! contributions instead of writing them. The coordinator commits jobs in
//! ascending slot order at the serial updates position, then runs the
//! unchanged map, HUD and emission. A job's error is committed with its
//! partial view, as the serial path would leave it; later jobs are discarded.
use super::{
    PacketViews, ViewQuery,
    assemble::{Effects, Seat, View, assemble},
};
use crate::client::{
    fast_hash::FastMap,
    server::HostPool,
    simulation::{
        PlayerAppearance,
        connection::{Connection, KnownPrey, Slot},
        food::{FoodDiffs, FoodEdit, FoodQuery, KnownSectors, SectorFood, stage_food},
        snakes::{KnownSnake, KnownSnakes, SnakeDeltaCache, SnakeReports},
    },
    visibility::SectorInterest,
};
use crate::{WorldRules, model::Snake};
use rayon::prelude::*;
use std::{collections::BTreeMap, sync::Arc};

/// Recipients per worker task.
const CHUNK: usize = 64;
/// Tasks per worker in one wave. Waves bound staged packets while leaving
/// idle workers other tasks to take over uneven views.
const WAVE_CHUNKS_PER_THREAD: usize = 4;
/// Recipient ranges shorter than this keep the serial updates.
const MIN_RANGE: usize = 1024;

/// A recipient's view as the job found it. Only shared references cross to
/// workers; connection queues and decoders are never touched.
struct Base<'a> {
    over: bool,
    snakes: &'a KnownSnakes,
    credit: u8,
    radius: u32,
    rules_sent: Option<WorldRules>,
    appeared: bool,
    session_pending: bool,
    prey: &'a BTreeMap<u64, KnownPrey>,
    food: &'a KnownSectors,
    decorated_food: &'a KnownSectors,
    seat: Seat<'a>,
}

impl<'a> Base<'a> {
    fn of(connection: &'a Connection) -> Self {
        Self {
            over: connection.over,
            snakes: &connection.snakes,
            credit: connection.credit,
            radius: connection.radius,
            rules_sent: connection.rules_sent,
            appeared: connection.appeared,
            session_pending: connection.session_pending,
            prey: &connection.prey,
            food: &connection.food,
            decorated_food: &connection.decorated_food,
            seat: Seat::of(connection),
        }
    }
}

/// Held food listings stay borrowed; the update returns an edit.
struct StagedFood<'a> {
    base: &'a KnownSectors,
    edit: Option<FoodEdit>,
}

/// A worker's effects: facts copied once per broadcast, task-local caches,
/// and stale-report contributions in observation order.
struct Staged<'a> {
    facts: &'a [(u64, f64)],
    reports: &'a [SnakeReports],
    deltas: &'a mut FastMap<usize, SnakeDeltaCache>,
    diffs: &'a FoodDiffs,
    stale: Vec<(u32, (bool, bool))>,
}

impl<'a> Effects for Staged<'a> {
    type Food = StagedFood<'a>;
    fn life(&self, slot: usize) -> u64 {
        self.facts[slot].0
    }
    fn heading(&self, slot: usize) -> f64 {
        self.facts[slot].1
    }
    fn changes(
        &mut self,
        slot: usize,
        packets: &mut Vec<Vec<u8>>,
        own: bool,
        known: &mut KnownSnake,
        snake: &Snake,
        wanted: impl FnOnce() -> f64,
        profile: &PlayerAppearance,
    ) -> Result<(), String> {
        // Delta caches only memoize the same deterministic encoding, so a
        // sparse cache per task yields the bytes a shared one would.
        self.deltas.entry(slot).or_default().changes(
            packets,
            own,
            slot as u16,
            known,
            snake,
            wanted,
            profile,
            &self.reports[slot],
        )
    }
    fn observe(&mut self, slot: usize, known: &KnownSnake) {
        let stale = self.reports[slot].stale(known);
        if stale != (false, false) {
            self.stale.push((slot as u32, stale));
        }
    }
    fn food(
        &mut self,
        packets: &mut Vec<Vec<u8>>,
        known: &mut StagedFood<'a>,
        interest: &SectorInterest,
        sectors: &[Option<Arc<SectorFood>>],
        changes: FoodQuery<'_>,
        eaters: &BTreeMap<(u64, u64), u16>,
    ) {
        known.edit = Some(stage_food(
            packets, known.base, interest, sectors, changes, self.diffs, eaters,
        ));
    }
}

/// One recipient's assembled update, waiting for its slot-order commit.
pub(super) struct Job {
    view: View<Option<FoodEdit>>,
    stale: Vec<(u32, (bool, bool))>,
    result: Result<PacketViews, String>,
}

/// Task-local scratch shared by the recipients of one chunk.
#[derive(Default)]
struct Scratch {
    deltas: FastMap<usize, SnakeDeltaCache>,
    diffs: FoodDiffs,
    candidates: Vec<usize>,
}

impl Job {
    fn run(
        query: &ViewQuery<'_>,
        slot: usize,
        base: &Base<'_>,
        facts: &[(u64, f64)],
        reports: &[SnakeReports],
        scratch: &mut Scratch,
    ) -> Self {
        let mut view = View {
            over: base.over,
            snakes: base.snakes.clone(),
            credit: base.credit,
            radius: base.radius,
            rules_sent: base.rules_sent,
            appeared: base.appeared,
            session_pending: base.session_pending,
            prey: base.prey.clone(),
            food: StagedFood {
                base: base.food,
                edit: None,
            },
            decorated_food: StagedFood {
                base: base.decorated_food,
                edit: None,
            },
        };
        let mut effects = Staged {
            facts,
            reports,
            deltas: &mut scratch.deltas,
            diffs: &scratch.diffs,
            stale: Vec::new(),
        };
        let result = assemble(
            query,
            slot,
            base.seat,
            &mut view,
            &mut effects,
            &mut scratch.candidates,
        );
        let View {
            over,
            snakes,
            credit,
            radius,
            rules_sent,
            appeared,
            session_pending,
            prey,
            food,
            decorated_food,
        } = view;
        Self {
            view: View {
                over,
                snakes,
                credit,
                radius,
                rules_sent,
                appeared,
                session_pending,
                prey,
                food: food.edit,
                decorated_food: decorated_food.edit,
            },
            stale: effects.stale,
            result,
        }
    }

    /// Apply the staged view and stale contributions, then return the
    /// update or the error the serial path would have returned here.
    pub(super) fn commit(
        self,
        connection: &mut Connection,
        reports: &mut [SnakeReports],
        query: &ViewQuery<'_>,
    ) -> Result<PacketViews, String> {
        let View {
            over,
            snakes,
            credit,
            radius,
            rules_sent,
            appeared,
            session_pending,
            prey,
            food,
            decorated_food,
        } = self.view;
        connection.over = over;
        connection.snakes = snakes;
        connection.credit = credit;
        connection.radius = radius;
        connection.rules_sent = rules_sent;
        connection.appeared = appeared;
        connection.session_pending = session_pending;
        connection.prey = prey;
        if let Some(edit) = food {
            connection.food.apply(
                edit,
                &connection.food_interest,
                query.food,
                query.food_changes,
            );
        }
        if let Some(edit) = decorated_food {
            connection.decorated_food.apply(
                edit,
                &connection.food_interest,
                query.decorated_food,
                query.food_changes,
            );
        }
        for (slot, stale) in self.stale {
            reports[slot as usize].fold(stale);
        }
        self.result
    }
}

/// A host-owned pool for recipient updates, with retained wave storage.
pub(in crate::client::simulation) struct ViewWorkers {
    pool: HostPool,
    chunk: usize,
    min_range: usize,
    wave: Vec<Option<Job>>,
}

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
            facts: Vec::new(),
            start: 0,
            end: 0,
        }
    }

    #[cfg(test)]
    pub(super) fn retained(&self) -> usize {
        self.wave.capacity()
    }

    /// Keep the wave allocation for the next broadcast.
    pub(super) fn retain(&mut self, mut waves: Waves) {
        waves.wave.clear();
        self.wave = waves.wave;
    }
}

/// Jobs for the current range part `start..end`; a slot without one uses the
/// serial updates.
pub(super) struct Waves {
    wave: Vec<Option<Job>>,
    /// Life and control heading of every slot, copied for this broadcast.
    facts: Vec<(u64, f64)>,
    start: usize,
    end: usize,
}

impl Waves {
    /// Assemble the wave holding `slot` unless it is already decided. The
    /// socket prefix `..early` and the bot range are separate, so the early
    /// socket handoff never waits for bot views. Every job of a wave finishes
    /// before its first commit.
    pub(super) fn ensure(
        &mut self,
        views: &ViewWorkers,
        query: &ViewQuery<'_>,
        slots: &[Slot],
        reports: &[SnakeReports],
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
        self.wave.clear();
        if last - first < views.min_range {
            self.end = last;
            return;
        }
        if self.facts.is_empty() {
            self.facts = slots.iter().map(|s| (s.life, s.control.heading)).collect();
        }
        let pool = views.pool.pool();
        let chunk = views.chunk;
        let end = last.min(slot + chunk * pool.current_num_threads() * WAVE_CHUNKS_PER_THREAD);
        self.end = end;
        // Slots not set up now are skipped by the loop and get no job.
        let bases: Vec<_> = slots[slot..end]
            .iter()
            .map(|s| s.connection.setup.then(|| Base::of(&s.connection)))
            .collect();
        self.wave.resize_with(end - slot, || None);
        let (wave, facts) = (&mut self.wave, &self.facts);
        pool.install(|| {
            wave.par_chunks_mut(chunk)
                .zip(bases.par_chunks(chunk))
                .enumerate()
                .for_each(|(part, (out, bases))| {
                    let mut scratch = Scratch::default();
                    for (k, (out, base)) in out.iter_mut().zip(bases).enumerate() {
                        *out = base.as_ref().map(|base| {
                            let slot = slot + part * chunk + k;
                            Job::run(query, slot, base, facts, reports, &mut scratch)
                        });
                    }
                });
        });
    }

    pub(super) fn take(&mut self, slot: usize) -> Option<Job> {
        self.wave.get_mut(slot.checked_sub(self.start)?)?.take()
    }
}
