# Protocol-19 WebSocket server

`slither-sim host` serves a shared world at `ws://127.0.0.1:8766/slither` for
custom servers and socket-based bots. For virtual time without sockets, use
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

The [configuration reference](configuration.md) owns host/world defaults,
limits and [timing and ping](configuration.md#timing-and-ping).

Save as `arena.json`:

```json
{
  "bind": "127.0.0.1:8766",
  "seed": 7,
  "maxClients": 4,
  "bots": 4,
  "botThreads": 2,
  "simulation": {
    "world": {
      "radius": 1500,
      "dynamicRadius": false,
      "foodCount": 550,
      "ambientReplenishPerSecond": 8.04,
      "dt": 0.007,
      "prey": { "count": 6 }
    },
    "leaderboardIntervalSeconds": 3.008
  }
}
```

This small 1,500-unit, eight-slot example scales the
[provisional food model](configuration.md#official-and-sandbox) and uses the
measured 7 ms tick. Six prey is a local gameplay choice, not a measured global
population or an official preset.

```sh
cargo run --release --locked -- host --config arena.json
```

[`HostConfig`](configuration.md#hostconfig) defines reserved remote slots,
background bots and derived simulation fields. Valid but unmeasured settings
warn on stderr; see [limits](configuration.md#limits). Record on a client;
host `recordWire` must be false.

`--bind IP:PORT` overrides the file. Leave added latency at zero for normal
remote use. On loopback, the [measured route](configuration.md#variation-and-pauses)
models an official-like path for every connection, including bots. See
[`LatencyConfig`](configuration.md#latencyconfig) for ordered delays and
processing, which is not simulated twice for remote clients.

### Presets

Select a world with `simulation.world.preset`; see
[preset names, overrides and defaults](configuration.md#presets).

For Battledome, save this minimal host config and pass it to `host --config`:

```json
{"simulation":{"world":{"preset":"battledome"}}}
```

This keeps default remote slots and bots. See
[Battledome rules and bot visibility](configuration.md#battledome) for its custom
mechanics and stock-browser limits.

Native and remote clients share the
[coverage and spawning models](server-rules.md#client-delivery-and-spawning),
including hosted bot startup. The browser sends no viewport or zoom to the host.
For Rust constructors and the default `Config::official(1)`, see
[world settings](configuration.md#official-and-sandbox).

## VPS use

Build on the VPS or copy a release executable for its platform:

```sh
slither-sim host --bind 0.0.0.0:8766 --config arena.json
```

The host serves plain `ws`. For `wss`, use a TLS reverse proxy forwarding
WebSocket upgrades on `/slither`; HTTPS browser pages require `wss`.
Game assets are not included.

The simulation thread owns the world, physics and per-tick encoding. A separate
socket thread owns the listener and nonblocking connections, releasing frames
at latency-model due times between ticks. OS scheduling and load affect delivery.
Slow clients disconnect before exhausting output buffers. Overload defers
wall-clock ticks and shifts delivery, slowing world time without skipping physics.

`botThreads` runs background seekers separately. Each decodes its slot's frames,
decides every `decisionTicks × dt` with staggered phases, models processing delay,
pings and rejoins after death. See [worker limits](configuration.md#hostconfig)
and [decision timing](configuration.md#timing-and-ping).

## Embedding and load checks

`Host::bind(config)?.start()?` consumes the bound host and returns `RunningHost`.
Keep the handle alive while serving.

| `RunningHost` method | Meaning |
| --- | --- |
| `local_addr()` | Bound socket address. |
| `ticks()` / `deferred_ticks()` | Executed physics ticks / wall-clock ticks postponed under load. |
| `simulation_seconds()` | Wall seconds spent running simulation ticks; divide its increase by elapsed wall time for simulation-thread load. |
| `players()` | Connected socket players, excluding background bots. |
| `wait()` | Consume the handle and block until a host thread fails; return its error. |
| `stop()` | Consume the handle, stop and join all threads; report any earlier failure. |

Dropping `RunningHost` also stops and joins its threads. `Host::run()` is
`start()` followed by `wait()`.

From a checkout, the
[`host_load` example](https://github.com/NikitaMGrimm/slither-sim/blob/main/examples/host_load.rs)
measures simulation-thread load, deferred ticks and loopback watcher excess delay:

```sh
cargo run --release --locked --features networking --example host_load -- --bots 500 --seconds 60
```

Options include `--bot-threads` (default 2) and `--round-trip-ms` (default 40,
without jitter). Excess delay is receipt time minus server time, relative to
the minimum in each 1,200-frame chunk; it is not absolute one-way latency.
The [tail-jitter study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-tail-jitter/REPORT.md)
records exploratory results from one shared Windows desktop.

## Packet compatibility

The native simulator's encoder uses protocol-19 layouts; world values,
compression and schedules can differ from official sessions. See
[mechanics and delivery limits](server-rules.md), [presentation](#presentation) and
[protocol sources](provenance.md#protocol-references).

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

- `l`: top ten living snakes, recipient rank and population, after the recipient's
  own snake appears; see [report clocks](server-rules.md#packet-timing).
- `k`: simulator collision-attributed kills.
- `F`/`w` and birth/pickup deltas: [sector food delivery](server-rules.md#food-sector-delivery).
- `M`/`V`: [global minimap](server-rules.md#client-delivery-and-spawning), with
  [per-connection encoding](server-rules.md#minimap-encoding). The stock client
  draws its own player marker; set resolution with `simulation.minimapSide`.
- Scores: stock setup table and quantized mass, with own `R` credits when needed.
  Leaderboard/record credits do not alter opponent geometry; rounding and the
  one-byte credit range limit precision.
- With `simulation.localRecords: true` (default false), `m` shows the highest-scoring
  completed life in this world run, resetting with the world. Official
  qualification, persistence and reset rules remain unknown.
  A qualifying death sends `v1` for victory-message entry, other deaths `v0`.
  The owning dead connection/life may submit once (including empty text); `v2`
  acknowledges. Slot reuse cannot edit that record.
- Dead sessions answer pings, including during victory entry, but ignore gameplay
  controls. The stock browser pings through its fade; withholding replies triggers
  its lag detector, slowing Play Again. This is a local compatibility policy.
- Opponent and food updates continue around the dead player's final position;
  prey retain their periodic reporting clock. Game-over and own removal are sent
  once; minimap updates stop. Otherwise the browser extrapolates stale opponents
  through the fade. The native bot frame stays dead with no playable observation.

The stock browser owns death-fade and return-to-play timing; the host has no menu
timer and streams until closure. Tick cadence, link delays and load affect delivery.

These ranking and record rules are local policies; packet cadences use a
[partial model with measured constraints](server-rules.md#packet-timing).
Keyboard turns use 0.033 times the size turn factor per count;
this remains an approximation. Ignored unknown packets are not decoded features.

## Presentation

See [pellet presentation and evidence limits](server-rules.md#client-delivery-and-spawning)
and the [presentation fixture](../tests/fixtures/client-presentation.json).
Colors are outside physics and bot observations, persisting across source death,
reconnects and sector re-entry. Birth/pickup telemetry supplies colors and eater
IDs. The wire exposes the first co-located pellet; physics consumes co-located
pellets newest first, before ambient refill.

## Stock browser client

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
