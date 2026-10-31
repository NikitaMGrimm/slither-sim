# Changelog

## Unreleased

### Added

- The README and Linux hosting guide state the measured one-core capacity:
  500 fully grown bot players on an AWS c7a.medium core for 30 minutes without
  falling behind
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-30-official-500-acceptance-v4/REPORT.md)).
- A `perf_baseline` catalog row, `engine-battledome-spawn-318`, that times
  spawn searches and ticks in a crowded synthetic Battledome. It needs the new
  off-by-default `perf-internals` Cargo feature, which exists only for the
  benchmark harness and has no stability promise
  ([guide](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/performance-workloads.md#battledome-spawn-row)).
- A study of storing each player's retained food sectors in flat sorted
  arrays instead of a tree. Food-update ticks get about 0.55 ms cheaper in
  every measured pair, but the whole run isn't reliably faster: in a
  separately pre-registered ten-pair confirmation, only 5 of 10 stock pairs
  improve against the required 8. The change is rejected for good and kept as
  a patch; no runtime change
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-retained-sector-layout/REPORT.md)).
- A real-socket Battledome recheck after the two spawn-search speedups, with
  bots boosting almost all the time. At 320 players the server still falls
  behind, but for 66 ticks of the measured window instead of 231, and frame
  jitter p99 drops from 8.4 to 6.3 ms. At 480 both versions are badly
  overloaded and the results are mixed. Spawn search still takes about 60% of
  the slowest ticks
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-battledome-post-spawn/REPORT.md)).

### Changed

- Food lists no longer keep the order in which food appeared. When food is
  eaten or removed by a shrinking wall, the last item in the list takes its
  place, so code that relied on that order, such as a bot breaking ties by
  list position, can behave differently. The `serve` protocol is now
  `slither-sim-json-v6`, the C ABI is `slither-sim-abi-v4`, and checkpoints
  are `slither-sim-checkpoint-v3` with rules v6; older checkpoints are
  rejected. `remove_food` still keeps the order of the remaining food.
- The fidelity collectors no longer find servers. They connect to the address
  you give them with the standard protocol-19 login. The tools that found
  outside servers and prepared pilots on them are removed; earlier studies keep
  the scripts they used.

### Performance

- Large food indexes use compact cell heads sooner.
- Large food worlds use compact cell heads for pickup lookups. The frozen
  10,000-snake replay saves 0.56 ms of world CPU per tick in ten timing pairs
  (eight faster), with unchanged food, snake state and ordered events.

- Removing eaten food no longer shifts the rest of the food list. In a frozen
  10,000-snake world, world CPU per tick falls from about 11.7 to 7.7 ms, with
  all ten timing pairs faster, and the 500-player catalog step gets about 28%
  faster. Pellets, snake growth and event order are unchanged.
- Bot observations in large worlds find nearby snakes through a bounds tree
  instead of checking every snake, and skip empty stretches of the food list
  when restoring food order. In a fresh 10,000-snake official-density world,
  observation CPU per decision falls from about 123 to 42 µs, and the 500-bot
  catalog step gets about 13% faster. Observations are unchanged.
- Food pickup reads pellet coordinates straight from its index and looks up a
  pellet's list position only when it is eaten. In a frozen 10,000-snake
  world, world CPU per tick falls from about 11.7 to 11.1 ms. Pickup results
  and event order are unchanged.
- Collision checks keep each snake's body bounds until its body changes,
  instead of rescanning every body point on every tick. In a frozen
  10,000-snake world, world CPU per tick falls from about 14.2 to 11.6 ms, and
  the 500-player catalog step gets about 6% faster. Collision results, death
  order and killer attribution are unchanged.
- Removing eaten food copies the surviving pellets in one pass instead of
  rotating them through the list. In a frozen 10,000-snake world, world CPU
  per tick falls from about 17.1 to 14.2 ms, and the 500-player catalog step
  gets about 10% faster. Pickup results and food order are unchanged.
