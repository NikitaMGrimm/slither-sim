"""Protocol-19 decoding and state tracking for slither.io calibration captures.

Reads `tools/fidelity/slither-headless.mjs` archives (`*.jsonl.gz`). Each incoming frame carries the
server's elapsed-milliseconds header, so `Frame.server` is a per-connection server clock.
The tracker mirrors the stock client's body bookkeeping (anchors, `cst` contraction, tail
removal); it does not reproduce display smoothing.
"""
import base64
import gzip
import json
import math
from dataclasses import dataclass, field

TAU = 2 * math.pi
PROTOCOL_VERSION = 19


@dataclass
class Frame:
    conn: int
    t: float  # local receipt clock, ms
    server: int  # cumulative server elapsed ms on this connection
    packets: list


@dataclass
class Out:
    conn: int
    t: float
    data: bytes


def split(a):
    if len(a) < 3:
        raise ValueError('short timing frame')
    return split_payload(a[2:])


def split_payload(a):
    """Split logical packets after any connection-specific header is removed."""
    if not a:
        raise ValueError('empty packet payload')
    m = 0
    if a[m] >= 32:
        return [a[m:]]
    out = []
    while m < len(a):
        if a[m] < 32:
            if m + 1 >= len(a):
                raise ValueError('truncated packet length')
            n = a[m] << 8 | a[m + 1]
            m += 2
        else:
            n = a[m] - 32
            m += 1
        if not n or m + n > len(a):
            raise ValueError('invalid packet length')
        out.append(a[m:m + n])
        m += n
    return out


def read(path):
    """Yield header/meta dicts, Frame and Out records in capture order."""
    conn, clock = 0, None
    with gzip.open(path, 'rt') as f:
        for line in f:
            o = json.loads(line)
            if isinstance(o, dict):
                if o.get('event') == 'connect':
                    conn, clock = conn + 1, None
                yield o
                continue
            d, t, b = o
            if d not in ('i', 'o'):
                raise ValueError('invalid frame direction')
            b = base64.b64decode(b, validate=True)
            if d == 'o':
                yield Out(conn, t, b)
                continue
            packets = split(b)
            h = b[0] << 8 | b[1]
            # The setup frame's header is an initial epoch (occasionally 0xffff),
            # not time elapsed since the preceding challenge frame. Make setup
            # the game-clock origin; later headers are elapsed intervals.
            clock = 0 if clock is None or any(p[:1] == b'a' for p in packets) else clock + h
            yield Frame(conn, t, clock, packets)


def u16(p, m):
    return p[m] << 8 | p[m + 1]


def u24(p, m):
    return p[m] << 16 | p[m + 1] << 8 | p[m + 2]


def scale(sct):
    return min(6.0, 1 + (sct - 2) / 106)


@dataclass
class Snake:
    id: int
    pts: list  # tail -> head anchors [x, y]
    ang: float
    wang: float
    sp: float
    fam: float
    iang: int = 0
    anchor_server: int = 0
    kills: int = 0
    nick: str = ''
    hist: list = field(default_factory=list)  # (server ms, ang, sp) since the last anchor
    base: tuple = None  # exact head position known at `anchor_server` (spawn head or anchor)
    rsc: int = 0

    @property
    def sct(self):
        return len(self.pts)

    def head(self, S):
        """Server head estimate: last anchor advanced through the reported headings."""
        x, y = self.base or self.pts[-1]
        steps = self.hist or [(self.anchor_server, self.ang, self.sp)]
        for i, (s0, a, sp) in enumerate(steps):
            s1 = steps[i + 1][0] if i + 1 < len(steps) else S
            dt = max(0, min(s1, S) - max(s0, self.anchor_server))
            x += math.cos(a) * sp / 32 * dt
            y += math.sin(a) * sp / 32 * dt
        return x, y


