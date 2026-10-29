# Reproduce the obstacle broad phase

This is synthetic fixed-work admission evidence, not a live capacity test.
Read the frozen [plan](PLAN.md), [attempts](ATTEMPTS.md) and [report](REPORT.md).
The external checkpoint, full logs, source manifests, binaries and complete
traces are retained in verified private archives. Public selected summaries
pin those archives and instrumented sources without repeating per-file maps.
The study was initially published without production changes. The separate
[maintainer decision](REPORT.md#maintainer-decision) now adopts the identical
measured source in #278 without new builds or timing. The historical
[`perf/spawn-obstacle-index` prototype branch](https://github.com/NikitaMGrimm/slither-sim/tree/perf/spawn-obstacle-index)
at `98371b98` has the same production Rust and oracle tests as measured runtime
`c717b268`. Use this study directory from current #278 for the commands below:
the historical prototype predates the committed `prepare_counting.py` helper.

Use a dedicated remote Linux builder. Default portable release on both arms;
empty `RUSTFLAGS`, no `target-cpu=native`. The selected machine is a Frankfurt
c7i.2xlarge Spot with SMT disabled, four physical/logical CPUs. Count on CPU 0,
controller on CPU 1; do not overlap builds, tests, transfers or reduction with
counting. Builds use at most four jobs on that remote machine. No SSH guest or
local native work was used.

## Fixture and profile

[prepare.py](prepare.py) extends the earlier
[head-distance overlay](../2026-10-29-spawn-head-distance/prepare.py), which
reuses the [minimap trace harness](../2026-10-29-pit-minimap-cache/trace_matrix.py)
and [ring-prefilter fixture](../2026-10-29-spawn-ring-prefilter/REPRODUCE.md).
Keep these helpers in their existing locations rather than copying them.

```sh
python prepare.py REPO OUTPUT base 9ad002c1
python prepare.py REPO OUTPUT candidate c717b268
python prepare.py REPO OUTPUT mutation c717b268
python build_staged.py OUTPUT base
python build_staged.py OUTPUT candidate
python build_staged.py OUTPUT mutation
OUTPUT/base/bin/study_live warmup OUTPUT/../fixture
```

[live_fixture.rs](live_fixture.rs) freezes the first qualifying checkpoint and
selects the ascending refused/successful seeds without timing selection. For a
fresh run, `OUTPUT/../fixture` must not already exist: warmup creates that sibling
directory and writes `world.json` and `fixture.json` there. Both `run_pairs.py`
and `select_results.py` read exactly that location, not `OUTPUT/fixture`.
For archived replay, restore those files there instead of running warmup.
Preserve the generated `world.json` and `fixture.json`; the checkpoint digest
is in the measurement pins; [fixture.json](fixture.json) records the census. For diagnostic
profiling only, rebuild the baseline with full DWARF and identical portable
codegen, sample `cpu-clock:u` at 499 Hz with DWARF call stacks, then classify
the folded admission stacks using [classify_profile.py](classify_profile.py).
Do not use profile-instrumented timings as the selected speed measurement.

## Proof and counting

Run `cargo test --locked --lib spawn::safety` first, then
`python tools/check.py rust`. The independent raw-geometry oracles cover head,
body and departure verdicts and direct inclusion of every original outer
`near` candidate. Require the wrong-skip mutation (`self.bits |= 0`) to fail
`obstacle_index_cell_edges_thresholds_invalid_and_extreme_geometry`, restore
the exact source, then repeat the focused tests.

Run the earlier catalog/host trace matrix, queued fixed parity and both protocol
wire controls. The inherited
[write_gates.py](../2026-10-29-spawn-ring-prefilter/write_gates.py) consumes the
completed correctness/build inventory and 15-command Rust receipt, writing
`OUTPUT/required-checks-pass.json` and `OUTPUT/parity-pass.json`. It records
only pass flags and does not produce the additional live-admission receipt.

[prepare_counting.py](prepare_counting.py) exposes the original private
controllers' remaining steps. Its `gates` command runs write_gates.py and binds
its receipts to the hashes of the fixture and each arm's `study_live` and
`pins.json` in `OUTPUT/gates-bound.json`; `parity` and `pins` refuse to run if
any of them changed afterwards. The recorded run called write_gates.py directly,
so its archive has no `gates-bound.json`. The `parity` command runs
`OUTPUT/ARM/bin/study_live admission OUTPUT/../fixture ROW 3` for both arms and
both refused/successful rows. It checks repeated results, unchanged world state
and complete raw byte equality (including result bodies/errors and RNG), then
writes `OUTPUT/live-parity-pass.json`. Its `pins` command freezes
`OUTPUT/measurement-pins.json` with the SHA-256 of this study's `run_pairs.py`
(`runnerSha256`), sibling `fixture/world.json` (`fixtureSha256`), sibling
`fixture/fixture.json` (`fixtureMetadataSha256`), and each base/candidate
`OUTPUT/ARM/pins.json` (`arms.ARM.pinsFileSha256`). All three proof receipts
must pass before freezing. These commands create fresh files exclusively;
restore archived receipts/manifests unchanged when replaying retained evidence.

```sh
python prepare_counting.py OUTPUT gates CORRECTNESS_JSON RUST_RECEIPTS_JSON
python prepare_counting.py OUTPUT parity
python prepare_counting.py OUTPUT pins
python run_pairs.py OUTPUT ../2026-10-28-broadcast-distance/reproduce.py 19 live-refused
# Repeat labels 19 and 13 for live-refused, live-successful, spawn, seats80, seats160.
python select_results.py OUTPUT SELECTED.json
python assemble.py INITIAL.json SECOND.json SELECTED.json measurements.json
python tables.py
python tables.py --check
```

Each live window makes 512 complete fresh-Safety admission calls, resetting the
frozen RNG outside counting; result serialization/drop is also outside. One warmup pair is discarded, then five AB/BA pairs
are retained. The protocol labels are separate repetitions of protocol-independent
admission; actual protocol-specific host/wire work is covered separately. Old
rows use 360 complete host ticks from identical frozen forks. Selected summaries
retain all five pairs, PMU scheduling, all/search/exhausted/other/slow-tail groups
and the frozen gate, including failed attempts.

Close each archive, verify every member against its manifest, publish the ready
receipt last, then download and verify size/SHA before deleting any source.
Reconstruction follows the ordered archive chain in `verification.json`;
later archives are deltas. Public tables are recomputed from `measurements.json`.
