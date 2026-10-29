# Reproduce food-diff reuse

Use a fresh external output directory, existing locked offline dependencies,
and one bounded Cargo target. No packages, services or guest settings change.
All raw artifacts and full process command lines remain local-only.

The [plan](PLAN.md) fixes the workload and interpretation. Baseline runtime is
`692cd561714b8af3ea95adb3adcbd807b3a48734`; the measured implementation is
`15792fa591c796492226fb64c1e97b98b6ef086a`. Later edits add only
research documentation and a source-comment link. Main's intervening #256 adds
research material; #257 updates package versions to 0.8.1 without changing Rust
source or dependency versions. The PR preserves those upstream changes. Measured
runtime pins remain explicitly v0.8.0 rather than an unmeasured current revision.

Reuse these sources by reference, reconstructing their execution layout outside
the checkout. Do not copy their code into this study:

- [Tick-zero full-host fixture and fork workflow](../2026-10-28-mature-covered-bounds/REPRODUCE.md),
  especially `fixture.rs`, `serve.py`, `queue249.py`, `accept249.py`, `compare.py`.
- [Shared source overlays, trace matrix and current-thread counter](../2026-10-28-broadcast-distance/reproduce.py)
  and [counter.rs](../2026-10-28-broadcast-distance/counter.rs).
- [Previous execution setup](../2026-10-28-verified-bounds-reuse/REPRODUCE.md).
- [Checkpoint real-socket harness](../2026-10-28-capacity-checkpoint/REPRODUCE.md),
  including its unchanged received-frame client, timing source, process/context
  accounting and `select.py` gates. Its `harness.patch` reconstructs those tools;
  the linked #248 helpers supply the remaining imports.

Apply this study's measurement-only `harness.patch` in that external directory.
Its root constants name the retained execution directories; bind them to the
corresponding reconstructed earlier harnesses. Those paths identify local-only
execution layout, not downloadable data. Keep the generated Rust source and
copied-binary SHA-256 receipts. The production change is in its commit and is
not duplicated as a patch.

The validated core checkpoint does not include `SimulatorBackend`'s hidden-food
index, presentation lifetimes, shared listing handles, or each connection's held
listings and sector interest. Restoring only physical state therefore cannot
restore this maintenance path. Warm once at full speed without sockets or wall
pacing per runtime/protocol, then fork that frozen **full backend** for every
pair. There is no warmup per pair or invented checkpoint continuation.

Archive both runtime revisions as `base.tar` and `candidate.tar`, then run:

```sh
python3 setup.py
python3 build.py base
python3 build.py candidate
python3 build.py mutation
python3 check.py
python3 live_build.py
python3 parity.py
python3 measure.py short base candidate --warm 0 --ticks 1080
python3 run_fixed.py stock
python3 run_fixed.py legacy
python3 select_fixed.py
python3 run_live.py
python3 cleanup_target.py
python3 select_live.py
```

In this session the last off window and following fine diagnostic fail the
unchanged body-class gate. Preserve that failure, then use
`python3 select_live_partial.py` to reproduce the post hoc per-window quality
reduction. It applies the original selector independently to each whole primary
window, keeping all three that pass and excluding the fourth without consulting
its timing. This is not a complete balanced confirmation. The out-of-class fine
window is retained only as a labelled explanatory diagnostic with preflight
accounting checks; it has no mature-class qualification. A fresh reproduction
with four qualified windows uses the original full selector instead.

Finish every build/test before timing. The trace matrix compares complete
decompressed bytes, with terminal-record checks. It has 23 available deterministic
catalog rows and four host fixtures (stock, mixed, protocol 13, and 2,281 seats).
The shared radius perturbation must change both stock/protocol-13 wire traces;
the focused previous-listing mutation must fail the cache oracle. Neither is
production code. Warm bytes are compared live with bounded buffers. The two
repeat fork traces per runtime and both direct-parent traces must agree in full.

`build.py` adds per-advance user-instruction and process-CPU samples to the linked
fixture. Maintenance is an actual backend food-epoch transition; selection also
checks every flag against the unchanged source-derived floating-point cadence.
CPU intervals include identical PMU/clock wrappers; no overhead is subtracted.
Controls, admission, frame draining and trace serialization remain outside the
advance interval. Warm parents each have exactly one OS thread; every child opens
its own counter, checking event scheduling. Counted windows use CPU 0 and frozen
parents; the comparer/controller uses another CPU. One discarded AB warmup
precedes five AB/BA/AB/BA/AB pairs.

Store session traces on bounded task-owned temporary storage or transfer them
to verified external storage between cohorts. The first short attempt exhausted
persistent space during its last proof trace and is excluded. Its complete
archive was copied externally, SHA-256 verified, and every removed trace compared
against that archive before deletion. The fresh short cohort repeats the full
acceptance protocol; no incomplete timing was substituted.

`live_build.py` overlays one cache switch on the checkpoint host. Disabled calls
the original `sector_changes`; enabled uses the candidate. The switch is sampled
at 100-tick boundaries with two-second settling gaps. `run_live.py` first checks
eight sockets, then warms one full 500-socket A4 world from tick zero. It retains
the checkpoint's mature screen and four 15-second ABBA windows, followed by a
separate fine diagnostic. The host/sockets share CPU 0; three full received-frame
client workers use CPUs 1/2/3. Fine clocks and socket-call counters are disabled
in the four primary windows. Same-process controls bound the CPU cost of warmup;
they do not establish identical trajectories or two immutable runtime binaries.

Fine brackets cover preparation, snake visibility/encoding, recipient food and
subscription advance plus processing-flag clearing. Socket counters count binary
WebSocket sends, underlying TCP writes and `Open::flush` calls between consecutive
simulation boundaries. Per-recipient values divide these interval totals by 500;
they are averages, not every recipient's identical count. Socket work can cross
a tick boundary. These diagnostics have clock/counter overhead and cannot replace
the primary tail measurement.

`select_live.py` runs the unchanged checkpoint body/client/queue/CPU-reconciliation
and quiet-process gates on complete interior 100-tick blocks. It then reduces
the same accepted rows by maintenance cadence and analyzes the separate fine
window. `select_live_partial.py` preserves this session's full-selection failure,
reapplies unchanged gates to each whole primary window, and labels the fine
diagnostic's preflight-only analysis. Keep failures and enforce the 3,600-second live host/client/controller
CPU ceiling. The target cleanup refuses live study executables and removes only
the resolved task-owned target after all builds/windows end. Rehash all copied
binaries and generated sources before selecting/publicizing results.
