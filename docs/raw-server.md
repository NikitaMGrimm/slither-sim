# Protocol-19 WebSocket server

`slither-sim host` serves one shared world at `ws://127.0.0.1:8766/slither`.
Use it for socket-based bots. For virtual-time runs without socket overhead, use
`SimulatorBackend` through the [portable API](client-api.md).

From the checkout:

```sh
cargo run --release --locked -- host
```

After the [initial build](../README.md#quick-start), connect from another terminal:

```sh
python examples/portable_bot.py --url ws://127.0.0.1:8766/slither --steps 200
```

The same bot works with or without `--url`. Stop the host with Ctrl+C.
No game UI is served.

## Configuration

Save as `arena.json`:

```json
{
  "bind": "127.0.0.1:8766",
  "seed": 7,
  "maxClients": 4,
  "bots": 4,
  "simulation": {
    "world": {"radius": 1500, "foodCount": 600},
    "broadcastTicks": 6,
    "leaderboardIntervalSeconds": 3.0,
    "latency": {"roundTripSeconds": [0.06], "jitterSeconds": 0.003}
  }
}
```

```sh
cargo run --release --locked -- host --config arena.json
```

`maxClients` reserves remote slots; `bots` adds food-seeking bots. Their combined
limit is 128. The host derives `simulation.agents` and `controlledSlot` from
these counts. Host `recordWire` must be false; enable recording on a client instead.

`--bind IP:PORT` overrides the file. Real sockets already include network and
compute delay; configured latency adds another ordered link. Keep added latency
at zero for normal remote use. Client processing delay is not simulated twice.

## VPS use

Build on the VPS or copy a release executable for its platform:

```sh
slither-sim host --bind 0.0.0.0:8766 --config arena.json
```

The host serves plain `ws`. For `wss`, use a TLS reverse proxy forwarding
WebSocket upgrades on `/slither`; HTTPS browser pages require `wss`.
Game assets are not included.

One nonblocking loop owns the world and bounded connection queues. Slow clients
are disconnected before exhausting output buffers. An overloaded host may slow
world time; it does not skip physics ticks to catch up.

## Packet compatibility

The host uses the native simulator's encoder. Packet layouts follow protocol 19;
world values, compression choices and update schedules can differ from official
sessions. See [rules](server-rules.md) and [verification](verification.md).

| Direction | Supported messages |
| --- | --- |
| Client to host | Login, appearance, headings 0–250, ping 251, keyboard 252, boost 253/254, bounded victory text; framing/session negotiation before login. |
| Host to client | Setup/radius, snakes, motion, body/growth/tail/mass, sector food, prey, minimap, pongs, game-over, leaderboard, kills, record and requested session IDs. |
| Additional messages decoded by `ClientState` | Alternate gameplay encodings, compressed food, own score credits, minimap formats/deltas, team scores and team layers. |
| Not provided by the host | Team gameplay, accounts, persistent daily records, administrative/debug services and unspecified extensions. |

Before login, `[1]` selects no header, `[2]` selects a two-byte sequence counter,
and neither selects two-byte elapsed milliseconds. A second framing request is
rejected. Optional `[4]` requests `S` after self appearance and may accompany
framing in either order; repeated requests are rejected. IDs identify connections,
not authentication or resumed lives. Challenge and direct logins are accepted.

### Reports and game-over

- `l`: top ten living snakes, recipient rank and population, on entry and every
  `leaderboardIntervalSeconds` (default 3; positive and finite).
- `k`: simulator collision-attributed kills.
- `W` then `F`: sector entry, including empty sectors; `w` clears on exit.
  Bot food coverage comes from `F`/`w`, not the browser's `W` display list.
- Scores: stock setup table and quantized mass, with own `R` credits when needed.
  Leaderboard/record credits do not alter opponent geometry; rounding and the
  one-byte credit range limit precision.
- `m`: highest-scoring completed life in this world run; resets with the world.
  A qualifying death sends `v1` for victory-message entry, other deaths `v0`.
  The owning dead connection/life may submit once (including empty text); `v2`
  acknowledges. Slot reuse cannot edit that record.

These ranking, cadence and record rules are local policies, not recovered official
server rules. Keyboard turns use 0.033 times the size turn factor per count;
this remains an approximation. Ignored unknown packets are not decoded features.

### Stock browser client

Official assets are not distributed. Redirect an idle client to the local server;
public discovery does not list it. For the tested HTTP bundle with `forceOnce`,
use its developer console:

```js
if (window.playing || window.connecting || window.connected) {
  throw new Error("Use a fresh idle tab");
}
waiting_for_sos = false;
forceOnce("127.0.0.1", 8766);
connect();
```

Reload to restore normal discovery. Other bundles may need different endpoint
configuration; an HTTPS page needs a connector selecting `wss`.
