# Reproduce retained-sector layout

All builds, tests, traces, profiles, reductions and timing run on the authorized
Linux guest. Use existing Rust 1.99, Python 3.11.4+, a C compiler, binutils,
locked offline dependencies and current-thread user PMU permissions. Install
nothing; change no system settings. No affinity, native CPU targeting, SIMD,
outside game servers or sockets are used. Public host description: a 4-vCPU
Zen 5 EPYC KVM guest.

## Fresh attribution

The frozen plan is [PLAN.md](PLAN.md), committed as `b409578e` before new data.
Archive merged post-#274 main `9ad002c1819eecac97b91410321bc611b4d6dde0` as
`main.tar` in a fresh external `profile/` directory. Copy this study's three
`retained-profile-*.py` adapters there. Reuse these files directly from
[the memory diagnostic](../2026-10-29-simulation-memory/REPRODUCE.md):

- `build.py`, `run.py`, `reduce.py` as `donor-build.py`, `donor-run.py`,
  `donor-reduce.py`.
- `instrumentation.patch`, unchanged.

The donor builder reads the existing
[full-backend fixture](../2026-10-28-mature-covered-bounds/fixture.rs) from the
archive. The adapter changes only compact-history footprint accessors, the
corrected L1d sampling period and build pins. Run:

```sh
python3 retained-profile-prepare.py
```

Verify the copied executable against `build.json`, preserve that receipt and
remove the sole task-owned target `/tmp/slither-retained-layout-target`.
Commit the build pins before collecting. With the guest otherwise idle, write
`session.json` with `startedUnix`, `measurementStopUnix` (start + 6,600),
`deadlineUnix` (start + 7,200), `runtime` (the baseline above),
`frozenPlanCommit` (`b409578e`) and `attributionBuildPinsCommit` (`00b197c8`).
The actual receipt additionally records the two-second idle preflight and its
empty list of competing processes above two CPU-counter quanta. Run:

```sh
python3 retained-profile-driver.py
python3 retained-profile-reduce.py
```

Run [attribution-tables.py](attribution-tables.py) beside the selected profile,
site and counter JSON files to recompute the published scope shares and medians.
The source ranges are pinned to the generated diagnostic tree in
`attribution-build.json`, not to subsequent runtime line numbers.

The driver warms one complete backend per protocol through tick 193,000 and
forks each 1,080-tick window. It collects three counter-only and three sampled
windows for cycles, L1d and dTLB, with instructions alongside each. Actual
food-epoch changes identify the 15 maintenance ticks; the other 1,065 are
ordinary. Sampling periods are 1,000,003 / 10,007 / 1,009 respectively.
Cycle/dTLB profiles are lossless and unthrottled. L1d sampling throttled in
both protocols and remains diagnostic-only, without a retry. L1d counter-only
results remain usable. IP samples and inline stacks are approximate attribution,
not precise load-address evidence. The selected data, event controls and
[selection](SELECTION.md) precede the runtime implementation.

## Exactness and primary measurements

