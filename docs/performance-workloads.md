# Performance workloads

The native benchmark harness, `examples/perf_baseline.rs`, keeps every workload in
one registration table. That table is the only case list: argument parsing, usage
and the catalog are derived from it. All workloads are synthetic and offline, use
seed 7 and the 7 ms physics tick, and do not describe production worlds or bot
playing strength. Build and comparison procedures are in the
[performance guide](performance.md).

## Catalog

```sh
cargo run --release --locked --example perf_baseline -- --list
```

`--list` prints one line of canonical JSON (sorted keys), format
`slither-perf-catalog-v1`, and exits 0. Each workload in registration order has:

- `name`: the `CASE` argument of `perf_baseline CASE ITERATIONS [--profile|--trace]`.
- `suite`: `ci` rows are timed and traced by PR CI, which allows at most ten;
  `full` rows are local defaults only.
- `iterations` and `traceIterations`: the default timed sample size and parity
  window. Each window keeps an uncompressed trace near 64 MB or less.
- `mode`, `operation`: informational labels such as `official` and `step+observe`.
- `profile`: whether `--profile` is supported.
- `available`: false when the row needs the `networking` feature and the harness
  was built without it. The row is still listed; running it fails.
- `legacy`: true only for the six historical cases.
- `synthetic`: null, or what is synthetic about the scene.
- `definition`: the resolved scene, including the serialized `Config`. A changed
  definition means the same name now measures different work.

## Families

- **Legacy:** `step-default`, `step-food`, `step-body`, `observe`, `portable` and
  `protocol`, described in the [performance guide](performance.md), come first.
  Their names, default iterations and traces are unchanged because archived
  reports cite them.
- **Sandbox and official:** `Config::sandbox()` and `Config::official(n)` at several
  populations, giving a scaling curve.
- **Dome proxy:** a dense, fixed arena with sandbox mechanics, no prey and no
  ambient food. Boost and death pellets are its only food. It stands in for the
  future Battle Dome; it does not implement Battle Dome rules.
- **Client:** `SimulatorBackend` lives on an official world, timed through
  `step_all` with wire recording. These rows need the `networking` feature.

Each sandbox, official and dome-proxy registration expands into `<id>-step`, which
times one `Simulator::step`, and `<id>-bot`, which times `step` followed by
`observe_all`, including dropping the observations.

## Steady-state lifecycle

Unlike the legacy cases, which reset after any death, scenario rows measure a
running population with deaths, death food, respawns and grown bodies:

- **Placement:** every slot is added, marked dead and enters through
  `Simulator::respawn` at a pose that passes `Simulator::spawn_is_clear`.
  Distances come from `spawn::sample_spawn_distance`. Each life is a straight body
  with a segment count drawn from the registered range.
- **Randomness:** poses and lengths come from a harness-owned generator. It never
  draws the simulator's RNG, so lifecycle work does not shift world randomness.
- **Control:** a stateless wander controller picks a heading per staggered window,
  occasionally chases another head, steers inward near the edge and boosts in a
  staggered window. Controls and lifecycle work are outside the timed interval.
- **Respawn:** between operations, new deaths are read through a cursor;
  `world.deaths` is not modified. Dead slots queue in death order and respawn with
  bounded attempts. A slot without a clear pose is deferred and retried a few
  ticks later.
- **Warmup and epochs:** an untimed warmup runs first. Long samples restore the
  warmed scene every 4,096 operations, bounding death history and drift.
- **Client rows:** a slot reconnects through `reconnect_slot` after its client
  reports the death, as a bot would. Actions go only to slots that are playing and
  not processing an earlier decision.

`--profile` replaces `step` with `step_profiled`. Profiled `-bot` rows also report
`observeSeconds`, the time spent in `observe_all`.

Reports keep the `slither-perf-baseline-v1` envelope. Scenario details add the
definition, lifecycle counts, population statistics and warmup results.

## Validity checks

Every mode checks a deterministic prefix: warmup plus the default trace window,
or the whole run if it is shorter. A steady scenario must show at least one body
death, one respawn and, when death food is enabled, one death-food pellet in that
prefix. Otherwise the harness exits non-zero with the counts.

Population depletion is reported, not fatal. `population` gives the minimum, mean
and maximum living snakes and total anchors at operation boundaries; `depleted`
is true when the mean is below 90% of the requested population. Check it before
interpreting throughput.

## Traces

`--trace` emits untimed canonical JSONL. Legacy traces are unchanged. Scenario
traces contain:

1. An `initial` full snapshot after placement.
2. One light `warmup` line per tick: actions, deaths, respawns, events, world
   statistics and the harness RNG state. Then a `warmed` full snapshot.
3. One line per measured operation (`step`, `bot` or client `decision`) with its
   actions, deaths, respawns and a full snapshot. `-bot` lines include the
   observations; client lines include frames and wire records.
4. An `epoch` snapshot whenever the warmed scene is restored.
5. A terminal `end` record with the operation count, physics ticks and lifecycle
   counts, followed by the usual report line. A trace without `end` is incomplete.

Snapshots and warmup statistics are taken right after the operation, before dead
slots respawn: respawning resets a slot's per-life state, which would otherwise
hide changes to a just-died snake from parity comparison. Scenario respawn records
carry each new pose and length; client slots reconnect before the next decision.

## Adding a workload

1. Add one registration to `REGISTRATIONS`. A new world needs a `World` variant;
   its exhaustive matches require a config, mode, label and definition.
2. Calibrate `iterations` so one release sample takes about one second. Choose a
   `traceIterations` that keeps the uncompressed trace near 64 MB or less.
3. Run the row in timing, `--profile` and `--trace` modes. Confirm the validity
   check passes, the population is not unexpectedly depleted, and two traces are
   byte-identical.
4. Keep at most ten `ci` rows. Run `cargo test --locked --example perf_baseline`.

Do not rename, resize or change the meaning of an existing row; register a new
name instead. No runner, workflow or table edit is needed.

The real Battle Dome registration lands one PR after its runtime config
constructor. PR CI builds the PR's harness against the base runtime, so the
harness can only use public API already on the base branch. Keep the dome-proxy
rows for continuity with earlier measurements.
