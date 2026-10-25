//! Ordered links, client decisions and wire recording for each slot.
use super::{NATIVE_PROFILE, SimulatorBackend, food::KnownSectors, snakes::KnownSnakes};
use crate::{
    Action,
    client::{
        ClientState, Minimap, WireEvent, WireRecord, encode, latency::Link, protocol,
        visibility::SectorInterest,
    },
    physics,
};
use std::{
    collections::{BTreeMap, VecDeque},
    f64::consts::TAU,
};

/// Messages in flight per direction of one connection before the run fails.
/// A local resource budget; the protocol has no such field.
pub(super) const MAX_IN_FLIGHT: usize = 4096;
/// Fit the host's 1 MiB send buffer and native client's message limit, leaving
/// room for the largest unmasked WebSocket header.
const MAX_FRAME_BYTES: usize = (1 << 20) - 10;
/// Retained wire bytes per slot before the run fails; drain to continue. Each
/// record also counts a fixed overhead so empty events stay bounded. A local
/// resource budget.
pub(super) const MAX_RECORD_BYTES: usize = 32 << 20;
pub(super) const RECORD_OVERHEAD: usize = 32;
#[derive(Clone, Debug)]
pub(super) struct KnownPrey {
    pub(super) wire: u16,
    pub(super) motion: [u32; 3],
}

#[derive(Clone, Debug)]
pub(super) enum Uplink {
    Connect,
    Message(Vec<u8>),
    /// Victory text for the record life; still read after game over.
    Victory(Vec<u8>),
}

/// Server and transport state of one connection.
#[derive(Clone, Debug, Default)]
pub(super) struct Connection {
    /// Opened by a connect; a closed remote slot has no session.
    pub(super) open: bool,
    pub(super) link: Link,
    pub(super) uplink: VecDeque<(f64, Uplink)>,
    pub(super) downlink: VecDeque<(f64, Vec<u8>)>,
    /// The server has received the connect and sent setup.
    pub(super) setup: bool,
    /// Game over was sent; afterwards only pings and victory text are accepted.
    pub(super) over: bool,
    /// Setup emission time and milliseconds of elapsed headers sent since then.
    /// Headers difference quantized absolute times, so rounding never drifts.
    pub(super) setup_at: Option<f64>,
    pub(super) header_ms: u64,
    pub(super) snakes: KnownSnakes,
    pub(super) food: KnownSectors,
    pub(super) decorated_food: KnownSectors,
    /// Stock sockets get decoration; bot clients retain physical food at every key.
    pub(super) decorations: bool,
    /// Last presented delivery, including marker-only frames. The physical
    /// latency stream must not advance for decorations, but output stays ordered.
    pub(super) presented_at: f64,
    pub(super) food_interest: SectorInterest,
    /// Keyed by simulator ID; each entry keeps the wire ID it was shown with.
    pub(super) prey: BTreeMap<u64, KnownPrey>,
    pub(super) radius: u32,
    /// XOR bases belong to recipients: missed maps and reconnects differ.
    pub(super) minimap_sent: Option<Minimap>,
    /// The connection's own snake has appeared; HUD packets wait for it.
    pub(super) appeared: bool,
    /// Pongs owed after setup; they lead the next broadcast.
    pub(super) pongs: usize,
    /// Rank frozen when the connection's snake died.
    pub(super) final_rank: Option<usize>,
    /// Version of the session record last sent; zero before any.
    pub(super) record_version: u64,
    /// Score credit last sent for the connection's own snake.
    pub(super) credit: u8,
    /// Opaque per-connection session ID, unique across slot reuse.
    pub(super) session: u32,
    /// The client asked for its session ID; `session_pending` until it is sent.
    pub(super) session_requested: bool,
    pub(super) session_pending: bool,
}

