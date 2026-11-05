//! Shared tick scheduling and recipient-ordered broadcasts.
mod assemble;
#[cfg(any(test, feature = "perf-internals"))]
#[doc(hidden)]
pub mod perf;
#[cfg(feature = "parallel")]
mod views;
use super::{
    EarlyFrames, MAX_PREY, PlayerAppearance, SimulatorBackend,
    connection::Slot,
    food::{FoodDiffs, FoodQuery, KnownSectors, SectorFood, food_identity, prepare_food},
    minimap::MinimapBroadcast,
    prey_visibility::PreyBounds,
    snakes::{BodyBounds, CoverageGrid, KnownSnake, SnakeBroadcast, SnakeDeltaCache, SnakeReports},
};
use crate::{
    Prey, Simulator, WorldRules, client::encode, model::Snake, physics, simulator::PickupItem,
};
use assemble::{Effects, Seat, View, assemble};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
#[cfg(feature = "parallel")]
pub(super) use views::ViewWorkers;

/// Immutable inputs retained for this tick's recipient preparation. Borrow
/// individual fields, never the backend or connection queues. Lazy caches,
/// recipient histories and stale-report observations stay on the serial owner.
struct ViewQuery<'a> {
    snakes: &'a [Snake],
    prey: &'a [Prey],
    radius: f64,
    rules: WorldRules,
    vision: f64,
    profiles: &'a [PlayerAppearance],
    record_life: Option<(usize, u64)>,
    bounds: &'a [[f64; 4]],
    grid: &'a CoverageGrid,
    body_bounds: &'a BodyBounds,
    prey_bounds: &'a PreyBounds,
    food: &'a [Option<Arc<SectorFood>>],
    decorated_food: &'a [Option<Arc<SectorFood>>],
    decorations: bool,
    food_changes: FoodQuery<'a>,
    food_eaters: &'a BTreeMap<(u64, u64), u16>,
    prey_eaters: &'a BTreeMap<u64, u16>,
    prey_wire: &'a BTreeMap<u64, u16>,
    prey_clocks: &'a BTreeMap<u64, PreyClock>,
}

/// Borrow one snake's frozen encoding inputs before mutating a recipient or
/// observing its history. Life/control are scalars, not a borrow of its queue.
struct SnakeQuery<'world> {
    snake: &'world Snake,
    life: u64,
    heading: f64,
    profile: &'world PlayerAppearance,
}

/// Rows the stock leaderboard shows.
pub(super) const LEADERBOARD_ROWS: usize = 10;
/// Physical packets, plus a browser view when pit decoration is active.
type PacketViews = (Vec<Vec<u8>>, Option<Vec<Vec<u8>>>);

/// The serial owner's effects: facts from the slots, the broadcast's shared
/// delta caches and food diffs, and immediate stale-report observations.
struct Serial<'a> {
    slots: &'a [Slot],
    reports: &'a mut [SnakeReports],
    deltas: &'a mut [SnakeDeltaCache],
    diffs: &'a FoodDiffs,
}

impl Effects for Serial<'_> {
    type Food = KnownSectors;
    fn life(&self, slot: usize) -> u64 {
        self.slots[slot].life
    }
    fn heading(&self, slot: usize) -> f64 {
        self.slots[slot].control.heading
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
        self.deltas[slot].changes(
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
        self.reports[slot].observe(known);
    }
    fn food(
        &mut self,
        packets: &mut Vec<Vec<u8>>,
        known: &mut KnownSectors,
        interest: &crate::client::visibility::SectorInterest,
        sectors: &[Option<Arc<SectorFood>>],
        changes: FoodQuery<'_>,
        eaters: &BTreeMap<(u64, u64), u16>,
    ) {
        prepare_food(
            packets, known, interest, sectors, changes, self.diffs, eaters,
        );
    }
}

/// One prey's correction clock, shared by every recipient: official
/// corrections for one prey align across clients, and a new viewer does not
/// restart it.
#[derive(Clone, Copy, Debug)]
pub(super) struct PreyClock {
    reported_at: f64,
    speed: u32,
    due: bool,
}

/// Highest-scoring completed life in this world run. It names the slot and
/// life so a later session in a reused slot cannot change it.
#[derive(Clone, Debug)]
pub(super) struct Record {
    slot: usize,
    life: u64,
    segments: usize,
    fam: u32,
    score: f64,
    pub(super) name: Vec<u8>,
    pub(super) message: Vec<u8>,
    pub(super) message_set: bool,
    version: u64,
}

