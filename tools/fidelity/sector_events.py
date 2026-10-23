"""Packet-ordered, positive-only protocol-19 sector receipts (no archive I/O).

Call ``SectorLedger.feed_frame`` with ALL logical packets from one frame, as
returned by the sibling ``slither_protocol.split`` / ``split_payload``. Supply
the original record/frame indices, connection/life IDs and clocks; packet indices
are zero-based positions in that complete frame. Consume the returned events
before feeding another frame. No event history or raw bytes are retained here.

The caller owns archive hashes, framing/protocol audit, completeness/gap checks,
own-life detection (including own ``s`` removals), pose gates and provenance.
Connection/life changes and setup/game-over packets reset sector and compression
history. Call ``reset`` at any additional discontinuity; never fill a missing
prefix with another connection's state. Server time may be None for header-free
input. Receipt time is arrival time, not server processing time.

W announces a sector with unknown food occupancy. An empty F explicitly lists
zero foods, but neither F nor W proves a complete physical-food/subscription
population. F adds to the received cache, as in the client decoder; even an empty
F does not erase older cached keys. w clears this recipient's cache, not global
food. Cached counts count reconstructible positive keys only. ``cache_unknown``
marks a missing F/w baseline or unlocated compressed deltas since the last w;
unannounced sectors remain unknown, never inferred empty. Quarter codes are
floor(rx/64), floor(ry/64), not a claim about subscription geometry. Reused keys
are not food-incarnation evidence.

Positive reintroductions are nonempty F after explicit same-epoch w without an
intervening W/F for that sector. Counts include same-frame ambiguities, which
are flagged on BOTH w and F regardless of their order. ``entry_candidate`` only
excludes those ambiguities; the caller must still audit intervening history and
apply its prospectively frozen primary gates. Counters are observed receipts,
not N_entry or a complete subscription population.

Validation is atomic per frame for the declared food/sector forms and the setup
prefix. Other commands yield ``unsupported`` events with unvalidated payloads;
this layer is not a general protocol validator. Memory is bounded by wire sector
and food keys plus one frame's decoded events, without a growing event history.
"""

from dataclasses import dataclass, field
import math

from slither_protocol import PROTOCOL_VERSION, u16


class MalformedPacket(ValueError):
    """A declared sector/food packet has an invalid length."""


class UnsupportedPacket(ValueError):
    """A setup declares a protocol outside this layer's contract."""


@dataclass(frozen=True)
class Source:
    connection: int
    life: int
    epoch: int
    record_index: int
    frame_index: int
    packet_index: int
    server_ms: float | None
    receipt_ms: float


@dataclass(frozen=True)
class Food:
    key: tuple[int, int, int, int]
    size_code: int
    color: int | None = None

    @property
    def size(self):
        return self.size_code / 5

    @property
    def quarter(self):
        return self.key[2] // 64, self.key[3] // 64


@dataclass(frozen=True)
class SectorEvent:
    kind: str
    command: str
    length: int
    source: Source
    sector: tuple[int, int] | None = None
    episode: int | None = None
    prior_announcement: str | None = None
    prior_episode: int | None = None
    announcement: str | None = None
    last_W: Source | None = None
    last_F: Source | None = None
    last_w: Source | None = None
    cached_count: int | None = None
    prior_cached_count: int | None = None
    cache_unknown: bool = True
    foods: tuple[Food, ...] = ()
    food_key: tuple[int, int, int, int] | None = None
    local: tuple[int, int] | None = None
    size_code: int | None = None
    color: int | None = None
    sector_compressed: bool = False
    sector_unknown: bool = False
    eater: int | None = None
    entry_kind: str | None = None
    listing_empty: bool | None = None
    clear_prior: str | None = None
    positive_reintroduction: bool = False
    same_frame_clear_f: bool = False
    entry_candidate: bool = False


@dataclass
class Counters:
    """Cumulative observed-receipt denominators, retained across resets."""

    w_announcements: int = 0  # W packets, including repeated announcements
    f_nonempty: int = 0
    f_empty: int = 0
    sector_episodes: int = 0  # first W/F after reset or clear
    clears_prior_f: int = 0
    clears_prior_w_only: int = 0
    clears_prior_unknown: int = 0
    positive_reintroductions: int = 0
    unambiguous_positive_reintroductions: int = 0
    initial_f: int = 0
    repeated_f: int = 0
    w_preceded_f: int = 0
    after_clear_f: int = 0
    additions: int = 0
    pickups: int = 0
    unknown_compressed_additions: int = 0
    unknown_compressed_pickups: int = 0
    unsupported_packets: int = 0
    resets: int = 0