pub(super) struct Slot {
    pub(super) state: ClientState,
    pub(super) connection: Connection,
    pub(super) life: u64,
    /// Command decided at `.0`, leaving the client after `.2` seconds of processing.
    pub(super) outbox: Option<(f64, Action, f64)>,
    /// Measured processing of the decision made at `.0`. It replaces the
    /// configured draw for an action sent at that same time; a decision without
    /// an action keeps the slot busy until `.0 + .1`.
    pub(super) processing_override: Option<(f64, f64)>,
    pub(super) previous_boost: Option<bool>,
    /// Command in force on the server.
    pub(super) control: Action,
    pub(super) pinged_at: Option<f64>,
    pub(super) records: Option<Vec<WireRecord>>,
    pub(super) record_bytes: usize,
    /// Heading byte last requested by the built-in background seeker.
    pub(super) seeker: Option<u8>,
    /// An external client exchanges this slot's raw payloads; the in-process
    /// decoder, pings, records and processing model stay idle.
    pub(super) remote: bool,
    /// Life seen alive at the last tick; its later absence is a completed life
    /// unless a disconnect or respawn cleared it first.
    pub(super) living: Option<u64>,
}
impl Slot {
    /// Fail before events are applied that their records could not retain.
    pub(super) fn room(&self, sizes: impl IntoIterator<Item = usize>) -> Result<(), String> {
        let needed: usize = sizes.into_iter().map(|size| size + RECORD_OVERHEAD).sum();
        if self.records.is_some() && self.record_bytes + needed > MAX_RECORD_BYTES {
            return Err("Wire record buffer is full; drain records first".into());
        }
        Ok(())
    }
    /// Fail before sending messages the uplink queue could not hold.
    pub(super) fn uplink_room(&self, messages: usize) -> Result<(), String> {
        if self.connection.uplink.len() + messages > MAX_IN_FLIGHT {
            return Err("Uplink queue is full".into());
        }
        Ok(())
    }
    pub(super) fn record(
        &mut self,
        time: f64,
        event: WireEvent,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.room([bytes.len()])?;
        if let Some(records) = &mut self.records {
            self.record_bytes += bytes.len() + RECORD_OVERHEAD;
            records.push(WireRecord {
                time,
                event,
                bytes: bytes.to_vec(),
            });
        }
        Ok(())
    }
}

impl SimulatorBackend {
    pub(super) fn connect(&mut self, slot: usize) -> Result<(), String> {
        self.slots[slot].room([0])?;
        let now = self.time();
        let appearance = self.profiles.get(slot).unwrap_or(&NATIVE_PROFILE);
        self.food_presentation
            .register(slot, appearance.skin, &appearance.custom_skin);
        let latency = &self.config.latency;
        self.connections = self.connections.wrapping_add(1);
        let mut connection = Connection {
            open: true,
            session: session_id(self.connections, self.session_key),
            link: Link::connect(latency, &mut self.random),
            ..Default::default()
        };
        let arrival = connection.link.uplink(latency, &mut self.random, now);
        connection.uplink.push_back((arrival, Uplink::Connect));
        let s = &mut self.slots[slot];
        s.connection = connection;
        if s.remote {
            return Ok(());
        }
        s.state.new_connection(now)?;
        s.record(now, WireEvent::Connect, &[])
    }

    /// Client side up to `now`: deliveries and departures in time order, then a
    /// ping if the client's own cadence asks for one.
    pub(super) fn client_until(&mut self, slot: usize, now: f64) -> Result<(), String> {
        // A remote client drains its own frames and sends its own pings.
        if self.slots[slot].remote {
            return Ok(());
        }
        loop {
            let s = &self.slots[slot];
            let frame = s
                .connection
                .downlink
                .front()
                .map(|(at, _)| *at)
                .filter(|at| *at <= now);
            let leave = s
                .outbox
                .map(|(at, _, processing)| at + processing)
                .filter(|at| *at <= now);
            match (frame, leave) {
                (Some(at), leave) if leave.is_none_or(|leave| at <= leave) => {
                    let s = &mut self.slots[slot];
                    // A full record buffer leaves the frame queued, undelivered.
                    s.room([s.connection.downlink[0].1.len()])?;
                    let (_, bytes) = s.connection.downlink.pop_front().unwrap();
                    s.state.receive_frame(&bytes, at)?;
                    s.record(at, WireEvent::Receive, &bytes)?;
                }
                (_, Some(at)) => self.transmit(slot, at)?,
                _ => break,
            }
        }
        let s = &mut self.slots[slot];
        if s.pinged_at.is_none_or(|at| at < now) {
            s.pinged_at = Some(now);
            if s.state.ping_due(now)? {
                s.room([1])?;
                s.uplink_room(1)?;
                s.state.record_sent(&[251], now)?;
                s.record(now, WireEvent::Send, &[251])?;
                self.uplink(slot, now, Uplink::Message(vec![251]))?;
            }
        }
        Ok(())
    }

