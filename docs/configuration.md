# Configuration reference

World, host and portable-client settings use camelCase in JSON and snake_case
in Rust. Omitted fields use constructor defaults; unknown fields are rejected.
Durations are seconds unless named `Ticks`; distances are world units.
World `Config` supports core-only builds; host and portable-client configs need
the default-enabled `networking` feature. [Limits](#limits) explains the bounds.

| Entry point | Configuration object |
| --- | --- |
| `host --config FILE` | `HostConfig`, with `simulation.world` holding world settings. |
| `client`: `{"op":"simulation","config":{...}}` | `SimulationConfig`, with nested `world`. |
| `serve`: `{"op":"reset","config":{...}}` | World `Config` directly. |
| `client`: `{"op":"network","config":{...}}` | `NetworkConfig` for a real WebSocket connection. |

Every world `Config` object also accepts a [`preset`](#presets).

## Timing and ping

- `world.dt: 0.007`: physics advances every 7 ms, about 143 ticks/s.
- `decisionTicks: 6`: virtual-time bot decisions and host background bots use
  a nominal 42 ms cadence; processing can delay a decision. Remote clients choose
  their own decision interval.

See [packet timing](server-rules.md#packet-timing) for per-family release rules
and shared minimap/leaderboard phases. Their default periods reproduce measured
official means, not an exact recovered scheduler.

Ping measures round-trip time. Native clients ping automatically after setup,
with at most one outstanding. Leave artificial latency at zero on real hosts,
as in the [server example](raw-server.md#configuration): sockets and processing
already take time. [Same-tick replies](server-rules.md#packet-timing) normally
add up to one physics tick (7 ms by default); OS scheduling and load can add more.

For virtual-time training, `latency.roundTripSeconds: [0.06]` models a 60 ms base
round trip, split evenly unless `uplinkShare` changes. Scheduling, variation and
pauses affect measured RTT. Adding delay at both ends of a real connection adds
both delays.

## HostConfig

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `bind` | `"127.0.0.1:8766"` | Literal IP and port; `--bind` overrides it. |
| `seed` | `1` | Unsigned 32-bit simulation seed. |
| `maxClients` | `4` | Reserved remote slots; at least 1. |
| `bots` | `4` | Background food-seeking bots; clients + bots must be at most 65,535. |
| `botThreads` | `2` | Background bot worker threads, separate from simulation and sockets; at least 1 when `bots > 0`. At most `bots` workers are started. |
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
| `minimapIntervalSeconds` | `2.504` | Positive finite period of the server-wide minimap clock. |
| `minimapSide` | `136` | Received protocol-19 minimap resolution; 1–512, the stock client's clamp. Rasterization remains approximate. |
| `leaderboardIntervalSeconds` | `3.008` | Positive finite period of the server-wide leaderboard clock; the first board waits for it. |
| `localRecords` | `false` | Opt into world-run records and victory-message entry; official qualification rules are unknown. |
| `latency` | Zero added delay | Link and processing settings below. |
| `recordWire` | `false` | Retain wire records; drain regularly. Unavailable on the host. |

Incremental updates send at most three new body anchors per snake per tick.
When `world.dt × 437.5 ≥ 126` world units, boosting snakes can outrun this window;
full resends preserve correctness at extra bandwidth cost and configs warn.
All possible arena radii must be at most 32,000 (an encoding-margin
[policy](#limits)). Prey require `round(speed × 32) ≤ 65535`,
`round(turnRate × 8) ≤ 65535` and a largest attainable 0.2-grid size ≤ 51, the
appearance packet's maximum. Endpoints may exceed 51 if all attainable sizes
fit (for example, `size: 51, maxSize: 51.01`).

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
| `pit` | `null` | `{"radius": r}`, `0 < r < radius`: a one-way inner boundary; entry allowed, exit lethal. Uses the head center's swept path; see [Battledome](#battledome). |
| `spawnRadius` | `null` | New lives follow this ring with anchors 42 apart. `add` keeps its ring angles, turning counterclockwise; the host samples uniform angle and direction. Body chords must clear the pit (`spawnRadius > hypot(pit.radius, 21)`); the ring must stay more than 60 (spawn wall margin) inside `radius`; head-tail gap must exceed newborn body diameter. Validation allows for floating-point rounding at every angle. `null` keeps the official radial grid and straight 2-segment newborns. |
| `killScoreSeconds` | `null` | Positive finite rolling kill-score window, per slot across lives; `null` reads as 0. See [score semantics](#battledome). |
| `minScore` | `null` | Permanent integer displayed-score floor, 10–18,436,960; requires `spawnRadius`. Converted to mass at world creation. The maximum is the score at the table's end, 430 segments; larger snakes display less, so growth could break a higher floor. See [floor behavior](#battledome). |

`radiusUpdates` overrides the model at each sample's tick. For recorded replay,
set `dynamicRadius` false to prevent model changes between samples. See
[rules and evidence limits](server-rules.md).

### Official and sandbox

Both constructors return `Config`:

| Constructor | Result |
| --- | --- |
| `Config::official(n)` | Radius `r = 670 × sqrt(max(n, 1))` with dynamic radius; food target `floor(7πr² / 90,000)` and refill `0.1024πr² / 90,000` attempts/s (per 300×300 sector: about 7 pellets, 0.1024 attempts/s); prey target `max(1, round(πr² / 10,000,000))`. |
| `Config::sandbox()` | `Config::official(1)` with radius 1000, fixed boundary, food target 350, 5 ambient attempts/s and no prey. |

`Config::default()`, omitted JSON fields, `SimulationConfig` and `host` use
`Config::official(1)`. Both share provisional kiosk, food admission, rim sliding,
retained boost speed and radius models rather than exact official rules;
see [evidence limits](server-rules.md#defaults).

`n` initializes radius, food and prey targets without spawning snakes. Dynamic
radius then follows living population, excluding unoccupied remote slots;
food and prey targets stay fixed. One snake is a deliberately small start,
not a typical official lobby; use `Config::official(n)` or explicit world fields
for larger scenes.

Prey density is a working assumption, not a calibrated global population.
Received density of about 0.04–0.18 per million square units near recorded
recipients is a lead, not a census. The
[official-defaults report](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-23-official-defaults/REPORT.md)
records these choices' sources and caveats; official prey population is unresolved.

Link and decision settings belong to `SimulationConfig`, which wraps either world:

```rust
use slither_sim::{Config, client::{HostConfig, SimulationConfig}};

let simulation = SimulationConfig {
    world: Config::official(8), // Or Config::sandbox().
    ..Default::default()
};
let host = HostConfig { simulation, ..Default::default() };
```

### Presets

JSON `"preset"` selects a constructor's world: `"official"`
(`Config::official(1)`), `"sandbox"` or `"battledome"`. Fields given beside it
override it; nested objects such as `kiosk` merge field by field, and `null`
clears optional settings. Changing `radius` does not rescale food. Without
`preset`, omitted fields use `Config::official(1)`. Repeated keys keep their last
value because configs pass through a JSON value. Replies and snapshots print
resolved fields, not the preset name.

```json
{"preset": "battledome", "killScoreSeconds": 300}
```

### Battledome

`Config::battledome()` (`"preset": "battledome"`) is our custom mode, not an
official mode or community-server model. Its arena, pit, permanent 400-point
floor and free boost at the floor are design choices. Other mechanics inherit
`Config::official(1)`, including its
[provisional models](server-rules.md#defaults).

| Setting | Value | Mode rule and rationale |
| --- | --- | --- |
| Outer wall | `radius: 3584`, `dynamicRadius: false` | Author-chosen fixed arena; inherited boundary behavior |
| Pit | `pit.radius: 1784`; entry allowed, exit lethal | Author-chosen one-way boundary |
| Size floor | `minScore: 400` | Permanent minimum size, including newborns |
| Spawn ring | `spawnRadius: 2684` | Midway between pit and wall, giving equal room in both directions |
| Spawn body | 26 anchors, 42 units apart along the ring, heading along it | Keeps the whole new body clear of pit and wall; host samples angle and direction with 600-unit clearance |
| Kill score | `killScoreSeconds: 900` | +1 per credited kill for 15 minutes; retained across lives so death does not erase recent kills |
| Pit death food | Normal death food; wall deaths still drop none | Returns pit victims' mass to play |
| Prey | `prey.count: 0` | Snakes only |
| Ambient food | `foodCount: 3138`, about 45.9 attempts/s | Inherits the official model's density at radius 3584 and its admission caps |

The pit tests the head center's swept path each tick: after entering distance
`< r`, reaching `r` kills with reason `pit` and no credited killer, including
entry and exit within one tick. Pit-entry state resets each life.

The floor uses the least mass on the protocol-19 fam grid displaying the chosen
score (400 gives 26 segments and fam ≈ 0.1589). Boosting and the kiosk penalty
spend only mass above it. A partial final boost pellet reflects actual mass
spent; at the floor, boost is free without pellets, preventing cost-free food
generation. Growth and normal death-food rules remain.

A kill at time `t` counts while `world.time < t + killScoreSeconds`, separately
from per-life `kills` and size `score`, without changing the size-based
leaderboard. The preset defines no deadline, winner or automatic episode reset.
A new world clears the rolling score.

These rules have synthetic offline regressions in
[`tests/battledome.rs`](https://github.com/NikitaMGrimm/slither-sim/blob/main/tests/battledome.rs),
not official-server measurements. For preset use in host files and client
requests, see [hosting](raw-server.md#presets) and
[portable simulation](client-api.md#simulation).

Kill scores appear in `serve` frames as `snakes.killScore`, in the C ABI and
Python as `snake_kill_score`, and in Rust as `Simulator::kill_score(slot)`.
Rust `Observation` omits the rolling score. Geometric frames and portable
`ClientFrame` omit pit geometry and the floor setting; retain world config
separately. Portable clients receive per-life kills and the size-based
leaderboard, without rolling kill score or distinct pit death reason.
The stock browser shows the pit edge as a ring of 749 decorative pellets:
centers 3 units outside the pit, about 15 units apart, palette 8, size byte 31
(6.2). These are design choices. Markers cannot be eaten and follow normal
sector delivery. Bots get only real food, including any pellet a marker hides in
the browser. The stock browser has no rolling kill-score display.

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
counts toward cell occupancy. Rejected boost emission still spends mass.

| Object | `cellSize` | `maxPerCell` |
| --- | --- | --- |
| `ambientFood` | `75` | `2` |
| `boostFood` | `75` | `9` |
| `deathFood` | `75` | `22` |

### Kiosk penalty

This historical, provisional anti-camping model has
[unverified official onset and recovery](server-rules.md#defaults).
Set `kiosk.enabled: false` to exclude it from replay. All numbers must be finite;
`floorScore` and `stationaryRadius` positive. `graceSeconds` and the three rates
may be zero; a zero rate freezes its phase.
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

`SimulationConfig.latency` adds artificial delay to any real socket delay;
default zero. Messages stay ordered. Virtual-time delivery and server input
application use physics ticks; the host's socket thread releases outbound frames
at due times between ticks. Processing settings apply to in-process bots;
remote clients use real compute time.

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `roundTripSeconds` | `[]` | Base RTT candidates, each finite and nonnegative; one selected per connection. Empty means zero. |
| `roundTripWeights` | `[]` | Empty means equal weights; otherwise one finite nonnegative weight per candidate, with a positive finite sum. |
| `roundTripSpread` | `0` | Log-normal spread around the selected RTT; finite and nonnegative. |
| `uplinkShare` | `0.5` | Fraction of RTT assigned to client-to-server travel, 0–1. |
| `variation` | Disabled | Per-message extra delay; see below. |
| `pauses` | Disabled | Per-direction delivery pauses; see below. |
| `processingSeconds` | `[0, 0]` | Ordered finite nonnegative range; connection draws a base bot processing time. |
| `processingJitterSeconds` | `0` | Uniform ± spread per decision, finite and nonnegative. |

The generator's normal draw is at most about 8.57, exponential draws at most
about 36.7 times their mean. Thus
`max(roundTripSeconds) × exp(8.57 × roundTripSpread) + medianSeconds ×
exp(12.1 × logSigma)` and `processingSeconds[1] + processingJitterSeconds` must
be finite. Measured `processingSeconds` supplied with a decision replaces its
modeled duration. Rust also provides `LatencyConfig::fixed(rtt)`,
`LatencyConfig::one_way(uplink, downlink)` and `LatencyConfig::measured_route()`.

### Variation and pauses

Each direction is ordered. Message delay is its base share plus
`medianSeconds × exp(logSigma × (√c·X + √(1−c)·Z))`, with `c` the
`correlatedShare`, `Z` a fresh normal draw and `X` a slow unit-variance
Ornstein–Uhlenbeck state per direction. Pauses hold all messages due within them
until their end; held frames arrive together.

| Field | Default | Meaning / limits |
| --- | --- | --- |
| `variation.medianSeconds` | `0` | Median extra delay, finite and nonnegative; zero disables variation. |
| `variation.logSigma` | `0` | Log standard deviation, finite and nonnegative. |
| `variation.correlatedShare` | `0` | Share of log variance from the slow state, 0–1. |
| `variation.correlationSeconds` | `0` | Decorrelation time of the slow state; positive when the share is. |
| `pauses.ratePerSecond` | `0` | Poisson pause starts per second and direction, at most 1000 (a processing bound; delivery cannot resolve finer pauses); zero disables pauses. |
| `pauses.meanSeconds` | `0` | Mean exponential pause length. `ratePerSecond × meanSeconds` must be at most 1: merged pauses last `(e^(rate × mean) − 1) / rate` on average, so more covers nearly all time. |

`LatencyConfig::measured_route()` is the retained low-RTT official route fitted
on training sessions and checked on held-out ones by the
[network path study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-25-network-path/REPORT.md):

```json
{"roundTripSeconds": [0.004974],
 "variation": {"medianSeconds": 0.001901, "logSigma": 0.5066,
               "correlatedShare": 0.1771, "correlationSeconds": 1.837},
 "pauses": {"ratePerSecond": 1.646, "meanSeconds": 0.01393}}
```

Against uniform jitter, held-out KS distance improves from 0.30 to 0.04, but
p99 is overpredicted by about 30% and some correlation is missed. Base RTT assumes
within-tick replies, as the host gives; even splitting and mirrored uplink are
assumptions. Use only without a real path, for in-process bots or loopback players.

## NetworkConfig

Real WebSocket connections, without simulated latency. Only `url` is required.

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

All three durations must be ≥ 1 ns and representable as a platform `Duration`;
unrepresentable clock deadlines fail at run time. See the
[portable API](client-api.md) for polling, processing
measurements, recording bounds and reconnection behavior.

## Limits

Bounds have three kinds, with rationale beside the owning checks.

**Real limits** are errors because something breaks beyond them:

| Bound | Reason |
| --- | --- |
| Sliding `dt < 16 / 437.5 s` (≈ 36.57 ms) | A slide leaves the head one contact lead (16 × scale) behind the rim. A boosting minimum-size snake that moves that far in one tick, keeping its heading, lands on or outside the rim and dies instead of sliding. Hard boundaries kill on contact anyway and accept longer steps, up to the substep bound below. |
| `dt < 0.007 × 2⁵³` s (≈ 6.3 × 10¹³ s; smaller if `usize` is narrower) | Prey advance in 7 ms substeps and boosting snakes drop food every 0.155 s; one tick's count of each must be an exact integer that fits `usize`. This also keeps a tick of boost travel, `dt × 437.5`, finite. |
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
| `minimapSide ≥ 1`, positive intervals, `decisionTicks ≥ 1` | Empty maps and zero periods have no meaning. |

Finite values, nonnegative delays, probabilities in 0–1 and ordered ranges follow
from field meanings.

Work per step scales with config and always finishes: each prey advances
`dt / 0.007` fixed substeps; ambient refill makes about
`ambientReplenishPerSecond × dt` attempts, even when the target is full, since
its clock must advance bit for bit. Neither is capped; only the operator or local
controller supplies configs, never remote players.

**Valid but unmeasured** settings load with a warning (stderr for the command
line, `warnings` in `serve` reset replies, `UserWarning` in Python):

- `dt` outside the measured 6.94–7.07 ms server loop. Contact, pickup and event
  timing may differ; prey still move in fixed 7 ms substeps.
- `dt × 437.5 ≥ 126`: snakes that outrun the incremental
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