/// Rank living seats by size, or living/recently scoring seats by rolling kills.
/// Ties use slot order; a newly admitted socket player clears the prior history.
pub(super) struct Board {
    /// `(display score, slot)` in rank order.
    pub(super) order: Vec<(f64, usize)>,
    /// Encoded top rows shared by every connection's packet.
    pub(super) rows: Vec<u8>,
}
impl Board {
    pub(super) fn new(world: &Simulator, profiles: &[PlayerAppearance]) -> Result<Self, String> {
        let kill_points = world.config().kill_score_seconds.is_some();
        let mut order: Vec<(f64, usize)> = world
            .snakes()
            .iter()
            .enumerate()
            .filter(|&(slot, snake)| {
                snake.alive || kill_points && world.kill_score(slot).unwrap_or(0) > 0
            })
            .map(|(slot, snake)| {
                (
                    if kill_points {
                        world.kill_score(slot).unwrap_or(0) as f64
                    } else {
                        wire_score(snake)
                    },
                    slot,
                )
            })
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let mut rows = Vec::new();
        for &(_, slot) in order.iter().take(LEADERBOARD_ROWS) {
            let (segments, fam) = if kill_points {
                encode::leaderboard_points(world.kill_score(slot).unwrap_or(0))?
            } else {
                let (segments, fam, _) = represented(&world.snakes()[slot]);
                (segments, fam)
            };
            let (color, name) = match profiles.get(slot) {
                Some(profile) => (profile.skin, profile.nickname.clone()),
                None => (0, slot.to_string().into_bytes()),
            };
            encode::leaderboard_row(&mut rows, segments, fam, color, &name)?;
        }
        Ok(Self { order, rows })
    }
    /// One-based rank a snake with this final score would hold among the living.
    pub(super) fn rank(&self, score: f64, slot: usize) -> usize {
        1 + self
            .order
            .partition_point(|&(other, i)| other > score || (other == score && i < slot))
    }
}

/// Wire representation of a snake's score: segments plus the byte credit the
/// stock client adds to its own snake (`R`), the quantized fam and the display
/// score they produce. Leaderboards and records rank and show this score; its
/// precision is bounded by whole credited segments.
fn represented(snake: &Snake) -> (usize, u32, f64) {
    let fam = encode::fam_u24(snake.fam);
    let credit = encode::segment_credit(snake.score, snake.segments, fam);
    let segments = snake.segments + usize::from(credit);
    (segments, fam, encode::display_score(segments, fam))
}
fn wire_score(snake: &Snake) -> f64 {
    represented(snake).2
}

impl SimulatorBackend {
    #[cfg(feature = "frame-bench")]
    pub(super) fn frame_bench_admitted_bots(&self) -> usize {
        self.slots
            .iter()
            .skip(self.player_latency.slots)
            .filter(|s| s.connection.open && s.connection.setup)
            .count()
    }
    #[cfg(feature = "frame-bench")]
    fn frame_bench_actions(
        &mut self,
        mut actions: Vec<Option<crate::Action>>,
    ) -> Vec<Option<crate::Action>> {
        if self.frame_bench {
            for ((action, snake), slot) in actions
                .iter_mut()
                .zip(self.world.snakes())
                .zip(&mut self.slots)
                .take(self.player_latency.slots)
            {
                // Same measurement-only orbit for every socket client. Socket
                // heads skip body tests, but rim/pit deaths remain possible and
                // bots still collide with (and can die on) the player's body.
                let radius = self.world.radius() * 0.75;
                let distance = snake.head.x.hypot(snake.head.y);
                *action = snake.alive.then_some(crate::Action {
                    heading: snake.head.y.atan2(snake.head.x)
                        + std::f64::consts::FRAC_PI_2
                        + ((distance - radius) / 200.).atan(),
                    boost: false,
                });
                if let Some(applied) = *action {
                    slot.control = applied;
                }
            }
        }
        actions
    }