- Food pickup keeps its fast grid in arenas up to a radius of about 131,000,
  official density for about 38,000 snakes, instead of 65,536 (about 9,700).
  Larger arenas fell back to a slower path: at 9,800 snakes a world step cost
  about 58 times as much as at 9,700. Pickup results are unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-population-scaling/REPORT.md#uncapped-10k-censored-with-a-pickup-algorithm-cliff)).
- Battledome spawn searches now check each candidate spot only against
  nearby snakes, using a grid built on demand in arenas with at least 32
  snakes. Every verdict, spawn position and output byte is unchanged. In a
  frozen 318-snake world built to match the live 320-player load, a failed
  spawn search uses about 76% less CPU (about 9 to 2 ms) and a successful one
  75% less; 80- and 160-player checks stay within 10 µs per tick. The study's
  frozen limit for an eight-snake overload rejected the change, because each
  failed search there costs 0.1 ms more; it was adopted anyway after the
  results, since that is about 1.5% of a 7 ms tick
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-spawn-obstacle-index/REPORT.md)).

## [0.8.2](https://github.com/NikitaMGrimm/slither-sim/compare/v0.8.1...v0.8.2) - 2026-10-29

### Added

- A memory study of the simulation thread in a fully grown 500-player world.
  Each player's snake history reserves a slot for every snake, 20.38 MB in
  total, to hold 176.7 kB of snakes that players actually know, and those
  paths take 44% of sampled address-translation misses on ordinary ticks.
  The test machine lacked deeper cache counters, so the study doesn't show
  that the thread is memory-bound overall. No runtime change
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-simulation-memory/REPORT.md)).
- A breakdown of the socket thread's CPU with 500 real sockets in a fully
  grown world: about 3.3 ms per food-update tick and 2.2 ms otherwise, mostly
  the sends themselves. An epoll version kept output byte-identical, but its
  live measurements never met the bar, so epoll is rejected and its code is
  kept only as a patch
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-gateway-pass-cost/REPORT.md)).
- A real-socket Battledome recheck of the minimap and spawn-search speedups,
  with bots boosting almost all the time. At 160 players neither version falls
  behind, and frame jitter drops from 2.2 to 1.6 ms. At 320 the server still
  falls behind, for 434 ticks of the measured window against 749 without the
  speedups, and spawn searches still take 64% of the slowest ticks. 480
  players were not measured
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-battledome-live-recheck/REPORT.md)).
- A follow-up Battledome check with the bots boosting almost all the time,
  the way Battledome players do. 80 players still play smoothly, with
  sub-millisecond jitter. At 160 there are occasional slow ticks, and at 320
  the server falls behind and respawns take over a second. Searching for safe
  spawn spots takes most of the time in the slowest ticks
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-battledome-boost/REPORT.md)).
- A Battledome check on one core of an AWS c7a.medium, with real WebSocket
  bots on a second machine, from 80 to 640 players. At 80 the median tick
  takes 1.1 ms. Up to 320 players the slowest tick stays under 7.5 ms, with no
  stalls and respawns within 20 ms. At 480 frame delays grow, and at 640 spawn
  searches overload the server and it falls behind. The bots boosted only
  3–5% of the time, so constant-boost human play is still untested
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-battledome-experience/REPORT.md)).
- A capacity check on one core of an AWS c7a.medium, with real WebSocket bots
  on a second machine. Starting from fully grown snakes and adding bots in
  steps, it reached its preset 750-bot limit at 6.4 ms median and 9.9 ms p99
  per tick without falling behind, so the 7 ms limit itself wasn't found. The
  p99 first passed 7 ms between 500 and 550 bots. Bots added during the climb
  started small, so the busiest steps had younger snakes than a fully grown
  server
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-c7a-mature-capacity/REPORT.md)).

### Performance

