# Portable bots: slither-client-v2

The simulator and WebSocket backends feed the same Rust decoder and return
`ClientFrame`. A bot can switch backends while keeping its inputs, action mapping
and state handling. See the [Rust](../examples/portable_bot.rs) and
[Python](../examples/portable_bot.py) examples.

## Interface versions

Interfaces are experimental, with one supported frame contract and no translation
of older versions. Check the envelope and frame contract before acting.

| Identifier | Boundary |
| --- | --- |
| Protocol `19` | Binary WebSocket packets and commands. |
| `slither-client-v2` | Bot frame fields, units and lifecycle. |
| `slither-client-json-v1` | The `client` JSON requests and reply envelope. |
| `slither-sim-json-v3` | The separate geometric [`serve` controller](bot-api.md). |
| `slither-sim-abi-v2` | The in-process [C ABI](c-api.md) over `serve` sessions. |
| Package version | Rust API and simulator implementation; also pin the commit for reproducibility. |

`info` reports `version`, `contract`, `wireProtocol: 19` and
`stability: "experimental"` before a backend starts. Responses allow additional
fields; requests, actions and configs reject unknown fields.

Changes to types, units, coordinates, nullability, status values, action mapping,
receipt meanings or resets require a new affected contract ID and updated
examples. Physics can change trajectories without changing that contract.
The [JSON fixture](https://github.com/NikitaMGrimm/slither-sim/blob/main/tests/fixtures/client-contract-v2.json) checks the current
shape independently of Rust structs; it does not freeze this preview permanently.

## Rust

Implement `client::Bot::decide(&ClientFrame) -> Option<Action>` and `reset()` for
stateful bots. Run with `BotRunner` and `SimulatorBackend` or `NetworkBackend`.
The runner checks the contract, decides only while playing, measures compute time
and sends actions. It resets bot state on connection/life changes and when play
ends. Reconnection is explicit.

Headings are absolute radians: zero is +x, pi/2 is +y. Both backends quantize them
to bytes 0–250, with separate boost-on/off commands.

### Reference policy

`client::ReferenceBot` is a stateless food seeker with short-horizon collision
avoidance. It reselects food using distance plus turn cost, evaluates 13 headings
over four turn-limited samples spanning 0.36 seconds, and penalizes nearby walls,
heads and body edges. Decisions allocate no heap memory and never boost. Reset
is a no-op; the adapter returns no action for non-playing/incompatible frames or
an absent observation. The host's existing background seekers are unchanged.

```rust
use slither_sim::client::{BotRunner, ReferenceBot, SimulationConfig, SimulatorBackend};

fn main() -> Result<(), String> {
    let backend = SimulatorBackend::new(7, SimulationConfig::default())?;
    let mut runner = BotRunner::new(backend);
    let mut bot = ReferenceBot;
    for _ in 0..200 {
        runner.step(&mut bot)?;
    }
    Ok(())
}
```

The same adapter accepts `NetworkBackend`. Core-only callers can use
`reference::decide(&Observation)` without the `networking` feature. The kernel
reads only that observation; network bots must supply received state, never a
debug snapshot. Safety margins are heuristic policy choices, not server rules.
The [portable qualification study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-24-reference-policy/REPORT.md)
records measured cost, negative controls and limits. This bot is a starting
reference, not a combat benchmark, enforced competition gate or capacity claim.

## Python, JavaScript and other languages

Run `slither-sim client` and exchange one JSON object per stdin/stdout line:

```json
{"protocol":"slither-client-json-v1","result":{}}
{"protocol":"slither-client-json-v1","error":"Reset first"}
```

The Rust process handles packet decoding. Start a virtual-time world and step it:

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
| `wire` | Drains opt-in wire records. |

Wait for `result.frame.status == "playing"` before sending an action; latency
may require several empty steps. Omitting an action holds the last command.
A replacement backend is installed only after successful construction.
Poll network backends regularly to avoid accumulating stale input.

The decoder retains at most 4,096 snake slots, 4,096 body anchors per snake,
65,536 body anchors in total (including removed records), and 131,072 food IDs.
Updates exceeding these limits fail before changing the frame's received state;
network backends disconnect on that error. Reconnect clears retained entities.

Minimap decoding also admits at most 1,310,720 logical grid cells per frame,
enough for the largest supported team snapshot (four 512×512 layers plus their
union). M, V and U each charge `side²`; legacy u charges `80²`; L charges
`(teamCount + 1) × side²`, including its union. This local resource limit bounds
repeated full-grid work even for empty snapshots or deltas. Exceeding it rejects
the whole frame before committing received state; each new frame has a fresh
budget. The separate L limit remains 1,048,576 cells across its team layers.

`processingSeconds` is measured bot compute time, finite and nonnegative. It replaces the configured sample for that decision, including when
no action is sent. Network computation has already elapsed, so the value is
recorded as metadata. Invalid actions are rejected before recording the duration;
later I/O or record-buffer errors do not roll back sent bytes or advanced time.

## What the bot knows

`ClientFrame.time` is local monotonic time. `observation.time` follows received
server elapsed time; header-free connections use local receipt time since setup.
Coordinates are centered at the arena origin, with body anchors head to tail.
Observations appear after self's full appearance and are absent after death.

Self is the first full snake appearance on a fresh connection, not the setup's
server number. A mid-life capture cannot establish self by that rule.

Entities come only from received messages. `received` gives independent age,
validity and prediction flags for appearance, position, heading, wanted heading,
speed, body and mass, plus category completeness and control histories. An empty
list does not imply complete coverage. Polling never refreshes a receipt.
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
These reports do not alter simulator physics. Team maps exceeding 1,048,576 total
cells are rejected before replacing received state.

## Compatibility and limits

Network config accepts an explicit `ws://` or `wss://` URL, optional `origin`,
Latin-1 nickname (24 characters), skin byte, `customSkin` (255 bytes), timeouts,
decision interval and `recordWire`. `requestSessionId: true` sends `[4]` before
login; it is off by default. IDs are not authentication or life resumption.
The default `handshake: "challenge"` uses the supported login exchange;
`"direct"` sends login immediately.

Challenges accept a bounded string-copy template, never executable JavaScript;
other syntax fails explicitly. TLS verifies certificates. A bare initial challenge
selects header-free framing, including bundled packets. Body contraction outside
0–1 is rejected. Matching protocol numbers alone does not ensure compatibility.

### Simulation

See the [configuration reference](configuration.md) for all world, simulation,
latency and network fields, their defaults and limits.

Defaults: one snake, six 7 ms ticks per decision/broadcast, 136×136 minimap every
2.5 seconds, leaderboard on entry/every 3 seconds and zero added delay.
`leaderboardIntervalSeconds` must be positive and finite.

Latency settings include `roundTripSeconds`, `roundTripWeights`, `roundTripSpread`,
`uplinkShare`, `jitterSeconds`, `congestion`, `processingSeconds` and
`processingJitterSeconds`:

```json
{"op":"simulation","seed":7,"config":{"agents":4,"latency":{"roundTripSeconds":[0.04,0.12],"roundTripWeights":[3,1],"jitterSeconds":0.003,"processingSeconds":[0.005,0.02]}}}
```

The same build, seed, config, actions and processing durations reproduce virtual
time. Retain measured durations when using `BotRunner`. The controlled decision
finishes processing before the next decision frame. Multi-agent callers use
`step_all`, `send_to`, `decision_pending` and `frame_for`; busy slots reject new
actions. Reconnect starts a new life at a clear spawn while other snakes continue.

Visibility, minimap rasterization and body presentation remain approximations;
reconstruction differs from the stock renderer's smoothing. See
[mechanics and delivery limits](server-rules.md).
Use the [host](raw-server.md) to exercise
real sockets; the native backend delivers protocol bytes inside the process.

### Recording and network bounds

`recordWire` defaults off. Drain regularly: recording fails at 32 MiB, with the
network backend retaining the final incoming message even if it crosses that
limit. Records hold application payloads and local send/receipt times, not TCP
frames. `replay_record` also needs separately retained processing durations to
reproduce processing metadata. Existing capture bytes can be replayed into v2.

Network messages are capped at 1 MiB. OS DNS resolution has no separate deadline
from the resolver; socket timeouts do not bound it. Switching backends preserves
inputs and action mapping, not official physics or equal playing strength.