    fn advance_food_interest(&mut self, now: f64) {
        let slots = self.slots.iter_mut().zip(self.world.snakes());
        #[cfg(feature = "frame-bench")]
        let mut slots = slots;
        #[cfg(feature = "frame-bench")]
        if self.frame_bench && self.world.config().pit.is_some() {
            for (slot, _) in slots.by_ref().take(self.player_latency.slots) {
                if slot.connection.setup {
                    slot.connection.food_interest.all(self.world.radius());
                }
            }
        }
        for (slot, snake) in slots {
            if slot.connection.setup {
                slot.connection
                    .food_interest
                    .advance(snake.head, snake.heading, now);
            }
        }
    }
    /// Text from the client whose dead life holds the record, accepted once.
    /// Anything else is ignored: this is a local session rule, not an official
    /// qualification check.
    pub(super) fn victory(&mut self, slot: usize, text: Vec<u8>, now: f64) -> Result<(), String> {
        let life = self.slots[slot].life;
        let dead = !self.world.snakes()[slot].alive;
        if self.record.as_ref().is_some_and(|record| {
            record.slot == slot && record.life == life && dead && !record.message_set
        }) {
            // The stock client dismisses its victory entry on v2. Reserve and
            // queue that acknowledgement before accepting the message.
            self.emit(slot, now, &[encode::victory_accepted()])?;
            let record = self.record.as_mut().unwrap();
            record.message = text;
            record.message_set = true;
            record.version += 1;
        }
        Ok(())
    }

    /// A life that ended by death since the last tick may set the record; a
    /// tie keeps the earlier holder.
    fn settle_lives(&mut self) {
        if !self.config.local_records {
            return;
        }
        for slot in 0..self.slots.len() {
            let snake = &self.world.snakes()[slot];
            let s = &mut self.slots[slot];
            if snake.alive {
                s.living = Some(s.life);
                continue;
            }
            if s.living.take() != Some(s.life) {
                continue;
            }
            let (segments, fam, score) = represented(snake);
            if self
                .record
                .as_ref()
                .is_some_and(|record| record.score >= score)
            {
                continue;
            }
            let name = match self.profiles.get(slot) {
                Some(profile) => profile.nickname.clone(),
                None => slot.to_string().into_bytes(),
            };
            let version = self.record.as_ref().map_or(1, |record| record.version + 1);
            self.record = Some(Record {
                slot,
                life: s.life,
                segments,
                fam,
                score,
                name,
                message: Vec::new(),
                message_set: false,
                version,
            });
        }
    }

    /// One server tick. Client sides are then brought up to the new time, so
    /// frames polled between ticks are already complete.
    pub(super) fn advance(&mut self) -> Result<(), String> {
        self.advance_with(0, &mut |_| Ok(()))
    }

    /// `advance`, handing the frames of slots `..early` to `hand_over` as soon
    /// as the broadcast has encoded them, before it encodes later slots.
    pub(super) fn advance_with(
        &mut self,
        early: usize,
        hand_over: &mut EarlyFrames,
    ) -> Result<(), String> {
        // A server loop pass reads what arrived by the time it sends: input
        // arriving during this tick shapes this tick's frame. Official input
        // reflection takes about one ping round trip (13.2 vs 13.4 ms median)
        // and pings return in as little as 4.9 ms; ingesting only arrivals
        // before the tick began added a full tick to both. See section 10 of
        // the scheduler study linked from `MOTION_REPORT_TICKS`.
        let end = if self.variable_dt {
            self.time() + self.world.config().dt
        } else {
            (self.tick + 1) as f64 * self.world.config().dt
        };
        // A drainable capacity failure must not leave an earlier slot's client
        // clock ahead of public time. Check every due departure and its earlier
        // receipts before any client advances into this tick.
        for slot in &self.slots {
            if let Some((at, _, processing)) = slot.outbox
                && at + processing <= end
            {
                slot.client_io_room(at + processing)?;
            }
        }
        for slot in 0..self.slots.len() {
            if let Some((at, _, processing)) = self.slots[slot].outbox
                && at + processing <= end
            {
                // These decisions already exist: release them before ingest,
                // with earlier receipts first, so they reach their arrival tick.
                self.client_io_until(slot, at + processing)?;
            }
            self.ingest(slot, end)?;
        }
        self.drive_in_world_bots()?;
        let actions: Vec<_> = self
            .world
            .snakes()
            .iter()
            .zip(&self.slots)
            .map(|(snake, slot)| snake.alive.then_some(slot.control))
            .collect();
        #[cfg(feature = "frame-bench")]
        let actions = self.frame_bench_actions(actions);
        let radius = self.world.radius();
        #[cfg(feature = "latency-rig")]
        let truth_before = self.latency_truth.as_mut().map(|r| r.stamp()).transpose()?;
        self.world
            .step(&actions)
            .map_err(|error| error.to_string())?;
        #[cfg(feature = "latency-rig")]
        if let Some(recorder) = &mut self.latency_truth {
            let after = recorder.stamp()?;
            // Capture the successful physics result before presentation or encoding.
            // Slot life, not connection generation, identifies a reused wire ID.
            recorder.record(
                self.tick + 1,
                truth_before.unwrap(),
                after,
                &self.world,
                self.slots.iter().map(|s| s.life),
            )?;
        }
        // Mirror update_radius's food removal before births and pickups,
        // including the smaller live pit edge in pit-only food worlds.
        if (self.world.config().food_only_inside_pit && self.world.radius() != radius)
            || (self.world.config().dynamic_radius && self.world.radius() < radius)
        {
            let culled = self.food_index.cull(self.world.food_radius());
            self.food_presentation.cull(&culled);
        }
        let births = self.world.food_spawn_events().unwrap_or(&[]);
        // Physics appends boost/death food, visits pickups in reverse food
        // order, then appends ambient replenishment. Preserve that ordering for
        // co-located lifetimes, including a refill at a just-eaten point.
        for event in births.iter().filter(|event| event.source.is_some()) {
            self.food_presentation.births(std::slice::from_ref(event))?;
            let color = self.food_presentation.newest(&event.food);
            self.food_index.add(&event.food, true, color);
        }
        for event in self.world.pickup_events().unwrap_or(&[]) {
            if let PickupItem::Prey(prey) = &event.item {
                self.prey_eaters
                    .entry(prey.id)
                    .or_insert(event.eater as u16);
            }
            if let PickupItem::Food(food) = &event.item {
                self.food_index.remove(food);
                // Only the oldest co-located lifetime is on the wire. Eating
                // a hidden newer pellet must not animate that visible pellet.
                if self.food_presentation.picked_up(food)? {
                    self.food_eaters
                        .entry(food_identity(food.point()))
                        .or_insert(event.eater as u16);
                }
            }
        }
        for event in births.iter().filter(|event| event.source.is_none()) {
            self.food_presentation.births(std::slice::from_ref(event))?;
            let color = self.food_presentation.newest(&event.food);
            self.food_index.add(&event.food, false, color);
        }
        self.settle_lives();
        self.tick += 1;
        let now = self.time();
        self.advance_food_interest(now);
        for slot in &mut self.slots {
            // A decision that sent nothing is complete once its duration elapses.
            if slot
                .processing_override
                .is_some_and(|(at, seconds)| at < now && now >= at + seconds)
            {
                slot.processing_override = None;
            }
        }
        self.broadcast_with(early, hand_over, false)?;
        for slot in 0..self.slots.len() {
            self.client_until(slot, now)?;
        }
        Ok(())
    }

