# Reproduce the correctness checks

Baseline `cf67b80e`; implementation `af420c84`. Later commits add only these
trace helpers and evidence notes. Full sources, logs, copied executables and
traces stay local-only outside Git. No timing result is produced here.

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

The required Linux `instructions:u` fixed-work pairs and slow-tick splits have
not run. Follow the [frozen plan](PLAN.md) and its
[fixed-work amendment](PLAN-ADDENDUM.md) only after the maintaining agent
releases the SSH bench. Do not substitute these debug checks for measurements.
