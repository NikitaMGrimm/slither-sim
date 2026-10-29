# Reproduce the exact head threshold comparison

All native work runs on an authorized AWS builder, never the shared Windows
machine. Baseline `d65d89ca`; candidate source pin is recorded in the selected
correctness evidence. Native sources, logs, binaries, whole traces and process
inventories remain local-only outside Git. Selected digests do not grant access.

Run focused `cargo test --locked --lib spawn::safety::head_tests` before the
full `tools/check.py rust` command group. Keep Cargo and Rust test concurrency
bounded remotely. The oracle freezes scalar projection/hypot and near arithmetic
independently of Safety. Its swept reference checks both comparison directions.
For the focused mutation, additionally reject `segment_distance == head_width`
at the optimized call site. Require a test failure, restore the original file
in `finally`, verify its exact digest and require focused tests to pass again.

Reuse the [previous trace/fixed-host recipe](../2026-10-29-spawn-ring-prefilter/REPRODUCE.md),
its `build_staged.py`, `fixed_parity.py`, `write_gates.py` and table reducer, and
the minimap study's `trace_matrix.py`, by reference. This study's [prepare.py](prepare.py)
wraps only changed preparation: its wire mutation incorrectly refuses every
nonempty-neighbor head query, and it extends the old full-host fixture with separated 80/160-viewer
controls. A reconnect every 72 ticks is queued outside the interval, then actual
admission, physics, broadcast and hand-over run inside it. No sampled geometry,
observations, cached poses or artificial admission result replaces the search.
These are labelled synthetic controls, not real sockets or naturally grown play.
Initialize their arena with the normal `grow_for_seats` method before connecting
the synthetic viewers. The first unprimed control correctly hit the unchanged
newborn growth hold, left five requests waiting and was rejected. Growth and
admission rules inside the measured interval are never bypassed.

```sh
python prepare.py REPO OUTPUT base BASE
python prepare.py REPO OUTPUT candidate CANDIDATE
python prepare.py REPO OUTPUT mutation CANDIDATE
python ../2026-10-29-spawn-ring-prefilter/build_staged.py OUTPUT base
python ../2026-10-29-spawn-ring-prefilter/build_staged.py OUTPUT candidate
python ../2026-10-29-spawn-ring-prefilter/build_staged.py OUTPUT mutation
python ../2026-10-29-pit-minimap-cache/trace_matrix.py REPO OUTPUT catalog
python ../2026-10-29-pit-minimap-cache/trace_matrix.py REPO OUTPUT host
python fixed_parity.py OUTPUT
```

The build helper verifies staged source manifests before locked/offline release
builds and clears this package's prior artifacts between arms. Full Rust checks
use the uninstrumented candidate tree. Bind actual completed exit receipts and
copied binary/archive digests with the previous `write_gates.py` before counting.
All 23 catalog, six host and two queued traces must match complete bytes and
terminal records; both queued wire mutations must differ. The wrapper preserves
the baseline/candidate admission requirement while allowing the deliberately
wrong refusal control to leave all requests waiting. Its complete wire output
must still differ, not merely its terminal state. The earlier 1.5-width control
was neutral on this fixture and is retained as a failed control.

Use one thread/core for counted fork children, a separate controller core and
the original repeated-fork, cross-arm and direct-parent proofs. AWS hardware
`instructions:u` access must qualify without any kernel/security setting change;
retain failed probes. Counted intervals require >=99% PMU scheduling. Timing
labels the final AWS virtualized Intel/Spot context, not a dedicated physical host.

```sh
python run_pairs.py OUTPUT ../2026-10-28-broadcast-distance/reproduce.py 19 spawn
python run_pairs.py OUTPUT ../2026-10-28-broadcast-distance/reproduce.py 13 spawn
```

Repeat both protocols with `seats80` and `seats160`. One discarded warmup precedes
five AB/BA/AB/BA/AB pairs for each row/protocol. The primary `spawn` row is
unchanged: warm 300 ticks, freeze the full host and count 360 ticks with 72 waiting
joins and 180 exhausted searches of exactly 4,096 poses. The controls require
each reconnect to finish that tick, no pending backlog afterward and the full
80/160 viewers alive. Preserve all pair traces and inventories. No build, test,
poll or reduction runs concurrently with counted work.

Run the previous study's `tables.py OUTPUT ROW` after each pair set, then combine
its three selected JSON objects under the row names in `measurements.json`.
This study's `tables.py measurements.json` prints the report tables. Keep all
pairs and groups, including cold/warmup data, ordinary/exhausted searches and
each arm's independently selected slowest CPU 1%. Pins appear once in correctness
evidence, not in every timing arm/file. No live capacity or latency claim follows.

Close and tar/gzip-read each stage archive, verify every manifest member remotely,
hash it and atomically publish readiness. Download and verify size and archive
digest locally before source deletion. The immutable source/byte-proof archive
is retrieved before timing. The final timing delta references that archive and
contains the timing/selected-results additions; independently check selected
JSON against its verified manifest. See verification.json for both pins and
retrieval method, and the retained Rust-source identity receipt for the complete
Rust check group's source binding.
Terminate the builder, remove its project-tagged task key/group and scan every
enabled region with the explicit limited profile. Never modify unrelated resources.
