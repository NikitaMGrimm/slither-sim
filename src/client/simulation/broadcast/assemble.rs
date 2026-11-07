//! One recipient's update, shared by the serial loop and view workers.
//!
//! The serial path moves the connection's view fields into a [`View`],
//! assembles, and moves them back, reading other snakes' life and control
//! from the slots. A view worker assembles into a copy and records its cache
//! writes and stale-report observations for the coordinator to commit. Either
//! way the same code decides every packet, in the same order.
use super::{PacketViews, SnakeQuery, ViewQuery};
use crate::client::{
    encode,
    simulation::{
        NATIVE_PROFILE, PlayerAppearance,
        connection::{Connection, KnownPrey},
        food::{FoodQuery, SectorFood},
        snakes::{KnownSnake, KnownSnakes, covered, full},
    },
    visibility::SectorInterest,
};
use crate::{WorldRules, model::Snake, physics};
use std::{collections::BTreeMap, sync::Arc};

/// The fields one recipient's update changes. `F` holds its food listings:
/// owned on the serial path, a borrowed base plus an edit on view workers.
pub(super) struct View<F> {
    pub(super) over: bool,
    pub(super) snakes: KnownSnakes,
    pub(super) credit: u8,
    pub(super) radius: u32,
    pub(super) rules_sent: Option<WorldRules>,
    pub(super) appeared: bool,
    pub(super) session_pending: bool,
    pub(super) prey: BTreeMap<u64, KnownPrey>,
    pub(super) food: F,
    pub(super) decorated_food: F,
}

/// Connection settings an update reads but never changes.
#[derive(Clone, Copy)]
pub(super) struct Seat<'a> {
    pub(super) rules_enabled: bool,
    pub(super) session: u32,
    pub(super) decorations: bool,
    pub(super) interest: &'a SectorInterest,
}

impl<'a> Seat<'a> {
    pub(super) fn of(connection: &'a Connection) -> Self {
        Self {
            rules_enabled: connection.rules_enabled,
            session: connection.session,
            decorations: connection.decorations,
            interest: &connection.food_interest,
        }
    }
}

impl View<crate::client::simulation::food::KnownSectors> {
    /// Move the view out; no allocation or reference count changes.
    pub(super) fn take(connection: &mut Connection) -> Self {
        Self {
            over: connection.over,
            snakes: std::mem::take(&mut connection.snakes),
            credit: connection.credit,
            radius: connection.radius,
            rules_sent: connection.rules_sent,
            appeared: connection.appeared,
            session_pending: connection.session_pending,
            prey: std::mem::take(&mut connection.prey),
            food: std::mem::take(&mut connection.food),
            decorated_food: std::mem::take(&mut connection.decorated_food),
        }
    }

    pub(super) fn put(self, connection: &mut Connection) {
        connection.over = self.over;
        connection.snakes = self.snakes;
        connection.credit = self.credit;
        connection.radius = self.radius;
        connection.rules_sent = self.rules_sent;
        connection.appeared = self.appeared;
        connection.session_pending = self.session_pending;
        connection.prey = self.prey;
        connection.food = self.food;
        connection.decorated_food = self.decorated_food;
    }
}

/// Where an update reads other snakes' facts and writes its caches.
pub(super) trait Effects {
    type Food;
    fn life(&self, slot: usize) -> u64;
    fn heading(&self, slot: usize) -> f64;
    /// Delta packets for a known snake whose life still matches.
    #[expect(
        clippy::too_many_arguments,
        reason = "Same inputs as SnakeDeltaCache::changes"
    )]
    fn changes(
        &mut self,
        slot: usize,
        packets: &mut Vec<Vec<u8>>,
        own: bool,
        known: &mut KnownSnake,
        snake: &Snake,
        wanted: impl FnOnce() -> f64,
        profile: &PlayerAppearance,
    ) -> Result<(), String>;
    /// Note this recipient's view of a snake for its stale-report flags.
    fn observe(&mut self, slot: usize, known: &KnownSnake);
    fn food(
        &mut self,
        packets: &mut Vec<Vec<u8>>,
        known: &mut Self::Food,
        interest: &SectorInterest,
        sectors: &[Option<Arc<SectorFood>>],
        changes: FoodQuery<'_>,
        eaters: &BTreeMap<(u64, u64), u16>,
    );
}