@dataclass
class World:
    """Partial packet reconstruction for analysis; minimap occupancy is not decoded."""
    msl: int = 42
    cst: float = 0.43
    grd: int = 21600
    radius: float = 21000
    sector_size: int = 300
    mscps: int = 430
    contract: bool = True  # apply the stock client's cst anchor contraction
    contract_depth: int = 1 << 30  # older anchors beyond this many are left in place
    self_id: int = None
    snakes: dict = field(default_factory=dict)
    foods: dict = field(default_factory=dict)
    prey: dict = field(default_factory=dict)
    lfs: tuple = (0, 0)
    lfe: tuple = (0, 0)
    last_eater: int = -1
    events: list = field(default_factory=list)

    def food_id(self, sx, sy, rx, ry):
        return sx << 24 | sy << 16 | rx << 8 | ry

    def add_food(self, sx, sy, rx, ry, size, S, kind):
        k = self.sector_size / 256
        x, y = sx * self.sector_size + rx * k, sy * self.sector_size + ry * k
        fid = self.food_id(sx, sy, rx, ry)
        self.foods[fid] = (x, y, size)
        if kind != 'F':
            self.events.append(('food', S, fid, x, y, size, kind))

    def feed(self, p, S):
        c = chr(p[0])
        d = len(p) - 1
        if c == 'a':
            if len(p) < 25:
                raise ValueError('short setup packet')
            if p[23] != PROTOCOL_VERSION:
                raise ValueError(f'Unsupported protocol version {p[23]}; expected {PROTOCOL_VERSION}')
            self.grd = u24(p, 1)
            self.mscps = u16(p, 4)
            self.sector_size = u16(p, 6)
            self.cst = u16(p, 21) / 1000
            if len(p) > 24:
                self.msl = p[24]
            self.radius = u24(p, 27) if len(p) > 29 else self.grd * .98
            self.events.append(('init', S, self.grd, self.radius))
        elif c == 'z':
            self.radius = u24(p, 1)
            self.events.append(('radius', S, self.radius))
        elif c == 's':
            self.snake_packet(p, S)
        elif c == 'R' and len(p) == 2:
            o = self.snakes.get(self.self_id)
            if o:
                old = o.rsc
                o.rsc = p[1]
                self.events.append(('rsc', S, o.id, old, o.rsc))
        elif c in 'gnGN+=':
            self.anchor(c, p, S)
        elif c in 'eE345d7':
            self.rotation(c, p, S)
        elif c == 'h':
            o = self.snakes.get(u16(p, 1))
            if o:
                old = o.fam
                o.fam = u24(p, 3) / 16777215
                self.events.append(('fam', S, o.id, old, o.fam))
        elif c == 'r':
            o = self.snakes.get(u16(p, 1))
            if o and o.pts:
                old = o.fam
                o.pts.pop(0)
                if d >= 4:
                    o.fam = u24(p, 3) / 16777215
                self.events.append(('tail', S, o.id, old, o.fam))
        elif c == 'k':
            o = self.snakes.get(u16(p, 1))
            if o:
                o.kills = u24(p, 3)
                self.events.append(('kill', S, o.id, o.kills))
        elif c == 'F':
            for m in range(3, len(p) - 3, 4):
                self.add_food(p[1], p[2], p[m + 1], p[m + 2], p[m + 3] / 5, S, 'F')
        elif c in 'bf':
            m = 1
            if d >= 5:
                self.lfs = (p[1], p[2])
                m = 3
            rx, ry = p[m], p[m + 1]
            m += 2
            if d in (4, 6):
                m += 1
            self.add_food(*self.lfs, rx, ry, p[m] / 5, S, c)
        elif c in 'cC<':
            m = 1
            if not ((c == 'c' and d == 2) or (c == '<' and d == 4) or (c == 'C' and d == 2)):
                self.lfe = (p[1], p[2])
                m = 3
            fid = self.food_id(*self.lfe, p[m], p[m + 1])
            m += 2
            if c == '<':
                self.last_eater = u16(p, m)
            eater = self.last_eater if c in '<C' else None
            food = self.foods.pop(fid, None)
            self.events.append(('eat', S, fid, food, eater))
        elif c == 'w':
            for k in [k for k, f in self.foods.items() if k >> 24 == p[1] and (k >> 16) & 255 == p[2]]:
                del self.foods[k]
        elif c == 'y':
            pid = u16(p, 1)
            if d == 2:
                self.prey.pop(pid, None)
            elif d == 4:
                self.events.append(('prey_eaten', S, pid, self.prey.pop(pid, None), u16(p, 3)))
            else:
                self.prey[pid] = {'x': u24(p, 4) / 5, 'y': u24(p, 7) / 5, 'size': p[10] / 5,
                                  'wang': u24(p, 12) * TAU / 16777215, 'ang': u24(p, 15) * TAU / 16777215,
                                  'sp': u16(p, 18) / 1000, 'server': S}
                self.events.append(('prey', S, pid, dict(self.prey[pid])))
        elif c == 'j':
            pr = self.prey.get(u16(p, 1))
            if pr:
                pr['x'], pr['y'] = 1 + u16(p, 3) * 3, 1 + u16(p, 5) * 3
                m = 7
                if d == 15:
                    pr['ang'], pr['wang'], pr['sp'] = u24(p, m + 1) * TAU / 16777215, u24(p, m + 4) * TAU / 16777215, u16(p, m + 7) / 1000
                elif d == 11:
                    pr['ang'], pr['sp'] = u24(p, m) * TAU / 16777215, u16(p, m + 3) / 1000
                elif d == 12:
                    pr['wang'], pr['sp'] = u24(p, m + 1) * TAU / 16777215, u16(p, m + 4) / 1000
                elif d == 13:
                    pr['ang'], pr['wang'] = u24(p, m + 1) * TAU / 16777215, u24(p, m + 4) * TAU / 16777215
                elif d == 9:
                    pr['ang'] = u24(p, m) * TAU / 16777215
                elif d == 10:
                    pr['wang'] = u24(p, m + 1) * TAU / 16777215
                elif d == 8:
                    pr['sp'] = u16(p, m) / 1000
                pr['server'] = S
                self.events.append(('prey_move', S, u16(p, 1), dict(pr)))
        elif c == 'l' and d >= 5:
            self.events.append(('players', S, u16(p, 4), u16(p, 2)))
        elif c == 'v':
            self.events.append(('game_over', S, p[1] if d else None))

    def snake_packet(self, p, S):
        sid = u16(p, 1)
        if len(p) - 1 <= 6:
            o = self.snakes.pop(sid, None)
            self.events.append(('remove', S, sid, len(p) > 3 and p[3] == 1, o))
            return
        m = 3
        ang = u24(p, m) * TAU / 16777215
        m += 4
        wang = u24(p, m) * TAU / 16777215
        m += 3
        sp = u16(p, m) / 1000
        m += 2
        fam = u24(p, m) / 16777215
        m += 4
        head = (u24(p, m) / 5, u24(p, m + 3) / 5)
        m += 6
        nick = bytes(p[m + 1:m + 1 + p[m]]).decode('latin1')
        m += 1 + p[m]
        m += 1 + p[m]
        m += 1
        pts, x, y, iang = [], 0.0, 0.0, 0
        while m < len(p):
            if not pts:
                x, y = u24(p, m) / 5, u24(p, m + 3) / 5
                m += 6
            elif m == len(p) - 2:
                iang = u16(p, m)
                x += math.cos(iang * TAU / 65536) * self.msl
                y += math.sin(iang * TAU / 65536) * self.msl
                m += 2
            else:
                x += (p[m] - 127) / 2
                y += (p[m + 1] - 127) / 2
                m += 2
            pts.append([x, y])
        o = Snake(sid, pts, ang, wang, sp, fam, iang, S, nick=nick, hist=[(S, ang, sp)], base=head)
        self.snakes[sid] = o
        if self.self_id is None:
            self.self_id = sid
        self.events.append(('spawn', S, sid, o))

    def anchor(self, c, p, S):
        m = 1
        if c in 'GN' or (c == '=' and len(p) == 7) or (c == '+' and len(p) == 10):
            o = self.snakes.get(self.self_id)
        else:
            o = self.snakes.get(u16(p, 1))
            m = 3
        if not o or not o.pts:
            return
        last = o.pts[-1]
        if c in '+=':
            o.iang = u16(p, m)
            x, y = float(u16(p, m + 2)), float(u16(p, m + 4))
            m += 6
        else:
            d = len(p) - 1
            if (c == 'G' and d == 2) or (c == 'N' and d == 5) or (c == 'g' and d == 4) or (c == 'n' and d == 7):
                o.iang = u16(p, m)
                m += 2
            a = o.iang * TAU / 65536
            x, y = last[0] + math.cos(a) * self.msl, last[1] + math.sin(a) * self.msl
        grow = c in 'nN+'
        if grow:
            o.fam = u24(p, m) / 16777215
        else:
            o.pts.pop(0)
        o.pts.append([x, y])
        # Stock-client contraction: older anchors move toward their head-side neighbor.
        k = len(o.pts) - 3
        if k >= 1 and self.contract:
            lx, ly = o.pts[k]
            n = 0
            for j in range(k - 1, max(-1, k - 1 - self.contract_depth), -1):
                n += 1
                w = self.cst * min(n, 4) / 4
                q = o.pts[j]
                q[0] += (lx - q[0]) * w
                q[1] += (ly - q[1]) * w
                lx, ly = q
        o.anchor_server = S
        o.base = (x, y)
        o.hist = [(S, o.ang, o.sp)]
        self.events.append(('anchor', S, o.id, x, y, grow, c))

    def rotation(self, c, p, S):
        plen = len(p)
        m = 1
        if c in 'd7' or (plen - 1 <= 2 and c in 'eE345'):
            o = self.snakes.get(self.self_id)
        else:
            o = self.snakes.get(u16(p, 1))
            m = 3
        if not o:
            return
        k = TAU / 256
        before = (o.ang, o.wang, o.sp)
        if plen == 6:
            o.ang, o.wang, o.sp = p[m] * k, p[m + 1] * k, p[m + 2] / 18
        elif plen in (5, 3):
            if c == 'e':
                o.ang, o.sp = p[m] * k, p[m + 1] / 18
            elif c in 'E4':
                o.wang, o.sp = p[m] * k, p[m + 1] / 18
            else:
                o.ang, o.wang = p[m] * k, p[m + 1] * k
        elif plen in (4, 2):
            if c == 'e':
                o.ang = p[m] * k
            elif c in 'E4':
                o.wang = p[m] * k
            elif c == '3':
                o.sp = p[m] / 18
            else:
                o.ang, o.wang, o.sp = p[m] * k, p[m + 1] * k, p[m + 2] / 18
        o.hist.append((S, o.ang, o.sp))
        self.events.append(('rot', S, o.id, before, (o.ang, o.wang, o.sp)))