    fn transmit(&mut self, slot: usize, at: f64) -> Result<(), String> {
        let s = &mut self.slots[slot];
        let Some((_, action, processing)) = s.outbox else {
            return Ok(());
        };
        let packets = protocol::action_packets(action, s.previous_boost)?;
        // Reserve everything first: a rejected departure stays queued with the
        // client's sent history, boost state and server control untouched.
        s.room(packets.iter().map(Vec::len))?;
        s.uplink_room(packets.len())?;
        s.outbox = None;
        s.previous_boost = Some(action.boost);
        s.state.set_processing_seconds(processing)?;
        for packet in packets {
            let s = &mut self.slots[slot];
            s.state.record_sent(&packet, at)?;
            s.record(at, WireEvent::Send, &packet)?;
            self.uplink(slot, at, Uplink::Message(packet))?;
        }
        Ok(())
    }

    pub(super) fn uplink(&mut self, slot: usize, sent: f64, message: Uplink) -> Result<(), String> {
        self.slots[slot].uplink_room(1)?;
        let connection = &mut self.slots[slot].connection;
        let arrival = connection
            .link
            .uplink(&self.config.latency, &mut self.random, sent);
        connection.uplink.push_back((arrival, message));
        Ok(())
    }

    /// Frame packets emitted at `now`. A packet too long for a length prefix (a
    /// dense large minimap) travels alone in its own frame, in stream order.
    /// Aggregate broadcasts are split to fit the bounded socket transports.
    pub(super) fn emit(
        &mut self,
        slot: usize,
        now: f64,
        packets: &[Vec<u8>],
    ) -> Result<(), String> {
        self.emit_presented(slot, now, packets, None)
    }

    pub(super) fn emit_presented(
        &mut self,
        slot: usize,
        now: f64,
        packets: &[Vec<u8>],
        physical: Option<&[Vec<u8>]>,
    ) -> Result<(), String> {
        let groups = frame_groups(packets);
        let connection = &mut self.slots[slot].connection;
        if connection.downlink.len() + groups.len() > MAX_IN_FLIGHT {
            return Err("Downlink queue is full".into());
        }
        // Decorations must not consume extra shared latency RNG draws: that
        // would change other bots' receipts and eventually physics. Advance
        // the baseline stream exactly as physical packets would, even when a
        // browser hides a colliding pellet. Extra presentation frames share
        // the last physical release; marker-only releases sample clones.
        let arrivals = physical.map(|packets| {
            frame_groups(packets)
                .iter()
                .map(|_| {
                    connection
                        .link
                        .downlink(&self.config.latency, &mut self.random, now)
                })
                .collect::<Vec<_>>()
        });
        let extra = if arrivals.as_ref().is_some_and(Vec::is_empty) && !groups.is_empty() {
            connection
                .link
                .clone()
                .downlink(&self.config.latency, &mut self.random.clone(), now)
        } else {
            now
        };
        for (index, group) in groups.into_iter().enumerate() {
            let setup = *connection.setup_at.get_or_insert(now);
            let target = ((now - setup) * 1000.).round().max(0.) as u64;
            let elapsed = target.saturating_sub(connection.header_ms).min(65535);
            connection.header_ms += elapsed;
            let bytes = encode::frame(elapsed as u16, group)?;
            let arrival = match &arrivals {
                Some(arrivals) => arrivals
                    .get(index)
                    .or_else(|| arrivals.last())
                    .copied()
                    .unwrap_or(extra),
                None => connection
                    .link
                    .downlink(&self.config.latency, &mut self.random, now),
            };
            let arrival = arrival.max(connection.presented_at);
            connection.presented_at = arrival;
            connection.downlink.push_back((arrival, bytes));
        }
        Ok(())
    }

