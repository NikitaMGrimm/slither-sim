# Changelog

## Unreleased

- Show pit edges in the stock browser with decorative pellets; bot food and
  physics stay unchanged.

### Breaking

- The minimum Rust version is 1.99 (was 1.86).
- `SimulationConfig.broadcastTicks` is removed: gameplay updates go out every
  tick. Configs that still set it are rejected
  ([simulation settings](docs/configuration.md#simulationconfig)).
- `LatencyConfig.jitterSeconds` and `congestion` are replaced by `variation`
  (lognormal per-message delay with a slow correlated part) and `pauses` (short
  delivery holds per direction), fitted to official captures;
  `LatencyConfig::measured_route()` returns the fitted route. Configs that set the
  old fields are rejected
  ([variation and pauses](docs/configuration.md#variation-and-pauses)).
- `Host::poll()` is removed. `Host::bind(config)?.start()?` returns a
  `RunningHost` with load counters, `wait()` and `stop()`; dropping it stops the
  host. `Host::run()` starts and waits. Physics, sockets and bots run on separate
  threads ([embedding](docs/raw-server.md#embedding-and-load-checks)).
- C ABI `slither-sim-abi-v2` (`SLITHER_ABI_VERSION` 2) adds `snake_kill_score`
  after `snake_kills` in `slither_frame`, and the `SLITHER_DEATH_PIT` death code.
  Rebuild against the new header ([C ABI](docs/c-api.md#compatibility)).
- `DeathReason::Pit` is new, and `transport::REASONS` has a third entry
  ([frames](docs/bot-api.md#frames)).
- A pickup that would overflow a snake's mass completes its tick without the
  gain, returns an error and stops the world. Sessions now tell a rejected step
  (state unchanged) from a terminal failure (all worlds cleared; reset
  required): `serve` uses `slither-sim-json-v3` with `sessionEnded` in error
  replies, Rust returns `transport::StepError::{Rejected, Ended}`, C returns
  `SLITHER_ENDED` (4), and Python raises `SessionEndedError`. Successful replies
  are unchanged ([session errors](docs/bot-api.md)).
- Arbitrary caps are gone; each [limit](docs/configuration.md#limits) is now a
  representation bound, a warning or a named policy. Hosts and simulated clients
  take up to 65,535 agents (was 128) and 65,536 prey (was 10,000). The caps on
  `foodCount`, ambient refill, `serve` agents and worlds, decision ticks, and
  latency, processing and network intervals are removed. `info` reports the
  `serve` maxima. Validation messages changed.
- Sliding boundaries need `dt` below 16 / 437.5 s (about 36.57 ms), so a boosting
  minimum-size snake cannot pass its contact lead; this replaces the 32 ms cap.
  Hard boundaries accept steps up to about 6.3 × 10¹³ s
  ([limits](docs/configuration.md#limits)).
- `maxPerCell: 0` admits no food. Kiosk rates, prey speeds, prey turn rate and
  nutrition factor may be zero ([limits](docs/configuration.md#limits)).
- Ticks long enough for a snake to outrun the incremental body window warn
  instead of failing, and such snakes are resent in full. Incremental updates
  carry up to three new anchors, up from two
  ([packet timing](docs/server-rules.md#packet-timing)).
- Simulated round trips are no longer clamped to 2 s, and pings slower than 2 s
  are reported instead of dropped
  ([timing](docs/configuration.md#timing-and-ping)).

### Added

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
  free and drops nothing), a 15-minute rolling kill score and no prey; other
  rules follow the official preset. It is built from four new world settings:
  `pit`, `spawnRadius`, `killScoreSeconds` and `minScore`.
- World configs take `"preset": "official" | "sandbox" | "battledome"` in `serve`
  and Python resets and in `host` and `client` config files; explicit fields
  override the preset ([presets](docs/configuration.md#presets)).
- Frames carry each snake's rolling kill score: `snakes.killScore` in `serve`
  JSON (an additive `slither-sim-json-v3` column), `kill_score` in `FrameRef`,
  `snake_kill_score` in Python and the C ABI, and `Simulator::kill_score` in Rust
  ([frames](docs/bot-api.md#frames)).
- `Config::warnings`, `SimulationConfig::warnings` and `HostConfig::warnings`
  flag valid but unmeasured settings: a `dt` outside the measured 6.94–7.07 ms
  loop, or ticks long enough to outrun the incremental body window
  (`dt × 437.5 ≥ 126`). `serve` returns them on reset, the CLI prints them, and
  Python's `Session.reset` issues `UserWarning`s
  ([limits](docs/configuration.md#limits)).
- Host `botThreads` (default 2) runs background bots on their own threads, apart
  from physics and sockets. The `host_load` example measures simulation load,
  deferred ticks and extra delivery delay
  ([hosting](docs/raw-server.md#embedding-and-load-checks)).
- A localhost-only host/capture/compare command records transport baselines and
  checks them against official-reference gates
  ([transport parity](docs/research-workflow.md#transport-parity)).
- A [cautious bot example](examples/cautious_bot.rs) that seeks food while
  avoiding walls and other snakes; the `portable_bot` examples are shorter.

### Changed

- Gameplay updates go out every physics tick (7 ms by default) with measured
  per-family pacing: motion and standalone mass changes at least 6 and 15 ticks
  apart per snake, anchors at once, ambient food on the shared 504 ms maintenance cycle and
  prey corrections on one clock per prey. These are partial models
  ([packet timing](docs/server-rules.md#packet-timing)).
- Minimaps and leaderboards follow server-wide clocks at the official mean
  periods (`minimapIntervalSeconds` 2.504, `leaderboardIntervalSeconds` 3.008).
  A join waits for each clock's next tick instead of getting an immediate board
  ([packet timing](docs/server-rules.md#packet-timing)).
- Input and pings that arrive during a tick take effect in that tick's frame,
  and pongs lead it, removing a tick from input delay and measured ping
  ([timing](docs/configuration.md#timing-and-ping)).

### Fixed

- A default `host` no longer refuses every player with `No clear spawn
  position`. Dynamic host arenas start sized for all seats, and a crowded world
  places a join at the most open spot ([hosting](docs/raw-server.md#presets)).
- Prey whose IDs share the low 16 bits (such as 0 and 65536) no longer hide each
  other on the wire; all 65,536 wire IDs can be live
  ([prey settings](docs/configuration.md#prey)).
- A tiny food `cellSize` no longer merges distinct
  [admission cells](docs/configuration.md#food-admission-limits).
- Prey catches can no longer make `fam` or `length` infinite: settings where one
  catch overflows are rejected, and accumulated overflow stops the world
  ([limits](docs/configuration.md#limits)).
- Unrepresentable `serve` columns, `worlds × agents` products and host client
  counts return errors instead of panicking. A failed reset keeps the previous
  session; a failed step frame ends it ([session errors](docs/bot-api.md)).

### Performance

- Batch observations (`observe_all_into`, `observe_selected_into`, transport
  frames and `serve`) are 16–19% faster sequentially and 5–12% faster in
  parallel in the 96- and 128-snake benchmark worlds, with identical results.
  Synthetic worst cases are slower: up to 15% when food clusters away from
  observers, 2.5–4% with sparse or narrow food
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-observation-food-index/REPORT.md)).
- The host hands players' frames to the socket thread before encoding bots'
  frames. With 480 localhost bots, mean ping fell from about 8.3 to 7.1 ms and
  p99 from 17–19 to 11–13 ms
  ([host load checks](docs/raw-server.md#embedding-and-load-checks)).

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