- Each player's record of the snakes it has been told about now keeps only
  those snakes, in one sorted list, instead of a slot for every snake in the
  world. Output is byte-identical. In a fully grown 500-player world, ticks
  use 14% less CPU with stock clients and 18% less with protocol 13, and the
  memory reserved for these records falls from 20 MB to 0.4 MB. The study's
  frozen 2% limit for small worlds rejected the change, because worlds of 1
  to 50 players pay up to 1.6 µs more per tick; it was adopted anyway after
  the results, since that cost is negligible next to a 7 ms tick
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-compact-snake-history/REPORT.md)).
- Battledome spawn searches now skip groups of eight body edges that are
  provably too far from a candidate head. Each group's bounds are built only
  when a search first reaches that snake, so ordinary respawns cost the same
  as before. Spawn positions, random draws and admission rules are unchanged,
  and output is byte-identical. In a synthetic overload where every spawn
  search fails, the host uses 63% fewer instructions and CPU p99 falls from
  1.5 to 0.6 ms; 80- and 160-player checks are unchanged within noise
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-spawn-head-blocks/REPORT.md)).
- Battledome spawn searches now decide whether a candidate head clears an
  older body edge from the squared distance, and fall back to the exact
  `hypot` only when the result is too close to call. Spawn positions, random
  draws and admission rules are unchanged, and output is byte-identical. In a
  synthetic overload where every spawn search fails, the host uses 40.5% fewer
  instructions and its slowest ticks take about half the CPU time (p99 3.3 to
  1.6 ms). In ordinary 80- and 160-player checks, instructions are unchanged
  but median tick CPU is 4–17 µs higher, which the study could not attribute
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-spawn-head-distance/REPORT.md)).
- Battledome spawn searches no longer build a candidate body whose head spot
  is already blocked, since the search would reject it anyway. Spawn positions,
  random draws and admission rules are unchanged, and output is byte-identical.
  In a synthetic overload with 72 players waiting to spawn, where every search
  tries all 4,096 candidates, the host uses 36.7% fewer instructions
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-spawn-ring-prefilter/REPORT.md)).
- Battledome's pit minimap now reuses its outline mask between updates
  instead of recomputing each cell's distance from the pit edge. Output is
  byte-identical. In a synthetic 80-player check, minimap ticks use about 52%
  fewer instructions (about 0.6% over all ticks); the first map, and any map
  after the outline changes, costs about 13% more
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-pit-minimap-cache/REPORT.md)).
- When a player's view moves, the host keeps its existing reference to each
  unchanged food sector instead of dropping and re-taking it. Output is
  byte-identical. In a fully grown 500-player world, food-update ticks run
  about 0.2% fewer instructions; no CPU-time change was measurable
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-food-packet-bytes/REPORT.md)).
- On the food-update tick, the host now skips each player's sector cleanup
  until one of their retained sectors can actually expire. Output is
  byte-identical. In a fully grown 500-player world those ticks run about
  3.6% fewer instructions, while other ticks run about 0.15% more, so total
  work is unchanged; the CPU-time saving is within measurement noise
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-subscription-cleanup/REPORT.md)).
- On the food-update tick every 504 ms, the host now works out each sector's
  food changes once and shares them with every player holding the same
  previous view, instead of recomputing them for each player. Output is
  byte-identical. In a fully grown 500-player world those ticks run about 7%
  fewer instructions (1% over all ticks); the CPU-time saving is within
  measurement noise, and the slowest ticks still exceed 7 ms
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-29-food-diffs/REPORT.md)).

## [0.8.1](https://github.com/NikitaMGrimm/slither-sim/compare/v0.8.0...v0.8.1) - 2026-10-29

### Added

- A capacity check with 500 real WebSocket players and fully grown snakes,
  with the host on one core of a Zen 5 EPYC. The median tick takes 4.3–4.9 ms
  of the 7 ms budget, but the slowest 1% take 8.8–9.2 ms, almost all of them
  the food update every 504 ms. The recent broadcast speedups removed 58% of
  the simulation thread's instructions but only about a third of its CPU time
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-capacity-checkpoint/REPORT.md)).

