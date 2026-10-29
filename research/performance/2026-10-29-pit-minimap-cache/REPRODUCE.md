# Reproduce the correctness checks

Native timing pins are baseline `cf67b80e` and implementation `af420c84`.
The final branch also integrates main's subsequent #264 food-listing change;
its checks and fresh full-byte traces are recorded separately. Full sources,
logs, copied executables and traces stay local-only outside Git. The portable
correctness commands below do not produce timing evidence.

Use one Cargo job and one Rust test thread. The focused commands are:

```sh
cargo test --locked -j 1 --lib client::minimap
cargo test --locked -j 1 --lib client::simulation::pit_tests
cargo test --locked -j 1 --lib client::simulation::protocol13_tests
cargo test --locked -j 1 --lib
cargo clippy --locked -j 1 --all-targets -- -D warnings
```

For the focused mutation, replace only `radius < self.min_near_outside` with
`radius <= self.min_near_outside` in `src/client/minimap.rs`; run the projection
tests and require a failing oracle. Restore the original bytes in a `finally`
block, then rerun the library tests. Never retain mutation code in production.

The portable [snapshot overlay](prepare_traces.py) reuses the
[earlier host trace fixture](../2026-10-27-host-snake-candidates/protocol_fixture.rs)
and [catalog validator](../../../tools/perf_baseline.py). It adds stock and
protocol-13 eight-seat Battledome rows, with live radius updates at 0.014, 0.532
and 3.01 seconds, crossing growth/shrink and several minimap opportunities.
The original fixture's controls, forced reconnect, full initial/final world
checkpoints, spawn/latency RNG and every frame/time remain in the trace. These
are deterministic correctness fixtures with scripted headings/boosts; they
are not the realistic received-frame AWS workload or performance rows.

Use a fresh external directory and existing locked dependencies. The overlay
uses debug builds and one bounded shared target, copies each executable, and
records source/archive/driver/binary hashes. Its counter is explicitly disabled.
The mutation changes only the cached projected pit radius by a factor of 0.98.

```sh
python -X utf8 prepare_traces.py REPO OUTPUT base cf67b80e
python -X utf8 prepare_traces.py REPO OUTPUT candidate af420c84
python -X utf8 trace_matrix.py REPO OUTPUT catalog
python -X utf8 prepare_traces.py REPO OUTPUT mutation af420c84
python -X utf8 trace_matrix.py REPO OUTPUT host
python -X utf8 trace_matrix.py REPO OUTPUT mutation
```

The matrix checks all 23 available non-socket catalog rows, the four historical
host fixtures (stock, mixed, protocol 13 and 2,281 seats), and both new Battledome
fixtures. It compares full decompressed bytes and checks terminal records;
successful hashes alone are insufficient. Raw output and stderr remain after
errors or truncation. The final mutation must change tag-2 wire-frame bytes
in both Battledome protocols, not merely an aggregate summary.

The initial catalog execution inherited the local default Rayon setting; the
subsequent host/mutation execution explicitly uses one Rayon thread. Both arms
of every pair use the same execution context. These are untimed parity checks.

The retained execution used the original 16-seat overlay for the 23 catalog
and four historical host rows. Its first supplementary Battledome initialization
refused slot 14, so no incomplete Battledome trace qualified. Its sources,
binaries and failure remain external. A fresh directory with eight seats then
repeats both supplementary protocol traces and the two wire mutation controls.
Use mode `battledome` instead of `host` for that supplementary-only execution.
The optional `CARGO_TARGET_DIR` lets both execution directories share one bounded
build target; copied executables and per-directory pins remain immutable.

## Native fixed-work collection

The maintaining agent released the guest before these measurements. Reuse the
common [spawn-study harness at its frozen preparation commit](https://github.com/NikitaMGrimm/slither-sim/tree/dc12d8781cd3ce1594d7d930e859c80b3f8532a1/research/performance/2026-10-29-spawn-ring-prefilter),
including its preparation wrapper, accessors, full-host fixture, native builder,
per-tick counter, pair runner, gate writer and raw reducer. Link those scripts;
do not copy them into this study. Use this study's portable trace helper as the
wrapper's `PRIOR_PREPARE`. Stage baseline `cf67b80e`, candidate `af420c84` and
mutation `af420c84` with `STUDY_STAGE_ONLY=1` and `STUDY_MUTATION=map`. All
builds remain locked/offline and serial. The native builder clears only package
artifacts in its verified task-owned target before each arm.

For the original cold row, keep 300 warm ticks. For a fresh independent warm
row, run `python warm_stage.py STAGED_OUTPUT` before the three native builds;
this reproduces the recorded 600-tick source manifests. Do not edit production
code. Build all three arms; run the native catalog and host matrices; run the
queued fixed parity with `FIXED_TRACE_TICKS=720`; bind the completed local Rust
receipt and native gates with `write_gates.py`. Then run:

```sh
python run_pairs.py OUTPUT PRIOR_REPRODUCE 19 minimap
python run_pairs.py OUTPUT PRIOR_REPRODUCE 13 minimap
python tables.py OUTPUT minimap
python summarize.py minimap-pairs.json --markdown
python summarize.py minimap-warm-pairs.json --markdown
```

The first three scripts are the linked common harness. `PRIOR_REPRODUCE` is the
[earlier inventory helper](../2026-10-28-broadcast-distance/reproduce.py).
The two selected files correspond to separate cold/warm output directories;
copy the raw reducer's `selected-fixed-minimap.json` under the respective name.
Omit `--markdown` for all per-quantile medians/ranges. Complete native
identities, gates and raw archive hashes are selected beside the report.
Full raw archives are local-only, not public downloads.

Freeze complete host state and compare repeated-fork/direct-parent and
cross-arm full bytes before accepting counts. One AB warmup is discarded, then
five AB/BA/AB/BA/AB pairs per protocol count 360 actual ticks on one CPU. No
status, build or profiler process runs during counted windows. CPU 1 runs the
controller. Retain inventories, failures and every pair; require at least 99%
PMU scheduling. These results are current-thread CPU/user instructions for
synthetic work, not sockets, network latency or live deferred ticks.

For retrieval, close and tar-read the archive, compute its size and digest,
then publish an atomic ready receipt. Download only after readiness, verify
size/digest, safely extract, and verify every per-file size/hash and successful
step receipt. Never remove remote source/results before that verification.
Finally remove only task-owned targets/processes/files and record cleanup.
