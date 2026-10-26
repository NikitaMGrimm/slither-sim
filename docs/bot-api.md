# Geometric controller: slither-sim-json-v3

Run `slither-sim serve` from a [release archive](../README.md#download-a-release)
without Rust, or `cargo run --release --locked -- serve` from a checkout. One
controller owns independent worlds and all their player slots. Send one UTF-8
JSON object per stdin line; read one stdout reply per request. EOF stops the
process. Rust callers can use [in-process sessions](#in-process-sessions) or the
library directly.

```json
{"protocol":"slither-sim-json-v3","result":{}}
{"protocol":"slither-sim-json-v3","error":"Reset first","sessionEnded":false}
```

Check `protocol`. Requests, configs and actions reject unknown fields; response
readers ignore additional fields. Error text is descriptive, not a stable code.
I/O failures stop the stream. Use trusted local controllers.

Every error reply includes `sessionEnded`. When false, no world advanced and
the session is unchanged. When true, advancement or frame collection failed
(for example, accumulated pickup mass overflowed); all worlds were cleared.
Reset before stepping again. The stream survives malformed requests, rejections
and terminal session failures.

Worlds step in parallel, one task per world, using every core;
`slither-sim serve --threads N` caps threads without changing replies.

## Requests

### info

```json
{"op":"info"}
```

Works before reset. Returns `version`, `defaultConfig`, `defaultDecisionTicks`,
`maxAgents`, `maxWorlds`, `maxDecisionTicks`, the food `origins` codes and the
observation `precision`. Representation limits: `maxAgents` and
`maxDecisionTicks` are 4294967295 (`u32` slot indices and tick counts;
`u32::MAX` means no slot). `maxWorlds` is the platform's `usize::MAX`;
`worlds × agents` must fit `usize`, with memory limiting practical world counts.

### reset

```json
{"op":"reset","seed":7,"worlds":1,"agents":2,"decisionTicks":6,"config":{"radius":1000,"dynamicRadius":false,"foodCount":350}}
```

All fields except `op` are optional. Defaults: seed 1, one world, four agents,
six decision ticks and `Config::official(1)`. Every world shares `agents`,
`decisionTicks` and `config`; world `w` uses seed `seed + w` (wrapping at 2³²).
`config` and nested objects use camelCase [world settings](configuration.md#world-config)
with defaults for omitted fields and an optional [`preset`](configuration.md#presets),
such as `{"preset":"battledome"}`. Replies print resolved fields.
Invalid resets preserve the previous session.

Returns `seed`, the resolved `config`, `decisionTicks`, `warnings` and `worlds`,
one [frame](#frames) per world. `warnings` lists valid but unmeasured settings
([limits](configuration.md#limits)), such as `dt` outside the measured server
loop; otherwise it is empty. `serve` also prints warnings to stderr.
Snakes start on a deterministic ring halfway to the boundary, facing inward:
a synthetic test setup. With `config.spawnRadius`, they instead lie and head
counterclockwise along that ring; with `config.minScore`, they start at its floor.

The same build, config, seed and actions produce the same worlds. Cross-platform
bitwise floating-point equivalence is not promised; retain these inputs and the
commit when recording results.

### step

```json
{"op":"step","actions":[{"heading":[3.141592653589793,0],"boost":[false,true]}]}
```

`actions` has one entry per world, each with a `heading` and a `boost` array of
one entry per agent slot. Living slots require a finite heading in radians; dead
slots may use `null`. Headings are absolute: zero is +x and pi/2 is +y. The
whole request is validated before any world advances.

Each world holds its actions for `decisionTicks` physics ticks, stopping early if
all its snakes die. Time follows simulation ticks, independent of controller
compute time. `config.dt` is seconds per tick; defaults give a 42 ms decision
interval. There is no network delay, automatic respawn, reward, victory
condition or episode timeout. Use `reset` for new worlds.

Returns `worlds`, one frame per world. Frame rows, counts and offsets are `u32`.
Unrepresentable frames, numeric overflow and reply allocation failure end
the session after advancement, as described above.

### snapshot

```json
{"op":"snapshot","world":0}
```

Returns world `world`'s (default 0) whole-world `config`, `time`, `snakes`,
`food`, `prey` and cumulative `deaths` at full precision, with string slot IDs.
This debug view excludes internal motion/RNG state and is not a restorable
checkpoint. Keep it out of limited-visibility bot evaluations.

## Frames

A frame stores visible entities once in columns; each living slot's view
indexes those rows.

| Member | Meaning |
| --- | --- |
| `time`, `ticks` | World time, and ticks advanced by this request (0 on reset). |
| `living` | Surviving snakes. |
| `deaths` | Death events from this request: `time`, `victim` and `killer` slots, `reason` and dropped `food`. |
| `radius`, `vision` | Arena radius around the origin, and the vision radius. |
| `snakes` | One row per snake that some living slot sees, including each slot's own. |
| `food`, `prey` | One row per item that some living slot sees. |
| `views` | One entry per slot: `null` for dead slots, otherwise `self` (its own `snakes` row) and the `snakes`, `food` and `prey` rows it sees. |

Columns are arrays of equal length; row `r` is element `r` of each.

| Table | Columns |
| --- | --- |
| `snakes` | `slot`, head `x`/`y`, `heading`, `speed`, `radius`, `length`, `segments`, `fam`, `score`, `kills`, `killScore`; anchors `bodyX`/`bodyY` |
| `food` | `x`, `y`, `value`, `size`, `origin` (index into `info.origins`: ambient, boost, death) |
| `prey` | `id`, `x`, `y`, `heading`, `targetHeading`, `speed`, `size` |

- `segments`: whole body segments.
- `fam`: growth not yet turned into a segment, in segment units; it can briefly
  exceed 1.
- `length`: `segments + fam`, the body length in segments, not a distance.
- `score`: the size points the game displays, from `segments` and `fam`.
- Food `value`: the gain for a minimum-size eater (`length` 2); larger eaters
  gain less. Food `size`, when present, is the protocol pellet size, which sets
  the display and the size-dependent gain; without it the eater gains `value`.

`kills` counts the current life's kills. `killScore` counts the slot's kills
within the last `config.killScoreSeconds`, across lives, and is 0 without that
window. Death `reason` is `body`, `boundary` or `pit` (only with `config.pit`).

Row `r`'s body anchors are `bodyX[bodyStart[r]:bodyStart[r+1]]` and the same
range of `bodyY`, head to tail; `bodyStart` has one more entry than there are rows.
Rows and view lists follow world order, so a slot's view lists opponents by
slot and food in spawn order. Coordinates use world units; speeds use world
units per second.

Food and prey are filtered by the vision circle. If an opponent's head or any
anchor is visible, **its entire stored snake record is included**, even outside
that circle. This authoritative simulation data includes metadata that may be
unavailable to network clients.

The controller can read every slot and debug snapshots. A shared competition
must separately restrict observations, isolate bots and define timing and
disconnection rules.

### Precision

Frames round coordinates (head, anchors, food, prey) to 0.01 world units,
headings to 0.0001 radians and speeds to 0.01 units per second; every other
number is exact. `info.precision` reports these steps. Rounding happens only
when a frame is written; simulation, snapshots, the [portable bot
API](client-api.md) and the [WebSocket host](raw-server.md) keep full precision.

Nearest rounding changes a heading by at most 0.00005 radians and a point by at
most 0.0071 world units. The [precision study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-24-observation-precision/REPORT.md)
compares those bounds with packet grids and simulation scales. Rounding can
still change a bot's choice at a decision threshold.

## Transport and official-server compatibility

See the [Python](../examples/bot.py) and [Node.js](../examples/bot.mjs) controllers.
`serve` does not accept binary WebSocket clients or reproduce received packet
coverage, prediction, receipt ages or network latency. Use the
[portable bot API](client-api.md) for received-state inputs and the
[WebSocket host](raw-server.md) for socket connections.

## Rust library

`Simulator::new`, `add`, `step`, `observe` and `observe_all` provide direct access;
see [the basic example](../examples/basic.rs). Rust callers can build their own
admission, respawn and match rules around the core.

Use `observe_all_into` or `observe_selected_into` to reuse a buffer's vectors
and avoid most per-call allocation, with the same observations as `observe_all`
and `observe_selected`. Windows bot loops ran 1.8–3.5× faster with
`observe_all_into` than `observe_all`; see [workloads](performance-workloads.md).

Set `default-features = false` on a path or Git dependency for the core alone:
no WebSocket/TLS dependencies, `client` module or CLI binary. Core simulation,
observations, JSON transport and the `basic`/`fidelity_replay` examples remain
available. `networking` is enabled by default and restores the full interface.

`Simulator::set_parallel(true)` splits large `observe_all` and `observe_selected`
batches over the current [rayon](https://docs.rs/rayon) pool without changing
results. It defaults off to avoid competing with parallel worlds. Size the pool
with `RAYON_NUM_THREADS` or use `ThreadPool::install`. `transport::serve` steps
worlds on the same pool. Both need the default-enabled `parallel` feature.

```sh
cargo build --lib --no-default-features --locked
```

Before embedding:

- Pin a dependency version or Git revision; record the resolved `Config` and
  seed. Defaults and seeded trajectories can change between versions. Choose
  [settings](configuration.md) explicitly, including geometric `vision`.
- Handle fallible core calls before committing related command queues or timers
  in your application. A rejected operation preserves the simulator world, not
  changes you already made to your own driver state.
- Define admission and respawn policy in your application.

| Method | Purpose |
| --- | --- |
| `observe_selected(&mask)` | Shared-index batch observations, one result per slot. Unselected/dead slots are `None`; wrong mask length is an error. |
| `body_is_clear(&body, clearance)` | Check all supplied edges against living snakes and the 60-unit arena margin; invalid input returns false. |
| `respawn_body(slot, body, heading)` | Replace a dead slot using supplied geometry and reset per-life motion, boost, penalty, death-time and join-order state. With `minScore`, the body needs at least the floor's segments; at exactly that many, the life starts at the floor's mass. |
| `spawn_body(head, heading)` | The configured new-life body and heading at `head`. With `spawnRadius`, its anchors lie along the circle through `head`, turning the way `heading` leans, heading along it; an error if that circle cannot hold them. Otherwise two straight segments behind `heading`. Check it with `body_is_clear`. |
| `set_ambient_rate(attempts_per_second)` | Change refill rate while preserving fractional placement credit. Zero discards pending credit and disables refill. |

Supplied bodies are head-first, one anchor per segment, with 2–430 finite points.
Respawn requires all points inside the arena; it does not enforce clearance or
reject self-intersections. Call `body_is_clear` separately for spawn exclusion.
The straight-body `spawn_is_clear` and `respawn` methods remain available.

Refill rates must be finite and nonnegative, small enough that one attempt
interval still changes the refill clock, and large enough to represent the
rescaled pending clock. Use the setter instead of directly
editing `config.ambient_replenish_per_second` when changing a running world.
Rejected operations leave the world unchanged. Numeric overflow during a tick
(pickup mass or prey motion) completes that tick without the invalid update,
returns `CoreStepError::Stopped(cause)`, and every later step returns
`CoreStepError::Rejected` with `ErrorKind::WorldStopped`. The original cause stays
available through `Simulator::status()`: `WorldStatus::Running` or
`WorldStatus::Stopped(&SimError)`. Action validation failures return
`CoreStepError::Rejected(cause)` before any state changes. `step_profiled` has the
same lifecycle.

Other fallible core calls return `SimError`. Replace `Result<_, String>`
propagation and prose matching with `SimError` (or a boxed `std::error::Error`)
and checks on `kind`/`code()`. Causes carry optional zero-based `world`/`slot`
and string `field` locations, a `Constraint` where applicable, and
`expected`/`actual` counts for action shape errors. Config fields use JSON names,
with indexed paths for radius samples. `ErrorCode` numeric values 1–17 and
`as_str()` names are stable; message wording is diagnostic. `Display`/`Error`
retain human messages and chained causes. Successful steps allocate no error
strings. JSON, C and Python errors and contract identifiers stay unchanged.

### Frozen Forager

The dependency-free `bot` feature exposes `bot::PolicyId::A1` and
`bot::Forager::new(PolicyId)`. Its `decide(&mut self, &Observation, &Timing,
&[Intent]) -> Action` method uses the received observation and explicit timing.
`Timing`, `Sent` and `Intent` live in `bot`; opponent intentions follow observation
order, with `Intent::default()` representing unknown wanted heading.
Parameters are private and fully resolved for the named release.

Use `default-features = false, features = ["bot"]` for an offline policy without
networking or Rayon. Core-only, C ABI and Python builds do not enable `bot`.
`networking`, enabled by default, includes it and exposes `client::ForagerBot::new(PolicyId::A1)`,
implementing the existing `client::Bot`. The adapter derives timing and intentions
from the complete `ClientFrame` and returns `None` outside a compatible playing
frame with an observation. Existing host, native background bots, examples and
JSON operations retain their current behavior.

One policy/adapter instance can be reused for sequential decisions across seats:
scratch buffers retain no policy state. Keep each seat's received frame, sent
commands and lifecycle separate. This heuristic does not guarantee safety;
cross-platform floating-point identity is subject to OS math libraries.
The [port evidence](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-26-forager-port/REPORT.md)
pins the original kernel, immutable parameters, bounded fixtures and validation
limits. A4-mean is retained only as private test data and is not selectable.

### In-process sessions

`transport::Session` is the session `serve` runs, without the stream. JSON
requests and typed steps act on the same worlds and may be mixed.

| Method | Purpose |
| --- | --- |
| `Session::new()` | An empty session; reset it before stepping. |
| `request(line)` | Answers one JSON request line with exactly the reply line `serve` writes, both without the newline. Errors become error replies. |
| `step(&headings, &boosts)` | Steps every world like a JSON `step` without writing JSON. Both slices hold `worlds × agents` entries, world-major: slot `s` of world `w` is entry `w × agents + s`. |
| `frame(w)` | World `w`'s current frame as a `FrameRef` of read-only columns, after any reset or step; `None` before reset or for unknown worlds. |
| `worlds()`, `agents()` | Session shape; 0 before the first reset. |

A NaN heading means no action; only dead slots may give it. Typed steps return
`Result<(), transport::StepError>`: `Rejected(SimError)` leaves the session
unchanged; `Ended(SimError)` clears every world after an
advancement or frame-collection failure, including worlds that succeeded.
The cause names the first failing world in stable world order, including when
worlds advance in parallel. Formatting `StepError` retains the JSON error text.
Then `frame` returns `None`, `worlds()` and `agents()` return 0; reset to continue.

`FrameRef` columns hold the values the JSON frame prints, rounded alike, in
fixed types: `u64` prey `id`; `u8` food `origin` and death `reason`; `u32`
`ticks`, `living`, `slot`, `segments`, `kills`, `kill_score`, death `victim`, `killer` and
`food`, and every row index and offset; `f64` everything else. Where the JSON
frame nests or prints `null`, the columns differ:

- Views are compressed sparse rows. `views.me[s]` is slot `s`'s own `snakes` row,
  or `transport::NONE` (`u32::MAX`) when the slot is dead. Its visible snakes are
  the rows `views.snakes[views.snakes_start[s]..views.snakes_start[s + 1]]`, and
  likewise `food` and `prey`. Each `*_start` has one entry more than there are
  slots; dead slots have empty ranges.
- `deaths` holds the columns `time`, `victim`, `killer` (`NONE` without a
  killer), `reason` (an index into `transport::REASONS`: body, boundary, pit) and
  dropped `food`.
- `food.origin` indexes `transport::ORIGINS`, or is `NO_ORIGIN` (255) without an
  origin; `food.size` is NaN without a size.

A `FrameRef` borrows its session, so it ends before the next request or step.
Sessions are `Send`: run separate sessions on separate threads.
