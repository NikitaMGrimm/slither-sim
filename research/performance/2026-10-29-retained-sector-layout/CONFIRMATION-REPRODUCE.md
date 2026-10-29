# Reproduce the separately pre-registered confirmation

This is the maintaining agent's post-result confirmation decision, not a change
to the original five-pair study. Read [CONFIRMATION-PLAN.md](CONFIRMATION-PLAN.md),
committed as `a6dfc764` and pushed before new observations. The original plan,
results and rejected verdict remain separate; neither series is pooled.

Use the original [guest prerequisites and import binding](REPRODUCE.md).
Every build, test, trace, reduction and timed run executes on the guest. No
installation, native CPU target, affinity, precision change or outside server.
Keep the one task-owned target `/tmp/slither-retained-layout-target`.

## Fresh arms and preparation

Create a fresh external directory, archive main `b750004a` as `base.tar` and
exact candidate `9a2f3bbc26f993f0fa83da7a139cb7430dea052a` as `candidate.tar`.
The latter applies the original patch without changes; both affected runtime
files equal `5edb5b2d` byte-for-byte. Main has only release metadata and #275's
research additions beyond the original baseline. Original patch SHA-256:
`1f31fd3dc14281bafc0089e86d6e764ea90e27af66c69c003d8dd3341a339568`.

Copy the original `retained-check.py`, `retained-setup-primary.py`,
`retained-prep-primary.py`, `retained-select-primary.py` and `tables.py` from
this study. Reuse [#274's helper patch](../2026-10-29-compact-snake-history/harness.patch)
as `harness.patch`. Copy the confirmation helpers beside them:
`confirmation-setup.py`, `confirmation_background.py`, `confirmation_gate.py`,
`confirmation-controls.py`, `confirmation-select.py`, `confirmation-tables.py`
and `confirmation-preflight.py`.

```sh
python3 confirmation-setup.py
python3 confirmation-controls.py
python3 confirmation-select.py --prepare-only
python3 -m py_compile setup.py confirmation_background.py compact-cohort.py \
    compact-proof-short.py select-confirmation-generated.py confirmation-tables.py
python3 retained-prep-primary.py
```

The setup reuses the old harness and changes only the four-row selection,
ten-pair count/receipt and Python request-boundary background recorder. Rust
instrumentation, fixture, counter selectors and brackets remain unchanged.
The original prep driver now runs two mature and two young cohorts because its
generated short-case list contains only 500 seats at tick zero. The four oracle
mutations, full offline Rust group, 27 traces, two wire controls, stream-byte
control, documentation tests and complete warm/fork proofs run before timing.
It verifies copied executables and removes the target before warm streams.

The nine synthetic method controls cover 8-versus-7 improving pairs, exact
strict maintenance medians, the absolute +10 µs boundary, exclusion of the
discarded AB, rejection of five-pair input, waited-child CPU, negative residuals
and PID reuse. They are method tests, not confirmation observations.

Verify all copied binaries and generated Rust aggregates against `ARM/pins.json`;
preserve/commit `confirmation-build.json` before timing, including the aggregate
controller identity. Keep all parents at their request barriers. Actual prep
and method logs remain retained, including routine setup failures.

## Counted windows and background diagnostics

Notify the maintaining agent, write `primary-timing-notice.json` with `unix`
seconds, wait at least 60 seconds and run:

```sh
python3 confirmation-preflight.py
python3 compact-time-primary.py
python3 confirmation-select.py
```

The reused controller runs one discarded AB and ten alternating AB/BA pairs on
each of the four rows. No extra guest commands run during counted windows.
Every child advances 1,080 ticks from its frozen parent; all counter scheduling
and actual maintenance flags are checked. Direct-parent stream proofs happen
only after the controller announces `ALL COUNTED WINDOWS FINISHED`.

For each request, the server writes `LABEL-background-raw.json` and
`LABEL-background.json` outside the measured advance. Raw snapshots retain
aggregate busy/steal counters and per-process start times, own/waited CPU and
study-tree membership. The selected per-window data includes the quantum,
request wall span, busy-minus-study residual (including negative values),
visible non-study CPU, new/exited non-study process counts and an explicit
`exclusionApplied: false`. All discarded and reported windows remain retained;
background diagnostics never filter or enter the adoption decision.

The selector reuses the first adapter's metric/storage transformations and the
old grouped metric reader, but executes neither old gate. It reads all eleven
pairs per row, requires ten reported pairs in the direct-proof receipts, adds
each arm's background receipt, and writes only `confirmation-results.json`.
`confirmation_gate.py` uses exact rational paired CPU ratios and integer median
costs for the pre-registered quorum/maintenance/young tests. No original result
file is read or rewritten by this gate.

Copy `confirmation-results.json` beside the published study's unchanged
`tables.py`, then run `python3 confirmation-tables.py`. It independently checks
the new gate and metric summaries, reusing only the old rendering format.
Run `python3 tables.py` separately to recompute the original rejection. Retain
their tables in separate files; do not average the series or replace old values.

## Conditional delivery

If confirmation fails, restore the two runtime files to main and keep the
candidate rejected for good, with its original patch and both series recorded.
If it passes, include the exact candidate and rerun integrated-head checks:
the visibility oracle, four runtime mutations and restoration, 27 traces and
wire controls, full Rust group, documentation and a stock complete warm-stream
spot check through tick 193,000 with six 144-tick continuations. This reuses
[#274's adoption driver](../2026-10-29-compact-snake-history/adoption.py), the
retained visibility check runner and the same sole target. The spot check makes
no timing requests. No measured runtime source changes or new timing series.

Keep raw sources, binaries, traces, snapshots and full inventories local-only.
Stream archives off the guest with normalized owner metadata, compare transfer
digests and stream them back for member-by-member comparison before pruning.
Only after verification remove the task's own directories/target, announce the
guest free and update #277's template body/footer. Do not merge.
