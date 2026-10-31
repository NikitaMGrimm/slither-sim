# Protocol-13/19 WebSocket server

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
  "players": { "max": 4, "latency": {} },
  "bots": { "count": 4, "threads": 2, "latency": {} },
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

`--bind IP:PORT` overrides the file. `players.latency` adds link delay to socket
players, useful for emulating a route on loopback; its processing fields must be
zero. `bots.latency` independently models bot links and processing.
`simulation.latency` must stay at its default. See
[`LatencyConfig`](configuration.md#latencyconfig) for ordered delays.

`--stats SECONDS` prints periodic [tick rate, load and population](hosting.md#run-as-a-service)
to stderr; the interval must be positive and may include fractional seconds.

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
With `spawnRadius`, a login keeps its socket and seat while waiting for a safe
body; setup and self appearance arrive on admission. Waiting sockets precede
background bots. One failed 4,096-pose search delays further admission searches
by 14 ms of world time; at most one search runs per tick. Disconnected waiters
are removed. A crowded world can wait indefinitely. Worlds without `spawnRadius`
keep the official fallback and
refusal behavior.
With `sizeToPopulation`, the host arena starts sized for background bots;
reserved, disconnected player seats do not enlarge it. See
[population sizing](configuration.md#population-sizing).
For Rust constructors and the default `Config::official(1)`, see
[world settings](configuration.md#official-and-sandbox).

## VPS use

[Linux hosting](hosting.md) installs a release as a systemd service, with an
optional TLS proxy for `wss`, upgrades and resource numbers. The host itself
serves plain `ws`. Game assets are not included.

The simulation thread owns the world, physics and per-tick encoding. A separate
socket thread owns the listener and nonblocking connections, releasing frames
at latency-model due times between ticks. OS scheduling and load affect delivery.
Newly encoded frames wake an idle socket thread. On Windows a per-host
high-resolution timer also keeps its input polling responsive, without spinning.
Slow clients disconnect before exhausting output buffers. Overload defers
wall-clock ticks and shifts delivery, slowing world time without skipping physics.
Deferred ticks show players a growing ping. For a world that is overloaded or
gets heavier as it ages, set `maxTick` (seconds) instead. Each tick then lasts
the real time since the previous tick started, between `simulation.world.dt` and
`maxTick`; when the work finishes early, the host still waits out the configured
tick. The world keeps real time instead of falling behind, at the cost of uneven
pacing on overloaded ticks. In a public session on 2026-10-31 with 64,000 bots,
a 0.08 s tick and `maxTick` 0.1, ticks measured 80.06 ms at the median and 91 ms
at p99, with no lost time in the measured window. This is one session, not a
capacity guarantee. Work longer than `maxTick` still loses time: `--stats`
reports it as `lost_seconds`, and `RunningHost::lost_seconds()` returns the
total. Motion and mass reports follow elapsed time, even on capped ticks.
Protocol 19 frames carry each tick's length. Protocol 13 frames don't, so NTL
shows stretched ticks as slightly faster motion. Without `maxTick` the host
keeps fixed ticks and identical output.
Remote output queues in both the simulation and socket thread each allow 2 MiB,
charging payload bytes plus a 64-byte allowance per message. Exceeding this
host resource policy closes only that connection, for either protocol; legacy
single messages do not share the virtual client's 4,096-frame limit. Input
commands have their own separate bound.

`bots.threads` runs background Forager clients using A4-mean. Each
decodes its slot's frames, decides every `decisionTicks × dt` with staggered phases, models processing delay,
pings and rejoins after death. Each worker shares one stateless policy instance;
its seats keep separate decoders, outboxes and sent histories. See [worker limits](configuration.md#hostconfig)
and [decision timing](configuration.md#timing-and-ping).
An unadmitted background bot waits one wall second after its connection is
refused before retrying, a host policy that limits crowded-arena admission work.
Socket players still choose when to retry their own joins.

## Embedding and load checks

`Host::bind(config)?.start()?` consumes the bound host and returns `RunningHost`.
Keep the handle alive while serving.

| `RunningHost` method | Meaning |
| --- | --- |
| `local_addr()` | Bound socket address. |
| `ticks()` / `deferred_ticks()` | Executed physics ticks / wall-clock ticks postponed under load. |
| `lost_seconds()` | Wall seconds discarded above `maxTick` (zero in fixed mode). |
| `simulation_seconds()` | Wall seconds spent running simulation ticks; divide its increase by elapsed wall time for simulation-thread load. |
| `players()` | Connected socket players, excluding background bots. |
| `is_finished()` | Whether a host thread has stopped; call `wait()` for its result. |
| `wait()` | Consume the handle and block until a host thread fails; return its error. |
| `stop()` | Consume the handle, stop and join all threads; report any earlier failure. |

Dropping `RunningHost` also stops and joins its threads. `Host::run()` is
`start()` followed by `wait()`.

From a checkout, the
[`host_load` example](https://github.com/NikitaMGrimm/slither-sim/blob/main/examples/host_load.rs)
reports the selected policy and frozen parameter-manifest hash, and measures
simulation-thread load, deferred ticks and loopback watcher excess delay:

```sh
cargo run --release --locked --features networking --example host_load -- --bots 500 --seconds 60
```

Options include `--bot-threads` (default 2) and `--round-trip-ms` (default 40,
without jitter). Excess delay is receipt time minus server time, relative to
the minimum in each 1,200-frame chunk; it is not absolute one-way latency.
The [tail-jitter study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-tail-jitter/REPORT.md)
records exploratory results from one shared Windows desktop.

## Packet compatibility

Stock clients and bots use protocol 19. A legacy login selects protocol 13
for that connection in the same world; world values, compression and schedules
can differ from official sessions. See
[mechanics and delivery limits](server-rules.md), [presentation](#presentation) and
[protocol sources](provenance.md#protocol-references).

| Direction | Supported messages |
| --- | --- |
| Client to host | Login, appearance, headings 0–250, ping 251, keyboard 252, boost 253/254, bounded victory text; framing/session negotiation before login. |
| Legacy login | `[115, selector, skin, nameLength, name…, 0, reserved, custom skin…]`, selector 11 or 12, name up to 24 bytes and custom skin up to 255 bytes; selects protocol 13. Stock login offsets and protocol-19 output are unchanged. |
| Short modern login | `[115, 31, versionHi, versionLo, skin, nameLength, name…, 0, reserved, custom skin…]`, same name/custom limits; selects protocol 19. This is a client-derived admission layout, not protocol-31 output. |
| Host to client | Setup/radius, snakes, motion, body/growth/tail/mass, sector food, prey, minimap, pongs, game-over, leaderboard, kills, record and requested session IDs. |
| Additional messages decoded by `ClientState` | Alternate gameplay encodings, compressed food, own score credits, minimap formats/deltas, team scores and team layers. |
| Not provided by the host | Team gameplay, accounts, persistent daily records, administrative/debug services and unspecified extensions. |

Before login, `[1]` selects no header, `[2]` selects a two-byte sequence counter,
and neither selects two-byte elapsed milliseconds. A second framing request is
rejected. Optional `[4]` requests `S` after self appearance and may accompany
framing in either order; repeated requests are rejected. IDs identify connections,
not authentication or resumed lives. Challenge and direct logins are accepted.
Stock `[99,0]` keeps its existing challenge. Legacy `[99]`, `[99,3]` and
`[99,4]` (optionally after `[4]`) receive the same encoded single-quoted 24-letter assignment, with the
first letter at source offset 7 and a statement filling the caller's `idba`
buffer. They accept its rotated 24-byte reply. This compatibility choice serves
some browser mods;
the [official catalog](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-protocol13-catalog/REPORT.md)
observed the stock 27-byte challenge even for that request. Neither is authentication.

Protocol 13 sends individual messages with elapsed headers by default, explicit
snake IDs and coordinate-based body/food updates. Both protocols normally use
the shared live-radius `M`/`V` minimaps. NTL keeps its dynamic own-player marker
in that projection; sending `U` would switch the marker to setup `grd` instead.
This applies to fixed and changing radii and to the Battledome pit raster.

Only legacy `[99,3]` requests a full-server map: that connection receives `U`
snapshots with occupied live-raster cells reprojected into the `grd` frame.
This path has plain zero runs and no client delta base; it also tracks `z`.
Clients that only read `M`, such as Precise, now show full snapshots but still
ignore `V` deltas. Client-derived projection and asymmetric orientation checks
are recorded in the
[minimap correction note](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-27-legacy-map-motion/REPORT.md).
The first body update after appearance uses an absolute coordinate to establish
the integer base; subsequent ordinary steps may use relative coordinates.

### SlitherSim rules extension

This is a SlitherSim extension, not official protocol. A socket opts in with the
HTTP upgrade header `X-SlitherSim-Food: physical`. `NetworkBackend` sends it
automatically on every connection. Reusing the existing bot signal keeps one
handshake choice for physical food and mode metadata. The server tracks rule
delivery separately from decoration state; stock browser sockets never receive
this message. In-process clients also opt in.

After setup, in the same frame, the host sends the following 24-byte `~` packet.
It sends another whenever any rule value changes, including subunit pit changes
as the wall moves. Values have the same meanings as geometric frame
[`rules`](bot-api.md#frames). These reports do not include individual kill scores.
The exact pit is independent of the integer-rounded wall report.

| Byte offsets | Encoding | Meaning |
| --- | --- | --- |
| 0 | u8 | Command `~` (126). |
| 1 | u8 | Extension version, currently 1. |
| 2 | u8 | Boundary mode: 0 sliding, 1 hard. |
| 3 | u8 | Flags: bit 0 pit present, bit 1 floor present, bit 2 kill window present, bit 3 free boost at floor; other bits zero. |
| 4–11 | big-endian IEEE 754 f64 | Live pit radius in world units; zero when absent. |
| 12–15 | big-endian u32 | `minScore`; zero when absent. |
| 16–23 | big-endian IEEE 754 f64 | `killScoreSeconds`; zero when absent. |

Present floating-point values must be finite and positive. A present floor must
be in the configured `minScore` range; free boost at floor must match its
presence. The decoder rejects invalid size, version, flags or values before
applying any packet in the frame. Reconnect clears received rules to unknown.

The pinned stock client has no `~` handler and its browser WebSocket constructor
sends no custom header. An offline check of its actual packet handler also
confirms an accidentally supplied `~` packet only increments traffic accounting:
[source pin and reproduction](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/battledome/2026-10-27-rules-extension/REPORT.md).
This source check is separate from the regression proving that the host excludes
stock sockets.

### Reports and game-over

- `l`: top ten, recipient rank and population, after the recipient's own snake
  appears; see [report clocks](server-rules.md#packet-timing). It ranks living
  snakes by size, or, with `killScoreSeconds`, living seats and recently scoring
  dead or disconnected seats by rolling kills ([Battledome](configuration.md#battledome)).
- `k`: simulator collision-attributed kills.
- `F`/`w` and birth/pickup deltas: [sector food delivery](server-rules.md#food-sector-delivery).
- `M`/`V` (19 and default 13), full-server `U` (13 after `[99,3]`): [global minimap](server-rules.md#client-delivery-and-spawning), with
  [per-connection encoding](server-rules.md#minimap-encoding). The stock client
  draws its own player marker; set resolution with `simulation.minimapSide`
  (`null`: 256 for pit worlds, 136 otherwise).
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

In a world with a pit, stock clients also receive its edge as decorative
pellets. Shrinking rings overlap for 0.5 simulation seconds while rapid `F`
additions fade in; growth replaces the ring immediately. The short outward trail
is decorative and does not move the lethal edge. A marker hides any real pellet
at the same wire position, and pickups never remove a marker. Socket bots that
want only real food send the WebSocket
upgrade header `X-SlitherSim-Food: physical`; portable bots do this
automatically. In-process clients and host bots always get real food. See
[pit presentation](configuration.md#battledome).

Worlds with `killScoreSeconds` use exact rolling kill counts for the stock
leaderboard number column and own rank. They include living seats plus dead or
disconnected seats with unexpired kills; population in that packet is the ranked
seat count. Ties use slot order. A newly admitted socket connection clears the
prior occupant's kills; same-connection respawns and bot lives retain theirs.
Displayed kill counts saturate at the stock table maximum; ranks use actual kills.
Own length and local records still show size; stock labels are unchanged. Pit
minimaps show the pit outline and inner snake cells, using the live wall projection
so the stock own-player marker stays aligned, including outside the pit.

Legacy browser connections receive the same decoration, kill leaderboard and
pit minimap through their own wire layouts.

Some browser mods can connect by entering the host's `IP:PORT` in their custom
server field. The host serves plain `ws`; use an HTTP page for clients that
only open plain WebSockets.

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