## [0.8.0](https://github.com/NikitaMGrimm/slither-sim/compare/v0.7.1...v0.8.0) - 2026-10-28

### Added

- A note on measuring the host with fully grown snakes. A 500-seat world
  with snakes as long as #248's can be saved and restored exactly, but the
  host can't run from it: the checkpoint doesn't keep the food history the
  host needs to show pellets to players. That study therefore reported only
  short-snake performance results
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-mature-broadcast/REPORT.md)).
- A study of whether one core can run 2,281 snakes, the most the stock client
  can show at official density. It can't yet: with 2,280 bots and one player,
  the server manages about 10 ticks per second against 143. Counting only the
  world's own work, as if every bot were a remote player, a tick costs 32–34 ms
  against a 7 ms budget, before the socket cost of 2,281 real connections.
  Three quarters of that is building each player's update; at 500 bots the
  same work fits in 70–78% of a tick
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-official-density-capacity/REPORT.md)).

### Breaking

- `Food` now implements `Copy`, so pellets are copied instead of cloned.
  Rust code with a non-`move` closure that uses a `Food` now captures it by
  reference instead of moving it; add `move` if the closure must own it.

### Performance

- The host now keeps each snake's body-block bounds between ticks and reuses
  them while every body point is unchanged, rebuilding them after any change.
  Output is byte-identical. With fully grown snakes, host ticks in the
  500-player official world run a further 10.9% fewer instructions with stock
  clients and 10.6% with protocol 13; with short snakes they run 0.6–1.0% more
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-verified-bounds-reuse/REPORT.md)).

- The host also skips body blocks that lie inside the square around a
  player's view but entirely outside its circle. Output is byte-identical.
  With fully grown snakes, host ticks in the 500-player official world run a
  further 10.6% fewer instructions with stock clients and 10.4% with protocol
  13; with short snakes the change is negligible
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-circle-body-coverage/REPORT.md)).

- When deciding what each player can see, the host now skips whole blocks of
  eight body segments whose bounds are too far away, instead of checking every
  segment. Output is byte-identical. With fully grown snakes, host ticks in
  the 500-player official world run 22.9% fewer instructions with stock
  clients and 22.5% with protocol 13, including the cost of building the
  bounds; worlds of short snakes run 0.6–1.0% more
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-mature-covered-bounds/REPORT.md)).

- Each tick, the host decides which snakes and prey each player can see by
  comparing squared distances, and only near the edge of view falls back to
  the slower exact distance it used before. Output is byte-identical. Host
  ticks in a 500-player official world with fully grown snakes run 23.6% fewer
  instructions with stock clients and 23.2% with protocol 13; with short
  snakes the saving is 3.0–3.3%
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-broadcast-distance/REPORT.md)).

- Bots running inside the server build their view of the game faster. They
  copy the food list from a snapshot kept until the food changes, and update
  other snakes' predicted heads once per motion report instead of replaying
  every report on each frame. What bots see and do is unchanged, and the
  500-seat official bot world runs 2.7% fewer instructions
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-client-frame-materialization/REPORT.md)).

## [0.7.1](https://github.com/NikitaMGrimm/slither-sim/compare/v0.7.0...v0.7.1) - 2026-10-28

### Added

- A study of how the simulator's cost grows from 500 to 10,000 snakes on one
  core. Moving the world stays at about 13–22 thousand instructions per snake
  per tick, but finding a safe spot for each new snake grows roughly with the
  square of the population and dominates from about 1,000 snakes. 10,000
  snakes on one core isn't realistic with the current code; 2,000 looks
  reachable once spawn placement gets faster. Game behavior is unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-population-scaling/REPORT.md)).