Archive baseline `9ad002c1819eecac97b91410321bc611b4d6dde0` as `base.tar` and
candidate `5edb5b2dd78c8d9d6ec4fb8112b9d6b1d60b943c` as `candidate.tar` in a
fresh external `primary/` directory. The production change and independent
tree-backed oracle are in the candidate archive. Copy this study's four
`retained-{check,setup-primary,prep-primary,select-primary}.py` helpers there,
and reuse [#274's harness.patch](../2026-10-29-compact-snake-history/harness.patch)
as `harness.patch` without copying it into this study.

```sh
python3 retained-setup-primary.py
python3 retained-prep-primary.py
```

The setup applies the existing helper patch, selects the one task-owned target,
replaces its prerequisite runner and adds untimed sector storage counts and
capacities to ready receipts. Reconstruct the two retained imports as described
in [#274's reproduction guide](../2026-10-29-compact-snake-history/REPRODUCE.md):
the broadcast-distance `reproduce.py` and `counter.rs`, and the mature-covered-
bounds `serve.py`, `compare.py`, `accept249.py`. Bind their external locations
in generated `build.py` and `setup.py` before running the prep driver if the
original retained directories are unavailable. This changes import locations,
not measurement code. Keep paths free of spaces.

[retained-check.py](retained-check.py) runs the focused visibility suite, then
four separate mutations: wrong outgoing timestamp, missing re-entry witness
refresh, added-deadline expiry and misaligned key/time compaction. Each must
fail at runtime with `test result: FAILED`. A `finally` restores the original
source bytes after each mutation. The restored suite passes before
`python3 tools/check.py rust`. The script writes the actual commands, expected
outcomes and exits to `checks/results.json`, then writes
`required-checks-pass.json` from those rows, the candidate archive digest and
the byte-checked source restoration. The receipt is not inherited or handwritten.

The prep driver runs those checks, builds base/candidate/radius-mutation release
binaries, compares all 27 complete catalog/host traces and requires the radius
mutation to change both full traces and host-frame payloads for both protocols.
It runs the stream-byte-flip control and documentation tests, verifies all
copied binaries and removes the target before the warm streams need that space.
Both complete warm streams match every byte through tick 193,000. Two forked
continuations per arm/protocol match before timing. Young-500 begins at zero;
1/10/50-seat controls warm for 600 ticks, omitting only the 500-seat disconnect
recipe. Every continuation contains 1,080 advances. Physical checkpoints omit
presentation lives, hidden births, held listings and sector interest: they
cannot replace these complete warmed backends.

Commit the actual primary source/build pins before timing. Notify the
maintaining agent, write `primary-timing-notice.json` as `{"unix": UNIX_SECONDS}`
and wait at least 60 seconds. With the guest otherwise idle, run:

```sh
python3 compact-time-primary.py
python3 retained-select-primary.py
```

Copy and run [select-proofs.py](select-proofs.py) in the execution directory
after the selector to collect the actual prerequisite, trace and mutation
receipts as `proofs.json`. It byte-checks the restored oracle source against the
candidate archive and requires complete warm/six-continuation proofs on all
ten cohorts.

No extra guest commands, builds, checks or reductions run during counted windows.
The reused controller records redacted full process commands at every boundary.
Each row/protocol has one discarded AB, then five AB/BA/AB/BA/AB pairs. The
same frozen parents supply every child. All four grouped user-only events must
run at least 99% of each interval. CPU/counters bracket the advance only;
policy/admission preparation, drains and serialization stay outside. Identical
wrapper overhead remains included, without subtraction. After all counted
windows, both direct parents produce complete continuations: all six streams,
packet/frame bytes, cumulative counts and endpoint summaries must match.

Use the new selector, not the older study's selector directly. It retains
integer CPU sums and applies this study's frozen gate: all ten mature whole-
window pairs improve; every short row's median paired absolute cost is at most
+10 microseconds/tick; mature maintenance median paired cost is nonpositive in
both protocols. No exclusions, outcome-driven retries or post-result tuning.
Results describe these no-socket fixtures and this guest, not live capacity.

Keep raw sources, binaries, traces, profiles, counter rows and redacted process
inventories outside Git. Before pruning task-owned files, archive them, verify
members against their source files, transfer the archive and verify its SHA-256.
Selected observations and table calculations belong beside the report; no
per-file source-hash maps or duplicated earlier harnesses are needed.

For final delivery checks, archive the completed PR checkout as
`final-delivery.tar` alongside the measured-arm archives and copy
[postchecks.py](postchecks.py) there. Run `python3 postchecks.py` after timing.
It runs the seven documentation tests, independently recomputes every selected
table and the frozen gate, and compares both final runtime files against the
measured arm. If rejected, it additionally checks the retained patch in both
directions against the two measured archives. It neither rebuilds nor retimes
the arms. `postprocessing-checks.json` records the commands and outcomes.