class ReceivedWorld(World):
    """Client receipt reconstruction; ``World`` follows stock body bookkeeping.

    Feed with ``feed_received(packet, server_ms, receipt_ms)`` or ``feed_frame``.
    Server time is used only by Snake.head's explicit extrapolation. Receipt
    ages and histories use the capture's local Frame.t, including repeated values.
    Appearance resets an ID's incarnation even when it was still in the inbox.
    """

    def __init__(self):
        super().__init__()
        self.contract = False  # canonical contraction happens before insertion
        self.connection = None
        self.connection_generation = 0
        self._incarnations = {}
        self._receipts = {}
        self._categories = {}
        self._last_receipt = None

    def reconnect(self, connection=None):
        generations = self._incarnations
        generation = self.connection_generation + 1
        self.__init__()
        self.connection = connection
        self.connection_generation = generation
        self._incarnations = generations

    def incarnation(self, snake_id):
        return self._receipts[snake_id]['incarnation']

    def feed_frame(self, frame):
        if self.connection != frame.conn:
            self.reconnect(frame.conn)
        for packet in frame.packets:
            if packet:
                self.feed_received(packet, frame.server, frame.t)

    def feed(self, packet, server_ms):
        raise TypeError('ReceivedWorld requires a local receipt clock; use feed_received')

    def anchor(self, code, packet, server_ms):
        own = code in 'GN' or (code == '=' and len(packet) == 7) or (code == '+' and len(packet) == 10)
        snake = self.snakes.get(self.self_id if own else u16(packet, 1))
        if snake:
            # Same measured rule as physics::append_anchor. Tail->head order.
            k = len(snake.pts) - 3
            for j in range(k - 1, -1, -1):
                weight = self.cst * min(k - j, 4) / 4
                snake.pts[j] = [a + (b - a) * weight
                                for a, b in zip(snake.pts[j], snake.pts[j + 1])]
        super().anchor(code, packet, server_ms)

    def feed_received(self, packet, server_ms, receipt_ms):
        if not math.isfinite(receipt_ms) or receipt_ms < 0:
            raise ValueError('Invalid receipt clock')
        if self._last_receipt is not None and receipt_ms < self._last_receipt:
            self.reconnect(self.connection)
        self._last_receipt = receipt_ms
        if not packet:
            return
        start = len(self.events)
        super().feed(packet, server_ms)
        code = chr(packet[0])
        for event in self.events[start:]:
            kind = event[0]
            if kind == 'spawn':
                sid, snake = event[2:4]
                incarnation = self._incarnations.get(sid, 0) + 1
                self._incarnations[sid] = incarnation
                self._receipts[sid] = {
                    'incarnation': incarnation,
                    'fields': {name: receipt_ms for name in (
                        'appearance', 'position', 'heading', 'wantedHeading', 'speed', 'body', 'mass')},
                    'wanted': [], 'speeds': [(receipt_ms, snake.sp / .032)],
                }
                self._categories['bodies'] = receipt_ms
            elif kind == 'remove':
                self._receipts.pop(event[2], None)
                self._categories['bodies'] = receipt_ms
            elif kind in ('anchor', 'tail', 'fam', 'rot'):
                sid = event[2]
                track = self._receipts.get(sid)
                if track is None:
                    continue
                fields = track['fields']
                if kind == 'anchor':
                    fields['body'] = fields['position'] = receipt_ms
                    if event[5]:
                        fields['mass'] = receipt_ms
                    self._categories['bodies'] = receipt_ms
                elif kind == 'tail':
                    fields['body'] = fields['mass'] = receipt_ms
                    self._categories['bodies'] = receipt_ms
                elif kind == 'fam':
                    fields['mass'] = receipt_ms
                else:
                    # A report of an unchanged value still refreshes that field,
                    # never the other fields omitted from this compressed packet.
                    plen = len(packet)
                    if plen == 6 or (plen in (4, 2) and code not in 'eE43'):
                        present = ('heading', 'wantedHeading', 'speed')
                    elif plen in (5, 3):
                        present = (('heading', 'speed') if code == 'e' else
                                   ('wantedHeading', 'speed') if code in 'E4' else
                                   ('heading', 'wantedHeading'))
                    else:
                        present = (('heading',) if code == 'e' else
                                   ('wantedHeading',) if code in 'E4' else ('speed',))
                    for name in present:
                        fields[name] = receipt_ms
                    before, after = event[3:5]
                    if 'wantedHeading' in present and before[1] != after[1]:
                        track['wanted'].insert(0, (receipt_ms, after[1]))
                        del track['wanted'][3:]
                    if 'speed' in present:
                        track['speeds'].insert(0, (receipt_ms, after[2] / .032))
                        track['speeds'] = [row for row in track['speeds']
                                           if receipt_ms - row[0] <= 500][:80]
        if code in 'FbfcC<w':
            self._categories['food'] = receipt_ms
        if code in 'yj':
            self._categories['prey'] = receipt_ms
        if code in 'MV':
            self._categories['minimap'] = receipt_ms

    @staticmethod
    def _field(receipt_ms, now_ms, predicted=False):
        return {'ageSeconds': max(0., (now_ms - receipt_ms) / 1000.) if receipt_ms is not None else 0.,
                'valid': receipt_ms is not None, 'predicted': predicted if receipt_ms is not None else False}

    def metadata(self, now_ms, snake_ids=None, extrapolation_server_ms=None):
        """Pure read; polling must not refresh any receipt timestamp.

        Pass extrapolation_server_ms only if the corresponding head coordinates
        were produced with Snake.head(server_ms). No opponent-inbox completeness
        is inferred from the capture observer's positives or sector subscriptions.
        """
        if not math.isfinite(now_ms) or now_ms < 0:
            raise ValueError('Invalid receipt clock')
        rows = []
        for sid in self.snakes if snake_ids is None else snake_ids:
            snake, track = self.snakes.get(sid), self._receipts.get(sid)
            if snake is None or track is None:
                continue
            row = {'snakeId': str(sid), 'incarnation': track['incarnation']}
            for name, at in track['fields'].items():
                predicted = name == 'position' and extrapolation_server_ms is not None and extrapolation_server_ms > snake.anchor_server
                row[name] = self._field(at, now_ms, predicted)
            def history(report):
                at, value = report
                return {'ageSeconds': max(0., (now_ms - at) / 1000.),
                        'valid': 0 <= now_ms - at <= 500, 'value': value}
            row['wantedHistory'] = [history(report) for report in track['wanted']
                                     if 0 <= now_ms - report[0] <= 500]
            earlier = [report for report in track['speeds'][1:] if 0 <= now_ms - report[0] <= 500]
            row['speedHistory'] = [history(min(earlier, key=lambda report: abs(now_ms - report[0] - age)))
                                    if earlier else {'ageSeconds': 0., 'valid': False, 'value': 0.}
                                    for age in (84, 252)]
            rows.append(row)
        categories = {}
        for name in ('bodies', 'food', 'prey'):
            field = self._field(self._categories.get(name), now_ms)
            categories[name] = {'ageSeconds': field['ageSeconds'], 'valid': field['valid'], 'complete': False}
        return {'connectionGeneration': self.connection_generation, 'snakes': rows, **categories,
                'minimap': self._field(self._categories.get('minimap'), now_ms)}