- A study of how snakes slide along the arena edge. In three official
  recordings with shallow edge contacts, the current sliding rule predicts
  positions to about one world unit, against about 25 for free motion. The
  recordings hold too few clean contacts to settle the rule's exact details,
  and a local trial of a capture found none either, so no capture is planned
  and the rule stays as it is
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-28-wall-sliding/REPORT.md)).
- A live capture meant to tell apart the two explanations left by the turn
  study: dividing by 111, or by the stock client's 106 with a fixed offset in
  the segment count. Two full 20-minute official sessions ran cleanly, and the
  test snake reached 37 and 100 segments, but it never completed a turn that
  met the frozen eligibility rules, so nothing was scored and the question
  stays open. Game behavior is unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-28-turn-offset-capture/REPORT.md)).
- A study of when prey start fleeing and how fast they flee. The current rules
  reproduce all 13 recorded test cases exactly, with flight-speed error 0.04
  against 0.38 for the older two-speed rule. Two close variants of each rule
  can't be told apart from five official recordings, and no capture with two
  connections could separate them, so the rules stay as they are
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-28-prey-flight-activation/REPORT.md)).
- A study of the boost speed-up and slow-down. The current rule, taken from
  the Android app's offline mode, reproduces all 1,567 recorded ramps behind it
  (slope error 0.10, against 0.15 for the older fitted formula). How speed
  carries over when a snake grows or shrinks mid-ramp can't be told apart from
  the recordings, and none of the 72 capture schedules tested could do it
  either, so the rule stays as it is
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-28-boost-ramp/REPORT.md)).
- A study of why the simulator's turning rule divides by 111 where the stock
  client divides by 106. On the recordings behind the original fit, 111 matches
  observed turning about five times better than 106, but a fixed offset of 7–8
  segments in the count also still fits, so the cause stays open. Game behavior
  is unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-28-turn-count-phase/REPORT.md)).
- A study that replays official and local recordings through SlitherClient,
  a Rust port of the stock client, and measures how much its predictions of
  other snakes get corrected. It maps the client's motion formulas against the
  simulator's and ranks the remaining fidelity gaps; the comparison is not
  matched, so it makes no parity claim, and game behavior is unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-27-client-view/REPORT.md)).
- The hosting guide explains how to build the server for the machine's own
  CPU. In our tests its game output matched the portable build exactly, and
  its median cycle count was 3.5–4.5% lower on one shared development machine;
  portable archives, the crate and Python wheels are unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-cpu-targets/REPORT.md)).
- Tools for testing whether official servers compensate for latency when
  judging collisions, and a first attempt. None of the 27 active official
  servers was close to empty (the quietest had 61 players), so no controlled
  encounter ran and the question stays open. Collision rules are unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-27-collision-delay/REPORT.md)).

### Changed

- When an arena that grows with its seats would get too big for the stock
  client, the error now says how many seats fit at official density (2,281
  for the official preset) and how to host more in a fixed, denser arena.
- The hosting guide's resource table now reflects this release's performance
  work. One Zen 4 server core now meets the delivery target with 375 official
  players at 46% CPU; 500 players use 61% CPU but still miss it. One Zen 5
  core meets the target at 500, and one Zen 4 core with 80 Battledome players.

### Performance

- Finding a safe spot for a new snake now skips snakes whose bodies are too
  far away to matter, instead of checking every body in the world. Output is
  byte-identical. At 2,000 snakes, placement drops from 162M to 3.1M
  instructions per tick and the whole tick from 197M to 38M. In a 500-player
  official server it was already only about 0.2% of the cost
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-28-spawn-bounds/REPORT.md)).
- Food pickup now works out each pellet's place in the food list from a
  bitmap, instead of rewriting the stored position of every later pellet after
  each removal. Stale entries are cleaned out 512 pellets per tick, so no
  single tick pays for a full index rebuild (each batch stays under 0.28M
  instructions). Output is byte-identical, and the 500-player official fixture
  runs 2.4–2.6% fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-pellet-bitset-ranks/REPORT.md)).
