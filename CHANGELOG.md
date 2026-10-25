# Changelog

## Unreleased

### Added

- Offline transport parity measurements and official-reference gates, with a
  localhost-only host/capture/compare command for recording transport baselines.

- Host `botThreads` (default 2) runs background clients separately from physics
  and sockets; the checkout's `host_load` example measures load, deferred ticks
  and watcher excess delay. See [hosting](docs/raw-server.md#embedding-and-load-checks).
- Advisory PR performance comparisons publish up to three covered progress
  updates and a terminal report, retain verified original-preview results after
  merge, and consolidate evidence on the rolling release PR. A pre-budgeted v2
  schedule continues unresolved rows within a provisional 30-minute job cap;
  hosted timing-model calibration remains unestablished.

- The `slither-sim` Python package is on PyPI: `pip install slither-sim`. From
  this release on, each crate release publishes the same version there.
  Stable-ABI wheels serve CPython 3.11+ on Linux (x86-64 and ARM64), macOS (Apple
  silicon and Intel) and Windows (x86-64); elsewhere the source distribution
  builds with Rust 1.86+. See the [Python guide](docs/python.md).
- GitHub Releases include prebuilt archives of the `slither-sim` CLI and the C
  ABI library with its header, for Linux (x86-64 and ARM64, glibc 2.17+), macOS
  (Apple silicon and Intel) and Windows (x86-64), with a `SHA256SUMS` file. They
  are unsigned. See [download a release](README.md#download-a-release).
- A Battledome mode: `Config::battledome()`, or `"preset": "battledome"` in JSON,
  raises the minimum snake size to 400 points in a fixed 3,584-unit arena with
  a one-way pit, a 15-minute kill score and no prey; physics and ecology are
  otherwise official. It is built from four general world settings: `pit`
  (leaving it kills with the new death reason `pit` and drops death food),
  `spawnRadius` (new lives lie along that ring), `killScoreSeconds` and
  `minScore` (new lives start at it; boosting and the kiosk penalty spend only
  mass above it, and boosting at it is free and drops no pellets).
  See [Battledome](docs/configuration.md#battledome).
- World configs accept `"preset": "official" | "sandbox" | "battledome"` in
  `serve` and Python resets and in `host` and `client` config files; explicit
  fields override the preset. See [presets](docs/configuration.md#presets).
- Frames carry each snake's kill score: `snakes.killScore` in `serve` JSON,
  `kill_score` in `FrameRef`, `snake_kill_score` in Python and the C ABI, and
  `Simulator::kill_score` in Rust. The column is additive to the current
  `slither-sim-json-v3`, and `pit` deaths only occur with the new opt-in `pit`.
- `Config::warnings`, `SimulationConfig::warnings` and `HostConfig::warnings`
  describe valid but unmeasured settings, starting with a `dt` outside the
  measured 6.94–7.07 ms loop and ticks that outrun the incremental body-update
  window (`dt × 437.5 ≥ 126`). `serve` reset replies
  carry them as `warnings`, the CLI prints them to stderr, and Python's
  `Session.reset` issues each as a `UserWarning`.

### Breaking

- `SimulationConfig.broadcastTicks` is removed; JSON configs containing it are
  rejected as unknown fields. Remove it to use per-tick gameplay emission.
- `Host::poll()` is removed. `Host::bind(config)?.start()?` returns a
  `RunningHost` with address/load counters, `wait()` and `stop()`; dropping it
  stops its threads. `Host::run()` starts and waits. Hosts now use separate
  simulation, socket and background bot threads.
- The unreleased C ABI `slither-sim-abi-v2` (`SLITHER_ABI_VERSION` 2) also gains
  `snake_kill_score` after `snake_kills` in `slither_frame`, and
  `SLITHER_DEATH_PIT` is a new `death_reason` code. Rebuild callers against the
  new header.
- `DeathReason` has a new `Pit` variant, and `transport::REASONS` a third entry.
- Limits are now real limits, warned unmeasured ranges or named policies; see
  [limits](docs/configuration.md#limits). Arbitrary caps are removed:
  `foodCount ≤ 100000`, `ambientReplenishPerSecond ≤ 10000`, prey
  `count ≤ 10000` (65,536 wire prey IDs for simulated clients and the host), 128
  agents for simulated clients and the host (now 65,535, which the protocol-19
  `u16` snake IDs, rank and population fields count), `serve`'s 128 agents,
  1,024 worlds and 1,000 decision ticks (now `u32` frame bounds and memory),
  1,000 decision ticks, the latency model's 2 s, 10 s and 1/s
  ceilings and Pareto-shape interval, the 2 s cap on measured processing time
  and the network backend's interval caps. `info` reports the new `serve`
  maxima.
- With sliding boundaries `dt` must be below 16 / 437.5 s (≈ 36.57 ms): a
  boosting minimum-size snake would otherwise pass its contact lead in one tick
  and die on shallow rim contact. The old 32 ms cap claimed body-radius
  tunneling, which swept collision tests rule out. Hard boundaries accept
  longer steps while one tick's count of 7 ms prey substeps is an exact integer
  that fits `usize` (≈ 6.3 × 10¹³ s); boost drops in such a step are counted,
  so the step always finishes.
- Zero `maxPerCell` admits nothing; kiosk rates, prey speeds, prey turn rate
  (also on the wire) and nutrition factor may be zero. Validation error texts
  changed.
- `Simulator::step` can fail after advancing: a pickup whose gain would
  overflow a snake's mass completes the tick without it, returns an error and
  stops the world. Sessions distinguish unchanged-state rejection from terminal
  failure, which clears all worlds and requires a reset: `serve` now uses
  `slither-sim-json-v3` with `sessionEnded` in error replies; Rust typed steps
  return `transport::StepError::{Rejected, Ended}`; C uses `slither-sim-abi-v2`
  with `SLITHER_ENDED` (4); Python raises `SessionEndedError` (`RuntimeError`).
- The wire arena radius limit of 32,000 is now documented as an encoding-margin
  policy rather than a derived bound; its value is unchanged.
- Long broadcast intervals load with a warning instead of an error: snakes that
  outrun the incremental window are resent in full. Up to three new anchors per
  broadcast now encode incrementally, up from two.
- A drawn round trip is no longer clamped to 2 s, and pings slower than 2 s are
  reported instead of dropped.

### Fixed

- Prey whose IDs share their low 16 bits, such as 0 and 65536, no longer share a
  protocol-19 wire ID and hide each other. A retired ID is reused right after
  its removal, so all 65,536 can be live.
- A tiny food `cellSize` no longer merges distinct admission cells.
- Prey catches can no longer make `fam` and `length` infinite: settings whose
  single catch overflows are rejected, and accumulated catches stop the world
  as described above.
- Unrepresentable `serve` frame columns, `worlds × agents` products and host
  client counts are errors instead of panics; a failing reset keeps the previous
  session, and a step whose frame fails ends it.

### Performance

- Batch observations (`observe_all_into`, `observe_selected_into`, transport
  frames and `serve`) are 16–19% faster sequentially and 5–12% faster on the
  parallel pool in the 96- and 128-snake benchmark worlds. The food grid uses
  finer cells and scans only the part of each row the vision circle can reach;
  results are unchanged. Batches with too few observers to repay the finer grid
  keep the previous cells. Batches can be up to about 15% slower when food is
  concentrated in patches away from the observers (synthetic
  `observe-clustered`). The synthetic `observe-wide` (sparse food) and
  `observe-strip` (narrow food) fixtures are about 2.5–4% slower. See the
  [observation food index study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-24-observation-food-index/REPORT.md).

### Changed

- Gameplay updates are emitted every physics tick (default 7 ms). Motion and
  standalone mass use shared per-snake six-/15-tick gates, anchors are prompt,
  ambient food joins shared 504 ms maintenance, and prey corrections share one
  clock per prey across viewers. These are partial models with retained
  counterexamples; see [packet timing](docs/server-rules.md#packet-timing).
- Minimaps and leaderboards follow server-wide clocks at the official mean
  periods, now the defaults: `minimapIntervalSeconds` 2.504 and
  `leaderboardIntervalSeconds` 3.008. A joining connection meets the running
  phase, so its first board no longer arrives on entry; the map leads its frame.

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
