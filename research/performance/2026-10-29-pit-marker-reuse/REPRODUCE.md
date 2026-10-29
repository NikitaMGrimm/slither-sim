# Reproduce exact marker-sector reuse

Current baseline `cc1bb1e9`, retained-vector implementation `46823eea`.
The original tree-based `e51e5bef` receipts remain historical, separate from
the [local amendment](PLAN-LOCAL.md). [Plan](PLAN.md), [attempts](ATTEMPTS.md).
Raw sources, binaries, logs and complete traces remain local-only.

Run marker coverage, food/listing, pit presentation and protocol-13 tests
serially with at most two Cargo jobs and one test thread, using locked/offline
dependencies. The existing full-union oracle
still compares the complete changed-hint sets and sector values. The new
coverage oracle checks disappearing/current/overlapping generations, growth,
shrink and exact/adjacent-float expiry times. Mutate
`self.affected.binary_search(&sector).is_ok()` to append `&& false`, require the coverage
test to fail, restore source in a `finally`, then require restored listing
tests and the complete Rust group to pass.

Reuse the earlier
[minimap trace preparation/matrix](https://github.com/NikitaMGrimm/slither-sim/tree/f1f426784354c9691f2c74c95a75d97d7e37c1d7/research/performance/2026-10-29-pit-minimap-cache) and
[spawn study's common fixed-work tools](https://github.com/NikitaMGrimm/slither-sim/tree/dc12d8781cd3ce1594d7d930e859c80b3f8532a1/research/performance/2026-10-29-spawn-ring-prefilter)
and adjacent Rust sources by reference, staged outside
the checkout. [prepare.py](prepare.py) changes only that builder's mutation to
drop affected-marker-sector hints. It records both earlier helper and wrapper
hashes. The shared code is not copied into this study. Use the corrected common
fixture, which explicitly enables and checks stock decorations for every live
viewer; the first failed run without that setting remains in the attempts.

```sh
python prepare.py PRIOR_SPAWN_PREPARE PRIOR_MINIMAP_PREPARE REPO OUTPUT base cc1bb1e9
python prepare.py PRIOR_SPAWN_PREPARE PRIOR_MINIMAP_PREPARE REPO OUTPUT candidate 46823eea
python prepare.py PRIOR_SPAWN_PREPARE PRIOR_MINIMAP_PREPARE REPO OUTPUT mutation 46823eea
python row_parity.py OUTPUT
python PRIOR_TRACE_MATRIX REPO OUTPUT catalog
python PRIOR_TRACE_MATRIX REPO OUTPUT host
```

[row_parity.py](row_parity.py) qualifies 80 live, decorated viewers, zero queued
admissions and at least three actual marker-radius changes over 360 counted
ticks. Both protocols retain five marker changes in the selected run. It checks
complete trace bytes and terminal initial/final world/RNG/queue state, plus the
mutation changing encoded frames in both protocols. These local debug checks
have no active timing or PMU counter. They do not estimate performance.

The shared row is synthetic fixed work: held viewer poses stay inside the pit
and an explicit radius schedule shrinks it. Pose resets and trace output remain
outside counting; real physics, food presentation/subscriptions, marker fades
and both protocol encoders remain inside. It is not naturally grown bodies,
network latency or capacity evidence. The `minimap` no-backlog and `spawn`
blocked-queue rows are separate controls, not substitutes for marker work.

## Untimed work estimate

After all trace gates pass, [count_work.py](count_work.py) clones those qualified
source trees into a fresh external directory and adds [work_counts.rs](work_counts.rs).
It verifies the original source hashes before overlaying, then builds at most
two jobs, one arm at a time. The generated source's full manifest stays private;
only its digest is selected. Clocks/PMU remain disabled and their fields must
be absent from every sample. The diagnostic binaries must never be used for
performance timing.

```sh
python -X utf8 count_work.py QUALIFIED_OUTPUT CENSUS_OUTPUT CARGO_TARGET
python work_table.py work-counts.json
```

The census runs shrinking and fixed-pit rows in both protocols, comparing all
four complete baseline/candidate byte streams. It requires distinct binaries,
unchanged nonempty-overlay counts, actual shrinking marker updates, fewer empty
lookups, and identical operation counts with zero marker updates in the fixed
control. The selected file references `local-current.json` for runtime pins;
diagnostic source/binary hashes and counter names are recorded once.

The current [qualification](local-current.json), [checks](local-checks.json) and
[counts](work-counts.json) concern the retained-vector source. The original
`correctness.json` / `verification.json` are historical tree-version receipts,
not substitutions for the revised-source gates.

## Future released-guest measurement

Only after the maintaining agent releases the SSH guest, use source-only
staging with `STUDY_STAGE_ONLY=1`, transfer and hash-verify the stages, then reuse
the spawn study's native `build_staged.py`, pair runner, counters, selectors and
quiet-context inventory. See that study's reproduction instructions for their
contracts; source manifests hash every staged file with portable path separators.
No guest access or timing occurred during this local continuation.

Run native full-byte gates before pairs:

```sh
python PRIOR_BUILD_STAGED NATIVE_OUTPUT base
python PRIOR_BUILD_STAGED NATIVE_OUTPUT candidate
python PRIOR_BUILD_STAGED NATIVE_OUTPUT mutation
python row_parity.py NATIVE_OUTPUT
python PRIOR_TRACE_MATRIX REPO NATIVE_OUTPUT catalog
python PRIOR_TRACE_MATRIX REPO NATIVE_OUTPUT host
python write_gates.py PRIOR_GATE_WRITER NATIVE_OUTPUT local-current.json local-checks.json
python PRIOR_RUN_PAIRS NATIVE_OUTPUT PRIOR_REPRODUCE 19 markers
python PRIOR_RUN_PAIRS NATIVE_OUTPUT PRIOR_REPRODUCE 13 markers
python PRIOR_TABLES NATIVE_OUTPUT markers
```

The writer reuses the prior gate checks against this study's shrinking-row
results, archive identities and completed Rust receipt; it does not invent a
success flag. Repeated-fork/direct-parent proof traces must agree in full.
One discarded AB warmup precedes five AB/BA/AB/BA/AB instructions:u pairs per
protocol. Pin host children to one core, keep the controller on another,
require >=99% counter scheduling, finish builds/checks first and retain full
private inventories before/after counting. Keep every result, failure and
whole-byte proof. Report marker, food epoch, minimap and slowest-CPU-1% groups
separately, paired instruction median/range and CPU p50/p99/max. No performance
or live capacity claim follows from the current correctness results alone.
Also inspect post-marker ticks (the preceding tick has a marker event), so
tracking cleanup cannot disappear into the unlabelled remainder. When source
pins are unchanged, reference the existing qualification instead of repeating
its manifest/archive/preparation identities in another evidence file.
