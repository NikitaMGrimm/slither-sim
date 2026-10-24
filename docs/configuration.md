# Configuration reference

These are the world, host and portable-client settings. JSON uses camelCase;
Rust fields use snake_case. Omitted fields use defaults; unknown fields are
rejected. Durations are seconds unless named `Ticks`; distances are world units.
Defaults below are constructor defaults, not the customized server example.
World `Config` is available in core-only builds; the host and portable-client
configs require the default-enabled `networking` feature. [Limits](#limits)
explains why each remaining bound exists.

| Entry point | Configuration object |
| --- | --- |
| `host --config FILE` | `HostConfig`, with `simulation.world` holding world settings. |
| `client`: `{"op":"simulation","config":{...}}` | `SimulationConfig`, with nested `world`. |
| `serve`: `{"op":"reset","config":{...}}` | World `Config` directly. |
| `client`: `{"op":"network","config":{...}}` | `NetworkConfig` for a real WebSocket connection. |

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
| `bots` | `4` | Background food-seeking bots; clients + bots must be at most 65,535. |
| `simulation` | Defaults below | Host derives `agents` and `controlledSlot`; `recordWire` must be false. |

See [hosting](raw-server.md) for TLS, connection limits and supported messages.

## SimulationConfig

Used directly by `SimulatorBackend`, or nested under `HostConfig.simulation`.

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `world` | `Config::official(1)` | World settings below. |
| `agents` | `1` | Slots, 1–65,535; overridden by the host. |
| `controlledSlot` | `0` | Bot-controlled slot, less than `agents`; overridden by the host. |
| `decisionTicks` | `6` | Physics ticks per bot decision, at least 1. |
| `broadcastTicks` | `6` | Physics ticks per gameplay broadcast, at least 1; see the warning below. |
| `minimapIntervalSeconds` | `2.5` | Positive finite interval. |
| `minimapSide` | `136` | Received protocol-19 minimap resolution; 1–512, the stock client's clamp. Rasterization remains approximate. |
| `leaderboardIntervalSeconds` | `3` | Positive finite interval; a board also arrives on entry. |
| `localRecords` | `false` | Opt into world-run records and victory-message entry; official qualification rules are unknown. |
| `latency` | Zero added delay | Link and processing settings below. |
| `recordWire` | `false` | Retain wire records; drain regularly. Unavailable on the host. |

Broadcasts send at most three new body anchors per snake incrementally. When
`broadcastTicks × world.dt × 437.5` reaches 126 world units, a boosting snake
can outrun that window and is resent in full, which costs bandwidth but stays
correct; such configs load with a warning. All possible arena radii must be at
most 32,000 (an encoding-margin [policy](#limits)), and configured prey speeds
must satisfy `round(speed × 32) ≤ 65535` and the turn rate
`round(turnRate × 8) ≤ 65535`. The largest attainable prey
size on the 0.2-unit spawn grid must not exceed 51, the maximum encodable in an
appearance packet. Range endpoints may exceed 51 if they still only produce
representable sizes (for example, `size: 51, maxSize: 51.01`).

## World Config

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `radius` | `670` | Positive finite arena radius. |
| `dynamicRadius` | `true` | Enable the provisional population-driven boundary model. |
| `vision` | `1500` | Positive finite snake/prey coverage radius; an approximation, not a measured official threshold. Food has separate sector interest. |
| `foodCount` | `109` | Ambient food target/startup placement attempts. Admission limits can reduce placement; memory bounds it. |
| `ambientReplenishPerSecond` | `≈1.60456356` | Ambient placement attempts/s below target; finite, nonnegative and small enough that one attempt interval still changes the refill clock. |
| `dt` | `0.007` | Positive finite physics step. With sliding boundaries it must be below 16 / 437.5 ≈ 0.036571 s; steps outside the measured 6.94–7.07 ms warn. |
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

There is no named preset selector in JSON or the CLI. For the compact sandbox,
set its fields explicitly:

```json
{"radius": 1000, "dynamicRadius": false, "foodCount": 350, "ambientReplenishPerSecond": 5, "prey": {"count": 0}}
```

### Prey

Speeds, `turnRate` and `nutritionFactor` must be finite and nonnegative; zero
means stationary, non-turning or non-nourishing prey. `fleeHalfWidth`, sizes
and `respawnSeconds` must be positive and finite. Speeds are world units/s,
turn rate is radians/s; `cruiseSpeed ≤ escapeSpeed ≤ sprintSpeed` and
`size ≤ maxSize`. The size range must contain a positive 0.2-grid value and
remain finite when multiplied by 5, so spawned prey can respect both bounds.
The largest catch, `size² × 0.0031535 × nutritionFactor`, added to a snake's
mass must be finite.
Protocol speed is divided by 0.032 to obtain world units/s; turn coefficients
use an 8 ms reference. These conversion scales are separate from `world.dt`.

| Field under `prey` | Default | Meaning |
| --- | --- | --- |
| `count` | `1` | Target population; zero disables initial spawning/replenishment. |
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
integer `maxPerCell`; zero admits no pellet of that origin. All existing food
counts toward cell occupancy.

| Object | `cellSize` | `maxPerCell` |
| --- | --- | --- |
| `ambientFood` | `75` | `2` |
| `boostFood` | `75` | `9` |
| `deathFood` | `75` | `22` |

### Kiosk penalty

This is a historical, provisional anti-camping model. Set `kiosk.enabled: false`
when it should not affect a replay. Numeric fields must be finite. `floorScore`
and `stationaryRadius` must be positive; `graceSeconds` and the three rates may
be zero, and a zero rate freezes its phase.
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
| `roundTripSeconds` | `[]` | Base RTT candidates, each finite and nonnegative; one selected per connection. Empty means zero. |
| `roundTripWeights` | `[]` | Empty means equal weights; otherwise one finite nonnegative weight per candidate, with a positive finite sum. |
| `roundTripSpread` | `0` | Log-normal spread around the selected RTT; finite and nonnegative. |
| `uplinkShare` | `0.5` | Fraction of RTT assigned to client-to-server travel, 0–1. |
| `jitterSeconds` | `0` | Uniform ± spread per one-way message, finite and nonnegative; delays cannot be negative. |
| `congestion` | Disabled | Episode settings below. |
| `processingSeconds` | `[0, 0]` | Ordered finite nonnegative range; connection draws a base bot processing time. |
| `processingJitterSeconds` | `0` | Uniform ± spread per decision, finite and nonnegative. |

The largest possible delay must be finite. The generator's normal draw is at
most about 8.57 and its exponential draws at most about 36.7 times their mean,
so `max(roundTripSeconds) × exp(8.57 × roundTripSpread) + extraSeconds[1] +
jitterSeconds` and `processingSeconds[1] + processingJitterSeconds` must be
finite. Measured `processingSeconds` supplied with a bot decision replaces its
modeled duration. Rust also provides `LatencyConfig::fixed(rtt)` and
`LatencyConfig::one_way(uplink, downlink)`.

### Congestion

All values must be finite. Means must also stay finite at 36.7 times their value.

| Field under `congestion` | Default | Meaning / limits |
| --- | --- | --- |
| `ratePerSecond` | `0` | Mean episode starts/connection-second, nonnegative; zero disables episodes. |
| `durationSeconds` | `0` | Mean exponentially drawn episode duration, nonnegative. |
| `followProbability` | `0` | Chance of a follow-up episode, 0–1. |
| `followSeconds` | `0` | Mean exponentially drawn wait for a follow-up, nonnegative. |
| `extraSeconds` | `[0, 0]` | Ordered nonnegative extra RTT bounds; Pareto draw from the lower bound, capped at the upper. A zero lower bound adds nothing. |
| `shape` | `1` | Pareto shape, positive. |

## NetworkConfig

For a bot connecting to a real WebSocket server; this object does not simulate
latency. Only `url` must be supplied; other fields use these defaults.

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `url` | Required | Explicit `ws://` or `wss://` URL, without embedded credentials. |
| `origin` | `null` | Optional WebSocket Origin header. |
| `nickname` | `"SlitherSim"` | At most 24 Latin-1 characters, as the stock client allows. |
| `skin` | `0` | Byte, 0–255. |
| `customSkin` | `[]` | At most 255 bytes. |
| `requestSessionId` | `false` | Request opaque session IDs during login. |
| `decisionIntervalSeconds` | `0.042` | Client polling/decision interval; does not set server broadcasts. Each decision reads for at least 1 ms. |
| `connectTimeoutSeconds` | `5` | Connection/handshake timeout; does not bound OS DNS resolution. |
| `idleTimeoutSeconds` | `10` | Timeout without received traffic. |
| `handshake` | `"challenge"` | Supported challenge exchange, or `"direct"` login. |
| `recordWire` | `false` | Retain application payloads and local send/receipt times. |

All three durations must be positive, at least 1 ns and representable as a
platform `Duration`; a deadline the platform clock cannot represent fails at
run time. See the [portable API](client-api.md) for polling, processing
measurements, recording bounds and reconnection behavior.

## Limits

Every bound is one of three kinds. The checks themselves keep their rationale
beside the owning code.

**Real limits** are errors because something breaks beyond them:

| Bound | Reason |
| --- | --- |
| Sliding `dt < 16 / 437.5 s` (≈ 36.57 ms) | A slide leaves the head one contact lead (16 × scale) behind the rim. A boosting minimum-size snake that moves that far in one tick, keeping its heading, lands on or outside the rim and dies instead of sliding. Hard boundaries kill on contact anyway and accept longer steps, up to the substep bound below. |
| `dt < 0.007 × usize::MAX` s (≈ 1.29 × 10¹⁷ s on 64-bit) | Prey advance in 7 ms substeps, and one tick's count must fit `usize`. This also keeps a tick of boost travel, `dt × 437.5`, finite. |
| Refill rate | Refill subtracts one attempt interval at a time while credit remains; the interval must still change the refill clock in f64, or the loop never ends. |
| Largest prey catch finite | Finite settings can still overflow `size² × 0.0031535 × nutritionFactor` and the eater's mass. Catches can still accumulate past f64; that pickup stops the world: its tick completes without the gain, the step returns an error and later steps are rejected (`serve` ends the session). |
| Largest latency draw finite | Delays are added to clocks. |
| Network durations ≥ 1 ns and representable | Socket timeouts reject zero; `Instant` arithmetic must not overflow. |
| `serve` `agents ≤ 4,294,967,295` | Frame slot indices are `u32`, and `u32::MAX` means none. |
| `serve` `decisionTicks ≤ 4,294,967,295` | Frames report the ticks a step advanced as `u32`. |
| `serve` `worlds × agents` fits `usize` | Typed steps and the C ABI index slots world-major. Memory bounds worlds otherwise. |
| `serve` frame columns | Rows, counts and offsets are `u32`. A step whose frame does not fit ends the session, since its worlds already advanced. |
| Wire `agents ≤ 65,535` | Protocol-19 snake IDs, leaderboard rank and living population are `u16`. |
| Wire prey speeds, turn rate, size | Their `u16`/`u8` packet fields; see [SimulationConfig](#simulationconfig). |
| Wire prey `count ≤ 65,536`; at most 65,536 prey live | Protocol-19 prey IDs are `u16`; each living prey keeps a unique one. A retired ID is free at once, since every connection receives its removal before any reuse in the same broadcast. |
| `customSkin` ≤ 255 bytes | Appearances give its length in one byte. |
| `minimapSide ≥ 1`, positive intervals, `decisionTicks`/`broadcastTicks ≥ 1` | Empty maps and zero periods have no meaning; broadcasts fall on ticks divisible by `broadcastTicks`. |

Domains such as finite values, nonnegative delays, probabilities in 0–1 and
ordered ranges follow from each field's meaning.

**Valid but unmeasured** settings load with a warning (stderr for the command
line, `warnings` in `serve` reset replies, `UserWarning` in Python):

- `dt` outside the measured 6.94–7.07 ms server loop. Contact, pickup and event
  timing may differ; prey still move in fixed 7 ms substeps.
- `broadcastTicks × dt × 437.5 ≥ 126`: snakes that outrun the incremental
  body-update window are resent in full, costing bandwidth.

Defaults such as food admission cells, kiosk rates, prey behavior, vision and
latency are provisional models; other values are accepted without implying a
measured range.

**Policies** are deliberate compatibility or resource choices, not legality:

| Policy | Value | Reason |
| --- | --- | --- |
| Nickname | 24 characters | Stock-client input; the field allows 255 bytes. |
| Victory text | 140 ASCII bytes | Stock-client input. |
| `minimapSide` | at most 512 | The stock client clamps larger sides. |
| Close reason | 123 bytes | WebSocket control payload (125) minus the close code. |
| Wire arena radius | at most 32,000 | Encoding margin. Anchors encode as `u16` around 32,550, so coordinates must stay within −32,550..32,985, and encoders reject anything outside. The 550 units left for heads and bodies near the rim are not derived; a hard-mode tick's overshoot grows with `dt`. |
| Simulated link queues | 4,096 messages per direction; 2,048 combined for remote slots | Bounded memory; the run fails when full. |
| Wire records | 32 MiB per slot or connection | Drain to continue. |
| Messages | 1 MiB | Host send buffer and native client message size. |
| Host input | 4 KiB per message | Far above the largest stock login. |
| Host sockets | 32 pending, 16 accepts and 32 reads per poll | Fairness; slow clients cannot starve physics. |
| Host deadlines | upgrade 5 s, login 10 s, close 1 s | Connection lifecycle. |
| Host catch-up | 8 ticks per poll, 0.25 s backlog | World time defers rather than bursts. |
| Decoder retention | 4,096 snakes, 65,536 body points (4,096 per snake), 131,072 food | A broken server cannot grow received state without bound; dense worlds can reach these first. |
| Spawn attempts | 64 draws per spawn | The spawn is deferred, even if a clear pose exists. |
| Network read floor | 1 ms per decision | A bot behind its schedule still reads its socket. |
