//! Raw remote slots sharing the native backend's world and scheduler.
use super::{
    EarlyFrames, MAX_NICKNAME, MAX_VICTORY_TEXT, PlayerAppearance, PlayerLatency, REMOTE_IN_FLIGHT,
    SimulationConfig, SimulatorBackend, client_message,
    connection::{Connection, Uplink},
};
use crate::Action;
use crate::client::LatencyConfig;

/// One shared world whose leading slots are external protocol-19 clients.
///
/// Remote slots exchange exact WebSocket payloads through the same server-side
/// scheduling, coverage and encoders as native slots. This process never
/// decodes their frames, pings for them or models their processing: a remote
/// client's compute time has already elapsed in real time. Configured latency
/// still delays both directions in order. The host drives remaining slots as
/// background seekers that reconnect after death; a remote slot stays dead until
/// the host connects it again.
pub(crate) struct RawSimulator {
    pub(super) backend: SimulatorBackend,
}

impl RawSimulator {
    /// Called by the host before any slots connect; bot slots keep config.latency.
    pub(crate) fn set_player_latency(&mut self, slots: usize, config: LatencyConfig) {
        self.backend.player_latency = PlayerLatency { slots, config };
    }

    pub(crate) fn new(
        seed: u32,
        config: SimulationConfig,
        remote_slots: usize,
    ) -> Result<Self, String> {
        if !(1..=config.agents).contains(&remote_slots) {
            return Err("Remote slots must be 1 through the agent count".into());
        }
        if config.record_wire {
            return Err("Raw simulation does not record wire messages".into());
        }
        Ok(Self {
            backend: SimulatorBackend::build(seed, config, remote_slots)?,
        })
    }

    pub(crate) fn time(&self) -> f64 {
        self.backend.time()
    }

    /// Authoritative fixture edits for host admission regressions.
    #[cfg(test)]
    pub(crate) fn fixture_world(&mut self) -> &mut crate::Simulator {
        &mut self.backend.world
    }

    pub(crate) fn dt(&self) -> f64 {
        self.backend.world.config().dt
    }

    pub(crate) fn wall_offset(&self) -> f64 {
        self.backend.wall_offset
    }

    pub(crate) fn set_wall_offset(&mut self, offset: f64) {
        self.shift_links(offset - self.backend.wall_offset);
        self.backend.wall_offset = offset;
    }

    pub(crate) fn set_dt(&mut self, dt: f64) -> Result<(), String> {
        self.backend.world.set_dt(dt).map_err(|e| e.to_string())?;
        self.backend.variable_dt = true;
        Ok(())
    }
    pub(crate) fn waits_for_spawn(&self) -> bool {
        self.backend.world.config().spawn_radius.is_some()
    }

    fn remote(&self, slot: usize) -> Result<(), String> {
        if slot < self.backend.remote {
            Ok(())
        } else {
            Err("Unknown remote slot".into())
        }
    }
    fn open(&self, slot: usize) -> Result<(), String> {
        self.remote(slot)?;
        if self.backend.slots[slot].connection.open {
            Ok(())
        } else {
            Err("Remote slot is not connected".into())
        }
    }

    pub(crate) fn grow_for_waiters(&mut self, waiting: usize, permitted: bool) -> bool {
        let seats = self.backend.world.occupied() + waiting;
        self.backend.world.grow_for_seats(seats, permitted)
    }

    /// Start a new session and life in `slot`, replacing any existing one and
    /// its in-flight messages. The server answers with setup once the connect
    /// arrives; the first broadcast after it begins with the slot's own snake.
    /// False leaves the session untouched while circular admission waits.
    #[cfg(test)]
    pub(crate) fn connect(
        &mut self,
        slot: usize,
        appearance: PlayerAppearance,
    ) -> Result<bool, String> {
        self.connect_waited(slot, appearance, 0.)
    }