    /// Give `hand_over` every queued frame of slots `..early`.
    fn hand_over_early(&mut self, early: usize, hand_over: &mut EarlyFrames) -> Result<(), String> {
        let early = early.min(self.slots.len());
        let frames: Vec<_> = self.slots[..early]
            .iter_mut()
            .enumerate()
            .filter(|(_, s)| !s.connection.downlink.is_empty())
            .map(|(slot, s)| (slot, s.connection.take_downlink()))
            .collect();
        if frames.is_empty() {
            Ok(())
        } else {
            hand_over(frames)
        }
    }

    /// A broadcast between physics ticks, as diagnostic tests issue.
    #[cfg(test)]
    pub(super) fn broadcast(&mut self) -> Result<(), String> {
        self.broadcast_with(0, &mut |_| Ok(()), true)
    }

    fn broadcast_with(
        &mut self,
        early: usize,
        hand_over: &mut EarlyFrames,
        advance_interest: bool,
    ) -> Result<(), String> {
        self.legacy_maps.clear();
        let now = self.time();
        // Ambient births reach the wire at the shared food maintenance, with
        // sector cleanup; see `FoodIndex`.
        let epoch = (now / crate::client::visibility::CLEANUP_SECONDS).floor() as u64;
        if epoch > self.food_epoch {
            self.food_epoch = epoch;
            self.food_index.reveal();
        }
        // Food edited outside physics (fixtures) leaves the index behind.
        if self.food_index.stale || self.food_index.count != self.world.food().len() {
            let food = self.world.food_in_birth_order();
            self.food_presentation.sync(&food)?;
            self.food_index.rebuild(&food, &self.food_presentation);
        }
        self.update_reports()?;
        // HUD clocks run even without recipients, so a join meets their phase.
        let minimap_due = clock_due(
            &mut self.next_minimap,
            now,
            self.config.minimap_interval_seconds,
        );
        let leaderboard_due = clock_due(
            &mut self.next_leaderboard,
            now,
            self.config.leaderboard_interval_seconds,
        );
        if !self.slots.iter().any(|slot| slot.connection.setup) {
            self.food_eaters.clear();
            self.prey_eaters.clear();
            return Ok(());
        }
        // Normal ticks already advance every subscription before entering this
        // function, including before a broadcast can fail. Standalone broadcasts
        // still need the refresh after an edited pose, even at the same time.
        // Dead clients keep their subscriptions for the rendered death fade.
        if advance_interest {
            self.advance_food_interest(now);
        }
        let food_changes = self.sync_food_sectors()?;
        self.assign_prey_wire()?;
        self.update_prey_clocks()?;
        let bounds = self.world.presentation_bounds();
        let grid = if let Some(mut grid) = self.coverage_grid.take() {
            grid.update(self.world.snakes(), &bounds, self.world.config().vision);
            grid
        } else {
            CoverageGrid::new(self.world.snakes(), &bounds, self.world.config().vision)
        };
        let prey_bounds = PreyBounds::new(self.world.prey(), self.world.config().vision);
        // Physics, profiles and report opportunities stay fixed throughout this
        // broadcast. Only observe's stale flags accumulate between recipients;
        // they are consumed by the next broadcast's update_reports.
        let mut snakes = SnakeBroadcast::new(
            self.world.snakes(),
            std::mem::take(&mut self.coverage_bounds),
        );
        let mut board = None;
        let mut map = None;
        // Only configured view workers assemble updates in waves; otherwise
        // every recipient runs the serial updates.
        #[cfg(feature = "parallel")]
        let mut waves = self.views.as_mut().map(ViewWorkers::waves);
        for slot in 0..self.slots.len() {
            if slot == early {
                self.hand_over_early(early, hand_over)?;
            }
            let connection = &self.slots[slot].connection;
            if !connection.setup {
                continue;
            }
            let alive = self.world.snakes()[slot].alive;
            let was_over = connection.over;
            // The board is needed when a leaderboard is due or to freeze the
            // rank at death.
            if board.is_none() && (leaderboard_due || !alive && !connection.over) {
                board = Some(Board::new(&self.world, &self.profiles)?);
            }
            if !alive && !was_over {
                let snake = &self.world.snakes()[slot];
                let score = if self.world.config().kill_score_seconds.is_some() {
                    self.world.kill_score(slot).unwrap_or(0) as f64
                } else {
                    wire_score(snake)
                };
                let rank = board.as_ref().unwrap().rank(score, slot);
                self.slots[slot].connection.final_rank = Some(rank);
            }
            // Official frames lead with pongs (81,830 of 81,843 pong frames),
            // and put the map before gameplay updates.
            let pongs = std::mem::take(&mut self.slots[slot].connection.pongs);
            let mut prefix = if pongs == 0 {
                Vec::new()
            } else {
                vec![encode::pong(); pongs]
            };
            // Reborrow at the original updates position: map/HUD/emission and
            // early socket handoff keep their existing serial ownership/order.
            let query = ViewQuery {
                snakes: self.world.snakes(),
                prey: self.world.prey(),
                radius: self.world.radius(),
                rules: self.world.rules(),
                vision: self.world.config().vision,
                profiles: &self.profiles,
                record_life: self.record.as_ref().map(|r| (r.slot, r.life)),
                bounds: &bounds,
                grid: &grid,
                body_bounds: &snakes.body_bounds,
                prey_bounds: &prey_bounds,
                food: &self.food_sectors,
                decorated_food: &self.decorated_food_sectors,
                decorations: self.pit_markers.is_some(),
                food_changes: food_changes.query(),
                food_eaters: &self.food_eaters,
                prey_eaters: &self.prey_eaters,
                prey_wire: &self.prey_wire,
                prey_clocks: &self.prey_clocks,
            };
            // View workers assembled this wave ahead; commit this recipient's
            // staged view in slot order, at the serial updates position.
            #[cfg(feature = "parallel")]
            let staged = match (&mut waves, &self.views) {
                (Some(waves), Some(views)) => {
                    waves.ensure(views, &query, &self.slots, &self.reports, slot, early);
                    waves.take(slot)
                }
                _ => None,
            };
            #[cfg(feature = "parallel")]
            let views = match staged {
                Some(staged) => {
                    staged.commit(&mut self.slots[slot].connection, &mut self.reports, &query)
                }
                None => Self::updates(
                    &query,
                    slot,
                    &mut self.slots,
                    &mut self.reports,
                    &mut snakes.deltas,
                    &mut snakes.candidates,
                    &food_changes.diffs,
                ),
            };
            #[cfg(not(feature = "parallel"))]
            let views = Self::updates(
                &query,
                slot,
                &mut self.slots,
                &mut self.reports,
                &mut snakes.deltas,
                &mut snakes.candidates,
                &food_changes.diffs,
            );
            let (updates, presented_updates) = views?;
            if minimap_due && let Some(packet) = self.minimap(slot, &mut map)? {
                prefix.push(packet);
            }
            // Most broadcasts have neither pongs nor a map. Keep updates' own
            // allocation instead of moving every packet through another Vec.
            let (mut packets, mut presented) = if prefix.is_empty() {
                (updates, presented_updates)
            } else {
                let presented = presented_updates.map(|updates| {
                    let mut presented = prefix.clone();
                    presented.extend(updates);
                    presented
                });
                prefix.extend(updates);
                (prefix, presented)
            };
            // The stock client ignores a leaderboard after game over, so the
            // death broadcast omits it; a record may follow.
            let live_board = board
                .as_ref()
                .filter(|_| leaderboard_due && (alive || was_over));
            let hud = self.hud(slot, live_board)?;
            if let Some(presented) = &mut presented {
                presented.extend(hud.iter().cloned());
            }
            packets.extend(hud);
            if let Some(presented) = presented {
                self.emit_presented(slot, now, &presented, Some(&packets))?;
            } else if !packets.is_empty() {
                self.emit(slot, now, &packets)?;
            }
        }
        if early >= self.slots.len() {
            self.hand_over_early(early, hand_over)?;
        }
        #[cfg(feature = "frame-bench")]
        if self.frame_bench && self.tick.is_multiple_of(72) {
            // Wire cells and physical pellets deliberately remain separate.
            // This low-rate census contributes to measured host load.
            let players: Vec<_> = self
                .slots
                .iter()
                .zip(self.world.snakes())
                .take(self.player_latency.slots)
                .enumerate()
                .map(|(id, (slot, snake))| {
                    serde_json::json!({"slot": id, "setup": slot.connection.setup,
                        "alive": snake.alive, "x": snake.head.x, "y": snake.head.y,
                        "protocol": if slot.connection.legacy.is_some() { 13 } else { 19 },
                        "wireLivingSnakes": slot.connection.snakes.frame_bench_len(),
                        "wireFood": slot.connection.food.perf_len(),
                        "decoratedWireFood": slot.connection.decorated_food.perf_len()})
                })
                .collect();
            eprintln!(
                "frame-bench: {}",
                serde_json::json!({"schema": 1,
                "tick": self.tick, "radius": self.world.radius(),
                "totalPellets": self.world.food().len(), "players": players,
                "totalAliveSnakes": self.world.snakes().iter().filter(|s| s.alive).count(),
                "foodTarget": self.world.config().food_count,
                "ambientAttemptsPerSecond": self.world.config().ambient_replenish_per_second,
                "sizeToPopulation": self.world.config().size_to_population,
                "admittedBotSlots": self.frame_bench_admitted_bots(),
                "aliveBots": self.world.snakes().iter().skip(self.player_latency.slots).filter(|s| s.alive).count(),
                "wholeMapFood": self.world.config().pit.is_some()})
            );
        }
        #[cfg(feature = "parallel")]
        if let (Some(views), Some(waves)) = (&mut self.views, waves) {
            views.retain(waves);
        }
        self.coverage_grid = Some(grid);
        self.coverage_bounds = snakes.body_bounds;
        self.food_eaters.clear();
        self.prey_eaters.clear();
        Ok(())
    }

