# Geometric controller: slither-sim-json-v5

Run `slither-sim serve` from a [release archive](../README.md#download-a-release)
without Rust, or `cargo run --release --locked -- serve` from a checkout. One
controller owns independent worlds and all their player slots. Send one UTF-8
JSON object per stdin line; read one stdout reply per request. EOF stops the
process. Rust callers can use [in-process sessions](#in-process-sessions) or the
library directly.

```json
{"protocol":"slither-sim-json-v5","result":{}}
{"protocol":"slither-sim-json-v5","error":{"code":11,"rule":null,"world":null,"slot":null,"field":null,"message":"Reset first"},"sessionEnded":false}
```

Check `protocol`. Requests, configs and actions reject unknown fields; response
readers ignore additional fields. Error `message` is descriptive; numeric `code`, optional `rule` and locations are stable.
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

Works before reset. Returns `version`, `rulesRevision` (`slither-sim-rules-v3`), `defaultConfig`, `defaultDecisionTicks`,
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
Default and official worlds are first sized for `agents`
([population sizing](configuration.md#population-sizing)).
Invalid resets preserve the previous session.

Returns `seed`, the resolved `config`, `decisionTicks`, `warnings` and `worlds`,
one [frame](#frames) per world. `warnings` lists valid but unmeasured settings
([limits](configuration.md#limits)), such as `dt` outside the measured server
loop; otherwise it is empty. `serve` also prints warnings to stderr.
Snakes start on a deterministic ring halfway to the boundary, facing inward:
a synthetic test setup. With `config.spawnRadius`, they instead lie and head
counterclockwise along that ring; with `config.minScore`, they start at its floor.

Optional `placement` defaults to `{"kind":"testRing"}`. This checks containment
but makes no neighbor-clearance promise. `{"kind":"admitted"}` instead tries
64 sampled poses per slot with preferred clearance 600; override these with
`attempts` (positive u32) and `preferredClearance` (finite, nonnegative).
It selects the first pose meeting that clearance, otherwise the first best
positive contact-margin pose. Exhaustion rejects the entire reset. Placement
uses a separate seed stream (`seed ^ 0x706c6163`), leaving food/prey draws intact.
This bounded core policy does not change host admission or retry behavior.

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

Returns world `world`'s (default 0) whole-world `config`, live `radius`, `time`, `snakes`,
`food`, `prey` and cumulative `deaths` at full precision, with string slot IDs.
This debug view excludes internal motion/RNG state and is not a restorable
checkpoint. `config.radius` retains the initial setting after startup sizing;
`radius` follows samples and the controller. This changed config meaning is the
geometric v4 cutover from v3. Keep snapshots out of limited-visibility bot evaluations.

## Structured errors

Geometric v5 replaces the v4 error string with an object containing `code`,
`rule`, `world`, `slot`, `field` and `message`. Absent rule/location values are
null. Indices are zero-based; fields retain indexed paths. `sessionEnded`
remains outside the object and preserves the rejected-versus-ended lifecycle.
C inspection and Python exceptions use the same cause and location values.

| Kind (`ErrorCode`) | Code |
| --- | ---: |
| InvalidRequest | 1 |
| InvalidConfig | 2 |
| ActionShape | 3 |
| InvalidAction | 4 |
| UnknownSlot / UnknownWorld / UnknownEntity | 5 / 6 / 7 |
| DuplicateId / InvalidState / PlacementExhausted | 8 / 9 / 10 |
| NotReset / WorldStopped | 11 / 12 |
| UnsupportedCheckpoint / InvalidCheckpoint | 13 / 14 |
| NumericOverflow / RepresentationLimit / AllocationFailed | 15 / 16 / 17 |

Constraint codes are Missing 1, NonFinite 2, Negative 3, NonPositive 4,
OutOfRange 5, LengthMismatch 6, Duplicate 7, Ordering 8, OutsideArena 9 and
InconsistentState 10. Codes identify kinds/constraints; diagnostic messages
may change. Location fields name the input being validated: native flat-array
shape errors use `headings`/`boosts`, while nested JSON array errors name the
world's `heading`/`boost` field. Future codes are additive; handle unknown values without treating
an error as success. Codes 18/19 are reserved for C pointer/panic boundaries.

## Frames

A frame stores visible entities once in columns; each living slot's view
indexes those rows.

| Member | Meaning |
| --- | --- |
| `time`, `ticks` | World time, and ticks advanced by this request (0 on reset). |
| `living` | Surviving snakes. |
| `deaths` | Death events from this request: `time`, `victim` and `killer` slots, `reason` and dropped `food`. |
| `radius`, `vision` | Live arena radius around the origin, and the vision radius. |
| `rules` | One world-rules object, shared by every slot in this frame; see below. |
| `snakes` | One row per snake that some living slot sees, including each slot's own. |
| `food`, `prey` | One row per item that some living slot sees. |
| `views` | One entry per slot: `null` for dead slots, otherwise `self` (its own `snakes` row) and the `snakes`, `food` and `prey` rows it sees. |

Every frame includes `rules`, even with no living slots:

```json
{"boundaryMode":"sliding","pitRadius":null,"minScore":null,"killScoreSeconds":null,"freeBoostAtFloor":false}
```

`boundaryMode` is `sliding` or `hard`; the optional numbers are null when absent.
`freeBoostAtFloor` is true when the configured permanent `minScore` floor makes
boosting free at that floor. Overrides are reflected directly; a preset name
is not a rules identity. Radius remains separate runtime state;
`rules.pitRadius` is the live pit radius and follows the wall in seat-following
circular floor worlds. The other mode settings remain fixed until the world is replaced. Portable received-state
frames contain only received information and do not invent simulated pit/floor metadata.

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
when a frame is written; simulation and snapshots keep full precision.

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

`Simulator::new(WorldOptions { seed, config, population })` initializes a world;
see [the basic example](../examples/basic.rs). `InitialPopulation` selects
`Empty`, `Dormant { ids }`, or `Placed { ids, placement }`. IDs are unique,
ordered, stable slots. Dormant slots are inactive and have no death timestamp.
`Placement::default()` is `TestRing`; `Admitted` uses the bounded search above.
Generated ecology is independent of the initial snake population. Configure
`Config::official(n)` explicitly when its ecology should be sized for n snakes.

Simulator fields are private. `config()`, `snakes()`, `food()`, `prey()`,
`deaths()`, `pickup_events()` and `food_spawn_events()` borrow existing storage;
`radius()` and `time()` return scalars. Config retains its existing flat keys
and retains the initial radius after startup sizing; `radius()` reports the live
boundary. Radius evolution keeps the initial Config as the density reference;
seat-following derives effective ecology targets from live area. An explicit
`set_ambient_rate` updates the initial-area attempt rate. No mutable slices or config setters are
exposed. Detached `Snake`, `Food` and `Prey` values remain editable.

`world.rules()` returns a copyable `WorldRules` without allocating.
`world.geometric_context(slot)?` returns those rules and the slot's rolling
`kill_score`, including credits retained across lives and their expiration.
Pass it beside `world.observe(slot)` to a raw Rust bot driver. It is not added
to `Observation` or `Snake`; unknown slots return `UnknownSlot` with the slot.

Use `append_dormant`, `spawn`, `retire` and the existing respawn methods for
slot lifecycle. `spawn(id, body, heading)` validates geometry, mass floor and
containment before appending every companion together. `retire(slot)` marks a
slot inactive without death food or a fabricated death event; slots never move.
Respawn and new replay lives clear per-life boost, speed, kiosk, pit and death
state and assign new join order, retaining rolling kill history.

`import_life(slot, ReplayLife)` installs a checked new replay life atomically,
including `InitialSpeed::Infer(observed_speed)` or `Known(SpeedState)`.
`ReplayLife::from_observed` takes a detached snake and explicit speed choice;
body length supplies segment count, and mass supplies display values. The old
`add`, live `update_mass`, and two-call `initialize_speed_state` are removed.
`correct_mass(slot, body, fam)` and `correct_pose(slot, head, body, heading)`
preserve speed/progress, boost, kiosk credit/timers, join order, kills and pit
history. Pose correction retains segment count. Replay corrections allow finite
overlaps and out-of-arena geometry within the checkpoint continuation envelope
(positions at most 2e150 for living snakes and 3e150 for inactive snakes/food;
headings and snake speed × dt at most 1e150 in magnitude). The 430-segment
spawn limit is not a replay cap.

`from_scenario(seed, config, Scenario { snakes, food, prey })` installs ordered
`(id, ReplayLife)` slots and `ReplayPrey { prey, mode }` at time zero without
generated ecology or placement draws. It labels supplied state; exact historical
continuation uses checkpoints. Checked `replace_food`, `insert_food` and
`remove_food` preserve order and invalidate occupancy caches; indices are immediate
positions, not durable identities. The corresponding prey operations enforce
unique IDs and synchronize modes/decision state. `record_events(EventRecording)`
enables step-local pickup/birth telemetry, retaining capacity when already enabled.
`deaths()` remains cumulative history for cursor-based consumers.

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
| `set_ambient_rate(attempts_per_second)` | Change initial-area refill rate while preserving fractional placement credit; seat-following scales it with live area. Zero discards pending credit and disables refill. |

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
strings. JSON, C and Python error handling is unchanged by the radius split;
geometric v5 now carries stable cause codes and locations.

### Frozen Forager

The dependency-free `bot` feature exposes `bot::Forager::new()` and
`Forager::default()`, both using the adopted A4-mean parameters. The
`decide(&mut self, &Observation, &Timing, &[Intent]) -> Action` method uses the
received observation and explicit timing.
`Timing`, `Sent` and `Intent` live in `bot`; opponent intentions follow observation
order, with `Intent::default()` representing unknown wanted heading.
Parameters are private and fully resolved for the named release.

Use `default-features = false, features = ["bot"]` for an offline policy without
networking or Rayon. Core-only, C ABI and Python builds do not enable `bot`.
`networking`, enabled by default, includes it and exposes `client::ForagerBot::new()`
and `ForagerBot::default()`, implementing the existing `client::Bot`. The adapter derives timing and intentions
from the complete `ClientFrame` and returns `None` outside a compatible playing
frame with an observation. Host bots, native background bots and the Rust
examples use this default adapter; JSON operations are unchanged.

Local host and native background bots use `ForagerBot::with_world_rules(&Config)`
to enter the configured one-way pit and treat its edge as a wall from inside.
The pit follows the live radius received in each frame. The adapter adds no
fields to `ClientFrame`; network bots must obtain mode rules separately.
Offline callers can supply the live pit radius to `Forager::decide_with_pit`.
Without a pit these calls preserve the frozen A4 decisions.

One policy/adapter instance can be reused for sequential decisions across seats:
scratch buffers retain no policy state. Keep each seat's received frame, sent
commands and lifecycle separate. This heuristic does not guarantee safety;
cross-platform floating-point identity is subject to OS math libraries.
The [port evidence](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-26-forager-port/REPORT.md)
pins the original kernel, immutable parameters, bounded fixtures and validation
limits. The [A4 adoption note](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-26-a4-adoption/REPORT.md)
records selected held-out gameplay and cost results from private tuning tooling,
including the maintainer's live-mean cost gate and the earlier tail-limit failures.

### Checkpoints

Use `serde_json::to_vec(&world.checkpoint()?)` for exact core continuation;
`Simulator` and its runtime controls no longer implement serde. Load with
`Simulator::from_checkpoint(&bytes, RestoreOptions::default())`, or use
`world.restore(&bytes, options)` to validate a complete candidate before replacing
the current world. Export also validates the complete state. Load errors carry
`UnsupportedCheckpoint` or `InvalidCheckpoint` causes and field locations.

The envelope is `slither-sim-checkpoint-v2`, with
`rulesRevision: "slither-sim-rules-v3"`, producer provenance, today's flat Config
and ordered slot/entity records. `config.radius` is the initial radius and
`state.radius` is the live radius; loading preserves both. Config must pass the
same validation as a new world's, and the live radius must equal `config.radius`
until the dynamic controller, circular host admission or a recorded update
could have moved it. Circular host admission may grow it before the first tick.
Version 1 is rejected because it stored the live radius in Config and cannot recover the initial value.
Rules v1/v2 are rejected: seat-following and live circular pit/spawn scaling
change tick behavior. The host queue clock is not core checkpoint state. No
controller state is added; occupancy uses living state and death timestamps,
and refill retains fractional credit in initial-area seconds.
The checkpoint includes RNG, timers and terminal causes;
debug `snapshot` replies and old Simulator JSON are rejected. Unknown fields and
missing companion records reject, as do radii, headings and one tick of snake
travel above 1e150, living snake coordinates beyond 2e150, and dead snake or food
coordinates beyond 3e150, which would overflow later steps. The live
radius may lie inside the initial spawn ring or pit after measured updates. Producer version/commit do not select rules.
`RestoreOptions` chooses `EventRecording { pickups, food_births }` and, with the
parallel feature, `parallel`; event buffers start empty and caches are rebuilt.
Sessions, sockets and host/client delivery are outside the format.

`ErrorCode`, `ErrorKind` and `Constraint` are non-exhaustive; include a wildcard
arm when matching them outside the crate.

### In-process sessions

`transport::Session` is the session `serve` runs, without the stream. JSON
requests and typed steps act on the same worlds and may be mixed.

| Method | Purpose |
| --- | --- |
| `Session::new()` | An empty session; reset it before stepping. |
| `reset(ResetOptions)` | Prepare and atomically commit fully initialized worlds and frames, returning `ResetSummary`. |
| `Session::prepare_reset(options)` | Build an opaque `PreparedReset` without changing any session; inspect `summary()` before committing. |
| `commit_reset(prepared)` | Install prepared worlds and frames without failure or allocation, returning the summary. |
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