    pub(crate) fn connect_waited(
        &mut self,
        slot: usize,
        appearance: PlayerAppearance,
        waited: f64,
    ) -> Result<bool, String> {
        self.remote(slot)?;
        if appearance.nickname.len() > MAX_NICKNAME || appearance.custom_skin.len() > 255 {
            return Err("Nickname or custom skin is too long".into());
        }
        let b = &mut self.backend;
        let (body, heading) = match b.spawn_pose_waited(slot, waited) {
            Ok(pose) => pose,
            Err(reason) if reason == crate::spawn::WAITING => return Ok(false),
            Err(reason) => return Err(reason),
        };
        b.slots[slot].living = None;
        b.world.retire(slot).map_err(|error| error.to_string())?;
        b.world
            .respawn_body(slot, body, heading)
            .map_err(|error| error.to_string())?;
        b.profiles[slot] = appearance;
        let s = &mut b.slots[slot];
        s.life += 1;
        s.control = Action {
            heading,
            boost: false,
        };
        b.connect(slot)?;
        b.slots[slot].connection.decorations = true;
        Ok(true)
    }

    /// A different socket player owns this seat. Clear the departed player's
    /// history only once admission succeeds; retries leave the board untouched.
    #[cfg(test)]
    pub(crate) fn connect_new_player(
        &mut self,
        slot: usize,
        appearance: PlayerAppearance,
    ) -> Result<bool, String> {
        self.connect_new_player_waited(slot, appearance, 0.)
    }

    pub(crate) fn connect_new_player_waited(
        &mut self,
        slot: usize,
        appearance: PlayerAppearance,
        waited: f64,
    ) -> Result<bool, String> {
        let admitted = self.connect_waited(slot, appearance, waited)?;
        if admitted {
            self.backend.world.reset_kill_score(slot);
        }
        Ok(admitted)
    }

    pub(crate) fn physical_food(&mut self, slot: usize) {
        self.backend.slots[slot].connection.decorations = false;
        self.backend.slots[slot].connection.rules_enabled = true;
    }

    /// Called after admission and before the queued connect emits setup. A
    /// reconnect gets a new adapter, including fresh anchor and minimap bases.
    pub(crate) fn protocol13(&mut self, slot: usize) {
        self.backend.slots[slot].connection.legacy =
            Some(Box::new(crate::client::encode13::Encoder::new(slot as u16)));
    }

    pub(crate) fn full_minimap(&mut self, slot: usize) {
        self.backend.slots[slot]
            .connection
            .legacy
            .as_mut()
            .expect("full minimap is only requested by protocol 13")
            .use_full_minimap();
    }

    /// Queue one client message sent now on the uplink; see `receive_at`.
    #[cfg(test)]
    pub(super) fn receive(&mut self, slot: usize, bytes: &[u8]) -> Result<(), String> {
        self.receive_at(slot, bytes, self.time())
    }

    /// Queue one client message the client sent at world time `sent`, which may
    /// fall between or after ticks. It arrives after the uplink delay and takes
    /// effect at the first tick from then. A rejected message changes nothing.
    pub(crate) fn receive_at(
        &mut self,
        slot: usize,
        bytes: &[u8],
        sent: f64,
    ) -> Result<(), String> {
        self.open(slot)?;
        if !client_message(bytes) {
            return Err("Unsupported client message".into());
        }
        self.uplink_at(slot, sent, Uplink::Message(bytes.to_vec()))
    }

    fn uplink_at(&mut self, slot: usize, sent: f64, message: Uplink) -> Result<(), String> {
        if !sent.is_finite() {
            return Err("Send time must be finite".into());
        }
        let connection = &self.backend.slots[slot].connection;
        if connection.uplink.len() >= REMOTE_IN_FLIGHT {
            return Err("Remote connection has too many undelivered messages".into());
        }
        self.backend.uplink(slot, sent, message)
    }

    /// Every queued server payload with its delivery time in world seconds, in
    /// order, including deliveries still in flight. A real-time host sends each
    /// at its own time instead of waiting for the tick that follows it. A closed
    /// slot has none.
    pub(crate) fn take(&mut self, slot: usize) -> Result<Vec<(f64, Vec<u8>)>, String> {
        self.remote(slot)?;
        let connection = &mut self.backend.slots[slot].connection;
        if let Some(reason) = &connection.output_fault {
            return Err(reason.clone());
        }
        Ok(connection.take_downlink())
    }

