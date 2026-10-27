//! Shared tick scheduling and recipient-ordered broadcasts.
use super::{
    EarlyFrames, MAX_PREY, NATIVE_PROFILE, PlayerAppearance, SimulatorBackend,
    connection::KnownPrey,
    food::{FoodChanges, food_identity, food_updates},
    minimap::MinimapBroadcast,
    prey_visibility::PreyBounds,
    snakes::{CoverageGrid, KnownSnake, SnakeBroadcast, covered, full},
};
use crate::{
    Simulator,
    client::{Minimap, encode},
    model::Snake,
    physics,
    simulator::PickupItem,
};
use std::collections::{BTreeMap, BTreeSet};

/// Rows the stock leaderboard shows.
pub(super) const LEADERBOARD_ROWS: usize = 10;
/// Physical packets, plus a browser view when pit decoration is active.
type PacketViews = (Vec<Vec<u8>>, Option<Vec<Vec<u8>>>);
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
        let end = (self.tick + 1) as f64 * self.world.config().dt;
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
        let actions: Vec<_> = self
            .world
            .snakes()
            .iter()
            .zip(&self.slots)
            .map(|(snake, slot)| snake.alive.then_some(slot.control))
            .collect();
        let radius = self.world.radius();
        self.world
            .step(&actions)
            .map_err(|error| error.to_string())?;
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
        for (slot, snake) in self.slots.iter_mut().zip(self.world.snakes()) {
            if slot.connection.setup {
                slot.connection
                    .food_interest
                    .advance(snake.head, snake.heading, now);
            }
        }
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
            self.food_presentation.sync(self.world.food())?;
            self.food_index
                .rebuild(self.world.food(), &self.food_presentation);
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
            for (slot, snake) in self.slots.iter_mut().zip(self.world.snakes()) {
                if slot.connection.setup {
                    slot.connection
                        .food_interest
                        .advance(snake.head, snake.heading, now);
                }
            }
        }
        let food_changes = self.sync_food_sectors()?;
        self.assign_prey_wire()?;
        self.update_prey_clocks()?;
        let bounds: Vec<[f64; 4]> = self
            .world
            .snakes()
            .iter()
            .map(|snake| {
                snake.body.iter().fold(
                    [snake.head.x, snake.head.y, snake.head.x, snake.head.y],
                    |[x0, y0, x1, y1], p| [x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)],
                )
            })
            .collect();
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
        let mut snakes = SnakeBroadcast::new(self.slots.len());
        let mut board = None;
        let mut map = None;
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
            let mut packets = vec![encode::pong(); pongs];
            let (updates, presented_updates) = self.updates(
                slot,
                &food_changes,
                &bounds,
                &grid,
                &prey_bounds,
                &mut snakes,
            )?;
            if minimap_due && let Some(packet) = self.minimap(slot, &mut map)? {
                packets.push(packet);
            }
            let mut presented = presented_updates.map(|updates| {
                let mut presented = packets.clone();
                presented.extend(updates);
                presented
            });
            packets.extend(updates);
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
        self.coverage_grid = Some(grid);
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
            *map = Some(MinimapBroadcast::new(Minimap::from_world(
                &self.world,
                self.config.minimap_side(),
            )?));
        }
        let map = map.as_mut().expect("minimap was just constructed");
        let packet = map.packet19(connection.minimap_sent.as_ref())?;
        self.slots[slot].connection.minimap_sent = Some(map.map.clone());
        Ok(Some(packet))
    }

    /// Changes since this connection's last frame, own snake first.
    fn updates(
        &mut self,
        slot: usize,
        food_changes: &FoodChanges,
        bounds: &[[f64; 4]],
        grid: &CoverageGrid,
        prey_bounds: &PreyBounds,
        snakes: &mut SnakeBroadcast,
    ) -> Result<PacketViews, String> {
        let world = &self.world;
        let me = &world.snakes()[slot];
        let record = self
            .record
            .as_ref()
            .is_some_and(|record| record.slot == slot && record.life == self.slots[slot].life);
        let mut packets = Vec::new();
        let connection = &mut self.slots[slot].connection;
        if !me.alive && !connection.over {
            // Game over alone leaves the stock client predicting its snake: send
            // the final mass, then game over, then the death removal.
            connection.over = true;
            let id = slot as u16;
            let known = connection.snakes.remove(slot);
            let fam = encode::fam_u24(me.fam);
            if known.as_ref().is_some_and(|known| known.fam != fam) {
                packets.push(encode::fam(id, fam));
            }
            // The stock game-over score adds the credit to the final mass.
            let credit = encode::segment_credit(me.score, me.segments, fam);
            if connection.credit != credit {
                connection.credit = credit;
                packets.push(encode::credit(credit));
            }
            // The local record rule qualifies this life; the stock v1 flag
            // offers its player a victory-message input.
            packets.push(encode::game_over(record));
            if known.is_some() {
                packets.push(encode::removal(id, true));
            }
        }
        let radius = world.radius().round() as u32;
        if connection.radius != radius {
            connection.radius = radius;
            packets.push(encode::radius(world.radius())?);
        }
        let rules = world.rules();
        if connection.rules_enabled && connection.rules_sent != Some(rules) {
            packets.push(crate::client::rules_packet::encode(rules));
            connection.rules_sent = Some(rules);
        }
        let vision = world.config().vision;
        let head = me.head;
        // Snakes that may cover the head, or that this connection shows and may
        // need removing; every other snake is neither shown nor eligible.
        let known = &self.slots[slot].connection.snakes;
        let SnakeBroadcast { deltas, candidates } = snakes;
        known.candidates(grid.near(head), candidates);
        let order = std::iter::once(slot).chain(candidates.iter().copied().filter(|&i| i != slot));
        for i in order {
            let snake = &world.snakes()[i];
            let life = self.slots[i].life;
            let control = self.slots[i].control.heading;
            let eligible = snake.alive && (i == slot || covered(head, snake, bounds[i], vision));
            let own = i == slot;
            let id = i as u16;
            let profile = self.profiles.get(i).unwrap_or(&NATIVE_PROFILE);
            let connection = &mut self.slots[slot].connection;
            let known = connection.snakes.get_mut(i);
            match (known, eligible) {
                (Some(known), true) if known.life == life => {
                    let reports = &self.reports[i];
                    deltas[i].changes(
                        &mut packets,
                        own,
                        id,
                        known,
                        snake,
                        || physics::quantize_heading(control),
                        profile,
                        reports,
                    )?;
                    self.reports[i].observe(known);
                }
                (Some(known), false) => {
                    let died = !snake.alive || known.life != life;
                    packets.push(encode::removal(id, died));
                    connection.snakes.remove(i);
                }
                (None, false) => {}
                (known, true) => {
                    let wanted = physics::quantize_heading(control);
                    if known.is_some() {
                        packets.push(encode::removal(id, true));
                    }
                    full(&mut packets, own, id, snake, wanted, profile)?;
                    let known = KnownSnake::new(life, snake, wanted)?;
                    self.reports[i].observe(&known);
                    connection.snakes.insert(i, known);
                }
            }
            let connection = &mut self.slots[slot].connection;
            if own && connection.snakes.contains_key(slot) {
                // HUD packets wait for the client's own appearance; a
                // requested session ID follows it so the client knows itself.
                connection.appeared = true;
                let fam = encode::fam_u24(snake.fam);
                let credit = encode::segment_credit(snake.score, snake.segments, fam);
                if connection.credit != credit {
                    connection.credit = credit;
                    packets.push(encode::credit(credit));
                }
                if std::mem::take(&mut connection.session_pending) {
                    packets.push(encode::session(id, connection.session));
                }
            }
        }
        let connection = &mut self.slots[slot].connection;
        let food_start = packets.len();
        food_updates(
            &mut packets,
            &mut connection.food,
            &connection.food_interest,
            &self.food_sectors,
            food_changes,
            &self.food_eaters,
        );
        let food_end = packets.len();
        let mut presented = if connection.decorations && self.pit_markers.is_some() {
            let mut presented = packets[..food_start].to_vec();
            food_updates(
                &mut presented,
                &mut connection.decorated_food,
                &connection.food_interest,
                &self.decorated_food_sectors,
                food_changes,
                &self.food_eaters,
            );
            Some(presented)
        } else {
            None
        };
        let mut visible = BTreeMap::new();
        for index in prey_bounds.near(head) {
            let prey = &world.prey()[index];
            if prey.position.distance(head) <= vision {
                visible.entry(prey.id).or_insert(prey);
            }
        }
        let gone: Vec<u64> = connection
            .prey
            .keys()
            .filter(|id| !visible.contains_key(id))
            .copied()
            .collect();
        for id in gone {
            let known = connection.prey.remove(&id).expect("known prey");
            packets.push(encode::prey_removal(
                known.wire,
                self.prey_eaters.get(&id).copied(),
            ));
        }
        for (id, prey) in visible {
            let wire = self.prey_wire[&id];
            let current = KnownPrey {
                wire,
                motion: [
                    encode::angle_u24(prey.heading),
                    encode::angle_u24(prey.target_heading),
                    u32::from(encode::speed_u16(prey.speed)?),
                ],
            };
            match connection.prey.get(&id) {
                None => packets.push(encode::prey_appearance(wire, prey)?),
                // Keep corrections even when quantized position is still.
                Some(known) if self.prey_clocks[&id].due => {
                    let changed = known.motion != current.motion;
                    packets.push(encode::prey_motion(wire, prey, changed)?)
                }
                Some(_) => continue,
            }
            connection.prey.insert(id, current);
        }
        if let Some(presented) = &mut presented {
            presented.extend_from_slice(&packets[food_end..]);
        }
        Ok((packets, presented))
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