/// Changes since this connection's last frame, own snake first.
pub(super) fn assemble<E: Effects>(
    query: &ViewQuery<'_>,
    slot: usize,
    seat: Seat<'_>,
    view: &mut View<E::Food>,
    effects: &mut E,
    candidates: &mut Vec<usize>,
) -> Result<PacketViews, String> {
    let me = &query.snakes[slot];
    let record = query.record_life == Some((slot, effects.life(slot)));
    let mut packets = Vec::new();
    if !me.alive && !view.over {
        // Game over alone leaves the stock client predicting its snake: send
        // the final mass, then game over, then the death removal.
        view.over = true;
        let id = slot as u16;
        let known = view.snakes.remove(slot);
        let fam = encode::fam_u24(me.fam);
        if known.as_ref().is_some_and(|known| known.fam != fam) {
            packets.push(encode::fam(id, fam));
        }
        // The stock game-over score adds the credit to the final mass.
        let credit = encode::segment_credit(me.score, me.segments, fam);
        if view.credit != credit {
            view.credit = credit;
            packets.push(encode::credit(credit));
        }
        // The local record rule qualifies this life; the stock v1 flag
        // offers its player a victory-message input.
        packets.push(encode::game_over(record));
        if known.is_some() {
            packets.push(encode::removal(id, true));
        }
    }
    let radius = query.radius.round() as u32;
    if view.radius != radius {
        view.radius = radius;
        packets.push(encode::radius(query.radius)?);
    }
    let rules = query.rules;
    if seat.rules_enabled && view.rules_sent != Some(rules) {
        packets.push(crate::client::rules_packet::encode(rules));
        view.rules_sent = Some(rules);
    }
    let vision = query.vision;
    let head = me.head;
    // Snakes that may cover the head, or that this connection shows and may
    // need removing; every other snake is neither shown nor eligible.
    view.snakes.candidates(query.grid.near(head), candidates);
    let order = std::iter::once(slot).chain(candidates.iter().copied().filter(|&i| i != slot));
    for i in order {
        // Compact life/control facts come from the effects' owner; no
        // world-sized snapshot or scan is added to the serial path.
        let SnakeQuery {
            snake,
            life,
            heading: control,
            profile,
        } = SnakeQuery {
            snake: &query.snakes[i],
            life: effects.life(i),
            heading: effects.heading(i),
            profile: query.profiles.get(i).unwrap_or(&NATIVE_PROFILE),
        };
        let eligible = snake.alive
            && (i == slot
                || covered(
                    head,
                    snake,
                    query.bounds[i],
                    vision,
                    query.body_bounds.for_snake(i),
                ));
        let own = i == slot;
        let id = i as u16;
        let known = view.snakes.get_mut(i);
        match (known, eligible) {
            (Some(known), true) if known.life == life => {
                effects.changes(
                    i,
                    &mut packets,
                    own,
                    known,
                    snake,
                    || physics::quantize_heading(control),
                    profile,
                )?;
                effects.observe(i, known);
            }
            (Some(known), false) => {
                let died = !snake.alive || known.life != life;
                packets.push(encode::removal(id, died));
                view.snakes.remove(i);
            }
            (None, false) => {}
            (known, true) => {
                let wanted = physics::quantize_heading(control);
                if known.is_some() {
                    packets.push(encode::removal(id, true));
                }
                full(&mut packets, own, id, snake, wanted, profile)?;
                let known = KnownSnake::new(life, snake, wanted)?;
                effects.observe(i, &known);
                view.snakes.insert(i, known);
            }
        }
        if own && view.snakes.contains_key(slot) {
            // HUD packets wait for the client's own appearance; a
            // requested session ID follows it so the client knows itself.
            view.appeared = true;
            let fam = encode::fam_u24(snake.fam);
            let credit = encode::segment_credit(snake.score, snake.segments, fam);
            if view.credit != credit {
                view.credit = credit;
                packets.push(encode::credit(credit));
            }
            if std::mem::take(&mut view.session_pending) {
                packets.push(encode::session(id, seat.session));
            }
        }
    }
    let food_start = packets.len();
    effects.food(
        &mut packets,
        &mut view.food,
        seat.interest,
        query.food,
        query.food_changes,
        query.food_eaters,
    );
    let food_end = packets.len();
    let mut presented = if seat.decorations && query.decorations {
        let mut presented = packets[..food_start].to_vec();
        effects.food(
            &mut presented,
            &mut view.decorated_food,
            seat.interest,
            query.decorated_food,
            query.food_changes,
            query.food_eaters,
        );
        Some(presented)
    } else {
        None
    };
    let mut visible = BTreeMap::new();
    for index in query.prey_bounds.near(head) {
        let prey = &query.prey[index];
        if prey.position.distance_le(head, vision) {
            visible.entry(prey.id).or_insert(prey);
        }
    }
    let gone: Vec<u64> = view
        .prey
        .keys()
        .filter(|id| !visible.contains_key(id))
        .copied()
        .collect();
    for id in gone {
        let known = view.prey.remove(&id).expect("known prey");
        packets.push(encode::prey_removal(
            known.wire,
            query.prey_eaters.get(&id).copied(),
        ));
    }
    for (id, prey) in visible {
        let wire = query.prey_wire[&id];
        let current = KnownPrey {
            wire,
            motion: [
                encode::angle_u24(prey.heading),
                encode::angle_u24(prey.target_heading),
                u32::from(encode::speed_u16(prey.speed)?),
            ],
        };
        match view.prey.get(&id) {
            None => packets.push(encode::prey_appearance(wire, prey)?),
            // Keep corrections even when quantized position is still.
            Some(known) if query.prey_clocks[&id].due => {
                let changed = known.motion != current.motion;
                packets.push(encode::prey_motion(wire, prey, changed)?)
            }
            Some(_) => continue,
        }
        view.prey.insert(id, current);
    }
    if let Some(presented) = &mut presented {
        presented.extend_from_slice(&packets[food_end..]);
    }
    Ok((packets, presented))
}