    /// Server payloads delivered by now, in order. Each is one WebSocket message.
    #[cfg(test)]
    pub(super) fn drain(&mut self, slot: usize) -> Result<Vec<Vec<u8>>, String> {
        self.open(slot)?;
        let now = self.time();
        let connection = &mut self.backend.slots[slot].connection;
        let downlink = &mut connection.downlink;
        let mut delivered = Vec::new();
        while downlink.front().is_some_and(|(at, _)| *at <= now) {
            let bytes = downlink.pop_front().unwrap().1;
            connection.downlink_bytes -= super::output_cost(&bytes);
            delivered.push(bytes);
        }
        Ok(delivered)
    }

    /// Close the session and remove its living snake without death food.
    /// Observers see a removal at their next broadcast. Closing a closed slot
    /// does nothing.
    pub(crate) fn disconnect(&mut self, slot: usize) -> Result<(), String> {
        self.remote(slot)?;
        let b = &mut self.backend;
        b.slots[slot].living = None;
        b.world.retire(slot).map_err(|error| error.to_string())?;
        b.slots[slot].connection = Connection::default();
        Ok(())
    }

    /// Answer the client's optional pre-login session-ID request `[4]` once:
    /// `S` follows its own appearance on this connection's ordered downlink.
    /// The ID is opaque and does not authenticate or resume anything.
    pub(crate) fn request_session_id(&mut self, slot: usize) -> Result<(), String> {
        self.open(slot)?;
        let connection = &mut self.backend.slots[slot].connection;
        if !connection.session_requested {
            connection.session_requested = true;
            connection.session_pending = true;
        }
        Ok(())
    }

    /// Queue a playing client's `[255, 118, text]` victory text. Only the
    /// connection whose dead life holds the session record can set it, once;
    /// other text is ignored when it arrives.
    #[cfg(test)]
    pub(super) fn victory(&mut self, slot: usize, text: &[u8]) -> Result<(), String> {
        self.victory_at(slot, text, self.time())
    }

    /// `victory` for text the client sent at world time `sent`.
    pub(crate) fn victory_at(&mut self, slot: usize, text: &[u8], sent: f64) -> Result<(), String> {
        self.open(slot)?;
        if text.len() > MAX_VICTORY_TEXT || !text.is_ascii() {
            return Err("Victory text must be at most 140 ASCII bytes".into());
        }
        self.uplink_at(slot, sent, Uplink::Victory(text.to_vec()))
    }

    /// Move every remote slot's link times `seconds` earlier in world time. A
    /// host that defers ticks maps the same wall time to an earlier world time;
    /// messages in flight and each stream's clocks keep their wall times.
    pub(crate) fn shift_links(&mut self, seconds: f64) {
        let remote = self.backend.remote;
        for slot in &mut self.backend.slots[..remote] {
            let connection = &mut slot.connection;
            for (at, _) in &mut connection.uplink {
                *at -= seconds;
            }
            for (at, _) in &mut connection.downlink {
                *at -= seconds;
            }
            connection.link.shift(seconds);
            connection.presented_at -= seconds;
        }
    }

    /// One shared tick, like `advance_ticks(1)`, except that the frames of
    /// remote slots `..early` go to `hand_over` as soon as they are encoded,
    /// so those connections do not wait for the rest of the broadcast. Frames
    /// queued outside the broadcast stay for `take`.
    pub(crate) fn advance_tick_with(
        &mut self,
        early: usize,
        hand_over: &mut EarlyFrames,
    ) -> Result<(), String> {
        if early > self.backend.remote {
            return Err("Early slots must be remote".into());
        }
        let b = &mut self.backend;
        if b.tick.is_multiple_of(b.config().decision_ticks as u64) {
            b.drive_background()?;
        }
        b.advance_with(early, hand_over)
    }

    /// Shared server ticks; background slots decide at the decision cadence.
    #[cfg(test)]
    pub(super) fn advance_ticks(&mut self, ticks: usize) -> Result<(), String> {
        for _ in 0..ticks {
            self.advance_tick_with(0, &mut |_| Ok(()))?;
        }
        Ok(())
    }
}
