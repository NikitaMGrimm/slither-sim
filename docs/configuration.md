# Configuration reference

These are the world, host and portable-client settings. JSON uses camelCase;
Rust fields use snake_case. Omitted fields use defaults; unknown fields are
rejected. Durations are seconds unless named `Ticks`; distances are world units.
Defaults below are constructor defaults, not the customized server example.
World `Config` is available in core-only builds; the host and portable-client
configs require the default-enabled `networking` feature.

| Entry point | Configuration object |
| --- | --- |
| `host --config FILE` | `HostConfig`, with `simulation.world` holding world settings. |
| `client`: `{"op":"simulation","config":{...}}` | `SimulationConfig`, with nested `world`. |
| `serve`: `{"op":"reset","config":{...}}` | World `Config` directly. |
| `client`: `{"op":"network","config":{...}}` | `NetworkConfig` for a real WebSocket connection. |

Every world `Config` object also accepts a [`preset`](#presets).

## Timing and ping

- `world.dt: 0.007`: physics advances every 7 ms, about 143 ticks/s.
- `broadcastTicks: 6`: gameplay updates are sent every six physics ticks,
  so `6 × 7 ms = 42 ms`, about 24 updates/s. It does not mean six milliseconds.
- `decisionTicks: 6`: virtual-time bot decisions and host background bots use
  a nominal 42 ms cadence; processing can delay a decision. Remote clients choose
  their own decision interval.

Broadcast cadence is a local setting, not an exact recovered official schedule.
Pongs can be sent between broadcasts. Physics keeps advancing between updates.
Minimaps and leaderboards have independent nominal 2.5/3-second clocks, serviced
at physics ticks; deadlines do not accumulate gameplay-broadcast rounding. Prey
position refreshes have their own bounded reporting intervals; see
[host fidelity](server-rules.md#packet-timing).

Ping is a measured round trip, not a target the server needs you to set. Native
clients send game pings automatically after setup, with at most one outstanding.
For a real host, leave artificial latency at zero: sockets and server processing
already take time. The [server example](raw-server.md#configuration) adds none.
Zero added delay does not imply a measured ping of zero.

For virtual-time training, `latency.roundTripSeconds: [0.06]` models a 60 ms base
round trip; it is split evenly unless `uplinkShare` changes. Tick scheduling,
jitter and congestion can affect the measured result. Do not add the same delay
at both ends of a real connection unless that extra delay is intentional.

## HostConfig

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `bind` | `"127.0.0.1:8766"` | Literal IP and port; `--bind` overrides it. |
| `seed` | `1` | Unsigned 32-bit simulation seed. |
| `maxClients` | `4` | Reserved remote slots; at least 1. |
| `bots` | `4` | Background food-seeking bots; clients + bots must be at most 128. |
| `simulation` | Defaults below | Host derives `agents` and `controlledSlot`; `recordWire` must be false. |

See [hosting](raw-server.md) for TLS, connection limits and supported messages.

## SimulationConfig

Used directly by `SimulatorBackend`, or nested under `HostConfig.simulation`.

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `world` | `Config::official(1)` | World settings below. |
| `agents` | `1` | Slots, 1–128; overridden by the host. |
| `controlledSlot` | `0` | Bot-controlled slot, less than `agents`; overridden by the host. |
| `decisionTicks` | `6` | Physics ticks per bot decision, 1–1000. |
| `broadcastTicks` | `6` | Physics ticks per gameplay broadcast, 1–1000, subject to the bound below. |
| `minimapIntervalSeconds` | `2.5` | Positive finite interval. |
| `minimapSide` | `136` | Received protocol-19 minimap resolution; configurable 1–512. Rasterization remains approximate. |
| `leaderboardIntervalSeconds` | `3` | Positive finite interval; a board also arrives on entry. |
| `localRecords` | `false` | Opt into world-run records and victory-message entry; official qualification rules are unknown. |
| `latency` | Zero added delay | Link and processing settings below. |
| `recordWire` | `false` | Retain wire records; drain regularly. Unavailable on the host. |

For incremental body encoding, `broadcastTicks × world.dt × 437.5` must be
less than 84 world units. All possible arena radii must be at most 32,000, and
configured prey speeds must satisfy `round(speed × 32) ≤ 65535`. The largest
attainable prey size on the 0.2-unit spawn grid must not exceed 51, the maximum
encodable in an appearance packet. Range endpoints may exceed 51 if they still
only produce representable sizes (for example, `size: 51, maxSize: 51.01`).

## World Config

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `radius` | `670` | Positive finite arena radius. |
| `dynamicRadius` | `true` | Enable the provisional population-driven boundary model. |
| `vision` | `1500` | Positive finite snake/prey coverage radius; an approximation, not a measured official threshold. Food has separate sector interest. |
| `foodCount` | `109` | Ambient food target/startup placement attempts, at most 100,000. Admission limits can reduce placement. |
| `ambientReplenishPerSecond` | `≈1.60456356` | Ambient placement attempts/s below target, 0–10,000. |
| `dt` | `0.007` | Positive finite physics step, at most 0.032 s. |
| `foodIndex` | `true` | Accelerate food lookup; disable for differential debugging. |
| `emitBoostFood` | `true` | Emit boost pellets; disabling preserves boost cost and random-stream consumption. |
| `speedModel` | `"retained_progress"` | Retained boost progress, or `"direct_speed"` for differential runs. |
| `prey` | Defaults below | Moving edible entities. |
| `radiusUpdates` | `[]` | Recorded `{time, radius}` samples applied at tick ends; times must be finite, nonnegative and strictly increasing; radii positive and finite. |
| `boundaryMode` | `"sliding"` | Approximate rim sliding, or `"hard"` for immediate boundary death. |
| `kiosk` | Defaults below | Provisional penalty for remaining in one area. |
| `deathFood` | Defaults below | Death-food admission limit. |
| `ambientFood` | Defaults below | Ambient-food admission limit. |
| `boostFood` | Defaults below | Boost-food admission limit. |
| `pit` | `null` | `{"radius": r}`, `0 < r < radius`: a one-way inner boundary. A snake whose head center has been inside (distance `< r`) dies with reason `pit`, dropping no food, when its head center reaches `r` again. Each tick tests the head center's swept path, so crossing the pit or clipping its edge within one tick also kills. Each life starts outside it. |
| `spawnRadius` | `null` | Fixed head distance of new lives, `0 ≤ d < radius` and `d > pit.radius`. Ring joins keep their angles; the host draws a uniform angle. `null` keeps the official radial grid. |
| `killScoreSeconds` | `null` | Positive finite kill-score window: each credited kill counts this long, kept per slot across lives. `null` keeps no kill score; it reads 0. |
| `minScore` | `null` | Least displayed score, an integer from a newborn's 10 to 31,066,725, the most a 430-segment snake displays. Its floor mass is the least mass on the protocol-19 fam grid that displays it, so clients decode the same score. New lives start at that mass; boosting and the kiosk penalty never shrink a snake below it, and boosting there is free and drops nothing. Growth and death food are unchanged. Read when a world is created. |

`radiusUpdates` sets radius at each supplied sample, overriding the model at that
tick. For recorded replay, set `dynamicRadius` false to avoid model changes
between samples. See [rules and evidence limits](server-rules.md).

The world and prey defaults are `Config::official(1)`.

### Official and sandbox

Both Rust constructors return the same `Config` type:

| Constructor | Result |
| --- | --- |
| `Config::official(n)` | Radius `r = 670 × sqrt(max(n, 1))` with dynamic radius; food target `floor(7πr² / 90,000)` and refill `0.1024πr² / 90,000` attempts/s (per 300×300 sector: about 7 pellets, 0.1024 attempts/s); prey target `max(1, round(πr² / 10,000,000))`. |
| `Config::sandbox()` | `Config::official(1)` with radius 1000, fixed boundary, food target 350, 5 ambient attempts/s and no prey. |

`Config::default()`, omitted JSON fields, `SimulationConfig` and `host` use
`Config::official(1)`. It is the best current approximation, not an exact
official replica. It keeps the provisional kiosk penalty, food cell limits, rim
sliding, retained boost speed and radius model; the sandbox shares them.

`n` initializes radius, food and prey targets; it does not spawn snakes. With
dynamic radius, the living population then drives the radius (unoccupied remote
slots do not count); food and prey targets are not rescaled. One snake is a
deliberately small start, not a typical official lobby; request
`Config::official(n)` or set world fields explicitly for larger scenes.

The prey density of one per 10 million square units is a working assumption,
not a calibrated global population. Received density near recorded recipients,
about 0.04–0.18 per million square units, is a lead rather than a census; the
official prey population remains unresolved. The
[official-defaults report](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-official-defaults/REPORT.md)
records the sources and caveats of these choices.

Neither constructor changes ping, broadcasts or decision timing. Those belong to
`SimulationConfig`, which wraps either world:

```rust
use slither_sim::{Config, client::{HostConfig, SimulationConfig}};

let simulation = SimulationConfig {
    world: Config::official(8), // Or Config::sandbox().
    ..Default::default()
};
let host = HostConfig { simulation, ..Default::default() };
```

### Presets

In JSON, `"preset"` selects a constructor's world as the base: `"official"`
(`Config::official(1)`), `"sandbox"` or `"battledome"`. Fields given beside it
replace the preset's values; nested objects such as `kiosk` merge field by
field, and `null` clears an optional setting. Changing `radius` does not rescale
the preset's food. Without `preset`, omitted fields use `Config::official(1)`.
Replies and snapshots print the resolved fields, not the preset name.

```json
{"preset": "battledome", "killScoreSeconds": 300}
```

### Battledome

`Config::battledome()` (`"preset": "battledome"`) is a custom mode for
Battledome players: official physics and ecology, unchanged, plus a thin rules
layer. It does not model any community server. Each value is a stated
Battledome rule, the official model or a mode design choice:

| Setting | Value | Label |
| --- | --- | --- |
| Outer wall | `radius: 3584`, `dynamicRadius: false`, normal boundary behavior | Stated rule |
| Pit | `pit.radius: 1784`; entry allowed, exit lethal | Stated rule; the swept head-center test, no food and the per-life reset are design choices |
| Spawning | `spawnRadius: 2684`, midway between pit and wall, with the usual clearance and heading | Design choice derived from the two radii |
| Kill score | `killScoreSeconds: 900`: +1 per credited kill for 15 minutes, per slot across lives | Stated rule (+1, 15 minutes); persistence is a design choice |
| Size floor | `minScore: 400`: boosting is not free, but no snake shrinks below 400 points | Stated rule |
| Spawn size | New lives start at the floor: 26 segments, fam ≈ 0.1589, the least protocol-exact mass displaying 400 | Design choice (our reading of the floor) |
| Boost at the floor | Free and drops nothing; food comes only from mass actually removed, so a partial step down drops a smaller pellet | Design choice |
| Spawn body | A multi-segment spawn lies along the spawn circle behind the head, heading along its tangent, and must clear other snakes over its whole length; straight if the circle is too small | Design choice, so a 1,000-unit body stays clear of wall and pit |
| Prey | `prey.count: 0` | Mode rule |
| Food | Official density at radius 3584: `foodCount` 3138, about 45.9 attempts/s; normal admission caps, death and boost food | Official model |
| Everything else | As `Config::official(1)`: `dt`, broadcast and decision ticks, vision, kiosk, speed model, boundary mode | Official model |

Kill scores appear in `serve` frames as `snakes.killScore`, in the C ABI and
Python as `snake_kill_score`, and in Rust as `Simulator::kill_score(slot)`.
Hosted stock clients do not yet draw the pit or show kill scores.

### Prey

All numeric values except `count` must be positive and finite. Speeds are world
units/s, turn rate is radians/s; `cruiseSpeed ≤ escapeSpeed ≤ sprintSpeed` and
`size ≤ maxSize`. The size range must contain a positive 0.2-grid value and
remain finite when multiplied by 5, so spawned prey can respect both bounds.
Protocol speed is divided by 0.032 to obtain world units/s; turn coefficients
use an 8 ms reference. These conversion scales are separate from `world.dt`.

| Field under `prey` | Default | Meaning |
| --- | --- | --- |
| `count` | `1` | Target population, at most 10,000; zero disables initial spawning/replenishment. |
| `cruiseSpeed` | `118.75` | Roaming speed (`3.8 / 0.032`). |
| `escapeSpeed` | `192.28125` | Flight target near a minimum-speed snake (`6.153 / 0.032`). |
| `sprintSpeed` | `343.65625` | Flight target near a full-boost snake (`10.997 / 0.032`). |
| `turnRate` | `3.625` | Maximum turning rate (`0.029 / 0.008`). |
| `fleeHalfWidth` | `320` | Threat detection square's half-width. |
| `size` | `6` | Lower spawned size. |
| `maxSize` | `9.2` | Upper spawned size. |
| `nutritionFactor` | `30` | Multiplier on ordinary food nutrition at the prey's size. |
| `respawnSeconds` | `1` | Replenishment interval while below target. |

### Food admission limits

Each object has `enabled` (default `true`), positive finite `cellSize`, and
positive integer `maxPerCell`. All existing food counts toward cell occupancy.

| Object | `cellSize` | `maxPerCell` |
| --- | --- | --- |
| `ambientFood` | `75` | `2` |
| `boostFood` | `75` | `9` |
| `deathFood` | `75` | `22` |

### Kiosk penalty

This is a historical, provisional anti-camping model. Set `kiosk.enabled: false`
when it should not affect a replay. Numeric fields must be positive and finite,
except `graceSeconds`, which may be zero.
Defaults fit a 62,000-to-8,500 change over six minutes for physical score and
120 minutes for displayed score. Recovery reuses the physical decay rate as an
approximation.

| Field under `kiosk` | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Enable the penalty. |
| `floorScore` | `8500` | Minimum physical/displayed score during the penalty. |
| `graceSeconds` | `1200` | Time before penalizing a qualifying stationary snake. |
| `stationaryRadius` | `800` | Distance threshold from its qualifying starting point. |
| `shrinkPerSecond` | `ln(62000/8500) / 360` | Exponential physical score decay rate. |
| `scoreDecayPerSecond` | `ln(62000/8500) / 7200` | Displayed score decay rate after physical size reaches the floor. |
| `recoveryPerSecond` | `ln(62000/8500) / 360` | Physical score restoration rate after leaving the area. |

## LatencyConfig

Under `SimulationConfig.latency`. These are artificial delays, additional to any
real socket delay. Messages remain ordered; delivery/application is tick-based.
Host remote clients use their own real compute time; processing settings below
apply to in-process bots, not remote clients.

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `roundTripSeconds` | `[]` | Base RTT candidates, each 0–2 s; one selected per connection. Empty means zero. |
| `roundTripWeights` | `[]` | Empty means equal weights; otherwise one finite nonnegative weight per candidate, with a positive finite sum. |
| `roundTripSpread` | `0` | Log-normal spread around the selected RTT, 0–2; resulting base RTT capped at 2 s. |
| `uplinkShare` | `0.5` | Fraction of RTT assigned to client-to-server travel, 0–1. |
| `jitterSeconds` | `0` | Uniform ± spread per one-way message, 0–2 s; delays cannot be negative. |
| `congestion` | Disabled | Episode settings below. |
| `processingSeconds` | `[0, 0]` | Ordered range within 0–2 s; connection draws a base bot processing time. |
| `processingJitterSeconds` | `0` | Uniform ± spread per decision, 0–2 s. |

Measured `processingSeconds` supplied with a bot decision replaces its modeled
duration. Rust also provides `LatencyConfig::fixed(rtt)` and
`LatencyConfig::one_way(uplink, downlink)`.

### Congestion

All values must be finite.

| Field under `congestion` | Default | Meaning / limits |
| --- | --- | --- |
| `ratePerSecond` | `0` | Mean episode starts/connection-second, 0–1; zero disables episodes. |
| `durationSeconds` | `0` | Mean exponentially drawn episode duration, 0–10 s. |
| `followProbability` | `0` | Chance of a follow-up episode, 0–1. |
| `followSeconds` | `0` | Mean exponentially drawn wait for a follow-up, 0–10 s. |
| `extraSeconds` | `[0, 0]` | Ordered extra RTT bounds within 0–2 s; capped Pareto draw. |
| `shape` | `1` | Pareto shape, 0.1–100. |

## NetworkConfig

For a bot connecting to a real WebSocket server; this object does not simulate
latency. Only `url` must be supplied; other fields use these defaults.

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `url` | Required | Explicit `ws://` or `wss://` URL, without embedded credentials. |
| `origin` | `null` | Optional WebSocket Origin header. |
| `nickname` | `"SlitherSim"` | At most 24 Latin-1 characters. |
| `skin` | `0` | Byte, 0–255. |
| `customSkin` | `[]` | At most 255 bytes. |
| `requestSessionId` | `false` | Request opaque session IDs during login. |
| `decisionIntervalSeconds` | `0.042` | Client polling/decision interval, 0.001–10 s; does not set server broadcasts. |
| `connectTimeoutSeconds` | `5` | Connection/handshake timeout, 0.001–60 s; does not bound OS DNS resolution. |
| `idleTimeoutSeconds` | `10` | Timeout without received traffic, 0.001–600 s. |
| `handshake` | `"challenge"` | Supported challenge exchange, or `"direct"` login. |
| `recordWire` | `false` | Retain application payloads and local send/receipt times. |

All three durations must be finite. See the [portable API](client-api.md) for
polling, processing measurements, recording bounds and reconnection behavior.