    /// Give every living prey a 16-bit wire ID. Truncating 64-bit simulator IDs
    /// would let a survivor and a much later prey share one (0 and 65536), so
    /// IDs are allocated: a prey keeps its ID while it lives, and a new prey
    /// takes the first free ID from its own truncated value. A retired prey's
    /// ID is free at once. Only set-up connections show prey, and each one gets
    /// `updates` in this broadcast, which sends every removal before any
    /// appearance, so a connection still showing a retired ID sees it removed
    /// before its reuse. All 65,536 IDs can therefore be live.
    fn assign_prey_wire(&mut self) -> Result<(), String> {
        let live: BTreeSet<u64> = self.world.prey().iter().map(|prey| prey.id).collect();
        self.prey_wire.retain(|id, _| live.contains(id));
        let mut taken: Option<BTreeSet<u16>> = None;
        for &id in &live {
            if self.prey_wire.contains_key(&id) {
                continue;
            }
            let taken = taken.get_or_insert_with(|| self.prey_wire.values().copied().collect());
            let wire = (0..=u16::MAX)
                .map(|offset| (id as u16).wrapping_add(offset))
                .find(|wire| !taken.contains(wire))
                .ok_or(format!(
                    "Protocol-19 prey IDs are 16-bit: more than {MAX_PREY} prey are live"
                ))?;
            taken.insert(wire);
            self.prey_wire.insert(id, wire);
        }
        Ok(())
    }

