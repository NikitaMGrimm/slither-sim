# Geometric controller: slither-sim-json-v2

Run `slither-sim serve`, from a checkout with `cargo run --release --locked -- serve`
or from a [release archive](../README.md#download-a-release) without Rust. One
controller owns a session of independent worlds and all their player slots. Send UTF-8 JSON objects, one per stdin line; read one
stdout reply per request. EOF stops the process. Rust callers can drive the
same sessions [in-process](#in-process-sessions) or use the library directly.

```json
{"protocol":"slither-sim-json-v2","result":{}}
{"protocol":"slither-sim-json-v2","error":"Reset first"}
```

Check `protocol`. Requests, configs and actions reject unknown fields; response
readers ignore additional fields. Error text is descriptive, not a stable code.
Malformed or rejected requests leave the stream usable; I/O failures stop it.
This interface is for trusted local controllers.

Sessions with several worlds step them in parallel, one world per task, on every
core; `slither-sim serve --threads N` caps the threads. Replies do not depend on
the thread count.

## Requests

### info

```json
{"op":"info"}
```

Works before reset. Returns `version`, `defaultConfig`, `defaultDecisionTicks`,
`maxAgents`, `maxWorlds`, `maxDecisionTicks`, the food `origins` codes and the
observation `precision`. The maxima are representation limits, not policies:
`maxAgents` and `maxDecisionTicks` are 4294967295 because frames hold `u32`
slot indices (`u32::MAX` means none) and tick counts; `maxWorlds` is the
platform's `usize::MAX`, since only `worlds × agents` must fit `usize` and memory
bounds worlds in practice.

### reset

```json
{"op":"reset","seed":7,"worlds":1,"agents":2,"decisionTicks":6,"config":{"radius":1000,"dynamicRadius":false,"foodCount":350}}
```

All fields except `op` are optional. Defaults: seed 1, one world, four agents,
six decision ticks and `Config::official(1)`. Every world shares `agents`,
`decisionTicks` and `config`; world `w` uses seed `seed + w` (wrapping at 2³²).
`config` and nested settings use camelCase; omitted fields use defaults. All
world settings are listed in the [configuration reference](configuration.md#world-config).
Invalid resets preserve the previous session.

Returns `seed`, the resolved `config`, `decisionTicks`, `warnings` and `worlds`,
one [frame](#frames) per world. `warnings` lists sentences about valid but
unmeasured settings, such as a `dt` outside the measured server loop; it is
empty otherwise. `slither-sim serve` also prints them to stderr, never stdout.
See [limits](configuration.md#limits). Snakes start on a deterministic ring
halfway to the boundary, facing inward; this is a synthetic test setup.

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
whole request is validated before any world advances; rejection changes nothing.

Each world holds its actions for `decisionTicks` physics ticks, stopping early if
all its snakes die. Time follows simulation ticks, independent of controller
compute time. `config.dt` is seconds per tick; defaults give a 42 ms decision
interval. There is no network delay, automatic respawn, reward, victory
condition or episode timeout. Use `reset` for new worlds.

Returns `worlds`, one frame per world. Frame rows, counts and offsets are
`u32`. A step whose frame cannot hold them, or whose pickup would overflow a
snake's mass, ends the session after its worlds advanced, so reset to continue.

### snapshot

```json
{"op":"snapshot","world":0}
```

Returns world `world`'s (default 0) whole-world `config`, `time`, `snakes`,
`food`, `prey` and cumulative `deaths` at full precision, with string slot IDs.
This debug view excludes internal motion/RNG state and is not a restorable
checkpoint. Keep it out of limited-visibility bot evaluations.

## Frames

A frame holds everything any living slot of that world can see, once, in
columns, plus each slot's view as row indices into those columns.

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
| `snakes` | `slot`, head `x`/`y`, `heading`, `speed`, `radius`, `length`, `segments`, `fam`, `score`, `kills`; anchors `bodyX`/`bodyY` |
| `food` | `x`, `y`, `value`, `size`, `origin` (index into `info.origins`: ambient, boost, death) |
| `prey` | `id`, `x`, `y`, `heading`, `targetHeading`, `speed`, `size` |

Row `r`'s body anchors are `bodyX[bodyStart[r]:bodyStart[r+1]]` and the same
range of `bodyY`, head to tail; `bodyStart` has one more entry than there are rows.
Rows and view lists follow world order, so a slot's view lists opponents by
slot and food in spawn order. Coordinates use world units; speeds use world
units per second.

Food and prey are filtered by the vision circle. If an opponent's head or any
anchor is visible, **its entire stored snake record is included**, even outside
that circle. This is authoritative simulation data, including metadata that may
be unavailable to a network client.

The controller can read every slot and debug snapshots. A shared competition
must separately restrict observations, isolate bots and define timing and
disconnection rules.

### Precision

Frames round coordinates (head, anchors, food, prey) to 0.01 world units,
headings to 0.0001 radians and speeds to 0.01 units per second; every other
number is exact. `info.precision` reports these steps. Rounding happens only
when a frame is written: the simulation, snapshots, the [portable bot
API](client-api.md) and the [WebSocket host](raw-server.md) keep full precision,
so players connected to a hosted server receive exactly what they did before.

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

A bot that observes every decision should keep one buffer and call
`observe_all_into` or `observe_selected_into`. They return the same observations
as `observe_all` and `observe_selected`, but refill the vectors already in the
buffer, which avoids most per-call allocation. On Windows, bot loops in the
[performance workloads](performance-workloads.md) ran 1.8–3.5× faster with
`observe_all_into` than with `observe_all`.

Set `default-features = false` on a path or Git dependency for the core alone:
no WebSocket/TLS dependencies, `client` module or CLI binary. Core simulation,
observations, JSON transport and the `basic`/`fidelity_replay` examples remain
available. `networking` is enabled by default and restores the full interface.

`Simulator::set_parallel(true)` lets `observe_all` and `observe_selected` split
large batches over the current [rayon](https://docs.rs/rayon) thread pool;
results are identical. It is off by default so that worlds you already run in
parallel do not compete for cores; size the pool with `RAYON_NUM_THREADS` or run
inside `ThreadPool::install`. `transport::serve` steps a session's worlds on the
same pool. Both need the default-enabled `parallel` feature.

```sh
cargo build --lib --no-default-features --locked
```

Before embedding:

- Pin a dependency version or Git revision and record the resolved `Config` and
  seed. Choose settings explicitly: default geometric `vision` is 1,500 units;
  defaults and seeded trajectories can change between versions. See
  [configuration](configuration.md) for the full settings.
- Handle fallible core calls before committing related command queues or timers
  in your application. A rejected operation preserves the simulator world, not
  changes you already made to your own driver state.
- Define admission and respawn policy in your application. If clearance is
  required, use the clearance checks below before respawning; respawn alone
  does not enforce it.

| Method | Purpose |
| --- | --- |
| `observe_selected(&mask)` | Shared-index batch observations, one result per slot. Unselected/dead slots are `None`; wrong mask length is an error. |
| `body_is_clear(&body, clearance)` | Check all supplied edges against living snakes and the 60-unit arena margin; invalid input returns false. |
| `respawn_body(slot, body, heading)` | Replace a dead slot using supplied geometry and reset per-life motion, boost, penalty, death-time and join-order state. |
| `set_ambient_rate(attempts_per_second)` | Change refill rate while preserving fractional placement credit. Zero discards pending credit and disables refill. |

Supplied bodies are head-first, one anchor per segment, with 2–430 finite points.
Respawn requires all points inside the arena; it does not enforce clearance or
reject self-intersections. Call `body_is_clear` separately for spawn exclusion.
The straight-body `spawn_is_clear` and `respawn` methods remain available.

Refill rates must be finite and nonnegative, small enough that one attempt
interval still changes the refill clock, and large enough to represent the
rescaled pending clock. Use the setter instead of directly
editing `config.ambient_replenish_per_second` when changing a running world.
Rejected operations leave the world unchanged. The exception is a pickup whose
gain would overflow its eater's mass: that step completes its tick without the
gain and returns an error, and every later step is rejected with it.

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

A NaN heading means no action; only dead slots may give it. Typed steps validate
like JSON steps, with the same error text where it applies, and reject the whole
step before any world advances. Worlds step on the rayon pool as in `serve`.

`FrameRef` columns hold the values the JSON frame prints, rounded alike, in
fixed types: `u64` prey `id`; `u8` food `origin` and death `reason`; `u32`
`ticks`, `living`, `slot`, `segments`, `kills`, death `victim`, `killer` and
`food`, and every row index and offset; `f64` everything else. Where the JSON
frame nests or prints `null`, the columns differ:

- Views are compressed sparse rows. `views.me[s]` is slot `s`'s own `snakes` row,
  or `transport::NONE` (`u32::MAX`) when the slot is dead. Its visible snakes are
  the rows `views.snakes[views.snakes_start[s]..views.snakes_start[s + 1]]`, and
  likewise `food` and `prey`. Each `*_start` has one entry more than there are
  slots; dead slots have empty ranges.
- `deaths` holds the columns `time`, `victim`, `killer` (`NONE` without a
  killer), `reason` (an index into `transport::REASONS`: body, boundary) and
  dropped `food`.
- `food.origin` indexes `transport::ORIGINS`, or is `NO_ORIGIN` (255) without an
  origin; `food.size` is NaN without a size.

A `FrameRef` borrows its session, so it ends before the next request or step.
Sessions are `Send`: run separate sessions on separate threads.