- When a player's update has no pong or minimap in front of it, the host sends
  the update's packet list as it is instead of copying it into a new list.
  Output is byte-identical, and the 500-player official fixture runs 1.6–1.7%
  fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-packet-lists/REPORT.md)).
- Each player's food-sector subscription is updated once per tick instead of
  twice. Output is byte-identical, and the 500-player official fixture runs
  1.9–2.1% fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-interest-refresh/REPORT.md)).
- Each tick, a snake's heading direction is computed once during movement and
  reused for its collision and pickup checks, and prey reuse the pickup circles
  already computed for food. Output is byte-identical, and the 500-player
  official fixture runs 2.6–2.8% fewer instructions with stock and protocol-13
  clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-core-heading-geometry/REPORT.md)).
- Each tick, the host rebuilds shared food listings only for sectors whose
  food changed or that a player started watching, instead of checking every
  watched sector; a moving Battledome pit still refreshes them all. Output is
  byte-identical, and the 500-player official fixture runs 7.2–7.8% fewer
  instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-shared-food-work/REPORT.md)).
- When the arena or Battledome pit shrinks, pellets left outside it are
  removed from the food cache directly instead of forcing a full rebuild on
  the next tick. Output is byte-identical. In an official test where players
  leave over time, ticks that remove pellets drop from a median of 26.7M to
  1.8M instructions, and the whole run uses 85% fewer instructions (37% fewer
  in Battledome); steady 500-player ticks cost 0.03–0.04% more
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-mirrored-food-retention/REPORT.md)).
- Snakes a player already sees that haven't changed since that player's last
  update are skipped before any packet work, and a snake's wanted heading is
  computed only when a packet needs it. Output is byte-identical, and the
  500-player official fixture runs 3.1–3.4% fewer instructions with stock and
  protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-snake-noops/REPORT.md)).
- The grid that finds which snakes each player might see is kept between
  ticks, and only snakes that moved into different cells are updated, instead
  of rebuilding it every tick. The exact visibility test and packet order are
  unchanged. Output is byte-identical, and the 500-player official fixture
  runs 11.6–12.1% fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-coverage-cells/REPORT.md)).
- Each player's prey visibility check looks only at prey in nearby x-axis
  strips instead of every prey in the world; the exact distance test and world
  order are unchanged. Output is byte-identical, and the 500-player official
  fixture runs 2.1–2.2% fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-prey-cells/REPORT.md)).
- Collision and prey lookups number their grid cells by truncating instead of
  calling `floor`; the cells only need to keep their order, and the exact tests
  are unchanged. Output is byte-identical, and the 500-player official fixture
  runs about 1.5% fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-monotone-bounds-cells/REPORT.md)).
- Food pickup updates the stored positions only of pellets after the first
  eaten one, since earlier pellets don't move. Output is byte-identical, and
  the 500-player official fixture runs 1.4–1.5% fewer instructions with stock
  and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-pellet-position-prefix/REPORT.md)).
- Food pickup removes eaten pellets by moving whole ranges of surviving
  pellets, instead of filtering the food list through a per-pellet mask.
  Output is byte-identical, and the 500-player official fixture runs 4.1–4.4%
  fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-food-compaction/REPORT.md)).
- Simulation ticks no longer compare the food cache with every pellet when no
  untracked food edit happened since the last comparison. Debug builds still
  check the skipped comparison. Output is byte-identical, and the 500-player
  official fixture runs 5.5–5.9% fewer instructions with stock and protocol-13
  clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-food-mirror-reuse/REPORT.md)).
- Host food delivery checks only the changed food sectors inside each
  player's subscribed range, instead of every changed sector in the world.
  Output is byte-identical, and the 500-player official fixture runs 6.4–6.8%
  fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-food-ranges/REPORT.md)).