@dataclass
class _Sector:
    foods: dict = field(default_factory=dict)
    episode: int = 0
    announcement: str | None = None
    had_f: bool = False
    baseline: bool = False
    cleared_unknowns: int = 0
    last_W: Source | None = None
    last_F: Source | None = None
    last_w: Source | None = None


@dataclass(frozen=True)
class _Packet:
    command: str
    length: int
    sector: tuple[int, int] | None = None
    pellets: tuple = ()  # (rx, ry, size_code, color)
    local: tuple[int, int] | None = None
    eater: int | None = None


def _decode(packet):
    if not isinstance(packet, (bytes, bytearray, memoryview)) or not packet:
        raise MalformedPacket("expected a nonempty logical packet")
    code, n = chr(packet[0]), len(packet)
    bad = MalformedPacket(f"malformed {code!r} packet length {n}")
    if code in ("W", "w"):
        if n != 3:
            raise bad
        return _Packet(code, n, (packet[1], packet[2]))
    if code == "F":
        if n < 3 or (n - 3) % 4:
            raise bad
        return _Packet(code, n, (packet[1], packet[2]), tuple(
            (packet[i + 1], packet[i + 2], packet[i + 3], packet[i])
            for i in range(3, n, 4)))
    if code in ("f", "b"):
        if n not in (4, 5, 6, 7):
            raise bad
        m = 3 if n in (6, 7) else 1
        sector = (packet[1], packet[2]) if m == 3 else None
        color = packet[m + 2] if n in (5, 7) else None
        return _Packet(code, n, sector,
                       ((packet[m], packet[m + 1], packet[-1], color),))
    if code in ("c", "C", "<"):
        explicit = n == (7 if code == "<" else 5)
        if not explicit and n != (5 if code == "<" else 3):
            raise bad
        m = 3 if explicit else 1
        return _Packet(code, n, (packet[1], packet[2]) if explicit else None,
                       local=(packet[m], packet[m + 1]),
                       eater=u16(packet, n - 2) if code == "<" else None)
    if code == "a":
        if n < 25:
            raise bad
        if packet[23] != PROTOCOL_VERSION:
            raise UnsupportedPacket(f"unsupported setup protocol {packet[23]}")
    return _Packet(code, n)