    /// Open each living snake's shared report opportunities for this tick.
    fn update_reports(&mut self) -> Result<(), String> {
        // Adaptive gates count wall time, including time discarded at maxTick.
        // Keep the fixed path exact; see the variable-dt regressions in tests.rs.
        let now = self.time() + self.wall_offset;
        for (slot, snake) in self.world.snakes().iter().enumerate() {
            let reports = &mut self.reports[slot];
            if !snake.alive {
                reports.motion_due = false;
                reports.fam_due = false;
                continue;
            }
            let wanted = physics::quantize_heading(self.slots[slot].control.heading);
            let motion = [
                encode::angle_byte(snake.heading),
                encode::angle_byte(wanted),
                encode::speed_byte(snake.speed)?,
            ];
            let fam = encode::fam_u24(snake.fam);
            if self.variable_dt {
                reports.update_timed(
                    self.slots[slot].life,
                    motion,
                    fam,
                    now,
                    self.config.delivery,
                );
                continue;
            }
            reports.update(
                self.slots[slot].life,
                motion,
                fam,
                self.tick,
                self.config.delivery,
            );
        }
        Ok(())
    }

    /// Advance every living prey's correction clock. Ordinary corrections
    /// recur near one third second; the 210 ms pulse guard is an engineering
    /// choice, not a universal receipt bound. Clean #111 data retains a 3044 ms
    /// repeat gap and fast reports outside exact speed 11; no replacement rearm
    /// law was identified. See docs/server-rules.md and
    /// https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-vps-timing/REPORT.md
    fn update_prey_clocks(&mut self) -> Result<(), String> {
        let now = self.time();
        let mut clocks = BTreeMap::new();
        for prey in self.world.prey() {
            let speed = u32::from(encode::speed_u16(prey.speed)?);
            let mut clock = self
                .prey_clocks
                .get(&prey.id)
                .copied()
                .unwrap_or(PreyClock {
                    reported_at: now,
                    speed,
                    due: false,
                });
            let pulse = clock.speed == 11000 || speed == 11000;
            let interval = if pulse { 0.210 } else { 1. / 3. };
            clock.due = now + 1e-9 >= clock.reported_at + interval;
            if clock.due {
                clock.reported_at = now;
                clock.speed = speed;
            }
            clocks.insert(prey.id, clock);
        }
        self.prey_clocks = clocks;
        Ok(())
    }