- Food pickup tests only the pellets in grid cells that a snake's pickup
  circle covers, instead of every pellet each tick, using a cell index kept up
  to date as food appears and is eaten. Output is byte-identical, and the
  500-player official fixture runs 14.7–15.6% fewer instructions with stock
  and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-pellet-lookup/REPORT.md)).
- The host's socket thread finds a player's connection by slot instead of
  searching every connected session for each delivery. Output is
  byte-identical, and the 500-player official fixture runs 3.5–3.9% fewer
  instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-delivery-index/REPORT.md)).
- Protocol-13 frames reuse each converted packet's buffer for the frame header
  instead of copying the packet into a new frame. Frames are byte-identical,
  and the 500-player official fixture runs 2.3% fewer instructions with all
  protocol-13 clients and 1.2% fewer with half; with stock clients it runs
  0.1% more
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-owned-legacy-frames/REPORT.md)).
- Protocol-13 clients that ask for full `U` minimaps share one decoded and
  projected map per distinct map state and projection, instead of each building
  its own. Output is byte-identical. With all 500 fixture clients opted in, a
  minimap tick takes 13M instead of 351M instructions and the whole fixture
  runs 7.2% fewer; clients that don't opt in are unaffected
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-full-map-sharing/REPORT.md)).
- Food pickup removes eaten pellets from its occupancy cache by index and
  copies the surviving ranges, instead of scanning the whole cache twice.
  Output is byte-identical, and the 500-player official fixture runs 4.0–4.2%
  fewer instructions with stock and protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-pickup-cache/REPORT.md)).
- Linux hosts ask the kernel once per gateway pass which sockets have input,
  and stop reading a socket that has none until it does. Before, every pass
  tried a read on every open socket. With 500 stock or protocol-13 players,
  socket-thread instructions fell by 18–21% and whole-host instructions by
  2.9–3.9%. Other platforms are unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-socket-readiness/REPORT.md)).
- Host broadcasts build each player's list of nearby and already shown snakes
  by merging two sorted lists into reused storage, instead of allocating and
  sorting a new list for every player. Output is byte-identical, and the
  500-player official fixture runs about 3% fewer instructions with stock and
  protocol-13 clients
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-snake-candidates/REPORT.md)).
- Battledome spawn searches reject a candidate whose head lacks the required
  runway before testing its whole body. Spawn choices are unchanged. A full
  search takes 34M instead of 79M instructions at 64 seats (36M instead of 86M
  at 80), and the 64- and 80-seat host fixtures run 27% fewer instructions
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-admission-heads/REPORT.md)).

## [0.7.0](https://github.com/NikitaMGrimm/slither-sim/compare/v0.6.2...v0.7.0) - 2026-10-27

### Breaking

- Battledome keeps its 3,584 radius up to 32 players and grows above that so
  each player keeps the same space: about 5,069 at 64 seats and 5,667 at 80.
  The pit and spawn band scale with it. A spawn that finds no strictly safe
  spot within two seconds takes the safest spot that still keeps clear of
  bodies and of other snakes' paths to the middle. `dynamicRadius: false` keeps
  the 3,584 arena. Checkpoints use rules revision `slither-sim-rules-v5` and
  reject earlier continuations.
- Battledome food stays inside the pit (`foodOnlyInsidePit`). Ambient food
  spawns only there, scaled to the pit's area. Every death drops food,
  including floor-size snakes and pit-edge deaths, but only pellets that land
  inside the pit remain. Boost pellets follow the same rule, so boost mass
  spent outside the pit is lost. Food left outside a moving edge is removed.
  Worlds without the setting keep their old food rules. Rust `Config` literals
  need the new `food_only_inside_pit` field.
- In worlds with `killScoreSeconds` (Battledome), the stock leaderboard and own
  rank show rolling kills instead of size. Players who left stay listed while
  their kills count; a new player in their seat starts at zero. Pit worlds'
  minimaps show only snakes inside the pit and default to 256 cells (136
  elsewhere), so `SimulationConfig.minimap_side` is now `Option<usize>`; JSON
  accepts an integer or `null`.
