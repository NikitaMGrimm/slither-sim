# Protocol-19 WebSocket server

`slither-sim host` serves one shared world at `ws://127.0.0.1:8766/slither`.
Use it for custom servers and socket-based bots. For virtual-time runs without
socket overhead, use `SimulatorBackend` through the [portable API](client-api.md).

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

The [configuration reference](configuration.md) lists every host/world setting,
default and limit, including [timing and ping](configuration.md#timing-and-ping).

Save as `arena.json`:

```json
{
  "bind": "127.0.0.1:8766",
  "seed": 7,
  "maxClients": 4,
  "bots": 4,
  "simulation": {
    "world": {
      "radius": 1500,
      "foodCount": 550,
      "ambientReplenishPerSecond": 8.04,
      "dt": 0.007
    },
    "broadcastTicks": 6,
    "leaderboardIntervalSeconds": 3.0
  }
}
```

The 1,500-unit arena and eight slots keep this example small. Food settings scale
the existing provisional ecology estimate: about 7 pellets and 0.1024 placement
attempts/s per 300×300 sector. `dt` is the measured 7 ms tick. This is not an
official server preset; see [rules and limits](server-rules.md).

```sh
cargo run --release --locked -- host --config arena.json
```

`maxClients` reserves remote slots; `bots` adds food-seeking bots. Their combined
limit is 128. The host derives `simulation.agents` and `controlledSlot` from
these counts. Host `recordWire` must be false; enable recording on a client instead.

`--bind IP:PORT` overrides the file. Real sockets already include network and
compute delay; configured latency adds another ordered link. Keep added latency
at zero for normal remote use. Client processing delay is not simulated twice.

### Presets

There are no named host/JSON presets; omitted settings use the compact defaults.
Remote clients receive all living snakes so native bot coverage cannot clip the
stock browser's viewport. This is a compact-host policy, not a measured official
subscription envelope; larger worlds incur more snake traffic. Food sectors and
prey keep `world.vision` coverage, as do all native bot observations. The browser
does not send its viewport or zoom to this host.

Rust callers can use `Config::default()` or `Config::realistic(n)`. The latter
scales radius and ecology with a provisional population model, rather than
reproducing an official server configuration.

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

[Client presentation](presentation.md) describes source-linked pellet colors,
their retained evidence and remaining uncertainty.

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
- With `simulation.localRecords: true`, `m` shows the highest-scoring completed
  life in this world run; resets with the world. This feature is off by default:
  official qualification, persistence and reset rules have not been established.
  A qualifying death sends `v1` for victory-message entry, other deaths `v0`.
  The owning dead connection/life may submit once (including empty text); `v2`
  acknowledges. Slot reuse cannot edit that record.
- An open dead session still answers pings, including during victory entry.
  The stock browser keeps pinging through its death fade; withholding replies
  triggers its lag detector and slows the return to Play Again. Gameplay
  controls remain ignored after game over. This is a local compatibility policy.
- Opponent, food and prey updates continue at the existing broadcast cadence
  around the dead player's final position. Game-over and own removal are sent
  once; minimap updates stop. Otherwise the browser extrapolates stale opponents
  through the fade. The native bot frame stays dead with no playable observation.

The tested stock browser owns the death timing: a 1,600 ms hold followed by a
fade subtracting `.004 * min(5, elapsed_ms / 8) * lag_mult` per rendered update.
With frequent frames and no lag, the fade takes about 2 seconds. The host keeps
streaming through this transition; it does not replace it with a fixed menu timer.
Retained official observations reported post-death world traffic and three
transport closes at 6.775, 6.861 and 7.124 seconds. Those are limited receipts,
not a universal timeout; this host leaves the connection open until the client
closes it, including during victory entry. Broadcast and delivery times remain
subject to the configured tick cadence, link delays and host load.

These ranking, cadence and record rules are local policies, not recovered official
server rules. Keyboard turns use 0.033 times the size turn factor per count;
this remains an approximation. Ignored unknown packets are not decoded features.

### Stock browser client

Official assets are not distributed. Redirect an idle client to the local server;
public discovery does not list it. For the tested HTTP bundle,
use its developer console:

```js
if (window.playing || window.connecting || window.connected) {
  throw new Error("Use a fresh idle tab");
}
waiting_for_sos = false;
// Keep a local discovery entry for Play Again; forceOnce is cleared on setup.
sos = [{ip: "127.0.0.1", po: 8766, ac: 999, active: true, sid: 1}];
clus = [];
loadSos = function () {}; // Ignore late public discovery responses until reload.
forcing = false;
fobso = null;
connect();
```

Reload to restore normal discovery. Other bundles may need different endpoint
configuration; an HTTPS page needs a connector selecting `wss`.