    /// A due minimap for a living connection that knows its own snake, as a
    /// delta when shorter. Raster and encodings are shared within this broadcast.
    pub(super) fn minimap(
        &mut self,
        slot: usize,
        map: &mut Option<MinimapBroadcast>,
    ) -> Result<Option<Vec<u8>>, String> {
        let connection = &self.slots[slot].connection;
        if !self.world.snakes()[slot].alive || !connection.snakes.contains_key(slot) {
            return Ok(None);
        }
        if map.is_none() {
            *map = Some(MinimapBroadcast::new(
                self.pit_projection
                    .render_world(&self.world, self.config.minimap_side())?,
            ));
        }
        let map = map.as_mut().expect("minimap was just constructed");
        let packet = map.packet19(connection.minimap_sent.as_ref())?;
        self.slots[slot].connection.minimap_sent = Some(map.map.clone());
        Ok(Some(packet))
    }

    /// Changes since this connection's last frame, own snake first. The view
    /// fields move out of the connection and back, so the slots stay readable
    /// for other snakes' facts; an error keeps the partial view, as before.
    fn updates(
        query: &ViewQuery<'_>,
        slot: usize,
        slots: &mut [Slot],
        reports: &mut [SnakeReports],
        deltas: &mut [SnakeDeltaCache],
        candidates: &mut Vec<usize>,
        diffs: &FoodDiffs,
    ) -> Result<PacketViews, String> {
        let mut view = View::take(&mut slots[slot].connection);
        let shared: &[Slot] = slots;
        let mut effects = Serial {
            slots: shared,
            reports,
            deltas,
            diffs,
        };
        let seat = Seat::of(&shared[slot].connection);
        let result = assemble(query, slot, seat, &mut view, &mut effects, candidates);
        view.put(&mut slots[slot].connection);
        result
    }