    /// Server side at `now`: arrived connects, pings and commands, in stream order.
    pub(super) fn ingest(&mut self, slot: usize, now: f64) -> Result<(), String> {
        while let Some((at, _)) = self.slots[slot].connection.uplink.front() {
            if *at > now {
                break;
            }
            let (_, message) = self.slots[slot].connection.uplink.pop_front().unwrap();
            let over = self.slots[slot].connection.over;
            match message {
                Uplink::Victory(text) => self.victory(slot, text, now)?,
                // The stock browser keeps pinging during its death fade and
                // victory entry. Missing pongs trips its lag detector and slows
                // the animation to one fifth speed, delaying Play Again.
                Uplink::Message(ref bytes) if bytes == &[251] => {
                    let connection = &mut self.slots[slot].connection;
                    if connection.setup {
                        connection.pongs += 1;
                    } else {
                        self.emit(slot, now, &[encode::pong()])?;
                    }
                }
                _ if over => {}
                Uplink::Connect => {
                    let radius = self.world.config.radius;
                    let connection = &mut self.slots[slot].connection;
                    connection.setup = true;
                    connection.radius = radius.round() as u32;
                    self.emit(
                        slot,
                        now,
                        &[encode::setup(radius, self.world.config.prey.turn_rate)?],
                    )?;
                }
                Uplink::Message(bytes) => match bytes[..] {
                    [heading @ 0..=250] => {
                        self.slots[slot].control.heading = (f64::from(heading) + 0.5) * TAU / 251.;
                    }
                    [252, amount] => {
                        // Keyboard turn: amounts below 128 turn the wanted heading
                        // down, 128 and above up, by the source bridge's 0.033 rad
                        // per count scaled by the size turn factor. An
                        // approximation; the closed server's rule is unaudited.
                        let (count, sign) = if amount < 128 {
                            (amount, -1.)
                        } else {
                            (amount - 128, 1.)
                        };
                        let turn = 0.033 * physics::scang(self.world.snakes[slot].segments);
                        let control = &mut self.slots[slot].control;
                        control.heading =
                            (control.heading + sign * turn * f64::from(count)).rem_euclid(TAU);
                    }
                    [253] => self.slots[slot].control.boost = true,
                    [254] => self.slots[slot].control.boost = false,
                    _ => return Err("Unsupported client message".into()),
                },
            }
        }
        Ok(())
    }
}

/// Bijective mix of a connection serial, so IDs stay distinct for 2^32
/// connections without revealing their order. Opaque; it authenticates nothing.
fn session_id(serial: u32, key: u32) -> u32 {
    let mut x = (serial ^ key).wrapping_mul(0x9e37_79b1);
    x ^= x >> 16;
    x = x.wrapping_mul(0x85eb_ca6b);
    x ^ x >> 13
}

/// Stock transport grouping, shared by physical scheduling and presentation.
fn frame_groups(packets: &[Vec<u8>]) -> Vec<&[Vec<u8>]> {
    let mut groups: Vec<&[Vec<u8>]> = Vec::new();
    let mut start = 0;
    let mut bytes = 2; // Elapsed-ms header.
    for (index, packet) in packets.iter().enumerate() {
        if !encode::frameable(packet) {
            if start < index {
                groups.push(&packets[start..index]);
            }
            groups.push(&packets[index..=index]);
            start = index + 1;
            bytes = 2;
        } else {
            let packet_bytes = packet.len() + 2;
            if bytes + packet_bytes > MAX_FRAME_BYTES && start < index {
                groups.push(&packets[start..index]);
                start = index;
                bytes = 2;
            }
            bytes += packet_bytes;
        }
    }
    if start < packets.len() {
        groups.push(&packets[start..]);
    }
    groups
}
