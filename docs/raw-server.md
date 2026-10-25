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

The 1,500-unit arena and eight slots keep this example small. Food settings scale
the existing provisional ecology estimate: about 7 pellets and 0.1024 placement
attempts/s per 300×300 sector. `dt` is the measured 7 ms tick. This is not an
official server preset; six prey is an explicit local choice to exercise prey
gameplay, not a measured global population. See [rules and limits](server-rules.md).

```sh
cargo run --release --locked -- host --config arena.json
```

`maxClients` reserves remote slots; `bots` adds food-seeking bots. Together they
may number 65,535, which the `u16` snake IDs, leaderboard rank and population
fields can count. The host derives `simulation.agents`
and `controlledSlot` from these counts. `slither-sim host` prints warnings about
valid but unmeasured settings to stderr; see [limits](configuration.md#limits). Host `recordWire` must be false; enable recording on a client instead.

`--bind IP:PORT` overrides the file. Real sockets already include network and
compute delay; configured latency adds another ordered link. Keep added latency
at zero for normal remote use; on loopback, the
[measured route](configuration.md#variation-and-pauses) gives every connection,
bots included, an official-like path. Client processing delay is not simulated
twice.

### Presets

`simulation.world` accepts `"preset": "official"`, `"sandbox"` or `"battledome"`;
explicit fields override the preset's values. Omitted settings without a preset
use the documented defaults. See [presets](configuration.md#presets) and
[Battledome](configuration.md#battledome).

Native and remote clients share finite snake/prey coverage (`world.vision`, default
1,500). Snakes remain visible while any centerline segment is covered. Food uses
a separate 45-sector entry stencil with heading lead and temporal retention.
These are approximations with [measured constraints and known gaps](server-rules.md);
the browser does not send its viewport or zoom to this host.

Human joins and hosted bot lives, including startup, request 600-unit clearance
between the complete newborn body and living heads/bodies. Crowded admission
waits or fails without near-contact fallback. This conservative model is not an
identified official exclusion rule.

The host world defaults to `Config::official(1)`; Rust callers can choose
`Config::official(n)`, `Config::sandbox()` or `Config::battledome()`. Neither is an exact official server
configuration; see [official and sandbox](configuration.md#official-and-sandbox).

## VPS use

Build on the VPS or copy a release executable for its platform:

```sh
slither-sim host --bind 0.0.0.0:8766 --config arena.json
```

The host serves plain `ws`. For `wss`, use a TLS reverse proxy forwarding
WebSocket upgrades on `/slither`; HTTPS browser pages require `wss`.
Game assets are not included.

The simulation thread owns the world, runs physics and encodes updates every
tick. A separate socket thread owns the listener and nonblocking connections;
it sends each frame at its latency-model delivery time, without waiting for the
next physics tick. Actual delivery also depends on OS scheduling and host load.
Slow clients are disconnected before exhausting output buffers. An overloaded
host defers wall-clock ticks and shifts the shared delivery schedule; it slows
world time without skipping physics.

`botThreads` (default 2) runs background seekers off the simulation thread.
Each bot decodes its own slot's frames, decides every `decisionTicks × dt` with
staggered phases, models processing delay, pings and rejoins after death.
At least one worker is required when `bots > 0`; the host starts no more workers
than bots. Socket clients choose their own decision intervals.

## Embedding and load checks

`Host::bind(config)?.start()?` consumes the bound host and returns `RunningHost`.
Keep that handle alive while serving. `Host::poll()` has been removed.

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

[Presentation](#presentation) describes pellet colors and their evidence limits.

The host uses the native simulator's encoder. Packet layouts follow protocol 19;
world values, compression choices and update schedules can differ from official
sessions. See [mechanics and delivery limits](server-rules.md) and
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

- `l`: top ten living snakes, recipient rank and population, on a server-wide
  clock of `leaderboardIntervalSeconds` (default 3.008) after the recipient's
  own snake appears.
- `k`: simulator collision-attributed kills.
- `F`: nonempty sector listings on entry; empty subscriptions stay internal and
  send neither `W` nor an empty `F`. Births/pickups update subscribed food;
  `w` clears on exit. See [food delivery](server-rules.md#food-sector-delivery).
- `M`/`V`: a global minimap (136×136 by default, configurable via
  `simulation.minimapSide`) on its own server-wide clock (default 2.504 s),
  first in its frame.
  First, reconnected and resized maps use full `M`; later maps use XOR `V` only
  when strictly shorter than `M`, against that connection's last queued bitmap.
  Living heads and body centerlines are rasterized; the stock client draws its
  own player marker. This compression choice is local; exact official pixel
  filtering and full/delta selection remain unknown. See
  [minimap encoding](server-rules.md#minimap-encoding).
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
- Opponent and food updates continue around the dead player's final position;
  prey retain their periodic reporting clock. Game-over and own removal are sent
  once; minimap updates stop. Otherwise the browser extrapolates stale opponents
  through the fade. The native bot frame stays dead with no playable observation.

The stock browser owns its death fade and return-to-play timing. The host keeps
streaming and answering pings until the client closes, including during victory
entry. It sets no fixed menu timer. Delivery remains subject to configured tick
cadence, link delays and host load.

These ranking and record rules are local policies; packet cadences use a
[partial model with measured constraints](server-rules.md#packet-timing).
Keyboard turns use 0.033 times the size turn factor per count;
this remains an approximation. Ignored unknown packets are not decoded features.

## Presentation

Pellet colors are assigned by a separate seeded presentation RNG, outside physics
and bot observations. Colors persist across source death, reconnects and sector
re-entry. The [presentation fixture](../tests/fixtures/client-presentation.json)
retains pooled ambient weights and stock/custom recipes; the current model limits
are in [mechanics and delivery](server-rules.md#client-delivery-and-spawning).
Primary skins 0–8 select matching boost/death colors; unknown skins use an
approximate modulo-9 fallback. Birth/pickup telemetry supplies colors and eater
IDs without guessing from proximity. The wire exposes the first co-located pellet;
physics consumes co-located pellets in insertion order before ambient refill.

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
