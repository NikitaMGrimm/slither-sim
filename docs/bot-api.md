# Geometric controller: slither-sim-json-v1

Run `slither-sim serve`. One controller owns the world and all player slots.
Send UTF-8 JSON objects, one per stdin line; read one stdout reply per request.
EOF stops the process. Rust callers can use the library directly.

```json
{"protocol":"slither-sim-json-v1","result":{}}
{"protocol":"slither-sim-json-v1","error":"Reset first"}
```

Check `protocol`. Requests, configs and actions reject unknown fields; response
readers ignore additional fields. Error text is descriptive, not a stable code.
Malformed or rejected requests leave the stream usable; I/O failures stop it.
This interface is for trusted local controllers.

## Requests

### info

```json
{"op":"info"}
```

Works before reset. Returns `version`, `defaultConfig`, `defaultDecisionTicks`,
`maxAgents` and `maxDecisionTicks`.

### reset

```json
{"op":"reset","seed":7,"agents":2,"decisionTicks":6,"config":{"radius":1000,"foodCount":350}}
```

All fields except `op` are optional. Defaults: seed 1, four agents, six decision
ticks and `Config::default()`. Agents must be 1–128 and decision ticks 1–1000.
`config` and nested settings use camelCase; omitted fields use defaults.
All world settings are listed in the [configuration reference](configuration.md#world-config).
Invalid resets preserve the previous world.

Returns `seed`, resolved `config`, `decisionTicks`, `observations` and `time`.
Slots have stable string IDs (`"0"`, `"1"`, …). Snakes start on a deterministic
ring halfway to the boundary, facing inward; this is a synthetic test setup.

The same build, config, seed and actions produce the same world. Cross-platform
bitwise floating-point equivalence is not promised; retain these inputs and the
commit when recording results.

### step

```json
{"op":"step","actions":[{"heading":3.141592653589793,"boost":false},{"heading":0,"boost":false}]}
```

Supply one entry per agent. Living slots require a finite heading in radians and
a boost boolean; dead slots may be `null`. Headings are absolute: zero is +x and
pi/2 is +y. The whole batch is validated before advancing; rejection changes nothing.

Actions are held for `decisionTicks` physics ticks, stopping early if all snakes
die. Time follows simulation ticks, independent of controller compute time.
`config.dt` is seconds per tick; defaults give a 42 ms decision interval.
There is no network delay, automatic respawn, reward, victory condition or episode
timeout. Use `reset` for a new world.

| Result | Meaning |
| --- | --- |
| `observations` | Stable slots; dead slots are `null`. |
| `deaths` | Death events from this request only. |
| `time`, `elapsedSeconds`, `ticks` | World time and interval actually advanced. |
| `living` | Surviving snakes. |

### observe

```json
{"op":"observe","slot":0}
```

Returns `{"observation": ...}`, matching reset/step without changing time,
visibility or RNG state. Dead slots return `null`; unknown slots return an error.

### snapshot

```json
{"op":"snapshot"}
```

Returns whole-world `config`, `time`, `snakes`, `food`, `prey` and cumulative
`deaths`. This debug view excludes internal motion/RNG state and is not a
restorable checkpoint. Keep it out of limited-visibility bot evaluations.

## Observation boundary

Observations contain `time`, `self`, `opponents`, `food`, `prey`, `arena` and
`visionRadius`. Coordinates and body anchors use world units; speed is world
units per second. Anchors run head to tail, separately from the current head.
`arena.center` is the origin.

Food and prey are filtered by the vision circle. If an opponent's head or any
anchor is visible, **its entire stored snake record is included**, even outside
that circle. This is authoritative simulation data, including metadata that may
be unavailable to a network client.

The controller can read every slot and debug snapshots. A shared competition
must separately restrict observations, isolate bots and define timing and
disconnection rules.

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

Set `default-features = false` on a path or Git dependency for the core alone:
no WebSocket/TLS dependencies, `client` module or CLI binary. Core simulation,
observations, JSON transport and the `basic`/`fidelity_replay` examples remain
available. `networking` is enabled by default and restores the full interface.

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

Refill rates must be finite and within 0–10,000; a rate too small to represent
the rescaled pending clock is rejected. Use the setter instead of directly
editing `config.ambient_replenish_per_second` when changing a running world.
Rejected operations leave the world unchanged.
