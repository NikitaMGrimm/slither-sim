# Portable bots: slither-client-v2

The simulator and WebSocket backends hand bots the same `ClientFrame`, decoded
by the same Rust code, so one bot runs on both unchanged. See the
[Rust](../examples/portable_bot.rs) and [Python](../examples/portable_bot.py)
examples, and the [cautious bot](../examples/cautious_bot.rs), which also avoids
walls and snakes.

## Interface versions

Interfaces are experimental; only the current version of each interface is
supported, without translation. Check the envelope and frame contract before acting.

| Identifier | Boundary |
| --- | --- |
| Protocol `19` | Binary WebSocket packets and commands. |
| `slither-client-v2` | Bot frame fields, units and lifecycle. |
| `slither-client-json-v2` | The `client` JSON requests and reply envelope. |
| `slither-sim-json-v3` | The separate geometric [`serve` controller](bot-api.md). |
| `slither-sim-abi-v2` | The in-process [C ABI](c-api.md) over `serve` sessions. |
| Package version | Rust API and simulator implementation; also pin the commit for reproducibility. |

`info` reports `version`, `contract`, `wireProtocol: 19` and
`stability: "experimental"` before a backend starts. Responses allow additional
fields; requests, actions and configs reject unknown fields.

Changes to types, units, coordinates, nullability, status values, action mapping,
receipt meanings or resets require a new affected contract ID and updated
examples. Physics can change trajectories without changing that contract.
The [JSON fixture](https://github.com/NikitaMGrimm/slither-sim/blob/main/tests/fixtures/client-contract-v2.json)
checks the current shape independently of Rust structs.

## Rust

Implement `client::Bot::decide(&ClientFrame) -> Option<Action>` and `reset()` for
stateful bots. Run with `BotRunner` and `SimulatorBackend` or `NetworkBackend`.
The runner checks the contract, decides and sends actions only while playing,
measures compute time, and resets bot state on connection/life changes or play
ending. Reconnection is explicit.

Headings are absolute radians: zero is +x, pi/2 is +y. Both backends quantize them
to bytes 0–250, with separate boost-on/off commands.

## Python, JavaScript and other languages

Run `slither-sim client` and exchange one JSON object per stdin/stdout line:

```json
{"protocol":"slither-client-json-v2","result":{}}
{"protocol":"slither-client-json-v2","error":"Reset first"}
```

Start and step a virtual-time world:

```json
{"op":"simulation","seed":7}
{"op":"step"}
{"op":"step","action":{"heading":1.5,"boost":false},"processingSeconds":0.003}
```

For a WebSocket connection:

```json
{"op":"network","config":{"url":"ws://127.0.0.1:8766/slither","nickname":"MyBot"}}
{"op":"step"}
```

| Request | Effect |
| --- | --- |
| `step` | Advances virtual time or polls the network; returns `{"backend":"simulation","frame":...}` or `"network"`. |
| `observe` | Returns the current frame without processing messages. |
| `reconnect` | Starts a new connection/session. |
| `info` | Reports capabilities and defaults. |
| `wire` | Drains opt-in wire records; errors when recording is disabled. |

Wait for `result.frame.status == "playing"` before acting; latency may require
several empty steps. Omit actions to hold the last command. Poll networks
regularly to avoid stale input. Backend replacement requires successful construction.

The decoder retains at most 4,096 snake slots, 4,096 body anchors per snake,
65,536 body anchors in total (including removed records), and 131,072 food IDs.
Updates exceeding these limits fail before changing the frame's received state;
network backends disconnect on that error. Reconnect clears retained entities.

Minimap decoding has a fresh budget of 1,310,720 logical cells per frame, bounding
full-grid work even for empty snapshots or deltas. It admits four 512×512 team
layers plus their union; exceeding it rejects the whole frame before committing
received state. M, V and U each charge `side²`; legacy u charges `80²`; L charges
`(teamCount + 1) × side²`, with a separate 1,048,576-cell limit across team layers.

`processingSeconds` is finite, nonnegative measured bot compute time, replacing
the configured sample even without an action. For networks it is metadata:
computation has already elapsed. Invalid actions reject it before recording;
later I/O or record-buffer errors cannot roll back sent bytes or advanced time.

## What the bot knows

`ClientFrame.time` is local monotonic time. `observation.time` follows received
server elapsed time; header-free connections use local receipt time since setup.
Coordinates are centered at the arena origin, with body anchors head to tail.
Observations appear after self's full appearance and are absent after death.

Self is the first full snake appearance on a fresh connection; neither the
setup's server number nor a mid-life capture establishes self.

Entities come only from messages. `received` tracks independent ages, validity
and prediction flags for appearance, position, heading, wanted heading, speed,
body and mass, plus category completeness and control histories. Empty lists do
not imply complete coverage; polling never refreshes receipts.
Food origins and unreported opponent score credits are unavailable.

`timing` holds game-ping round trips, their recent range, pending ping age,
processing time and actually sent commands. Missing measurements are `null`;
true one-way delays and future delivery times are not exposed. Reconnect clears
entities, compression state, pings, commands and HUD reports. Reset bot state
when connection generation or life changes.

### HUD and settings

HUD values are server reports and may include players outside local visibility.

| Field | Meaning |
| --- | --- |
| `hud.leaderboard` | Own rank/row, population and entries with names, colors, mass and computed scores. |
| `hud.record` | Reported record holder's mass, score, name and message. |
| `hud.session` | Opaque snake/session IDs; these neither select self nor resume a life. |
| `hud.snakes` | Known profiles, custom skins, cosmetics and optional kill reports with receipt ages. |
| `hud.removals` | Latest 64 known-snake removals and death flags when sent. |
| `hud.teamScores` | Team scores and up to 4,096 history pairs. |
| `hud.minimapTeams` | Team layers from `L`; `minimap` starts from their union and can receive deltas. |

An unknown kill count differs from a reported zero; observations use zero until
a report arrives, so consult the profile to distinguish them. Leaderboard and
record scores use the setup score table and retain transmitted segment/fraction
values. They do not overwrite opponents' mass-derived scores; own `R` credits
are tracked separately.

`settings` retains server number, optional `gameMode`/`teamVal` and wire constants
`sectorCountAlongEdge`, `spangdv`, `nsp1`, `nsp2`, `nsp3`, `mamu` and `mamu2`.
These reports do not alter simulator physics.

## Compatibility and limits

Network config accepts an explicit `ws://` or `wss://` URL, optional `origin`,
Latin-1 nickname (24 characters), skin byte, `customSkin` (255 bytes), timeouts,
decision interval and `recordWire`. `requestSessionId: true` sends `[4]` before
login; it defaults off. IDs are not authentication or life resumption.
The default `handshake: "challenge"` uses the supported login exchange;
`"direct"` sends login immediately.

Challenges accept a bounded string-copy template, never executable JavaScript;
other syntax fails explicitly. TLS verifies certificates. A bare initial challenge
selects header-free framing, including bundled packets. Body contraction outside
0–1 is rejected. Matching protocol numbers alone does not ensure compatibility.

### Simulation

See [configuration](configuration.md) for world, simulation, latency and network
fields, defaults and limits, and [packet timing](server-rules.md#packet-timing)
for per-family release rules.

For the custom [Battledome mode](configuration.md#battledome), put the preset
inside `config.world`:

```json
{"op":"simulation","seed":7,"config":{"world":{"preset":"battledome"}}}
```

The frame still contains only decoded protocol reports. Pit geometry, the size
floor and the rolling kill score are not transmitted; retain the chosen world
config separately when a bot needs the mode rules. Reported kills count the
current life, and leaderboard ranks use size scores.

Example [latency settings](configuration.md#latencyconfig):

```json
{"op":"simulation","seed":7,"config":{"agents":4,"latency":{"roundTripSeconds":[0.04,0.12],"roundTripWeights":[3,1],"variation":{"medianSeconds":0.002,"logSigma":0.5},"processingSeconds":[0.005,0.02]}}}
```

The same build, seed, config, actions and processing durations reproduce virtual
time. Retain measured durations when using `BotRunner`. The controlled decision
finishes processing before the next decision frame. Multi-agent callers use
`step_all`, `send_to`, `decision_pending` and `frame_for`; busy slots reject new
actions. Reconnect starts a new life at a clear spawn while other snakes continue.

Visibility, minimap rasterization and body presentation are approximations;
reconstruction differs from stock renderer smoothing. See [mechanics and delivery
limits](server-rules.md). The native backend delivers protocol bytes in-process;
use the [host](raw-server.md) for real sockets.

### Recording and network bounds

`recordWire` defaults off. Drain regularly: recording fails at 32 MiB, with the
network backend retaining the final incoming message even if it crosses that
limit. Records hold application payloads and local send/receipt times, not TCP
frames. `replay_record` also needs separately retained processing durations to
reproduce processing metadata. Existing capture bytes can be replayed into v2.

Network messages are capped at 1 MiB. OS DNS resolution has no separate deadline
from the resolver; socket timeouts do not bound it. Switching backends preserves
inputs and action mapping, not official physics or equal playing strength.
