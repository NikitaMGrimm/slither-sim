# Changelog

## Unreleased

## [0.6.2](https://github.com/NikitaMGrimm/slither-sim/compare/v0.6.1...v0.6.2) - 2026-10-26

### Added

- `slither-sim host --stats SECONDS` prints the tick rate, deferred ticks,
  game-loop load and player count every SECONDS.
- Evidence for browser mods that use an older login: an official server answers
  it with protocol 13, which this server doesn't serve yet, and a tested mod
  can't play protocol 19 ([report](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-legacy-login/REPORT.md)).
- PR fidelity reports add fixture-backed food nutrition, death-pellet size and
  prey residuals, show units and source fixtures, and summarize regression tests
  in one pass/fail count line.
- PR comparisons focus timing on two regression canaries, official workloads and
  500/80-client socket hosts, retain ten state traces, and run both runtimes with
  the head workload configuration. Manual calibration retains CPU/wall noise.

### Changed

- **Breaking:** Battledome grows above 32 players so every spawn still passes
  the strict safety checks. Wall, pit and spawn band scale together, reaching
  3.25 times the base radius at 80 seats; a spawn queue waiting 2–4 seconds gets
  up to twice that. Games of 16–32 players are unchanged. In local runs at 80
  seats everyone spawned within 0.6 s and spawn search fell from 1.7 ms to under
  0.01 ms per tick ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/battledome/2026-10-26-spawn-capacity/REPORT.md)). When the growing pit passes over a snake
  that hasn't entered it, the snake can still leave. `dynamicRadius: false` keeps
  the fixed arena ([configuration](docs/configuration.md#battledome)).
- **Breaking:** checkpoints use rules revision `slither-sim-rules-v3` and reject
  v1/v2. Frame `rules.pitRadius` follows the live pit in Battledome.

### Performance

- Host broadcasts test prey against shared bounding boxes before the exact
  distance check, with byte-identical output: 4.6% fewer instructions on the
  official 500-client workload; the live CPU change stayed within noise
  ([measurements](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-host-prey-visibility/REPORT.md)).

## [0.6.1](https://github.com/NikitaMGrimm/slither-sim/compare/v0.6.0...v0.6.1) - 2026-10-26

### Added

- [Linux hosting guide](docs/hosting.md): release install, systemd service,
  optional TLS proxy, join check, updates and resource use.
- Python `Session.config` returns the resolved world config of the last reset
  as a new camelCase dict, matching a JSON reset reply's `config`
  ([Python](docs/python.md#api)).

### Performance

- Host broadcasts visit only the snakes each recipient already knows instead of
  every world slot, with byte-identical output: about 6% less host CPU per tick
  with 500 clients in the [single-core measurements](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-host-known-slots/REPORT.md).

## [0.6.0](https://github.com/NikitaMGrimm/slither-sim/compare/v0.5.0...v0.6.0) - 2026-10-26

### Breaking

- Official and host arenas follow occupied seats by default (`dynamicRadius`
  is now true). Joins and leaves move the wall; deaths and respawns don't. It
  grows at once when players join and shrinks at 23.4375 units/s when they
  leave, ignoring changes within 3% of the current radius. Hosts start sized
  for their bots instead of every reserved seat. Food and prey targets follow
  the live area; a shrinking wall removes the food outside it and surplus prey.
  This is our design, not a recovered official law, and it replaces the
  living-count controller. Checkpoints stay `slither-sim-checkpoint-v2`; the
  rules revision becomes `slither-sim-rules-v2`, so v1 checkpoints are rejected
  ([population sizing](docs/configuration.md#population-sizing)).
- Geometric JSON is `slither-sim-json-v5` and the C ABI is `slither-sim-abi-v3`
  (`SLITHER_ABI_VERSION` 3). Frames carry each world's fixed rules: boundary
  mode, pit radius, minimum score, kill-score window and free boost at the
  floor. They appear as `rules` in JSON, `FrameRef::rules` in Rust, a frozen
  rules object on Python frames and an inline `slither_world_rules` at the end
  of `slither_frame`. Errors are objects with stable `code` and `rule` numbers
  and optional world, slot and field. Python exceptions carry the same
  attributes, and C adds `slither_read_error`. Rust drivers get
  `Simulator::rules()` and `geometric_context(slot)`. Rebuild C callers;
  checkpoints and the portable `client` contracts are unchanged
  ([frames](docs/bot-api.md#frames), [C ABI](docs/c-api.md#compatibility)).
- The A1 Forager policy is removed: A4-mean is the only bot policy, and
  `PolicyId` is gone (`Forager::new()`, `ForagerBot::new()`). The
  `cautious_bot` example is now `forager` and keeps only its core-observation
  path; `portable_bot` runs the policy on received frames.
- Config `radius` is the initial, population-sized radius; the live boundary
  is runtime state in `Simulator::radius()`, frames and a new snapshot `radius`
  field. Checkpoints are `slither-sim-checkpoint-v2`, store both radii and
  reject v1 ([checkpoints](docs/bot-api.md#checkpoints)).

### Added

- `transport_parity.py measure` records view strata (peer-count bins with
  workload proxies), and `compare --view-matched` adds an opt-in per-capture
  gate that reports thin bins and workload mismatches as inconclusive
  ([transport parity](docs/research-workflow.md#transport-parity)).
- A retained [official food-interest pilot](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-food-interest/REPORT.md):
  one avoidance abort ended the route after three completed slots, failing the
  frozen feasibility gate; no food-model change.
- A two-client `paired-collision` collector and a frozen official pilot plan
  for measuring the collision hitbox and head-on resolution order
  ([research workflow](docs/research-workflow.md#paired-collision-trials)). The
  first official pilot failed feasibility without a qualified encounter
  ([execution](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-collision-pilot/EXECUTION.md)).

### Fixed

- Checkpoints reject a config value with a noncanonical JSON type, such as
  `"prey": []`. Serde had read such arrays positionally and filled the
  missing settings from defaults. The checkpoint's config key list now comes
  from the checkpoint record itself.
- `SimulationConfig::validate()` checks the population-sized world that
  backends and hosts run. It no longer rejects a spawn ring that fits the sized
  arena or accepts a prey target that scales past the u16 prey IDs.
- Simulation configs must be JSON objects; arrays were read positionally and
  started defaults. A repeated key now keeps its last value, as in world
  configs, instead of being rejected.

## [0.5.0](https://github.com/NikitaMGrimm/slither-sim/compare/v0.4.0...v0.5.0) - 2026-10-26

### Breaking

- Host settings are grouped: `maxClients` → `players.max`, `bots` →
  `bots.count`, `botThreads` → `bots.threads` and `simulation.latency` →
  `bots.latency`. Sockets no longer inherit the simulation's latency;
  `players.latency` sets their added link delay, with zero processing. Flat keys
  and a non-default host `simulation.latency` are rejected
  ([HostConfig](docs/configuration.md#hostconfig)).
- Official worlds keep their initial radius, sized for their population. The
  provisional dynamic controller, which did not match official servers, is
  opt-in. Hosts, resets and simulation backends size official worlds for all
  their snakes, scaling food with area and keeping the official prey density.
  Fixed radii, sandbox and Battledome are unchanged
  ([population sizing](docs/configuration.md#population-sizing)).
- `SimulationConfig.broadcastTicks` is removed: gameplay updates go out every
  tick, and configs that set it are rejected
  ([SimulationConfig](docs/configuration.md#simulationconfig)).
- `LatencyConfig.jitterSeconds` and `congestion` are replaced by `variation`
  (lognormal per-message delay with a slow correlated part) and `pauses` (short
  delivery holds per direction); the old fields are rejected.
  `LatencyConfig::measured_route()` is fitted to one home connection to an
  official server; near-server captures attribute its pauses to that connection
  and its collector, not the server
  ([variation and pauses](docs/configuration.md#variation-and-pauses)).
- The portable `client` JSON protocol is now `slither-client-json-v2`, because
  v0.4.0 requests with the removed broadcast and latency fields are rejected.
  The frame contract stays `slither-client-v2`
  ([portable bots](docs/client-api.md#interface-versions)).
- Sessions tell a rejected step (state unchanged) from a terminal failure (all
  worlds cleared; reset required). For example, a pickup that would overflow a
  snake's mass completes its tick without the gain and stops the world. `serve`
  uses `slither-sim-json-v3` with `sessionEnded` in error replies, Rust returns
  `transport::StepError::{Rejected, Ended}`, C returns `SLITHER_ENDED` (4) and
  Python raises `SessionEndedError`. Successful replies are unchanged
  ([session errors](docs/bot-api.md)).
- C ABI `slither-sim-abi-v2` (`SLITHER_ABI_VERSION` 2) adds `snake_kill_score`
  after `snake_kills` in `slither_frame` and the `SLITHER_DEATH_PIT` death code;
  rebuild against the new header ([C ABI](docs/c-api.md#compatibility)). Rust
  adds `DeathReason::Pit`, and `transport::REASONS` has a third entry
  ([frames](docs/bot-api.md#frames)).
- With recording off, `NetworkBackend::drain_records` and the JSON `wire` op
  return an error, as simulation already did
  ([recording](docs/client-api.md#recording-and-network-bounds)).
- Arbitrary caps are gone; each [limit](docs/configuration.md#limits) is a
  representation bound, a warning or a named policy. Hosts and simulated clients
  take up to 65,535 agents (was 128) and 65,536 prey (was 10,000). The caps on
  `foodCount`, ambient refill, `serve` agents and worlds, decision ticks, and
  latency, processing and network intervals are removed; `info` reports the
  `serve` maxima. `radius` and `radiusUpdates[].radius` reject values above
  `1e150`, matching checkpoints. Validation messages changed.
- Sliding boundaries need `dt` below 16 / 437.5 s (about 36.57 ms), so a
  boosting minimum-size snake cannot pass its contact lead; this replaces the
  32 ms cap. Hard boundaries accept steps up to about 6.3 × 10¹³ s.
  `maxPerCell: 0` admits no food, and kiosk rates, prey speeds, prey turn rate
  and nutrition factor may be zero ([limits](docs/configuration.md#limits)).
- Ticks long enough for a snake to outrun the incremental body window warn
  instead of failing, and such snakes are resent in full. Incremental updates
  carry up to three new anchors, up from two
  ([packet timing](docs/server-rules.md#packet-timing)).
- Simulated round trips are no longer clamped to 2 s, and pings slower than 2 s
  are reported instead of dropped
  ([timing](docs/configuration.md#timing-and-ping)).
- The minimum Rust version is 1.99 (was 1.86).
- `Host::poll()` is removed. `Host::bind(config)?.start()?` returns a
  `RunningHost` with load counters, `wait()` and `stop()`; dropping it stops the
  host, and `Host::run()` starts and waits. Physics, sockets and bots run on
  separate threads ([embedding](docs/raw-server.md#embedding-and-load-checks)).
- Rust `Simulator` state is private, with borrowed read accessors and checked
  lifecycle, replay and food/prey edits. `WorldOptions` selects an empty,
  dormant or placed initial population; `Placement` defaults to the existing
  test ring. `add`, public `update_mass` and the two-call speed import are
  removed. Typed `Session::prepare_reset`/`commit_reset` keeps rejected resets
  atomic, and Python resets skip a full-frame JSON round trip. Config keys, wire
  replies and contract IDs are unchanged
  ([Rust API](docs/bot-api.md#rust-library)).
- Continuation uses validated `Simulator::checkpoint`, `from_checkpoint` and
  atomic `restore` instead of Simulator/runtime serde. Checkpoints use
  `slither-sim-checkpoint-v1` / `slither-sim-rules-v1`; debug snapshots cannot
  restore worlds.
- Core calls return `SimError`; `step`/`step_profiled` return
  `CoreStepError::{Rejected, Stopped}`, and `transport::StepError` carries
  `SimError` causes. Match `kind` or `code()` rather than message text, and read
  `Simulator::status()` for the original stop cause. `ErrorCode`, `ErrorKind`
  and `Constraint` are non-exhaustive, so matches need a wildcard arm. Wire
  errors are unchanged ([Rust errors](docs/bot-api.md#rust-library)).
- `physics::crossed_anchor` drops its unused argument,
  `physics::append_client_anchor` is removed, and `Minimap::from_snakes` returns
  `Result` for an invalid side.

### Added

- A `bot` feature, on by default through `networking`, adds the frozen Forager
  policy (`bot::Forager`) and `client::ForagerBot`. The default policy is A4
  (A4-mean); A1 stays selectable. The [adoption evidence](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/bots/2026-10-26-a4-adoption/REPORT.md)
  records gameplay, cost and the maintainer's mean-cost gate. The
  [`cautious_bot`](https://github.com/NikitaMGrimm/slither-sim/blob/v0.5.0/examples/cautious_bot.rs) example runs it.
- `pip install slither-sim`: the Python package is published to PyPI with each
  crate release. Stable-ABI wheels cover CPython 3.11+ on Linux (x86-64, ARM64),
  macOS (Apple silicon, Intel) and Windows (x86-64); other platforms build from
  source with Rust 1.99+ ([Python guide](docs/python.md)).
- [Release archives](README.md#download-a-release) hold the CLI, the C ABI
  library and header, and `SHA256SUMS`, for Linux (x86-64, ARM64; glibc 2.17+),
  macOS (Apple silicon, Intel) and Windows (x86-64). They are unsigned.
- [Battledome](docs/configuration.md#battledome), our own mode:
  `Config::battledome()` or `"preset": "battledome"` gives a fixed 3,584-unit
  arena with a one-way pit, a permanent 400-point size floor (boosting at it is
  free and drops nothing; death food returns only mass above it), a 15-minute
  rolling kill score, no prey, and ambient refill that counts all food. Newborns
  start in an outer 2,684–3,400-unit band once body, head path and departure are
  clear; waiting socket players go before background bots. Other rules follow
  the official preset. New world settings: `pit`, `spawnRadius`,
  `spawnRadiusMax`, `killScoreSeconds`, `minScore` and
  `ambientRefillCountsAllFood`.
- The stock browser shows a pit's edge as a ring of decorative pellets. Physics
  and bot food are unchanged; socket bots get real food only by sending the
  `X-SlitherSim-Food: physical` upgrade header
  ([Battledome](docs/configuration.md#battledome)).
- World configs take `"preset": "official" | "sandbox" | "battledome"` in
  `serve` and Python resets, `host` config files and `client` simulation
  requests; explicit fields override the preset
  ([presets](docs/configuration.md#presets)).
- Frames carry each snake's rolling kill score: `snakes.killScore` in `serve`
  JSON (an additive `slither-sim-json-v3` column), `kill_score` in `FrameRef`,
  `snake_kill_score` in Python and the C ABI, and `Simulator::kill_score` in
  Rust ([frames](docs/bot-api.md#frames)).
- `Config::warnings`, `SimulationConfig::warnings` and `HostConfig::warnings`
  flag valid but unmeasured settings: a `dt` outside the measured 6.94–7.07 ms
  loop, or ticks long enough to outrun the incremental body window
  (`dt × 437.5 ≥ 126`). `serve` returns them on reset, the CLI prints them, and
  Python's `Session.reset` issues `UserWarning`s
  ([limits](docs/configuration.md#limits)).
- Host `bots.threads` (default 2) runs background bots on their own threads,
  apart from physics and sockets. The `host_load` example measures simulation
  load, deferred ticks and extra delivery delay
  ([hosting](docs/raw-server.md#embedding-and-load-checks)).
- `SimulationConfig.delivery` chooses when visible state is sent. The default,
  `responsive`, is our own design and sends steering, speed and mass changes in
  the next frame; `official` keeps the measured pacing. Train bots meant for
  official servers with `official`
  ([SimulationConfig](docs/configuration.md#simulationconfig)).
- A localhost-only host/capture/compare command records transport baselines and
  checks them against official-reference gates
  ([transport parity](docs/research-workflow.md#transport-parity)).

### Changed

- Gameplay updates go out every physics tick (7 ms by default). `official`
  delivery uses measured per-family pacing: motion and standalone mass changes
  at least 6 and 15 ticks apart per snake, anchors at once, ambient food on the
  shared 504 ms maintenance cycle and prey corrections on one clock per prey.
  These are partial models
  ([packet timing](docs/server-rules.md#packet-timing)).
- Minimaps and leaderboards follow server-wide clocks at the official mean
  periods (`minimapIntervalSeconds` 2.504, `leaderboardIntervalSeconds` 3.008).
  A join waits for each clock's next tick instead of getting an immediate board
  ([packet timing](docs/server-rules.md#packet-timing)).
- Input and pings that arrive during a tick take effect in that tick's frame,
  and pongs lead it, removing a tick from input delay and measured ping
  ([timing](docs/configuration.md#timing-and-ping)).
- Host and native background bots and both Rust bot examples use the shared
  default Forager, and boost-only changes are now sent
  ([portable bots](docs/client-api.md#rust)).
- Transport checks compare against timing recorded near the official servers
  instead of home captures, whose delay spikes came from the home route. New
  fixtures cover rare server timing exceptions; simulator cadence is unchanged
  ([reference audit](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-server-timing-references/REPORT.md)).
- New S-shaped snake logo and square project icon, paired with SlitherClient's
  C-shaped snake.

### Fixed

- A default `host` no longer refuses every player with `No clear spawn
  position`. Host arenas start sized for all seats, and a crowded world places a
  join at the most open spot ([hosting](docs/raw-server.md#presets)).
- A host no longer drops a healthy client when a message near the 1 MiB limit
  follows a small one that is still waiting for its batch to fill.
- The host bounds raw socket reads per poll, so one long fragmented message
  cannot stall other connections. A background bot whose decoder fails
  reconnects alone, and stopping no longer waits out a long physics tick.
- A new world starts at its configured radius instead of one population step
  smaller, so resetting with the returned config reproduces the first frame.
  `Placement::TestRing` rejects initial bodies that extend outside a small
  arena, and a refused hosted join says the candidate search was exhausted
  rather than implying the arena is full.
- Prey respawn is no longer capped at one per tick, and a replacement waits a
  full `respawnSeconds` after the catch. With a coarse `dt`, pickup follows each
  prey motion step instead of the straight line between endpoints, and prey
  motion that overflows ends the world with an error instead of producing NaN
  positions.
- Prey whose IDs share the low 16 bits (such as 0 and 65536) no longer hide each
  other on the wire; all 65,536 wire IDs can be live
  ([prey settings](docs/configuration.md#prey)).
- Prey catches can no longer make `fam` or `length` infinite: settings where one
  catch overflows are rejected, and accumulated overflow stops the world
  ([limits](docs/configuration.md#limits)).
- Kiosk shrinkage no longer deletes pending growth: it removes only mass the
  score shows, and a zero shrink rate changes nothing.
- A tiny food `cellSize` no longer merges distinct
  [admission cells](docs/configuration.md#food-admission-limits).
- Native commands with processing delay apply in the tick they arrive instead of
  one tick later. Cancelling or non-finite one-way legs fail validation, and
  latency scheduling returns an error instead of hanging or producing
  non-finite times.
- Native recording failures leave pending decisions and client clocks intact,
  and rejected sends no longer suppress a later boost change.
- Unrepresentable `serve` columns, `worlds × agents` products and host client
  counts return errors instead of panicking. A failed reset keeps the previous
  session; a failed step frame ends it ([session errors](docs/bot-api.md)).
- A world stopped by mass overflow stays stopped after checkpoint restore
  ([limits](docs/configuration.md#limits)).
- `Config::deserialize` called directly from Rust accepts presets, like JSON
  parsing ([presets](docs/configuration.md#presets)).
- Python `Session.reset` issues its warnings before replacing worlds, so a
  warning raised as an error keeps the previous session
  ([Python](docs/python.md)).

### Performance

- The host uses about 5% less CPU per tick at 250 and 500 socket clients, with
  byte-identical output. Unchanged food subscriptions stay in place, recipients
  with the same history of a snake share one encoded delta, and delivered
  frames reach the socket without an extra copy. 250 clients now narrowly meet
  the 10 ms delivery grade on one core; 500 still fall behind
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-host-efficiency/REPORT.md)).
- The host hands players' frames to the socket thread before encoding bots'
  frames. With 480 localhost bots, mean ping fell from about 8.3 to 7.1 ms and
  p99 from 17–19 to 11–13 ms
  ([host load checks](docs/raw-server.md#embedding-and-load-checks)).
- Crowded hosts spend far less time choosing spawn positions: candidates stop
  being scored once they cannot win, and the chosen pose is unchanged. A
  64-seat Battledome host drops from about 45 to 3.5 ms per tick. Background
  bots refused a spawn wait one second before retrying; socket players keep
  their own join timing
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-25-battledome-spawn-cost/REPORT.md)).
- Batch observations (`observe_all_into`, `observe_selected_into`, transport
  frames and `serve`) are 16–19% faster sequentially and 5–12% faster in
  parallel in the 96- and 128-snake benchmark worlds, with identical results.
  Synthetic worst cases are slower: up to 15% when food clusters away from
  observers, 2.5–4% with sparse or narrow food
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-observation-food-index/REPORT.md)).
- A [native Linux capacity study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-host-capacity/REPORT.md)
  finds that one core keeps official-like delivery for 100 socket clients and
  real time for 500 background bots, and profiles where the host's CPU goes.

## [0.4.0](https://github.com/NikitaMGrimm/slither-sim/compare/v0.3.1...v0.4.0) - 2026-10-24

### Added

- *(python)* add an in-process Python package over transport sessions ([#64](https://github.com/NikitaMGrimm/slither-sim/pull/64))
- *(tools)* measure serve transport cost at Python and Node controllers ([#62](https://github.com/NikitaMGrimm/slither-sim/pull/62))
- *(transport)* drive sessions in-process and add a C ABI ([#63](https://github.com/NikitaMGrimm/slither-sim/pull/63))
- *(serve)* [**breaking**] replace slither-sim-json-v1 with columnar multi-world frames ([#61](https://github.com/NikitaMGrimm/slither-sim/pull/61))
- *(serve)* [**breaking**] add --parallel and pass ServeOptions to transport::serve ([#59](https://github.com/NikitaMGrimm/slither-sim/pull/59))

### Other

- cache Rust builds, lint once and add macOS jobs ([#67](https://github.com/NikitaMGrimm/slither-sim/pull/67))
- streamline user and contributor guides ([#65](https://github.com/NikitaMGrimm/slither-sim/pull/65))

### Breaking

- `slither-sim serve` speaks `slither-sim-json-v2`, and v1 is removed. Replies
  carry one frame per world: everything any living slot sees, once, in columns,
  with each slot's view as row indices into them. With 128 living snakes, replies
  are about 12× smaller, the server answers about twice as fast and Python's
  `json.loads` is about 11× faster.
- To migrate, send one `{"heading":[...],"boost":[...]}` step entry per world
  (`null` headings for dead slots) and read `result.worlds[w].views[slot]`.
  `observe` and `elapsedSeconds` are removed (use `ticks × config.dt`), and
  `snapshot` takes `world`. See the [bot API](docs/bot-api.md).
- Frames round coordinates to 0.01 world units, headings to 0.0001 rad and speeds
  to 0.01 units/s; other numbers stay exact. Real players mostly receive coarser
  protocol-19 data, and the worst error is 0.05% of the smallest snake radius;
  see the [observation precision study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-24-observation-precision/REPORT.md).
  The simulation, snapshots, the portable client and the WebSocket host keep full
  precision, so hosted players are unaffected.

### Added

- One `serve` session runs many worlds: `reset` takes `worlds` (up to 1,024),
  world `w` uses seed `seed + w`, and worlds step in parallel on every core.
  `slither-sim serve --threads N` caps the threads; replies do not depend on it.
- `transport::Session` drives `serve` sessions in-process. `request` answers a
  JSON request line with the exact reply line `serve` writes. `step` takes
  heading and boost slices for every slot of every world and writes no JSON;
  `frame` then reads each world's frame as typed, read-only columns with the
  values the JSON frame prints. See [in-process sessions](docs/bot-api.md#in-process-sessions).
- `slither-sim-abi-v1`, a C ABI over these sessions, in the unpublished
  `slither-sim-ffi` workspace crate (`ffi/`, with a hand-written header). Any
  language that can call C can reset through JSON, then step from typed arrays
  and read frame columns through pointers. `examples/ffi_bot.py` drives it with
  Python's standard-library `ctypes`. See the [C ABI guide](docs/c-api.md).
- The `slither-sim` Python package (`python/`, import `slither_sim`; not yet on
  PyPI) runs these sessions in-process. `Session.step` takes float64 and
  bool/uint8 buffers, such as numpy arrays, or plain sequences, and releases the
  GIL; `Session.frame` returns the columns as `array.array` copies. With 128
  agents, a Python decision loop took 0.27 ms per step against 0.56 ms through
  JSON requests. It shares the crate's version, as does `slither-sim-ffi`, through
  `[workspace.package]`. See the [Python guide](docs/python.md).

## [0.3.1](https://github.com/NikitaMGrimm/slither-sim/compare/v0.3.0...v0.3.1) - 2026-10-24

### Changed

- *(simulator)* reuse per-step buffers between steps ([#57](https://github.com/NikitaMGrimm/slither-sim/pull/57))
- *(transport)* write JSON replies without building a Value tree ([#55](https://github.com/NikitaMGrimm/slither-sim/pull/55))
- *(client)* share food sector listings and index known snakes by slot ([#54](https://github.com/NikitaMGrimm/slither-sim/pull/54))
- *(observation)* index batch observation food in a 2D grid ([#53](https://github.com/NikitaMGrimm/slither-sim/pull/53))
- *(simulator)* track food occupancy and pickup candidates across a step ([#52](https://github.com/NikitaMGrimm/slither-sim/pull/52))
- *(simulator)* add exact broad phases for collisions and prey pickup ([#51](https://github.com/NikitaMGrimm/slither-sim/pull/51))

### Added

- List benchmark workloads with `perf_baseline --list` from one registration table.
  New synthetic steady-state workloads cover sandbox, official worlds at 30–500
  snakes, a dense no-prey arena standing in for a future Battle Dome, and an
  official in-process client path, with deaths, death food and respawns inside the
  measured and traced windows. The six historical cases and their traces are unchanged.
  See [performance workloads](docs/performance-workloads.md).
- Compare a base ref with the working tree in one command:
  `tools/perf_compare.py --base REF` freezes both source snapshots, builds them
  in separate targets, checks full-state trace parity, records phase profiles and
  runs paired timings. Runners and PR comparisons take workloads, sample sizes
  and trace windows from the catalog; PR CI times and traces its `ci` suite.
- Observe on several cores: `Simulator::set_parallel(true)` splits large
  `observe_all` and `observe_selected` batches over the rayon thread pool, with
  identical results. Off by default; needs the new default `parallel` feature.
  `perf_baseline --parallel` times it.
- Refill a caller-owned buffer with `Simulator::observe_all_into` and
  `observe_selected_into`. Observations are identical to `observe_all` and
  `observe_selected`, but reuse the previous call's vectors; on Windows, bot
  loops that observe every decision ran 1.8–3.5× faster than with `observe_all`.
  `slither-sim serve` keeps one such buffer per session.

## [0.3.0](https://github.com/NikitaMGrimm/slither-sim/compare/v0.2.1...v0.3.0) - 2026-10-23

### Maintenance

- publish manual releases and reset the automatic timer
- clarify fidelity gaps and simplify navigation ([#46](https://github.com/NikitaMGrimm/slither-sim/pull/46))
- measure PRs only when measured inputs change ([#44](https://github.com/NikitaMGrimm/slither-sim/pull/44))
- separate maintained guides from research records ([#41](https://github.com/NikitaMGrimm/slither-sim/pull/41))

### Changed

- **Breaking Rust API:** replace `Config::realistic(n)` with `Config::official(n)`.
  `Config::default()` and omitted JSON world settings now select the official
  approximation, with population-driven radius and nonzero prey. Existing kiosk,
  food-admission, sliding-boundary and retained-speed models remain enabled.
  Use `Config::sandbox()` or explicit JSON settings for the previous compact,
  fixed-radius scene. Model estimates and their limitations remain documented;
  no exact official-server equivalence is claimed.

## [0.2.1](https://github.com/NikitaMGrimm/slither-sim/compare/v0.2.0...v0.2.1) - 2026-10-23

### Changes

- Bound food-sector clearing in bundled client frames, including preflight
  validation, to avoid repeatedly scanning unrelated retained food.
- Bound minimap rasterization for extreme geometry while preserving ordinary
  output, and reject frames exceeding the aggregate minimap decoding work budget.
- Reject protocol-19 simulation-client prey sizes that cannot fit the wire format.
- Index collision body bounds by stable snake slot and skip observation-index
  construction when no selected snake is alive. Add reproducible offline
  workloads, full-state parity traces and [fixture-specific performance reports](docs/performance.md).
- Add isolated observation-phase diagnostics, retained-client workloads and
  optional whole-comparison deadlines with retained incomplete-run evidence.
- Automate compatible patch-release preparation and publication with a 12-hour
  cooldown and protected checks for release-bot PRs; see [releasing](docs/maintainers/releasing.md).
- Add informational performance and fidelity comparisons to PR CI. Synthetic
  comparisons do not establish official-server fidelity or general speedups.
- Reduce PR-comparison overhead and report reconnect work separately in
  retained-client measurements.

## 0.2.0 (2026-10-23)

- Rust compatibility: `SimulationConfig` adds `local_records`, defaulting to
  false. Update exhaustive struct literals or use `..Default::default()`.
  The JSON protocol identifiers remain unchanged.
- Publish the current evidence-constrained host defaults: finite 1,500-unit
  snake/prey coverage, retained food sectors, 136-cell minimaps and independent
  minimap/leaderboard clocks. The compact default arena and provisional ecology remain
  explicit; no exact official-server preset or overall accuracy is claimed.
- Retain paired-snake targets through bounded stale-pose pauses without granting
  clean-route credit, and screen food-route corridors against complete received
  bodies. Add controlled snake/food collectors, ordered sector diagnostics and
  explicit paired capture budgets up to 3,600 seconds.

- Run declared paired-prey routes through the managed evidence collector and
  verify route settings against the plan; retain process outcomes across audit errors.

- Move crossing acquisition's near waypoint inward and hold achieved endpoints
  for observation; retain commanded hold offsets and release timing in metadata.

- Require fresh reference geometry before crossing endpoints advance; separate
  the initial approach from completed legs and retain exact endpoint witnesses.
  Record the paired visibility recheck's model failures without retuning physics.

- Hash and decode the same replay-source snapshot; retain protocol and tool
  identities and distinguish replay-window checks from a full evidence audit.

- Compare verified evidence batches with bounded permutation tests and declared
  exposure/effect gates; add manual, read-only CI intake for pinned bundles.
  Keep measured calibration separate from synthetic prey/radius behavior tests.

- Keep empty food subscriptions internal and send per-connection minimap XOR
  deltas when shorter. Add fixed diagonal collection routes and clarify the
  source/evidence boundary, ecology models and offline collision rationale.

- Match the default prey turn setup to physics; give leaderboards independent
  deadlines and bound prey reporting on gameplay broadcasts. Add an offline packet
  census, source-pinned cadence fixture and generic localhost capture workflow;
  keep recording ping timing through post-death observation and capture shutdown.
- Keep correcting predicted prey positions even when authoritative grid/motion
  is unchanged, preventing drifting ghosts and later snap-backs.
- Use shared finite 1,500-unit snake/prey coverage and retained food-sector interest;
  restore 136-cell minimaps with an independent clock. Require safe whole-body
  admission for hosted startup, joins and respawns, with no cramped fallback.
- Make synthetic world-run records/victory entry opt-in (`localRecords`, default false).

- Restore source-linked pellet palettes, persistent ambient colors, birth types
  and explicit pickup attribution from the earlier client presentation layer.

- Keep answering post-death pings so the stock browser's death fade and victory
  entry do not trigger lag slowdown; document a local endpoint that survives Play Again.
- Continue opponent, food and prey updates through the stock death animation,
  preserving broadcast cadence while suppressing repeated death and minimap reports.

- Keep configured prey sizes positive and within their bounds, ambient food
  inside tiny arenas, and finite extreme respawn poses numerically usable.
- Disambiguate initial protocol timing headers and bound retained client bodies
  and food with atomic frame rejection.
- Require the original v1 evidence source minimum and report replay targets
  left unprocessed after early simulator death.

## 0.1.0 (2026-10-22)

- Deterministic headless Rust simulator with configurable worlds, geometric
  observations and mechanics regression fixtures.
- Core-only library builds, selected-slot observations, supplied-body respawn
  and runtime ambient refill-rate control.
- Portable bot frames (`slither-client-v2`) reconstructed by a shared
  protocol-19 decoder, with simulator and WebSocket backends, receipt ages,
  measured ping and simulated latency.
- JSON-lines interfaces for portable bots (`slither-client-json-v1`) and direct
  world control (`slither-sim-json-v1`), with Rust, Python and JavaScript examples.
- Bounded shared-world WebSocket host with profiles, leaderboards, kill reports,
  record messages and optional connection identifiers.
- Evidence tools for explicit-endpoint captures, decoding, auditing, recorded-input
  replay and bundles containing frozen plans, attempt records and source snapshots.
- Offline worked study, documented fidelity limits, extraction provenance and
  manual checks on Windows and Linux.
- Automatic pull-request checks, shared local/CI check commands, contributor
  task guidance and protocol-labelled regression fixtures.

Interfaces remain experimental.