    /// Leaderboard when `board` is given, then an unsent record. Both wait for
    /// the connection's own appearance.
    fn hud(&mut self, slot: usize, board: Option<&Board>) -> Result<Vec<Vec<u8>>, String> {
        let mut packets = Vec::new();
        let connection = &mut self.slots[slot].connection;
        if !connection.appeared {
            return Ok(packets);
        }
        if let Some(board) = board {
            let (row, rank) = match board.order.iter().position(|&(_, i)| i == slot) {
                Some(index) if index < LEADERBOARD_ROWS => (index as u8 + 1, index + 1),
                Some(index) => (0, index + 1),
                None if self.world.config().kill_score_seconds.is_some() => (0, 0),
                None => (0, connection.final_rank.unwrap_or(0)),
            };
            packets.push(encode::leaderboard(
                row,
                rank,
                board.order.len(),
                &board.rows,
            ));
        }
        if let Some(record) = self
            .record
            .as_ref()
            .filter(|record| record.version != connection.record_version)
        {
            packets.push(encode::record(
                record.segments,
                record.fam,
                &record.name,
                &record.message,
            )?);
            connection.record_version = record.version;
        }
        Ok(packets)
    }
}

/// Service a server-wide clock of absolute periods `floor(t / interval)`, so
/// phase never accumulates tick rounding: true once period `next` has begun.
/// A period count past `u64` saturates, leaving the clock due every tick.
pub(super) fn clock_due(next: &mut u64, now: f64, interval: f64) -> bool {
    let current = ((now + 1e-9) / interval).floor() as u64;
    let due = current >= *next;
    if due {
        *next = current.saturating_add(1);
    }
    due
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Config,
        client::{SimulationConfig, visibility::SectorInterest},
        model::Point,
    };

    #[test]
    fn immutable_query_is_shareable() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<ViewQuery<'_>>();
        assert_sync::<SnakeQuery<'_>>();
    }

    fn backend() -> SimulatorBackend {
        let mut config = SimulationConfig {
            agents: 2,
            world: Config::sandbox(),
            ..Default::default()
        };
        config.world.radius = 6000.;
        config.world.food_count = 0;
        config.world.ambient_replenish_per_second = 0.;
        config.world.prey.count = 0;
        let mut backend = SimulatorBackend::new(7, config).unwrap();
        backend.advance_ticks(4).unwrap();
        assert!(backend.slots.iter().all(|s| s.connection.setup));
        backend
    }

    fn place(backend: &mut SimulatorBackend, slot: usize, x: f64) {
        let snake = &backend.world.snakes()[slot];
        let shift = x - snake.head.x;
        let body = snake
            .body
            .iter()
            .map(|p| Point {
                x: p.x + shift,
                y: p.y - snake.head.y,
            })
            .collect();
        backend
            .world
            .correct_pose(slot, Point { x, y: 0. }, body, snake.heading)
            .unwrap();
    }

    fn interests(backend: &SimulatorBackend) -> Vec<SectorInterest> {
        backend
            .slots
            .iter()
            .map(|s| s.connection.food_interest.clone())
            .collect()
    }

    fn check_advanced(backend: &SimulatorBackend, mut previous: Vec<SectorInterest>) {
        for ((previous, slot), snake) in previous
            .iter_mut()
            .zip(&backend.slots)
            .zip(backend.world.snakes())
        {
            previous.advance(snake.head, snake.heading, backend.time());
            assert_eq!(
                format!("{:?}", slot.connection.food_interest),
                format!("{previous:?}")
            );
        }
    }

    #[test]
    fn tick_and_standalone_broadcast_keep_subscription_history() {
        let mut backend = backend();
        for x in [1000., -1000., 0., 1000.] {
            place(&mut backend, 0, x);
            let previous = interests(&backend);
            backend.advance().unwrap();
            check_advanced(&backend, previous);
            let time = backend.time();
            place(&mut backend, 0, x + 500.);
            let previous = interests(&backend);
            backend.broadcast().unwrap();
            assert_eq!(backend.time(), time);
            check_advanced(&backend, previous);
        }
    }

    #[test]
    fn interrupted_tick_advances_even_later_recipients_subscriptions() {
        let mut backend = backend();
        place(&mut backend, 0, 1000.);
        place(&mut backend, 1, -1000.);
        backend.slots[0].connection.pongs = 1;
        backend.slots[0]
            .connection
            .downlink
            .resize(super::super::connection::MAX_IN_FLIGHT, (f64::MAX, vec![0]));
        let previous = interests(&backend);
        assert_eq!(backend.advance().unwrap_err(), "Downlink queue is full");
        check_advanced(&backend, previous);
    }
}