class SectorLedger:
    def __init__(self):
        self.counters = Counters()
        self.epoch = 0
        self._identity = None
        self._last_frame = None
        self._sectors = {}
        self._add_sector = None
        self._eat_sector = None
        self._eater = None
        self._unresolved = 0

    def reset(self):
        """Forget histories at a caller-detected gap/death; retain counters."""
        self.epoch += 1
        self.counters.resets += 1
        self._sectors.clear()
        self._add_sector = self._eat_sector = self._eater = None
        self._unresolved = 0
        self._last_frame = None

    def feed_frame(self, packets, *, connection, life, record_index, frame_index,
                   server_ms, receipt_ms):
        """Return immutable per-packet events; retain no results or raw input.

        Indices must increase within a connection/life, even across setup/death.
        Validate the whole frame before applying anything. Supplying the complete
        frame is necessary to flag F-before-w as well as w-before-F ambiguity.
        Initial/repeated/W-preceded/after-clear F strata include empty listings;
        only nonempty after-clear F increments positive reintroductions.
        """
        for value in (connection, life, record_index, frame_index):
            if type(value) is not int or value < 0:
                raise ValueError("IDs and source indices must be nonnegative integers")
        for clock in (receipt_ms,) if server_ms is None else (receipt_ms, server_ms):
            if type(clock) not in (int, float) or not math.isfinite(clock) or clock < 0:
                raise ValueError("invalid source clock")
        decoded = [_decode(packet) for packet in packets]
        if not decoded:
            raise ValueError("empty logical frame")
        identity = connection, life
        if identity == self._identity and self._last_frame is not None:
            record, frame, receipt, server = self._last_frame
            if record_index <= record or frame_index <= frame or receipt_ms < receipt:
                raise ValueError("out-of-order source frame")
            if (server_ms is not None and server is not None and server_ms < server
                    and not any(p.command == "a" for p in decoded)):
                raise ValueError("server clock regressed without setup/life reset")

        # Separate setup/death segments so a same-frame lifecycle boundary cannot
        # pair a clear in the old history with a listing in the new one.
        ambiguous, segment = set(), 0
        clears, listings = set(), set()
        for p in decoded:
            if p.command in ("a", "v"):
                ambiguous.update((segment, key) for key in clears & listings)
                segment += 1
                clears, listings = set(), set()
            elif p.command == "w":
                clears.add(p.sector)
            elif p.command == "F":
                listings.add(p.sector)
        ambiguous.update((segment, key) for key in clears & listings)

        if identity != self._identity:
            self.reset()
            self._identity = identity
        events, segment = [], 0
        for index, p in enumerate(decoded):
            if p.command in ("a", "v"):
                self.reset()
                segment += 1
            source = Source(connection, life, self.epoch, record_index, frame_index,
                            index, server_ms, receipt_ms)
            events.append(self._apply(p, source, (segment, p.sector) in ambiguous))
        self._last_frame = record_index, frame_index, receipt_ms, server_ms
        return events

    def _apply(self, p, source, ambiguous):
        code, counts = p.command, self.counters
        common = dict(command=code, length=p.length, source=source)
        if code in ("a", "v"):
            return SectorEvent("reset", **common)
        if code not in ("W", "F", "w", "f", "b", "c", "C", "<"):
            counts.unsupported_packets += 1
            return SectorEvent("unsupported", **common)

        sector, compressed, eater = p.sector, False, None
        if code in ("f", "b"):
            counts.additions += 1
            compressed = sector is None
            if sector is not None:
                self._add_sector = sector
            sector = self._add_sector
        elif code in ("c", "C", "<"):
            counts.pickups += 1
            compressed = sector is None
            if sector is not None:
                self._eat_sector = sector
            sector = self._eat_sector
            if code == "<":
                self._eater = p.eater
            eater = self._eater if code in ("C", "<") else None
        local, size, color = p.local, None, None
        if code in ("f", "b"):
            rx, ry, size, color = p.pellets[0]
            local = rx, ry
        common.update(sector=sector, sector_compressed=compressed, eater=eater,
                      local=local, size_code=size, color=color)
        if sector is None:
            self._unresolved += 1
            if code in ("f", "b"):
                counts.unknown_compressed_additions += 1
            else:
                counts.unknown_compressed_pickups += 1
            return SectorEvent(code, **common, sector_unknown=True)

        state = self._sectors.setdefault(sector, _Sector())
        prior_announcement = state.announcement
        prior_episode = state.episode if state.announcement else None
        prior_count = len(state.foods)
        entry_kind, positive, clear_prior = None, False, None
        foods = tuple(Food((*sector, rx, ry), size, color)
                      for rx, ry, size, color in p.pellets)
        key = (*sector, *p.local) if p.local is not None else None
        if code in ("W", "F"):
            if code == "F":
                if state.announcement == "W":
                    entry_kind = "W_preceded"
                    counts.w_preceded_f += 1
                elif state.announcement == "F":
                    entry_kind = "repeated"
                    counts.repeated_f += 1
                elif state.last_w is not None:
                    entry_kind = "after_clear"
                    counts.after_clear_f += 1
                    positive = bool(foods)
                else:
                    entry_kind = "initial"
                    counts.initial_f += 1
            if state.announcement is None:
                state.episode += 1
                counts.sector_episodes += 1
            state.announcement = code
            if code == "W":
                counts.w_announcements += 1
                state.last_W = source
            else:
                counts.f_nonempty += bool(foods)
                counts.f_empty += not foods
                counts.positive_reintroductions += positive
                counts.unambiguous_positive_reintroductions += positive and not ambiguous
                state.had_f = state.baseline = True
                state.last_F = source
                for food in foods:
                    state.foods[food.key] = food
        elif code == "w":
            if state.had_f:
                clear_prior = "known_F"
                counts.clears_prior_f += 1
            elif state.announcement == "W":
                clear_prior = "W_only"
                counts.clears_prior_w_only += 1
            else:
                clear_prior = "unknown"
                counts.clears_prior_unknown += 1
            state.foods.clear()
            state.announcement, state.had_f = None, False
            state.baseline, state.last_w = True, source
            state.cleared_unknowns = self._unresolved
        elif code in ("f", "b"):
            state.foods[foods[0].key] = foods[0]
        else:
            state.foods.pop(key, None)
        return SectorEvent(
            code, **common, episode=state.episode if state.announcement else None,
            prior_announcement=prior_announcement, prior_episode=prior_episode,
            announcement=state.announcement, last_W=state.last_W, last_F=state.last_F,
            last_w=state.last_w, cached_count=len(state.foods),
            prior_cached_count=prior_count,
            cache_unknown=not state.baseline or state.cleared_unknowns < self._unresolved,
            foods=foods, food_key=key, entry_kind=entry_kind,
            listing_empty=not foods if code == "F" else None,
            clear_prior=clear_prior,
            positive_reintroduction=positive,
            same_frame_clear_f=ambiguous and code in ("w", "F"),
            entry_candidate=positive and not ambiguous)