- SlitherSim hosts tell bot clients the live mode rules: pit radius, size
  floor, kill-score window, boundary and free floor boost. They are sent at
  setup and whenever they change, and surface as the optional
  `ClientFrame.rules`. Host and socket bots read the same field, so
  `ForagerBot::with_world_rules` is gone; use `new()`. Client JSON stays v2,
  and stock browsers never receive the message.

### Added

- Hosts accept the NTL browser mod. Its legacy login gets protocol 13 on that
  connection: snakes, food, death and rejoin, and Battledome's kill board, pit
  minimap and edge ring all work. Its minimaps are the same `M`/`V` packets
  protocol 19 gets, so NTL scales the map and its own-position dot to the live
  arena in every mode. Stock clients and bots still get protocol 19, byte for
  byte as before.
- The Frontier and Precise slither browser mods can join hosts too. Frontier's
  short login gets protocol 19; Precise gets protocol 13 and updates its
  minimap only from full `M` maps, since it ignores `V` deltas. NTL's other
  handshake requests also work: `[99]`, and `[99,3]` with its dynamic minimap
  off, which gets a `U` map of the whole grid to match its dot.

### Changed

- Host bots each wear their own stock skin, kept across lives, instead of all
  using skin 0. Their death pellets take that skin's colors.
- Host bots have names such as `Mamba (bot)`, kept across lives, instead of
  none. Official servers mark their bots with the same suffix.
- Linux hosting resource guidance now includes real socket tests on one Zen 4
  and one Zen 5 EPYC core, with delivery results and Battledome spawn waits.

### Fixed

- A client that falls too far behind is disconnected instead of stopping the
  host. Each socket may queue up to 2 MiB of output, counted the same way for
  protocol 13 and 19; stock clients previously had a 4,096-frame limit.
- Hosts send a newly encoded frame right away instead of at the socket
  thread's next 1 ms poll. The 99th-percentile delay between encoding and
  sending fell from 1.5 to 0.03 ms on Windows and from 1.6 to 0.4 ms on Linux
  under WSL2, without busy waiting
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-27-host-frame-timing/REPORT.md)).
- Battledome background bots head into the pit after spawning and treat its
  edge as a lethal wall once inside, instead of dying on it. In three local
  three-minute runs with 64 bots, they spent 87–97% of their time in the pit
  (previously 15–18%) and never died at its edge (previously 18–23 times a
  minute) ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/battledome/2026-10-27-pit-aware-bots/REPORT.md)).
  Bots in worlds without a pit decide exactly as before.
- Food removed by moving arena boundaries updates the affected sectors without
  rebuilding the whole food index or revealing waiting ambient births before
  their scheduled update. Survivor colors stay unchanged.
- The pit-edge pellet ring stays visible while the arena shrinks. Replacement
  pellets used to restart the stock client's fade-in before it finished, so the
  ring could stay nearly invisible
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/battledome/2026-10-26-stock-presentation/REPORT.md)).

### Performance

- Hosts encode each minimap once, and reuse each delta for every player who
  holds the same previous map, instead of encoding both per player. Minimap
  ticks were a large spike: with 500 players they took 291 million
  simulation-thread instructions against 16 million on other ticks, and now
  take 19 million. Battledome with 64 players drops from 112 to 12 million.
  Packets are unchanged
  ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-minimap-sharing/REPORT.md)).
- Host food-sector listings track subscription changes incrementally instead
  of rebuilding the union of every client's subscriptions each tick. Packet
  bytes and order are unchanged; the official-500 fixture uses 2.5% fewer user
  instructions ([study](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-27-host-sector-union/REPORT.md)).
- Host food updates reuse each broadcast's changed sectors for every recipient
  instead of rescanning per recipient. Output is byte-identical; the official
  500-client fixture runs 6.5% fewer instructions
  ([measurements](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/performance/2026-10-26-host-food-sharing/REPORT.md)).

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
